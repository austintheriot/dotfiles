//! Child processes with a deadline. A wedged tool must not stall the actor,
//! so every spawn in this crate goes through [`run_bounded`].

use std::io::{self, Read};
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// The limit for every external tool the daemon runs.
pub const CHILD_LIMIT: Duration = Duration::from_secs(1);
const POLL: Duration = Duration::from_millis(5);
const READ_GRACE: Duration = Duration::from_millis(50);

/// A child that exited within its limit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finished {
    /// How it exited.
    pub status: ExitStatus,
    /// Everything it wrote to stdout.
    pub stdout: Vec<u8>,
}

/// Runs `command` with stdin and stderr closed and stdout collected, for at
/// most `limit`. Past the limit it kills and reaps the child.
///
/// # Errors
///
/// Returns the spawn or wait error, or an error of kind `TimedOut` when the
/// child or its stdout outlives the limit.
pub fn run_bounded(command: &mut Command, limit: Duration) -> io::Result<Finished> {
    let deadline = Instant::now() + limit;
    let mut child = command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
    let stdout = match drain(child.stdout.take()) {
        Ok(receiver) => receiver,
        Err(error) => {
            stop(&mut child);
            return Err(error);
        }
    };
    let status = wait_until(&mut child, deadline, limit)?;
    let remaining = deadline.saturating_duration_since(Instant::now()).max(READ_GRACE);
    match stdout.recv_timeout(remaining) {
        Ok(read) => read.map(|bytes| Finished { status, stdout: bytes }),
        Err(RecvTimeoutError::Timeout) => Err(timed_out(limit)),
        Err(RecvTimeoutError::Disconnected) => Err(io::Error::other("the stdout reader stopped")),
    }
}

fn drain(pipe: Option<ChildStdout>) -> io::Result<Receiver<io::Result<Vec<u8>>>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::Builder::new().name("child-stdout".to_string()).spawn(move || {
        let mut bytes = Vec::new();
        let read = match pipe {
            Some(mut pipe) => pipe.read_to_end(&mut bytes).map(|_| bytes),
            None => Ok(bytes),
        };
        let _receiver_gone = sender.send(read);
    })?;
    Ok(receiver)
}

fn wait_until(child: &mut Child, deadline: Instant, limit: Duration) -> io::Result<ExitStatus> {
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            stop(child);
            return Err(timed_out(limit));
        }
        std::thread::sleep(POLL);
    }
}

fn stop(child: &mut Child) {
    let _killed = child.kill();
    let _reaped = child.wait();
}

fn timed_out(limit: Duration) -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, format!("ran longer than {limit:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::stub_executable;

    #[test]
    fn a_child_past_the_limit_times_out_well_under_two_seconds() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let stub = stub_executable(directory.path(), "wedged", "exec sleep 10");
        let started = Instant::now();
        let error = run_bounded(&mut Command::new(stub), CHILD_LIMIT).expect_err("a timeout");
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_millis(1500), "took {:?}", started.elapsed());
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
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }
}
