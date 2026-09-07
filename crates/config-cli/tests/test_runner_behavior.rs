//! `config test` behavior, asserted against the built binary.

use std::process::Command;

/// A named suite is forwarded, not swallowed.
#[test]
fn a_named_suite_reaches_the_runner() {
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["test", "definitely-not-a-real-suite"])
        .output()
        .expect("the binary runs");

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );

    // Positive control: the command must say something, or the naming
    // assertion below holds for a silent no-op.
    assert!(!combined.is_empty(), "a bad suite name must be reported");
    assert!(
        combined.contains("definitely-not-a-real-suite"),
        "the suite name must reach the runner and be named back: {combined:?}"
    );
    assert_ne!(run.status.code(), Some(0), "an unknown suite is not success");
}

/// `--help` executes no tests.
#[test]
fn help_runs_no_tests() {
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["test", "--help"])
        .output()
        .expect("the binary runs");

    assert_eq!(run.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        stdout.contains("config test"),
        "help names `config test`: {stdout:?}"
    );
    assert!(
        !stdout.contains("suite(s) passed"),
        "--help must not have run the suite: {stdout:?}"
    );
}

/// `-q` combined with a suite name is a usage error, matching the shell
/// script: quiet mode is only honored by the whole-suite run.
#[test]
fn quiet_with_a_suite_name_is_a_usage_error() {
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["test", "-q", "some-suite"])
        .output()
        .expect("the binary runs");

    assert_eq!(
        run.status.code(),
        Some(2),
        "quiet plus a suite name is exit 2"
    );
}

/// `-w` combined with `-d` is a usage error: a watch loop would rebuild the
/// docker image on every change.
#[test]
fn watch_with_docker_is_a_usage_error() {
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["test", "-w", "-d"])
        .output()
        .expect("the binary runs");

    assert_eq!(run.status.code(), Some(2), "watch plus docker is exit 2");
}

/// Two suite names is a usage error, not a silent pick of the first.
#[test]
fn two_suite_names_is_a_usage_error() {
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["test", "one-suite", "two-suite"])
        .output()
        .expect("the binary runs");

    assert_eq!(run.status.code(), Some(2), "two suite names is exit 2");
}

/// An unrecognized flag is a usage error naming nothing more than the
/// standard usage line, matching the shell script's `-*)` catch-all.
#[test]
fn an_unknown_flag_is_a_usage_error() {
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["test", "--bogus-flag"])
        .output()
        .expect("the binary runs");

    assert_eq!(run.status.code(), Some(2), "an unknown flag is exit 2");
}
