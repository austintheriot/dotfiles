//! Lints every tracked shell script, and asserts the lint gates where a push
//! is blocked.
//!
//! Converted from tests/shellcheck.test.sh. The repo carried SC2086
//! suppression comments while shellcheck was neither installed nor a tracked
//! dependency, so those comments were decorative and nothing was linted. This
//! file makes them load-bearing.
//!
//! Do not write a literal suppression comment in this file's prose either.
//! shellcheck 0.9.0, the version in the Debian bookworm test container, reads
//! one inside a comment as a real directive and fails to parse the file,
//! while 0.11.0 ignores it. The container caught exactly that.
//!
//! Dialect matters. shellcheck supports sh, bash, dash and ksh, and rejects
//! zsh outright. The zsh scripts are excluded explicitly, by name, rather
//! than skipped by a silent parse failure: a wrong exclusion should be
//! visible in this file, not inferred from a tool's error message. Every
//! excluded name is asserted to still exist and to still be zsh, so the list
//! cannot rot into excusing a bash script.
//!
//! Version skew is real and deliberately not pinned. The container ships
//! 0.9.0; Homebrew on macOS ships 0.11.0. A newer release adds checks, so the
//! strictest environment decides whether a push passes, and that is the macOS
//! CI job. Findings are fixed at their site rather than suppressed globally.
//!
//! What the conversion changed: the shell suite located the workflow's
//! dependency step with `grep -n 'deps install --yes' -A 2` and then with
//! `awk '/Install the suite.s dependencies \(from deps.toml\)/{found=1} ...'`.
//! Both key off the step's human-readable name and a fixed line offset, so
//! rewording the name, or moving `--only` one line further down, silently
//! stopped reading the step while the assertion stayed green. Reading
//! `jobs.*.steps[]` finds the step by what it runs.

use dotfiles_test_support::repo::{read_workflow, root as repo_root};
use std::path::Path;
use std::process::Command;

/// Scripts shellcheck refuses outright, so they are never linted.
///
/// `tmux-split.sh` is deliberately absent: it became a POSIX shim over
/// `tmux-tools split`, so it carries a `#!/bin/sh` shebang and is linted like
/// any other sh script. The "every zsh exclusion is still a zsh script"
/// assertion below is what caught the dialect change.
const ZSH_SCRIPTS: [&str; 5] = [
    ".scripts/tmux-close.sh",
    ".scripts/tmux-setup.sh",
    ".scripts/tmux-start.sh",
    ".scripts/zsh-git-widgets.sh",
    "tests/leak-check.sh",
];

/// Files sourced at shell-init time, so they carry no shebang and shellcheck
/// cannot infer their dialect. Linted as bash.
///
/// `depcheck-hook.sh` is sourced from `.zshrc` but is portable POSIX shell,
/// not zsh-specific, so it is linted rather than excluded.
///
/// `tests/lib.sh` was the other entry and left with the shell harness on
/// 2026-09-11.
const SOURCED_BASH: [&str; 1] = ["deps/depcheck-hook.sh"];

/// Codes excluded everywhere, as flags rather than a `.shellcheckrc`: the rc
/// file needs shellcheck 0.10 or later and would have to be copied into the
/// test image, while `-e` works on every version this repo runs against.
///
/// `SC1091` is a sourced path shellcheck cannot resolve. `-x` follows a
/// sourced file where the working directory allows it; the container runs
/// from a different directory and cannot, and that is not a defect.
///
/// `SC2016` fires on single-quoted shell text. This repo prints shell
/// commands as text (install instructions the reader runs) and passes shell
/// snippets to another shell or to grep. Both are correctly single-quoted;
/// expanding them is the bug.
const EXCLUDES: &str = "SC1091,SC2016";

/// The floor a broken discovery step has to clear. A lint that checks nothing
/// passes trivially.
///
/// Measured, not guessed: 24 tracked `*.sh` paths minus the 5 zsh exclusions
/// is 19. It was 24 against 29 tracked paths until 2026-09-11, when deleting
/// the shell harness removed five: `tests/lib.sh`, `tests/run-all.sh`, and
/// three `*.test.sh` suites whose subject was the harness itself.
///
/// An earlier value of 25 was above what a broken discovery step could reach
/// and below what a whole-repo walk would reach, so it failed loudly only
/// after discovery was fixed. Keep this at the real count minus the
/// exclusions, never above it.
const MINIMUM_LINTED: usize = 19;

/// True when this run is a place the lint has to gate rather than an ad-hoc
/// host run.
///
/// A missing binary in CI or in the container is a broken gate, so the tests
/// below fail there. Only a host run skips.
fn lint_must_gate() -> bool {
    std::env::var_os("CI").is_some() || Path::new("/.dockerenv").is_file()
}

fn shellcheck_is_installed() -> bool {
    Command::new("shellcheck")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn suite_workflow() -> yaml_serde::Value {
    read_workflow(&repo_root().join(".github/workflows/test-suite.yml"))
}

/// The workflow step that installs the suite's deps.toml dependencies, as its
/// whole `run:` script.
///
/// Found by what the step runs, not by its display name and not by a line
/// offset from it. The shell suite used both, so rewording the name or moving
/// `--only` one line down stopped it reading the step at all.
///
/// Two steps in this workflow run `deps install --yes`: this one and the
/// harness one that reads deps-ci.toml. `--only` is what tells them apart,
/// because only the deps.toml call subsets its manifest. Matching on
/// `deps install --yes` alone would find whichever came first in the file, so
/// reordering the two steps would silently point every assertion below at
/// the harness step.
fn deps_install_step(workflow: &yaml_serde::Value) -> Option<&yaml_serde::Value> {
    dotfiles_test_support::repo::jobs_of(workflow)
        .into_iter()
        .filter_map(|(_, job)| job.get("steps")?.as_sequence())
        .flatten()
        .find(|step| {
            step.get("run")
                .and_then(yaml_serde::Value::as_str)
                .is_some_and(|script| {
                    script.contains("deps install --yes") && script.contains("--only")
                })
        })
}

/// Every tracked `*.sh` path, relative to the root.
///
/// Tracked files only: the worktree is the whole home directory, so a plain
/// walk would visit every vendored plugin and cache in it. The container has
/// no repository, so it falls back to the directories this repo owns.
///
/// The pathspec has to be rooted with `:/` and the output asked for with
/// `--full-name`. A bare `*.sh` is relative to the current directory, and
/// cargo runs an integration test from the crate directory, so it resolves
/// against `crates/config-cli` and matches nothing. That made the whole
/// branch dead on every run: it returned zero paths, fell through to the
/// walk below, and the walk covers neither `deps/` nor the root, so ten
/// tracked scripts were never linted. Measured from `crates/config-cli`:
/// `ls-files "*.sh"` returns 0, `ls-files --full-name ":/*.sh"` returns 29.
fn shell_scripts(root: &Path) -> Vec<String> {
    let tracked = Command::new("git")
        .arg(format!("--git-dir={}", root.join(".cfg").display()))
        .arg(format!("--work-tree={}", root.display()))
        .args(["ls-files", "--full-name", ":/*.sh"])
        .output();
    if let Ok(output) = tracked
        && output.status.success()
    {
        let listing = String::from_utf8_lossy(&output.stdout);
        let mut found: Vec<String> = listing
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect();
        if !found.is_empty() {
            found.sort();
            return found;
        }
    }
    let mut found = Vec::new();
    // The same set `ls-files` returns, because the two paths must agree.
    //
    // `tests/run-in-docker.sh` builds the container tree with `git archive`,
    // so there is no repository there and `ls-files` cannot run: the walk is
    // the container's only discovery path, and the container is the strictest
    // gate this repo has.
    //
    // The list read `.scripts`, `tests`, `.claude/hooks` and so missed every
    // script under `deps/` plus `setup.sh` at the root: ten files, which is
    // exactly the set the rooted-pathspec fix in `f28cfed2` added on the host.
    // Left as it was, the host would lint 24 and the container 14 against one
    // floor, and the container would fail on a gap rather than on a defect.
    for directory in [".scripts", "tests", ".claude/hooks", "deps"] {
        collect_scripts(root, &root.join(directory), &mut found);
    }
    // Root-level scripts are not in any of those directories. `setup.sh` is
    // the bootstrap entry point and had never been linted by either path.
    collect_root_scripts(root, &mut found);
    found.sort();
    found
}

/// The `*.sh` files directly in the repository root.
fn collect_root_scripts(root: &Path, found: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension == "sh")
            && let Ok(relative) = path.strip_prefix(root)
            && let Some(name) = relative.to_str()
        {
            found.push(name.to_string());
        }
    }
}

fn collect_scripts(root: &Path, directory: &Path, found: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect_scripts(root, &path, found);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("sh")
            && let Ok(relative) = path.strip_prefix(root)
            && let Some(text) = relative.to_str()
        {
            found.push(text.to_string());
        }
    }
}

/// The first line of a file, or an empty string.
fn first_line(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .next()
        .unwrap_or_default()
        .to_string()
}

/// A lint suite that exists but never runs where a push is blocked is
/// decorative, which is the failure this whole item was filed against. Three
/// places have to carry shellcheck, and each is asserted rather than assumed.
#[test]
fn shellcheck_is_a_tracked_dependency() {
    let manifest =
        std::fs::read_to_string(repo_root().join("deps/deps.toml")).expect("deps.toml is readable");
    assert!(
        manifest.lines().any(|line| line.trim() == "[shellcheck]"),
        "deps/deps.toml declares no [shellcheck] entry, so `config install` \
         does not provide the linter"
    );
}

/// Without it in the image the pre-push gate runs no lint.
#[test]
fn the_test_container_installs_shellcheck() {
    let dockerfile = std::fs::read_to_string(repo_root().join("tests/docker/Dockerfile"))
        .expect("the Dockerfile is readable");
    assert!(
        dockerfile.contains("shellcheck"),
        "tests/docker/Dockerfile never names shellcheck, so the container \
         runs the suite with no linter"
    );
}

/// The positive control the next two tests lean on: the step that installs
/// the deps.toml dependencies was located at all.
#[test]
fn the_deps_install_step_is_present_in_the_workflow() {
    let workflow = suite_workflow();
    assert!(
        deps_install_step(&workflow).is_some(),
        "no step in test-suite.yml runs `deps install --yes`, so every \
         assertion about that step would pass vacuously"
    );
}

/// CI has to install the linter through the deps engine, so the package name
/// lives in deps.toml alone rather than in a hand-written per-platform list
/// the workflow copy of which was the one that drifted.
#[test]
fn ci_installs_shellcheck_through_the_deps_engine() {
    let workflow = suite_workflow();
    let step = deps_install_step(&workflow).expect("positive control: the step was found");
    let script = step
        .get("run")
        .and_then(yaml_serde::Value::as_str)
        .expect("the step runs a script");
    assert!(
        script.contains("shellcheck"),
        "the deps install step's --only set does not name shellcheck, so CI \
         runs the lint with no linter; it runs: {script}"
    );
}

/// Both runners, not just one. A lint that gates on Linux and not macOS lets
/// a macOS-only script regress, and an `if: runner.os == ...` on that step
/// would restore exactly that gap.
#[test]
fn the_dependency_step_runs_on_both_runners() {
    let workflow = suite_workflow();
    let step = deps_install_step(&workflow).expect("positive control: the step was found");
    let condition = step.get("if").and_then(yaml_serde::Value::as_str);
    assert!(
        condition.is_none(),
        "the deps install step is gated on `if: {}`, so the lint runs on one \
         platform and a script that only exists on the other regresses \
         unlinted",
        condition.unwrap_or_default()
    );
}

/// A rename would otherwise leave a dead entry in an exclusion list and
/// silently drop a real script from the lint.
#[test]
fn every_excluded_script_still_exists() {
    let root = repo_root();
    let excluded: Vec<&str> = ZSH_SCRIPTS.iter().chain(SOURCED_BASH.iter()).copied().collect();
    assert!(
        !excluded.is_empty(),
        "positive control: the exclusion lists are empty"
    );

    let dead: Vec<&str> = excluded
        .iter()
        .filter(|script| !root.join(script).is_file())
        .copied()
        .collect();
    assert!(
        dead.is_empty(),
        "these excluded scripts no longer exist, so the exclusion excuses \
         nothing and may be hiding a rename: {dead:?}"
    );
}

/// If a zsh exclusion is rewritten in bash it belongs in the lint, and this
/// is what says so. It is what caught tmux-split.sh's dialect change.
#[test]
fn every_zsh_exclusion_is_still_a_zsh_script() {
    let root = repo_root();
    assert!(
        !ZSH_SCRIPTS.is_empty(),
        "positive control: the zsh exclusion list is empty"
    );

    let not_zsh: Vec<&str> = ZSH_SCRIPTS
        .iter()
        .filter(|script| root.join(script).is_file())
        .filter(|script| {
            let path = root.join(script);
            if first_line(&path).contains("zsh") && first_line(&path).starts_with("#!") {
                return false;
            }
            // A sourced zsh file has no shebang; zsh-only syntax marks it.
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            !(text.contains("zle ")
                || text.contains("autoload -Uz")
                || text.contains("zmodload")
                || text.lines().any(|line| line.starts_with("emulate ")))
        })
        .copied()
        .collect();
    assert!(
        not_zsh.is_empty(),
        "these scripts are excluded as zsh but read as another dialect, so \
         they belong in the lint: {not_zsh:?}"
    );
}

/// Once a sourced file grows a shebang, shellcheck infers the dialect and the
/// explicit override is stale.
#[test]
fn every_sourced_bash_override_is_still_shebang_less() {
    let root = repo_root();
    assert!(
        !SOURCED_BASH.is_empty(),
        "positive control: the sourced-bash list is empty"
    );

    let with_shebang: Vec<&str> = SOURCED_BASH
        .iter()
        .filter(|script| root.join(script).is_file())
        .filter(|script| first_line(&root.join(script)).starts_with("#!"))
        .copied()
        .collect();
    assert!(
        with_shebang.is_empty(),
        "these scripts now carry a shebang, so the explicit dialect override \
         is stale: {with_shebang:?}"
    );
}

/// The positive control for the lint itself: discovery found scripts.
#[test]
fn the_discovery_step_finds_shell_scripts() {
    let found = shell_scripts(&repo_root());
    assert!(
        !found.is_empty(),
        "no *.sh files were discovered, so a clean lint run would mean \
         nothing was checked"
    );
}

/// The lint. An absent linter is a skip on a host run and a failure wherever
/// the lint has to gate.
#[test]
fn shellcheck_reports_no_findings() {
    if !shellcheck_is_installed() {
        assert!(
            !lint_must_gate(),
            "shellcheck is not installed, and this run is CI or the test \
             container, where the lint has to gate. A missing linter there is \
             a broken gate, not a platform difference."
        );
        dotfiles_test_support::skip("shellcheck is not installed; `config install` adds it");
        return;
    }

    let root = repo_root();
    let scripts = shell_scripts(&root);
    assert!(
        !scripts.is_empty(),
        "positive control: no *.sh files were discovered"
    );

    let mut linted = 0_usize;
    let mut findings: Vec<String> = Vec::new();
    for script in &scripts {
        if ZSH_SCRIPTS.contains(&script.as_str()) {
            continue;
        }
        let path = root.join(script);
        if !path.is_file() {
            continue;
        }
        let mut command = Command::new("shellcheck");
        command.current_dir(&root).args(["-x", "-e", EXCLUDES]);
        if SOURCED_BASH.contains(&script.as_str()) {
            command.args(["-s", "bash"]);
        }
        let output = command
            .args(["-f", "gcc", script])
            .output()
            .expect("shellcheck runs");
        linted += 1;
        let reported = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let reported = reported.trim();
        if !reported.is_empty() {
            findings.push(reported.to_string());
        }
    }

    assert!(
        linted >= MINIMUM_LINTED,
        "the lint covered only {linted} scripts, below the floor of \
         {MINIMUM_LINTED}, so a broken discovery step would read as a clean run"
    );
    assert!(
        findings.is_empty(),
        "shellcheck reports findings:\n{}",
        findings.join("\n")
    );
}
