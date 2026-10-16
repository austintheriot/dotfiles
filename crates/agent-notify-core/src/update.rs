//! The daemon's state machine: `update(state, input, now) -> effects`.
//!
//! Cards are not effects. The shell calls [`view`](crate::view::view) after
//! every update and pushes the result.

use crate::label::{location_line, turn_ended_body, turn_failed_body};
use crate::state::{Card, CardCause, CardNote, CardPhase, DaemonState, Dnd, FocusedPane, InfoLifetime, OneoffCard, QuietMode, SessionRecord, Turn};
use crate::wire::{AgentEvent, Control, DndCommand, InputReason, Kind, Origin, QuietUntil, Request};
use crate::{CardId, DisplayText, EpochMs, EventId, PaneId, PaneKey, PromptId, ServerId, SessionId};

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
    /// An agent event put no new card on screen, and why. Under
    /// [`SuppressReason::Dnd`] the new card is stored but held off screen.
    /// For every other reason no new card exists. Goes to the log and `status`.
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

/// Why an agent event put no new card on screen. These are the reasons
/// [`update`] emits. A snoozed card has no reason of its own: a new offer
/// replaces it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuppressReason {
    /// `TurnEnded`: the agent already called `send` this turn. No card.
    ExplicitSent,
    /// `TurnEnded` while the session is quiet. No card.
    Quiet,
    /// The session's visible attention card outranks the new info card. The
    /// new card is dropped and the attention card stays.
    Outranked,
    /// The focus facts at arrival say the user is looking at the pane. No card.
    Focused,
    /// Do-not-disturb is on. The new card is stored but held off screen.
    Dnd,
    /// The event belongs to a turn that already ended, to another prompt
    /// (`TurnEnded`), or to an unseen prompt sent at or before the current
    /// turn's start. No card, and the turn does not change. An `Explicit` from
    /// such an unseen prompt is the exception: it shows its card and ends
    /// next-send quiet, but leaves the turn alone, and gets no reason.
    Stale,
    /// The event kind never makes a card: a session start or end, a turn
    /// start, quiet on or off, a resolved input, or an event other than
    /// `send` without a session. A turn start, and a resolved elicitation,
    /// also clear the session's card.
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
        /// True when it replaced an attention card the user could see. A
        /// snoozed card was hidden, so replacing it does not count.
        replaced_attention: bool,
    },
}

/// One card per session: the higher kind wins, and on a tie the newer card.
/// An expiring or snoozed card always gives way, so a hidden card never
/// outranks a visible one.
#[must_use]
pub fn merge(existing: Option<Card>, incoming: Card) -> Merged {
    match existing {
        Some(current) if current.kind > incoming.kind && !matches!(current.phase, CardPhase::Expiring { .. } | CardPhase::Snoozed { .. }) => Merged::Kept(current),
        Some(current) => {
            let replaced_attention = current.kind == Kind::Attention && !matches!(current.phase, CardPhase::Snoozed { .. });
            Merged::Replaced { replaced_attention, card: incoming }
        }
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

/// True when `request` is an agent event that can put a card on screen. Only
/// these need the focus facts, so the shell reads presence for no other.
#[must_use]
pub fn may_offer_card(request: &Request) -> bool {
    matches!(
        request,
        Request::Agent {
            event: AgentEvent::TurnEnded { .. } | AgentEvent::TurnFailed { .. } | AgentEvent::NeedsInput { .. } | AgentEvent::Explicit { .. },
            ..
        }
    )
}

/// Applies one input and returns the work the shell must do.
pub fn update(state: &mut DaemonState, input: DaemonInput, now: EpochMs) -> Vec<Effect> {
    state.revision += 1;
    let mut effects = Vec::new();
    match input {
        DaemonInput::Received { request, focus } => receive(state, request, &focus, now, &mut effects),
        DaemonInput::Ui(intent) => ui_intent(state, intent, now, &mut effects),
        DaemonInput::Tick => tick(state, now, &mut effects),
        DaemonInput::PaneSnapshot(snapshot) => reap(state, &snapshot, &mut effects),
        DaemonInput::FocusOutcome { card, outcome } => focus_outcome(state, &card, outcome, now),
    }
    effects
}

fn receive(state: &mut DaemonState, request: Request, focus: &FocusSnapshot, now: EpochMs, effects: &mut Vec<Effect>) {
    if let Some(shared) = focus.screen_shared {
        state.screen_shared = shared;
    }
    match request {
        Request::Agent { event_id, sent_at_ms, origin, event, .. } => {
            agent_event(state, Arrival { event_id, sent_at: sent_at_ms, origin, event }, focus, now, effects);
        }
        Request::Control { control, .. } => control_event(state, control, now, effects),
        Request::Query { .. } | Request::Unknown => {}
    }
}

struct Arrival {
    event_id: EventId,
    sent_at: EpochMs,
    origin: Origin,
    event: AgentEvent,
}

fn agent_event(state: &mut DaemonState, arrival: Arrival, focus: &FocusSnapshot, now: EpochMs, effects: &mut Vec<Effect>) {
    let Arrival { event_id, sent_at, origin, event } = arrival;
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
        if let Some(pane) = ended_session_pane(state, &session, origin.pane) {
            effects.push(Effect::SetPaneState { pane, state: PaneState::Idle });
        }
        effects.push(Effect::Suppressed { event_id, reason: SuppressReason::NoCardForEvent });
        return;
    }
    let context = OfferContext { now, dnd: state.dnd_active(now), info_lifetime: state.info_lifetime };
    let record = state.sessions.entry(session).or_insert_with(|| SessionRecord::new(&origin));
    let stepped = step(record, origin.prompt.as_ref(), event, sent_at, now);
    if !stepped.is_stale() {
        record.absorb(&origin);
    }
    match stepped {
        Step::Offer(offer) | Step::StaleOffer(offer) => {
            if is_suppressed_by_focus(focus, record.pane.as_ref()) {
                effects.push(Effect::Suppressed { event_id, reason: SuppressReason::Focused });
            } else {
                offer_card(&mut record.card, offer, event_id, context, &mut state.last_sound_at, effects);
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

/// Drops the session's record and names the pane to idle. With no record (a
/// daemon restart lost it), the hook's own pane is idled, unless another
/// session's record now holds that pane.
fn ended_session_pane(state: &mut DaemonState, session: &SessionId, origin_pane: Option<PaneKey>) -> Option<PaneKey> {
    if let Some(record) = state.sessions.remove(session) {
        return record.pane;
    }
    origin_pane.filter(|pane| !state.sessions.values().any(|record| record.pane.as_ref() == Some(pane)))
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
    let context = OfferContext { now, dnd: state.dnd_active(now), info_lifetime: state.info_lifetime };
    let mut slot = None;
    offer_card(&mut slot, CardOffer { kind, cause: CardCause::Explicit, body }, event_id, context, &mut state.last_sound_at, effects);
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
            state.frozen_view = None;
            effects.push(Effect::PersistDnd(state.dnd));
        }
        Control::Dnd { command: DndCommand::Off } => {
            end_dnd(state);
            effects.push(Effect::PersistDnd(Dnd::Off));
        }
        Control::Jump => {
            if let Some((card, target)) = jump_target(state) {
                effects.push(Effect::RunFocusPlan { card, target });
            }
        }
        Control::Unknown => {}
    }
}

/// Turns DND off. Info cards that waited go to the finished row, so the
/// return to the desk is not a burst of stale toasts. The frozen view went
/// stale with the DND flag, as it does when DND turns on.
fn end_dnd(state: &mut DaemonState) {
    state.dnd = Dnd::Off;
    state.frozen_view = None;
    for card in cards_mut(state) {
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
    /// A card from an event sent before the current turn started. The turn
    /// and the record's location stay as they are.
    StaleOffer(CardOffer),
    Clear,
    Nothing(SuppressReason),
}

impl Step {
    fn is_stale(&self) -> bool {
        matches!(self, Step::StaleOffer(_) | Step::Nothing(SuppressReason::Stale))
    }
}

fn step(record: &mut SessionRecord, prompt: Option<&PromptId>, event: AgentEvent, sent_at: EpochMs, now: EpochMs) -> Step {
    match event {
        AgentEvent::TurnStarted => {
            begin_turn(record, sent_at);
            record.turn = Turn::Working { prompt: prompt.cloned() };
            Step::Clear
        }
        AgentEvent::Explicit { kind, body } => {
            let offer = CardOffer { kind, cause: CardCause::Explicit, body };
            let late = match turn_match(&record.turn, prompt) {
                TurnMatch::Other if sent_before_turn_start(record, sent_at) => true,
                TurnMatch::Other => {
                    begin_turn(record, sent_at);
                    record.turn = Turn::Working { prompt: prompt.cloned() };
                    false
                }
                TurnMatch::Current | TurnMatch::Ended => false,
            };
            if record.quiet.ends_on(QuietUntil::NextSend) {
                record.quiet = QuietMode::Armed;
            }
            if late {
                return Step::StaleOffer(offer);
            }
            record.explicit_sent = true;
            Step::Offer(offer)
        }
        AgentEvent::TurnEnded { last_message } => {
            if turn_match(&record.turn, prompt) != TurnMatch::Current {
                return Step::Nothing(SuppressReason::Stale);
            }
            record.turn = Turn::Finished { prompt: current_prompt(&record.turn, prompt) };
            if record.card.as_ref().is_some_and(|card| matches!(card.cause, CardCause::NeedsInput(_))) {
                record.card = None;
            }
            expire_quiet(record, now);
            if record.explicit_sent {
                Step::Nothing(SuppressReason::ExplicitSent)
            } else if record.quiet != QuietMode::Armed {
                Step::Nothing(SuppressReason::Quiet)
            } else {
                Step::Offer(CardOffer { kind: Kind::Info, cause: CardCause::TurnEnded, body: turn_ended_body(last_message.as_ref()) })
            }
        }
        AgentEvent::TurnFailed { error } => {
            if !enter_turn_of(record, prompt, sent_at) {
                return Step::Nothing(SuppressReason::Stale);
            }
            record.turn = Turn::Failed { prompt: current_prompt(&record.turn, prompt) };
            Step::Offer(CardOffer { kind: Kind::Attention, cause: CardCause::TurnFailed, body: turn_failed_body(&error) })
        }
        AgentEvent::NeedsInput { reason, message } => {
            if !enter_turn_of(record, prompt, sent_at) {
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

fn begin_turn(record: &mut SessionRecord, sent_at: EpochMs) {
    record.turn_started_at = Some(sent_at);
    record.explicit_sent = false;
    if record.quiet.ends_on(QuietUntil::NextPrompt) {
        record.quiet = QuietMode::Armed;
    }
}

/// Async hooks reach the daemon in connect order, not hook order, so an event
/// for an older prompt can arrive after a newer turn started.
///
/// `sent_at` comes from the wall clock of each CLI process. If that clock goes
/// back between two hook processes (an NTP step, a manual change), a real
/// event for a new prompt can compare as sent before the turn start and be
/// dropped as stale instead of adopted. That risk is accepted: the window is
/// one clock step, and a monotonic clock is not shared across processes.
fn sent_before_turn_start(record: &SessionRecord, sent_at: EpochMs) -> bool {
    record.turn_started_at.is_some_and(|started| sent_at <= started)
}

/// For mid-turn events. A prompt the record has not seen means its
/// `TurnStarted` was lost, so the event starts that turn, unless the event
/// was sent before the current turn started. Returns false when the event is
/// stale: it repeats a turn that already ended, or it comes from an older turn.
fn enter_turn_of(record: &mut SessionRecord, prompt: Option<&PromptId>, sent_at: EpochMs) -> bool {
    match turn_match(&record.turn, prompt) {
        TurnMatch::Current => true,
        TurnMatch::Ended => false,
        TurnMatch::Other if sent_before_turn_start(record, sent_at) => false,
        TurnMatch::Other => {
            begin_turn(record, sent_at);
            true
        }
    }
}

fn current_prompt(turn: &Turn, prompt: Option<&PromptId>) -> Option<PromptId> {
    prompt.cloned().or_else(|| turn.prompt().cloned())
}

#[derive(Clone, Copy)]
struct OfferContext {
    now: EpochMs,
    dnd: bool,
    info_lifetime: InfoLifetime,
}

fn offer_card(slot: &mut Option<Card>, offer: CardOffer, event_id: EventId, context: OfferContext, last_sound_at: &mut Option<EpochMs>, effects: &mut Vec<Effect>) {
    let OfferContext { now, dnd, info_lifetime } = context;
    let incoming = Card {
        id: CardId::for_event(&event_id),
        kind: offer.kind,
        cause: offer.cause,
        body: offer.body,
        event_id: event_id.clone(),
        created: now,
        phase: CardPhase::Showing { fade_at: if offer.kind == Kind::Info { info_lifetime.fade_at(now) } else { None } },
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

fn expire_quiet(record: &mut SessionRecord, now: EpochMs) {
    if let QuietMode::Until { deadline, .. } = record.quiet
        && now >= deadline
    {
        record.quiet = QuietMode::Armed;
    }
}

fn cards_mut(state: &mut DaemonState) -> impl Iterator<Item = &mut Card> {
    let sessions = state.sessions.values_mut().filter_map(|record| record.card.as_mut());
    let oneoffs = state.oneoff_cards.iter_mut().map(|oneoff| &mut oneoff.card);
    sessions.chain(oneoffs)
}

fn remove_card(state: &mut DaemonState, card_id: &CardId) {
    for record in state.sessions.values_mut() {
        if record.card.as_ref().is_some_and(|card| &card.id == card_id) {
            record.card = None;
        }
    }
    state.oneoff_cards.retain(|oneoff| &oneoff.card.id != card_id);
}

fn ui_intent(state: &mut DaemonState, intent: UiIntent, now: EpochMs, effects: &mut Vec<Effect>) {
    match intent {
        UiIntent::Clicked { card_id } => {
            if let Some(target) = click_target(state, &card_id) {
                effects.push(Effect::RunFocusPlan { card: card_id, target });
            }
        }
        UiIntent::Closed { card_id } => close(state, &card_id, now),
        UiIntent::HoverChanged { hovered: true } => {
            if !state.stack_hovered {
                state.frozen_view = Some(crate::view::view(state, now));
                state.stack_hovered = true;
            }
        }
        UiIntent::HoverChanged { hovered: false } => {
            state.stack_hovered = false;
            state.frozen_view = None;
            let resume_at = now.plus_ms(HOVER_RESUME_MS);
            for card in cards_mut(state) {
                if let CardPhase::Showing { fade_at: Some(fade_at) } = &mut card.phase {
                    *fade_at = (*fade_at).max(resume_at);
                }
            }
        }
    }
}

fn click_target(state: &DaemonState, card_id: &CardId) -> Option<FocusTarget> {
    for (session, record) in &state.sessions {
        if record.card.as_ref().is_some_and(|card| &card.id == card_id) {
            return Some(match &record.pane {
                Some(pane) => FocusTarget::Session { session: session.clone(), pane: pane.clone() },
                None => FocusTarget::RaiseOnly,
            });
        }
    }
    state.oneoff_cards.iter().any(|oneoff| &oneoff.card.id == card_id).then_some(FocusTarget::RaiseOnly)
}

/// The newest attention card that is not on its way out, and where it goes.
/// A snoozed card counts: the user closed it, but the agent still waits.
/// Card ids embed a ULID, so the larger id breaks a tie on `created` stably.
fn jump_target(state: &DaemonState) -> Option<(CardId, FocusTarget)> {
    let sessions = state.sessions.values().filter_map(|record| record.card.as_ref());
    let oneoffs = state.oneoff_cards.iter().map(|oneoff| &oneoff.card);
    let newest = sessions
        .chain(oneoffs)
        .filter(|card| card.kind == Kind::Attention && !matches!(card.phase, CardPhase::Expiring { .. }))
        .max_by_key(|card| (card.created, &card.id))?;
    let card = newest.id.clone();
    click_target(state, &card).map(|target| (card, target))
}

fn close(state: &mut DaemonState, card_id: &CardId, now: EpochMs) {
    for record in state.sessions.values_mut() {
        let Some(kind) = record.card.as_ref().filter(|card| &card.id == card_id).map(|card| card.kind) else {
            continue;
        };
        let expiring = record.card.as_ref().is_some_and(|card| matches!(card.phase, CardPhase::Expiring { .. }));
        match kind {
            Kind::Info => record.card = None,
            Kind::Attention if expiring => record.card = None,
            Kind::Attention => {
                let prompt = record.turn.prompt().cloned();
                if let Some(card) = &mut record.card {
                    card.phase = CardPhase::Snoozed { until: now.plus_ms(SNOOZE_MS), prompt };
                }
            }
        }
        return;
    }
    state.oneoff_cards.retain(|oneoff| &oneoff.card.id != card_id);
}

fn tick(state: &mut DaemonState, now: EpochMs, effects: &mut Vec<Effect>) {
    if let Dnd::On { until: Some(until) } = state.dnd
        && now >= until
    {
        end_dnd(state);
        effects.push(Effect::PersistDnd(Dnd::Off));
    }
    let dwelled = state.focus.as_ref().filter(|focus| now.since(focus.since) >= FOCUS_DWELL_MS).cloned();
    let hovered = state.stack_hovered;
    for record in state.sessions.values_mut() {
        expire_quiet(record, now);
        let dwell_clears = dwelled.as_ref().is_some_and(|focus| {
            record.pane.as_ref() == Some(&focus.pane) && record.card.as_ref().is_some_and(|card| card.created <= focus.since)
        });
        if dwell_clears {
            record.card = None;
        }
        let keep = record.card.as_mut().is_none_or(|card| advance(card, now, hovered, Some(&record.turn)));
        if !keep {
            record.card = None;
        }
    }
    state.oneoff_cards.retain_mut(|oneoff| advance(&mut oneoff.card, now, hovered, None));
}

/// Moves a card along its timers. Returns false when the card must go.
/// `turn` is the owning session's turn, `None` for a one-off card.
fn advance(card: &mut Card, now: EpochMs, hovered: bool, turn: Option<&Turn>) -> bool {
    match &card.phase {
        CardPhase::Showing { fade_at: Some(fade_at) } if !hovered && now.since(*fade_at) >= HOVER_GRACE_MS => {
            card.phase = CardPhase::Folded;
        }
        CardPhase::Snoozed { until, prompt } if now >= *until => {
            if !still_waiting(turn, prompt.as_ref()) {
                return false;
            }
            card.phase = CardPhase::Showing { fade_at: None };
            card.note = None;
        }
        CardPhase::Expiring { remove_at } if now >= *remove_at => return false,
        _ => {}
    }
    true
}

/// A snoozed card is still worth showing only while its session waits on the
/// user, or still failed, in the turn the card was snoozed in.
fn still_waiting(turn: Option<&Turn>, snoozed_prompt: Option<&PromptId>) -> bool {
    turn.is_some_and(|turn| matches!(turn, Turn::NeedsYou { .. } | Turn::Failed { .. }) && turn.prompt() == snoozed_prompt)
}

/// Drops records whose pane is gone, whose pane now names another session, or
/// whose server was replaced by a new one on the same socket. Then writes the
/// pane state of every record kept on the snapshot's server again, so a write
/// that tmux lost heals within one snapshot.
fn reap(state: &mut DaemonState, snapshot: &PaneSnapshot, effects: &mut Vec<Effect>) {
    state.sessions.retain(|session, record| {
        let Some(pane) = &record.pane else { return true };
        if pane.server.socket_path != snapshot.server.socket_path {
            return true;
        }
        if pane.server != snapshot.server {
            return false;
        }
        snapshot.panes.iter().any(|tag| tag.pane == pane.pane && tag.session.as_ref() == Some(session))
    });
    let rewrites = state.sessions.values().filter_map(|record| {
        let pane = record.pane.as_ref().filter(|pane| pane.server == snapshot.server)?;
        Some(Effect::SetPaneState { pane: pane.clone(), state: pane_state(&record.turn) })
    });
    effects.extend(rewrites);
}

fn focus_outcome(state: &mut DaemonState, card_id: &CardId, outcome: FocusOutcome, now: EpochMs) {
    if outcome == FocusOutcome::Done {
        remove_card(state, card_id);
        return;
    }
    let Some(card) = cards_mut(state).find(|card| &card.id == card_id) else { return };
    match outcome {
        FocusOutcome::Done => {}
        FocusOutcome::SessionGone => {
            card.note = Some(CardNote::SessionEnded);
            card.phase = CardPhase::Expiring { remove_at: now.plus_ms(SESSION_GONE_MS) };
        }
        FocusOutcome::TmuxFailed(reason) => card.note = Some(CardNote::SwitchFailed(reason)),
        FocusOutcome::RaiseFailed => {
            card.note = Some(CardNote::NotRaised);
            card.phase = CardPhase::Expiring { remove_at: now.plus_ms(NOT_RAISED_MS) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{CardCause, QuietMode, Turn};
    use crate::test_support::{agent_input, card, event_id, origin, pane, prompt, reasons, send, send_sent_at, session};
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
    fn a_needs_input_sent_before_the_current_turn_started_is_stale() {
        let mut state = DaemonState::default();
        send(&mut state, 200, "a", Some("p2"), AgentEvent::TurnStarted);
        let effects = send_sent_at(&mut state, 250, 100, "a", Some("p1"), needs_permission());
        assert_eq!(reasons(&effects), vec![SuppressReason::Stale]);
        assert!(card(&state, "a").is_none(), "no phantom permission card");
        assert_eq!(state.sessions[&session("a")].turn, Turn::Working { prompt: Some(prompt("p2")) });
        send(&mut state, 300, "a", Some("p2"), turn_ended("Done."));
        assert_eq!(card(&state, "a").map(|shown| shown.cause), Some(CardCause::TurnEnded), "p2 still gets its finished card");
    }

    #[test]
    fn a_needs_input_sent_after_the_current_turn_started_is_adopted() {
        let mut state = DaemonState::default();
        send(&mut state, 200, "a", Some("p2"), AgentEvent::TurnStarted);
        let effects = send_sent_at(&mut state, 350, 300, "a", Some("p3"), needs_permission());
        assert!(reasons(&effects).is_empty());
        let expected = Turn::NeedsYou { prompt: Some(prompt("p3")), reason: InputReason::Permission };
        assert_eq!(state.sessions[&session("a")].turn, expected);
        assert_eq!(state.sessions[&session("a")].turn_started_at, Some(EpochMs(300)));
    }

    #[test]
    fn a_record_with_no_turn_start_time_adopts_as_before() {
        let mut state = DaemonState::default();
        send(&mut state, 500, "a", Some("p1"), needs_permission());
        assert_eq!(state.sessions[&session("a")].turn_started_at, None);
        let effects = send_sent_at(&mut state, 600, 100, "a", Some("p0"), needs_permission());
        assert!(reasons(&effects).iter().all(|reason| *reason != SuppressReason::Stale));
        let expected = Turn::NeedsYou { prompt: Some(prompt("p0")), reason: InputReason::Permission };
        assert_eq!(state.sessions[&session("a")].turn, expected);
    }

    #[test]
    fn an_explicit_send_from_before_the_current_turn_does_not_take_it_over() {
        let mut state = DaemonState::default();
        send(&mut state, 200, "a", Some("p2"), AgentEvent::TurnStarted);
        send_sent_at(&mut state, 250, 100, "a", Some("p1"), explicit(Kind::Info, "old"));
        assert!(!state.sessions[&session("a")].explicit_sent, "the late send belongs to p1, so p2's backup card still shows");
        assert_eq!(state.sessions[&session("a")].turn, Turn::Working { prompt: Some(prompt("p2")) });
        let effects = send(&mut state, 300, "a", Some("p2"), turn_ended("Done."));
        assert!(reasons(&effects).is_empty());
        assert_eq!(card(&state, "a").map(|shown| shown.cause), Some(CardCause::TurnEnded));
    }

    #[test]
    fn a_stale_explicit_send_still_ends_next_send_quiet_and_shows_its_card() {
        let mut state = DaemonState::default();
        send(&mut state, 50, "a", None, AgentEvent::Quiet { until: QuietUntil::NextSend, max_minutes: 120 });
        send(&mut state, 200, "a", Some("p2"), AgentEvent::TurnStarted);
        let effects = send_sent_at(&mut state, 250, 100, "a", Some("p1"), explicit(Kind::Info, "CI green"));
        assert!(reasons(&effects).is_empty(), "the card is offered, so nothing is suppressed");
        let record = &state.sessions[&session("a")];
        assert_eq!(record.quiet, QuietMode::Armed);
        assert_eq!(record.turn, Turn::Working { prompt: Some(prompt("p2")) });
        assert_eq!(record.turn_started_at, Some(EpochMs(200)));
        let shown = card(&state, "a").expect("the late send's card");
        assert_eq!((shown.id.clone(), shown.cause, shown.body.as_str()), (CardId::for_event(&event_id(250)), CardCause::Explicit, "CI green"));
    }

    fn located(prompt_name: Option<&str>, pane_number: u32, window_name: &str, title: &str, cwd: &str) -> Origin {
        let mut located = origin(Some("a"), prompt_name);
        located.pane = Some(pane(pane_number));
        located.window = Some(crate::wire::WindowLabel { index: 7, name: text(window_name) });
        located.session_title = Some(text(title));
        located.cwd = std::path::PathBuf::from(cwd);
        located
    }

    #[test]
    fn a_stale_event_does_not_overwrite_the_records_location_or_title() {
        let stale_events = [needs_permission(), AgentEvent::TurnFailed { error: text("x") }, explicit(Kind::Info, "late"), turn_ended("old")];
        for stale in stale_events {
            let mut state = DaemonState::default();
            let fresh = located(Some("p2"), 1, "new-window", "New title", "/new");
            update(&mut state, crate::test_support::agent_input_sent_at(200, 200, fresh, AgentEvent::TurnStarted), EpochMs(200));
            let late = located(Some("p1"), 2, "old-window", "Old title", "/old");
            update(&mut state, crate::test_support::agent_input_sent_at(250, 100, late, stale.clone()), EpochMs(250));
            let record = &state.sessions[&session("a")];
            assert_eq!(record.pane, Some(pane(1)), "{stale:?}");
            assert_eq!(record.window.as_ref().map(|window| window.name.as_str()), Some("new-window"), "{stale:?}");
            assert_eq!(record.title, Some(text("New title")), "{stale:?}");
            assert_eq!(record.cwd, std::path::PathBuf::from("/new"), "{stale:?}");
        }
    }

    #[test]
    fn a_fresh_event_takes_the_newer_location() {
        let mut state = DaemonState::default();
        let first = located(Some("p1"), 1, "old-window", "Old title", "/old");
        update(&mut state, agent_input(100, first, AgentEvent::TurnStarted), EpochMs(100));
        let moved = located(Some("p1"), 2, "new-window", "New title", "/new");
        update(&mut state, agent_input(200, moved, needs_permission()), EpochMs(200));
        let record = &state.sessions[&session("a")];
        assert_eq!(record.pane, Some(pane(2)));
        assert_eq!(record.window.as_ref().map(|window| window.name.as_str()), Some("new-window"));
        assert_eq!(record.title, Some(text("New title")));
        assert_eq!(record.cwd, std::path::PathBuf::from("/new"));
    }

    #[test]
    fn turn_started_records_its_sent_time() {
        let mut state = DaemonState::default();
        send_sent_at(&mut state, 210, 200, "a", Some("p2"), AgentEvent::TurnStarted);
        assert_eq!(state.sessions[&session("a")].turn_started_at, Some(EpochMs(200)));
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
        assert_eq!(card(&state, "a").map(|shown| shown.phase.clone()), Some(CardPhase::Folded));
        assert!(matches!(card(&state, "b").map(|shown| shown.phase.clone()), Some(CardPhase::Showing { .. })));
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

    use crate::state::CardNote;
    use crate::update::{FocusOutcome, PaneSnapshot, PaneTag, UiIntent};
    use crate::test_support::server;
    use crate::{PaneId, ServerId};

    fn card_id_of(state: &DaemonState, name: &str) -> CardId {
        card(state, name).map(|shown| shown.id.clone()).expect("a card")
    }

    fn tick(state: &mut DaemonState, now: i64) -> Vec<Effect> {
        update(state, DaemonInput::Tick, EpochMs(now))
    }

    fn phase(state: &DaemonState, name: &str) -> Option<CardPhase> {
        card(state, name).map(|shown| shown.phase.clone())
    }

    const SHOW_MS: i64 = 6_000;

    fn folding_after_show_ms() -> DaemonState {
        DaemonState { info_lifetime: InfoLifetime::FoldAfter { show_ms: SHOW_MS }, ..DaemonState::default() }
    }

    #[test]
    fn by_default_an_info_card_stays_until_the_user_handles_it() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", None, explicit(Kind::Info, "x"));
        tick(&mut state, 3_600_000);
        assert_eq!(phase(&state, "a"), Some(CardPhase::Showing { fade_at: None }));
    }

    #[test]
    fn a_click_on_an_info_card_asks_for_the_focus_plan() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", Some("p1"), turn_ended("Done."));
        let id = card_id_of(&state, "a");
        let effects = update(&mut state, DaemonInput::Ui(UiIntent::Clicked { card_id: id.clone() }), EpochMs(1));
        assert_eq!(effects, vec![Effect::RunFocusPlan { card: id, target: FocusTarget::Session { session: session("a"), pane: pane(1) } }]);
    }

    #[test]
    fn an_info_card_folds_after_its_show_time_plus_grace() {
        let mut state = folding_after_show_ms();
        send(&mut state, 0, "a", None, explicit(Kind::Info, "x"));
        tick(&mut state, SHOW_MS + HOVER_GRACE_MS - 1);
        assert!(matches!(phase(&state, "a"), Some(CardPhase::Showing { .. })));
        tick(&mut state, SHOW_MS + HOVER_GRACE_MS);
        assert_eq!(phase(&state, "a"), Some(CardPhase::Folded));
    }

    #[test]
    fn hover_holds_the_fold_and_leaving_gives_time_back() {
        let mut state = folding_after_show_ms();
        send(&mut state, 0, "a", None, explicit(Kind::Info, "x"));
        update(&mut state, DaemonInput::Ui(UiIntent::HoverChanged { hovered: true }), EpochMs(1_000));
        assert!(state.frozen_view.is_some());
        tick(&mut state, 60_000);
        assert!(matches!(phase(&state, "a"), Some(CardPhase::Showing { .. })));
        update(&mut state, DaemonInput::Ui(UiIntent::HoverChanged { hovered: false }), EpochMs(60_000));
        assert!(state.frozen_view.is_none());
        tick(&mut state, 60_000 + HOVER_RESUME_MS + HOVER_GRACE_MS - 1);
        assert!(matches!(phase(&state, "a"), Some(CardPhase::Showing { .. })));
        tick(&mut state, 60_000 + HOVER_RESUME_MS + HOVER_GRACE_MS);
        assert_eq!(phase(&state, "a"), Some(CardPhase::Folded));
    }

    #[test]
    fn attention_cards_never_fold() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", Some("p1"), needs_permission());
        tick(&mut state, 3_600_000);
        assert!(matches!(phase(&state, "a"), Some(CardPhase::Showing { fade_at: None })));
    }

    #[test]
    fn a_focus_dwell_clears_a_card_that_was_there_before_the_focus() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", Some("p1"), needs_permission());
        update(&mut state, control(10, Control::FocusChanged { pane: pane(1), focused: true }), EpochMs(10));
        tick(&mut state, 10 + FOCUS_DWELL_MS - 1);
        assert!(card(&state, "a").is_some());
        tick(&mut state, 10 + FOCUS_DWELL_MS);
        assert!(card(&state, "a").is_none());
    }

    #[test]
    fn leaving_before_the_dwell_keeps_the_card() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", Some("p1"), needs_permission());
        update(&mut state, control(10, Control::FocusChanged { pane: pane(1), focused: true }), EpochMs(10));
        update(&mut state, control(500, Control::FocusChanged { pane: pane(1), focused: false }), EpochMs(500));
        tick(&mut state, 5_000);
        assert!(card(&state, "a").is_some());
    }

    #[test]
    fn a_card_that_arrives_during_focus_is_not_cleared_by_that_focus() {
        let mut state = DaemonState::default();
        update(&mut state, control(0, Control::FocusChanged { pane: pane(1), focused: true }), EpochMs(0));
        send(&mut state, 100, "a", Some("p1"), needs_permission());
        tick(&mut state, 10_000);
        assert!(card(&state, "a").is_some());
    }

    #[test]
    fn quiet_ends_at_its_deadline() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", None, AgentEvent::Quiet { until: QuietUntil::NextSend, max_minutes: 2 });
        tick(&mut state, 2 * 60_000 - 1);
        assert_ne!(state.sessions[&session("a")].quiet, QuietMode::Armed);
        tick(&mut state, 2 * 60_000);
        assert_eq!(state.sessions[&session("a")].quiet, QuietMode::Armed);
    }

    #[test]
    fn a_turn_end_after_the_quiet_deadline_shows_the_backup_card_without_a_tick() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 1, "a", None, AgentEvent::Quiet { until: QuietUntil::NextSend, max_minutes: 1 });
        let effects = send(&mut state, 1 + 60_000, "a", Some("p1"), turn_ended("Done."));
        assert!(reasons(&effects).is_empty());
        assert_eq!(card(&state, "a").map(|shown| shown.cause), Some(CardCause::TurnEnded));
        assert_eq!(state.sessions[&session("a")].quiet, QuietMode::Armed);
    }

    #[test]
    fn a_turn_end_before_the_quiet_deadline_stays_quiet() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 1, "a", None, AgentEvent::Quiet { until: QuietUntil::NextSend, max_minutes: 1 });
        let effects = send(&mut state, 60_000, "a", Some("p1"), turn_ended("Done."));
        assert_eq!(reasons(&effects), vec![SuppressReason::Quiet]);
    }

    #[test]
    fn timed_dnd_ends_on_a_tick() {
        let mut state = DaemonState::default();
        update(&mut state, control(0, Control::Dnd { command: DndCommand::On { for_minutes: Some(1) } }), EpochMs(0));
        let effects = tick(&mut state, 60_000);
        assert_eq!(state.dnd, Dnd::Off);
        assert_eq!(effects, vec![Effect::PersistDnd(Dnd::Off)]);
    }

    #[test]
    fn turning_dnd_on_drops_the_frozen_view() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", None, explicit(Kind::Info, "x"));
        update(&mut state, DaemonInput::Ui(UiIntent::HoverChanged { hovered: true }), EpochMs(1));
        assert!(state.frozen_view.is_some());
        update(&mut state, control(2, Control::Dnd { command: DndCommand::On { for_minutes: None } }), EpochMs(2));
        assert!(state.frozen_view.is_none());
        let model = crate::view::view(&state, EpochMs(3));
        assert!(model.dnd && model.cards.is_empty());
    }

    #[test]
    fn a_click_asks_for_the_focus_plan() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", Some("p1"), needs_permission());
        let id = card_id_of(&state, "a");
        let effects = update(&mut state, DaemonInput::Ui(UiIntent::Clicked { card_id: id.clone() }), EpochMs(1));
        assert_eq!(effects, vec![Effect::RunFocusPlan { card: id, target: FocusTarget::Session { session: session("a"), pane: pane(1) } }]);
    }

    #[test]
    fn intents_for_unknown_cards_do_nothing() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", Some("p1"), needs_permission());
        let before = state.clone();
        let stale = CardId::for_event(&event_id(999));
        assert!(update(&mut state, DaemonInput::Ui(UiIntent::Clicked { card_id: stale.clone() }), EpochMs(1)).is_empty());
        update(&mut state, DaemonInput::Ui(UiIntent::Closed { card_id: stale }), EpochMs(1));
        assert_eq!(state.sessions, before.sessions);
    }

    #[test]
    fn closing_info_removes_it_and_closing_attention_snoozes_it() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", None, explicit(Kind::Info, "x"));
        send(&mut state, 1, "b", Some("p1"), needs_permission());
        let info = card_id_of(&state, "a");
        let attention = card_id_of(&state, "b");
        update(&mut state, DaemonInput::Ui(UiIntent::Closed { card_id: info }), EpochMs(2));
        update(&mut state, DaemonInput::Ui(UiIntent::Closed { card_id: attention }), EpochMs(2));
        assert!(card(&state, "a").is_none());
        assert_eq!(phase(&state, "b"), Some(CardPhase::Snoozed { until: EpochMs(2 + SNOOZE_MS), prompt: Some(prompt("p1")) }));
        tick(&mut state, 2 + SNOOZE_MS);
        assert_eq!(phase(&state, "b"), Some(CardPhase::Showing { fade_at: None }));
    }

    #[test]
    fn an_explicit_send_during_a_snooze_replaces_the_hidden_card() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", Some("p1"), needs_permission());
        let attention = card_id_of(&state, "a");
        update(&mut state, DaemonInput::Ui(UiIntent::Closed { card_id: attention.clone() }), EpochMs(1));
        let effects = send(&mut state, 2, "a", Some("p2"), explicit(Kind::Info, "x"));
        assert!(reasons(&effects).is_empty());
        let shown = card(&state, "a").expect("a card");
        assert_eq!((shown.id.clone(), shown.cause), (CardId::for_event(&event_id(2)), CardCause::Explicit));
        assert!(matches!(shown.phase, CardPhase::Showing { .. }));
        tick(&mut state, 1 + SNOOZE_MS);
        assert_ne!(card(&state, "a").map(|shown| shown.id.clone()), Some(attention));
    }

    #[test]
    fn a_snoozed_card_does_not_come_back_after_a_focused_new_prompt_needs_input() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", Some("p1"), needs_permission());
        let attention = card_id_of(&state, "a");
        update(&mut state, DaemonInput::Ui(UiIntent::Closed { card_id: attention }), EpochMs(1));
        let input = with_focus(agent_input(2, origin(Some("a"), Some("p2")), needs_permission()), present_at(1));
        let effects = update(&mut state, input, EpochMs(2));
        assert_eq!(reasons(&effects), vec![SuppressReason::Focused]);
        assert!(matches!(state.sessions[&session("a")].turn, Turn::NeedsYou { .. }));
        assert!(matches!(phase(&state, "a"), Some(CardPhase::Snoozed { .. })));
        tick(&mut state, 1 + SNOOZE_MS);
        assert!(card(&state, "a").is_none());
    }

    #[test]
    fn merge_replaces_a_snoozed_attention_card_with_a_new_info_card() {
        let snoozed = Card {
            id: CardId::for_event(&event_id(1)),
            kind: Kind::Attention,
            cause: CardCause::NeedsInput(InputReason::Permission),
            body: text("Allow Bash?"),
            event_id: event_id(1),
            created: EpochMs(1),
            phase: CardPhase::Snoozed { until: EpochMs(1 + SNOOZE_MS), prompt: Some(prompt("p1")) },
            note: None,
        };
        let incoming = Card {
            id: CardId::for_event(&event_id(2)),
            kind: Kind::Info,
            cause: CardCause::Explicit,
            body: text("CI green"),
            event_id: event_id(2),
            created: EpochMs(2),
            phase: CardPhase::Showing { fade_at: Some(EpochMs(2 + SHOW_MS)) },
            note: None,
        };
        assert_eq!(merge(Some(snoozed), incoming.clone()), Merged::Replaced { card: incoming, replaced_attention: false });
    }

    #[test]
    fn a_snoozed_card_does_not_come_back_after_its_prompt_is_answered() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", Some("p1"), needs_permission());
        let attention = card_id_of(&state, "a");
        update(&mut state, DaemonInput::Ui(UiIntent::Closed { card_id: attention }), EpochMs(1));
        send(&mut state, 2, "a", Some("p1"), AgentEvent::InputResolved);
        tick(&mut state, 1 + SNOOZE_MS);
        assert!(card(&state, "a").is_none());
    }

    #[test]
    fn an_explicit_send_for_an_unseen_prompt_adopts_it_and_silences_its_turn_end() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", Some("p2"), explicit(Kind::Info, "CI green"));
        assert_eq!(state.sessions[&session("a")].turn, Turn::Working { prompt: Some(prompt("p2")) });
        send(&mut state, 3, "a", Some("p2"), needs_permission());
        let effects = send(&mut state, 4, "a", Some("p2"), turn_ended("Done."));
        assert_eq!(reasons(&effects), vec![SuppressReason::ExplicitSent]);
        assert!(card(&state, "a").is_none_or(|shown| shown.cause != CardCause::TurnEnded));
    }

    #[test]
    fn focus_outcomes() {
        let mut state = DaemonState::default();
        for (number, name) in [(0, "done"), (1, "gone"), (2, "tmux"), (3, "raise")] {
            send(&mut state, number, name, Some("p1"), needs_permission());
        }
        let outcome = |state: &mut DaemonState, name: &str, outcome: FocusOutcome| {
            let card_id = card_id_of(state, name);
            update(state, DaemonInput::FocusOutcome { card: card_id, outcome }, EpochMs(10));
        };
        outcome(&mut state, "done", FocusOutcome::Done);
        outcome(&mut state, "gone", FocusOutcome::SessionGone);
        outcome(&mut state, "tmux", FocusOutcome::TmuxFailed(DisplayText::new("no client")));
        outcome(&mut state, "raise", FocusOutcome::RaiseFailed);
        assert!(card(&state, "done").is_none());
        assert_eq!(card(&state, "gone").map(|shown| (shown.note.clone(), shown.phase.clone())), Some((Some(CardNote::SessionEnded), CardPhase::Expiring { remove_at: EpochMs(10 + SESSION_GONE_MS) })));
        assert_eq!(card(&state, "tmux").and_then(|shown| shown.note.clone()), Some(CardNote::SwitchFailed(DisplayText::new("no client"))));
        assert_eq!(phase(&state, "raise"), Some(CardPhase::Expiring { remove_at: EpochMs(10 + NOT_RAISED_MS) }));
        tick(&mut state, 10 + SESSION_GONE_MS);
        assert!(card(&state, "gone").is_none() && card(&state, "raise").is_none());
    }

    #[test]
    fn a_pane_snapshot_drops_dead_and_retagged_sessions() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "kept", None, AgentEvent::TurnStarted);
        let mut other = origin(Some("moved"), None);
        other.pane = Some(pane(2));
        update(&mut state, agent_input(1, other, AgentEvent::TurnStarted), EpochMs(1));
        let mut gone = origin(Some("gone"), None);
        gone.pane = Some(pane(3));
        update(&mut state, agent_input(2, gone, AgentEvent::TurnStarted), EpochMs(2));
        let snapshot = PaneSnapshot {
            server: server(),
            panes: vec![
                PaneTag { pane: PaneId::parse("%1").expect("pane"), session: Some(session("kept")) },
                PaneTag { pane: PaneId::parse("%2").expect("pane"), session: Some(session("someone-else")) },
            ],
        };
        update(&mut state, DaemonInput::PaneSnapshot(snapshot), EpochMs(3));
        let names: Vec<&str> = state.sessions.keys().map(SessionId::as_str).collect();
        assert_eq!(names, vec!["kept"]);
    }

    #[test]
    fn a_pane_snapshot_rewrites_the_state_of_every_pane_it_keeps() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "kept", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 1, "kept", Some("p1"), needs_permission());
        let mut elsewhere = origin(Some("elsewhere"), None);
        elsewhere.pane = Some(PaneKey { server: ServerId { socket_path: crate::SocketPath::parse(std::path::Path::new("/tmp/other")).expect("absolute"), ..server() }, pane: PaneId::parse("%1").expect("pane") });
        update(&mut state, agent_input(2, elsewhere, AgentEvent::TurnStarted), EpochMs(2));
        let snapshot = PaneSnapshot { server: server(), panes: vec![PaneTag { pane: PaneId::parse("%1").expect("pane"), session: Some(session("kept")) }] };
        let effects = update(&mut state, DaemonInput::PaneSnapshot(snapshot), EpochMs(3));
        assert_eq!(effects, vec![Effect::SetPaneState { pane: pane(1), state: PaneState::NeedsYou }]);
    }

    #[test]
    fn a_new_server_on_the_same_socket_drops_the_old_servers_sessions() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", None, AgentEvent::TurnStarted);
        let restarted = ServerId { pid: 200, ..server() };
        update(&mut state, DaemonInput::PaneSnapshot(PaneSnapshot { server: restarted, panes: Vec::new() }), EpochMs(1));
        assert!(state.sessions.is_empty());
    }
}

#[cfg(test)]
mod jump_tests {
    use super::*;
    use crate::DisplayText;
    use crate::test_support::{agent_input, card, event_id, origin, send, session};
    use crate::wire::{AgentEvent, Control, InputReason, Request, WIRE_VERSION};

    fn jump(state: &mut DaemonState, number: i64) -> Vec<Effect> {
        let request = Request::Control { v: WIRE_VERSION, event_id: event_id(number), sent_at_ms: EpochMs(number), control: Control::Jump };
        update(state, DaemonInput::Received { request, focus: FocusSnapshot::unknown() }, EpochMs(number))
    }

    fn needs_input() -> AgentEvent {
        AgentEvent::NeedsInput { reason: InputReason::Permission, message: DisplayText::new("Allow?") }
    }

    fn needs(state: &mut DaemonState, number: i64, name: &str) {
        send(state, number, name, Some("p1"), needs_input());
    }

    fn card_id(state: &DaemonState, name: &str) -> CardId {
        card(state, name).map(|shown| shown.id.clone()).expect("the session has a card")
    }

    #[test]
    fn jump_targets_the_newest_attention_card() {
        let mut state = DaemonState::default();
        needs(&mut state, 1, "a");
        needs(&mut state, 2, "b");
        send(&mut state, 3, "c", None, AgentEvent::Explicit { kind: Kind::Info, body: DisplayText::new("done") });
        let expected = card_id(&state, "b");
        let effects = jump(&mut state, 4);
        assert!(
            matches!(
                effects.as_slice(),
                [Effect::RunFocusPlan { card, target: FocusTarget::Session { session: target, .. } }]
                    if *card == expected && *target == session("b")
            ),
            "{effects:?}"
        );
    }

    #[test]
    fn jump_breaks_a_creation_time_tie_on_the_larger_card_id() {
        let mut state = DaemonState::default();
        update(&mut state, agent_input(6, origin(Some("a"), Some("p1")), needs_input()), EpochMs(10));
        update(&mut state, agent_input(5, origin(Some("b"), Some("p1")), needs_input()), EpochMs(10));
        let expected = card_id(&state, "a");
        assert!(expected > card_id(&state, "b"));
        let effects = jump(&mut state, 11);
        assert!(matches!(effects.as_slice(), [Effect::RunFocusPlan { card, .. }] if *card == expected), "{effects:?}");
    }

    #[test]
    fn jump_reaches_a_snoozed_attention_card() {
        let mut state = DaemonState::default();
        needs(&mut state, 1, "a");
        let expected = card_id(&state, "a");
        update(&mut state, DaemonInput::Ui(UiIntent::Closed { card_id: expected.clone() }), EpochMs(2));
        let effects = jump(&mut state, 3);
        assert!(matches!(effects.as_slice(), [Effect::RunFocusPlan { card, .. }] if *card == expected), "{effects:?}");
    }

    #[test]
    fn jump_without_an_attention_card_does_nothing() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", None, AgentEvent::Explicit { kind: Kind::Info, body: DisplayText::new("done") });
        assert!(jump(&mut state, 2).is_empty());
    }
}

#[cfg(test)]
mod card_life_tests {
    use super::*;
    use crate::state::{CardCause, Turn};
    use crate::test_support::{agent_input, card, event_id, every_event, has_card_from, origin, pane, prompt, reasons, send, session};
    use crate::wire::{AgentEvent, Control, DndCommand, InputReason, Query, Request, WIRE_VERSION};

    fn needs_permission() -> AgentEvent {
        AgentEvent::NeedsInput { reason: InputReason::Permission, message: DisplayText::new("Allow Bash?") }
    }

    fn info(body: &str) -> AgentEvent {
        AgentEvent::Explicit { kind: Kind::Info, body: DisplayText::new(body) }
    }

    fn card_id_of(state: &DaemonState, name: &str) -> CardId {
        card(state, name).map(|shown| shown.id.clone()).expect("a card")
    }

    fn close(state: &mut DaemonState, card_id: CardId, now: i64) {
        update(state, DaemonInput::Ui(UiIntent::Closed { card_id }), EpochMs(now));
    }

    fn outcome(state: &mut DaemonState, card: CardId, outcome: FocusOutcome, now: i64) {
        update(state, DaemonInput::FocusOutcome { card, outcome }, EpochMs(now));
    }

    fn dnd(state: &mut DaemonState, command: DndCommand, now: i64) {
        let request = Request::Control { v: WIRE_VERSION, event_id: event_id(now), sent_at_ms: EpochMs(now), control: Control::Dnd { command } };
        update(state, DaemonInput::Received { request, focus: FocusSnapshot::unknown() }, EpochMs(now));
    }

    fn hover(state: &mut DaemonState, now: i64) {
        update(state, DaemonInput::Ui(UiIntent::HoverChanged { hovered: true }), EpochMs(now));
    }

    #[test]
    fn closing_an_expiring_attention_card_removes_it_instead_of_snoozing_it() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", Some("p1"), needs_permission());
        let id = card_id_of(&state, "a");
        outcome(&mut state, id.clone(), FocusOutcome::RaiseFailed, 1);
        assert!(matches!(card(&state, "a").map(|shown| &shown.phase), Some(CardPhase::Expiring { .. })));
        close(&mut state, id, 2);
        assert!(card(&state, "a").is_none());
    }

    #[test]
    fn a_card_back_from_a_snooze_drops_its_old_note() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", Some("p1"), needs_permission());
        let id = card_id_of(&state, "a");
        outcome(&mut state, id.clone(), FocusOutcome::TmuxFailed(DisplayText::new("no client")), 1);
        assert!(card(&state, "a").is_some_and(|shown| shown.note.is_some()));
        close(&mut state, id, 2);
        update(&mut state, DaemonInput::Tick, EpochMs(2 + SNOOZE_MS));
        let shown = card(&state, "a").expect("the card shows again");
        assert_eq!((shown.phase.clone(), shown.note.clone()), (CardPhase::Showing { fade_at: None }, None));
    }

    #[test]
    fn turning_dnd_off_drops_the_frozen_view() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", None, info("x"));
        dnd(&mut state, DndCommand::On { for_minutes: None }, 1);
        hover(&mut state, 2);
        assert!(state.frozen_view.is_some());
        dnd(&mut state, DndCommand::Off, 3);
        assert!(state.frozen_view.is_none());
    }

    #[test]
    fn a_timed_dnd_end_drops_the_frozen_view() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", None, info("x"));
        dnd(&mut state, DndCommand::On { for_minutes: Some(1) }, 1);
        hover(&mut state, 2);
        assert!(state.frozen_view.is_some());
        update(&mut state, DaemonInput::Tick, EpochMs(1 + 60_000));
        assert!(state.frozen_view.is_none());
    }

    #[test]
    fn a_new_attention_card_over_a_snoozed_one_plays_the_sound() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", Some("p1"), needs_permission());
        let snoozed = card_id_of(&state, "a");
        close(&mut state, snoozed, 1);
        let effects = send(&mut state, 10_000, "a", Some("p2"), needs_permission());
        assert!(effects.contains(&Effect::PlaySound));
    }

    #[test]
    fn a_new_attention_card_over_a_visible_one_stays_silent() {
        let mut state = DaemonState::default();
        send(&mut state, 0, "a", Some("p1"), needs_permission());
        let effects = send(&mut state, 10_000, "a", Some("p2"), needs_permission());
        assert!(!effects.contains(&Effect::PlaySound));
    }

    #[test]
    fn turn_started_ends_an_older_prompts_attention_card() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", Some("p1"), needs_permission());
        send(&mut state, 3, "a", Some("p2"), AgentEvent::TurnStarted);
        assert!(card(&state, "a").is_none());
        assert_eq!(state.sessions[&session("a")].turn, Turn::Working { prompt: Some(prompt("p2")) });
    }

    #[test]
    fn adoption_keeps_an_older_prompts_attention_card_until_a_turn_start() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), AgentEvent::TurnStarted);
        send(&mut state, 2, "a", Some("p1"), needs_permission());
        let effects = send(&mut state, 3, "a", Some("p2"), info("CI green"));
        assert_eq!(reasons(&effects), vec![SuppressReason::Outranked]);
        assert_eq!(state.sessions[&session("a")].turn, Turn::Working { prompt: Some(prompt("p2")) });
        let kept = card(&state, "a").expect("the p1 card stays");
        assert_eq!((kept.cause, kept.phase.clone()), (CardCause::NeedsInput(InputReason::Permission), CardPhase::Showing { fade_at: None }));
        send(&mut state, 4, "a", Some("p3"), AgentEvent::TurnStarted);
        assert!(card(&state, "a").is_none());
    }

    #[test]
    fn an_explicit_send_on_an_idle_record_does_not_adopt_its_prompt() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "a", Some("p1"), info("x"));
        let record = &state.sessions[&session("a")];
        assert_eq!(record.turn, Turn::Idle);
        assert_eq!(record.turn_started_at, None);
        assert!(record.explicit_sent);
        assert_eq!(card(&state, "a").map(|shown| shown.cause), Some(CardCause::Explicit));
    }

    #[test]
    fn session_ended_without_a_record_idles_its_pane() {
        let mut state = DaemonState::default();
        let effects = send(&mut state, 1, "a", None, AgentEvent::SessionEnded);
        assert!(effects.contains(&Effect::SetPaneState { pane: pane(1), state: PaneState::Idle }));
        assert!(state.sessions.is_empty());
    }

    #[test]
    fn session_ended_without_a_record_leaves_a_pane_another_session_holds() {
        let mut state = DaemonState::default();
        send(&mut state, 1, "b", Some("p1"), AgentEvent::TurnStarted);
        let effects = update(&mut state, agent_input(2, origin(Some("a"), None), AgentEvent::SessionEnded), EpochMs(2));
        assert!(!effects.iter().any(|effect| matches!(effect, Effect::SetPaneState { .. })));
        assert!(state.sessions.contains_key(&session("b")));
    }

    fn agent_request(input: &DaemonInput) -> &Request {
        let DaemonInput::Received { request, .. } = input else { panic!("an agent input") };
        request
    }

    #[test]
    fn only_turn_ends_failures_input_requests_and_sends_may_offer_a_card() {
        let offering: Vec<bool> = every_event()
            .into_iter()
            .map(|event| may_offer_card(agent_request(&agent_input(1, origin(Some("a"), Some("p1")), event))))
            .collect();
        assert_eq!(offering, vec![false, false, true, true, true, false, true, false, false, false, false]);
        let control = Request::Control { v: WIRE_VERSION, event_id: event_id(1), sent_at_ms: EpochMs(1), control: Control::Jump };
        assert!(!may_offer_card(&control));
        let query = Request::Query { v: WIRE_VERSION, event_id: event_id(1), query: Query::Status };
        assert!(!may_offer_card(&query));
        assert!(!may_offer_card(&Request::Unknown));
    }

    const SESSION_CHOICES: [Option<&str>; 3] = [Some("a"), Some("b"), None];
    const PROMPT_CHOICES: [Option<&str>; 3] = [Some("p1"), Some("p2"), None];

    /// One generated input: an agent event, DND on or off, or a tick.
    fn generated_input(choice: (usize, usize, usize, u32), number: i64) -> DaemonInput {
        let (action, session_choice, prompt_choice, pane_number) = choice;
        let events = every_event();
        match events.get(action) {
            Some(event) => {
                let mut from = origin(SESSION_CHOICES[session_choice], PROMPT_CHOICES[prompt_choice]);
                from.pane = Some(pane(pane_number));
                agent_input(number, from, event.clone())
            }
            None if action == events.len() => dnd_input(number, DndCommand::On { for_minutes: None }),
            None if action == events.len() + 1 => dnd_input(number, DndCommand::Off),
            None => DaemonInput::Tick,
        }
    }

    fn dnd_input(number: i64, command: DndCommand) -> DaemonInput {
        let request = Request::Control { v: WIRE_VERSION, event_id: event_id(number), sent_at_ms: EpochMs(number), control: Control::Dnd { command } };
        DaemonInput::Received { request, focus: FocusSnapshot::unknown() }
    }

    fn looking_at(pane: PaneKey) -> FocusSnapshot {
        FocusSnapshot { frontmost: Frontmost::Terminal, active_pane: Some(pane), idle_seconds: Some(0), screen_locked: Some(false), screen_shared: None }
    }

    fn input_choice() -> impl proptest::strategy::Strategy<Value = (usize, usize, usize, u32)> {
        (0..every_event().len() + 3, 0..SESSION_CHOICES.len(), 0..PROMPT_CHOICES.len(), 1u32..3)
    }

    proptest::proptest! {
        /// The shell reads focus facts only when `may_offer_card` says so. For
        /// every other request the facts must change nothing, from any state.
        #[test]
        fn focus_facts_matter_only_for_requests_that_may_offer_a_card(
            history in proptest::collection::vec((input_choice(), 1i64..20_000), 0..30),
            last in input_choice(),
        ) {
            let mut state = DaemonState::default();
            let mut now = 0;
            for (step, (choice, gap)) in history.into_iter().enumerate() {
                now += gap;
                let number = i64::try_from(step).expect("a short history") + 1;
                let input = match generated_input(choice, number) {
                    DaemonInput::Received { request: Request::Agent { v, event_id, origin, event, .. }, focus } => {
                        DaemonInput::Received { request: Request::Agent { v, event_id, sent_at_ms: EpochMs(now), origin, event }, focus }
                    }
                    other => other,
                };
                update(&mut state, input, EpochMs(now));
            }
            let DaemonInput::Received { request, .. } = generated_input(last, 1_000) else { return Ok(()) };
            let pane_looked_at = pane(last.3);
            let mut without_facts = state.clone();
            let mut with_facts = state;
            let unknown = update(&mut without_facts, DaemonInput::Received { request: request.clone(), focus: FocusSnapshot::unknown() }, EpochMs(now + 1));
            let focused = update(&mut with_facts, DaemonInput::Received { request: request.clone(), focus: looking_at(pane_looked_at) }, EpochMs(now + 1));
            if !may_offer_card(&request) {
                proptest::prop_assert_eq!(&unknown, &focused);
                proptest::prop_assert_eq!(&without_facts, &with_facts);
                proptest::prop_assert!(!has_card_from(&without_facts, 1_000));
                proptest::prop_assert!(!unknown.contains(&Effect::PlaySound));
            }
        }
    }

    #[test]
    fn an_event_that_may_not_offer_a_card_never_makes_one() {
        for event in every_event() {
            for (session_name, prompt_name) in [(Some("a"), Some("p1")), (Some("a"), None), (None, None)] {
                let mut working = DaemonState::default();
                send(&mut working, 0, "a", Some("p1"), AgentEvent::TurnStarted);
                for mut state in [DaemonState::default(), working] {
                    let input = agent_input(1, origin(session_name, prompt_name), event.clone());
                    let offers = may_offer_card(agent_request(&input));
                    update(&mut state, input, EpochMs(1));
                    assert!(offers || !has_card_from(&state, 1), "{event:?} from {session_name:?} made a card");
                }
            }
        }
    }
}
