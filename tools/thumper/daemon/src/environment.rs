//! The environment variables the daemon reads, read once at startup.

use std::ffi::OsString;
use std::path::PathBuf;

use thumper_core::paths::{PathInputs, Platform};

/// A snapshot of the daemon's environment. Empty values count as unset.
#[derive(Debug, Clone, Default)]
pub struct DaemonEnvironment {
    /// `$HOME`.
    pub home: Option<PathBuf>,
    /// `$XDG_RUNTIME_DIR`.
    pub xdg_runtime_dir: Option<PathBuf>,
    /// `$XDG_STATE_HOME`.
    pub xdg_state_home: Option<PathBuf>,
    /// `$THUMPER_SOCKET`.
    pub socket_override: Option<PathBuf>,
    /// `$THUMPER_LOG_DIR`.
    pub log_dir_override: Option<PathBuf>,
    /// `$PATH`.
    pub path: Option<OsString>,
    /// `$THUMPER_TMUX`, which the launchd plist sets.
    pub tmux_override: Option<PathBuf>,
    /// `$THUMPER_AEROSPACE`, which the launchd plist sets.
    pub aerospace_override: Option<PathBuf>,
}

impl DaemonEnvironment {
    /// Reads the variables.
    #[must_use]
    pub fn read() -> Self {
        let path = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty()).map(PathBuf::from);
        DaemonEnvironment {
            home: path("HOME"),
            xdg_runtime_dir: path("XDG_RUNTIME_DIR"),
            xdg_state_home: path("XDG_STATE_HOME"),
            socket_override: path("THUMPER_SOCKET"),
            log_dir_override: path("THUMPER_LOG_DIR"),
            path: std::env::var_os("PATH").filter(|value| !value.is_empty()),
            tmux_override: path("THUMPER_TMUX"),
            aerospace_override: path("THUMPER_AEROSPACE"),
        }
    }

    /// The facts the core's path rules read.
    #[must_use]
    pub fn path_inputs(&self) -> PathInputs<'_> {
        PathInputs {
            platform: Platform::current(),
            home: self.home.as_deref(),
            xdg_runtime_dir: self.xdg_runtime_dir.as_deref(),
            xdg_state_home: self.xdg_state_home.as_deref(),
            socket_override: self.socket_override.as_deref(),
            log_dir_override: self.log_dir_override.as_deref(),
        }
    }
}
