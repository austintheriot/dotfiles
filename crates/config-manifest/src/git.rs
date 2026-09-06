use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};

use crate::doctor::{CrateName, Stamp};

/// Both shapes this repo comes in: the real dotfiles repo is bare at
/// `<root>/.cfg` with `<root>` as the worktree, and every test fixture is a
/// normal repository with `.git` inside `<root>`.
#[derive(Debug, Clone)]
pub struct Git {
    prefix: Vec<String>,
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
        Git { prefix }
    }

    fn command(&self, args: &[&str], index: Option<&Path>) -> Command {
        let mut command = Command::new("git");
        command.args(&self.prefix).args(args);
        if let Some(index) = index {
            command.env("GIT_INDEX_FILE", index);
        }
        command
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

    /// The expected stamp for every workspace member, read out of the
    /// worktree through a temp index.
    ///
    /// The IO edge for `doctor`. Everything above it is pure. Mirrors
    /// `.scripts/config/config-stamp`'s worktree gather (a temp index, one
    /// `add -- crates`, one `write-tree`), so the two never read the
    /// workspace two different ways; `doctor` is a second consumer of the
    /// same worktree state, not a second definition of what a stamp is.
    pub fn workspace_stamps(&self) -> anyhow::Result<BTreeMap<CrateName, Stamp>> {
        const WORKSPACE: &str = "crates";

        let index = TempIndex::create()?;
        let index_path: &Path = &index.path;
        self.run_checked(&["read-tree", "--empty"], Some(index_path))?;
        self.run_checked(&["add", "--", WORKSPACE], Some(index_path))?;
        let root_tree = self.run_checked(&["write-tree"], Some(index_path))?;
        let root_tree = root_tree.trim();

        let object_at = |relative: &str| -> anyhow::Result<String> {
            let spec = format!("{root_tree}:{WORKSPACE}/{relative}");
            let text = self.output_text(&["rev-parse", "--verify", "--quiet", &spec])?;
            let trimmed = text.trim();
            if trimmed.is_empty() {
                bail!("{WORKSPACE}/{relative} is not in the stamped tree");
            }
            Ok(trimmed.to_string())
        };

        let lock_blob = object_at("Cargo.lock")?;
        let workspace_blob = object_at("Cargo.toml")?;

        let manifest_spec = format!("{root_tree}:{WORKSPACE}/Cargo.toml");
        let manifest_text = self.output_text(&["show", &manifest_spec])?;
        let members = parse_workspace_members(&manifest_text);
        if members.is_empty() {
            bail!("no workspace members found in {WORKSPACE}/Cargo.toml");
        }

        let mut stamps = BTreeMap::new();
        for member in members {
            let crate_name = CrateName::parse(&member)
                .map_err(|error| anyhow::anyhow!("invalid workspace member name {member}: {error:?}"))?;
            let crate_tree = object_at(&member)?;
            let stamp = Stamp::parse(&format!("{crate_tree}:{lock_blob}:{workspace_blob}"))
                .map_err(|error| anyhow::anyhow!("workspace_stamps produced a malformed stamp: {error:?}"))?;
            stamps.insert(crate_name, stamp);
        }
        Ok(stamps)
    }
}

/// Parses the single-line `members = [...]` array out of a workspace
/// `Cargo.toml`, the same shape `.scripts/config/config-stamp` parses with
/// `sed`. Kept here rather than shared with the shell script because the two
/// have no common runtime to share it through; both must agree the array is
/// single-line, which the shell script's own comment already documents as a
/// requirement on the manifest, not an assumption unique to one reader.
fn parse_workspace_members(manifest_text: &str) -> Vec<String> {
    manifest_text
        .lines()
        .find_map(|line| {
            let line = line.trim();
            let inner = line.strip_prefix("members = [")?.strip_suffix(']')?;
            Some(
                inner
                    .split(',')
                    .map(|entry| entry.trim().trim_matches('"'))
                    .filter(|entry| !entry.is_empty())
                    .map(str::to_string)
                    .collect(),
            )
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        std::fs::write(dir.path().join("marker.txt"), "hello\n").expect("write");
        bare(&["add", "marker.txt"]);
        bare(&["commit", "-q", "-m", "init"]);

        let git = Git::discover(dir.path());
        let text = git.output_text(&["show", "main:marker.txt"]).expect("git works");
        assert_eq!(text, "hello\n");
    }

    #[test]
    fn workspace_stamps_reads_crate_tree_and_shared_blobs_out_of_the_worktree() {
        let dir = tempfile::tempdir().expect("tempdir");
        run(dir.path(), &["init", "-q", "-b", "main"]);
        std::fs::create_dir_all(dir.path().join("crates/one/src")).expect("mkdir");
        std::fs::write(
            dir.path().join("crates/Cargo.toml"),
            "[workspace]\nmembers = [\"one\"]\n",
        )
        .expect("write");
        std::fs::write(dir.path().join("crates/Cargo.lock"), "lock\n").expect("write");
        std::fs::write(
            dir.path().join("crates/one/Cargo.toml"),
            "[package]\nname = \"one\"\n",
        )
        .expect("write");
        std::fs::write(dir.path().join("crates/one/src/main.rs"), "fn main() {}\n")
            .expect("write");
        run(dir.path(), &["add", "."]);
        run(dir.path(), &["commit", "-q", "-m", "init"]);

        let git = Git::discover(dir.path());
        let stamps = git.workspace_stamps().expect("git works");
        let one = CrateName::parse("one").expect("valid");
        assert!(stamps.contains_key(&one));
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
