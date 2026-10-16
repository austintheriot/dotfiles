//! Shared test helpers: a throwaway tmux server and its identity.

#![allow(dead_code)] // Each integration target uses a different subset.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use agent_notify_core::{PaneId, PaneKey, ServerId, SocketPath};
use agent_notify_daemon::services::{AttachRequest, WindowRaiser};
use agent_notify_daemon::tmux::Tmux;
pub use dotfiles_test_support::tmux_server::Server;

/// A tmux server with one session, `main`, in a scratch directory.
pub fn tmux_server(label: &str) -> Server {
    let server = Server::new(label);
    server.new_session("main", Path::new("/tmp"));
    server
}

/// The server's identity, read the way the daemon reads it.
pub fn server_id(server: &Server) -> ServerId {
    ServerId {
        socket_path: socket(server),
        pid: server.pid(),
        start_time: server.stdout(&["display-message", "-p", "#{start_time}"]).parse().expect("a start time"),
    }
}

/// The server's socket path as the core type.
pub fn socket(server: &Server) -> SocketPath {
    SocketPath::parse(&server.socket_path()).expect("an absolute socket path")
}

/// The adapter, pointed at the `tmux` on `PATH`.
pub fn adapter() -> Tmux {
    let found = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|directory| directory.join("tmux"))
        .find(|candidate| candidate.is_file())
        .expect("tmux on PATH");
    Tmux::new(found)
}

/// The first pane of `target`, for example `main:0`.
pub fn first_pane(server: &Server, target: &str) -> PaneId {
    let id = server.stdout(&["list-panes", "-t", target, "-F", "#{pane_id}"]);
    PaneId::parse(id.lines().next().expect("a pane")).expect("a pane id")
}

/// A second pane in the window of `target`, running the same long sleep as
/// the harness, so a pane that outlives its test cannot wait on input.
pub fn split(server: &Server, target: &str) -> PaneId {
    let id = server.stdout(&["split-window", "-d", "-t", target, "-P", "-F", "#{pane_id}", "sleep", "600"]);
    PaneId::parse(&id).expect("a pane id")
}

/// A new window in `session`, returning its first pane.
pub fn new_window_in(server: &Server, session: &str) -> PaneId {
    let window = server.new_window(session, Path::new("/tmp"), &[]);
    first_pane(server, &window)
}

/// A new window in `main`, returning its first pane.
pub fn new_window(server: &Server) -> PaneId {
    new_window_in(server, "main")
}

/// Tags `pane` with a session id, as the SessionStart hook does.
pub fn tag(server: &Server, pane: &PaneId, session: &str) {
    let status = server.tmux(&["set-option", "-p", "-t", pane.as_str(), "@agent_notify_session", session]).status;
    assert!(status.success(), "tagging {}", pane.as_str());
}

/// One pane option, or `""` when unset.
pub fn pane_option(server: &Server, pane: &PaneId, option: &str) -> String {
    server.stdout(&["show-options", "-p", "-v", "-t", pane.as_str(), option])
}

/// One window option of the window holding `pane`, or `""` when unset.
pub fn window_option(server: &Server, pane: &PaneId, option: &str) -> String {
    server.stdout(&["show-options", "-w", "-v", "-t", pane.as_str(), option])
}

/// A key for `pane` on `server`.
pub fn key(server: &Server, pane: &PaneId) -> PaneKey {
    PaneKey { server: server_id(server), pane: pane.clone() }
}

/// A raiser that records what it was asked to do and answers `!fail`.
#[derive(Debug, Default)]
pub struct RecordingRaiser {
    pub raises: Mutex<u32>,
    pub attaches: Mutex<Vec<AttachRequest>>,
    pub fail: bool,
}

impl WindowRaiser for RecordingRaiser {
    fn raise(&self) -> bool {
        *self.raises.lock().expect("unpoisoned") += 1;
        !self.fail
    }

    fn open_terminal(&self, request: &AttachRequest) -> bool {
        self.attaches.lock().expect("unpoisoned").push(request.clone());
        !self.fail
    }
}

/// The socket path of `server` as a `PathBuf`, for `AttachRequest` checks.
pub fn socket_of(server: &Server) -> PathBuf {
    server.socket_path()
}
