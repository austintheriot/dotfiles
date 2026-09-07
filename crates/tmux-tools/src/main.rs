#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
//! The IO half of the tmux and zsh helpers.
//!
//! Performs the tmux and git calls and hands their results to `tmux_core`,
//! which decides. Exit 2 is every usage error, matching the repo-wide
//! convention `check-deps.sh` and `config-manifest` already use.

use std::process::ExitCode;

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
