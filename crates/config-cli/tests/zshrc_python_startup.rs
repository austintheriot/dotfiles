//! The lazy pyenv setup in `.zshrc`.
//!
//! `eval "$(pyenv init - zsh)"` costs 380ms at every shell startup, measured
//! on this machine. Two things inside it account for nearly all of that: a
//! `bash --norc` spawned only to remove the shims directory from `PATH`
//! before putting it back, and a `pyenv rehash` subprocess at 250ms on its
//! own.
//!
//! Neither is needed to make `python3` resolve. `pyenv init` ends with the
//! shims directory on `PATH` and `PYENV_SHELL` set; putting the shims on
//! `PATH` directly reaches the same end state for free. `rehash` only
//! regenerates the shim files, which change when a version or a package with
//! an entry point is installed: a `pyenv install` concern, not a per-shell
//! one.
//!
//! The contract, mirroring the nvm suite:
//!
//! 1. Startup runs no pyenv subprocess, so it stays fast.
//! 2. `python3` and `pip` still resolve through the shims.
//! 3. `pyenv` is available but not yet loaded.
//! 4. Calling `pyenv` loads the real thing and then behaves normally.
//!
//! Point 4 is the one worth testing rather than assuming. A shim that loads
//! the real thing and forgets to re-dispatch swallows its first call
//! silently, and every later call then works: the shape that passes a
//! careless test. Both the first call and the second are asserted for that
//! reason.

use std::path::PathBuf;

/// What `path_helper` starts a login shell with, and the baseline the
/// resolution assertions run against.
///
/// **This is a fix for a real defect rather than hygiene.** `.zshrc` skips
/// its shim prepend when the shims are on `PATH` anywhere, so an inherited
/// `PATH` that already carries them further back leaves them there, behind
/// whatever came first. Run from a shell with Homebrew ahead of the shims
/// (Claude Code's own environment is one), the shell suite's two resolution
/// assertions failed while a real login shell on the same machine resolved
/// correctly. Same config, two answers, decided by the caller.
///
/// Nothing here adds pyenv or Homebrew: these assertions are about what
/// `.zshrc` does with `PATH`, and pre-seeding either would answer the
/// question for it.
const STARTUP_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

fn pyenv_directory() -> PathBuf {
    dotfiles_test_support::repo::root().join(".pyenv")
}

fn zshrc_text() -> String {
    let path = dotfiles_test_support::repo::root().join(".zshrc");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is unreadable: {error}", path.display()))
}

/// Whether this machine can exercise the runtime half of the suite.
///
/// Returns the reason it cannot, so each caller reports the same cause the
/// shell suite did rather than inventing its own.
fn runtime_blocker() -> Option<String> {
    let root = dotfiles_test_support::repo::root();
    if std::env::var_os("HOME").map(PathBuf::from).as_deref() != Some(root.as_path()) {
        return Some("HOME is not the repo".to_string());
    }
    if !root.join(".zshrc").is_file() {
        return Some("HOME is not the repo".to_string());
    }
    if !pyenv_directory().is_dir() {
        return Some(format!("no pyenv at {}", pyenv_directory().display()));
    }
    if !dotfiles_test_support::zsh::available() {
        return Some("no zsh on this machine".to_string());
    }
    None
}

/// The value a probe script printed for `name`, from `KEY=value` lines.
fn field(text: &str, name: &str) -> String {
    text.lines()
        .find_map(|line| line.strip_prefix(&format!("{name}=")))
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Assertions 1 and 2. Startup pays nothing.
///
/// Asserted on the source text, the same way the nvm suite does: an
/// unconditional `pyenv init` at top level is the shape being removed, and
/// it is visible without starting a shell at all.
#[test]
fn startup_runs_no_pyenv_subprocess() {
    let text = zshrc_text();

    let init: Vec<String> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim_start();
            trimmed.starts_with("eval ") && trimmed.contains("$(pyenv init")
        })
        .map(|(index, line)| format!("{}: {}", index + 1, line.trim()))
        .collect();
    assert!(
        init.is_empty(),
        "startup evals `pyenv init`, which costs 380ms at every shell: {init:?}"
    );

    let rehash: Vec<String> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim_start().trim_start_matches("command ");
            trimmed.starts_with("pyenv rehash")
        })
        .map(|(index, line)| format!("{}: {}", index + 1, line.trim()))
        .collect();
    assert!(
        rehash.is_empty(),
        "startup runs `pyenv rehash`, a 250ms subprocess that only matters \
         after a `pyenv install`: {rehash:?}"
    );
}

/// Assertions 3 through 7. The end state is reached anyway.
///
/// One shell produces all five answers, because five shells would cost five
/// startups to observe one startup's result.
///
/// The laziness probe is the subtle one. The real `pyenv init` defines a
/// `pyenv` function too, so the function's existence is not the tell and
/// neither is its size. The real init also sources a completion, which
/// defines `_pyenv`; the lazy shim does not. That absence is the tell.
#[test]
fn startup_reaches_pyenvs_end_state_without_running_it() {
    if let Some(reason) = runtime_blocker() {
        dotfiles_test_support::skip(&format!("{reason}: pyenv startup state"));
        return;
    }
    let output = dotfiles_test_support::zsh::run_with_path(
        STARTUP_PATH,
        r#"
        print -r -- "python3=$(command -v python3)"
        print -r -- "pip3=$(command -v pip3)"
        print -r -- "shell=${PYENV_SHELL:-unset}"
        print -r -- "root=${PYENV_ROOT:-unset}"
        if typeset -f pyenv >/dev/null 2>&1; then
            if (( $+functions[_pyenv] )); then print "kind=eager"; else print "kind=lazy"; fi
        else
            print "kind=missing"
        fi
        "#,
    );
    let report = String::from_utf8_lossy(&output.stdout);
    let shims = pyenv_directory().join("shims");

    assert_eq!(
        field(&report, "python3"),
        shims.join("python3").to_string_lossy(),
        "python3 does not resolve through the pyenv shims; report was {report:?}"
    );
    assert_eq!(
        field(&report, "pip3"),
        shims.join("pip3").to_string_lossy(),
        "pip3 does not resolve through the pyenv shims; report was {report:?}"
    );
    // `pyenv init` exports both. A shim that skips them leaves `pyenv shell`
    // and anything else reading them broken in a way that only shows up
    // later.
    assert_eq!(
        field(&report, "shell"),
        "zsh",
        "PYENV_SHELL is not exported at startup; report was {report:?}"
    );
    assert_eq!(
        field(&report, "root"),
        pyenv_directory().to_string_lossy(),
        "PYENV_ROOT is not exported at startup; report was {report:?}"
    );
    assert_eq!(
        field(&report, "kind"),
        "lazy",
        "pyenv is not in the lazy state at startup, so either it is missing \
         or the expensive init already ran; report was {report:?}"
    );
}

/// Assertions 8 through 12. Calling `pyenv` loads it and it works.
///
/// The recursion trap: the real `pyenv init` defines its own `pyenv`
/// function, so the shim must `unfunction` itself before evaluating that or
/// the two definitions fight and the first call never returns.
///
/// These run with the CALLER's `PATH`, deliberately, not [`STARTUP_PATH`].
/// They call the real `pyenv`, which on this machine is a Homebrew install,
/// so a baseline that excludes Homebrew makes them fail with "command not
/// found" -- a fact about the baseline rather than about the shim. What they
/// test (does the shim load the real thing and re-dispatch) does not depend
/// on `PATH` order at all.
#[test]
fn calling_pyenv_loads_it_and_it_keeps_working() {
    if let Some(reason) = runtime_blocker() {
        dotfiles_test_support::skip(&format!("{reason}: pyenv lazy loading"));
        return;
    }

    // The positive control. Every assertion below compares against this, so
    // an empty expected version would make all of them pass by comparing
    // nothing to nothing.
    let expected = expected_version();
    assert!(
        !expected.is_empty(),
        "the real pyenv resolved no version name, so the assertions below \
         would compare an empty string against an empty string"
    );

    // The `pyenv` call that loads the real thing has to happen in THIS
    // shell, not in a command substitution. `$(pyenv ...)` runs in a
    // subshell, so the real init would load there and be discarded, leaving
    // the parent still lazy. That is correct behaviour and not what this
    // asserts, so the version goes to a file instead.
    let scratch = tempfile::Builder::new()
        .prefix("pyenv-version-")
        .tempdir()
        .expect("a temp directory");
    let version_file = scratch.path().join("version");
    let output = dotfiles_test_support::zsh::run(&format!(
        r#"
        pyenv version-name > {file} 2>&1
        if (( $+functions[_pyenv] )); then print "after=eager"; else print "after=lazy"; fi
        print -r -- "second=$(pyenv version-name 2>&1 | tail -1)"
        print -r -- "shims=$(print -r -- $PATH | tr ':' '\n' | grep -cx {shims})"
        "#,
        file = shell_quote(&version_file.to_string_lossy()),
        shims = shell_quote(&pyenv_directory().join("shims").to_string_lossy()),
    ));
    let report = String::from_utf8_lossy(&output.stdout);

    let first = std::fs::read_to_string(&version_file).unwrap_or_default();
    assert_eq!(
        first.trim(),
        expected,
        "the first pyenv call did not return the right version, which is the \
         shape of a shim that loads the real thing and forgets to \
         re-dispatch; report was {report:?}"
    );

    assert_eq!(
        field(&report, "after"),
        "eager",
        "pyenv was not loaded for real after the first call; report was {report:?}"
    );

    // A second call must still work. A shim that unfunctions itself and
    // forgets to re-dispatch leaves the first call silent and every later
    // one fine, which is the shape that passes a careless test.
    assert_eq!(
        field(&report, "second"),
        expected,
        "a second pyenv call did not work; report was {report:?}"
    );

    // `pyenv init` removes the shims directory from PATH before prepending
    // it, to avoid a duplicate on a re-source. The lazy path prepends
    // directly, so a shell that loads pyenv for real must not end up with
    // two.
    assert_eq!(
        field(&report, "shims"),
        "1",
        "the shims directory is not on PATH exactly once after loading; \
         report was {report:?}"
    );
}

/// The version the real `pyenv` binary reports.
///
/// pyenv is installed by Homebrew here, so `$PYENV_ROOT/bin` does not exist
/// and the binary is found on `PATH`. Resolving it the way a shell does
/// keeps this working under either installation shape.
fn expected_version() -> String {
    let output = dotfiles_test_support::zsh::run("command pyenv version-name 2>/dev/null");
    // The LAST line, not the whole of stdout. Each platform variant echoes
    // "Loaded <platform> configuration" as it loads, so a shell that
    // correctly sourced the config prints a banner ahead of the answer.
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .lines()
        .last()
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// A single-quoted shell word, so a path with a space cannot split.
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}
