//! Decides what one CLI invocation does. Pure: the shell gathers [`Inputs`],
//! this module returns an [`Invocation`], and the shell runs it.

use std::path::{Path, PathBuf};

use agent_notify_core::fallback::{FallbackNote, fallback_note};
use agent_notify_core::hook::{ClaudeHookInput, HookDecision, TagWrite, decide_hook};
use agent_notify_core::wire::{AgentEvent, AgentKind, Kind, Origin, RepoInfo, Request, WIRE_VERSION, WindowLabel};
use agent_notify_core::{DisplayText, EpochMs, EventId, PaneKey, PromptId, SessionId};

use crate::cli::{Command, HookAgent};

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
    pub log: Option<String>,
    /// A problem with the input that did not stop the send, for the log at warn.
    pub warning: Option<String>,
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
    fn nothing(reason: String) -> Self {
        Invocation { tags: Vec::new(), request: None, fallback: None, exit: Exit::Success, stderr: None, log: Some(reason), warning: None }
    }

    fn usage(message: &str) -> Self {
        Invocation { tags: Vec::new(), request: None, fallback: None, exit: Exit::Usage, stderr: Some(message.to_string()), log: Some(message.to_string()), warning: None }
    }

    fn agent(inputs: &Inputs, origin: Origin, event: AgentEvent, tags: Vec<TagWrite>) -> Self {
        let fallback = fallback_note(&origin, &event);
        let request = Request::Agent { v: WIRE_VERSION, event_id: inputs.event_id.clone(), sent_at_ms: inputs.now, origin, event };
        Invocation { tags, request: Some(request), fallback, exit: Exit::Success, stderr: None, log: None, warning: None }
    }
}

/// Decides what `command` does with `inputs`.
#[must_use]
pub fn plan_invocation(command: &Command, inputs: &Inputs) -> Invocation {
    match command {
        Command::Hook { agent: HookAgent::ClaudeCode { .. } } => plan_hook(inputs),
        Command::Send { kind, body } => plan_send(Kind::from(*kind), body, inputs),
        Command::Quiet { until, max_minutes, off } => plan_quiet(until.map(Into::into), *max_minutes, *off, inputs),
    }
}

fn plan_hook(inputs: &Inputs) -> Invocation {
    let Some(raw) = inputs.stdin.as_deref().filter(|raw| !raw.trim().is_empty()) else {
        return Invocation::nothing("no hook payload on stdin".to_string());
    };
    let payload: ClaudeHookInput = match serde_json::from_str(raw) {
        Ok(payload) => payload,
        Err(error) => return Invocation::nothing(format!("hook payload did not parse: {error}")),
    };
    let pane = found(inputs);
    match decide_hook(&payload, pane.and_then(|facts| facts.session_tag.as_ref())) {
        HookDecision::Ignore { reason } => Invocation::nothing(reason),
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
        assert!(invocation.log.as_deref().is_some_and(|reason| reason.contains("session id")));
    }

    #[test]
    fn an_ignored_hook_event_logs_its_reason() {
        let invocation = plan_invocation(&hook(), &inputs(PaneLookup::NotInTmux, Some(&payload("PreCompact", ""))));
        assert!(invocation.request.is_none());
        assert!(invocation.log.as_deref().is_some_and(|reason| reason.contains("PreCompact")));
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

    proptest! {
        #[test]
        fn any_stdin_plans_without_panicking_and_exits_zero(stdin in proptest::collection::vec(any::<u8>(), 0..2000)) {
            let text = String::from_utf8_lossy(&stdin).into_owned();
            let invocation = plan_invocation(&hook(), &inputs(PaneLookup::NotInTmux, Some(&text)));
            prop_assert_eq!(invocation.exit, Exit::Success);
        }
    }
}
