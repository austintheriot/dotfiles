//! Decides what one CLI invocation does. Pure: the shell gathers [`Inputs`],
//! this module returns an [`Invocation`], and the shell runs it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use agent_notify_core::fallback::{FallbackNote, fallback_note};
use agent_notify_core::hook::{ClaudeHookInput, HookDecision, TagWrite, decide_hook};
use agent_notify_core::wire::{AgentEvent, AgentKind, Control, DndCommand, Kind, Origin, Query, RepoInfo, Request, WIRE_VERSION, WindowLabel};
use agent_notify_core::{DisplayText, EpochMs, EventId, PaneId, PaneKey, PromptId, ServerId, SessionId, SocketPath};

use crate::cli::{Command, DndAction, FocusDirection, HookAgent};
use crate::client::{BUDGET, QUERY_BUDGET};

/// Everything the planner reads.
#[derive(Debug, Clone)]
pub struct Inputs {
    /// The clock, read once.
    pub now: EpochMs,
    /// This invocation's id.
    pub event_id: EventId,
    /// The process working directory.
    pub cwd: PathBuf,
    /// What the tmux read found.
    pub pane: PaneLookup,
    /// Git facts, read only outside tmux.
    pub repo: Option<RepoInfo>,
    /// The hook payload, for `hook` only.
    pub stdin: Option<String>,
}

/// The result of reading the current tmux pane.
#[derive(Debug, Clone)]
pub enum PaneLookup {
    /// `$TMUX` or `$TMUX_PANE` is unset.
    NotInTmux,
    /// The pane and its tags.
    Found(PaneFacts),
    /// Inside tmux, but the read failed. The request goes without pane facts.
    Unreadable {
        /// For the log.
        reason: String,
    },
}

/// One pane and its agent-notify tags.
#[derive(Debug, Clone)]
pub struct PaneFacts {
    /// The pane.
    pub key: PaneKey,
    /// Its window.
    pub window: WindowLabel,
    /// `@agent_notify_session`.
    pub session_tag: Option<SessionId>,
    /// `@agent_notify_prompt`.
    pub prompt_tag: Option<PromptId>,
}

/// What the shell does.
#[derive(Debug, Clone)]
pub struct Invocation {
    /// Pane tag writes, before the request.
    pub tags: Vec<TagWrite>,
    /// The request to send.
    pub request: Option<Request>,
    /// Shown when the daemon does not acknowledge.
    pub fallback: Option<FallbackNote>,
    /// The exit status.
    pub exit: Exit,
    /// One line for stderr.
    pub stderr: Option<String>,
    /// Why nothing was sent, for the log.
    pub log: Option<NothingSent>,
    /// A problem with the input that did not stop the send, for the log at warn.
    pub warning: Option<String>,
    /// What to do with the answer.
    pub output: Output,
}

/// What the shell does with the daemon's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// Show the fallback notification when the daemon does not acknowledge.
    FallbackOnFailure,
    /// Print nothing, whatever the daemon does. tmux shows any output of a hook command.
    Silent,
    /// Print the line on success. Exit 1 with a message when the daemon does not answer.
    Confirm(Option<&'static str>),
    /// Print the status report.
    Status(StatusFormat),
    /// Print whether do-not-disturb is on.
    DndStatus,
}

impl Output {
    /// How long the CLI waits for the daemon.
    pub fn budget(self) -> Duration {
        if self.is_query() { QUERY_BUDGET } else { BUDGET }
    }

    /// True when the answer is a read of daemon state. `status --tmux` runs
    /// about once a second and `cli.log` never rotates, so a query logs at debug.
    pub fn is_query(self) -> bool {
        matches!(self, Output::Status(_) | Output::DndStatus)
    }
}

/// How `status` prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFormat {
    /// For a person.
    Text,
    /// The report as JSON.
    Json,
    /// A tmux status-line segment.
    Tmux,
}

/// Why an invocation sent nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NothingSent {
    /// A fixed phrase with no input values in it, for the log at info.
    pub reason: &'static str,
    /// The payload's session, whenever the hook payload parsed.
    pub session_id: Option<SessionId>,
    /// Detail that can carry hook payload values, for the log at debug.
    pub detail: Option<String>,
}

/// The exit status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// 0.
    Success,
    /// 2.
    Usage,
}

impl Invocation {
    fn nothing(reason: &'static str, session_id: Option<SessionId>, detail: Option<String>) -> Self {
        let log = Some(NothingSent { reason, session_id, detail });
        Invocation { tags: Vec::new(), request: None, fallback: None, exit: Exit::Success, stderr: None, log, warning: None, output: Output::FallbackOnFailure }
    }

    fn usage(message: &'static str) -> Self {
        let log = Some(NothingSent { reason: message, session_id: None, detail: None });
        Invocation {
            tags: Vec::new(),
            request: None,
            fallback: None,
            exit: Exit::Usage,
            stderr: Some(message.to_string()),
            log,
            warning: None,
            output: Output::FallbackOnFailure,
        }
    }

    fn agent(inputs: &Inputs, origin: Origin, event: AgentEvent, tags: Vec<TagWrite>) -> Self {
        let fallback = fallback_note(&origin, &event);
        let request = Request::Agent { v: WIRE_VERSION, event_id: inputs.event_id.clone(), sent_at_ms: inputs.now, origin, event };
        Invocation { tags, request: Some(request), fallback, exit: Exit::Success, stderr: None, log: None, warning: None, output: Output::FallbackOnFailure }
    }

    fn control(inputs: &Inputs, control: Control, output: Output) -> Self {
        let request = Request::Control { v: WIRE_VERSION, event_id: inputs.event_id.clone(), sent_at_ms: inputs.now, control };
        Invocation { tags: Vec::new(), request: Some(request), fallback: None, exit: Exit::Success, stderr: None, log: None, warning: None, output }
    }

    fn query(inputs: &Inputs, query: Query, output: Output) -> Self {
        let request = Request::Query { v: WIRE_VERSION, event_id: inputs.event_id.clone(), query };
        Invocation { tags: Vec::new(), request: Some(request), fallback: None, exit: Exit::Success, stderr: None, log: None, warning: None, output }
    }
}

/// Decides what `command` does with `inputs`.
#[must_use]
pub fn plan_invocation(command: &Command, inputs: &Inputs) -> Invocation {
    match command {
        Command::Hook { agent: HookAgent::ClaudeCode { .. } } => plan_hook(inputs),
        Command::Send { kind, body } => plan_send(Kind::from(*kind), body, inputs),
        Command::Quiet { until, max_minutes, off } => plan_quiet(until.map(Into::into), *max_minutes, *off, inputs),
        Command::FocusChanged { socket, pid, start_time, pane, direction } => plan_focus_changed(socket, *pid, *start_time, pane, *direction, inputs),
        Command::Dnd { action } => plan_dnd(*action, inputs),
        Command::Jump => Invocation::control(inputs, Control::Jump, Output::Confirm(None)),
        Command::Status { json, tmux } => {
            let format = match (json, tmux) {
                (true, _) => StatusFormat::Json,
                (false, true) => StatusFormat::Tmux,
                (false, false) => StatusFormat::Text,
            };
            Invocation::query(inputs, Query::Status, Output::Status(format))
        }
        Command::Doctor => Invocation::nothing("doctor gathers its own facts in main", None, None),
    }
}

fn plan_focus_changed(socket: &Path, pid: u32, start_time: i64, pane: &str, direction: FocusDirection, inputs: &Inputs) -> Invocation {
    let (Ok(socket_path), Ok(pane)) = (SocketPath::parse(socket), PaneId::parse(pane)) else {
        return Invocation { output: Output::Silent, ..Invocation::usage("focus-changed needs an absolute --socket and a %pane id") };
    };
    let pane = PaneKey { server: ServerId { socket_path, pid, start_time }, pane };
    let focused = matches!(direction, FocusDirection::In);
    Invocation::control(inputs, Control::FocusChanged { pane, focused }, Output::Silent)
}

fn plan_dnd(action: DndAction, inputs: &Inputs) -> Invocation {
    match action {
        DndAction::On { for_minutes } => Invocation::control(
            inputs,
            Control::Dnd { command: DndCommand::On { for_minutes } },
            Output::Confirm(Some("do-not-disturb is on")),
        ),
        DndAction::Off => {
            Invocation::control(inputs, Control::Dnd { command: DndCommand::Off }, Output::Confirm(Some("do-not-disturb is off")))
        }
        DndAction::Status => Invocation::query(inputs, Query::Status, Output::DndStatus),
    }
}

fn plan_hook(inputs: &Inputs) -> Invocation {
    let Some(raw) = inputs.stdin.as_deref().filter(|raw| !raw.trim().is_empty()) else {
        return Invocation::nothing("no hook payload on stdin", None, None);
    };
    let payload: ClaudeHookInput = match serde_json::from_str(raw) {
        Ok(payload) => payload,
        Err(error) => return Invocation::nothing("hook payload did not parse", None, Some(error.to_string())),
    };
    let pane = found(inputs);
    match decide_hook(&payload, pane.and_then(|facts| facts.session_tag.as_ref())) {
        HookDecision::Ignore { reason, detail } => Invocation::nothing(reason.phrase(), Some(payload.session_id.clone()), Some(detail)),
        HookDecision::Send { tags, event } => {
            let origin = origin(inputs, Some(payload.session_id.clone()), payload.prompt(), payload.session_title.clone(), payload.cwd.clone());
            let tags = if pane.is_some() { tags } else { Vec::new() };
            let warning = payload.has_invalid_prompt_id().then(|| "hook payload field prompt_id has an invalid format; sent without a prompt".to_string());
            Invocation { warning, ..Invocation::agent(inputs, origin, event, tags) }
        }
    }
}

fn plan_send(kind: Kind, body: &str, inputs: &Inputs) -> Invocation {
    let body = DisplayText::new(body);
    if body.is_blank() {
        return Invocation::usage("--body is empty after removing control and invisible characters");
    }
    let (session, prompt) = tags(inputs);
    let origin = origin(inputs, session, prompt, None, inputs.cwd.clone());
    Invocation::agent(inputs, origin, AgentEvent::Explicit { kind, body }, Vec::new())
}

fn plan_quiet(until: Option<agent_notify_core::wire::QuietUntil>, max_minutes: u16, off: bool, inputs: &Inputs) -> Invocation {
    let (Some(session), prompt) = tags(inputs) else {
        return Invocation::usage("quiet needs a tmux pane with a Claude session tag: $TMUX_PANE is unset, or no SessionStart hook tagged this pane");
    };
    let event = match (off, until) {
        (true, _) => AgentEvent::QuietOff,
        (false, Some(until)) => AgentEvent::Quiet { until, max_minutes },
        (false, None) => return Invocation::usage("quiet needs --until or --off"),
    };
    let origin = origin(inputs, Some(session), prompt, None, inputs.cwd.clone());
    Invocation { fallback: None, ..Invocation::agent(inputs, origin, event, Vec::new()) }
}

fn found(inputs: &Inputs) -> Option<&PaneFacts> {
    match &inputs.pane {
        PaneLookup::Found(facts) => Some(facts),
        PaneLookup::NotInTmux | PaneLookup::Unreadable { .. } => None,
    }
}

fn tags(inputs: &Inputs) -> (Option<SessionId>, Option<PromptId>) {
    found(inputs).map_or((None, None), |facts| (facts.session_tag.clone(), facts.prompt_tag.clone()))
}

fn origin(inputs: &Inputs, session: Option<SessionId>, prompt: Option<PromptId>, session_title: Option<DisplayText>, cwd: PathBuf) -> Origin {
    let pane = found(inputs);
    Origin {
        agent: AgentKind::ClaudeCode,
        session,
        prompt,
        pane: pane.map(|facts| facts.key.clone()),
        window: pane.map(|facts| facts.window.clone()),
        repo: inputs.repo.clone(),
        session_title,
        cwd: lossy(&cwd),
    }
}

/// The wire field is a JSON string, so a non-UTF-8 directory would make the
/// request fail to encode and lose every event but the fallback ones.
fn lossy(cwd: &Path) -> PathBuf {
    PathBuf::from(cwd.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{HookAgent, KindArg, UntilArg};
    use agent_notify_core::hook::TagWrite;
    use agent_notify_core::wire::{AgentEvent, MAX_LINE_BYTES, QuietUntil, encode_line};
    use agent_notify_core::{DisplayText, PaneId, ServerId, SocketPath};
    use proptest::prelude::*;

    fn pane_key() -> PaneKey {
        PaneKey {
            server: ServerId { socket_path: SocketPath::parse(Path::new("/tmp/tmux-501/default")).expect("absolute"), pid: 1799, start_time: 1 },
            pane: PaneId::parse("%12").expect("pane"),
        }
    }

    fn found(session_tag: Option<&str>, prompt_tag: Option<&str>) -> PaneLookup {
        PaneLookup::Found(PaneFacts {
            key: pane_key(),
            window: WindowLabel { index: 4, name: DisplayText::new("code/feature-x") },
            session_tag: session_tag.map(|tag| SessionId::parse(tag).expect("id")),
            prompt_tag: prompt_tag.map(|tag| PromptId::parse(tag).expect("id")),
        })
    }

    fn inputs(pane: PaneLookup, stdin: Option<&str>) -> Inputs {
        Inputs {
            now: EpochMs(1_000),
            event_id: EventId::from_parts(EpochMs(1_000), [7; 10]),
            cwd: PathBuf::from("/home/dev/code"),
            pane,
            repo: None,
            stdin: stdin.map(str::to_string),
        }
    }

    fn hook() -> Command {
        Command::Hook { agent: HookAgent::ClaudeCode { dry_run: false } }
    }

    fn payload(event: &str, extra: &str) -> String {
        format!(r#"{{"session_id":"session-a","prompt_id":"prompt-1","cwd":"/home/dev/code","hook_event_name":"{event}"{extra}}}"#)
    }

    fn agent_parts(invocation: &Invocation) -> (&Origin, &AgentEvent) {
        match &invocation.request {
            Some(Request::Agent { origin, event, .. }) => (origin, event),
            other => panic!("expected an agent request, got {other:?}"),
        }
    }

    #[test]
    fn session_start_in_tmux_binds_the_tag_and_sends() {
        let invocation = plan_invocation(&hook(), &inputs(found(None, None), Some(&payload("SessionStart", r#","source":"startup","session_title":"Fix sync""#))));
        assert_eq!(invocation.tags, vec![TagWrite::BindSession(SessionId::parse("session-a").expect("id"))]);
        let (origin, _) = agent_parts(&invocation);
        assert_eq!(origin.pane, Some(pane_key()));
        assert_eq!(origin.session_title, Some(DisplayText::new("Fix sync")));
        assert_eq!(invocation.exit, Exit::Success);
    }

    #[test]
    fn a_hook_outside_tmux_sends_without_tags() {
        let invocation = plan_invocation(&hook(), &inputs(PaneLookup::NotInTmux, Some(&payload("SessionStart", r#","source":"startup""#))));
        assert!(invocation.tags.is_empty());
        assert_eq!(agent_parts(&invocation).0.pane, None);
    }

    #[test]
    fn an_unreadable_pane_still_sends_without_tags() {
        let invocation = plan_invocation(&hook(), &inputs(PaneLookup::Unreadable { reason: "no server".into() }, Some(&payload("Stop", ""))));
        assert!(invocation.tags.is_empty());
        assert!(invocation.request.is_some());
    }

    #[test]
    fn session_end_unbinds_only_its_own_tag() {
        let ending = payload("SessionEnd", r#","reason":"clear""#);
        assert_eq!(plan_invocation(&hook(), &inputs(found(Some("session-a"), None), Some(&ending))).tags, vec![TagWrite::UnbindSession]);
        assert!(plan_invocation(&hook(), &inputs(found(Some("session-b"), None), Some(&ending))).tags.is_empty());
    }

    #[test]
    fn a_permission_prompt_carries_a_fallback_and_stop_does_not() {
        let permission = plan_invocation(&hook(), &inputs(found(None, None), Some(&payload("Notification", r#","notification_type":"permission_prompt","message":"Allow Bash?""#))));
        assert_eq!(permission.fallback.map(|note| note.message), Some("Allow Bash?".to_string()));
        let stop = plan_invocation(&hook(), &inputs(found(None, None), Some(&payload("Stop", r#","last_assistant_message":"Done.""#))));
        assert_eq!(stop.fallback, None);
    }

    #[test]
    fn an_invalid_prompt_id_still_sends_the_permission_prompt_without_a_prompt() {
        let stdin = r#"{"session_id":"session-a","prompt_id":"a:b.c","cwd":"/home/dev/code","hook_event_name":"Notification","notification_type":"permission_prompt","message":"Allow Bash?"}"#;
        let invocation = plan_invocation(&hook(), &inputs(found(None, None), Some(stdin)));
        let (origin, event) = agent_parts(&invocation);
        assert!(matches!(event, AgentEvent::NeedsInput { .. }), "{event:?}");
        assert_eq!(origin.prompt, None);
        assert_eq!(invocation.fallback.as_ref().map(|note| note.message.as_str()), Some("Allow Bash?"));
        let warning = invocation.warning.as_deref().expect("a warning for the log");
        assert!(warning.contains("prompt_id"), "{warning:?}");
        assert!(!warning.contains("a:b.c"), "the warning must not carry the value: {warning:?}");
    }

    #[test]
    fn a_valid_prompt_id_gives_no_warning() {
        let invocation = plan_invocation(&hook(), &inputs(found(None, None), Some(&payload("Stop", ""))));
        assert_eq!(invocation.warning, None);
        assert_eq!(agent_parts(&invocation).0.prompt.as_ref().map(PromptId::as_str), Some("prompt-1"));
    }

    #[test]
    fn a_same_session_start_keeps_the_prompt_tag() {
        let compact = payload("SessionStart", r#","source":"compact""#);
        let invocation = plan_invocation(&hook(), &inputs(found(Some("session-a"), Some("prompt-1")), Some(&compact)));
        assert!(invocation.tags.is_empty(), "{:?}", invocation.tags);
        assert!(invocation.request.is_some());
        let other = plan_invocation(&hook(), &inputs(found(Some("session-b"), Some("prompt-1")), Some(&compact)));
        assert_eq!(other.tags, vec![TagWrite::BindSession(SessionId::parse("session-a").expect("id"))]);
    }

    #[test]
    fn an_ignored_hook_logs_its_own_reason_and_the_session() {
        let cases = [
            (payload("PreCompact", ""), "hook event not mapped"),
            (payload("Notification", r#","notification_type":"idle_prompt""#), "notification type not mapped"),
        ];
        for (stdin, reason) in cases {
            let invocation = plan_invocation(&hook(), &inputs(PaneLookup::NotInTmux, Some(&stdin)));
            assert!(invocation.request.is_none(), "{stdin}");
            let log = invocation.log.expect("a nothing-sent line");
            assert_eq!(log.reason, reason, "{stdin}");
            assert_eq!(log.session_id.as_ref().map(SessionId::as_str), Some("session-a"), "{stdin}");
        }
    }

    #[test]
    fn an_unparsed_payload_logs_no_session() {
        let invocation = plan_invocation(&hook(), &inputs(PaneLookup::NotInTmux, Some("not json")));
        assert_eq!(invocation.log.and_then(|log| log.session_id), None);
    }

    #[test]
    fn bad_hook_input_sends_nothing_and_exits_zero() {
        for stdin in [None, Some(""), Some("not json"), Some(r#"{"hook_event_name":"Stop"}"#)] {
            let invocation = plan_invocation(&hook(), &inputs(PaneLookup::NotInTmux, stdin));
            assert!(invocation.request.is_none(), "{stdin:?}");
            assert_eq!(invocation.exit, Exit::Success);
            assert!(invocation.log.is_some(), "the reason must be logged for {stdin:?}");
        }
    }

    #[test]
    fn malformed_session_id_sends_nothing() {
        let stdin = r#"{"session_id":"a b;rm -rf","cwd":"/x","hook_event_name":"Stop"}"#;
        let invocation = plan_invocation(&hook(), &inputs(PaneLookup::NotInTmux, Some(stdin)));
        assert!(invocation.request.is_none());
        assert_eq!(invocation.exit, Exit::Success);
        let note = invocation.log.expect("a log note");
        assert_eq!(note.reason, "hook payload did not parse");
        assert!(note.detail.is_some_and(|detail| detail.contains("session id")));
    }

    #[test]
    fn an_ignored_hook_event_logs_its_reason() {
        let invocation = plan_invocation(&hook(), &inputs(PaneLookup::NotInTmux, Some(&payload("PreCompact", ""))));
        assert!(invocation.request.is_none());
        let note = invocation.log.expect("a log note");
        assert!(!note.reason.contains("PreCompact"), "the info reason must not carry payload values");
        assert!(note.detail.is_some_and(|detail| detail.contains("PreCompact")));
    }

    #[test]
    fn send_reads_session_and_prompt_from_the_pane_tags() {
        let command = Command::Send { kind: KindArg::Attention, body: "CI is red".into() };
        let invocation = plan_invocation(&command, &inputs(found(Some("session-a"), Some("prompt-9")), None));
        let (origin, event) = agent_parts(&invocation);
        assert_eq!(origin.session.as_ref().map(SessionId::as_str), Some("session-a"));
        assert_eq!(origin.prompt.as_ref().map(PromptId::as_str), Some("prompt-9"));
        assert_eq!(*event, AgentEvent::Explicit { kind: Kind::Attention, body: DisplayText::new("CI is red") });
        assert!(invocation.fallback.is_some_and(|note| note.urgent));
    }

    #[test]
    fn a_non_utf8_cwd_still_gives_a_request_that_encodes() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let mut odd = inputs(PaneLookup::NotInTmux, None);
        odd.cwd = PathBuf::from(OsStr::from_bytes(b"/home/dev/caf\xe9"));
        let command = Command::Send { kind: KindArg::Info, body: "Report ready".into() };
        let request = plan_invocation(&command, &odd).request.expect("a request");
        let line = encode_line(&request).expect("a lossy cwd encodes");
        assert!(line.contains("/home/dev/caf\u{FFFD}"), "{line}");
    }

    #[test]
    fn send_outside_tmux_has_no_session() {
        let command = Command::Send { kind: KindArg::Info, body: "Report ready".into() };
        let invocation = plan_invocation(&command, &inputs(PaneLookup::NotInTmux, None));
        assert_eq!(agent_parts(&invocation).0.session, None);
    }

    #[test]
    fn a_blank_body_is_a_usage_error() {
        let command = Command::Send { kind: KindArg::Info, body: " \u{200B} ".into() };
        let invocation = plan_invocation(&command, &inputs(found(Some("session-a"), None), None));
        assert_eq!(invocation.exit, Exit::Usage);
        assert!(invocation.request.is_none());
        assert!(invocation.stderr.as_deref().is_some_and(|message| message.contains("--body")));
    }

    #[test]
    fn quiet_needs_a_tagged_pane() {
        let command = Command::Quiet { until: Some(UntilArg::NextSend), max_minutes: 120, off: false };
        for pane in [PaneLookup::NotInTmux, found(None, None)] {
            let invocation = plan_invocation(&command, &inputs(pane, None));
            assert_eq!(invocation.exit, Exit::Usage);
            assert!(invocation.stderr.is_some());
        }
        let tagged = plan_invocation(&command, &inputs(found(Some("session-a"), None), None));
        assert_eq!(*agent_parts(&tagged).1, AgentEvent::Quiet { until: QuietUntil::NextSend, max_minutes: 120 });
        assert_eq!(tagged.fallback, None);
    }

    #[test]
    fn quiet_off() {
        let command = Command::Quiet { until: None, max_minutes: 120, off: true };
        let invocation = plan_invocation(&command, &inputs(found(Some("session-a"), None), None));
        assert_eq!(*agent_parts(&invocation).1, AgentEvent::QuietOff);
    }

    #[test]
    fn largest_possible_request_fits_one_line() {
        let big = "\"\\".repeat(4096);
        let mut facts = match found(Some("session-a"), Some("prompt-1")) {
            PaneLookup::Found(facts) => facts,
            _ => unreachable!(),
        };
        facts.window.name = DisplayText::new(&big);
        let mut maximal = inputs(PaneLookup::Found(facts), None);
        maximal.repo = Some(RepoInfo {
            main_repo_name: DisplayText::new(&big),
            head: agent_notify_core::wire::RepoHead::Branch { name: DisplayText::new(&big) },
        });
        let payload = serde_json::json!({
            "session_id": "session-a",
            "prompt_id": "prompt-1",
            "cwd": format!("/{}", "\"".repeat(4095)),
            "hook_event_name": "Stop",
            "session_title": big,
            "last_assistant_message": big,
        });
        maximal.stdin = Some(payload.to_string());
        let invocation = plan_invocation(&hook(), &maximal);
        let request = invocation.request.expect("a request");
        let line = encode_line(&request).expect("encodes");
        assert!(line.len() <= MAX_LINE_BYTES, "a maximal request is {} bytes", line.len());
    }

    fn bare_inputs() -> Inputs {
        Inputs {
            now: EpochMs(5),
            event_id: EventId::from_parts(EpochMs(5), [0; 10]),
            cwd: PathBuf::from("/"),
            pane: PaneLookup::NotInTmux,
            repo: None,
            stdin: None,
        }
    }

    fn focus_changed(pane: &str, socket: &str, direction: FocusDirection) -> Command {
        Command::FocusChanged { socket: PathBuf::from(socket), pid: 77, start_time: 1_700_000_000, pane: pane.to_string(), direction }
    }

    #[test]
    fn focus_changed_builds_a_silent_control_with_the_server_identity() {
        let invocation = plan_invocation(&focus_changed("%3", "/tmp/tmux-501/default", FocusDirection::In), &bare_inputs());
        assert_eq!(invocation.output, Output::Silent);
        let Some(Request::Control { control: Control::FocusChanged { pane, focused }, .. }) = &invocation.request else {
            panic!("a focus control: {invocation:?}");
        };
        assert!(*focused);
        assert_eq!(pane.pane.as_str(), "%3");
        assert_eq!(pane.server.socket_path.as_path(), Path::new("/tmp/tmux-501/default"));
        assert_eq!((pane.server.pid, pane.server.start_time), (77, 1_700_000_000));
        assert!(invocation.fallback.is_none());
    }

    #[test]
    fn focus_out_sends_focused_false() {
        let invocation = plan_invocation(&focus_changed("%3", "/tmp/s", FocusDirection::Out), &bare_inputs());
        assert!(matches!(invocation.request, Some(Request::Control { control: Control::FocusChanged { focused: false, .. }, .. })));
    }

    #[test]
    fn focus_changed_rejects_a_bad_pane_or_relative_socket() {
        for command in [focus_changed("3", "/tmp/s", FocusDirection::In), focus_changed("%3", "relative/s", FocusDirection::Out)] {
            let invocation = plan_invocation(&command, &bare_inputs());
            assert_eq!(invocation.exit, Exit::Usage, "{command:?}");
            assert!(invocation.request.is_none());
            assert_eq!(invocation.output, Output::Silent);
        }
    }

    #[test]
    fn dnd_and_jump_build_confirmed_controls() {
        let on = plan_invocation(&Command::Dnd { action: DndAction::On { for_minutes: Some(30) } }, &bare_inputs());
        assert!(matches!(
            on.request,
            Some(Request::Control { control: Control::Dnd { command: DndCommand::On { for_minutes: Some(30) } }, .. })
        ));
        assert_eq!(on.output, Output::Confirm(Some("do-not-disturb is on")));
        let off = plan_invocation(&Command::Dnd { action: DndAction::Off }, &bare_inputs());
        assert!(matches!(off.request, Some(Request::Control { control: Control::Dnd { command: DndCommand::Off }, .. })));
        assert_eq!(off.output, Output::Confirm(Some("do-not-disturb is off")));
        let jump = plan_invocation(&Command::Jump, &bare_inputs());
        assert!(matches!(jump.request, Some(Request::Control { control: Control::Jump, .. })));
        assert_eq!(jump.output, Output::Confirm(None));
    }

    #[test]
    fn status_and_dnd_status_are_queries() {
        let cases = [
            (Command::Status { json: false, tmux: true }, Output::Status(StatusFormat::Tmux)),
            (Command::Status { json: true, tmux: false }, Output::Status(StatusFormat::Json)),
            (Command::Status { json: false, tmux: false }, Output::Status(StatusFormat::Text)),
            (Command::Dnd { action: DndAction::Status }, Output::DndStatus),
        ];
        for (command, output) in cases {
            let invocation = plan_invocation(&command, &bare_inputs());
            assert!(matches!(invocation.request, Some(Request::Query { query: Query::Status, .. })), "{command:?}");
            assert_eq!(invocation.output, output, "{command:?}");
        }
    }

    #[test]
    fn queries_get_the_longer_budget_and_everything_else_the_hook_budget() {
        for output in [Output::Status(StatusFormat::Tmux), Output::Status(StatusFormat::Json), Output::Status(StatusFormat::Text), Output::DndStatus] {
            assert_eq!(output.budget(), QUERY_BUDGET, "{output:?}");
        }
        for output in [Output::FallbackOnFailure, Output::Silent, Output::Confirm(None), Output::Confirm(Some("x"))] {
            assert_eq!(output.budget(), BUDGET, "{output:?}");
        }
        assert!(QUERY_BUDGET > BUDGET);
    }

    #[test]
    fn agent_commands_keep_the_fallback_output() {
        let command = Command::Send { kind: KindArg::Info, body: "Report ready".into() };
        assert_eq!(plan_invocation(&command, &inputs(PaneLookup::NotInTmux, None)).output, Output::FallbackOnFailure);
        assert_eq!(plan_invocation(&hook(), &inputs(PaneLookup::NotInTmux, Some(&payload("Stop", "")))).output, Output::FallbackOnFailure);
    }

    #[test]
    fn only_agent_commands_read_the_origin() {
        assert!(!Command::Jump.reads_origin());
        assert!(!Command::Doctor.reads_origin());
        assert!(!Command::Status { json: false, tmux: true }.reads_origin());
        assert!(!Command::Dnd { action: DndAction::Off }.reads_origin());
        assert!(!focus_changed("%1", "/tmp/s", FocusDirection::In).reads_origin());
        assert!(Command::Send { kind: KindArg::Info, body: "x".to_string() }.reads_origin());
        assert!(hook().reads_origin());
        assert!(Command::Quiet { until: None, max_minutes: 120, off: true }.reads_origin());
    }

    proptest! {
        #[test]
        fn any_stdin_plans_without_panicking_and_exits_zero(stdin in proptest::collection::vec(any::<u8>(), 0..2000)) {
            let text = String::from_utf8_lossy(&stdin).into_owned();
            let invocation = plan_invocation(&hook(), &inputs(PaneLookup::NotInTmux, Some(&text)));
            prop_assert_eq!(invocation.exit, Exit::Success);
        }
    }
}
