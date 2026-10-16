//! Shared test helpers: a throwaway tmux server and its identity, and a
//! running daemon on a private socket.

#![allow(dead_code)] // Each integration target uses a different subset.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use agent_notify_core::EpochMs;
use agent_notify_core::status::DaemonFacts;
use agent_notify_core::view::PanelViewModel;
use agent_notify_core::{PaneId, PaneKey, ServerId, SocketPath};
use agent_notify_daemon::actor::{Daemon, DaemonConfig};
use agent_notify_daemon::dnd_store::DndStore;
use agent_notify_daemon::services::{AttachRequest, Presence, PresenceReader, Services, Silent, ToastPresenter, UnknownFrontmost, UnknownPresence, WindowRaiser};
use agent_notify_daemon::socket::{BindOutcome, ServeOptions, bind};
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

/// A raiser that records what it was asked to do, waits `delay`, and
/// answers `!fail`.
#[derive(Debug, Default)]
pub struct RecordingRaiser {
    pub raises: Mutex<u32>,
    pub attaches: Mutex<Vec<AttachRequest>>,
    pub fail: bool,
    pub delay: Duration,
}

impl WindowRaiser for RecordingRaiser {
    fn raise(&self) -> bool {
        *self.raises.lock().expect("unpoisoned") += 1;
        std::thread::sleep(self.delay);
        !self.fail
    }

    fn open_terminal(&self, request: &AttachRequest) -> bool {
        self.attaches.lock().expect("unpoisoned").push(request.clone());
        std::thread::sleep(self.delay);
        !self.fail
    }
}

/// A presence reader that counts its reads and takes `delay` to answer,
/// like a slow `ioreg`.
#[derive(Debug, Default)]
pub struct SlowPresence {
    pub delay: Duration,
    pub reads: AtomicU32,
}

impl PresenceReader for SlowPresence {
    fn presence(&self) -> Presence {
        self.reads.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(self.delay);
        Presence::default()
    }
}

/// A presenter that keeps every view it is given.
#[derive(Debug, Default)]
pub struct RecordingPresenter {
    views: Mutex<Vec<PanelViewModel>>,
    changed: Condvar,
}

impl ToastPresenter for RecordingPresenter {
    fn present(&self, view: &PanelViewModel) {
        self.views.lock().expect("unpoisoned").push(view.clone());
        self.changed.notify_all();
    }
}

impl RecordingPresenter {
    /// Waits up to 3 s for a presented view that satisfies `test`.
    pub fn wait_for(&self, test: impl Fn(&PanelViewModel) -> bool) -> PanelViewModel {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut views = self.views.lock().expect("unpoisoned");
        loop {
            if let Some(found) = views.iter().rev().find(|view| test(view)) {
                return found.clone();
            }
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "no matching view in 3 s; saw {views:#?}");
            views = self.changed.wait_timeout(views, left).expect("unpoisoned").0;
        }
    }

    /// How many views were pushed.
    pub fn pushes(&self) -> usize {
        self.views.lock().expect("unpoisoned").len()
    }
}

static DAEMON_TESTS: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Runs daemon tests one at a time. Each starts a tmux server that loads the
/// user's tmux config, and eleven at once kept the CPU so busy that the
/// adapter's tmux calls hit their 1 s limit, which drops a pane-state write
/// or fails a focus plan. Hold the guard for the whole test.
pub async fn one_daemon_at_a_time() -> tokio::sync::MutexGuard<'static, ()> {
    DAEMON_TESTS.lock().await
}

/// What a test may change about the daemon it starts.
pub struct DaemonSetup {
    pub presence: Arc<dyn PresenceReader>,
    pub raiser: Arc<RecordingRaiser>,
    pub config: DaemonConfig,
}

impl Default for DaemonSetup {
    fn default() -> Self {
        DaemonSetup {
            presence: Arc::new(UnknownPresence),
            raiser: Arc::new(RecordingRaiser::default()),
            config: DaemonConfig { serve: ServeOptions { line_timeout: Duration::from_millis(500), ..ServeOptions::default() }, ..DaemonConfig::default() },
        }
    }
}

/// A running daemon on a private socket, a tmux server, and recorders.
pub struct DaemonWorld {
    pub daemon: Daemon,
    pub socket: PathBuf,
    pub presenter: Arc<RecordingPresenter>,
    pub raiser: Arc<RecordingRaiser>,
    pub server: Server,
    pub dnd_path: PathBuf,
    pub scratch: tempfile::TempDir,
}

fn scratch_directory() -> tempfile::TempDir {
    tempfile::Builder::new().prefix("an-").tempdir_in("/tmp").expect("a directory under /tmp")
}

/// Starts a daemon whose tmux adapter points at a fresh tmux server.
pub fn spawn_daemon(label: &str) -> DaemonWorld {
    spawn_daemon_with(label, scratch_directory(), DaemonSetup::default())
}

/// Starts a daemon with its state in `scratch`, which may hold a DND file already.
pub fn spawn_daemon_in(label: &str, scratch: tempfile::TempDir) -> DaemonWorld {
    spawn_daemon_with(label, scratch, DaemonSetup::default())
}

/// Starts a daemon with `setup` in a fresh scratch directory.
pub fn spawn_daemon_setup(label: &str, setup: DaemonSetup) -> DaemonWorld {
    spawn_daemon_with(label, scratch_directory(), setup)
}

/// Starts a daemon with its state in `scratch` and the services `setup` names.
pub fn spawn_daemon_with(label: &str, scratch: tempfile::TempDir, setup: DaemonSetup) -> DaemonWorld {
    let socket = scratch.path().join("run/agent-notify.sock");
    let dnd_path = scratch.path().join("state/dnd.json");
    let presenter = Arc::new(RecordingPresenter::default());
    let services = Arc::new(Services {
        tmux: Some(adapter()),
        raiser: setup.raiser.clone(),
        presenter: presenter.clone(),
        frontmost: Arc::new(UnknownFrontmost),
        presence: setup.presence,
        sound: Arc::new(Silent),
        dnd_store: DndStore::new(dnd_path.clone()),
        clock: Arc::new(|| EpochMs(1_760_500_000_000)),
        facts: DaemonFacts { stamp: "stamp-test".to_string(), started_ms: EpochMs(1_760_499_000_000), tools: Vec::new() },
    });
    let BindOutcome::Bound(listener) = bind(&socket).expect("binds") else { panic!("nothing else runs here") };
    let daemon = Daemon::start(services, listener, setup.config);
    DaemonWorld { daemon, socket, presenter, raiser: setup.raiser, server: tmux_server(label), dnd_path, scratch }
}

/// Sends one line and returns the reply as JSON, or `Null` when the
/// daemon closed with no reply.
pub async fn send_line(socket: &Path, line: String) -> serde_json::Value {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let mut stream = tokio::net::UnixStream::connect(socket).await.expect("connects");
    stream.write_all(format!("{line}\n").as_bytes()).await.expect("writes");
    let mut reply = String::new();
    tokio::time::timeout(Duration::from_secs(2), BufReader::new(stream).read_line(&mut reply)).await.expect("in time").expect("reads");
    serde_json::from_str(&reply).unwrap_or(serde_json::Value::Null)
}

/// An agent event line from `pane` on `server`, in the wire fixture's shape.
pub fn agent_line(server: &Server, pane: &PaneId, number: u32, session: &str, prompt: &str, event: serde_json::Value) -> String {
    let id = server_id(server);
    serde_json::json!({
        "type": "agent", "v": 1,
        "event_id": format!("{number:026}"),
        "sent_at_ms": 1_760_500_000_000_i64 + i64::from(number),
        "origin": {
            "agent": "claude_code", "session": session, "prompt": prompt,
            "pane": { "server": { "socket_path": id.socket_path.as_path(), "pid": id.pid, "start_time": id.start_time }, "pane": pane.as_str() },
            "window": { "index": 0, "name": "main" },
            "session_title": "Fix sync retry",
            "cwd": "/tmp"
        },
        "event": event
    })
    .to_string()
}

/// A control line.
pub fn control_line(number: u32, control: serde_json::Value) -> String {
    serde_json::json!({ "type": "control", "v": 1, "event_id": format!("{number:026}"), "sent_at_ms": 1_760_500_000_000_i64 + i64::from(number), "control": control })
        .to_string()
}

/// A query line.
pub fn query_line(number: u32, query: &str) -> String {
    serde_json::json!({ "type": "query", "v": 1, "event_id": format!("{number:026}"), "query": query }).to_string()
}

/// The session ids a `status` reply lists.
pub fn status_sessions(reply: &serde_json::Value) -> Vec<String> {
    reply["data"]["sessions"].as_array().map_or_else(Vec::new, |rows| rows.iter().filter_map(|row| row["session"].as_str().map(str::to_string)).collect())
}

/// Asks for `status` every 50 ms until `wanted` holds for the reply, for up
/// to 3 s, and returns that reply.
pub async fn status_until(socket: &Path, wanted: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut number = 9_000;
    loop {
        number += 1;
        let reply = send_line(socket, query_line(number, "status")).await;
        if wanted(&reply) {
            return reply;
        }
        assert!(Instant::now() < deadline, "no matching status in 3 s; last {reply}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Polls `read` until it returns `expected`, for up to 3 s. tmux options
/// change on the worker thread, which has no hook a test can wait on.
pub fn eventually(expected: &str, read: impl Fn() -> String) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let value = read();
        if value == expected {
            return;
        }
        assert!(Instant::now() < deadline, "still {value:?}, wanted {expected:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The socket path of `server` as a `PathBuf`, for `AttachRequest` checks.
pub fn socket_of(server: &Server) -> PathBuf {
    server.socket_path()
}
