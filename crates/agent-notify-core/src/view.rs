//! The panel view model: a pure function of [`DaemonState`](crate::state::DaemonState).

use serde::Serialize;

use crate::wire::Kind;
use crate::{CardId, EpochMs};

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
