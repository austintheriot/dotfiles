//! The `status` report and the tmux state values. Pure: the daemon passes its
//! own facts in, and the CLI renders the result.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::label::location_line;
use crate::state::{Card, CardPhase, DaemonState, Dnd, QuietMode, SessionRecord, Turn};
use crate::update::{Effect, PaneState, SuppressReason};
use crate::wire::{AgentEvent, Control, Kind, MAX_REPLY_BYTES, QuietUntil, Reply, Request, encode_line};
use crate::{EpochMs, EventId, SessionId};

/// How many decisions `status` shows.
pub const MAX_DECISIONS: usize = 50;
/// How many sessions `status` shows at most, the newest ones.
pub const MAX_STATUS_SESSIONS: usize = 100;
/// The longest text field of a session row, in characters.
pub const MAX_STATUS_TEXT_CHARS: usize = 256;
/// The pane option that holds a session's turn state.
pub const PANE_STATE_OPTION: &str = "@agent_notify_state";
/// The window option the tmux status line reads.
pub const WINDOW_STATE_OPTION: &str = "@agent_notify_window_state";

/// Facts only the daemon process knows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonFacts {
    /// The daemon's build stamp.
    pub stamp: String,
    /// When the daemon started.
    pub started_ms: EpochMs,
    /// The external tools the daemon resolved at startup.
    pub tools: Vec<ToolStatus>,
}

/// One external tool and the absolute path the daemon runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolStatus {
    /// The tool, for example `tmux`.
    pub name: String,
    /// The absolute path, or `None` when the daemon did not find the tool.
    pub path: Option<PathBuf>,
}

/// One request the daemon handled, and what came of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    /// When.
    pub at_ms: EpochMs,
    /// The request's event id.
    pub event_id: EventId,
    /// The event or control kind, for example `turn_ended`.
    pub kind: String,
    /// `card` when the event left a card on screen, `suppressed: dnd` when it
    /// left a card that DND holds off screen, `no_card` when its kind never
    /// makes one, `suppressed: <reason>` for no card, or `handled` for a control.
    pub outcome: String,
}

/// Do-not-disturb as `status` shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DndStatus {
    /// True while cards are held back.
    pub on: bool,
    /// The end time, when one was set.
    pub until_ms: Option<EpochMs>,
}

/// One session record as `status` shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionStatus {
    /// The session.
    pub session: SessionId,
    /// The pane id, for example `%3`.
    pub pane: Option<String>,
    /// The card's line 1.
    pub location: String,
    /// `idle`, `working`, `finished`, `needs_you` or `failed`.
    pub turn: String,
    /// `armed`, or the quiet mode and the time left.
    pub quiet: String,
    /// The card's kind and phase, for example `attention showing`.
    pub card: Option<String>,
}

/// The `agent-notify status` report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusReport {
    /// The daemon's own facts.
    pub daemon: DaemonFacts,
    /// When the report was made.
    pub now_ms: EpochMs,
    /// Do-not-disturb.
    pub dnd: DndStatus,
    /// The newest session records that fit one reply, ordered by session id.
    pub sessions: Vec<SessionStatus>,
    /// The newest decisions, oldest first.
    pub decisions: Vec<Decision>,
}

/// Builds the report. `decisions` is the daemon's history, oldest first.
///
/// The report always fits one reply line of [`MAX_REPLY_BYTES`]: each
/// session text is cut to [`MAX_STATUS_TEXT_CHARS`], and only the newest
/// sessions that fit are listed, at most [`MAX_STATUS_SESSIONS`]. A session
/// is newer when its current turn started later; one with no turn is oldest.
#[must_use]
pub fn status_report(state: &DaemonState, now: EpochMs, daemon: &DaemonFacts, decisions: &[Decision]) -> StatusReport {
    let skip = decisions.len().saturating_sub(MAX_DECISIONS);
    let without_sessions = StatusReport {
        daemon: daemon.clone(),
        now_ms: now,
        dnd: dnd_status(state, now),
        sessions: Vec::new(),
        decisions: decisions.iter().skip(skip).cloned().collect(),
    };
    let sessions = sessions_that_fit(&without_sessions, newest_sessions(state, now));
    StatusReport { sessions, ..without_sessions }
}

/// A deadline that passed reads as off even before a tick clears it.
fn dnd_status(state: &DaemonState, now: EpochMs) -> DndStatus {
    let on = state.dnd_active(now);
    let until_ms = match state.dnd {
        Dnd::On { until } if on => until,
        Dnd::On { .. } | Dnd::Off => None,
    };
    DndStatus { on, until_ms }
}

fn newest_sessions(state: &DaemonState, now: EpochMs) -> Vec<SessionStatus> {
    let mut records: Vec<(&SessionId, &SessionRecord)> = state.sessions.iter().collect();
    records.sort_by(|left, right| (right.1.turn_started_at, right.0).cmp(&(left.1.turn_started_at, left.0)));
    records
        .into_iter()
        .take(MAX_STATUS_SESSIONS)
        .map(|(session, record)| SessionStatus {
            session: session.clone(),
            pane: record.pane.as_ref().map(|pane| capped(pane.pane.as_str())),
            location: capped(&location_line(record.window.as_ref(), record.repo.as_ref(), &record.cwd)),
            turn: capped(turn_name(&record.turn)),
            quiet: capped(&quiet_text(&record.quiet, now)),
            card: record.card.as_ref().map(|card| capped(&card_text(card))),
        })
        .collect()
}

/// Keeps rows from the front of `newest_first` while the reply still fits,
/// counting one comma per row, then orders them by session id.
fn sessions_that_fit(without_sessions: &StatusReport, newest_first: Vec<SessionStatus>) -> Vec<SessionStatus> {
    let budget = MAX_REPLY_BYTES.saturating_sub(reply_line_bytes(without_sessions));
    let mut kept: Vec<SessionStatus> = newest_first
        .into_iter()
        .scan(0_usize, |used, row| {
            *used = used.saturating_add(encoded_bytes(&row).saturating_add(1));
            (*used < budget).then_some(row)
        })
        .collect();
    kept.sort_by(|left, right| left.session.cmp(&right.session));
    kept
}

fn reply_line_bytes(report: &StatusReport) -> usize {
    let line = serde_json::to_value(report)
        .and_then(|data| encode_line(&Reply { data: Some(data), ..Reply::accepted(&report.daemon.stamp) }));
    line.map_or(MAX_REPLY_BYTES, |line| line.len())
}

fn encoded_bytes(row: &SessionStatus) -> usize {
    serde_json::to_string(row).map_or(MAX_REPLY_BYTES, |text| text.len())
}

fn capped(text: &str) -> String {
    if text.chars().count() <= MAX_STATUS_TEXT_CHARS {
        return text.to_string();
    }
    text.chars().take(MAX_STATUS_TEXT_CHARS - 1).chain(['…']).collect()
}

/// The history entry for one request and the effects it caused. Queries make
/// none: they change nothing. Focus changes make none either: tmux sends one
/// on every pane switch, and they would push the agent decisions out of the log.
#[must_use]
pub fn decision(request: &Request, effects: &[Effect], now: EpochMs) -> Option<Decision> {
    let (event_id, kind, default_outcome) = match request {
        Request::Agent { event_id, event, .. } => (event_id, event_kind(event), "card"),
        Request::Control { control: Control::FocusChanged { .. }, .. } => return None,
        Request::Control { event_id, control, .. } => (event_id, control_kind(control), "handled"),
        Request::Query { .. } | Request::Unknown => return None,
    };
    let suppressed = effects.iter().find_map(|effect| match effect {
        Effect::Suppressed { event_id: id, reason } if id == event_id => Some(*reason),
        _ => None,
    });
    let outcome = match suppressed {
        None => default_outcome.to_string(),
        Some(SuppressReason::NoCardForEvent) => "no_card".to_string(),
        Some(reason) => format!("suppressed: {}", reason_name(reason)),
    };
    Some(Decision { at_ms: now, event_id: event_id.clone(), kind: kind.to_string(), outcome })
}

/// The tmux option value for `state`. `None` means "unset the option".
#[must_use]
pub fn pane_state_value(state: PaneState) -> Option<&'static str> {
    match state {
        PaneState::Idle => None,
        PaneState::Working => Some("working"),
        PaneState::Finished => Some("finished"),
        PaneState::NeedsYou => Some("needs-you"),
        PaneState::Failed => Some("failed"),
    }
}

/// Reads a value [`pane_state_value`] wrote. Empty or unknown text is idle.
#[must_use]
pub fn parse_pane_state(value: &str) -> PaneState {
    match value {
        "working" => PaneState::Working,
        "finished" => PaneState::Finished,
        "needs-you" => PaneState::NeedsYou,
        "failed" => PaneState::Failed,
        _ => PaneState::Idle,
    }
}

/// The state a window shows: the most urgent state of its panes.
#[must_use]
pub fn window_state(states: impl IntoIterator<Item = PaneState>) -> PaneState {
    states.into_iter().max_by_key(|state| urgency(*state)).unwrap_or(PaneState::Idle)
}

fn urgency(state: PaneState) -> u8 {
    match state {
        PaneState::Idle => 0,
        PaneState::Working => 1,
        PaneState::Finished => 2,
        PaneState::Failed => 3,
        PaneState::NeedsYou => 4,
    }
}

fn turn_name(turn: &Turn) -> &'static str {
    match turn {
        Turn::Idle => "idle",
        Turn::Working { .. } => "working",
        Turn::Finished { .. } => "finished",
        Turn::NeedsYou { .. } => "needs_you",
        Turn::Failed { .. } => "failed",
    }
}

fn quiet_text(quiet: &QuietMode, now: EpochMs) -> String {
    match quiet {
        QuietMode::Armed => "armed".to_string(),
        QuietMode::Until { until, deadline } => {
            let mode = match until {
                QuietUntil::NextSend => "next-send",
                QuietUntil::NextPrompt => "next-prompt",
            };
            format!("until {mode}, at most {} s more", deadline.since(now).max(0) / 1_000)
        }
    }
}

fn card_text(card: &Card) -> String {
    let phase = match card.phase {
        CardPhase::Showing { .. } => "showing",
        CardPhase::Folded => "folded",
        CardPhase::Snoozed { .. } => "snoozed",
        CardPhase::Expiring { .. } => "expiring",
    };
    format!("{} {phase}", kind_name(card.kind))
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Info => "info",
        Kind::Attention => "attention",
    }
}

/// The snake_case name of `event`, equal to its wire `type` tag.
#[must_use]
pub fn event_kind(event: &AgentEvent) -> &'static str {
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

/// The control's kind as logs and `status` name it, for example `focus_changed`.
#[must_use]
pub fn control_kind(control: &Control) -> &'static str {
    match control {
        Control::FocusChanged { .. } => "focus_changed",
        Control::Dnd { .. } => "dnd",
        Control::Jump => "jump",
        Control::Unknown => "unknown",
    }
}

fn reason_name(reason: SuppressReason) -> &'static str {
    match reason {
        SuppressReason::ExplicitSent => "explicit_sent",
        SuppressReason::Quiet => "quiet",
        SuppressReason::Outranked => "outranked",
        SuppressReason::Focused => "focused",
        SuppressReason::Dnd => "dnd",
        SuppressReason::Stale => "stale",
        SuppressReason::NoCardForEvent => "no_card_for_event",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{agent_input, event_id, every_event, has_card_from, origin, pane, send, session};
    use crate::text::DISPLAY_TEXT_MAX_BYTES;
    use crate::wire::{DndCommand, InputReason, WindowLabel};
    use crate::{DisplayText, PaneId, PaneKey};

    fn facts() -> DaemonFacts {
        DaemonFacts { stamp: "stamp-1".to_string(), started_ms: EpochMs(0), tools: Vec::new() }
    }

    #[test]
    fn the_report_lists_sessions_with_turn_quiet_and_card() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", Some("p1"), AgentEvent::Quiet { until: QuietUntil::NextSend, max_minutes: 1 });
        send(&mut state, 3, "b", Some("p1"), AgentEvent::NeedsInput { reason: InputReason::Permission, message: DisplayText::new("Allow?") });
        let report = status_report(&state, EpochMs(4), &facts(), &[]);
        let rows: Vec<(&str, &str, Option<&str>)> =
            report.sessions.iter().map(|row| (row.turn.as_str(), row.quiet.as_str(), row.card.as_deref())).collect();
        assert_eq!(
            rows,
            vec![("working", "until next-send, at most 59 s more", None), ("needs_you", "armed", Some("attention showing"))]
        );
        assert_eq!(report.sessions[0].pane.as_deref(), Some("%1"));
        assert_eq!(report.sessions[0].location, "4 · code/feature-x");
        assert!(!report.dnd.on);
        assert_eq!(report.daemon, facts());
    }

    #[test]
    fn the_report_keeps_the_newest_decisions() {
        let decisions: Vec<Decision> = (0..60)
            .map(|number| Decision { at_ms: EpochMs(number), event_id: event_id(number), kind: "turn_ended".to_string(), outcome: "card".to_string() })
            .collect();
        let report = status_report(&DaemonState::default(), EpochMs(100), &facts(), &decisions);
        assert_eq!(report.decisions.len(), MAX_DECISIONS);
        assert_eq!(report.decisions.first().map(|decision| decision.at_ms), Some(EpochMs(10)));
    }

    fn control_request(number: i64, control: Control) -> Request {
        Request::Control { v: crate::wire::WIRE_VERSION, event_id: event_id(number), sent_at_ms: EpochMs(number), control }
    }

    #[test]
    fn controls_other_than_focus_changes_make_decisions() {
        let table = [
            (Control::FocusChanged { pane: pane(1), focused: true }, None),
            (Control::FocusChanged { pane: pane(1), focused: false }, None),
            (Control::Dnd { command: DndCommand::Off }, Some("dnd")),
            (Control::Jump, Some("jump")),
            (Control::Unknown, Some("unknown")),
        ];
        for (control, expected) in table {
            let made = decision(&control_request(1, control.clone()), &[], EpochMs(1));
            assert_eq!(made.as_ref().map(|entry| entry.kind.as_str()), expected, "{control:?}");
        }
    }

    #[test]
    fn many_focus_changes_after_an_agent_decision_leave_that_decision_in_the_report() {
        let mut state = DaemonState::default();
        let input = agent_input(1, origin(Some("a"), Some("p1")), AgentEvent::TurnEnded { last_message: None });
        let crate::update::DaemonInput::Received { request: agent, .. } = &input else { panic!("an agent input") };
        let agent = agent.clone();
        let effects = crate::update::update(&mut state, input, EpochMs(1));
        let focus_changes = (2..62).map(|number| control_request(number, Control::FocusChanged { pane: pane(1), focused: number % 2 == 0 }));
        let decisions: Vec<Decision> = std::iter::once(decision(&agent, &effects, EpochMs(1)))
            .chain(focus_changes.map(|request| decision(&request, &[], request_time(&request))))
            .flatten()
            .collect();
        let report = status_report(&state, EpochMs(100), &facts(), &decisions);
        assert!(report.decisions.iter().any(|entry| entry.event_id == event_id(1)), "{:?}", report.decisions);
    }

    fn request_time(request: &Request) -> EpochMs {
        match request {
            Request::Agent { sent_at_ms, .. } | Request::Control { sent_at_ms, .. } => *sent_at_ms,
            Request::Query { .. } | Request::Unknown => EpochMs(0),
        }
    }

    #[test]
    fn a_decision_names_the_suppression_reason() {
        let mut state = DaemonState::default();
        let input = agent_input(7, origin(Some("a"), Some("p1")), AgentEvent::TurnEnded { last_message: None });
        let crate::update::DaemonInput::Received { request, .. } = &input else { panic!("an agent input") };
        let request = request.clone();
        let effects = crate::update::update(&mut state, input, EpochMs(7));
        let made = decision(&request, &effects, EpochMs(7)).expect("agent requests make decisions");
        assert_eq!(made.kind, "turn_ended");
        assert!(made.outcome == "card" || made.outcome.starts_with("suppressed: "), "{made:?}");
        let suppressed = [Effect::Suppressed { event_id: event_id(7), reason: SuppressReason::Quiet }];
        assert_eq!(decision(&request, &suppressed, EpochMs(7)).map(|made| made.outcome), Some("suppressed: quiet".to_string()));
    }

    #[test]
    fn pane_state_values_round_trip_and_idle_unsets() {
        for state in [PaneState::Working, PaneState::Finished, PaneState::NeedsYou, PaneState::Failed] {
            let value = pane_state_value(state).expect("a non-idle state has a value");
            assert_eq!(parse_pane_state(value), state);
        }
        assert_eq!(pane_state_value(PaneState::Idle), None);
        assert_eq!(parse_pane_state(""), PaneState::Idle);
        assert_eq!(parse_pane_state("from-the-future"), PaneState::Idle);
    }

    #[test]
    fn a_window_shows_its_most_urgent_pane() {
        assert_eq!(window_state([PaneState::Finished, PaneState::NeedsYou, PaneState::Working]), PaneState::NeedsYou);
        assert_eq!(window_state([PaneState::Finished, PaneState::Failed]), PaneState::Failed);
        assert_eq!(window_state([PaneState::Idle, PaneState::Working]), PaneState::Working);
        assert_eq!(window_state([]), PaneState::Idle);
    }

    #[test]
    fn the_report_survives_a_json_round_trip() {
        let report = status_report(&DaemonState::default(), EpochMs(1), &facts(), &[]);
        let text = serde_json::to_string(&report).expect("encodes");
        assert_eq!(serde_json::from_str::<StatusReport>(&text).expect("decodes"), report);
    }

    #[test]
    fn event_kinds_match_the_wire_type_tags() {
        use crate::wire::SessionSource;
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
            assert_eq!(Some(event_kind(&event)), tagged["type"].as_str(), "{event:?}");
        }
    }

    fn outcome_of(state: &mut DaemonState, number: i64, session_name: Option<&str>, event: AgentEvent) -> String {
        let input = agent_input(number, origin(session_name, Some("p1")), event);
        let crate::update::DaemonInput::Received { request, .. } = &input else { panic!("an agent input") };
        let request = request.clone();
        let effects = crate::update::update(state, input, EpochMs(number));
        decision(&request, &effects, EpochMs(number)).expect("agent requests make decisions").outcome
    }

    #[test]
    fn a_decision_says_card_only_when_the_event_left_a_card() {
        for event in every_event() {
            for session_name in [Some("a"), None] {
                let mut state = DaemonState::default();
                send(&mut state, 0, "a", Some("p1"), AgentEvent::TurnStarted);
                let outcome = outcome_of(&mut state, 1, session_name, event.clone());
                assert_eq!(outcome == "card", has_card_from(&state, 1), "{event:?} from {session_name:?}: {outcome}");
            }
        }
    }

    #[test]
    fn a_decision_names_a_merged_or_held_card_by_where_the_card_went() {
        let needs_input = || AgentEvent::NeedsInput { reason: InputReason::Permission, message: DisplayText::new("Allow?") };
        let with_card = || {
            let mut state = DaemonState::default();
            send(&mut state, 0, "a", Some("p1"), needs_input());
            state
        };
        let dnd_on = |mut state: DaemonState| {
            state.dnd = Dnd::On { until: None };
            state
        };
        for event in every_event() {
            for (label, mut state) in [("an existing card", with_card()), ("dnd", dnd_on(DaemonState::default())), ("dnd over an existing card", dnd_on(with_card()))] {
                let outcome = outcome_of(&mut state, 1, Some("a"), event.clone());
                let held = state.dnd_active(EpochMs(1));
                let has_card = has_card_from(&state, 1);
                assert_eq!(outcome == "card", has_card && !held, "{event:?} with {label}: {outcome}");
                assert_eq!(outcome == "suppressed: dnd", has_card && held, "{event:?} with {label}: {outcome}");
            }
        }
        assert_eq!(outcome_of(&mut with_card(), 1, Some("a"), needs_input()), "card");
        assert_eq!(outcome_of(&mut dnd_on(DaemonState::default()), 1, Some("a"), needs_input()), "suppressed: dnd");
    }

    #[test]
    fn an_event_kind_that_never_makes_a_card_is_no_card_not_suppressed() {
        let mut state = DaemonState::default();
        assert_eq!(outcome_of(&mut state, 1, Some("a"), AgentEvent::TurnStarted), "no_card");
        assert_eq!(outcome_of(&mut state, 2, Some("a"), AgentEvent::QuietOff), "no_card");
        assert_eq!(outcome_of(&mut state, 3, Some("a"), AgentEvent::SessionEnded), "no_card");
        assert_eq!(outcome_of(&mut state, 4, Some("b"), AgentEvent::TurnEnded { last_message: None }), "card");
        assert_eq!(outcome_of(&mut state, 5, Some("b"), AgentEvent::TurnEnded { last_message: None }), "suppressed: stale");
    }

    #[test]
    fn dnd_past_its_deadline_reports_off_with_no_end() {
        let rows = [
            (Dnd::Off, DndStatus { on: false, until_ms: None }),
            (Dnd::On { until: None }, DndStatus { on: true, until_ms: None }),
            (Dnd::On { until: Some(EpochMs(101)) }, DndStatus { on: true, until_ms: Some(EpochMs(101)) }),
            (Dnd::On { until: Some(EpochMs(100)) }, DndStatus { on: false, until_ms: None }),
            (Dnd::On { until: Some(EpochMs(5)) }, DndStatus { on: false, until_ms: None }),
        ];
        for (dnd, expected) in rows {
            let state = DaemonState { dnd, ..DaemonState::default() };
            assert_eq!(status_report(&state, EpochMs(100), &facts(), &[]).dnd, expected, "{dnd:?}");
        }
    }

    fn crowded_state(count: u32, window_name: &str, pane_digits: usize) -> DaemonState {
        let mut state = DaemonState::default();
        for number in 0..count {
            let mut from = origin(Some(&format!("session-{number:04}")), Some("p1"));
            let pane_id = format!("%{}{}", number, "9".repeat(pane_digits));
            from.pane = Some(PaneKey { server: crate::test_support::server(), pane: PaneId::parse(&pane_id).expect("a pane id") });
            from.window = Some(WindowLabel { index: number, name: DisplayText::new(window_name) });
            let mut record = SessionRecord::new(&from);
            record.turn_started_at = Some(EpochMs(i64::from(number)));
            state.sessions.insert(session(&format!("session-{number:04}")), record);
        }
        state
    }

    fn reply_bytes(report: &StatusReport) -> usize {
        let reply = Reply { data: Some(serde_json::to_value(report).expect("encodes")), ..Reply::accepted(&report.daemon.stamp) };
        encode_line(&reply).expect("encodes").len()
    }

    fn full_decisions() -> Vec<Decision> {
        (0..MAX_DECISIONS)
            .map(|number| Decision {
                at_ms: EpochMs(1_800_000_000_000),
                event_id: event_id(i64::try_from(number).expect("small")),
                kind: "session_started".to_string(),
                outcome: "suppressed: no_card_for_event".to_string(),
            })
            .collect()
    }

    #[test]
    fn a_huge_status_report_still_fits_in_one_reply() {
        let widest = "\"\u{20000}\n".repeat(DISPLAY_TEXT_MAX_BYTES);
        for window_name in ["x".repeat(8192), widest] {
            let state = crowded_state(200, &window_name, 8192);
            let busy = DaemonFacts { stamp: "a".repeat(64), ..facts() };
            let report = status_report(&state, EpochMs(1_000), &busy, &full_decisions());
            let size = reply_bytes(&report);
            assert!(size < MAX_REPLY_BYTES, "{size} bytes");
            assert!(!report.sessions.is_empty() && report.sessions.len() <= MAX_STATUS_SESSIONS, "{}", report.sessions.len());
            assert_eq!(report.decisions.len(), MAX_DECISIONS);
        }
    }

    #[test]
    fn long_session_text_is_cut_with_an_ellipsis() {
        let state = crowded_state(1, &"y".repeat(1000), 1000);
        let row = &status_report(&state, EpochMs(1_000), &facts(), &[]).sessions[0];
        for text in [&row.location, row.pane.as_ref().expect("a pane")] {
            assert_eq!(text.chars().count(), MAX_STATUS_TEXT_CHARS, "{text}");
            assert!(text.ends_with('…'), "{text}");
        }
        let short = crowded_state(1, "main", 0);
        assert_eq!(status_report(&short, EpochMs(1_000), &facts(), &[]).sessions[0].location, "0 · main");
    }

    #[test]
    fn the_report_keeps_the_newest_sessions_in_id_order() {
        let state = crowded_state(150, "main", 0);
        let report = status_report(&state, EpochMs(1_000), &facts(), &[]);
        assert_eq!(report.sessions.len(), MAX_STATUS_SESSIONS);
        let names: Vec<&str> = report.sessions.iter().map(|row| row.session.as_str()).collect();
        let expected: Vec<String> = (50..150).map(|number| format!("session-{number:04}")).collect();
        assert_eq!(names, expected.iter().map(String::as_str).collect::<Vec<_>>());
    }

    #[test]
    fn info_and_attention_render_their_names() {
        assert_eq!(kind_name(Kind::Info), "info");
        assert_eq!(kind_name(Kind::Attention), "attention");
    }
}
