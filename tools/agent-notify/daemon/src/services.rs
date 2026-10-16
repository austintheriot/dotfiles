//! The platform services the daemon calls. Each trait has one
//! implementation per platform (macOS here, Wayland and X11 in plan 3) plus
//! a neutral one. `main` builds the record once; nothing reaches for a
//! global.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_notify_core::EpochMs;
use agent_notify_core::update::Frontmost;
use agent_notify_core::view::PanelViewModel;

/// Draws the toast stack. Must return at once: it is called from the actor.
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
    /// The tmux session to attach.
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
pub trait SoundPlayer: Send + Sync {
    /// Starts the sound and returns at once.
    fn play(&self);
}

/// The wall clock, injected so tests can fix the time.
pub type Clock = Arc<dyn Fn() -> EpochMs + Send + Sync>;

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
