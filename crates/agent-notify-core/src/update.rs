//! The daemon's state machine: `update(state, input, now) -> effects`.
//!
//! Cards are not effects. The shell calls [`view`](crate::view::view) after
//! every update and pushes the result.

use crate::label::{location_line, turn_ended_body, turn_failed_body};
use crate::state::{Card, CardCause, CardPhase, DaemonState, Dnd, FocusedPane, OneoffCard, QuietMode, SessionRecord, Turn};
use crate::wire::{AgentEvent, Control, DndCommand, InputReason, Kind, Origin, QuietUntil, Request};
use crate::{CardId, DisplayText, EpochMs, EventId, PaneId, PaneKey, PromptId, ServerId, SessionId};

/// How long an info card shows before it folds.
pub const INFO_SHOW_MS: i64 = 6_000;
/// Extra time before a fold, for a hover message still in transit.
pub const HOVER_GRACE_MS: i64 = 300;
/// The least time left on an info card after the pointer leaves the stack.
pub const HOVER_RESUME_MS: i64 = 2_000;
/// Continuous pane focus that clears the pane's card.
pub const FOCUS_DWELL_MS: i64 = 1_000;
/// How long the close button hides an attention card.
pub const SNOOZE_MS: i64 = 600_000;
/// The least time between two sounds.
pub const SOUND_GAP_MS: i64 = 3_000;
/// How long a "Session ended" card stays.
pub const SESSION_GONE_MS: i64 = 3_000;
/// How long a "window not raised" card stays.
pub const NOT_RAISED_MS: i64 = 2_000;
/// The user counts as present after input within this many seconds.
pub const IDLE_LIMIT_SECONDS: u32 = 30;

/// One input to [`update`].
#[derive(Debug, Clone, PartialEq, Eq)]
// Each input is moved once into `update` and dropped. Boxing `Request` would change every call site for no gain.
#[allow(clippy::large_enum_variant)]
pub enum DaemonInput {
    /// A decoded request, with the focus facts read when it arrived.
    Received {
        /// The request.
        request: Request,
        /// Focus facts at arrival.
        focus: FocusSnapshot,
    },
    /// A UI intent.
    Ui(UiIntent),
    /// The 250 ms timer.
    Tick,
    /// Pane tags read from one tmux server.
    PaneSnapshot(PaneSnapshot),
    /// The result of a focus plan.
    FocusOutcome {
        /// The clicked card.
        card: CardId,
        /// What happened.
        outcome: FocusOutcome,
    },
}

/// What the user did on the panel. Intents carry only ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiIntent {
    /// A click on a card or a finished-row entry.
    Clicked {
        /// The card.
        card_id: CardId,
    },
    /// The close button.
    Closed {
        /// The card.
        card_id: CardId,
    },
    /// The pointer entered or left the stack.
    HoverChanged {
        /// True on enter.
        hovered: bool,
    },
}

/// Focus facts the shell reads when a request arrives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusSnapshot {
    /// Which app is frontmost.
    pub frontmost: Frontmost,
    /// The active pane of the client's current window.
    pub active_pane: Option<PaneKey>,
    /// Seconds since the last user input.
    pub idle_seconds: Option<u32>,
    /// True while the screen is locked.
    pub screen_locked: Option<bool>,
    /// True while the screen is shared or recorded.
    pub screen_shared: Option<bool>,
}

impl FocusSnapshot {
    /// No facts known. Every card shows.
    #[must_use]
    pub fn unknown() -> Self {
        FocusSnapshot { frontmost: Frontmost::Unknown, active_pane: None, idle_seconds: None, screen_locked: None, screen_shared: None }
    }
}

/// The frontmost app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frontmost {
    /// The terminal that hosts tmux.
    Terminal,
    /// Another app.
    Other,
    /// Not known on this platform.
    Unknown,
}

/// Pane tags read from one tmux server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneSnapshot {
    /// The server read.
    pub server: ServerId,
    /// Every pane on it. Empty when the server is gone.
    pub panes: Vec<PaneTag>,
}

/// One pane and its session tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneTag {
    /// The pane.
    pub pane: PaneId,
    /// `@agent_notify_session`.
    pub session: Option<SessionId>,
}

/// What a focus plan did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusOutcome {
    /// Switched and raised.
    Done,
    /// No pane carries the session tag.
    SessionGone,
    /// tmux refused.
    TmuxFailed(DisplayText),
    /// Switched, but the window did not come forward.
    RaiseFailed,
}

/// Work for the shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Read the focus world for `target` and run the focus plan.
    RunFocusPlan {
        /// The clicked card.
        card: CardId,
        /// Where to go.
        target: FocusTarget,
    },
    /// Play the attention sound.
    PlaySound,
    /// Write `@agent_notify_state` on a pane.
    SetPaneState {
        /// The pane.
        pane: PaneKey,
        /// The state.
        state: PaneState,
    },
    /// Save do-not-disturb to disk.
    PersistDnd(Dnd),
    /// An event made no new card, and why. Goes to the log and `status`.
    Suppressed {
        /// The event.
        event_id: EventId,
        /// Why.
        reason: SuppressReason,
    },
}

/// Where a click goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusTarget {
    /// The pane that carries `session`'s tag. `pane` was its last location.
    Session {
        /// The session.
        session: SessionId,
        /// Its last known pane.
        pane: PaneKey,
    },
    /// No pane: raise the terminal only.
    RaiseOnly,
}

/// The value of `@agent_notify_state`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneState {
    /// No turn, or the session ended.
    Idle,
    /// Working.
    Working,
    /// Finished.
    Finished,
    /// Waiting for the user.
    NeedsYou,
    /// Stopped on an error.
    Failed,
}

/// Why an event made no new card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuppressReason {
    /// The agent already sent its own notification this turn.
    ExplicitSent,
    /// The session is quiet.
    Quiet,
    /// An attention card outranks the new info card.
    Outranked,
    /// The user is looking at the pane.
    Focused,
    /// Do-not-disturb holds the card off screen.
    Dnd,
    /// The event belongs to an older or already-ended turn.
    Stale,
    /// The event never makes a card.
    NoCardForEvent,
}

/// The result of [`merge`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Merged {
    /// The existing card stays.
    Kept(Card),
    /// The incoming card replaces the old one.
    Replaced {
        /// The new card.
        card: Card,
        /// True when it replaced an attention card.
        replaced_attention: bool,
    },
}

/// One card per session: the higher kind wins, and on a tie the newer card.
/// An expiring card always gives way.
#[must_use]
pub fn merge(existing: Option<Card>, incoming: Card) -> Merged {
    match existing {
        Some(current) if current.kind > incoming.kind && !matches!(current.phase, CardPhase::Expiring { .. }) => Merged::Kept(current),
        Some(current) => Merged::Replaced { replaced_attention: current.kind == Kind::Attention, card: incoming },
        None => Merged::Replaced { card: incoming, replaced_attention: false },
    }
}

/// True when the user is looking at `pane`. Any unknown fact means "show".
#[must_use]
pub fn is_suppressed_by_focus(snapshot: &FocusSnapshot, pane: Option<&PaneKey>) -> bool {
    let Some(pane) = pane else { return false };
    snapshot.frontmost == Frontmost::Terminal
        && snapshot.active_pane.as_ref() == Some(pane)
        && snapshot.idle_seconds.is_some_and(|seconds| seconds < IDLE_LIMIT_SECONDS)
        && snapshot.screen_locked == Some(false)
}

/// Applies one input and returns the work the shell must do.
pub fn update(state: &mut DaemonState, input: DaemonInput, now: EpochMs) -> Vec<Effect> {
    state.revision += 1;
    let mut effects = Vec::new();
    match input {
        DaemonInput::Received { request, focus } => receive(state, request, &focus, now, &mut effects),
        DaemonInput::Ui(_) | DaemonInput::Tick | DaemonInput::PaneSnapshot(_) | DaemonInput::FocusOutcome { .. } => {}
    }
    effects
}

fn receive(state: &mut DaemonState, request: Request, focus: &FocusSnapshot, now: EpochMs, effects: &mut Vec<Effect>) {
    if let Some(shared) = focus.screen_shared {
        state.screen_shared = shared;
    }
    match request {
        Request::Agent { event_id, origin, event, .. } => agent_event(state, event_id, origin, event, focus, now, effects),
        Request::Control { control, .. } => control_event(state, control, now, effects),
        Request::Query { .. } | Request::Unknown => {}
    }
}

fn agent_event(state: &mut DaemonState, event_id: EventId, origin: Origin, event: AgentEvent, focus: &FocusSnapshot, now: EpochMs, effects: &mut Vec<Effect>) {
    let Some(session) = origin.session.clone() else {
        oneoff_event(state, event_id, &origin, event, focus, now, effects);
        return;
    };
    if matches!(event, AgentEvent::SessionStarted { .. })
        && let Some(pane) = &origin.pane
    {
        state.sessions.retain(|id, record| *id == session || record.pane.as_ref() != Some(pane));
    }
    if matches!(event, AgentEvent::SessionEnded) {
        if let Some(record) = state.sessions.remove(&session)
            && let Some(pane) = record.pane
        {
            effects.push(Effect::SetPaneState { pane, state: PaneState::Idle });
        }
        effects.push(Effect::Suppressed { event_id, reason: SuppressReason::NoCardForEvent });
        return;
    }
    let dnd = state.dnd_active(now);
    let record = state.sessions.entry(session).or_insert_with(|| SessionRecord::new(&origin));
    record.absorb(&origin);
    match step(record, origin.prompt.as_ref(), event, now) {
        Step::Offer(offer) => {
            if is_suppressed_by_focus(focus, record.pane.as_ref()) {
                effects.push(Effect::Suppressed { event_id, reason: SuppressReason::Focused });
            } else {
                offer_card(&mut record.card, offer, event_id, now, dnd, &mut state.last_sound_at, effects);
            }
        }
        Step::Clear => {
            record.card = None;
            effects.push(Effect::Suppressed { event_id, reason: SuppressReason::NoCardForEvent });
        }
        Step::Nothing(reason) => effects.push(Effect::Suppressed { event_id, reason }),
    }
    if let Some(pane) = &record.pane {
        effects.push(Effect::SetPaneState { pane: pane.clone(), state: pane_state(&record.turn) });
    }
}

fn oneoff_event(state: &mut DaemonState, event_id: EventId, origin: &Origin, event: AgentEvent, focus: &FocusSnapshot, now: EpochMs, effects: &mut Vec<Effect>) {
    let AgentEvent::Explicit { kind, body } = event else {
        effects.push(Effect::Suppressed { event_id, reason: SuppressReason::NoCardForEvent });
        return;
    };
    if is_suppressed_by_focus(focus, origin.pane.as_ref()) {
        effects.push(Effect::Suppressed { event_id, reason: SuppressReason::Focused });
        return;
    }
    let dnd = state.dnd_active(now);
    let mut slot = None;
    offer_card(&mut slot, CardOffer { kind, cause: CardCause::Explicit, body }, event_id, now, dnd, &mut state.last_sound_at, effects);
    if let Some(card) = slot {
        state.oneoff_cards.push(OneoffCard {
            location: location_line(origin.window.as_ref(), origin.repo.as_ref(), &origin.cwd),
            pane: origin.pane.clone(),
            card,
        });
    }
}

fn control_event(state: &mut DaemonState, control: Control, now: EpochMs, effects: &mut Vec<Effect>) {
    match control {
        Control::FocusChanged { pane, focused: true } => state.focus = Some(FocusedPane { pane, since: now }),
        Control::FocusChanged { pane, focused: false } => {
            if state.focus.as_ref().is_some_and(|focus| focus.pane == pane) {
                state.focus = None;
            }
        }
        Control::Dnd { command: DndCommand::On { for_minutes } } => {
            state.dnd = Dnd::On { until: for_minutes.map(|minutes| now.plus_ms(i64::from(minutes) * 60_000)) };
            effects.push(Effect::PersistDnd(state.dnd));
        }
        Control::Dnd { command: DndCommand::Off } => {
            end_dnd(state);
            effects.push(Effect::PersistDnd(Dnd::Off));
        }
        Control::Unknown => {}
    }
}

/// Turns DND off. Info cards that waited go to the finished row, so the
/// return to the desk is not a burst of stale toasts.
fn end_dnd(state: &mut DaemonState) {
    state.dnd = Dnd::Off;
    let sessions = state.sessions.values_mut().filter_map(|record| record.card.as_mut());
    let oneoffs = state.oneoff_cards.iter_mut().map(|oneoff| &mut oneoff.card);
    for card in sessions.chain(oneoffs) {
        if card.kind == Kind::Info && matches!(card.phase, CardPhase::Showing { .. }) {
            card.phase = CardPhase::Folded;
        }
    }
}

struct CardOffer {
    kind: Kind,
    cause: CardCause,
    body: DisplayText,
}

enum Step {
    Offer(CardOffer),
    Clear,
    Nothing(SuppressReason),
}

fn step(record: &mut SessionRecord, prompt: Option<&PromptId>, event: AgentEvent, now: EpochMs) -> Step {
    match event {
        AgentEvent::TurnStarted => {
            begin_turn(record);
            record.turn = Turn::Working { prompt: prompt.cloned() };
            Step::Clear
        }
        AgentEvent::Explicit { kind, body } => {
            record.explicit_sent = true;
            if record.quiet.ends_on(QuietUntil::NextSend) {
                record.quiet = QuietMode::Armed;
            }
            Step::Offer(CardOffer { kind, cause: CardCause::Explicit, body })
        }
        AgentEvent::TurnEnded { last_message } => {
            if turn_match(&record.turn, prompt) != TurnMatch::Current {
                return Step::Nothing(SuppressReason::Stale);
            }
            record.turn = Turn::Finished { prompt: current_prompt(&record.turn, prompt) };
            if record.card.as_ref().is_some_and(|card| matches!(card.cause, CardCause::NeedsInput(_))) {
                record.card = None;
            }
            if record.explicit_sent {
                Step::Nothing(SuppressReason::ExplicitSent)
            } else if record.quiet != QuietMode::Armed {
                Step::Nothing(SuppressReason::Quiet)
            } else {
                Step::Offer(CardOffer { kind: Kind::Info, cause: CardCause::TurnEnded, body: turn_ended_body(last_message.as_ref()) })
            }
        }
        AgentEvent::TurnFailed { error } => {
            if !enter_turn_of(record, prompt) {
                return Step::Nothing(SuppressReason::Stale);
            }
            record.turn = Turn::Failed { prompt: current_prompt(&record.turn, prompt) };
            Step::Offer(CardOffer { kind: Kind::Attention, cause: CardCause::TurnFailed, body: turn_failed_body(&error) })
        }
        AgentEvent::NeedsInput { reason, message } => {
            if !enter_turn_of(record, prompt) {
                return Step::Nothing(SuppressReason::Stale);
            }
            record.turn = Turn::NeedsYou { prompt: current_prompt(&record.turn, prompt), reason };
            Step::Offer(CardOffer { kind: Kind::Attention, cause: CardCause::NeedsInput(reason), body: message })
        }
        AgentEvent::InputResolved => {
            if let Turn::NeedsYou { prompt: waiting, .. } = &record.turn {
                record.turn = Turn::Working { prompt: waiting.clone() };
            }
            let elicitation = record.card.as_ref().is_some_and(|card| card.cause == CardCause::NeedsInput(InputReason::Elicitation));
            if elicitation { Step::Clear } else { Step::Nothing(SuppressReason::NoCardForEvent) }
        }
        AgentEvent::Quiet { until, max_minutes } => {
            record.quiet = QuietMode::Until { until, deadline: now.plus_ms(i64::from(max_minutes) * 60_000) };
            Step::Nothing(SuppressReason::NoCardForEvent)
        }
        AgentEvent::QuietOff => {
            record.quiet = QuietMode::Armed;
            Step::Nothing(SuppressReason::NoCardForEvent)
        }
        AgentEvent::SessionStarted { .. } | AgentEvent::SessionEnded | AgentEvent::Unknown => Step::Nothing(SuppressReason::NoCardForEvent),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TurnMatch {
    /// Also when the event or the record has no prompt.
    Current,
    Ended,
    Other,
}

fn turn_match(turn: &Turn, prompt: Option<&PromptId>) -> TurnMatch {
    let (Some(event_prompt), Some(current)) = (prompt, turn.prompt()) else { return TurnMatch::Current };
    if current != event_prompt {
        TurnMatch::Other
    } else if matches!(turn, Turn::Finished { .. } | Turn::Failed { .. }) {
        TurnMatch::Ended
    } else {
        TurnMatch::Current
    }
}

fn begin_turn(record: &mut SessionRecord) {
    record.explicit_sent = false;
    if record.quiet.ends_on(QuietUntil::NextPrompt) {
        record.quiet = QuietMode::Armed;
    }
}

/// For mid-turn events. A prompt the record has not seen means its
/// `TurnStarted` was lost, so the event starts that turn. Returns false when
/// the event repeats a turn that already ended.
fn enter_turn_of(record: &mut SessionRecord, prompt: Option<&PromptId>) -> bool {
    match turn_match(&record.turn, prompt) {
        TurnMatch::Current => true,
        TurnMatch::Ended => false,
        TurnMatch::Other => {
            begin_turn(record);
            true
        }
    }
}

fn current_prompt(turn: &Turn, prompt: Option<&PromptId>) -> Option<PromptId> {
    prompt.cloned().or_else(|| turn.prompt().cloned())
}

fn offer_card(slot: &mut Option<Card>, offer: CardOffer, event_id: EventId, now: EpochMs, dnd: bool, last_sound_at: &mut Option<EpochMs>, effects: &mut Vec<Effect>) {
    let incoming = Card {
        id: CardId::for_event(&event_id),
        kind: offer.kind,
        cause: offer.cause,
        body: offer.body,
        event_id: event_id.clone(),
        created: now,
        phase: CardPhase::Showing { fade_at: (offer.kind == Kind::Info).then(|| now.plus_ms(INFO_SHOW_MS)) },
        note: None,
    };
    match merge(slot.take(), incoming) {
        Merged::Kept(existing) => {
            *slot = Some(existing);
            effects.push(Effect::Suppressed { event_id, reason: SuppressReason::Outranked });
        }
        Merged::Replaced { card, replaced_attention } => {
            let attention = card.kind == Kind::Attention;
            *slot = Some(card);
            if dnd {
                effects.push(Effect::Suppressed { event_id, reason: SuppressReason::Dnd });
                return;
            }
            let sound_allowed = last_sound_at.is_none_or(|at| now.since(at) >= SOUND_GAP_MS);
            if attention && !replaced_attention && sound_allowed {
                *last_sound_at = Some(now);
                effects.push(Effect::PlaySound);
            }
        }
    }
}

fn pane_state(turn: &Turn) -> PaneState {
    match turn {
        Turn::Idle => PaneState::Idle,
        Turn::Working { .. } => PaneState::Working,
        Turn::Finished { .. } => PaneState::Finished,
        Turn::NeedsYou { .. } => PaneState::NeedsYou,
        Turn::Failed { .. } => PaneState::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{CardCause, QuietMode, Turn};
    use crate::test_support::{agent_input, card, event_id, origin, pane, prompt, reasons, send, session};
    use crate::wire::{AgentEvent, InputReason, Kind, QuietUntil, SessionSource};
    use crate::DisplayText;

    fn text(value: &str) -> DisplayText {
        DisplayText::new(value)
    }

    fn turn_ended(message: &str) -> AgentEvent {
        AgentEvent::TurnEnded { last_message: Some(text(message)) }
    }

    fn needs_permission() -> AgentEvent {
        AgentEvent::NeedsInput { reason: InputReason::Permission, message: text("Allow Bash?") }
    }

    fn explicit(kind: Kind, body: &str) -> AgentEvent {
        AgentEvent::Explicit { kind, body: text(body) }
    }

    #[test]
    fn turn_ended_without_an_explicit_send_shows_an_info_card() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", Some("p1"), turn_ended("Done. All pass."));
        let shown = card(&state, "a").expect("a card");
        assert_eq!((shown.kind, shown.cause), (Kind::Info, CardCause::TurnEnded));
        assert_eq!(shown.body.as_str(), "Done.");
        assert_eq!(shown.id, CardId::for_event(&event_id(2)));
    }

    #[test]
    fn turn_ended_after_an_explicit_send_keeps_the_explicit_card() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", None, explicit(Kind::Info, "CI green"));
        let effects = send(&mut state, 3, "a", Some("p1"), turn_ended("Done."));
        assert_eq!(reasons(&effects), vec![SuppressReason::ExplicitSent]);
        assert_eq!(card(&state, "a").map(|shown| shown.cause), Some(CardCause::Explicit));
    }

    #[test]
    fn turn_ended_while_quiet_shows_nothing() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", None, AgentEvent::Quiet { until: QuietUntil::NextSend, max_minutes: 120 });
        let effects = send(&mut state, 3, "a", Some("p1"), turn_ended("Done."));
        assert_eq!(reasons(&effects), vec![SuppressReason::Quiet]);
        assert!(card(&state, "a").is_none());
    }

    #[test]
    fn quiet_until_next_prompt_ends_on_turn_started() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", None, AgentEvent::Quiet { until: QuietUntil::NextPrompt, max_minutes: 120 });
        send(&mut state, 2, "a", Some("p1"), AgentEvent::TurnStarted);
        assert_eq!(state.sessions[&session("a")].quiet, QuietMode::Armed);
    }

    #[test]
    fn quiet_until_next_send_ends_on_explicit_and_not_on_turn_started() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", None, AgentEvent::Quiet { until: QuietUntil::NextSend, max_minutes: 120 });
        send(&mut state, 2, "a", Some("p1"), AgentEvent::TurnStarted);
        assert_ne!(state.sessions[&session("a")].quiet, QuietMode::Armed);
        send(&mut state, 3, "a", None, explicit(Kind::Info, "x"));
        assert_eq!(state.sessions[&session("a")].quiet, QuietMode::Armed);
    }

    #[test]
    fn quiet_off_rearms() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", None, AgentEvent::Quiet { until: QuietUntil::NextSend, max_minutes: 120 });
        send(&mut state, 2, "a", None, AgentEvent::QuietOff);
        assert_eq!(state.sessions[&session("a")].quiet, QuietMode::Armed);
    }

    #[test]
    fn needs_input_shows_attention_and_the_turn_end_replaces_it_with_info() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", Some("p1"), needs_permission());
        assert_eq!(card(&state, "a").map(|shown| shown.kind), Some(Kind::Attention));
        send(&mut state, 3, "a", Some("p1"), turn_ended("Done."));
        let shown = card(&state, "a").expect("a card");
        assert_eq!((shown.kind, shown.cause), (Kind::Info, CardCause::TurnEnded));
    }

    #[test]
    fn a_late_needs_input_for_an_ended_turn_is_stale() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", Some("p1"), turn_ended("Done."));
        let effects = send(&mut state, 3, "a", Some("p1"), needs_permission());
        assert_eq!(reasons(&effects), vec![SuppressReason::Stale]);
        assert_eq!(card(&state, "a").map(|shown| shown.kind), Some(Kind::Info));
    }

    #[test]
    fn a_turn_end_for_another_prompt_is_stale() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", Some("p2"), AgentEvent::TurnStarted);
        let effects = send(&mut state, 3, "a", Some("p1"), turn_ended("old"));
        assert_eq!(reasons(&effects), vec![SuppressReason::Stale]);
        assert!(card(&state, "a").is_none());
    }

    #[test]
    fn needs_input_for_an_unseen_prompt_starts_that_turn() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        let effects = send(&mut state, 2, "a", Some("p2"), needs_permission());
        assert!(reasons(&effects).is_empty());
        assert_eq!(card(&state, "a").map(|shown| shown.kind), Some(Kind::Attention));
        let expected = Turn::NeedsYou { prompt: Some(prompt("p2")), reason: InputReason::Permission };
        assert_eq!(state.sessions[&session("a")].turn, expected);
    }

    #[test]
    fn turn_failed_for_an_unseen_prompt_starts_that_turn() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", Some("p2"), AgentEvent::TurnFailed { error: text("overloaded") });
        assert_eq!(card(&state, "a").map(|shown| shown.cause), Some(CardCause::TurnFailed));
        assert_eq!(state.sessions[&session("a")].turn, Turn::Failed { prompt: Some(prompt("p2")) });
    }

    #[test]
    fn an_explicit_send_before_a_permission_prompt_still_silences_the_turn_end() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", None, explicit(Kind::Info, "CI green"));
        send(&mut state, 3, "a", Some("p1"), needs_permission());
        let effects = send(&mut state, 4, "a", Some("p1"), turn_ended("Done."));
        assert_eq!(reasons(&effects), vec![SuppressReason::ExplicitSent]);
        assert!(card(&state, "a").is_none(), "the permission card clears and no backup card shows");
    }

    #[test]
    fn an_explicit_send_while_waiting_for_the_user_silences_the_turn_end() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", Some("p1"), needs_permission());
        send(&mut state, 3, "a", None, explicit(Kind::Info, "CI green"));
        send(&mut state, 4, "a", Some("p1"), AgentEvent::InputResolved);
        let effects = send(&mut state, 5, "a", Some("p1"), turn_ended("Done."));
        assert_eq!(reasons(&effects), vec![SuppressReason::ExplicitSent]);
        assert!(card(&state, "a").is_none());
    }

    #[test]
    fn an_explicit_send_before_any_turn_start_still_counts() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", None, explicit(Kind::Info, "CI green"));
        let effects = send(&mut state, 2, "a", Some("p1"), turn_ended("Done."));
        assert_eq!(reasons(&effects), vec![SuppressReason::ExplicitSent]);
    }

    #[test]
    fn a_new_turn_start_resets_the_explicit_flag() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", None, explicit(Kind::Info, "CI green"));
        send(&mut state, 3, "a", Some("p1"), turn_ended("Done."));
        send(&mut state, 4, "a", Some("p2"), AgentEvent::TurnStarted);
        let effects = send(&mut state, 5, "a", Some("p2"), turn_ended("Done again."));
        assert!(reasons(&effects).is_empty());
        assert_eq!(card(&state, "a").map(|shown| shown.cause), Some(CardCause::TurnEnded));
    }

    #[test]
    fn adopting_an_unseen_prompt_resets_the_explicit_flag() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", None, explicit(Kind::Info, "CI green"));
        send(&mut state, 3, "a", Some("p2"), needs_permission());
        let effects = send(&mut state, 4, "a", Some("p2"), turn_ended("Done."));
        assert!(reasons(&effects).is_empty());
        assert_eq!(card(&state, "a").map(|shown| shown.cause), Some(CardCause::TurnEnded));
    }

    #[test]
    fn a_duplicate_turn_end_is_stale() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", Some("p1"), turn_ended("Done."));
        let effects = send(&mut state, 3, "a", Some("p1"), turn_ended("Done."));
        assert_eq!(reasons(&effects), vec![SuppressReason::Stale]);
        assert_eq!(card(&state, "a").map(|shown| shown.id.clone()), Some(CardId::for_event(&event_id(2))));
    }

    #[test]
    fn info_never_replaces_attention_and_attention_replaces_info() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnFailed { error: text("overloaded") });
        let effects = send(&mut state, 2, "a", None, explicit(Kind::Info, "x"));
        assert_eq!(reasons(&effects), vec![SuppressReason::Outranked]);
        assert_eq!(card(&state, "a").map(|shown| shown.cause), Some(CardCause::TurnFailed));

        send(&mut state, 3, "b", None, explicit(Kind::Info, "x"));
        send(&mut state, 4, "b", None, explicit(Kind::Attention, "y"));
        assert_eq!(card(&state, "b").map(|shown| shown.kind), Some(Kind::Attention));
    }

    #[test]
    fn turn_failed_shows_attention_even_while_quiet() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", None, AgentEvent::Quiet { until: QuietUntil::NextSend, max_minutes: 120 });
        send(&mut state, 2, "a", Some("p1"), AgentEvent::TurnFailed { error: text("overloaded") });
        let shown = card(&state, "a").expect("a card");
        assert_eq!((shown.kind, shown.body.as_str()), (Kind::Attention, "Stopped: overloaded"));
    }

    #[test]
    fn turn_started_clears_the_card() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnFailed { error: text("x") });
        send(&mut state, 2, "a", Some("p2"), AgentEvent::TurnStarted);
        assert!(card(&state, "a").is_none());
        let record = &state.sessions[&session("a")];
        assert_eq!(record.turn, Turn::Working { prompt: Some(prompt("p2")) });
        assert!(!record.explicit_sent);
    }

    #[test]
    fn a_new_session_in_a_pane_drops_the_old_record() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "old", Some("p1"), AgentEvent::TurnFailed { error: text("x") });
        send(&mut state, 2, "new", None, AgentEvent::SessionStarted { source: SessionSource::Clear });
        assert!(!state.sessions.contains_key(&session("old")));
        assert!(state.sessions.contains_key(&session("new")));
    }

    #[test]
    fn compact_with_the_same_id_keeps_the_record() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnFailed { error: text("x") });
        send(&mut state, 2, "a", None, AgentEvent::SessionStarted { source: SessionSource::Compact });
        assert!(card(&state, "a").is_some());
    }

    #[test]
    fn session_ended_removes_the_record_and_idles_the_pane() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        let effects = send(&mut state, 2, "a", None, AgentEvent::SessionEnded);
        assert!(state.sessions.is_empty());
        assert!(effects.contains(&Effect::SetPaneState { pane: pane(1), state: PaneState::Idle }));
    }

    #[test]
    fn an_event_for_an_unknown_session_creates_the_record() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), turn_ended("Done."));
        assert_eq!(card(&state, "a").map(|shown| shown.kind), Some(Kind::Info));
    }

    #[test]
    fn input_resolved_clears_only_an_elicitation_card() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), needs_permission());
        send(&mut state, 2, "a", Some("p1"), AgentEvent::InputResolved);
        assert!(card(&state, "a").is_some(), "a permission card is not an elicitation");

        send(&mut state, 3, "b", Some("p1"), AgentEvent::NeedsInput { reason: InputReason::Elicitation, message: text("Which?") });
        send(&mut state, 4, "b", Some("p1"), AgentEvent::InputResolved);
        assert!(card(&state, "b").is_none());
        assert!(matches!(state.sessions[&session("b")].turn, Turn::Working { .. }));
    }

    #[test]
    fn an_explicit_send_without_a_session_is_a_oneoff_card() {
        let mut state = DaemonState::default();
        let input = agent_input(1, origin(None, None), explicit(Kind::Info, "Report written"));
        update(&mut state, input, EpochMs(1));
        assert!(state.sessions.is_empty());
        assert_eq!(state.oneoff_cards.len(), 1);
        assert_eq!(state.oneoff_cards[0].location, "4 · code/feature-x");
    }

    #[test]
    fn a_title_survives_a_send_without_one() {
        let mut state = DaemonState::default();
        let mut titled = origin(Some("a"), Some("p1"));
        titled.session_title = Some(text("Fix sync retry"));
        update(&mut state, agent_input(1, titled, AgentEvent::TurnStarted), EpochMs(1));
        send(&mut state, 2, "a", None, explicit(Kind::Info, "x"));
        assert_eq!(state.sessions[&session("a")].title, Some(text("Fix sync retry")));
    }

    #[test]
    fn agent_events_report_the_pane_state() {
        let mut state = DaemonState::default();
        let effects = send(&mut state, 1, "a", Some("p1"), needs_permission());
        assert!(effects.contains(&Effect::SetPaneState { pane: pane(1), state: PaneState::NeedsYou }));
    }

    #[test]
    fn every_update_increases_the_revision() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", Some("p1"), AgentEvent::TurnStarted);
        assert_eq!(state.revision, 2);
    }

    use crate::state::Dnd;
    use crate::update::Frontmost;
    use crate::wire::{Control, DndCommand, Request, WIRE_VERSION};

    fn present_at(pane_number: u32) -> FocusSnapshot {
        FocusSnapshot {
            frontmost: Frontmost::Terminal,
            active_pane: Some(pane(pane_number)),
            idle_seconds: Some(5),
            screen_locked: Some(false),
            screen_shared: Some(false),
        }
    }

    fn with_focus(input: DaemonInput, focus: FocusSnapshot) -> DaemonInput {
        match input {
            DaemonInput::Received { request, .. } => DaemonInput::Received { request, focus },
            other => other,
        }
    }

    fn control(number: i64, control: Control) -> DaemonInput {
        DaemonInput::Received {
            request: Request::Control { v: WIRE_VERSION, event_id: event_id(number), sent_at_ms: EpochMs(number), control },
            focus: FocusSnapshot::unknown(),
        }
    }

    #[test]
    fn a_user_looking_at_the_pane_gets_no_card() {
        let mut state = DaemonState::default();
        let input = with_focus(agent_input(1, origin(Some("a"), Some("p1")), needs_permission()), present_at(1));
        let effects = update(&mut state, input, EpochMs(1));
        assert_eq!(reasons(&effects), vec![SuppressReason::Focused]);
        assert!(card(&state, "a").is_none());
    }

    #[test]
    fn each_missing_presence_fact_means_show() {
        let variants = [
            FocusSnapshot { frontmost: Frontmost::Other, ..present_at(1) },
            FocusSnapshot { frontmost: Frontmost::Unknown, ..present_at(1) },
            FocusSnapshot { active_pane: Some(pane(2)), ..present_at(1) },
            FocusSnapshot { idle_seconds: Some(IDLE_LIMIT_SECONDS), ..present_at(1) },
            FocusSnapshot { idle_seconds: None, ..present_at(1) },
            FocusSnapshot { screen_locked: Some(true), ..present_at(1) },
            FocusSnapshot { screen_locked: None, ..present_at(1) },
        ];
        for focus in variants {
            let mut state = DaemonState::default();
            let input = with_focus(agent_input(1, origin(Some("a"), Some("p1")), needs_permission()), focus.clone());
            update(&mut state, input, EpochMs(1));
            assert!(card(&state, "a").is_some(), "expected a card for {focus:?}");
        }
    }

    #[test]
    fn a_shared_screen_is_remembered_from_the_snapshot() {
        let mut state = DaemonState::default();
        let focus = FocusSnapshot { screen_shared: Some(true), ..FocusSnapshot::unknown() };
        update(&mut state, with_focus(agent_input(1, origin(Some("a"), None), AgentEvent::TurnStarted), focus), EpochMs(1));
        assert!(state.screen_shared);
    }

    #[test]
    fn the_first_attention_card_plays_a_sound_and_info_cards_do_not() {
        let mut state = DaemonState::default();
        let info = send(&mut state, 1, "a", None, explicit(Kind::Info, "x"));
        assert!(!info.contains(&Effect::PlaySound));
        let attention = send(&mut state, 2, "b", Some("p1"), needs_permission());
        assert!(attention.contains(&Effect::PlaySound));
    }

    #[test]
    fn sounds_are_at_least_three_seconds_apart() {
        let mut state = DaemonState::default();
        send(&mut state, 1_000, "a", Some("p1"), needs_permission());
        let too_soon = send(&mut state, 2_000, "b", Some("p1"), needs_permission());
        assert!(!too_soon.contains(&Effect::PlaySound));
        let later = send(&mut state, 1_000 + SOUND_GAP_MS, "c", Some("p1"), needs_permission());
        assert!(later.contains(&Effect::PlaySound));
    }

    #[test]
    fn attention_replacing_attention_in_one_session_is_silent() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), needs_permission());
        let replaced = send(&mut state, 10_000, "a", Some("p1"), needs_permission());
        assert!(!replaced.contains(&Effect::PlaySound));
    }

    #[test]
    fn dnd_keeps_cards_in_state_without_sound_and_persists() {
        let mut state = DaemonState::default();
        let on = update(&mut state, control(1, Control::Dnd { command: DndCommand::On { for_minutes: None } }), EpochMs(1));
        assert_eq!(on, vec![Effect::PersistDnd(Dnd::On { until: None })]);
        let effects = send(&mut state, 2, "a", Some("p1"), needs_permission());
        assert!(!effects.contains(&Effect::PlaySound));
        assert_eq!(reasons(&effects), vec![SuppressReason::Dnd]);
        assert!(card(&state, "a").is_some());
    }

    #[test]
    fn dnd_off_folds_waiting_info_cards_and_keeps_attention_showing() {
        let mut state = DaemonState::default();
        update(&mut state, control(1, Control::Dnd { command: DndCommand::On { for_minutes: None } }), EpochMs(1));
        send(&mut state, 2, "a", None, explicit(Kind::Info, "x"));
        send(&mut state, 3, "b", Some("p1"), needs_permission());
        let off = update(&mut state, control(4, Control::Dnd { command: DndCommand::Off }), EpochMs(4));
        assert_eq!(off, vec![Effect::PersistDnd(Dnd::Off)]);
        assert_eq!(card(&state, "a").map(|shown| shown.phase), Some(CardPhase::Folded));
        assert!(matches!(card(&state, "b").map(|shown| shown.phase), Some(CardPhase::Showing { .. })));
    }

    #[test]
    fn timed_dnd_is_active_until_its_end() {
        let mut state = DaemonState::default();
        update(&mut state, control(0, Control::Dnd { command: DndCommand::On { for_minutes: Some(30) } }), EpochMs(0));
        assert!(state.dnd_active(EpochMs(30 * 60_000 - 1)));
        assert!(!state.dnd_active(EpochMs(30 * 60_000)));
    }

    #[test]
    fn focus_events_track_the_focused_pane() {
        let mut state = DaemonState::default();
        update(&mut state, control(5, Control::FocusChanged { pane: pane(1), focused: true }), EpochMs(5));
        assert_eq!(state.focus.as_ref().map(|focus| (focus.pane.clone(), focus.since)), Some((pane(1), EpochMs(5))));
        update(&mut state, control(6, Control::FocusChanged { pane: pane(2), focused: false }), EpochMs(6));
        assert!(state.focus.is_some(), "focus-out of another pane changes nothing");
        update(&mut state, control(7, Control::FocusChanged { pane: pane(1), focused: false }), EpochMs(7));
        assert!(state.focus.is_none());
    }
}
