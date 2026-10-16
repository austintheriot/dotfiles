//! The AeroSpace window query and focus (spike D2, D3).

use std::path::PathBuf;
use std::process::Command;

use crate::child::{CHILD_LIMIT, run_bounded};
use crate::presence::TERMINAL_BUNDLE_ID;

/// AeroSpace, by absolute path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aerospace {
    binary: PathBuf,
}

impl Aerospace {
    /// Wraps the binary at `binary`.
    #[must_use]
    pub fn new(binary: PathBuf) -> Self {
        Aerospace { binary }
    }

    /// The first Alacritty window on any monitor.
    #[must_use]
    pub fn terminal_window(&self) -> Option<u64> {
        let mut command = Command::new(&self.binary);
        command.args(["list-windows", "--monitor", "all", "--app-bundle-id", TERMINAL_BUNDLE_ID, "--format", "%{window-id}"]);
        let finished = run_bounded(&mut command, CHILD_LIMIT).ok()?;
        finished.status.success().then(|| first_window_id(&String::from_utf8_lossy(&finished.stdout))).flatten()
    }

    /// Focuses `window_id`. True when AeroSpace exits 0. That is not a
    /// raise on its own (spike S1-11): the caller activates the app after.
    #[must_use]
    pub fn focus(&self, window_id: u64) -> bool {
        let mut command = Command::new(&self.binary);
        command.args(["focus", "--window-id", &window_id.to_string()]);
        run_bounded(&mut command, CHILD_LIMIT).is_ok_and(|finished| finished.status.success())
    }
}

/// The first window id in `list-windows` output.
#[must_use]
pub fn first_window_id(output: &str) -> Option<u64> {
    output.lines().find_map(|line| line.trim().parse().ok())
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    use crate::test_support::stub_executable;

    #[test]
    fn the_first_window_id_is_used() {
        assert_eq!(first_window_id("4021\n4022\n"), Some(4021));
        assert_eq!(first_window_id("  77  \n"), Some(77));
    }

    #[test]
    fn no_window_is_none() {
        assert_eq!(first_window_id(""), None);
        assert_eq!(first_window_id("\n"), None);
        assert_eq!(first_window_id("error\n"), None);
    }

    #[test]
    fn the_terminal_window_comes_from_the_binary_output() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let binary = stub_executable(directory.path(), "aerospace", "printf '4021\\n4022\\n'");
        assert_eq!(Aerospace::new(binary).terminal_window(), Some(4021));
    }

    #[test]
    fn a_failing_binary_has_no_terminal_window() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let binary = stub_executable(directory.path(), "aerospace", "printf '4021\\n'\nexit 1");
        assert_eq!(Aerospace::new(binary).terminal_window(), None);
    }

    #[test]
    fn a_wedged_binary_is_abandoned_within_the_limit() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let binary = stub_executable(directory.path(), "aerospace", "exec sleep 5");
        let aerospace = Aerospace::new(binary);
        let started = Instant::now();
        assert_eq!(aerospace.terminal_window(), None);
        assert!(!aerospace.focus(4021));
        assert!(started.elapsed() < Duration::from_secs(4), "took {:?}", started.elapsed());
    }

    #[test]
    fn focus_reports_the_exit_status() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let succeeds = stub_executable(directory.path(), "succeeds", "exit 0");
        let fails = stub_executable(directory.path(), "fails", "exit 1");
        assert!(Aerospace::new(succeeds).focus(4021));
        assert!(!Aerospace::new(fails).focus(4021));
        assert!(!Aerospace::new(directory.path().join("missing")).focus(4021));
    }
}
