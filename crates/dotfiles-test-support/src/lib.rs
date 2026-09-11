#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
//! Test-support for the converted shell suites.
//!
//! Holds one thing: a runtime skip that the pre-push gate can count. Rust's
//! harness has no representation for "this check cannot run on this
//! machine": `#[ignore]` is a compile-time decision and hides the count, and
//! `eprintln!` is swallowed by `cargo test --quiet`, which is the command
//! `tests/rust-checks.sh` runs. Measured 2026-09-10: that command reported
//! zero skip lines from `nvim_runtime.rs`, which already skips this way.
//!
//! So a skip is recorded to a file whose path the gate sets, and the gate
//! reports the count. See the spec's section 4b.1.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The environment variable naming the skip log. Set by `tests/rust-checks.sh`.
pub const SKIP_LOG_VARIABLE: &str = "DOTFILES_SKIP_LOG";

/// Where skips are recorded, or `None` when no gate is listening.
#[must_use]
pub fn skip_log_path() -> Option<PathBuf> {
    std::env::var_os(SKIP_LOG_VARIABLE).map(PathBuf::from)
}

/// Records that a check could not run here, and why.
///
/// Call this and return early. It is a no-op when no log is configured, so a
/// developer running `cargo test` by hand needs no setup.
pub fn skip(reason: &str) {
    if let Some(log) = skip_log_path() {
        skip_to(&log, reason);
    }
}

/// Appends one skip to an explicit path. Public so the mechanism is testable
/// without mutating the environment, which parallel tests share.
pub fn skip_to(log: &Path, reason: &str) {
    let escaped = reason.replace('\\', "\\\\").replace('"', "\\\"");
    let line = format!("{{\"reason\":\"{escaped}\"}}\n");
    if let Ok(mut handle) = OpenOptions::new().create(true).append(true).open(log) {
        let _ = handle.write_all(line.as_bytes());
    }
}

/// The checkout under test, and the tracked files the converted suites read.
///
/// Promoted here on its second caller: `workflow_labels.rs` was the first and
/// the next YAML conversion is the second, so the helpers move rather than
/// being copied a third time.
///
/// Every helper here panics on a malformed checkout rather than returning an
/// error, so the crate root's `deny(clippy::expect_used)` is lifted for this
/// module alone. A caller is a test: a missing workflow directory or an
/// unparseable workflow has no recovery, and an `expect` message names the
/// broken file where a `?` would surface as an opaque harness error. Each
/// panic is documented in its own `# Panics` section.
#[allow(
    clippy::expect_used,
    reason = "a broken checkout is a test failure, and the message names it"
)]
pub mod repo {
    use std::path::{Path, PathBuf};

    /// The repository root, at run time.
    ///
    /// `DOTFILES_ROOT` first, then `HOME`, accepting either only when
    /// `crates/Cargo.toml` is under it. The manifest walk-up is the last
    /// resort and exists for the `rust-checks.sh` snapshot, where neither
    /// variable points at the archived tree.
    ///
    /// Not `CARGO_MANIFEST_DIR` alone: that is a compile-time constant, so a
    /// binary built in one tree and run against another reads the wrong root,
    /// which is how these tests passed on the host and failed under the gate.
    ///
    /// # Panics
    ///
    /// Panics when no candidate holds `crates/Cargo.toml` and the manifest
    /// directory has no grandparent, which means the crate was moved out of
    /// the workspace.
    #[must_use]
    pub fn root() -> PathBuf {
        for variable in ["DOTFILES_ROOT", "HOME"] {
            if let Some(value) = std::env::var_os(variable) {
                let candidate = PathBuf::from(value);
                if candidate.join("crates/Cargo.toml").is_file() {
                    return candidate;
                }
            }
        }
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("the repo root is two levels above this crate")
            .to_path_buf()
    }

    /// Every workflow file in this checkout, both extensions, sorted.
    #[must_use]
    pub fn workflow_files() -> Vec<PathBuf> {
        workflow_files_in(&root())
    }

    /// Every workflow file under an explicit root, both extensions, sorted.
    ///
    /// Takes the root so the helper is testable against a temporary tree
    /// rather than only against the checkout it happens to run in.
    #[must_use]
    pub fn workflow_files_in(root: &Path) -> Vec<PathBuf> {
        let directory = root.join(".github/workflows");
        let Ok(entries) = std::fs::read_dir(&directory) else {
            return Vec::new();
        };
        let mut found: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                matches!(
                    path.extension().and_then(|extension| extension.to_str()),
                    Some("yml" | "yaml")
                )
            })
            .collect();
        found.sort();
        found
    }

    /// Parses workflow text as YAML.
    ///
    /// # Panics
    ///
    /// Panics when the text is not YAML. A workflow that does not parse is a
    /// broken workflow, so there is nothing for a caller to recover from.
    #[must_use]
    pub fn parse_workflow(text: &str) -> yaml_serde::Value {
        yaml_serde::from_str(text).expect("a workflow parses as YAML")
    }

    /// Reads and parses one workflow file.
    ///
    /// # Panics
    ///
    /// Panics when the file is unreadable or is not YAML.
    #[must_use]
    pub fn read_workflow(path: &Path) -> yaml_serde::Value {
        let text = std::fs::read_to_string(path).expect("a workflow file is readable");
        parse_workflow(&text)
    }

    /// The file name of a path, for use in a failure message.
    ///
    /// # Panics
    ///
    /// Panics when the path has no UTF-8 file name.
    #[must_use]
    pub fn file_name(path: &Path) -> String {
        path.file_name()
            .and_then(|name| name.to_str())
            .expect("a workflow path has a UTF-8 file name")
            .to_string()
    }

    /// The jobs mapping of one workflow, as (key, job) pairs in file order.
    #[must_use]
    pub fn jobs_of(workflow: &yaml_serde::Value) -> Vec<(String, &yaml_serde::Value)> {
        let Some(jobs) = workflow.get("jobs").and_then(yaml_serde::Value::as_mapping) else {
            return Vec::new();
        };
        jobs.iter()
            .filter_map(|(key, job)| key.as_str().map(|key| (key.to_string(), job)))
            .collect()
    }

    /// The trigger mapping of one workflow.
    ///
    /// Reads the key both ways: `on` is the YAML 1.1 boolean `true`, which
    /// `yaml_serde` resolves before the mapping is ours to inspect.
    #[must_use]
    pub fn triggers_of(workflow: &yaml_serde::Value) -> Option<&yaml_serde::Value> {
        workflow
            .get("on")
            .or_else(|| workflow.get(yaml_serde::Value::Bool(true)))
    }

    /// Every `run:` script in a workflow's steps, concatenated.
    ///
    /// Scoped to the steps rather than the whole file, so a mention of a
    /// script inside a comment cannot satisfy an assertion that the workflow
    /// runs it.
    #[must_use]
    pub fn run_scripts(workflow: &yaml_serde::Value) -> String {
        jobs_of(workflow)
            .iter()
            .filter_map(|(_, job)| job.get("steps")?.as_sequence())
            .flatten()
            .filter_map(|step| step.get("run")?.as_str())
            .collect::<Vec<&str>>()
            .join("\n")
    }

    /// Every `uses:` value in a workflow's steps, with the job key that owns
    /// it, so a failure message can name where a pin lives.
    #[must_use]
    pub fn step_uses(workflow: &yaml_serde::Value) -> Vec<(String, String)> {
        jobs_of(workflow)
            .iter()
            .flat_map(|(key, job)| {
                let steps = job
                    .get("steps")
                    .and_then(yaml_serde::Value::as_sequence)
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                steps
                    .iter()
                    .filter_map(|step| step.get("uses")?.as_str())
                    .map(|uses| (key.clone(), uses.trim().to_string()))
                    .collect::<Vec<(String, String)>>()
            })
            .collect()
    }
}

/// Spawning the shell that Tranche B's suites take as their subject.
///
/// `expect` is allowed here for the same reason `repo` allows it: a helper
/// whose only caller is a test with no recovery path should panic with a
/// stated reason rather than thread a `Result` no one can act on.
#[allow(
    clippy::expect_used,
    reason = "a broken fixture is a panic, not a recoverable error"
)]
pub mod zsh {
    use std::path::Path;
    use std::process::{Command, Output};

    /// Whether a zsh is on PATH at all.
    #[must_use]
    pub fn available() -> bool {
        Command::new("zsh")
            .arg("-c")
            .arg("exit 0")
            .output()
            .is_ok_and(|output| output.status.success())
    }

    /// Spawns an interactive login zsh with an isolated environment.
    ///
    /// Both flags are load-bearing and neither is redundant. `-i` is what
    /// makes zsh source `.zshrc` at all: a non-interactive login shell reads
    /// `.zprofile` and `.zlogin` and skips `.zshrc` entirely, so a `-l -c`
    /// fixture observes none of the config this tranche is about. Measured
    /// 2026-09-10: `zsh -l -c 'whence -w parse_git_dirty'` prints `none` and
    /// `zsh -i -c` prints `function`. `-l` is kept because
    /// `tests/zshrc-node-startup.test.sh:193` spells both, so the fixture
    /// reproduces the startup path the shell suites measured.
    ///
    /// The environment is CLEARED rather than inherited. Without that,
    /// `DOTFILES_PLATFORM` and `NVM_DIR` arrive from the developer's own
    /// shell and any assertion of the form "the config produced X" is
    /// satisfied by the caller instead of by the config. Three shell suites
    /// already guard against this deliberately, and
    /// `zshrc-node-startup.test.sh:189` calls `env -i` "load-bearing" in its
    /// own comment. Only `HOME`, `PATH` and `TERM` pass through: `HOME` is
    /// where zsh finds the startup files, `PATH` is what the config's own
    /// resolution assertions are about, and `TERM` keeps an interactive shell
    /// from complaining about an unknown terminal.
    ///
    /// # Panics
    ///
    /// Panics when zsh cannot be spawned. Callers guard with [`available`],
    /// so reaching this means the shell vanished mid-run.
    fn spawn_with_path(home: &Path, path: &str, script: &str) -> Output {
        Command::new("zsh")
            .args(["-l", "-i", "-c"])
            .arg(script)
            .env_clear()
            .env("HOME", home)
            .env("PATH", path)
            .env("TERM", "xterm")
            .output()
            .expect("zsh spawns")
    }

    /// The caller's `PATH`, or a minimal one when it has none.
    fn inherited_path() -> String {
        std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_string())
    }

    fn spawn(home: &Path, script: &str) -> Output {
        spawn_with_path(home, &inherited_path(), script)
    }

    /// The same shell with an explicit `PATH`.
    ///
    /// `PATH` is the one variable a startup-resolution test cannot let the
    /// caller decide. `.zshrc` skips its shim prepend when the shims are on
    /// `PATH` anywhere, so a caller whose `PATH` already carries them further
    /// back leaves them there, behind whatever came first. Run from a shell
    /// with Homebrew ahead of the pyenv shims, the shell suite's two
    /// resolution assertions failed while a real login shell on the same
    /// machine resolved correctly: same config, two answers, decided by the
    /// caller. `zshrc-python-startup.test.sh:110` pinned
    /// `/usr/bin:/bin:/usr/sbin:/sbin` for exactly that block, which is what
    /// `path_helper` starts a login shell with.
    ///
    /// # Panics
    ///
    /// Panics when zsh cannot be spawned.
    #[must_use]
    pub fn run_with_path(path: &str, script: &str) -> Output {
        spawn_with_path(&super::repo::root(), path, script)
    }

    /// An interactive login shell in the repo itself, so the tracked
    /// `.zshrc` loads.
    ///
    /// # Panics
    ///
    /// Panics when zsh cannot be spawned.
    #[must_use]
    pub fn run(script: &str) -> Output {
        spawn(&super::repo::root(), script)
    }

    /// The same shell with a fixture `$HOME`, so a test can assert that a
    /// variant is NOT loaded without depending on the machine it runs on.
    ///
    /// # Panics
    ///
    /// Panics when zsh cannot be spawned.
    #[must_use]
    pub fn run_in_home(home: &Path, script: &str) -> Output {
        spawn(home, script)
    }
}

/// Writing stub executables that a test is about to run.
///
/// The suites that drive `config` build fixture homes full of shell stubs and
/// then execute them immediately. That write-then-exec sequence races: an
/// `exec` of a file the kernel still considers open for writing fails with
/// `ETXTBSY` ("Text file busy", errno 26), and the suite fails on the spawn
/// rather than on anything it meant to assert.
///
/// Observed once in nine full push-gate runs, only inside the Linux test
/// container, naming a different test each time. The precise trigger is NOT
/// established: three probes (a write-then-exec loop on macOS, the same on
/// Linux, and a cross-thread fork-inheritance probe on Linux) each reported
/// zero occurrences. What is established is the failing syscall and that the
/// failure is transient, which is what this module answers.
pub mod stub {
    use std::io;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::process::{Command, Output};
    use std::time::Duration;

    /// How many times [`run`] re-attempts a spawn that lost the race.
    ///
    /// Small on purpose. A real `ETXTBSY` clears as soon as the writing
    /// descriptor closes, so one retry almost always suffices; a large budget
    /// would turn a genuine "this file is permanently held open" defect into a
    /// slow test rather than a failing one.
    const ATTEMPTS: u32 = 5;

    /// Writes `body` at `path` and marks it executable.
    ///
    /// Creates the parent directory, so a caller can name a stub inside a
    /// fixture tree that does not exist yet.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`io::Error`] when the directory, the file, or
    /// the permission change fails.
    pub fn write(path: &Path, body: &str) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, body)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
    }

    /// Runs `command`, re-attempting while the OS reports `ETXTBSY`.
    ///
    /// Every other error, and every non-zero exit, is returned unchanged: this
    /// retries a spawn that could not start, never a program that ran and
    /// failed. A test asserting on a failing exit status still sees it.
    ///
    /// # Errors
    ///
    /// Returns the spawn error when it is not `ETXTBSY`, or when `ETXTBSY`
    /// persists past [`ATTEMPTS`].
    pub fn run(command: &mut Command) -> io::Result<Output> {
        let mut backoff = Duration::from_millis(5);
        for attempt in 1..=ATTEMPTS {
            match command.output() {
                Ok(output) => return Ok(output),
                Err(error) if is_text_file_busy(&error) && attempt < ATTEMPTS => {
                    std::thread::sleep(backoff);
                    backoff *= 2;
                }
                Err(error) => return Err(error),
            }
        }
        // ATTEMPTS is a non-zero constant, so the loop above always returns.
        Err(io::Error::other("the retry loop did not run"))
    }

    /// Whether this spawn failure is the write-then-exec race.
    fn is_text_file_busy(error: &io::Error) -> bool {
        error.raw_os_error() == Some(26)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn write_creates_a_runnable_stub_in_a_missing_directory() {
            let home = tempfile::tempdir().expect("a temporary directory");
            let stub = home.path().join("nested/deeper/stub.sh");
            write(&stub, "#!/bin/sh\nprintf ran\n").expect("the stub is writable");

            let output = run(&mut Command::new(&stub)).expect("the stub runs");
            assert!(output.status.success(), "the stub did not exit 0");
            assert_eq!(String::from_utf8_lossy(&output.stdout), "ran");
        }

        /// A non-zero exit is a RESULT, not a spawn failure, so it comes back
        /// intact rather than being retried away.
        #[test]
        fn run_returns_a_failing_exit_status_untouched() {
            let home = tempfile::tempdir().expect("a temporary directory");
            let stub = home.path().join("fails.sh");
            write(&stub, "#!/bin/sh\nexit 3\n").expect("the stub is writable");

            let output = run(&mut Command::new(&stub)).expect("the stub runs");
            assert_eq!(output.status.code(), Some(3), "the exit status was rewritten");
        }

        /// The retry must not swallow a real error. A missing program is
        /// ENOENT, not ETXTBSY, so it returns immediately.
        #[test]
        fn run_reports_a_missing_program_rather_than_retrying() {
            let home = tempfile::tempdir().expect("a temporary directory");
            let absent = home.path().join("no-such-stub");

            let error = run(&mut Command::new(&absent)).expect_err("a missing program cannot run");
            assert!(!is_text_file_busy(&error), "ENOENT was misread as the race");
        }

        #[test]
        fn text_file_busy_is_recognised_and_other_errors_are_not() {
            assert!(is_text_file_busy(&io::Error::from_raw_os_error(26)));
            assert!(!is_text_file_busy(&io::Error::from_raw_os_error(2)));
        }
    }
}
