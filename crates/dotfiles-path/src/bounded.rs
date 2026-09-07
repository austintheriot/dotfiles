use std::fmt;

/// The byte cap on captured subprocess output.
const MAX_TEXT_LEN: usize = 4096;

/// Subprocess output, bounded and safe to render.
///
/// An unbounded subprocess string inside an error type is how a terminal
/// gets a control sequence written to it, so control bytes are replaced
/// rather than carried and the length is capped. `truncating` cannot fail:
/// the caller holds bytes a process already produced, and refusing them
/// would lose the only diagnostic the failure has.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BoundedText(String);

impl BoundedText {
    /// Bound and sanitize captured output.
    ///
    /// Infallible by design, for the reason the type documentation gives.
    pub fn truncating(raw: &str) -> Self {
        let mut kept = String::with_capacity(raw.len().min(MAX_TEXT_LEN));
        for character in raw.chars() {
            if kept.len() >= MAX_TEXT_LEN {
                break;
            }
            // Newline and tab survive: subprocess stderr is line-oriented
            // and stripping them would run the diagnostic together.
            if character.is_control() && character != '\n' && character != '\t' {
                kept.push('\u{fffd}');
            } else {
                kept.push(character);
            }
        }
        BoundedText(kept)
    }

    /// The bounded text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BoundedText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_a_control_byte_rather_than_carrying_it() {
        let bounded = BoundedText::truncating("E: failed\u{1b}[2J");
        assert!(!bounded.as_str().contains('\u{1b}'));
        assert!(bounded.as_str().starts_with("E: failed"));
    }

    #[test]
    fn keeps_newlines_and_tabs() {
        let bounded = BoundedText::truncating("line one\nline\ttwo");
        assert_eq!(bounded.as_str(), "line one\nline\ttwo");
    }

    #[test]
    fn caps_the_length() {
        let bounded = BoundedText::truncating(&"a".repeat(8192));
        assert!(bounded.as_str().len() <= 4096);
    }
}
