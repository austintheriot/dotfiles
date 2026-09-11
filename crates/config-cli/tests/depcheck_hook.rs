//! `deps/depcheck-hook.sh` is the 24-hour-throttled shell-startup nag.
//!
//! The hook resolves `config` through `~/.local/bin` rather than through
//! `PATH`, because `.zshrc` can source it before that directory is on `PATH`.
//! So every test runs it under an isolated `$HOME` holding a stub `config`
//! that logs each invocation. That is what makes "the throttle skipped the
//! check" observable at all: wall clock cannot distinguish it, because this
//! machine's real shell startup is dominated by nvm.
//!
//! The hook is sourced by an interactive zsh in real use, so it is sourced by
//! zsh here too. Two of these cases exist because zsh's arithmetic reacts to
//! a malformed cache differently than `sh`'s would.
//!
//! Converted whole from `tests/depcheck-hook.test.sh`, which ran **27**
//! assertions, measured by running it. The plan for this tranche says 21,
//! which is the fifth count error of the day.
//!
//! One assertion pair changed shape deliberately. The shell suite checked
//! POSIX portability with `sh -n` and `zsh -n`, PARSE checks, and
//! `.claude/rules/dotfiles-tests.md` records that a parse check is not an
//! execution check. Here the hook is also SOURCED by a real `sh` and its
//! output compared against zsh's, which catches a zsh-only construct that
//! parses fine everywhere and behaves differently at run time.

use dotfiles_test_support::repo::root as repo_root;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// The hook under test.
fn hook() -> PathBuf {
    repo_root().join("deps/depcheck-hook.sh")
}

fn write_executable(path: &Path, body: &str) {
    dotfiles_test_support::stub::write(path, body).expect("the stub is writable");
}

/// An isolated `$HOME` with a stub `config` whose exit status the caller
/// fixes, plus a copy of the hook the shell will source.
struct Home {
    directory: TempDir,
}

impl Home {
    /// A fresh home whose stub `config` exits with `exit_status` and appends
    /// one line to the invocation log each time it runs.
    fn new(exit_status: i32) -> Self {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let home = directory.path();
        fs::create_dir_all(home.join(".cache")).expect("the cache directory is creatable");

        let log = home.join("invocations.log");
        write_executable(
            &home.join(".local/bin/config"),
            &format!(
                "#!/bin/sh\nprintf 'invoked\\n' >> '{}'\nexit {exit_status}\n",
                log.display()
            ),
        );
        fs::write(&log, "").expect("the log is writable");

        fs::create_dir_all(home.join("deps")).expect("the deps directory is creatable");
        fs::copy(hook(), home.join("deps/depcheck-hook.sh")).expect("the hook is copyable");

        Self { directory }
    }

    fn path(&self) -> &Path {
        self.directory.path()
    }

    /// Replaces the stub with one that records its argv instead of a count.
    fn record_arguments_instead(&self, exit_status: i32) {
        write_executable(
            &self.path().join(".local/bin/config"),
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexit {exit_status}\n",
                self.path().join("args.log").display()
            ),
        );
    }

    fn recorded_arguments(&self) -> String {
        fs::read_to_string(self.path().join("args.log"))
            .unwrap_or_default()
            .trim_end_matches('\n')
            .to_string()
    }

    fn invocations(&self) -> usize {
        fs::read_to_string(self.path().join("invocations.log"))
            .unwrap_or_default()
            .lines()
            .filter(|line| line.contains("invoked"))
            .count()
    }

    fn cache_path(&self) -> PathBuf {
        self.path().join(".cache/depcheck-last-run")
    }

    fn write_cache(&self, contents: &str) {
        fs::write(self.cache_path(), contents).expect("the cache is writable");
    }

    /// Sources the hook in a non-interactive zsh under this home, returning
    /// stdout.
    fn run(&self) -> String {
        let output = self.spawn("zsh", &["-c"]);
        String::from_utf8_lossy(&output.stdout)
            .trim_end_matches('\n')
            .to_string()
    }

    /// Sources the hook and returns only what it wrote to stderr, which must
    /// always be empty because this runs during shell startup.
    ///
    /// Truncated, because a failure here reports the offending cache value
    /// back and one fixture is deliberately 200KB long.
    fn run_stderr(&self) -> String {
        let output = self.spawn("zsh", &["-c"]);
        String::from_utf8_lossy(&output.stderr)
            .chars()
            .take(120)
            .collect()
    }

    fn spawn(&self, shell: &str, flags: &[&str]) -> std::process::Output {
        Command::new(shell)
            .args(flags)
            .arg(format!(
                ". '{}'",
                self.path().join("deps/depcheck-hook.sh").display()
            ))
            .env("HOME", self.path())
            .output()
            .unwrap_or_else(|_| panic!("{shell} runs"))
    }
}

/// A cold cache runs the check, nags, and records the timestamp.
#[test]
fn a_cold_cache_runs_the_check_and_nags() {
    let home = Home::new(1);

    let output = home.run();
    assert!(
        output.contains("depcheck: missing dependencies detected"),
        "a cold cache did not nag: {output:?}"
    );
    assert_eq!(
        home.invocations(),
        1,
        "a cold cache did not run the check exactly once"
    );
    assert!(
        fs::metadata(home.cache_path()).is_ok_and(|data| data.len() > 0),
        "a cold cache did not write the timestamp, so the throttle would \
         never engage and every shell would pay for the check"
    );
}

/// A fresh cache skips the check entirely; a cache older than 24 hours does
/// not.
///
/// Both directions in one test, because "skipped" is only meaningful against
/// a run that is known to happen. Asserting the skip alone would pass on a
/// hook that never checks anything.
#[test]
fn the_throttle_skips_within_a_day_and_expires_after_one() {
    let home = Home::new(1);

    home.run();
    assert_eq!(home.invocations(), 1, "the first run did not check");

    let fresh = home.run();
    assert_eq!(fresh, "", "a fresh cache nagged: {fresh:?}");
    assert_eq!(
        home.invocations(),
        1,
        "a fresh cache re-ran the check, so the throttle does nothing"
    );

    let long_ago = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock after the epoch")
        .as_secs()
        .saturating_sub(90_000);
    home.write_cache(&format!("{long_ago}\n"));

    let stale = home.run();
    assert!(
        stale.contains("depcheck: missing dependencies detected"),
        "a stale cache did not nag again: {stale:?}"
    );
    assert_eq!(
        home.invocations(),
        2,
        "a stale cache did not re-run the check, so the nag would never \
         return after the first day"
    );
}

/// Nothing missing means no nag, but the check still ran.
#[test]
fn a_passing_check_runs_and_stays_silent() {
    let home = Home::new(0);

    let output = home.run();
    assert_eq!(output, "", "a passing check nagged: {output:?}");
    assert_eq!(
        home.invocations(),
        1,
        "a passing check never ran, so the silence proves nothing"
    );
}

/// An unbuilt engine says so, rather than blaming the dependencies.
///
/// 127 is "the engine is not on disk", which on a fresh clone is the normal
/// state until `config build` runs. Reporting it as missing dependencies
/// sends the reader to `depcheck`, which cannot run either, so the message
/// has to name the real problem. The nag for a genuinely missing dependency
/// is asserted elsewhere, so the two branches are distinguished rather than
/// one being asserted alone.
#[test]
fn an_unbuilt_engine_names_itself() {
    let home = Home::new(127);

    let output = home.run();
    assert!(
        output.contains("depcheck: the deps engine is not built"),
        "an unbuilt engine did not name itself: {output:?}"
    );
    assert!(
        !output.contains("missing dependencies"),
        "an unbuilt engine blamed the dependencies: {output:?}"
    );
}

/// The startup check is never asked to install.
///
/// The nag path must stay non-interactive. `config deps install` prompts per
/// dependency, so the hook reaching the install verb, or passing `--yes` to
/// get past the prompt, would be the defect.
#[test]
fn the_startup_check_asks_only_to_check() {
    let home = Home::new(1);
    home.record_arguments_instead(1);

    home.run();
    let arguments = home.recorded_arguments();
    assert_eq!(
        arguments, "deps check",
        "the startup check invoked config with something other than \
         `deps check`"
    );
    assert!(
        !arguments.contains("install") && !arguments.contains("--yes"),
        "the startup check reached the install verb: {arguments:?}"
    );
}

/// A malformed cache never leaks an error into startup.
///
/// An all-digit value longer than zsh's arithmetic width passes a digits-only
/// guard and then makes `$((...))` print "number truncated after 19 digits".
/// Every one of these must be treated as stale and produce clean stderr.
#[test]
fn a_malformed_cache_keeps_startup_silent() {
    for bad in [
        "",
        "notanumber",
        "   ",
        "-500",
        "1e10",
        "99999999999999999999999999",
        "12 34",
    ] {
        let home = Home::new(0);
        home.write_cache(bad);
        let stderr = home.run_stderr();
        assert_eq!(
            stderr, "",
            "a cache of {bad:?} wrote to stderr during startup: {stderr:?}"
        );
    }

    let home = Home::new(0);
    home.write_cache(&"9".repeat(200_000));
    let stderr = home.run_stderr();
    assert_eq!(
        stderr, "",
        "an oversized cache wrote to stderr during startup: {stderr:?}"
    );
}

/// An unwritable cache path never leaks an error.
///
/// The failing redirect is reported by the shell before `date` runs, so
/// `date ... 2>/dev/null` cannot suppress it. It costs only the throttle,
/// which re-checks on the next shell.
#[test]
fn an_unwritable_cache_path_keeps_startup_silent() {
    let home = Home::new(0);
    // A regular file where the directory must be, so mkdir -p and the
    // redirect both fail.
    fs::remove_dir_all(home.path().join(".cache")).expect("the cache directory is removable");
    fs::write(home.path().join(".cache"), "").expect("the blocking file is writable");

    let stderr = home.run_stderr();
    assert_eq!(
        stderr, "",
        "an unwritable cache path wrote to stderr during startup: {stderr:?}"
    );
}

/// The manual alias is defined.
#[test]
fn the_depcheck_alias_is_defined() {
    let home = Home::new(0);
    let output = Command::new("zsh")
        .arg("-ic")
        .arg(format!(
            ". '{}'; alias depcheck",
            home.path().join("deps/depcheck-hook.sh").display()
        ))
        .env("HOME", home.path())
        .output()
        .expect("zsh runs");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("config deps install"),
        "the depcheck alias is not defined: {text:?}"
    );
}

/// The hook is portable to a POSIX shell.
///
/// `.zshrc` sources this, and `.zshrc`'s own dependants read it under `sh`
/// from other callers (the pre-push hook, CI), so it must not depend on a
/// zsh-only construct.
///
/// PARSED under both shells and then EXECUTED under `sh`, because a parse
/// check is not an execution check: `dash -n` accepts `[[ ]]`, and a
/// zsh-only construct can parse under `sh` and misbehave at run time. The
/// executed run must produce the same nag zsh produces, from the same
/// fixture.
#[test]
fn the_hook_is_portable_to_a_posix_shell() {
    for shell in ["sh", "zsh"] {
        let parsed = Command::new(shell)
            .args(["-n"])
            .arg(hook())
            .output()
            .unwrap_or_else(|_| panic!("{shell} runs"));
        assert!(
            parsed.status.success(),
            "the hook does not parse under {shell}: {}",
            String::from_utf8_lossy(&parsed.stderr)
        );
    }

    let home = Home::new(1);
    let executed = home.spawn("sh", &["-c"]);
    assert!(
        String::from_utf8_lossy(&executed.stdout).contains("depcheck: missing dependencies"),
        "the hook did not nag when EXECUTED under sh, so it depends on a \
         zsh-only construct that a parse check cannot see: stdout {:?} \
         stderr {:?}",
        String::from_utf8_lossy(&executed.stdout),
        String::from_utf8_lossy(&executed.stderr)
    );
    assert!(
        executed.stderr.is_empty(),
        "the hook wrote to stderr when executed under sh: {}",
        String::from_utf8_lossy(&executed.stderr)
    );
}

/// The hook is actually wired into `.zshrc`.
///
/// The hook file being correct is not the same as it running. Without this,
/// every other assertion in this file passes on a machine where the nag never
/// fires because nothing sources the hook.
#[test]
fn the_zshrc_sources_the_hook() {
    let zshrc = repo_root().join(".zshrc");
    let text = fs::read_to_string(&zshrc)
        .unwrap_or_else(|_| panic!("{} is readable", zshrc.display()));

    // Comment lines do not source anything, and a grep satisfied by a comment
    // is how two assertions went vacuous in Tranche A.
    let sourced = text
        .lines()
        .map(|line| line.split('#').next().unwrap_or(""))
        .any(|code| code.contains("depcheck-hook.sh"));
    assert!(
        sourced,
        "{} does not source depcheck-hook.sh outside a comment, so the nag \
         never fires on this machine",
        zshrc.display()
    );
}
