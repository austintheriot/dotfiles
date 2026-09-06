//! The stamp comparison policy, as a pure function.
//!
//! The stamp *computation* lives in `.scripts/config/config-stamp` and must
//! stay there: `config-build` calls it to decide what to compile, so a Rust
//! owner would be the binary the stamp guards. The comparison is a different
//! value with a different producer. `pre-push` never builds; it consumes two
//! lists and produces a refusal or a pass, which is plan-shaped.

use std::collections::BTreeMap;

use crate::check::Rendered;

/// The freshness of one crate's installed binary against a pushed ref.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StampVerdict {
    Fresh,
    Stale { crate_name: String, built: String, pushed: String },
    /// The installed binary reported no stamp for this crate.
    NotBuilt { crate_name: String },
    /// The build knows this crate and the pushed ref does not.
    Unknown { crate_name: String },
}

/// Compares built stamps against the stamps committed on a ref.
///
/// Keyed by crate name rather than by position, because `config-stamp`
/// iterates the workspace member list while git iterates a tree, and nothing
/// makes those orders agree.
pub fn verify(
    built: &[(String, String)],
    pushed: &[(String, String)],
) -> Vec<StampVerdict> {
    let built_by_name: BTreeMap<&str, &str> = built
        .iter()
        .map(|(name, tree)| (name.as_str(), tree.as_str()))
        .collect();
    let pushed_by_name: BTreeMap<&str, &str> = pushed
        .iter()
        .map(|(name, tree)| (name.as_str(), tree.as_str()))
        .collect();

    let mut names: Vec<&str> = built_by_name.keys().copied().collect();
    names.extend(pushed_by_name.keys().copied());
    names.sort_unstable();
    names.dedup();

    names
        .into_iter()
        .map(|name| match (built_by_name.get(name), pushed_by_name.get(name)) {
            (Some(built_tree), Some(pushed_tree)) if built_tree == pushed_tree => {
                StampVerdict::Fresh
            }
            (Some(built_tree), Some(pushed_tree)) => StampVerdict::Stale {
                crate_name: name.to_string(),
                built: (*built_tree).to_string(),
                pushed: (*pushed_tree).to_string(),
            },
            (None, Some(_)) => StampVerdict::NotBuilt { crate_name: name.to_string() },
            // Fail closed. A crate the build knows and the ref does not is
            // what pushing a new crate without its sources looks like.
            (Some(_), None) => StampVerdict::Unknown { crate_name: name.to_string() },
            (None, None) => unreachable!("a name came from one of the two maps"),
        })
        .collect()
}

/// Renders verdicts to two streams and an exit code.
pub fn render(verdicts: &[StampVerdict]) -> Rendered {
    // An empty comparison is a broken caller, not a pass. This is the shape a
    // member-list parse failure produces, and the whole point of the gate is
    // that it refuses when it cannot answer.
    if verdicts.is_empty() {
        return Rendered {
            stdout: String::new(),
            stderr: "pre-push: no crates were compared, so the stamp gate cannot answer\n"
                .to_string(),
            exit_code: 2,
        };
    }

    let mut stdout = String::new();
    let mut stderr = String::new();

    for verdict in verdicts {
        match verdict {
            StampVerdict::Fresh => {}
            StampVerdict::Stale { crate_name, built, pushed } => stderr.push_str(&format!(
                "pre-push: {crate_name} is stale (built {built}, pushed {pushed}); run `config build`\n"
            )),
            StampVerdict::NotBuilt { crate_name } => stderr.push_str(&format!(
                "pre-push: {crate_name} reported no stamp; run `config build`\n"
            )),
            StampVerdict::Unknown { crate_name } => stderr.push_str(&format!(
                "pre-push: {crate_name} is built here but absent from the pushed ref\n"
            )),
        }
    }

    if stderr.is_empty() {
        stdout.push_str(&format!(
            "pre-push: every stamp matches the pushed ref ({} crate(s))\n",
            verdicts.len()
        ));
        return Rendered { stdout, stderr, exit_code: 0 };
    }
    Rendered { stdout, stderr, exit_code: 1 }
}

/// Splits one `config-stamp` output line into a crate name and its stamp.
///
/// The stamp is `<crate-tree>:<lock-blob>:<workspace-blob>`, an opaque
/// string that itself contains colons. Splitting on the first space only,
/// rather than on whitespace generally or on any colon, is what keeps the
/// stamp opaque: a caller that split on `:` would shred every stamp into
/// three fields and compare none of them correctly.
pub fn parse_stamp_line(line: &str) -> Option<(String, String)> {
    let (name, stamp) = line.split_once(' ')?;
    if name.is_empty() || stamp.is_empty() {
        return None;
    }
    Some((name.to_string(), stamp.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(name: &str, tree: &str) -> (String, String) {
        (name.to_string(), tree.to_string())
    }

    #[test]
    fn matching_stamps_are_fresh() {
        let verdicts = verify(
            &[pair("config-manifest", "aaa"), pair("dotfiles-path", "bbb")],
            &[pair("config-manifest", "aaa"), pair("dotfiles-path", "bbb")],
        );
        assert_eq!(verdicts, vec![StampVerdict::Fresh, StampVerdict::Fresh]);
    }

    #[test]
    fn a_differing_stamp_is_stale_and_names_the_crate() {
        let verdicts = verify(
            &[pair("config-manifest", "aaa")],
            &[pair("config-manifest", "zzz")],
        );
        assert_eq!(
            verdicts,
            vec![StampVerdict::Stale {
                crate_name: "config-manifest".to_string(),
                built: "aaa".to_string(),
                pushed: "zzz".to_string(),
            }]
        );
    }

    // The whole reason this is a set rather than one value: editing one crate
    // must not mark another stale, and a verdict has to say which.
    #[test]
    fn one_stale_crate_does_not_taint_its_neighbour() {
        let verdicts = verify(
            &[pair("config-manifest", "aaa"), pair("dotfiles-path", "bbb")],
            &[pair("config-manifest", "aaa"), pair("dotfiles-path", "zzz")],
        );
        assert_eq!(verdicts[0], StampVerdict::Fresh);
        assert!(matches!(
            &verdicts[1],
            StampVerdict::Stale { crate_name, .. } if crate_name == "dotfiles-path"
        ));
    }

    // A binary that reported no stamp is NotBuilt, not Stale. pre-push tells
    // the reader to run `config build` in that case, and "stale" would send
    // them looking for a source change that does not exist.
    #[test]
    fn a_missing_built_stamp_is_not_built() {
        let verdicts = verify(&[], &[pair("config-manifest", "aaa")]);
        assert_eq!(
            verdicts,
            vec![StampVerdict::NotBuilt { crate_name: "config-manifest".to_string() }]
        );
    }

    // A crate present in the build but absent from the pushed ref is Unknown
    // rather than Fresh. Fail closed: this is what a new crate pushed without
    // its sources looks like, and treating it as Fresh would pass the gate.
    #[test]
    fn a_crate_absent_from_the_ref_is_unknown() {
        let verdicts = verify(&[pair("brand-new", "aaa")], &[]);
        assert_eq!(
            verdicts,
            vec![StampVerdict::Unknown { crate_name: "brand-new".to_string() }]
        );
    }

    // Order must not decide the verdict: config-stamp iterates the member
    // list and git iterates the tree, and nothing guarantees they agree.
    #[test]
    fn input_order_does_not_change_the_verdict() {
        let verdicts = verify(
            &[pair("dotfiles-path", "bbb"), pair("config-manifest", "aaa")],
            &[pair("config-manifest", "aaa"), pair("dotfiles-path", "bbb")],
        );
        assert!(verdicts.iter().all(|verdict| *verdict == StampVerdict::Fresh));
    }

    #[test]
    fn an_empty_build_list_is_not_silently_fresh() {
        // Both empty is the shape a broken member-list sed produces. It must
        // not render as a pass.
        let rendered = render(&verify(&[], &[]));
        assert_ne!(rendered.exit_code, 0, "an empty comparison passed the gate");
    }

    #[test]
    fn fresh_renders_exit_zero() {
        let rendered = render(&[StampVerdict::Fresh]);
        assert_eq!(rendered.exit_code, 0);
    }

    #[test]
    fn stale_renders_nonzero_and_names_the_rebuild_command() {
        let rendered = render(&[StampVerdict::Stale {
            crate_name: "config-manifest".to_string(),
            built: "aaa".to_string(),
            pushed: "zzz".to_string(),
        }]);
        assert_ne!(rendered.exit_code, 0);
        assert!(
            rendered.stderr.contains("config build"),
            "the reader was not told how to fix it: {:?}",
            rendered.stderr
        );
        assert!(rendered.stderr.contains("config-manifest"));
    }

    #[test]
    fn not_built_renders_nonzero_and_names_the_rebuild_command() {
        let rendered = render(&[StampVerdict::NotBuilt {
            crate_name: "config-manifest".to_string(),
        }]);
        assert_ne!(rendered.exit_code, 0);
        assert!(rendered.stderr.contains("config build"));
    }

    // The parse contract this task exists to protect. A stamp is
    // `<crate-tree>:<lock-blob>:<workspace-blob>`, so splitting on the first
    // space and treating the remainder as opaque must hand back the whole
    // colon-bearing string unchanged. Splitting on ':' instead, or taking
    // the second whitespace-delimited token, would shred it.
    #[test]
    fn a_stamp_containing_colons_round_trips_through_the_parse_unchanged() {
        let stamp = "ae9a51fb0be23dd0fbd390782e9db67cdca4fc47:\
                     77d60be85f30081491e56107a29946103c7ff6a9:\
                     aaf8278240ba79d2ccb6f6fb03cd3660cfff9764";
        let line = format!("config-manifest {stamp}");
        let parsed = parse_stamp_line(&line);
        assert_eq!(parsed, Some(("config-manifest".to_string(), stamp.to_string())));
    }
}
