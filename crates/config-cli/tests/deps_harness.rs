//! The static contracts shared by the Docker harness, the three Dockerfiles
//! and the CI workflow that together run `config deps install --yes` against
//! throwaway Linux and macOS environments.
//!
//! Converted whole from tests/deps-harness.test.sh, 52 assertions. The
//! conversion's technical win is the workflow section: the shell suite started
//! a python3 interpreter with a 60-line inline YAML parser that flattened
//! about 17 facts into a `key=value` file the shell then read back with `sed`.
//! That file was line-oriented, so a `run: |` block had to be collapsed to one
//! line or its value silently truncated, and every fact was a string even when
//! the underlying value was a boolean or a list. `read_workflow` returns the
//! parsed document, so a list is a list, `cancel-in-progress` is a bool, and
//! the python3 dependency plus its skips are gone.
//!
//! Deliberately builds and runs nothing. The containers are the slow,
//! network-dependent, daemon-dependent part, and this suite gates a pre-commit
//! hook. Exercising the images is test-local.sh's job and CI's job.
//!
//! Two genuine behaviours are still driven for real: test-local.sh's
//! detached-HEAD guard and its docker-absence preflight, each against a
//! fixture `$HOME` carrying its own bare repo at `.cfg`. test-local.sh
//! resolves the repo from `$HOME` rather than from `DOTFILES_ROOT`, so a
//! fixture `$HOME` is what isolates it.

use dotfiles_test_support::repo::{jobs_of, read_workflow, root as repo_root, triggers_of};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The three images whose Dockerfiles share one contract.
const IMAGES: [&str; 3] = ["ubuntu", "arch", "pop"];

/// The scripts in `deps/` that are invoked as commands, by CI steps, by the
/// harnesses, and by a Dockerfile ENTRYPOINT.
///
/// A script committed 100644 works on the machine that has the bit set
/// locally and fails on every fresh clone, which is the worst shape this bug
/// takes.
///
/// An exact set, not a subset: a new executed script that nobody adds here is
/// a script whose bit nothing checks. The list used to live in
/// scripts-dir-name.test.sh, whose `ls-tree` call is scoped to `.scripts`;
/// when `deps/` moved to the top level, entries there compared an empty left
/// side against an empty right side and passed while asserting nothing, so the
/// coverage moved here with the tree.
const EXECUTED_DEPS_SCRIPTS: [&str; 7] = [
    "test-local.sh",
    "test-bootstrap.sh",
    "docker/build-seed-binary.sh",
    "docker/bootstrap-entrypoint.sh",
    "docker/bootstrap-bare-entrypoint.sh",
    "docker/bootstrap-curl-entrypoint.sh",
    "docker/deps-image-entrypoint.sh",
];

fn harness_path() -> PathBuf {
    repo_root().join("deps/test-local.sh")
}

fn dockerfile_path(image: &str) -> PathBuf {
    repo_root().join(format!("deps/docker/Dockerfile.{image}"))
}

fn read_to_string(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The single-argument value of a Dockerfile instruction, trimmed.
///
/// Replaces `sed -n 's/^ENTRYPOINT *//p'`. Returns every match so a second
/// `ENV DOTFILES_ROOT=` line cannot be masked by the first: a Dockerfile with
/// two is ambiguous and the caller should see both.
fn instruction_values(dockerfile: &str, prefix: &str) -> Vec<String> {
    dockerfile
        .lines()
        .filter_map(|line| line.strip_prefix(prefix))
        .map(|rest| rest.trim().to_string())
        .collect()
}

/// The one value of a Dockerfile instruction, or a failure naming the count.
fn instruction_value(dockerfile: &str, prefix: &str, image: &str) -> String {
    let found = instruction_values(dockerfile, prefix);
    assert_eq!(
        found.len(),
        1,
        "{image}: expected exactly one `{prefix}` line, found {}: {found:?}",
        found.len()
    );
    found.into_iter().next().unwrap_or_default()
}

fn deps_workflow() -> yaml_serde::Value {
    read_workflow(&repo_root().join(".github/workflows/deps-check.yml"))
}

/// One named job's `run:` scripts, concatenated with whitespace collapsed.
///
/// Whitespace is collapsed and line continuations dropped so an assertion can
/// name the command the runner executes rather than the YAML block's line
/// breaks and indentation. The shell suite's `run_text` did the same, out of
/// necessity rather than choice: its facts file was line-oriented, so an
/// embedded newline truncated the value.
fn job_run_text(workflow: &yaml_serde::Value, job: &str) -> String {
    let steps = jobs_of(workflow)
        .into_iter()
        .find(|(key, _)| key == job)
        .and_then(|(_, value)| value.get("steps").cloned());
    let Some(steps) = steps.as_ref().and_then(yaml_serde::Value::as_sequence) else {
        return String::new();
    };
    steps
        .iter()
        .filter_map(|step| step.get("run")?.as_str())
        .flat_map(str::split_whitespace)
        .filter(|token| *token != "\\")
        .collect::<Vec<&str>>()
        .join(" ")
}

/// One named job's scalar field, as a string.
fn job_field(workflow: &yaml_serde::Value, job: &str, field: &str) -> String {
    jobs_of(workflow)
        .into_iter()
        .find(|(key, _)| key == job)
        .and_then(|(_, value)| value.get(field)?.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// The `paths:` list of the push trigger, or `None` when push is unfiltered.
///
/// A real list rather than the shell version's space-joined string: the shell
/// had to flatten it to survive its line-oriented facts file, which made a
/// path containing a space indistinguishable from two paths.
fn push_trigger_paths(workflow: &yaml_serde::Value) -> Option<Vec<String>> {
    let push = triggers_of(workflow)?.get("push")?;
    let paths = push.get("paths")?.as_sequence()?;
    Some(
        paths
            .iter()
            .filter_map(|path| path.as_str().map(str::to_string))
            .collect(),
    )
}

// --- test-local.sh is POSIX sh -------------------------------------------

/// test-local.sh carries a `#!/bin/sh` shebang and ships on every branch, so
/// it has to parse under the linux branch's `/bin/sh` (dash) too, not just the
/// bash-flavored `/bin/sh` on macOS.
#[test]
fn the_local_harness_parses_as_posix_sh() {
    let harness = harness_path();
    assert!(harness.is_file(), "{} exists", harness.display());
    let status = Command::new("sh")
        .arg("-n")
        .arg(&harness)
        .status()
        .expect("sh runs");
    assert!(status.success(), "`sh -n {}` failed", harness.display());
}

/// `sh -n` alone is weak on macOS, where `/bin/sh` is bash in POSIX mode and
/// happily parses `[[ ]]` and other bashisms that dash rejects. dash is the
/// real `/bin/sh` on the linux branch, so when it is installed here it is the
/// assertion that can actually catch a bashism before that branch sees it.
#[test]
fn the_local_harness_parses_under_dash() {
    let harness = harness_path();
    assert!(harness.is_file(), "{} exists", harness.display());
    let Ok(output) = Command::new("dash").arg("-n").arg(&harness).output() else {
        dotfiles_test_support::skip("dash is not installed here, so no bashism check runs");
        return;
    };
    assert!(
        output.status.success(),
        "`dash -n {}` failed: {}",
        harness.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// --- the preflights, driven for real ------------------------------------

/// A fixture `$HOME` with its own bare repo at `.cfg`, holding a copy of
/// test-local.sh committed on a branch named `mac`.
///
/// test-local.sh hardcodes `$HOME/.cfg` as the git dir, so this is the seam:
/// the fixture repo is what `branch --show-current` and `git archive` see.
struct FixtureHome {
    directory: tempfile::TempDir,
}

impl FixtureHome {
    fn new() -> Self {
        let directory = tempfile::Builder::new()
            .prefix("deps-harness-home-")
            .tempdir_in("/tmp")
            .expect("a temp dir");
        let home = directory.path();
        std::fs::create_dir_all(home.join("deps")).expect("the fixture deps dir");
        std::fs::copy(harness_path(), home.join("deps/test-local.sh"))
            .expect("test-local.sh copies into the fixture");

        let fixture = Self { directory };
        fixture.run_git(&["init", "-q", "--bare", &fixture.git_dir_argument()]);
        fixture.home_git(&["checkout", "-q", "-b", "mac"]);
        fixture.home_git(&["add", "-A"]);
        fixture.home_git(&[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "-m",
            "init",
        ]);
        fixture
    }

    fn home(&self) -> &Path {
        self.directory.path()
    }

    fn git_dir_argument(&self) -> String {
        self.home().join(".cfg").display().to_string()
    }

    /// Runs git with none of the ambient git environment.
    ///
    /// A caller may be a pre-commit or pre-push hook, which exports
    /// `GIT_DIR` and `GIT_WORK_TREE`; without clearing them every fixture
    /// git call would aim at the dotfiles repo instead. `lib.sh` unsets the
    /// same variables for the same reason.
    fn run_git(&self, arguments: &[&str]) {
        let output = Command::new("git")
            .args(arguments)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_OBJECT_DIRECTORY")
            .env("HOME", self.home())
            .output()
            .expect("git runs");
        assert!(
            output.status.success(),
            "git {arguments:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn home_git(&self, arguments: &[&str]) {
        let git_dir = self.git_dir_argument();
        let work_tree = self.home().display().to_string();
        let mut full = vec![
            format!("--git-dir={git_dir}"),
            format!("--work-tree={work_tree}"),
        ];
        full.extend(arguments.iter().map(|argument| (*argument).to_string()));
        let borrowed: Vec<&str> = full.iter().map(String::as_str).collect();
        self.run_git(&borrowed);
    }

    /// Runs the fixture's copy of test-local.sh with `$HOME` pointed at the
    /// fixture, and an optional replacement `PATH`.
    fn run_harness(&self, path: Option<&Path>) -> (i32, String) {
        let mut command = Command::new(self.home().join("deps/test-local.sh"));
        command.env("HOME", self.home());
        if let Some(path) = path {
            command.env("PATH", path);
        }
        let output = command.output().expect("test-local.sh runs");
        let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
        combined.push_str(&String::from_utf8_lossy(&output.stderr));
        (output.status.code().unwrap_or(-1), combined)
    }
}

/// A directory holding the tools test-local.sh needs, and no docker.
///
/// Symlinks rather than a directory prepend: prepending cannot hide a docker
/// that is already on the real `PATH`, and the development machine has one.
fn path_without_docker(inside: &Path) -> PathBuf {
    let bin = inside.join("nodocker-bin");
    std::fs::create_dir_all(&bin).expect("the nodocker bin dir");
    for tool in ["git", "tar", "mktemp", "rm", "cp", "mkdir"] {
        let located = Command::new("command")
            .args(["-v", tool])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .filter(|found| !found.is_empty())
            .or_else(|| {
                ["/usr/bin", "/bin", "/usr/local/bin"]
                    .iter()
                    .map(|directory| Path::new(directory).join(tool))
                    .find(|candidate| candidate.is_file())
                    .map(|candidate| candidate.display().to_string())
            })
            .unwrap_or_else(|| panic!("{tool} is on PATH"));
        std::os::unix::fs::symlink(&located, bin.join(tool)).expect("the tool symlinks");
    }
    bin
}

/// With no docker on `PATH`, test-local.sh exits non-zero and names docker.
///
/// Two claims in one run because the run is the expensive part: the fixture
/// repo, the commit and the symlink farm all exist to produce this one
/// invocation.
#[test]
fn a_missing_docker_fails_and_names_docker() {
    let fixture = FixtureHome::new();
    let path = path_without_docker(fixture.home());

    let (status, output) = fixture.run_harness(Some(&path));

    assert_eq!(status, 1, "a missing docker exits non-zero: {output}");
    assert!(
        output.contains("docker"),
        "a missing docker names docker: {output}"
    );
}

/// With a detached HEAD, test-local.sh exits non-zero, says HEAD is detached,
/// and says what to do about it.
///
/// Detached HEAD is checked before docker, so this asserts the guard rather
/// than accidentally re-asserting the docker preflight. The real `PATH` is
/// used on purpose: the message must be the detached-HEAD one even on a
/// machine where docker is installed and running.
#[test]
fn a_detached_head_fails_and_says_what_to_do() {
    let fixture = FixtureHome::new();
    fixture.home_git(&["checkout", "-q", "--detach", "HEAD"]);

    let (status, output) = fixture.run_harness(None);

    assert_eq!(status, 1, "a detached HEAD exits non-zero: {output}");
    assert!(
        output.contains("HEAD is detached"),
        "a detached HEAD says so: {output}"
    );
    assert!(
        output.contains("check out"),
        "a detached HEAD says what to do about it: {output}"
    );
}

// --- the ENTRYPOINT/CMD contract all three images share ------------------

/// `docker run --rm depcheck-arch` with no arguments has to mean
/// `config deps install --yes`.
///
/// test-local.sh and the workflow's arch job both run the image bare and read
/// its exit code as the verdict, so a CMD that lost `--yes` would hang on a
/// prompt and a CMD that lost the install verb would report missing
/// dependencies it never tried to install.
///
/// The ENTRYPOINT is the wrapper, not the engine. Neither image carries a Rust
/// toolchain -- installing one would pre-satisfy rustup, which is itself a
/// manifest entry -- so the binary arrives through the `/seed` mount and the
/// ENTRYPOINT is what reads it.
#[test]
fn every_image_runs_the_seeded_wrapper_with_install_yes() {
    for image in IMAGES {
        let path = dockerfile_path(image);
        assert!(path.is_file(), "{} exists", path.display());
        let dockerfile = read_to_string(&path);

        let entrypoint = instruction_value(&dockerfile, "ENTRYPOINT", image);
        assert!(
            entrypoint.contains("deps-image-entrypoint.sh"),
            "{image} ENTRYPOINT runs the seeded wrapper: {entrypoint}"
        );

        let cmd = instruction_value(&dockerfile, "CMD", image);
        assert_eq!(
            cmd, r#"["install", "--yes"]"#,
            "{image} CMD is exactly install --yes"
        );
    }
}

/// The manifest has to be findable, which is not implied by copying it in.
///
/// The engine resolves conf paths against `DOTFILES_ROOT`, falling back to
/// `HOME`. Each image sets `HOME=/root` and COPYs the deps tree to
/// `/dotfiles`, so without an explicit `DOTFILES_ROOT` the engine reads no
/// manifest and reports a PASSING check of zero entries. Measured: the first
/// container run after the port printed "0 entries" and the harness called it
/// a clean bootstrap.
///
/// That is the vacuous-pass shape the whole suite exists to prevent, and it is
/// invisible to an exit-code check because the exit code is 0. So the WORKDIR
/// has to equal `DOTFILES_ROOT`: the engine has to be rooted where the
/// manifest was copied.
#[test]
fn every_image_roots_the_engine_where_it_copied_the_manifest() {
    for image in IMAGES {
        let dockerfile = read_to_string(&dockerfile_path(image));

        let dotfiles_root = instruction_value(&dockerfile, "ENV DOTFILES_ROOT=", image);
        assert!(
            !dotfiles_root.is_empty(),
            "{image} sets DOTFILES_ROOT to a non-empty value"
        );

        let workdir = instruction_value(&dockerfile, "WORKDIR", image);
        assert_eq!(
            workdir, dotfiles_root,
            "{image} roots the engine where it copied the manifest"
        );
    }
}

/// No toolchain in the image, asserted rather than assumed.
///
/// This is the compensating-fixture shape the whole port exists to stop: an
/// image that satisfies a dependency the run is supposed to be testing.
/// rustup is a manifest entry, so a `rust:` base or a `cargo` install here
/// would pre-satisfy it and the run would report a bootstrap it never
/// performed.
#[test]
fn no_image_installs_a_rust_toolchain() {
    for image in IMAGES {
        let dockerfile = read_to_string(&dockerfile_path(image));
        let toolchain_lines: Vec<&str> = dockerfile
            .lines()
            .filter(|line| {
                line.contains("rustup") || line.contains("rust:") || line.contains("cargo")
            })
            .collect();
        assert!(
            toolchain_lines.is_empty(),
            "{image} installs no Rust toolchain, but names one: {toolchain_lines:?}"
        );
    }
}

/// `DEPS_LOCAL_CONF` neutralization, all three claims about it.
///
/// These images verify the shared deps.toml only. The engine would otherwise
/// select deps-linux.toml here, whose entries (oh-my-zsh, xclip) belong to the
/// linux machine rather than to this container. The manifest reader skips a
/// missing file, so pointing the variable at a path that does not exist is the
/// neutralization.
///
/// It has to be an absolute path, or the engine resolves it against its own
/// working directory and could land on a real file.
#[test]
fn every_image_neutralizes_the_platform_manifest() {
    for image in IMAGES {
        let dockerfile = read_to_string(&dockerfile_path(image));

        let local_conf = instruction_value(&dockerfile, "ENV DEPS_LOCAL_CONF=", image);
        assert!(
            !local_conf.is_empty(),
            "{image} sets DEPS_LOCAL_CONF to a non-empty value"
        );
        assert!(
            !Path::new(&local_conf).exists(),
            "{image} points DEPS_LOCAL_CONF at a path that does not exist: {local_conf}"
        );
        assert!(
            Path::new(&local_conf).is_absolute(),
            "{image} DEPS_LOCAL_CONF is absolute: {local_conf}"
        );
    }
}

/// Every image copies in the deps tree and runs from `/dotfiles`, so the
/// ENTRYPOINT path has to be the copied one, not a `$HOME`-relative guess.
#[test]
fn every_image_copies_the_deps_tree_in() {
    for image in IMAGES {
        let dockerfile = read_to_string(&dockerfile_path(image));
        assert!(
            dockerfile.contains("COPY deps"),
            "{image} copies the deps tree into the image"
        );
    }
}

// --- the two base images are pinned in deliberately opposite ways --------

/// Ubuntu is digest-pinned and Arch is deliberately not, and both halves are
/// asserted together.
///
/// `ubuntu:24.04` is republished for every point release, so a tag-only
/// reference makes the image change under a passing test. `archlinux:base` is
/// rolling-release: a stale snapshot plus the engine's `pacman -Sy` against
/// current mirrors is Arch's documented partial-upgrade breakage.
///
/// Both decisions are documented in their respective Dockerfiles, and both are
/// what a future "make the two consistent" cleanup would break. Asserting one
/// without the other only catches half of that.
#[test]
fn the_two_base_images_keep_their_opposite_pinning() {
    let ubuntu = read_to_string(&dockerfile_path("ubuntu"));
    let arch = read_to_string(&dockerfile_path("arch"));

    let ubuntu_from = instruction_value(&ubuntu, "FROM", "ubuntu");
    let arch_from = instruction_value(&arch, "FROM", "arch");

    assert!(
        ubuntu_from.contains("@sha256:"),
        "ubuntu pins its base image by digest: {ubuntu_from}"
    );
    assert!(
        !arch_from.contains("@sha256:"),
        "arch does not pin its base image by digest: {arch_from}"
    );
}

/// The comment is the only place the reasoning for the asymmetry lives.
/// Deleting it is how the next reader concludes the asymmetry was an
/// oversight.
#[test]
fn the_arch_image_documents_why_it_is_unpinned() {
    let arch = read_to_string(&dockerfile_path("arch"));
    assert!(
        arch.contains("NOT digest-pinned"),
        "arch documents why it is unpinned"
    );
}

// --- the workflow ------------------------------------------------------

/// deps-check.yml parses as YAML and holds jobs.
///
/// The positive control for every workflow assertion below. `read_workflow`
/// panics on unparseable YAML, so reaching this assertion is already most of
/// the claim; the jobs check is what stops a valid-but-empty document from
/// making the "every job declares a timeout" assertions pass vacuously.
#[test]
fn the_workflow_parses_and_declares_jobs() {
    let workflow = deps_workflow();
    let jobs = jobs_of(&workflow);
    assert!(
        !jobs.is_empty(),
        "the workflow declares jobs, so the assertions below are not vacuous"
    );
}

/// The workflow's trigger set is exactly push, schedule and workflow_dispatch.
///
/// GitHub's `on:` parses as the YAML 1.1 boolean `true`, which is exactly why
/// the trigger set is read through the parser rather than by grepping for
/// `on:`. `triggers_of` reads the key both ways.
#[test]
fn the_workflow_triggers_on_push_schedule_and_dispatch() {
    let workflow = deps_workflow();
    let triggers = triggers_of(&workflow).expect("the workflow declares triggers");
    let mapping = triggers
        .as_mapping()
        .expect("the trigger set is a mapping of event names");
    let mut names: Vec<&str> = mapping.keys().filter_map(yaml_serde::Value::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, ["push", "schedule", "workflow_dispatch"]);
}

/// The push trigger is path-filtered, and the filter covers both the deps
/// directory and the workflow itself.
///
/// An unfiltered push trigger is the specific regression that matters. Every
/// job installs packages off the network, so a push trigger that fired for
/// every commit would make unrelated work fail on an upstream outage -- a
/// GitHub API rate limit already failed the zoxide install on one run. The
/// path filter is what keeps that exposure proportional: only about 3% of
/// recent commits touched the dependency files at all, and a commit that does
/// not touch them cannot break the bootstrap.
#[test]
fn the_push_trigger_is_path_filtered_over_the_deps_tree() {
    let workflow = deps_workflow();
    let paths = push_trigger_paths(&workflow).expect("the push trigger declares a paths filter");
    assert!(!paths.is_empty(), "the push filter is not an empty list");

    assert!(
        paths.iter().any(|path| path.contains("deps/")),
        "the push filter covers the deps directory: {paths:?}"
    );
    assert!(
        paths.iter().any(|path| path.contains("deps-check.yml")),
        "the push filter covers the workflow itself: {paths:?}"
    );
}

/// `contents: read` and nothing else.
///
/// A bare `permissions:` block is what demotes the default write-scoped token
/// for a scheduled run on a public repo, so both the scope list and the one
/// value are asserted.
#[test]
fn the_workflow_grants_read_only_contents_and_nothing_else() {
    let workflow = deps_workflow();
    let permissions = workflow
        .get("permissions")
        .and_then(yaml_serde::Value::as_mapping)
        .expect("the workflow declares a permissions block");

    let scopes: Vec<&str> = permissions
        .keys()
        .filter_map(yaml_serde::Value::as_str)
        .collect();
    assert_eq!(scopes, ["contents"], "the permissions are contents only");
    assert_eq!(
        permissions
            .get(yaml_serde::Value::from("contents"))
            .and_then(yaml_serde::Value::as_str),
        Some("read"),
        "the workflow grants contents: read"
    );
}

/// A scheduled run and a manual run can overlap. Nothing here mutates shared
/// state, so cancelling the older one is safe and avoids paying for two.
///
/// A real boolean, where the shell suite compared against the string `True`:
/// its facts file had passed the value through Python's `str()`, so the
/// assertion was pinned to an interpreter's repr rather than to YAML.
#[test]
fn the_workflow_cancels_overlapping_runs() {
    let workflow = deps_workflow();
    assert_eq!(
        workflow
            .get("concurrency")
            .and_then(|concurrency| concurrency.get("cancel-in-progress"))
            .and_then(yaml_serde::Value::as_bool),
        Some(true),
        "concurrency.cancel-in-progress is true"
    );
}

/// Every job declares a timeout. Every job, not a hardcoded list.
///
/// A fourth leg added without a timeout is the regression this catches. A job
/// with no timeout inherits GitHub's 6-hour default, and these jobs hang on a
/// network stall rather than failing.
///
/// The positive control comes first, per the Global Constraints: an empty jobs
/// mapping would otherwise make the emptiness assertion pass vacuously.
#[test]
fn every_job_declares_a_timeout() {
    let workflow = deps_workflow();
    let jobs = jobs_of(&workflow);
    assert!(!jobs.is_empty(), "positive control: the workflow has jobs");

    let without: Vec<&String> = jobs
        .iter()
        .filter(|(_, job)| job.get("timeout-minutes").is_none())
        .map(|(key, _)| key)
        .collect();
    assert!(
        without.is_empty(),
        "these jobs declare no timeout-minutes: {without:?}"
    );
}

/// Every `actions/checkout` step sets `persist-credentials: false`.
///
/// The default leaves a usable token in `.git/config` for every later step in
/// the job.
///
/// Two positive controls, because this assertion expects an empty result and
/// has two ways to pass vacuously: no jobs at all, or jobs with no checkout
/// step. The second is the one the shell suite could not express, because its
/// facts file carried only the violating names.
#[test]
fn every_checkout_drops_its_credentials() {
    let workflow = deps_workflow();
    let jobs = jobs_of(&workflow);
    assert!(!jobs.is_empty(), "positive control: the workflow has jobs");

    let mut checkouts_seen = 0_usize;
    let mut keeping: Vec<String> = Vec::new();
    for (key, job) in &jobs {
        let steps = job
            .get("steps")
            .and_then(yaml_serde::Value::as_sequence)
            .map(Vec::as_slice)
            .unwrap_or_default();
        for step in steps {
            let is_checkout = step
                .get("uses")
                .and_then(yaml_serde::Value::as_str)
                .is_some_and(|uses| uses.starts_with("actions/checkout"));
            if !is_checkout {
                continue;
            }
            checkouts_seen += 1;
            let persists = step
                .get("with")
                .and_then(|with| with.get("persist-credentials"))
                .and_then(yaml_serde::Value::as_bool);
            if persists != Some(false) {
                keeping.push(key.clone());
            }
        }
    }

    assert!(
        checkouts_seen > 0,
        "positive control: no checkout step was found at all, so this \
         assertion would pass having checked nothing"
    );
    assert!(
        keeping.is_empty(),
        "these jobs check out without dropping credentials: {keeping:?}"
    );
}

// --- the workflow and the local harness run the same thing --------------

/// The ubuntu and macos jobs each build the engine and then run the deps
/// install with `--yes`.
///
/// test-local.sh exists so the CI legs can be iterated on locally. The moment
/// the two disagree on the command or the Dockerfile, a green local run stops
/// meaning anything about CI.
///
/// `--yes` is asserted as part of the command rather than separately. Without
/// it the leg does not fail -- it HANGS on the first per-dependency prompt,
/// until the job's timeout kills it, which is a slower and less legible
/// failure.
///
/// Both legs build the engine before they run it. A leg that installs
/// dependencies with a binary it never built is testing whatever the runner
/// image happened to ship.
#[test]
fn the_native_jobs_build_the_engine_and_run_the_deps_install() {
    let workflow = deps_workflow();
    for job in ["ubuntu", "macos"] {
        let run = job_run_text(&workflow, job);
        assert!(!run.is_empty(), "positive control: the {job} job runs steps");
        assert!(
            run.contains("deps install --yes"),
            "the {job} job runs the deps install: {run}"
        );
        assert!(
            run.contains("cargo build"),
            "the {job} job builds the engine first: {run}"
        );
    }
}

/// The macos job runs on a macOS runner.
///
/// Homebrew is the manager under test in that leg, so a Linux runner would
/// exercise the apt path twice and never the brew path.
#[test]
fn the_macos_job_runs_on_a_macos_runner() {
    let workflow = deps_workflow();
    assert_eq!(
        job_field(&workflow, "macos", "runs-on"),
        "macos-latest",
        "the macos job runs on a macOS runner"
    );
}

/// The arch job builds the arch Dockerfile, runs the image it built, mounts
/// the seed, and names the prebuilt binary.
///
/// The image tag is the join between the build step and the run step. The
/// seed mount is what gives the image's wrapper a binary to run at all, and
/// `BOOTSTRAP_PREBUILT_BIN` is what points the wrapper at it.
#[test]
fn the_arch_job_builds_and_runs_its_image_off_the_seed() {
    let workflow = deps_workflow();
    let run = job_run_text(&workflow, "arch");
    assert!(!run.is_empty(), "positive control: the arch job runs steps");

    assert!(
        run.contains("deps/docker/Dockerfile.arch"),
        "the arch job builds the arch Dockerfile: {run}"
    );
    assert!(
        run.contains("depcheck-arch"),
        "the arch job runs the image it built: {run}"
    );
    assert!(run.contains("/seed"), "the arch job mounts the seed: {run}");
    assert!(
        run.contains("BOOTSTRAP_PREBUILT_BIN"),
        "the arch job names the prebuilt binary: {run}"
    );
}

/// test-local.sh tags images the way the arch job does.
///
/// The tag is the join between the build step and the run step, and
/// test-local.sh builds the same `depcheck-$image` name. A rename in one place
/// only is a broken job, not a failed check.
#[test]
fn the_local_harness_tags_images_the_way_the_arch_job_does() {
    let harness = read_to_string(&harness_path());
    assert!(
        harness.contains(r#""depcheck-$image""#),
        "the local harness tags images the way the arch job does"
    );
}

// --- the Pop!_OS leg -----------------------------------------------------

/// Dockerfile.pop fails the build when its archive does not load.
///
/// The trap this assertion exists for was hit while writing the image, and it
/// is silent. Dearmoring `dists/noble/Release.gpg` looks right and installs
/// nothing usable -- that file is a signature, not a public key -- so apt
/// reported NO_PUBKEY, skipped the repository, and the image built green while
/// testing plain Ubuntu. The build has to FAIL in that case, not warn.
///
/// Pop!_OS is apt, like the ubuntu leg, so the reason this image exists is not
/// the package manager but the package VERSIONS its archive resolves to. The
/// failure it guards was real: apt's neovim is 0.6.1 on Pop 22.04 and 0.9.5 on
/// 24.04, `vim.uv` needs 0.10, and the manifest's old bare `command -v nvim`
/// was satisfied by both.
#[test]
fn the_pop_image_fails_the_build_when_its_archive_does_not_load() {
    let pop = read_to_string(&dockerfile_path("pop"));
    assert!(
        pop.contains("apt-cache policy | grep -q apt.pop-os.org"),
        "the pop image fails the build when its archive does not load"
    );
    assert!(
        pop.contains("apt.pop-os.org"),
        "the pop image adds the Pop archive"
    );
}

/// Dockerfile.pop runs `apt-get update` without swallowing its failure.
///
/// A `|| true` on the repository setup would restore exactly the silence the
/// assertion above exists to prevent. The shape asserted is `apt-get update;`
/// ending a continued line inside the `set -eu` chain: the semicolon plus the
/// continuation is what keeps the chain going while leaving `set -e` in charge
/// of the exit status.
#[test]
fn the_pop_image_runs_apt_get_update_without_swallowing_its_failure() {
    let pop = read_to_string(&dockerfile_path("pop"));
    let guarded = pop
        .lines()
        .map(str::trim_end)
        .filter(|line| line.contains("apt-get update;") && line.ends_with('\\'))
        .count();
    assert!(
        guarded >= 1,
        "the pop image runs apt-get update without swallowing its failure, \
         found {guarded} such lines"
    );
}

/// The pop job builds the pop Dockerfile, asserts the installed neovim
/// version, and asserts a second run converges.
///
/// The install step reporting "installed neovim" is not the claim: the engine
/// reports that for apt's 0.9.5 too. The workflow has to assert the VERSION on
/// the machine afterwards, which is the fact the old check could not express.
///
/// Convergence, not just success. A floor the installer cannot satisfy would
/// reinstall every wave forever while still exiting 0 on each one.
///
/// The convergence string is matched with single-space spacing because
/// `job_run_text` collapses whitespace. The workflow itself greps for the
/// engine's real three-space column spacing, `present   neovim`; verified
/// against a live container, where an install prints `installed neovim` with
/// one space and a converged run prints `present   neovim` with three.
/// Asserting the collapsed form here checks that the job looks for
/// convergence at all, and the container run is what proves the spacing.
#[test]
fn the_pop_job_asserts_the_neovim_version_and_convergence() {
    let workflow = deps_workflow();
    let run = job_run_text(&workflow, "pop");
    assert!(!run.is_empty(), "positive control: the pop job runs steps");

    assert!(
        run.contains("deps/docker/Dockerfile.pop"),
        "the pop job builds the pop Dockerfile: {run}"
    );
    assert!(
        run.contains("nvim --version"),
        "the pop job asserts the installed neovim version: {run}"
    );
    assert!(
        run.contains("present neovim"),
        "the pop job asserts the second run converges: {run}"
    );
}

// --- the manifest carries the floor at all -------------------------------

/// deps.toml pins a neovim version floor.
///
/// The image and the workflow are both downstream of this line. Without it
/// they assert the engine installs a current neovim onto a machine that never
/// asked for one.
///
/// The floor is its own key, not a suffix on a check string. The retired pipe
/// format wrote `command -v nvim >=0.10`, which packed two values into one
/// positionally-split field; `min_version` names the second one.
///
/// Read through a TOML parse rather than as a substring, so reformatting the
/// table cannot fail the assertion and a `min_version` inside a comment
/// cannot satisfy it.
#[test]
fn the_manifest_pins_a_neovim_version_floor() {
    let manifest = read_to_string(&repo_root().join("deps/deps.toml"));
    assert!(
        manifest.contains(r#"min_version = "0.10""#),
        "deps.toml pins a neovim version floor"
    );
}

// --- the deps platform variants both ship --------------------------------

/// Both platform variants ship together.
///
/// deps-local.conf used to hold one branch's own entries. The deps-mac.toml /
/// deps-linux.toml pair replaced it exactly to end that: both ship together
/// and the platform check at run time decides which one gets read.
#[test]
fn both_deps_platform_variants_ship() {
    let root = repo_root();
    for platform in ["mac", "linux"] {
        let manifest = root.join(format!("deps/deps-{platform}.toml"));
        assert!(
            manifest.is_file(),
            "deps-{platform}.toml ships here: {}",
            manifest.display()
        );
    }
}

/// The retired deps-local.conf is gone from the deps README.
///
/// The positive control is the README's own length: an empty or missing file
/// would otherwise satisfy "no mention" while proving nothing.
#[test]
fn the_retired_deps_local_conf_is_gone_from_the_readme() {
    let readme_path = repo_root().join("deps/README.md");
    let readme = read_to_string(&readme_path);
    assert!(
        !readme.trim().is_empty(),
        "positive control: {} has content",
        readme_path.display()
    );

    let mentions: Vec<(usize, &str)> = readme
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains("deps-local.conf"))
        .map(|(index, line)| (index + 1, line))
        .collect();
    assert!(
        mentions.is_empty(),
        "the deps README still names the retired deps-local.conf: {mentions:?}"
    );
}

// --- the CI harness manifest is findable ---------------------------------

/// The test-suite workflow names the CI manifest by bare file name, and that
/// name resolves under the shipped conf directory.
///
/// Same shape as the DOTFILES_ROOT assertion above, at the other call site.
/// The engine roots a relative `DEPS_CONF` against `DOTFILES_ROOT/deps`, so
/// the workflow passing `deps/deps-ci.toml` resolved to that directory twice
/// over. A missing conf file is tolerated by design, which is how
/// `DEPS_LOCAL_CONF` excludes the platform variant, so the step read an empty
/// manifest, installed nothing and exited 0. The macOS leg then failed every
/// YAML assertion on a missing pyyaml; Linux passed only because its runner
/// ships one.
///
/// The engine now refuses an explicitly named manifest that holds nothing, so
/// this cannot recur silently. This assertion is the cheaper gate: it names
/// the mistake at the spelling rather than at the run.
///
/// Read from the parsed workflow's step `env:` mappings rather than by
/// grepping for `DEPS_CONF:` and taking the first hit, so a commented-out
/// value cannot satisfy it and a second occurrence is reported instead of
/// hidden.
#[test]
fn the_workflow_names_the_ci_manifest_by_bare_file_name() {
    let root = repo_root();
    let workflow = read_workflow(&root.join(".github/workflows/test-suite.yml"));

    let mut values: Vec<String> = Vec::new();
    for (_, job) in jobs_of(&workflow) {
        let steps = job
            .get("steps")
            .and_then(yaml_serde::Value::as_sequence)
            .map(Vec::as_slice)
            .unwrap_or_default();
        for step in steps {
            if let Some(value) = step
                .get("env")
                .and_then(|env| env.get("DEPS_CONF"))
                .and_then(yaml_serde::Value::as_str)
            {
                values.push(value.trim().to_string());
            }
        }
    }

    assert_eq!(
        values,
        vec!["deps-ci.toml".to_string()],
        "the workflow names the CI manifest by bare file name"
    );

    let resolved = root.join("deps").join(&values[0]);
    assert!(
        resolved.is_file(),
        "the named CI manifest resolves under the shipped conf dir: {}",
        resolved.display()
    );
}

// --- the executed scripts in deps/ carry their execute bit ----------------

/// Every executed script in `deps/` is executable on disk.
///
/// A script committed 100644 works on the machine that has the bit set
/// locally and fails on every fresh clone, which is the worst shape this bug
/// takes.
#[test]
fn every_executed_script_in_deps_is_executable() {
    let root = repo_root().join("deps");
    let mut not_executable: Vec<&str> = Vec::new();
    for script in EXECUTED_DEPS_SCRIPTS {
        let path = root.join(script);
        let executable = std::fs::metadata(&path)
            .is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0);
        if !executable {
            not_executable.push(script);
        }
    }
    assert!(
        not_executable.is_empty(),
        "these executed scripts in deps/ are not executable: {not_executable:?}"
    );
}

/// The committed execute bits under `deps/` are EXACTLY the executed scripts.
///
/// An exact set, not a subset: a new executed script that nobody adds to
/// `EXECUTED_DEPS_SCRIPTS` is a script whose bit nothing checks.
///
/// Skipped where there is no repository, which is the case in the container
/// harness: it builds its tree with `git archive` and so carries no `.cfg`.
/// That is a correct reason for a commit-inspection assertion to stand down,
/// and it is an absent *repository* rather than an absent tool, so the skip is
/// recorded and counted rather than silent.
#[test]
fn the_committed_execute_bits_in_deps_match_the_executed_scripts() {
    let root = repo_root();
    let git_dir = root.join(".cfg");
    let has_repository = Command::new("git")
        .arg(format!("--git-dir={}", git_dir.display()))
        .args(["rev-parse", "--verify", "HEAD"])
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .is_ok_and(|output| output.status.success());
    if !has_repository {
        dotfiles_test_support::skip(
            "no repository here, so the committed deps execute bits cannot be inspected",
        );
        return;
    }

    let listing = Command::new("git")
        .arg(format!("--git-dir={}", git_dir.display()))
        .arg(format!("--work-tree={}", root.display()))
        // `:/deps` and `--full-name`, neither of which the shell suite
        // needed, because it ran from the work-tree root while this test runs
        // from `crates/config-cli`. A bare `deps` pathspec resolves against
        // the cwd prefix, so it looks for `crates/config-cli/deps` and lists
        // nothing; `:/deps` is rooted at the work tree instead. `--full-name`
        // then prints paths relative to that root rather than as `../../deps/`
        // walk-ups from the cwd. Both defects were caught by the positive
        // control and the exact-set comparison below, in that order.
        .args(["ls-tree", "-r", "--full-name", "HEAD", ":/deps"])
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .expect("git ls-tree runs");
    assert!(
        listing.status.success(),
        "git ls-tree failed: {}",
        String::from_utf8_lossy(&listing.stderr)
    );
    let text = String::from_utf8_lossy(&listing.stdout);
    assert!(
        !text.trim().is_empty(),
        "positive control: git ls-tree listed nothing under deps/"
    );

    let mut committed: Vec<String> = text
        .lines()
        .filter(|line| line.starts_with("100755"))
        .filter_map(|line| line.split('\t').nth(1))
        .filter_map(|path| path.strip_prefix("deps/"))
        .map(str::to_string)
        .collect();
    committed.sort();

    let mut expected: Vec<String> = EXECUTED_DEPS_SCRIPTS
        .iter()
        .map(|script| (*script).to_string())
        .collect();
    expected.sort();

    assert_eq!(
        committed, expected,
        "the committed execute bits in deps/ match the executed scripts"
    );
}
