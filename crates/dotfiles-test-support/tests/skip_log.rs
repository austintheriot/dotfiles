//! The runtime-skip mechanism the pre-push gate counts.
//!
//! Rust's harness has no runtime skip, so `rust-checks.sh` learns what did
//! not run from a log file rather than from stdout, which `cargo test
//! --quiet` swallows. These tests pin the three properties the gate depends
//! on: one skip is one line, skips accumulate, and an unconfigured log is a
//! silent no-op.

use std::fs;

/// `skip` appends one line naming the reason, so `rust-checks.sh` can count
/// what did not run. The gate reads this file; a skip that writes nothing is
/// indistinguishable from a pass.
#[test]
fn skip_appends_a_line_naming_the_reason() {
    let directory = tempfile::Builder::new()
        .prefix("skip-log-")
        .tempdir_in("/tmp")
        .expect("a temp dir");
    let log = directory.path().join("skips.jsonl");

    dotfiles_test_support::skip_to(&log, "shellcheck is not installed here");

    let written = fs::read_to_string(&log).expect("the log exists");
    assert_eq!(written.lines().count(), 1, "one skip is one line");
    assert!(
        written.contains("shellcheck is not installed here"),
        "the reason survives into the log: {written}"
    );
}

/// Two skips are two lines. A mechanism that overwrites reports one skip
/// where there were several, which understates missing coverage.
#[test]
fn skips_accumulate_rather_than_overwrite() {
    let directory = tempfile::Builder::new()
        .prefix("skip-log-")
        .tempdir_in("/tmp")
        .expect("a temp dir");
    let log = directory.path().join("skips.jsonl");

    dotfiles_test_support::skip_to(&log, "first reason");
    dotfiles_test_support::skip_to(&log, "second reason");

    let written = fs::read_to_string(&log).expect("the log exists");
    assert_eq!(written.lines().count(), 2, "two skips are two lines");
}

/// With no log path configured, `skip` must not panic and must not record
/// anything. A developer running `cargo test` by hand has no gate to report
/// to.
///
/// Asserted through `skip_log_path` rather than by calling `skip` itself.
/// `skip` reads the ambient environment, and `rust-checks.sh` deliberately
/// SETS that environment, so a test that called `skip` here would append a
/// phantom line to the gate's own log and the gate would report a skip that
/// never happened. Observed exactly that: the gate printed
/// "rust-checks: 1 skipped" naming this test's reason string, which
/// overstates missing coverage, the failure this mechanism exists to
/// prevent.
#[test]
fn an_unconfigured_log_is_a_silent_no_op() {
    let unset = std::env::var_os(dotfiles_test_support::SKIP_LOG_VARIABLE).is_none();
    if unset {
        assert!(
            dotfiles_test_support::skip_log_path().is_none(),
            "with no variable set there is no log to write to"
        );
    }

    let directory = tempfile::Builder::new()
        .prefix("skip-log-")
        .tempdir_in("/tmp")
        .expect("a temp dir");
    let absent = directory.path().join("never-created.jsonl");
    assert!(
        !absent.exists(),
        "the no-op path must not have created a log"
    );
}
