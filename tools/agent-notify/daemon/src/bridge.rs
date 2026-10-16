//! The messages between the Rust shell and the web view. UI messages
//! carry card ids only; the daemon never takes a tmux target from the page.

use agent_notify_core::CardId;
use agent_notify_core::update::UiIntent;
use agent_notify_core::view::PanelViewModel;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The event that carries a new [`PanelViewModel`] to the page.
pub const VIEW_MODEL_EVENT: &str = "view-model";
/// The event that carries native hover to the page (macOS).
pub const HOVER_EVENT: &str = "hover";

/// The most bytes of a page error message that reach the daemon log.
pub const MAX_ERROR_MESSAGE_BYTES: usize = 1024;
/// The most bytes of a page error stack that reach the daemon log.
pub const MAX_ERROR_STACK_BYTES: usize = 4 * 1024;

/// A message from the page.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiMessage {
    /// A click on a card or a finished-row entry.
    Clicked {
        /// The card.
        card_id: CardId,
    },
    /// The close button.
    Closed {
        /// The card.
        card_id: CardId,
    },
    /// The pointer entered or left the stack. Sent only where the platform
    /// has no native hover (plan 3).
    Hover {
        /// True on enter.
        hovered: bool,
    },
    /// The stack's rendered height, so the shell can size the panel.
    Layout {
        /// CSS pixels.
        content_height_px: u32,
    },
    /// An uncaught error in the page.
    Error {
        /// The message.
        message: String,
        /// The stack, when the page has one.
        stack: Option<String>,
    },
}

/// What the page reads once at start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct UiBootstrap {
    /// The current view model.
    pub view_model: PanelViewModel,
    /// True when the shell reports hover (macOS). The page then shows close
    /// buttons from [`HOVER_EVENT`] instead of CSS `:hover`.
    pub native_hover: bool,
}

/// The payload of [`HOVER_EVENT`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
pub struct HoverPayload {
    /// True while the pointer is over the panel.
    pub hovered: bool,
}

/// Where a UI message goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Routed {
    /// To the core.
    Intent(UiIntent),
    /// To the panel geometry. The page sends this height unchecked, so the
    /// receiver must clamp it.
    Layout {
        /// CSS pixels, unchecked.
        content_height_px: u32,
    },
    /// To the log. Both texts are already cut to their byte limits.
    Error {
        /// The message, at most [`MAX_ERROR_MESSAGE_BYTES`] bytes.
        message: String,
        /// The stack, at most [`MAX_ERROR_STACK_BYTES`] bytes.
        stack: Option<String>,
    },
}

/// Routes one message.
#[must_use]
pub fn route(message: UiMessage) -> Routed {
    match message {
        UiMessage::Clicked { card_id } => Routed::Intent(UiIntent::Clicked { card_id }),
        UiMessage::Closed { card_id } => Routed::Intent(UiIntent::Closed { card_id }),
        UiMessage::Hover { hovered } => Routed::Intent(UiIntent::HoverChanged { hovered }),
        UiMessage::Layout { content_height_px } => Routed::Layout { content_height_px },
        UiMessage::Error { message, stack } => Routed::Error {
            message: truncate_with_marker(message, MAX_ERROR_MESSAGE_BYTES),
            stack: stack.map(|text| truncate_with_marker(text, MAX_ERROR_STACK_BYTES)),
        },
    }
}

fn truncate_with_marker(text: String, limit_bytes: usize) -> String {
    const MARKER: char = '…';
    if text.len() <= limit_bytes {
        return text;
    }
    let budget = limit_bytes.saturating_sub(MARKER.len_utf8());
    let cut = (0..=budget).rev().find(|index| text.is_char_boundary(*index)).unwrap_or(0);
    format!("{}{MARKER}", &text[..cut])
}

#[cfg(test)]
mod tests {
    use agent_notify_core::{CardId, EpochMs, EventId};

    use super::*;

    fn card() -> CardId {
        CardId::for_event(&EventId::from_parts(EpochMs(1), [0; 10]))
    }

    #[test]
    fn ui_messages_decode_from_tagged_json() {
        let text = format!(r#"{{"type":"clicked","card_id":"{}"}}"#, card().as_str());
        assert_eq!(serde_json::from_str::<UiMessage>(&text).expect("decodes"), UiMessage::Clicked { card_id: card() });
        let layout: UiMessage = serde_json::from_str(r#"{"type":"layout","content_height_px":212}"#).expect("decodes");
        assert_eq!(layout, UiMessage::Layout { content_height_px: 212 });
        let error: UiMessage = serde_json::from_str(r#"{"type":"error","message":"boom","stack":null}"#).expect("decodes");
        assert_eq!(error, UiMessage::Error { message: "boom".to_string(), stack: None });
    }

    #[test]
    fn a_ui_message_cannot_name_a_tmux_target() {
        let text = format!(r#"{{"type":"clicked","card_id":"{}","pane":"%1"}}"#, card().as_str());
        assert!(serde_json::from_str::<UiMessage>(&text).is_err(), "an extra pane field is refused");
    }

    #[test]
    fn a_ui_message_with_a_bad_card_id_is_refused() {
        let text = r#"{"type":"clicked","card_id":"x"}"#;
        assert!(serde_json::from_str::<UiMessage>(text).is_err(), "a malformed card id is refused");
    }

    #[test]
    fn routing_turns_card_messages_into_intents() {
        assert_eq!(route(UiMessage::Clicked { card_id: card() }), Routed::Intent(UiIntent::Clicked { card_id: card() }));
        assert_eq!(route(UiMessage::Closed { card_id: card() }), Routed::Intent(UiIntent::Closed { card_id: card() }));
        assert_eq!(route(UiMessage::Hover { hovered: true }), Routed::Intent(UiIntent::HoverChanged { hovered: true }));
        assert_eq!(route(UiMessage::Layout { content_height_px: 9 }), Routed::Layout { content_height_px: 9 });
    }

    #[test]
    fn short_error_text_passes_through_unchanged() {
        let routed = route(UiMessage::Error { message: "boom".to_string(), stack: Some("at a:1".to_string()) });
        assert_eq!(routed, Routed::Error { message: "boom".to_string(), stack: Some("at a:1".to_string()) });
    }

    #[test]
    fn long_multibyte_error_text_is_cut_on_a_char_boundary() {
        let message = "é".repeat(2_000);
        let stack = "漢".repeat(5_000);
        let Routed::Error { message: cut_message, stack: Some(cut_stack) } =
            route(UiMessage::Error { message, stack: Some(stack) })
        else {
            panic!("an error routes to an error");
        };
        assert!(cut_message.len() <= MAX_ERROR_MESSAGE_BYTES, "{}", cut_message.len());
        assert!(cut_message.ends_with('…'));
        assert!(cut_message.starts_with("éé"));
        assert!(cut_stack.len() <= MAX_ERROR_STACK_BYTES, "{}", cut_stack.len());
        assert!(cut_stack.ends_with('…'));
        assert!(cut_stack.len() > MAX_ERROR_STACK_BYTES - 8, "the cut keeps nearly the whole budget");
    }

    #[test]
    fn text_exactly_at_the_limit_is_not_marked() {
        let message = "a".repeat(MAX_ERROR_MESSAGE_BYTES);
        let Routed::Error { message: kept, .. } = route(UiMessage::Error { message: message.clone(), stack: None }) else {
            panic!("an error routes to an error");
        };
        assert_eq!(kept, message);
    }
}
