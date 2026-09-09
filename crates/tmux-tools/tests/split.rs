//! Drives `tmux-tools split`, covering the three-way exit contract and the
//! pane arrangements `.scripts/tmux-split.sh` produced.
//!
//! The exit-code cases need no tmux server at all, because an unrecognized
//! name and a missing argument both return before any tmux call. The
//! arrangement cases use the `TMUX_TOOLS_SOCKET` seam, the same throwaway
//! server `close_and_worktree_config.rs` uses.

use std::process::Command;

fn tmux(socket_dir: &std::path::Path, socket: &str, arguments: &[&str]) -> std::process::Output {
    Command::new("tmux")
        // Relocates the socket into the test\'s own directory, which Drop
        // removes. tmux 3.4 does not unlink it on kill-server.
        .env("TMUX_TMPDIR", socket_dir)
        .args(["-L", socket])
        .args(arguments)
        .output()
        .expect("tmux runs")
}

/// Where tmux puts a socket when nothing relocates it, which is where every
/// test socket this file ever created stayed: tmux 3.4 does not unlink the
/// file on `kill-server`, so the shared directory held 1435 dead sockets.
fn shared_socket_path(socket: &str) -> std::path::PathBuf {
    let uid = Command::new("id").arg("-u").output().expect("id -u runs");
    let uid = String::from_utf8(uid.stdout).expect("utf-8").trim().to_owned();
    std::path::PathBuf::from(format!("/tmp/tmux-{uid}/{socket}"))
}

/// An unrecognized layout exits 3 and prints no usage text.
///
/// The old shell version reached this case through positional-parameter
/// inheritance and signalled it by printing usage, so a user who typed a
/// perfectly good session name got told they had used the command wrong.
#[test]
fn an_unrecognized_layout_exits_three_and_stays_quiet() {
    let binary = env!("CARGO_BIN_EXE_tmux-tools");
    let run = Command::new(binary)
        .args(["split", "my-feature-branch"])
        .output()
        .expect("the binary runs");

    assert_eq!(run.status.code(), Some(3), "an unrecognized layout is exit 3");
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        !stderr.to_lowercase().contains("usage"),
        "a valid session name must not be told it is a usage error, got {stderr:?}"
    );
}

/// No argument at all IS a usage error, and exits 2.
///
/// Paired with the test above so the two outcomes are distinguished rather
/// than merely both non-zero: 2 and 3 must not collapse into one code.
#[test]
fn no_argument_is_a_usage_error() {
    let binary = env!("CARGO_BIN_EXE_tmux-tools");
    let run = Command::new(binary)
        .args(["split"])
        .output()
        .expect("the binary runs");

    assert_eq!(run.status.code(), Some(2), "a missing argument is exit 2");
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.to_lowercase().contains("usage"),
        "a genuine usage error still says usage, got {stderr:?}"
    );
}

/// A recognized layout exits 0 and actually splits the window.
///
/// The positive control for the exit-3 test above: without it, a binary
/// that exited 3 for every name would pass that test.
#[test]
fn a_recognized_layout_exits_zero_and_splits() {
    let socket = format!("tmux-tools-test-split-known-{}", std::process::id());
    let socket_dir = tempfile::Builder::new().prefix("tt-").tempdir_in("/tmp").expect("a socket dir");
    tmux(socket_dir.path(), &socket, &["new-session", "-d", "-s", "probe"]);

    let before = tmux(socket_dir.path(), &socket, &["list-panes", "-t", "probe:1", "-F", "#{pane_id}"]);
    let panes_before = String::from_utf8_lossy(&before.stdout).lines().count();
    assert_eq!(panes_before, 1, "the control must start with one pane");

    let pane = String::from_utf8_lossy(&before.stdout).trim().to_string();

    let binary = env!("CARGO_BIN_EXE_tmux-tools");
    let run = Command::new(binary)
        .args(["split", "terms"])
        .env("TMUX_TOOLS_SOCKET", &socket)
        .env("TMUX_TMPDIR", socket_dir.path())
        .env("TMUX_PANE", &pane)
        .output()
        .expect("the binary runs");

    assert_eq!(
        run.status.code(),
        Some(0),
        "a recognized layout is exit 0, stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    let after = tmux(socket_dir.path(), &socket, &["list-panes", "-t", "probe:1", "-F", "#{pane_id}"]);
    let panes_after = String::from_utf8_lossy(&after.stdout).lines().count();
    assert_eq!(panes_after, 2, "terms with the default count makes two panes");

    tmux(socket_dir.path(), &socket, &["kill-server"]);
    assert!(
        !shared_socket_path(&socket).exists(),
        "the test left its socket file in the shared tmux directory: {socket}"
    );
}

/// An unrecognized layout leaves the window's pane count untouched.
///
/// This is the spec's named acceptance test, run against a detached session
/// rather than an interactive one. Step 7 of the brief runs the interactive
/// shape by hand; this pins the same property in the suite.
#[test]
fn an_unrecognized_layout_splits_nothing() {
    let socket = format!("tmux-tools-test-split-unknown-{}", std::process::id());
    let socket_dir = tempfile::Builder::new().prefix("tt-").tempdir_in("/tmp").expect("a socket dir");
    tmux(socket_dir.path(), &socket, &["new-session", "-d", "-s", "my-feature-branch"]);

    let before = tmux(
        socket_dir.path(),
        &socket,
        &["list-panes", "-t", "my-feature-branch:1", "-F", "#{pane_id}"],
    );
    let before_output = String::from_utf8_lossy(&before.stdout).into_owned();
    assert_eq!(
        before_output.lines().count(),
        1,
        "the control must start with one pane"
    );
    let pane = before_output.trim().to_string();

    let binary = env!("CARGO_BIN_EXE_tmux-tools");
    let run = Command::new(binary)
        .args(["split", "my-feature-branch"])
        .env("TMUX_TOOLS_SOCKET", &socket)
        .env("TMUX_TMPDIR", socket_dir.path())
        .env("TMUX_PANE", &pane)
        .output()
        .expect("the binary runs");
    assert_eq!(run.status.code(), Some(3), "an unrecognized layout is exit 3");

    let after = tmux(
        socket_dir.path(),
        &socket,
        &["list-panes", "-t", "my-feature-branch:1", "-F", "#{pane_id}"],
    );
    let after_output = String::from_utf8_lossy(&after.stdout);
    assert_eq!(
        after_output.lines().count(),
        1,
        "a session name that is not a layout must leave one pane, got {after_output:?}"
    );

    tmux(socket_dir.path(), &socket, &["kill-server"]);
    assert!(
        !shared_socket_path(&socket).exists(),
        "the test left its socket file in the shared tmux directory: {socket}"
    );
}

/// The editor layout leaves focus on the editor pane, as
/// `create_editor_with_terminals`'s trailing `tmux select-pane -L` did.
#[test]
fn the_editor_layout_focuses_the_editor_pane() {
    let socket = format!("tmux-tools-test-split-editor-{}", std::process::id());
    let socket_dir = tempfile::Builder::new().prefix("tt-").tempdir_in("/tmp").expect("a socket dir");
    tmux(socket_dir.path(), &socket, &["new-session", "-d", "-s", "probe"]);

    let before = tmux(socket_dir.path(), &socket, &["list-panes", "-t", "probe:1", "-F", "#{pane_id}"]);
    let editor_pane = String::from_utf8_lossy(&before.stdout).trim().to_string();

    let binary = env!("CARGO_BIN_EXE_tmux-tools");
    let run = Command::new(binary)
        .args(["split", "|"])
        .env("TMUX_TOOLS_SOCKET", &socket)
        .env("TMUX_TMPDIR", socket_dir.path())
        .env("TMUX_PANE", &editor_pane)
        .output()
        .expect("the binary runs");
    assert_eq!(
        run.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    let after = tmux(socket_dir.path(), &socket, &["list-panes", "-t", "probe:1", "-F", "#{pane_id}"]);
    assert_eq!(
        String::from_utf8_lossy(&after.stdout).lines().count(),
        3,
        "one editor plus two terminals"
    );

    let active = tmux(
        socket_dir.path(),
        &socket,
        &["display-message", "-p", "-t", "probe:1", "#{pane_id}"],
    );
    assert_eq!(
        String::from_utf8_lossy(&active.stdout).trim(),
        editor_pane,
        "focus returns to the original left-hand pane"
    );

    tmux(socket_dir.path(), &socket, &["kill-server"]);
    assert!(
        !shared_socket_path(&socket).exists(),
        "the test left its socket file in the shared tmux directory: {socket}"
    );
}
