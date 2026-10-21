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
    /// `$THUMPER_SOCKET`.
    pub socket_override: Option<&'a Path>,
    /// `$THUMPER_LOG_DIR`.
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
        (Some(_), _) => return SocketLocation::Unavailable("THUMPER_SOCKET is not absolute"),
        (None, Platform::MacOs) => match absolute(inputs.home) {
            Some(home) => home.join("Library/Application Support/thumper/thumper.sock"),
            None => return SocketLocation::Unavailable("HOME is unset or relative"),
        },
        (None, Platform::Linux) => match absolute(inputs.xdg_runtime_dir) {
            Some(runtime) => runtime.join("thumper/thumper.sock"),
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
    match inputs.platform {
        Platform::MacOs => absolute(inputs.home).map(|home| home.join("Library/Logs/thumper")),
        Platform::Linux | Platform::Other => xdg_state_directory(inputs),
    }
}

/// Where the daemon keeps state that must survive a restart: do-not-disturb
/// and the install stamp. Not the socket directory: on Linux that is a tmpfs.
#[must_use]
pub fn state_directory(inputs: &PathInputs<'_>) -> Option<PathBuf> {
    match inputs.platform {
        Platform::MacOs => {
            absolute(inputs.home).map(|home| home.join("Library/Application Support/thumper"))
        }
        Platform::Linux | Platform::Other => xdg_state_directory(inputs),
    }
}

fn xdg_state_directory(inputs: &PathInputs<'_>) -> Option<PathBuf> {
    absolute(inputs.xdg_state_home)
        .map(Path::to_path_buf)
        .or_else(|| absolute(inputs.home).map(|home| home.join(".local/state")))
        .map(|state| state.join("thumper"))
}

fn absolute(path: Option<&Path>) -> Option<&Path> {
    path.filter(|path| path.is_absolute())
}


#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(platform: Platform) -> PathInputs<'static> {
        PathInputs {
            platform,
            home: Some(Path::new("/Users/dev")),
            xdg_runtime_dir: Some(Path::new("/run/user/1000")),
            xdg_state_home: None,
            socket_override: None,
            log_dir_override: None,
        }
    }

    #[test]
    fn macos_socket_lives_in_application_support() {
        assert_eq!(
            socket_location(&inputs(Platform::MacOs)),
            SocketLocation::At(PathBuf::from("/Users/dev/Library/Application Support/thumper/thumper.sock"))
        );
    }

    #[test]
    fn linux_socket_lives_in_the_runtime_dir_and_fails_closed_without_it() {
        assert_eq!(
            socket_location(&inputs(Platform::Linux)),
            SocketLocation::At(PathBuf::from("/run/user/1000/thumper/thumper.sock"))
        );
        let missing = PathInputs { xdg_runtime_dir: None, ..inputs(Platform::Linux) };
        assert!(matches!(socket_location(&missing), SocketLocation::Unavailable(_)));
        let relative = PathInputs { xdg_runtime_dir: Some(Path::new("run")), ..inputs(Platform::Linux) };
        assert!(matches!(socket_location(&relative), SocketLocation::Unavailable(_)));
    }

    #[test]
    fn the_override_wins_when_absolute() {
        let overridden = PathInputs { socket_override: Some(Path::new("/tmp/test.sock")), ..inputs(Platform::Linux) };
        assert_eq!(socket_location(&overridden), SocketLocation::At(PathBuf::from("/tmp/test.sock")));
        let relative = PathInputs { socket_override: Some(Path::new("test.sock")), ..inputs(Platform::MacOs) };
        assert!(matches!(socket_location(&relative), SocketLocation::Unavailable(_)));
    }

    #[test]
    fn an_overlong_socket_path_is_unavailable() {
        let long_home = format!("/Users/{}", "d".repeat(80));
        let long = PathInputs { home: Some(Path::new(&long_home)), ..inputs(Platform::MacOs) };
        assert!(matches!(socket_location(&long), SocketLocation::Unavailable(_)));
    }

    #[test]
    fn log_directories() {
        assert_eq!(log_directory(&inputs(Platform::MacOs)), Some(PathBuf::from("/Users/dev/Library/Logs/thumper")));
        assert_eq!(log_directory(&inputs(Platform::Linux)), Some(PathBuf::from("/Users/dev/.local/state/thumper")));
        let state = PathInputs { xdg_state_home: Some(Path::new("/state")), ..inputs(Platform::Linux) };
        assert_eq!(log_directory(&state), Some(PathBuf::from("/state/thumper")));
        let overridden = PathInputs { log_dir_override: Some(Path::new("/tmp/logs")), ..inputs(Platform::MacOs) };
        assert_eq!(log_directory(&overridden), Some(PathBuf::from("/tmp/logs")));
        let homeless = PathInputs { home: None, ..inputs(Platform::MacOs) };
        assert_eq!(log_directory(&homeless), None);
    }

    #[test]
    fn the_state_directory_follows_the_platform() {
        let home = Path::new("/home/dev");
        let inputs = |platform, xdg_state_home| PathInputs {
            platform,
            home: Some(home),
            xdg_runtime_dir: None,
            xdg_state_home,
            socket_override: None,
            log_dir_override: None,
        };
        assert_eq!(
            state_directory(&inputs(Platform::MacOs, None)),
            Some(PathBuf::from("/home/dev/Library/Application Support/thumper"))
        );
        assert_eq!(state_directory(&inputs(Platform::Linux, None)), Some(PathBuf::from("/home/dev/.local/state/thumper")));
        assert_eq!(
            state_directory(&inputs(Platform::Linux, Some(Path::new("/state")))),
            Some(PathBuf::from("/state/thumper"))
        );
        assert_eq!(state_directory(&inputs(Platform::Linux, Some(Path::new("relative")))), Some(PathBuf::from("/home/dev/.local/state/thumper")));
    }
}
