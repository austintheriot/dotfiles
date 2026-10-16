//! `agent-notifyd`: the composition root. `main` builds every service once;
//! nothing below it constructs one.

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

mod geometry;
mod logging;
#[cfg(target_os = "macos")]
mod macos;
mod shell;

use std::process::ExitCode;

/// The build stamp `config build --tools` embeds.
const STAMP: &str = match option_env!("CONFIG_MANIFEST_STAMP") {
    Some(stamp) => stamp,
    None => "unstamped",
};

fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("--stamp") {
        println!("{STAMP}");
        return ExitCode::SUCCESS;
    }
    run()
}

#[cfg(target_os = "macos")]
fn run() -> ExitCode {
    macos::run()
}

#[cfg(not(target_os = "macos"))]
fn run() -> ExitCode {
    eprintln!("agent-notifyd: this platform is not supported yet (plan 3 adds Linux)");
    ExitCode::from(1)
}
