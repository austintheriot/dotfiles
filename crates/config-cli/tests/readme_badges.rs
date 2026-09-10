//! Every CI badge in the README must report on something real.
//!
//! Converted from tests/readme-badges.test.sh. A badge is a URL, and a URL
//! rots silently. Renaming a workflow file, or dropping a branch from a
//! workflow's `on: push:` list, leaves a badge that still renders but reports
//! on nothing: GitHub serves "no status" rather than an error, so the README
//! keeps looking fine while telling the reader less than it claims.
//!
//! The reachable facts are asserted, never the rendered image. The network is
//! never touched, so this file passes offline and in the container.
//!
//! The shell suite read the trigger block with
//! `awk '/^on:/{found=1; next} found && /^[a-z]/{exit} found'` and then asked
//! `grep -q "$branch"` of the result. That matches the branch name anywhere in
//! the block, so `paths: - 'main.rs'` or a comment mentioning main satisfied
//! it while `branches: [release]` sat right above. Reading `on.push.branches`
//! as a list asks the question the assertion meant.

use dotfiles_test_support::repo::{read_workflow, root as repo_root, triggers_of};
use std::path::PathBuf;

/// Branches this repository pushes. One branch: `main`. The `mac` and
/// `linux` pair collapsed, so a badge naming either reports on a branch
/// nothing lands on.
const PUSHED_BRANCHES: [&str; 1] = ["main"];

/// One badge as the README spells it: the workflow file it names and the
/// branch it queries.
struct Badge {
    workflow: String,
    branch: Option<String>,
}

fn readme_path() -> PathBuf {
    repo_root().join("README.md")
}

fn readme_text() -> String {
    std::fs::read_to_string(readme_path()).expect("README.md is readable")
}

/// Every CI badge the README carries, in document order.
///
/// Matched on the badge URL's own shape rather than on the Markdown around
/// it, because the link text, the label and the surrounding prose are all
/// free to change without changing what the badge reports on.
fn badges(readme: &str) -> Vec<Badge> {
    readme
        .split("actions/workflows/")
        .skip(1)
        .filter_map(|rest| {
            let url = rest.split(')').next()?;
            let (workflow, query) = url.split_once("/badge.svg")?;
            let branch = query
                .split_once("branch=")
                .map(|(_, value)| value.split('&').next().unwrap_or_default().to_string());
            Some(Badge {
                workflow: workflow.to_string(),
                branch,
            })
        })
        .collect()
}

/// Branches a workflow's `on: push:` fires for.
fn push_branches(workflow: &yaml_serde::Value) -> Vec<String> {
    let Some(branches) = triggers_of(workflow)
        .and_then(|triggers| triggers.get("push"))
        .and_then(|push| push.get("branches"))
    else {
        return Vec::new();
    };
    match branches.as_sequence() {
        Some(entries) => entries
            .iter()
            .filter_map(|entry| entry.as_str().map(str::to_string))
            .collect(),
        None => branches
            .as_str()
            .map(|single| vec![single.to_string()])
            .unwrap_or_default(),
    }
}

/// Without a README there is no badge block to check, and the file is tracked,
/// so its absence is a broken checkout rather than a platform difference.
#[test]
fn the_home_readme_exists() {
    let path = readme_path();
    assert!(path.is_file(), "no README at {}", path.display());
}

/// The positive control every other test here leans on: at least one badge is
/// present, so an empty list of offenders means "nothing wrong" rather than
/// "nothing read".
#[test]
fn the_readme_carries_at_least_one_ci_badge() {
    let found = badges(&readme_text());
    assert!(
        !found.is_empty(),
        "the README carries no CI badge, so every assertion about badges \
         would pass vacuously"
    );
}

/// A badge naming a renamed or deleted workflow renders as "no status"
/// forever.
#[test]
fn every_badge_names_a_workflow_file_that_exists() {
    let found = badges(&readme_text());
    assert!(!found.is_empty(), "positive control: badges were found");

    let directory = repo_root().join(".github/workflows");
    let missing: Vec<&String> = found
        .iter()
        .map(|badge| &badge.workflow)
        .filter(|workflow| !directory.join(workflow).is_file())
        .collect();
    assert!(
        missing.is_empty(),
        "these badges name workflow files that do not exist: {missing:?}"
    );
}

/// Without `?branch=`, GitHub reports whatever it considers the default,
/// which is a claim about a branch this README never names. Pinning makes the
/// badge say which branch it reports on.
#[test]
fn every_badge_pins_an_explicit_branch() {
    let found = badges(&readme_text());
    assert!(!found.is_empty(), "positive control: badges were found");

    let unpinned: Vec<&String> = found
        .iter()
        .filter(|badge| badge.branch.is_none())
        .map(|badge| &badge.workflow)
        .collect();
    assert!(
        unpinned.is_empty(),
        "these badges pin no branch, so they report on whatever GitHub calls \
         the default: {unpinned:?}"
    );
}

/// A badge for a branch nothing lands on reports on nothing.
#[test]
fn every_badge_names_a_branch_this_repo_pushes() {
    let found = badges(&readme_text());
    assert!(!found.is_empty(), "positive control: badges were found");

    let unknown: Vec<String> = found
        .iter()
        .filter_map(|badge| badge.branch.as_ref())
        .filter(|branch| !PUSHED_BRANCHES.contains(&branch.as_str()))
        .cloned()
        .collect();
    assert!(
        unknown.is_empty(),
        "these badges name branches this repo does not push: {unknown:?}"
    );
}

/// A badge for a branch the workflow's `on: push:` ignores renders as "no
/// status" forever, which is the rot this suite exists to catch.
#[test]
fn every_badge_workflow_is_triggered_on_the_branch_it_reports() {
    let found = badges(&readme_text());
    assert!(!found.is_empty(), "positive control: badges were found");

    let directory = repo_root().join(".github/workflows");
    let mut checked = 0_usize;
    let mut untriggered: Vec<String> = Vec::new();
    for badge in &found {
        let Some(branch) = badge.branch.as_ref() else {
            continue;
        };
        let path = directory.join(&badge.workflow);
        if !path.is_file() {
            continue;
        }
        checked += 1;
        if !push_branches(&read_workflow(&path)).contains(branch) {
            untriggered.push(format!("{}:{branch}", badge.workflow));
        }
    }
    assert!(
        checked > 0,
        "positive control: no badge named both an existing workflow and a \
         branch, so an empty offender list would mean nothing"
    );
    assert!(
        untriggered.is_empty(),
        "these badges report on a branch their workflow's on: push: ignores: \
         {untriggered:?}"
    );
}

/// The badge block belongs above the first section heading, where a reader
/// sees it before anything else.
#[test]
fn the_badges_appear_above_the_first_section_heading() {
    let readme = readme_text();
    let first_badge = readme
        .lines()
        .position(|line| line.contains("badge.svg"))
        .expect("positive control: the README carries a badge line");
    let first_heading = readme
        .lines()
        .position(|line| line.starts_with("## "))
        .expect("positive control: the README carries a section heading");
    assert!(
        first_badge < first_heading,
        "the badges are below the first section heading (badge line {}, \
         heading line {})",
        first_badge + 1,
        first_heading + 1
    );
}

/// The hole the awk-and-grep pair left: the branch name appearing anywhere in
/// the trigger block satisfied the old check, so a path filter mentioning the
/// branch name kept the assertion green while `branches:` named something
/// else entirely.
#[test]
fn a_branch_named_only_in_a_path_filter_does_not_count_as_a_trigger() {
    let workflow = dotfiles_test_support::repo::parse_workflow(
        "on:\n  push:\n    branches: [release]\n    paths:\n      - 'main.rs'\n",
    );
    assert_eq!(
        push_branches(&workflow),
        vec!["release".to_string()],
        "only on.push.branches names a trigger branch; a path filter is a \
         different question"
    );
}
