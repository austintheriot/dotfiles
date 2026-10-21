//! Identifiers and timestamps that cross the socket. Each identifier is
//! checked when it is built, so holding one proves its format.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Milliseconds since the Unix epoch. The only form time takes in this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(type = "number"))]
pub struct EpochMs(pub i64);

impl EpochMs {
    /// This time plus `milliseconds`, saturating.
    #[must_use]
    pub fn plus_ms(self, milliseconds: i64) -> Self {
        EpochMs(self.0.saturating_add(milliseconds))
    }

    /// Milliseconds from `earlier` to this time. Negative when `earlier` is later.
    #[must_use]
    pub fn since(self, earlier: EpochMs) -> i64 {
        self.0.saturating_sub(earlier.0)
    }
}

/// A value failed the format check of the identifier it was meant to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidId {
    /// Which identifier.
    pub kind: &'static str,
    /// The rejected text, cut to 64 characters.
    pub value: String,
}

impl fmt::Display for InvalidId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} has an invalid format: {:?}", self.kind, self.value)
    }
}

impl std::error::Error for InvalidId {}

fn invalid(kind: &'static str, value: &str) -> InvalidId {
    InvalidId { kind, value: value.chars().take(64).collect() }
}

macro_rules! checked_id {
    ($(#[$doc:meta])* $name:ident, $kind:literal, $check:expr) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(type = "string"))]
        pub struct $name(String);

        impl $name {
            /// Checks `value` and wraps it.
            ///
            /// # Errors
            ///
            /// Returns [`InvalidId`] when `value` does not have this identifier's format.
            pub fn parse(value: &str) -> Result<Self, InvalidId> {
                let check: fn(&str) -> bool = $check;
                if check(value) { Ok($name(value.to_string())) } else { Err(invalid($kind, value)) }
            }

            /// The identifier as text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = InvalidId;
            fn try_from(value: String) -> Result<Self, InvalidId> {
                $name::parse(&value)
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> String {
                id.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

fn is_token(value: &str) -> bool {
    (1..=128).contains(&value.len())
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const ULID_TIME_MASK: u128 = (1 << 48) - 1;

checked_id!(
    /// A Claude Code `session_id`.
    SessionId, "session id", is_token
);
checked_id!(
    /// A Claude Code `prompt_id`. One per user turn.
    PromptId, "prompt id", is_token
);
checked_id!(
    /// A tmux pane id such as `%12`.
    PaneId, "pane id",
    |value| value.len() >= 2 && value.starts_with('%') && value[1..].bytes().all(|byte| byte.is_ascii_digit())
);
checked_id!(
    /// A ULID that names one CLI invocation in every log line.
    EventId, "event id",
    |value| value.len() == 26 && value.bytes().all(|byte| CROCKFORD.contains(&byte))
);
checked_id!(
    /// One card instance. Fresh for every card, never reused.
    CardId, "card id",
    |value| value.strip_prefix("card-").is_some_and(is_token)
);

impl PaneId {
    /// The number after `%`. Used only to order panes.
    #[must_use]
    pub fn number(&self) -> u32 {
        self.0[1..].parse().unwrap_or(u32::MAX)
    }
}

impl EventId {
    /// Builds a ULID from a timestamp and 80 random bits. Later times sort later.
    #[must_use]
    pub fn from_parts(time: EpochMs, random: [u8; 10]) -> Self {
        let millis = u128::try_from(time.0).unwrap_or(0) & ULID_TIME_MASK;
        let random_bits = random.iter().fold(0_u128, |bits, byte| (bits << 8) | u128::from(*byte));
        let value = (millis << 80) | random_bits;
        let encoded = (0..26)
            .rev()
            .map(|position| char::from(CROCKFORD[((value >> (position * 5)) & 0x1f) as usize]))
            .collect();
        EventId(encoded)
    }
}

impl CardId {
    /// The card created by `event`. Event ids are unique, so card ids are too.
    #[must_use]
    pub fn for_event(event: &EventId) -> Self {
        CardId(format!("card-{}", event.as_str()))
    }
}

/// An absolute socket path with no NUL byte.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "PathBuf", into = "PathBuf")]
pub struct SocketPath(PathBuf);

impl SocketPath {
    /// Checks `path` and wraps it.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidId`] for a relative path or a path with a NUL byte.
    pub fn parse(path: &Path) -> Result<Self, InvalidId> {
        let has_nul = path.as_os_str().as_encoded_bytes().contains(&0);
        if path.is_absolute() && !has_nul {
            Ok(SocketPath(path.to_path_buf()))
        } else {
            Err(invalid("socket path", &path.display().to_string()))
        }
    }

    /// The path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl TryFrom<PathBuf> for SocketPath {
    type Error = InvalidId;
    fn try_from(path: PathBuf) -> Result<Self, InvalidId> {
        SocketPath::parse(&path)
    }
}

impl From<SocketPath> for PathBuf {
    fn from(path: SocketPath) -> PathBuf {
        path.0
    }
}

/// One tmux server. Pane ids restart at `%0` on a new server with the same
/// socket path, so the pid and start time are part of the identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ServerId {
    /// The server's socket path.
    pub socket_path: SocketPath,
    /// `#{pid}`.
    pub pid: u32,
    /// `#{start_time}`, seconds since the epoch.
    pub start_time: i64,
}

/// One pane on one tmux server.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PaneKey {
    /// The server.
    pub server: ServerId,
    /// The pane on that server.
    pub pane: PaneId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_ids_accept_uuid_text_and_reject_other_characters() {
        assert!(SessionId::parse("0b6e8a8c-1f2d-4c3b-9a7e-5d4c3b2a1f00").is_ok());
        assert!(SessionId::parse("").is_err());
        assert!(SessionId::parse("a b").is_err());
        assert!(SessionId::parse("a;rm").is_err());
        assert!(SessionId::parse(&"x".repeat(129)).is_err());
    }

    #[test]
    fn pane_ids_need_a_percent_and_digits() {
        assert_eq!(PaneId::parse("%12").map(|pane| pane.number()), Ok(12));
        for bad in ["12", "%", "%1a", "%-1", "%%1"] {
            assert!(PaneId::parse(bad).is_err(), "{bad} must be rejected");
        }
    }

    #[test]
    fn event_ids_encode_time_first() {
        assert_eq!(
            EventId::from_parts(EpochMs(0), [0; 10]).as_str(),
            "00000000000000000000000000"
        );
        assert_eq!(
            EventId::from_parts(EpochMs(1), [0; 10]).as_str(),
            "00000000010000000000000000"
        );
        let earlier = EventId::from_parts(EpochMs(1_760_000_000_000), [255; 10]);
        let later = EventId::from_parts(EpochMs(1_760_000_000_001), [0; 10]);
        assert!(earlier < later);
    }

    #[test]
    fn event_ids_round_trip_through_parse() {
        let id = EventId::from_parts(EpochMs(1_760_000_000_000), [9; 10]);
        assert_eq!(EventId::parse(id.as_str()), Ok(id.clone()));
        assert!(EventId::parse("0000000000000000000000000I").is_err());
    }

    #[test]
    fn card_ids_derive_from_event_ids() {
        let event = EventId::from_parts(EpochMs(5), [1; 10]);
        let card = CardId::for_event(&event);
        assert_eq!(card.as_str(), format!("card-{}", event.as_str()));
        assert!(CardId::parse(card.as_str()).is_ok());
    }

    #[test]
    fn socket_paths_must_be_absolute_without_nul() {
        assert!(SocketPath::parse(Path::new("/tmp/tmux-501/default")).is_ok());
        assert!(SocketPath::parse(Path::new("tmux-501/default")).is_err());
        assert!(SocketPath::parse(Path::new("/tmp/a\0b")).is_err());
    }

    #[test]
    fn ids_decode_with_the_same_checks() {
        assert!(serde_json::from_str::<PaneId>("\"%3\"").is_ok());
        assert!(serde_json::from_str::<PaneId>("\"3\"").is_err());
        assert!(serde_json::from_str::<SocketPath>("\"relative\"").is_err());
    }

    #[test]
    fn epoch_arithmetic_saturates() {
        assert_eq!(EpochMs(i64::MAX).plus_ms(1), EpochMs(i64::MAX));
        assert_eq!(EpochMs(10).since(EpochMs(4)), 6);
    }
}
