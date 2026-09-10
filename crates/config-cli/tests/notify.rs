//! `.claude/hooks/notify.sh` decides whether the user is already looking at
//! this pane, and fires a macOS notification only when they are not.
//!
//! The hook shells out to aerospace (to find the frontmost app) and to
//! osascript (to raise the notification). Both are addressed through
//! `AEROSPACE_BIN` and `OSASCRIPT_BIN`, so the tests point them at stubs: one
//! that reports whichever app the case wants, and one that records the
//! arguments it was called with.
//!
//! Converted whole from `tests/notify.test.sh`, which ran **13** assertions,
//! measured by running it rather than by counting `assert_` call sites.
//!
//! The three in-tmux cases need a tmux server. They get one of their own on a
//! private socket, never the developer's, and the three trap guards that cost
//! a debugging cycle on 2026-09-09 apply here too: the socket directory is
//! created before the first tmux call and lives directly under `/tmp` because
//! a Unix socket path is capped at 104 bytes on macOS, and the teardown kills
//! through the same wrapper that set `TMUX_TMPDIR`.

use dotfiles_test_support::repo::root as repo_root;
use dotfiles_test_support::skip;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use tempfile::TempDir;

/// The hook under test.
fn hook() -> PathBuf {
    repo_root().join(".claude/hooks/notify.sh")
}

/// The two stubs the hook is pointed at, plus the file one of them writes.
struct Stubs {
    directory: TempDir,
    calls: PathBuf,
}

impl Stubs {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let calls = directory.path().join("osascript-calls");

        // Reads the destination from the environment rather than baking it
        // in, which is how the shell suite wired it: one stub, many runs.
        write_executable(
            &directory.path().join("osascript"),
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$CALLS\"\n",
        );
        write_executable(
            &directory.path().join("aerospace"),
            "#!/bin/sh\nprintf '%s\\n' \"${FRONTMOST_APP:-}\"\n",
        );

        Self { directory, calls }
    }

    fn osascript(&self) -> PathBuf {
        self.directory.path().join("osascript")
    }

    fn aerospace(&self) -> PathBuf {
        self.directory.path().join("aerospace")
    }

    /// Empties the record, so a read cannot see the previous run's call.
    fn clear(&self) {
        fs::write(&self.calls, "").expect("the record is writable");
    }

    /// Whatever osascript received. Empty means the hook suppressed the
    /// notification.
    fn recorded(&self) -> String {
        fs::read_to_string(&self.calls).unwrap_or_default()
    }
}

fn write_executable(path: &Path, body: &str) {
    fs::write(path, body).expect("the stub is writable");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("the stub is executable");
}

/// Runs the hook outside tmux and returns what osascript received.
fn run_notify(stubs: &Stubs, frontmost: &str, kind: &str, directory: &Path) -> String {
    stubs.clear();
    let mut child = Command::new(hook())
        .arg(kind)
        .current_dir(directory)
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .env("AEROSPACE_BIN", stubs.aerospace())
        .env("OSASCRIPT_BIN", stubs.osascript())
        .env("CALLS", &stubs.calls)
        .env("FRONTMOST_APP", frontmost)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the hook runs");
    child.wait().expect("the hook exits");
    stubs.recorded()
}

/// A tmux server of this suite's own, on a socket of its own.
///
/// A cut-down cousin of `crates/tmux-tools/tests/support/mod.rs`. Not shared
/// with it: that module lives in another crate's test tree, and promoting a
/// helper across crates for one caller is the copy this repo's rules say to
/// defer until a third caller wants it.
struct Server {
    socket: String,
    socket_dir: TempDir,
}

impl Server {
    fn new(label: &str) -> Self {
        let socket = format!("nt-{label}-{}", std::process::id());
        // Directly under /tmp, never the ambient temp directory: a socket
        // under /var/folders/.../T/ overruns the 104-byte cap a Unix socket
        // path has on macOS, and tmux reports that as "File name too long".
        let socket_dir = tempfile::Builder::new()
            .prefix("nt-")
            .tempdir_in("/tmp")
            .expect("a socket directory under /tmp");
        let path_length = socket_dir
            .path()
            .join(format!("tmux-0/{socket}"))
            .as_os_str()
            .len();
        assert!(
            path_length <= 104,
            "the socket path is {path_length} bytes, over the 104-byte cap"
        );
        Self { socket, socket_dir }
    }

    /// Runs one tmux command against this server.
    ///
    /// Every call goes through here, so the `TMUX_TMPDIR` that relocates the
    /// socket is set once and cannot be missed by a teardown path.
    fn tmux(&self, arguments: &[&str]) -> Output {
        Command::new("tmux")
            .env("TMUX_TMPDIR", self.socket_dir.path())
            .args(["-L", &self.socket])
            .args(arguments)
            .output()
            .expect("tmux runs")
    }

    fn stdout(&self, arguments: &[&str]) -> String {
        String::from_utf8_lossy(&self.tmux(arguments).stdout)
            .trim_end_matches('\n')
            .to_string()
    }

    /// The value tmux exports as `$TMUX`, pointed at this server.
    ///
    /// The hook only consults tmux when `$TMUX` and `$TMUX_PANE` are both
    /// set, and the socket path in the first field is what sends the hook's
    /// own `tmux display` call to this server rather than the developer's.
    fn tmux_variable(&self, session: &str) -> String {
        let socket_path = self.stdout(&["display-message", "-p", "-t", session, "#{socket_path}"]);
        let server_pid = self.stdout(&["display-message", "-p", "-t", session, "#{pid}"]);
        let session_id = self.stdout(&["display-message", "-p", "-t", session, "#{session_id}"]);
        format!(
            "{socket_path},{server_pid},{}",
            session_id.trim_start_matches('$')
        )
    }

    /// A detached session running an inert command rather than a shell, with
    /// the globally installed window-naming hooks neutralised.
    ///
    /// The inert command is not a convenience. `.zshrc`'s precmd calls the
    /// window-naming script on every prompt, so a shell-backed window renames
    /// itself a beat after creation. A conversion that spawns shells in test
    /// windows brings that race back as flakiness rather than as a failure.
    fn new_session(&self, name: &str, directory: &Path) {
        let directory = directory.to_str().expect("a utf-8 directory");
        self.tmux(&[
            "new-session",
            "-d",
            "-s",
            name,
            "-c",
            directory,
            "sleep",
            "86400",
        ]);
        // An explicit no-op at index [0], not the empty string: a hook set to
        // '' leaves the inherited global array entry in place and the global
        // hook goes on firing.
        for hook in [
            "after-new-window",
            "after-split-window",
            "after-select-window",
            "after-select-pane",
            "after-kill-pane",
            "client-session-changed",
        ] {
            self.tmux(&[
                "set-hook",
                "-t",
                name,
                &format!("{hook}[0]"),
                "run-shell -b true",
            ]);
        }
    }

    fn new_window(&self, session: &str, directory: &Path) -> String {
        let directory = directory.to_str().expect("a utf-8 directory");
        self.stdout(&[
            "new-window",
            "-d",
            "-t",
            session,
            "-c",
            directory,
            "-P",
            "-F",
            "#{window_id}",
            "sleep",
            "86400",
        ])
    }
}

impl Drop for Server {
    /// Kills through [`Server::tmux`], the same wrapper that set
    /// `TMUX_TMPDIR`. A bare `tmux -L <socket> kill-server` addresses the
    /// shared directory instead, so the real server keeps running and the
    /// `TempDir` drop that follows removes its socket from under it.
    fn drop(&mut self) {
        self.tmux(&["kill-server"]);
    }
}

/// Runs the hook as if it were executing inside a given pane of a test
/// session.
fn run_notify_in_pane(
    stubs: &Stubs,
    server: &Server,
    session: &str,
    pane: &str,
    frontmost: &str,
    directory: &Path,
) -> String {
    stubs.clear();
    let mut child = Command::new(hook())
        .arg("stop")
        .current_dir(directory)
        .env("TMUX", server.tmux_variable(session))
        .env("TMUX_PANE", pane)
        .env("TMUX_TMPDIR", server.socket_dir.path())
        .env("AEROSPACE_BIN", stubs.aerospace())
        .env("OSASCRIPT_BIN", stubs.osascript())
        .env("CALLS", &stubs.calls)
        .env("FRONTMOST_APP", frontmost)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the hook runs");
    child.wait().expect("the hook exits");
    stubs.recorded()
}

/// Outside tmux, frontmost-Alacritty alone decides.
#[test]
fn focus_suppression_outside_tmux() {
    let stubs = Stubs::new();
    let workspace = tempfile::tempdir().expect("a temporary directory");

    assert_eq!(
        run_notify(&stubs, "Alacritty", "stop", workspace.path()),
        "",
        "the notification fired while the user was already looking at the terminal"
    );

    // A positive control for the two suppression assertions above and below:
    // an empty record must mean "suppressed", not "the stub never ran".
    assert!(
        run_notify(&stubs, "Safari", "stop", workspace.path()).contains("display notification"),
        "the hook fired nothing with another app frontmost, so an empty \
         record proves nothing about suppression"
    );

    assert!(
        run_notify(&stubs, "", "stop", workspace.path()).contains("display notification"),
        "the hook stayed silent when aerospace reported nothing, so a \
         machine without aerospace would never notify"
    );
}

/// Inside tmux, the pane must also be the active pane of the active window.
#[test]
fn focus_suppression_inside_tmux() {
    if Command::new("tmux").arg("-V").output().is_err() {
        skip("tmux is not installed, so the in-pane focus cases cannot run");
        return;
    }
    let stubs = Stubs::new();
    let workspace = tempfile::tempdir().expect("a temporary directory");
    let server = Server::new("focus");
    let session = "notify-focus";
    server.new_session(session, workspace.path());

    let window = server.new_window(session, workspace.path());
    server.tmux(&[
        "split-window",
        "-d",
        "-t",
        &window,
        "-c",
        workspace.path().to_str().expect("a utf-8 directory"),
    ]);
    server.tmux(&["select-window", "-t", &window]);

    let panes = server.stdout(&["list-panes", "-t", &window, "-F", "#{pane_id}"]);
    let panes: Vec<&str> = panes.lines().collect();
    assert_eq!(panes.len(), 2, "the split did not produce two panes");
    let active_pane = server.stdout(&["display-message", "-p", "-t", &window, "#{pane_id}"]);
    let idle_pane = panes
        .iter()
        .find(|pane| **pane != active_pane)
        .expect("a second pane");

    assert_eq!(
        run_notify_in_pane(
            &stubs,
            &server,
            session,
            &active_pane,
            "Alacritty",
            workspace.path()
        ),
        "",
        "the notification fired in the pane the user is looking at"
    );

    assert!(
        run_notify_in_pane(
            &stubs,
            &server,
            session,
            idle_pane,
            "Alacritty",
            workspace.path()
        )
        .contains("display notification"),
        "an inactive pane was treated as focused, so work finishing out of \
         sight would notify nobody"
    );

    let other_window = server.new_window(session, workspace.path());
    let background_pane = server.stdout(&["list-panes", "-t", &other_window, "-F", "#{pane_id}"]);
    let background_pane = background_pane.lines().next().expect("a pane");
    assert!(
        run_notify_in_pane(
            &stubs,
            &server,
            session,
            background_pane,
            "Alacritty",
            workspace.path()
        )
        .contains("display notification"),
        "a pane in a non-active window was treated as focused"
    );

    // The kill is what proves TMUX_TMPDIR took effect at all: a socket left
    // in the shared directory means every call above reached the shared
    // server, which tmux reports with exit 0 and no message.
    server.tmux(&["kill-server"]);
    let uid = String::from_utf8(
        Command::new("id")
            .arg("-u")
            .output()
            .expect("id -u runs")
            .stdout,
    )
    .expect("utf-8");
    let stray = PathBuf::from(format!("/tmp/tmux-{}/{}", uid.trim(), server.socket));
    assert!(
        !stray.exists(),
        "TMUX_TMPDIR did not take effect: the server's socket is in the \
         shared directory at {}",
        stray.display()
    );
}

/// Each notification kind gets its own title.
#[test]
fn the_title_names_the_kind() {
    let stubs = Stubs::new();
    let workspace = tempfile::tempdir().expect("a temporary directory");

    assert!(
        run_notify(&stubs, "Safari", "stop", workspace.path()).contains("Claude finished"),
        "the stop kind did not use the finished title"
    );
    assert!(
        run_notify(&stubs, "Safari", "notification", workspace.path())
            .contains("Claude needs input"),
        "the notification kind did not use the needs-input title"
    );
    assert!(
        run_notify(&stubs, "Safari", "something-else", workspace.path())
            .contains("with title \"Claude\""),
        "an unknown kind did not fall back to a plain title"
    );
}

/// The message body is the working directory's basename, with double quotes
/// escaped for the AppleScript string literal.
#[test]
fn the_message_is_the_escaped_directory_name() {
    let stubs = Stubs::new();
    let workspace = tempfile::tempdir().expect("a temporary directory");

    let named = workspace.path().join("my-project");
    fs::create_dir_all(&named).expect("the fixture directory is creatable");
    assert!(
        run_notify(&stubs, "Safari", "stop", &named).contains("notification \"my-project\""),
        "the message was not the basename of the working directory"
    );

    // An unescaped quote closes the AppleScript string literal early, so
    // osascript sees a different script than the one intended.
    let quoted = workspace.path().join("say \"hi\"");
    fs::create_dir_all(&quoted).expect("the fixture directory is creatable");
    assert!(
        run_notify(&stubs, "Safari", "stop", &quoted).contains("\\\"hi\\\""),
        "a double quote in the directory name was not escaped"
    );
}

/// The hook exits zero and drains stdin whichever way it decides.
///
/// A hook that exits non-zero, or that leaves the payload on the pipe, stalls
/// the turn it was called from.
#[test]
fn the_hook_drains_stdin_and_exits_zero() {
    let stubs = Stubs::new();
    let workspace = tempfile::tempdir().expect("a temporary directory");

    let fired = Command::new(hook())
        .arg("stop")
        .current_dir(workspace.path())
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .env("AEROSPACE_BIN", stubs.aerospace())
        .env("OSASCRIPT_BIN", stubs.osascript())
        .env("CALLS", &stubs.calls)
        .env("FRONTMOST_APP", "Safari")
        .stdin(Stdio::null())
        .output()
        .expect("the hook runs");
    assert!(
        fired.status.success(),
        "the hook exited {:?} after firing",
        fired.status.code()
    );

    let mut child = Command::new(hook())
        .arg("stop")
        .current_dir(workspace.path())
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .env("AEROSPACE_BIN", stubs.aerospace())
        .env("OSASCRIPT_BIN", stubs.osascript())
        .env("CALLS", &stubs.calls)
        .env("FRONTMOST_APP", "Alacritty")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the hook runs");
    child
        .stdin
        .as_mut()
        .expect("a stdin pipe")
        .write_all(b"some hook payload")
        .expect("the payload is writable");
    let suppressed = child.wait().expect("the hook exits");
    assert!(
        suppressed.success(),
        "the hook exited {:?} when suppressed with a payload on stdin",
        suppressed.code()
    );
}
