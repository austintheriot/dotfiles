#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
//! The IO half of the tmux and zsh helpers.
//!
//! Performs the tmux and git calls and hands their results to `tmux_core`,
//! which decides. Exit 2 is every usage error, matching the repo-wide
//! convention `check-deps.sh` and `config-manifest` already use.

mod git_branches;
mod repo;
mod tmux;

use std::path::Path;
use std::process::ExitCode;

use tmux::{Server, WindowTarget};
use tmux_core::{RepositoryFacts, WindowFacts, format_branches, window_name};

/// Where the fallback `@wname_bare_repos` patterns live, relative to `$HOME`.
///
/// A file rather than a compiled-in constant, and deliberately so. The
/// patterns name an employer and its products, and this repository is
/// public. `.claude/local/` is excluded in `.cfg/info/exclude`, so the names
/// stay out of every published artifact while the binary keeps working for
/// whoever has the file. A machine without it simply has no fallback, which
/// is the correct behavior for anyone but its author.
const BARE_REPOS_CONFIG: &str = ".claude/local/tmux-bare-repos.conf";

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    match arguments.next().as_deref() {
        // config-build embeds this at compile time and config-manifest's
        // doctor subcommand probes it at runtime to decide whether the
        // installed binary matches its source; every crate with a
        // src/main.rs must answer this flag or doctor reports it as never
        // installed. CONFIG_MANIFEST_STAMP is the variable name config-build
        // sets for every crate, not only config-manifest itself.
        Some("--stamp") => {
            println!(
                "{}",
                option_env!("CONFIG_MANIFEST_STAMP").unwrap_or("unstamped")
            );
            ExitCode::SUCCESS
        }
        Some("name-windows") => name_windows(arguments.collect()),
        Some("close") => close(arguments.collect()),
        Some("worktree-config") => worktree_config(),
        Some("list-branches") => list_branches(),
        Some(other) => {
            eprintln!("tmux-tools: unknown subcommand {other}");
            ExitCode::from(2)
        }
        None => {
            eprintln!("tmux-tools: a subcommand is required");
            ExitCode::from(2)
        }
    }
}

/// What `name-windows` was asked to update.
enum Mode {
    /// Every window in every session (`-a`).
    All,
    /// Every window in one session (`-s <session>`).
    Session(String),
    /// One window (`-w <window>`).
    Window(String),
    /// The window behind `$TMUX_PANE`, or nothing when that is unset.
    Default,
}

/// Parses `name-windows`'s arguments, matching the shell script's flags.
///
/// # Errors
///
/// Returns an error message for an unrecognized flag or a flag missing its
/// required argument.
fn parse_mode(arguments: &[String]) -> Result<Mode, String> {
    match arguments {
        [] => Ok(Mode::Default),
        [flag] if flag == "-a" || flag == "--all" => Ok(Mode::All),
        [flag, target] if flag == "-s" || flag == "--session" => Ok(Mode::Session(target.clone())),
        [flag, target] if flag == "-w" || flag == "--window" => Ok(Mode::Window(target.clone())),
        [flag, ..] if flag.starts_with('-') => Err(format!("unknown option: {flag}")),
        [target] => Ok(Mode::Session(target.clone())),
        _ => Err(format!("unexpected arguments: {}", arguments.join(" "))),
    }
}

/// Runs the `name-windows` subcommand.
fn name_windows(arguments: Vec<String>) -> ExitCode {
    let mode = match parse_mode(&arguments) {
        Ok(mode) => mode,
        Err(message) => {
            eprintln!("tmux-tools name-windows: {message}");
            return ExitCode::from(2);
        }
    };

    let server = Server::from_env();

    let targets = match mode {
        Mode::All => server.list_windows(&["-a"]),
        Mode::Session(session) => server.list_windows(&["-t", &session]),
        Mode::Window(window) => server.display_message(&window).into_iter().collect(),
        Mode::Default => match std::env::var("TMUX_PANE") {
            Ok(pane) => server.display_message(&pane).into_iter().collect(),
            Err(_) => Vec::new(),
        },
    };

    for target in targets {
        update_window(&server, &target);
    }

    ExitCode::SUCCESS
}

/// Runs the `close` subcommand: kills every pane in the current window
/// except the target pane, matching `tmux-close.sh`'s
/// `tmux kill-pane -a -t "$(tmux display-message -p '#{pane_id}')"`.
///
/// Accepts an optional `-t <pane>` so a test can target a specific pane
/// without an attached client; the shell script never passes this flag and
/// always resolves the active pane itself.
///
/// The shell script has no `-z` early-exit branch for "no tmux session":
/// that check lives in the shim's caller, which stays sourced so the
/// message and its `return` still reach the interactive shell. See the
/// report for the exact division of labor.
fn close(arguments: Vec<String>) -> ExitCode {
    let explicit_target = match arguments.as_slice() {
        [] => None,
        [flag, target] if flag == "-t" => Some(target.clone()),
        _ => {
            eprintln!("tmux-tools close: unexpected arguments: {}", arguments.join(" "));
            return ExitCode::from(2);
        }
    };

    let server = Server::from_env();

    let target_pane = match explicit_target.or_else(|| server.current_pane_id()) {
        Some(pane) => pane,
        None => {
            eprintln!("tmux-tools close: no active pane to target");
            return ExitCode::from(2);
        }
    };

    if server.kill_other_panes(&target_pane) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Runs the `worktree-config` subcommand: prints the numbered-worktree
/// window count that `tmux-setup.sh` sizes its worktree window loop with.
///
/// `tmux-worktree-config.sh`'s only effect is the shell assignment
/// `WORKTREE_COUNT=15`, which a subprocess cannot reproduce in its parent
/// shell. Printing the number is the honest port: the shim script still
/// performs the assignment itself, from this binary's output. See the
/// report for why this differs from a tmux-option-setting subcommand.
fn worktree_config() -> ExitCode {
    println!("{WORKTREE_COUNT}");
    ExitCode::SUCCESS
}

/// The number of numbered worktree windows `tmux-setup.sh`
/// creates, matching `tmux-worktree-config.sh`'s `WORKTREE_COUNT=15`.
const WORKTREE_COUNT: u32 = 15;

/// Runs the `list-branches` subcommand: one `git for-each-ref` call, formatted
/// the way `zsh-git-widgets.sh`'s `git | rg | sed | sed` pipeline formatted
/// its candidates, one branch per line on stdout.
///
/// `fzf` and the trailing `cut -f1` that reads the picked line back out stay
/// in the widget: this binary's job ends at producing the candidate list,
/// and picking one is the interactive part `tmux-core` holds no IO to run.
fn list_branches() -> ExitCode {
    let refs = git_branches::list();
    for line in format_branches(&refs) {
        println!("{line}");
    }
    ExitCode::SUCCESS
}

/// Splits a window's `@wname_bare_repos` value on `|` for
/// `tmux_core::window_name`, falling back to the script's documented
/// default list when the window carries no override.
///
/// Splitting the raw option string is this binary's job, not the pure
/// crate's: reading a tmux option is IO, and `tmux_core` performs none.
fn bare_repo_patterns(target: &WindowTarget) -> Vec<String> {
    if !target.bare_repos.is_empty() {
        return target.bare_repos.split('|').map(str::to_string).collect();
    }
    fallback_bare_repos()
}

/// Reads the fallback patterns from [`BARE_REPOS_CONFIG`].
///
/// One pattern per line, `#` and blank lines ignored. An absent or unreadable
/// file yields no patterns rather than an error: the fallback is a personal
/// convenience, and a machine without the file should name every repository
/// the ordinary way instead of failing a prompt redraw.
fn fallback_bare_repos() -> Vec<String> {
    let Some(home) = std::env::var_os("HOME") else {
        return Vec::new();
    };
    let path = Path::new(&home).join(BARE_REPOS_CONFIG);
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// Computes a window's name from its tmux state and directory, then renames
/// it only when the computed name differs from the current one.
///
/// Renaming unconditionally fires the six `after-*` tmux hooks
/// (`.config/tmux/tmux.conf`), which would make every window rename cascade
/// into more renames and is what the spec names as the source of the shell
/// suite's flakiness on a developer machine.
fn update_window(server: &Server, target: &WindowTarget) {
    // We own a window's name when we set it last, when it is empty, or when
    // tmux is still auto-renaming it. Anything else means the user renamed
    // it by hand, and that name is authoritative until it goes empty again.
    //
    // A window the user owns is left untouched entirely, matching the shell
    // script's own early return here: computing a name and finding it equal
    // to current_name would skip the rename correctly, but would still
    // overwrite @wname_auto with the user's own name, which corrupts the
    // ownership tracking for the run after the user's next rename.
    let user_owns_the_name = !target.current_name.is_empty()
        && target.current_name != target.owned_name
        && !target.automatic_rename;
    if user_owns_the_name {
        return;
    }

    if target.pane_current_path.is_empty() {
        return;
    }

    let facts = window_facts(target);
    let patterns = bare_repo_patterns(target);
    let computed = window_name(&facts, &patterns);

    if computed != target.current_name {
        server.rename(&target.window_id, &computed);
    }
    if computed != target.owned_name {
        server.set_owned_name(&target.window_id, &computed);
    }
}

/// Builds the `tmux_core::WindowFacts` for one window this binary still
/// owns the name of. `update_window` returns before this runs for a window
/// the user renamed by hand, so `manual_name` is always `None` here: this
/// function only ever computes the automatic part of the name.
fn window_facts(target: &WindowTarget) -> WindowFacts {
    let directory = Path::new(&target.pane_current_path);
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let is_home = home.as_deref() == Some(directory);

    let directory_basename = if directory.as_os_str() == "/" {
        "/".to_string()
    } else {
        directory
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    };

    let repository: Option<RepositoryFacts> = repo::inspect(directory);

    WindowFacts {
        directory_basename,
        is_home,
        repository,
        manual_name: None,
        label: (!target.label.is_empty()).then(|| target.label.clone()),
    }
}
