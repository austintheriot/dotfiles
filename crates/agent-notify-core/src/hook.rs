//! Maps a Claude Code hook payload to the pane-tag writes and the agent event
//! it implies. See the spec's section 5.4 for the table.

use std::path::PathBuf;

use serde::Deserialize;

use crate::wire::{AgentEvent, InputReason, SessionSource};
use crate::{DisplayText, PromptId, SessionId};

/// The pane option that names the Claude session living in a pane.
pub const SESSION_TAG_OPTION: &str = "@agent_notify_session";
/// The pane option that names the current turn in a pane.
pub const PROMPT_TAG_OPTION: &str = "@agent_notify_prompt";

/// The fields of a Claude Code hook payload this crate reads.
#[derive(Debug, Clone, Deserialize)]
pub struct ClaudeHookInput {
    /// The session.
    pub session_id: SessionId,
    /// The turn. Absent before the first prompt.
    #[serde(default)]
    pub prompt_id: Option<PromptId>,
    /// The session's working directory.
    pub cwd: PathBuf,
    /// Which hook fired.
    pub hook_event_name: String,
    /// `SessionStart` only.
    #[serde(default)]
    pub source: Option<SessionSource>,
    /// Present when the session has a title.
    #[serde(default)]
    pub session_title: Option<DisplayText>,
    /// `Stop` and `StopFailure`.
    #[serde(default)]
    pub last_assistant_message: Option<DisplayText>,
    /// `StopFailure`: the error type.
    #[serde(default)]
    pub error: Option<String>,
    /// `Notification`.
    #[serde(default)]
    pub notification_type: Option<String>,
    /// `Notification`.
    #[serde(default)]
    pub message: Option<DisplayText>,
}

/// A change to the pane tags, applied before the request is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagWrite {
    /// Set the session tag and clear the prompt tag.
    BindSession(SessionId),
    /// Set the prompt tag.
    BindPrompt(PromptId),
    /// Clear both tags.
    UnbindSession,
}

/// What one hook invocation does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookDecision {
    /// Write `tags`, then send `event`.
    Send {
        /// Tag writes, in order.
        tags: Vec<TagWrite>,
        /// The event to send.
        event: AgentEvent,
    },
    /// Do nothing. `reason` goes to the log.
    Ignore {
        /// Why nothing happens.
        reason: String,
    },
}

/// Decides the tag writes and event for one hook payload.
///
/// `pane_session_tag` is the pane's current session tag, so a late
/// `SessionEnd` from an old session does not erase a new session's tag.
#[must_use]
pub fn decide_hook(input: &ClaudeHookInput, pane_session_tag: Option<&SessionId>) -> HookDecision {
    let send = |tags: Vec<TagWrite>, event: AgentEvent| HookDecision::Send { tags, event };
    match input.hook_event_name.as_str() {
        "SessionStart" => send(
            vec![TagWrite::BindSession(input.session_id.clone())],
            AgentEvent::SessionStarted { source: input.source.unwrap_or(SessionSource::Unknown) },
        ),
        "UserPromptSubmit" => send(
            input.prompt_id.iter().cloned().map(TagWrite::BindPrompt).collect(),
            AgentEvent::TurnStarted,
        ),
        "Stop" => send(
            Vec::new(),
            AgentEvent::TurnEnded { last_message: input.last_assistant_message.clone() },
        ),
        "StopFailure" => send(Vec::new(), AgentEvent::TurnFailed { error: failure_text(input) }),
        "Notification" => notification(input),
        "SessionEnd" => {
            let tags = if pane_session_tag == Some(&input.session_id) {
                vec![TagWrite::UnbindSession]
            } else {
                Vec::new()
            };
            send(tags, AgentEvent::SessionEnded)
        }
        other => HookDecision::Ignore { reason: format!("hook event {other:?} is not mapped") },
    }
}

fn failure_text(input: &ClaudeHookInput) -> DisplayText {
    match (&input.last_assistant_message, &input.error) {
        (Some(message), _) if !message.is_blank() => message.clone(),
        (_, Some(error)) => DisplayText::new(error),
        _ => DisplayText::new("unknown error"),
    }
}

fn notification(input: &ClaudeHookInput) -> HookDecision {
    let message = |default: &str| {
        input
            .message
            .clone()
            .filter(|message| !message.is_blank())
            .unwrap_or_else(|| DisplayText::new(default))
    };
    let event = match input.notification_type.as_deref() {
        Some("permission_prompt") => AgentEvent::NeedsInput {
            reason: InputReason::Permission,
            message: message("Needs your permission"),
        },
        Some("elicitation_dialog" | "elicitation_url_dialog") => AgentEvent::NeedsInput {
            reason: InputReason::Elicitation,
            message: message("Needs an answer"),
        },
        Some("elicitation_complete") => AgentEvent::InputResolved,
        other => {
            return HookDecision::Ignore { reason: format!("notification type {other:?} is not mapped") };
        }
    };
    HookDecision::Send { tags: Vec::new(), event }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::session;

    fn input(event: &str, extra: serde_json::Value) -> ClaudeHookInput {
        let mut payload = serde_json::json!({
            "session_id": "session-a",
            "prompt_id": "prompt-1",
            "transcript_path": "/home/dev/.claude/projects/x/session-a.jsonl",
            "cwd": "/home/dev/code",
            "permission_mode": "default",
            "hook_event_name": event,
        });
        for (key, value) in extra.as_object().expect("an object") {
            payload[key] = value.clone();
        }
        serde_json::from_value(payload).expect("a valid synthetic payload")
    }

    fn sent(decision: HookDecision) -> (Vec<TagWrite>, AgentEvent) {
        match decision {
            HookDecision::Send { tags, event } => (tags, event),
            HookDecision::Ignore { reason } => panic!("expected a send, got ignore: {reason}"),
        }
    }

    #[test]
    fn session_start_binds_the_session_tag() {
        let (tags, event) = sent(decide_hook(&input("SessionStart", serde_json::json!({"source": "clear"})), None));
        assert_eq!(tags, vec![TagWrite::BindSession(session("session-a"))]);
        assert_eq!(event, AgentEvent::SessionStarted { source: SessionSource::Clear });
    }

    #[test]
    fn user_prompt_submit_binds_the_prompt_tag() {
        let (tags, event) = sent(decide_hook(&input("UserPromptSubmit", serde_json::json!({"prompt": "hi"})), None));
        assert_eq!(tags, vec![TagWrite::BindPrompt(crate::test_support::prompt("prompt-1"))]);
        assert_eq!(event, AgentEvent::TurnStarted);
    }

    #[test]
    fn stop_carries_the_last_message() {
        let (tags, event) = sent(decide_hook(
            &input("Stop", serde_json::json!({"last_assistant_message": "Done. Tests pass.", "stop_hook_active": false})),
            None,
        ));
        assert!(tags.is_empty());
        assert_eq!(event, AgentEvent::TurnEnded { last_message: Some(DisplayText::new("Done. Tests pass.")) });
    }

    #[test]
    fn stop_failure_prefers_the_rendered_message_then_the_error_type() {
        let (_, rendered) = sent(decide_hook(
            &input("StopFailure", serde_json::json!({"error": "rate_limit", "last_assistant_message": "API Error: Rate limit reached"})),
            None,
        ));
        assert_eq!(rendered, AgentEvent::TurnFailed { error: DisplayText::new("API Error: Rate limit reached") });
        let (_, bare) = sent(decide_hook(&input("StopFailure", serde_json::json!({"error": "overloaded"})), None));
        assert_eq!(bare, AgentEvent::TurnFailed { error: DisplayText::new("overloaded") });
    }

    #[test]
    fn mapped_notification_types() {
        let cases = [
            ("permission_prompt", Some(AgentEvent::NeedsInput { reason: InputReason::Permission, message: DisplayText::new("Claude needs your permission") })),
            ("elicitation_dialog", Some(AgentEvent::NeedsInput { reason: InputReason::Elicitation, message: DisplayText::new("Claude needs your permission") })),
            ("elicitation_url_dialog", Some(AgentEvent::NeedsInput { reason: InputReason::Elicitation, message: DisplayText::new("Claude needs your permission") })),
            ("elicitation_complete", Some(AgentEvent::InputResolved)),
            ("idle_prompt", None),
            ("agent_completed", None),
            ("a_future_type", None),
        ];
        for (notification_type, expected) in cases {
            let decision = decide_hook(
                &input("Notification", serde_json::json!({"notification_type": notification_type, "message": "Claude needs your permission"})),
                None,
            );
            match (decision, expected) {
                (HookDecision::Send { event, .. }, Some(expected)) => assert_eq!(event, expected, "{notification_type}"),
                (HookDecision::Ignore { .. }, None) => {}
                (other, expected) => panic!("{notification_type}: got {other:?}, expected {expected:?}"),
            }
        }
    }

    #[test]
    fn a_blank_notification_message_gets_a_default() {
        let (_, event) = sent(decide_hook(
            &input("Notification", serde_json::json!({"notification_type": "permission_prompt", "message": "  "})),
            None,
        ));
        assert_eq!(event, AgentEvent::NeedsInput { reason: InputReason::Permission, message: DisplayText::new("Needs your permission") });
    }

    #[test]
    fn session_end_unbinds_only_a_matching_tag() {
        let ending = input("SessionEnd", serde_json::json!({"reason": "clear"}));
        let (matching, event) = sent(decide_hook(&ending, Some(&session("session-a"))));
        assert_eq!(matching, vec![TagWrite::UnbindSession]);
        assert_eq!(event, AgentEvent::SessionEnded);
        let (other, _) = sent(decide_hook(&ending, Some(&session("session-b"))));
        assert!(other.is_empty(), "a newer session's tag must survive a late SessionEnd");
    }

    #[test]
    fn unknown_hook_events_are_ignored_with_a_reason() {
        let decision = decide_hook(&input("PreCompact", serde_json::json!({})), None);
        assert!(matches!(decision, HookDecision::Ignore { reason } if reason.contains("PreCompact")));
    }

    #[test]
    fn a_missing_prompt_id_is_allowed() {
        let payload = serde_json::json!({"session_id": "session-a", "cwd": "/x", "hook_event_name": "SessionStart", "source": "startup"});
        let parsed: ClaudeHookInput = serde_json::from_value(payload).expect("decodes");
        assert_eq!(parsed.prompt_id, None);
    }
}
