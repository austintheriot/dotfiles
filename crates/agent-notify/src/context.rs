//! Gathers [`Inputs`] from the environment, stdin, tmux and git. The only IO
//! before the planner runs.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_notify_core::paths::{PathInputs, Platform};
use agent_notify_core::wire::{RepoHead, RepoInfo};
use agent_notify_core::{DisplayText, EpochMs, EventId, PaneId};
use tmux_core::{HeadState, RepositoryFacts};

use crate::cli::Command;
use crate::invocation::{Inputs, PaneLookup};
use crate::tmux;

/// A hook payload larger than this is cut. Claude Code payloads are a few KB.
const MAX_STDIN_BYTES: u64 = 4 * 1024 * 1024;

/// The environment variables the CLI reads, read once.
#[derive(Debug, Clone, Default)]
pub struct EnvSnapshot {
    /// `$TMUX`.
    pub tmux: Option<String>,
    /// `$TMUX_PANE`.
    pub tmux_pane: Option<String>,
    /// `$HOME`.
    pub home: Option<PathBuf>,
    /// `$XDG_RUNTIME_DIR`.
    pub xdg_runtime_dir: Option<PathBuf>,
    /// `$XDG_STATE_HOME`.
    pub xdg_state_home: Option<PathBuf>,
    /// `$AGENT_NOTIFY_SOCKET`.
    pub socket_override: Option<PathBuf>,
    /// `$AGENT_NOTIFY_LOG_DIR`.
    pub log_dir_override: Option<PathBuf>,
    /// `$AGENT_NOTIFY_LOG`: `error`, `warn`, `info`, `debug` or `trace`.
    pub log_level: Option<String>,
    /// `$AGENT_NOTIFY_TRACE=1`: log the raw hook payload.
    pub trace: bool,
}

impl EnvSnapshot {
    /// Reads the variables. Empty values count as unset.
    pub fn read() -> Self {
        let text = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
        let path = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty()).map(PathBuf::from);
        EnvSnapshot {
            tmux: text("TMUX"),
            tmux_pane: text("TMUX_PANE"),
            home: path("HOME"),
            xdg_runtime_dir: path("XDG_RUNTIME_DIR"),
            xdg_state_home: path("XDG_STATE_HOME"),
            socket_override: path("AGENT_NOTIFY_SOCKET"),
            log_dir_override: path("AGENT_NOTIFY_LOG_DIR"),
            log_level: text("AGENT_NOTIFY_LOG"),
            trace: text("AGENT_NOTIFY_TRACE").as_deref() == Some("1"),
        }
    }

    /// The facts the core's path rules read.
    pub fn path_inputs(&self) -> PathInputs<'_> {
        PathInputs {
            platform: Platform::current(),
            home: self.home.as_deref(),
            xdg_runtime_dir: self.xdg_runtime_dir.as_deref(),
            xdg_state_home: self.xdg_state_home.as_deref(),
            socket_override: self.socket_override.as_deref(),
            log_dir_override: self.log_dir_override.as_deref(),
        }
    }
}

/// Reads everything the planner needs.
pub fn gather(command: &Command, env: &EnvSnapshot, now: EpochMs, random: [u8; 10]) -> Inputs {
    let stdin = command.is_hook().then(read_stdin);
    let pane = lookup_pane(env);
    let cwd = std::env::current_dir().unwrap_or_default();
    let repo = match pane {
        PaneLookup::Found(_) => None,
        PaneLookup::NotInTmux | PaneLookup::Unreadable { .. } => tmux_tools::repo::inspect(&repo_directory(stdin.as_deref(), &cwd)).map(repo_info),
    };
    Inputs { now, event_id: EventId::from_parts(now, random), cwd, pane, repo, stdin }
}

/// The directory whose git facts go on the card: the hook payload's `cwd`
/// when it has one, so the repository and the cwd line name one directory.
fn repo_directory(stdin: Option<&str>, process_cwd: &Path) -> PathBuf {
    stdin
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .and_then(|payload| payload.get("cwd").and_then(serde_json::Value::as_str).filter(|cwd| !cwd.is_empty()).map(PathBuf::from))
        .unwrap_or_else(|| process_cwd.to_path_buf())
}

/// The wall clock, read once per invocation.
pub fn clock() -> EpochMs {
    let millis = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis());
    EpochMs(i64::try_from(millis).unwrap_or(i64::MAX))
}

/// 80 random bits for the event id. Falls back to the pid, which keeps ids
/// unique across concurrent hooks in the same millisecond.
pub fn random_bytes() -> [u8; 10] {
    let mut bytes = [0_u8; 10];
    if File::open("/dev/urandom").and_then(|mut source| source.read_exact(&mut bytes)).is_err() {
        bytes[..4].copy_from_slice(&std::process::id().to_be_bytes());
    }
    bytes
}

fn read_stdin() -> String {
    let mut bytes = Vec::new();
    match std::io::stdin().take(MAX_STDIN_BYTES).read_to_end(&mut bytes) {
        Ok(_) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(error) => {
            tracing::warn!(error = %error, "stdin read failed");
            String::new()
        }
    }
}

fn lookup_pane(env: &EnvSnapshot) -> PaneLookup {
    let (Some(tmux_value), Some(pane_value)) = (&env.tmux, &env.tmux_pane) else {
        return PaneLookup::NotInTmux;
    };
    let Some(socket) = tmux::socket_from_tmux_env(tmux_value) else {
        return PaneLookup::Unreadable { reason: format!("TMUX is not <socket>,<pid>,<session>: {tmux_value:?}") };
    };
    let pane = match PaneId::parse(pane_value) {
        Ok(pane) => pane,
        Err(error) => return PaneLookup::Unreadable { reason: error.to_string() },
    };
    match tmux::read_pane(&socket, &pane) {
        Ok(line) => tmux::parse_pane_line(&line, socket, pane).map_or_else(|reason| PaneLookup::Unreadable { reason }, PaneLookup::Found),
        Err(error) => PaneLookup::Unreadable { reason: error.to_string() },
    }
}

fn repo_info(facts: RepositoryFacts) -> RepoInfo {
    let head = match facts.head {
        HeadState::Branch(name) => RepoHead::Branch { name: DisplayText::new(&name) },
        HeadState::Detached { short_sha } => RepoHead::Detached { short_sha: DisplayText::new(&short_sha) },
    };
    RepoInfo { main_repo_name: DisplayText::new(&facts.main_repo_name), head }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_facts_come_from_the_payload_cwd_when_it_has_one() {
        let stdin = r#"{"session_id":"session-a","cwd":"/home/dev/other","hook_event_name":"Stop"}"#;
        assert_eq!(repo_directory(Some(stdin), Path::new("/home/dev/code")), PathBuf::from("/home/dev/other"));
    }

    #[test]
    fn git_facts_come_from_the_process_cwd_otherwise() {
        for stdin in [None, Some(""), Some("not json"), Some(r#"{"hook_event_name":"Stop"}"#), Some(r#"{"cwd":""}"#), Some(r#"{"cwd":7}"#)] {
            assert_eq!(repo_directory(stdin, Path::new("/home/dev/code")), PathBuf::from("/home/dev/code"), "{stdin:?}");
        }
    }
}
