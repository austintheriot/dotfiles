//! Child processes with a deadline. A wedged tool must not stall the actor,
//! so every spawn in this crate goes through [`run_bounded`]. The child runs
//! in its own process group, so a kill also reaches what it started. The
//! reap after SIGKILL is not bounded: a process in an uninterruptible kernel
//! wait outlives the signal.

use std::fmt;
use std::io::{self, Read};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

/// The limit for every external tool the daemon runs.
pub const CHILD_LIMIT: Duration = Duration::from_secs(1);
/// The most stderr bytes kept. The rest is read and dropped.
pub const STDERR_LIMIT_BYTES: usize = 64 * 1024;
const POLL: Duration = Duration::from_millis(5);
const READ_GRACE: Duration = Duration::from_millis(50);
const READ_CHUNK_BYTES: usize = 8192;
const KILL: &str = "/bin/kill";

/// How the child's output streams ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamEnd {
    /// Both streams reached end of file.
    Closed,
    /// A process the child started held a stream open past the deadline.
    /// The output is what arrived before the deadline, and the child's
    /// process group was killed.
    HeldOpen,
}

/// A child that exited within its limit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finished {
    /// How it exited.
    pub status: ExitStatus,
    /// What it wrote to stdout.
    pub stdout: Vec<u8>,
    /// The first [`STDERR_LIMIT_BYTES`] it wrote to stderr.
    pub stderr: Vec<u8>,
    /// Whether the output is complete.
    pub stream_end: StreamEnd,
}

/// Why a bounded run produced no [`Finished`].
#[derive(Debug)]
pub enum RunError {
    /// The child did not start.
    Spawn(io::Error),
    /// The child ran past `limit`. It and its process group were killed.
    TimedOut {
        /// The limit it ran past.
        limit: Duration,
    },
    /// Waiting on the child or reading its output failed. A child still
    /// running was killed and reaped.
    Io(io::Error),
}

impl fmt::Display for RunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::Spawn(error) => write!(formatter, "did not start: {error}"),
            RunError::TimedOut { limit } => write!(formatter, "ran longer than {limit:?}"),
            RunError::Io(error) => write!(formatter, "failed while running: {error}"),
        }
    }
}

impl std::error::Error for RunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RunError::Spawn(error) | RunError::Io(error) => Some(error),
            RunError::TimedOut { .. } => None,
        }
    }
}

/// Runs `command` in its own process group with stdin closed and stdout and
/// stderr collected, for at most `limit`. Past the limit it kills the group
/// and reaps the child.
///
/// # Errors
///
/// Returns [`RunError::Spawn`] when the child does not start,
/// [`RunError::TimedOut`] when it runs past `limit`, and [`RunError::Io`]
/// when waiting or reading fails.
pub fn run_bounded(command: &mut Command, limit: Duration) -> Result<Finished, RunError> {
    let deadline = Instant::now() + limit;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(RunError::Spawn)?;
    let events = match start_readers(&mut child) {
        Ok(events) => events,
        Err(error) => {
            stop(&mut child);
            return Err(RunError::Io(error));
        }
    };
    let mut output = Output::default();
    let status = wait_for_exit(&mut child, deadline, limit, &events, &mut output)?;
    let drain_deadline = deadline.max(Instant::now() + READ_GRACE);
    while !output.is_closed() && Instant::now() < drain_deadline {
        output.absorb(&events, drain_deadline.saturating_duration_since(Instant::now()));
    }
    let stream_end = if output.is_closed() {
        StreamEnd::Closed
    } else {
        kill_group(child.id());
        StreamEnd::HeldOpen
    };
    output.finish(status, stream_end)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stream {
    Stdout,
    Stderr,
}

#[derive(Debug)]
enum Event {
    Bytes(Stream, Vec<u8>),
    Closed(Stream),
    Failed(Stream, io::Error),
}

#[derive(Debug, Default)]
struct Output {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdout_closed: bool,
    stderr_closed: bool,
    failure: Option<io::Error>,
}

impl Output {
    fn is_closed(&self) -> bool {
        self.stdout_closed && self.stderr_closed
    }

    fn absorb(&mut self, events: &Receiver<Event>, wait: Duration) {
        if self.is_closed() {
            std::thread::sleep(wait);
            return;
        }
        match events.recv_timeout(wait) {
            Ok(Event::Bytes(Stream::Stdout, bytes)) => self.stdout.extend(bytes),
            Ok(Event::Bytes(Stream::Stderr, bytes)) => self.stderr.extend(bytes),
            Ok(Event::Closed(stream)) => self.close(stream),
            Ok(Event::Failed(stream, error)) => {
                self.close(stream);
                self.failure.get_or_insert(error);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                self.close(Stream::Stdout);
                self.close(Stream::Stderr);
            }
        }
    }

    fn close(&mut self, stream: Stream) {
        match stream {
            Stream::Stdout => self.stdout_closed = true,
            Stream::Stderr => self.stderr_closed = true,
        }
    }

    fn finish(self, status: ExitStatus, stream_end: StreamEnd) -> Result<Finished, RunError> {
        match self.failure {
            Some(error) => Err(RunError::Io(error)),
            None => Ok(Finished { status, stdout: self.stdout, stderr: self.stderr, stream_end }),
        }
    }
}

fn start_readers(child: &mut Child) -> io::Result<Receiver<Event>> {
    let (sender, events) = mpsc::channel();
    forward(child.stdout.take(), Stream::Stdout, usize::MAX, sender.clone())?;
    forward(child.stderr.take(), Stream::Stderr, STDERR_LIMIT_BYTES, sender)?;
    Ok(events)
}

fn forward(pipe: Option<impl Read + Send + 'static>, stream: Stream, keep_bytes: usize, sender: Sender<Event>) -> io::Result<()> {
    std::thread::Builder::new().name(format!("child-{stream:?}")).spawn(move || match pipe {
        Some(pipe) => pump(pipe, stream, keep_bytes, &sender),
        None => {
            let _receiver_gone = sender.send(Event::Closed(stream));
        }
    })?;
    Ok(())
}

fn pump(mut pipe: impl Read, stream: Stream, keep_bytes: usize, sender: &Sender<Event>) {
    let mut buffer = [0_u8; READ_CHUNK_BYTES];
    let mut kept = 0_usize;
    loop {
        let event = match pipe.read(&mut buffer) {
            Ok(0) => Event::Closed(stream),
            Ok(count) => {
                let keep = count.min(keep_bytes - kept);
                kept += keep;
                if keep == 0 {
                    continue;
                }
                Event::Bytes(stream, buffer[..keep].to_vec())
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => Event::Failed(stream, error),
        };
        let last = !matches!(event, Event::Bytes(..));
        if sender.send(event).is_err() || last {
            return;
        }
    }
}

fn wait_for_exit(
    child: &mut Child,
    deadline: Instant,
    limit: Duration,
    events: &Receiver<Event>,
    output: &mut Output,
) -> Result<ExitStatus, RunError> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if Instant::now() >= deadline => {
                stop(child);
                return Err(RunError::TimedOut { limit });
            }
            Ok(None) => output.absorb(events, POLL),
            Err(error) => {
                stop(child);
                return Err(RunError::Io(error));
            }
        }
    }
}

fn stop(child: &mut Child) {
    kill_group(child.id());
    let _killed = child.kill();
    let _reaped = child.wait();
}

/// std has no `killpg`, and `unsafe_code` is denied, so the group kill runs
/// `/bin/kill` with a negative process id.
fn kill_group(group: u32) {
    let _signalled = Command::new(KILL)
        .args(["-KILL", "--", &format!("-{group}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::test_support::stub_executable;

    #[test]
    fn a_child_past_the_limit_times_out_well_under_two_seconds() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let stub = stub_executable(directory.path(), "wedged", "exec sleep 10");
        let started = Instant::now();
        let error = run_bounded(&mut Command::new(stub), CHILD_LIMIT).expect_err("a timeout");
        assert!(matches!(error, RunError::TimedOut { .. }), "{error:?}");
        assert!(started.elapsed() < Duration::from_millis(1500), "took {:?}", started.elapsed());
    }

    fn grandchild_stub(directory: &Path, name: &str, tail: &str) -> (PathBuf, PathBuf) {
        let pid_file = directory.join(format!("{name}.pid"));
        let body = format!("sleep 10 &\necho $! > '{}'\n{tail}", pid_file.display());
        (stub_executable(directory, name, &body), pid_file)
    }

    fn is_running(pid_file: &Path) -> bool {
        let pid = std::fs::read_to_string(pid_file).expect("the stub wrote its grandchild's pid");
        Command::new("/bin/kill")
            .args(["-0", pid.trim()])
            .stderr(Stdio::null())
            .status()
            .expect("kill runs")
            .success()
    }

    fn stops_within(limit: Duration, pid_file: &Path) -> bool {
        let deadline = Instant::now() + limit;
        while Instant::now() < deadline {
            if !is_running(pid_file) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        !is_running(pid_file)
    }

    #[test]
    fn a_child_whose_grandchild_holds_stdout_keeps_its_exit_status() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let (stub, pid_file) = grandchild_stub(directory.path(), "forks", "printf 'done'\nexit 4");
        let started = Instant::now();
        let finished = run_bounded(&mut Command::new(stub), CHILD_LIMIT).expect("a finished run");
        assert!(started.elapsed() < Duration::from_millis(1500), "took {:?}", started.elapsed());
        assert_eq!(finished.status.code(), Some(4));
        assert_eq!(finished.stdout, b"done");
        assert_eq!(finished.stream_end, StreamEnd::HeldOpen);
        assert!(stops_within(CHILD_LIMIT, &pid_file), "the grandchild outlived the call");
    }

    #[test]
    fn a_timeout_kills_the_whole_process_group() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let (stub, pid_file) = grandchild_stub(directory.path(), "wedged", "exec sleep 10");
        let error = run_bounded(&mut Command::new(stub), CHILD_LIMIT).expect_err("a timeout");
        assert!(matches!(error, RunError::TimedOut { .. }), "{error:?}");
        assert!(stops_within(CHILD_LIMIT, &pid_file), "the grandchild outlived the call");
    }

    #[test]
    fn stderr_is_captured_on_success_and_failure() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let fails = stub_executable(directory.path(), "fails", "printf 'no server running' >&2\nexit 1");
        let succeeds = stub_executable(directory.path(), "succeeds", "printf 'out'\nprintf 'warning' >&2");
        let failed = run_bounded(&mut Command::new(fails), CHILD_LIMIT).expect("a finished run");
        assert_eq!(failed.status.code(), Some(1));
        assert_eq!(failed.stderr, b"no server running");
        let finished = run_bounded(&mut Command::new(succeeds), CHILD_LIMIT).expect("a finished run");
        assert_eq!((finished.stdout.as_slice(), finished.stderr.as_slice()), (&b"out"[..], &b"warning"[..]));
        assert_eq!(finished.stream_end, StreamEnd::Closed);
    }

    #[test]
    fn stderr_is_capped_and_drained() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let stub = stub_executable(directory.path(), "noisy", "head -c 1000000 /dev/zero >&2");
        let finished = run_bounded(&mut Command::new(stub), Duration::from_secs(5)).expect("a finished run");
        assert!(finished.status.success());
        assert_eq!(finished.stderr.len(), STDERR_LIMIT_BYTES);
        assert_eq!(finished.stream_end, StreamEnd::Closed);
    }

    #[test]
    fn a_child_that_exits_in_time_reports_status_and_stdout() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let succeeds = stub_executable(directory.path(), "succeeds", "printf 'hello\\n'");
        let fails = stub_executable(directory.path(), "fails", "printf 'partial'\nexit 3");
        let finished = run_bounded(&mut Command::new(succeeds), CHILD_LIMIT).expect("a finished run");
        assert!(finished.status.success());
        assert_eq!(finished.stdout, b"hello\n");
        let failed = run_bounded(&mut Command::new(fails), CHILD_LIMIT).expect("a finished run");
        assert_eq!(failed.status.code(), Some(3));
        assert_eq!(failed.stdout, b"partial");
    }

    #[test]
    fn output_larger_than_the_pipe_buffer_does_not_deadlock() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let stub = stub_executable(directory.path(), "chatty", "head -c 1000000 /dev/zero");
        let finished = run_bounded(&mut Command::new(stub), Duration::from_secs(5)).expect("a finished run");
        assert!(finished.status.success());
        assert_eq!(finished.stdout.len(), 1_000_000);
    }

    #[test]
    fn a_missing_binary_is_an_error() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let error = run_bounded(&mut Command::new(directory.path().join("missing")), CHILD_LIMIT).expect_err("no binary");
        assert!(matches!(&error, RunError::Spawn(cause) if cause.kind() == io::ErrorKind::NotFound), "{error:?}");
    }
}
