//! Where the socket and the log files live. Inputs are plain values read by
//! the caller, so every platform's rule is testable on any machine.

use std::path::{Path, PathBuf};

/// The longest Unix socket path, in bytes, before the terminating NUL. macOS
/// caps `sun_path` at 104 bytes.
pub const SUN_PATH_MAX_BYTES: usize = 103;

/// The platform rules to apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// macOS.
    MacOs,
    /// Linux.
    Linux,
    /// Anything else. No socket.
    Other,
}

impl Platform {
    /// The platform this binary was built for.
    #[must_use]
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Platform::MacOs
        } else if cfg!(target_os = "linux") {
            Platform::Linux
        } else {
            Platform::Other
        }
    }
}

/// Environment facts the path rules read.
#[derive(Debug, Clone, Copy)]
pub struct PathInputs<'a> {
    /// The platform.
    pub platform: Platform,
    /// `$HOME`.
    pub home: Option<&'a Path>,
    /// `$XDG_RUNTIME_DIR`.
    pub xdg_runtime_dir: Option<&'a Path>,
    /// `$XDG_STATE_HOME`.
    pub xdg_state_home: Option<&'a Path>,
    /// `$AGENT_NOTIFY_SOCKET`.
    pub socket_override: Option<&'a Path>,
    /// `$AGENT_NOTIFY_LOG_DIR`.
    pub log_dir_override: Option<&'a Path>,
}

/// Where the daemon socket is, or why there is none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocketLocation {
    /// The socket path.
    At(PathBuf),
    /// No socket. The CLI uses the fallback and `doctor` reports the reason.
    Unavailable(&'static str),
}

/// Resolves the socket path. Fails closed: a missing or relative directory
/// gives `Unavailable`, never a path in a shared directory.
#[must_use]
pub fn socket_location(inputs: &PathInputs<'_>) -> SocketLocation {
    let candidate = match (inputs.socket_override, inputs.platform) {
        (Some(path), _) if path.is_absolute() => path.to_path_buf(),
        (Some(_), _) => return SocketLocation::Unavailable("AGENT_NOTIFY_SOCKET is not absolute"),
        (None, Platform::MacOs) => match absolute(inputs.home) {
            Some(home) => home.join("Library/Application Support/agent-notify/agent-notify.sock"),
            None => return SocketLocation::Unavailable("HOME is unset or relative"),
        },
        (None, Platform::Linux) => match absolute(inputs.xdg_runtime_dir) {
            Some(runtime) => runtime.join("agent-notify/agent-notify.sock"),
            None => return SocketLocation::Unavailable("XDG_RUNTIME_DIR is unset or relative"),
        },
        (None, Platform::Other) => return SocketLocation::Unavailable("unsupported platform"),
    };
    if candidate.as_os_str().len() > SUN_PATH_MAX_BYTES {
        return SocketLocation::Unavailable("socket path is longer than 103 bytes");
    }
    SocketLocation::At(candidate)
}

/// The directory for log files, or `None` when there is nowhere safe to write.
#[must_use]
pub fn log_directory(inputs: &PathInputs<'_>) -> Option<PathBuf> {
    if let Some(directory) = absolute(inputs.log_dir_override) {
        return Some(directory.to_path_buf());
    }
    let home = absolute(inputs.home);
    match inputs.platform {
        Platform::MacOs => home.map(|home| home.join("Library/Logs/agent-notify")),
        Platform::Linux | Platform::Other => absolute(inputs.xdg_state_home)
            .map(Path::to_path_buf)
            .or_else(|| home.map(|home| home.join(".local/state")))
            .map(|state| state.join("agent-notify")),
    }
}

fn absolute(path: Option<&Path>) -> Option<&Path> {
    path.filter(|path| path.is_absolute())
}

