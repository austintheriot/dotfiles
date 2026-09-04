use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};

use crate::tree::{TreeListing, parse_ls_tree};

/// Both shapes this repo comes in: the real dotfiles repo is bare at
/// `<root>/.cfg` with `<root>` as the worktree, and every test fixture is a
/// normal repository with `.git` inside `<root>`.
#[derive(Debug, Clone)]
pub struct Git {
    prefix: Vec<String>,
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
        Git { prefix }
    }

    fn output(&self, args: &[&str]) -> anyhow::Result<std::process::Output> {
        Command::new("git")
            .args(&self.prefix)
            .args(args)
            .output()
            .with_context(|| format!("failed to spawn git {}", args.join(" ")))
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
}
