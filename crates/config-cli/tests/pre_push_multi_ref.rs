//! `tests/pre-push` gates every ref in a push, and its stamps are per-crate
//! rather than one workspace-wide id.
//!
//! A single push can carry several refs (`git push --atomic origin main
//! feature`, or a branch and a tag together), so several lines can arrive on
//! the hook's stdin where one used to. The hook took the first ref and ran
//! the stamp check against that one, which was defended by a comment calling
//! a second ref "rare enough".
//!
//! The gap that matters is the stamp check. It verifies the built
//! `config-cli` against the crate tree in the pushed ref, and two pushed refs
//! can hold different trees there. Checking only the first ref lets a binary
//! that is stale for the second ref through the gate that exists to catch
//! exactly that.
//!
//! This file also holds the gate's entry condition, which is the reason the
//! 2026-09-06 branch collapse went unnoticed. The hook used to set its gate
//! flag by matching `refs/heads/mac` and `refs/heads/linux`, and the shell
//! suite fed it exactly those two literals, so pushing `main` skipped the
//! whole stamp block and every suite stayed green.
//!
//! The hook is driven through its documented stdin protocol, one line per ref
//! as `<local ref> <local sha> <remote ref> <remote sha>`. The costly halves
//! (the leak scan, the Rust checks, and the Docker suite) are replaced with
//! stubs, because what is under test is which refs the hook considers, not
//! what those programs do with them.
//!
//! Converted whole from `tests/pre-push-multi-ref.test.sh`, which ran **17**
//! assertions, measured by running it rather than by counting `assert_` call
//! sites.

use dotfiles_test_support::repo::root as repo_root;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The hook under test.
fn hook() -> PathBuf {
    repo_root().join("tests/pre-push")
}

/// The real `config-stamp`, which this suite does NOT stub.
///
/// `pre-push` resolves it relative to its own script location, so the tool
/// that gates a push is the one that ships with the hook rather than
/// something read out of a possibly-fake `$HOME`.
fn stamp_command() -> PathBuf {
    repo_root().join(".scripts/config/config-stamp")
}

fn write_executable(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("the parent directory is creatable");
    }
    fs::write(path, body).expect("the file is writable");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("the file is executable");
}

/// Runs git in a directory, with any inherited git environment removed.
///
/// A hook or a pre-commit environment exports `GIT_DIR`, and with it set every
/// call here would answer for THAT repository rather than the fixture.
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

fn git(directory: &Path, arguments: &[&str]) -> String {
    let output = git_command(directory)
        .args(arguments)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {arguments:?} in {} failed: {}",
        directory.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .trim_end_matches('\n')
        .to_string()
}

/// An empty repository on `main`, with a committer configured.
fn make_repo(parent: &Path, name: &str) -> PathBuf {
    let path = parent.join(name);
    fs::create_dir_all(&path).expect("the fixture directory is creatable");
    git(&path, &["init", "-q", "-b", "main"]);
    git(&path, &["config", "user.email", "t@t"]);
    git(&path, &["config", "user.name", "t"]);
    git(
        &path,
        &["commit", "-q", "--allow-empty", "-m", "init"],
    );
    path
}

fn commit_all(repository: &Path, message: &str) -> String {
    git(repository, &["add", "-A"]);
    git(repository, &["commit", "-q", "-m", message]);
    git(repository, &["rev-parse", "HEAD"])
}

/// A fake `$HOME` holding stubs for every script the hook requires.
///
/// EVERY script the hook requires needs a stub here, because the hook fails
/// closed on a missing dependency and this fixture points `HOME` at a
/// directory holding only what this creates. Adding a hook dependency without
/// adding its stub makes every "the push passes" assertion fail with exit 1,
/// which reads as a ref-selection bug and is not one. That happened when the
/// Rust checks were added.
fn make_hook_home(parent: &Path) -> PathBuf {
    let home = parent.join("hookhome");
    for script in ["leak-check.sh", "run-in-docker.sh", "rust-checks.sh"] {
        write_executable(&home.join("tests").join(script), "#!/bin/sh\nexit 0\n");
    }
    home
}

/// The stub `config-cli`, reporting one stamp and refusing any other.
///
/// `verify-stamps` IS stubbed rather than left to a wildcard `exit 0`. The
/// real subcommand reads stdin and refuses on a mismatch; a stub that exits 0
/// unconditionally would pass this file regardless of what `pre-push` piped
/// in, which would hide the exact regression this suite exists to catch.
fn write_config_cli_stub(directory: &Path, stamp: &str) {
    write_executable(
        &directory.join("config-cli"),
        &format!(
            r#"#!/bin/sh
case $1 in
    --stamp) printf '%s\n' "{stamp}" ;;
    verify-stamps)
        status=0
        while IFS= read -r line; do
            [ -n "$line" ] || continue
            crate=${{line%% *}}
            expected=${{line#* }}
            if [ "$expected" != "{stamp}" ]; then
                printf 'stub: %s is stale (built %s, pushed %s)\n' \
                    "$crate" "{stamp}" "$expected" >&2
                status=1
            fi
        done
        exit "$status"
        ;;
    *) exit 0 ;;
esac
"#
        ),
    );
}

/// The hook's exit status and combined output for one push.
struct HookRun {
    status: i32,
    output: String,
}

/// Feeds the hook a push over its stdin protocol.
fn run_hook(
    repository: &Path,
    home: &Path,
    bin_dir: &Path,
    refs: &[(&str, &str)],
) -> HookRun {
    let stdin_text: String = refs
        .iter()
        .map(|(name, sha)| format!("{name} {sha} {name} {sha}\n"))
        .collect();

    let mut child = Command::new(hook())
        .args(["origin", &repository.display().to_string()])
        .current_dir(repository)
        .env("HOME", home)
        .env("CONFIG_BIN_DIR", bin_dir)
        // The hook resolves its own PATH for git; the stub directory goes
        // first so a stubbed config-cli is found before any real one.
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin_dir.display(),
                std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_string())
            ),
        )
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_PREFIX")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the hook runs");
    child
        .stdin
        .as_mut()
        .expect("a stdin pipe")
        .write_all(stdin_text.as_bytes())
        .expect("the ref list is writable");
    let output = child.wait_with_output().expect("the hook exits");

    HookRun {
        status: output.status.code().unwrap_or(-1),
        output: format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    }
}

/// A fixture repository with `main` and `feature` holding DIFFERENT crate
/// trees, plus the shas and folded stamps of each.
///
/// The difference is the whole point: if both branches carried the same tree,
/// a hook checking either ref would pass and the bug would be invisible.
struct TwoBranchFixture {
    repository: PathBuf,
    main_sha: String,
    feature_sha: String,
    main_tree: String,
    feature_tree: String,
    main_stamp: String,
    feature_stamp: String,
}

impl TwoBranchFixture {
    fn new(parent: &Path) -> Self {
        let repository = make_repo(parent, "push-multi");

        // A workspace manifest and lockfile alongside the crate, because
        // config-stamp enumerates members from crates/Cargo.toml rather than
        // hardcoding a single crate name.
        fs::create_dir_all(repository.join("crates/config-manifest/src"))
            .expect("the crate directory is creatable");
        fs::write(
            repository.join("crates/Cargo.toml"),
            "[workspace]\nmembers = [\"config-manifest\"]\n",
        )
        .expect("the manifest is writable");
        fs::write(repository.join("crates/Cargo.lock"), "lock\n").expect("the lock is writable");
        fs::write(
            repository.join("crates/config-manifest/src/main.rs"),
            "fn main() {}\n",
        )
        .expect("the source is writable");
        fs::write(
            repository.join("crates/config-manifest/lib.rs"),
            "main-crate\n",
        )
        .expect("the source is writable");
        let main_sha = commit_all(&repository, "main crate");
        let main_tree = git(&repository, &["rev-parse", "HEAD:crates/config-manifest"]);

        git(&repository, &["checkout", "-q", "-b", "feature"]);
        fs::write(
            repository.join("crates/config-manifest/lib.rs"),
            "feature-crate\n",
        )
        .expect("the source is writable");
        let feature_sha = commit_all(&repository, "feature crate");
        let feature_tree = git(&repository, &["rev-parse", "HEAD:crates/config-manifest"]);
        git(&repository, &["checkout", "-q", "main"]);

        let folded = |reference: &str, tree: &str| {
            format!(
                "{tree}:{}:{}",
                git(&repository, &["rev-parse", &format!("{reference}:crates/Cargo.lock")]),
                git(&repository, &["rev-parse", &format!("{reference}:crates/Cargo.toml")])
            )
        };
        let main_stamp = folded("HEAD", &main_tree);
        let feature_stamp = folded("feature", &feature_tree);

        Self {
            repository,
            main_sha,
            feature_sha,
            main_tree,
            feature_tree,
            main_stamp,
            feature_stamp,
        }
    }
}

/// The tracked hook is runnable at all.
#[test]
fn the_pre_push_hook_is_executable() {
    let executable = fs::metadata(hook())
        .is_ok_and(|data| data.permissions().mode() & 0o111 != 0);
    assert!(
        executable,
        "{} is missing or not executable, so git would skip the whole gate",
        hook().display()
    );
}

/// The stamp check must consider every pushed ref, in either order.
#[test]
fn the_stamp_gate_considers_every_pushed_ref() {
    let fixtures = tempfile::tempdir().expect("a temporary directory");
    let fixture = TwoBranchFixture::new(fixtures.path());
    let home = make_hook_home(fixtures.path());
    let stub_dir = fixtures.path().join("hook-stubs");
    fs::create_dir_all(&stub_dir).expect("the stub directory is creatable");

    assert_ne!(
        fixture.main_tree, fixture.feature_tree,
        "the fixture branches hold the same crate tree, so a hook checking \
         either ref would pass and the bug under test would be invisible"
    );

    // The binary is stamped for main. A push of main alone is legitimately
    // fine.
    write_config_cli_stub(&stub_dir, &fixture.main_stamp);
    let main_only = run_hook(
        &fixture.repository,
        &home,
        &stub_dir,
        &[("refs/heads/main", &fixture.main_sha)],
    );
    assert_eq!(
        main_only.status, 0,
        "a main-only push was blocked though the binary matches main: {}",
        main_only.output
    );

    // The gate must ACTUALLY RUN for main. This is the assertion whose
    // absence let the branch collapse silently disable stamp verification:
    // the old case arm matched refs/heads/mac and refs/heads/linux only, so
    // pushing main skipped the whole block and no suite noticed. Exit status
    // alone cannot catch that, because a skipped gate and a passed gate both
    // exit 0, so this reads the hook's own trace line.
    //
    // The preceding assertion is also the positive control: a hook that dies
    // before reaching either branch produces no output, and the check below
    // would be the only failure, which reads as a missing gate rather than a
    // broken hook.
    assert!(
        !main_only.output.is_empty(),
        "the hook produced no output at all for a main push"
    );
    assert!(
        main_only.output.contains("stamp gate passed"),
        "pushing main did not reach the stamp gate: {}",
        main_only.output
    );

    // The same binary, now pushing BOTH branches. feature carries a different
    // crate tree, so the binary is stale for feature and the push must be
    // blocked. Before the fix the hook read only the first ref, saw main, and
    // passed.
    let both = [
        ("refs/heads/main", fixture.main_sha.as_str()),
        ("refs/heads/feature", fixture.feature_sha.as_str()),
    ];
    let stale_second = run_hook(&fixture.repository, &home, &stub_dir, &both);
    assert_eq!(
        stale_second.status, 1,
        "a two-ref push passed while the binary was stale for the second \
         ref: {}",
        stale_second.output
    );

    // The mirror image: stamped for feature, pushing both. Whichever ref git
    // lists first, the hook must not pass a binary stale for the other one.
    write_config_cli_stub(&stub_dir, &fixture.feature_stamp);
    let stale_first = run_hook(&fixture.repository, &home, &stub_dir, &both);
    assert_eq!(
        stale_first.status, 1,
        "a two-ref push passed while the binary was stale for the first \
         ref: {}",
        stale_first.output
    );
}

/// A ref with no crates is skipped; a workspace naming zero members is not.
///
/// `config-stamp` exits 2 for both, so the hook must not read the two as one
/// condition: the first has nothing to gate, and the second is the "a gate
/// that checks nothing" failure this whole change closes.
#[test]
fn a_crateless_ref_is_skipped_but_a_broken_workspace_blocks() {
    let fixtures = tempfile::tempdir().expect("a temporary directory");
    let home = make_hook_home(fixtures.path());
    let stub_dir = fixtures.path().join("hook-stubs");
    write_config_cli_stub(&stub_dir, "irrelevant");

    let crateless = make_repo(fixtures.path(), "push-crateless");
    fs::write(crateless.join("README.md"), "notes\n").expect("the file is writable");
    let crateless_sha = commit_all(&crateless, "no crates");

    // An EMPTY binary directory on purpose. A ref with no crates needs no
    // built binary, so the hook must not demand one before it discovers there
    // is nothing to verify.
    let empty_bin = fixtures.path().join("no-binaries");
    fs::create_dir_all(&empty_bin).expect("the directory is creatable");

    let crateless_run = run_hook(
        &crateless,
        &home,
        &empty_bin,
        &[("refs/heads/main", &crateless_sha)],
    );
    // Positive control before the narrow property: an empty output would
    // satisfy a "the gate ran" check for the wrong reason.
    assert!(
        !crateless_run.output.is_empty(),
        "the hook produced no output at all for a crateless push"
    );
    assert_eq!(
        crateless_run.status, 0,
        "a ref carrying no crates was blocked by the stamp gate: {}",
        crateless_run.output
    );
    assert!(
        crateless_run.output.contains("stamp gate passed"),
        "a crateless push did not report that the gate ran: {}",
        crateless_run.output
    );

    // The other side of the same exit code. An empty member list must still
    // block the push, so the skip above cannot be widened to "config-stamp
    // failed".
    let broken = make_repo(fixtures.path(), "push-broken");
    fs::create_dir_all(broken.join("crates/config-manifest/src"))
        .expect("the crate directory is creatable");
    fs::write(
        broken.join("crates/Cargo.toml"),
        "[workspace]\nmembers = []\n",
    )
    .expect("the manifest is writable");
    fs::write(broken.join("crates/Cargo.lock"), "lock\n").expect("the lock is writable");
    fs::write(
        broken.join("crates/config-manifest/src/main.rs"),
        "fn main() {}\n",
    )
    .expect("the source is writable");
    let broken_sha = commit_all(&broken, "empty members");

    let broken_run = run_hook(&broken, &home, &stub_dir, &[("refs/heads/main", &broken_sha)]);
    assert!(
        !broken_run.output.is_empty(),
        "the hook produced no output at all for a broken-workspace push"
    );
    assert_eq!(
        broken_run.status, 1,
        "a workspace naming zero members did not block the push: {}",
        broken_run.output
    );
    assert!(
        broken_run.output.contains("cannot read workspace stamps"),
        "the block did not name the unreadable workspace: {}",
        broken_run.output
    );
}

/// Stamps are per-crate, not one workspace-wide id.
///
/// With one workspace-wide stamp, editing any crate marks every binary stale
/// and the gate refuses a push over a binary byte-identical to what its own
/// sources produce. A false refusal is how a gate gets bypassed.
#[test]
fn stamps_are_per_crate_and_an_empty_member_list_is_an_error() {
    let fixtures = tempfile::tempdir().expect("a temporary directory");
    let workspace = fixtures.path().join("stamp-scope");
    fs::create_dir_all(workspace.join("crates/crate-one")).expect("creatable");
    fs::create_dir_all(workspace.join("crates/crate-two")).expect("creatable");
    fs::write(
        workspace.join("crates/Cargo.toml"),
        "[workspace]\nmembers = [\"crate-one\", \"crate-two\"]\n",
    )
    .expect("the manifest is writable");
    fs::write(workspace.join("crates/Cargo.lock"), "lock\n").expect("the lock is writable");
    fs::write(workspace.join("crates/crate-one/src.rs"), "one\n").expect("writable");
    fs::write(workspace.join("crates/crate-two/src.rs"), "two\n").expect("writable");

    git(&workspace, &["init", "-q", "-b", "main"]);
    git(&workspace, &["config", "user.email", "t@t"]);
    git(&workspace, &["config", "user.name", "t"]);
    commit_all(&workspace, "init");

    let stamp_in_workspace = |crate_name: Option<&str>| -> std::process::Output {
        let mut command = Command::new(stamp_command());
        command.env("DOTFILES_ROOT", &workspace);
        if let Some(name) = crate_name {
            command.arg(name);
        }
        command.output().expect("config-stamp runs")
    };
    let stamp_text = |crate_name: &str| -> String {
        let output = stamp_in_workspace(Some(crate_name));
        String::from_utf8_lossy(&output.stdout)
            .trim_end_matches('\n')
            .to_string()
    };

    let before_two = stamp_text("crate-two");
    // Positive control. Both comparisons below hold two stamp outputs against
    // each other, so if config-stamp failed and printed nothing they would
    // compare empty to empty and pass vacuously.
    assert!(
        before_two.len() > 40
            && before_two
                .split(':')
                .next()
                .is_some_and(|first| first.len() == 40
                    && first.chars().all(|byte| byte.is_ascii_hexdigit())),
        "the fixture stamp is not well formed: {before_two:?}"
    );

    fs::write(workspace.join("crates/crate-one/src.rs"), "one changed\n").expect("writable");
    commit_all(&workspace, "edit crate-one");

    let after_two = stamp_text("crate-two");
    let after_one = stamp_text("crate-one");

    assert_eq!(
        before_two, after_two,
        "editing crate-one changed crate-two's stamp, so every binary would \
         be marked stale by an unrelated edit"
    );
    assert_ne!(
        after_one, before_two,
        "editing crate-one left its own stamp unchanged, so the gate would \
         pass a genuinely stale binary"
    );

    // An empty member list must abort rather than iterate zero crates. A gate
    // that checks nothing and exits 0 is the failure this whole change closes.
    fs::write(
        workspace.join("crates/Cargo.toml"),
        "[workspace]\nmembers = []\n",
    )
    .expect("the manifest is writable");
    commit_all(&workspace, "empty members");

    let empty = stamp_in_workspace(None);
    assert_eq!(
        empty.status.code(),
        Some(2),
        "an empty member list was not an error: {}",
        String::from_utf8_lossy(&empty.stderr)
    );
}
