//! Idle time, screen lock and the frontmost app, read without `unsafe`.
//! The parsers are pure; [`IoregPresence`] runs `/usr/sbin/ioreg`.

use std::process::Command;

use agent_notify_core::update::Frontmost;

use crate::child::{CHILD_LIMIT, run_bounded};
use crate::services::{Presence, PresenceReader};

/// Alacritty's bundle id.
pub const TERMINAL_BUNDLE_ID: &str = "org.alacritty";
const IOREG: &str = "/usr/sbin/ioreg";
const CONSOLE_USERS_KEY: &str = "\"IOConsoleUsers\"";
const ON_CONSOLE_KEY: &str = "\"kCGSSessionOnConsoleKey\"";
const SCREEN_LOCKED_KEY: &str = "\"CGSSessionScreenIsLocked\"";

/// Seconds since the last input, from `ioreg -c IOHIDSystem -d 4`.
#[must_use]
pub fn parse_hid_idle_seconds(ioreg: &str) -> Option<u32> {
    let line = ioreg.lines().find(|line| line.contains("\"HIDIdleTime\""))?;
    let nanoseconds: u64 = line.split('=').nth(1)?.trim().parse().ok()?;
    u32::try_from(nanoseconds / 1_000_000_000).ok()
}

/// Screen lock, from `ioreg -n Root -d 1`. Only the user record with
/// `kCGSSessionOnConsoleKey=Yes` counts: another logged-in user's locked
/// session says nothing about this screen. That record carries
/// `CGSSessionScreenIsLocked` only while the screen is locked. `None` when
/// there is no console user list or no user on the console.
#[must_use]
pub fn parse_screen_locked(ioreg: &str) -> Option<bool> {
    let line = ioreg.lines().find(|line| line.trim_start().starts_with(CONSOLE_USERS_KEY))?;
    let (_, list) = line.split_once('=')?;
    let console = user_records(list).into_iter().find(|record| field(record, ON_CONSOLE_KEY) == Some("Yes"))?;
    Some(field(console, SCREEN_LOCKED_KEY) == Some("Yes"))
}

fn unquoted(text: &str) -> impl Iterator<Item = (usize, char)> + '_ {
    text.char_indices()
        .scan(false, |in_quotes, (index, character)| {
            if character == '"' {
                *in_quotes = !*in_quotes;
            }
            Some((index, character, *in_quotes))
        })
        .filter(|(_, character, in_quotes)| !in_quotes && *character != '"')
        .map(|(index, character, _)| (index, character))
}

fn user_records(list: &str) -> Vec<&str> {
    let braces: Vec<(usize, char)> = unquoted(list).filter(|(_, character)| matches!(character, '{' | '}')).collect();
    braces
        .windows(2)
        .filter_map(|pair| match pair {
            [(start, '{'), (end, '}')] => Some(&list[start + 1..*end]),
            _ => None,
        })
        .collect()
}

fn field<'record>(record: &'record str, key: &str) -> Option<&'record str> {
    let commas: Vec<usize> = unquoted(record).filter(|(_, character)| *character == ',').map(|(index, _)| index).collect();
    let starts = std::iter::once(0).chain(commas.iter().map(|comma| comma + 1));
    let ends = commas.iter().copied().chain(std::iter::once(record.len()));
    starts.zip(ends).find_map(|(start, end)| {
        let (name, value) = record[start..end].split_once('=')?;
        (name.trim() == key).then(|| value.trim())
    })
}

/// Classifies the frontmost app by bundle id.
#[must_use]
pub fn frontmost_from_bundle(bundle: Option<&str>) -> Frontmost {
    match bundle {
        Some(TERMINAL_BUNDLE_ID) => Frontmost::Terminal,
        Some(_) => Frontmost::Other,
        None => Frontmost::Unknown,
    }
}

/// Reads presence with two `ioreg` runs, about 20 ms together.
#[derive(Debug, Clone, Copy, Default)]
pub struct IoregPresence;

impl PresenceReader for IoregPresence {
    fn presence(&self) -> Presence {
        Presence {
            idle_seconds: ioreg(&["-c", "IOHIDSystem", "-d", "4"]).as_deref().and_then(parse_hid_idle_seconds),
            screen_locked: ioreg(&["-n", "Root", "-d", "1"]).as_deref().and_then(parse_screen_locked),
        }
    }
}

fn ioreg(arguments: &[&str]) -> Option<String> {
    match run_bounded(Command::new(IOREG).args(arguments), CHILD_LIMIT) {
        Ok(finished) if finished.status.success() => Some(String::from_utf8_lossy(&finished.stdout).into_owned()),
        Ok(finished) => {
            tracing::debug!(status = ?finished.status, "ioreg failed");
            None
        }
        Err(error) => {
            tracing::debug!(%error, "ioreg did not finish");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use agent_notify_core::update::Frontmost;

    use super::*;

    const IDLE: &str = "    | |   \"HIDIdleTime\" = 345156583\n    | |   \"HIDParameters\" = {}\n";
    const UNLOCKED: &str = "+-o Root  <class IORegistryEntry>\n    \"IOConsoleUsers\" = ({\"kCGSSessionOnConsoleKey\"=Yes,\"kCGSSessionUserIDKey\"=501})\n";
    const LOCKED: &str =
        "+-o Root  <class IORegistryEntry>\n    \"IOConsoleUsers\" = ({\"CGSSessionScreenIsLocked\"=Yes,\"kCGSSessionOnConsoleKey\"=Yes})\n";
    const OFF_CONSOLE_USER_LOCKED: &str = "+-o Root  <class IORegistryEntry>\n    \"IOConsoleUsers\" = ({\"kCGSSessionUserNameKey\"=\"second, user\",\"CGSSessionScreenIsLocked\"=Yes,\"kCGSSessionOnConsoleKey\"=No,\"kCGSSessionUserIDKey\"=502},{\"kCGSSessionOnConsoleKey\"=Yes,\"kCGSSessionUserNameKey\"=\"first\",\"kCGSSessionUserIDKey\"=501})\n";
    const ON_CONSOLE_USER_LOCKED: &str = "+-o Root  <class IORegistryEntry>\n    \"IOConsoleUsers\" = ({\"kCGSSessionOnConsoleKey\"=No,\"kCGSSessionUserIDKey\"=502},{\"kCGSSessionOnConsoleKey\"=Yes,\"CGSSessionScreenIsLocked\"=Yes,\"kCGSSessionUserIDKey\"=501})\n";
    const NO_CONSOLE_USER: &str = "+-o Root  <class IORegistryEntry>\n    \"IOConsoleUsers\" = ({\"kCGSSessionOnConsoleKey\"=No,\"CGSSessionScreenIsLocked\"=Yes,\"kCGSSessionUserIDKey\"=502})\n";
    const NAME_HOLDS_A_BRACE: &str = "+-o Root  <class IORegistryEntry>\n    \"IOConsoleUsers\" = ({\"kCGSSessionUserNameKey\"=\"a},{b\",\"kCGSSessionOnConsoleKey\"=Yes,\"CGSSessionScreenIsLocked\"=Yes})\n";

    #[test]
    fn idle_time_is_nanoseconds_rounded_down_to_seconds() {
        assert_eq!(parse_hid_idle_seconds(IDLE), Some(0));
        assert_eq!(parse_hid_idle_seconds("\"HIDIdleTime\" = 125000000000"), Some(125));
    }

    #[test]
    fn unreadable_idle_time_is_none() {
        assert_eq!(parse_hid_idle_seconds(""), None);
        assert_eq!(parse_hid_idle_seconds("\"HIDIdleTime\" = <data>"), None);
        assert_eq!(parse_hid_idle_seconds("\"HIDIdleTime\" = 99999999999999999999999"), None);
    }

    #[test]
    fn screen_lock_reads_the_console_user_record() {
        assert_eq!(parse_screen_locked(LOCKED), Some(true));
        assert_eq!(parse_screen_locked(UNLOCKED), Some(false));
        assert_eq!(parse_screen_locked("+-o Root\n"), None);
    }

    #[test]
    fn a_locked_session_off_the_console_does_not_read_as_locked() {
        assert_eq!(parse_screen_locked(OFF_CONSOLE_USER_LOCKED), Some(false));
        assert_eq!(parse_screen_locked(ON_CONSOLE_USER_LOCKED), Some(true));
    }

    #[test]
    fn no_user_on_the_console_is_unknown() {
        assert_eq!(parse_screen_locked(NO_CONSOLE_USER), None);
        assert_eq!(parse_screen_locked("    \"IOConsoleUsers\" = ()\n"), None);
    }

    #[test]
    fn a_brace_inside_a_quoted_value_does_not_split_the_record() {
        assert_eq!(parse_screen_locked(NAME_HOLDS_A_BRACE), Some(true));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_real_ioreg_reports_an_idle_time() {
        assert!(IoregPresence.presence().idle_seconds.is_some());
    }

    #[test]
    fn the_terminal_is_alacritty_by_bundle_id() {
        assert_eq!(frontmost_from_bundle(Some("org.alacritty")), Frontmost::Terminal);
        assert_eq!(frontmost_from_bundle(Some("com.apple.finder")), Frontmost::Other);
        assert_eq!(frontmost_from_bundle(None), Frontmost::Unknown);
    }
}
