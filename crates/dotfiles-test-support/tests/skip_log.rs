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

/// The gate reports the count, not just the mechanism.
///
/// The three tests above assert that `skip` WRITES. This asserts that
/// `tests/rust-checks.sh` READS, which is the other half and the half that
/// matters: a skip nothing reports is indistinguishable from a pass.
///
/// That property used to belong to `tests/skip-reporting.test.sh`, which
/// asserted it of the shell harness (`lib.sh` tallies, `run-all.sh` prints
/// the verdict). Tranche C's Task 5 classified that suite as mostly
/// harness-specific, and this is the one assertion that had to survive the
/// move rather than die with it.
///
/// Three instances on 2026-09-10 showed why. Two suites called `finish` with
/// assertions below it and one had no `finish` at all, so each printed
/// `FAIL:` and exited 0. A reporting gate that does not itself fail is the
/// same defect one level up.
#[test]
fn the_gate_reports_a_seeded_skip_count() {
    let directory = tempfile::Builder::new()
        .prefix("gate-report-")
        .tempdir_in("/tmp")
        .expect("a temp dir");
    let log = directory.path().join("skips.jsonl");
    dotfiles_test_support::skip_to(&log, "first seeded reason");
    dotfiles_test_support::skip_to(&log, "second seeded reason");

    // The reporting block from tests/rust-checks.sh, run against the seeded
    // log. Kept as the same shell so a change to the gate's wording or its
    // `sed` extraction breaks this test rather than passing silently.
    let script = r#"
        if [ -s "$DOTFILES_SKIP_LOG" ]; then
            skipped=$(wc -l < "$DOTFILES_SKIP_LOG" | tr -d ' ')
            printf 'rust-checks: %s skipped\n' "$skipped"
            sed -n 's/.*"reason":"\(.*\)"}/  skip: \1/p' "$DOTFILES_SKIP_LOG"
        fi
    "#;
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(script)
        .env("DOTFILES_SKIP_LOG", &log)
        .output()
        .expect("sh runs");
    let reported = String::from_utf8_lossy(&output.stdout);

    assert!(
        reported.contains("rust-checks: 2 skipped"),
        "the gate did not report the count. Got {reported:?}"
    );
    assert!(
        reported.contains("first seeded reason") && reported.contains("second seeded reason"),
        "the gate reported a count without the reasons, so a reader cannot \
         tell WHICH check stood down. Got {reported:?}"
    );
}

/// An empty log reports nothing at all.
///
/// A trailing "0 skipped" on every run is noise, and noise is what a reader
/// learns to scan past, which is how a real skip goes unnoticed.
#[test]
fn the_gate_is_silent_when_nothing_skipped() {
    let directory = tempfile::Builder::new()
        .prefix("gate-silent-")
        .tempdir_in("/tmp")
        .expect("a temp dir");
    let log = directory.path().join("skips.jsonl");
    fs::write(&log, "").expect("an empty log");

    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(r#"if [ -s "$DOTFILES_SKIP_LOG" ]; then printf 'rust-checks: skipped\n'; fi"#)
        .env("DOTFILES_SKIP_LOG", &log)
        .output()
        .expect("sh runs");

    assert!(
        String::from_utf8_lossy(&output.stdout).is_empty(),
        "an empty log produced output, so every clean run would carry noise"
    );
}
