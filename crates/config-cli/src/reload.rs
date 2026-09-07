//! `config reload`: re-source the tmux config, then print the line to
//! re-source zsh.
//!
//! Ported from `.scripts/config/config-reload`. Two effects, two prints,
//! kept in one module because there is no pure core worth extracting from
//! four straight-line steps.

use std::path::PathBuf;
use std::process::{Command, ExitCode};

/// `config reload`'s arguments: none. The struct exists so the subcommand
/// has a distinct clap type to hang its help text on.
///
/// Re-sources `~/.config/tmux/tmux.conf` when run inside tmux, regenerates
/// the Alacritty platform config, and prints the line to re-source zsh.
///
/// The zsh line is printed rather than run: a child process cannot re-source
/// the shell that called it, so the calling shell has to run that line
/// itself. The explanation of why goes to stderr and the line itself goes to
/// stdout, so `$(config reload)` captures something runnable rather than the
/// explanation.
#[derive(clap::Args)]
// `override_usage` rather than relying on the derived line: clap renders
// subcommand usage as `<parent bin_name> <subcommand>`, which would print
// `config-cli reload`. The dispatcher's own name is `config`, and
// `tests/config-usage.test.sh` pins the literal string `config reload` in
// this command's help text, so the usage line has to say what a caller
// actually types.
#[command(override_usage = "config reload")]
pub(crate) struct ReloadArgs;

/// Run `config reload`. Always exits successfully: every step here is
/// best-effort, matching the shell script it replaces, which had no failure
/// path of its own beyond `set -e` on the Alacritty regeneration step's own
/// exit code (checked, not propagated).
pub(crate) fn run(_arguments: ReloadArgs) -> ExitCode {
    let Some(home) = std::env::var_os("HOME") else {
        eprintln!("config-cli: reload: $HOME is not set");
        return ExitCode::FAILURE;
    };
    let home = PathBuf::from(home);

    if std::env::var_os("TMUX").is_some() {
        reload_tmux(&home);
    }

    regenerate_alacritty(&home);

    // The explanation is commentary for the human reading the terminal; the
    // zsh line is the payload a caller captures with `$(config reload)`. The
    // two cannot share a stream or that capture would include the sentence.
    eprintln!("a child process cannot re-source the calling shell; run the line below");
    println!("source ~/.zshrc");

    ExitCode::SUCCESS
}

/// Re-source the tmux config inside the running tmux server.
///
/// Silent on failure, matching the shell script: an unset `TMUX` already
/// gates this call, so a failure here means the tmux binary or the config
/// path is missing, and there is nothing this command can do about either.
fn reload_tmux(home: &std::path::Path) {
    let tmux_conf = home.join(".config/tmux/tmux.conf");
    let status = Command::new("tmux").arg("source").arg(&tmux_conf).status();
    if matches!(status, Ok(status) if status.success()) {
        println!("reloaded tmux config");
    }
}

/// Regenerate the machine-specific Alacritty config that tmux's colors
/// depend on.
///
/// Alacritty has no conditional import, so the file it imports has to be
/// generated for this machine's variant. Regenerated here because editing a
/// variant is exactly when it goes stale; Alacritty picks the result up on
/// its own.
fn regenerate_alacritty(home: &std::path::Path) {
    let script = home.join(".scripts/alacritty-platform.sh");
    let status = Command::new(&script).status();
    if matches!(status, Ok(status) if status.success()) {
        println!("regenerated alacritty platform config");
    }
}
