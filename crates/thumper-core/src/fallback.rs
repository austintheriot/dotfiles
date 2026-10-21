//! The plain OS notification the CLI shows when the daemon does not answer.
//! Only events that would produce a card get one, and never `TurnEnded`: the
//! CLI cannot know whether the agent already sent its own notification.

use crate::label::{location_line, turn_failed_body};
use crate::wire::{AgentEvent, Kind, Origin};

/// The subtitle that tells the user the daemon is down.
pub const OFFLINE_SUBTITLE: &str = "thumper offline";

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


#[cfg(test)]
mod tests {
    use super::*;
    use crate::DisplayText;
    use crate::wire::{AgentKind, InputReason, WindowLabel};
    use std::path::PathBuf;

    fn origin() -> Origin {
        Origin {
            agent: AgentKind::ClaudeCode,
            session: None,
            prompt: None,
            pane: None,
            window: Some(WindowLabel { index: 4, name: DisplayText::new("code/x") }),
            repo: None,
            session_title: None,
            cwd: PathBuf::from("/home/dev/code"),
        }
    }

    #[test]
    fn attention_events_and_explicit_sends_get_a_note() {
        let needs = AgentEvent::NeedsInput { reason: InputReason::Permission, message: DisplayText::new("Allow Bash?") };
        let note = fallback_note(&origin(), &needs).expect("a note");
        assert_eq!(note.title, "4 · code/x");
        assert_eq!(note.subtitle, OFFLINE_SUBTITLE);
        assert_eq!(note.message, "Allow Bash?");
        assert!(note.urgent);

        let failed = AgentEvent::TurnFailed { error: DisplayText::new("overloaded") };
        assert_eq!(fallback_note(&origin(), &failed).map(|note| note.message), Some("Stopped: overloaded".to_string()));

        let info = AgentEvent::Explicit { kind: Kind::Info, body: DisplayText::new("Report ready") };
        assert_eq!(fallback_note(&origin(), &info).map(|note| note.urgent), Some(false));
    }

    #[test]
    fn turn_ended_and_bookkeeping_events_get_no_note() {
        for event in [
            AgentEvent::TurnEnded { last_message: Some(DisplayText::new("Done.")) },
            AgentEvent::TurnStarted,
            AgentEvent::QuietOff,
            AgentEvent::SessionEnded,
            AgentEvent::InputResolved,
        ] {
            assert_eq!(fallback_note(&origin(), &event), None, "{event:?}");
        }
    }
}
