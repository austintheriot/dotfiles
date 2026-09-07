//! `config test`: run the test suite, optionally in Docker or in a watch
//! loop.
//!
//! Ported from `.scripts/config/config-test`. Argument validation lives
//! here because the combinations that are usage errors (`-q` with a suite
//! name, `-w` with `-d`) have to be rejected before anything runs. Suite
//! selection itself stays in `tests/run-all.sh`: that script already
//! resolves a bare name against `tests/<name>.test.sh`, and a second copy of
//! that resolution here is exactly the drift this migration exists to
//! remove.

use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::time::Duration;

/// `config test`'s arguments: the same four the shell script accepted.
///
/// Runs every suite in `~/tests` by default. Name a suite to run only that
/// one: `config test deps-manifest` runs `tests/deps-manifest.test.sh`.
///
/// `--quiet` is honored by the whole-suite run only, so it cannot be
/// combined with `--docker` or a suite name. `--watch` cannot be combined
/// with `--docker`, which would rebuild the image on every change.
///
/// Exits 2 on a usage error, otherwise the exit status of the suite.
#[derive(clap::Args)]
#[command(override_usage = "config test [-q|--quiet] [-d|--docker] [-w|--watch] [suite]")]
pub(crate) struct TestArgs {
    /// Print only failures and the summary, not every passing assertion.
    #[arg(short, long)]
    quiet: bool,
    /// Run in the Debian container from tests/docker/, the same image the
    /// pre-push hook uses. Git-dependent assertions skip there, because the
    /// container has no repository.
    #[arg(short, long)]
    docker: bool,
    /// Rerun after every change to a tracked file. Polls a checksum once a
    /// second.
    #[arg(short, long)]
    watch: bool,
    /// Run only this suite, by name, with or without the `.test.sh` suffix.
    suite: Option<String>,
}

/// Run `config test`.
///
/// Returns `ExitCode::from(2)` for a usage error, and otherwise the exit
/// status of whichever runner the arguments selected.
pub(crate) fn run(arguments: TestArgs) -> ExitCode {
    if arguments.quiet && (arguments.docker || arguments.suite.is_some()) {
        return usage_error();
    }
    if arguments.watch && arguments.docker {
        return usage_error();
    }

    let Some(home) = std::env::var_os("HOME") else {
        eprintln!("config test: $HOME is not set");
        return ExitCode::FAILURE;
    };
    let home = PathBuf::from(home);

    if !arguments.watch {
        return run_once(&home, &arguments);
    }

    watch(&home, &arguments)
}

fn usage_error() -> ExitCode {
    eprintln!("usage: config test [-q|--quiet] [-d|--docker] [-w|--watch] [suite]");
    ExitCode::from(2)
}

/// Runs the suite exactly once, through whichever of the three runners the
/// arguments selected, and returns its exit status.
fn run_once(home: &std::path::Path, arguments: &TestArgs) -> ExitCode {
    let (program, mut command) = if arguments.docker {
        let program = home.join("tests/run-in-docker.sh");
        let mut command = Command::new(&program);
        if let Some(suite) = &arguments.suite {
            command.arg(suite);
        }
        (program, command)
    } else if let Some(suite) = &arguments.suite {
        let program = home.join(format!("tests/{suite}.test.sh"));
        (program.clone(), Command::new(program))
    } else {
        let program = home.join("tests/run-all.sh");
        let mut command = Command::new(&program);
        if arguments.quiet {
            command.arg("-q");
        }
        (program, command)
    };

    match command.status() {
        Ok(status) => exit_code_from(&status),
        Err(error) => {
            // The shell script this replaces execs the suite script
            // directly, so a missing file surfaces as the shell's own "No
            // such file or directory" naming the full path. Matched here so
            // a typo'd suite name is still named back to the caller.
            eprintln!("config test: {}: {error}", program.display());
            ExitCode::FAILURE
        }
    }
}

/// Reruns the suite after every change to a tracked file, polling a
/// checksum once a second, matching the shell script's loop.
///
/// Never returns on its own: the loop is interrupted by the caller (a
/// signal), matching the shell script, which had no exit condition of its
/// own beyond that.
fn watch(home: &std::path::Path, arguments: &TestArgs) -> ExitCode {
    let mut last = fingerprint(home);
    let _ = run_once(home, arguments);
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let current = fingerprint(home);
        if current != last {
            last = current;
            let _ = run_once(home, arguments);
        }
    }
}

/// A checksum over every tracked file's content, tracked files only: the
/// worktree is the whole home directory, so listing untracked files walks
/// all of it and takes minutes. Matches the shell script's `git ls-files
/// --cached | xargs cksum | cksum` pipeline.
fn fingerprint(home: &std::path::Path) -> Option<Vec<u8>> {
    let git_dir = home.join(".cfg");
    let list = Command::new("git")
        .arg("--git-dir")
        .arg(&git_dir)
        .arg("--work-tree")
        .arg(home)
        .args(["ls-files", "-z", "--cached"])
        .stdout(Stdio::piped())
        .output()
        .ok()?;

    let mut hasher_input = Vec::new();
    for relative_path in list.stdout.split(|byte| *byte == 0) {
        if relative_path.is_empty() {
            continue;
        }
        let path = home.join(String::from_utf8_lossy(relative_path).as_ref());
        if let Ok(contents) = std::fs::read(&path) {
            hasher_input.extend_from_slice(&contents);
        }
    }
    Some(hasher_input)
}

/// Converts a child process's exit status into an `ExitCode`, matching the
/// shell script's `exec_status=$?`. A process killed by a signal has no
/// exit code on Unix, so that case is reported as a generic failure rather
/// than invented as zero.
fn exit_code_from(status: &std::process::ExitStatus) -> ExitCode {
    match status.code() {
        Some(code) => {
            let byte = u8::try_from(code.rem_euclid(256)).unwrap_or(1);
            ExitCode::from(byte)
        }
        None => ExitCode::FAILURE,
    }
}
