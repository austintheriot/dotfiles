//! The Rust gate is wired, its skip path is loud, and the lint policy it
//! enforces is actually declared.
//!
//! The gate exists because a push once printed `SKIP cargo test (cargo not
//! found)` and passed: the container leg is Rust-free by design, so the
//! container can never run these checks. A test that only asserted the
//! checks pass would not catch the gate being removed from the hook, which
//! is the failure that actually happened.
//!
//! Converted whole from `tests/rust-gate.test.sh`, which ran **25**
//! assertions on this machine from 12 `assert_*` call sites: two sit in
//! loops, over the six member manifests and over the ten crate roots. The
//! call-site count undercounts by more than half here, which is why the
//! executed count is what this file records.
//!
//! THIS CONVERSION RETIRES `python3-yaml` FROM THE CONTAINER. This suite and
//! `workflow-shell-quoting` were the last two importers of the python yaml
//! module, named as the package's remaining justification by the comment
//! corrected in `fd3b4435`. `workflow-shell-quoting` converted earlier in
//! this group, so with this file the package and its comment leave
//! `tests/docker/Dockerfile` in the same commit. The workflow is parsed here
//! with `yaml_serde`, the parser the other converted suites already use.
//!
//! TWO ASSERTIONS ARE NOT CARRIED OVER, and neither is dropped silently.
//! The shell suite asserted that `run-all.sh` runs clippy and that the
//! clippy leg goes through `run_suite` so it is counted. Their subject is
//! `tests/run-all.sh`, which Tranche C's Task 6 deletes outright, and the
//! Rust harness has no `run_suite` to route a leg through: `rust-checks.sh`
//! runs clippy directly and the pre-push hook calls it. The surviving half
//! of that concern, that clippy runs at all and gates before merge, is
//! asserted below against `rust-checks.sh` and `test-suite.yml`, both of
//! which outlive the shell harness. Task 5 is where the rest of the
//! harness-only assertions get classified; these two are recorded here so
//! they are not simply lost.

use dotfiles_test_support::repo::{self, root as repo_root};
use dotfiles_test_support::skip;
use std::path::{Path, PathBuf};
use std::process::Command;

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()))
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|data| data.permissions().mode() & 0o111 != 0)
}

#[test]
fn the_checks_script_exists_and_is_executable() {
    let script = repo_root().join("tests/rust-checks.sh");
    assert!(
        is_executable(&script),
        "{} must exist and be executable, or the hook calls nothing",
        script.display()
    );
}

/// The hook is the only thing that makes these checks a gate rather than a
/// script nobody runs. Asserted against the hook text because driving a real
/// push from a test would push.
#[test]
fn the_hook_calls_the_checks_before_the_container_suite() {
    let hook = read(&repo_root().join("tests/pre-push"));
    assert!(
        hook.contains("rust-checks.sh"),
        "tests/pre-push does not call rust-checks.sh, so the gate is not a gate"
    );

    // Ordering matters: the Rust checks are seconds and the Docker suite is
    // minutes, so a Rust failure must not wait behind a container build.
    //
    // Matched against the INVOCATION SHAPE, `"$HOME/tests/<script>.sh"`, not
    // the bare filename. A bare-filename match also hits prose comments that
    // mention either script, so a comment could flip this assertion without
    // touching how the hook actually runs. That happened once: a header
    // comment mentioning "rust-checks.sh" before "run-in-docker.sh" made the
    // assertion pass regardless of the real call order, and rewording the
    // comment would have made it fail regardless too.
    let line_of = |needle: &str| {
        hook.lines()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("tests/pre-push never invokes {needle}"))
    };
    let rust_line = line_of("$HOME/tests/rust-checks.sh");
    let docker_line = line_of("$HOME/tests/run-in-docker.sh");
    assert!(
        rust_line < docker_line,
        "rust-checks.sh is invoked at line {rust_line} and run-in-docker.sh at \
         line {docker_line}; the seconds-long check must not wait behind the \
         minutes-long container build"
    );
}

/// A machine without Rust can still push shell changes, but the skip has to
/// be loud. A gate that says nothing when it skips is indistinguishable from
/// a gate that is not installed.
#[test]
fn an_absent_cargo_skips_loudly_rather_than_blocking_or_passing_silently() {
    let script = repo_root().join("tests/rust-checks.sh");
    let output = Command::new(&script)
        .arg("HEAD")
        // PATH emptied so `command -v cargo` fails.
        .env("PATH", "/nonexistent")
        .output()
        .expect("rust-checks.sh runs");

    assert!(
        output.status.success(),
        "an absent cargo blocked the push; a machine without Rust must still \
         be able to push shell changes"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("SKIP"),
        "an absent cargo passed silently rather than printing SKIP: {combined}"
    );
}

/// Parsed rather than grepped: a reformat of the workflow must not produce a
/// false pass or a false failure.
#[test]
fn the_ci_workflow_runs_clippy() {
    let workflow_path = repo_root().join(".github/workflows/test-suite.yml");
    let workflow = repo::read_workflow(&workflow_path);
    let scripts = repo::run_scripts(&workflow);

    assert!(
        !scripts.is_empty(),
        "positive control: no `run:` steps were parsed out of {}, so a clean \
         search would prove nothing",
        workflow_path.display()
    );
    assert!(
        scripts.contains("cargo clippy"),
        "test-suite.yml does not run cargo clippy, so the lint invariant does \
         not gate before merge"
    );
}

/// The checks script must run clippy itself, which is the half of the old
/// `run-all.sh` assertions that survives the shell harness. See the module
/// doc.
#[test]
fn the_checks_script_runs_clippy() {
    let script = read(&repo_root().join("tests/rust-checks.sh"));
    assert!(
        script.contains("cargo clippy"),
        "tests/rust-checks.sh does not run cargo clippy, so the pre-push gate \
         enforces nothing about lints"
    );
}

/// Every workspace member manifest.
fn member_manifests(crates: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(crates) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("Cargo.toml"))
        .filter(|path| path.is_file())
        .collect();
    found.sort();
    found
}

/// Every crate root that can carry the panic-family carve-out.
fn crate_roots(crates: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(crates) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .flat_map(|entry| {
            let source = entry.path().join("src");
            [source.join("lib.rs"), source.join("main.rs")]
        })
        .filter(|path| path.is_file())
        .collect();
    found.sort();
    found
}

/// The lint POLICY, not just the gate that enforces it.
///
/// Without these, the policy is held only by clippy currently passing:
/// delete `[lints] workspace = true` from one member manifest, or drop a
/// `cfg_attr` line, and clippy still exits 0 with fewer lints while the
/// suite still reports every assertion green. That exact scenario was
/// verified before the shell suite added these: removing config-manifest's
/// opt-in left clippy at exit 0 and the suite at 8 passed.
///
/// Skipped where there is no `crates/`, which is the test container: the
/// Dockerfile copies the workspace into its BUILDER stage only, then ships
/// just the binary. `skip` rather than an early return, so the container run
/// reports the coverage it did not exercise instead of silently passing.
#[test]
fn the_lint_policy_is_declared_and_every_member_opts_in() {
    let crates = repo_root().join("crates");
    let workspace_manifest = crates.join("Cargo.toml");
    if !workspace_manifest.is_file() {
        skip("lint policy assertions (no crates/ in this environment)");
        return;
    }

    let workspace = read(&workspace_manifest);
    assert!(
        workspace.contains("[workspace.lints.rust]"),
        "crates/Cargo.toml does not declare the rust lint policy"
    );
    assert!(
        workspace.contains("[workspace.lints.clippy]"),
        "crates/Cargo.toml does not declare the clippy lint policy"
    );
    // forbid rather than deny, so a local #[allow] cannot lift it.
    assert!(
        workspace.contains(r#"unsafe_code = "forbid""#),
        "unsafe_code must be forbidden, not merely denied: a local #[allow] \
         can lift a deny"
    );

    // Every member must opt in, or the workspace policy reaches nothing.
    let manifests = member_manifests(&crates);
    assert!(
        !manifests.is_empty(),
        "positive control: no member manifests found under {}",
        crates.display()
    );
    for manifest in &manifests {
        let name = manifest
            .parent()
            .and_then(Path::file_name)
            .and_then(|value| value.to_str())
            .unwrap_or("a member");
        // Matched as the TABLE, not the bare value: `workspace = true` also
        // appears on every dependency line, so the value alone matched a
        // dependency and passed even with the [lints] table deleted.
        assert!(
            read(manifest).contains("[lints]"),
            "{name} does not opt into the workspace lints, so the policy \
             reaches none of its code"
        );
    }

    // Every crate root must carry the panic-family carve-out. Cargo cannot
    // express "deny in src, allow in cfg(test)" through [workspace.lints],
    // so this lives per root, and config-manifest needs it on BOTH roots:
    // doctor.rs is a module of lib.rs, so a main.rs attribute cannot reach
    // it.
    let roots = crate_roots(&crates);
    assert!(
        !roots.is_empty(),
        "positive control: no crate roots found under {}",
        crates.display()
    );
    for root in &roots {
        let label = root.strip_prefix(&crates).unwrap_or(root);
        assert!(
            read(root).contains("cfg_attr(not(test), deny("),
            "{} does not deny unwrap and expect outside tests",
            label.display()
        );
    }
}
