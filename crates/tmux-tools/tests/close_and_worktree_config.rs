//! Drives `tmux-tools close` and `tmux-tools worktree-config` against a
//! throwaway tmux server, the same seam `name_windows.rs` uses.

use std::process::Command;

fn tmux(socket: &str, arguments: &[&str]) -> std::process::Output {
    Command::new("tmux")
        .args(["-L", socket])
        .args(arguments)
        .output()
        .expect("tmux runs")
}

/// `close` kills every other pane in the current window, leaving the target
/// pane and the session itself untouched.
///
/// Mirrors `tmux-close.sh`'s only real effect:
/// `tmux kill-pane -a -t <target pane>`.
#[test]
fn it_closes_every_pane_but_the_target() {
    let socket = format!("tmux-tools-test-close-{}", std::process::id());

    tmux(&socket, &["new-session", "-d", "-s", "probe"]);
    tmux(&socket, &["split-window", "-t", "probe:1"]);

    let before = tmux(&socket, &["list-panes", "-t", "probe:1", "-F", "#{pane_id}"]);
    let before_output = String::from_utf8_lossy(&before.stdout).into_owned();
    // Positive control: the fixture must actually have two panes, or the
    // "dropped to one" assertion below would hold trivially for a session
    // that only ever had one.
    assert_eq!(before_output.lines().count(), 2, "the control must start with two panes");

    let target_pane = before_output.lines().next().expect("a pane id").to_string();

    let binary = env!("CARGO_BIN_EXE_tmux-tools");
    let run = Command::new(binary)
        .args(["close", "-t", &target_pane])
        .env("TMUX_TOOLS_SOCKET", &socket)
        .output()
        .expect("the binary runs");
    assert!(run.status.success(), "stderr: {}", String::from_utf8_lossy(&run.stderr));

    let after = tmux(&socket, &["list-panes", "-t", "probe:1", "-F", "#{pane_id}"]);
    let after_output = String::from_utf8_lossy(&after.stdout);
    let pane_count_after = after_output.lines().count();
    assert_eq!(pane_count_after, 1, "expected one pane left, got {after_output:?}");
    assert!(
        after_output.trim() == target_pane,
        "the surviving pane must be the target, got {after_output:?}"
    );

    let session_still_exists = tmux(&socket, &["has-session", "-t", "probe"]).status.success();
    assert!(session_still_exists, "the session must survive close");

    tmux(&socket, &["kill-server"]);
}

/// `worktree-config` prints the numbered-worktree-window count.
///
/// `tmux-worktree-config.sh`'s only effect is `WORKTREE_COUNT=15`, a shell
/// variable assignment with no tmux call at all. A subprocess cannot set a
/// variable in its parent shell, so the honest port is a subcommand that
/// prints the same number for the shim to capture. See the report for why
/// this differs from the brief's "assert a tmux option" framing: the script
/// has no tmux-observable effect to assert.
#[test]
fn it_prints_the_worktree_count() {
    let binary = env!("CARGO_BIN_EXE_tmux-tools");
    let run = Command::new(binary)
        .args(["worktree-config"])
        .output()
        .expect("the binary runs");
    assert!(run.status.success(), "stderr: {}", String::from_utf8_lossy(&run.stderr));

    let stdout = String::from_utf8_lossy(&run.stdout);
    assert_eq!(stdout.trim(), "15", "expected the worktree count, got {stdout:?}");
}
