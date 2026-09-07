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
    for entry in ["verify-stamps", "doctor", "--stamp"] {
        assert!(help.contains(entry), "--help omits {entry}: {help}");
    }
    assert!(
        help.contains("stamp"),
        "--help does not describe what verify-stamps does: {help}"
    );
    assert!(
        help.contains("source"),
        "--help does not describe what doctor does: {help}"
    );
}

#[test]
fn each_subcommand_has_its_own_help() {
    let verify_stamps = run(&["verify-stamps", "--help"]).success();
    let verify_stamps_help = stdout_of(&verify_stamps);
    assert!(
        verify_stamps_help.contains("--ref"),
        "verify-stamps --help omits --ref: {verify_stamps_help}"
    );

    let doctor = run(&["doctor", "--help"]).success();
    let doctor_help = stdout_of(&doctor);
    assert!(
        doctor_help.contains("doctor"),
        "doctor --help does not describe the subcommand: {doctor_help}"
    );
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
        message.contains("verify-stamps") && message.contains("doctor"),
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
    let assert = run(&["verify-stamps", "--bogus"]).code(2);
    let message = stderr_of(&assert);
    assert!(
        message.contains("--bogus"),
        "unknown flag error does not name it: {message}"
    );
}

#[test]
fn ref_without_a_value_is_a_usage_error() {
    run(&["verify-stamps", "--ref"]).code(2);
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
        .args(["doctor"])
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

#[test]
fn describe_prints_one_line_and_succeeds() {
    // config-help formats each description with `printf '  %-14s %s\n'`, so a
    // second line or a leading space shifts every row after it. The line count
    // is the assertion, not just the presence of text.
    let assert = run(&["--describe"]).success();
    let description = stdout_of(&assert);
    assert!(
        !description.trim().is_empty(),
        "--describe printed nothing: {description:?}"
    );
    assert_eq!(
        description.lines().count(),
        1,
        "--describe printed more than one line: {description:?}"
    );
    assert!(
        description.ends_with('\n'),
        "--describe did not end with a newline: {description:?}"
    );
    assert!(
        !description.starts_with(char::is_whitespace),
        "--describe indented its line, which the %-14s column already does: {description:?}"
    );
}

#[test]
fn describe_writes_nothing_to_stderr() {
    // A warning on stderr lands in the same terminal as the listing, which is
    // the failure mode the shell side replaced: `sed` on a binary printed
    // "RE error: illegal byte sequence" while the pipeline still exited 0.
    let assert = run(&["--describe"]).success();
    let noise = stderr_of(&assert);
    assert!(noise.is_empty(), "--describe wrote to stderr: {noise:?}");
}

#[test]
fn describe_stays_a_global_flag_not_a_subcommand() {
    // Same constraint --stamp and --version carry: config-help invokes
    // `config-<sub> --describe`, with no subcommand word to put in front of it.
    let assert = run(&["--help"]).success();
    let help = stdout_of(&assert);
    assert!(help.contains("--describe"), "--help omits --describe: {help}");
}

#[test]
fn describe_works_while_dotfiles_root_is_set() {
    // --root is env-backed, so a shell that exports DOTFILES_ROOT makes clap
    // treat --root as supplied. An `exclusive` --describe would collide with it
    // and exit 2, which is the bug the --stamp comment already records.
    Command::cargo_bin("config-manifest")
        .expect("binary built")
        .env("DOTFILES_ROOT", "/tmp")
        .arg("--describe")
        .assert()
        .success();
}

#[test]
fn describe_does_no_work() {
    // Asking a command what it does must not be the same as doing it. Pointed
    // at a directory that is not a repo, `doctor` fails; `--describe` must
    // succeed there, which proves it returns before the gather.
    let empty = tempfile::tempdir().expect("tempdir");
    Command::cargo_bin("config-manifest")
        .expect("binary built")
        .args(["--root".as_ref(), empty.path().as_os_str()])
        .arg("--describe")
        .assert()
        .success();
    Command::cargo_bin("config-manifest")
        .expect("binary built")
        .args(["--root".as_ref(), empty.path().as_os_str()])
        .arg("doctor")
        .assert()
        .failure();
}
