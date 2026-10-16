//! `agent-notify doctor`: gathers facts about the installation, then checks
//! them with pure rules.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use agent_notify_core::paths::{Platform, SocketLocation, log_directory, socket_location, state_directory};
use agent_notify_core::status::StatusReport;
use agent_notify_core::wire::encode_line;
use agent_notify_core::{DisplayText, PaneId};

use crate::child::wait_bounded;
use crate::client;
use crate::context::{self, EnvSnapshot};
use crate::invocation::{self, QUERY_BUDGET};
use crate::log::LOG_FILE;
use crate::{render, tmux};

/// The plugin's key in `enabledPlugins`.
pub const PLUGIN_KEY: &str = "agent-notify@dotfiles-tools";
/// The launchd label and bundle id of the daemon. The TypeScript app holds a
/// second copy, and a test ties the two together.
pub const LAUNCHD_LABEL: &str = "dev.dotfiles.agent-notifyd";
const PERMISSION_RULES: [&str; 2] = ["Bash(agent-notify send:*)", "Bash(agent-notify quiet:*)"];
const CLI_LOG_WARN_BYTES: u64 = 50 * 1024 * 1024;
const MEBIBYTE: u64 = 1024 * 1024;
const CHILD_LIMIT: Duration = Duration::from_secs(2);

/// Whether launchd runs the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchdState {
    /// Loaded in the user's GUI domain.
    Loaded,
    /// Not loaded.
    NotLoaded,
    /// Turned off in Login Items.
    Disabled,
    /// `launchctl` or `id` did not answer in time, or did not start.
    Unknown,
}

/// The CLI log file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogFile {
    /// Where the file is.
    pub path: PathBuf,
    /// Its size in bytes.
    pub bytes: u64,
}

/// Everything the checks read.
#[derive(Debug, Clone)]
pub struct DoctorFacts {
    /// This machine's platform.
    pub platform: Platform,
    /// The first `agent-notify` on PATH.
    pub cli_on_path: Option<PathBuf>,
    /// This binary.
    pub cli_self: Option<PathBuf>,
    /// The socket location.
    pub socket: SocketLocation,
    /// The daemon's status report, or why there is none.
    pub status: Result<StatusReport, String>,
    /// The stamp the last install wrote.
    pub installed_stamp: Option<String>,
    /// The text of `~/.claude/settings.json`.
    pub settings: Option<String>,
    /// `$TMUX_PANE` is set.
    pub in_tmux_pane: bool,
    /// Whether this pane has a session tag. `None` outside tmux or when unreadable.
    pub pane_session_tag: Option<bool>,
    /// tmux's `focus-events`. `None` outside tmux.
    pub focus_events: Option<bool>,
    /// terminal-notifier or notify-send.
    pub fallback_notifier: Option<PathBuf>,
    /// The launchd agent. `None` off macOS.
    pub launchd: Option<LaunchdState>,
    /// The CLI log. `None` when there is no log yet.
    pub cli_log: Option<LogFile>,
}

/// A check's result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    /// Fine.
    Pass,
    /// Works, with a gap.
    Warn,
    /// Broken.
    Fail,
}

/// One check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// Short name.
    pub name: &'static str,
    /// Result.
    pub status: CheckStatus,
    /// What was found, and what to do. Safe to print on one line.
    pub detail: String,
}

fn check(name: &'static str, status: CheckStatus, detail: impl Into<String>) -> Check {
    let detail = detail.into();
    Check { name, status, detail: DisplayText::new(&detail).as_str().replace('\n', " ") }
}

/// Runs every rule over `facts`.
pub fn evaluate(facts: &DoctorFacts) -> Vec<Check> {
    let mut checks = vec![cli_check(facts), socket_check(&facts.socket), daemon_check(&facts.status)];
    if let Ok(report) = &facts.status {
        checks.push(current_check(report, facts.installed_stamp.as_deref()));
        checks.push(tools_check(report, facts.platform));
    }
    checks.extend(settings_checks(facts.settings.as_deref()));
    checks.extend(tmux_checks(facts));
    checks.push(match &facts.fallback_notifier {
        Some(path) => check("fallback notifier", CheckStatus::Pass, path.display().to_string()),
        None => check("fallback notifier", CheckStatus::Warn, "terminal-notifier or notify-send not found: no notification when the daemon is down"),
    });
    if let Some(state) = facts.launchd {
        checks.push(login_item_check(state));
    }
    checks.push(cli_log_check(facts.cli_log.as_ref()));
    checks
}

fn cli_check(facts: &DoctorFacts) -> Check {
    match (&facts.cli_on_path, &facts.cli_self) {
        (None, _) => check("cli on PATH", CheckStatus::Fail, "agent-notify is not on PATH: hooks call ~/.local/bin/agent-notify, run `config build`"),
        (Some(found), Some(this)) if found != this => {
            check("cli on PATH", CheckStatus::Warn, format!("PATH resolves {}, but this is {}", found.display(), this.display()))
        }
        (Some(found), _) => check("cli on PATH", CheckStatus::Pass, found.display().to_string()),
    }
}

fn socket_check(socket: &SocketLocation) -> Check {
    match socket {
        SocketLocation::At(path) => check("socket", CheckStatus::Pass, path.display().to_string()),
        SocketLocation::Unavailable(reason) => check("socket", CheckStatus::Fail, format!("no socket path: {reason}")),
    }
}

fn daemon_check(status: &Result<StatusReport, String>) -> Check {
    match status {
        Ok(report) => check(
            "daemon answers",
            CheckStatus::Pass,
            format!("stamp {}, up {}", report.daemon.stamp, render::duration_text(report.now_ms.since(report.daemon.started_ms))),
        ),
        Err(problem) => check("daemon answers", CheckStatus::Fail, format!("{problem}: run `config build --tools`")),
    }
}

fn current_check(report: &StatusReport, installed: Option<&str>) -> Check {
    match installed {
        Some(stamp) if stamp == report.daemon.stamp => check("daemon is current", CheckStatus::Pass, stamp.to_string()),
        Some(stamp) => check(
            "daemon is current",
            CheckStatus::Warn,
            format!(
                "running {}, installed {stamp}: run `launchctl kickstart -k gui/$(id -u)/{LAUNCHD_LABEL}`",
                report.daemon.stamp
            ),
        ),
        None => check("daemon is current", CheckStatus::Warn, "no install stamp: run `config build --tools`"),
    }
}

fn tools_check(report: &StatusReport, platform: Platform) -> Check {
    let missing: Vec<&str> = report.daemon.tools.iter().filter(|tool| tool.path.is_none()).map(|tool| tool.name.as_str()).collect();
    if missing.contains(&"tmux") {
        return check("daemon tools", CheckStatus::Fail, "the daemon cannot find tmux: clicks cannot switch panes; set AGENT_NOTIFY_TMUX in the launchd plist");
    }
    match (missing.is_empty(), platform) {
        (true, _) => check("daemon tools", CheckStatus::Pass, "every tool resolves"),
        (false, Platform::MacOs) => {
            check("daemon tools", CheckStatus::Warn, format!("not found: {}; the raise uses app activation only", missing.join(", ")))
        }
        (false, _) => check("daemon tools", CheckStatus::Warn, format!("not found: {}", missing.join(", "))),
    }
}

fn settings_checks(settings: Option<&str>) -> Vec<Check> {
    let Some(value) = settings.and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok()) else {
        return vec![check("plugin enabled", CheckStatus::Fail, "~/.claude/settings.json is missing or not JSON")];
    };
    let enabled = value["enabledPlugins"][PLUGIN_KEY] == serde_json::Value::Bool(true);
    let allowed: Vec<&str> = value["permissions"]["allow"].as_array().map(|rules| rules.iter().filter_map(|rule| rule.as_str()).collect()).unwrap_or_default();
    let missing: Vec<&str> = PERMISSION_RULES.iter().copied().filter(|rule| !allowed.contains(rule)).collect();
    vec![
        if enabled {
            check("plugin enabled", CheckStatus::Pass, PLUGIN_KEY)
        } else {
            check("plugin enabled", CheckStatus::Fail, format!("enabledPlugins has no \"{PLUGIN_KEY}\": true"))
        },
        if missing.is_empty() {
            check("permission rules", CheckStatus::Pass, "send and quiet are allowed")
        } else {
            check("permission rules", CheckStatus::Warn, format!("missing from permissions.allow: {}; agents get a prompt", missing.join(", ")))
        },
    ]
}

fn tmux_checks(facts: &DoctorFacts) -> Vec<Check> {
    if !facts.in_tmux_pane {
        return vec![check("tmux pane", CheckStatus::Warn, "$TMUX_PANE is unset: run doctor inside tmux; outside tmux a click only raises the terminal")];
    }
    let mut checks = vec![check("tmux pane", CheckStatus::Pass, "inside tmux")];
    match facts.pane_session_tag {
        Some(true) => checks.push(check("session tag", CheckStatus::Pass, "this pane has @agent_notify_session")),
        Some(false) => checks.push(check(
            "session tag",
            CheckStatus::Warn,
            "this pane has no @agent_notify_session: start Claude Code here, or check that the SessionStart hook runs",
        )),
        None => checks.push(check("session tag", CheckStatus::Warn, "could not read this pane's tags from tmux")),
    }
    match facts.focus_events {
        Some(true) => checks.push(check("focus events", CheckStatus::Pass, "focus-events is on")),
        Some(false) => checks.push(check("focus events", CheckStatus::Fail, "focus-events is off: add `set -g focus-events on` to tmux.conf")),
        None => checks.push(check("focus events", CheckStatus::Warn, "could not read focus-events from tmux")),
    }
    checks
}

fn login_item_check(state: LaunchdState) -> Check {
    match state {
        LaunchdState::Loaded => check("login item", CheckStatus::Pass, format!("{LAUNCHD_LABEL} is loaded")),
        LaunchdState::NotLoaded => check("login item", CheckStatus::Fail, format!("{LAUNCHD_LABEL} is not loaded: run `config build --tools`")),
        LaunchdState::Disabled => check(
            "login item",
            CheckStatus::Fail,
            "turned off: System Settings > General > Login Items & Extensions > Allow in the Background",
        ),
        LaunchdState::Unknown => check("login item", CheckStatus::Warn, "launchctl did not answer in time: run `launchctl print gui/$(id -u)` by hand"),
    }
}

fn cli_log_check(log: Option<&LogFile>) -> Check {
    match log {
        None => check("cli log", CheckStatus::Pass, "no log yet"),
        Some(file) if file.bytes < CLI_LOG_WARN_BYTES => {
            check("cli log", CheckStatus::Pass, format!("{}, {} MiB", file.path.display(), file.bytes / MEBIBYTE))
        }
        Some(file) => check(
            "cli log",
            CheckStatus::Warn,
            format!("{} is {} MiB: the CLI never rotates it, so delete it when you do not need the history", file.path.display(), file.bytes / MEBIBYTE),
        ),
    }
}

/// True when `launchctl print-disabled` lists `label` as disabled. macOS has
/// printed both `=> disabled` and `=> true` for that.
pub fn parse_disabled(output: &str, label: &str) -> bool {
    let quoted = format!("\"{label}\"");
    output.lines().any(|line| {
        let mut parts = line.trim().splitn(2, "=>");
        parts.next().is_some_and(|key| key.trim() == quoted)
            && parts.next().is_some_and(|value| matches!(value.trim(), "disabled" | "true"))
    })
}

/// One line per check.
pub fn render(checks: &[Check]) -> String {
    checks
        .iter()
        .map(|check| {
            let tag = match check.status {
                CheckStatus::Pass => "ok    ",
                CheckStatus::Warn => "warn  ",
                CheckStatus::Fail => "FAIL  ",
            };
            format!("{tag}{}: {}\n", check.name, check.detail)
        })
        .collect()
}

/// True when any check failed.
pub fn any_failed(checks: &[Check]) -> bool {
    checks.iter().any(|check| check.status == CheckStatus::Fail)
}

/// Reads every fact. Each read that fails becomes `None` or an `Err`, never a panic.
pub fn gather(env: &EnvSnapshot) -> DoctorFacts {
    let platform = Platform::current();
    let socket = socket_location(&env.path_inputs());
    let tmux_socket = env.tmux.as_deref().and_then(tmux::socket_from_tmux_env);
    let pane = env.tmux_pane.as_deref().and_then(|pane| PaneId::parse(pane).ok());
    let pane_session_tag = match (&tmux_socket, &pane) {
        (Some(socket), Some(pane)) => tmux::read_pane(socket, pane)
            .ok()
            .and_then(|line| tmux::parse_pane_line(&line, socket.clone(), pane.clone()).ok())
            .map(|facts| facts.session_tag.is_some()),
        _ => None,
    };
    let focus_events = tmux_socket.as_ref().and_then(|socket| tmux::show_global_option(socket, "focus-events").ok()).map(|value| value.trim() == "on");
    let notifier = match platform {
        Platform::MacOs => "terminal-notifier",
        Platform::Linux | Platform::Other => "notify-send",
    };
    DoctorFacts {
        platform,
        cli_on_path: find_on_path("agent-notify"),
        cli_self: std::env::current_exe().ok(),
        status: query_status(&socket, env),
        socket,
        installed_stamp: state_directory(&env.path_inputs())
            .and_then(|directory| std::fs::read_to_string(directory.join("installed-stamp")).ok())
            .map(|text| text.trim().to_string()),
        settings: env.home.as_ref().and_then(|home| std::fs::read_to_string(home.join(".claude/settings.json")).ok()),
        in_tmux_pane: env.tmux_pane.is_some(),
        pane_session_tag,
        focus_events,
        fallback_notifier: find_on_path(notifier),
        launchd: (platform == Platform::MacOs).then(launchd_state),
        cli_log: cli_log(env),
    }
}

fn query_status(socket: &SocketLocation, env: &EnvSnapshot) -> Result<StatusReport, String> {
    let path = match socket {
        SocketLocation::At(path) => path,
        SocketLocation::Unavailable(reason) => return Err(render::delivery_problem(&client::Delivery::NoSocket(reason))),
    };
    let inputs = context::gather(&crate::cli::Command::Doctor, env, context::clock(), context::random_bytes());
    let line = encode_line(&invocation::status_request(&inputs)).map_err(|error| format!("the request did not encode: {error}"))?;
    render::report_from(&client::deliver(path, &line, QUERY_BUDGET))
}

fn cli_log(env: &EnvSnapshot) -> Option<LogFile> {
    let path = log_directory(&env.path_inputs())?.join(LOG_FILE);
    std::fs::metadata(&path).ok().map(|metadata| LogFile { path, bytes: metadata.len() })
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|directory| directory.join(name)).find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

fn launchd_state() -> LaunchdState {
    let Some(uid) = run_text("/usr/bin/id", &["-u"]).map(|text| text.trim().to_string()) else { return LaunchdState::Unknown };
    let domain = format!("gui/{uid}");
    let disabled = run_text("/bin/launchctl", &["print-disabled", &domain]).map(|output| parse_disabled(&output, LAUNCHD_LABEL));
    let loaded = run_status("/bin/launchctl", &["print", &format!("{domain}/{LAUNCHD_LABEL}")]);
    launchd_from(disabled, loaded)
}

/// `None` means the question got no answer. A loaded agent whose disabled
/// check never ran is `Unknown`, because a Login Items switch can be off.
fn launchd_from(disabled: Option<bool>, loaded: Option<bool>) -> LaunchdState {
    match (disabled, loaded) {
        (Some(true), _) => LaunchdState::Disabled,
        (_, Some(false)) => LaunchdState::NotLoaded,
        (Some(false), Some(true)) => LaunchdState::Loaded,
        (None, Some(true)) | (_, None) => LaunchdState::Unknown,
    }
}

fn run_status(program: &str, arguments: &[&str]) -> Option<bool> {
    let mut child = Command::new(program).args(arguments).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().ok()?;
    wait_bounded(&mut child, CHILD_LIMIT).ok().map(|status| status.success())
}

fn run_text(program: &str, arguments: &[&str]) -> Option<String> {
    let mut child = Command::new(program).args(arguments).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let status = wait_bounded(&mut child, CHILD_LIMIT).ok()?;
    let output = child.wait_with_output().ok()?;
    status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use agent_notify_core::EpochMs;
    use agent_notify_core::paths::{Platform, SocketLocation};
    use agent_notify_core::status::{DaemonFacts, DndStatus, StatusReport, ToolStatus};

    use super::*;

    const GOOD_SETTINGS: &str = r#"{
        "enabledPlugins": { "agent-notify@dotfiles-tools": true },
        "permissions": { "allow": ["Bash(agent-notify send:*)", "Bash(agent-notify quiet:*)"] }
    }"#;

    fn report(stamp: &str, aerospace: Option<&str>) -> StatusReport {
        StatusReport {
            daemon: DaemonFacts {
                stamp: stamp.to_string(),
                started_ms: EpochMs(0),
                tools: vec![
                    ToolStatus { name: "tmux".to_string(), path: Some("/opt/homebrew/bin/tmux".into()) },
                    ToolStatus { name: "aerospace".to_string(), path: aerospace.map(PathBuf::from) },
                ],
            },
            now_ms: EpochMs(1),
            dnd: DndStatus { on: false, until_ms: None },
            sessions: Vec::new(),
            decisions: Vec::new(),
        }
    }

    fn healthy() -> DoctorFacts {
        DoctorFacts {
            platform: Platform::MacOs,
            cli_on_path: Some(PathBuf::from("/home/dev/.local/bin/agent-notify")),
            cli_self: Some(PathBuf::from("/home/dev/.local/bin/agent-notify")),
            socket: SocketLocation::At(PathBuf::from("/home/dev/Library/Application Support/agent-notify/agent-notify.sock")),
            status: Ok(report("stamp-1", Some("/opt/homebrew/bin/aerospace"))),
            installed_stamp: Some("stamp-1".to_string()),
            settings: Some(GOOD_SETTINGS.to_string()),
            in_tmux_pane: true,
            pane_session_tag: Some(true),
            focus_events: Some(true),
            fallback_notifier: Some(PathBuf::from("/opt/homebrew/bin/terminal-notifier")),
            launchd: Some(LaunchdState::Loaded),
            cli_log: Some(LogFile { path: PathBuf::from("/home/dev/Library/Logs/agent-notify/cli.log"), bytes: 1_024 }),
        }
    }

    fn status_of(checks: &[Check], name: &str) -> CheckStatus {
        checks.iter().find(|check| check.name == name).map(|check| check.status).unwrap_or_else(|| panic!("no check {name}"))
    }

    fn detail_of<'a>(checks: &'a [Check], name: &str) -> &'a str {
        checks.iter().find(|check| check.name == name).map(|check| check.detail.as_str()).unwrap_or_else(|| panic!("no check {name}"))
    }

    #[test]
    fn a_healthy_install_passes_everything() {
        let checks = evaluate(&healthy());
        assert!(checks.iter().all(|check| check.status == CheckStatus::Pass), "{checks:?}");
        assert!(!any_failed(&checks));
    }

    #[test]
    fn a_dead_daemon_fails() {
        let facts = DoctorFacts { status: Err("the daemon is not running".to_string()), ..healthy() };
        let checks = evaluate(&facts);
        assert_eq!(status_of(&checks, "daemon answers"), CheckStatus::Fail);
        assert!(any_failed(&checks));
    }

    #[test]
    fn a_stale_daemon_warns_with_the_restart_command() {
        let facts = DoctorFacts { installed_stamp: Some("stamp-2".to_string()), ..healthy() };
        let checks = evaluate(&facts);
        assert_eq!(status_of(&checks, "daemon is current"), CheckStatus::Warn);
        assert!(detail_of(&checks, "daemon is current").contains("launchctl kickstart -k"));
    }

    #[test]
    fn missing_settings_entries_are_named() {
        let facts = DoctorFacts { settings: Some("{}".to_string()), ..healthy() };
        let checks = evaluate(&facts);
        assert_eq!(status_of(&checks, "plugin enabled"), CheckStatus::Fail);
        assert_eq!(status_of(&checks, "permission rules"), CheckStatus::Warn);
    }

    #[test]
    fn tmux_problems_are_reported() {
        let facts = DoctorFacts { pane_session_tag: Some(false), focus_events: Some(false), ..healthy() };
        let checks = evaluate(&facts);
        assert_eq!(status_of(&checks, "session tag"), CheckStatus::Warn);
        assert_eq!(status_of(&checks, "focus events"), CheckStatus::Fail);
    }

    #[test]
    fn a_disabled_login_item_fails_with_where_to_fix_it() {
        let facts = DoctorFacts { launchd: Some(LaunchdState::Disabled), ..healthy() };
        let checks = evaluate(&facts);
        assert_eq!(status_of(&checks, "login item"), CheckStatus::Fail);
    }

    #[test]
    fn an_unanswered_launchctl_is_unknown_not_a_pass() {
        let facts = DoctorFacts { launchd: Some(LaunchdState::Unknown), ..healthy() };
        let checks = evaluate(&facts);
        assert_eq!(status_of(&checks, "login item"), CheckStatus::Warn);
        assert!(detail_of(&checks, "login item").contains("did not answer"));
    }

    #[test]
    fn missing_tools_in_the_daemon_environment() {
        let facts = DoctorFacts { status: Ok(report("stamp-1", None)), ..healthy() };
        assert_eq!(status_of(&evaluate(&facts), "daemon tools"), CheckStatus::Warn);
    }

    #[test]
    fn launchctl_disabled_output_parses_both_spellings() {
        let label = LAUNCHD_LABEL;
        assert!(parse_disabled("\t\"dev.dotfiles.agent-notifyd\" => disabled\n", label));
        assert!(parse_disabled("\t\"dev.dotfiles.agent-notifyd\" => true\n", label));
        assert!(!parse_disabled("\t\"dev.dotfiles.agent-notifyd\" => enabled\n", label));
        assert!(!parse_disabled("\t\"other\" => disabled\n", label));
    }

    #[test]
    fn render_puts_one_check_per_line() {
        let text = render(&evaluate(&healthy()));
        assert!(text.lines().all(|line| line.starts_with("ok    ") || line.starts_with("warn  ") || line.starts_with("FAIL  ")), "{text}");
    }

    #[test]
    fn the_cli_log_warns_at_fifty_mebibytes_and_passes_below() {
        let log_of = |bytes: u64| DoctorFacts { cli_log: Some(LogFile { path: PathBuf::from("/home/dev/cli.log"), bytes }), ..healthy() };
        assert_eq!(status_of(&evaluate(&log_of(CLI_LOG_WARN_BYTES - 1)), "cli log"), CheckStatus::Pass);
        let checks = evaluate(&log_of(CLI_LOG_WARN_BYTES));
        assert_eq!(status_of(&checks, "cli log"), CheckStatus::Warn);
        let detail = detail_of(&checks, "cli log");
        assert!(detail.contains("/home/dev/cli.log") && detail.contains("never rotates"), "{detail}");
    }

    #[test]
    fn unreadable_tmux_facts_inside_tmux_warn_instead_of_vanishing() {
        let facts = DoctorFacts { pane_session_tag: None, focus_events: None, ..healthy() };
        let checks = evaluate(&facts);
        assert_eq!(status_of(&checks, "session tag"), CheckStatus::Warn);
        assert_eq!(status_of(&checks, "focus events"), CheckStatus::Warn);
        assert!(detail_of(&checks, "focus events").contains("could not read"));
    }

    #[test]
    fn outside_tmux_the_pane_facts_stay_omitted() {
        let facts = DoctorFacts { in_tmux_pane: false, pane_session_tag: None, focus_events: None, ..healthy() };
        let checks = evaluate(&facts);
        assert!(checks.iter().all(|check| check.name != "session tag" && check.name != "focus events"));
    }

    #[test]
    fn a_loaded_agent_is_unknown_when_the_disabled_check_never_ran() {
        assert_eq!(launchd_from(Some(false), Some(true)), LaunchdState::Loaded);
        assert_eq!(launchd_from(None, Some(true)), LaunchdState::Unknown);
        assert_eq!(launchd_from(Some(true), Some(true)), LaunchdState::Disabled);
        assert_eq!(launchd_from(Some(true), None), LaunchdState::Disabled);
        assert_eq!(launchd_from(None, Some(false)), LaunchdState::NotLoaded);
        assert_eq!(launchd_from(Some(false), None), LaunchdState::Unknown);
    }

    #[test]
    fn a_missing_cli_log_passes() {
        let checks = evaluate(&DoctorFacts { cli_log: None, ..healthy() });
        assert_eq!(status_of(&checks, "cli log"), CheckStatus::Pass);
        assert!(detail_of(&checks, "cli log").contains("no log yet"));
    }

    #[test]
    fn daemon_supplied_text_cannot_carry_terminal_escapes_or_extra_lines() {
        let mut hostile = report("stamp\u{1b}[2J\nFAIL  forged: x", None);
        hostile.daemon.tools[1].name = "aero\u{1b}]0;owned\u{7}".to_string();
        let facts = DoctorFacts { status: Ok(hostile), installed_stamp: None, ..healthy() };
        let text = render(&evaluate(&facts));
        assert!(!text.contains('\u{1b}') && !text.contains('\u{7}'), "{text:?}");
        assert!(!text.lines().any(|line| line.starts_with("FAIL  forged")), "{text:?}");
    }
}
