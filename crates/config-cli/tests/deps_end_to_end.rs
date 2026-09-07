//! `deps check` and `deps install` driven against the built binary.
//!
//! Every test writes its own manifest into a temporary directory and points
//! `DEPS_CONF` at it, so no test reads the repository's real conf files and
//! none of them can install anything. That is what keeps this crate
//! hermetic, which the host-side Rust gate depends on: a test that mutated
//! the real `$HOME` would make the gate's result depend on the machine it
//! ran on.

use std::path::Path;
use std::process::{Command, Output};

/// Run the built binary with a fixture manifest and no platform variant.
///
/// `DEPS_LOCAL_CONF` is set to a path that does not exist on purpose. An
/// explicit value is what suppresses the platform variant, so a fixture run
/// cannot pull `deps-mac.conf` or `deps-linux.conf` into a manifest the test
/// wrote by hand.
fn run_against(conf: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(arguments)
        .env("DEPS_CONF", conf)
        .env("DEPS_LOCAL_CONF", "/nonexistent/deps-platform.conf")
        .output()
        .expect("the binary runs")
}

/// Write a one-entry manifest into a fresh temporary directory.
fn fixture_manifest(line: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let fixture = tempfile::tempdir().expect("tempdir");
    let conf = fixture.path().join("deps.conf");
    std::fs::write(&conf, line).expect("write");
    (fixture, conf)
}

/// `deps check` on a manifest whose dependency is present exits 0 and says so.
#[test]
fn check_exits_zero_when_everything_is_present() {
    // A check that is true on any machine: `sh` is on PATH everywhere this
    // repo runs, and the test asserts that below rather than assuming it.
    let (_fixture, conf) = fixture_manifest("sh|command -v sh|https://example.invalid/sh\n");

    let run = run_against(&conf, &["deps", "check"]);
    let stdout = String::from_utf8_lossy(&run.stdout);

    // Positive control: the run must produce output, or the exit assertion
    // could hold for a binary that did nothing at all.
    assert!(!stdout.is_empty(), "a check must report something");
    assert_eq!(
        run.status.code(),
        Some(0),
        "everything present is exit 0, stdout: {stdout}"
    );
    assert!(
        stdout.contains("present   sh"),
        "the report must name the present dependency, got {stdout:?}"
    );
}

/// `deps check` on a manifest with an absent dependency exits 1.
#[test]
fn check_exits_one_when_something_is_missing() {
    let (_fixture, conf) = fixture_manifest(
        "nonexistent-tool|command -v definitely-not-a-real-binary|https://example.invalid/x\n",
    );

    let run = run_against(&conf, &["deps", "check"]);
    let stdout = String::from_utf8_lossy(&run.stdout);

    // Positive control before the exit assertion, so exit 1 cannot come from
    // a binary that produced nothing and failed for an unrelated reason.
    assert!(!stdout.is_empty(), "a check must report something");
    assert_eq!(
        run.status.code(),
        Some(1),
        "a missing dependency is exit 1, stdout: {stdout}"
    );
}

/// `--dry-run` spawns nothing.
///
/// Asserted by pointing the run at a dependency whose install would fail
/// loudly if attempted, and requiring exit to reflect readiness rather than
/// an install failure.
#[test]
fn dry_run_spawns_nothing() {
    let (_fixture, conf) = fixture_manifest(
        "nonexistent-tool|command -v definitely-not-a-real-binary|https://example.invalid/x\n",
    );

    let run = run_against(&conf, &["deps", "install", "--dry-run", "--yes"]);
    let stdout = String::from_utf8_lossy(&run.stdout);

    assert!(!stdout.is_empty(), "a dry run must say what it would do");
    assert_ne!(
        run.status.code(),
        Some(3),
        "exit 3 means an attempt failed, and a dry run attempts nothing: {stdout}"
    );
}

/// A dry run names the command it would run, rather than only a count.
///
/// The disclosure is the point of `--dry-run`: a preview that reported only
/// "1 entry" would pass `dry_run_spawns_nothing` while telling the reader
/// nothing about what is about to happen on their machine.
#[test]
fn dry_run_discloses_the_command_it_would_run() {
    let (_fixture, conf) = fixture_manifest("ripgrep|command -v rg|https://example.invalid/rg\n");

    let run = run_against(&conf, &["deps", "install", "--dry-run", "--yes"]);
    let stdout = String::from_utf8_lossy(&run.stdout);

    assert!(!stdout.is_empty(), "a dry run must say what it would do");
    assert!(
        stdout.contains("would install"),
        "a dry run must announce itself as a preview, got {stdout:?}"
    );
}

/// An `--only` value the manifest does not hold is a caller error, exit 2.
///
/// `check-deps.sh:101-107` treated a typo the same way. A silently narrowed
/// selection would let an install report success having installed none of
/// what the caller named.
#[test]
fn an_unknown_only_value_is_a_caller_error() {
    let (_fixture, conf) = fixture_manifest("sh|command -v sh|https://example.invalid/sh\n");

    let run = run_against(&conf, &["deps", "check", "--only", "not-in-this-manifest"]);
    let stderr = String::from_utf8_lossy(&run.stderr);

    // Positive control: exit 2 with a silent stderr would be indistinguishable
    // from a crash, so the message must exist before its code is trusted.
    assert!(!stderr.is_empty(), "a caller error must explain itself");
    assert_eq!(
        run.status.code(),
        Some(2),
        "an unknown --only value is exit 2, stderr: {stderr}"
    );
    assert!(
        stderr.contains("not-in-this-manifest"),
        "the error must name the offending selector, got {stderr:?}"
    );
}

/// A conf file that does not parse is a caller error, exit 2.
#[test]
fn an_unparsable_manifest_is_a_caller_error() {
    let (_fixture, conf) = fixture_manifest("this line has no pipes at all\n");

    let run = run_against(&conf, &["deps", "check"]);
    let stderr = String::from_utf8_lossy(&run.stderr);

    assert!(!stderr.is_empty(), "a caller error must explain itself");
    assert_eq!(
        run.status.code(),
        Some(2),
        "an unparsable manifest is exit 2, stderr: {stderr}"
    );
}

/// `--only` narrows what a dry run plans, not what the report covers.
///
/// `deps_core::reconcile` walks the whole manifest by design, so every entry
/// keeps a row and the readiness verdict still covers the machine rather than
/// the selection. What `--only` decides is which steps the plan emits, which
/// is what a dry run discloses.
#[test]
fn only_narrows_which_steps_a_dry_run_plans() {
    let (_fixture, conf) = fixture_manifest(concat!(
        "ripgrep|command -v definitely-not-ripgrep|https://example.invalid/rg\n",
        "fd|command -v definitely-not-fd|https://example.invalid/fd\n",
    ));

    // Positive control for the absence asserted below: unnarrowed, the dry
    // run does plan a step for `fd`, so its absence afterwards is the
    // selection taking effect and not a preview that plans nothing at all.
    let full = run_against(&conf, &["deps", "install", "--dry-run", "--yes"]);
    let full_stdout = String::from_utf8_lossy(&full.stdout);
    assert!(
        full_stdout.contains("would    "),
        "an unnarrowed dry run must plan steps, got {full_stdout:?}"
    );
    let full_steps = full_stdout.matches("would    ").count();
    assert_eq!(full_steps, 2, "both entries get a step, got {full_stdout:?}");

    let narrowed = run_against(&conf, &["deps", "install", "--dry-run", "--yes", "--only", "ripgrep"]);
    let narrowed_stdout = String::from_utf8_lossy(&narrowed.stdout);
    let narrowed_steps = narrowed_stdout.matches("would    ").count();

    assert_eq!(
        narrowed_steps, 1,
        "--only leaves one step planned, got {narrowed_stdout:?}"
    );
}
