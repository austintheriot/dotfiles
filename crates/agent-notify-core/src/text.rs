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
    /// Removes control, bidi and zero-width characters, then cuts to
    /// [`DISPLAY_TEXT_MAX_BYTES`] on a character boundary.
    #[must_use]
    pub fn new(raw: &str) -> Self {
        let mut cleaned = String::with_capacity(raw.len().min(DISPLAY_TEXT_MAX_BYTES));
        for character in raw.chars().filter(|character| !is_removed(*character)) {
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

/// `\n` survives because a card body's first sentence ends at a line break.
fn is_removed(character: char) -> bool {
    (character.is_control() && character != '\n')
        || matches!(
            character,
            '\u{061C}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
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
            let cleaned: String = raw.chars().filter(|character| !is_removed(*character)).collect();
            prop_assert!(text.as_str().len() <= DISPLAY_TEXT_MAX_BYTES);
            prop_assert!(cleaned.starts_with(text.as_str()));
            prop_assert!(!text.as_str().chars().any(is_removed));
        }
    }
}
