//! The tmux side of the CLI: one pane read and the tag writes. Every call
//! passes `-S <socket>` and targets a `%pane_id`, so it reaches the server
//! the agent runs in and never a default server.

use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

use agent_notify_core::hook::{PROMPT_TAG_OPTION, SESSION_TAG_OPTION, TagWrite};
use agent_notify_core::wire::WindowLabel;
use agent_notify_core::{DisplayText, PaneId, PaneKey, PromptId, ServerId, SessionId, SocketPath};

use crate::invocation::PaneFacts;

/// `display-message -p` prints other control characters as octal escapes
/// (`\037`), but a tab raw.
const FIELD_SEPARATOR: char = '\t';

/// The socket path from `$TMUX`, which tmux sets to `<socket>,<pid>,<session>`.
/// The socket path can contain commas, so the split runs from the right.
pub fn socket_from_tmux_env(value: &str) -> Option<SocketPath> {
    let mut parts = value.rsplitn(3, ',');
    let (_session_index, _pid, socket) = (parts.next()?, parts.next()?, parts.next()?);
    SocketPath::parse(Path::new(socket)).ok()
}

/// The window name is the last field, so a separator inside a name stays in the name.
fn pane_format() -> String {
    let separator = FIELD_SEPARATOR;
    format!("#{{pid}}{separator}#{{start_time}}{separator}#{{window_index}}{separator}#{{{SESSION_TAG_OPTION}}}{separator}#{{{PROMPT_TAG_OPTION}}}{separator}#{{window_name}}")
}

/// Parses the output of [`read_pane`].
///
/// # Errors
///
/// Returns a message naming the field that is missing or not a number.
pub fn parse_pane_line(line: &str, socket_path: SocketPath, pane: PaneId) -> Result<PaneFacts, String> {
    let line = line.strip_suffix('\n').unwrap_or(line);
    let mut fields = line.splitn(6, FIELD_SEPARATOR);
    let mut next = |name: &str| fields.next().ok_or_else(|| format!("tmux printed no {name} field"));
    let pid = next("pid")?.parse::<u32>().map_err(|error| format!("pid: {error}"))?;
    let start_time = next("start_time")?.parse::<i64>().map_err(|error| format!("start_time: {error}"))?;
    let index = next("window_index")?.parse::<u32>().map_err(|error| format!("window_index: {error}"))?;
    let session_tag = SessionId::parse(next("session tag")?).ok();
    let prompt_tag = PromptId::parse(next("prompt tag")?).ok();
    let name = DisplayText::new(next("window_name")?);
    Ok(PaneFacts {
        key: PaneKey { server: ServerId { socket_path, pid, start_time }, pane },
        window: WindowLabel { index, name },
        session_tag,
        prompt_tag,
    })
}

/// Reads the pane's server identity, window and tags in one tmux call.
///
/// # Errors
///
/// Returns the spawn error, or tmux's stderr when tmux fails.
pub fn read_pane(socket: &SocketPath, pane: &PaneId) -> io::Result<String> {
    run_tmux(socket, &["display-message", "-p", "-t", pane.as_str(), &pane_format()])
}

/// Applies one tag write to `pane`.
///
/// # Errors
///
/// Returns the first tmux failure.
pub fn apply_tag(socket: &SocketPath, pane: &PaneId, tag: &TagWrite) -> io::Result<()> {
    let target = pane.as_str();
    let unset_prompt = ["set-option", "-p", "-q", "-u", "-t", target, PROMPT_TAG_OPTION];
    match tag {
        TagWrite::BindSession(session) => {
            run_tmux(socket, &["set-option", "-p", "-t", target, SESSION_TAG_OPTION, session.as_str()])?;
            run_tmux(socket, &unset_prompt)?;
        }
        TagWrite::BindPrompt(prompt) => {
            run_tmux(socket, &["set-option", "-p", "-t", target, PROMPT_TAG_OPTION, prompt.as_str()])?;
        }
        TagWrite::UnbindSession => {
            run_tmux(socket, &["set-option", "-p", "-q", "-u", "-t", target, SESSION_TAG_OPTION])?;
            run_tmux(socket, &unset_prompt)?;
        }
    }
    Ok(())
}

fn run_tmux(socket: &SocketPath, arguments: &[&str]) -> io::Result<String> {
    let output = Command::new("tmux")
        .arg("-S")
        .arg(socket.as_path())
        .args(arguments)
        .stdin(Stdio::null())
        .output()?;
    if !output.status.success() {
        let command = arguments.first().copied().unwrap_or("");
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::other(format!("tmux {command} failed: {}", stderr.trim())));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn socket() -> SocketPath {
        SocketPath::parse(Path::new("/tmp/tmux-501/default")).expect("absolute")
    }

    fn pane() -> PaneId {
        PaneId::parse("%12").expect("pane")
    }

    fn line(fields: &[&str]) -> String {
        let mut joined = fields.join(&FIELD_SEPARATOR.to_string());
        joined.push('\n');
        joined
    }

    #[test]
    fn the_tmux_variable_names_the_socket() {
        assert_eq!(socket_from_tmux_env("/private/tmp/tmux-501/default,1799,0"), SocketPath::parse(Path::new("/private/tmp/tmux-501/default")).ok());
        assert_eq!(socket_from_tmux_env("/tmp/a,b/sock,1,0"), SocketPath::parse(Path::new("/tmp/a,b/sock")).ok());
        assert_eq!(socket_from_tmux_env("garbage"), None);
        assert_eq!(socket_from_tmux_env("relative/sock,1,0"), None);
    }

    #[test]
    fn a_pane_line_parses_into_facts() {
        let facts = parse_pane_line(&line(&["1799", "1760400000", "4", "session-a", "prompt-1", "code/feature-x"]), socket(), pane()).expect("parses");
        assert_eq!(facts.key.server.pid, 1799);
        assert_eq!(facts.key.server.start_time, 1_760_400_000);
        assert_eq!(facts.window.index, 4);
        assert_eq!(facts.window.name.as_str(), "code/feature-x");
        assert_eq!(facts.session_tag.as_ref().map(SessionId::as_str), Some("session-a"));
        assert_eq!(facts.prompt_tag.as_ref().map(PromptId::as_str), Some("prompt-1"));
    }

    #[test]
    fn window_name_with_separator_and_newline_parses() {
        let name = format!("a{FIELD_SEPARATOR}b\nc");
        let facts = parse_pane_line(&line(&["1", "2", "3", "", "", &name]), socket(), pane()).expect("parses");
        assert_eq!(facts.window.name.as_str(), "ab\nc", "the separator is a control character, so DisplayText drops it");
        assert_eq!(facts.window.index, 3);
    }

    #[test]
    fn empty_or_invalid_tags_are_absent() {
        let facts = parse_pane_line(&line(&["1", "2", "3", "", "bad tag;", "w"]), socket(), pane()).expect("parses");
        assert_eq!(facts.session_tag, None);
        assert_eq!(facts.prompt_tag, None);
    }

    #[test]
    fn bad_numbers_are_errors() {
        assert!(parse_pane_line(&line(&["x", "2", "3", "", "", "w"]), socket(), pane()).is_err());
        assert!(parse_pane_line("1\t2", socket(), pane()).is_err());
    }
}
