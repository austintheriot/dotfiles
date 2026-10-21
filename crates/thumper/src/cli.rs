//! Command-line arguments.

use std::path::PathBuf;

use thumper_core::wire::{Kind, QuietUntil};
use clap::error::ErrorKind;
use clap::{Arg, ArgAction, ArgGroup, ArgMatches, Args, FromArgMatches, Parser, Subcommand, ValueEnum};

/// The thumper command line.
#[derive(Debug, Parser)]
#[command(name = "thumper", version, about = "Contextual notifications for coding agents in tmux")]
pub struct Cli {
    /// The subcommand.
    #[command(subcommand)]
    pub command: Command,
}

/// One subcommand.
#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Read a hook payload on stdin and send the mapped event. Always exits 0.
    Hook {
        /// The agent whose hook fired.
        #[command(subcommand)]
        agent: HookAgent,
    },
    /// Send a notification from an agent.
    Send {
        /// info for finished work, attention when the user must act.
        #[arg(long, value_enum)]
        kind: KindArg,
        /// One or two short sentences.
        #[arg(long)]
        body: String,
    },
    /// Silence the end-of-turn backup notification for this session.
    Quiet {
        /// When quiet ends on its own.
        #[arg(long, value_enum, conflicts_with = "off", required_unless_present = "off")]
        until: Option<UntilArg>,
        /// The latest end, in minutes.
        #[arg(long, default_value_t = 120, value_parser = clap::value_parser!(u16).range(1..=1440))]
        max_minutes: u16,
        /// End quiet now.
        #[arg(long)]
        off: bool,
    },
    /// Report a tmux pane focus change. tmux's pane-focus hooks call this.
    FocusChanged {
        /// The tmux server socket, `#{socket_path}`.
        #[arg(long)]
        socket: PathBuf,
        /// The tmux server pid, `#{pid}`.
        #[arg(long)]
        pid: u32,
        /// The tmux server start time, `#{start_time}`.
        #[arg(long)]
        start_time: i64,
        /// The pane, `#{pane_id}`.
        #[arg(long)]
        pane: String,
        /// `--in` or `--out`.
        #[command(flatten)]
        direction: FocusDirection,
    },
    /// Hold every card back, or let them through again.
    Dnd {
        /// on, off or status.
        #[command(subcommand)]
        action: DndAction,
    },
    /// Go to the newest card that needs you.
    Jump,
    /// Print the daemon's state.
    Status {
        /// The full report as JSON.
        #[arg(long, conflicts_with = "tmux")]
        json: bool,
        /// A short segment for the tmux status line. Prints nothing when no daemon is installed.
        #[arg(long)]
        tmux: bool,
    },
    /// Check the installation and print what to fix.
    Doctor,
}

/// Which way focus moved. Parsed from `--in` and `--out`, exactly one of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    /// The pane gained focus.
    In,
    /// The pane lost focus.
    Out,
}

const FOCUS_IN: &str = "in";
const FOCUS_OUT: &str = "out";

impl FromArgMatches for FocusDirection {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, clap::Error> {
        match (matches.get_flag(FOCUS_IN), matches.get_flag(FOCUS_OUT)) {
            (true, false) => Ok(FocusDirection::In),
            (false, true) => Ok(FocusDirection::Out),
            _ => Err(clap::Error::raw(ErrorKind::MissingRequiredArgument, "focus-changed needs exactly one of --in and --out\n")),
        }
    }

    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> {
        *self = FocusDirection::from_arg_matches(matches)?;
        Ok(())
    }
}

impl Args for FocusDirection {
    fn augment_args(command: clap::Command) -> clap::Command {
        command
            .arg(Arg::new(FOCUS_IN).long(FOCUS_IN).action(ArgAction::SetTrue).help("The pane gained focus"))
            .arg(Arg::new(FOCUS_OUT).long(FOCUS_OUT).action(ArgAction::SetTrue).help("The pane lost focus"))
            .group(ArgGroup::new("direction").args([FOCUS_IN, FOCUS_OUT]).required(true).multiple(false))
    }

    fn augment_args_for_update(command: clap::Command) -> clap::Command {
        FocusDirection::augment_args(command)
    }
}

/// A do-not-disturb action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Subcommand)]
pub enum DndAction {
    /// Hold every card back.
    On {
        /// End on its own after this many minutes.
        #[arg(long, value_parser = clap::value_parser!(u16).range(1..=1440))]
        for_minutes: Option<u16>,
    },
    /// Let cards through again.
    Off,
    /// Print whether do-not-disturb is on.
    Status,
}

/// The agent behind a hook.
#[derive(Debug, Clone, Subcommand)]
pub enum HookAgent {
    /// Claude Code.
    ClaudeCode {
        /// Print the request and send nothing.
        #[arg(long)]
        dry_run: bool,
    },
}

/// `--kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum KindArg {
    /// Finished work.
    Info,
    /// The user must act.
    Attention,
}

impl From<KindArg> for Kind {
    fn from(kind: KindArg) -> Kind {
        match kind {
            KindArg::Info => Kind::Info,
            KindArg::Attention => Kind::Attention,
        }
    }
}

/// `--until`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum UntilArg {
    /// The session's next `send`.
    NextSend,
    /// The session's next user prompt.
    NextPrompt,
}

impl From<UntilArg> for QuietUntil {
    fn from(until: UntilArg) -> QuietUntil {
        match until {
            UntilArg::NextSend => QuietUntil::NextSend,
            UntilArg::NextPrompt => QuietUntil::NextPrompt,
        }
    }
}

impl Command {
    /// True for `hook ... --dry-run`.
    pub fn is_dry_run(&self) -> bool {
        matches!(self, Command::Hook { agent: HookAgent::ClaudeCode { dry_run: true } })
    }

    /// True for `hook`, which must exit 0 whatever happens.
    pub fn is_hook(&self) -> bool {
        matches!(self, Command::Hook { .. })
    }

    /// True for the commands whose request carries an origin, so the CLI
    /// reads the tmux pane and the git repository first.
    pub fn reads_origin(&self) -> bool {
        matches!(self, Command::Hook { .. } | Command::Send { .. } | Command::Quiet { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(arguments: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("thumper").chain(arguments.iter().copied()))
    }

    #[test]
    fn accepted_command_lines() {
        assert!(parse(&["hook", "claude-code"]).is_ok());
        assert!(parse(&["hook", "claude-code", "--dry-run"]).is_ok_and(|cli| cli.command.is_dry_run()));
        assert!(parse(&["send", "--kind", "attention", "--body", "CI is red"]).is_ok());
        assert!(parse(&["quiet", "--until", "next-send", "--max-minutes", "180"]).is_ok());
        assert!(parse(&["quiet", "--off"]).is_ok());
    }

    #[test]
    fn usage_errors() {
        for arguments in [
            &["send", "--kind", "loud", "--body", "x"][..],
            &["send", "--body", "x"],
            &["quiet"],
            &["quiet", "--off", "--until", "next-send"],
            &["quiet", "--until", "next-send", "--max-minutes", "0"],
            &["hook", "codex"],
        ] {
            let error = parse(arguments).expect_err("a usage error");
            assert_eq!(error.exit_code(), 2, "{arguments:?}");
        }
    }

    const FOCUS: [&str; 9] = ["focus-changed", "--socket", "/tmp/tmux-501/default", "--pid", "77", "--start-time", "1700000000", "--pane", "%3"];

    fn focus_direction(flags: &[&str]) -> Result<FocusDirection, clap::Error> {
        let arguments: Vec<&str> = FOCUS.iter().chain(flags).copied().collect();
        parse(&arguments).map(|cli| match cli.command {
            Command::FocusChanged { direction, .. } => direction,
            other => panic!("a focus-changed command, got {other:?}"),
        })
    }

    #[test]
    fn focus_changed_takes_exactly_one_direction() {
        assert_eq!(focus_direction(&["--in"]).expect("--in parses"), FocusDirection::In);
        assert_eq!(focus_direction(&["--out"]).expect("--out parses"), FocusDirection::Out);
        for flags in [&[][..], &["--in", "--out"]] {
            let error = focus_direction(flags).expect_err("a usage error");
            assert_eq!(error.exit_code(), 2, "{flags:?}");
        }
    }

    #[test]
    fn the_new_command_lines_parse() {
        assert!(parse(&["dnd", "on"]).is_ok_and(|cli| matches!(cli.command, Command::Dnd { action: DndAction::On { for_minutes: None } })));
        assert!(parse(&["dnd", "on", "--for-minutes", "30"]).is_ok_and(|cli| matches!(cli.command, Command::Dnd { action: DndAction::On { for_minutes: Some(30) } })));
        assert!(parse(&["dnd", "off"]).is_ok());
        assert!(parse(&["dnd", "status"]).is_ok());
        assert!(parse(&["jump"]).is_ok());
        assert!(parse(&["status"]).is_ok_and(|cli| matches!(cli.command, Command::Status { json: false, tmux: false })));
        assert!(parse(&["status", "--json"]).is_ok());
        assert!(parse(&["status", "--tmux"]).is_ok());
        assert!(parse(&["doctor"]).is_ok());
        for arguments in [&["dnd", "on", "--for-minutes", "0"][..], &["dnd", "on", "--for-minutes", "1441"], &["status", "--json", "--tmux"], &["dnd"]] {
            let error = parse(arguments).expect_err("a usage error");
            assert_eq!(error.exit_code(), 2, "{arguments:?}");
        }
    }
}
