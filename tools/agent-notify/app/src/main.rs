//! `agent-notifyd`: the composition root. Task 16 fills in `run`.

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

mod geometry;
mod logging;
#[cfg(target_os = "macos")]
mod macos;

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
    eprintln!("agent-notifyd: not wired yet");
    ExitCode::from(1)
}
