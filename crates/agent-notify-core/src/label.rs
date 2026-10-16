//! The text a card shows. Built only from origin data and agent text.

use std::path::Path;

use crate::DisplayText;
use crate::wire::{RepoHead, RepoInfo, WindowLabel};

const ABBREVIATIONS: [&str; 9] = ["e.g", "i.e", "etc", "vs", "cf", "approx", "mr", "mrs", "dr"];

/// Line 1 of a card: `<window index> · <window name>` inside tmux, else
/// `<repo> · <branch>`, else the directory name.
#[must_use]
pub fn location_line(window: Option<&WindowLabel>, repo: Option<&RepoInfo>, cwd: &Path) -> String {
    if let Some(window) = window {
        return if window.name.is_blank() {
            window.index.to_string()
        } else {
            format!("{} · {}", window.index, window.name.as_str())
        };
    }
    match repo {
        Some(RepoInfo { main_repo_name, head: RepoHead::Branch { name } }) => {
            format!("{} · {}", main_repo_name.as_str(), name.as_str())
        }
        Some(RepoInfo { main_repo_name, head: RepoHead::Detached { .. } }) => {
            main_repo_name.as_str().to_string()
        }
        None => {
            let name = cwd.file_name().map_or_else(
                || cwd.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
            DisplayText::new(&name).as_str().to_string()
        }
    }
}

/// The first sentence of `text`, as a prefix of the trimmed text.
///
/// Ends at `.`, `!` or `?` followed by whitespace or the end (not after a
/// common abbreviation), at a CJK full stop, or at `\n`. A leading code fence
/// runs to its closing fence. The input is [`DisplayText`], which has already
/// turned `\r`, U+2028 and U+2029 into spaces, so `\n` is the only line break.
#[must_use]
pub fn first_sentence(text: &str) -> &str {
    let trimmed = text.trim();
    if trimmed.starts_with("```") {
        return through_closing_fence(trimmed);
    }
    for (index, character) in trimmed.char_indices() {
        let end = index + character.len_utf8();
        match character {
            '\n' => return trimmed[..index].trim_end(),
            '。' | '！' | '？' => return &trimmed[..end],
            '.' | '!' | '?' => {
                let at_boundary = trimmed[end..].chars().next().is_none_or(char::is_whitespace);
                let is_abbreviation = character == '.' && ends_with_abbreviation(&trimmed[..index]);
                if at_boundary && !is_abbreviation {
                    return &trimmed[..end];
                }
            }
            _ => {}
        }
    }
    trimmed
}

fn ends_with_abbreviation(before: &str) -> bool {
    let word = before.rsplit(char::is_whitespace).next().unwrap_or("");
    let word = word.trim_start_matches(|character: char| !character.is_alphanumeric());
    ABBREVIATIONS.iter().any(|abbreviation| word.eq_ignore_ascii_case(abbreviation))
}

fn through_closing_fence(trimmed: &str) -> &str {
    let body_start = trimmed.find('\n').map_or(trimmed.len(), |index| index + 1);
    match trimmed[body_start..].find("```") {
        Some(offset) => &trimmed[..body_start + offset + 3],
        None => trimmed,
    }
}

/// The body of a `TurnEnded` card.
#[must_use]
pub fn turn_ended_body(last_message: Option<&DisplayText>) -> DisplayText {
    let sentence = last_message.map_or("", |message| first_sentence(message.as_str()));
    if sentence.is_empty() { DisplayText::new("Finished") } else { DisplayText::new(sentence) }
}

/// The body of a `TurnFailed` card.
#[must_use]
pub fn turn_failed_body(error: &DisplayText) -> DisplayText {
    DisplayText::new(&format!("Stopped: {}", error.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::RepoHead;
    use proptest::prelude::*;

    fn window(index: u32, name: &str) -> WindowLabel {
        WindowLabel { index, name: DisplayText::new(name) }
    }

    fn repo(head: RepoHead) -> RepoInfo {
        RepoInfo { main_repo_name: DisplayText::new("code"), head }
    }

    #[test]
    fn location_prefers_the_tmux_window() {
        let branch = repo(RepoHead::Branch { name: DisplayText::new("main") });
        assert_eq!(location_line(Some(&window(4, "code/feature-x")), Some(&branch), Path::new("/x")), "4 · code/feature-x");
        assert_eq!(location_line(Some(&window(4, "  ")), None, Path::new("/x")), "4");
    }

    #[test]
    fn location_outside_tmux_uses_repo_and_branch() {
        let branch = repo(RepoHead::Branch { name: DisplayText::new("feature-x") });
        let detached = repo(RepoHead::Detached { short_sha: DisplayText::new("abc1234") });
        assert_eq!(location_line(None, Some(&branch), Path::new("/x")), "code · feature-x");
        assert_eq!(location_line(None, Some(&detached), Path::new("/x")), "code");
        assert_eq!(location_line(None, None, Path::new("/home/dev/notes")), "notes");
        assert_eq!(location_line(None, None, Path::new("/")), "/");
    }

    #[test]
    fn first_sentence_cases() {
        let cases = [
            ("Done. All 41 tests pass.", "Done."),
            ("  Fixed it  ", "Fixed it"),
            ("No punctuation at all", "No punctuation at all"),
            ("Use a map, e.g. a BTreeMap. Then sort.", "Use a map, e.g. a BTreeMap."),
            ("Bumped to v1.2.3 today. Next.", "Bumped to v1.2.3 today."),
            ("See https://example.com/a.b/c for details. More.", "See https://example.com/a.b/c for details."),
            ("Summary line\nSecond line.", "Summary line"),
            ("Summary line \nSecond line.", "Summary line"),
            ("- Fixed X\n- Fixed Y", "- Fixed X"),
            ("完了しました。次へ", "完了しました。"),
            ("Is it green? Yes.", "Is it green?"),
            ("```rust\nfn main() {}\n```\nAfter.", "```rust\nfn main() {}\n```"),
            ("```\nunclosed", "```\nunclosed"),
            ("", ""),
            ("   \n  ", ""),
        ];
        for (input, expected) in cases {
            assert_eq!(first_sentence(input), expected, "input {input:?}");
        }
    }

    #[test]
    fn bodies() {
        assert_eq!(turn_ended_body(None).as_str(), "Finished");
        assert_eq!(turn_ended_body(Some(&DisplayText::new("  "))).as_str(), "Finished");
        assert_eq!(turn_ended_body(Some(&DisplayText::new("Done. More."))).as_str(), "Done.");
        assert_eq!(turn_failed_body(&DisplayText::new("API Error: overloaded")).as_str(), "Stopped: API Error: overloaded");
        let cleaned_bodies = [
            ("Summary line\rSecond line.", "Summary line Second line."),
            ("Summary line\r\nSecond line.", "Summary line"),
            ("Summary line\u{2028}Second line.", "Summary line Second line."),
            ("Done.\u{2029}Next.", "Done."),
            ("Done\u{2029}next.", "Done next."),
        ];
        for (raw, expected) in cleaned_bodies {
            assert_eq!(turn_ended_body(Some(&DisplayText::new(raw))).as_str(), expected, "{raw:?}");
        }
    }

    proptest! {
        #[test]
        fn first_sentence_is_a_non_empty_prefix_of_the_trimmed_input(text in ".{0,300}") {
            let trimmed = text.trim();
            let sentence = first_sentence(&text);
            prop_assert!(trimmed.starts_with(sentence));
            prop_assert_eq!(sentence.is_empty(), trimmed.is_empty());
        }
    }
}
