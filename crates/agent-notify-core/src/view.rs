//! The panel view model: a pure function of [`DaemonState`](crate::state::DaemonState).

use serde::Serialize;

use crate::label::location_line;
use crate::state::{Card, CardNote, CardPhase, DaemonState, FocusedPane};
use crate::wire::Kind;
use crate::{CardId, DisplayText, EpochMs, PaneKey};

/// Everything the toast panel draws.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PanelViewModel {
    /// Increases with every update. The UI drops a push with a lower value.
    pub revision: u64,
    /// Cards, newest first.
    pub cards: Vec<CardView>,
    /// The "Finished: 2, 4, 6" row, oldest first.
    pub finished_row: Vec<FinishedEntry>,
    /// True while do-not-disturb hides everything.
    pub dnd: bool,
}

/// One card as drawn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CardView {
    /// The id the UI sends back in intents.
    pub card_id: CardId,
    /// Info or attention.
    pub kind: Kind,
    /// Line 1.
    pub location: String,
    /// Line 2, the session title.
    pub title: Option<String>,
    /// Line 3. `None` while the screen is shared.
    pub body: Option<String>,
    /// For the relative time.
    pub created_ms: EpochMs,
    /// A status line after a click.
    pub note: Option<String>,
}

/// One entry in the finished row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FinishedEntry {
    /// The folded card.
    pub card_id: CardId,
    /// The window index, or the location line outside tmux.
    pub label: String,
}

/// The most info cards on screen at once. Older ones join the finished row.
pub const MAX_VISIBLE_INFO_CARDS: usize = 3;

/// Builds the panel view model from `state`.
#[must_use]
pub fn view(state: &DaemonState, now: EpochMs) -> PanelViewModel {
    if let Some(frozen) = &state.frozen_view {
        return PanelViewModel { revision: state.revision, ..frozen.clone() };
    }
    let dnd = state.dnd_active(now);
    if dnd {
        return PanelViewModel { revision: state.revision, cards: Vec::new(), finished_row: Vec::new(), dnd };
    }
    let mut showing = Vec::new();
    let mut folded = Vec::new();
    for placed in placed_cards(state) {
        if hidden_by_focus(state.focus.as_ref(), &placed) {
            continue;
        }
        match placed.card.phase {
            CardPhase::Showing { .. } | CardPhase::Expiring { .. } => showing.push(placed),
            CardPhase::Folded => folded.push(placed),
            CardPhase::Snoozed { .. } => {}
        }
    }
    showing.sort_by_key(|placed| std::cmp::Reverse(placed.card.created));
    let mut cards = Vec::new();
    let mut info_shown = 0;
    for placed in showing {
        if placed.card.kind == Kind::Info {
            info_shown += 1;
            if info_shown > MAX_VISIBLE_INFO_CARDS {
                folded.push(placed);
                continue;
            }
        }
        cards.push(card_view(&placed, state.screen_shared));
    }
    folded.sort_by_key(|placed| placed.card.created);
    let finished_row = folded.iter().map(finished_entry).collect();
    PanelViewModel { revision: state.revision, cards, finished_row, dnd }
}

struct Placed<'a> {
    location: String,
    window_index: Option<u32>,
    title: Option<&'a DisplayText>,
    pane: Option<&'a PaneKey>,
    card: &'a Card,
}

fn placed_cards(state: &DaemonState) -> Vec<Placed<'_>> {
    let sessions = state.sessions.values().filter_map(|record| {
        record.card.as_ref().map(|card| Placed {
            location: location_line(record.window.as_ref(), record.repo.as_ref(), &record.cwd),
            window_index: record.window.as_ref().map(|window| window.index),
            title: record.title.as_ref(),
            pane: record.pane.as_ref(),
            card,
        })
    });
    let oneoffs = state.oneoff_cards.iter().map(|oneoff| Placed {
        location: oneoff.location.clone(),
        window_index: None,
        title: None,
        pane: oneoff.pane.as_ref(),
        card: &oneoff.card,
    });
    sessions.chain(oneoffs).collect()
}

/// Focusing a pane hides its card at once. A card that arrived while the pane
/// already had focus still shows: the focus check at arrival let it through.
fn hidden_by_focus(focus: Option<&FocusedPane>, placed: &Placed<'_>) -> bool {
    focus.is_some_and(|focus| placed.pane == Some(&focus.pane) && focus.since >= placed.card.created)
}

fn card_view(placed: &Placed<'_>, screen_shared: bool) -> CardView {
    CardView {
        card_id: placed.card.id.clone(),
        kind: placed.card.kind,
        location: placed.location.clone(),
        title: placed.title.map(|title| title.as_str().to_string()),
        body: (!screen_shared).then(|| placed.card.body.as_str().to_string()),
        created_ms: placed.card.created,
        note: placed.card.note.as_ref().map(|note| note_text(note, placed.window_index)),
    }
}

fn note_text(note: &CardNote, window_index: Option<u32>) -> String {
    match note {
        CardNote::SessionEnded => "Session ended".to_string(),
        CardNote::NotRaised => window_index.map_or_else(
            || "Switched, window not raised".to_string(),
            |index| format!("Switched to {index}, window not raised"),
        ),
        CardNote::SwitchFailed(reason) => format!("Switch failed: {}", reason.as_str()),
    }
}

fn finished_entry(placed: &Placed<'_>) -> FinishedEntry {
    FinishedEntry {
        card_id: placed.card.id.clone(),
        label: placed.window_index.map_or_else(|| placed.location.clone(), |index| index.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{CardNote, CardPhase, DaemonState, Dnd, FocusedPane};
    use crate::test_support::{card, pane, send, session};
    use crate::wire::{AgentEvent, InputReason};
    use crate::DisplayText;

    fn needs(state: &mut DaemonState, number: i64, name: &str) {
        send(state, number, name, Some("p1"), AgentEvent::NeedsInput { reason: InputReason::Permission, message: DisplayText::new("Allow?") });
    }

    fn info(state: &mut DaemonState, number: i64, name: &str) {
        send(state, number, name, None, AgentEvent::Explicit { kind: Kind::Info, body: DisplayText::new("done") });
    }

    #[test]
    fn cards_are_newest_first_with_their_lines() {
        let mut state = DaemonState::default();
        needs(&mut state, 1, "a");
        info(&mut state, 2, "b");
        let model = view(&state, EpochMs(3));
        assert_eq!(model.revision, state.revision);
        let kinds: Vec<Kind> = model.cards.iter().map(|shown| shown.kind).collect();
        assert_eq!(kinds, vec![Kind::Info, Kind::Attention]);
        assert_eq!(model.cards[1].location, "4 · code/feature-x");
        assert_eq!(model.cards[1].body.as_deref(), Some("Allow?"));
    }

    #[test]
    fn at_most_three_info_cards_show_and_the_rest_fold() {
        let mut state = DaemonState::default();
        for (number, name) in [(1, "a"), (2, "b"), (3, "c"), (4, "d")] {
            info(&mut state, number, name);
        }
        let model = view(&state, EpochMs(5));
        assert_eq!(model.cards.len(), MAX_VISIBLE_INFO_CARDS);
        assert_eq!(model.finished_row.len(), 1);
        assert_eq!(model.finished_row[0].card_id, card(&state, "a").map(|shown| shown.id.clone()).expect("a card"));
        assert_eq!(model.finished_row[0].label, "4");
    }

    #[test]
    fn folded_and_snoozed_cards() {
        let mut state = DaemonState::default();
        info(&mut state, 1, "a");
        needs(&mut state, 2, "b");
        if let Some(record) = state.sessions.get_mut(&session("a")) && let Some(shown) = &mut record.card {
            shown.phase = CardPhase::Folded;
        }
        if let Some(record) = state.sessions.get_mut(&session("b")) && let Some(shown) = &mut record.card {
            shown.phase = CardPhase::Snoozed { until: EpochMs(1_000_000) };
        }
        let model = view(&state, EpochMs(3));
        assert!(model.cards.is_empty());
        assert_eq!(model.finished_row.len(), 1);
    }

    #[test]
    fn a_pane_focused_after_its_card_hides_the_card() {
        let mut state = DaemonState::default();
        needs(&mut state, 1, "a");
        state.focus = Some(FocusedPane { pane: pane(1), since: EpochMs(2) });
        assert!(view(&state, EpochMs(2)).cards.is_empty());
        state.focus = Some(FocusedPane { pane: pane(1), since: EpochMs(0) });
        assert_eq!(view(&state, EpochMs(2)).cards.len(), 1, "a card that arrived during focus still shows");
    }

    #[test]
    fn dnd_hides_everything() {
        let mut state = DaemonState::default();
        needs(&mut state, 1, "a");
        state.dnd = Dnd::On { until: None };
        let model = view(&state, EpochMs(2));
        assert!(model.dnd);
        assert!(model.cards.is_empty() && model.finished_row.is_empty());
    }

    #[test]
    fn a_shared_screen_hides_bodies() {
        let mut state = DaemonState::default();
        needs(&mut state, 1, "a");
        state.screen_shared = true;
        assert_eq!(view(&state, EpochMs(2)).cards[0].body, None);
    }

    #[test]
    fn notes_render_with_the_window_index() {
        let mut state = DaemonState::default();
        needs(&mut state, 1, "a");
        if let Some(record) = state.sessions.get_mut(&session("a")) && let Some(shown) = &mut record.card {
            shown.note = Some(CardNote::NotRaised);
        }
        assert_eq!(view(&state, EpochMs(2)).cards[0].note.as_deref(), Some("Switched to 4, window not raised"));
    }

    #[test]
    fn a_frozen_view_is_returned_with_the_current_revision() {
        let mut state = DaemonState::default();
        needs(&mut state, 1, "a");
        state.frozen_view = Some(view(&state, EpochMs(1)));
        info(&mut state, 2, "b");
        let model = view(&state, EpochMs(3));
        assert_eq!(model.cards.len(), 1, "new cards wait while the pointer is over the stack");
        assert_eq!(model.revision, state.revision);
    }
}
