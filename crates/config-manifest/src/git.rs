use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};

use crate::path::{CommitId, RelPath};
use crate::plan::{SyncPlan, TreeEdit};
use crate::tree::{TreeListing, parse_ls_tree};

/// Both shapes this repo comes in: the real dotfiles repo is bare at
/// `<root>/.cfg` with `<root>` as the worktree, and every test fixture is a
/// normal repository with `.git` inside `<root>`.
#[derive(Debug, Clone)]
pub struct Git {
    prefix: Vec<String>,
    env: Vec<(String, String)>,
}

/// Removes the temp index on every exit path, so a failed apply leaves no
/// file behind and a later apply never inherits stale index state.
struct TempIndex {
    path: PathBuf,
}

impl TempIndex {
    fn create() -> anyhow::Result<TempIndex> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "config-manifest-index-{}-{nanos}",
            std::process::id()
        ));
        if path.exists() {
            bail!("temp index {} already exists", path.display());
        }
        Ok(TempIndex { path })
    }
}

impl Drop for TempIndex {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Git {
    pub fn discover(root: &Path) -> Git {
        let cfg: PathBuf = root.join(".cfg");
        let prefix = if cfg.is_dir() {
            vec![
                format!("--git-dir={}", cfg.display()),
                format!("--work-tree={}", root.display()),
            ]
        } else {
            vec!["-C".to_string(), root.display().to_string()]
        };
        Git {
            prefix,
            env: Vec::new(),
        }
    }

    pub fn with_env(&self, key: &str, value: &str) -> Git {
        let mut env = self.env.clone();
        env.push((key.to_string(), value.to_string()));
        Git {
            prefix: self.prefix.clone(),
            env,
        }
    }

    fn command(&self, args: &[&str], index: Option<&Path>) -> Command {
        let mut command = Command::new("git");
        command.args(&self.prefix).args(args);
        for (key, value) in &self.env {
            command.env(key, value);
        }
        if let Some(index) = index {
            command.env("GIT_INDEX_FILE", index);
        }
        command
    }

    fn output(&self, args: &[&str]) -> anyhow::Result<std::process::Output> {
        self.command(args, None)
            .output()
            .with_context(|| format!("failed to spawn git {}", args.join(" ")))
    }

    fn run_checked(&self, args: &[&str], index: Option<&Path>) -> anyhow::Result<String> {
        let output = self
            .command(args, index)
            .output()
            .with_context(|| format!("failed to spawn git {}", args.join(" ")))?;
        if !output.status.success() {
            bail!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        String::from_utf8(output.stdout)
            .with_context(|| format!("git {} output is not UTF-8", args.join(" ")))
    }

    pub(crate) fn output_text(&self, args: &[&str]) -> anyhow::Result<String> {
        self.run_checked(args, None)
    }

    pub fn show_manifest(&self, rev: &str) -> anyhow::Result<Option<String>> {
        let spec = format!("{rev}:.sync-manifest");
        let output = self.output(&["show", &spec])?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(Some(
            String::from_utf8(output.stdout).context(".sync-manifest is not UTF-8")?,
        ))
    }

    pub fn ls_tree(&self, rev: &str) -> anyhow::Result<TreeListing> {
        let output = self.output(&["ls-tree", "-r", "-z", rev])?;
        if !output.status.success() {
            bail!(
                "git ls-tree {rev} failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        parse_ls_tree(&output.stdout).with_context(|| format!("parsing ls-tree output for {rev}"))
    }

    pub fn rev_parse(&self, rev: &str) -> anyhow::Result<CommitId> {
        let spec = format!("{rev}^{{commit}}");
        let text = self.output_text(&["rev-parse", "--verify", "--quiet", &spec])?;
        CommitId::parse(text.trim()).with_context(|| format!("rev-parse {rev} returned a non-id"))
    }

    pub fn current_branch(&self) -> anyhow::Result<Option<String>> {
        let output = self.output(&["symbolic-ref", "--quiet", "--short", "HEAD"])?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(Some(
            String::from_utf8(output.stdout)
                .context("branch name is not UTF-8")?
                .trim()
                .to_string(),
        ))
    }

    pub fn has_remote(&self, remote: &str) -> anyhow::Result<bool> {
        Ok(self
            .output(&["remote", "get-url", remote])?
            .status
            .success())
    }

    pub fn fetch(&self, remote: &str) -> anyhow::Result<()> {
        self.output_text(&["fetch", "--quiet", remote]).map(|_| ())
    }

    pub fn dirty_paths(&self) -> anyhow::Result<Vec<RelPath>> {
        let text = self.output_text(&["status", "--porcelain", "-z", "--untracked-files=no"])?;
        let mut records = text.split('\0').filter(|record| !record.is_empty());
        let mut paths = Vec::new();
        while let Some(record) = records.next() {
            let status = record.get(0..2);
            let Some(path_text) = record.get(3..) else {
                continue;
            };
            paths.push(
                RelPath::parse(path_text).with_context(|| format!("status path {path_text}"))?,
            );
            let is_rename_or_copy =
                status.is_some_and(|code| code.contains('R') || code.contains('C'));
            if is_rename_or_copy {
                records.next();
            }
        }
        Ok(paths)
    }

    /// The only writer in the crate. Everything before update-ref creates
    /// unreferenced objects only; update-ref is a compare-and-swap against
    /// the commit the plan was computed from, so a moved branch fails here
    /// and nothing observable has changed.
    pub fn commit_plan(
        &self,
        plan: &SyncPlan,
        target_branch: &str,
        message: &str,
    ) -> anyhow::Result<CommitId> {
        let index = TempIndex::create()?;
        let index_path: &Path = &index.path;
        let expected = plan.planned_against().as_str();

        self.run_checked(&["read-tree", expected], Some(index_path))?;
        for edit in plan.edits() {
            match edit {
                TreeEdit::Set(set) => {
                    let info = format!(
                        "{},{},{}",
                        set.mode().as_git_mode(),
                        set.blob().as_str(),
                        set.path().as_str()
                    );
                    self.run_checked(
                        &["update-index", "--add", "--cacheinfo", &info],
                        Some(index_path),
                    )?;
                }
                TreeEdit::Remove(remove) => {
                    self.run_checked(
                        &[
                            "update-index",
                            "--force-remove",
                            "--",
                            remove.path().as_str(),
                        ],
                        Some(index_path),
                    )?;
                }
            }
        }
        let tree = self.run_checked(&["write-tree"], Some(index_path))?;
        let commit_text = self.run_checked(
            &["commit-tree", tree.trim(), "-p", expected, "-m", message],
            None,
        )?;
        let commit =
            CommitId::parse(commit_text.trim()).context("commit-tree returned a non-id")?;
        let target_ref = format!("refs/heads/{target_branch}");
        self.run_checked(
            &["update-ref", &target_ref, commit.as_str(), expected],
            None,
        )?;
        Ok(commit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::RelPath;
    use crate::tree::FileMode;

    fn run(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.email=t@t", "-c", "user.name=t"])
            .args(args)
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?} failed");
    }

    #[test]
    fn lists_a_normal_repo_and_reads_its_manifest() {
        let dir = tempfile::tempdir().expect("tempdir");
        run(dir.path(), &["init", "-q", "-b", "main"]);
        std::fs::write(
            dir.path().join(".sync-manifest"),
            ".sync-manifest\nrun.sh\n",
        )
        .expect("write");
        std::fs::write(dir.path().join("run.sh"), "#!/bin/sh\n").expect("write");
        run(dir.path(), &["add", "."]);
        run(dir.path(), &["update-index", "--chmod=+x", "run.sh"]);
        run(dir.path(), &["commit", "-q", "-m", "init"]);

        let git = Git::discover(dir.path());
        let manifest = git
            .show_manifest("main")
            .expect("git works")
            .expect("manifest present");
        assert_eq!(manifest, ".sync-manifest\nrun.sh\n");
        let listing = git.ls_tree("main").expect("git works");
        let exec = RelPath::parse("run.sh").expect("valid");
        assert_eq!(listing.get(&exec).expect("present").0, FileMode::Executable);
        assert_eq!(git.show_manifest("no-such-ref").expect("git ran"), None);
    }

    #[test]
    fn discovers_a_bare_repo_at_root_dot_cfg() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = dir.path().join(".cfg");
        let status = Command::new("git")
            .args(["init", "-q", "--bare", "-b", "main"])
            .arg(&cfg)
            .status()
            .expect("git runs");
        assert!(status.success());
        let bare = |args: &[&str]| {
            let status = Command::new("git")
                .arg(format!("--git-dir={}", cfg.display()))
                .arg(format!("--work-tree={}", dir.path().display()))
                .args(["-c", "user.email=t@t", "-c", "user.name=t"])
                .args(args)
                .current_dir(dir.path())
                .status()
                .expect("git runs");
            assert!(status.success(), "git {args:?} failed");
        };
        std::fs::write(dir.path().join(".sync-manifest"), ".sync-manifest\n").expect("write");
        bare(&["add", ".sync-manifest"]);
        bare(&["commit", "-q", "-m", "init"]);

        let git = Git::discover(dir.path());
        let listing = git.ls_tree("main").expect("git works");
        assert_eq!(listing.paths().count(), 1);
    }

    use crate::manifest;
    use crate::plan::{TargetSnapshot, plan_sync};

    /// linux checked out; mac has a changed shared.txt and a new shared file;
    /// linux has a shared file mac lacks.
    fn sync_fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        run(dir.path(), &["init", "-q", "-b", "linux"]);
        std::fs::write(
            dir.path().join(".sync-manifest"),
            ".sync-manifest\nshared.txt\ndir/\n~local.txt\n",
        )
        .expect("write");
        std::fs::write(dir.path().join("shared.txt"), "same\n").expect("write");
        std::fs::create_dir_all(dir.path().join("dir")).expect("mkdir");
        std::fs::write(dir.path().join("dir/gone.txt"), "only on linux\n").expect("write");
        std::fs::write(dir.path().join("local.txt"), "linux local\n").expect("write");
        run(dir.path(), &["add", "."]);
        run(dir.path(), &["commit", "-q", "-m", "base"]);
        run(dir.path(), &["checkout", "-q", "-b", "mac"]);
        std::fs::write(dir.path().join("shared.txt"), "changed on mac\n").expect("write");
        std::fs::write(dir.path().join("dir/new.txt"), "new on mac\n").expect("write");
        std::fs::remove_file(dir.path().join("dir/gone.txt")).expect("rm");
        std::fs::write(dir.path().join("local.txt"), "mac local\n").expect("write");
        run(dir.path(), &["add", "-A"]);
        run(dir.path(), &["commit", "-q", "-m", "mac changes"]);
        dir
    }

    fn plan_for(git: &Git, source: &str, target: &str) -> crate::plan::SyncPlan {
        let manifest_text = git.show_manifest(source).expect("git").expect("manifest");
        let manifest = manifest::parse(&manifest_text).expect("valid");
        let source_listing = git.ls_tree(source).expect("git");
        let target_listing = git.ls_tree(target).expect("git");
        let union: Vec<&RelPath> = source_listing
            .paths()
            .chain(target_listing.paths())
            .collect();
        let shared = manifest.partition(union.into_iter()).shared;
        let target_head = git.rev_parse(target).expect("git");
        plan_sync(
            &shared,
            &source_listing,
            &TargetSnapshot::new(target_listing, target_head),
        )
    }

    fn with_identity(git: &Git) -> Git {
        git.with_env("GIT_AUTHOR_NAME", "t")
            .with_env("GIT_AUTHOR_EMAIL", "t@t")
            .with_env("GIT_COMMITTER_NAME", "t")
            .with_env("GIT_COMMITTER_EMAIL", "t@t")
    }

    #[test]
    fn rev_parse_current_branch_and_remote_queries() {
        let dir = sync_fixture();
        let git = Git::discover(dir.path());
        let head = git.rev_parse("mac").expect("git");
        assert_eq!(head.as_str().len(), 40);
        assert_eq!(git.current_branch().expect("git"), Some("mac".to_string()));
        assert!(!git.has_remote("origin").expect("git"));
        assert!(git.rev_parse("no-such-ref").is_err());
        run(dir.path(), &["checkout", "-q", "--detach"]);
        assert_eq!(git.current_branch().expect("git"), None);
    }

    #[test]
    fn dirty_paths_lists_worktree_and_index_changes() {
        let dir = sync_fixture();
        let git = Git::discover(dir.path());
        assert!(git.dirty_paths().expect("git").is_empty());
        std::fs::write(dir.path().join("shared.txt"), "edited\n").expect("write");
        std::fs::write(dir.path().join("untracked.txt"), "x\n").expect("write");
        run(dir.path(), &["add", "untracked.txt"]);
        let mut dirty: Vec<String> = git
            .dirty_paths()
            .expect("git")
            .iter()
            .map(|path| path.as_str().to_string())
            .collect();
        dirty.sort();
        assert_eq!(dirty, vec!["shared.txt", "untracked.txt"]);
    }

    #[test]
    fn dirty_paths_handles_paths_with_spaces() {
        let dir = sync_fixture();
        std::fs::write(dir.path().join("dir/has space.txt"), "original\n").expect("write");
        run(dir.path(), &["add", "dir/has space.txt"]);
        run(dir.path(), &["commit", "-q", "-m", "add spaced file"]);
        std::fs::write(dir.path().join("dir/has space.txt"), "edited\n").expect("write");

        let git = Git::discover(dir.path());
        let dirty: Vec<String> = git
            .dirty_paths()
            .expect("git")
            .iter()
            .map(|path| path.as_str().to_string())
            .collect();
        assert_eq!(dirty, vec!["dir/has space.txt"]);
    }

    #[test]
    fn dirty_paths_reports_the_new_name_of_a_staged_rename() {
        let dir = sync_fixture();
        run(dir.path(), &["mv", "shared.txt", "renamed.txt"]);

        let git = Git::discover(dir.path());
        let dirty: Vec<String> = git
            .dirty_paths()
            .expect("git")
            .iter()
            .map(|path| path.as_str().to_string())
            .collect();
        assert_eq!(dirty, vec!["renamed.txt"]);
    }

    #[test]
    fn commit_plan_lands_on_the_target_without_touching_the_worktree() {
        let dir = sync_fixture();
        let git = with_identity(&Git::discover(dir.path()));
        let before = git.rev_parse("linux").expect("git");
        let plan = plan_for(&git, "mac", "linux");
        assert!(!plan.is_empty());

        let commit = git
            .commit_plan(&plan, "linux", "Sync shared paths from mac (test)")
            .expect("apply");

        assert_eq!(git.rev_parse("linux").expect("git"), commit);
        let parent = git.output_text(&["rev-parse", "linux^"]).expect("git");
        assert_eq!(parent.trim(), before.as_str());
        let synced = git.ls_tree("linux").expect("git");
        let source = git.ls_tree("mac").expect("git");
        assert_eq!(
            synced.get(&RelPath::parse("shared.txt").expect("valid")),
            source.get(&RelPath::parse("shared.txt").expect("valid"))
        );
        assert_eq!(
            synced.get(&RelPath::parse("dir/gone.txt").expect("valid")),
            None
        );
        assert!(
            synced
                .get(&RelPath::parse("dir/new.txt").expect("valid"))
                .is_some()
        );
        assert_eq!(
            synced
                .get(&RelPath::parse("local.txt").expect("valid"))
                .map(|(_, blob)| blob.as_str()),
            git.ls_tree(before.as_str())
                .expect("git")
                .get(&RelPath::parse("local.txt").expect("valid"))
                .map(|(_, blob)| blob.as_str()),
            "per-branch file untouched"
        );
        assert_eq!(git.current_branch().expect("git"), Some("mac".to_string()));
        assert!(
            git.dirty_paths().expect("git").is_empty(),
            "worktree untouched"
        );
        assert!(
            git.output_text(&["status", "--porcelain"])
                .expect("git")
                .is_empty()
        );
    }

    #[test]
    fn commit_plan_refuses_when_the_target_moved_and_leaves_the_ref_alone() {
        let dir = sync_fixture();
        let git = with_identity(&Git::discover(dir.path()));
        let plan = plan_for(&git, "mac", "linux");
        run(dir.path(), &["checkout", "-q", "linux"]);
        std::fs::write(dir.path().join("dir/late.txt"), "landed after planning\n").expect("write");
        run(dir.path(), &["add", "dir/late.txt"]);
        run(dir.path(), &["commit", "-q", "-m", "moved"]);
        run(dir.path(), &["checkout", "-q", "mac"]);
        let moved_to = git.rev_parse("linux").expect("git");

        let error = git
            .commit_plan(&plan, "linux", "stale plan")
            .expect_err("must refuse");
        assert!(format!("{error:#}").contains("update-ref"), "{error:#}");
        assert_eq!(
            git.rev_parse("linux").expect("git"),
            moved_to,
            "ref not moved"
        );
        assert!(
            git.output_text(&["status", "--porcelain"])
                .expect("git")
                .is_empty()
        );
    }

    #[test]
    fn temp_index_guard_removes_its_file_on_drop() {
        let guard = TempIndex::create().expect("guard");
        let path = guard.path.clone();
        assert!(
            !path.exists(),
            "create reserves a name, git creates the file"
        );
        std::fs::write(&path, b"index bytes").expect("write");
        drop(guard);
        assert!(!path.exists(), "dropped guard removes the file");
    }
}
