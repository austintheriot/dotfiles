//! Builders for unit tests. Panics are fine here: a bad literal is a test bug.

// Each builder is used by some test module, not by all of them.
#![allow(dead_code)]

use std::path::Path;

use crate::{EpochMs, EventId, PaneId, PaneKey, PromptId, ServerId, SessionId, SocketPath};

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
