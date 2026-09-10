//! The name of the user scripts directory, and the modes of the scripts in it.
//!
//! The directory was renamed from the old dotted name to `.scripts`, and the
//! old name was referenced from 33 tracked files: shell aliases, tmux hooks,
//! two Dockerfile COPY targets, three anchored regexes (the leak-check allow
//! list, the pre-push trigger filter, and a CI path filter), and a lot of
//! prose. A rename that misses any one of those fails silently rather than
//! loudly: an unmatched tmux hook path just stops renaming windows, and an
//! unmatched trigger regex just stops running the test suite before a push.
//!
//! So this asserts the invariant rather than the edit: the directory has
//! exactly one name, and that name appears nowhere in its old form.
//!
//! Converted whole from `tests/scripts-dir-name.test.sh`, which reported 15
//! assertions from 15 `assert_*` call sites plus two `skip`s.
//!
//! Two things the shell version could not do, and this one does:
//!
//! 1. The shell suite excluded ITSELF from the stale-name sweep, because it
//!    names the old directory in its own prose. That exclusion was a hole,
//!    patched by a fifteenth assertion that grepped the suite's own text for
//!    a live path. Here the needle is assembled from runtime segments, so
//!    this file is not a match and needs no exclusion and no patch.
//! 2. `git ls-tree` runs with a rooted pathspec and `--full-name`, because
//!    an integration test's cwd is `crates/config-cli` rather than the work
//!    tree root. A bare pathspec would resolve against the cwd prefix and
//!    list nothing, which compares an empty set against an empty set and
//!    passes while asserting nothing.

use dotfiles_test_support::repo::root as repo_root;
use std::path::{Path, PathBuf};
use std::process::Command;

// --- the two names --------------------------------------------------------

/// The current name of the user scripts directory.
const NEW_NAME: &str = ".scripts";

/// The old name, assembled at run time rather than written as a literal.
///
/// This file is inside the sweep's own search roots, so a literal here would
/// make the sweep report this file forever. The shell suite hit exactly that
/// and answered it by excluding itself, which left a hole it then needed a
/// separate assertion to cover. Assembling the needle removes the hole
/// instead of patching it.
fn old_stem() -> String {
    format!("{}-scripts", "my")
}

/// The old dotted directory name, assembled for the same reason.
fn old_name() -> String {
    format!(".{}", old_stem())
}

// --- the script lists -----------------------------------------------------

/// The scripts something runs as a command, relative to `.scripts`.
///
/// The execute bit is load-bearing for each: a tmux hook or a CI step that
/// runs one as a command fails silently when the bit is missing.
const EXECUTED_SCRIPTS: [&str; 12] = [
    "alacritty-platform.sh",
    "tmux-update-window-names.sh",
    "tmux-worktree-config.sh",
    "config/config-stamp",
    "config/config-build",
    "config/config",
    "config/config-install-hooks",
    "config/config-install-repo-hooks",
    "config/config-install",
    "config/config-test",
    "config/config-reload",
    "git-hooks/post-checkout",
];

/// The scripts `.zshrc` sources, relative to `.scripts`.
///
/// Deliberately NOT executable. `source` ignores the execute bit, so marking
/// one executable would advertise a way of running it that does not work:
/// executing `tmux-split.sh` in a subshell changes that subshell and exits.
const SOURCED_SCRIPTS: [&str; 6] = [
    "tmux-close.sh",
    "tmux-setup.sh",
    "tmux-split.sh",
    "tmux-start.sh",
    "zsh-git-widgets.sh",
    "platform.sh",
];

/// The committed execute bits under `.scripts`, as an exact set.
///
/// An exact set rather than a subset: a new executed script that nobody adds
/// here is a script whose committed bit nothing checks.
const COMMITTED_EXECUTABLE: [&str; 17] = [
    "alacritty-platform.sh",
    "config/config",
    "config/config-build",
    "config/config-deps",
    "config/config-doctor",
    "config/config-help",
    "config/config-init",
    "config/config-install",
    "config/config-install-hooks",
    "config/config-install-repo-hooks",
    "config/config-prereqs",
    "config/config-reload",
    "config/config-stamp",
    "config/config-test",
    "git-hooks/post-checkout",
    "tmux-update-window-names.sh",
    "tmux-worktree-config.sh",
];

/// How many files live under `.scripts` in the commit.
///
/// An exact count rather than "more than zero": a partial `git add` is the
/// failure that actually happened. The commit that renamed the directory
/// recorded 15 deletions and zero additions, because the files were absent
/// from disk when `git add` ran. `git add` on a missing path stages nothing
/// and exits 0, so the staged rename collapsed into pure deletions and the
/// commit passed every on-disk check. HEAD carried 237 tracked files instead
/// of 252 and every tmux hook broke with "returned 127".
///
/// The number moves when a file is added or removed under `.scripts`, and
/// that is the point. Update it deliberately, in the same commit as the
/// change that moves it.
const COMMITTED_SCRIPT_COUNT: usize = 24;

/// The tracked roots the stale-name sweep reads.
///
/// An allowlist of tracked paths, not a filesystem walk. The root is `$HOME`
/// on a developer machine, where a few hundred tracked files sit among tens
/// of thousands on disk: `.claude` alone holds gigabytes of vendored plugins
/// and session transcripts. An unscoped walk spends minutes reading trees a
/// rename must never rewrite, and the transcripts quote the old name as
/// conversation history, so it would also report them forever.
const SEARCH_ROOTS: [&str; 21] = [
    ".agents",
    ".config",
    ".github",
    "tests",
    "docs",
    ".claude/agents",
    ".claude/data",
    ".claude/hooks",
    ".claude/rules",
    ".claude/scripts",
    ".claude/skills",
    ".claude/CLAUDE.md",
    ".zshrc",
    ".zshrc-mac",
    ".zshrc-linux",
    "DOTFILES.md",
    "README.md",
    "README-MAC.md",
    "README-LINUX.md",
    "TODO-AGENTS.md",
    ".scripts",
];

// --- reading the tree -----------------------------------------------------

/// Whether a path is executable by its owner.
///
/// Read through the mode bits rather than by running the file, so a script
/// with a broken interpreter line still reports the property under test.
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
}

/// The git directory of this checkout, or `None` where there is no repository.
///
/// THE CHECK THAT DISTINGUISHES A WORKING REPOSITORY FROM AN ARCHIVED TREE.
/// `root.join(".git").exists()` is the obvious form and it is wrong here:
/// this repository is bare at `~/.cfg` with `$HOME` as the work tree, so on a
/// developer machine that path does not exist and the check would skip the
/// assertion on the one machine where it can run. On a CI runner the root IS
/// a checkout and `.git` is a real directory. Both shapes are live, so both
/// are tried, and each is confirmed by asking git rather than the filesystem:
/// a directory named `.git` that git cannot resolve a HEAD out of is not a
/// repository this test can inspect.
///
/// The container tree is built with `git archive` and carries neither, which
/// is a correct reason for a commit-inspection assertion to stand down.
fn git_dir() -> Option<PathBuf> {
    let root = repo_root();
    for candidate in [root.join(".cfg"), root.join(".git")] {
        let resolved = Command::new("git")
            .arg(format!("--git-dir={}", candidate.display()))
            .args(["rev-parse", "--verify", "HEAD"])
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .output()
            .is_ok_and(|output| output.status.success());
        if resolved {
            return Some(candidate);
        }
    }
    None
}

/// `git ls-tree -r` over `.scripts` at HEAD, as raw lines.
///
/// The pathspec is rooted (`:/.scripts`) and `--full-name` is passed, both
/// because this test's cwd is `crates/config-cli` rather than the work tree
/// root. A bare `.scripts` pathspec resolves against the cwd prefix, so git
/// looks for `crates/config-cli/.scripts`, lists nothing, and every set
/// comparison below compares an empty left side against an empty right side
/// and passes while asserting nothing.
fn committed_scripts(git_dir: &Path) -> Vec<String> {
    let root = repo_root();
    let listing = Command::new("git")
        .arg(format!("--git-dir={}", git_dir.display()))
        .arg(format!("--work-tree={}", root.display()))
        .args([
            "ls-tree",
            "-r",
            "--full-name",
            "HEAD",
            &format!(":/{NEW_NAME}"),
        ])
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .expect("git ls-tree runs");
    assert!(
        listing.status.success(),
        "git ls-tree failed: {}",
        String::from_utf8_lossy(&listing.stderr)
    );
    let text = String::from_utf8_lossy(&listing.stdout);
    assert!(
        !text.trim().is_empty(),
        "positive control: git ls-tree listed nothing under {NEW_NAME}/"
    );
    text.lines().map(str::to_string).collect()
}

/// Every regular file under a tracked root, recursively.
///
/// Symlinks are not followed: `.config` and `.claude` carry links into
/// machine-local trees, and following one would read files this repository
/// does not track.
fn files_under(path: &Path, found: &mut Vec<PathBuf>) {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_symlink() {
        return;
    }
    if metadata.is_file() {
        found.push(path.to_path_buf());
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        files_under(&entry.path(), found);
    }
}

/// The tracked files that still hold `needle`, as paths relative to the root.
///
/// Two exclusions, both necessary rather than convenient:
///
/// - `TODO-AGENTS.md` quotes history verbatim: an item can be a pasted error
///   message naming the old directory. Rewriting a quote falsifies it, and
///   the file holds no live path this repo resolves.
/// - `__pycache__` holds compiled bytecode that embeds whatever string the
///   `.py` source said when it was last imported. It is regenerated from the
///   source this sweep already reads, and rewriting a `.pyc` is not a thing a
///   rename does.
///
/// The shell suite carried a third exclusion, for itself. This file needs
/// none: its needles are assembled at run time.
fn tracked_files_naming(needle: &str) -> Vec<String> {
    let root = repo_root();
    let mut candidates: Vec<PathBuf> = Vec::new();
    for search_root in SEARCH_ROOTS {
        files_under(&root.join(search_root), &mut candidates);
    }
    let mut naming: Vec<String> = candidates
        .iter()
        .filter_map(|path| {
            let relative = path.strip_prefix(&root).ok()?.to_str()?;
            if relative == "TODO-AGENTS.md" || relative.contains("/__pycache__/") {
                return None;
            }
            let text = std::fs::read_to_string(path).ok()?;
            text.contains(needle).then(|| relative.to_string())
        })
        .collect();
    naming.sort();
    naming
}

// --- the directory itself carries the new name ----------------------------

/// `.scripts` exists and the old directory is gone.
#[test]
fn the_scripts_directory_carries_the_new_name_only() {
    let root = repo_root();
    assert!(
        root.join(NEW_NAME).is_dir(),
        "{NEW_NAME} is not a directory under {}",
        root.display()
    );
    let old = root.join(old_name());
    assert!(
        !old.exists(),
        "the old scripts directory still exists at {}",
        old.display()
    );
}

// --- the move preserved each script's mode --------------------------------

/// Every listed script is present in `.scripts`, executed and sourced alike.
///
/// `git mv` preserves a mode; a rename done by copy-and-delete would not.
/// This is the set comparison, so it carries a positive control: an empty
/// script list would report nothing missing and pass while asserting nothing.
#[test]
fn every_script_moved_to_the_new_directory() {
    let root = repo_root();
    let scripts_dir = root.join(NEW_NAME);
    assert!(
        !EXECUTED_SCRIPTS.is_empty() && !SOURCED_SCRIPTS.is_empty(),
        "positive control: the script lists are empty, so nothing was checked"
    );
    let missing: Vec<&str> = EXECUTED_SCRIPTS
        .iter()
        .chain(SOURCED_SCRIPTS.iter())
        .copied()
        .filter(|script| !scripts_dir.join(script).is_file())
        .collect();
    assert!(
        missing.is_empty(),
        "these scripts are absent from {NEW_NAME}/: {missing:?}"
    );
}

/// Every executed script still carries the execute bit on disk.
#[test]
fn every_executed_script_is_still_executable() {
    let scripts_dir = repo_root().join(NEW_NAME);
    let not_executable: Vec<&str> = EXECUTED_SCRIPTS
        .iter()
        .copied()
        .filter(|script| {
            let path = scripts_dir.join(script);
            path.is_file() && !is_executable(&path)
        })
        .collect();
    assert!(
        not_executable.is_empty(),
        "these executed scripts are not executable: {not_executable:?}"
    );
}

/// No sourced script carries the execute bit.
///
/// The inverse of the assertion above, and a real invariant rather than
/// symmetry for its own sake. Without it, marking every script executable
/// would satisfy the check above while breaking nothing loudly and quietly
/// advertising a broken way to invoke the sourced ones.
#[test]
fn no_sourced_script_is_marked_executable() {
    let scripts_dir = repo_root().join(NEW_NAME);
    let wrongly_executable: Vec<&str> = SOURCED_SCRIPTS
        .iter()
        .copied()
        .filter(|script| {
            let path = scripts_dir.join(script);
            path.is_file() && is_executable(&path)
        })
        .collect();
    assert!(
        wrongly_executable.is_empty(),
        "these sourced scripts are marked executable: {wrongly_executable:?}"
    );
}

// --- no tracked file still names the old directory ------------------------

/// The sweep's own search roots resolve to files in this checkout.
///
/// The shell suite's "at least one search root is present" assertion, which
/// was already a positive control. Kept as its own test rather than folded
/// into the two sweeps below, because a broken root list is a different
/// failure from a stale reference and sends a reader somewhere else.
#[test]
fn the_stale_name_sweep_reads_something() {
    let root = repo_root();
    let mut candidates: Vec<PathBuf> = Vec::new();
    for search_root in SEARCH_ROOTS {
        files_under(&root.join(search_root), &mut candidates);
    }
    assert!(
        !candidates.is_empty(),
        "positive control: no search root under {} holds a file",
        root.display()
    );
}

/// No tracked file still names the old dotted directory.
#[test]
fn no_tracked_file_still_names_the_old_directory() {
    let naming = tracked_files_naming(&old_name());
    assert!(
        naming.is_empty(),
        "these tracked files still name the old scripts directory: {naming:?}"
    );
}

/// No tracked file still names the bare old stem.
///
/// Searched separately from the dotted form. The first sweep of this rename
/// matched only the dotted form and left two references behind: a test
/// description, and worse, an assertion whose needle was the bare stem. Once
/// the path in its input became `.scripts`, that assertion passed no matter
/// what the code did. A stale word in prose is cosmetic; a stale word in an
/// assertion is a test that stopped testing.
#[test]
fn no_tracked_file_still_names_the_old_stem() {
    let naming = tracked_files_naming(&old_stem());
    assert!(
        naming.is_empty(),
        "these tracked files still name the old scripts stem: {naming:?}"
    );
}

// --- the scripts exist in the commit, not only on disk --------------------

/// The commit under `.scripts` holds exactly the expected number of files.
///
/// Every on-disk assertion above is exactly the hole the rename fell through,
/// so this asks HEAD instead of the filesystem.
///
/// Skipped where there is no repository, which is the container: its tree
/// comes from `git archive`. That skip is recorded and counted rather than
/// silent, and this block is the origin of that rule. It used to print one
/// line into the suite's captured output and nothing else, so `finish`
/// counted only passes and failures and the pre-push Docker gate printed PASS
/// over an assertion that never ran. A stale count shipped and CI caught it,
/// not this suite. A gate that says nothing when it skips is
/// indistinguishable from a gate that is not installed.
#[test]
fn every_script_is_committed_not_only_on_disk() {
    let Some(git_dir) = git_dir() else {
        dotfiles_test_support::skip(
            "no repository here, so the committed scripts cannot be inspected",
        );
        return;
    };
    let committed = committed_scripts(&git_dir);
    assert_eq!(
        committed.len(),
        COMMITTED_SCRIPT_COUNT,
        "the commit holds {} files under {NEW_NAME}/, expected {COMMITTED_SCRIPT_COUNT}",
        committed.len()
    );
}

/// The committed execute bits under `.scripts` are exactly the expected set.
///
/// The execute bits have to survive the commit too. A script committed 100644
/// fails at runtime on a fresh clone while working on the machine that has
/// the bit set locally, which is the worst shape for this bug.
///
/// Scoped to `.scripts`, which is the point of the rooted pathspec: an entry
/// naming a path outside it would compare an empty left side against an empty
/// right side and pass while asserting nothing. That is how the seven `deps`
/// scripts silently left this suite's coverage when `deps/` moved to the top
/// level.
#[test]
fn the_committed_execute_bits_match_the_executed_scripts() {
    let Some(git_dir) = git_dir() else {
        dotfiles_test_support::skip(
            "no repository here, so the committed execute bits cannot be inspected",
        );
        return;
    };
    let prefix = format!("{NEW_NAME}/");
    let mut committed: Vec<String> = committed_scripts(&git_dir)
        .iter()
        .filter(|line| line.starts_with("100755"))
        .filter_map(|line| line.split('\t').nth(1))
        .filter_map(|path| path.strip_prefix(&prefix))
        .map(str::to_string)
        .collect();
    committed.sort();

    let mut expected: Vec<String> = COMMITTED_EXECUTABLE
        .iter()
        .map(|script| (*script).to_string())
        .collect();
    expected.sort();

    assert_eq!(
        committed, expected,
        "the committed execute bits under {NEW_NAME}/ do not match the expected set"
    );
}

/// No path under the old directory survives in the commit.
#[test]
fn no_old_directory_path_survives_in_the_commit() {
    let Some(git_dir) = git_dir() else {
        dotfiles_test_support::skip(
            "no repository here, so the commit cannot be searched for the old directory",
        );
        return;
    };
    let root = repo_root();
    let listing = Command::new("git")
        .arg(format!("--git-dir={}", git_dir.display()))
        .arg(format!("--work-tree={}", root.display()))
        .args(["ls-tree", "-r", "--name-only", "--full-name", "HEAD"])
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .expect("git ls-tree runs");
    assert!(
        listing.status.success(),
        "git ls-tree failed: {}",
        String::from_utf8_lossy(&listing.stderr)
    );
    let text = String::from_utf8_lossy(&listing.stdout);
    assert!(
        !text.trim().is_empty(),
        "positive control: git ls-tree listed no tracked files at all"
    );
    let old_prefix = format!("{}/", old_name());
    let surviving: Vec<&str> = text
        .lines()
        .filter(|path| path.starts_with(&old_prefix))
        .collect();
    assert!(
        surviving.is_empty(),
        "these paths under the old scripts directory survive in the commit: {surviving:?}"
    );
}

// --- the three anchored regexes point at the new directory ----------------

/// The pre-push trigger filter matches `.scripts`.
///
/// These are the references a plain text sweep gets wrong, because each is an
/// escaped regex rather than a bare path. Asserted individually so a failure
/// names which gate stopped matching, instead of only reporting that some
/// file somewhere still holds the old string.
#[test]
fn the_pre_push_trigger_filter_matches_the_new_directory() {
    let hook = repo_root().join("tests/pre-push");
    let text = std::fs::read_to_string(&hook)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", hook.display()));
    assert!(
        text.contains(r"^\.scripts/.*\.sh$"),
        "tests/pre-push carries no anchored {NEW_NAME} trigger pattern"
    );
}

/// The leak-check allow list matches `.scripts`.
///
/// `leak-allow.conf` is machine-local and untracked, so this stands down
/// where it is absent. The skip is recorded and counted: a machine without
/// the file has no coverage here, and that is worth reporting rather than
/// hiding.
#[test]
fn the_leak_check_allow_list_matches_the_new_directory() {
    let allow_list = repo_root().join(".claude/local/leak-allow.conf");
    let Ok(text) = std::fs::read_to_string(&allow_list) else {
        dotfiles_test_support::skip("leak-allow.conf is machine-local and absent here");
        return;
    };
    assert!(
        text.contains(r"^\.scripts/tmux-.*\.sh$"),
        "leak-allow.conf carries no anchored {NEW_NAME} allow pattern"
    );
}

/// The CI path filter watches the `deps` tree.
#[test]
fn the_ci_path_filter_watches_the_deps_tree() {
    let workflow = repo_root().join(".github/workflows/deps-check.yml");
    let text = std::fs::read_to_string(&workflow)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", workflow.display()));
    assert!(
        text.contains("'deps/**'"),
        "deps-check.yml carries no deps path filter"
    );
}
