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
        let source = listing(&[
            ("100644", ID_M, ".sync-manifest"),
            ("100644", ID_A, "shared.txt"),
        ]);
        let target = TargetSnapshot::new(source.clone(), head());
        let shared = shared_of(".sync-manifest\nshared.txt\n", &source, target.listing());
        let plan = plan_sync(&shared, &source, &target);
        assert!(plan.is_empty());
        assert_eq!(plan.planned_against(), &head());
    }

    #[test]
    fn a_differing_blob_is_a_set_and_a_missing_target_file_is_a_set() {
        let source = listing(&[
            ("100644", ID_M, ".sync-manifest"),
            ("100644", ID_A, "shared.txt"),
            ("100755", ID_B, "dir/run.sh"),
        ]);
        let target_listing = listing(&[
            ("100644", ID_M, ".sync-manifest"),
            ("100644", ID_B, "shared.txt"),
        ]);
        let target = TargetSnapshot::new(target_listing, head());
        let shared = shared_of(
            ".sync-manifest\nshared.txt\ndir/\n",
            &source,
            target.listing(),
        );
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
            listing(&[
                ("100644", ID_M, ".sync-manifest"),
                ("100644", ID_B, "dir/old.txt"),
            ]),
            head(),
        );
        let shared = shared_of(".sync-manifest\ndir/\n", &source, target.listing());
        let plan = plan_sync(&shared, &source, &target);
        let rendered: Vec<String> = plan.edits().iter().map(ToString::to_string).collect();
        assert_eq!(rendered, vec!["remove dir/old.txt".to_string()]);
    }

    #[test]
    fn excluded_and_per_branch_paths_are_never_planned() {
        let source = listing(&[
            ("100644", ID_A, "dir/common.txt"),
            ("100644", ID_A, "dir/platform.txt"),
            ("100644", ID_A, "local.txt"),
        ]);
        let target = TargetSnapshot::new(
            listing(&[
                ("100644", ID_B, "dir/common.txt"),
                ("100644", ID_B, "dir/platform.txt"),
                ("100644", ID_B, "local.txt"),
            ]),
            head(),
        );
        let shared = shared_of(
            "dir/\n!dir/platform.txt\n~local.txt\n",
            &source,
            target.listing(),
        );
        let plan = plan_sync(&shared, &source, &target);
        let rendered: Vec<String> = plan.edits().iter().map(ToString::to_string).collect();
        assert_eq!(rendered, vec![format!("set dir/common.txt 100644 {ID_A}")]);
    }

    #[test]
    fn apply_to_reproduces_the_source_on_shared_paths_and_leaves_the_rest() {
        let source = listing(&[
            ("100644", ID_M, ".sync-manifest"),
            ("100644", ID_A, "shared.txt"),
            ("100644", ID_A, "local.txt"),
        ]);
        let target_listing = listing(&[
            ("100644", ID_M, ".sync-manifest"),
            ("100644", ID_B, "shared.txt"),
            ("100644", ID_B, "local.txt"),
            ("100644", ID_B, "gone.txt"),
        ]);
        let target = TargetSnapshot::new(target_listing.clone(), head());
        let shared = shared_of(
            ".sync-manifest\nshared.txt\ngone.txt\n~local.txt\n",
            &source,
            target.listing(),
        );
        let after = plan_sync(&shared, &source, &target).apply_to(&target_listing);
        assert_eq!(
            after.get(&rel("shared.txt")),
            source.get(&rel("shared.txt"))
        );
        assert_eq!(after.get(&rel("gone.txt")), None);
        assert_eq!(
            after.get(&rel("local.txt")),
            target_listing.get(&rel("local.txt"))
        );
    }

    fn arbitrary_path() -> impl Strategy<Value = String> {
        (
            prop_oneof![Just("a"), Just("b"), Just("c"), Just("d")],
            prop::collection::vec("[a-z][a-z0-9]{0,4}", 1..3),
        )
            .prop_map(|(first, rest)| {
                std::iter::once(first.to_string())
                    .chain(rest)
                    .collect::<Vec<_>>()
                    .join("/")
            })
    }

    fn arbitrary_blob() -> impl Strategy<Value = String> {
        "[0-9a-f]{40}"
    }

    fn arbitrary_listing() -> impl Strategy<Value = TreeListing> {
        prop::collection::btree_map(
            arbitrary_path(),
            (
                prop_oneof![Just("100644"), Just("100755")],
                arbitrary_blob(),
            ),
            0..8,
        )
        .prop_map(|entries| {
            let mut bytes = Vec::new();
            for (path, (mode, blob)) in entries {
                bytes.extend_from_slice(format!("{mode} blob {blob}\t{path}\0").as_bytes());
            }
            parse_ls_tree(&bytes).expect("generated listing is valid")
        })
    }

    #[test]
    fn generator_reaches_the_shared_rules() {
        use proptest::strategy::ValueTree;
        use proptest::test_runner::TestRunner;

        let mut runner = TestRunner::default();
        let strategy = arbitrary_listing();
        let draws = 200;
        let reaching = (0..draws)
            .filter(|_| {
                let listing = strategy
                    .new_tree(&mut runner)
                    .expect("strategy produces a value")
                    .current();
                listing.paths().any(|path| {
                    ["a", "b", "c"]
                        .iter()
                        .any(|prefix| rel_starts_with(path, prefix))
                })
            })
            .count();
        assert!(
            reaching * 2 >= draws,
            "only {reaching} of {draws} draws reached a/, b/, or c/"
        );
    }

    fn rel_starts_with(path: &RelPath, prefix: &str) -> bool {
        let text = path.as_str();
        text == prefix || text.starts_with(&format!("{prefix}/"))
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
            let has_no_divergence = report
                .findings
                .iter()
                .all(|finding| !matches!(finding, crate::check::Finding::Diverged { .. }));
            prop_assert!(has_no_divergence);
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
