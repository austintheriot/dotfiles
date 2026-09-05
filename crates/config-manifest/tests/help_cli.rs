use std::process::Command;

use assert_cmd::prelude::*;

fn run(args: &[&str]) -> assert_cmd::assert::Assert {
    Command::cargo_bin("config-manifest")
        .expect("binary built")
        .args(args)
        .assert()
}

fn stdout_of(assert: &assert_cmd::assert::Assert) -> String {
    String::from_utf8(assert.get_output().stdout.clone()).expect("utf8")
}

fn stderr_of(assert: &assert_cmd::assert::Assert) -> String {
    String::from_utf8(assert.get_output().stderr.clone()).expect("utf8")
}

#[test]
fn help_lists_every_subcommand_with_a_description() {
    let assert = run(&["--help"]).success();
    let help = stdout_of(&assert);
    for entry in ["check", "sync", "--stamp"] {
        assert!(help.contains(entry), "--help omits {entry}: {help}");
    }
    assert!(
        help.contains("drift"),
        "--help does not describe what check does: {help}"
    );
    assert!(
        help.contains("shared"),
        "--help does not describe what sync does: {help}"
    );
}

#[test]
fn each_subcommand_has_its_own_help() {
    let check = run(&["check", "--help"]).success();
    let check_help = stdout_of(&check);
    assert!(
        check_help.contains("origin/mac"),
        "check --help does not name the default refs: {check_help}"
    );

    let sync = run(&["sync", "--help"]).success();
    let sync_help = stdout_of(&sync);
    for flag in ["--dry-run", "--to"] {
        assert!(
            sync_help.contains(flag),
            "sync --help omits {flag}: {sync_help}"
        );
    }
}

#[test]
fn version_prints_the_crate_version() {
    let assert = run(&["--version"]).success();
    let out = stdout_of(&assert);
    assert!(
        out.starts_with("config-manifest "),
        "--version is not name-prefixed: {out}"
    );
    assert!(
        out.trim()
            .split(' ')
            .nth(1)
            .is_some_and(|version| version.split('.').count() == 3),
        "--version does not print a three-part version: {out}"
    );
}

#[test]
fn no_arguments_is_a_usage_error_that_names_the_subcommands() {
    let assert = run(&[]).code(2);
    let message = stderr_of(&assert);
    assert!(
        message.contains("check") && message.contains("sync"),
        "bare invocation does not point at the subcommands: {message}"
    );
}

#[test]
fn an_unknown_subcommand_is_a_usage_error_naming_the_offender() {
    let assert = run(&["bogus"]).code(2);
    let message = stderr_of(&assert);
    assert!(
        message.contains("bogus"),
        "unknown subcommand error does not name it: {message}"
    );
}

#[test]
fn an_unknown_flag_names_the_offending_flag() {
    let assert = run(&["sync", "--bogus"]).code(2);
    let message = stderr_of(&assert);
    assert!(
        message.contains("--bogus"),
        "unknown flag error does not name it: {message}"
    );
}

#[test]
fn to_without_a_value_is_a_usage_error() {
    run(&["sync", "--to"]).code(2);
}

#[test]
fn stamp_and_version_stay_global_flags_not_subcommands() {
    // config-build and tests/config-manifest-lifecycle.test.sh both call
    // `config-manifest --stamp` and `config-manifest --version` directly. A
    // refactor that turned either into a subcommand would break the build
    // script and the pre-push stamp comparison.
    run(&["--stamp"]).success().stdout("unstamped\n");
    run(&["--version"])
        .success()
        .stdout(format!("config-manifest {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_documents_the_dotfiles_root_environment_variable() {
    // DOTFILES_ROOT is how every test and the Docker image point the binary at
    // a repo other than $HOME. It was invisible in the usage string before.
    let assert = run(&["--help"]).success();
    let help = stdout_of(&assert);
    assert!(
        help.contains("DOTFILES_ROOT"),
        "--help does not document DOTFILES_ROOT: {help}"
    );
}

#[test]
fn an_explicit_root_flag_overrides_the_environment_variable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let elsewhere = tempfile::tempdir().expect("tempdir");
    // Neither directory is a repo, so the run fails either way; what is under
    // test is which path the binary reports, not the outcome.
    let assert = Command::cargo_bin("config-manifest")
        .expect("binary built")
        .env("DOTFILES_ROOT", elsewhere.path())
        .args(["--root".as_ref(), dir.path().as_os_str()])
        .args(["check", "mac", "linux"])
        .assert()
        .failure();
    let message = stderr_of(&assert);
    assert!(
        !message.contains(elsewhere.path().to_str().expect("utf8")),
        "--root did not override DOTFILES_ROOT: {message}"
    );
}

#[test]
fn stamp_works_while_dotfiles_root_is_set() {
    // config-build runs `config-manifest --stamp` from a shell that already
    // exports DOTFILES_ROOT, and tests/config-manifest-lifecycle.test.sh does
    // the same. An env-backed --root that counts as "supplied" would make
    // --stamp collide with it and exit 2 instead of printing the stamp.
    Command::cargo_bin("config-manifest")
        .expect("binary built")
        .env("DOTFILES_ROOT", "/tmp")
        .arg("--stamp")
        .assert()
        .success()
        .stdout("unstamped\n");
}
