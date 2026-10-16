//! Pure decisions for agent-notify: wire types, hook mapping, labels, and the
//! daemon's state machine. No IO, no clock, no randomness: time arrives as
//! `EpochMs` data, so every rule is testable with plain values.

pub mod fallback;
pub mod hook;
pub mod label;
pub mod ids;
pub mod paths;
pub mod text;
pub mod wire;

#[cfg(test)]
pub(crate) mod test_support;

pub use ids::{
    CardId, EpochMs, EventId, InvalidId, PaneId, PaneKey, PromptId, ServerId, SessionId,
    SocketPath,
};
pub use text::DisplayText;
