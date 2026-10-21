//! The agent-notify gate: `tools/agent-notify` is its own Cargo workspace
//! with a Bun UI and Bun installer tests, so the `crates/` checks never reach
//! it. `tests/tools-checks.sh` runs them at pre-push and
//! `.github/workflows/agent-notify.yml` runs them in CI, both only when a push
//! touches a path that build reads.

use dotfiles_test_support::repo::{self, root as repo_root};
use dotfiles_test_support::skip;
use std::path::{Path, PathBuf};
use std::process::Command;

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()))
}

fn checks_script() -> PathBuf {
    repo_root().join("tests/tools-checks.sh")
}

fn workflow_path() -> PathBuf {
    repo_root().join(".github/workflows/agent-notify.yml")
}

fn tools_trigger_paths() -> String {
    read(&repo_root().join("tests/pre-push"))
        .lines()
        .map(str::trim)
        .find_map(|line| {
            line.strip_prefix("TOOLS_TRIGGER_PATHS='")?
                .strip_suffix('\'')
                .map(str::to_string)
        })
        .expect("TOOLS_TRIGGER_PATHS is defined in tests/pre-push")
}

fn matches_pattern(pattern: &str, path: &str) -> bool {
    use std::io::Write;
    let mut child = Command::new("grep")
        .args(["-Eq", pattern])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("grep runs");
    if let Some(mut stdin) = child.stdin.take() {
        writeln!(stdin, "{path}").expect("grep reads the path");
    }
    child.wait().expect("grep exits").success()
}

/// The paths in the installer's `STAMP_PATHS`, which name what the daemon
/// build reads.
fn stamp_paths() -> Vec<String> {
    let plan = read(&repo_root().join("tools/agent-notify/scripts/install-plan.ts"));
    let line = plan
        .lines()
        .find(|line| line.contains("STAMP_PATHS"))
        .expect("install-plan.ts declares STAMP_PATHS");
    let list = line.split('[').nth(1).expect("STAMP_PATHS is an array literal");
    list.split(']')
        .next()
        .unwrap_or_default()
        .split(',')
        .map(|entry| entry.trim().trim_matches('"').to_string())
        .filter(|entry| !entry.is_empty())
        .collect()
}

/// Every `path = "..."` dependency of a tools workspace member, as a
/// repository-relative directory.
fn tools_path_dependencies() -> Vec<String> {
    let root = repo_root();
    let workspace = root.join("tools/agent-notify");
    let mut found: Vec<String> = ["daemon", "app"]
        .iter()
        .flat_map(|member| {
            let manifest_directory = workspace.join(member);
            read(&manifest_directory.join("Cargo.toml"))
                .lines()
                .filter_map(|line| {
                    let after = line.split("path = \"").nth(1)?;
                    Some(after.split('"').next()?.to_string())
                })
                .map(|relative| {
                    let joined = manifest_directory.join(relative);
                    let resolved = std::fs::canonicalize(&joined)
                        .unwrap_or_else(|error| panic!("{} resolves: {error}", joined.display()));
                    let canonical_root = std::fs::canonicalize(&root).expect("the root resolves");
                    resolved
                        .strip_prefix(&canonical_root)
                        .expect("a path dependency stays inside the repository")
                        .to_string_lossy()
                        .into_owned()
                })
                .collect::<Vec<_>>()
        })
        .collect();
    found.sort();
    found.dedup();
    found
}

/// One example file for each path the tools checks depend on.
fn paths_the_tools_checks_read() -> Vec<String> {
    let as_example = |path: String| {
        if path.ends_with(".toml") || path.ends_with(".lock") {
            path
        } else {
            format!("{path}/src/lib.rs")
        }
    };
    let mut paths: Vec<String> = stamp_paths()
        .into_iter()
        .chain(tools_path_dependencies())
        .map(as_example)
        .collect();
    paths.push("tools/agent-notify/ui/src/model.ts".to_string());
    paths.push("tests/tools-checks.sh".to_string());
    paths.push(".github/workflows/agent-notify.yml".to_string());
    paths
}

const UNRELATED_PATHS: [&str; 4] = [
    "docs/superpowers/plans/some-plan.md",
    "crates/tmux-tools/src/lib.rs",
    ".zshrc",
    ".github/workflows/test-suite.yml",
];

#[test]
fn the_tools_checks_script_exists_and_is_executable() {
    use std::os::unix::fs::PermissionsExt;
    let script = checks_script();
    let mode = std::fs::metadata(&script)
        .unwrap_or_else(|error| panic!("{} exists: {error}", script.display()))
        .permissions()
        .mode();
    assert!(mode & 0o111 != 0, "{} is not executable", script.display());
}

#[test]
fn the_hook_runs_the_tools_checks_before_the_suite_can_exit_early() {
    let hook = read(&repo_root().join("tests/pre-push"));
    let line_of = |needle: &str| {
        hook.lines()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("tests/pre-push never contains {needle}"))
    };
    let tools_line = line_of("\"$HOME/tests/tools-checks.sh\"");
    let early_exit_line = line_of("skipping the suite");
    assert!(
        tools_line < early_exit_line,
        "tools-checks.sh runs at line {tools_line}, after the suite's early exit at \
         line {early_exit_line}, so a push that touches only tools/ never reaches it"
    );
    assert!(
        hook.contains("grep -qE \"$TOOLS_TRIGGER_PATHS\""),
        "the hook does not gate the tools checks on TOOLS_TRIGGER_PATHS"
    );
}

#[test]
fn the_tools_trigger_matches_every_path_the_tools_checks_read() {
    let pattern = tools_trigger_paths();
    let paths = paths_the_tools_checks_read();
    assert!(
        paths.iter().any(|path| path.starts_with("crates/agent-notify-core")),
        "positive control: no path dependency on agent-notify-core was found in {paths:?}"
    );
    let unmatched: Vec<&String> = paths
        .iter()
        .filter(|path| !matches_pattern(&pattern, path))
        .collect();
    assert!(
        unmatched.is_empty(),
        "TOOLS_TRIGGER_PATHS does not match {unmatched:?}, so a push that edits them skips \
         the agent-notify checks"
    );
}

#[test]
fn the_tools_trigger_ignores_unrelated_paths() {
    let pattern = tools_trigger_paths();
    let matched: Vec<&str> = UNRELATED_PATHS
        .into_iter()
        .filter(|path| matches_pattern(&pattern, path))
        .collect();
    assert!(
        matched.is_empty(),
        "TOOLS_TRIGGER_PATHS matches {matched:?}, so unrelated pushes pay for the Tauri build"
    );
}

/// GitHub's `paths` filter, for the two shapes the workflow uses: a
/// directory ending in `/**` and an exact file.
fn filter_covers(filter: &str, path: &str) -> bool {
    match filter.strip_suffix("/**") {
        Some(directory) => path.starts_with(&format!("{directory}/")),
        None => filter == path,
    }
}

fn path_filters(workflow: &yaml_serde::Value, event: &str) -> Vec<String> {
    repo::triggers_of(workflow)
        .and_then(|triggers| triggers.get(event))
        .and_then(|trigger| trigger.get("paths"))
        .and_then(yaml_serde::Value::as_sequence)
        .map(|filters| {
            filters
                .iter()
                .filter_map(|filter| filter.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn the_workflow_filters_on_every_path_the_tools_checks_read() {
    let workflow = repo::read_workflow(&workflow_path());
    for event in ["push", "pull_request"] {
        let filters = path_filters(&workflow, event);
        assert!(
            !filters.is_empty(),
            "agent-notify.yml has no `{event}.paths` filter, so it runs on every {event}"
        );
        let uncovered: Vec<String> = paths_the_tools_checks_read()
            .into_iter()
            .filter(|path| !filters.iter().any(|filter| filter_covers(filter, path)))
            .collect();
        assert!(
            uncovered.is_empty(),
            "agent-notify.yml `{event}.paths` does not cover {uncovered:?}"
        );
        let covered: Vec<&str> = UNRELATED_PATHS
            .into_iter()
            .filter(|path| filters.iter().any(|filter| filter_covers(filter, path)))
            .collect();
        assert!(
            covered.is_empty(),
            "agent-notify.yml `{event}.paths` covers unrelated {covered:?}"
        );
    }
}

#[test]
fn the_workflow_and_the_script_run_every_check() {
    let workflow_scripts = repo::run_scripts(&repo::read_workflow(&workflow_path()));
    let script = read(&checks_script());
    for check in [
        "cargo clippy --locked --all-targets -- -D warnings",
        "cargo test --locked",
        "bun install --frozen-lockfile",
        "bun run build",
        "bun run typecheck",
        "bun test",
    ] {
        assert!(
            workflow_scripts.contains(check),
            "agent-notify.yml does not run `{check}`"
        );
        assert!(script.contains(check), "tests/tools-checks.sh does not run `{check}`");
    }
}

#[test]
fn an_absent_toolchain_skips_loudly() {
    let output = Command::new(checks_script())
        .arg("HEAD")
        .env("PATH", "/nonexistent")
        .output()
        .expect("tools-checks.sh runs");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "an absent toolchain blocked the push: {combined}");
    assert!(combined.contains("SKIP"), "an absent toolchain passed silently: {combined}");
}

// --- the script against stub toolchains -----------------------------------

fn fixture_git(directory: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(arguments)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_PREFIX")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn fixture_repo(parent: &Path, files: &[&str]) -> PathBuf {
    let repository = parent.join("repo");
    for file in files {
        let path = repository.join(file);
        std::fs::create_dir_all(path.parent().expect("a fixture file has a parent"))
            .expect("the fixture directory is creatable");
        std::fs::write(&path, "").expect("the fixture file is writable");
    }
    fixture_git(&repository, &["init", "-q", "."]);
    fixture_git(&repository, &["add", "."]);
    fixture_git(
        &repository,
        &[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "fixture",
        ],
    );
    repository
}

fn write_stub(directory: &Path, name: &str, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    let path = directory.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("the stub is writable");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("the stub is executable");
}

/// Runs the script against a fixture repository, with `cargo` and `bun`
/// replaced by stubs that echo their arguments.
fn tools_checks_with_stubs(files: &[&str], cargo_body: &str) -> (Option<i32>, String) {
    let sandbox = tempfile::Builder::new()
        .prefix("tools-checks")
        .tempdir_in("/tmp")
        .expect("a temporary directory is creatable");
    let bin = sandbox.path().join("bin");
    std::fs::create_dir(&bin).expect("the stub directory is creatable");
    write_stub(&bin, "cargo", cargo_body);
    write_stub(&bin, "bun", "echo \"BUN $* in $(basename \"$PWD\")\"");
    let repository = fixture_repo(sandbox.path(), files);
    let inherited = std::env::var("PATH").unwrap_or_default();
    let output = Command::new(checks_script())
        .arg("HEAD")
        .env("PATH", format!("{}:{inherited}", bin.display()))
        .env("GIT_DIR", repository.join(".git"))
        .env("GIT_WORK_TREE", &repository)
        .env("CARGO_TARGET_DIR", sandbox.path().join("target"))
        .output()
        .expect("tools-checks.sh runs");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.code(), combined)
}

const TOOLS_WORKSPACE: [&str; 2] = ["tools/agent-notify/Cargo.toml", "crates/Cargo.toml"];

fn on_macos_or_skip(reason: &str) -> bool {
    if cfg!(target_os = "macos") {
        return true;
    }
    skip(reason);
    false
}

#[test]
fn a_ref_without_the_tools_workspace_has_nothing_to_check() {
    if !on_macos_or_skip("tools-checks.sh skips before its ref check off macOS") {
        return;
    }
    let (code, output) = tools_checks_with_stubs(&["crates/Cargo.toml"], "echo CARGO_RAN");
    assert_eq!(code, Some(0), "a ref without tools/agent-notify blocked: {output}");
    assert!(!output.contains("CARGO_RAN"), "cargo ran on a ref with no tools workspace: {output}");
    assert!(output.contains("nothing to check"), "the skip was silent: {output}");
}

#[test]
fn a_failing_check_still_runs_the_rest_and_blocks() {
    if !on_macos_or_skip("tools-checks.sh runs its checks on macOS only") {
        return;
    }
    let (code, output) = tools_checks_with_stubs(
        &TOOLS_WORKSPACE,
        "if [ \"$1\" = clippy ]; then exit 1; fi\necho \"CARGO $1\"",
    );
    assert!(output.contains("CARGO test"), "a clippy failure skipped cargo test: {output}");
    assert!(
        output.contains("BUN test in agent-notify"),
        "a clippy failure skipped the installer tests: {output}"
    );
    assert_eq!(code, Some(1), "a failing check did not block the push: {output}");
}

#[test]
fn an_interrupt_stops_the_gate() {
    if !on_macos_or_skip("tools-checks.sh runs its checks on macOS only") {
        return;
    }
    let (code, output) = tools_checks_with_stubs(
        &TOOLS_WORKSPACE,
        "if [ \"$1\" = clippy ]; then exit 130; fi\necho \"CARGO $1\"",
    );
    assert!(!output.contains("CARGO test"), "an interrupt still started cargo test: {output}");
    assert!(
        !output.contains("BUN test in agent-notify"),
        "an interrupt still started the installer tests: {output}"
    );
    assert_eq!(code, Some(130), "an interrupt did not report 130: {output}");
}
