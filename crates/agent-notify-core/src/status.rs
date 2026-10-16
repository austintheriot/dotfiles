//! The `status` report and the tmux state values. Pure: the daemon passes its
//! own facts in, and the CLI renders the result.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::label::location_line;
use crate::state::{Card, CardPhase, DaemonState, Dnd, QuietMode, Turn};
use crate::update::{Effect, PaneState, SuppressReason};
use crate::wire::{AgentEvent, Control, Kind, QuietUntil, Request};
use crate::{EpochMs, EventId, SessionId};

/// How many decisions `status` shows.
pub const MAX_DECISIONS: usize = 50;
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
    /// `card`, `handled`, or `suppressed: <reason>`.
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
    /// Every session record, ordered by session id.
    pub sessions: Vec<SessionStatus>,
    /// The newest decisions, oldest first.
    pub decisions: Vec<Decision>,
}

/// Builds the report. `decisions` is the daemon's history, oldest first.
#[must_use]
pub fn status_report(state: &DaemonState, now: EpochMs, daemon: &DaemonFacts, decisions: &[Decision]) -> StatusReport {
    let until_ms = match state.dnd {
        Dnd::On { until } => until,
        Dnd::Off => None,
    };
    let sessions = state
        .sessions
        .iter()
        .map(|(session, record)| SessionStatus {
            session: session.clone(),
            pane: record.pane.as_ref().map(|pane| pane.pane.as_str().to_string()),
            location: location_line(record.window.as_ref(), record.repo.as_ref(), &record.cwd),
            turn: turn_name(&record.turn).to_string(),
            quiet: quiet_text(&record.quiet, now),
            card: record.card.as_ref().map(card_text),
        })
        .collect();
    let skip = decisions.len().saturating_sub(MAX_DECISIONS);
    StatusReport {
        daemon: daemon.clone(),
        now_ms: now,
        dnd: DndStatus { on: state.dnd_active(now), until_ms },
        sessions,
        decisions: decisions.iter().skip(skip).cloned().collect(),
    }
}

/// The history entry for one request and the effects it caused. Queries make
/// none: they change nothing.
#[must_use]
pub fn decision(request: &Request, effects: &[Effect], now: EpochMs) -> Option<Decision> {
    let (event_id, kind, default_outcome) = match request {
        Request::Agent { event_id, event, .. } => (event_id, event_kind(event), "card"),
        Request::Control { event_id, control, .. } => (event_id, control_kind(control), "handled"),
        Request::Query { .. } | Request::Unknown => return None,
    };
    let suppressed = effects.iter().find_map(|effect| match effect {
        Effect::Suppressed { event_id: id, reason } if id == event_id => Some(*reason),
        _ => None,
    });
    let outcome = suppressed.map_or_else(|| default_outcome.to_string(), |reason| format!("suppressed: {}", reason_name(reason)));
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
    use crate::DisplayText;
    use crate::state::DaemonState;
    use crate::test_support::{event_id, send};
    use crate::update::{Effect, PaneState, SuppressReason};
    use crate::wire::{AgentEvent, InputReason, Kind, QuietUntil};

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

    #[test]
    fn a_decision_names_the_suppression_reason() {
        let mut state = DaemonState::default();
        let input = crate::test_support::agent_input(
            7,
            crate::test_support::origin(Some("a"), Some("p1")),
            AgentEvent::TurnEnded { last_message: None },
        );
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

    #[test]
    fn info_and_attention_render_their_names() {
        assert_eq!(kind_name(Kind::Info), "info");
        assert_eq!(kind_name(Kind::Attention), "attention");
    }
}
