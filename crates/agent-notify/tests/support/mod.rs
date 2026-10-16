//! A sandbox for running the real `agent-notify` binary: a private socket
//! path, log directory, and recording stubs for both fallback notifiers, so
//! no test shows a real notification.

// Each test binary uses a different part of this module.
#![allow(dead_code)]

use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tempfile::TempDir;

const RECORDING_STUB: &str = "#!/bin/sh\ncall=$(mktemp \"$CALLS_DIR/call.XXXXXX\")\nfor argument in \"$@\"; do printf '%s\\0' \"$argument\" >> \"$call\"; done\n";

static ONE_SANDBOX_AT_A_TIME: Mutex<()> = Mutex::new(());

pub struct Sandbox {
    root: TempDir,
    stubs: PathBuf,
    calls: PathBuf,
    pub socket: PathBuf,
    pub logs: PathBuf,
    _turn: MutexGuard<'static, ()>,
}

impl Sandbox {
    /// Waits for the previous test's sandbox to drop. Tests assert wall-clock
    /// bounds, and concurrent sandboxes queue behind each other's process
    /// launches, which an endpoint scanner serializes on a developer machine.
    /// A test that holds two sandboxes at once deadlocks.
    pub fn new() -> Self {
        let turn = ONE_SANDBOX_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner);
        // Under /tmp: a Unix socket path is capped at 104 bytes on macOS.
        let root = tempfile::Builder::new().prefix("an-").tempdir_in("/tmp").expect("a directory under /tmp");
        let stubs = root.path().join("bin");
        let calls = root.path().join("calls");
        fs::create_dir_all(&stubs).expect("stub dir");
        fs::create_dir_all(&calls).expect("calls dir");
        for name in ["terminal-notifier", "notify-send"] {
            dotfiles_test_support::stub::write(&stubs.join(name), RECORDING_STUB).expect("a stub");
        }
        Sandbox { socket: root.path().join("agent-notify.sock"), logs: root.path().join("logs"), stubs, calls, root, _turn: turn }
    }

    pub fn command(&self, arguments: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_agent-notify"));
        let path = format!("{}:{}", self.stubs.display(), std::env::var("PATH").unwrap_or_default());
        command
            .args(arguments)
            .current_dir(self.root.path())
            .env_clear()
            .env("PATH", path)
            .env("HOME", self.root.path())
            .env("AGENT_NOTIFY_SOCKET", &self.socket)
            .env("AGENT_NOTIFY_LOG_DIR", &self.logs)
            .env("CALLS_DIR", &self.calls);
        command
    }

    /// Replaces or adds the program `name` on the sandbox `PATH`.
    pub fn stub(&self, name: &str, script: &str) {
        dotfiles_test_support::stub::write(&self.stubs.join(name), script).expect("a stub");
    }

    /// Runs `command` with `stdin`, capturing its output. Panics when the
    /// command outlives [`RUN_DEADLINE`], so a client bug fails the suite.
    pub fn run(&self, command: &mut Command, stdin: &str) -> Output {
        let mut child = start(command.stdout(Stdio::piped()).stderr(Stdio::piped()), stdin);
        let stdout = drain(child.stdout.take());
        let stderr = drain(child.stderr.take());
        let status = finish(&mut child, command);
        Output { status, stdout: stdout.join().expect("stdout reader"), stderr: stderr.join().expect("stderr reader") }
    }

    /// Like [`Sandbox::run`], but leaves stdout and stderr as the caller set them.
    pub fn run_with_own_output(&self, command: &mut Command, stdin: &str) -> ExitStatus {
        let mut child = start(command, stdin);
        finish(&mut child, command)
    }

    /// One entry per notifier call, each the call's argv after the program name.
    pub fn fallback_calls(&self) -> Vec<Vec<String>> {
        let mut files: Vec<PathBuf> = fs::read_dir(&self.calls).expect("calls dir").map(|entry| entry.expect("entry").path()).collect();
        files.sort();
        files
            .iter()
            .map(|file| {
                let text = fs::read_to_string(file).expect("call file");
                text.split('\0').filter(|argument| !argument.is_empty()).map(str::to_string).collect()
            })
            .collect()
    }

    pub fn log_text(&self) -> String {
        fs::read_to_string(self.logs.join("cli.log")).unwrap_or_default()
    }
}

const RUN_DEADLINE: Duration = Duration::from_secs(30);

fn start(command: &mut Command, stdin: &str) -> Child {
    let mut child = command.stdin(Stdio::piped()).spawn().expect("agent-notify starts");
    if let Some(mut input) = child.stdin.take() {
        input.write_all(stdin.as_bytes()).expect("stdin write");
    }
    child
}

fn drain(stream: Option<impl Read + Send + 'static>) -> JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut stream) = stream {
            stream.read_to_end(&mut bytes).expect("child output");
        }
        bytes
    })
}

fn finish(child: &mut Child, command: &Command) -> ExitStatus {
    let deadline = Instant::now() + RUN_DEADLINE;
    loop {
        if let Some(status) = child.try_wait().expect("a child status") {
            return status;
        }
        if Instant::now() >= deadline {
            let _killed = child.kill();
            let _reaped = child.wait();
            panic!("{command:?} ran longer than {RUN_DEADLINE:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub enum ReplyMode {
    Ack,
    Reject,
    Never,
}

const ACCEPT_DEADLINE: Duration = Duration::from_secs(10);
/// How long a [`ReplyMode::Never`] daemon holds the connection open. A
/// client that waits for the daemon takes at least this long.
const SILENT_HOLD: Duration = Duration::from_secs(2);
const ACCEPT_POLL: Duration = Duration::from_millis(10);

/// Accepts one connection on `path`. Returns the request line it read, or
/// `None` when no client connects within [`ACCEPT_DEADLINE`], so a client
/// bug fails a test instead of hanging the suite.
pub fn listen(path: &Path, mode: ReplyMode) -> JoinHandle<Option<String>> {
    let listener = UnixListener::bind(path).expect("bind the test socket");
    listener.set_nonblocking(true).expect("a non-blocking listener");
    std::thread::spawn(move || {
        let stream = accept_before(&listener, Instant::now() + ACCEPT_DEADLINE)?;
        stream.set_nonblocking(false).ok()?;
        let answer = match mode {
            ReplyMode::Never => {
                std::thread::sleep(SILENT_HOLD);
                return None;
            }
            ReplyMode::Ack => r#"{"ok":true,"daemon":"test"}"#,
            ReplyMode::Reject => r#"{"ok":false,"code":"unknown_kind","accepts":[1,1]}"#,
        };
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line).ok()?;
        (&stream).write_all(format!("{answer}\n").as_bytes()).ok()?;
        Some(line)
    })
}

fn accept_before(listener: &UnixListener, deadline: Instant) -> Option<UnixStream> {
    loop {
        match listener.accept() {
            Ok((stream, _)) => return Some(stream),
            Err(error) if error.kind() == ErrorKind::WouldBlock && Instant::now() < deadline => std::thread::sleep(ACCEPT_POLL),
            Err(_) => return None,
        }
    }
}
