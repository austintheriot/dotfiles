//! The crate build lifecycle: the stamp, the build script, and the binaries
//! being on `PATH`.
//!
//! A stamp is the tree id of a crate as it is in the WORKTREE, computed
//! through a temp index, so the same content stamps identically whether or
//! not it is committed. `config-build` embeds it in each binary via
//! `CONFIG_MANIFEST_STAMP`; `pre-push` asks each binary for it and compares it
//! to the pushed commit's subtree id, refusing a stale binary without
//! compiling.
//!
//! The crate named `config-manifest` appears throughout as a fixture crate
//! name, built inside throwaway repositories. The real crate of that name is
//! library-only and installs no binary; `config-cli` is the installed binary
//! these assertions reach for.
//!
//! The build half needs cargo. Inside the Docker suite there is no cargo (the
//! runtime image is Rust-free; the binary is copied in from a builder stage),
//! so that half skips and the binary assertions still run.
//!
//! Converted whole from `tests/config-manifest-lifecycle.test.sh`, which ran
//! **33** assertions, measured by running it rather than by counting
//! `assert_` call sites.

use dotfiles_test_support::repo::root as repo_root;
use dotfiles_test_support::skip;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn stamp_command() -> PathBuf {
    repo_root().join(".scripts/config/config-stamp")
}

fn build_command() -> PathBuf {
    repo_root().join(".scripts/config/config-build")
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

fn commit(repository: &Path, message: &str) {
    git(repository, &["add", "-A"]);
    git(
        repository,
        &[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "-m",
            message,
        ],
    );
}

/// A throwaway repository carrying one workspace member named
/// `config-manifest`.
fn make_fixture_repo(parent: &Path) -> PathBuf {
    let repository = parent.join("stamp");
    fs::create_dir_all(repository.join("crates/config-manifest/src"))
        .expect("the crate directory is creatable");
    git(&repository, &["init", "-q", "-b", "main"]);
    fs::write(
        repository.join("crates/Cargo.toml"),
        "[workspace]\nmembers = [\"config-manifest\"]\n",
    )
    .expect("the manifest is writable");
    fs::write(repository.join("crates/Cargo.lock"), "lock\n").expect("the lock is writable");
    fs::write(
        repository.join("crates/config-manifest/Cargo.toml"),
        "[package]\nname = \"config-manifest\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("the crate manifest is writable");
    fs::write(
        repository.join("crates/config-manifest/src/main.rs"),
        "fn main() {}\n",
    )
    .expect("the source is writable");
    commit(&repository, "add crate");
    repository
}

/// Runs `config-stamp` against a fixture root and returns its trimmed stdout.
fn stamp_in(root: &Path, arguments: &[&str]) -> String {
    let output = Command::new(stamp_command())
        .args(arguments)
        .env("DOTFILES_ROOT", root)
        .output()
        .expect("config-stamp runs");
    String::from_utf8_lossy(&output.stdout)
        .trim_end_matches('\n')
        .to_string()
}

/// Whether a string is a folded triple of 40-hex object ids.
fn is_folded_stamp(text: &str) -> bool {
    let parts: Vec<&str> = text.split(':').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| part.len() == 40 && part.chars().all(|byte| byte.is_ascii_hexdigit()))
}

/// The stamp follows worktree content, not commits.
#[test]
fn the_stamp_follows_worktree_content() {
    let fixtures = tempfile::tempdir().expect("a temporary directory");
    let repository = make_fixture_repo(fixtures.path());

    let committed = git(&repository, &["rev-parse", "HEAD:crates/config-manifest"]);
    let stamped = stamp_in(&repository, &["config-manifest"]);

    assert!(
        is_folded_stamp(&stamped),
        "the stamp is not a folded triple of 40-hex object ids: {stamped:?}"
    );
    assert_eq!(
        stamped.split(':').next().unwrap_or_default(),
        committed,
        "a clean worktree did not stamp to the committed subtree id"
    );

    fs::write(
        repository.join("crates/config-manifest/src/main.rs"),
        "fn main() { println!(\"edited\"); }\n",
    )
    .expect("the source is writable");
    let edited = stamp_in(&repository, &["config-manifest"]);
    assert_ne!(
        edited, committed,
        "an uncommitted edit left the stamp unchanged, so the push gate \
         would pass a binary built from different source"
    );

    commit(&repository, "edit");
    assert_eq!(
        edited.split(':').next().unwrap_or_default(),
        git(&repository, &["rev-parse", "HEAD:crates/config-manifest"]),
        "committing the same content did not stamp to the new subtree id"
    );

    // Files that are neither tracked nor addable do not move the stamp.
    fs::create_dir_all(repository.join("crates/config-manifest/target"))
        .expect("the target directory is creatable");
    fs::write(
        repository.join("crates/config-manifest/target/junk"),
        "junk\n",
    )
    .expect("the junk file is writable");
    fs::write(
        repository.join("crates/config-manifest/.gitignore"),
        "target/\n",
    )
    .expect("the ignore file is writable");
    commit(&repository, "ignore target");
    assert_eq!(
        stamp_in(&repository, &["config-manifest"])
            .split(':')
            .next()
            .unwrap_or_default(),
        git(&repository, &["rev-parse", "HEAD:crates/config-manifest"]),
        "an ignored file moved the stamp, so every build would look stale"
    );
}

/// The installed binary is on `PATH`, and the library member installs none.
#[test]
fn the_installed_binaries_are_what_they_should_be() {
    assert!(
        which("config-cli").is_some(),
        "config-cli is not on PATH, so the gate's stamp comparison has \
         nothing to ask"
    );

    assert!(
        which("config-manifest").is_none(),
        "config-manifest is on PATH though it is library-only, which is how \
         a stale second binary would go unnoticed"
    );
}

/// The first `PATH` entry holding an executable of this name.
fn which(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")?
        .to_str()?
        .split(':')
        .map(|directory| Path::new(directory).join(name))
        .find(|candidate| {
            fs::metadata(candidate).is_ok_and(|data| data.permissions().mode() & 0o111 != 0)
        })
}

/// `config-build` installs the binary, skips the library, embeds the stamp,
/// and stays quiet when its output is captured.
///
/// Compiles the whole workspace, so this is the slow test in the file.
#[test]
fn config_build_installs_and_stamps() {
    if which("cargo").is_none() {
        skip("cargo is not installed, so the config-build assertions cannot run");
        return;
    }
    let fixtures = tempfile::tempdir().expect("a temporary directory");
    let bin_dir = fixtures.path().join("bin");

    let build = Command::new(build_command())
        .env("CONFIG_BIN_DIR", &bin_dir)
        .output()
        .expect("config-build runs");
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    assert_eq!(
        build.status.code(),
        Some(0),
        "config-build did not exit 0: {output}"
    );
    let installed = bin_dir.join("config-cli");
    assert!(
        fs::metadata(&installed).is_ok_and(|data| data.permissions().mode() & 0o111 != 0),
        "config-build installed no executable at {}: {output}",
        installed.display()
    );
    assert!(
        output.contains(&installed.display().to_string()),
        "config-build did not report where it installed: {output}"
    );

    // A library member is built but not installed, so a crate that stops
    // being a binary stops producing one. A silent install of a stale copy is
    // how the two-binary window would reopen.
    assert!(
        output.contains("config-manifest is a library, nothing to install"),
        "config-build did not say the library installs nothing: {output}"
    );
    assert!(
        !bin_dir.join("config-manifest").exists(),
        "config-build installed a binary for a library member"
    );

    // Cargo's progress reaches a human, and only a human. `cargo build
    // --quiet` suppresses the per-crate "Compiling ..." lines, which are the
    // only progress a cold compile of six crates emits, and `config init`
    // runs this with no redirection. Gating on a tty keeps both: progress
    // where somebody is watching, silence where the output is captured. This
    // capture is the piped case.
    let progress: Vec<&str> = output
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            line.starts_with(char::is_whitespace)
                && (trimmed.starts_with("Compiling") || trimmed.starts_with("Finished"))
        })
        .collect();
    assert!(
        progress.is_empty(),
        "a captured build printed cargo progress: {progress:?}"
    );

    // The tty half, asserted by reading the script rather than by allocating
    // a pty: the suite runs headless in eight CI legs and in Docker, so a
    // real terminal is not available. What is checkable is that the quiet
    // flag is CONDITIONAL. An unconditional `--quiet` is the defect, and an
    // unconditional absence is the noise.
    //
    // Comments are stripped first, so a mention inside one cannot satisfy
    // either check. That exact vacuity bit two assertions in Tranche A.
    let build_code: String = fs::read_to_string(build_command())
        .expect("config-build is readable")
        .lines()
        .map(|line| format!("{}\n", line.split('#').next().unwrap_or("")))
        .collect();
    assert!(
        build_code.contains("-t 1"),
        "config-build does not decide quietness from a tty"
    );
    assert!(
        !build_code
            .lines()
            .any(|line| line.contains("cargo build") && line.contains("--quiet")),
        "config-build passes --quiet unconditionally, which is what made a \
         cold `config init` print one line and then nothing for minutes"
    );

    // The stamp lives inside the binary, not in a file beside it, so a stale
    // or foreign config-cli on PATH cannot report a stamp it was not built
    // with.
    let worktree_stamp = stamp_in(&repo_root(), &["config-cli"]);
    assert!(
        is_folded_stamp(&worktree_stamp),
        "the worktree stamp is malformed, so the two comparisons below would \
         hold one empty string against another: {worktree_stamp:?}"
    );
    let reported = Command::new(&installed)
        .arg("--stamp")
        .output()
        .expect("the installed binary runs");
    assert_eq!(
        String::from_utf8_lossy(&reported.stdout).trim_end_matches('\n'),
        worktree_stamp,
        "the installed binary does not report the worktree stamp"
    );
    assert!(
        output.contains(&worktree_stamp),
        "config-build did not report the stamp it embedded: {output}"
    );

    // A build with no CONFIG_MANIFEST_STAMP must SAY so rather than print an
    // empty line, which pre-push would compare against a real tree id. This
    // is how Docker and CI build the binary.
    let unstamped_target = fixtures.path().join("unstamped-target");
    let compiled = Command::new("cargo")
        .args(["build", "--release", "--locked", "--quiet"])
        .arg("--manifest-path")
        .arg(repo_root().join("crates/config-cli/Cargo.toml"))
        .env("CARGO_TARGET_DIR", &unstamped_target)
        .env_remove("CONFIG_MANIFEST_STAMP")
        .output()
        .expect("cargo runs");
    assert!(
        compiled.status.success(),
        "the unstamped build failed: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let unstamped = Command::new(unstamped_target.join("release/config-cli"))
        .arg("--stamp")
        .output()
        .expect("the unstamped binary runs");
    assert_eq!(
        String::from_utf8_lossy(&unstamped.stdout).trim_end_matches('\n'),
        "unstamped",
        "a binary built without the variable did not report `unstamped`, so \
         the gate would compare an empty string against a real tree id"
    );
}

/// The shell drift check is gone and nothing calls it.
#[test]
fn the_shell_drift_check_is_gone_and_uncalled() {
    let root = repo_root();
    assert!(
        !root.join("tests/check-branch-drift.sh").exists(),
        "tests/check-branch-drift.sh is back"
    );

    let callers = |needle: &str| -> String {
        let output = Command::new("grep")
            .args(["-rln", needle])
            .arg(root.join("tests"))
            .arg(root.join(".github"))
            .arg(root.join(".scripts"))
            .arg(root.join(".claude/rules"))
            .output()
            .expect("grep runs");
        // The searched trees are tests/, .github/, .scripts/ and
        // .claude/rules/, matching the shell suite. NOT crates/: that tree
        // holds cargo's build artifacts, and grepping it walked hundreds of
        // .rmeta files and matched this test binary's own incremental cache.
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    };

    // Positive control. The assertion below expects an EMPTY result, so
    // without a pattern that must match, a broken grep invocation would
    // report a pass.
    assert!(
        callers("config-build").contains("config/config-build"),
        "the caller search found nothing for a name that is genuinely \
         referenced, so the empty result below would prove nothing"
    );

    let stale = callers(r"check-branch-drift\.sh");
    assert_eq!(
        stale, "",
        "a script, workflow, or rule still names check-branch-drift.sh: \
         {stale}"
    );
}

/// The toolchain pin names an exact version, and it is the one installed.
///
/// The stamp covers source, not compiler. `edition = "2024"` is not a pin:
/// every toolchain from 1.85 onward compiles it, so two machines could
/// produce matching stamps from different compilers with the gate seeing no
/// difference.
#[test]
fn the_toolchain_is_pinned_to_the_installed_version() {
    let toolchain_file = repo_root().join("crates/rust-toolchain.toml");
    if !toolchain_file.is_file() || which("rustc").is_none() {
        skip("crates/rust-toolchain.toml or rustc is absent, so the pin cannot be checked");
        return;
    }

    let text = fs::read_to_string(&toolchain_file).expect("the toolchain file is readable");
    let pinned = text
        .lines()
        .find_map(|line| line.strip_prefix("channel = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or_default()
        .to_string();
    let exact = pinned.split('.').count() == 3
        && pinned
            .split('.')
            .all(|part| !part.is_empty() && part.chars().all(|byte| byte.is_ascii_digit()));
    assert!(
        exact,
        "the pin names a channel rather than an exact version: {pinned:?}"
    );

    // Read from INSIDE crates/, because rustup only honours
    // crates/rust-toolchain.toml when the working directory is under it.
    // Reading from wherever the suite happens to run reports the machine's
    // DEFAULT toolchain, which passed on a developer box whose default
    // already matched and failed on a runner whose default was 1.98.0.
    let version = Command::new("rustc")
        .arg("--version")
        .current_dir(toolchain_file.parent().expect("crates/ is the parent"))
        .output()
        .expect("rustc runs");
    let installed = String::from_utf8_lossy(&version.stdout)
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        pinned, installed,
        "the pinned toolchain is not the one installed"
    );
}

/// One workspace, one lockfile, one resolution.
///
/// The members list is also what the stamp enumerates, so it is the single
/// place that says which crates exist.
#[test]
fn there_is_one_workspace_and_one_lockfile() {
    let root = repo_root();
    let manifest = root.join("crates/Cargo.toml");
    if !manifest.is_file() {
        skip("crates/Cargo.toml is absent, so the workspace shape cannot be checked");
        return;
    }

    let text = fs::read_to_string(&manifest).expect("the workspace manifest is readable");
    assert!(
        text.lines().any(|line| line.starts_with("members = [")),
        "the workspace declares no members, and the stamp enumerates that list"
    );
    assert!(
        root.join("crates/Cargo.lock").is_file(),
        "the shared lockfile is not at the workspace root"
    );
}

/// No per-crate lockfile and no proptest regressions file is tracked.
#[test]
fn no_per_crate_lockfile_or_regressions_file_is_tracked() {
    let root = repo_root();
    let cfg = root.join(".cfg");
    if !cfg.is_dir() {
        skip("no .cfg repository here, so tracked paths cannot be listed");
        return;
    }

    // Rooted pathspecs (`:/crates/...`) and `--full-name`, because
    // `cargo test` runs with the working directory at `crates/config-cli`.
    // A bare relative pathspec resolves against THAT directory and matches
    // nothing, which the positive control below caught on the first run.
    let ls_files = |pathspec: &str| -> String {
        let output = Command::new("git")
            .arg(format!("--git-dir={}", cfg.display()))
            .arg(format!("--work-tree={}", root.display()))
            .args(["ls-files", "--full-name", pathspec])
            .output()
            .expect("git ls-files runs");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    };

    // Positive control first. An empty-expected assertion passes when its
    // pipeline breaks for an unrelated reason, so prove the pipeline reaches
    // the repository before asserting the narrow property.
    assert!(
        !ls_files(":/crates/*").is_empty(),
        "ls-files reached no crates at all, so the two empty results below \
         would prove nothing"
    );
    assert_eq!(
        ls_files(":/crates/*/Cargo.lock"),
        "",
        "a per-crate lockfile is still tracked"
    );
    assert_eq!(
        ls_files(":/crates/*/proptest-regressions/*"),
        "",
        "a proptest regressions file is tracked"
    );
}

/// `config-stamp` is per-crate and ref-scoped, and names an unknown crate.
///
/// Driven against a fixture repository, not the ambient root: the Docker
/// runner's image carries no `.git` or `.cfg` at all, so a call defaulting to
/// `$HOME` would fail there for a reason unrelated to `config-stamp` itself.
#[test]
fn config_stamp_is_per_crate_and_ref_scoped() {
    let fixtures = tempfile::tempdir().expect("a temporary directory");
    let repository = make_fixture_repo(fixtures.path());

    // The per-crate form is what pre-push iterates.
    let listing = stamp_in(&repository, &[]);
    let named = listing.lines().any(|line| {
        line.strip_prefix("config-manifest ")
            .is_some_and(is_folded_stamp)
    });
    assert!(
        named,
        "config-stamp did not name config-manifest with a folded stamp: \
         {listing:?}"
    );

    // The single-crate form is for scripting.
    let one = stamp_in(&repository, &["config-manifest"]);
    assert!(
        is_folded_stamp(&one),
        "config-stamp <crate> did not print a bare folded stamp: {one:?}"
    );

    // The error path names the crate, so a status 2 from an unrelated cause
    // (a missing usage.sh, a mktemp failure) does not read as this error.
    let unknown = Command::new(stamp_command())
        .arg("no-such-crate")
        .env("DOTFILES_ROOT", &repository)
        .output()
        .expect("config-stamp runs");
    let message = format!(
        "{}{}",
        String::from_utf8_lossy(&unknown.stdout),
        String::from_utf8_lossy(&unknown.stderr)
    );
    assert!(
        message.contains("no-such-crate"),
        "an unknown crate is not named in the error: {message:?}"
    );
    assert_eq!(
        unknown.status.code(),
        Some(2),
        "an unknown crate did not exit 2"
    );

    // A ref-scoped stamp is what the push gate needs: it must compare the
    // binary against what is being published, not against the worktree.
    let head_stamp = stamp_in(&repository, &["--ref", "HEAD", "config-manifest"]);
    assert!(
        is_folded_stamp(&head_stamp),
        "a ref-scoped stamp is malformed: {head_stamp:?}"
    );
}
