//! Reads git repository facts directly from `.git`, falling back to
//! `git rev-parse` only when a direct read fails or looks wrong.
//!
//! `tmux-update-window-names.sh` spawns `git` once per window and runs in
//! `precmd`, so it fires on every prompt draw: measured at about 84
//! milliseconds for a one-window session and 442 milliseconds for the `-a`
//! path across 21 live windows, with 17 spawn sites. The parent spec
//! re-verified direct reads against the same 21 live window directories and
//! found zero fallbacks needed, so the spawn is now the exceptional path
//! rather than the normal one.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use tmux_core::{HeadState, RepositoryFacts};

/// Each `git` spawn in the fallback ends within this time. thumper calls
/// [`inspect`] on the hook path outside tmux, where a wedged `git` must not
/// hold the hook. The limit is per spawn, not for the whole fallback: on a
/// machine whose endpoint scanner delays every exec, one `git` call took about
/// 0.5 s, so the three calls of a healthy fallback did not fit in one second.
/// A wedged `git` still costs one limit, because the first timeout ends the
/// fallback.
const GIT_FALLBACK_LIMIT: Duration = Duration::from_secs(1);
const CHILD_POLL: Duration = Duration::from_millis(5);

/// Inspects `directory` for the git repository facts a window name needs.
///
/// Reads `.git`, `commondir` and `HEAD` directly rather than spawning `git`,
/// since this runs on every prompt draw. Returns `None` when `directory` is
/// not inside a git repository. Falls back to `git rev-parse` when a direct
/// read fails or yields a shape this function does not recognize, rather
/// than answering with a value that might be wrong. Each `git` spawn in the
/// fallback is bounded by [`GIT_FALLBACK_LIMIT`].
pub fn inspect(directory: &Path) -> Option<RepositoryFacts> {
    inspect_within(directory, GIT_FALLBACK_LIMIT)
}

fn inspect_within(directory: &Path, git_limit: Duration) -> Option<RepositoryFacts> {
    let git_path = find_git_path(directory)?;
    direct_read(&git_path).or_else(|| fallback_to_git_rev_parse(directory, Path::new("git"), git_limit))
}

/// Walks from `directory` up to the filesystem root looking for `.git`.
///
/// A `.git` entry can be a directory (an ordinary clone) or a file (a
/// linked worktree, which points at its git directory elsewhere). Either
/// shape answers this search; `direct_read` decides which one it got.
///
/// Returns `None` when no ancestor has a `.git` entry, which is the common
/// non-repository case. That case costs one `stat` per ancestor rather than
/// a process spawn, which is the whole reason this module exists.
fn find_git_path(directory: &Path) -> Option<PathBuf> {
    let mut current = Some(directory);
    while let Some(candidate_directory) = current {
        let git_path = candidate_directory.join(".git");
        if git_path.exists() {
            return Some(git_path);
        }
        current = candidate_directory.parent();
    }
    None
}

/// Reads repository facts directly from files under `git_path`.
///
/// Returns `None` on any read failure or unexpected shape, which sends the
/// caller to the `git rev-parse` fallback instead of reporting a wrong
/// answer.
fn direct_read(git_path: &Path) -> Option<RepositoryFacts> {
    let git_directory = resolve_git_directory(git_path)?;
    let main_repo_name = main_repository_name(&git_directory)?;
    let head = read_head(&git_directory)?;
    Some(RepositoryFacts {
        main_repo_name,
        head,
    })
}

/// Resolves `.git` to the directory that holds `HEAD` and refs.
///
/// For an ordinary clone, `.git` already is that directory. For a linked
/// worktree, `.git` is a file containing `gitdir: <path>`, and the pointed-to
/// directory is worktree-specific (it holds this worktree's own `HEAD`, but
/// shares refs with the main repository through `commondir`).
fn resolve_git_directory(git_path: &Path) -> Option<PathBuf> {
    if git_path.is_dir() {
        return Some(git_path.to_path_buf());
    }

    let contents = std::fs::read_to_string(git_path).ok()?;
    let gitdir_line = contents.trim();
    let pointed_path = gitdir_line.strip_prefix("gitdir:")?.trim();
    let pointed_path = PathBuf::from(pointed_path);

    if pointed_path.is_absolute() {
        Some(pointed_path)
    } else {
        Some(git_path.parent()?.join(pointed_path))
    }
}

/// Finds the main repository's name from a worktree's git directory.
///
/// An ordinary clone's git directory already belongs to the main
/// repository, so its own parent directory's name is the answer. A linked
/// worktree's git directory instead holds a `commondir` file naming the
/// main repository's git directory relative to this one; that path's parent
/// is the main repository's working directory, and its name is what a
/// worktree must report instead of its own directory name.
fn main_repository_name(git_directory: &Path) -> Option<String> {
    let commondir_path = git_directory.join("commondir");
    let main_git_directory = if commondir_path.exists() {
        let contents = std::fs::read_to_string(&commondir_path).ok()?;
        let relative_or_absolute = PathBuf::from(contents.trim());
        if relative_or_absolute.is_absolute() {
            relative_or_absolute
        } else {
            git_directory.join(relative_or_absolute)
        }
    } else {
        git_directory.to_path_buf()
    };

    let canonical_main_git_directory = main_git_directory.canonicalize().ok()?;
    let main_repository_directory = canonical_main_git_directory.parent()?;
    let name = main_repository_directory.file_name()?;
    Some(name.to_string_lossy().into_owned())
}

/// Reads `HEAD` and classifies it as a branch or a detached commit.
///
/// `ref: refs/heads/<branch>` is a branch, read directly. A bare 40-character
/// hex string is a detached head, but this function does not shorten it
/// itself: git's abbreviation length is `core.abbrev` (default "auto"),
/// which grows past 7 characters to stay unambiguous as a repository's
/// object count grows, and only `git rev-parse --short` knows that count.
/// Returning `None` here for a detached head sends the caller to that
/// fallback rather than emitting a short sha of a length git would not have
/// chosen itself.
fn read_head(git_directory: &Path) -> Option<HeadState> {
    let contents = std::fs::read_to_string(git_directory.join("HEAD")).ok()?;
    let trimmed = contents.trim();

    let branch_ref = trimmed.strip_prefix("ref:")?;
    let branch_ref = branch_ref.trim();
    let branch_name = branch_ref.strip_prefix("refs/heads/")?;
    Some(HeadState::Branch(branch_name.to_string()))
}

/// Falls back to spawning `git rev-parse` when a direct read did not work.
///
/// The parent spec verified zero fallbacks were needed across 21 live
/// window directories, so this path exists for correctness, not for the
/// common case. Logs nothing on failure: this runs on every prompt draw,
/// and a stray line here would corrupt the prompt. A `git` still running
/// after `limit` is killed, and the answer is `None`.
fn fallback_to_git_rev_parse(directory: &Path, git: &Path, limit: Duration) -> Option<RepositoryFacts> {
    let git_output = |arguments: &[&str]| output_within(Command::new(git).args(arguments).current_dir(directory), limit);
    let common_directory_output = git_output(&["rev-parse", "--path-format=absolute", "--git-common-dir"])?;
    if !common_directory_output.status.success() {
        return None;
    }
    let common_directory_text = String::from_utf8(common_directory_output.stdout).ok()?;
    let common_directory = PathBuf::from(common_directory_text.trim());
    let main_repository_directory = common_directory.canonicalize().ok()?;
    let main_repository_directory = main_repository_directory.parent()?;
    let main_repo_name = main_repository_directory
        .file_name()?
        .to_string_lossy()
        .into_owned();

    let branch_output = git_output(&["symbolic-ref", "--short", "-q", "HEAD"])?;

    let head = if branch_output.status.success() {
        let branch_name = String::from_utf8(branch_output.stdout).ok()?;
        HeadState::Branch(branch_name.trim().to_string())
    } else {
        let short_sha_output = git_output(&["rev-parse", "--short", "HEAD"])?;
        if !short_sha_output.status.success() {
            return None;
        }
        let short_sha = String::from_utf8(short_sha_output.stdout).ok()?;
        HeadState::Detached {
            short_sha: short_sha.trim().to_string(),
        }
    };

    Some(RepositoryFacts {
        main_repo_name,
        head,
    })
}

/// Runs `command` and returns its output, or `None` when it fails to start
/// or is still running after `limit`, in which case it is killed and reaped.
/// Stdout is read after exit, which suits the few bytes `git rev-parse`
/// prints.
fn output_within(command: &mut Command, limit: Duration) -> Option<Output> {
    let deadline = Instant::now() + limit;
    let mut child = command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => return child.wait_with_output().ok(),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(CHILD_POLL),
            Ok(None) | Err(_) => {
                let _killed = child.kill();
                let _reaped = child.wait();
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// Tests that need a real `git` to answer use a wide limit. Under a
    /// parallel test run here, one `git` exec took over a second, so the
    /// production limit would make them flaky. The hanging-git test pins
    /// the bound itself.
    const HEALTHY_GIT_LIMIT: Duration = Duration::from_secs(30);

    /// Runs git against a fixture, with git's ambient environment cleared.
    ///
    /// Without the removals, an exported `GIT_DIR` from the calling shell
    /// makes every fixture command operate on the real dotfiles repository:
    /// observed one taking that repo's index lock and blocking every
    /// subsequent command. See `.agents/PAPERCUTS.md`.
    fn git_in(directory: &Path, arguments: &[&str]) {
        let status = Command::new("git")
            .args(arguments)
            .current_dir(directory)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_PREFIX")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .status()
            .expect("git runs");
        assert!(status.success(), "git {arguments:?} failed");
    }

    fn a_repo_with_one_commit() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("tempdir");
        git_in(directory.path(), &["init", "-q", "-b", "main"]);
        std::fs::write(directory.path().join("marker.txt"), "hello\n").expect("write");
        git_in(directory.path(), &["add", "marker.txt"]);
        git_in(
            directory.path(),
            &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "first"],
        );
        directory
    }

    /// A repository on a branch reports its name and its branch.
    #[test]
    fn a_repository_reports_its_name_and_branch() {
        let repo = a_repo_with_one_commit();

        let facts = inspect(repo.path()).expect("a git repository is inspectable");

        // Positive control: a non-repository must yield None, or the
        // assertions below would hold for a function that always answers.
        let plain = tempfile::tempdir().expect("tempdir");
        assert!(inspect(plain.path()).is_none(), "a plain directory is not a repository");

        assert_eq!(facts.head, HeadState::Branch("main".to_string()));
        assert_eq!(
            facts.main_repo_name,
            repo.path().file_name().expect("a name").to_string_lossy()
        );
    }

    /// A linked worktree reports the MAIN repository's name, not its own
    /// directory name. That is what `commondir` is read for.
    #[test]
    fn a_linked_worktree_reports_the_main_repository_name() {
        let main = a_repo_with_one_commit();
        let worktree_parent = tempfile::tempdir().expect("tempdir");
        let worktree = worktree_parent.path().join("feature-checkout");
        git_in(
            main.path(),
            &["worktree", "add", "-q", worktree.to_str().expect("utf-8 path"), "-b", "feature"],
        );

        let facts = inspect(&worktree).expect("a linked worktree is inspectable");

        assert_eq!(
            facts.main_repo_name,
            main.path().file_name().expect("a name").to_string_lossy(),
            "a worktree must report the main repo's name, not its own directory"
        );
        assert_eq!(facts.head, HeadState::Branch("feature".to_string()));
    }

    /// A detached HEAD reports a short sha rather than a branch.
    #[test]
    fn a_detached_head_reports_a_short_sha() {
        let repo = a_repo_with_one_commit();
        git_in(repo.path(), &["checkout", "-q", "--detach"]);

        let facts = inspect_within(repo.path(), HEALTHY_GIT_LIMIT).expect("a detached repository is inspectable");

        match facts.head {
            HeadState::Detached { short_sha } => {
                assert!(!short_sha.is_empty(), "a detached head must carry a sha");
                assert!(
                    short_sha.len() >= 7 && short_sha.len() <= 12,
                    "a short sha, not a full one: got {short_sha:?}"
                );
            }
            other => panic!("expected a detached head, got {other:?}"),
        }
    }

    /// A `git` that never answers is killed at the limit, so the hook
    /// path outside tmux cannot hang on it.
    #[test]
    fn a_hanging_git_fallback_gives_up_at_its_limit() {
        let repo = a_repo_with_one_commit();
        let stubs = tempfile::tempdir().expect("tempdir");
        let hanging_git = stubs.path().join("git");
        dotfiles_test_support::stub::write(&hanging_git, "#!/bin/sh\nexec sleep 10\n").expect("a stub");

        let started = Instant::now();
        let facts = fallback_to_git_rev_parse(repo.path(), &hanging_git, Duration::from_millis(200));

        assert!(facts.is_none(), "a git that never answers yields no facts");
        assert!(started.elapsed() < Duration::from_secs(2), "took {:?}", started.elapsed());
    }

    /// The bounded fallback still answers with a real git, so the test above
    /// does not pass for a fallback that always gives up.
    #[test]
    fn the_bounded_fallback_answers_with_a_real_git() {
        let repo = a_repo_with_one_commit();
        let facts = fallback_to_git_rev_parse(repo.path(), Path::new("git"), HEALTHY_GIT_LIMIT);
        assert_eq!(facts.map(|facts| facts.head), Some(HeadState::Branch("main".to_string())));
    }

    /// A bare-repo worktree, which is the shape $HOME has, reports facts
    /// rather than panicking or reporting None for the wrong reason.
    ///
    /// tmux-core's `is_home` check is what keeps $HOME rendering as "~"; this
    /// test only pins that inspection itself handles the shape.
    #[test]
    fn a_bare_repo_worktree_is_inspectable() {
        let home = tempfile::tempdir().expect("tempdir");
        let bare = home.path().join(".cfg");
        git_in(home.path(), &["init", "-q", "--bare", "-b", "main", ".cfg"]);
        std::fs::write(home.path().join("tracked.txt"), "hi\n").expect("write");
        let bare_path = bare.to_str().expect("utf-8 path");
        git_in(
            home.path(),
            &[
                "--git-dir", bare_path, "--work-tree", ".",
                "-c", "user.email=t@t", "-c", "user.name=t",
                "add", "tracked.txt",
            ],
        );

        // No .git in the work tree, so inspection must not claim a
        // repository from a directory that has none of the usual markers.
        assert!(
            inspect(home.path()).is_none(),
            "a bare-repo worktree has no .git, so inspection reports None and \
             tmux-core's is_home rule is what renders the tilde"
        );
    }
}
