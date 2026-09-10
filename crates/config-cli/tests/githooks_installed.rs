//! This machine's git hooks are installed, not merely present in the work
//! tree.
//!
//! The hook scripts (`tests/pre-commit`, `tests/pre-push`) are tracked, so
//! they travel with the repo. The symlinks under `.cfg/hooks` that make git
//! actually run them do not travel, and nothing recreates them on a new
//! machine. This machine had `tests/pre-push` tracked and executable for a
//! full day while `.cfg/hooks/pre-push` did not exist, so every push skipped
//! the leak scan, the stamp gate and the test suite silently.
//! `container_image.rs` asserts the hook file's contents; only this file
//! asserts git will run it.
//!
//! Skipped entirely where there is no `.cfg` repository: the test image
//! carries the tracked files but not the bare repo they came from, and a
//! machine without `.cfg` has no hooks to install.
//!
//! Converted whole from `tests/githooks-installed.test.sh`, which ran **7**
//! assertions from 4 `assert_*` call sites: one for `core.hooksPath` plus
//! three inside a loop over two hooks. The shell suite's skip branch spelled
//! all seven out by hand so the tally matched; here one `skip` names the
//! whole suite, because the Rust harness counts tests rather than
//! assertions.
//!
//! The skip decision lives in [`hooks_repository`] rather than inline, so
//! both directions are testable. A test must never call
//! `dotfiles_test_support::skip` to prove skipping works: `skip` reads the
//! ambient `DOTFILES_SKIP_LOG` the gate sets, so such a test records a
//! phantom skip in the gate's own tally.

use dotfiles_test_support::repo::root as repo_root;
use dotfiles_test_support::skip;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The two hooks that must be installed for a push or commit to be gated.
const HOOKS: [&str; 2] = ["pre-commit", "pre-push"];

/// The `.cfg` repository under a root, or `None` where there is none.
///
/// Separated from the tests so the skip condition can be exercised in both
/// directions against a temporary tree, without touching the environment
/// that parallel tests share.
fn hooks_repository(root: &Path) -> Option<PathBuf> {
    let candidate = root.join(".cfg");
    candidate.is_dir().then_some(candidate)
}

/// The value of `core.hooksPath`, empty when unset.
fn configured_hooks_path(cfg_dir: &Path, root: &Path) -> String {
    let output = Command::new("git")
        .arg(format!("--git-dir={}", cfg_dir.display()))
        .arg(format!("--work-tree={}", root.display()))
        .args(["config", "--get", "core.hooksPath"])
        .output();
    match output {
        Ok(result) => String::from_utf8_lossy(&result.stdout).trim().to_string(),
        Err(_) => String::new(),
    }
}

/// Whether a path is executable by its owner.
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|data| data.permissions().mode() & 0o111 != 0)
}

#[test]
fn the_hooks_are_installed_and_are_the_tracked_scripts() {
    let root = repo_root();
    let Some(cfg_dir) = hooks_repository(&root) else {
        skip(&format!(
            "no .cfg repository at {}: the hook symlinks cannot exist",
            root.join(".cfg").display()
        ));
        return;
    };

    // core.hooksPath overrides $GIT_DIR/hooks wholesale. If it is ever set,
    // the symlinks below stop being the thing git runs, and every assertion
    // after this one would pass while no hook fired.
    let hooks_path = configured_hooks_path(&cfg_dir, &root);
    assert_eq!(
        hooks_path, "",
        "core.hooksPath is set to {hooks_path:?}, which replaces .cfg/hooks \
         wholesale, so the symlinks below are not what git runs"
    );

    for hook in HOOKS {
        let installed = cfg_dir.join("hooks").join(hook);
        let tracked = root.join("tests").join(hook);

        assert!(
            installed.symlink_metadata().is_ok(),
            "{hook} is not installed at {}; run `config install-hooks`",
            installed.display()
        );

        // Executability is what git checks before running a hook. A present
        // but non-executable hook is skipped without a word.
        assert!(
            is_executable(&installed),
            "{hook} at {} is not executable, so git skips it silently",
            installed.display()
        );

        // The installed hook must be the tracked one, so editing
        // `tests/<hook>` changes what runs. A copy would drift silently.
        let target = std::fs::read_link(&installed).unwrap_or_else(|_| installed.clone());
        assert_eq!(
            target,
            tracked,
            "{hook} resolves to {} rather than the tracked {}",
            target.display(),
            tracked.display()
        );
    }
}

/// The skip condition fires where there is no `.cfg`, and does not where
/// there is one.
///
/// Both directions, because a skip predicate that always says "skip" removes
/// the suite from every machine, which is the failure the shell version's
/// own comment records: a silent skip took this suite out of the container
/// and out of CI at once.
#[test]
fn the_skip_condition_tracks_the_presence_of_a_cfg_repository() {
    let empty = tempfile::tempdir().expect("a temporary directory");
    assert!(
        hooks_repository(empty.path()).is_none(),
        "a tree with no .cfg must skip, or the suite fails on every machine \
         that legitimately has no bare repository"
    );

    let populated = tempfile::tempdir().expect("a temporary directory");
    std::fs::create_dir(populated.path().join(".cfg")).expect("a fixture .cfg directory");
    assert_eq!(
        hooks_repository(populated.path()),
        Some(populated.path().join(".cfg")),
        "a tree with a .cfg must NOT skip, or the suite silently asserts \
         nothing on the machine it exists to guard"
    );
}
