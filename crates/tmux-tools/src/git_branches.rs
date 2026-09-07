//! The one `git for-each-ref` call behind `list-branches`, replacing the
//! widget's `git branch | rg | sed | sed` stages with a single spawn.
//!
//! `git branch --all --format=...` and `git for-each-ref --format=...
//! refs/heads refs/remotes` walk the same ref set in the same order for the
//! same `--sort`, but `for-each-ref` also exposes the untruncated
//! `%(refname)`, which is how this module tells a local branch apart from a
//! remote-tracking one without re-deriving it from the short name.

use std::process::Command;

use tmux_core::BranchRef;

/// Lists every local and remote-tracking branch, most recently committed
/// first, matching the widget's `git branch --sort=-committerdate --all`.
///
/// Returns an empty list when `git` is missing, the current directory is
/// not a repository, or the call otherwise fails: `list-branches` then
/// prints nothing, which leaves the picker with an empty candidate list
/// rather than a spurious error breaking the widget's pipeline.
#[must_use]
pub fn list() -> Vec<BranchRef> {
    let output = Command::new("git")
        .args([
            "for-each-ref",
            "--format=%(refname)%09%(refname:short)",
            "--sort=-committerdate",
            "refs/heads",
            "refs/remotes",
        ])
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_ref_line)
        .collect()
}

/// Parses one `%(refname)<TAB>%(refname:short)` line into a [`BranchRef`].
///
/// `is_remote` comes from the full `refname`, which still carries the
/// `refs/remotes/` prefix that `refname:short` strips, rather than from a
/// guess based on the short name's shape.
fn parse_ref_line(line: &str) -> Option<BranchRef> {
    let (full_name, short_name) = line.split_once('\t')?;
    Some(BranchRef {
        name: short_name.to_string(),
        is_remote: full_name.starts_with("refs/remotes/"),
    })
}
