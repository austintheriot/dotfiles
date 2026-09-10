#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
//! Test-support for the converted shell suites.
//!
//! Holds one thing: a runtime skip that the pre-push gate can count. Rust's
//! harness has no representation for "this check cannot run on this
//! machine": `#[ignore]` is a compile-time decision and hides the count, and
//! `eprintln!` is swallowed by `cargo test --quiet`, which is the command
//! `tests/rust-checks.sh` runs. Measured 2026-09-10: that command reported
//! zero skip lines from `nvim_runtime.rs`, which already skips this way.
//!
//! So a skip is recorded to a file whose path the gate sets, and the gate
//! reports the count. See the spec's section 4b.1.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The environment variable naming the skip log. Set by `tests/rust-checks.sh`.
pub const SKIP_LOG_VARIABLE: &str = "DOTFILES_SKIP_LOG";

/// Where skips are recorded, or `None` when no gate is listening.
#[must_use]
pub fn skip_log_path() -> Option<PathBuf> {
    std::env::var_os(SKIP_LOG_VARIABLE).map(PathBuf::from)
}

/// Records that a check could not run here, and why.
///
/// Call this and return early. It is a no-op when no log is configured, so a
/// developer running `cargo test` by hand needs no setup.
pub fn skip(reason: &str) {
    if let Some(log) = skip_log_path() {
        skip_to(&log, reason);
    }
}

/// Appends one skip to an explicit path. Public so the mechanism is testable
/// without mutating the environment, which parallel tests share.
pub fn skip_to(log: &Path, reason: &str) {
    let escaped = reason.replace('\\', "\\\\").replace('"', "\\\"");
    let line = format!("{{\"reason\":\"{escaped}\"}}\n");
    if let Ok(mut handle) = OpenOptions::new().create(true).append(true).open(log) {
        let _ = handle.write_all(line.as_bytes());
    }
}
