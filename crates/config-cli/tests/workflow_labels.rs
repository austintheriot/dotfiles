//! Every workflow and job must carry a readable label, and the suite workflow
//! must run the test suite on both platforms.
//!
//! Converted from tests/workflow-labels.test.sh. That suite shelled out to an
//! inline Python parser at line 50 to do nothing more than filter `.yml` and
//! `.yaml`, and to a second one at line 120 to read the matrix runners,
//! because a whole-file grep matched the human-readable `label:` strings too
//! and stayed green after the macOS runner was deleted. The shell had no
//! parser, so it borrowed one, and then skipped ten assertions wherever no
//! interpreter was on PATH. `serde_yaml` needs no interpreter, so those ten
//! skips are gone rather than ported.
//!
//! GitHub falls back to the filename when a workflow has no `name:`, and to
//! the job key when a job has no `name:`. A workflow with neither reads as
//! its bare filename with a single job named after its key in the Actions
//! sidebar, which says nothing about what ran or why it matters.
//!
//! Job keys are deliberately not asserted on: tests/deps-harness.test.sh
//! reads jobs by key (`jobs['macos']`), so the keys are an interface. This
//! file checks the display names layered on top of them.

use std::path::{Path, PathBuf};

/// The checkout whose workflows are under test.
///
/// The repository root, at run time.
///
/// `DOTFILES_ROOT` first, then `HOME`, matching `nvim_lua_units.rs` and
/// `nvim_config_load.rs`. The manifest walk-up is the last resort and exists
/// for the `rust-checks.sh` snapshot, where neither variable points at the
/// archived tree.
///
/// Not `CARGO_MANIFEST_DIR` alone: that is a compile-time constant, so a
/// binary built in one tree and run against another reads the wrong root,
/// which is how these tests passed on the host and failed under the gate.
fn repo_root() -> PathBuf {
    for variable in ["DOTFILES_ROOT", "HOME"] {
        if let Some(value) = std::env::var_os(variable) {
            let candidate = PathBuf::from(value);
            if candidate.join("crates/Cargo.toml").is_file() {
                return candidate;
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the repo root is two levels above config-cli")
        .to_path_buf()
}

/// Every workflow file, both extensions. The shell suite reached for an
/// inline Python parser at line 50 to do exactly this.
fn workflow_files(root: &Path) -> Vec<PathBuf> {
    let directory = root.join(".github/workflows");
    let Ok(entries) = std::fs::read_dir(&directory) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("yml" | "yaml")
            )
        })
        .collect();
    found.sort();
    found
}

fn parse_workflow(text: &str) -> serde_yaml::Value {
    serde_yaml::from_str(text).expect("a workflow parses as YAML")
}

fn read_workflow(path: &Path) -> serde_yaml::Value {
    let text = std::fs::read_to_string(path).expect("a workflow file is readable");
    parse_workflow(&text)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .expect("a workflow path has a UTF-8 file name")
        .to_string()
}

/// The jobs mapping of one workflow, as (key, job) pairs.
fn jobs_of(workflow: &serde_yaml::Value) -> Vec<(String, &serde_yaml::Value)> {
    let Some(jobs) = workflow.get("jobs").and_then(serde_yaml::Value::as_mapping) else {
        return Vec::new();
    };
    jobs.iter()
        .filter_map(|(key, job)| key.as_str().map(|key| (key.to_string(), job)))
        .collect()
}

/// The parsed workflow whose job is to run the suite.
fn suite_workflow_path() -> PathBuf {
    repo_root().join(".github/workflows/test-suite.yml")
}

/// Every `runner:` value named by the suite workflow's matrix.
///
/// Read out of the parsed matrix rather than grepped from the file. The shell
/// suite's comment at line 117 records why: a whole-file grep also matches
/// the human-readable `label:` strings, so deleting the macOS entry from the
/// matrix left the grep green.
fn matrix_runners(workflow: &serde_yaml::Value) -> Vec<String> {
    let mut found: Vec<String> = jobs_of(workflow)
        .iter()
        .filter_map(|(_, job)| job.get("strategy")?.get("matrix")?.get("include"))
        .filter_map(serde_yaml::Value::as_sequence)
        .flatten()
        .filter_map(|entry| entry.get("runner")?.as_str().map(str::to_string))
        .collect();
    found.sort();
    found.dedup();
    found
}

/// Every `run:` script in the suite workflow's steps, concatenated.
///
/// Scoped to the steps rather than the whole file, so a mention of a script
/// inside a comment cannot satisfy an assertion that the workflow runs it.
fn suite_run_scripts(workflow: &serde_yaml::Value) -> String {
    jobs_of(workflow)
        .iter()
        .filter_map(|(_, job)| job.get("steps")?.as_sequence())
        .flatten()
        .filter_map(|step| step.get("run")?.as_str())
        .collect::<Vec<&str>>()
        .join("\n")
}

/// The directory that holds the workflows exists at all. Without it every
/// other assertion here is about an empty listing.
#[test]
fn the_workflow_directory_exists() {
    let directory = repo_root().join(".github/workflows");
    assert!(
        directory.is_dir(),
        "no workflow directory at {}",
        directory.display()
    );
}

/// The positive control every other test in this file leans on: at least one
/// `.yml` or `.yaml` file is there, so an empty result below means "nothing
/// wrong" rather than "nothing read".
#[test]
fn at_least_one_workflow_is_present() {
    let found = workflow_files(&repo_root());
    assert!(
        !found.is_empty(),
        "no .yml or .yaml files under .github/workflows, so every assertion \
         about workflow contents would pass vacuously"
    );
}

/// GitHub falls back to the filename when a workflow declares no `name:`, so
/// an unnamed workflow reads as its bare filename in the Actions sidebar.
#[test]
fn every_workflow_has_a_name() {
    let found = workflow_files(&repo_root());
    assert!(!found.is_empty(), "positive control: workflows were found");

    let unnamed: Vec<String> = found
        .iter()
        .filter(|path| {
            read_workflow(path)
                .get("name")
                .and_then(serde_yaml::Value::as_str)
                .is_none_or(str::is_empty)
        })
        .map(|path| file_name(path))
        .collect();
    assert!(
        unnamed.is_empty(),
        "these workflows declare no name: {unnamed:?}"
    );
}

/// A name equal to the filename stem, or a single word, is the opaque case:
/// it gives the reader nothing the filename did not. That is what produced
/// the "branch-drift.yml" heading in the Actions sidebar.
#[test]
fn no_workflow_name_merely_restates_its_filename() {
    let found = workflow_files(&repo_root());
    assert!(!found.is_empty(), "positive control: workflows were found");

    let terse: Vec<String> = found
        .iter()
        .filter_map(|path| {
            let name = read_workflow(path)
                .get("name")
                .and_then(serde_yaml::Value::as_str)?
                .trim()
                .to_string();
            let file = file_name(path);
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default();
            (name == stem || !name.contains(' ')).then(|| format!("{file}:{name}"))
        })
        .collect();
    assert!(
        terse.is_empty(),
        "these workflow names restate the filename or are a single word: \
         {terse:?}"
    );
}

/// GitHub falls back to the job key when a job declares no `name:`, and the
/// keys are terse because tests/deps-harness.test.sh reads jobs by key.
#[test]
fn every_job_has_a_name() {
    let found = workflow_files(&repo_root());
    assert!(!found.is_empty(), "positive control: workflows were found");

    let mut jobs_seen = 0_usize;
    let mut unnamed: Vec<String> = Vec::new();
    for path in &found {
        let workflow = read_workflow(path);
        for (key, job) in jobs_of(&workflow) {
            jobs_seen += 1;
            let named = job
                .get("name")
                .and_then(serde_yaml::Value::as_str)
                .is_some_and(|name| !name.trim().is_empty());
            if !named {
                unnamed.push(format!("{}/{key}", file_name(path)));
            }
        }
    }
    assert!(
        jobs_seen > 0,
        "positive control: no jobs were read at all, so an empty unnamed list \
         would mean nothing"
    );
    assert!(unnamed.is_empty(), "these jobs declare no name: {unnamed:?}");
}

/// Before this workflow existed, no workflow ran tests/run-all.sh at all: the
/// suite only ever ran from the local pre-push hook, so a green push meant
/// "Docker was running on the author's laptop", not "CI verified this".
#[test]
fn a_workflow_runs_the_test_suite() {
    let path = suite_workflow_path();
    assert!(
        path.is_file(),
        "no suite workflow at {}",
        path.display()
    );
}

/// A suite workflow that no push triggers verifies nothing about what lands
/// on the default branch.
#[test]
fn the_suite_workflow_runs_on_push() {
    let workflow = read_workflow(&suite_workflow_path());
    // `on` is the YAML 1.1 boolean `true`, which is why this reads the key
    // both ways: serde_yaml resolves the bare word before the mapping is
    // ours to inspect.
    let triggers = workflow
        .get("on")
        .or_else(|| workflow.get(serde_yaml::Value::Bool(true)))
        .expect("the suite workflow declares triggers");
    assert!(
        triggers.get("push").is_some(),
        "the suite workflow declares no push trigger; its triggers are \
         {triggers:?}"
    );
}

/// The workflow must run the suite runner itself, not a subset of it.
#[test]
fn the_suite_workflow_runs_run_all_sh() {
    let workflow = read_workflow(&suite_workflow_path());
    let scripts = suite_run_scripts(&workflow);
    assert!(
        !scripts.is_empty(),
        "positive control: the suite workflow has no run: scripts at all"
    );
    assert!(
        scripts.contains("run-all.sh"),
        "no step in the suite workflow runs run-all.sh; its scripts are \
         {scripts}"
    );
}

/// Linux is where the container harness is exercised; GitHub's macOS runners
/// ship no Docker daemon, so dropping the Linux leg skips the container
/// suites entirely.
#[test]
fn the_matrix_includes_a_linux_runner() {
    let workflow = read_workflow(&suite_workflow_path());
    let runners = matrix_runners(&workflow);
    assert!(
        !runners.is_empty(),
        "positive control: no runner: values were read out of the matrix"
    );
    assert!(
        runners.iter().any(|runner| runner == "ubuntu-latest"),
        "the matrix names no ubuntu-latest runner; it names {runners:?}"
    );
}

/// macOS reaches notify.test.sh, which drives .claude/hooks/notify.sh, and
/// the Darwin-gated Alacritty app-bundle check in deps-manifest.test.sh.
/// Deleting this entry once left a whole-file grep green.
#[test]
fn the_matrix_includes_a_macos_runner() {
    let workflow = read_workflow(&suite_workflow_path());
    let runners = matrix_runners(&workflow);
    assert!(
        !runners.is_empty(),
        "positive control: no runner: values were read out of the matrix"
    );
    assert!(
        runners.iter().any(|runner| runner == "macos-latest"),
        "the matrix names no macos-latest runner; it names {runners:?}"
    );
}

/// A matrix job that stops at the first failing platform hides whether the
/// other one is also broken, which is the whole point of running both.
#[test]
fn the_matrix_does_not_fail_fast() {
    let workflow = read_workflow(&suite_workflow_path());
    let strategies: Vec<&serde_yaml::Value> = jobs_of(&workflow)
        .iter()
        .filter_map(|(_, job)| job.get("strategy"))
        .collect();
    assert!(
        !strategies.is_empty(),
        "positive control: the suite workflow declares no matrix strategy"
    );
    for strategy in strategies {
        assert_eq!(
            strategy.get("fail-fast").and_then(serde_yaml::Value::as_bool),
            Some(false),
            "the suite workflow's matrix must set fail-fast: false, so one \
             platform's failure does not hide the other's state"
        );
    }
}
