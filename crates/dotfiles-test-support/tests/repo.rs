//! The repository helpers the converted YAML suites share.
//!
//! `workflow_files` is the piece worth a test of its own: the shell suites it
//! replaces filtered `.yml` and `.yaml` by shelling out to an inline Python
//! parser, and a filter that silently drops one extension would leave every
//! caller asserting about a short listing rather than about the workflows.

use std::fs;
use std::path::PathBuf;

/// Both YAML extensions count as a workflow. GitHub accepts either, so a
/// helper that reads only one hides whatever the other spells.
#[test]
fn workflow_files_finds_both_extensions() {
    let directory = tempfile::Builder::new()
        .prefix("repo-helpers-")
        .tempdir()
        .expect("a temp dir");
    let workflows = directory.path().join(".github/workflows");
    fs::create_dir_all(&workflows).expect("the workflow directory is creatable");
    fs::write(workflows.join("one.yml"), "name: One\n").expect("one.yml is writable");
    fs::write(workflows.join("two.yaml"), "name: Two\n").expect("two.yaml is writable");
    fs::write(workflows.join("notes.md"), "not a workflow\n").expect("notes.md is writable");

    let found = dotfiles_test_support::repo::workflow_files_in(directory.path());
    let names: Vec<String> = found
        .iter()
        .filter_map(|path: &PathBuf| path.file_name()?.to_str().map(str::to_string))
        .collect();
    assert_eq!(
        names,
        vec!["one.yml".to_string(), "two.yaml".to_string()],
        "both extensions are workflows and nothing else is"
    );
}

/// A tree with no workflow directory yields an empty listing rather than a
/// panic, so a caller's own positive control is what reports the absence.
#[test]
fn a_tree_without_workflows_yields_an_empty_listing() {
    let directory = tempfile::Builder::new()
        .prefix("repo-helpers-")
        .tempdir()
        .expect("a temp dir");
    assert!(
        dotfiles_test_support::repo::workflow_files_in(directory.path()).is_empty(),
        "no .github/workflows means no workflow files"
    );
}

/// `parse_workflow` hands back a mapping the caller can index, which is the
/// whole reason these suites stopped grepping.
#[test]
fn parse_workflow_reads_a_mapping() {
    let parsed = dotfiles_test_support::repo::parse_workflow("name: Suite\njobs:\n  one: {}\n");
    assert_eq!(
        parsed.get("name").and_then(yaml_serde::Value::as_str),
        Some("Suite"),
        "the top-level name is readable"
    );
}

/// The root the suites read must be a real checkout: `crates/Cargo.toml` is
/// the marker every resolution path checks for.
#[test]
fn root_names_a_checkout() {
    let root = dotfiles_test_support::repo::root();
    assert!(
        root.join("crates/Cargo.toml").is_file(),
        "repo::root() returned {}, which holds no crates/Cargo.toml",
        root.display()
    );
}
