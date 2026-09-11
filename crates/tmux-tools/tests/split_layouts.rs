//! Integration tests for `.scripts/tmux-split.sh`, converted from
//! `tests/tmux-split.test.sh`.
//!
//! The script is sourced, not executed, and splits whatever pane
//! `$TMUX_PANE` points at. Each case sources it against a fresh single-pane
//! window and counts the panes that result.
//!
//! Every layout defaults to `DEFAULT_VERTICAL_SPLITS` (2). An unknown layout
//! must leave the sourcing shell alive, which is why the shim returns the
//! binary's status rather than using `exec`: `exec` in a sourced script
//! replaces the interactive shell and closes the user's terminal.
//!
//! `split.rs` drives the BINARY and covers its three-way exit contract.
//! This file drives the SHIM, because the properties left over are the
//! shim's own: that it forwards layout names and counts unchanged, and that
//! it never takes the sourcing shell down with it.

mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use support::{Server, repo_root};

/// The tracked shim under test.
fn script_path() -> PathBuf {
    repo_root().join(".scripts/tmux-split.sh")
}

/// A throwaway server plus a fixture directory for its windows.
struct Fixture {
    server: Server,
    directory: tempfile::TempDir,
}

impl Fixture {
    /// Starts a server with one session to hang test windows off.
    fn new(label: &str) -> Self {
        let server = Server::new(label);
        let directory = tempfile::Builder::new()
            .prefix("tt-fixtures-")
            .tempdir_in("/tmp")
            .expect("a fixture directory");
        server.new_session("main", directory.path());
        Fixture { server, directory }
    }

    /// Creates a single-pane window and returns its id and its pane's id.
    fn fresh_window(&self) -> (String, String) {
        let window = self
            .server
            .new_window("main", self.directory.path(), &[]);
        let pane = self
            .server
            .stdout(&["list-panes", "-t", &window, "-F", "#{pane_id}"])
            .lines()
            .next()
            .expect("a pane id")
            .to_string();
        (window, pane)
    }

    /// Panes currently in `window`.
    fn pane_count(&self, window: &str) -> usize {
        self.server
            .stdout(&["list-panes", "-t", window, "-F", "#{pane_id}"])
            .lines()
            .count()
    }

    /// Sources the shim from zsh against `pane`, with `arguments` passed as
    /// positionals.
    ///
    /// Arguments go through as positionals, not interpolated into the
    /// command string: layout names include `|` and `\`, which the shell
    /// would otherwise read as operators.
    ///
    /// The script path is saved and `shift`ed away before the `source`,
    /// because a sourced script inherits the caller's positional
    /// parameters. Without the shift `"$1"` is still the script path from
    /// `zsh -c`'s own argument list, the shim forwards that path as a
    /// layout name, and the case under test never runs. Observed here
    /// first: the no-argument case exited 127 with
    /// `zsh:source:1: no such file or directory: terms`. That inheritance
    /// is the exact defect this port removed from `tmux-start.sh`, and it
    /// is just as live in a test harness.
    ///
    /// The shim is sourced from zsh because `.zshrc` loads it that way.
    fn source_shim(&self, pane: &str, arguments: &[&str]) -> Output {
        let mut command = Command::new("zsh");
        command
            .args(["-c", "script=$1; shift; source \"$script\"", "zsh"])
            .arg(script_path())
            .args(arguments)
            .env("PATH", path_with_binary())
            .env("TMUX_TOOLS_SOCKET", self.server.socket())
            .env("TMUX_TMPDIR", self.server.socket_dir())
            .env("TMUX_PANE", pane)
            // Cleared, not pointed anywhere: the shim's own subprocess must
            // never resolve "the current pane" against the developer's live
            // session.
            .env_remove("TMUX");
        command.output().expect("zsh runs")
    }

    /// Splits a brand-new window with `arguments` and returns the pane
    /// count that results.
    fn split_in_new_window(&self, arguments: &[&str]) -> usize {
        let (window, pane) = self.fresh_window();
        self.source_shim(&pane, arguments);
        let count = self.pane_count(&window);
        self.server.tmux(&["kill-window", "-t", &window]);
        count
    }
}

/// `tmux-tools` has to be on `PATH` for the shim to find it.
///
/// The shim calls a bare `tmux-tools`, so a machine whose installed binary
/// is stale would otherwise be the thing under test rather than this
/// build's.
fn path_with_binary() -> String {
    let binary = Path::new(env!("CARGO_BIN_EXE_tmux-tools"));
    format!(
        "{}:{}",
        binary.parent().expect("the binary has a directory").display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

/// Every layout name and count the shim forwards produces the arrangement
/// the script's own `case` produced.
#[test]
fn every_layout_builds_its_arrangement() {
    let fixture = Fixture::new("split-layouts");

    // Positive control: a fresh window must start at one pane, or every
    // count below would be measuring something that was already split.
    let (control_window, _) = fixture.fresh_window();
    assert_eq!(
        fixture.pane_count(&control_window),
        1,
        "the control must start with one pane"
    );
    fixture
        .server
        .tmux(&["kill-window", "-t", &control_window]);

    for (arguments, expected, description) in [
        (vec!["terms"], 2, "terms with no count gives 2 panes"),
        (vec!["terms", "3"], 3, "terms honours an explicit count"),
        (vec!["terms", "1"], 1, "terms with a count of 1 does not split"),
        (
            vec!["|"],
            3,
            "editor layout with no count gives 1 editor plus 2 terminals",
        ),
        (vec!["\\"], 3, "backslash is an alias for the editor layout"),
        (vec!["code"], 3, "code is an alias for the editor layout"),
        (
            vec!["|", "4"],
            5,
            "editor layout honours an explicit terminal count",
        ),
        (
            vec!["-", "2", "2"],
            3,
            "main-above-two-below builds 3 panes",
        ),
        (
            vec!["_", "2", "2"],
            3,
            "underscore is an alias for main-above-two-below",
        ),
        (
            vec!["-", "3", "3"],
            5,
            "main-above-two-below honours explicit counts",
        ),
    ] {
        assert_eq!(
            fixture.split_in_new_window(&arguments),
            expected,
            "{description}"
        );
    }

    fixture.server.shutdown();
}

/// The editor layout leaves focus on the leftmost pane.
#[test]
fn the_editor_layout_focuses_the_leftmost_pane() {
    let fixture = Fixture::new("split-focus");
    let (window, pane) = fixture.fresh_window();

    fixture.source_shim(&pane, &["|"]);
    assert_eq!(
        fixture.pane_count(&window),
        3,
        "the control must have built the editor layout"
    );

    let active = fixture
        .server
        .stdout(&["display-message", "-p", "-t", &window, "#{pane_id}"]);
    assert_eq!(active, pane, "the editor layout focuses the leftmost pane");

    fixture.server.shutdown();
}

/// An unknown layout exits 3, prints nothing, and splits nothing.
///
/// Exit 3, not 1, and no usage text. An unrecognized name is not a usage
/// error: `tmux-start.sh` passes a session name here and most session names
/// are not layout names, so a user who typed a perfectly good session name
/// must not be told they used the command wrong.
#[test]
fn an_unknown_layout_is_quiet_and_changes_nothing() {
    let fixture = Fixture::new("split-unknown");
    let (window, pane) = fixture.fresh_window();

    let run = fixture.source_shim(&pane, &["bogus-layout"]);
    assert_eq!(run.status.code(), Some(3), "an unknown layout exits 3");

    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(said.trim(), "", "an unknown layout prints nothing");
    assert_eq!(
        fixture.pane_count(&window),
        1,
        "an unknown layout splits nothing"
    );

    fixture.server.shutdown();
}

/// No layout argument is a usage error: exit 2, usage text, and no split.
///
/// Paired with the test above so the two outcomes are distinguished rather
/// than merely both non-zero. Without it those assertions would pass for a
/// shim that had collapsed every failure into one silent code.
#[test]
fn no_layout_argument_is_a_usage_error() {
    let fixture = Fixture::new("split-usage");
    let (window, pane) = fixture.fresh_window();

    let run = fixture.source_shim(&pane, &[]);
    assert_eq!(run.status.code(), Some(2), "no layout argument exits 2");

    let said = String::from_utf8_lossy(&run.stderr).to_lowercase();
    assert!(
        said.contains("usage"),
        "a usage error still prints usage, got {said:?}"
    );
    assert_eq!(
        fixture.pane_count(&window),
        1,
        "no layout argument splits nothing"
    );

    fixture.server.shutdown();
}

/// Usage must not take the sourcing shell down with it.
///
/// The shim is sourced into an interactive shell, so `exit` in its usage
/// path would close the user's terminal on a typo'd layout name. These two
/// check that the shell reaches the statement after the `source`.
#[test]
fn the_sourcing_shell_survives_both_failure_paths() {
    let fixture = Fixture::new("split-survive");
    let (_, pane) = fixture.fresh_window();

    for (arguments, description) in [
        (
            vec!["bogus-layout"],
            "an unknown layout leaves the sourcing shell alive",
        ),
        (vec![], "no layout argument leaves the sourcing shell alive"),
    ] {
        let mut command = Command::new("zsh");
        command
            .args([
                "-c",
                "script=$1; shift; source \"$script\" >/dev/null 2>&1; print -r -- SURVIVED",
                "zsh",
            ])
            .arg(script_path())
            .args(&arguments)
            .env("PATH", path_with_binary())
            .env("TMUX_TOOLS_SOCKET", fixture.server.socket())
            .env("TMUX_TMPDIR", fixture.server.socket_dir())
            .env("TMUX_PANE", &pane)
            .env_remove("TMUX");
        let run = command.output().expect("zsh runs");
        assert_eq!(
            String::from_utf8_lossy(&run.stdout).trim(),
            "SURVIVED",
            "{description}"
        );
        // SURVIVED alone is not enough. A shim whose binary was not on
        // PATH exits 127 and the shell survives that too, so this test
        // would pass while testing nothing. Pin the status the shim is
        // supposed to propagate.
        assert_ne!(
            run.status.code(),
            Some(127),
            "tmux-tools must be on PATH, or this test proves nothing"
        );
    }

    fixture.server.shutdown();
}
