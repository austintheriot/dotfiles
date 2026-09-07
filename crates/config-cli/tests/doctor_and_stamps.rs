//! `config doctor` and `config-cli verify-stamps`, asserted against the built
//! binary.
//!
//! Both subcommands were `config-manifest` subcommands before this suite
//! existed. `verify-stamps` is what the pre-push hook runs, so its exit codes
//! and its stdin interface are a gate contract rather than a preference: a
//! silent success on an unreadable ref would let a stale binary through, and a
//! failure that explains nothing would block a push with no way to act on it.

use std::io::Write;
use std::process::{Command, Stdio};

/// `config doctor` reports what the config-manifest binary reported.
#[test]
fn doctor_reports_installed_binary_drift() {
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .arg("doctor")
        .output()
        .expect("the binary runs");

    // Positive control before the contract. Exit 2 is clap's answer to a verb
    // it does not know, so accepting it here would let this test pass against
    // a binary that has no doctor subcommand at all, which is exactly the
    // state this suite was written to leave.
    let code = run.status.code();
    assert!(code.is_some(), "doctor must exit, not be killed by a signal");
    assert_ne!(
        code,
        Some(2),
        "exit 2 means clap rejected the verb, not that doctor ran: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    // On a clean machine doctor exits 0 and prints nothing on stdout, which is
    // the contract tests/config.test.sh asserts and the habitual-run promise
    // in the shim's own help text. Exit 1 is the stale-binary report, which is
    // a legitimate answer on a tree mid-edit, so only the silence of a clean
    // run is asserted.
    assert!(
        code == Some(0) || code == Some(1),
        "doctor answers 0 for clean or 1 for stale, got {code:?}: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    if code == Some(0) {
        assert!(
            run.stdout.is_empty(),
            "a clean doctor prints nothing on stdout, or the gate's silence check breaks"
        );
    }
}

/// `config doctor --help` renders the name a caller actually types.
#[test]
fn doctor_help_names_the_dispatcher_spelling() {
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["doctor", "--help"])
        .output()
        .expect("the binary runs");

    assert_eq!(run.status.code(), Some(0), "--help on a real verb exits 0");
    let help = String::from_utf8_lossy(&run.stdout);
    assert!(!help.is_empty(), "--help must print something");
    assert!(
        help.contains("config doctor"),
        "help must name the dispatcher spelling, not config-cli doctor, got {help:?}"
    );
}

/// `verify-stamps` rejects a ref whose stamps do not match.
#[test]
fn verify_stamps_rejects_a_mismatch() {
    let run = spawn_verify_stamps(&["--ref", "definitely-not-a-ref"], "");

    assert_ne!(
        run.status.code(),
        Some(0),
        "an unresolvable ref is not success"
    );
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(!stderr.is_empty(), "a failure must explain itself");
    assert!(
        stderr.contains("definitely-not-a-ref"),
        "the refusal must name the ref it refused, got {stderr:?}"
    );
}

/// `verify-stamps` reads the pushed stamps from stdin, as pre-push pipes them.
#[test]
fn verify_stamps_reads_its_stamps_from_stdin() {
    // A real workspace binary, so `has_installed_binary` keeps it on the
    // pushed side rather than dropping it as a library. The stamp is a
    // fabricated one no installed binary can report, so the comparison has a
    // verdict to reach and reaching it proves stdin was read at all. A crate
    // name with no `src/main.rs` would be filtered out of both sides and the
    // run would be indistinguishable from empty stdin.
    let pushed = "config-cli deadbeef:deadbeef:deadbeef\n";
    let run = spawn_verify_stamps(&["--ref", "HEAD"], pushed);

    assert!(
        run.status.code().is_some(),
        "verify-stamps must exit, not be killed by a signal"
    );
    // Positive control: an empty stdin must not produce the same answer, or
    // this test would pass against a subcommand that ignores stdin entirely.
    let empty = spawn_verify_stamps(&["--ref", "HEAD"], "");
    assert_ne!(
        (run.status.code(), run.stdout.clone(), run.stderr.clone()),
        (
            empty.status.code(),
            empty.stdout.clone(),
            empty.stderr.clone()
        ),
        "stdin content must change the answer, or stdin is not being read"
    );
}

/// `--ref` is required, because a refusal that cannot name a ref is unactionable.
#[test]
fn verify_stamps_requires_a_ref() {
    let run = spawn_verify_stamps(&[], "");

    assert_eq!(
        run.status.code(),
        Some(2),
        "a missing required flag is exit 2"
    );
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains("--ref"),
        "the usage error must name the missing flag, got {stderr:?}"
    );
}

/// Runs `config-cli verify-stamps` with the given arguments and stdin text.
fn spawn_verify_stamps(arguments: &[&str], stdin_text: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .arg("verify-stamps")
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary runs");
    {
        let stdin = child.stdin.as_mut().expect("stdin is piped");
        stdin
            .write_all(stdin_text.as_bytes())
            .expect("stdin accepts the pushed stamps");
    }
    child.wait_with_output().expect("the binary exits")
}

/// An explicit `--root` overrides the environment variable.
///
/// Ported from the deleted `config-manifest` binary's own suite. `--root` is
/// env-backed, so a shell that exports `DOTFILES_ROOT` supplies it; the flag
/// has to win, or a test pointing at a fixture repo would silently diagnose
/// the ambient one.
#[test]
fn an_explicit_root_flag_overrides_the_environment_variable() {
    let target = tempfile::tempdir().expect("tempdir");
    let elsewhere = tempfile::tempdir().expect("tempdir");
    // Neither directory is a repo, so the run fails either way; what is under
    // test is which path the command reports, not the outcome.
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .env("DOTFILES_ROOT", elsewhere.path())
        .args(["doctor", "--root"])
        .arg(target.path())
        .output()
        .expect("the binary runs");

    let message = String::from_utf8_lossy(&run.stderr);
    // Positive control: the failure must say something, or the absence check
    // below would hold for a command that printed nothing at all.
    assert!(!message.is_empty(), "a failed gather must explain itself");
    assert!(
        !message.contains(elsewhere.path().to_str().expect("utf8")),
        "--root did not override DOTFILES_ROOT, got {message:?}"
    );
}

/// `--stamp` still works while `DOTFILES_ROOT` is exported.
///
/// `config-build` runs `config-cli --stamp` from a shell that already exports
/// `DOTFILES_ROOT`. An env-backed `--root` that counted as "supplied" would
/// make an exclusive `--stamp` collide with it and exit 2 instead of printing
/// the stamp, which is the bug the flag's own comment records.
#[test]
fn stamp_works_while_dotfiles_root_is_set() {
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .env("DOTFILES_ROOT", "/tmp")
        .arg("--stamp")
        .output()
        .expect("the binary runs");

    assert_eq!(run.status.code(), Some(0), "--stamp exits 0");
    let stamp = String::from_utf8_lossy(&run.stdout);
    assert!(!stamp.trim().is_empty(), "--stamp printed nothing");
}

/// An unknown flag on a subcommand is a usage error that names the flag.
#[test]
fn an_unknown_flag_names_the_offending_flag() {
    let run = spawn_verify_stamps(&["--bogus-flag"], "");

    assert_eq!(run.status.code(), Some(2), "an unknown flag is exit 2");
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains("--bogus-flag"),
        "the error must name the offending flag, got {stderr:?}"
    );
}
