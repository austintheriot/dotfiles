use std::collections::BTreeSet;

use crate::manifest::{Classification, Manifest, PathPattern};
use crate::path::RelPath;
use crate::tree::TreeListing;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentOn {
    A,
    B,
    Both,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    Diverged {
        rule: PathPattern,
    },
    Unmatched {
        path: RelPath,
        present_on: PresentOn,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckReport {
    pub findings: Vec<Finding>,
    pub shared_rules_checked: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: u8,
}

pub fn check(manifest: &Manifest, listing_a: &TreeListing, listing_b: &TreeListing) -> CheckReport {
    let union: BTreeSet<&RelPath> = listing_a.paths().chain(listing_b.paths()).collect();
    let mut findings = Vec::new();
    let mut shared_rules_checked = 0;

    for rule in manifest.shared_rules() {
        shared_rules_checked += 1;
        let diverged = union.iter().any(|path| {
            rule.matches(path)
                && manifest.classify(path) != Classification::Excluded
                && listing_a.get(path) != listing_b.get(path)
        });
        if diverged {
            findings.push(Finding::Diverged { rule: rule.clone() });
        }
    }

    let partition = manifest.partition(union.iter().copied());
    for path in partition.unmatched {
        let present_on = match (
            listing_a.get(&path).is_some(),
            listing_b.get(&path).is_some(),
        ) {
            (true, true) => PresentOn::Both,
            (true, false) => PresentOn::A,
            (false, true) => PresentOn::B,
            (false, false) => continue,
        };
        findings.push(Finding::Unmatched { path, present_on });
    }

    CheckReport {
        findings,
        shared_rules_checked,
    }
}

/// Reproduces tests/check-branch-drift.sh byte for byte, including which
/// stream each line goes to: pre-push, branch-drift.yml and deps-harness
/// grep this text.
pub fn render(report: &CheckReport, ref_a: &str, ref_b: &str) -> Rendered {
    let mut stdout = String::new();
    let mut stderr = String::new();

    let diverged: Vec<&PathPattern> = report
        .findings
        .iter()
        .filter_map(|finding| match finding {
            Finding::Diverged { rule } => Some(rule),
            Finding::Unmatched { .. } => None,
        })
        .collect();
    let unmatched: Vec<(&RelPath, PresentOn)> = report
        .findings
        .iter()
        .filter_map(|finding| match finding {
            Finding::Unmatched { path, present_on } => Some((path, *present_on)),
            Finding::Diverged { .. } => None,
        })
        .collect();

    for rule in &diverged {
        stdout.push_str(&format!("diverged: {rule}\n"));
    }

    if !unmatched.is_empty() {
        stderr.push_str(&format!(
            "\ncheck-branch-drift: {} tracked file(s) match no .sync-manifest rule\n\n",
            unmatched.len()
        ));
        for (path, present_on) in &unmatched {
            let refs = match present_on {
                PresentOn::A => ref_a.to_string(),
                PresentOn::B => ref_b.to_string(),
                PresentOn::Both => format!("{ref_a}, {ref_b}"),
            };
            stderr.push_str(&format!("  {:<44} (on {refs})\n", path.as_str()));
        }
        stderr.push_str("\nEvery tracked file must match a rule, so a file added on one branch\n");
        stderr.push_str("cannot escape this check. Add one of these to .sync-manifest:\n\n");
        stderr.push_str(&format!(
            "  {:<44} shared: must be identical on both branches\n",
            "path/to/file"
        ));
        stderr.push_str(&format!(
            "  {:<44} per-branch: tracked, never compared\n",
            "~path/to/file"
        ));
        stderr.push_str(&format!(
            "  {:<44} excluded from an enclosing shared path\n\n",
            "!path/to/file"
        ));
        return Rendered {
            stdout,
            stderr,
            exit_code: 1,
        };
    }

    if !diverged.is_empty() {
        stderr.push_str(&format!(
            "\ncheck-branch-drift: {ref_a} and {ref_b} diverged on {} of {} shared path(s)\n",
            diverged.len(),
            report.shared_rules_checked
        ));
        return Rendered {
            stdout,
            stderr,
            exit_code: 1,
        };
    }

    stdout.push_str(&format!(
        "check-branch-drift: {ref_a} and {ref_b} match on all {} shared path(s)\n",
        report.shared_rules_checked
    ));
    Rendered {
        stdout,
        stderr,
        exit_code: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest;
    use crate::tree::parse_ls_tree;

    const ID_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const ID_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const ID_M: &str = "cccccccccccccccccccccccccccccccccccccccc";

    fn listing(entries: &[(&str, &str, &str)]) -> TreeListing {
        let mut bytes = Vec::new();
        for (mode, id, path) in entries {
            bytes.extend_from_slice(format!("{mode} blob {id}\t{path}\0").as_bytes());
        }
        parse_ls_tree(&bytes).expect("valid listing")
    }

    fn manifest_of(text: &str) -> Manifest {
        manifest::parse(text).expect("valid manifest")
    }

    #[test]
    fn identical_shared_paths_are_clean_and_counted() {
        let manifest = manifest_of(".sync-manifest\nshared.txt\n");
        let listing_a = listing(&[
            ("100644", ID_M, ".sync-manifest"),
            ("100644", ID_A, "shared.txt"),
        ]);
        let report = check(&manifest, &listing_a, &listing_a);
        assert!(report.findings.is_empty());
        assert_eq!(report.shared_rules_checked, 2);
        let rendered = render(&report, "mac", "linux");
        assert_eq!(
            rendered.stdout,
            "check-branch-drift: mac and linux match on all 2 shared path(s)\n"
        );
        assert_eq!(rendered.stderr, "");
        assert_eq!(rendered.exit_code, 0);
    }

    #[test]
    fn a_differing_blob_under_a_shared_rule_is_diverged() {
        let manifest = manifest_of(".sync-manifest\nshared.txt\n");
        let listing_a = listing(&[
            ("100644", ID_M, ".sync-manifest"),
            ("100644", ID_A, "shared.txt"),
        ]);
        let listing_b = listing(&[
            ("100644", ID_M, ".sync-manifest"),
            ("100644", ID_B, "shared.txt"),
        ]);
        let report = check(&manifest, &listing_a, &listing_b);
        assert_eq!(report.findings.len(), 1);
        let rendered = render(&report, "mac", "linux");
        assert_eq!(rendered.stdout, "diverged: shared.txt\n");
        assert_eq!(
            rendered.stderr,
            "\ncheck-branch-drift: mac and linux diverged on 1 of 2 shared path(s)\n"
        );
        assert_eq!(rendered.exit_code, 1);
    }

    #[test]
    fn a_mode_change_alone_is_diverged() {
        let manifest = manifest_of("run.sh\n");
        let listing_a = listing(&[("100644", ID_A, "run.sh")]);
        let listing_b = listing(&[("100755", ID_A, "run.sh")]);
        assert_eq!(check(&manifest, &listing_a, &listing_b).findings.len(), 1);
    }

    #[test]
    fn a_directory_rule_diverges_on_added_removed_or_changed_files_beneath() {
        let manifest = manifest_of("dir/\n");
        let listing_a = listing(&[("100644", ID_A, "dir/x.txt")]);
        let listing_b = listing(&[("100644", ID_A, "dir/x.txt"), ("100644", ID_B, "dir/y.txt")]);
        let report = check(&manifest, &listing_a, &listing_b);
        assert_eq!(render(&report, "a", "b").stdout, "diverged: dir/\n");
    }

    #[test]
    fn an_excluded_path_inside_a_shared_directory_may_differ() {
        let manifest = manifest_of("dir/\n!dir/platform.txt\n");
        let listing_a = listing(&[
            ("100644", ID_A, "dir/common.txt"),
            ("100644", ID_A, "dir/platform.txt"),
        ]);
        let listing_b = listing(&[
            ("100644", ID_A, "dir/common.txt"),
            ("100644", ID_B, "dir/platform.txt"),
        ]);
        let report = check(&manifest, &listing_a, &listing_b);
        assert!(report.findings.is_empty());
        assert_eq!(report.shared_rules_checked, 1);
    }

    #[test]
    fn a_per_branch_path_is_neither_compared_nor_unmatched() {
        let manifest = manifest_of(".sync-manifest\n~per-branch.txt\n");
        let listing_a = listing(&[
            ("100644", ID_M, ".sync-manifest"),
            ("100644", ID_A, "per-branch.txt"),
        ]);
        let listing_b = listing(&[
            ("100644", ID_M, ".sync-manifest"),
            ("100644", ID_B, "per-branch.txt"),
        ]);
        let report = check(&manifest, &listing_a, &listing_b);
        assert!(report.findings.is_empty());
        assert_eq!(report.shared_rules_checked, 1);
    }

    #[test]
    fn an_unmatched_file_on_either_side_is_reported_with_its_refs() {
        let manifest = manifest_of(".sync-manifest\n");
        let listing_a = listing(&[
            ("100644", ID_M, ".sync-manifest"),
            ("100644", ID_A, "only-a.txt"),
        ]);
        let listing_b = listing(&[
            ("100644", ID_M, ".sync-manifest"),
            ("100644", ID_B, "only-b.txt"),
            ("100644", ID_A, "only-a.txt"),
        ]);
        let report = check(&manifest, &listing_a, &listing_b);
        assert_eq!(
            report.findings,
            vec![
                Finding::Unmatched {
                    path: RelPath::parse("only-a.txt").expect("valid"),
                    present_on: PresentOn::Both
                },
                Finding::Unmatched {
                    path: RelPath::parse("only-b.txt").expect("valid"),
                    present_on: PresentOn::B
                },
            ]
        );
        let rendered = render(&report, "origin/mac", "origin/linux");
        assert_eq!(rendered.stdout, "");
        assert_eq!(
            rendered.stderr,
            "\ncheck-branch-drift: 2 tracked file(s) match no .sync-manifest rule\n\n\
\x20\x20only-a.txt                                   (on origin/mac, origin/linux)\n\
\x20\x20only-b.txt                                   (on origin/linux)\n\
\nEvery tracked file must match a rule, so a file added on one branch\n\
cannot escape this check. Add one of these to .sync-manifest:\n\n\
\x20\x20path/to/file                                 shared: must be identical on both branches\n\
\x20\x20~path/to/file                                per-branch: tracked, never compared\n\
\x20\x20!path/to/file                                excluded from an enclosing shared path\n\n"
        );
        assert_eq!(rendered.exit_code, 1);
    }

    #[test]
    fn diverged_lines_print_before_the_unmatched_block_and_unmatched_decides_the_summary() {
        let manifest = manifest_of(".sync-manifest\nshared.txt\n");
        let listing_a = listing(&[
            ("100644", ID_M, ".sync-manifest"),
            ("100644", ID_A, "shared.txt"),
            ("100644", ID_A, "stray.txt"),
        ]);
        let listing_b = listing(&[
            ("100644", ID_M, ".sync-manifest"),
            ("100644", ID_B, "shared.txt"),
        ]);
        let rendered = render(&check(&manifest, &listing_a, &listing_b), "mac", "linux");
        assert_eq!(rendered.stdout, "diverged: shared.txt\n");
        assert!(
            rendered
                .stderr
                .contains("1 tracked file(s) match no .sync-manifest rule")
        );
        assert!(!rendered.stderr.contains("diverged on"));
        assert_eq!(rendered.exit_code, 1);
    }

    #[test]
    fn a_file_sharing_a_directory_rules_name_is_unmatched() {
        let manifest = manifest_of(".sync-manifest\ndir/\n");
        let listing_a = listing(&[("100644", ID_M, ".sync-manifest"), ("100644", ID_A, "dir")]);
        let report = check(&manifest, &listing_a, &listing_a);
        assert_eq!(
            report.findings,
            vec![Finding::Unmatched {
                path: RelPath::parse("dir").expect("valid"),
                present_on: PresentOn::Both
            }]
        );
        let rendered = render(&report, "mac", "linux");
        assert_eq!(rendered.exit_code, 1);
    }

    #[test]
    fn a_lookalike_path_is_not_absorbed_by_an_exact_rule() {
        let manifest = manifest_of(".sync-manifest\nfile.txt\n");
        let listing_a = listing(&[
            ("100644", ID_M, ".sync-manifest"),
            ("100644", ID_A, "file.txt"),
            ("100644", ID_A, "file.txt.bak"),
        ]);
        let report = check(&manifest, &listing_a, &listing_a);
        assert_eq!(report.findings.len(), 1);
        assert!(render(&report, "a", "b").stderr.contains("file.txt.bak"));
    }
}
