//! The platform services the daemon calls. Each trait has one
//! implementation per platform (macOS here, Wayland and X11 in plan 3) plus
//! a neutral one. `main` builds the record once; nothing reaches for a
//! global.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_notify_core::EpochMs;
use agent_notify_core::status::DaemonFacts;
use agent_notify_core::update::Frontmost;
use agent_notify_core::view::PanelViewModel;

use crate::dnd_store::DndStore;
use crate::tmux::Tmux;

/// Draws the toast stack.
///
/// The actor task calls [`ToastPresenter::present`] on a Tokio worker
/// thread, so it must return at once: hand the view to the UI thread and
/// never wait on it, on IO or on a child process.
pub trait ToastPresenter: Send + Sync {
    /// Shows `view`, or hides the panel when it has no cards and no finished row.
    fn present(&self, view: &PanelViewModel);
}

/// Brings the terminal forward.
pub trait WindowRaiser: Send + Sync {
    /// Raises the terminal window. False when it did not come forward.
    fn raise(&self) -> bool;
    /// Starts a new terminal window attached to a tmux session. False when
    /// the terminal did not start.
    fn open_terminal(&self, request: &AttachRequest) -> bool;
}

/// A terminal to start, attached to a tmux session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachRequest {
    /// The tmux binary.
    pub tmux: PathBuf,
    /// The tmux server socket.
    pub socket: PathBuf,
    /// The tmux session id to attach, for example `$3`. Pass it as `-t`
    /// as it is: the `=` prefix matches names only.
    pub session: String,
}

/// Reads which app is frontmost.
pub trait FrontmostReader: Send + Sync {
    /// The frontmost app, or `Unknown` when it cannot be read.
    fn frontmost(&self) -> Frontmost;
}

/// Facts about the user's presence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Presence {
    /// Seconds since the last input event.
    pub idle_seconds: Option<u32>,
    /// True while the screen is locked.
    pub screen_locked: Option<bool>,
}

/// Reads idle time and screen lock.
pub trait PresenceReader: Send + Sync {
    /// The current facts. Unreadable facts are `None`, which the core treats as "show".
    fn presence(&self) -> Presence;
}

/// Plays the attention sound.
///
/// The actor task calls [`SoundPlayer::play`] on a Tokio worker thread, so
/// it must return at once: spawn the player and never wait for it to end.
pub trait SoundPlayer: Send + Sync {
    /// Starts the sound and returns at once.
    fn play(&self);
}

/// The wall clock, injected so tests can fix the time.
pub type Clock = Arc<dyn Fn() -> EpochMs + Send + Sync>;

/// Every service the daemon uses, built once by `main`.
///
/// The frontmost and presence readers, and every tmux call, block their
/// thread. The daemon runs them only on its effects worker or on a blocking
/// thread, never on a Tokio worker thread.
pub struct Services {
    /// tmux, when found.
    pub tmux: Option<Tmux>,
    /// The terminal raiser. Runs on the effects worker.
    pub raiser: Arc<dyn WindowRaiser>,
    /// The toast panel. Called from the actor.
    pub presenter: Arc<dyn ToastPresenter>,
    /// The frontmost-app reader. Runs on a blocking thread.
    pub frontmost: Arc<dyn FrontmostReader>,
    /// The presence reader. Runs on a blocking thread.
    pub presence: Arc<dyn PresenceReader>,
    /// The attention sound. Called from the actor.
    pub sound: Arc<dyn SoundPlayer>,
    /// The DND file. Saved on the effects worker.
    pub dnd_store: DndStore,
    /// The wall clock.
    pub clock: Clock,
    /// Facts for `status`.
    pub facts: DaemonFacts,
}

/// The system clock. A backwards step is an accepted risk, documented on the
/// core's `update::sent_before_turn_start`.
#[must_use]
pub fn system_clock() -> Clock {
    Arc::new(|| {
        let millis = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis());
        EpochMs(i64::try_from(millis).unwrap_or(i64::MAX))
    })
}

/// A raiser for platforms without one. Every raise reports failure.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoRaiser;

impl WindowRaiser for NoRaiser {
    fn raise(&self) -> bool {
        false
    }

    fn open_terminal(&self, _request: &AttachRequest) -> bool {
        false
    }
}

/// A frontmost reader that knows nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnknownFrontmost;

impl FrontmostReader for UnknownFrontmost {
    fn frontmost(&self) -> Frontmost {
        Frontmost::Unknown
    }
}

/// A presence reader that knows nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnknownPresence;

impl PresenceReader for UnknownPresence {
    fn presence(&self) -> Presence {
        Presence::default()
    }
}

/// No sound.
#[derive(Debug, Clone, Copy, Default)]
pub struct Silent;

impl SoundPlayer for Silent {
    fn play(&self) {}
}

/// No panel. For a headless run and for tests that do not look at the view.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoPresenter;

impl ToastPresenter for NoPresenter {
    fn present(&self, _view: &PanelViewModel) {}
}
