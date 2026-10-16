#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
//! The agent-notify CLI. Hooks and agents call it. It tags tmux panes, sends
//! one request to the daemon, and falls back to a plain OS notification.
//! `hook` exits 0 whatever happens: a `Stop` hook that exits 2 blocks the stop.

mod child;
mod cli;
mod client;
mod context;
mod fallback;
mod invocation;
mod log;
mod tmux;

use std::ffi::OsString;
use std::io::{self, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::ExitCode;

use agent_notify_core::paths::{Platform, SocketLocation, log_directory, socket_location};
use agent_notify_core::SessionId;
use agent_notify_core::wire::{AgentEvent, Request, encode_line};
use clap::Parser;
use clap::error::ErrorKind;

use crate::cli::{Cli, Command};
use crate::client::Delivery;
use crate::context::EnvSnapshot;
use crate::invocation::{Exit, Inputs, Invocation, PaneLookup};

fn main() -> ExitCode {
    let arguments: Vec<OsString> = std::env::args_os().collect();
    if arguments.get(1).is_some_and(|argument| argument == "--stamp") {
        // config-build embeds the stamp and config doctor probes it.
        let _ = writeln!(io::stdout(), "{}", option_env!("CONFIG_MANIFEST_STAMP").unwrap_or("unstamped"));
        return ExitCode::SUCCESS;
    }
    let is_hook = arguments.get(1).is_some_and(|argument| argument == "hook");
    let env = EnvSnapshot::read();
    let level = if env.trace { Some("trace") } else { env.log_level.as_deref() };
    log::init(log_directory(&env.path_inputs()).as_deref(), level);
    std::panic::set_hook(Box::new(|info| {
        tracing::error!(panic = %info, backtrace = %std::backtrace::Backtrace::force_capture(), "panic");
        let _ = writeln!(io::stderr(), "agent-notify: internal error: {info}");
    }));
    match catch_unwind(AssertUnwindSafe(|| run(&arguments, &env, is_hook))) {
        Ok(code) => code,
        Err(_panic) if is_hook => ExitCode::SUCCESS,
        Err(_panic) => ExitCode::from(1),
    }
}

fn run(arguments: &[OsString], env: &EnvSnapshot, is_hook: bool) -> ExitCode {
    let cli = match Cli::try_parse_from(arguments) {
        Ok(cli) => cli,
        Err(error) if matches!(error.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => error.exit(),
        Err(error) if is_hook => {
            tracing::warn!(error = %error, "hook arguments did not parse");
            return ExitCode::SUCCESS;
        }
        Err(error) if error.kind() == ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => error.exit(),
        Err(error) => {
            let _ = writeln!(io::stderr(), "{}", usage_line(&error));
            return exit_code(Exit::Usage);
        }
    };
    let inputs = context::gather(&cli.command, env, context::clock(), context::random_bytes());
    if env.trace {
        tracing::trace!(event_id = %inputs.event_id, stdin = ?inputs.stdin, "raw hook input");
    }
    let invocation = invocation::plan_invocation(&cli.command, &inputs);
    execute(&cli.command, &inputs, &invocation, env)
}

fn usage_line(error: &clap::Error) -> String {
    let rendered = error.render().to_string();
    let wanted = if error.kind() == ErrorKind::MissingRequiredArgument { 2 } else { 1 };
    let message = rendered.lines().take(wanted).map(str::trim).collect::<Vec<_>>().join(" ");
    format!("agent-notify: {}", message.strip_prefix("error: ").unwrap_or(&message))
}

/// The named fields every log line about a request carries (spec 9.2).
struct LineFields<'a> {
    event: &'static str,
    session_id: &'a str,
    pane: &'a str,
}

const ABSENT: &str = "none";

fn line_fields<'a>(request: Option<&'a Request>, lookup: &'a PaneLookup) -> LineFields<'a> {
    let lookup_pane = match lookup {
        PaneLookup::Found(facts) => facts.key.pane.as_str(),
        PaneLookup::NotInTmux | PaneLookup::Unreadable { .. } => ABSENT,
    };
    match request {
        Some(Request::Agent { origin, event, .. }) => LineFields {
            event: event_name(event),
            session_id: origin.session.as_ref().map_or(ABSENT, SessionId::as_str),
            pane: origin.pane.as_ref().map_or(lookup_pane, |pane| pane.pane.as_str()),
        },
        _ => LineFields { event: ABSENT, session_id: ABSENT, pane: lookup_pane },
    }
}

fn event_name(event: &AgentEvent) -> &'static str {
    match event {
        AgentEvent::SessionStarted { .. } => "session_started",
        AgentEvent::TurnStarted => "turn_started",
        AgentEvent::TurnEnded { .. } => "turn_ended",
        AgentEvent::TurnFailed { .. } => "turn_failed",
        AgentEvent::NeedsInput { .. } => "needs_input",
        AgentEvent::InputResolved => "input_resolved",
        AgentEvent::Explicit { .. } => "explicit",
        AgentEvent::Quiet { .. } => "quiet",
        AgentEvent::QuietOff => "quiet_off",
        AgentEvent::SessionEnded => "session_ended",
        AgentEvent::Unknown => "unknown",
    }
}

fn execute(command: &Command, inputs: &Inputs, invocation: &Invocation, env: &EnvSnapshot) -> ExitCode {
    let event_id = inputs.event_id.as_str();
    let fields = line_fields(invocation.request.as_ref(), &inputs.pane);
    if let Some(note) = &invocation.log {
        tracing::info!(event_id, pane = fields.pane, reason = note.reason, "nothing sent");
        if let Some(detail) = &note.detail {
            tracing::debug!(event_id, detail = %detail, "nothing sent detail");
        }
    }
    if let Some(warning) = &invocation.warning {
        tracing::warn!(event_id, warning = %warning, "hook input problem");
    }
    if let Some(message) = &invocation.stderr {
        let _ = writeln!(io::stderr(), "agent-notify: {message}");
    }
    if command.is_dry_run() {
        for tag in &invocation.tags {
            let _ = writeln!(io::stdout(), "tag: {tag:?}");
        }
        if let Some(request) = &invocation.request
            && let Ok(line) = encode_line(request)
        {
            let _ = write!(io::stdout(), "{line}");
        }
        return exit_code(invocation.exit);
    }
    if let PaneLookup::Found(facts) = &inputs.pane {
        for tag in &invocation.tags {
            if let Err(error) = tmux::apply_tag(&facts.key.server.socket_path, &facts.key.pane, tag) {
                tracing::warn!(event_id, error = %error, tag = ?tag, "tag write failed");
            }
        }
    }
    if let PaneLookup::Unreadable { reason } = &inputs.pane {
        tracing::warn!(event_id, reason = %reason, "tmux pane unreadable");
    }
    let Some(request) = &invocation.request else { return exit_code(invocation.exit) };
    let attempt = attempt(request, |line| send(line, env));
    match &attempt {
        Ok(delivery) => {
            tracing::info!(
                event_id,
                event = fields.event,
                session_id = fields.session_id,
                pane = fields.pane,
                delivered = delivery.acknowledged(),
                reason = %delivery.reason(),
                daemon = delivery.daemon(),
                "sent"
            );
            tracing::debug!(event_id, request = ?request, "sent request");
        }
        Err(error) => tracing::error!(event_id, event = fields.event, session_id = fields.session_id, pane = fields.pane, error = %error, "request did not encode"),
    }
    if needs_fallback(&attempt) {
        fall_back(command, invocation, event_id, &fields);
    }
    exit_code(invocation.exit)
}

fn attempt(request: &Request, send: impl FnOnce(&str) -> Delivery) -> Result<Delivery, serde_json::Error> {
    encode_line(request).map(|line| send(&line))
}

fn needs_fallback(attempt: &Result<Delivery, serde_json::Error>) -> bool {
    !attempt.as_ref().is_ok_and(Delivery::acknowledged)
}

fn send(line: &str, env: &EnvSnapshot) -> Delivery {
    match socket_location(&env.path_inputs()) {
        SocketLocation::At(path) => client::deliver(&path, line, client::BUDGET),
        SocketLocation::Unavailable(reason) => Delivery::NoSocket(reason),
    }
}

fn fall_back(command: &Command, invocation: &Invocation, event_id: &str, fields: &LineFields<'_>) {
    match &invocation.fallback {
        Some(note) => match fallback::notify(Platform::current(), note) {
            Ok(()) => {
                tracing::info!(event_id, event = fields.event, session_id = fields.session_id, pane = fields.pane, "fallback shown");
                if !command.is_hook() {
                    let _ = writeln!(io::stderr(), "agent-notify: the daemon did not answer; showed a plain notification");
                }
            }
            Err(error) => tracing::warn!(event_id, event = fields.event, session_id = fields.session_id, pane = fields.pane, error = %error, "fallback failed"),
        },
        None if !command.is_hook() => {
            let _ = writeln!(io::stderr(), "agent-notify: the daemon did not answer; nothing was recorded");
        }
        None => {}
    }
}

fn exit_code(exit: Exit) -> ExitCode {
    match exit {
        Exit::Success => ExitCode::SUCCESS,
        Exit::Usage => ExitCode::from(2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_notify_core::wire::{AgentEvent, AgentKind, Kind, Origin, WIRE_VERSION};
    use agent_notify_core::{DisplayText, EpochMs, EventId};
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::path::PathBuf;

    fn request_in(cwd: PathBuf) -> Request {
        Request::Agent {
            v: WIRE_VERSION,
            event_id: EventId::from_parts(EpochMs(1), [1; 10]),
            sent_at_ms: EpochMs(1),
            origin: Origin { agent: AgentKind::ClaudeCode, session: None, prompt: None, pane: None, window: None, repo: None, session_title: None, cwd },
            event: AgentEvent::Explicit { kind: Kind::Attention, body: DisplayText::new("CI is red") },
        }
    }

    #[test]
    fn an_unencodable_request_is_never_sent_and_falls_back() {
        let request = request_in(PathBuf::from(OsStr::from_bytes(b"/tmp/\xff")));
        let attempt = attempt(&request, |_line| panic!("an unencodable request must not be sent"));
        assert!(attempt.is_err());
        assert!(needs_fallback(&attempt));
    }

    #[test]
    fn log_event_names_match_the_wire_type_tags() {
        use agent_notify_core::wire::{InputReason, QuietUntil, SessionSource};
        let text = DisplayText::new;
        let events = [
            AgentEvent::SessionStarted { source: SessionSource::Startup },
            AgentEvent::TurnStarted,
            AgentEvent::TurnEnded { last_message: None },
            AgentEvent::TurnFailed { error: text("x") },
            AgentEvent::NeedsInput { reason: InputReason::Permission, message: text("x") },
            AgentEvent::InputResolved,
            AgentEvent::Explicit { kind: Kind::Info, body: text("x") },
            AgentEvent::Quiet { until: QuietUntil::NextSend, max_minutes: 1 },
            AgentEvent::QuietOff,
            AgentEvent::SessionEnded,
        ];
        for event in events {
            let tagged = serde_json::to_value(&event).expect("serializes");
            assert_eq!(Some(event_name(&event)), tagged["type"].as_str(), "{event:?}");
        }
    }

    #[test]
    fn only_an_acknowledgement_skips_the_fallback() {
        let request = request_in(PathBuf::from("/tmp"));
        assert!(!needs_fallback(&attempt(&request, |_line| Delivery::Acked { daemon: None })));
        assert!(needs_fallback(&attempt(&request, |_line| Delivery::TimedOut)));
        assert!(needs_fallback(&attempt(&request, |_line| Delivery::Rejected { code: None })));
    }

    #[test]
    fn a_usage_error_becomes_one_line_without_the_error_prefix() {
        let error = Cli::try_parse_from(["agent-notify", "send", "--kind", "nope", "--body", "x"]).expect_err("a usage error");
        let line = usage_line(&error);
        assert!(line.starts_with("agent-notify: "), "{line:?}");
        assert!(!line.contains('\n'), "{line:?}");
        assert!(!line.contains("error: "), "{line:?}");
        assert!(line.contains("nope"), "{line:?}");
    }

    #[test]
    fn a_missing_argument_is_named_on_the_one_line() {
        let error = Cli::try_parse_from(["agent-notify", "send", "--kind", "info"]).expect_err("a usage error");
        let line = usage_line(&error);
        assert!(!line.contains('\n'), "{line:?}");
        assert!(line.contains("--body"), "{line:?}");
        assert_eq!(line, line.trim(), "{line:?}");
    }
}
