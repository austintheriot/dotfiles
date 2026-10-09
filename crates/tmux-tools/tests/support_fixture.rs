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

/// Every window the fixture hands back already runs the inert command, not a
/// shell and not the forked tmux child that has yet to exec it.
///
/// `.zshrc`'s precmd calls the window-naming script on every prompt, so a
/// shell-backed window renames itself a beat after creation and overwrites
/// the name an assertion is about to read. `8591f242` fixed that once.
///
/// The window count is not decoration. `new-window` returns once tmux has
/// forked, and until the child execs, macOS reports the pane's command as
/// `tmux`. One read lost that race about 1 time in 30 on a loaded host and
/// failed two CI attempts in a row on run 37990239026; fifty reads make a
/// fixture that does not wait fail here rather than somewhere downstream.
#[test]
fn every_returned_window_already_runs_the_inert_command() {
    const WINDOWS: usize = 50;
    let server = Server::new("inert");
    let pane_command =
        |target: &str| server.stdout(&["display-message", "-p", "-t", target, "#{pane_current_command}"]);

    server.new_session("probe", server.socket_dir());
    let session_window = ("probe:".to_string(), pane_command("probe:"));
    let created_windows = (0..WINDOWS).map(|_| {
        let window = server.new_window("probe", server.socket_dir(), &[]);
        let command = pane_command(&window);
        (window, command)
    });
    let not_yet_inert: Vec<(String, String)> = std::iter::once(session_window)
        .chain(created_windows)
        .filter(|(_, command)| command != "sleep")
        .collect();

    assert!(
        not_yet_inert.is_empty(),
        "a returned window must already run sleep, or a shell's precmd or the \
         pre-exec tmux child races every assertion: {not_yet_inert:?}"
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
