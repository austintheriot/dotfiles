//! Branch-list formatting, ported from `zsh-git-widgets.sh`'s Ctrl+G picker.
//!
//! The widget's `git branch --format` output goes through `rg`, two `sed`
//! calls, `fzf`, then `cut` before landing in `LBUFFER`. This module
//! reproduces the non-interactive stages: the `rg` filter, and the two
//! `sed` substitutions. `fzf` itself, and the trailing `cut -f1` that reads
//! the user's picked line back out, stay in the widget, because they are
//! the interactive part this crate holds no IO to run.
//!
//! Measured against `%(refname:short)` output specifically: neither `sed`
//! substitution ever matches that format's text (no leading `* `, no
//! `remotes/` prefix), so this function's only real transformation is the
//! `rg` filter. Both `sed` patterns are reproduced anyway, because the
//! contract is "whatever the shell pipeline emitted," not "whatever the
//! shell pipeline emitted after a human decided which stages actually fire."

/// One ref the widget's `git branch --format` line named, before filtering.
///
/// `name` is `%(refname:short)`: a local branch keeps its bare name, and a
/// remote branch keeps its `remote/branch` form except when the ref is a
/// remote's `HEAD` symref, which `refname:short` collapses to the bare
/// remote name (`origin/HEAD` renders as `origin`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchRef {
    /// The ref's short name, exactly as `%(refname:short)` renders it.
    pub name: String,

    /// Whether this ref lives under `refs/remotes/`.
    ///
    /// Unused by `format_branches` today: the pipeline's `rg` filter and
    /// `sed` substitutions both operate on `name` alone. Carried because
    /// `tmux-tools` already knows it from `git for-each-ref`, and a caller
    /// that needs to tell local and remote refs apart (to color them, for
    /// instance) should not have to re-derive it from the name.
    pub is_remote: bool,
}

/// Formats `refs` the way the widget's shell pipeline formatted them.
///
/// Reproduces the `rg -v 'HEAD|^origin'` filter: a ref is dropped when its
/// name contains `HEAD` anywhere, or when its name starts with `origin`.
/// The second half drops every branch on the `origin` remote, not only its
/// `HEAD` pointer, because that is what the shell's anchored-but-unslashed
/// pattern actually matches (`origin/main` starts with `origin` too).
///
/// Also reproduces both `sed` substitutions (a leading `* ` marker, and a
/// `remotes/<remote>/` prefix), even though `%(refname:short)` never
/// produces either shape: the contract is the pipeline's output, not the
/// subset of the pipeline that happens to have visible effect on today's
/// `--format` string.
#[must_use]
pub fn format_branches(refs: &[BranchRef]) -> Vec<String> {
    refs.iter()
        .map(|reference| reference.name.as_str())
        .filter(|name| !name.contains("HEAD") && !name.starts_with("origin"))
        .map(strip_current_branch_marker)
        .map(|name| strip_remotes_prefix(&name))
        .collect()
}

/// Reproduces `sed 's/^[* ]*//'`: drops any leading run of `*` and space.
fn strip_current_branch_marker(name: &str) -> String {
    name.trim_start_matches(['*', ' ']).to_string()
}

/// Reproduces `sed 's#^remotes/[^/]*/##'`: drops a `remotes/<remote>/`
/// prefix, for whichever remote it names.
fn strip_remotes_prefix(name: &str) -> String {
    let Some(after_remotes) = name.strip_prefix("remotes/") else {
        return name.to_string();
    };
    match after_remotes.split_once('/') {
        Some((_remote, rest)) => rest.to_string(),
        None => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The formatted output matches what the shell pipeline produced.
    ///
    /// Byte for byte, because the widget assigns the chosen line into
    /// LBUFFER and the user runs it: a stray space or a lost prefix becomes
    /// a broken command in the user's shell.
    #[test]
    fn it_formats_a_local_branch_the_way_the_pipeline_did() {
        let refs = vec![BranchRef {
            name: "feature/login".to_string(),
            is_remote: false,
        }];

        let formatted = format_branches(&refs);

        // Positive control: one ref in must produce one line out, or the
        // indexing below is reasoning about an empty vector.
        assert_eq!(formatted.len(), 1, "the control must format one branch");
        assert_eq!(formatted[0], "feature/login");
    }

    /// The `rg -v 'HEAD|^origin'` filter drops the collapsed `origin/HEAD`
    /// symref (which `refname:short` renders as the bare name `origin`),
    /// while `main` survives.
    ///
    /// Verified against a live repository, not assumed from the plan: a
    /// synthetic repo with `refs/remotes/origin/HEAD` pointing at
    /// `refs/remotes/origin/main` renders the symref's `refname:short` as
    /// `origin`, not `origin/HEAD`, so the test's ref name matches what git
    /// actually emits rather than the plan's illustrative string.
    #[test]
    fn it_drops_the_refs_the_pipeline_dropped() {
        let refs = vec![
            BranchRef { name: "main".to_string(), is_remote: false },
            BranchRef { name: "origin".to_string(), is_remote: true },
        ];

        let formatted = format_branches(&refs);

        assert!(
            !formatted.iter().any(|line| line == "origin"),
            "the collapsed origin/HEAD symref was filtered by the pipeline and must stay filtered"
        );
        assert!(formatted.iter().any(|line| line == "main"), "main must survive");
    }

    /// The `^origin` half of the `rg` pattern is not anchored on a slash: it
    /// drops every branch on the `origin` remote, not only `origin`'s `HEAD`
    /// pointer. This disagrees with a narrower reading of the plan ("drop
    /// origin/HEAD"), so it gets its own test pinned to the shell's actual
    /// regex.
    #[test]
    fn it_drops_every_branch_on_the_origin_remote_not_only_its_head() {
        let refs = vec![
            BranchRef { name: "origin/main".to_string(), is_remote: true },
            BranchRef { name: "upstream/feature-x".to_string(), is_remote: true },
        ];

        let formatted = format_branches(&refs);

        assert!(
            !formatted.iter().any(|line| line.starts_with("origin")),
            "origin/main starts with origin, so rg -v '^origin' drops it too"
        );
        assert!(
            formatted.iter().any(|line| line == "upstream/feature-x"),
            "a non-origin remote's branch is not touched by either rg pattern"
        );
    }

    /// A ref whose name merely contains HEAD, not just refs named exactly
    /// HEAD, is dropped: `rg -v 'HEAD'` is a substring match, unanchored.
    #[test]
    fn it_drops_any_ref_whose_name_contains_head() {
        let refs = vec![
            BranchRef { name: "HEAD".to_string(), is_remote: false },
            BranchRef { name: "detached-HEAD-backup".to_string(), is_remote: false },
            BranchRef { name: "main".to_string(), is_remote: false },
        ];

        let formatted = format_branches(&refs);

        assert_eq!(formatted, vec!["main".to_string()]);
    }

    /// A non-origin remote branch passes through with its `remote/branch`
    /// form intact: the widget does not strip remote prefixes in general,
    /// only `remotes/<remote>/` when `%(refname:short)` happened to emit
    /// that shape, which it never does for a real `--format` run.
    #[test]
    fn a_non_origin_remote_branch_keeps_its_remote_prefix() {
        let refs = vec![BranchRef {
            name: "upstream/feature-x".to_string(),
            is_remote: true,
        }];

        assert_eq!(format_branches(&refs), vec!["upstream/feature-x".to_string()]);
    }

    /// The `remotes/<remote>/` sed substitution still fires on a name that
    /// carries that literal prefix, even though `%(refname:short)` never
    /// produces one. Pinning this keeps the substitution honest as
    /// reproduced shell behavior, not dead code nobody noticed was untested.
    #[test]
    fn a_literal_remotes_prefix_is_still_stripped() {
        let refs = vec![BranchRef {
            name: "remotes/upstream/feature-x".to_string(),
            is_remote: true,
        }];

        assert_eq!(format_branches(&refs), vec!["feature-x".to_string()]);
    }

    /// The `* ` current-branch marker sed substitution still fires on a name
    /// that carries it, for the same reason: reproduced because it is part
    /// of the shell's contract, not because `--format` output needs it.
    #[test]
    fn a_leading_current_branch_marker_is_still_stripped() {
        let refs = vec![BranchRef {
            name: "* main".to_string(),
            is_remote: false,
        }];

        assert_eq!(format_branches(&refs), vec!["main".to_string()]);
    }
}
