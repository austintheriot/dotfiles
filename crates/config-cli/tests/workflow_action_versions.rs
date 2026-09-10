//! Every third-party action a workflow uses must carry an explicit version,
//! and none may sit on a major version whose Node runtime GitHub has retired.
//!
//! Converted from tests/workflow-action-versions.test.sh. That suite found
//! the deprecated pins with `grep -rn -F` over the whole file and the
//! unpinned ones with
//! `grep -rhoE '^[[:space:]]*-?[[:space:]]*uses:[[:space:]]*[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+[[:space:]]*$'`.
//! Both patterns read lines, not workflows, so both had the same two holes.
//!
//! A commented-out `# - uses: actions/checkout@v4` satisfied the deprecated
//! grep and failed the build over a line that runs nothing. A `uses:` written
//! as a flow-mapping step, or carried on a continuation, matched neither
//! pattern and so was neither pinned nor checked. Parsing the steps closes
//! both: a comment is not a step, and a step's `uses:` is a string whatever
//! the file's line shape.
//!
//! GitHub retires the Node runtime an action declares, then forces the action
//! onto the current runtime and prints a deprecation warning on every run:
//!
//!   Node.js 20 is deprecated. The following actions target Node.js 20 but
//!   are being forced to run on Node.js 24: actions/checkout@v4.
//!
//! The warning is not fatal, but "forced onto a runtime the action was never
//! tested against" is a real compatibility risk, and a warning nobody acts on
//! trains everyone to ignore the CI log.

use dotfiles_test_support::repo::{file_name, root as repo_root, step_uses, workflow_files};

/// Action references whose major version still declares a Node runtime
/// GitHub has retired. Extend this list when GitHub announces the next
/// deprecation.
///
/// `actions/checkout@v5` and later declare node24; v4 and earlier declare
/// node20 or older. Held as whole references rather than as a name and a
/// version range, because the retirement is per action: the next entry will
/// name a different action at a different major.
const DEPRECATED_PINS: [&str; 4] = [
    "actions/checkout@v1",
    "actions/checkout@v2",
    "actions/checkout@v3",
    "actions/checkout@v4",
];

/// Every `uses:` in every workflow, as (file, job, reference).
fn all_step_uses() -> Vec<(String, String, String)> {
    workflow_files()
        .iter()
        .flat_map(|path| {
            let file = file_name(path);
            step_uses(&dotfiles_test_support::repo::read_workflow(path))
                .into_iter()
                .map(move |(job, uses)| (file.clone(), job, uses))
                .collect::<Vec<(String, String, String)>>()
        })
        .collect()
}

/// The directory that holds the workflows exists at all. Without it every
/// other assertion here is about an empty listing.
#[test]
fn the_workflow_directory_is_present() {
    let directory = repo_root().join(".github/workflows");
    assert!(
        directory.is_dir(),
        "no workflow directory at {}",
        directory.display()
    );
}

/// The positive control the other two tests lean on: the parser reached at
/// least one `uses:`, so an empty list of offenders below means "nothing
/// wrong" rather than "nothing read".
#[test]
fn at_least_one_action_reference_is_read() {
    let found = all_step_uses();
    assert!(
        !found.is_empty(),
        "no uses: values were parsed out of any workflow's steps, so every \
         assertion about action pins would pass vacuously"
    );
}

/// A pin on a retired Node runtime earns a deprecation warning on every run.
#[test]
fn no_workflow_pins_an_action_on_a_deprecated_node_runtime() {
    let found = all_step_uses();
    assert!(
        !found.is_empty(),
        "positive control: no uses: values were read"
    );

    let offenders: Vec<String> = found
        .iter()
        .filter(|(_, _, uses)| DEPRECATED_PINS.contains(&uses.as_str()))
        .map(|(file, job, uses)| format!("{file}/{job}: {uses}"))
        .collect();
    assert!(
        offenders.is_empty(),
        "these steps pin an action whose major version declares a retired \
         Node runtime: {offenders:?}"
    );
}

/// Without an explicit version an upstream release silently changes what CI
/// runs.
///
/// A reference is pinned when it names a ref after `@`, or when it is a local
/// path (`./.github/actions/foo`), which is versioned by the commit that
/// contains it. Docker references (`docker://image:tag`) carry their own tag.
#[test]
fn every_action_reference_carries_an_explicit_version() {
    let found = all_step_uses();
    assert!(
        !found.is_empty(),
        "positive control: no uses: values were read"
    );

    let unpinned: Vec<String> = found
        .iter()
        .filter(|(_, _, uses)| !uses.starts_with("./") && !uses.contains('@'))
        .map(|(file, job, uses)| format!("{file}/{job}: {uses}"))
        .collect();
    assert!(
        unpinned.is_empty(),
        "these steps use an action with no version, so an upstream release \
         changes what CI runs: {unpinned:?}"
    );
}

/// The hole the line-shaped grep left: a commented-out step is not a step.
///
/// The old `grep -F 'actions/checkout@v4'` matched inside a comment, so
/// re-adding a disabled line failed the build over something that runs
/// nothing. The parser cannot see a comment at all.
#[test]
fn a_commented_out_step_is_not_a_step() {
    let workflow = dotfiles_test_support::repo::parse_workflow(
        "jobs:\n  one:\n    steps:\n      # - uses: actions/checkout@v4\n      \
         - uses: actions/checkout@v7\n",
    );
    let uses: Vec<String> = step_uses(&workflow)
        .into_iter()
        .map(|(_, reference)| reference)
        .collect();
    assert_eq!(
        uses,
        vec!["actions/checkout@v7".to_string()],
        "a comment is not a step, so a disabled reference must not register"
    );
}

/// The other hole: a step written as a flow mapping matched neither pattern,
/// so its action was checked by nothing.
#[test]
fn a_flow_mapping_step_is_still_read() {
    let workflow = dotfiles_test_support::repo::parse_workflow(
        "jobs:\n  one:\n    steps: [{uses: actions/checkout@v4}]\n",
    );
    let uses: Vec<String> = step_uses(&workflow)
        .into_iter()
        .map(|(_, reference)| reference)
        .collect();
    assert_eq!(
        uses,
        vec!["actions/checkout@v4".to_string()],
        "the line shape is YAML syntax, not content, so it must not change \
         which references the checks see"
    );
}
