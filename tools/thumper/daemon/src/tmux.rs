//! The stateless tmux adapter. Every call passes `-u -S <socket>` and an
//! argv, never a shell string, and runs under the bounded child runner.
//!
//! A tmux server that restarts on the same socket path reuses pane ids from
//! `%0`, so every read prints `#{pid}` and `#{start_time}` on each line, and
//! a line from a different server makes the whole read `ServerGone`. A line
//! with no readable identity makes it `Failed`: tmux prints a newline in a
//! user field raw, which splits a line, and that must not end sessions. Writes
//! check identity first; the gap between the check and the write is one tmux
//! round trip.
//!
//! Fields are tab-separated: tmux 3.4 prints a tab raw but escapes other
//! control bytes. Each format holds at most one field a user can set, and it
//! is last, so a tab inside it stays in it. Sessions are named by
//! `#{session_id}`, which tmux never escapes, so the id is always a valid
//! `-t` target where a printed name is not.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use thumper_core::focus::{FocusWorld, TmuxSwitch, WorldClient, WorldPane};
use thumper_core::status::{PANE_STATE_OPTION, WINDOW_STATE_OPTION, pane_state_value, parse_pane_state, window_state};
use thumper_core::update::{PaneSnapshot, PaneState, PaneTag};
use thumper_core::{PaneId, PaneKey, ServerId, SessionId, SocketPath};

use crate::child::{CHILD_LIMIT, RunError, StreamEnd, run_bounded};

const SEPARATOR: char = '\t';
const IDENTITY: &str = "#{pid}\t#{start_time}";
const SESSION_TAG: &str = "#{@thumper_session}";

/// Why a tmux call did not do its job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TmuxError {
    /// The binary did not start.
    Spawn(String),
    /// No server on the socket, a different server than the one named, or
    /// the target pane or window is gone.
    ServerGone,
    /// tmux ran and refused, did not answer within the limit, or printed
    /// output the adapter cannot read. The text says which.
    Failed(String),
}

impl fmt::Display for TmuxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TmuxError::Spawn(reason) => write!(formatter, "tmux did not start: {reason}"),
            TmuxError::ServerGone => write!(formatter, "the tmux server is gone"),
            TmuxError::Failed(reason) => write!(formatter, "tmux failed: {reason}"),
        }
    }
}

impl std::error::Error for TmuxError {}

/// The panes of one window and their states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowPanes {
    /// `#{window_id}`, for example `@3`.
    pub window: String,
    /// Each pane's current `@thumper_state`.
    pub states: Vec<(PaneId, PaneState)>,
}

/// tmux, by absolute path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tmux {
    binary: PathBuf,
}

impl Tmux {
    /// Wraps the binary at `binary`.
    #[must_use]
    pub fn new(binary: PathBuf) -> Self {
        Tmux { binary }
    }

    /// The binary, for a terminal that attaches to a session.
    #[must_use]
    pub fn binary(&self) -> &Path {
        &self.binary
    }

    /// Reads the identity of the server on `socket`.
    ///
    /// # Errors
    ///
    /// `ServerGone` when nothing answers on the socket.
    pub fn server_id(&self, socket: &SocketPath) -> Result<ServerId, TmuxError> {
        let text = self.run(socket, &["list-sessions", "-F", IDENTITY])?;
        let first = text.lines().next().ok_or(TmuxError::ServerGone)?;
        identified_fields(first, socket, 3).map(|(identity, _)| identity)
    }

    /// Every pane with its tag and tmux session id, and every client.
    ///
    /// # Errors
    ///
    /// `ServerGone` when the server is gone or restarted.
    pub fn focus_world(&self, server: &ServerId) -> Result<FocusWorld, TmuxError> {
        let socket = &server.socket_path;
        let panes = self.run(socket, &["list-panes", "-a", "-F", &format!("{IDENTITY}\t#{{pane_id}}\t#{{session_id}}\t{SESSION_TAG}")])?;
        let clients = self.run(socket, &["list-clients", "-F", &format!("{IDENTITY}\t#{{client_tty}}\t#{{client_activity}}\t#{{session_id}}")])?;
        parse_world(&panes, &clients, server)
    }

    /// Every pane's session tag on whichever server answers on `socket`,
    /// named by the identity its lines carry. After a restart on the same
    /// socket the snapshot names the new server, so the core drops the
    /// records of the old one and keeps reading the new one.
    ///
    /// # Errors
    ///
    /// `ServerGone` when nothing answers on the socket: every session on it
    /// ended. `Spawn` or `Failed` must not end any session.
    pub fn pane_snapshot(&self, socket: &SocketPath) -> Result<PaneSnapshot, TmuxError> {
        let text = self.run(socket, &["list-panes", "-a", "-F", &format!("{IDENTITY}\t#{{pane_id}}\t{SESSION_TAG}")])?;
        parse_pane_snapshot(&text, socket)
    }

    /// The current pane of the most recently active client. A tie in
    /// activity goes to the lowest tty, as in the core's `plan_focus`.
    ///
    /// # Errors
    ///
    /// `ServerGone` when the server is gone or restarted.
    pub fn active_pane(&self, server: &ServerId) -> Result<Option<PaneKey>, TmuxError> {
        let text = self.run(&server.socket_path, &["list-clients", "-F", &format!("{IDENTITY}\t#{{client_activity}}\t#{{client_tty}}\t#{{pane_id}}")])?;
        parse_active_pane(&text, server)
    }

    /// The session tag on `pane`. `None` when the pane is untagged or gone:
    /// tmux 3.4 answers a missing `%pane` target with exit 0 and an empty
    /// pane id, and the identity on that line says the server is the same.
    ///
    /// # Errors
    ///
    /// `ServerGone` when the server is gone or restarted; `Failed` when tmux
    /// printed no line.
    pub fn session_tag(&self, pane: &PaneKey) -> Result<Option<SessionId>, TmuxError> {
        let format = format!("{IDENTITY}\t#{{pane_id}}\t{SESSION_TAG}");
        let text = self.run(&pane.server.socket_path, &["display-message", "-p", "-t", pane.pane.as_str(), &format])?;
        if text.lines().next().is_none() {
            return Err(TmuxError::Failed("tmux printed nothing for the pane".to_string()));
        }
        match parse_pane_tags(&text, &pane.server)?.into_iter().next() {
            Some((found, tag)) if found == pane.pane => Ok(tag),
            _ => Ok(None),
        }
    }

    /// Makes `pane` current in its window and session, and moves a client
    /// to it as `switch` says. `AttachNewTerminal` only selects: the caller
    /// starts the terminal.
    ///
    /// No identity check: `server` gives only the socket. The caller must
    /// check identity just before, as `run_focus` does with [`Tmux::session_tag`].
    ///
    /// # Errors
    ///
    /// The first tmux error.
    pub fn switch(&self, server: &ServerId, pane: &PaneId, switch: &TmuxSwitch) -> Result<(), TmuxError> {
        let target = pane.as_str();
        let socket = &server.socket_path;
        self.run(socket, &["select-window", "-t", target, ";", "select-pane", "-t", target])?;
        if let TmuxSwitch::SwitchClient { tty } = switch {
            self.run(socket, &["switch-client", "-c", tty, "-t", target])?;
        }
        Ok(())
    }

    /// Writes `state` on the pane and the most urgent state of its window
    /// on the window. Idle unsets the option.
    ///
    /// # Errors
    ///
    /// `ServerGone` when the server is gone or restarted; nothing is written then.
    pub fn set_pane_state(&self, pane: &PaneKey, state: PaneState) -> Result<(), TmuxError> {
        let socket = &pane.server.socket_path;
        let target = pane.pane.as_str();
        let format = format!("{IDENTITY}\t#{{window_id}}\t#{{pane_id}}\t#{{{PANE_STATE_OPTION}}}");
        let window = parse_window_panes(&self.run(socket, &["list-panes", "-t", target, "-F", &format])?, &pane.server)?;
        let merged = window_state(window.states.iter().map(|(id, current)| if *id == pane.pane { state } else { *current }));
        let arguments: Vec<String> = option_arguments("-p", target, PANE_STATE_OPTION, state)
            .into_iter()
            .chain([";".to_string()])
            .chain(option_arguments("-w", &window.window, WINDOW_STATE_OPTION, merged))
            .collect();
        let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
        self.run(socket, &borrowed).map(|_| ())
    }

    /// `-u`: without a UTF-8 locale, which launchd does not set, tmux prints
    /// a tab and non-ASCII bytes as `_`.
    fn command(&self, socket: &SocketPath, arguments: &[&str]) -> Command {
        let mut command = Command::new(&self.binary);
        command.arg("-u").arg("-S").arg(socket.as_path()).args(arguments);
        command
    }

    fn run(&self, socket: &SocketPath, arguments: &[&str]) -> Result<String, TmuxError> {
        let finished = run_bounded(&mut self.command(socket, arguments), CHILD_LIMIT).map_err(|error| match error {
            RunError::Spawn(cause) => TmuxError::Spawn(cause.to_string()),
            RunError::TimedOut { .. } => timed_out(),
            RunError::Io(cause) => TmuxError::Failed(cause.to_string()),
        })?;
        if finished.stream_end == StreamEnd::HeldOpen {
            return Err(TmuxError::Failed("tmux held its output open".to_string()));
        }
        if finished.status.success() {
            return Ok(String::from_utf8_lossy(&finished.stdout).into_owned());
        }
        let stderr = String::from_utf8_lossy(&finished.stderr).trim().to_string();
        if is_server_gone(&stderr, arguments) { Err(TmuxError::ServerGone) } else { Err(TmuxError::Failed(stderr)) }
    }
}

fn timed_out() -> TmuxError {
    TmuxError::Failed(format!("tmux did not answer in {} s", CHILD_LIMIT.as_secs()))
}

fn option_arguments(scope: &str, target: &str, option: &str, state: PaneState) -> Vec<String> {
    match pane_state_value(state) {
        Some(value) => vec!["set-option".into(), scope.into(), "-t".into(), target.into(), option.into(), value.into()],
        None => vec!["set-option".into(), scope.into(), "-u".into(), "-t".into(), target.into(), option.into()],
    }
}

/// Only these connect errors mean nothing answers on the socket. Any other
/// errno, such as a permission or sandbox refusal, can hide a live server.
const GONE_CONNECT_ERRORS: [&str; 2] = ["(No such file or directory)", "(Connection refused)"];
const MISSING_TARGET_PREFIXES: [&str; 3] = ["can't find pane: ", "can't find window: ", "can't find session: "];

/// Gone is a fact tmux states: no server, or a missing target that this
/// call named with `-t`. Everything else is `Failed`, which ends no session.
fn is_server_gone(stderr: &str, arguments: &[&str]) -> bool {
    let targets: Vec<&str> = arguments.windows(2).filter(|pair| pair[0] == "-t").map(|pair| pair[1]).collect();
    stderr.lines().any(|line| {
        line.starts_with("no server running on ")
            || (line.starts_with("error connecting to ") && GONE_CONNECT_ERRORS.iter().any(|errno| line.ends_with(errno)))
            || MISSING_TARGET_PREFIXES.iter().any(|prefix| line.strip_prefix(prefix).is_some_and(|target| targets.contains(&target)))
    })
}

/// Splits `line` into at most `count` fields (three or more) and parses the
/// identity in the first two. An ordinary line split by a raw newline in a
/// user field has no identity, which is unreadable output, not a different
/// server. A crafted value can still forge one: `set-option` with a newline
/// then a pid, a start time and a pane id adds a line with any identity,
/// which reads as `ServerGone` or as a fake pane. That risk is accepted,
/// because only a process that can reach the socket can set the value, and
/// such a process can already run `kill-server`.
fn identified_fields<'line>(line: &'line str, socket: &SocketPath, count: usize) -> Result<(ServerId, Vec<&'line str>), TmuxError> {
    let fields: Vec<&str> = line.splitn(count.max(3), SEPARATOR).collect();
    let pid = fields.first().and_then(|value| value.parse().ok());
    let start_time = fields.get(1).and_then(|value| value.parse().ok());
    match (pid, start_time) {
        (Some(pid), Some(start_time)) => Ok((ServerId { socket_path: socket.clone(), pid, start_time }, fields)),
        _ => Err(TmuxError::Failed(format!("unreadable tmux line {line:?}"))),
    }
}

fn checked_fields<'line>(line: &'line str, server: &ServerId, count: usize) -> Result<Vec<&'line str>, TmuxError> {
    let (found, fields) = identified_fields(line, &server.socket_path, count)?;
    if found != *server {
        return Err(TmuxError::ServerGone);
    }
    if fields.len() < count {
        return Err(TmuxError::Failed(format!("short tmux line {line:?}")));
    }
    Ok(fields)
}

fn tag_of(text: &str) -> Option<SessionId> {
    SessionId::parse(text).ok()
}

/// Parses the `focus_world` reads. Pane lines are identity, pane id,
/// session id and tag; client lines are identity, tty, activity and session id.
///
/// # Errors
///
/// `ServerGone` when any line comes from a different server; `Failed` when
/// a line has no readable identity.
pub fn parse_world(panes: &str, clients: &str, server: &ServerId) -> Result<FocusWorld, TmuxError> {
    let panes = panes
        .lines()
        .map(|line| {
            let fields = checked_fields(line, server, 5)?;
            Ok(PaneId::parse(fields[2]).ok().map(|pane| WorldPane { pane, tmux_session: fields[3].to_string(), session_tag: tag_of(fields[4]) }))
        })
        .collect::<Result<Vec<_>, TmuxError>>()?
        .into_iter()
        .flatten()
        .collect();
    let clients = clients
        .lines()
        .map(|line| {
            let fields = checked_fields(line, server, 5)?;
            Ok(WorldClient { tty: fields[2].to_string(), activity: fields[3].parse().unwrap_or(0), tmux_session: fields[4].to_string() })
        })
        .collect::<Result<Vec<_>, TmuxError>>()?;
    Ok(FocusWorld { panes, clients })
}

/// Parses identity, pane id and session tag lines. Lines with a bad pane id are skipped.
///
/// # Errors
///
/// `ServerGone` when any line comes from a different server; `Failed` when
/// a line has no readable identity.
pub fn parse_pane_tags(text: &str, server: &ServerId) -> Result<Vec<(PaneId, Option<SessionId>)>, TmuxError> {
    let rows = text
        .lines()
        .map(|line| {
            let fields = checked_fields(line, server, 4)?;
            Ok(PaneId::parse(fields[2]).ok().map(|pane| (pane, tag_of(fields[3]))))
        })
        .collect::<Result<Vec<_>, TmuxError>>()?;
    Ok(rows.into_iter().flatten().collect())
}

/// Parses a `pane_snapshot` read. The first line names the server, and
/// every other line must carry the same identity.
///
/// # Errors
///
/// `ServerGone` for no lines or lines from two servers; `Failed` for an
/// unreadable identity.
pub fn parse_pane_snapshot(text: &str, socket: &SocketPath) -> Result<PaneSnapshot, TmuxError> {
    let first = text.lines().next().ok_or(TmuxError::ServerGone)?;
    let (server, _) = identified_fields(first, socket, 3)?;
    let panes = parse_pane_tags(text, &server)?.into_iter().map(|(pane, session)| PaneTag { pane, session }).collect();
    Ok(PaneSnapshot { server, panes })
}

/// Parses `list-clients` lines of identity, activity, tty and pane id into
/// the pane of the most recent client. A tie goes to the lowest tty.
///
/// # Errors
///
/// `ServerGone` when any line comes from a different server; `Failed` when
/// a line has no readable identity.
pub fn parse_active_pane(text: &str, server: &ServerId) -> Result<Option<PaneKey>, TmuxError> {
    let rows = text
        .lines()
        .map(|line| {
            let fields = checked_fields(line, server, 5)?;
            Ok((fields[2].parse::<i64>().unwrap_or(0), fields[3], PaneId::parse(fields[4]).ok()))
        })
        .collect::<Result<Vec<_>, TmuxError>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|(activity, tty, pane)| pane.map(|pane| (activity, tty, pane)))
        .max_by_key(|(activity, tty, _)| (*activity, std::cmp::Reverse(*tty)))
        .map(|(_, _, pane)| PaneKey { server: server.clone(), pane }))
}

/// Parses the panes of one window with their states.
///
/// # Errors
///
/// `ServerGone` when any line comes from a different server or the window is empty.
pub fn parse_window_panes(text: &str, server: &ServerId) -> Result<WindowPanes, TmuxError> {
    let rows = text
        .lines()
        .map(|line| {
            let fields = checked_fields(line, server, 5)?;
            Ok((fields[2].to_string(), PaneId::parse(fields[3]).ok().map(|pane| (pane, parse_pane_state(fields[4])))))
        })
        .collect::<Result<Vec<_>, TmuxError>>()?;
    let window = rows.first().map(|(window, _)| window.clone()).ok_or(TmuxError::ServerGone)?;
    Ok(WindowPanes { window, states: rows.into_iter().filter_map(|(_, state)| state).collect() })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use thumper_core::{PaneId, ServerId, SessionId, SocketPath};
    use dotfiles_test_support::tmux_server::Server;

    use super::*;
    use crate::child::{StreamEnd, run_bounded};
    use crate::test_support::stub_executable;

    /// These tests check how a run ends, not how fast it is. One spawn from
    /// a test binary measured 0.6 s to 0.75 s on a development host, which
    /// left the 1 s production limit no margin.
    const TEST_RUN_LIMIT: std::time::Duration = std::time::Duration::from_secs(5);

    fn socket() -> SocketPath {
        SocketPath::parse(Path::new("/tmp/tmux-501/default")).expect("absolute")
    }

    fn server() -> ServerId {
        ServerId { socket_path: socket(), pid: 77, start_time: 1_700_000_000 }
    }

    fn line(fields: &[&str]) -> String {
        format!("{}\n", fields.join("\t"))
    }

    fn pane(id: &str) -> PaneId {
        PaneId::parse(id).expect("a pane id")
    }

    #[test]
    fn the_world_lists_tagged_panes_and_clients() {
        let panes = [line(&["77", "1700000000", "%1", "$0", "session-a"]), line(&["77", "1700000000", "%2", "$3", ""])].concat();
        let clients = line(&["77", "1700000000", "/dev/ttys004", "1700000500", "$0"]);
        let world = parse_world(&panes, &clients, &server()).expect("same server");
        assert_eq!(world.panes.len(), 2);
        assert_eq!(world.panes[0].session_tag, Some(SessionId::parse("session-a").expect("id")));
        assert_eq!(world.panes[1].session_tag, None);
        assert_eq!(world.panes[1].tmux_session, "$3");
        assert_eq!(world.clients[0].tty, "/dev/ttys004");
        assert_eq!(world.clients[0].activity, 1_700_000_500);
        assert_eq!(world.clients[0].tmux_session, "$0");
    }

    #[test]
    fn a_line_from_another_server_is_server_gone() {
        let panes = line(&["78", "1700000000", "%1", "$0", "session-a"]);
        assert!(matches!(parse_world(&panes, "", &server()), Err(TmuxError::ServerGone)));
        let restarted = line(&["77", "1700000999", "%1", "session-a"]);
        assert!(matches!(parse_pane_tags(&restarted, &server()), Err(TmuxError::ServerGone)));
        let clients = [line(&["77", "1700000000", "100", "/dev/ttys001", "%1"]), line(&["77", "1700000999", "300", "/dev/ttys002", "%4"])].concat();
        assert!(matches!(parse_active_pane(&clients, &server()), Err(TmuxError::ServerGone)));
    }

    #[test]
    fn a_different_valid_identity_mid_read_is_server_gone() {
        let panes = [line(&["77", "1700000000", "%1", "$0", "session-a"]), line(&["77", "1700000999", "%2", "$0", ""])].concat();
        assert!(matches!(parse_world(&panes, "", &server()), Err(TmuxError::ServerGone)));
    }

    #[test]
    fn an_unreadable_identity_on_any_line_is_failed() {
        let split_by_newline = format!("{}b\tc\n", line(&["77", "1700000000", "%1", "$0", "a"]));
        assert!(matches!(parse_world(&split_by_newline, "", &server()), Err(TmuxError::Failed(_))), "mid-read garbage");
        let tags = format!("{}b\n", line(&["77", "1700000000", "%1", "a"]));
        assert!(matches!(parse_pane_tags(&tags, &server()), Err(TmuxError::Failed(_))), "mid-read garbage");
        let snapshot = format!("{}b\tc\n", line(&["91", "1700000123", "%0", "a"]));
        assert!(matches!(parse_pane_snapshot(&snapshot, &socket()), Err(TmuxError::Failed(_))), "mid-read garbage");
        assert!(matches!(parse_world("garbage\n", "", &server()), Err(TmuxError::Failed(_))), "first-line garbage");
        assert!(matches!(parse_pane_tags(&line(&["77", "x", "%1", ""]), &server()), Err(TmuxError::Failed(_))), "first-line garbage");
        let windows = format!("{}b\n", line(&["77", "1700000000", "@3", "%1", "working"]));
        assert!(matches!(parse_window_panes(&windows, &server()), Err(TmuxError::Failed(_))), "mid-read garbage");
    }

    #[test]
    fn the_last_field_may_hold_the_separator() {
        let panes = line(&["77", "1700000000", "%1", "$0", "odd\tvalue"]);
        let world = parse_world(&panes, "", &server()).expect("parses");
        assert_eq!(world.panes[0].pane, pane("%1"));
        assert_eq!(world.panes[0].tmux_session, "$0");
        assert_eq!(world.panes[0].session_tag, None, "a tag with a tab is not a session id");
        let states = parse_window_panes(&line(&["77", "1700000000", "@3", "%1", "working\tmore"]), &server()).expect("parses");
        assert_eq!(states.states, vec![(pane("%1"), PaneState::Idle)], "the whole last field is the value");
    }

    #[test]
    fn pane_tags_skip_bad_ids() {
        let tags = parse_pane_tags(&[line(&["77", "1700000000", "%1", "session-a"]), line(&["77", "1700000000", "%2", "not an id!"])].concat(), &server())
            .expect("same server");
        assert_eq!(tags, vec![(pane("%1"), Some(SessionId::parse("session-a").expect("id"))), (pane("%2"), None)]);
    }

    #[test]
    fn a_snapshot_names_the_server_its_lines_came_from() {
        let text = [line(&["91", "1700000123", "%0", "session-a"]), line(&["91", "1700000123", "%4", ""])].concat();
        let snapshot = parse_pane_snapshot(&text, &socket()).expect("one server");
        assert_eq!(snapshot.server, ServerId { socket_path: socket(), pid: 91, start_time: 1_700_000_123 });
        assert_eq!(snapshot.panes.len(), 2);
        assert_eq!(snapshot.panes[0].session, Some(SessionId::parse("session-a").expect("id")));
        assert_eq!(snapshot.panes[1].session, None);
    }

    #[test]
    fn a_snapshot_with_two_identities_or_none_is_server_gone() {
        let mixed = [line(&["91", "1700000123", "%0", "session-a"]), line(&["92", "1700000123", "%4", ""])].concat();
        assert!(matches!(parse_pane_snapshot(&mixed, &socket()), Err(TmuxError::ServerGone)));
        assert!(matches!(parse_pane_snapshot("", &socket()), Err(TmuxError::ServerGone)));
        assert!(matches!(parse_pane_snapshot(&line(&["x", "1", "%0", ""]), &socket()), Err(TmuxError::Failed(_))));
    }

    #[test]
    fn the_active_pane_is_the_most_recent_clients() {
        let clients = [line(&["77", "1700000000", "100", "/dev/ttys001", "%1"]), line(&["77", "1700000000", "300", "/dev/ttys002", "%4"])].concat();
        let active = parse_active_pane(&clients, &server()).expect("same server");
        assert_eq!(active.map(|key| key.pane), Some(pane("%4")));
        assert_eq!(parse_active_pane("", &server()).expect("no clients"), None);
    }

    #[test]
    fn a_tie_in_client_activity_picks_the_lowest_tty_in_any_order() {
        let first = line(&["77", "1700000000", "300", "/dev/ttys001", "%1"]);
        let second = line(&["77", "1700000000", "300", "/dev/ttys002", "%4"]);
        for clients in [[first.clone(), second.clone()].concat(), [second, first].concat()] {
            let active = parse_active_pane(&clients, &server()).expect("same server");
            assert_eq!(active.map(|key| key.pane), Some(pane("%1")), "{clients:?}");
        }
    }

    #[test]
    fn window_panes_carry_their_state() {
        let panes = [line(&["77", "1700000000", "@3", "%1", "working"]), line(&["77", "1700000000", "@3", "%2", ""])].concat();
        let window = parse_window_panes(&panes, &server()).expect("same server");
        assert_eq!(window.window, "@3");
        assert_eq!(window.states, vec![(pane("%1"), PaneState::Working), (pane("%2"), PaneState::Idle)]);
    }

    #[test]
    fn a_short_line_from_the_right_server_is_failed() {
        assert!(matches!(parse_pane_tags(&line(&["77", "1700000000", "%1"]), &server()), Err(TmuxError::Failed(_))));
    }

    #[test]
    fn server_gone_comes_only_from_tmux_saying_so() {
        let asked: &[&str] = &["set-option", "-p", "-t", "%9", "@x", "y", ";", "set-option", "-w", "-t", "@3", "@y", "z"];
        let rows: [(&str, bool); 15] = [
            ("no server running on /tmp/tmux-501/default", true),
            ("error connecting to /tmp/tmux-501/default (No such file or directory)", true),
            ("error connecting to /tmp/tmux-501/default (Connection refused)", true),
            ("can't find pane: %9", true),
            ("can't find window: @3", true),
            ("error connecting to /tmp/tmux-501/default (Permission denied)", false),
            ("error connecting to /tmp/tmux-501/default (Operation not permitted)", false),
            ("error connecting to /tmp/tmux-501/default (File name too long)", false),
            ("error connecting to /tmp/x (No such file or directory) and more", false),
            ("unknown option: @x No such file or directory", false),
            ("can't find pane: %10", false),
            ("can't find window: @4", false),
            ("can't find client: /dev/ttys001", false),
            ("unknown option: @x", false),
            ("", false),
        ];
        for (stderr, gone) in rows {
            assert_eq!(is_server_gone(stderr, asked), gone, "{stderr:?}");
        }
        assert!(is_server_gone("can't find session: $2", &["display-message", "-t", "$2"]));
        assert_eq!(timed_out(), TmuxError::Failed("tmux did not answer in 1 s".to_string()));
    }

    fn tmux_on_path() -> PathBuf {
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|directory| directory.join("tmux"))
            .find(|candidate| candidate.is_file())
            .expect("tmux on PATH")
    }

    fn started(label: &str) -> (Server, SocketPath) {
        let server = Server::new(label);
        server.new_session("main", Path::new("/tmp"));
        let socket = SocketPath::parse(&server.socket_path()).expect("absolute");
        (server, socket)
    }

    #[test]
    fn a_c_locale_reads_the_identity_and_non_ascii_text_unchanged() {
        let (server, socket) = started("an-locale");
        let pane_id = server.stdout(&["display-message", "-p", "-t", "main", "#{pane_id}"]);
        assert!(server.tmux(&["set-option", "-p", "-t", &pane_id, "@thumper_note", "café"]).status.success());
        let directory = tempfile::tempdir().expect("a temporary directory");
        let body = format!("exec /usr/bin/env -u LANG -u LC_CTYPE -u LC_MESSAGES -u TMUX LC_ALL=C '{}' \"$@\"", tmux_on_path().display());
        let tmux = Tmux::new(stub_executable(directory.path(), "tmux-c-locale", &body));
        let identity = tmux.server_id(&socket).expect("the tab separator survives a C locale");
        assert_eq!(identity.pid, server.pid());
        let note = tmux.run(&socket, &["display-message", "-p", "-t", &pane_id, "#{@thumper_note}"]).expect("reads");
        assert_eq!(note, "café\n");
    }

    #[test]
    fn tmux_client_calls_close_their_streams() {
        let (server, socket) = started("an-streams");
        let pane_id = server.stdout(&["display-message", "-p", "-t", "main", "#{pane_id}"]);
        let tmux = Tmux::new(tmux_on_path());
        let calls: [&[&str]; 4] = [
            &["list-panes", "-a", "-F", "#{pane_id}"],
            &["display-message", "-p", "-t", &pane_id, "#{pid}"],
            &["set-option", "-p", "-t", &pane_id, "@thumper_state", "working"],
            &["select-window", "-t", &pane_id, ";", "select-pane", "-t", &pane_id],
        ];
        for arguments in calls {
            let finished = run_bounded(&mut tmux.command(&socket, arguments), TEST_RUN_LIMIT).expect("a finished run");
            assert!(finished.status.success(), "{arguments:?}: {finished:?}");
            assert_eq!(finished.stream_end, StreamEnd::Closed, "{arguments:?}");
        }
    }

    #[test]
    fn a_server_started_through_the_runner_leaves_its_group_and_closes_the_streams() {
        let server = Server::new("an-spawn");
        let mut command = Command::new(tmux_on_path());
        command.env("TMUX_TMPDIR", server.socket_dir()).args(["-u", "-L", server.socket(), "-f", "/dev/null", "new-session", "-d", "-s", "main", "sleep", "600"]);
        let finished = run_bounded(&mut command, TEST_RUN_LIMIT).expect("a finished run");
        assert!(finished.status.success(), "{finished:?}");
        assert_eq!(finished.stream_end, StreamEnd::Closed);
        assert!(server.tmux(&["has-session"]).status.success(), "the server outlived the client's process group");
    }
}
