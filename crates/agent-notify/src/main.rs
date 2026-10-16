#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
//! The agent-notify CLI. Hooks and agents call it. It tags tmux panes, sends
//! one request to the daemon, and falls back to a plain OS notification.
//! `hook` exits 0 whatever happens: a `Stop` hook that exits 2 blocks the stop.

mod child;
mod cli;
mod client;
mod context;
mod doctor;
mod fallback;
mod invocation;
mod log;
mod render;
mod tmux;

use std::ffi::OsString;
use std::io::{self, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::ExitCode;
use std::time::Duration;

use agent_notify_core::paths::{Platform, SocketLocation, log_directory, socket_location};
use agent_notify_core::SessionId;
use agent_notify_core::status::{control_kind, event_kind};
use agent_notify_core::wire::{Control, Request, encode_line};
use clap::Parser;
use clap::error::ErrorKind;

use crate::cli::{Cli, Command};
use crate::client::Delivery;
use crate::context::EnvSnapshot;
use crate::invocation::{Exit, Inputs, Invocation, Output, PaneLookup, SendLog};

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
        let location = report_panic(info.location(), info.payload());
        let _ = writeln!(io::stderr(), "agent-notify: internal error at {location}");
    }));
    match catch_unwind(AssertUnwindSafe(|| run(&arguments, &env, is_hook))) {
        Ok(code) => code,
        Err(_panic) if is_hook => ExitCode::SUCCESS,
        Err(_panic) => ExitCode::from(1),
    }
}

/// Logs a panic and returns its location. A panic message can quote hook
/// payload text, so only the location goes out at error and on stderr.
fn report_panic(location: Option<&std::panic::Location<'_>>, payload: &(dyn std::any::Any + Send)) -> String {
    let location = location.map_or_else(|| "an unknown location".to_string(), ToString::to_string);
    tracing::error!(location = %location, "panic");
    let message = payload.downcast_ref::<&str>().copied().or_else(|| payload.downcast_ref::<String>().map(String::as_str)).unwrap_or("a non-text payload");
    tracing::debug!(location = %location, payload = ?message, backtrace = %std::backtrace::Backtrace::force_capture(), "panic detail");
    location
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
    if matches!(cli.command, Command::Doctor) {
        let checks = doctor::evaluate(&doctor::gather(env));
        return emit(&render::Printed { stdout: doctor::render(&checks), stderr: None, success: !doctor::any_failed(&checks) });
    }
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
            event: event_kind(event),
            session_id: origin.session.as_ref().map_or(ABSENT, SessionId::as_str),
            pane: origin.pane.as_ref().map_or(lookup_pane, |pane| pane.pane.as_str()),
        },
        Some(Request::Control { control, .. }) => LineFields {
            event: control_kind(control),
            session_id: ABSENT,
            pane: match control {
                Control::FocusChanged { pane, .. } => pane.pane.as_str(),
                Control::Dnd { .. } | Control::Jump | Control::Unknown => lookup_pane,
            },
        },
        _ => LineFields { event: ABSENT, session_id: ABSENT, pane: lookup_pane },
    }
}

fn execute(command: &Command, inputs: &Inputs, invocation: &Invocation, env: &EnvSnapshot) -> ExitCode {
    let event_id = inputs.event_id.as_str();
    let fields = line_fields(invocation.request.as_ref(), &inputs.pane);
    if let Some(note) = &invocation.log {
        let session_id = note.session_id.as_ref().map_or(fields.session_id, SessionId::as_str);
        tracing::info!(event_id, session_id, pane = fields.pane, reason = note.reason, "nothing sent");
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
    let output = invocation.output;
    let attempt = attempt(request, |line| send(line, env, output.budget()));
    match &attempt {
        Ok(delivery) if output.send_log() == SendLog::Debug => {
            tracing::debug!(event_id, delivered = delivery.acknowledged(), reason = %delivery.reason(), daemon = delivery.daemon(), "sent");
            tracing::debug!(event_id, request = ?request, "sent request");
        }
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
    match output {
        Output::FallbackOnFailure => {
            if needs_fallback(&attempt) {
                fall_back(command, invocation, event_id, &fields);
            }
            exit_code(invocation.exit)
        }
        Output::Silent => exit_code(invocation.exit),
        Output::Confirm(message) => confirm(&attempt, message),
        Output::Status(format) => {
            let socket_file_exists = matches!(socket_location(&env.path_inputs()), SocketLocation::At(path) if path.exists());
            emit(&attempt_printed(&attempt, |delivery| render::status_output(format, delivery, socket_file_exists)))
        }
        Output::DndStatus => emit(&attempt_printed(&attempt, render::dnd_output)),
    }
}

fn attempt_printed(attempt: &Result<Delivery, serde_json::Error>, render: impl FnOnce(&Delivery) -> render::Printed) -> render::Printed {
    match attempt {
        Ok(delivery) => render(delivery),
        Err(error) => render::Printed {
            stdout: String::new(),
            stderr: Some(format!("the request did not encode: {error}; run `agent-notify doctor`")),
            success: false,
        },
    }
}

fn emit(printed: &render::Printed) -> ExitCode {
    let _ = write!(io::stdout().lock(), "{}", printed.stdout);
    if let Some(line) = &printed.stderr {
        let _ = writeln!(io::stderr().lock(), "agent-notify: {line}");
    }
    if printed.success { ExitCode::SUCCESS } else { ExitCode::from(1) }
}

fn confirm(attempt: &Result<Delivery, serde_json::Error>, message: Option<&str>) -> ExitCode {
    let problem = match attempt {
        Ok(delivery) if delivery.acknowledged() => {
            if let Some(message) = message {
                let _ = writeln!(io::stdout(), "{message}");
            }
            return ExitCode::SUCCESS;
        }
        Ok(delivery) => render::delivery_problem(delivery),
        Err(error) => format!("the request did not encode: {error}"),
    };
    let _ = writeln!(io::stderr(), "agent-notify: {problem}; run `agent-notify doctor`");
    ExitCode::from(1)
}

fn attempt(request: &Request, send: impl FnOnce(&str) -> Delivery) -> Result<Delivery, serde_json::Error> {
    encode_line(request).map(|line| send(&line))
}

fn needs_fallback(attempt: &Result<Delivery, serde_json::Error>) -> bool {
    !attempt.as_ref().is_ok_and(Delivery::acknowledged)
}

fn send(line: &str, env: &EnvSnapshot, budget: Duration) -> Delivery {
    match socket_location(&env.path_inputs()) {
        SocketLocation::At(path) => client::deliver(&path, line, budget),
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
    use agent_notify_core::wire::{AgentEvent, AgentKind, Control, DndCommand, Kind, Origin, WIRE_VERSION};
    use agent_notify_core::{DisplayText, EpochMs, EventId, PaneId, PaneKey, ServerId, SocketPath};
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

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
    fn only_an_acknowledgement_skips_the_fallback() {
        let request = request_in(PathBuf::from("/tmp"));
        assert!(!needs_fallback(&attempt(&request, |_line| Delivery::Acked { daemon: None, data: None })));
        assert!(needs_fallback(&attempt(&request, |_line| Delivery::TimedOut)));
        assert!(needs_fallback(&attempt(&request, |_line| Delivery::Rejected { code: None })));
    }

    fn control_request(control: Control) -> Request {
        Request::Control { v: WIRE_VERSION, event_id: EventId::from_parts(EpochMs(1), [1; 10]), sent_at_ms: EpochMs(1), control }
    }

    #[test]
    fn a_control_line_names_its_kind_and_the_focused_pane() {
        let pane = PaneKey {
            server: ServerId { socket_path: SocketPath::parse(Path::new("/tmp/tmux-501/default")).expect("absolute"), pid: 77, start_time: 1 },
            pane: PaneId::parse("%3").expect("pane"),
        };
        let cases = [
            (control_request(Control::FocusChanged { pane, focused: true }), "focus_changed", "%3"),
            (control_request(Control::Dnd { command: DndCommand::Off }), "dnd", ABSENT),
            (control_request(Control::Jump), "jump", ABSENT),
        ];
        for (request, event, pane) in &cases {
            let fields = line_fields(Some(request), &PaneLookup::NotInTmux);
            assert_eq!((fields.event, fields.session_id, fields.pane), (*event, ABSENT, *pane), "{request:?}");
        }
    }

    #[derive(Clone, Default)]
    struct Captured(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl Write for Captured {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().map_err(|_poisoned| io::Error::other("poisoned"))?.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn panic_log(level: tracing::Level, payload: &(dyn std::any::Any + Send)) -> String {
        let captured = Captured::default();
        let writer = captured.clone();
        let subscriber = tracing_subscriber::fmt().with_writer(move || writer.clone()).with_ansi(false).with_max_level(level).finish();
        tracing::subscriber::with_default(subscriber, || report_panic(Some(std::panic::Location::caller()), payload));
        let bytes = captured.0.lock().expect("an unpoisoned buffer").clone();
        String::from_utf8(bytes).expect("UTF-8 log text")
    }

    #[test]
    fn a_panic_logs_a_fixed_line_and_its_location_at_error_and_the_payload_only_at_debug() {
        let payload: Box<dyn std::any::Any + Send> = Box::new("secret-payload \u{1b}[31m".to_string());
        let at_error = panic_log(tracing::Level::ERROR, payload.as_ref());
        assert!(at_error.contains("ERROR") && at_error.contains("panic"), "{at_error}");
        assert!(at_error.contains("main.rs:"), "the location is on the error line: {at_error}");
        assert!(!at_error.contains("secret-payload"), "{at_error}");
        let at_debug = panic_log(tracing::Level::DEBUG, payload.as_ref());
        assert!(at_debug.contains("secret-payload"), "{at_debug}");
        assert!(!at_debug.contains('\u{1b}'), "the payload is escaped: {at_debug:?}");
    }

    #[test]
    fn a_str_payload_is_logged_at_debug_too() {
        let payload: Box<dyn std::any::Any + Send> = Box::new("static-payload");
        assert!(panic_log(tracing::Level::DEBUG, payload.as_ref()).contains("static-payload"));
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
