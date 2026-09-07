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
            // Newline and tab survive: subprocess stderr is line-oriented
            // and stripping them would run the diagnostic together.
            let replaced = character.is_control()
                && character != '\n'
                && character != '\t';
            let candidate = if replaced { '\u{fffd}' } else { character };

            // Asks whether the character FITS, rather than whether the cap
            // is already reached. The former check appended a straddling
            // character whole: 4095 ASCII bytes plus a four-byte emoji
            // produced 4099 against a 4096 cap. Breaking rather than
            // skipping, so the kept text stays a prefix of the input.
            if kept.len() + candidate.len_utf8() > MAX_TEXT_LEN {
                break;
            }
            kept.push(candidate);
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

    /// The cap is a byte cap, and a multi-byte character must not cross it.
    ///
    /// The loop checked the length before pushing, so a character whose
    /// encoding straddled the boundary was appended whole and the result
    /// exceeded the cap. Measured before the fix: 4095 ASCII bytes plus a
    /// four-byte emoji yielded **4099** bytes against a 4096 cap. The
    /// existing length test uses pure ASCII, where one character is one byte,
    /// so it cannot observe this.
    #[test]
    fn a_multibyte_character_cannot_cross_the_cap() {
        // Positive control: the ASCII case must already sit exactly at the
        // cap, or an over-cap result below would not be attributable to the
        // multi-byte boundary.
        let ascii = "a".repeat(MAX_TEXT_LEN + 10);
        assert_eq!(BoundedText::truncating(&ascii).as_str().len(), MAX_TEXT_LEN);

        // Every width that can straddle a byte boundary.
        for wide in ['\u{00e9}', '\u{20ac}', '\u{1F600}'] {
            let width = wide.len_utf8();
            for offset in 1..=width {
                let mut raw = "a".repeat(MAX_TEXT_LEN - offset);
                raw.push(wide);
                let bounded = BoundedText::truncating(&raw);
                assert!(
                    bounded.as_str().len() <= MAX_TEXT_LEN,
                    "{width}-byte char at offset {offset} produced {} bytes",
                    bounded.as_str().len()
                );
            }
        }
    }
}
