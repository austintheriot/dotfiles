//! Pure decisions for agent-notify: wire types, hook mapping, labels, and the
//! daemon's state machine. No IO, no clock, no randomness: time arrives as
//! `EpochMs` data, so every rule is testable with plain values.

pub mod text;

pub use text::DisplayText;
