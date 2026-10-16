//! Command-line arguments.

use agent_notify_core::wire::{Kind, QuietUntil};
use clap::{Parser, Subcommand, ValueEnum};

/// The agent-notify command line.
#[derive(Debug, Parser)]
#[command(name = "agent-notify", version, about = "Contextual notifications for coding agents in tmux")]
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(arguments: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("agent-notify").chain(arguments.iter().copied()))
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
}
