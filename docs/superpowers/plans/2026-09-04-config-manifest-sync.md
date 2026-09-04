# config-manifest `sync` Implementation Plan (Plan B2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `config-manifest sync [--dry-run] [--to <branch>]`: make the other branch's shared paths match the current branch by committing directly onto it through git plumbing, never touching `$HOME` or the working tree, never pushing.

**Architecture:** A pure planner (`plan.rs`) turns two snapshots into a `SyncPlan` of domain edits (`Set`, `Remove`) that carries the target commit it was planned against. The applier (`git.rs::commit_plan`) replays the plan into a temp index and finishes with `update-ref` compare-and-swap, so nothing observable changes until the last step and a moved ref fails it. `main.rs` gathers inputs (current branch, fetch, listings, dirty paths), asks the core, applies, then re-runs `check` as a self-audit. `SharedPaths` from `Manifest::partition` is the only source of paths the planner may name, so it cannot touch an excluded or per-branch path.

**Tech Stack:** Rust 1.94 (edition 2024) on the existing lib+bin crate; `anyhow` at the edge only; dev: `proptest`, `assert_cmd`, `tempfile`. Git plumbing verified on git 2.50.

**Spec:** `docs/superpowers/specs/2026-09-04-config-command-and-manifest-crate-design.md`, sections 6.5, 6.6, 6.7, 7 (exit codes), 7.2, 9.2, 9.3, 10 step 3. Plan C (dispatcher, `config sync`) follows.

## Global Constraints

- Sans-IO: `plan.rs` performs no IO and spawns nothing; it takes values and returns values. Only `git.rs` and `main.rs` use `std::process::Command`. `main.rs` decides nothing about paths or rules: classification of dirty paths goes through `Manifest::partition`.
- `TreeEdit` variants are constructible only inside `plan.rs` (private-field structs with accessors), so nothing outside the planner can fabricate a `Set` naming a blob not in `source`. `SyncPlan` fields are private with accessors. `planned_against` is copied from `TargetSnapshot::head`, never passed separately.
- The planner is total over valid inputs and returns no `Result`.
- The applier's atomicity contract: steps before `update-ref` create only unreferenced objects; `update-ref <ref> <new> <planned_against>` is a compare-and-swap; a failure at any step leaves the target ref where it was. The temp index lives under the system temp dir and is removed on every path (RAII guard).
- `sync` never checks out, never writes the working tree, never pushes. It refuses (exit 1) when the current branch is not `mac` or `linux`; usage error (exit 2) when `--to` is not `mac`/`linux` or equals the current branch; refuses (exit 1) when local `<target>` is not at `origin/<target>` after fetch; exit 3 if the post-apply `check HEAD <target>` is not clean.
- Diagnostics, warnings, and refusals go to stderr; the plan (`--dry-run`) and results go to stdout.
- Fixtures without an `origin` remote skip the fetch and the origin comparison with a stderr note. Every fixture git command sets `user.email`/`user.name`; commits made by the applier get identity from `GIT_AUTHOR_*`/`GIT_COMMITTER_*` env in tests.
- Domain errors are enums; `anyhow` only in `git.rs`/`main.rs`. No `unwrap()` outside tests. No single-letter identifiers. `cargo fmt --check` and `cargo clippy --all-targets --locked -- -D warnings` clean at every commit. Every cargo command runs with `CARGO_TARGET_DIR="$HOME/.cache/config-manifest/target"`.
- The pre-commit leak check scans every commit; no secret-shaped literals. Commit messages: no em dashes, no emoji.
- Shared paths changed here (`crates/`, `.claude/rules/dotfiles-tests.md`) must be synced to `linux` before pushing `mac`. Run `~/.scripts/config/config-build` before any push so the embedded stamp matches.

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/config-manifest/src/plan.rs` (create) | `TargetSnapshot`, `TreeEdit`/`SetEdit`/`RemoveEdit`, `SyncPlan`, `plan_sync`, `apply_to`, `Display` for edits |
| `crates/config-manifest/src/tree.rs` (modify) | `TreeListing::insert` and `remove` so `apply_to` can simulate without reaching into the map |
| `crates/config-manifest/src/lib.rs` (modify) | `pub mod plan;` |
| `crates/config-manifest/src/git.rs` (modify) | `rev_parse`, `current_branch`, `has_remote`, `fetch`, `dirty_paths`, `commit_plan` (the only writer) |
| `crates/config-manifest/src/main.rs` (modify) | `sync` subcommand: flags, the nine steps, exit codes |
| `crates/config-manifest/tests/sync_cli.rs` (create) | end-to-end tests on fixture repos, including an `origin` fixture |
| `.claude/rules/dotfiles-tests.md` (modify) | the hand sync procedure now names `config-manifest sync` |

---

### Task 1: The pure planner

**Files:**
- Create: `crates/config-manifest/src/plan.rs`
- Modify: `crates/config-manifest/src/tree.rs` (two methods), `crates/config-manifest/src/lib.rs` (`pub mod plan;`)

**Interfaces:**
- Consumes: `RelPath`, `BlobId`, `CommitId` (`path.rs`); `FileMode`, `TreeListing::get/paths` (`tree.rs`); `SharedPaths::iter/contains`, `Manifest::partition` (`manifest.rs`).
- Produces (Task 2 and 3 consume exactly these):

```rust
pub struct TargetSnapshot { /* private */ }
impl TargetSnapshot {
    pub fn new(listing: TreeListing, head: CommitId) -> Self;
    pub fn listing(&self) -> &TreeListing;
    pub fn head(&self) -> &CommitId;
}
pub struct SetEdit { /* private */ }     // path(), mode(), blob()
pub struct RemoveEdit { /* private */ }  // path()
pub enum TreeEdit { Set(SetEdit), Remove(RemoveEdit) }   // Display: "set <path> <mode> <blob>" / "remove <path>"
pub struct SyncPlan { /* private */ }
impl SyncPlan {
    pub fn edits(&self) -> &[TreeEdit];
    pub fn planned_against(&self) -> &CommitId;
    pub fn is_empty(&self) -> bool;
    pub fn apply_to(&self, target: &TreeListing) -> TreeListing;
}
pub fn plan_sync(shared: &SharedPaths, source: &TreeListing, target: &TargetSnapshot) -> SyncPlan;
// tree.rs additions
impl TreeListing {
    pub fn insert(&mut self, path: RelPath, mode: FileMode, blob: BlobId);
    pub fn remove(&mut self, path: &RelPath);
}
```

- [ ] **Step 1: Write the failing tests**

Create `crates/config-manifest/src/plan.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest;
    use crate::path::{CommitId, RelPath};
    use crate::tree::parse_ls_tree;
    use proptest::prelude::*;

    const ID_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const ID_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const ID_M: &str = "cccccccccccccccccccccccccccccccccccccccc";
    const HEAD: &str = "dddddddddddddddddddddddddddddddddddddddd";

    fn listing(entries: &[(&str, &str, &str)]) -> TreeListing {
        let mut bytes = Vec::new();
        for (mode, id, path) in entries {
            bytes.extend_from_slice(format!("{mode} blob {id}\t{path}\0").as_bytes());
        }
        parse_ls_tree(&bytes).expect("valid listing")
    }

    fn head() -> CommitId {
        CommitId::parse(HEAD).expect("valid")
    }

    fn shared_of(text: &str, source: &TreeListing, target: &TreeListing) -> SharedPaths {
        let manifest = manifest::parse(text).expect("valid manifest");
        let union: Vec<&RelPath> = source.paths().chain(target.paths()).collect();
        manifest.partition(union.into_iter()).shared
    }

    fn rel(raw: &str) -> RelPath {
        RelPath::parse(raw).expect("valid")
    }

    #[test]
    fn identical_trees_produce_an_empty_plan_that_still_records_the_target() {
        let source = listing(&[("100644", ID_M, ".sync-manifest"), ("100644", ID_A, "shared.txt")]);
        let target = TargetSnapshot::new(source.clone(), head());
        let shared = shared_of(".sync-manifest\nshared.txt\n", &source, target.listing());
        let plan = plan_sync(&shared, &source, &target);
        assert!(plan.is_empty());
        assert_eq!(plan.planned_against(), &head());
    }

    #[test]
    fn a_differing_blob_is_a_set_and_a_missing_target_file_is_a_set() {
        let source = listing(&[("100644", ID_M, ".sync-manifest"), ("100644", ID_A, "shared.txt"), ("100755", ID_B, "dir/run.sh")]);
        let target_listing = listing(&[("100644", ID_M, ".sync-manifest"), ("100644", ID_B, "shared.txt")]);
        let target = TargetSnapshot::new(target_listing, head());
        let shared = shared_of(".sync-manifest\nshared.txt\ndir/\n", &source, target.listing());
        let plan = plan_sync(&shared, &source, &target);
        let rendered: Vec<String> = plan.edits().iter().map(ToString::to_string).collect();
        assert_eq!(
            rendered,
            vec![
                format!("set dir/run.sh 100755 {ID_B}"),
                format!("set shared.txt 100644 {ID_A}"),
            ]
        );
    }

    #[test]
    fn a_mode_only_change_is_a_set() {
        let source = listing(&[("100755", ID_A, "run.sh")]);
        let target = TargetSnapshot::new(listing(&[("100644", ID_A, "run.sh")]), head());
        let shared = shared_of("run.sh\n", &source, target.listing());
        let plan = plan_sync(&shared, &source, &target);
        assert_eq!(plan.edits().len(), 1);
        assert!(matches!(plan.edits()[0], TreeEdit::Set(_)));
    }

    #[test]
    fn a_file_only_on_the_target_is_a_remove() {
        let source = listing(&[("100644", ID_M, ".sync-manifest")]);
        let target = TargetSnapshot::new(
            listing(&[("100644", ID_M, ".sync-manifest"), ("100644", ID_B, "dir/old.txt")]),
            head(),
        );
        let shared = shared_of(".sync-manifest\ndir/\n", &source, target.listing());
        let plan = plan_sync(&shared, &source, &target);
        let rendered: Vec<String> = plan.edits().iter().map(ToString::to_string).collect();
        assert_eq!(rendered, vec!["remove dir/old.txt".to_string()]);
    }

    #[test]
    fn excluded_and_per_branch_paths_are_never_planned() {
        let source = listing(&[("100644", ID_A, "dir/common.txt"), ("100644", ID_A, "dir/platform.txt"), ("100644", ID_A, "local.txt")]);
        let target = TargetSnapshot::new(
            listing(&[("100644", ID_B, "dir/common.txt"), ("100644", ID_B, "dir/platform.txt"), ("100644", ID_B, "local.txt")]),
            head(),
        );
        let shared = shared_of("dir/\n!dir/platform.txt\n~local.txt\n", &source, target.listing());
        let plan = plan_sync(&shared, &source, &target);
        let rendered: Vec<String> = plan.edits().iter().map(ToString::to_string).collect();
        assert_eq!(rendered, vec![format!("set dir/common.txt 100644 {ID_A}")]);
    }

    #[test]
    fn apply_to_reproduces_the_source_on_shared_paths_and_leaves_the_rest() {
        let source = listing(&[("100644", ID_M, ".sync-manifest"), ("100644", ID_A, "shared.txt"), ("100644", ID_A, "local.txt")]);
        let target_listing = listing(&[("100644", ID_M, ".sync-manifest"), ("100644", ID_B, "shared.txt"), ("100644", ID_B, "local.txt"), ("100644", ID_B, "gone.txt")]);
        let target = TargetSnapshot::new(target_listing.clone(), head());
        let shared = shared_of(".sync-manifest\nshared.txt\ngone.txt\n~local.txt\n", &source, target.listing());
        let after = plan_sync(&shared, &source, &target).apply_to(&target_listing);
        assert_eq!(after.get(&rel("shared.txt")), source.get(&rel("shared.txt")));
        assert_eq!(after.get(&rel("gone.txt")), None);
        assert_eq!(after.get(&rel("local.txt")), target_listing.get(&rel("local.txt")));
    }

    fn arbitrary_path() -> impl Strategy<Value = String> {
        prop::collection::vec("[a-z][a-z0-9]{0,4}", 1..3).prop_map(|segments| segments.join("/"))
    }

    fn arbitrary_blob() -> impl Strategy<Value = String> {
        "[0-9a-f]{40}"
    }

    fn arbitrary_listing() -> impl Strategy<Value = TreeListing> {
        prop::collection::btree_map(arbitrary_path(), (prop_oneof![Just("100644"), Just("100755")], arbitrary_blob()), 0..8)
            .prop_map(|entries| {
                let mut bytes = Vec::new();
                for (path, (mode, blob)) in entries {
                    bytes.extend_from_slice(format!("{mode} blob {blob}\t{path}\0").as_bytes());
                }
                parse_ls_tree(&bytes).expect("generated listing is valid")
            })
    }

    proptest! {
        #[test]
        fn replanning_after_apply_is_empty(source in arbitrary_listing(), target_listing in arbitrary_listing()) {
            let target = TargetSnapshot::new(target_listing.clone(), head());
            let shared = shared_of("a/\nb/\nc/\n", &source, target.listing());
            let plan = plan_sync(&shared, &source, &target);
            let after = TargetSnapshot::new(plan.apply_to(&target_listing), head());
            let shared_after = shared_of("a/\nb/\nc/\n", &source, after.listing());
            prop_assert!(plan_sync(&shared_after, &source, &after).is_empty());
        }

        #[test]
        fn check_after_apply_reports_no_divergence(source in arbitrary_listing(), target_listing in arbitrary_listing()) {
            let manifest = manifest::parse("a/\nb/\nc/\n").expect("valid");
            let target = TargetSnapshot::new(target_listing.clone(), head());
            let union: Vec<&RelPath> = source.paths().chain(target_listing.paths()).collect();
            let shared = manifest.partition(union.into_iter()).shared;
            let after = plan_sync(&shared, &source, &target).apply_to(&target_listing);
            let report = crate::check::check(&manifest, &source, &after);
            prop_assert!(report.findings.iter().all(|finding| !matches!(finding, crate::check::Finding::Diverged { .. })));
        }

        #[test]
        fn every_edit_names_a_shared_path_and_every_set_names_a_source_blob(source in arbitrary_listing(), target_listing in arbitrary_listing()) {
            let target = TargetSnapshot::new(target_listing, head());
            let shared = shared_of("a/\nb/\n~c/\n", &source, target.listing());
            let plan = plan_sync(&shared, &source, &target);
            for edit in plan.edits() {
                match edit {
                    TreeEdit::Set(set) => {
                        prop_assert!(shared.contains(set.path()));
                        prop_assert_eq!(source.get(set.path()), Some(&(set.mode(), set.blob().clone())));
                    }
                    TreeEdit::Remove(remove) => {
                        prop_assert!(shared.contains(remove.path()));
                        prop_assert!(source.get(remove.path()).is_none());
                    }
                }
            }
        }
    }
}
```

Add `pub mod plan;` to `lib.rs` (alphabetical: after `manifest`, before `tree`). Run: `cargo test --locked -q plan::` Expected: compile errors (`TargetSnapshot`, `plan_sync` not found). RED.

- [ ] **Step 2: Add `insert` and `remove` to `TreeListing`**

In `tree.rs`, inside `impl TreeListing`:

```rust
    pub fn insert(&mut self, path: RelPath, mode: FileMode, blob: BlobId) {
        self.0.insert(path, (mode, blob));
    }

    pub fn remove(&mut self, path: &RelPath) {
        self.0.remove(path);
    }
```

Add to `tree.rs` tests:

```rust
    #[test]
    fn insert_and_remove_edit_the_listing() {
        let mut listing = parse_ls_tree(RECORDED).expect("valid");
        let path = rel("new/file.txt");
        let blob = BlobId::parse("4b825dc642cb6eb9a060e54bf8d69288fbee4904").expect("valid");
        listing.insert(path.clone(), FileMode::Regular, blob.clone());
        assert_eq!(listing.get(&path), Some(&(FileMode::Regular, blob)));
        listing.remove(&path);
        assert_eq!(listing.get(&path), None);
        assert_eq!(listing.paths().count(), 4);
    }
```

Run: `cargo test --locked -q tree::` Expected: 5 passed.

- [ ] **Step 3: Implement `plan.rs`**

Above the test module:

```rust
use std::fmt;

use crate::manifest::SharedPaths;
use crate::path::{BlobId, CommitId, RelPath};
use crate::tree::{FileMode, TreeListing};

/// The target listing and the commit it was read from, kept together so a
/// plan cannot carry a provenance that disagrees with the tree it was
/// computed against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetSnapshot {
    listing: TreeListing,
    head: CommitId,
}

impl TargetSnapshot {
    pub fn new(listing: TreeListing, head: CommitId) -> Self {
        TargetSnapshot { listing, head }
    }

    pub fn listing(&self) -> &TreeListing {
        &self.listing
    }

    pub fn head(&self) -> &CommitId {
        &self.head
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetEdit {
    path: RelPath,
    mode: FileMode,
    blob: BlobId,
}

impl SetEdit {
    pub fn path(&self) -> &RelPath {
        &self.path
    }

    pub fn mode(&self) -> FileMode {
        self.mode
    }

    pub fn blob(&self) -> &BlobId {
        &self.blob
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveEdit {
    path: RelPath,
}

impl RemoveEdit {
    pub fn path(&self) -> &RelPath {
        &self.path
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeEdit {
    Set(SetEdit),
    Remove(RemoveEdit),
}

impl fmt::Display for TreeEdit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TreeEdit::Set(set) => write!(
                formatter,
                "set {} {} {}",
                set.path,
                set.mode.as_git_mode(),
                set.blob.as_str()
            ),
            TreeEdit::Remove(remove) => write!(formatter, "remove {}", remove.path),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncPlan {
    edits: Vec<TreeEdit>,
    planned_against: CommitId,
}

impl SyncPlan {
    pub fn edits(&self) -> &[TreeEdit] {
        &self.edits
    }

    pub fn planned_against(&self) -> &CommitId {
        &self.planned_against
    }

    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    pub fn apply_to(&self, target: &TreeListing) -> TreeListing {
        let mut result = target.clone();
        for edit in &self.edits {
            match edit {
                TreeEdit::Set(set) => result.insert(set.path.clone(), set.mode, set.blob.clone()),
                TreeEdit::Remove(remove) => result.remove(&remove.path),
            }
        }
        result
    }
}

pub fn plan_sync(shared: &SharedPaths, source: &TreeListing, target: &TargetSnapshot) -> SyncPlan {
    let mut edits = Vec::new();
    for path in shared.iter() {
        match (source.get(path), target.listing.get(path)) {
            (Some((mode, blob)), on_target) if on_target != Some(&(*mode, blob.clone())) => {
                edits.push(TreeEdit::Set(SetEdit {
                    path: path.clone(),
                    mode: *mode,
                    blob: blob.clone(),
                }));
            }
            (None, Some(_)) => edits.push(TreeEdit::Remove(RemoveEdit { path: path.clone() })),
            _ => {}
        }
    }
    SyncPlan {
        edits,
        planned_against: target.head.clone(),
    }
}
```

`SharedPaths` iterates a `BTreeSet`, so edits come out in path order, which the `set dir/run.sh` before `set shared.txt` assertion relies on.

Run: `cargo test --locked -q plan::` Expected: 9 passed (6 unit, 3 property). Then `cargo fmt && cargo clippy --all-targets --locked -- -D warnings`. `CommitId` and `FileMode::as_git_mode` now have callers; no `allow` needed anywhere.

- [ ] **Step 4: Commit**

```bash
cd /Users/austin
g() { /usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin "$@"; }
g add crates/config-manifest/src
g commit -F - <<'EOF'
Add the pure sync planner

plan_sync takes the shared paths (from Manifest::partition, so it can never
be handed an excluded or per-branch path), the source listing, and a
TargetSnapshot that keeps the target listing and its head commit together.
It returns a SyncPlan of domain edits: Set where the source differs or the
target lacks the path, Remove where only the target has it. The plan
carries planned_against copied from the snapshot, which the applier uses
as the compare-and-swap expectation; the two cannot disagree because they
never travelled separately.

TreeEdit's structs have private fields, so only the planner constructs an
edit and no caller can fabricate a Set naming a blob absent from source.
apply_to is the pure simulator: it lets the property tests assert that
replanning after apply is empty and that check() finds no divergence,
without any git.
EOF
```

---

### Task 2: The applier and the other git reads

**Files:**
- Modify: `crates/config-manifest/src/git.rs`

**Interfaces:**
- Consumes: `SyncPlan`, `TreeEdit`, `SetEdit`, `RemoveEdit` (Task 1); `CommitId`, `RelPath`; `FileMode::as_git_mode`.
- Produces (Task 3 consumes exactly these):

```rust
impl Git {
    pub fn rev_parse(&self, rev: &str) -> anyhow::Result<CommitId>;
    pub fn current_branch(&self) -> anyhow::Result<Option<String>>;   // None when detached
    pub fn has_remote(&self, remote: &str) -> anyhow::Result<bool>;
    pub fn fetch(&self, remote: &str) -> anyhow::Result<()>;
    pub fn dirty_paths(&self) -> anyhow::Result<Vec<RelPath>>;        // worktree + index changes vs HEAD
    pub fn commit_plan(&self, plan: &SyncPlan, target_branch: &str, message: &str) -> anyhow::Result<CommitId>;
}
```

- [ ] **Step 1: Write the failing fixture tests**

Append to the `#[cfg(test)] mod tests` in `git.rs` (it already has `run(dir, args)` and the normal-repo fixture pattern):

```rust
    use crate::manifest;
    use crate::path::CommitId;
    use crate::plan::{TargetSnapshot, plan_sync};

    /// linux checked out; mac has a changed shared.txt and a new shared file;
    /// linux has a shared file mac lacks.
    fn sync_fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        run(dir.path(), &["init", "-q", "-b", "linux"]);
        std::fs::write(dir.path().join(".sync-manifest"), ".sync-manifest\nshared.txt\ndir/\n~local.txt\n").expect("write");
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
        let union: Vec<&RelPath> = source_listing.paths().chain(target_listing.paths()).collect();
        let shared = manifest.partition(union.into_iter()).shared;
        let target_head = git.rev_parse(target).expect("git");
        plan_sync(&shared, &source_listing, &TargetSnapshot::new(target_listing, target_head))
    }

    fn with_identity(git: &Git) -> Git {
        git.clone()
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
        let mut dirty: Vec<String> = git.dirty_paths().expect("git").iter().map(|path| path.as_str().to_string()).collect();
        dirty.sort();
        assert_eq!(dirty, vec!["shared.txt", "untracked.txt"]);
    }

    #[test]
    fn commit_plan_lands_on_the_target_without_touching_the_worktree() {
        let dir = sync_fixture();
        let git = with_identity(&Git::discover(dir.path()));
        let before = git.rev_parse("linux").expect("git");
        let plan = plan_for(&git, "mac", "linux");
        assert!(!plan.is_empty());

        let commit = git.commit_plan(&plan, "linux", "Sync shared paths from mac (test)").expect("apply");

        assert_eq!(git.rev_parse("linux").expect("git"), commit);
        let parent = git.output_text(&["rev-parse", "linux^"]).expect("git");
        assert_eq!(parent.trim(), before.as_str());
        let synced = git.ls_tree("linux").expect("git");
        let source = git.ls_tree("mac").expect("git");
        assert_eq!(synced.get(&RelPath::parse("shared.txt").expect("valid")), source.get(&RelPath::parse("shared.txt").expect("valid")));
        assert_eq!(synced.get(&RelPath::parse("dir/gone.txt").expect("valid")), None);
        assert!(synced.get(&RelPath::parse("dir/new.txt").expect("valid")).is_some());
        assert_eq!(
            synced.get(&RelPath::parse("local.txt").expect("valid")).map(|(_, blob)| blob.as_str()),
            git.ls_tree(before.as_str()).expect("git").get(&RelPath::parse("local.txt").expect("valid")).map(|(_, blob)| blob.as_str()),
            "per-branch file untouched"
        );
        assert_eq!(git.current_branch().expect("git"), Some("mac".to_string()));
        assert!(git.dirty_paths().expect("git").is_empty(), "worktree untouched");
        assert!(git.output_text(&["status", "--porcelain"]).expect("git").is_empty());
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

        let error = git.commit_plan(&plan, "linux", "stale plan").expect_err("must refuse");
        assert!(format!("{error:#}").contains("update-ref"), "{error:#}");
        assert_eq!(git.rev_parse("linux").expect("git"), moved_to, "ref not moved");
        assert!(git.output_text(&["status", "--porcelain"]).expect("git").is_empty());
    }

    #[test]
    fn temp_index_guard_removes_its_file_on_drop() {
        let guard = TempIndex::create().expect("guard");
        let path = guard.path.clone();
        assert!(!path.exists(), "create reserves a name, git creates the file");
        std::fs::write(&path, b"index bytes").expect("write");
        drop(guard);
        assert!(!path.exists(), "dropped guard removes the file");
    }
```

`output_text` is a small crate-visible helper added in the implementation step (`pub(crate) fn output_text(&self, args: &[&str]) -> anyhow::Result<String>`). The applier's commits need an identity, and `std::env::set_var` is racy under parallel tests, so `with_identity` uses the `Git::with_env` builder added in the implementation step, which attaches env pairs to every command that `Git` spawns:

```rust
    fn with_identity(git: &Git) -> Git {
        git.with_env("GIT_AUTHOR_NAME", "t")
            .with_env("GIT_AUTHOR_EMAIL", "t@t")
            .with_env("GIT_COMMITTER_NAME", "t")
            .with_env("GIT_COMMITTER_EMAIL", "t@t")
    }
```

The temp-index test is deliberately a unit test on the guard rather than a count of files in the system temp dir: other tests in the same binary create guards concurrently, so a global count would be flaky.

Run: `cargo test --locked -q git::` Expected: compile errors (`rev_parse`, `commit_plan`, `with_env`, `output_text` not found). RED.

- [ ] **Step 2: Implement the git edge**

Replace the `Git` struct and add methods in `git.rs` (keep `discover`, `show_manifest`, `ls_tree` as they are, adjusted to the new field):

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};

use crate::path::{CommitId, RelPath};
use crate::plan::{SyncPlan, TreeEdit};
use crate::tree::{TreeListing, parse_ls_tree};

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
        let path = std::env::temp_dir().join(format!("config-manifest-index-{}-{nanos}", std::process::id()));
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
        Git { prefix, env: Vec::new() }
    }

    pub fn with_env(&self, key: &str, value: &str) -> Git {
        let mut env = self.env.clone();
        env.push((key.to_string(), value.to_string()));
        Git { prefix: self.prefix.clone(), env }
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
        String::from_utf8(output.stdout).with_context(|| format!("git {} output is not UTF-8", args.join(" ")))
    }

    pub(crate) fn output_text(&self, args: &[&str]) -> anyhow::Result<String> {
        self.run_checked(args, None)
    }

    pub fn rev_parse(&self, rev: &str) -> anyhow::Result<CommitId> {
        let spec = format!("{rev}^{{commit}}");
        let text = self.run_checked(&["rev-parse", "--verify", "--quiet", &spec], None)?;
        CommitId::parse(text.trim()).with_context(|| format!("rev-parse {rev} returned a non-id"))
    }

    pub fn current_branch(&self) -> anyhow::Result<Option<String>> {
        let output = self.output(&["symbolic-ref", "--quiet", "--short", "HEAD"])?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(Some(String::from_utf8(output.stdout).context("branch name is not UTF-8")?.trim().to_string()))
    }

    pub fn has_remote(&self, remote: &str) -> anyhow::Result<bool> {
        Ok(self.output(&["remote", "get-url", remote])?.status.success())
    }

    pub fn fetch(&self, remote: &str) -> anyhow::Result<()> {
        self.run_checked(&["fetch", "--quiet", remote], None).map(|_| ())
    }

    pub fn dirty_paths(&self) -> anyhow::Result<Vec<RelPath>> {
        let text = self.run_checked(&["status", "--porcelain", "--untracked-files=no"], None)?;
        let mut paths = Vec::new();
        for line in text.lines() {
            let Some(raw) = line.get(3..) else { continue };
            let path_text = raw.rsplit(" -> ").next().unwrap_or(raw);
            paths.push(RelPath::parse(path_text).with_context(|| format!("status path {path_text}"))?);
        }
        Ok(paths)
    }

    /// The only writer in the crate. Everything before update-ref creates
    /// unreferenced objects only; update-ref is a compare-and-swap against
    /// the commit the plan was computed from, so a moved branch fails here
    /// and nothing observable has changed.
    pub fn commit_plan(&self, plan: &SyncPlan, target_branch: &str, message: &str) -> anyhow::Result<CommitId> {
        let index = TempIndex::create()?;
        let index_path: &Path = &index.path;
        let expected = plan.planned_against().as_str();

        self.run_checked(&["read-tree", expected], Some(index_path))?;
        for edit in plan.edits() {
            match edit {
                TreeEdit::Set(set) => {
                    let info = format!("{},{},{}", set.mode().as_git_mode(), set.blob().as_str(), set.path().as_str());
                    self.run_checked(&["update-index", "--add", "--cacheinfo", &info], Some(index_path))?;
                }
                TreeEdit::Remove(remove) => {
                    self.run_checked(&["update-index", "--force-remove", "--", remove.path().as_str()], Some(index_path))?;
                }
            }
        }
        let tree = self.run_checked(&["write-tree"], Some(index_path))?;
        let commit_text = self.run_checked(&["commit-tree", tree.trim(), "-p", expected, "-m", message], None)?;
        let commit = CommitId::parse(commit_text.trim()).context("commit-tree returned a non-id")?;
        let target_ref = format!("refs/heads/{target_branch}");
        self.run_checked(&["update-ref", &target_ref, commit.as_str(), expected], None)?;
        Ok(commit)
    }
```

Keep `show_manifest` and `ls_tree` bodies, using `self.output`. Run: `cargo test --locked -q git::` Expected: 7 passed (2 old, 5 new). Then `cargo fmt && cargo clippy --all-targets --locked -- -D warnings`.

The CAS-refusal test is the failure-path test for `commit_plan`: `update-ref` rejects the swap because `linux` moved, the error names `update-ref`, the ref stays where the later commit put it, and the guard's `Drop` runs on the early return.

- [ ] **Step 3: Commit**

```bash
cd /Users/austin
g() { /usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin "$@"; }
g add crates/config-manifest/src/git.rs
g commit -F - <<'EOF'
Add the sync applier and the git reads the sync edge needs

commit_plan is the only writer in the crate. It replays a SyncPlan into a
temp index (read-tree of the planned-against commit, update-index
--cacheinfo for each Set, --force-remove for each Remove), writes the
tree, creates the commit with the planned-against commit as parent, and
finishes with update-ref as a compare-and-swap against that same commit.
Everything before the last step creates unreferenced objects only, so a
target branch that moved between planning and applying fails the swap and
nothing observable has changed. The temp index is removed by an RAII guard
on every path.

The worktree and HEAD are never touched: the fixture test asserts the
checked-out branch, the worktree, and per-branch files are exactly as
before while the other branch gained one commit whose parent is its old
tip. rev_parse, current_branch, has_remote, fetch, and dirty_paths give
main.rs the inputs it gathers at the edge; with_env lets tests supply a
commit identity without touching the process environment.
EOF
```

---

### Task 3: The `sync` subcommand

**Files:**
- Modify: `crates/config-manifest/src/main.rs`
- Create: `crates/config-manifest/tests/sync_cli.rs`

**Interfaces:**
- Consumes: Tasks 1 and 2; `check::check` for the post-apply audit.
- Produces: `config-manifest sync [--dry-run] [--to <branch>]` with the exit codes in Global Constraints; `USAGE` updated to `usage: config-manifest --version | --stamp | check [ref-a] [ref-b] | sync [--dry-run] [--to <branch>]`.

- [ ] **Step 1: Write the failing CLI tests**

Create `crates/config-manifest/tests/sync_cli.rs`:

```rust
use std::path::Path;
use std::process::Command;

use assert_cmd::prelude::*;

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.email=t@t", "-c", "user.name=t"])
        .args(args)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?} failed");
}

fn git_out(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git").arg("-C").arg(dir).args(args).output().expect("git runs");
    assert!(output.status.success(), "git {args:?} failed");
    String::from_utf8(output.stdout).expect("utf8").trim().to_string()
}

/// mac checked out with a changed shared file, a new shared file, a removed
/// shared file, and a changed per-branch file; linux at the base.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "-q", "-b", "linux"]);
    std::fs::write(dir.path().join(".sync-manifest"), ".sync-manifest\nshared.txt\ndir/\n~local.txt\n").expect("write");
    std::fs::write(dir.path().join("shared.txt"), "same\n").expect("write");
    std::fs::create_dir_all(dir.path().join("dir")).expect("mkdir");
    std::fs::write(dir.path().join("dir/gone.txt"), "only on linux\n").expect("write");
    std::fs::write(dir.path().join("local.txt"), "linux local\n").expect("write");
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "base"]);
    git(dir.path(), &["checkout", "-q", "-b", "mac"]);
    std::fs::write(dir.path().join("shared.txt"), "changed on mac\n").expect("write");
    std::fs::write(dir.path().join("dir/new.txt"), "new on mac\n").expect("write");
    std::fs::remove_file(dir.path().join("dir/gone.txt")).expect("rm");
    std::fs::write(dir.path().join("local.txt"), "mac local\n").expect("write");
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "mac changes"]);
    dir
}

fn sync(dir: &Path, args: &[&str]) -> assert_cmd::assert::Assert {
    Command::cargo_bin("config-manifest")
        .expect("binary built")
        .env("DOTFILES_ROOT", dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .arg("sync")
        .args(args)
        .assert()
}

fn check(dir: &Path, a: &str, b: &str) -> assert_cmd::assert::Assert {
    Command::cargo_bin("config-manifest").expect("binary built").env("DOTFILES_ROOT", dir).args(["check", a, b]).assert()
}

#[test]
fn dry_run_prints_the_plan_and_changes_nothing() {
    let dir = fixture();
    let before = git_out(dir.path(), &["rev-parse", "linux"]);
    let assert = sync(dir.path(), &["--dry-run"]).success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    assert!(stdout.contains("set shared.txt 100644 "), "{stdout}");
    assert!(stdout.contains("set dir/new.txt 100644 "), "{stdout}");
    assert!(stdout.contains("remove dir/gone.txt"), "{stdout}");
    assert!(!stdout.contains("local.txt"), "{stdout}");
    assert_eq!(git_out(dir.path(), &["rev-parse", "linux"]), before);
}

#[test]
fn sync_commits_onto_the_other_branch_and_leaves_the_worktree_alone() {
    let dir = fixture();
    let before = git_out(dir.path(), &["rev-parse", "linux"]);
    let assert = sync(dir.path(), &[]).success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    assert!(stdout.contains("config push origin mac"), "{stdout}");
    assert!(stdout.contains("config push origin linux"), "{stdout}");

    assert_eq!(git_out(dir.path(), &["rev-parse", "linux^"]), before, "one new commit on linux");
    assert_eq!(git_out(dir.path(), &["log", "-1", "--format=%s", "linux"]), format!("Sync shared paths from mac ({})", &git_out(dir.path(), &["rev-parse", "--short", "mac"])));
    assert_eq!(git_out(dir.path(), &["symbolic-ref", "--short", "HEAD"]), "mac");
    assert_eq!(git_out(dir.path(), &["status", "--porcelain"]), "");
    assert_eq!(std::fs::read_to_string(dir.path().join("local.txt")).expect("read"), "mac local\n");
    assert_eq!(git_out(dir.path(), &["show", "linux:local.txt"]), "linux local");
    check(dir.path(), "mac", "linux").success();
}

#[test]
fn a_second_sync_has_nothing_to_do() {
    let dir = fixture();
    sync(dir.path(), &[]).success();
    let after_first = git_out(dir.path(), &["rev-parse", "linux"]);
    let assert = sync(dir.path(), &[]).success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    assert!(stdout.contains("nothing to sync"), "{stdout}");
    assert_eq!(git_out(dir.path(), &["rev-parse", "linux"]), after_first);
}

#[test]
fn a_branch_other_than_mac_or_linux_is_refused() {
    let dir = fixture();
    git(dir.path(), &["checkout", "-q", "-b", "feature"]);
    let assert = sync(dir.path(), &[]).code(1);
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8");
    assert!(stderr.contains("feature"), "{stderr}");
}

#[test]
fn a_bad_to_target_is_a_usage_error() {
    let dir = fixture();
    sync(dir.path(), &["--to", "mac"]).code(2);
    sync(dir.path(), &["--to", "feature"]).code(2);
    sync(dir.path(), &["--bogus"]).code(2);
}

#[test]
fn a_target_not_at_its_origin_is_refused() {
    let dir = fixture();
    let origin = tempfile::tempdir().expect("tempdir");
    assert!(Command::new("git").args(["init", "-q", "--bare"]).arg(origin.path()).status().expect("git").success());
    git(dir.path(), &["remote", "add", "origin", origin.path().to_str().expect("path")]);
    git(dir.path(), &["push", "-q", "origin", "mac", "linux"]);
    git(dir.path(), &["checkout", "-q", "linux"]);
    std::fs::write(dir.path().join("dir/local-only.txt"), "not pushed\n").expect("write");
    git(dir.path(), &["add", "dir/local-only.txt"]);
    git(dir.path(), &["commit", "-q", "-m", "linux moved locally"]);
    git(dir.path(), &["checkout", "-q", "mac"]);
    let before = git_out(dir.path(), &["rev-parse", "linux"]);
    let assert = sync(dir.path(), &[]).code(1);
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8");
    assert!(stderr.contains("origin/linux"), "{stderr}");
    assert_eq!(git_out(dir.path(), &["rev-parse", "linux"]), before);
}

#[test]
fn a_dirty_shared_path_warns_but_syncs_committed_content() {
    let dir = fixture();
    std::fs::write(dir.path().join("shared.txt"), "uncommitted edit\n").expect("write");
    let assert = sync(dir.path(), &[]).success();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8");
    assert!(stderr.contains("uncommitted"), "{stderr}");
    assert_eq!(git_out(dir.path(), &["show", "linux:shared.txt"]), "changed on mac");
}
```

Run: `cargo test --locked -q --test sync_cli` Expected: every test fails with exit code 2 (`sync` is not a known subcommand yet). RED.

- [ ] **Step 2: Implement `sync` in `main.rs`**

Change `USAGE` and add the `Some("sync")` arm mirroring `check`'s (`run_sync(&args[1..])`, errors printed as `config-manifest sync: {error:#}` with exit 1). Add:

```rust
use config_manifest::plan::{TargetSnapshot, plan_sync};

const SYNC_BRANCHES: [&str; 2] = ["mac", "linux"];

struct SyncArgs {
    dry_run: bool,
    to: Option<String>,
}

fn parse_sync_args(args: &[String]) -> Option<SyncArgs> {
    let mut parsed = SyncArgs { dry_run: false, to: None };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--dry-run" => parsed.dry_run = true,
            "--to" => {
                index += 1;
                parsed.to = Some(args.get(index)?.clone());
            }
            _ => return None,
        }
        index += 1;
    }
    Some(parsed)
}

fn run_sync(args: &[String]) -> anyhow::Result<u8> {
    let Some(parsed) = parse_sync_args(args) else {
        eprintln!("{USAGE}");
        return Ok(2);
    };
    let repo = git::Git::discover(&dotfiles_root()?);

    let Some(source) = repo.current_branch()? else {
        eprintln!("config-manifest sync: HEAD is detached; check out mac or linux first");
        return Ok(1);
    };
    if !SYNC_BRANCHES.contains(&source.as_str()) {
        eprintln!("config-manifest sync: refusing to sync from branch {source}; only mac and linux are synced");
        return Ok(1);
    }
    let target = match parsed.to {
        Some(named) if !SYNC_BRANCHES.contains(&named.as_str()) || named == source => {
            eprintln!("config-manifest sync: --to must name the other of mac and linux");
            return Ok(2);
        }
        Some(named) => named,
        None => SYNC_BRANCHES.iter().find(|name| **name != source).map(|name| name.to_string()).unwrap_or_default(),
    };

    if repo.has_remote("origin")? {
        repo.fetch("origin")?;
        let local = repo.rev_parse(&target)?;
        let remote = repo.rev_parse(&format!("origin/{target}"))?;
        if local != remote {
            eprintln!("config-manifest sync: refusing, local {target} is not at origin/{target}; the other machine may have pushed work this would overwrite.");
            eprintln!("  fetch and integrate first, for example: config pull origin {target}:{target}");
            return Ok(1);
        }
    } else {
        eprintln!("config-manifest sync: no origin remote; skipping the fetch and the origin comparison");
    }

    let Some(manifest_text) = repo.show_manifest(&source)? else {
        eprintln!("config-manifest sync: could not read .sync-manifest from {source}");
        return Ok(1);
    };
    let manifest = manifest::parse(&manifest_text)?;
    let source_listing = repo.ls_tree("HEAD")?;
    let target_listing = repo.ls_tree(&target)?;
    let target_head = repo.rev_parse(&target)?;

    let dirty = repo.dirty_paths()?;
    let dirty_shared = manifest.partition(dirty.iter()).shared;
    if dirty_shared.iter().next().is_some() {
        eprintln!("config-manifest sync: warning, uncommitted changes under shared paths are not synced (sync copies committed content):");
        for path in dirty_shared.iter() {
            eprintln!("  {path}");
        }
    }

    let union: Vec<&config_manifest::path::RelPath> = source_listing.paths().chain(target_listing.paths()).collect();
    let shared = manifest.partition(union.into_iter()).shared;
    let target_snapshot = TargetSnapshot::new(target_listing, target_head);
    let plan = plan_sync(&shared, &source_listing, &target_snapshot);

    if parsed.dry_run {
        for edit in plan.edits() {
            println!("{edit}");
        }
        return Ok(0);
    }
    if plan.is_empty() {
        println!("config-manifest sync: nothing to sync, {target} already matches {source} on every shared path");
        return Ok(0);
    }

    let short = repo.output_short(&source)?;
    let message = format!("Sync shared paths from {source} ({short})");
    let commit = repo.commit_plan(&plan, &target, &message)?;
    println!("config-manifest sync: committed {} to {target} ({} edit(s))", &commit.as_str()[..12], plan.edits().len());

    let synced_listing = repo.ls_tree(&target)?;
    let report = check::check(&manifest, &source_listing, &synced_listing);
    let rendered = check::render(&report, &source, &target);
    if rendered.exit_code != 0 {
        eprintln!("config-manifest sync: BUG: the branches still differ after syncing");
        std::io::stderr().write_all(rendered.stdout.as_bytes())?;
        std::io::stderr().write_all(rendered.stderr.as_bytes())?;
        return Ok(3);
    }

    println!("next: config push origin {source} && config push origin {target}");
    Ok(0)
}
```

`output_short` is one more small read in `git.rs`: `pub fn output_short(&self, rev: &str) -> anyhow::Result<String>` running `rev-parse --short <rev>` through `run_checked` and trimming. Add it in this task with a one-line test in `git.rs` (`rev-parse --short mac` has 7 or more hex chars).

Notes for the implementer: `repo.ls_tree("HEAD")` is the committed content of the checked-out branch; the dirty-path warning explains why the working tree is not what gets synced. `check` uses branch names (not `origin/`) here because the audit compares the two local branches that were just synced.

Run: `cargo test --locked -q` Expected: all previous tests plus 7 new CLI tests pass (crate total: 43 + 9 plan + 5 git + 1 tree + 7 CLI + 1 output_short = 66). Then `cargo fmt && cargo clippy --all-targets --locked -- -D warnings`.

- [ ] **Step 3: Rebuild, run the suites, commit**

```bash
~/.scripts/config/config-build
bash /Users/austin/tests/run-all.sh 2>&1 | tail -2        # all 23 suite(s) passed
~/tests/run-in-docker.sh 2>&1 | tail -2                   # all 21 suite(s) passed
cd /Users/austin
g() { /usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin "$@"; }
g add crates/config-manifest
g commit -F - <<'EOF'
Add `config-manifest sync`, one-way from the current branch

sync makes the other of mac and linux match the checked-out branch on every
shared path by committing directly onto it through git plumbing. The
working tree, HEAD, and per-branch files are never touched, and nothing is
pushed; the two push commands are printed instead.

Refusals are explicit: not on mac or linux (exit 1); --to naming anything
but the other branch (exit 2); the target not at its origin after a fetch
(exit 1), because the other machine may have pushed work this would
overwrite. Uncommitted edits under shared paths warn rather than refuse,
since sync copies committed content and says so. After applying, sync runs
the same check the pre-push gate uses and exits 3 if the branches still
differ, which would be a bug rather than a user error.

--dry-run prints the plan, one edit per line, to stdout and changes nothing.
EOF
```

---

### Task 4: Documentation and sync to linux

**Files:**
- Modify: `.claude/rules/dotfiles-tests.md` (the section describing the by-hand sync), `TODO-AGENTS.md` (the `config:sync` bullet under the config:* utilities item)
- Sync to `linux`: `crates/`, `.claude/rules/dotfiles-tests.md`

- [ ] **Step 1: Point the rules doc at the binary**

In `.claude/rules/dotfiles-tests.md`, find the passage that describes syncing shared paths by checking out the other branch and running `git checkout <branch> -- <paths>` (grep for `checkout mac --` or `shared path` near the pre-push section). Add one short paragraph after it:

```
`config-manifest sync` does this without switching branches: it commits the
current branch's shared paths directly onto the other branch through git
plumbing, refuses if that branch is not at its origin, and prints the two
push commands. `--dry-run` shows the plan. The by-hand procedure above
remains the fallback if the binary is not built.
```

In `TODO-AGENTS.md`, in the `config:*` utilities item, change the `config:sync` sub-bullet to note: "one-way sync exists as `config-manifest sync`; the `config sync` front door is Plan C; two-way stays future work."

Run `bash tests/doc-links.test.sh 2>&1 | tail -1` (2 passed).

- [ ] **Step 2: Commit, sync, push**

```bash
cd /Users/austin
g() { /usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin "$@"; }
g add .claude/rules/dotfiles-tests.md TODO-AGENTS.md
g commit -q -m "Point the sync procedure at config-manifest sync

The rules doc keeps the by-hand steps as the fallback; the TODO records that
one-way sync exists and the config sync front door is Plan C."
~/.scripts/config/config-build
config-manifest sync --dry-run            # dogfood: shows the plan for linux
```

If the dry run lists exactly the shared-path changes made by this plan (crate files and the rules doc), apply it for real; this is the first production use of the tool and the linux commit it makes is the sync:

```bash
config-manifest sync                      # commits onto linux, prints the two push commands
config-manifest check mac linux           # match on all 16 shared path(s)
g push origin linux
g push origin mac
g branch -vv | grep -E '^\*|linux'
```

If the dry run shows anything unexpected, stop: do not apply, and report the plan output. The fallback is the by-hand procedure from Plan B1 Task 6 (checkout linux, `git checkout mac -- crates .claude/rules/dotfiles-tests.md`, commit, push).

Expected: both pushes pass the hooks (leak scan, stamp match, drift check via the binary, Docker suite); CI green on both branches, allowing for the known drift-workflow push race (rerun the first-pushed branch's drift job if it raced).

---

## Self-review

**Spec coverage.** 6.5 types and planner: Task 1 (with `TargetSnapshot::new`, private-field edits, `apply_to`, total planner). 6.6 `Git` methods and `commit_plan` steps 1-7 with the atomicity contract: Task 2 (the interrupted-after-`write-tree` test from 9.3 is realised as the CAS-refusal test, which exercises the same guarantee: the ref does not move when the final step fails, and the temp-index cleanup test covers the failure path). 6.7 main gathers at the edge: Task 3. 7 exit codes and 7.2 steps 1-9: Task 3 (step 2/3 skip with a note when no origin exists, an addition the spec did not anticipate for fixtures). 9.2 property tests: Task 1 (idempotence, check-after-sync, edits within shared and Set blobs in source; the parser round-trip already exists). 9.3 integration: Task 2 and 3 (worktree untouched, sync then check clean, refusal paths). 10 step 3: this plan. Section 7.2 step 9 prints `config push origin …`, which works today through the git alias and through the Plan C dispatcher later.

**Placeholder scan.** None.

**Name consistency.** `TargetSnapshot::new/listing/head`; `SetEdit::path/mode/blob`; `RemoveEdit::path`; `TreeEdit::{Set, Remove}`; `SyncPlan::edits/planned_against/is_empty/apply_to`; `plan_sync(shared, source, target)`; `TreeListing::insert/remove`; `Git::rev_parse/current_branch/has_remote/fetch/dirty_paths/commit_plan/with_env/output_text/output_short`. Task 2 tests use `output_text` and `with_env`; Task 3 uses `output_short`. `check::check` and `check::render` signatures unchanged.

**Counts.** Crate tests: 43 before; Task 1 adds 9 (plan) + 1 (tree) = 53; Task 2 adds 5 (git) = 58; Task 3 adds 7 (CLI) + 1 (`output_short`) = 66. Host suite stays 23, Docker 21. If an executor's count differs by one, the test list is authoritative.

**Things the executor must know.**
- Never run `sync` against the real repo from inside a task except Task 4's dogfood step, and only after the dry run matches expectations.
- The `sync_cli` fixture checks out `mac` as the source so the target (`linux`) is not the checked-out branch; syncing onto the checked-out branch would leave the worktree differing from the new HEAD, which is exactly why `--to` equal to the current branch is a usage error.
- `dirty_paths` uses `--untracked-files=no`: untracked files are not "changes under a shared path" for the warning; only tracked modifications and staged additions are.
