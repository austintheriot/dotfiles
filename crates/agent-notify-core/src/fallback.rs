//! The plain OS notification the CLI shows when the daemon does not answer.
//! Only events that would produce a card get one, and never `TurnEnded`: the
//! CLI cannot know whether the agent already sent its own notification.

use crate::label::{location_line, turn_failed_body};
use crate::wire::{AgentEvent, Kind, Origin};

/// The subtitle that tells the user the daemon is down.
pub const OFFLINE_SUBTITLE: &str = "agent-notify offline";

/// The text of one fallback notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackNote {
    /// The location line.
    pub title: String,
    /// Always [`OFFLINE_SUBTITLE`].
    pub subtitle: &'static str,
    /// The card body.
    pub message: String,
    /// True for attention events. Linux maps it to `--urgency=critical`.
    pub urgent: bool,
}

/// The fallback for `event`, or `None` when the event shows no fallback.
#[must_use]
pub fn fallback_note(origin: &Origin, event: &AgentEvent) -> Option<FallbackNote> {
    let (message, urgent) = match event {
        AgentEvent::NeedsInput { message, .. } => (message.as_str().to_string(), true),
        AgentEvent::TurnFailed { error } => (turn_failed_body(error).as_str().to_string(), true),
        AgentEvent::Explicit { kind, body } => (body.as_str().to_string(), *kind == Kind::Attention),
        _ => return None,
    };
    Some(FallbackNote {
        title: location_line(origin.window.as_ref(), origin.repo.as_ref(), &origin.cwd),
        subtitle: OFFLINE_SUBTITLE,
        message,
        urgent,
    })
}

