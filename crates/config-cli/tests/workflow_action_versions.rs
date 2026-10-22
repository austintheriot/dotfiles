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

/// One `uses:` reference, with the text of the workflow it came from so a
/// SHA pin's version comment can be read: the YAML parser drops comments.
struct ActionUse {
    file: String,
    job: String,
    reference: String,
    workflow_text: String,
}

impl ActionUse {
    fn label(&self) -> String {
        format!("{}/{}: {}", self.file, self.job, self.reference)
    }
}

/// Every `uses:` in every workflow.
fn all_step_uses() -> Vec<ActionUse> {
    workflow_files()
        .iter()
        .flat_map(|path| {
            let file = file_name(path);
            let workflow_text = std::fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()));
            step_uses(&dotfiles_test_support::repo::parse_workflow(&workflow_text))
                .into_iter()
                .map(move |(job, reference)| ActionUse {
                    file: file.clone(),
                    job,
                    reference,
                    workflow_text: workflow_text.clone(),
                })
                .collect::<Vec<ActionUse>>()
        })
        .collect()
}

/// Local actions are versioned by the commit that holds them, and Docker
/// references carry their own tag.
fn is_remote(reference: &str) -> bool {
    !reference.starts_with("./") && !reference.starts_with("docker://")
}

fn is_commit_sha(git_ref: &str) -> bool {
    git_ref.len() == 40
        && git_ref
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}

/// The `# vX.Y.Z` comment that names the release a SHA pin points at. Every
/// line that carries the reference must carry the same comment, so a pin
/// with no comment anywhere, or with two that disagree, has no version.
fn version_comment(workflow_text: &str, reference: &str) -> Option<String> {
    let comments: Vec<Option<&str>> = workflow_text
        .lines()
        .filter(|line| line.contains(reference))
        .map(|line| {
            let (_, comment) = line.split_once(reference)?;
            let version = comment.trim().strip_prefix('#')?.trim();
            let digits = version.strip_prefix('v')?;
            let well_formed = !digits.is_empty()
                && digits.split('.').all(|part| {
                    !part.is_empty() && part.chars().all(|digit| digit.is_ascii_digit())
                });
            well_formed.then_some(version)
        })
        .collect();
    let first = (*comments.first()?)?;
    comments
        .iter()
        .all(|comment| *comment == Some(first))
        .then(|| first.to_string())
}

/// The reference as `name@vMAJOR`, the shape `DEPRECATED_PINS` holds. A SHA
/// pin takes its major version from its comment.
fn major_pin(workflow_text: &str, reference: &str) -> Option<String> {
    let (name, git_ref) = reference.split_once('@')?;
    if !is_commit_sha(git_ref) {
        return Some(reference.to_string());
    }
    let version = version_comment(workflow_text, reference)?;
    let major = version.split('.').next()?;
    Some(format!("{name}@{major}"))
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
        .filter(|action| {
            major_pin(&action.workflow_text, &action.reference)
                .is_some_and(|pin| DEPRECATED_PINS.contains(&pin.as_str()))
        })
        .map(ActionUse::label)
        .collect();
    assert!(
        offenders.is_empty(),
        "these steps pin an action whose major version declares a retired \
         Node runtime: {offenders:?}"
    );
}

/// A tag is a pointer its owner can move, and this repository is public, so a
/// moved tag would run someone else's code in every workflow. A commit SHA
/// cannot move.
#[test]
fn every_remote_action_is_pinned_to_a_commit_sha() {
    let found = all_step_uses();
    assert!(
        !found.is_empty(),
        "positive control: no uses: values were read"
    );

    let unpinned: Vec<String> = found
        .iter()
        .filter(|action| is_remote(&action.reference))
        .filter(|action| {
            !action
                .reference
                .split_once('@')
                .is_some_and(|(_, git_ref)| is_commit_sha(git_ref))
        })
        .map(ActionUse::label)
        .collect();
    assert!(
        unpinned.is_empty(),
        "these steps name an action by a movable ref instead of a full commit \
         SHA: {unpinned:?}"
    );
}

/// Without the comment a reader cannot tell which release a SHA is, and the
/// retired-runtime check above has no major version to judge.
#[test]
fn every_sha_pin_names_its_release_in_a_comment() {
    let unnamed: Vec<String> = all_step_uses()
        .iter()
        .filter(|action| is_remote(&action.reference))
        .filter(|action| version_comment(&action.workflow_text, &action.reference).is_none())
        .map(ActionUse::label)
        .collect();
    assert!(
        unnamed.is_empty(),
        "these steps pin a SHA with no `# vX.Y.Z` comment, or with comments \
         that disagree, on every line that carries it: {unnamed:?}"
    );
}

/// The control for the two tests above: a SHA pin is judged by its comment,
/// so a SHA of a retired major is still caught.
#[test]
fn a_sha_pin_takes_its_major_version_from_its_comment() {
    let reference = "actions/checkout@0123456789abcdef0123456789abcdef01234567";
    let text = format!("    steps:\n      - uses: {reference} # v4.2.2\n");
    assert_eq!(
        major_pin(&text, reference).as_deref(),
        Some("actions/checkout@v4")
    );
    let uncommented = format!("    steps:\n      - uses: {reference}\n");
    assert_eq!(version_comment(&uncommented, reference), None);
    let disagreeing =
        format!("      - uses: {reference} # v4.2.2\n      - uses: {reference} # v7.0.1\n");
    assert_eq!(version_comment(&disagreeing, reference), None);
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
