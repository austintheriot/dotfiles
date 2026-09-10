//! A branch change renames tmux windows on the event, not on the next prompt.
//!
//! THE LAG THIS CLOSES. Nothing in tmux watches git, so the only trigger was
//! zsh's precmd: a checkout in pane A renamed the window when pane A next
//! drew a prompt, and a checkout made anywhere else (another pane, an editor,
//! a script) never renamed the window showing that repo. A git post-checkout
//! hook fires on the event itself.
//!
//! The hook is a tracked file, `.scripts/git-hooks/post-checkout`, installed
//! per repository by `config install-repo-hooks [dir]` into the repository's
//! COMMON git directory, so one install covers every worktree of that repo.
//! Not a global `core.hooksPath`: that would replace `~/.cfg`'s own hooks,
//! which `githooks_installed.rs` asserts against.
//!
//! `tmux-tools` is STUBBED on PATH here and records its argv. The hook's whole
//! contract is "invoke the renamer with `-a`"; whether the renamer then names
//! windows correctly is the tmux suites' question, and it would need a tmux
//! server this suite has no business starting.
//!
//! Converted whole from `tests/git-post-checkout-hook.test.sh`, which ran
//! **11** assertions, measured by running it.
//!
//! One assertion changed shape deliberately. The shell suite spelled the
//! POSIX-sh check as `dash -n "$HOOK"`, a PARSE check, and
//! `.claude/rules/dotfiles-tests.md` records that a parse check is not an
//! execution check: `dash -n` accepts `[[ ]]` because dash parses `[[` as a
//! command word. Here the hook is EXECUTED under `dash` with a stub renamer,
//! which catches a bashism that only fails at run time.

use assert_cmd::Command as AssertCommand;
use dotfiles_test_support::repo::root as repo_root;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// The installer under test.
fn installer() -> PathBuf {
    repo_root().join(".scripts/config/config-install-repo-hooks")
}

/// The tracked hook the installer links.
fn hook() -> PathBuf {
    repo_root().join(".scripts/git-hooks/post-checkout")
}

/// Whether a path is executable by its owner.
fn is_executable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|data| data.permissions().mode() & 0o111 != 0)
}

/// A directory holding a stub `tmux-tools` that appends its argv to a file.
///
/// The hook BACKGROUNDS the real call, so the stub writes synchronously and
/// the caller polls for the record. Driven by attempt count rather than by
/// wall clock: a fixed-duration loop completes a different number of
/// iterations under parallel test execution than it does alone, which has
/// already produced a test that ran one iteration where it meant sixty.
struct StubRenamer {
    directory: TempDir,
    record: PathBuf,
}

impl StubRenamer {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let record = directory.path().join("tmux-tools.argv");
        let stub = directory.path().join("tmux-tools");
        fs::write(
            &stub,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> {}\n",
                shell_quote(&record)
            ),
        )
        .expect("the stub is writable");
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755))
            .expect("the stub is executable");
        Self { directory, record }
    }

    /// `PATH` with the stub directory first, for a child process only.
    fn path_with_stub(&self) -> String {
        let inherited = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_string());
        format!("{}:{inherited}", self.directory.path().display())
    }

    /// Forgets any recorded invocation, so the next read cannot see a stale one.
    fn clear(&self) {
        let _ = fs::remove_file(&self.record);
    }

    /// The first recorded argv line, waiting for the backgrounded call.
    ///
    /// Returns `None` when nothing was recorded after every attempt, which is
    /// what a hook that never invokes the renamer produces.
    fn first_invocation(&self) -> Option<String> {
        for _ in 0..100 {
            if let Ok(text) = fs::read_to_string(&self.record)
                && let Some(line) = text.lines().next()
            {
                return Some(line.to_string());
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        None
    }
}

/// A path as a single-quoted shell word.
fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', r"'\''"))
}

/// A fixture repository with one commit, on `main`.
fn make_repo(parent: &Path, name: &str) -> PathBuf {
    let path = parent.join(name);
    fs::create_dir_all(&path).expect("the fixture directory is creatable");
    git(&path, &["init", "-q", "-b", "main"]);
    git(
        &path,
        &[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "init",
        ],
    );
    path
}

/// Runs git in a directory, with any inherited git environment removed.
///
/// A hook or a pre-commit environment exports `GIT_DIR`, and with it set every
/// call here would answer for THAT repository rather than the fixture.
fn git(directory: &Path, arguments: &[&str]) -> std::process::Output {
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
    output
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

/// The tracked hook and installer are present and runnable.
#[test]
fn the_hook_and_the_installer_are_executable_files() {
    assert!(
        is_executable(&hook()),
        "{} is missing or not executable, so git would skip it silently",
        hook().display()
    );
    assert!(
        is_executable(&installer()),
        "{} is missing or not executable",
        installer().display()
    );
}

/// The hook runs under `dash`, so a bashism cannot ship in it.
///
/// EXECUTED, not parsed. `dash -n` accepts `[[ ]]` because dash parses `[[`
/// as a command word, so the shell suite's parse check could not detect the
/// bashism it existed to catch.
#[test]
fn the_hook_executes_under_dash() {
    let stub = StubRenamer::new();
    let output = Command::new("dash")
        .arg(hook())
        .args(["oldhead", "newhead", "1"])
        .env("PATH", stub.path_with_stub())
        .output();
    let Ok(output) = output else {
        // dash absent is not a failure of the hook. The container carries it;
        // a developer machine may not.
        dotfiles_test_support::skip("dash is not installed, so the POSIX-sh check cannot run");
        return;
    };
    assert!(
        output.status.success(),
        "the hook failed under dash (exit {:?}): {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "the hook wrote to stderr under dash, which a bashism does: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Installing links the tracked hook into the common git directory, and a
/// checkout then invokes the renamer for every window.
#[test]
fn installing_links_the_hook_and_a_checkout_invokes_the_renamer() {
    let fixtures = tempfile::tempdir().expect("a temporary directory");
    let repository = make_repo(fixtures.path(), "hooked");
    let stub = StubRenamer::new();

    AssertCommand::new(installer())
        .arg(&repository)
        .assert()
        .success();

    let link = repository.join(".git/hooks/post-checkout");
    assert!(
        link.symlink_metadata()
            .is_ok_and(|data| data.file_type().is_symlink()),
        "{} is not a symlink, so the tracked hook is not what git runs",
        link.display()
    );
    assert_eq!(
        fs::read_link(&link).expect("the link resolves"),
        hook(),
        "the installed hook does not point at the tracked one, so editing \
         the tracked file would change nothing"
    );

    let checkout = git_command(&repository)
        .args(["checkout", "-q", "-b", "feature"])
        .env("PATH", stub.path_with_stub())
        .output()
        .expect("git runs");
    assert!(
        checkout.status.success(),
        "the checkout failed: {}",
        String::from_utf8_lossy(&checkout.stderr)
    );
    assert_eq!(
        stub.first_invocation().as_deref(),
        Some("name-windows -a"),
        "a branch checkout did not invoke the renamer for every window"
    );
}

/// Installing twice leaves one link, not a copy and not an error.
#[test]
fn installing_again_is_idempotent() {
    let fixtures = tempfile::tempdir().expect("a temporary directory");
    let repository = make_repo(fixtures.path(), "twice");

    AssertCommand::new(installer())
        .arg(&repository)
        .assert()
        .success();
    AssertCommand::new(installer())
        .arg(&repository)
        .assert()
        .success();

    let link = repository.join(".git/hooks/post-checkout");
    assert!(
        link.symlink_metadata()
            .is_ok_and(|data| data.file_type().is_symlink()),
        "installing twice replaced the link with a copy at {}",
        link.display()
    );
    assert_eq!(
        fs::read_link(&link).expect("the link resolves"),
        hook(),
        "installing twice left the link pointing somewhere else"
    );
}

/// A worktree shares the install, because the link goes into the COMMON git
/// directory rather than the worktree's own.
#[test]
fn a_checkout_in_a_worktree_fires_the_same_hook() {
    let fixtures = tempfile::tempdir().expect("a temporary directory");
    let repository = make_repo(fixtures.path(), "shared");
    let stub = StubRenamer::new();

    AssertCommand::new(installer())
        .arg(&repository)
        .assert()
        .success();

    let worktree = fixtures.path().join("wt");
    git(
        &repository,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature-two",
            &worktree.display().to_string(),
        ],
    );

    stub.clear();
    let checkout = git_command(&worktree)
        .args(["checkout", "-q", "-b", "feature-three"])
        .env("PATH", stub.path_with_stub())
        .output()
        .expect("git runs");
    assert!(
        checkout.status.success(),
        "the worktree checkout failed: {}",
        String::from_utf8_lossy(&checkout.stderr)
    );
    assert_eq!(
        stub.first_invocation().as_deref(),
        Some("name-windows -a"),
        "a checkout in a worktree did not fire the hook, so the install went \
         into the worktree's private git directory rather than the common one"
    );
}

/// A directory that is not a repository is refused with the usage code.
#[test]
fn a_directory_that_is_not_a_repository_is_refused_with_exit_two() {
    let fixtures = tempfile::tempdir().expect("a temporary directory");
    let plain = fixtures.path().join("plain-dir");
    fs::create_dir_all(&plain).expect("the fixture directory is creatable");

    AssertCommand::new(installer())
        .arg(&plain)
        .assert()
        .code(2);
}
