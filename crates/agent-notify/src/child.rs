//! Waiting for a child process with a deadline. The hook must not hang on a
//! wedged notifier or tmux server.

use std::io;
use std::process::{Child, ExitStatus};
use std::time::{Duration, Instant};

const POLL: Duration = Duration::from_millis(5);

/// Waits up to `limit` for `child` to exit. Past the limit it kills and
/// reaps the child.
///
/// # Errors
///
/// Returns the wait error, or an error of kind `TimedOut` after the kill.
pub fn wait_bounded(child: &mut Child, limit: Duration) -> io::Result<ExitStatus> {
    let deadline = Instant::now() + limit;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            let _killed = child.kill();
            let _reaped = child.wait();
            return Err(io::Error::new(io::ErrorKind::TimedOut, format!("ran longer than {limit:?}")));
        }
        std::thread::sleep(POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::Instant;

    #[test]
    fn a_child_past_its_limit_is_killed_and_reaped() {
        let mut child = Command::new("sleep").arg("10").spawn().expect("sleep starts");
        let started = Instant::now();
        let error = wait_bounded(&mut child, Duration::from_millis(100)).expect_err("a timeout");
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(2), "took {:?}", started.elapsed());
        assert!(child.try_wait().expect("a reaped child").is_some(), "the child must be reaped");
    }

    #[test]
    fn a_child_that_exits_in_time_reports_its_status() {
        let mut success = Command::new("true").spawn().expect("true starts");
        assert!(wait_bounded(&mut success, Duration::from_secs(5)).expect("a status").success());
        let mut failure = Command::new("false").spawn().expect("false starts");
        assert!(!wait_bounded(&mut failure, Duration::from_secs(5)).expect("a status").success());
    }
}
