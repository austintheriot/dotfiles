//! Daemon state. Owned by one actor in the daemon and changed only by
//! [`update`](crate::update::update).

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::view::PanelViewModel;
use crate::wire::{InputReason, Kind, Origin, QuietUntil, RepoInfo, WindowLabel};
use crate::{CardId, DisplayText, EpochMs, EventId, PaneKey, PromptId, SessionId};

/// All decision state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DaemonState {
    /// One record per Claude session.
    pub sessions: BTreeMap<SessionId, SessionRecord>,
    /// Cards from `send` with no session.
    pub oneoff_cards: Vec<OneoffCard>,
    /// Do-not-disturb.
    pub dnd: Dnd,
    /// The focused tmux pane, from the pane focus hooks.
    pub focus: Option<FocusedPane>,
    /// True while the pointer is over the stack.
    pub stack_hovered: bool,
    /// The view shown when the pointer entered the stack.
    pub frozen_view: Option<PanelViewModel>,
    /// True while the screen is shared. Hides card bodies.
    pub screen_shared: bool,
    /// The last time a sound played.
    pub last_sound_at: Option<EpochMs>,
    /// Increases with every update.
    pub revision: u64,
}

impl DaemonState {
    /// True while do-not-disturb is on at `now`.
    #[must_use]
    pub fn dnd_active(&self, now: EpochMs) -> bool {
        match self.dnd {
            Dnd::Off => false,
            Dnd::On { until: None } => true,
            Dnd::On { until: Some(until) } => now < until,
        }
    }
}

/// What the daemon knows about one session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    /// The latest known pane.
    pub pane: Option<PaneKey>,
    /// The latest known window.
    pub window: Option<WindowLabel>,
    /// The repository, outside tmux.
    pub repo: Option<RepoInfo>,
    /// The working directory.
    pub cwd: PathBuf,
    /// The latest session title.
    pub title: Option<DisplayText>,
    /// The current turn.
    pub turn: Turn,
    /// True once the agent called `send` in the current turn, in any turn
    /// state. A new prompt's turn resets it.
    pub explicit_sent: bool,
    /// Whether the `Stop` backup is silenced.
    pub quiet: QuietMode,
    /// At most one card per session.
    pub card: Option<Card>,
}

impl SessionRecord {
    /// A record for a session first seen in `origin`.
    #[must_use]
    pub fn new(origin: &Origin) -> Self {
        SessionRecord {
            pane: origin.pane.clone(),
            window: origin.window.clone(),
            repo: origin.repo.clone(),
            cwd: origin.cwd.clone(),
            title: origin.session_title.clone(),
            turn: Turn::Idle,
            explicit_sent: false,
            quiet: QuietMode::Armed,
            card: None,
        }
    }

    /// Takes the newer location and title facts from `origin`. Absent facts
    /// keep the old value, so a Bash `send` without a title keeps the title.
    pub fn absorb(&mut self, origin: &Origin) {
        if origin.pane.is_some() {
            self.pane.clone_from(&origin.pane);
        }
        if origin.window.is_some() {
            self.window.clone_from(&origin.window);
        }
        if origin.repo.is_some() {
            self.repo.clone_from(&origin.repo);
        }
        if origin.session_title.is_some() {
            self.title.clone_from(&origin.session_title);
        }
        self.cwd.clone_from(&origin.cwd);
    }
}

/// A card from `send` with no session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneoffCard {
    /// Line 1, fixed when the card was made.
    pub location: String,
    /// The pane, when known.
    pub pane: Option<PaneKey>,
    /// The card.
    pub card: Card,
}

/// One card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    /// Fresh for every card.
    pub id: CardId,
    /// Info or attention.
    pub kind: Kind,
    /// The event that made it.
    pub cause: CardCause,
    /// Line 3.
    pub body: DisplayText,
    /// The request that made it.
    pub event_id: EventId,
    /// When it was made.
    pub created: EpochMs,
    /// Where it is in its life.
    pub phase: CardPhase,
    /// A status line after a click.
    pub note: Option<CardNote>,
}

/// The event that made a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardCause {
    /// `agent-notify send`.
    Explicit,
    /// The `Stop` backup.
    TurnEnded,
    /// `StopFailure`.
    TurnFailed,
    /// A permission prompt or a question.
    NeedsInput(InputReason),
}

/// Where a card is in its life.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardPhase {
    /// On screen. Info cards fold at `fade_at`.
    Showing {
        /// When an info card folds. `None` for attention cards.
        fade_at: Option<EpochMs>,
    },
    /// In the finished row.
    Folded,
    /// Hidden by the close button until `until`. It shows again only while
    /// the session still waits on the user or still failed in `prompt`.
    Snoozed {
        /// When it shows again.
        until: EpochMs,
        /// The session's prompt when the card was snoozed.
        prompt: Option<PromptId>,
    },
    /// Showing a note, removed at `remove_at`.
    Expiring {
        /// When it goes.
        remove_at: EpochMs,
    },
}

/// A status line shown on a card after a click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardNote {
    /// No pane carries the session tag any more.
    SessionEnded,
    /// tmux switched, the window did not come forward.
    NotRaised,
    /// tmux refused the switch.
    SwitchFailed(DisplayText),
}

/// The current turn of a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Turn {
    /// No turn seen yet.
    Idle,
    /// The agent is working.
    Working {
        /// The turn.
        prompt: Option<PromptId>,
    },
    /// The turn ended.
    Finished {
        /// The turn.
        prompt: Option<PromptId>,
    },
    /// The agent waits for the user.
    NeedsYou {
        /// The turn.
        prompt: Option<PromptId>,
        /// Why it waits.
        reason: InputReason,
    },
    /// The turn stopped on an error.
    Failed {
        /// The turn.
        prompt: Option<PromptId>,
    },
}

impl Turn {
    /// The turn's prompt id, when known.
    #[must_use]
    pub fn prompt(&self) -> Option<&PromptId> {
        match self {
            Turn::Idle => None,
            Turn::Working { prompt }
            | Turn::Finished { prompt }
            | Turn::NeedsYou { prompt, .. }
            | Turn::Failed { prompt } => prompt.as_ref(),
        }
    }
}

/// Whether the `Stop` backup is silenced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuietMode {
    /// `TurnEnded` may show a card.
    Armed,
    /// `TurnEnded` shows no card until `until` happens or `deadline` passes.
    Until {
        /// The event that ends quiet.
        until: QuietUntil,
        /// The latest end.
        deadline: EpochMs,
    },
}

impl QuietMode {
    /// True when `event` ends this quiet mode.
    #[must_use]
    pub fn ends_on(self, event: QuietUntil) -> bool {
        matches!(self, QuietMode::Until { until, .. } if until == event)
    }
}

/// Do-not-disturb.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dnd {
    /// Cards and sounds as usual.
    #[default]
    Off,
    /// No cards on screen, no sounds.
    On {
        /// The end, or `None` until `dnd off`.
        until: Option<EpochMs>,
    },
}

/// The tmux pane that has focus, and since when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusedPane {
    /// The pane.
    pub pane: PaneKey,
    /// When it gained focus.
    pub since: EpochMs,
}
