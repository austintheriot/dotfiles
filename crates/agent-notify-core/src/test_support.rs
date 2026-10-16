//! Builders for unit tests. Panics are fine here: a bad literal is a test bug.

// Each builder is used by some test module, not by all of them.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use crate::DisplayText;
use crate::state::{Card, DaemonState};
use crate::update::{DaemonInput, Effect, FocusSnapshot, SuppressReason, update};
use crate::wire::{AgentEvent, AgentKind, Origin, Request, WIRE_VERSION, WindowLabel};
use crate::{EpochMs, EventId, PaneId, PaneKey, PromptId, ServerId, SessionId, SocketPath};

pub const WINDOW_INDEX: u32 = 4;

pub fn event_id(number: i64) -> EventId {
    EventId::from_parts(EpochMs(number), [0; 10])
}

pub fn session(name: &str) -> SessionId {
    SessionId::parse(name).expect("a valid session id literal")
}

pub fn prompt(name: &str) -> PromptId {
    PromptId::parse(name).expect("a valid prompt id literal")
}

pub fn server() -> ServerId {
    ServerId {
        socket_path: SocketPath::parse(Path::new("/tmp/tmux-501/default")).expect("absolute"),
        pid: 100,
        start_time: 1_700_000_000,
    }
}

pub fn pane(number: u32) -> PaneKey {
    PaneKey { server: server(), pane: PaneId::parse(&format!("%{number}")).expect("a pane id") }
}

pub fn origin(session_name: Option<&str>, prompt_name: Option<&str>) -> Origin {
    Origin {
        agent: AgentKind::ClaudeCode,
        session: session_name.map(session),
        prompt: prompt_name.map(prompt),
        pane: Some(pane(1)),
        window: Some(WindowLabel { index: WINDOW_INDEX, name: DisplayText::new("code/feature-x") }),
        repo: None,
        session_title: None,
        cwd: PathBuf::from("/home/dev/code"),
    }
}

pub fn agent_input(number: i64, origin: Origin, event: AgentEvent) -> DaemonInput {
    agent_input_sent_at(number, number, origin, event)
}

pub fn agent_input_sent_at(number: i64, sent_at: i64, origin: Origin, event: AgentEvent) -> DaemonInput {
    DaemonInput::Received {
        request: Request::Agent {
            v: WIRE_VERSION,
            event_id: event_id(number),
            sent_at_ms: EpochMs(sent_at),
            origin,
            event,
        },
        focus: FocusSnapshot::unknown(),
    }
}

/// Like [`send`], but the CLI built the request at `sent_at`, so a test can
/// deliver an event later than it was sent.
pub fn send_sent_at(
    state: &mut DaemonState,
    number: i64,
    sent_at: i64,
    session_name: &str,
    prompt_name: Option<&str>,
    event: AgentEvent,
) -> Vec<Effect> {
    update(state, agent_input_sent_at(number, sent_at, origin(Some(session_name), prompt_name), event), EpochMs(number))
}

/// Sends `event` for session `session_name` at time `number`, which is also
/// the event id, so a test can name the card an event made.
pub fn send(
    state: &mut DaemonState,
    number: i64,
    session_name: &str,
    prompt_name: Option<&str>,
    event: AgentEvent,
) -> Vec<Effect> {
    update(state, agent_input(number, origin(Some(session_name), prompt_name), event), EpochMs(number))
}

pub fn card<'a>(state: &'a DaemonState, session_name: &str) -> Option<&'a Card> {
    state.sessions.get(&session(session_name)).and_then(|record| record.card.as_ref())
}

pub fn reasons(effects: &[Effect]) -> Vec<SuppressReason> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Suppressed { reason, .. } => Some(*reason),
            _ => None,
        })
        .collect()
}
