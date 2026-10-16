//! Properties of `update` over generated input sequences for one session, and
//! of `plan_focus` over generated tmux worlds. The reference model restates
//! the turn rules of spec 6.2 as the code applies them: `explicit_sent` is per
//! turn, a mid-turn event with an unseen prompt adopts that prompt as a new
//! turn when it was sent after the current turn started, and an event is
//! stale for another prompt (`TurnEnded`), for a turn that already ended, or
//! for an unseen prompt sent at or before the current turn's start.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use agent_notify_core::focus::{FocusPlan, FocusWorld, WorldClient, WorldPane, plan_focus};
use agent_notify_core::state::{Card, CardCause, CardPhase, DaemonState, QuietMode, SessionRecord, Turn};
use agent_notify_core::update::{
    DaemonInput, Effect, FOCUS_DWELL_MS, FocusOutcome, FocusSnapshot, FocusTarget, PaneSnapshot, PaneTag, SNOOZE_MS, SuppressReason, UiIntent, update,
};
use agent_notify_core::wire::{AgentEvent, AgentKind, Control, InputReason, Kind, Origin, QuietUntil, Request, WIRE_VERSION};
use agent_notify_core::{CardId, DisplayText, EpochMs, EventId, PaneId, PaneKey, PromptId, ServerId, SessionId, SocketPath};
use proptest::collection::vec;
use proptest::prelude::*;

const QUIET_MAX_MINUTES: u16 = 120;
const QUIET_MAX_MS: i64 = 120 * 60_000;
const SESSION_PANE: u32 = 1;

#[derive(Debug, Clone)]
enum Op {
    Start(Option<u8>),
    Explicit(Kind, Option<u8>),
    End(Option<u8>),
    Fail(Option<u8>),
    Needs(Option<u8>, InputReason),
    Resolved,
    Quiet(QuietUntil),
    QuietOff,
    SessionEnded,
    Snapshot { tagged: bool },
    Focus { pane: u32, focused: bool },
    Hover(bool),
    Close,
    Click,
    Outcome(FocusOutcome),
    Tick { advance_ms: i64 },
}

impl Op {
    fn is_agent_event(&self) -> bool {
        matches!(
            self,
            Op::Start(_) | Op::Explicit(..) | Op::End(_) | Op::Fail(_) | Op::Needs(..) | Op::Resolved | Op::Quiet(_) | Op::QuietOff | Op::SessionEnded
        )
    }
}

fn maybe_prompt() -> impl Strategy<Value = Option<u8>> {
    prop_oneof![4 => (0_u8..3).prop_map(Some), 1 => Just(None)]
}

fn agent_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => maybe_prompt().prop_map(Op::Start),
        3 => (prop_oneof![Just(Kind::Info), Just(Kind::Attention)], maybe_prompt()).prop_map(|(kind, prompt)| Op::Explicit(kind, prompt)),
        3 => maybe_prompt().prop_map(Op::End),
        2 => maybe_prompt().prop_map(Op::Fail),
        3 => (maybe_prompt(), prop_oneof![Just(InputReason::Permission), Just(InputReason::Elicitation)]).prop_map(|(prompt, reason)| Op::Needs(prompt, reason)),
        2 => Just(Op::Resolved),
        2 => prop_oneof![Just(QuietUntil::NextSend), Just(QuietUntil::NextPrompt)].prop_map(Op::Quiet),
        1 => Just(Op::QuietOff),
        1 => Just(Op::SessionEnded),
    ]
}

fn shell_op() -> impl Strategy<Value = Op> {
    let outcome = prop_oneof![
        Just(FocusOutcome::Done),
        Just(FocusOutcome::SessionGone),
        Just(FocusOutcome::TmuxFailed(DisplayText::new("refused"))),
        Just(FocusOutcome::RaiseFailed),
    ];
    let advance = prop_oneof![Just(0_i64), Just(500), Just(FOCUS_DWELL_MS), Just(6_300), Just(SNOOZE_MS), Just(QUIET_MAX_MS)];
    prop_oneof![
        1 => any::<bool>().prop_map(|tagged| Op::Snapshot { tagged }),
        2 => (1_u32..3, any::<bool>()).prop_map(|(pane, focused)| Op::Focus { pane, focused }),
        1 => any::<bool>().prop_map(Op::Hover),
        2 => Just(Op::Close),
        1 => Just(Op::Click),
        2 => outcome.prop_map(Op::Outcome),
        3 => advance.prop_map(|advance_ms| Op::Tick { advance_ms }),
    ]
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![3 => agent_op(), 2 => shell_op()]
}

fn ops() -> impl Strategy<Value = Vec<Op>> {
    vec(op(), 1..60)
}

fn session_named(index: u8) -> SessionId {
    SessionId::parse(&format!("s{index}")).expect("a session id")
}

fn session() -> SessionId {
    session_named(0)
}

fn prompt_id(number: u8) -> PromptId {
    PromptId::parse(&format!("p{number}")).expect("a prompt id")
}

fn prompt_number(prompt: &PromptId) -> u8 {
    prompt.as_str().strip_prefix('p').and_then(|digits| digits.parse().ok()).expect("a prompt id this test made")
}

fn event_id(number: usize) -> EventId {
    EventId::from_parts(EpochMs(i64::try_from(number).expect("small")), [0; 10])
}

fn server() -> ServerId {
    ServerId { socket_path: SocketPath::parse(Path::new("/tmp/tmux-501/default")).expect("absolute"), pid: 1, start_time: 1 }
}

fn pane_id(number: u32) -> PaneId {
    PaneId::parse(&format!("%{number}")).expect("a pane id")
}

fn pane_key(number: u32) -> PaneKey {
    PaneKey { server: server(), pane: pane_id(number) }
}

fn agent_event(op: &Op) -> Option<(Option<u8>, AgentEvent)> {
    let text = DisplayText::new;
    Some(match op {
        Op::Start(prompt) => (*prompt, AgentEvent::TurnStarted),
        Op::Explicit(kind, prompt) => (*prompt, AgentEvent::Explicit { kind: *kind, body: text("x") }),
        Op::End(prompt) => (*prompt, AgentEvent::TurnEnded { last_message: Some(text("Done.")) }),
        Op::Fail(prompt) => (*prompt, AgentEvent::TurnFailed { error: text("x") }),
        Op::Needs(prompt, reason) => (*prompt, AgentEvent::NeedsInput { reason: *reason, message: text("x") }),
        Op::Resolved => (None, AgentEvent::InputResolved),
        Op::Quiet(until) => (None, AgentEvent::Quiet { until: *until, max_minutes: QUIET_MAX_MINUTES }),
        Op::QuietOff => (None, AgentEvent::QuietOff),
        Op::SessionEnded => (None, AgentEvent::SessionEnded),
        _ => return None,
    })
}

/// What one input did: its effects and the id a card from it would carry.
struct Applied {
    effects: Vec<Effect>,
    card_id: CardId,
}

impl Applied {
    fn suppressed(&self, wanted: SuppressReason) -> bool {
        self.effects.iter().any(|effect| matches!(effect, Effect::Suppressed { reason, .. } if *reason == wanted))
    }

    fn explained(&self) -> bool {
        self.effects.iter().any(|effect| matches!(effect, Effect::Suppressed { .. }))
    }
}

/// The daemon state for one session in pane `%1`, and the clock.
struct World {
    state: DaemonState,
    now: i64,
    sent: usize,
}

impl World {
    fn new() -> Self {
        World { state: DaemonState::default(), now: 0, sent: 0 }
    }

    fn record(&self) -> Option<&SessionRecord> {
        self.state.sessions.get(&session())
    }

    fn card(&self) -> Option<&Card> {
        self.record().and_then(|record| record.card.as_ref())
    }

    fn apply(&mut self, op: &Op) -> Applied {
        self.sent += 1;
        if let Op::Tick { advance_ms } = op {
            self.now += advance_ms;
        }
        let event = event_id(self.sent);
        let input = self.input(op, event.clone());
        let effects = update(&mut self.state, input, EpochMs(self.now));
        Applied { effects, card_id: CardId::for_event(&event) }
    }

    fn targeted_card(&self) -> CardId {
        self.card().map_or_else(|| CardId::for_event(&event_id(0)), |card| card.id.clone())
    }

    fn input(&self, op: &Op, event: EventId) -> DaemonInput {
        if let Some((prompt, agent_event)) = agent_event(op) {
            let origin = Origin {
                agent: AgentKind::ClaudeCode,
                session: Some(session()),
                prompt: prompt.map(prompt_id),
                pane: Some(pane_key(SESSION_PANE)),
                window: None,
                repo: None,
                session_title: None,
                cwd: PathBuf::from("/x"),
            };
            let request = Request::Agent { v: WIRE_VERSION, event_id: event, sent_at_ms: EpochMs(self.now), origin, event: agent_event };
            return DaemonInput::Received { request, focus: FocusSnapshot::unknown() };
        }
        match op {
            Op::Snapshot { tagged } => {
                let panes = if *tagged { vec![PaneTag { pane: pane_id(SESSION_PANE), session: Some(session()) }] } else { Vec::new() };
                DaemonInput::PaneSnapshot(PaneSnapshot { server: server(), panes })
            }
            Op::Focus { pane, focused } => DaemonInput::Received {
                request: Request::Control { v: WIRE_VERSION, event_id: event, sent_at_ms: EpochMs(self.now), control: Control::FocusChanged { pane: pane_key(*pane), focused: *focused } },
                focus: FocusSnapshot::unknown(),
            },
            Op::Hover(hovered) => DaemonInput::Ui(UiIntent::HoverChanged { hovered: *hovered }),
            Op::Close => DaemonInput::Ui(UiIntent::Closed { card_id: self.targeted_card() }),
            Op::Click => DaemonInput::Ui(UiIntent::Clicked { card_id: self.targeted_card() }),
            Op::Outcome(outcome) => DaemonInput::FocusOutcome { card: self.targeted_card(), outcome: outcome.clone() },
            _ => DaemonInput::Tick,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Working,
    NeedsYou,
    Finished,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TurnMatch {
    Current,
    Ended,
    Other,
    Older,
}

/// The reference model of one session record's turn facts.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ModelRecord {
    prompt: Option<u8>,
    phase: Phase,
    turn_started_at: Option<i64>,
    explicit_sent: bool,
    quiet: Option<(QuietUntil, i64)>,
}

impl ModelRecord {
    fn fresh() -> Self {
        ModelRecord { prompt: None, phase: Phase::Idle, turn_started_at: None, explicit_sent: false, quiet: None }
    }

    fn observed(record: &SessionRecord) -> Self {
        let (phase, prompt) = match &record.turn {
            Turn::Idle => (Phase::Idle, None),
            Turn::Working { prompt } => (Phase::Working, prompt.as_ref()),
            Turn::NeedsYou { prompt, .. } => (Phase::NeedsYou, prompt.as_ref()),
            Turn::Finished { prompt } => (Phase::Finished, prompt.as_ref()),
            Turn::Failed { prompt } => (Phase::Failed, prompt.as_ref()),
        };
        let quiet = match record.quiet {
            QuietMode::Armed => None,
            QuietMode::Until { until, deadline } => Some((until, deadline.0)),
        };
        let turn_started_at = record.turn_started_at.map(|started| started.0);
        ModelRecord { prompt: prompt.map(prompt_number), phase, turn_started_at, explicit_sent: record.explicit_sent, quiet }
    }

    /// A record with no prompt, or an event with none, means the current turn.
    /// An unseen prompt is a new turn only when it was sent after the current
    /// turn started. Otherwise it is stale, like a repeat of an ended turn.
    fn turn_match(&self, prompt: Option<u8>, sent_at: i64) -> TurnMatch {
        match (prompt, self.prompt) {
            (Some(event_prompt), Some(current)) if event_prompt != current => {
                if self.turn_started_at.is_some_and(|started| sent_at <= started) { TurnMatch::Older } else { TurnMatch::Other }
            }
            (Some(_), Some(_)) if matches!(self.phase, Phase::Finished | Phase::Failed) => TurnMatch::Ended,
            _ => TurnMatch::Current,
        }
    }

    fn begin_turn(&mut self, prompt: Option<u8>, sent_at: i64) {
        self.prompt = prompt;
        self.phase = Phase::Working;
        self.turn_started_at = Some(sent_at);
        self.explicit_sent = false;
        if matches!(self.quiet, Some((QuietUntil::NextPrompt, _))) {
            self.quiet = None;
        }
    }

    fn expire_quiet(&mut self, now: i64) {
        if self.quiet.is_some_and(|(_, deadline)| now >= deadline) {
            self.quiet = None;
        }
    }

    fn enter_mid_turn(&mut self, prompt: Option<u8>, phase: Phase, sent_at: i64) {
        match self.turn_match(prompt, sent_at) {
            TurnMatch::Ended | TurnMatch::Older => return,
            TurnMatch::Other => self.begin_turn(prompt, sent_at),
            TurnMatch::Current => {}
        }
        self.prompt = prompt.or(self.prompt);
        self.phase = phase;
    }

    /// True when `TurnEnded` for `prompt`, sent and received at `now`, passes
    /// the turn rules. A pending attention card is not part of the turn facts.
    fn turn_end_offers_info(&self, prompt: Option<u8>, now: i64) -> bool {
        let mut after_expiry = self.clone();
        after_expiry.expire_quiet(now);
        self.turn_match(prompt, now) == TurnMatch::Current && !self.explicit_sent && after_expiry.quiet.is_none()
    }

    /// The test world sends every request at the time it arrives, so
    /// `sent_at` is `now`. Events between two ticks share one sent time.
    fn apply_agent_event(&mut self, op: &Op, now: i64) {
        let sent_at = now;
        match op {
            Op::Start(prompt) => self.begin_turn(*prompt, sent_at),
            Op::Explicit(_, prompt) => {
                match self.turn_match(*prompt, sent_at) {
                    TurnMatch::Older => return,
                    TurnMatch::Other => self.begin_turn(*prompt, sent_at),
                    TurnMatch::Current | TurnMatch::Ended => {}
                }
                self.explicit_sent = true;
                if matches!(self.quiet, Some((QuietUntil::NextSend, _))) {
                    self.quiet = None;
                }
            }
            Op::End(prompt) if self.turn_match(*prompt, sent_at) == TurnMatch::Current => {
                self.prompt = prompt.or(self.prompt);
                self.phase = Phase::Finished;
                self.expire_quiet(now);
            }
            Op::Fail(prompt) => self.enter_mid_turn(*prompt, Phase::Failed, sent_at),
            Op::Needs(prompt, _) => self.enter_mid_turn(*prompt, Phase::NeedsYou, sent_at),
            Op::Resolved if self.phase == Phase::NeedsYou => self.phase = Phase::Working,
            Op::Quiet(until) => self.quiet = Some((*until, now + QUIET_MAX_MS)),
            Op::QuietOff => self.quiet = None,
            _ => {}
        }
    }
}

/// The reference model of the session: absent until an event creates it.
#[derive(Debug, Clone, Default)]
struct Model {
    record: Option<ModelRecord>,
}

impl Model {
    fn record_or_fresh(&self) -> ModelRecord {
        self.record.clone().unwrap_or_else(ModelRecord::fresh)
    }

    fn apply(&mut self, op: &Op, now: i64) {
        match op {
            Op::SessionEnded | Op::Snapshot { tagged: false } => self.record = None,
            Op::Tick { .. } => {
                if let Some(record) = &mut self.record {
                    record.expire_quiet(now);
                }
            }
            agent if agent.is_agent_event() => self.record.get_or_insert_with(ModelRecord::fresh).apply_agent_event(agent, now),
            _ => {}
        }
    }
}

fn outranks_an_info_card(card: &Card) -> bool {
    card.kind == Kind::Attention && !matches!(card.phase, CardPhase::Expiring { .. } | CardPhase::Snoozed { .. })
}

fn dwell_cleared(before: &DaemonState, card: &Card, now: i64) -> bool {
    before
        .focus
        .as_ref()
        .is_some_and(|focus| focus.pane == pane_key(SESSION_PANE) && EpochMs(now).since(focus.since) >= FOCUS_DWELL_MS && card.created <= focus.since)
}

/// The exits an attention card may take: replacement by `merge`, the user's
/// close (a snooze that later lapses), the session ending, a new turn, the
/// turn ending or an elicitation resolving, a focus outcome, or focus dwell.
fn attention_exit_allowed(before: &DaemonState, card: &Card, op: &Op, applied: &Applied, after: &World) -> bool {
    if after.card().is_some_and(|shown| shown.id == applied.card_id) {
        return true;
    }
    match op {
        Op::SessionEnded | Op::Snapshot { tagged: false } => after.record().is_none(),
        Op::Start(_) | Op::Outcome(FocusOutcome::Done) => true,
        Op::End(_) => matches!(card.cause, CardCause::NeedsInput(_)) && !applied.suppressed(SuppressReason::Stale),
        Op::Resolved => card.cause == CardCause::NeedsInput(InputReason::Elicitation),
        Op::Tick { .. } => timer_lapsed(before, card, after.now) || dwell_cleared(before, card, after.now),
        _ => false,
    }
}

/// A snoozed card lapses at `until` once its session no longer waits in the
/// snoozed turn. An expiring card lapses at `remove_at`.
fn timer_lapsed(before: &DaemonState, card: &Card, now: i64) -> bool {
    match &card.phase {
        CardPhase::Snoozed { until, prompt } => {
            let still_waiting = before
                .sessions
                .get(&session())
                .is_some_and(|record| matches!(record.turn, Turn::NeedsYou { .. } | Turn::Failed { .. }) && record.turn.prompt() == prompt.as_ref());
            now >= until.0 && !still_waiting
        }
        CardPhase::Expiring { remove_at } => now >= remove_at.0,
        _ => false,
    }
}

fn world_pane() -> impl Strategy<Value = WorldPane> {
    (0_u32..6, proptest::option::of(0_u8..2), 0_u8..3).prop_map(|(number, tag, tmux)| WorldPane { pane: pane_id(number), session_tag: tag.map(session_named), tmux_session: format!("t{tmux}") })
}

fn world_client() -> impl Strategy<Value = WorldClient> {
    (0_u8..3, 0_u8..3, any::<i64>()).prop_map(|(tty, tmux, activity)| WorldClient { tty: format!("/dev/ttys00{tty}"), tmux_session: format!("t{tmux}"), activity })
}

fn focus_world() -> impl Strategy<Value = Option<FocusWorld>> {
    proptest::option::of((vec(world_pane(), 0..6), vec(world_client(), 0..4)).prop_map(|(panes, clients)| FocusWorld { panes, clients }))
}

fn focus_target() -> impl Strategy<Value = FocusTarget> {
    prop_oneof![
        1 => Just(FocusTarget::RaiseOnly),
        4 => (0_u8..2, 0_u32..6).prop_map(|(index, last)| FocusTarget::Session { session: session_named(index), pane: pane_key(last) }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn every_agent_event_yields_a_new_card_or_a_reason(ops in ops()) {
        let mut world = World::new();
        for op in &ops {
            let applied = world.apply(op);
            if op.is_agent_event() {
                let new_card = world.card().is_some_and(|card| card.id == applied.card_id);
                prop_assert!(applied.explained() || new_card, "{:?} gave neither a new card nor a reason in {:?}", op, ops);
            }
        }
    }

    #[test]
    fn the_model_tracks_the_session_record(ops in ops()) {
        let mut world = World::new();
        let mut model = Model::default();
        for op in &ops {
            world.apply(op);
            model.apply(op, world.now);
            prop_assert_eq!(world.record().map(ModelRecord::observed), model.record.clone(), "after {:?} in {:?}", op, ops);
        }
    }

    /// The card was made in the record's current turn, or in a turn with no
    /// known prompt, unless it is an attention card that outranked an info
    /// `send` whose prompt adopted a new turn.
    #[test]
    fn a_card_belongs_to_the_current_turn_unless_it_outranked_an_adopting_send(ops in ops()) {
        let mut world = World::new();
        let mut made_in: BTreeMap<CardId, Option<PromptId>> = BTreeMap::new();
        let mut outranked_an_adoption: BTreeSet<CardId> = BTreeSet::new();
        for op in &ops {
            let prompt_before = world.record().and_then(|record| record.turn.prompt().cloned());
            let card_before = world.card().map(|card| card.id.clone());
            let applied = world.apply(op);
            let Some(record) = world.record() else { continue };
            let current = record.turn.prompt().cloned();
            if matches!(op, Op::Explicit(..)) && applied.suppressed(SuppressReason::Outranked) && current != prompt_before
                && let Some(kept) = card_before
            {
                outranked_an_adoption.insert(kept);
            }
            let Some(card) = &record.card else { continue };
            if card.id == applied.card_id {
                made_in.insert(card.id.clone(), current.clone());
            }
            let made = made_in.get(&card.id).expect("every card comes from an event this test sent");
            let orphan_allowed = outranked_an_adoption.contains(&card.id) && card.kind == Kind::Attention;
            prop_assert!(made.is_none() || *made == current || orphan_allowed, "card {:?} from {:?} under turn {:?} after {:?} in {:?}", card, made, current, op, ops);
        }
    }

    #[test]
    fn turn_ended_shows_info_exactly_when_the_model_says_so(ops in ops()) {
        let mut world = World::new();
        let mut model = Model::default();
        for op in &ops {
            let expectation = match op {
                Op::End(prompt) => {
                    let pending_attention = world.card().is_some_and(|card| outranks_an_info_card(card) && !matches!(card.cause, CardCause::NeedsInput(_)));
                    Some(model.record_or_fresh().turn_end_offers_info(*prompt, world.now) && !pending_attention)
                }
                _ => None,
            };
            let applied = world.apply(op);
            model.apply(op, world.now);
            if let Some(expect_info) = expectation {
                let shown = world.card().is_some_and(|card| card.id == applied.card_id && card.cause == CardCause::TurnEnded);
                prop_assert_eq!(shown, expect_info, "after {:?} in {:?}", op, ops);
            }
        }
    }

    #[test]
    fn quiet_always_ends_by_its_deadline(ops in ops()) {
        let mut world = World::new();
        for op in &ops {
            world.apply(op);
            if let (Op::Tick { .. }, Some(QuietMode::Until { deadline, .. })) = (op, world.record().map(|record| record.quiet)) {
                prop_assert!(world.now < deadline.0, "quiet outlived its deadline {:?} at {} in {:?}", deadline, world.now, ops);
            }
        }
    }

    #[test]
    fn an_attention_card_leaves_only_through_a_listed_exit(ops in ops()) {
        let mut world = World::new();
        for op in &ops {
            let before = world.state.clone();
            let attention = world.card().filter(|card| card.kind == Kind::Attention).cloned();
            let applied = world.apply(op);
            let Some(card) = attention else { continue };
            if world.card().is_some_and(|shown| shown.id == card.id) {
                continue;
            }
            prop_assert!(attention_exit_allowed(&before, &card, op, &applied, &world), "{:?} removed {:?} in {:?}", op, card, ops);
        }
    }

    #[test]
    fn plan_focus_is_total_and_lands_on_a_pane_tagged_for_the_session(target in focus_target(), world in focus_world()) {
        let plan = plan_focus(&target, world.as_ref());
        match (&target, &world) {
            (FocusTarget::RaiseOnly, _) => prop_assert_eq!(plan, FocusPlan::RaiseOnly),
            (FocusTarget::Session { .. }, None) => prop_assert_eq!(plan, FocusPlan::SessionGone),
            (FocusTarget::Session { session, pane: last }, Some(world)) => {
                let tagged: Vec<&WorldPane> = world.panes.iter().filter(|candidate| candidate.session_tag.as_ref() == Some(session)).collect();
                if tagged.is_empty() {
                    prop_assert_eq!(plan, FocusPlan::SessionGone);
                } else {
                    let FocusPlan::Focus { pane, .. } = &plan else {
                        return Err(TestCaseError::fail(format!("{plan:?} for a session with a tagged pane in {world:?}")));
                    };
                    prop_assert!(tagged.iter().any(|candidate| candidate.pane == *pane), "{:?} is not tagged {:?} in {:?}", pane, session, world);
                    if tagged.iter().any(|candidate| candidate.pane == last.pane) {
                        prop_assert_eq!(pane, &last.pane);
                    }
                }
            }
        }
    }
}
