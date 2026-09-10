//! `~/.profile`, which puts `~/.local/bin` on PATH for bash and sh.
//!
//! Reported from a bare container: `config init` finished, and `config` was
//! still not found. `install-hooks` links it into `~/.local/bin`, which no
//! default PATH carries, and only `.zshrc` added that directory, so a bash
//! or sh login never found it, no matter how many shells were started.
//!
//! The file is deliberately minimal and POSIX: bash reads it at login, sh
//! reads it as `$ENV` in some configurations, and it must not depend on
//! anything zsh provides.
//!
//! Converted whole from `tests/profile-path.test.sh`, which ran **9**
//! assertions from 6 `assert_*` call sites: two of them are in loops, over
//! the two shells and over the three system directories.
//!
//! Every effect is driven through a real shell rather than grepped, so each
//! assertion is about what the profile DOES and not about how it is spelled.
//!
//! ONE ASSERTION WAS WEAKER THAN IT LOOKED, found while sabotaging this
//! conversion on 2026-09-10. The shell suite checked POSIX-ness with
//! `sh -n` and `dash -n`. A `[[ -n "$HOME" ]]` bashism appended to the
//! profile is accepted by BOTH, exit 0, no output: dash parses `[[` as an
//! ordinary command word, so a syntax check cannot reject it, and macOS
//! `/bin/sh` is bash in POSIX mode and accepts it outright. So the check
//! named the exact defect it could not detect. This version keeps both
//! syntax checks and adds a run under dash, which fails with `[[: not
//! found`. That is a strengthening, not a fix for a shipped bug: the tracked
//! profile is POSIX and passes either way.

use dotfiles_test_support::repo::root as repo_root;
use dotfiles_test_support::skip;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The tracked profile.
fn profile() -> PathBuf {
    repo_root().join(".profile")
}

/// Whether a program is on PATH.
fn available(program: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {program}"))
        .output()
        .is_ok_and(|output| output.status.success())
}

/// A fixture home holding a copy of the profile and an empty `.local/bin`.
fn fixture_home() -> tempfile::TempDir {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join(".local/bin")).expect("a fixture .local/bin");
    std::fs::copy(profile(), home.path().join(".profile")).expect("the profile copies");
    home
}

/// Runs a script in `shell` with the fixture home and a minimal PATH.
///
/// PATH is pinned rather than inherited: every assertion here is about what
/// the profile ADDS, and a caller whose PATH already carries `~/.local/bin`
/// would satisfy them without the profile doing anything.
fn run_in(shell: &str, home: &Path, script: &str) -> String {
    let output = Command::new(shell)
        .arg("-c")
        .arg(script)
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin")
        .output()
        .unwrap_or_else(|error| panic!("{shell} spawns: {error}"));
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Whether `checker -n` accepts the profile as a script.
fn parses_under(checker: &str) -> bool {
    Command::new(checker)
        .arg("-n")
        .arg(profile())
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Whether the profile RUNS clean under `checker`, with output.
///
/// `-n` alone is not enough and the shell suite's version of this check was
/// weaker than it looked. Measured 2026-09-10: a `[[ -n "$HOME" ]]` bashism
/// appended to the profile is accepted by BOTH `sh -n` and `dash -n` on this
/// machine, exit 0, silently. dash parses `[[` as an ordinary command word,
/// so no syntax checker can reject it; only running it can, where dash says
/// `[[: not found`. A check that cannot fail on the defect it names is
/// theatre, so this executes the file and requires a clean run.
fn runs_clean_under(checker: &str, home: &Path) -> (bool, String) {
    let output = Command::new(checker)
        .arg("-c")
        .arg(r#". "$HOME/.profile""#)
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin")
        .output()
        .unwrap_or_else(|error| panic!("{checker} spawns: {error}"));
    let complaints = String::from_utf8_lossy(&output.stderr).trim().to_string();
    (output.status.success() && complaints.is_empty(), complaints)
}

#[test]
fn the_profile_exists() {
    let path = profile();
    assert!(path.is_file(), "no profile at {}", path.display());
}

#[test]
fn both_shells_pick_up_local_bin_from_the_profile() {
    for shell in ["sh", "bash"] {
        if !available(shell) {
            skip(&format!(
                "{shell} picks up ~/.local/bin from the profile: no {shell} here"
            ));
            continue;
        }

        let home = fixture_home();
        let marker = home.path().join(".local/bin/only-in-local-bin");
        std::fs::write(&marker, "#!/bin/sh\nprintf found\n").expect("the marker writes");
        let mut permissions = std::fs::metadata(&marker)
            .expect("the marker exists")
            .permissions();
        {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(0o755);
        }
        std::fs::set_permissions(&marker, permissions).expect("the marker is executable");

        // A positive control: the marker must be invisible BEFORE the
        // profile is sourced, or "yes" afterwards would prove nothing about
        // the profile.
        let before = run_in(
            shell,
            home.path(),
            "command -v only-in-local-bin >/dev/null 2>&1 && printf yes || printf no",
        );
        assert_eq!(
            before, "no",
            "{shell} found the marker without sourcing the profile, so this \
             check cannot attribute anything to the profile"
        );

        let after = run_in(
            shell,
            home.path(),
            r#". "$HOME/.profile"; command -v only-in-local-bin >/dev/null 2>&1 && printf yes || printf no"#,
        );
        assert_eq!(
            after, "yes",
            "{shell} did not pick up ~/.local/bin from the profile"
        );
    }
}

/// A bashism here fails on a machine whose `/bin/sh` is dash, which is every
/// Debian and Ubuntu box, exactly where this file matters most.
#[test]
fn the_profile_is_posix() {
    assert!(parses_under("sh"), "the profile does not parse under sh");

    if available("dash") {
        assert!(parses_under("dash"), "the profile does not parse under dash");

        // The one that can actually see a bashism. See `runs_clean_under`.
        let home = fixture_home();
        let (clean, complaints) = runs_clean_under("dash", home.path());
        assert!(
            clean,
            "the profile does not run clean under dash, which is /bin/sh on \
             every Debian and Ubuntu box: {complaints}"
        );
    } else {
        skip("the profile parses under dash: dash is not installed");
    }
}

/// A login shell can source this more than once (a nested login, `su -`, a
/// terminal that re-runs it), and a PATH that grows without bound on every
/// source is a slow leak that eventually shows up as a mysterious slowdown.
#[test]
fn sourcing_it_three_times_adds_the_entry_once() {
    let home = fixture_home();
    let path = run_in(
        "sh",
        home.path(),
        r#". "$HOME/.profile"; . "$HOME/.profile"; . "$HOME/.profile"; printf "%s" "$PATH""#,
    );

    let wanted = format!("{}/.local/bin", home.path().display());
    let occurrences = path.split(':').filter(|entry| *entry == wanted).count();
    assert_eq!(
        occurrences, 1,
        "sourcing the profile three times put {wanted} on PATH {occurrences} \
         times; PATH was {path}"
    );
}

/// The reported session included a typo, `$PATHD` instead of `$PATH`, which
/// emptied PATH and made even `iconv` disappear. The profile must never be
/// able to do that to someone: every system directory has to survive it.
#[test]
fn the_profile_keeps_the_existing_path() {
    let home = fixture_home();
    let output = Command::new("sh")
        .arg("-c")
        .arg(r#". "$HOME/.profile"; printf "%s" "$PATH""#)
        .env("HOME", home.path())
        .env("PATH", "/usr/bin:/bin:/sbin")
        .output()
        .expect("sh spawns");
    let after = String::from_utf8_lossy(&output.stdout).to_string();

    for required in ["/usr/bin", "/bin", "/sbin"] {
        assert!(
            after.split(':').any(|entry| entry == required),
            "the profile dropped {required} from PATH; PATH became {after}"
        );
    }
}
