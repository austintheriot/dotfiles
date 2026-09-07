//! The argv surface, asserted against the built binary.
//!
//! Exit 2 for every caller error is a repo-wide convention that
//! tests/deps-docs.test.sh relies on as its oracle for "the parser rejected
//! this flag", so these assertions protect that oracle rather than just this
//! binary.

use std::process::Command;

#[test]
fn no_arguments_is_a_usage_error() {
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .output()
        .expect("the binary runs");

    assert_eq!(run.status.code(), Some(2), "no arguments is exit 2");
}

#[test]
fn an_unknown_subcommand_is_a_usage_error_naming_the_offender() {
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .arg("bogus-verb")
        .output()
        .expect("the binary runs");

    assert_eq!(run.status.code(), Some(2), "an unknown subcommand is exit 2");
    let stderr = String::from_utf8_lossy(&run.stderr);

    // Positive control: stderr must carry something, or the naming
    // assertion below would hold for a binary that says nothing at all.
    assert!(!stderr.is_empty(), "a usage error must explain itself");
    assert!(
        stderr.contains("bogus-verb"),
        "the error must name the offending verb, got {stderr:?}"
    );
}

#[test]
fn deps_accepts_the_check_verb() {
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["deps", "check", "--help"])
        .output()
        .expect("the binary runs");

    assert_eq!(run.status.code(), Some(0), "--help on a real verb exits 0");
}
