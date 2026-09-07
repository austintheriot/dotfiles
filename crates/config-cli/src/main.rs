#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
//! The adapter that runs the pure dependency core.
//!
//! Owns argv and the exit code. Every decision about what to install lives
//! in `deps_core`, and every capability lives in `deps/`.
//!
//! Exit 2 is every caller error, matching the repo-wide convention
//! `check-deps.sh` established and `deps-docs.test.sh` uses as its oracle.
//! The verb-to-code mapping has exactly one owner, `deps_core::exit_status`,
//! reached here through `Rendered::exit_code`.

use std::process::ExitCode;

use clap::{Parser, Subcommand};

mod deps;

/// The `config-cli` command line: one subcommand, `deps`.
#[derive(Parser)]
// `subcommand_required` cannot coexist with the exclusive `--stamp` flag, so
// the missing-subcommand case is handled after parsing instead, matching
// config-manifest's Cli.
#[command(name = "config-cli", disable_help_subcommand = true)]
struct Cli {
    /// Print the build-time stamp of the crate this binary was built from.
    ///
    /// `config-build` reads this after installing to confirm the binary it
    /// just built is the one that landed on PATH, and `config doctor` reads
    /// it to decide whether an installed binary matches its source.
    #[arg(long)]
    stamp: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

/// A top-level `config-cli` subcommand.
#[derive(Subcommand)]
enum Command {
    /// Check or install the dependencies the manifest names.
    Deps {
        #[command(subcommand)]
        verb: DepsVerb,
    },
}

/// A `deps` verb: whether to change anything.
#[derive(Subcommand)]
pub(crate) enum DepsVerb {
    /// Report what is missing, changing nothing.
    Check(DepsArgs),
    /// Install what is missing.
    Install(DepsArgs),
}

/// Flags shared by every `deps` verb.
#[derive(clap::Args)]
pub(crate) struct DepsArgs {
    /// Print what would happen, spawning nothing.
    #[arg(long)]
    dry_run: bool,
    /// Approve every step without prompting. Required in CI, which pins it.
    #[arg(long)]
    yes: bool,
    /// Restrict the run to these dependencies, comma-separated.
    #[arg(long, value_delimiter = ',')]
    only: Vec<String>,
}

fn main() -> ExitCode {
    // clap exits 2 itself for a parse error, which is the convention this
    // binary must keep.
    let cli = Cli::parse();

    if cli.stamp {
        // `option_env!` is read at compile time, so cargo rebuilds when
        // CONFIG_MANIFEST_STAMP changes. config-build sets this same
        // variable name for every workspace binary it compiles, so the
        // stamp travels inside each binary rather than living in a file a
        // stale binary could be copied without.
        println!(
            "{}",
            option_env!("CONFIG_MANIFEST_STAMP").unwrap_or("unstamped")
        );
        return ExitCode::SUCCESS;
    }

    match cli.command {
        Some(Command::Deps { verb }) => deps::run(verb),
        None => {
            // A bare invocation is a usage error, so the help goes to
            // stderr and the exit code stays 2, matching every other usage
            // error this binary and config-manifest report.
            let mut command = <Cli as clap::CommandFactory>::command();
            let _ = command.write_help(&mut std::io::stderr());
            ExitCode::from(2)
        }
    }
}
