//! One request, one reply, inside a fixed time budget.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use agent_notify_core::wire::{MAX_REPLY_BYTES, Reply, ReplyCode};
use socket2::{Domain, SockAddr, Socket, Type};

/// What happened to one request.
#[derive(Debug)]
pub enum Delivery {
    /// The daemon accepted it.
    Acked {
        /// The daemon's build stamp.
        daemon: Option<String>,
        /// The answer to a query.
        data: Option<serde_json::Value>,
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
            Delivery::Acked { daemon, .. } => daemon.as_deref(),
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

/// The wire spelling of a refusal code, such as `unsupported_version`.
pub fn reply_code_name(code: ReplyCode) -> &'static str {
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
    let Some(left) = remaining() else { return Delivery::TimedOut };
    let mut stream = match connect(path, left) {
        Ok(stream) => stream,
        Err(error) => return classify(&error),
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
    let limit = u64::try_from(MAX_REPLY_BYTES).unwrap_or(u64::MAX);
    match BufReader::new((&stream).take(limit)).read_line(&mut reply) {
        Ok(0) => Delivery::BadReply("the daemon closed the connection without a reply".to_string()),
        Ok(_) => match serde_json::from_str::<Reply>(&reply) {
            Ok(reply) if reply.ok => Delivery::Acked { daemon: reply.daemon, data: reply.data },
            Ok(reply) => Delivery::Rejected { code: reply.code },
            Err(error) => Delivery::BadReply(error.to_string()),
        },
        Err(error) => classify(&error),
    }
}

/// A blocking Unix connect to a full accept queue waits on `SO_SNDTIMEO`,
/// which std leaves infinite. Linux reports the expiry as `EAGAIN`.
fn connect(path: &Path, timeout: Duration) -> io::Result<UnixStream> {
    let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
    socket.set_write_timeout(Some(timeout))?;
    socket.connect(&SockAddr::unix(path)?)?;
    Ok(UnixStream::from(OwnedFd::from(socket)))
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
    use crate::invocation::BUDGET;
    use agent_notify_core::status::{DaemonFacts, Decision, DndStatus, MAX_DECISIONS, SessionStatus, StatusReport, ToolStatus};
    use agent_notify_core::{EpochMs, EventId, SessionId};
    use socket2::{Domain, SockAddr, Socket, Type};
    use std::io::{BufRead, BufReader};
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;

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
        assert!(matches!(deliver(&path, "{}\n", BUDGET), Delivery::Acked { daemon: Some(daemon), .. } if daemon == "stamp"));
    }

    #[test]
    fn a_query_reply_carries_its_data() {
        let directory = socket_dir();
        let path = directory.path().join("s.sock");
        serve(&path, Some(r#"{"ok":true,"daemon":"stamp","data":{"answer":42}}"#));
        let delivery = deliver(&path, "{}\n", BUDGET);
        assert!(
            matches!(&delivery, Delivery::Acked { data: Some(data), .. } if data["answer"] == 42),
            "{delivery:?}"
        );
    }

    #[test]
    fn only_an_acknowledgement_names_the_daemon() {
        assert_eq!(Delivery::Acked { daemon: Some("stamp".to_string()), data: None }.daemon(), Some("stamp"));
        assert_eq!(Delivery::Acked { daemon: None, data: None }.daemon(), None);
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
    fn a_full_accept_queue_returns_within_the_budget() {
        let directory = socket_dir();
        let path = directory.path().join("s.sock");
        let address = SockAddr::unix(&path).expect("an address");
        let listener = Socket::new(Domain::UNIX, Type::STREAM, None).expect("a socket");
        listener.bind(&address).expect("bind");
        listener.listen(1).expect("listen");
        let queued: Vec<Socket> = (0..64)
            .map_while(|_| {
                let client = Socket::new(Domain::UNIX, Type::STREAM, None).ok()?;
                client.set_nonblocking(true).ok()?;
                client.connect(&address).ok().map(|()| client)
            })
            .collect();
        assert!(!queued.is_empty(), "the queue took no connection at all");
        let started = Instant::now();
        let delivery = deliver(&path, "{}\n", BUDGET);
        assert!(started.elapsed() < BUDGET + Duration::from_millis(300), "took {:?}", started.elapsed());
        if cfg!(target_os = "linux") {
            assert!(matches!(delivery, Delivery::TimedOut), "{delivery:?}");
        }
    }

    const REPORT_SESSIONS: usize = 40;

    fn busy_status_report() -> StatusReport {
        let tools = ["tmux", "aerospace", "afplay", "open", "ioreg"]
            .iter()
            .map(|name| ToolStatus { name: (*name).to_string(), path: Some(PathBuf::from(format!("/opt/homebrew/Cellar/{name}/10.20.30/bin/{name}"))) })
            .collect();
        let sessions = (0..REPORT_SESSIONS)
            .map(|number| SessionStatus {
                session: SessionId::parse(&format!("0f8e2c1a-5b7d-4e3f-9a6b-{number:012}")).expect("a session id"),
                pane: Some(format!("%{}", 1000 + number)),
                location: format!("{} · {}", 100 + number, "a-long-repository-name/feature-branch-with-a-long-descriptive-name"),
                turn: "needs_you".to_string(),
                quiet: "until next-prompt, at most 119 min 59 s more".to_string(),
                card: Some("attention showing".to_string()),
            })
            .collect();
        let decisions = (0..MAX_DECISIONS)
            .map(|number| Decision {
                at_ms: EpochMs(1_800_000_000_000),
                event_id: EventId::from_parts(EpochMs(1_800_000_000_000), [u8::try_from(number).expect("under 256"); 10]),
                kind: "focus_changed".to_string(),
                outcome: "suppressed: do_not_disturb".to_string(),
            })
            .collect();
        StatusReport {
            daemon: DaemonFacts { stamp: "a".repeat(64), started_ms: EpochMs(1_800_000_000_000), tools },
            now_ms: EpochMs(1_800_000_000_000),
            dnd: DndStatus { on: true, until_ms: Some(EpochMs(1_800_000_000_000)) },
            sessions,
            decisions,
        }
    }

    #[test]
    fn a_busy_status_report_fits_in_one_reply() {
        let report = busy_status_report();
        assert_eq!(report.decisions.len(), MAX_DECISIONS);
        let reply = Reply { data: Some(serde_json::to_value(&report).expect("a report encodes")), ..Reply::accepted(&report.daemon.stamp) };
        let line = format!("{}\n", serde_json::to_string(&reply).expect("a reply encodes"));
        let size = line.len();
        println!("a status reply with {REPORT_SESSIONS} sessions and {MAX_DECISIONS} decisions is {size} bytes, cap {MAX_REPLY_BYTES}");
        assert!(size < MAX_REPLY_BYTES, "{size} bytes");
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
            (Delivery::Acked { daemon: Some("stamp".to_string()), data: None }, "acked"),
            (Delivery::Acked { daemon: None, data: None }, "acked"),
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
