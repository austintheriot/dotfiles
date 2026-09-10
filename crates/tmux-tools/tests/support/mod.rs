//! The throwaway tmux server every tmux-tools integration test drives.
//!
//! Promoted here on its third caller. `name_windows.rs`, `split.rs` and
//! `close_and_worktree_config.rs` each carried their own copy of `fn tmux`
//! and `fn shared_socket_path`, so a trap fixed in one stayed live in the
//! other two.
//!
//! Three tmux behaviours cost a debugging cycle on 2026-09-09 and are
//! assertions here rather than comments, because a comment does not fail:
//!
//! - tmux falls back to the shared socket directory, exits 0 and says
//!   nothing when `TMUX_TMPDIR` names a directory whose parent is missing.
//!   [`Server::new`] creates the directory before the first tmux call and
//!   [`Server::shutdown`] asserts the shared directory holds no socket of
//!   this server's own.
//! - A Unix socket path is capped at 104 bytes on macOS, and a temp
//!   directory under `/var/folders/.../T/` overruns it. The directory is
//!   built directly under `/tmp`.
//! - A teardown that does not set `TMUX_TMPDIR` aims at a path that no
//!   longer exists, leaves the real server running, and then removes its
//!   socket from under it. [`Drop`] kills through [`Server::tmux`], the same
//!   wrapper that set the variable.

// Each integration test binary compiles this module separately and uses
// only the part it needs, so every helper is dead code in some binary. The
// alternative is a per-item allow on nearly every item here.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A tmux server on a socket of its own, in a directory of its own.
///
/// Every call goes through [`Server::tmux`], so the `TMUX_TMPDIR` that
/// relocates the socket is set once and cannot be missed by a teardown
/// path. Dropping the value kills the server and removes the directory.
pub struct Server {
    socket: String,
    socket_dir: tempfile::TempDir,
}

impl Server {
    /// Starts no server, but reserves a socket name and creates the
    /// directory tmux will put the socket in.
    ///
    /// `label` distinguishes one test's socket from another's in the same
    /// process, since `cargo test` runs them in parallel threads.
    ///
    /// # Panics
    ///
    /// Panics when the socket directory cannot be created under `/tmp`, or
    /// when the resulting socket path would exceed the 104-byte cap a Unix
    /// socket path has on macOS. Both are environment failures a test has
    /// no way to recover from, and both are silent in tmux itself.
    #[must_use]
    pub fn new(label: &str) -> Self {
        let socket = format!("tt-{label}-{}", std::process::id());
        // Directly under /tmp, never the ambient temp directory: a socket
        // under /var/folders/.../T/ measured 111 bytes and tmux failed with
        // "File name too long".
        let socket_dir = tempfile::Builder::new()
            .prefix("tt-")
            .tempdir_in("/tmp")
            .expect("a socket directory under /tmp");

        // tmux creates <TMUX_TMPDIR>/tmux-<uid>/<socket>, so the byte count
        // the kernel sees is longer than the directory alone.
        let path_length = socket_dir.path().join(format!("tmux-0/{socket}")).as_os_str().len();
        assert!(
            path_length <= 104,
            "the socket path is {path_length} bytes, over the 104-byte cap: {}",
            socket_dir.path().display()
        );

        Server { socket, socket_dir }
    }

    /// The socket name, for handing to the binary under test through
    /// `TMUX_TOOLS_SOCKET`.
    #[must_use]
    pub fn socket(&self) -> &str {
        &self.socket
    }

    /// The directory holding the socket, for handing to the binary under
    /// test through `TMUX_TMPDIR`.
    #[must_use]
    pub fn socket_dir(&self) -> &Path {
        self.socket_dir.path()
    }

    /// Runs one tmux command against this server.
    ///
    /// # Panics
    ///
    /// Panics when `tmux` cannot be executed at all.
    pub fn tmux(&self, arguments: &[&str]) -> Output {
        Command::new("tmux")
            .env("TMUX_TMPDIR", self.socket_dir.path())
            .args(["-L", &self.socket])
            .args(arguments)
            .output()
            .expect("tmux runs")
    }

    /// Runs one tmux command and returns its stdout, trailing newline
    /// trimmed.
    #[must_use]
    pub fn stdout(&self, arguments: &[&str]) -> String {
        String::from_utf8_lossy(&self.tmux(arguments).stdout)
            .trim_end_matches('\n')
            .to_string()
    }

    /// Creates a detached session running an inert command rather than a
    /// shell, and neutralises the globally installed window-naming hooks.
    ///
    /// The inert command is not a convenience. `.zshrc`'s precmd calls the
    /// window-naming script on every prompt, so a shell-backed window
    /// renames itself a beat after creation and overwrites the name an
    /// assertion is about to read. Fixed once in `8591f242` for the shell
    /// suite; a conversion that spawns shells here brings the race back,
    /// and it presents as flakiness rather than as a failure.
    pub fn new_session(&self, name: &str, directory: &Path) {
        let directory = directory.to_str().expect("a utf-8 directory");
        self.tmux(&[
            "new-session",
            "-d",
            "-s",
            name,
            "-c",
            directory,
            INERT_WINDOW_COMMAND,
            INERT_WINDOW_ARGUMENT,
        ]);
        self.isolate_hooks(name);
    }

    /// Creates a detached window running the same inert command, and
    /// returns its `#{window_id}`.
    ///
    /// `extra` carries flags such as `-n <name>` for the cases that need a
    /// window tmux does not consider automatically renamable.
    #[must_use]
    pub fn new_window(&self, session: &str, directory: &Path, extra: &[&str]) -> String {
        let directory = directory.to_str().expect("a utf-8 directory");
        let mut arguments = vec!["new-window", "-d", "-t", session, "-c", directory];
        arguments.extend_from_slice(extra);
        arguments.extend_from_slice(&[
            "-P",
            "-F",
            "#{window_id}",
            INERT_WINDOW_COMMAND,
            INERT_WINDOW_ARGUMENT,
        ]);
        self.stdout(&arguments)
    }

    /// Overrides every hook the real config installs globally, so a test
    /// session's windows are renamed only by the calls the test makes.
    ///
    /// An explicit no-op at index `[0]`, not the empty string: a hook set
    /// to `''` leaves the inherited global array entry in place and the
    /// global hook goes on firing, which `8591f242` already fixed once.
    fn isolate_hooks(&self, session: &str) {
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
                session,
                &format!("{hook}[0]"),
                "run-shell -b true",
            ]);
        }
    }

    /// Kills the server and asserts it left no socket in the shared tmux
    /// directory.
    ///
    /// A socket there means `TMUX_TMPDIR` never took effect and every call
    /// in the test reached the shared server instead. tmux reports that
    /// with exit 0 and no message, so only this assertion catches it.
    ///
    /// [`Drop`] calls this too, so a test that returns early or panics
    /// still kills its server; calling it explicitly is what gives the
    /// assertion a place to fail from.
    ///
    /// # Panics
    ///
    /// Panics when the shared tmux directory holds this server's socket, or
    /// when the server is still running after the kill.
    pub fn shutdown(&self) {
        self.tmux(&["kill-server"]);

        let stray = shared_socket_path(&self.socket);
        assert!(
            !stray.exists(),
            "TMUX_TMPDIR did not take effect: the server's socket is in the shared directory at {}",
            stray.display()
        );

        // The kill has to have reached the real server. A teardown that
        // does not set TMUX_TMPDIR addresses the shared directory, fails
        // with "error connecting", and leaves the server running with its
        // socket about to be removed from under it by the TempDir drop.
        // That failure is invisible in the shared directory, so `has-server`
        // through this same wrapper is what observes it.
        let alive = self.tmux(&["has-session"]).status.success();
        assert!(
            !alive,
            "the server on socket {} survived kill-server: the teardown did not reach it",
            self.socket
        );
    }
}

impl Drop for Server {
    /// Kills the server through [`Server::tmux`], which is the wrapper that
    /// set `TMUX_TMPDIR`.
    ///
    /// A teardown that shells out to a bare `tmux -L <socket> kill-server`
    /// addresses the shared directory instead, so the real server keeps
    /// running and the `TempDir` drop that follows removes its socket from
    /// under it.
    fn drop(&mut self) {
        self.tmux(&["kill-server"]);
    }
}

/// Where tmux puts a socket when nothing relocates it.
///
/// Every test socket this suite ever created stayed here: tmux 3.4 does not
/// unlink the file on `kill-server`, and 1435 dead sockets were found in
/// the shared directory.
///
/// # Panics
///
/// Panics when `id -u` cannot be run or does not print a user id.
#[must_use]
pub fn shared_socket_path(socket: &str) -> PathBuf {
    let uid = Command::new("id").arg("-u").output().expect("id -u runs");
    let uid = String::from_utf8(uid.stdout).expect("utf-8");
    PathBuf::from(format!("/tmp/tmux-{}/{socket}", uid.trim()))
}

/// The checkout under test, for the tests that assert against a tracked
/// shell shim.
///
/// `DOTFILES_ROOT` then `HOME`, accepting either only when
/// `crates/Cargo.toml` is under it, and falling back to the manifest walk-up
/// for the `rust-checks.sh` snapshot where neither variable points at the
/// archived tree. This mirrors `dotfiles_test_support::repo::root`, which
/// this crate cannot depend on without changing `crates/Cargo.lock`.
///
/// Not `CARGO_MANIFEST_DIR` alone: that is a compile-time constant, so a
/// binary built in one tree and run against another reads the wrong root,
/// which is how tests passed on the host and failed under the gate.
///
/// # Panics
///
/// Panics when no candidate holds `crates/Cargo.toml` and the manifest
/// directory has no grandparent.
#[must_use]
pub fn repo_root() -> PathBuf {
    for variable in ["DOTFILES_ROOT", "HOME"] {
        if let Some(value) = std::env::var_os(variable) {
            let candidate = PathBuf::from(value);
            if candidate.join("crates/Cargo.toml").is_file() {
                return candidate;
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the repo root is two levels above this crate")
        .to_path_buf()
}

/// The command a test window runs instead of an interactive shell.
///
/// A process that sits there gives a window with a real working directory
/// and no opinion about its own name. `sleep` rather than `cat`, so a
/// window that outlives its test cannot sit on a pipe waiting for input.
const INERT_WINDOW_COMMAND: &str = "sleep";

/// `sleep`'s argument, kept beside it so the two cannot drift apart.
const INERT_WINDOW_ARGUMENT: &str = "86400";
