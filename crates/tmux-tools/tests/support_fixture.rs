//! Asserts the throwaway-server fixture's own three trap guards.
//!
//! Each guard replaces a comment that did not fail. The traps are tmux
//! behaviours that report success while doing the wrong thing, so a fixture
//! that carried them as prose was one edit away from every suite silently
//! driving the developer's real server.

mod support;

use support::{Server, shared_socket_path};

/// The socket directory exists before the first tmux call.
///
/// When `TMUX_TMPDIR` names a directory whose parent is missing, tmux falls
/// back to the shared path and exits 0 with no message, so this is the
/// precondition the silent fallback needs.
#[test]
fn the_socket_directory_exists_before_the_first_tmux_call() {
    let server = Server::new("dir-exists");
    assert!(
        server.socket_dir().is_dir(),
        "the socket directory must exist before tmux is asked to use it"
    );
}

/// The socket path fits inside the 104-byte cap a Unix socket path has.
#[test]
fn the_socket_path_is_short_enough_for_a_unix_socket() {
    let server = Server::new("short-path");
    let length = server
        .socket_dir()
        .join(format!("tmux-0/{}", server.socket()))
        .as_os_str()
        .len();
    assert!(length <= 104, "the socket path is {length} bytes");
    assert!(
        server.socket_dir().starts_with("/tmp/"),
        "the socket directory must be directly under /tmp, got {}",
        server.socket_dir().display()
    );
}

/// `TMUX_TMPDIR` actually took effect: a running server's socket is in the
/// fixture's own directory and not in the shared one.
///
/// The positive control for `shutdown`'s assertion. Without it, a fixture
/// that started no server at all would satisfy "no socket in the shared
/// directory" trivially.
#[test]
fn a_started_server_puts_its_socket_in_the_fixtures_own_directory() {
    let server = Server::new("relocated");
    server.new_session("probe", server.socket_dir());

    let uid = String::from_utf8(
        std::process::Command::new("id")
            .arg("-u")
            .output()
            .expect("id -u runs")
            .stdout,
    )
    .expect("utf-8");
    let own = server
        .socket_dir()
        .join(format!("tmux-{}", uid.trim()))
        .join(server.socket());
    assert!(
        own.exists(),
        "the control must find the socket in the fixture's directory: {}",
        own.display()
    );
    assert!(
        !shared_socket_path(server.socket()).exists(),
        "the socket must not be in the shared directory"
    );

    server.shutdown();
}

/// A window a test creates runs an inert command, not a shell.
///
/// `.zshrc`'s precmd calls the window-naming script on every prompt, so a
/// shell-backed window renames itself a beat after creation and overwrites
/// the name an assertion is about to read. `8591f242` fixed that once.
#[test]
fn test_windows_run_an_inert_command_rather_than_a_shell() {
    let server = Server::new("inert");
    server.new_session("probe", server.socket_dir());
    let window = server.new_window("probe", server.socket_dir(), &[]);

    let command = server.stdout(&[
        "display-message",
        "-p",
        "-t",
        &window,
        "#{pane_current_command}",
    ]);
    assert_eq!(
        command, "sleep",
        "a test window must not run a shell, or .zshrc's precmd races every assertion"
    );

    server.shutdown();
}

/// `isolate_hooks` sets an explicit no-op at index `[0]`, not an empty
/// string.
///
/// A hook set to `''` leaves the inherited global array entry in place, so
/// the global hook goes on firing and the isolation is not delivered.
#[test]
fn hook_isolation_installs_a_no_op_rather_than_an_empty_string() {
    let server = Server::new("hooks");
    server.new_session("probe", server.socket_dir());

    let hooks = server.stdout(&["show-hooks", "-t", "probe"]);
    // Positive control: the read must produce something, or the assertion
    // below would hold for an empty listing.
    assert!(!hooks.trim().is_empty(), "the control must list hooks");

    for hook in [
        "after-new-window",
        "after-split-window",
        "after-select-window",
        "after-select-pane",
        "after-kill-pane",
        "client-session-changed",
    ] {
        let entry = hooks
            .lines()
            .find(|line| line.starts_with(&format!("{hook}[0]")))
            .unwrap_or_else(|| panic!("no {hook}[0] entry in {hooks:?}"));
        assert!(
            entry.contains("run-shell -b true"),
            "{hook}[0] must be an explicit no-op, got {entry:?}"
        );
    }

    server.shutdown();
}
