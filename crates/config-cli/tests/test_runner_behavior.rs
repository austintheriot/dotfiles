//! `config test` behavior, asserted against the built binary.

use std::os::unix::fs::PermissionsExt;
use std::process::Command;

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

/// With no suite name, `config test` runs cargo's test harness over the
/// workspace, not a shell script under `tests/`.
///
/// Asserted through `CARGO` rather than through a real run: a nested
/// `cargo test` here would recurse into this very test binary. Pointing
/// `CARGO` at a recorder that prints its own argv and exits proves which
/// program and which arguments the subcommand chose.
#[test]
fn a_bare_run_invokes_cargo_test() {
    let (_recorder_dir, recorder) = argv_recorder();
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .arg("test")
        .env("CARGO", &recorder)
        .output()
        .expect("the binary runs");

    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        !stdout.is_empty(),
        "positive control: the recorder printed nothing, so no program ran"
    );
    assert!(
        stdout.contains("test") && stdout.contains("--locked"),
        "a bare run must invoke `cargo test --locked`: {stdout:?}"
    );
}

/// A suite name becomes a cargo test filter rather than a path to a shell
/// script, so a name that matches nothing is cargo's problem, not a missing
/// file.
#[test]
fn a_suite_name_becomes_a_cargo_filter() {
    let (_recorder_dir, recorder) = argv_recorder();
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["test", "definitely-not-a-real-suite"])
        .env("CARGO", &recorder)
        .output()
        .expect("the binary runs");

    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        !stdout.is_empty(),
        "positive control: the recorder printed nothing, so no program ran"
    );
    assert!(
        stdout.contains("definitely-not-a-real-suite"),
        "the suite name must reach cargo as a filter: {stdout:?}"
    );
    assert!(
        !stdout.contains(".test.sh"),
        "a suite name must not be resolved against tests/<name>.test.sh: \
         {stdout:?}"
    );
}

/// `--quiet` still reaches the runner, which is cargo's own `--quiet` now.
#[test]
fn quiet_reaches_the_runner() {
    let (_recorder_dir, recorder) = argv_recorder();
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["test", "-q"])
        .env("CARGO", &recorder)
        .output()
        .expect("the binary runs");

    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        !stdout.is_empty(),
        "positive control: the recorder printed nothing, so no program ran"
    );
    assert!(
        stdout.contains("--quiet"),
        "-q must reach the runner: {stdout:?}"
    );
}

/// A shell script that prints its own arguments one per line and exits 0.
/// Stands in for `cargo` so a test can read back which arguments
/// `config test` chose without running a nested build.
///
/// The returned `TempDir` owns the script and must stay alive for the whole
/// run: dropping it deletes the script, and the child would then fail to
/// spawn rather than record anything.
fn argv_recorder() -> (tempfile::TempDir, std::path::PathBuf) {
    let directory = tempfile::TempDir::new().expect("a recorder directory");
    let script = directory.path().join("cargo-recorder.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\nfor argument in \"$@\"; do printf '%s\\n' \"$argument\"; done\n",
    )
    .expect("the recorder is written");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("the recorder is executable");
    (directory, script)
}
