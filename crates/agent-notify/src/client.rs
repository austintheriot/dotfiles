//! One request, one reply, inside a fixed time budget.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use agent_notify_core::wire::{Reply, ReplyCode};

/// Connect, write and reply together. A hook must not hold up the agent.
pub const BUDGET: Duration = Duration::from_millis(300);
const MAX_REPLY_BYTES: u64 = 64 * 1024;

/// What happened to one request.
#[derive(Debug)]
pub enum Delivery {
    /// The daemon accepted it.
    Acked {
        /// The daemon's build stamp.
        daemon: Option<String>,
    },
    /// The daemon refused it.
    Rejected {
        /// Why.
        code: Option<ReplyCode>,
    },
    /// No socket path on this machine.
    NoSocket(&'static str),
    /// The connect or write failed.
    ConnectFailed(io::ErrorKind),
    /// The budget ran out.
    TimedOut,
    /// The reply was missing or not a reply.
    BadReply(String),
}

impl Delivery {
    /// True only when the daemon accepted the request.
    pub fn acknowledged(&self) -> bool {
        matches!(self, Delivery::Acked { .. })
    }

    /// The build stamp of the daemon that acknowledged, for the log.
    pub fn daemon(&self) -> Option<&str> {
        match self {
            Delivery::Acked { daemon } => daemon.as_deref(),
            _ => None,
        }
    }

    /// A short phrase for the log, such as `rejected: unsupported_version`.
    pub fn reason(&self) -> String {
        match self {
            Delivery::Acked { .. } => "acked".to_string(),
            Delivery::Rejected { code: Some(code) } => format!("rejected: {}", reply_code_name(*code)),
            Delivery::Rejected { code: None } => "rejected".to_string(),
            Delivery::NoSocket(reason) => format!("no socket: {reason}"),
            Delivery::ConnectFailed(kind) => format!("connect failed: {kind:?}"),
            Delivery::TimedOut => "timed out".to_string(),
            Delivery::BadReply(text) => format!("bad reply: {text}"),
        }
    }
}

fn reply_code_name(code: ReplyCode) -> &'static str {
    match code {
        ReplyCode::BadRequest => "bad_request",
        ReplyCode::UnsupportedVersion => "unsupported_version",
        ReplyCode::UnknownKind => "unknown_kind",
        ReplyCode::LineTooLong => "line_too_long",
        ReplyCode::Unknown => "unknown",
    }
}

/// Sends `line` to the socket at `path` and reads one reply line. A daemon
/// that accepts after the deadline makes a duplicate; that is the accepted
/// failure, since a silent drop is worse.
pub fn deliver(path: &Path, line: &str, budget: Duration) -> Delivery {
    let started = Instant::now();
    let remaining = || budget.checked_sub(started.elapsed()).filter(|left| !left.is_zero());
    let mut stream = match UnixStream::connect(path) {
        Ok(stream) => stream,
        Err(error) => return Delivery::ConnectFailed(error.kind()),
    };
    let Some(left) = remaining() else { return Delivery::TimedOut };
    if let Err(error) = stream.set_write_timeout(Some(left)).and_then(|()| stream.write_all(line.as_bytes())) {
        return classify(&error);
    }
    let Some(left) = remaining() else { return Delivery::TimedOut };
    if let Err(error) = stream.set_read_timeout(Some(left)) {
        return classify(&error);
    }
    let mut reply = String::new();
    match BufReader::new((&stream).take(MAX_REPLY_BYTES)).read_line(&mut reply) {
        Ok(0) => Delivery::BadReply("the daemon closed the connection without a reply".to_string()),
        Ok(_) => match serde_json::from_str::<Reply>(&reply) {
            Ok(reply) if reply.ok => Delivery::Acked { daemon: reply.daemon },
            Ok(reply) => Delivery::Rejected { code: reply.code },
            Err(error) => Delivery::BadReply(error.to_string()),
        },
        Err(error) => classify(&error),
    }
}

fn classify(error: &io::Error) -> Delivery {
    match error.kind() {
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => Delivery::TimedOut,
        kind => Delivery::ConnectFailed(kind),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::os::unix::net::UnixListener;

    fn socket_dir() -> tempfile::TempDir {
        tempfile::Builder::new().prefix("an-client-").tempdir_in("/tmp").expect("a directory")
    }

    fn serve(path: &Path, answer: Option<&'static str>) {
        let listener = UnixListener::bind(path).expect("bind");
        std::thread::spawn(move || {
            let Ok((stream, _)) = listener.accept() else { return };
            let Some(answer) = answer else {
                std::thread::sleep(Duration::from_secs(2));
                return;
            };
            let mut line = String::new();
            if BufReader::new(&stream).read_line(&mut line).is_ok() {
                let _sent = (&stream).write_all(format!("{answer}\n").as_bytes());
            }
        });
    }

    #[test]
    fn an_ok_reply_is_an_acknowledgement() {
        let directory = socket_dir();
        let path = directory.path().join("s.sock");
        serve(&path, Some(r#"{"ok":true,"daemon":"stamp"}"#));
        assert!(matches!(deliver(&path, "{}\n", BUDGET), Delivery::Acked { daemon: Some(daemon) } if daemon == "stamp"));
    }

    #[test]
    fn only_an_acknowledgement_names_the_daemon() {
        assert_eq!(Delivery::Acked { daemon: Some("stamp".to_string()) }.daemon(), Some("stamp"));
        assert_eq!(Delivery::Acked { daemon: None }.daemon(), None);
        assert_eq!(Delivery::TimedOut.daemon(), None);
    }

    #[test]
    fn a_refusal_is_not_an_acknowledgement() {
        let directory = socket_dir();
        let path = directory.path().join("s.sock");
        serve(&path, Some(r#"{"ok":false,"code":"unsupported_version","accepts":[1,1]}"#));
        let delivery = deliver(&path, "{}\n", BUDGET);
        assert!(matches!(delivery, Delivery::Rejected { code: Some(ReplyCode::UnsupportedVersion) }));
        assert!(!delivery.acknowledged());
    }

    #[test]
    fn a_silent_daemon_times_out_within_the_budget() {
        let directory = socket_dir();
        let path = directory.path().join("s.sock");
        serve(&path, None);
        let started = Instant::now();
        assert!(matches!(deliver(&path, "{}\n", BUDGET), Delivery::TimedOut));
        assert!(started.elapsed() < BUDGET + Duration::from_millis(200), "took {:?}", started.elapsed());
    }

    #[test]
    fn a_missing_or_stale_socket_fails_to_connect() {
        let directory = socket_dir();
        let missing = directory.path().join("missing.sock");
        assert!(matches!(deliver(&missing, "{}\n", BUDGET), Delivery::ConnectFailed(_)));
        let stale = directory.path().join("stale.sock");
        std::fs::write(&stale, "").expect("a regular file");
        assert!(matches!(deliver(&stale, "{}\n", BUDGET), Delivery::ConnectFailed(_)));
    }

    #[test]
    fn a_non_json_reply_is_a_bad_reply() {
        let directory = socket_dir();
        let path = directory.path().join("s.sock");
        serve(&path, Some("hello"));
        assert!(matches!(deliver(&path, "{}\n", BUDGET), Delivery::BadReply(_)));
    }

    #[test]
    fn every_outcome_has_a_short_reason() {
        let cases = [
            (Delivery::Acked { daemon: Some("stamp".to_string()) }, "acked"),
            (Delivery::Acked { daemon: None }, "acked"),
            (Delivery::Rejected { code: Some(ReplyCode::UnsupportedVersion) }, "rejected: unsupported_version"),
            (Delivery::Rejected { code: Some(ReplyCode::BadRequest) }, "rejected: bad_request"),
            (Delivery::Rejected { code: Some(ReplyCode::UnknownKind) }, "rejected: unknown_kind"),
            (Delivery::Rejected { code: Some(ReplyCode::LineTooLong) }, "rejected: line_too_long"),
            (Delivery::Rejected { code: Some(ReplyCode::Unknown) }, "rejected: unknown"),
            (Delivery::Rejected { code: None }, "rejected"),
            (Delivery::NoSocket("HOME is unset or relative"), "no socket: HOME is unset or relative"),
            (Delivery::ConnectFailed(io::ErrorKind::NotFound), "connect failed: NotFound"),
            (Delivery::TimedOut, "timed out"),
            (Delivery::BadReply("expected value".to_string()), "bad reply: expected value"),
        ];
        for (delivery, reason) in cases {
            assert_eq!(delivery.reason(), reason, "{delivery:?}");
        }
    }
}
