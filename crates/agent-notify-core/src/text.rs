//! Text that came from an agent or a hook and will be shown on screen.

use serde::{Deserialize, Serialize};

/// The largest [`DisplayText`], in bytes.
pub const DISPLAY_TEXT_MAX_BYTES: usize = 4096;

/// Agent-supplied text after cleaning. Never markup, never part of a command.
///
/// Decoding runs the same cleaning, so a daemon never holds text that skipped it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub struct DisplayText(String);

impl DisplayText {
    /// Turns `\r`, the line separator and the paragraph separator into a
    /// space, removes other control, bidi, zero-width, soft hyphen,
    /// interlinear annotation and tag characters, then cuts to
    /// [`DISPLAY_TEXT_MAX_BYTES`] on a character boundary.
    #[must_use]
    pub fn new(raw: &str) -> Self {
        let mut cleaned = String::with_capacity(raw.len().min(DISPLAY_TEXT_MAX_BYTES));
        for character in raw.chars().filter_map(cleaned_character) {
            if cleaned.len() + character.len_utf8() > DISPLAY_TEXT_MAX_BYTES {
                break;
            }
            cleaned.push(character);
        }
        DisplayText(cleaned)
    }

    /// The cleaned text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// True when the text holds only whitespace.
    #[must_use]
    pub fn is_blank(&self) -> bool {
        self.0.trim().is_empty()
    }
}

impl From<String> for DisplayText {
    fn from(raw: String) -> Self {
        DisplayText::new(&raw)
    }
}

impl From<DisplayText> for String {
    fn from(text: DisplayText) -> Self {
        text.0
    }
}

/// A line break other than `\n` becomes a space so the words on each side
/// stay apart.
fn cleaned_character(character: char) -> Option<char> {
    match character {
        '\r' | '\u{2028}' | '\u{2029}' => Some(' '),
        removed if is_removed(removed) => None,
        kept => Some(kept),
    }
}

/// `\n` survives because a card body's first sentence ends at a line break.
/// Tag characters (U+E0000 to U+E007F) can spell text no one sees.
fn is_removed(character: char) -> bool {
    (character.is_control() && character != '\n')
        || matches!(
            character,
            '\u{00AD}'
                | '\u{061C}'
                | '\u{180E}'
                | '\u{200B}'..='\u{200F}'
                | '\u{2028}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
                | '\u{FFF9}'..='\u{FFFB}'
                | '\u{E0000}'..='\u{E007F}'
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn removes_bidi_overrides_and_zero_width_characters() {
        let text = DisplayText::new("safe\u{202E}txt.exe\u{200B}\u{FEFF}");
        assert_eq!(text.as_str(), "safetxt.exe");
    }

    #[test]
    fn removes_terminal_control_characters() {
        assert_eq!(DisplayText::new("a\u{1b}[31mred\u{7}\u{9b}").as_str(), "a[31mred");
    }

    #[test]
    fn removes_every_invisible_or_hiding_class() {
        let cases = [
            ("tab", "a\tb", "ab"),
            ("language tag", "a\u{E0001}b", "ab"),
            ("tag characters that spell hidden text", "a\u{E0068}\u{E0069}\u{E007F}b", "ab"),
            ("first tag code point", "a\u{E0000}b", "ab"),
            ("soft hyphen", "a\u{00AD}b", "ab"),
            ("mongolian vowel separator", "a\u{180E}b", "ab"),
            ("interlinear annotation anchor", "a\u{FFF9}b", "ab"),
            ("interlinear annotation separator", "a\u{FFFA}b", "ab"),
            ("interlinear annotation terminator", "a\u{FFFB}b", "ab"),
            ("just past the tag block stays", "a\u{E0080}b", "a\u{E0080}b"),
            ("the replacement character stays", "a\u{FFFD}b", "a\u{FFFD}b"),
        ];
        for (name, raw, expected) in cases {
            assert_eq!(DisplayText::new(raw).as_str(), expected, "{name}");
        }
    }

    #[test]
    fn line_breaks_other_than_newline_become_a_space_so_words_stay_apart() {
        let cases = [
            ("carriage return", "a\rb", "a b"),
            ("carriage return before a newline", "a\r\nb", "a \nb"),
            ("line separator", "a\u{2028}b", "a b"),
            ("paragraph separator", "a\u{2029}b", "a b"),
        ];
        for (name, raw, expected) in cases {
            assert_eq!(DisplayText::new(raw).as_str(), expected, "{name}");
        }
    }

    #[test]
    fn keeps_newlines_and_non_latin_text() {
        assert_eq!(DisplayText::new("完了。\nnext").as_str(), "完了。\nnext");
    }

    #[test]
    fn cuts_on_a_character_boundary() {
        let text = DisplayText::new(&"é".repeat(3000));
        assert_eq!(text.as_str().len(), DISPLAY_TEXT_MAX_BYTES);
        assert!(text.as_str().chars().all(|character| character == 'é'));
    }

    #[test]
    fn decoding_cleans_again() {
        let text: DisplayText = serde_json::from_str("\"a\\u202eb\"").expect("decodes");
        assert_eq!(text.as_str(), "ab");
    }

    #[test]
    fn blank_means_only_whitespace() {
        assert!(DisplayText::new(" \n ").is_blank());
        assert!(!DisplayText::new(" x ").is_blank());
    }

    fn long_strings() -> impl Strategy<Value = String> {
        proptest::collection::vec(any::<char>(), 0..6000).prop_map(String::from_iter)
    }

    proptest! {
        #[test]
        fn output_is_a_bounded_clean_prefix_of_the_cleaned_input(raw in long_strings()) {
            let text = DisplayText::new(&raw);
            let cleaned: String = raw.chars().filter_map(cleaned_character).collect();
            prop_assert!(text.as_str().len() <= DISPLAY_TEXT_MAX_BYTES);
            prop_assert!(cleaned.starts_with(text.as_str()));
            prop_assert!(!text.as_str().chars().any(is_removed));
        }
    }
}
