//! Turns daemon answers into text. Pure: `main` prints the result.

use std::io;

use thumper_core::status::StatusReport;
use thumper_core::{DisplayText, EpochMs};

use crate::client::{Delivery, reply_code_name};
use crate::invocation::StatusFormat;

/// One line that says why the daemon gave no usable answer.
pub fn delivery_problem(delivery: &Delivery) -> String {
    match delivery {
        Delivery::Acked { .. } => "the daemon answered".to_string(),
        Delivery::Rejected { code: Some(code) } => format!("the daemon refused the request ({})", reply_code_name(*code)),
        Delivery::Rejected { code: None } => "the daemon refused the request".to_string(),
        Delivery::NoSocket(reason) => format!("no socket: {reason}"),
        Delivery::ConnectFailed(io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused) => "the daemon is not running".to_string(),
        Delivery::ConnectFailed(kind) => format!("could not connect to the daemon ({kind})"),
        Delivery::TimedOut => "the daemon did not answer in time".to_string(),
        Delivery::BadReply(message) => format!("the daemon sent a bad reply: {message}"),
    }
}

/// What `main` prints and how it exits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Printed {
    /// For stdout, with its own newlines.
    pub stdout: String,
    /// One line for stderr.
    pub stderr: Option<String>,
    /// Exit 0 when true, 1 when false.
    pub success: bool,
}

impl Printed {
    fn ok(stdout: String) -> Self {
        Printed { stdout, stderr: None, success: true }
    }

    fn failed(problem: String) -> Self {
        Printed { stdout: String::new(), stderr: Some(format!("{problem}; run `thumper doctor`")), success: false }
    }
}

/// The `status` output in `format`. `socket_file_exists` tells "no daemon
/// installed" (print nothing in tmux) from "daemon installed but down".
pub fn status_output(format: StatusFormat, delivery: &Delivery, socket_file_exists: bool) -> Printed {
    let report = report_from(delivery);
    match (format, report) {
        (StatusFormat::Tmux, report) => Printed::ok(tmux_segment(report.as_ref().ok(), socket_file_exists)),
        (StatusFormat::Json, Ok(report)) => match serde_json::to_string_pretty(&report) {
            Ok(text) => Printed::ok(format!("{text}\n")),
            Err(error) => Printed::failed(format!("the report did not encode: {error}")),
        },
        (StatusFormat::Text, Ok(report)) => Printed::ok(status_text(&report)),
        (StatusFormat::Json | StatusFormat::Text, Err(problem)) => Printed::failed(problem),
    }
}

/// `dnd status`: `on` or `off`.
pub fn dnd_output(delivery: &Delivery) -> Printed {
    match report_from(delivery) {
        Ok(report) => Printed::ok(if report.dnd.on { "on\n" } else { "off\n" }.to_string()),
        Err(problem) => Printed::failed(problem),
    }
}

/// The daemon's report, or one line that says why there is none.
pub fn report_from(delivery: &Delivery) -> Result<StatusReport, String> {
    match delivery {
        Delivery::Acked { data: Some(data), .. } => {
            serde_json::from_value(data.clone()).map_err(|error| format!("the daemon sent a report this CLI cannot read: {error}"))
        }
        Delivery::Acked { data: None, .. } => Err("the daemon answered without a report".to_string()),
        other => Err(delivery_problem(other)),
    }
}

fn safe(text: &str) -> String {
    DisplayText::new(text).as_str().replace('\n', " ")
}

/// The report for a person. Daemon-supplied text is cleaned for a terminal.
pub fn status_text(report: &StatusReport) -> String {
    let now = report.now_ms;
    let mut lines = vec![
        format!("thumperd {}, up {}", safe(&report.daemon.stamp), duration_text(now.since(report.daemon.started_ms))),
        format!("do-not-disturb: {}", dnd_text(report, now)),
    ];
    lines.push("tools:".to_string());
    for tool in &report.daemon.tools {
        let path = tool.path.as_ref().map_or_else(|| "not found".to_string(), |path| safe(&path.display().to_string()));
        lines.push(format!("  {}: {path}", safe(&tool.name)));
    }
    lines.push(format!("sessions: {}", report.sessions.len()));
    for session in &report.sessions {
        let pane = session.pane.as_deref().map_or_else(|| "-".to_string(), safe);
        let card = session.card.as_deref().map_or_else(|| "no card".to_string(), safe);
        lines.push(format!("  {pane}  {}  {}  quiet {}  {card}", safe(&session.location), safe(&session.turn), safe(&session.quiet)));
    }
    lines.push("decisions, newest last:".to_string());
    for decision in &report.decisions {
        lines.push(format!(
            "  {} ago  {}  {}  {}",
            duration_text(now.since(decision.at_ms)),
            safe(&decision.kind),
            safe(&decision.outcome),
            decision.event_id
        ));
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

fn dnd_text(report: &StatusReport, now: EpochMs) -> String {
    match (report.dnd.on, report.dnd.until_ms) {
        (false, _) => "off".to_string(),
        (true, Some(until)) if until.since(now) > 0 => format!("on, {} left", duration_text(until.since(now))),
        (true, _) => "on".to_string(),
    }
}

/// The tmux status-line segment. Empty unless do-not-disturb is on or an
/// installed daemon does not answer. It holds fixed text only, because tmux
/// interprets `#[...]` and `#(...)` in status output.
pub fn tmux_segment(report: Option<&StatusReport>, socket_file_exists: bool) -> String {
    match report {
        Some(report) if report.dnd.on => "#[fg=yellow]DND#[default] ".to_string(),
        Some(_) => String::new(),
        None if socket_file_exists => "#[fg=red]notify offline#[default] ".to_string(),
        None => String::new(),
    }
}

/// `45s`, `5m`, `3h 12m`. Negative spans read as `0s`.
pub fn duration_text(milliseconds: i64) -> String {
    let seconds = milliseconds.max(0) / 1_000;
    match seconds {
        0..60 => format!("{seconds}s"),
        60..3_600 => format!("{}m", seconds / 60),
        _ => format!("{}h {}m", seconds / 3_600, seconds % 3_600 / 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use thumper_core::wire::ReplyCode;

    #[test]
    fn every_failure_reads_as_one_line() {
        let cases = [
            (Delivery::Acked { daemon: None, data: None }, "the daemon answered"),
            (Delivery::ConnectFailed(io::ErrorKind::PermissionDenied), "could not connect to the daemon (permission denied)"),
            (Delivery::ConnectFailed(io::ErrorKind::ConnectionRefused), "the daemon is not running"),
            (Delivery::Rejected { code: Some(ReplyCode::UnsupportedVersion) }, "the daemon refused the request (unsupported_version)"),
            (Delivery::Rejected { code: None }, "the daemon refused the request"),
            (Delivery::NoSocket("HOME is unset or relative"), "no socket: HOME is unset or relative"),
            (Delivery::ConnectFailed(io::ErrorKind::NotFound), "the daemon is not running"),
            (Delivery::TimedOut, "the daemon did not answer in time"),
            (Delivery::BadReply("expected value".to_string()), "the daemon sent a bad reply: expected value"),
        ];
        for (delivery, line) in cases {
            assert_eq!(delivery_problem(&delivery), line, "{delivery:?}");
        }
    }

    use thumper_core::status::{DaemonFacts, Decision, DndStatus, SessionStatus, StatusReport, ToolStatus};
    use thumper_core::{EpochMs, EventId, SessionId};

    use crate::invocation::StatusFormat;

    fn report(dnd_on: bool) -> StatusReport {
        StatusReport {
            daemon: DaemonFacts {
                stamp: "stamp-1".to_string(),
                started_ms: EpochMs(0),
                tools: vec![ToolStatus { name: "tmux".to_string(), path: Some("/opt/homebrew/bin/tmux".into()) }],
            },
            now_ms: EpochMs(3 * 3_600_000 + 12 * 60_000),
            dnd: DndStatus { on: dnd_on, until_ms: None },
            sessions: vec![SessionStatus {
                session: SessionId::parse("session-a").expect("an id"),
                pane: Some("%3".to_string()),
                location: "4 · code/feature-x".to_string(),
                turn: "needs_you".to_string(),
                quiet: "armed".to_string(),
                card: Some("attention showing".to_string()),
            }],
            decisions: vec![Decision {
                at_ms: EpochMs(3 * 3_600_000 + 11 * 60_000),
                event_id: EventId::from_parts(EpochMs(1), [0; 10]),
                kind: "needs_input".to_string(),
                outcome: "card".to_string(),
            }],
        }
    }

    fn acked(report: &StatusReport) -> Delivery {
        Delivery::Acked { daemon: Some("stamp-1".to_string()), data: Some(serde_json::to_value(report).expect("encodes")) }
    }

    #[test]
    fn durations_read_naturally() {
        assert_eq!(duration_text(45_000), "45s");
        assert_eq!(duration_text(5 * 60_000 + 3_000), "5m");
        assert_eq!(duration_text(3 * 3_600_000 + 12 * 60_000), "3h 12m");
        assert_eq!(duration_text(-5), "0s");
    }

    #[test]
    fn status_text_names_the_daemon_dnd_sessions_and_decisions() {
        let text = status_text(&report(false));
        assert!(text.contains("thumperd stamp-1, up 3h 12m"), "{text}");
        assert!(text.contains("do-not-disturb: off"), "{text}");
        assert!(text.contains("%3  4 · code/feature-x  needs_you  quiet armed  attention showing"), "{text}");
        assert!(text.contains("1m ago  needs_input  card"), "{text}");
        assert!(text.contains("tmux: /opt/homebrew/bin/tmux"), "{text}");
    }

    #[test]
    fn terminal_escapes_in_daemon_strings_never_reach_the_text_output() {
        let mut hostile = report(false);
        hostile.sessions[0].location = "evil\u{1b}]0;owned\u{7}\nforged line".to_string();
        hostile.sessions[0].pane = Some("%3\u{1b}[31m".to_string());
        hostile.daemon.tools[0].name = "tmux\u{1b}[2J".to_string();
        let text = status_text(&hostile);
        assert!(!text.contains('\u{1b}') && !text.contains('\u{7}'), "{text:?}");
        let session_lines = text.lines().filter(|line| line.contains("evil")).count();
        assert_eq!(session_lines, 1, "{text:?}");
        assert!(!text.lines().any(|line| line.starts_with("forged")), "{text:?}");
    }

    #[test]
    fn the_do_not_disturb_line_reads_per_deadline() {
        let now = EpochMs(10 * 60_000);
        let cases = [
            (false, None, "do-not-disturb: off"),
            (true, None, "do-not-disturb: on\n"),
            (true, Some(EpochMs(10 * 60_000 + 90_000)), "do-not-disturb: on, 1m left"),
            (true, Some(EpochMs(9 * 60_000)), "do-not-disturb: on\n"),
            (true, Some(now), "do-not-disturb: on\n"),
        ];
        for (on, until_ms, expected) in cases {
            let mut dnd_report = report(on);
            dnd_report.now_ms = now;
            dnd_report.dnd.until_ms = until_ms;
            let text = status_text(&dnd_report);
            assert!(text.contains(expected), "{expected:?} in {text}");
        }
    }

    #[test]
    fn the_tmux_segment_shows_dnd_and_offline_only() {
        assert_eq!(tmux_segment(Some(&report(true)), true), "#[fg=yellow]DND#[default] ");
        assert_eq!(tmux_segment(Some(&report(false)), true), "");
        assert_eq!(tmux_segment(None, true), "#[fg=red]notify offline#[default] ");
        assert_eq!(tmux_segment(None, false), "");
    }

    #[test]
    fn status_output_by_format() {
        let delivery = acked(&report(false));
        let json = status_output(StatusFormat::Json, &delivery, true);
        assert!(json.success);
        assert_eq!(serde_json::from_str::<StatusReport>(&json.stdout).expect("JSON"), report(false));
        let down = status_output(StatusFormat::Text, &Delivery::TimedOut, true);
        assert!(!down.success);
        assert!(down.stderr.as_deref().is_some_and(|line| line.contains("did not answer in time")));
        let tmux = status_output(StatusFormat::Tmux, &Delivery::TimedOut, true);
        assert!(tmux.success, "the tmux segment never fails the status line");
        assert_eq!(tmux.stdout, "#[fg=red]notify offline#[default] ");
    }

    #[test]
    fn json_output_keeps_daemon_strings_as_data() {
        let mut hostile = report(false);
        hostile.sessions[0].location = "a\u{1b}b".to_string();
        let json = status_output(StatusFormat::Json, &acked(&hostile), true);
        assert_eq!(serde_json::from_str::<StatusReport>(&json.stdout).expect("JSON"), hostile);
    }

    #[test]
    fn dnd_output_prints_one_word() {
        assert_eq!(dnd_output(&acked(&report(true))).stdout, "on\n");
        assert_eq!(dnd_output(&acked(&report(false))).stdout, "off\n");
        assert!(!dnd_output(&Delivery::TimedOut).success);
    }

    #[test]
    fn a_reply_without_a_report_is_a_failure() {
        let delivery = Delivery::Acked { daemon: None, data: None };
        assert!(!status_output(StatusFormat::Text, &delivery, true).success);
    }
}
