//! Pins the scope of `tmux-tools close`, converted from
//! `tests/tmux-close.test.sh`.
//!
//! The scope is the current WINDOW, not the current session:
//! `kill-pane -a` kills every pane in the target's window and leaves other
//! windows and other sessions alone. The bystander tests below pin that
//! down, and they are the half of this suite that
//! `close_and_worktree_config.rs` does not already cover.
//!
//! The "refuses to run outside tmux" case is not a test of the binary. That
//! check lives in `.scripts/tmux-close.sh`, which stays sourced so its
//! message and `return` reach the interactive shell, so it is asserted
//! against the shim's own source and by sourcing the shim from zsh.

mod support;

use std::path::Path;
use std::process::Command;

use support::{Server, repo_root};

/// Runs `tmux-tools close` against `pane` on `server`.
fn close(server: &Server, pane: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_tmux-tools"))
        .args(["close", "-t", pane])
        .env("TMUX_TOOLS_SOCKET", server.socket())
        .env("TMUX_TMPDIR", server.socket_dir())
        .output()
        .expect("the binary runs")
}

/// Creates a window holding `panes` panes and returns its `#{window_id}`.
fn window_with_panes(server: &Server, session: &str, directory: &Path, panes: usize) -> String {
    let window = server.new_window(session, directory, &[]);
    for _ in 1..panes {
        server.tmux(&["split-window", "-d", "-t", &window]);
    }
    window
}

/// The panes in `window`, in listing order.
fn panes(server: &Server, window: &str) -> Vec<String> {
    server
        .stdout(&["list-panes", "-t", window, "-F", "#{pane_id}"])
        .lines()
        .map(str::to_string)
        .collect()
}

/// `close` reduces a multi-pane window to the target pane alone, and the
/// survivor is the pane that was targeted rather than whichever pane tmux
/// would have picked.
#[test]
fn it_leaves_only_the_target_pane() {
    let server = Server::new("close-scope");
    let fixtures = tempfile::Builder::new()
        .prefix("tt-fixtures-")
        .tempdir_in("/tmp")
        .expect("a fixture directory");
    server.new_session("main", fixtures.path());

    let window = window_with_panes(&server, "main", fixtures.path(), 4);
    let before = panes(&server, &window);
    // Positive control: the fixture must really have four panes, or "one
    // pane left" would hold for a window that only ever had one.
    assert_eq!(before.len(), 4, "the control must start with four panes");

    // The second pane, not the first: a close that ignored its target and
    // kept the window's first pane would pass against the first.
    let keep = before[1].clone();
    let run = close(&server, &keep);
    assert!(
        run.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    let after = panes(&server, &window);
    assert_eq!(after.len(), 1, "every pane except the target is closed");
    assert_eq!(after[0], keep, "the surviving pane is the one targeted");

    server.shutdown();
}

/// A single-pane window is left exactly as it was.
#[test]
fn a_lone_pane_survives_untouched() {
    let server = Server::new("close-lone");
    let fixtures = tempfile::Builder::new()
        .prefix("tt-fixtures-")
        .tempdir_in("/tmp")
        .expect("a fixture directory");
    server.new_session("main", fixtures.path());

    let window = window_with_panes(&server, "main", fixtures.path(), 1);
    let before = panes(&server, &window);
    assert_eq!(before.len(), 1, "the control must start with one pane");

    close(&server, &before[0]);

    let after = panes(&server, &window);
    assert_eq!(after.len(), 1, "a lone pane survives");
    assert_eq!(after[0], before[0], "the lone pane is unchanged");

    server.shutdown();
}

/// Another window in the same session keeps its panes.
#[test]
fn a_bystander_window_keeps_its_panes() {
    let server = Server::new("close-window");
    let fixtures = tempfile::Builder::new()
        .prefix("tt-fixtures-")
        .tempdir_in("/tmp")
        .expect("a fixture directory");
    server.new_session("main", fixtures.path());

    let bystander = window_with_panes(&server, "main", fixtures.path(), 3);
    let target = window_with_panes(&server, "main", fixtures.path(), 3);
    assert_eq!(
        panes(&server, &bystander).len(),
        3,
        "the control must start with three panes in the bystander"
    );

    let keep = panes(&server, &target)[0].clone();
    close(&server, &keep);

    assert_eq!(
        panes(&server, &target).len(),
        1,
        "the target window is reduced to one pane"
    );
    assert_eq!(
        panes(&server, &bystander).len(),
        3,
        "a different window in the same session keeps its panes"
    );

    server.shutdown();
}

/// Another session keeps its panes.
#[test]
fn a_bystander_session_keeps_its_panes() {
    let server = Server::new("close-session");
    let fixtures = tempfile::Builder::new()
        .prefix("tt-fixtures-")
        .tempdir_in("/tmp")
        .expect("a fixture directory");
    server.new_session("main", fixtures.path());
    server.new_session("other", fixtures.path());
    server.tmux(&["split-window", "-d", "-t", "other:1"]);

    assert_eq!(
        panes(&server, "other:1").len(),
        2,
        "the control must start with two panes in the other session"
    );

    let target = window_with_panes(&server, "main", fixtures.path(), 3);
    let keep = panes(&server, &target)[0].clone();
    close(&server, &keep);

    assert_eq!(
        panes(&server, &target).len(),
        1,
        "the target window is reduced to one pane"
    );
    assert_eq!(
        panes(&server, "other:1").len(),
        2,
        "another session keeps its panes"
    );

    server.shutdown();
}

/// The shim refuses to run with no tmux session, and says so.
///
/// The binary has no such branch on purpose: the check must reach the
/// interactive shell that sourced the shim, and a subprocess cannot
/// `return` into its parent. So this drives the shim the way `alias c`
/// does, from zsh, with `$TMUX` cleared.
#[test]
fn the_shim_refuses_to_run_outside_tmux() {
    let shim = repo_root().join(".scripts/tmux-close.sh");
    assert!(shim.is_file(), "the shim is tracked at {}", shim.display());

    let run = Command::new("zsh")
        .args(["-c", "source \"$1\"", "zsh"])
        .arg(&shim)
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .output()
        .expect("zsh runs");

    let printed = format!(
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        printed.contains("no tmux session"),
        "the shim must say there is no tmux session, got {printed:?}"
    );
}
