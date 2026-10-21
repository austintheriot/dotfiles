//! The plain OS notification shown when the daemon does not acknowledge.
//! Text goes in as separate argv elements, never through a shell.

use std::io;
use std::process::{Command, Stdio};
use std::time::Duration;

use thumper_core::fallback::FallbackNote;
use thumper_core::paths::Platform;

use crate::child::wait_bounded;

const ALACRITTY_BUNDLE_ID: &str = "org.alacritty";
const NOTIFIER_LIMIT: Duration = Duration::from_secs(2);

/// Shows `note` with the platform's notifier and waits for it to exit.
///
/// # Errors
///
/// Returns the spawn error, or an error naming the notifier when it fails.
pub fn notify(platform: Platform, note: &FallbackNote) -> io::Result<()> {
    let Some((program, arguments)) = argv(platform, note) else {
        return Err(io::Error::other("no fallback notifier on this platform"));
    };
    let mut child = Command::new(program).args(&arguments).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;
    let status = wait_bounded(&mut child, NOTIFIER_LIMIT).map_err(|error| io::Error::new(error.kind(), format!("{program}: {error}")))?;
    if status.success() { Ok(()) } else { Err(io::Error::other(format!("{program} exited with {status}"))) }
}

/// The notifier program and its arguments for `note`, or `None` when the
/// platform has no notifier.
pub fn argv(platform: Platform, note: &FallbackNote) -> Option<(&'static str, Vec<String>)> {
    match platform {
        Platform::MacOs => Some((
            "terminal-notifier",
            vec![
                "-title".to_string(),
                guard_leading_dash(&note.title),
                "-subtitle".to_string(),
                guard_leading_dash(note.subtitle),
                "-message".to_string(),
                guard_leading_dash(&note.message),
                "-activate".to_string(),
                ALACRITTY_BUNDLE_ID.to_string(),
            ],
        )),
        Platform::Linux => Some((
            "notify-send",
            vec![
                "--app-name=thumper".to_string(),
                if note.urgent { "--urgency=critical" } else { "--urgency=normal" }.to_string(),
                "--".to_string(),
                neutralize_markup(&note.title),
                neutralize_markup(&format!("{}\n{}", note.subtitle, note.message)),
            ],
        )),
        Platform::Other => None,
    }
}

/// terminal-notifier reads options through the Cocoa argument domain, which
/// takes a value that starts with `-` as the next option's name. A leading
/// space keeps agent text a value.
pub fn guard_leading_dash(text: &str) -> String {
    if text.starts_with('-') { format!(" {text}") } else { text.to_string() }
}

/// Notification servers disagree on body markup. COSMIC parses tags, cuts
/// the body at a bare `<`, and shows entities literally, so no escaping works
/// on every server. Fullwidth look-alikes render the same everywhere.
pub fn neutralize_markup(text: &str) -> String {
    text.chars()
        .map(|character| match character {
            '<' => '\u{ff1c}',
            '>' => '\u{ff1e}',
            '&' => '\u{ff06}',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use thumper_core::fallback::OFFLINE_SUBTITLE;
    use proptest::prelude::*;

    #[test]
    fn leading_dash_is_guarded() {
        assert_eq!(guard_leading_dash("-execute rm -rf ~"), " -execute rm -rf ~");
        assert_eq!(guard_leading_dash("ok -execute"), "ok -execute");
    }

    #[test]
    fn markup_characters_become_look_alikes() {
        assert_eq!(neutralize_markup(r#"<b>"a" & 'b'</b> a < b"#), "\u{ff1c}b\u{ff1e}\"a\" \u{ff06} 'b'\u{ff1c}/b\u{ff1e} a \u{ff1c} b");
        assert_eq!(neutralize_markup("plain text"), "plain text");
    }

    #[test]
    fn other_platforms_have_no_notifier() {
        let note = FallbackNote { title: "t".to_string(), subtitle: OFFLINE_SUBTITLE, message: "m".to_string(), urgent: false };
        assert_eq!(argv(Platform::Other, &note), None);
    }

    fn value_after<'list>(arguments: &'list [String], flag: &str) -> Option<&'list String> {
        let at = arguments.iter().position(|argument| argument == flag)?;
        arguments.get(at + 1)
    }

    fn reaches_argv_guarded(value: &str, text: &str) -> bool {
        value == text || (text.starts_with('-') && value == format!(" {text}"))
    }

    fn dangerous_text() -> impl Strategy<Value = String> {
        prop_oneof![any::<String>(), any::<String>().prop_map(|text| format!("-{text}")), "[-<>&\n]{1,3}.*"]
    }

    proptest! {
        #[test]
        fn macos_argv_keeps_agent_text_as_values(title in dangerous_text(), message in dangerous_text(), urgent in any::<bool>()) {
            let note = FallbackNote { title: title.clone(), subtitle: OFFLINE_SUBTITLE, message: message.clone(), urgent };
            let Some((program, arguments)) = argv(Platform::MacOs, &note) else {
                return Err(TestCaseError::fail("macOS has a notifier"));
            };
            prop_assert_eq!(program, "terminal-notifier");
            for flag in ["-title", "-subtitle", "-message"] {
                let value = value_after(&arguments, flag);
                prop_assert!(value.is_some_and(|value| !value.starts_with('-')), "{flag} value {value:?}");
            }
            prop_assert!(value_after(&arguments, "-title").is_some_and(|value| reaches_argv_guarded(value, &title)));
            prop_assert!(value_after(&arguments, "-message").is_some_and(|value| reaches_argv_guarded(value, &message)));
            prop_assert_eq!(value_after(&arguments, "-subtitle").map(String::as_str), Some(OFFLINE_SUBTITLE));
            prop_assert!(!arguments.iter().any(|argument| argument == "-execute"));
        }

        #[test]
        fn linux_argv_ends_options_and_neutralizes_markup(title in dangerous_text(), message in dangerous_text(), urgent in any::<bool>()) {
            let note = FallbackNote { title: title.clone(), subtitle: OFFLINE_SUBTITLE, message: message.clone(), urgent };
            let Some((program, arguments)) = argv(Platform::Linux, &note) else {
                return Err(TestCaseError::fail("Linux has a notifier"));
            };
            prop_assert_eq!(program, "notify-send");
            let end_of_options = arguments.iter().position(|argument| argument == "--");
            prop_assert_eq!(end_of_options.map(|at| at + 3), Some(arguments.len()), "{:?}", arguments);
            let positionals = &arguments[arguments.len() - 2..];
            for positional in positionals {
                prop_assert!(!positional.contains(['<', '>', '&']), "{positional:?}");
            }
            prop_assert_eq!(&positionals[0], &neutralize_markup(&title));
            prop_assert_eq!(&positionals[1], &neutralize_markup(&format!("{OFFLINE_SUBTITLE}\n{message}")));
            let urgency = if urgent { "--urgency=critical" } else { "--urgency=normal" };
            prop_assert!(arguments.iter().any(|argument| argument == urgency));
        }
    }
}
