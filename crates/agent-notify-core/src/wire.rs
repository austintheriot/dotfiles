//! The request and reply that cross the Unix socket, one JSON object per line.
//!
//! Evolution: an added optional field keeps `v`. An unknown variant decodes to
//! `Unknown` and the daemon replies `unknown_kind`. A removed or renamed field
//! or variant, or a changed meaning, increments `v`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{DisplayText, EpochMs, EventId, PaneKey, PromptId, SessionId};

/// The version this build writes.
pub const WIRE_VERSION: u16 = 1;
/// The oldest version this build accepts.
pub const OLDEST_ACCEPTED_VERSION: u16 = 1;
/// The longest request line the daemon reads, in bytes.
pub const MAX_LINE_BYTES: usize = 65_536;

/// One request from the CLI to the daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
// One request lives for one line. Boxing `Origin` would change every call site for no gain.
#[allow(clippy::large_enum_variant)]
pub enum Request {
    /// Something an agent did.
    Agent {
        /// Wire version.
        v: u16,
        /// Names this invocation in every log line.
        event_id: EventId,
        /// When the CLI built the request.
        sent_at_ms: EpochMs,
        /// Where the event came from.
        origin: Origin,
        /// What happened.
        event: AgentEvent,
    },
    /// Something the user or tmux did.
    Control {
        /// Wire version.
        v: u16,
        /// Names this invocation in every log line.
        event_id: EventId,
        /// When the CLI built the request.
        sent_at_ms: EpochMs,
        /// What to change.
        control: Control,
    },
    /// A read of daemon state.
    Query {
        /// Wire version.
        v: u16,
        /// Names this invocation in every log line.
        event_id: EventId,
        /// What to read.
        query: Query,
    },
    /// A request type from a newer CLI.
    #[serde(other)]
    Unknown,
}

/// Where an agent event came from. The CLI fills it from the hook payload,
/// the pane tags, and one tmux read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    /// Which agent.
    pub agent: AgentKind,
    /// The Claude session. `None` only for `send` with no session tag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionId>,
    /// The turn, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<PromptId>,
    /// The tmux pane, when inside tmux.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane: Option<PaneKey>,
    /// The tmux window, when inside tmux.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowLabel>,
    /// The git repository, read only outside tmux.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<RepoInfo>,
    /// The latest session title known to the sender.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_title: Option<DisplayText>,
    /// The agent's working directory.
    pub cwd: PathBuf,
}

/// The agent that produced an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    /// Claude Code.
    ClaudeCode,
    /// An agent from a newer CLI.
    #[serde(other)]
    Unknown,
}

/// A tmux window's index and name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowLabel {
    /// `#{window_index}`.
    pub index: u32,
    /// `#{window_name}`.
    pub name: DisplayText,
}

/// Git facts for a working directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoInfo {
    /// The main repository's name, also inside a linked worktree.
    pub main_repo_name: DisplayText,
    /// Branch or detached commit.
    pub head: RepoHead,
}

/// The state of HEAD.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RepoHead {
    /// On a branch.
    Branch {
        /// The branch name.
        name: DisplayText,
    },
    /// Detached.
    Detached {
        /// The short commit sha.
        short_sha: DisplayText,
    },
}

/// What an agent did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// A session started, resumed, cleared, compacted or forked.
    SessionStarted {
        /// Why it started.
        source: SessionSource,
    },
    /// The user submitted a prompt.
    TurnStarted,
    /// The agent finished its turn.
    TurnEnded {
        /// The agent's last message.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        last_message: Option<DisplayText>,
    },
    /// The turn stopped on an API error.
    TurnFailed {
        /// The rendered error.
        error: DisplayText,
    },
    /// The agent waits for the user.
    NeedsInput {
        /// Why it waits.
        reason: InputReason,
        /// The prompt text.
        message: DisplayText,
    },
    /// An elicitation was answered.
    InputResolved,
    /// The agent called `agent-notify send`.
    Explicit {
        /// Info or attention.
        kind: Kind,
        /// The agent's text.
        body: DisplayText,
    },
    /// The agent silenced the `Stop` backup.
    Quiet {
        /// When quiet ends.
        until: QuietUntil,
        /// The latest end, in minutes.
        max_minutes: u16,
    },
    /// The agent ended quiet.
    QuietOff,
    /// The session ended.
    SessionEnded,
    /// An event from a newer CLI.
    #[serde(other)]
    Unknown,
}

/// The two card kinds. `Attention` outranks `Info`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Finished work. Fades after a few seconds.
    Info,
    /// The user must act. Stays until handled.
    Attention,
}

/// Why a session started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionSource {
    /// A new session.
    Startup,
    /// `--resume`.
    Resume,
    /// `/clear`.
    Clear,
    /// After compaction.
    Compact,
    /// A fork.
    Fork,
    /// A source from a newer Claude Code.
    #[serde(other)]
    Unknown,
}

/// Why an agent waits for the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputReason {
    /// A permission prompt.
    Permission,
    /// A question to the user.
    Elicitation,
    /// A reason from a newer CLI.
    #[serde(other)]
    Unknown,
}

/// When quiet ends on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuietUntil {
    /// At the session's next `send`.
    NextSend,
    /// At the session's next user prompt.
    NextPrompt,
}

/// A change requested by the user or by tmux.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Control {
    /// A tmux pane gained or lost focus.
    FocusChanged {
        /// The pane.
        pane: PaneKey,
        /// True on focus-in.
        focused: bool,
    },
    /// Do-not-disturb.
    Dnd {
        /// On or off.
        command: DndCommand,
    },
    /// A control from a newer CLI.
    #[serde(other)]
    Unknown,
}

/// A do-not-disturb change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DndCommand {
    /// Turn on, until `dnd off` or for a number of minutes.
    On {
        /// Optional end, in minutes from now.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        for_minutes: Option<u16>,
    },
    /// Turn off.
    Off,
}

/// A read of daemon state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Query {
    /// The `status` report.
    Status,
    /// The panel view model.
    ViewModel,
    /// A query from a newer CLI.
    #[serde(other)]
    Unknown,
}

/// The daemon's answer to one request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reply {
    /// True when the daemon accepted the request.
    pub ok: bool,
    /// The daemon's build stamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daemon: Option<String>,
    /// Why the daemon refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<ReplyCode>,
    /// The accepted version range, on refusal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepts: Option<[u16; 2]>,
    /// The answer to a query.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl Reply {
    /// An acknowledgement from a daemon with build stamp `daemon`.
    #[must_use]
    pub fn accepted(daemon: &str) -> Self {
        Reply { ok: true, daemon: Some(daemon.to_string()), code: None, accepts: None, data: None }
    }

    /// A refusal, with the accepted version range.
    #[must_use]
    pub fn rejected(code: ReplyCode) -> Self {
        Reply {
            ok: false,
            daemon: None,
            code: Some(code),
            accepts: Some([OLDEST_ACCEPTED_VERSION, WIRE_VERSION]),
            data: None,
        }
    }
}

/// Why the daemon refused a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyCode {
    /// The line was not a valid request.
    BadRequest,
    /// `v` is outside the accepted range.
    UnsupportedVersion,
    /// The request or event type is unknown to this daemon.
    UnknownKind,
    /// The line was longer than [`MAX_LINE_BYTES`].
    LineTooLong,
    /// A code from a newer daemon.
    #[serde(other)]
    Unknown,
}

/// Why a request line did not decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Longer than [`MAX_LINE_BYTES`].
    LineTooLong {
        /// The line length.
        bytes: usize,
    },
    /// Not JSON, or a field failed its check.
    Malformed {
        /// The parser's message.
        message: String,
    },
    /// `v` outside the accepted range.
    UnsupportedVersion {
        /// The version sent.
        version: u16,
    },
    /// A request, event, control or query type this build does not know.
    UnknownKind,
}

impl DecodeError {
    /// The reply code the daemon sends for this error.
    #[must_use]
    pub fn reply_code(&self) -> ReplyCode {
        match self {
            DecodeError::LineTooLong { .. } => ReplyCode::LineTooLong,
            DecodeError::Malformed { .. } => ReplyCode::BadRequest,
            DecodeError::UnsupportedVersion { .. } => ReplyCode::UnsupportedVersion,
            DecodeError::UnknownKind => ReplyCode::UnknownKind,
        }
    }
}

/// Decodes and checks one request line.
///
/// # Errors
///
/// Returns a [`DecodeError`] for an oversized line, malformed JSON, a version
/// outside the accepted range, or an unknown kind.
pub fn decode_request(line: &str) -> Result<Request, DecodeError> {
    if line.len() > MAX_LINE_BYTES {
        return Err(DecodeError::LineTooLong { bytes: line.len() });
    }
    let request: Request = serde_json::from_str(line)
        .map_err(|error| DecodeError::Malformed { message: error.to_string() })?;
    let version = match &request {
        Request::Agent { v, .. } | Request::Control { v, .. } | Request::Query { v, .. } => *v,
        Request::Unknown => return Err(DecodeError::UnknownKind),
    };
    if !(OLDEST_ACCEPTED_VERSION..=WIRE_VERSION).contains(&version) {
        return Err(DecodeError::UnsupportedVersion { version });
    }
    match &request {
        Request::Agent { event: AgentEvent::Unknown, .. }
        | Request::Control { control: Control::Unknown, .. }
        | Request::Query { query: Query::Unknown, .. } => Err(DecodeError::UnknownKind),
        _ => Ok(request),
    }
}

/// Encodes `value` as one JSON line, newline included.
///
/// # Errors
///
/// Returns the serializer's error. The types in this module do not produce one.
pub fn encode_line<Value: Serialize>(value: &Value) -> Result<String, serde_json::Error> {
    let mut line = serde_json::to_string(value)?;
    line.push('\n');
    Ok(line)
}
