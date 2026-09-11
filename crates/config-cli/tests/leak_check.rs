//! `tests/leak-check.sh`, the credential gate for a PUBLIC repository.
//!
//! Every assertion here is a boundary assertion. This repository is public,
//! and the guard is the only thing between an internal term or a credential
//! and `github.com`. **Weakening one of these is a security regression, not a
//! test regression.** In particular: an assertion that the guard BLOCKS is
//! worth more than one that it passes, and a change that turns a block into a
//! pass needs a reason stronger than "the test was awkward".
//!
//! Two modes are covered. Staged mode is what `tests/pre-commit` runs. Range
//! mode is what `tests/pre-push` runs, and it exists because
//! `git commit-tree` and `git commit --no-verify` never invoke pre-commit, so
//! without a push-time scan those commits reach the public repo unscanned.
//!
//! # No project term is written here, and none can be
//!
//! The guard's layer-2 rules read their patterns from an **untracked file
//! outside this repository** (`~/.claude/local/leak-patterns.conf`, reachable
//! through `LEAK_PATTERN_FILE`) precisely so the terms it defends are never
//! published. That indirection is preserved exactly: every test that needs a
//! term writes [`FAKE_TERM`] to a temporary file and points
//! `LEAK_PATTERN_FILE` at it. The real file is never read, so this suite runs
//! on a machine that does not have one, and no real term appears in this
//! source.
//!
//! [`FAKE_TERM`] is built from repeated letters and matches nothing real.
//!
//! # Nothing here is a credential
//!
//! Every planted secret is a fake built from repeated letters or the RFC 4122
//! example UUID. They are also **assembled at run time rather than written as
//! literals**, because `leak-check.sh` self-excludes only its own path: this
//! file IS scanned when committed, so a secret-shaped literal here would
//! block the commit that adds it.
//!
//! Converted whole from `tests/leak-check.test.sh`, which ran **64**
//! assertions, measured by running the suite rather than by counting
//! `assert_` call sites.

use dotfiles_test_support::repo::root as repo_root;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// A term the fake pattern file names. Chosen to match nothing real, and the
/// only term any test in this file knows about.
const FAKE_TERM: &str = "ZZFAKECORPZZ";

/// The zero sha git sends for a brand-new remote branch.
const ZERO_SHA: &str = "0000000000000000000000000000000000000000";

fn leak_check() -> PathBuf {
    repo_root().join("tests/leak-check.sh")
}

fn pre_push() -> PathBuf {
    repo_root().join("tests/pre-push")
}

// --- planted fakes ----------------------------------------------------------
//
// Assembled from parts at run time. See the module docs: a literal here would
// be scanned and would block the commit that adds this file.

/// A key-prefix token matching layer 1's `possible credential` rule.
fn plant_key(letter: char) -> String {
    format!("token = ghp_{}\n", std::iter::repeat_n(letter, 24).collect::<String>())
}

/// A secret-shaped assignment with a literal value.
fn plant_password() -> String {
    format!("password = \"{}{}\"\n", "correcthorse", "battery1234")
}

/// A bare UUID assignment. The RFC 4122 example UUID, split so the whole
/// value never appears as one literal.
fn plant_uuid() -> String {
    format!(
        "registry_token={}-{}-{}-{}-{}\n",
        "123e4567", "e89b", "12d3", "a456", "426614174000"
    )
}

/// A line carrying the fake project term.
fn plant_term() -> String {
    format!("internal note about {FAKE_TERM}\n")
}

fn clean_text() -> String {
    "nothing to see here\n".to_owned()
}

/// A value long enough to match the secret-assignment rule, built around
/// `word`.
///
/// The rule needs 12 or more characters from a class of letters, digits and
/// punctuation, so the caller picks the word that decides whether the guard's
/// placeholder filter rejects the line.
fn plausible_literal(word: &str) -> String {
    format!("{word}{}", "1234567890ab")
}

/// A git invocation in a fixture, with any ambient git environment cleared.
///
/// A pre-commit hook exports `GIT_DIR` and friends, and every fixture
/// `git init` would otherwise target the developer's repository.
fn git(directory: &Path, arguments: &[&str]) {
    let output = git_command(directory)
        .args(arguments)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_command(directory: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(directory);
    for variable in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_PREFIX",
        "GIT_OBJECT_DIRECTORY",
    ] {
        command.env_remove(variable);
    }
    command
}

fn git_stdout(directory: &Path, arguments: &[&str]) -> String {
    let output = git_command(directory)
        .args(arguments)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A git repository plus the pattern and allow files the guard reads.
///
/// The pattern and allow files live in this fixture's temporary directory and
/// are pointed at through `LEAK_PATTERN_FILE` and `LEAK_ALLOW_FILE`, which is
/// the same indirection the real guard uses. Nothing here reads
/// `~/.claude/local/`.
struct Fixture {
    directory: TempDir,
}

/// One run of the guard: its exit status and its combined output.
struct Run {
    status: i32,
    text: String,
}

impl Run {
    fn assert_status(&self, expected: i32, description: &str) {
        assert_eq!(
            self.status, expected,
            "{description}\nguard output:\n{}",
            self.text
        );
    }

    fn assert_contains(&self, needle: &str, description: &str) {
        assert!(
            self.text.contains(needle),
            "{description}\nexpected to contain {needle:?}, output was:\n{}",
            self.text
        );
    }

    fn assert_lacks(&self, needle: &str, description: &str) {
        assert!(
            !self.text.contains(needle),
            "{description}\nexpected NOT to contain {needle:?}, output was:\n{}",
            self.text
        );
    }
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let fixture = Self { directory };

        fs::write(
            fixture.pattern_file(),
            format!("# fake project terms for the test suite\n{FAKE_TERM}\n"),
        )
        .expect("the pattern file is writable");
        fs::write(
            fixture.allow_file(),
            "# paths where project terms are allowed\n^docs/allowed\\.md$\n",
        )
        .expect("the allow file is writable");

        let repo = fixture.repo();
        fs::create_dir_all(&repo).expect("creatable");
        git(&repo, &["init", "-q", "-b", "main", "."]);
        git(&repo, &["config", "user.email", "t@t"]);
        git(&repo, &["config", "user.name", "t"]);
        fixture
    }

    fn root(&self) -> &Path {
        self.directory.path()
    }

    fn repo(&self) -> PathBuf {
        self.root().join("repo")
    }

    fn pattern_file(&self) -> PathBuf {
        self.root().join("leak-patterns.conf")
    }

    fn allow_file(&self) -> PathBuf {
        self.root().join("leak-allow.conf")
    }

    /// Writes `content` at `path` inside the fixture repo and stages it.
    fn stage(&self, path: &str, content: &str) {
        let full = self.repo().join(path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).expect("creatable");
        }
        fs::write(&full, content).expect("writable");
        git(&self.repo(), &["add", "--", path]);
    }

    fn unstage_all(&self) {
        let repo = self.repo();
        git(&repo, &["reset", "-q"]);
        let _ = git_command(&repo)
            .args(["checkout", "-q", "--", "."])
            .output();
        git(&repo, &["clean", "-qfd"]);
    }

    fn commit(&self, path: &str, content: &str, message: &str) -> String {
        self.stage(path, content);
        git(&self.repo(), &["commit", "-q", "-m", message]);
        git_stdout(&self.repo(), &["rev-parse", "HEAD"])
    }

    /// Runs the guard in the fixture repo, with the fixture's pattern and
    /// allow files in place.
    fn run(&self, arguments: &[&str]) -> Run {
        self.run_with(arguments, |_| {})
    }

    fn run_with(&self, arguments: &[&str], configure: impl FnOnce(&mut Command)) -> Run {
        let mut command = Command::new(leak_check());
        command
            .args(arguments)
            .current_dir(self.repo())
            .env("LEAK_PATTERN_FILE", self.pattern_file())
            .env("LEAK_ALLOW_FILE", self.allow_file())
            .env_remove("SKIP_LEAK_CHECK")
            .env_remove("LEAK_ALLOW_NO_PATTERNS")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_PREFIX")
            .env_remove("GIT_OBJECT_DIRECTORY");
        configure(&mut command);
        let output = command.output().expect("leak-check.sh runs");
        Run {
            status: output.status.code().unwrap_or(-1),
            text: format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        }
    }

    /// Drives `tests/pre-push` with git's stdin protocol, pointed at the
    /// fixture.
    ///
    /// The pushed ref is a feature branch and the changed paths match no
    /// trigger path, so neither the drift check nor the Docker suite runs:
    /// the leak scan is the only gate exercised.
    ///
    /// `HOME` is set alongside `GIT_DIR` and `GIT_WORK_TREE` because the hook
    /// resolves its own scripts as `$HOME/tests/...`. On CI, the checkout and
    /// the runner home are different directories, so without this the hook
    /// looks for `leak-check.sh` in the wrong tree.
    fn run_pre_push(&self, local_sha: &str, remote_sha: &str) -> Run {
        use std::io::Write;
        use std::process::Stdio;

        let repo = self.repo();
        let mut child = Command::new(pre_push())
            .arg("origin")
            .arg(format!("file://{}", repo.display()))
            .current_dir(&repo)
            .env("HOME", repo_root())
            .env("GIT_DIR", repo.join(".git"))
            .env("GIT_WORK_TREE", &repo)
            .env("LEAK_PATTERN_FILE", self.pattern_file())
            .env("LEAK_ALLOW_FILE", self.allow_file())
            .env_remove("SKIP_LEAK_CHECK")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("pre-push runs");

        let refline =
            format!("refs/heads/feature {local_sha} refs/heads/feature {remote_sha}\n");
        child
            .stdin
            .take()
            .expect("stdin is piped")
            .write_all(refline.as_bytes())
            .expect("the ref line is writable");

        let output = child.wait_with_output().expect("pre-push finishes");
        Run {
            status: output.status.code().unwrap_or(-1),
            text: format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        }
    }
}

// --- paths git quotes -------------------------------------------------------

/// Git C-quotes any path with a byte outside printable ASCII, so `café/` is
/// reported as `"caf\303\251/"`. The scan fed that quoted form back to
/// `git diff -- <path>`, which matched nothing, so the path had no diff header
/// and the unscannable check blocked it as "no readable diff" with a diagnosis
/// naming `.gitattributes` and binaries. Wrong cause, and every non-ASCII path
/// was uncommittable.
///
/// **Two sides**, because a fix that only stops the false block could also
/// stop the true one. A clean file under such a path must pass, and a
/// credential under it must be blocked AS A CREDENTIAL, not as an unreadable
/// path.
///
/// The TODO that recorded this predicted a silent exit 0. Measured before the
/// fix: exit 2 on both, so the gate failed closed and nothing leaked.
/// Recorded so the security claim is not overstated.
#[test]
fn a_non_ascii_path_is_scanned_rather_than_blocked_as_unreadable() {
    let fixture = Fixture::new();

    fixture.stage("café/notes.md", &clean_text());
    fixture
        .run(&[])
        .assert_status(0, "a clean file under a non-ASCII path passes");
    fixture.unstage_all();

    fixture.stage("café/notes.md", &plant_key('A'));
    let run = fixture.run(&[]);
    run.assert_status(1, "a secret under a non-ASCII path is blocked");
    run.assert_lacks(
        "no readable diff",
        "and blocked for the secret, not for an unreadable path",
    );
}

// --- staged mode ------------------------------------------------------------

#[test]
fn staged_clean_content_passes() {
    let fixture = Fixture::new();
    fixture.stage("notes.txt", &clean_text());
    fixture
        .run(&[])
        .assert_status(0, "staged: clean content passes");
}

#[test]
fn staged_a_key_prefix_token_is_blocked_and_labelled() {
    let fixture = Fixture::new();
    fixture.stage("notes.txt", &plant_key('A'));
    let run = fixture.run(&[]);
    run.assert_status(1, "staged: a key-prefix token is blocked");
    run.assert_contains(
        "[possible credential]",
        "staged: the key is labelled a possible credential",
    );
    run.assert_contains("COMMIT BLOCKED", "staged: the block message names the commit");
}

#[test]
fn staged_a_hardcoded_password_is_blocked_and_labelled() {
    let fixture = Fixture::new();
    fixture.stage("notes.txt", &plant_password());
    let run = fixture.run(&[]);
    run.assert_status(1, "staged: a hardcoded password is blocked");
    run.assert_contains(
        "[hardcoded secret assignment]",
        "staged: the password is labelled a secret assignment",
    );
}

#[test]
fn staged_a_bare_uuid_assignment_is_blocked_and_labelled() {
    let fixture = Fixture::new();
    fixture.stage("notes.txt", &plant_uuid());
    let run = fixture.run(&[]);
    run.assert_status(1, "staged: a bare UUID assignment is blocked");
    run.assert_contains(
        "[bare UUID (possible token)]",
        "staged: the UUID is labelled a possible token",
    );
}

#[test]
fn staged_a_project_term_is_blocked_and_labelled() {
    let fixture = Fixture::new();
    fixture.stage("notes.txt", &plant_term());
    let run = fixture.run(&[]);
    run.assert_status(1, "staged: a project term is blocked");
    run.assert_contains("[project term]", "staged: the term is labelled a project term");
}

/// Regression for the term-scope filter matching nothing once more than one
/// file is staged.
///
/// The header list is a multi-line set of `+++ b/<path>` lines. Feeding it to
/// `awk` through `-v` silently produces an empty lookup table, because `awk -v`
/// does not accept a value containing a newline. With a SECOND staged file the
/// term rule then never sees any hunk as in scope, and a leaked term passes
/// clean.
///
/// A single staged file happened to still work, because the lookup table being
/// empty and the file's own header both failing to match looked the same as
/// "not in scope" either way. **Two files is the shape that tells them apart**,
/// which is why this test stages two and puts the term in the second.
#[test]
fn staged_a_term_in_the_second_of_two_files_is_blocked() {
    let fixture = Fixture::new();
    fixture.stage("first.txt", &clean_text());
    fixture.stage("second.txt", &plant_term());
    let run = fixture.run(&[]);
    run.assert_status(
        1,
        "staged: a term in the second of two staged files is blocked",
    );
    run.assert_contains(
        FAKE_TERM,
        "staged: the term is named even when it is not the first file",
    );
}

/// The allow list applies to term rules only. A credential in an allowed path
/// is still a credential.
#[test]
fn staged_the_allow_list_covers_terms_and_not_credentials() {
    let fixture = Fixture::new();

    fixture.stage("docs/allowed.md", &plant_term());
    fixture
        .run(&[])
        .assert_status(0, "staged: a project term in an allowed path passes");
    fixture.unstage_all();

    fixture.stage("docs/allowed.md", &plant_key('A'));
    fixture.run(&[]).assert_status(
        1,
        "staged: a credential in an allowed path is still blocked",
    );
}

/// Placeholders and environment references are not secrets.
///
/// **The shell suite's version of this was near-vacuous, and this one is
/// deliberately stronger.** It staged `password=${DB_PASSWORD}` and
/// `api_key = "<your-key-here>"` and asserted exit 0. Measured during this
/// conversion: with the guard's `grep -viE` placeholder filter deleted
/// entirely, both of those STILL pass, because the secret-assignment regex
/// requires 12 or more characters from a class that excludes `$` and `<`, so
/// the value never matched in the first place. The filter the assertion named
/// was never reached.
///
/// So the original two lines are kept, and values that DO reach the filter are
/// added beside them: a literal beginning with a letter, long enough to match,
/// that the filter rejects on the word `placeholder` or `example`. Sabotaging
/// the filter now turns this test red, and the three-line grouping says which
/// half did the work.
#[test]
fn staged_env_references_and_placeholders_pass() {
    let fixture = Fixture::new();

    // The shapes the primary regex cannot match. Kept from the shell suite so
    // a regex that grew to accept `$` or `<` is still caught here.
    fixture.stage(
        "config.sh",
        "password=${DB_PASSWORD}\napi_key = \"<your-key-here>\"\n",
    );
    fixture
        .run(&[])
        .assert_status(0, "staged: env references pass");
    fixture.unstage_all();

    // The shapes that DO match the primary regex and are rejected by the
    // placeholder filter. This is the half that exercises the filter.
    fixture.stage(
        "config.sh",
        &format!(
            "password={}\napi_key = \"{}\"\n",
            plausible_literal("placeholder"),
            plausible_literal("example")
        ),
    );
    fixture.run(&[]).assert_status(
        0,
        "staged: a matching literal that reads as a placeholder still passes",
    );
    fixture.unstage_all();

    // Positive control, and the reason the two above are not simply a guard
    // that never fires: a literal of the same shape with no placeholder word
    // in it must still block.
    //
    // Assembled at run time like every other planted fake here. Written as a
    // literal, it blocked the commit that added this test, which is the guard
    // working on its own suite.
    fixture.stage(
        "config.sh",
        &format!("password={}\n", plausible_literal("passphrase")),
    );
    fixture.run(&[]).assert_status(
        1,
        "staged: a matching literal that is NOT a placeholder is blocked",
    );
}

/// The escape hatch works and says so.
#[test]
fn staged_skip_leak_check_skips_and_announces() {
    let fixture = Fixture::new();
    fixture.stage("notes.txt", &plant_key('A'));
    let run = fixture.run_with(&[], |command| {
        command.env("SKIP_LEAK_CHECK", "1");
    });
    run.assert_status(0, "staged: SKIP_LEAK_CHECK skips the check");
    run.assert_contains("SKIPPED", "staged: the skip is announced");
}

/// The script never scans itself. Its own source contains the generic patterns
/// it searches for, so scanning it would always self-trip. It is the only path
/// excluded in either mode, which means a change to that file is unguarded and
/// depends on human review.
#[test]
fn staged_the_script_excludes_its_own_path() {
    let fixture = Fixture::new();
    fixture.stage("tests/leak-check.sh", &plant_key('A'));
    fixture
        .run(&[])
        .assert_status(0, "staged: the script excludes its own path");
}

// --- range mode -------------------------------------------------------------

/// History for the range-mode tests. `base` is the last pushed commit, and
/// everything after it is what a push would publish.
struct History {
    base: String,
    clean_tip: String,
    leaky_tip: String,
    removed_tip: String,
}

impl History {
    fn build(fixture: &Fixture) -> Self {
        let base = fixture.commit("README.md", &clean_text(), "base");
        let clean_tip = fixture.commit("more.txt", &clean_text(), "clean change");
        let leaky_tip = fixture.commit("notes.txt", &plant_key('A'), "oops");
        let removed_tip = fixture.commit("notes.txt", &clean_text(), "remove it");
        Self {
            base,
            clean_tip,
            leaky_tip,
            removed_tip,
        }
    }
}

#[test]
fn range_clean_commits_pass() {
    let fixture = Fixture::new();
    let history = History::build(&fixture);
    fixture
        .run(&["--range", &format!("{}..{}", history.base, history.clean_tip)])
        .assert_status(0, "range: clean commits pass");
}

#[test]
fn range_a_secret_in_a_pushed_commit_is_blocked() {
    let fixture = Fixture::new();
    let history = History::build(&fixture);
    let run = fixture.run(&["--range", &format!("{}..{}", history.base, history.leaky_tip)]);
    run.assert_status(1, "range: a secret in a pushed commit is blocked");
    run.assert_contains("PUSH BLOCKED", "range: the block message names the push");
    run.assert_contains(
        "[possible credential]",
        "range: the credential label is the same as staged mode",
    );
}

/// The secret is added in one commit and removed in the next. The net diff is
/// empty, but both commits are pushed, so the secret is published. Range mode
/// scans each commit's own diff for exactly this reason.
#[test]
fn range_a_secret_added_then_removed_inside_the_range_is_still_blocked() {
    let fixture = Fixture::new();
    let history = History::build(&fixture);

    fixture
        .run(&[
            "--range",
            &format!("{}..{}", history.leaky_tip, history.removed_tip),
        ])
        .assert_status(0, "range: removing a secret is itself clean");

    fixture
        .run(&[
            "--range",
            &format!("{}..{}", history.clean_tip, history.removed_tip),
        ])
        .assert_status(
            1,
            "range: a secret added then removed inside the range is still blocked",
        );
}

#[test]
fn range_the_script_excludes_its_own_path() {
    let fixture = Fixture::new();
    let history = History::build(&fixture);
    let self_tip = fixture.commit("tests/leak-check.sh", &plant_key('A'), "edit the guard");
    fixture
        .run(&[
            "--range",
            &format!("{}..{}", history.removed_tip, self_tip),
        ])
        .assert_status(0, "range: the script excludes its own path");
}

#[test]
fn range_the_allow_list_covers_terms_and_not_credentials() {
    let fixture = Fixture::new();
    let history = History::build(&fixture);

    let term_tip = fixture.commit("docs/allowed.md", &plant_term(), "allowed term");
    fixture
        .run(&["--range", &format!("{}..{}", history.removed_tip, term_tip)])
        .assert_status(0, "range: a project term in an allowed path passes");

    let key_tip = fixture.commit(
        "docs/allowed.md",
        &plant_key('A'),
        "credential in allowed path",
    );
    fixture
        .run(&["--range", &format!("{term_tip}..{key_tip}")])
        .assert_status(1, "range: a credential in an allowed path is still blocked");
}

#[test]
fn range_skip_leak_check_skips_and_names_pre_push() {
    let fixture = Fixture::new();
    let history = History::build(&fixture);
    let run = fixture.run_with(
        &["--range", &format!("{}..{}", history.base, history.leaky_tip)],
        |command| {
            command.env("SKIP_LEAK_CHECK", "1");
        },
    );
    run.assert_status(0, "range: SKIP_LEAK_CHECK skips the check");
    run.assert_contains("pre-push", "range: the skip names pre-push");
}

/// Misuse is a distinct exit code, so a caller can tell "I invoked this wrong"
/// from "this found a leak".
#[test]
fn misuse_is_a_usage_error() {
    let fixture = Fixture::new();
    fixture
        .run(&["--range"])
        .assert_status(2, "range: a missing range value is a usage error");
    fixture
        .run(&["--bogus"])
        .assert_status(2, "an unknown argument is a usage error");
}

/// An unresolvable range must not fail open. Without validation, git's failure
/// inside the path listing would produce an empty scan set and a false-clean
/// exit 0.
#[test]
fn range_an_unresolvable_range_is_a_usage_error_not_clean() {
    let fixture = Fixture::new();
    History::build(&fixture);
    let run = fixture.run(&[
        "--range",
        "0123456789abcdef0123456789abcdef01234567..HEAD",
    ]);
    run.assert_status(
        2,
        "range: an unresolvable range is a usage error, not clean",
    );
    run.assert_contains(
        "cannot resolve range",
        "range: the error names the unresolved range",
    );
}

#[test]
fn range_an_empty_range_passes() {
    let fixture = Fixture::new();
    let history = History::build(&fixture);
    fixture
        .run(&[
            "--range",
            &format!("{}..{}", history.removed_tip, history.removed_tip),
        ])
        .assert_status(0, "range: an empty range passes");
}

/// A git failure mid-scan must block, not pass.
///
/// The scan functions run inside a command substitution, so an `exit` inside
/// them cannot reach the parent: the parent has to observe the failure some
/// other way. Without that, the guard prints its own diagnostic and then exits
/// 0, which is the fail-open shape this whole file exists to close.
///
/// The shim resolves the real git first rather than hardcoding a path: the
/// suite's other calls use whatever git is on `PATH`, and shadowing it with a
/// different build mid-test is its own confusion.
#[test]
fn a_git_failure_mid_scan_exits_two_not_zero() {
    let fixture = Fixture::new();
    History::build(&fixture);
    fixture.commit("planted.txt", &plant_key('A'), "plant");

    let real_git = which("git").expect("git is on PATH");
    let shim_directory = fixture.root().join("git-shim-fail");
    fs::create_dir_all(&shim_directory).expect("creatable");
    let shim = shim_directory.join("git");
    fs::write(
        &shim,
        format!(
            "#!/bin/sh\n\
             # Fail only the -p invocation the added-lines scan makes, so the\n\
             # range still resolves and the path list is still produced.\n\
             for arg in \"$@\"; do\n\
             \x20   if [ \"$arg\" = \"-p\" ]; then\n\
             \x20       printf 'simulated git failure\\n' >&2\n\
             \x20       exit 128\n\
             \x20   fi\n\
             done\n\
             exec {} \"$@\"\n",
            shell_quote(&real_git)
        ),
    )
    .expect("writable");
    make_executable(&shim);

    let path = std::env::var("PATH").unwrap_or_default();
    let run = fixture.run_with(&["--range", "HEAD~1..HEAD"], |command| {
        command.env("PATH", format!("{}:{path}", shim_directory.display()));
    });

    run.assert_status(2, "a git failure mid-scan exits 2, not 0");
    run.assert_contains("HEAD~1..HEAD", "the failure names the range");
}

// --- unscannable paths ------------------------------------------------------

/// A one-line `.gitattributes` entry makes an ordinary text file unscannable:
/// git prints "Binary files differ" and there are no `+` lines for the content
/// rules to read. The guard must notice a path it listed produced no hunk.
///
/// The positive control comes first: the same credential in a plain staged
/// file must block as a credential, or the `-diff` case below would prove
/// nothing.
#[test]
fn a_diff_marked_path_does_not_pass_silently() {
    let fixture = Fixture::new();
    History::build(&fixture);

    fixture.stage("hidden.txt", &plant_key('B'));
    fixture
        .run(&[])
        .assert_status(1, "a credential in a plain staged file is blocked");

    fixture.stage(".gitattributes", "hidden.txt -diff\n");
    let run = fixture.run(&[]);
    run.assert_status(2, "a -diff marked path does not pass silently");
    run.assert_contains("hidden.txt", "the block names the unscannable path");
}

/// The substring hazard, in the one shape that distinguishes the two
/// implementations.
///
/// A shorter path must not read as scanned because a longer path containing it
/// has a header. The `.gitattributes` pattern is anchored with a leading slash
/// on purpose: a bare `styles.css` matches at ANY depth, so it would unset
/// diff for `vendor/styles.css` too and both paths would be unscannable, which
/// the buggy and the fixed form report identically.
///
/// With only `styles.css` unset: a substring match finds `styles.css` inside
/// `+++ b/vendor/styles.css` and lets the credential through, and the anchored
/// match blocks.
#[test]
fn a_diff_path_is_blocked_even_when_a_longer_path_shares_its_name() {
    let fixture = Fixture::new();
    History::build(&fixture);

    fixture.stage("styles.css", &plant_key('D'));
    fixture.stage("vendor/styles.css", "plain\n");
    fixture.stage(".gitattributes", "/styles.css -diff\n");

    let run = fixture.run(&[]);
    run.assert_status(
        2,
        "a -diff path is blocked even when a longer path shares its name",
    );
    run.assert_contains(
        "styles.css",
        "the block names the short path, not the long one",
    );
}

/// A renamed file must not read as unscannable in range mode.
///
/// Git's rename detection pairs an add and a delete of similar content into
/// ONE entry reported under the NEW path, while `--name-only` (which builds
/// the path list) still lists the OLD path. So the old path has no
/// `+++ b/<path>` header and the guard blocked the push for a path carrying
/// nothing. Nine paths blocked this way on the first push of `main`, every one
/// a file moved years ago. `--no-renames` exists for this.
#[test]
fn a_renamed_path_does_not_read_as_unscannable() {
    let fixture = Fixture::new();
    let repo = fixture.repo();

    fixture.commit("seed.txt", "seed\n", "seed");
    fixture.commit(
        "olddir/moved.txt",
        "a line\nb line\nc line\nd line\ne line\n",
        "add the file at its old path",
    );

    fs::create_dir_all(repo.join("newdir")).expect("creatable");
    git(&repo, &["mv", "olddir/moved.txt", "newdir/moved.txt"]);
    git(
        &repo,
        &["commit", "-q", "-m", "move it, which git reports as a rename"],
    );

    fixture
        .run(&["--range", "HEAD~2..HEAD"])
        .assert_status(0, "a renamed path does not read as unscannable");
}

/// An empty file counts as scanned, not as unscannable.
///
/// Git emits a `diff --git` line for a zero-byte file and no hunk, so it never
/// produces a `+++` header. It also cannot carry a secret. Blocking on it
/// refused a push over a 0-byte file nobody references.
///
/// The second half is **the positive control**, and it is the reason this test
/// asserts two things rather than one: a genuinely unreadable file in the same
/// shape must still block, or the fix above has made the guard useless rather
/// than correct.
#[test]
fn an_empty_file_is_scanned_but_a_non_empty_unreadable_one_still_blocks() {
    let fixture = Fixture::new();
    let repo = fixture.repo();

    fixture.commit("seed.txt", "seed\n", "seed");

    fs::write(repo.join("placeholder"), "").expect("writable");
    git(&repo, &["add", "placeholder"]);
    git(&repo, &["commit", "-q", "-m", "add a zero-byte file"]);

    fixture
        .run(&["--range", "HEAD~1..HEAD"])
        .assert_status(0, "an empty file does not read as unscannable");

    fs::write(repo.join(".gitattributes"), "placeholder -diff\n").expect("writable");
    fs::write(
        repo.join("placeholder"),
        "real content that cannot be read\n",
    )
    .expect("writable");
    git(&repo, &["add", ".gitattributes", "placeholder"]);
    git(
        &repo,
        &["commit", "-q", "-m", "mark a NON-empty file unreadable"],
    );

    fixture
        .run(&["--range", "HEAD~1..HEAD"])
        .assert_status(2, "a non-empty unreadable file still blocks");
}

// --- range mode: merge commits ----------------------------------------------

/// `git log -p` shows no diff for a merge commit by default, so content that
/// exists only in the merge itself (an "evil merge": a conflict resolution, or
/// extra content stapled on during the merge) is invisible unless the scan
/// passes `--diff-merges=first-parent`.
///
/// Side-branch commits are scanned either way, because the range walks them as
/// ordinary commits.
#[test]
fn range_a_secret_introduced_only_in_a_merge_commit_is_blocked() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let merge_base = fixture.commit("base.txt", &clean_text(), "merge base");

    git(&repo, &["checkout", "-q", "-b", "left", &merge_base]);
    fixture.commit("left.txt", &clean_text(), "left branch change");

    git(&repo, &["checkout", "-q", "-b", "right", &merge_base]);
    fixture.commit("right.txt", &clean_text(), "right branch change");

    git(&repo, &["checkout", "-q", "left"]);
    let _ = git_command(&repo)
        .args(["merge", "-q", "--no-commit", "--no-ff", "right"])
        .output();
    fixture.stage("evil.txt", &plant_key('A'));
    git(&repo, &["commit", "-q", "-m", "evil merge"]);
    let evil_merge = git_stdout(&repo, &["rev-parse", "HEAD"]);

    fixture
        .run(&["--range", &format!("{merge_base}..{evil_merge}")])
        .assert_status(
            1,
            "range: a secret introduced only in a merge commit is blocked",
        );
}

#[test]
fn range_an_ordinary_clean_merge_passes() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let merge_base = fixture.commit("base.txt", &clean_text(), "merge base");

    git(&repo, &["checkout", "-q", "-b", "left2", &merge_base]);
    fixture.commit("left2.txt", &clean_text(), "left2 branch change");
    git(&repo, &["checkout", "-q", "-b", "right2", &merge_base]);
    fixture.commit("right2.txt", &clean_text(), "right2 branch change");
    git(&repo, &["checkout", "-q", "left2"]);
    let _ = git_command(&repo)
        .args(["merge", "-q", "--no-edit", "right2"])
        .output();
    let clean_merge = git_stdout(&repo, &["rev-parse", "HEAD"]);

    fixture
        .run(&["--range", &format!("{merge_base}..{clean_merge}")])
        .assert_status(0, "range: an ordinary clean merge passes");
}

// --- pre-push wiring --------------------------------------------------------

#[test]
fn pre_push_allows_a_clean_range_and_blocks_one_with_a_secret() {
    let fixture = Fixture::new();
    let history = History::build(&fixture);

    fixture
        .run_pre_push(&history.clean_tip, &history.base)
        .assert_status(0, "pre-push: a clean range is allowed through the leak gate");

    let run = fixture.run_pre_push(&history.leaky_tip, &history.base);
    run.assert_status(1, "pre-push: a range with a secret is blocked");
    run.assert_contains("PUSH BLOCKED", "pre-push: the leak check reports the block");
    run.assert_contains(
        "pre-push: leak check failed",
        "pre-push: the hook names the failing gate",
    );
}

/// A brand-new remote branch has the zero sha, so the whole history is the
/// range.
#[test]
fn pre_push_scans_a_new_remote_branch_from_the_empty_tree() {
    let fixture = Fixture::new();
    let history = History::build(&fixture);
    fixture
        .run_pre_push(&history.leaky_tip, ZERO_SHA)
        .assert_status(
            1,
            "pre-push: a new remote branch is scanned from the empty tree",
        );
}

/// The remote sha is a well-formed but unknown object, not the zero sha "new
/// branch" sentinel. The guard cannot resolve the range and exits 2, and
/// `pre-push` must report that distinctly from an actual leak rather than
/// collapsing it into "leak check failed".
#[test]
fn pre_push_reports_an_unresolvable_range_as_could_not_scan() {
    let fixture = Fixture::new();
    let history = History::build(&fixture);
    let run = fixture.run_pre_push(&history.leaky_tip, "abcdef1234567890abcdef1234567890abcdef12");
    run.assert_status(1, "pre-push: an unresolvable range still blocks the push");
    run.assert_contains(
        "could not scan",
        "pre-push: an unresolvable range is reported as could-not-scan, not a leak",
    );
}

/// A delete-only push has the zero LOCAL sha for every ref, so the loop that
/// builds ranges never runs. The hook must still say so, per its own
/// convention that a skipped gate announces itself: a gate that says nothing
/// when it stands down is indistinguishable from a gate that is not installed.
#[test]
fn pre_push_announces_a_delete_only_push_and_a_clean_scan_count() {
    let fixture = Fixture::new();
    let history = History::build(&fixture);

    let run = fixture.run_pre_push(ZERO_SHA, &history.base);
    run.assert_status(0, "pre-push: a delete-only push passes");
    run.assert_contains(
        "no pushed range to scan",
        "pre-push: a delete-only push announces no range to scan",
    );

    fixture
        .run_pre_push(&history.clean_tip, &history.base)
        .assert_contains(
            "leak scan passed for 1 range(s)",
            "pre-push: a clean push announces the scanned range count",
        );
}

// --- the pattern file itself ------------------------------------------------

/// Layer 2 is the only layer defending project terms, and the pattern file it
/// reads is untracked on purpose, so it is **absent by default on every fresh
/// machine**. A missing file must block rather than silently reduce the guard
/// to its credential rules.
///
/// Status 3, not 2: `pre-push` reports 2 as "could not scan this range", which
/// is a different problem with a different fix.
///
/// The opt-out is asserted in the same test, because the block is only correct
/// if there is a stated way past it for a machine with no terms to defend.
#[test]
fn a_missing_pattern_file_blocks_unless_explicitly_permitted() {
    let fixture = Fixture::new();
    let absent = fixture.root().join("no-such-patterns.conf");
    assert!(
        !absent.exists(),
        "the fixture's stand-in pattern file is genuinely absent"
    );

    fixture.stage("term-only.txt", &plant_term());

    fixture
        .run_with(&[], |command| {
            command.env("LEAK_PATTERN_FILE", &absent);
        })
        .assert_status(3, "a missing pattern file is a configuration error");

    fixture
        .run_with(&[], |command| {
            command
                .env("LEAK_PATTERN_FILE", &absent)
                .env("LEAK_ALLOW_NO_PATTERNS", "1");
        })
        .assert_status(
            0,
            "the explicit opt-out permits a run with no pattern file",
        );
}

/// The sanctioned bypass of this repository's primary control must not fire on
/// a value that reads as "do not skip".
///
/// `[ -n ]` is true for the string `0`, so `SKIP_LEAK_CHECK=0` disabled the
/// guard for anyone who meant the opposite. An unrecognized value is announced
/// rather than silently declining, because the person who set it believes the
/// guard is off and would not understand the block.
#[test]
fn skip_leak_check_honours_only_true_values_and_announces_the_rest() {
    let fixture = Fixture::new();
    fixture.stage("skip-probe.txt", &plant_key('C'));

    fixture
        .run_with(&[], |command| {
            command.env("SKIP_LEAK_CHECK", "1");
        })
        .assert_status(0, "SKIP_LEAK_CHECK=1 skips");

    fixture
        .run_with(&[], |command| {
            command.env("SKIP_LEAK_CHECK", "0");
        })
        .assert_status(1, "SKIP_LEAK_CHECK=0 does not skip");

    fixture
        .run_with(&[], |command| {
            command.env("SKIP_LEAK_CHECK", "maybe");
        })
        .assert_contains(
            "not a recognized",
            "an unrecognized skip value is announced",
        );
}

// --- helpers ----------------------------------------------------------------

fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(program))
        .find(|candidate| candidate.is_file())
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("the file is executable");
}
