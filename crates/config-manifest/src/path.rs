use std::fmt;

/// Why a string was rejected as a git object id.
///
/// Two variants rather than one opaque message because the two failures have
/// different causes: a length mismatch is usually an abbreviated id or a
/// truncated field, while non-hex text is usually a different kind of value
/// entirely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdError {
    /// The character count is neither 40 (SHA-1) nor 64 (SHA-256). Carries
    /// the count found, since that is what distinguishes an abbreviated id
    /// from an unrelated string.
    Length(usize),
    /// The length was right but the text is not lowercase hex, so it cannot
    /// be an id git would produce. Carries the input, because the caller
    /// needs to see which field of a composite stamp went wrong.
    NonHex(String),
}

impl fmt::Display for IdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdError::Length(len) => write!(
                formatter,
                "object id has {len} characters, expected 40 or 64"
            ),
            IdError::NonHex(raw) => write!(formatter, "object id is not lowercase hex: {raw}"),
        }
    }
}

impl std::error::Error for IdError {}

fn parse_object_id(raw: &str) -> Result<String, IdError> {
    let len = raw.len();
    if len != 40 && len != 64 {
        return Err(IdError::Length(len));
    }
    let is_lower_hex = raw
        .chars()
        .all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch));
    if !is_lower_hex {
        return Err(IdError::NonHex(raw.to_string()));
    }
    Ok(raw.to_string())
}

/// A git tree id.
///
/// Distinct from a bare `String` so a value read out of a stamp cannot be
/// passed where an unrelated string was wanted. The stamp folds one tree id
/// and two blob ids, and a transposition there would produce a well-formed
/// stamp that gates pushes wrongly.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TreeId(String);

impl TreeId {
    /// Accepts text as a git object id.
    ///
    /// The only constructor, which is what makes "a `TreeId` is well formed"
    /// an invariant rather than a convention. Both SHA-1 and SHA-256 widths
    /// are accepted, since the repository's hash algorithm is not this
    /// crate's decision.
    ///
    /// # Errors
    ///
    /// Returns [`IdError::Length`] when the input is neither 40 nor 64
    /// characters, and [`IdError::NonHex`] when it contains anything outside
    /// `0-9a-f`.
    pub fn parse(raw: &str) -> Result<Self, IdError> {
        parse_object_id(raw).map(TreeId)
    }

    /// Borrows the id as text, for embedding in a stamp or a report.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_accept_40_and_64_lowercase_hex_only() {
        let sha1 = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
        let sha256 = "a".repeat(64);
        assert!(TreeId::parse(sha1).is_ok());
        assert!(TreeId::parse(&sha256).is_ok());
        assert_eq!(TreeId::parse("abc"), Err(IdError::Length(3)));
        assert_eq!(
            TreeId::parse(&"G".repeat(40)),
            Err(IdError::NonHex("G".repeat(40)))
        );
    }
}
