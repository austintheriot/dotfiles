//! Absolute paths for the external tools the daemon runs. launchd starts
//! the daemon with a minimal `PATH`, so a bare `tmux` fails with ENOENT
//! (spike S1-7). The daemon resolves each tool once, at startup.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use agent_notify_core::status::ToolStatus;

use crate::environment::DaemonEnvironment;

/// The tools found at startup. `None` turns off the feature that needs it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolPaths {
    /// tmux. Without it a click only raises the terminal.
    pub tmux: Option<PathBuf>,
    /// AeroSpace. Without it the raise uses app activation only.
    pub aerospace: Option<PathBuf>,
}

impl ToolPaths {
    /// Resolves every tool from the overrides and `PATH`.
    #[must_use]
    pub fn discover(environment: &DaemonEnvironment) -> Self {
        let path = environment.path.as_deref();
        ToolPaths {
            tmux: resolve("tmux", environment.tmux_override.as_deref(), path, is_executable),
            aerospace: resolve("aerospace", environment.aerospace_override.as_deref(), path, is_executable),
        }
    }

    /// One row per tool, for `status` and `doctor`.
    #[must_use]
    pub fn statuses(&self) -> Vec<ToolStatus> {
        vec![
            ToolStatus { name: "tmux".to_string(), path: self.tmux.clone() },
            ToolStatus { name: "aerospace".to_string(), path: self.aerospace.clone() },
        ]
    }
}

/// Finds `name`. An override must be an absolute executable; when it is not,
/// the answer is `None`, never a different binary from `PATH`.
pub fn resolve(name: &str, override_path: Option<&Path>, path: Option<&OsStr>, is_executable: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    if let Some(candidate) = override_path {
        return (candidate.is_absolute() && is_executable(candidate)).then(|| candidate.to_path_buf());
    }
    std::env::split_paths(path?)
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(name))
        .find(|candidate| is_executable(candidate))
}

/// True for a regular file with an execute bit.
#[must_use]
pub fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    use super::*;

    fn only(paths: &'static [&'static str]) -> impl Fn(&Path) -> bool {
        move |candidate| paths.iter().any(|path| Path::new(path) == candidate)
    }

    fn search_path(entries: &[&str]) -> OsString {
        std::env::join_paths(entries).expect("joinable")
    }

    #[test]
    fn the_override_wins_when_it_is_an_absolute_executable() {
        let path = search_path(&["/usr/bin"]);
        let found = resolve("tmux", Some(Path::new("/opt/tmux")), Some(&path), only(&["/opt/tmux", "/usr/bin/tmux"]));
        assert_eq!(found, Some(PathBuf::from("/opt/tmux")));
    }

    #[test]
    fn a_bad_override_finds_nothing_rather_than_another_binary() {
        let path = search_path(&["/usr/bin"]);
        assert_eq!(resolve("tmux", Some(Path::new("tmux")), Some(&path), only(&["/usr/bin/tmux"])), None);
        assert_eq!(resolve("tmux", Some(Path::new("/missing/tmux")), Some(&path), only(&["/usr/bin/tmux"])), None);
    }

    #[test]
    fn path_is_searched_in_order_and_relative_entries_are_skipped() {
        let path = search_path(&["bin", "/opt/homebrew/bin", "/usr/local/bin"]);
        let found = resolve("tmux", None, Some(&path), only(&["bin/tmux", "/opt/homebrew/bin/tmux", "/usr/local/bin/tmux"]));
        assert_eq!(found, Some(PathBuf::from("/opt/homebrew/bin/tmux")));
    }

    #[test]
    fn nothing_found_is_none() {
        assert_eq!(resolve("tmux", None, None, only(&[])), None);
        let path = search_path(&["/usr/bin"]);
        assert_eq!(resolve("tmux", None, Some(&path), only(&[])), None);
    }

    #[test]
    fn statuses_name_every_tool() {
        let tools = ToolPaths { tmux: Some(PathBuf::from("/opt/homebrew/bin/tmux")), aerospace: None };
        let names: Vec<(String, bool)> = tools.statuses().into_iter().map(|status| (status.name, status.path.is_some())).collect();
        assert_eq!(names, vec![("tmux".to_string(), true), ("aerospace".to_string(), false)]);
    }
}
