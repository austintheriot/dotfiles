//! `config reload` behavior, asserted against the built binary.

use std::process::Command;

/// The zsh line goes to stdout and the explanation goes to stderr.
///
/// The split is the contract, not formatting: a caller runs
/// `$(config reload)` and must get a runnable line, so the explanation
/// cannot be on stdout with it.
#[test]
fn the_runnable_line_is_on_stdout_and_the_explanation_on_stderr() {
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .arg("reload")
        .env_remove("TMUX")
        .output()
        .expect("the binary runs");

    let stdout = String::from_utf8_lossy(&run.stdout);
    let stderr = String::from_utf8_lossy(&run.stderr);

    // Positive control: both streams must carry something, or the
    // assertions below hold for a command that printed nothing.
    assert!(!stdout.is_empty(), "stdout must carry the runnable line");
    assert!(!stderr.is_empty(), "stderr must carry the explanation");

    assert!(
        stdout.contains("source ~/.zshrc"),
        "the runnable line belongs on stdout: {stdout:?}"
    );
    assert!(
        !stdout.contains("cannot re-source"),
        "the explanation must not pollute stdout, or $(config reload) breaks: {stdout:?}"
    );
    assert!(
        stderr.contains("cannot re-source"),
        "the explanation belongs on stderr: {stderr:?}"
    );
}

/// Outside tmux, the tmux half is skipped rather than failing.
#[test]
fn outside_tmux_the_tmux_half_is_skipped() {
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .arg("reload")
        .env_remove("TMUX")
        .output()
        .expect("the binary runs");

    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        !stdout.contains("reloaded tmux config"),
        "no TMUX means no tmux reload was attempted: {stdout:?}"
    );
}

/// `--help` prints usage and executes nothing.
///
/// The incident this guards: `config install-hooks --help` once linked the
/// hooks and rewrote ~/.local/bin/config before printing help.
#[test]
fn help_executes_nothing() {
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["reload", "--help"])
        .output()
        .expect("the binary runs");

    assert_eq!(run.status.code(), Some(0), "--help exits 0");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        stdout.contains("config reload"),
        "help must name the command as `config reload`, not `config-cli reload`: {stdout:?}"
    );
    assert!(
        !stdout.contains("source ~/.zshrc"),
        "--help must not print the payload, which would mean it ran: {stdout:?}"
    );
}
