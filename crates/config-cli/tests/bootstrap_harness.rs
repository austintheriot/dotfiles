//! The bootstrap container harness: the four `Dockerfile.bootstrap*` images,
//! their three entrypoints, the prebuilt seam, `deps/test-bootstrap.sh`,
//! `setup.sh`, `tests/pre-push`, and the bootstrap jobs in
//! `.github/workflows/deps-check.yml`.
//!
//! Builds and runs no container, for the reason the shell suite gave: the
//! image is the slow, network-dependent, daemon-dependent part, and this
//! gate runs from a pre-commit hook. Exercising the image is
//! `test-bootstrap.sh`'s job and CI's job. What is left is the set of static
//! contracts that can rot silently.
//!
//! Converted whole from `tests/bootstrap-harness.test.sh`, which reported 99
//! assertions from 77 `assert_*` call sites: seven loops (over tools,
//! entrypoint claims, overlay paths, seam entrypoints, and the two curl
//! Dockerfiles) expanded one call site into several assertions each.
//!
//! Three things the shell version could not do, and this one does:
//!
//! 1. The workflow assertions ran under `python3 -c "import yaml"` and
//!    `skip`ped wherever python3 or PyYAML was absent. `yaml_serde` removes
//!    the interpreter dependency, so they never skip.
//! 2. The "installs no sudo" and "installs no git" assertions for the two
//!    curl images grepped `^RUN (apt-get|pacman)`, which in
//!    `Dockerfile.bootstrap-curl` matches only `RUN apt-get update` -- the
//!    package list sits on a backslash continuation. Both assertions
//!    therefore passed against a line that names no packages at all. Here
//!    continuations are joined first and a positive control proves the
//!    install command was found before anything asserts what it omits.
//! 3. Every assertion expecting an empty result is paired with a positive
//!    control, per the spec's section 4, so an extraction that silently
//!    yields nothing fails instead of passing.

use dotfiles_test_support::repo::{jobs_of, read_workflow, root as repo_root, triggers_of};
use std::path::{Path, PathBuf};

// --- reading the tracked files ---------------------------------------------

/// Reads a tracked file, naming it when it is missing.
///
/// A test asserting about a file's content has nothing to recover from when
/// the file is absent, and the path in the panic message is what makes the
/// failure actionable.
fn read_tracked(relative: &str) -> String {
    let path = repo_root().join(relative);
    match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => panic!("cannot read {}: {error}", path.display()),
    }
}

/// A shell script with its comments stripped.
///
/// The same guard `tests/pre-push`'s own assertions use, and for the reason
/// the shell suite stated there: these files explain themselves in prose that
/// a raw grep counts as code. Matching prose is not matching code.
///
/// The shell suite applied the guard only to `pre-push` and matched the
/// entrypoints with a whole-file `grep -qF`. So deleting
/// `check 'tmux was installed by the bootstrap' command -v tmux` from
/// `bootstrap-entrypoint.sh` left its assertion green, because the line
/// above it is a comment mentioning tmux. Found by sabotage during this
/// conversion.
///
/// Naive on purpose: a `#` inside a quoted string would be treated as a
/// comment. These are the repo's own scripts and none carries one, and the
/// alternative is a shell parser, which is a larger thing to be wrong about.
fn strip_comments(script: &str) -> String {
    script
        .lines()
        .map(|line| match line.find('#') {
            Some(index) => &line[..index],
            None => line,
        })
        .collect::<Vec<&str>>()
        .join("\n")
}

/// The absolute path of a tracked file in this checkout.
fn tracked(relative: &str) -> PathBuf {
    repo_root().join(relative)
}

/// Whether a path is executable by its owner.
///
/// The shell suite's `test -x`. Checked through the mode bits rather than by
/// running the file, so a script with a broken interpreter line still reports
/// the property under test.
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|metadata| metadata.permissions().mode() & 0o100 != 0)
        .unwrap_or(false)
}

/// One logical line per backslash continuation, so a command split across
/// several physical lines is one string.
///
/// This is the shell suite's `sed -e ':a' -e '/\\$/{N;s/\\\n//;ba' -e '}'`,
/// which it applied to `test-bootstrap.sh` for exactly this reason: the
/// `docker run` invocation grew to three lines and a single-line grep
/// reported the harness had stopped running the bare image at all. The same
/// joining is applied to the Dockerfiles here, which the shell version did
/// not do, and which is why two of its assertions were vacuous.
fn join_continuations(text: &str) -> Vec<String> {
    let mut joined: Vec<String> = Vec::new();
    let mut pending = String::new();
    for line in text.lines() {
        if let Some(head) = line.strip_suffix('\\') {
            pending.push_str(head);
            continue;
        }
        pending.push_str(line);
        joined.push(std::mem::take(&mut pending));
    }
    if !pending.is_empty() {
        joined.push(pending);
    }
    joined
}

/// Every package-install command in a Dockerfile, as whole logical lines.
///
/// Matches `apt-get install` and `pacman -S...` after joining
/// continuations, so the package list is part of the line that names the
/// command however the file is wrapped. Callers assert on whole words within
/// these lines.
fn install_commands(dockerfile: &str) -> Vec<String> {
    join_continuations(dockerfile)
        .into_iter()
        .filter(|line| {
            let trimmed = line.trim_start();
            line.contains("apt-get install")
                || trimmed.starts_with("RUN pacman")
                || trimmed.starts_with("&& pacman")
                || (trimmed.starts_with("pacman") && trimmed.contains("-S"))
        })
        .collect()
}

/// Whether a whole word appears in any of these lines.
///
/// Whole-word, matching the shell suite's `grep -w`: `curl` must not be
/// satisfied by `ca-certificates` and `git` must not be satisfied by
/// `github-cli`.
fn names_package(lines: &[String], package: &str) -> bool {
    lines.iter().any(|line| {
        line.split(|character: char| !character.is_ascii_alphanumeric() && character != '-')
            .any(|word| word == package)
    })
}

// --- the tracked paths under test -----------------------------------------

const DOCKERFILE: &str = "deps/docker/Dockerfile.bootstrap";
const ENTRYPOINT: &str = "deps/docker/bootstrap-entrypoint.sh";
const HARNESS: &str = "deps/test-bootstrap.sh";
const WORKFLOW: &str = ".github/workflows/deps-check.yml";
const SETUP: &str = "setup.sh";
const BARE_DOCKERFILE: &str = "deps/docker/Dockerfile.bootstrap-bare";
const BARE_ENTRYPOINT: &str = "deps/docker/bootstrap-bare-entrypoint.sh";
const CURL_DOCKERFILE: &str = "deps/docker/Dockerfile.bootstrap-curl";
const CURL_ARCH_DOCKERFILE: &str = "deps/docker/Dockerfile.bootstrap-curl-arch";
const CURL_ENTRYPOINT: &str = "deps/docker/bootstrap-curl-entrypoint.sh";
const SEED_HELPER: &str = "deps/docker/seed-prebuilt.sh";
const PRE_PUSH: &str = "tests/pre-push";

// --- the files exist -------------------------------------------------------

/// `Dockerfile.bootstrap` exists. (shell assertion 1)
#[test]
fn the_bootstrap_dockerfile_exists() {
    let path = tracked(DOCKERFILE);
    assert!(path.is_file(), "no Dockerfile at {}", path.display());
}

/// The entrypoint exists and is executable, because the image invokes it
/// through `/bin/sh` but the harness copies it as-is. (shell assertion 2)
#[test]
fn the_entrypoint_exists_and_is_executable() {
    let path = tracked(ENTRYPOINT);
    assert!(path.is_file(), "no entrypoint at {}", path.display());
    assert!(is_executable(&path), "{} is not executable", path.display());
}

/// `test-bootstrap.sh` exists and is executable: `tests/pre-push` runs it
/// only when it is, so a non-executable harness is a silently skipped gate.
/// (shell assertion 3)
#[test]
fn the_local_harness_exists_and_is_executable() {
    let path = tracked(HARNESS);
    assert!(path.is_file(), "no harness at {}", path.display());
    assert!(is_executable(&path), "{} is not executable", path.display());
}

// --- the image installs nothing the bootstrap should install ---------------

/// The tools the bootstrap must install, and which the image must therefore
/// not preinstall. One shell assertion each: 4 through 10.
const TOOLS_THE_BOOTSTRAP_INSTALLS: [&str; 7] =
    ["tmux", "zsh", "fzf", "ripgrep", "neovim", "shellcheck", "zoxide"];

/// The positive control for the seven assertions below: the install command
/// was found at all.
///
/// The shell suite had no equivalent, and that omission is what made two of
/// its assertions on the curl images vacuous. Without this, an extraction
/// that matched nothing would report every tool as absent and pass.
#[test]
fn the_bootstrap_image_names_an_install_command() {
    let commands = install_commands(&read_tracked(DOCKERFILE));
    assert!(
        !commands.is_empty(),
        "no package-install command was found in {DOCKERFILE}, so every \
         assertion about what it installs would pass vacuously"
    );
}

/// The image preinstalls none of the tools the bootstrap installs.
///
/// The whole value of this image is that it starts with almost nothing. An
/// `apt-get` line that quietly grows tmux or zsh turns the bootstrap
/// assertions into assertions about the image, and they would keep passing
/// while the bootstrap itself broke. (shell assertions 4-10)
#[test]
fn the_bootstrap_image_preinstalls_none_of_the_bootstrapped_tools() {
    let commands = install_commands(&read_tracked(DOCKERFILE));
    assert!(!commands.is_empty(), "positive control: an install command");
    let preinstalled: Vec<&str> = TOOLS_THE_BOOTSTRAP_INSTALLS
        .into_iter()
        .filter(|tool| names_package(&commands, tool))
        .collect();
    assert!(
        preinstalled.is_empty(),
        "{DOCKERFILE} preinstalls tools the bootstrap must install: \
         {preinstalled:?}"
    );
}

/// The image installs git, which is the one prerequisite `setup.sh` cannot
/// install: installing things is what the not-yet-cloned repo knows how to
/// do. (shell assertion 11)
#[test]
fn the_bootstrap_image_installs_git() {
    let commands = install_commands(&read_tracked(DOCKERFILE));
    assert!(
        names_package(&commands, "git"),
        "{DOCKERFILE} must install git, which setup.sh cannot install \
         itself; install lines were {commands:?}"
    );
}

// --- it must not run as root ----------------------------------------------

/// The image switches to a non-root user.
///
/// Root hides two real failures: `config install-hooks` refuses a
/// `~/.local/bin` it does not own, and every `sudo` in an install command is
/// a no-op for root. (shell assertion 12)
///
/// The shell suite asserted this with `grep -qE '^USER +[a-z]'`, which
/// `USER root` satisfies: `root` is lowercase. So the one edit the assertion
/// exists to catch passed it. Found by sabotage during this conversion, and
/// fixed here by reading the effective user rather than its first character.
#[test]
fn the_bootstrap_image_switches_to_a_non_root_user() {
    let dockerfile = read_tracked(DOCKERFILE);
    let declared: Vec<&str> = dockerfile
        .lines()
        .filter_map(|line| line.strip_prefix("USER ").map(str::trim))
        .collect();
    assert!(
        !declared.is_empty(),
        "{DOCKERFILE} has no `USER` line at all, so the image runs as root by \
         default and the non-root axis is lost"
    );
    let effective = declared.last().copied().unwrap_or("root");
    assert!(
        effective != "root" && effective != "0",
        "{DOCKERFILE} ends up running as {effective}, which hides both the \
         ~/.local/bin ownership failure and every no-op `sudo` in an install \
         command"
    );
}

/// The non-root user has passwordless sudo, or every privileged install in
/// the run blocks on a prompt no container can answer.
/// (shell assertion 13)
#[test]
fn the_non_root_user_has_passwordless_sudo() {
    assert!(
        read_tracked(DOCKERFILE).contains("NOPASSWD"),
        "{DOCKERFILE} grants no passwordless sudo, so a privileged install \
         would block on a password prompt"
    );
}

// --- the collision fixture ------------------------------------------------

/// The image plants a pre-existing `.zshrc`.
///
/// A pre-existing dotfile is the single most likely way a naive bootstrap
/// fails, because `git checkout` refuses to overwrite an untracked file.
/// Without this the container tests a path no real machine takes.
/// (shell assertion 14)
#[test]
fn the_bootstrap_image_plants_a_pre_existing_zshrc() {
    assert!(
        read_tracked(DOCKERFILE).contains(".zshrc"),
        "{DOCKERFILE} plants no pre-existing .zshrc, so the checkout \
         collision is never exercised"
    );
}

/// The entrypoint asserts the planted file survived, which is what
/// distinguishes a careful bootstrap from a destructive one.
/// (shell assertion 15)
#[test]
fn the_entrypoint_asserts_the_planted_file_survived() {
    assert!(
        read_tracked(ENTRYPOINT).contains("predating the bootstrap"),
        "{ENTRYPOINT} never checks the planted .zshrc content survived, so a \
         bootstrap that destroyed it would pass"
    );
}

// --- digest pinning -------------------------------------------------------

/// Whether a Dockerfile's `FROM` carries a full digest.
///
/// `ubuntu:24.04` and `debian:bookworm-slim` are republished for every point
/// release, so a tag-only reference makes the image change silently under a
/// passing test. `Dockerfile.arch` is deliberately the opposite;
/// `deps-harness.test.sh` owns that assertion.
fn from_is_digest_pinned(dockerfile: &str) -> bool {
    dockerfile.lines().any(|line| {
        let Some(reference) = line.strip_prefix("FROM ") else {
            return false;
        };
        let Some((_, digest)) = reference.split_once("@sha256:") else {
            return false;
        };
        let hex: String = digest
            .chars()
            .take_while(char::is_ascii_hexdigit)
            .collect();
        hex.len() == 64
    })
}

/// The base image is pinned by digest. (shell assertion 16)
#[test]
fn the_bootstrap_base_image_is_pinned_by_digest() {
    assert!(
        from_is_digest_pinned(&read_tracked(DOCKERFILE)),
        "{DOCKERFILE} does not pin its base image by a 64-hex sha256 digest, \
         so a republished tag can change the image under a passing test"
    );
}

// --- an empty build context -----------------------------------------------

/// The Dockerfile copies nothing into the image.
///
/// The repo arrives through a real clone from a bind-mounted bare repo. A
/// `COPY` here would make the container test a file transfer instead of a
/// clone, which is the exact gap this image exists to close.
/// (shell assertion 17)
#[test]
fn the_bootstrap_dockerfile_copies_nothing_in() {
    let dockerfile = read_tracked(DOCKERFILE);
    assert!(
        !dockerfile.is_empty(),
        "positive control: {DOCKERFILE} is empty, so finding no COPY proves \
         nothing"
    );
    let copies: Vec<&str> = dockerfile
        .lines()
        .filter(|line| line.trim_start().starts_with("COPY "))
        .collect();
    assert!(
        copies.is_empty(),
        "{DOCKERFILE} copies files into the image, so the container tests a \
         file transfer instead of a clone: {copies:?}"
    );
}

/// The entrypoint clones from the mounted seed rather than from a URL, so
/// the run needs no network for the clone itself. (shell assertion 18)
#[test]
fn the_entrypoint_clones_from_the_mounted_seed() {
    assert!(
        read_tracked(ENTRYPOINT).contains("/seed"),
        "{ENTRYPOINT} never names the /seed mount, so it is not cloning from \
         the bind-mounted bare repo"
    );
}

// --- what the entrypoint verifies -----------------------------------------

/// The facts a finished bootstrap must leave behind, which an exit code does
/// not prove: `setup.sh` can exit 0 having hooked up nothing useful.
/// One shell assertion each: 19 through 23.
const CLAIMS_THE_ENTRYPOINT_VERIFIES: [&str; 5] = [
    "status.showUntrackedFiles",
    ".local/bin/config",
    "pre-commit",
    "pre-push",
    "tmux",
];

/// The entrypoint verifies every fact a finished bootstrap must leave
/// behind. (shell assertions 19-23)
///
/// Read from the comment-stripped body, so a claim that survives only in
/// prose does not satisfy the assertion. See `strip_comments`.
#[test]
fn the_entrypoint_verifies_every_finished_bootstrap_fact() {
    let entrypoint = strip_comments(&read_tracked(ENTRYPOINT));
    assert!(
        !entrypoint.trim().is_empty(),
        "positive control: stripping comments from {ENTRYPOINT} left no code"
    );
    let unverified: Vec<&str> = CLAIMS_THE_ENTRYPOINT_VERIFIES
        .into_iter()
        .filter(|claim| !entrypoint.contains(claim))
        .collect();
    assert!(
        unverified.is_empty(),
        "{ENTRYPOINT} never checks these, so setup.sh could exit 0 having \
         hooked up nothing useful: {unverified:?}"
    );
}

/// The entrypoint re-runs `config init`, which must converge.
///
/// Paired with the assertion below on purpose: the two re-run behaviours are
/// opposite. `config init` converges; `setup.sh` refuses. A harness that
/// checked only one would let the other regress into either a failure or a
/// clobber. (shell assertion 24)
#[test]
fn the_entrypoint_re_runs_config_init() {
    let entrypoint = read_tracked(ENTRYPOINT);
    let re_runs = join_continuations(&entrypoint)
        .iter()
        .any(|line| line.contains("init --yes") && line.contains("config"));
    assert!(
        re_runs,
        "{ENTRYPOINT} never re-runs `config init --yes`, so convergence on a \
         second run is untested"
    );
}

/// The entrypoint asserts `setup.sh` refuses to re-clone over an existing
/// `~/.cfg`, which may hold unpushed commits. (shell assertion 25)
#[test]
fn the_entrypoint_asserts_setup_refuses_to_re_clone() {
    assert!(
        read_tracked(ENTRYPOINT).contains("refuses to clobber"),
        "{ENTRYPOINT} never asserts setup.sh refuses an existing ~/.cfg, so a \
         re-clone over unpushed commits would pass"
    );
}

// --- the local harness and CI agree ---------------------------------------

/// The local harness names the same image file CI builds, so the two cannot
/// drift onto different Dockerfiles. (shell assertion 26)
#[test]
fn the_local_harness_names_the_same_image_file() {
    assert!(
        read_tracked(HARNESS).contains("Dockerfile.bootstrap"),
        "{HARNESS} never names Dockerfile.bootstrap, so it is building some \
         other image than the one under test"
    );
}

/// The working-tree overlay list in the local harness.
///
/// The harness must test the working tree, not the last commit;
/// `test-local.sh` documents that trap for its own archive and the overlay is
/// what avoids it.
fn harness_overlay_line() -> String {
    let harness = read_tracked(HARNESS);
    join_continuations(&harness)
        .into_iter()
        .find(|line| line.trim_start().starts_with("for path in "))
        .unwrap_or_default()
}

/// The local harness has an overlay list at all. The positive control for
/// the four path assertions below. (shell assertion 27)
#[test]
fn the_local_harness_has_an_overlay_list() {
    let overlay = harness_overlay_line();
    assert!(
        !overlay.is_empty(),
        "{HARNESS} has no `for path in ...` overlay loop, so it archives the \
         last commit and tests code nobody wrote yet"
    );
}

/// Every tree the bootstrap builds from, not just the entry script.
///
/// Omitting one produces a machine that cannot exist: an earlier version of
/// this list carried `deps` but not `crates`, so a working-tree manifest was
/// parsed by HEAD's engine and a new check kind failed with "unknown field
/// `login_shell`" -- a bug in neither tree, only in their combination.
///
/// `crates` is load-bearing because step 4 of `config init` COMPILES
/// config-cli from these sources, so the engine under test must come from
/// the same tree as the manifest it reads.
/// One shell assertion each: 28 through 31.
const TREES_THE_OVERLAY_MUST_CARRY: [&str; 4] = ["setup.sh", "deps", "crates", ".scripts/config"];

/// The overlay carries every tree the bootstrap builds from.
/// (shell assertions 28-31)
#[test]
fn the_overlay_carries_every_tree_the_bootstrap_builds_from() {
    let overlay = harness_overlay_line();
    assert!(
        !overlay.is_empty(),
        "positive control: no overlay loop was found, so finding no missing \
         tree in it would prove nothing"
    );
    let missing: Vec<&str> = TREES_THE_OVERLAY_MUST_CARRY
        .into_iter()
        .filter(|tree| !overlay.contains(tree))
        .collect();
    assert!(
        missing.is_empty(),
        "the overlay loop in {HARNESS} omits trees the bootstrap builds \
         from, so a working-tree file would be parsed by HEAD's engine: \
         {missing:?}"
    );
}

/// The overlay drops the gitignored build output.
///
/// `crates/target` is gitignored and multiple gigabytes, and it gets
/// committed into the throwaway repo, so a missing exclusion turns a
/// minutes-long harness into an unusable one. (shell assertion 32)
#[test]
fn the_overlay_drops_the_gitignored_build_output() {
    assert!(
        read_tracked(HARNESS).contains(r#"rm -rf "$staging/crates/target""#),
        "{HARNESS} never removes crates/target from the staging tree, so \
         gigabytes of gitignored build output are committed into the \
         throwaway repo"
    );
}

// --- the CI job -----------------------------------------------------------
//
// The three python-gated blocks in the shell suite live here. They ran under
// `python3 -c "import yaml"` and skipped where either was absent, so the
// coverage was conditional on the machine. `yaml_serde` makes them
// unconditional, which is the point of this conversion.

/// Every job key in the deps-check workflow.
fn deps_check_job_keys() -> Vec<String> {
    let workflow = read_workflow(&tracked(WORKFLOW));
    jobs_of(&workflow)
        .into_iter()
        .map(|(key, _)| key)
        .collect()
}

/// The positive control for every job assertion below: the workflow parsed
/// and declares jobs at all.
#[test]
fn the_deps_check_workflow_declares_jobs() {
    let keys = deps_check_job_keys();
    assert!(
        !keys.is_empty(),
        "{WORKFLOW} parsed but declares no jobs, so every assertion about a \
         named job would pass vacuously"
    );
}

/// The workflow declares a `bootstrap` job. (shell assertion 33)
#[test]
fn the_workflow_declares_a_bootstrap_job() {
    let keys = deps_check_job_keys();
    assert!(!keys.is_empty(), "positive control: the workflow has jobs");
    assert!(
        keys.iter().any(|key| key == "bootstrap"),
        "{WORKFLOW} declares no `bootstrap` job; jobs are {keys:?}"
    );
}

/// The workflow declares the bare bootstrap job, or the bare leg only ever
/// runs locally. (shell assertion 62)
#[test]
fn the_workflow_declares_the_bare_bootstrap_job() {
    let keys = deps_check_job_keys();
    assert!(!keys.is_empty(), "positive control: the workflow has jobs");
    assert!(
        keys.iter().any(|key| key == "bootstrap-bare"),
        "{WORKFLOW} declares no `bootstrap-bare` job; jobs are {keys:?}"
    );
}

/// The workflow declares the curl-pipe bootstrap job.
/// (shell assertion 79)
#[test]
fn the_workflow_declares_the_curl_pipe_bootstrap_job() {
    let keys = deps_check_job_keys();
    assert!(!keys.is_empty(), "positive control: the workflow has jobs");
    assert!(
        keys.iter().any(|key| key == "bootstrap-curl"),
        "{WORKFLOW} declares no `bootstrap-curl` job; jobs are {keys:?}"
    );
}

/// The workflow declares the Arch curl-pipe bootstrap job, which is the only
/// leg reaching pacman through the full bootstrap. (shell assertion 80)
#[test]
fn the_workflow_declares_the_arch_curl_pipe_bootstrap_job() {
    let keys = deps_check_job_keys();
    assert!(!keys.is_empty(), "positive control: the workflow has jobs");
    assert!(
        keys.iter().any(|key| key == "bootstrap-curl-arch"),
        "{WORKFLOW} declares no `bootstrap-curl-arch` job; jobs are {keys:?}"
    );
}

/// The trigger keys of the deps-check workflow.
///
/// `on` is the YAML 1.1 boolean `true`, which a real parser resolves before
/// the mapping is ours to inspect, so the key is read both ways.
fn deps_check_trigger_keys() -> Vec<String> {
    let workflow = read_workflow(&tracked(WORKFLOW));
    let Some(triggers) = triggers_of(&workflow) else {
        return Vec::new();
    };
    let Some(mapping) = triggers.as_mapping() else {
        return Vec::new();
    };
    mapping
        .iter()
        .filter_map(|(key, _)| key.as_str().map(str::to_string))
        .collect()
}

/// The positive control for the trigger assertions: the `on` block parsed as
/// a mapping with keys in it.
#[test]
fn the_deps_check_workflow_declares_triggers() {
    let keys = deps_check_trigger_keys();
    assert!(
        !keys.is_empty(),
        "{WORKFLOW} declares no triggers as a mapping, so every assertion \
         about a named trigger would pass vacuously"
    );
}

/// The workflow runs on a schedule.
///
/// The full bootstrap installs over the network on every run, so it must not
/// fire for every commit: an upstream outage would fail work that has
/// nothing to do with dependencies. This workflow's answer is a
/// path-filtered push plus a schedule, which keeps the exposure
/// proportional. (shell assertion 34)
#[test]
fn the_workflow_runs_on_a_schedule() {
    let keys = deps_check_trigger_keys();
    assert!(!keys.is_empty(), "positive control: the workflow has triggers");
    assert!(
        keys.iter().any(|key| key == "schedule"),
        "{WORKFLOW} has no schedule trigger, so a dependency that rots \
         between commits is never caught; triggers are {keys:?}"
    );
}

/// The workflow can be dispatched by hand, so a suspected bootstrap
/// regression can be checked without inventing a commit.
/// (shell assertion 35)
#[test]
fn the_workflow_can_be_dispatched_by_hand() {
    let keys = deps_check_trigger_keys();
    assert!(!keys.is_empty(), "positive control: the workflow has triggers");
    assert!(
        keys.iter().any(|key| key == "workflow_dispatch"),
        "{WORKFLOW} has no workflow_dispatch trigger; triggers are {keys:?}"
    );
}

/// The `paths:` filter on the push trigger.
fn deps_check_push_paths() -> Vec<String> {
    let workflow = read_workflow(&tracked(WORKFLOW));
    let Some(triggers) = triggers_of(&workflow) else {
        return Vec::new();
    };
    let Some(paths) = triggers.get("push").and_then(|push| push.get("paths")) else {
        return Vec::new();
    };
    let Some(sequence) = paths.as_sequence() else {
        return Vec::new();
    };
    sequence
        .iter()
        .filter_map(|entry| entry.as_str().map(str::to_string))
        .collect()
}

/// The push trigger is path-filtered rather than unconditional.
///
/// An unfiltered push trigger is the regression to catch: it makes every
/// commit depend on the network health of every upstream package host.
/// (shell assertion 36)
#[test]
fn the_push_trigger_is_path_filtered() {
    let paths = deps_check_push_paths();
    assert!(
        !paths.is_empty(),
        "{WORKFLOW}'s push trigger carries no paths filter, so the full \
         network-dependent bootstrap fires for every commit"
    );
}

/// The paths the push filter must cover.
///
/// `setup.sh` and the config scripts are part of what this workflow
/// verifies, so a change to either must reach the filter. Without this the
/// bootstrap job exists but never runs on the commits most likely to break
/// it. One shell assertion each: 37 and 38.
const PATHS_THE_FILTER_MUST_COVER: [&str; 2] = ["setup.sh", ".scripts/config"];

/// The push filter covers `setup.sh` and the config scripts.
/// (shell assertions 37-38)
#[test]
fn the_push_filter_covers_what_the_workflow_verifies() {
    let paths = deps_check_push_paths();
    assert!(
        !paths.is_empty(),
        "positive control: no paths filter was read, so finding nothing \
         missing from it would prove nothing"
    );
    let uncovered: Vec<&str> = PATHS_THE_FILTER_MUST_COVER
        .into_iter()
        .filter(|needle| !paths.iter().any(|path| path.contains(needle)))
        .collect();
    assert!(
        uncovered.is_empty(),
        "{WORKFLOW}'s push filter never matches these, so the bootstrap job \
         exists but skips the commits most likely to break it: \
         {uncovered:?}; the filter is {paths:?}"
    );
}

// --- setup.sh is reachable as a raw URL -----------------------------------

/// `setup.sh` is at the repo root, where a raw URL can reach it. The curl
/// one-liner is the point of the split. (shell assertion 39)
#[test]
fn setup_is_at_the_repo_root() {
    let path = tracked(SETUP);
    assert!(
        path.is_file(),
        "no setup.sh at the repo root ({}), so the documented raw URL \
         resolves to nothing",
        path.display()
    );
}

/// `setup.sh` is executable, so a cloned checkout can run it directly.
/// (shell assertion 40)
#[test]
fn setup_is_executable() {
    let path = tracked(SETUP);
    assert!(is_executable(&path), "{} is not executable", path.display());
}

/// The raw URL documented in `setup.sh`'s own usage block.
fn documented_raw_url() -> Option<String> {
    let setup = read_tracked(SETUP);
    setup.lines().find_map(|line| {
        let start = line.find("https://raw.githubusercontent.com/")?;
        let candidate: String = line[start..]
            .chars()
            .take_while(|character| !character.is_whitespace())
            .collect();
        candidate.contains("setup.sh").then_some(candidate)
    })
}

/// The usage block documents a raw URL. A one-liner documented against a
/// moved file is a broken one-liner. (shell assertion 41)
#[test]
fn the_usage_block_documents_a_raw_url() {
    assert!(
        documented_raw_url().is_some(),
        "{SETUP}'s usage block documents no raw.githubusercontent.com URL \
         ending at setup.sh, so the copy-pasteable one-liner is unverified"
    );
}

/// The documented URL ends at `setup.sh`, so it names a path that exists.
/// (shell assertion 42)
#[test]
fn the_documented_url_ends_at_setup() {
    let url = documented_raw_url().unwrap_or_default();
    assert!(
        !url.is_empty(),
        "positive control: no documented URL was found at all"
    );
    assert!(
        url.ends_with("setup.sh"),
        "the documented URL does not end at setup.sh: {url}"
    );
}

// --- the bare-root leg ----------------------------------------------------
//
// A user ran the documented one-liner in a plain `docker run debian` and
// found two bugs in seconds that the image above cannot expose, because it
// installs sudo and git and runs as an ordinary user:
//
//   - every install command carried a hardcoded `sudo`, so all eleven
//     privileged installs failed with "sh: 1: sudo: not found";
//   - setup.sh said only "install git, then run this again".
//
// This leg varies exactly those axes and nothing else, so the coverage
// cannot quietly drift back to the comfortable shape.

/// The bare Dockerfile exists. (shell assertion 43)
#[test]
fn the_bare_dockerfile_exists() {
    let path = tracked(BARE_DOCKERFILE);
    assert!(path.is_file(), "no Dockerfile at {}", path.display());
}

/// The bare entrypoint is executable. (shell assertion 44)
#[test]
fn the_bare_entrypoint_is_executable() {
    let path = tracked(BARE_ENTRYPOINT);
    assert!(path.is_file(), "no entrypoint at {}", path.display());
    assert!(is_executable(&path), "{} is not executable", path.display());
}

/// The positive control for the two bare-image omission assertions below.
///
/// The shell suite had no equivalent and matched `^RUN apt-get install`,
/// which reached the package list here only because it happens to sit on the
/// same physical line. That is a property of the current formatting, not of
/// the contract.
#[test]
fn the_bare_image_names_an_install_command() {
    let commands = install_commands(&read_tracked(BARE_DOCKERFILE));
    assert!(
        !commands.is_empty(),
        "no package-install command was found in {BARE_DOCKERFILE}, so every \
         assertion about what it omits would pass vacuously"
    );
}

/// The bare image installs no sudo, which is one of the three axes that made
/// the hardcoded-`sudo` bug reachable. (shell assertion 45)
#[test]
fn the_bare_image_installs_no_sudo() {
    let commands = install_commands(&read_tracked(BARE_DOCKERFILE));
    assert!(!commands.is_empty(), "positive control: an install command");
    assert!(
        !names_package(&commands, "sudo"),
        "{BARE_DOCKERFILE} installs sudo, which removes the axis that made \
         the hardcoded-sudo bug reachable: {commands:?}"
    );
}

/// The bare image installs no git, which is the axis that made the
/// "install git, then run this again" bug reachable. (shell assertion 46)
#[test]
fn the_bare_image_installs_no_git() {
    let commands = install_commands(&read_tracked(BARE_DOCKERFILE));
    assert!(!commands.is_empty(), "positive control: an install command");
    assert!(
        !names_package(&commands, "git"),
        "{BARE_DOCKERFILE} installs git, which removes the axis that made the \
         missing-git bug reachable: {commands:?}"
    );
}

/// Whether a Dockerfile explicitly declares `USER root`.
///
/// Stated explicitly in the file rather than left to Docker's default, so
/// the intent is not mistaken for an oversight.
fn runs_as_root(dockerfile: &str) -> bool {
    dockerfile
        .lines()
        .any(|line| line.strip_prefix("USER ").map(str::trim) == Some("root"))
}

/// The bare image runs as root, the third axis. (shell assertion 47)
#[test]
fn the_bare_image_runs_as_root() {
    assert!(
        runs_as_root(&read_tracked(BARE_DOCKERFILE)),
        "{BARE_DOCKERFILE} has no explicit `USER root`, so the root axis is \
         either lost or left to a default a reader will mistake for an \
         oversight"
    );
}

/// The bare image creates no unprivileged user, which would reintroduce the
/// comfortable shape this leg exists to avoid. (shell assertion 48)
#[test]
fn the_bare_image_creates_no_unprivileged_user() {
    let dockerfile = read_tracked(BARE_DOCKERFILE);
    assert!(
        !dockerfile.is_empty(),
        "positive control: {BARE_DOCKERFILE} is empty"
    );
    let creations: Vec<&str> = dockerfile
        .lines()
        .filter(|line| line.contains("useradd"))
        .collect();
    assert!(
        creations.is_empty(),
        "{BARE_DOCKERFILE} creates an unprivileged user, which drops it back \
         to the comfortable shape: {creations:?}"
    );
}

/// The bare image installs curl, which the one-liner requires.
///
/// Deliberate rather than an oversight: the documented entry point is
/// `curl ... | sh`, so an image without curl cannot reach `setup.sh` at all,
/// and two dependencies install through `curl | sh`. Measured: omitting it
/// produced two false failures. (shell assertion 49)
#[test]
fn the_bare_image_installs_curl() {
    let commands = install_commands(&read_tracked(BARE_DOCKERFILE));
    assert!(
        !commands.is_empty(),
        "positive control: an install command in {BARE_DOCKERFILE}"
    );
    assert!(
        names_package(&commands, "curl"),
        "{BARE_DOCKERFILE} does not install curl, so the documented \
         `curl ... | sh` entry point cannot reach setup.sh at all; install \
         lines were {commands:?}"
    );
}

/// The bare base image is pinned by digest, so a republished tag cannot
/// change a passing test underneath us. (shell assertion 50)
#[test]
fn the_bare_base_image_is_pinned_by_digest() {
    assert!(
        from_is_digest_pinned(&read_tracked(BARE_DOCKERFILE)),
        "{BARE_DOCKERFILE} does not pin its base image by a 64-hex sha256 \
         digest"
    );
}

/// The bare Dockerfile copies nothing in: the repo arrives by a real clone
/// from the mounted seed. (shell assertion 51)
#[test]
fn the_bare_dockerfile_copies_nothing_in() {
    let dockerfile = read_tracked(BARE_DOCKERFILE);
    assert!(
        !dockerfile.is_empty(),
        "positive control: {BARE_DOCKERFILE} is empty"
    );
    let copies: Vec<&str> = dockerfile
        .lines()
        .filter(|line| line.trim_start().starts_with("COPY "))
        .collect();
    assert!(
        copies.is_empty(),
        "{BARE_DOCKERFILE} copies files into the image: {copies:?}"
    );
}

/// What the bare entrypoint must assert, phase by phase.
///
/// Phase 1 covers the missing-git path, which was the second reported bug.
/// It used to assert a printed suggestion; the script now installs git
/// itself, because the documented entry point is always piped and refusing
/// there left a bare image needing two commands.
///
/// The last two are phase 2's load-bearing pair: without them the leg could
/// pass on an image where nothing needing root was ever attempted, which is
/// precisely the hole the reported bug lived in.
///
/// The piped-path claim is spelled `< /dev/null`, with the redirection
/// operator, rather than the bare path. The shell suite matched
/// `/dev/null` alone, which the unrelated `2>/dev/null` on the `chown` line
/// also satisfies, so deleting the redirection that creates the piped case
/// left the assertion green. Found by sabotage during this conversion.
///
/// One shell assertion each: 52 through 59.
const BARE_ENTRYPOINT_CLAIMS: [(&str, &str); 8] = [
    ("phase 1", "runs setup.sh before git is installed"),
    (
        "installs git rather than only naming it",
        "asserts git gets installed rather than only named",
    ),
    ("it confirms git arrived", "asserts the install is confirmed"),
    ("assumes no sudo", "asserts the git install assumes no sudo"),
    (
        "continues into the clone",
        "asserts the run continues into the clone rather than stopping",
    ),
    (
        "< /dev/null",
        "drives the piped path rather than the prompt path",
    ),
    (
        "privileged install actually succeeded",
        "proves a privileged install succeeded",
    ),
    ("sudo: not found", "checks nothing reported a missing sudo"),
];

/// The bare entrypoint asserts every property this leg exists for.
/// (shell assertions 52-59)
#[test]
fn the_bare_entrypoint_asserts_every_property_the_leg_exists_for() {
    let entrypoint = strip_comments(&read_tracked(BARE_ENTRYPOINT));
    assert!(
        !entrypoint.trim().is_empty(),
        "positive control: stripping comments from {BARE_ENTRYPOINT} left no \
         code"
    );
    let missing: Vec<&str> = BARE_ENTRYPOINT_CLAIMS
        .into_iter()
        .filter(|(needle, _)| !entrypoint.contains(needle))
        .map(|(_, description)| description)
        .collect();
    assert!(
        missing.is_empty(),
        "{BARE_ENTRYPOINT} no longer {missing:?}, so this leg has lost the \
         coverage the reported bugs were found through"
    );
}

/// The local harness builds the bare image, or the bare leg only ever runs
/// in CI and nobody sees it fail before pushing. (shell assertion 60)
#[test]
fn the_local_harness_builds_the_bare_image() {
    assert!(
        read_tracked(HARNESS).contains("Dockerfile.bootstrap-bare"),
        "{HARNESS} never names Dockerfile.bootstrap-bare, so the bare leg \
         only runs in CI"
    );
}

/// The local harness runs the bare image.
///
/// Asserted on the `docker run` reaching `$IMAGE-bare` rather than on a
/// literal image name, because the harness interpolates the name and the
/// literal never appears in the source.
///
/// Continuations are joined first. The invocation grew a `-v` mount and a
/// `-e` variable and now spans three lines, so a single-line grep reported
/// the harness had stopped running the bare image at all.
/// (shell assertion 61)
#[test]
fn the_local_harness_runs_the_bare_image() {
    let harness = read_tracked(HARNESS);
    let runs = join_continuations(&harness)
        .iter()
        .any(|line| line.contains("docker run") && line.contains("$IMAGE-bare"));
    assert!(
        runs,
        "{HARNESS} has no `docker run ... $IMAGE-bare` invocation, so it \
         builds the bare image and never runs it"
    );
}

// --- the curl-pipe legs ---------------------------------------------------
//
// A fourth axis the images above do not vary: how setup.sh ARRIVES.
// bootstrap-bare-entrypoint.sh copies it out of the seed and runs it as a
// local file, and says so in its own comment, so the documented
// `curl -fsSL ... | sh` shape was untested until these legs existed.

/// The curl-pipe Dockerfile exists. (shell assertion 63)
#[test]
fn the_curl_pipe_dockerfile_exists() {
    let path = tracked(CURL_DOCKERFILE);
    assert!(path.is_file(), "no Dockerfile at {}", path.display());
}

/// The Arch curl-pipe Dockerfile exists. (shell assertion 64)
#[test]
fn the_arch_curl_pipe_dockerfile_exists() {
    let path = tracked(CURL_ARCH_DOCKERFILE);
    assert!(path.is_file(), "no Dockerfile at {}", path.display());
}

/// The curl-pipe entrypoint is executable. (shell assertion 65)
#[test]
fn the_curl_pipe_entrypoint_is_executable() {
    let path = tracked(CURL_ENTRYPOINT);
    assert!(path.is_file(), "no entrypoint at {}", path.display());
    assert!(is_executable(&path), "{} is not executable", path.display());
}

/// The curl-pipe run actually pipes from curl.
///
/// The one property that distinguishes this harness from the bare one. If
/// the entrypoint ever reads `setup.sh` off the mount, this leg has silently
/// become a duplicate of bootstrap-bare and the coverage is gone.
/// (shell assertion 66)
#[test]
fn the_curl_pipe_run_actually_pipes_from_curl() {
    let entrypoint = read_tracked(CURL_ENTRYPOINT);
    let pipes = join_continuations(&entrypoint)
        .iter()
        .any(|line| line.contains(r#"curl -fsSL "$BASE/setup.sh" | sh"#));
    assert!(
        pipes,
        "{CURL_ENTRYPOINT} never pipes `curl -fsSL \"$BASE/setup.sh\" | sh`, \
         so this leg has become a duplicate of the bare one"
    );
}

/// The curl-pipe run never copies `setup.sh` out of the seed, which would
/// make it the local-file path the bare leg already covers.
/// (shell assertion 67)
#[test]
fn the_curl_pipe_run_never_copies_setup_out_of_the_seed() {
    let entrypoint = read_tracked(CURL_ENTRYPOINT);
    assert!(
        !entrypoint.is_empty(),
        "positive control: {CURL_ENTRYPOINT} is empty"
    );
    let copies: Vec<&str> = entrypoint
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("cp ")
                && trimmed.contains("/seed/setup.sh")
                && !trimmed.contains("SERVE")
        })
        .collect();
    assert!(
        copies.is_empty(),
        "{CURL_ENTRYPOINT} copies setup.sh out of the seed to run it as a \
         local file, which is the path the bare leg already covers: \
         {copies:?}"
    );
}

/// The curl-pipe run passes no `--yes`.
///
/// The pipe alone must be enough for `setup.sh` to infer unattended, and
/// passing `--yes` would hide a regression in that inference.
/// (shell assertion 68)
#[test]
fn the_curl_pipe_run_passes_no_yes_flag() {
    let entrypoint = read_tracked(CURL_ENTRYPOINT);
    assert!(
        !entrypoint.is_empty(),
        "positive control: {CURL_ENTRYPOINT} is empty"
    );
    let with_flag: Vec<String> = join_continuations(&entrypoint)
        .into_iter()
        .filter(|line| line.contains("sh -s --") && line.contains("--yes"))
        .collect();
    assert!(
        with_flag.is_empty(),
        "{CURL_ENTRYPOINT} passes --yes through the pipe, which hides a \
         regression in setup.sh's own unattended inference: {with_flag:?}"
    );
}

/// The fetch status is checked apart from the pipeline.
///
/// A pipeline reports only its last command's status, so `curl` failing and
/// `sh` reading an empty script exits 0 -- measured. Without a standalone
/// fetch check, "the piped run exits 0" passes when `setup.sh` was never
/// fetched. (shell assertion 69)
#[test]
fn the_fetch_status_is_checked_apart_from_the_pipeline() {
    assert!(
        strip_comments(&read_tracked(CURL_ENTRYPOINT)).contains("fetch_status"),
        "{CURL_ENTRYPOINT} never checks the fetch status separately, so \
         `the piped run exits 0` passes when setup.sh was never fetched"
    );
}

/// Both curl images, so each assertion below runs once per image.
///
/// They share one entrypoint on purpose: the assertions are about the
/// bootstrap contract, not the distribution, and a per-image copy is how one
/// leg quietly stops checking what the other still does.
const CURL_DOCKERFILES: [&str; 2] = [CURL_DOCKERFILE, CURL_ARCH_DOCKERFILE];

/// The positive control the shell suite lacked, and the reason two of its
/// assertions were vacuous.
///
/// Its pattern was `^RUN (apt-get|pacman)`, which in
/// `Dockerfile.bootstrap-curl` matches only `RUN apt-get update`: the
/// package list is on a backslash continuation. So "installs no sudo" and
/// "installs no git" were asserted against a line naming no packages, and
/// would have kept passing if the file had grown `sudo git` on the
/// continuation.
#[test]
fn both_curl_images_name_an_install_command_with_packages() {
    for dockerfile in CURL_DOCKERFILES {
        let commands = install_commands(&read_tracked(dockerfile));
        assert!(
            !commands.is_empty(),
            "no package-install command was found in {dockerfile}, so every \
             assertion about what it omits would pass vacuously"
        );
        assert!(
            names_package(&commands, "curl"),
            "the install command found in {dockerfile} names no curl, so it \
             is not the line that lists packages and the omission \
             assertions below would be vacuous: {commands:?}"
        );
    }
}

/// Both curl images install no sudo, staying bare on the axis the bare leg
/// established. (shell assertions 70 and 74)
#[test]
fn both_curl_images_install_no_sudo() {
    for dockerfile in CURL_DOCKERFILES {
        let commands = install_commands(&read_tracked(dockerfile));
        assert!(
            !commands.is_empty(),
            "positive control: an install command in {dockerfile}"
        );
        assert!(
            !names_package(&commands, "sudo"),
            "{dockerfile} installs sudo, which removes the axis that made the \
             hardcoded-sudo bug reachable: {commands:?}"
        );
    }
}

/// Both curl images install no git, so `setup.sh` must install it during the
/// run. (shell assertions 71 and 75)
#[test]
fn both_curl_images_install_no_git() {
    for dockerfile in CURL_DOCKERFILES {
        let commands = install_commands(&read_tracked(dockerfile));
        assert!(
            !commands.is_empty(),
            "positive control: an install command in {dockerfile}"
        );
        assert!(
            !names_package(&commands, "git"),
            "{dockerfile} installs git, which removes the axis that made the \
             missing-git bug reachable: {commands:?}"
        );
    }
}

/// Both curl images run as root. (shell assertions 72 and 76)
#[test]
fn both_curl_images_run_as_root() {
    for dockerfile in CURL_DOCKERFILES {
        assert!(
            runs_as_root(&read_tracked(dockerfile)),
            "{dockerfile} has no explicit `USER root`, so the root axis is \
             lost or left to a default"
        );
    }
}

/// Both curl images use the shared curl-pipe entrypoint, rather than a
/// per-image copy. (shell assertions 73 and 77)
#[test]
fn both_curl_images_use_the_shared_entrypoint() {
    for dockerfile in CURL_DOCKERFILES {
        assert!(
            read_tracked(dockerfile).contains("bootstrap-curl-entrypoint.sh"),
            "{dockerfile} does not use the shared curl-pipe entrypoint, and a \
             divergent copy is how one leg quietly stops checking what the \
             other still does"
        );
    }
}

/// The Arch curl-pipe image is Arch. It is the only leg that reaches pacman
/// through the full bootstrap, so it must actually target Arch.
/// (shell assertion 78)
#[test]
fn the_arch_curl_pipe_image_is_arch() {
    let dockerfile = read_tracked(CURL_ARCH_DOCKERFILE);
    let is_arch = dockerfile
        .lines()
        .any(|line| line.starts_with("FROM archlinux"));
    assert!(
        is_arch,
        "{CURL_ARCH_DOCKERFILE} does not start FROM archlinux, so the only \
         leg reaching pacman through the full bootstrap does not target Arch"
    );
}

// --- the toolchain seam ---------------------------------------------------
//
// The deps engine IS a Rust binary, so it must exist before the dependency
// install that places rustup. The images cannot compile it themselves: the
// build context is only deps and there is no toolchain, and adding one would
// pre-satisfy the rustup entry the run exists to exercise. So the binary
// arrives through the /seed mount instead.

/// The seam helper ships. (shell assertion 81)
#[test]
fn the_seam_helper_ships() {
    let path = tracked(SEED_HELPER);
    assert!(path.is_file(), "no seam helper at {}", path.display());
}

/// The seam helper has content, which is the shell suite's `test -s`: an
/// empty file would satisfy `test -f` and source cleanly, defining nothing.
/// (shell assertion 82)
#[test]
fn the_seam_helper_has_content() {
    let path = tracked(SEED_HELPER);
    let size = std::fs::metadata(&path).map(|metadata| metadata.len());
    assert!(
        matches!(size, Ok(bytes) if bytes > 0),
        "{} is empty or unreadable, so sourcing it would define no \
         seed_prebuilt and every leg would run against whatever is on PATH",
        path.display()
    );
}

/// Every entrypoint that must source and call the prebuilt seam.
///
/// A leg that skipped the seam would run against whatever happened to be on
/// PATH, which is the fail-open this replaced.
/// One shell assertion pair each: 83 through 88.
const SEAM_ENTRYPOINTS: [&str; 3] = [ENTRYPOINT, BARE_ENTRYPOINT, CURL_ENTRYPOINT];

/// Every entrypoint sources the prebuilt seam. (shell assertions 83, 85, 87)
#[test]
fn every_entrypoint_sources_the_prebuilt_seam() {
    let skipped: Vec<&str> = SEAM_ENTRYPOINTS
        .into_iter()
        .filter(|entrypoint| {
            !strip_comments(&read_tracked(entrypoint)).contains("seed-prebuilt.sh")
        })
        .collect();
    assert!(
        skipped.is_empty(),
        "these entrypoints never source seed-prebuilt.sh, so they run \
         against whatever is on PATH: {skipped:?}"
    );
}

/// Every entrypoint calls it, not merely sources it. Sourcing defines the
/// function; only the call copies the binary onto PATH.
/// (shell assertions 84, 86, 88)
#[test]
fn every_entrypoint_calls_the_prebuilt_seam() {
    let uncalled: Vec<&str> = SEAM_ENTRYPOINTS
        .into_iter()
        .filter(|entrypoint| {
            !strip_comments(&read_tracked(entrypoint))
                .lines()
                .any(|line| line.trim_end() == "seed_prebuilt")
        })
        .collect();
    assert!(
        uncalled.is_empty(),
        "these entrypoints source the seam and never call seed_prebuilt, so \
         no prebuilt binary reaches PATH: {uncalled:?}"
    );
}

/// Runs the seam helper under a scratch `HOME` and returns its exit status.
///
/// Driven as a real process rather than asserted on spelling. Grepping for
/// the absence of `:-` would pass against a file that dropped the variable
/// entirely, so the behaviour is exercised instead.
fn run_seam(home: &Path, prebuilt: Option<&Path>) -> std::process::ExitStatus {
    let helper = tracked(SEED_HELPER);
    let script = format!(". \"{}\"; seed_prebuilt", helper.display());
    let mut command = std::process::Command::new("sh");
    command
        .arg("-c")
        .arg(script)
        .env_clear()
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    match prebuilt {
        Some(path) => {
            command.env("BOOTSTRAP_PREBUILT_BIN", path);
        }
        None => {
            command.env_remove("BOOTSTRAP_PREBUILT_BIN");
        }
    }
    match command.status() {
        Ok(status) => status,
        Err(error) => panic!("cannot run the seam helper: {error}"),
    }
}

/// A scratch directory to stand in for `HOME`.
fn scratch_home() -> tempfile::TempDir {
    match tempfile::Builder::new()
        .prefix("seam-probe-home-")
        .tempdir_in("/tmp")
    {
        Ok(directory) => directory,
        Err(error) => panic!("cannot make a scratch HOME: {error}"),
    }
}

/// An unset prebuilt binary fails the run.
///
/// The seam is REQUIRED, not optional, and this is the inverse of the
/// assertion it replaced. It was optional while the engine was a shell
/// script any image could run: an unset variable skipped the copy and the run
/// proceeded against whatever was on PATH. That expired the moment the engine
/// became a binary no image can build. An optional seam is a gate that can
/// pass having tested a path that no longer ships, which is this project's
/// dominant bug shape. (shell assertion 89)
#[test]
fn an_unset_prebuilt_binary_fails_the_run() {
    let home = scratch_home();
    let status = run_seam(home.path(), None);
    assert!(
        !status.success(),
        "the seam accepted an unset BOOTSTRAP_PREBUILT_BIN, so a dropped -e \
         flag or a renamed step would run against whatever is on PATH"
    );
}

/// A set-but-missing prebuilt binary fails the run, so a path typo is loud
/// rather than a silent fall-through. (shell assertion 90)
#[test]
fn a_set_but_missing_prebuilt_binary_fails_the_run() {
    let home = scratch_home();
    let status = run_seam(home.path(), Some(Path::new("/nonexistent/config-cli")));
    assert!(
        !status.success(),
        "the seam accepted a BOOTSTRAP_PREBUILT_BIN that does not exist"
    );
}

/// A real prebuilt binary is accepted.
///
/// The positive control. Without it both assertions above would pass against
/// a helper that refuses every input, including a correct one.
/// (shell assertion 91)
#[test]
fn a_real_prebuilt_binary_is_accepted() {
    let home = scratch_home();
    let binary = home.path().join("config-cli");
    if let Err(error) = std::fs::write(&binary, "#!/bin/sh\nexit 0\n") {
        panic!("cannot write the probe binary: {error}");
    }
    use std::os::unix::fs::PermissionsExt;
    if let Err(error) = std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)) {
        panic!("cannot make the probe binary executable: {error}");
    }

    let status = run_seam(home.path(), Some(&binary));
    assert!(
        status.success(),
        "the seam refused a correct prebuilt binary, so the two failure \
         assertions above prove nothing about the variable and only that the \
         helper refuses everything"
    );
}

/// And the accepted binary lands on PATH, which is the seam's whole job.
/// (shell assertion 92)
#[test]
fn an_accepted_prebuilt_binary_lands_on_path() {
    let home = scratch_home();
    let binary = home.path().join("config-cli");
    if let Err(error) = std::fs::write(&binary, "#!/bin/sh\nexit 0\n") {
        panic!("cannot write the probe binary: {error}");
    }
    use std::os::unix::fs::PermissionsExt;
    if let Err(error) = std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)) {
        panic!("cannot make the probe binary executable: {error}");
    }

    let status = run_seam(home.path(), Some(&binary));
    assert!(status.success(), "positive control: the seam accepted it");

    let landed = home.path().join(".local/bin/config-cli");
    assert!(
        is_executable(&landed),
        "the seam exited 0 without leaving an executable at {}, so the copy \
         it exists to perform did not happen",
        landed.display()
    );
}

// --- pre-push runs the deep checks, not only the suite --------------------
//
// WHY THIS EXISTS. The GitHub Actions quota ran out on 2026-09-09, so local
// gates now carry what CI carried. pre-push already ran the leak scan, the
// stamp gate, the Rust checks and the test suite in Docker -- but NOT the
// bootstrap legs, which are the class that has caught the most real bugs
// this week: the orphaned nvim runtime, the missing tree-sitter CLI, the
// system-wide font path on Arch, and the node-on-PATH ordering bug were all
// invisible to the suite and obvious to a bare container.
//
// Two harnesses already existed for this and nothing ran them automatically:
//   deps/test-local.sh     - `config deps install` against ubuntu, Pop, arch
//   deps/test-bootstrap.sh - the real entry point against a bare clone image
//
// Asserted as pre-push INVOKING them, because a harness nobody runs is
// documentation.
//
// These eight assertions sat AFTER the shell suite's `finish` call, so its
// own summary line counted 92 of the 99 and reported the last seven nowhere.
// In Rust every test is counted by the harness, which is one more thing the
// conversion fixes.

/// The pre-push hook exists. (shell assertion 93)
#[test]
fn the_pre_push_hook_exists() {
    let path = tracked(PRE_PUSH);
    assert!(path.is_file(), "no pre-push hook at {}", path.display());
}

/// The pre-push hook with comments stripped.
///
/// The same guard the other script parses here use: this file explains the
/// harnesses in prose that a raw grep would count as a call. Matching prose
/// is not matching code.
fn pre_push_code() -> String {
    strip_comments(&read_tracked(PRE_PUSH))
}

/// The comment-stripped body is non-empty. The positive control for the four
/// assertions below: a strip that removed everything would make each of them
/// report an absence that is really a parsing bug.
/// (shell assertion 94)
#[test]
fn the_pre_push_body_was_read() {
    let code = pre_push_code();
    assert!(
        code.split_whitespace().count() > 10,
        "stripping comments from {PRE_PUSH} left no code, so every assertion \
         about its body would report a parsing bug as a missing feature"
    );
}

/// The loop that runs the bootstrap harnesses.
///
/// Anchored to the loop rather than to any mention of the harness names. A
/// first version of the shell assertion grepped the comment-stripped body
/// for each filename and passed on a sabotaged loop, because the skip
/// message itself names both harnesses in a `printf`.
fn pre_push_harness_loop() -> String {
    let code = pre_push_code();
    join_continuations(&code)
        .into_iter()
        .find(|line| line.trim_start().starts_with("for harness in "))
        .unwrap_or_default()
}

/// pre-push loops over the bootstrap harnesses. (shell assertion 95)
#[test]
fn pre_push_loops_over_the_bootstrap_harnesses() {
    let harness_loop = pre_push_harness_loop();
    assert!(
        !harness_loop.is_empty(),
        "{PRE_PUSH} has no `for harness in ...` loop in its code, so the \
         bootstrap legs run only when someone remembers"
    );
}

/// The harnesses the loop must name.
/// One shell assertion each: 96 and 97.
const HARNESSES_PRE_PUSH_MUST_RUN: [&str; 2] = ["test-local.sh", "test-bootstrap.sh"];

/// The harness loop includes both harnesses. (shell assertions 96-97)
#[test]
fn the_harness_loop_includes_both_harnesses() {
    let harness_loop = pre_push_harness_loop();
    assert!(
        !harness_loop.is_empty(),
        "positive control: no harness loop was found, so finding nothing \
         missing from it would prove nothing"
    );
    let missing: Vec<&str> = HARNESSES_PRE_PUSH_MUST_RUN
        .into_iter()
        .filter(|harness| !harness_loop.contains(harness))
        .collect();
    assert!(
        missing.is_empty(),
        "the harness loop in {PRE_PUSH} omits {missing:?}, so that leg runs \
         only when someone remembers"
    );
}

/// The harness loop actually executes each harness.
///
/// The path alone is not enough: the executable-check guard one line above
/// contains the same string, so a first version of this passed on a body
/// whose invocation had been replaced by `true`. The invocation is the line
/// that ENDS the `env` continuation, so it is matched with its trailing
/// `; then`. (shell assertion 98)
#[test]
fn the_harness_loop_executes_each_harness() {
    let code = pre_push_code();
    assert!(
        !code.trim().is_empty(),
        "positive control: the comment-stripped body is empty"
    );
    assert!(
        code.contains(r#""$HOME/deps/$harness"; then"#),
        "{PRE_PUSH} names the harness paths but never executes them, which is \
         what a body whose invocation was replaced by `true` looks like"
    );
}

/// The deep checks can be skipped deliberately.
///
/// A push that touches only a test file does not need three container
/// bootstraps, and a 20-minute unconditional gate is how `--no-verify`
/// becomes a habit, which is the one thing this repo's rules forbid
/// outright.
///
/// Anchored to the TEST, not to the variable's appearance: the skip message
/// mentions the variable by name, so a grep for the name alone passed on a
/// sabotaged condition. (shell assertion 99)
#[test]
fn the_deep_checks_can_be_skipped_deliberately() {
    let code = pre_push_code();
    assert!(
        !code.trim().is_empty(),
        "positive control: the comment-stripped body is empty"
    );
    let tested = code
        .lines()
        .any(|line| line.trim_start().starts_with(r#"if [ "${DOTFILES_SKIP_BOOTSTRAP"#));
    assert!(
        tested,
        "{PRE_PUSH} never TESTS DOTFILES_SKIP_BOOTSTRAP, so the deep checks \
         are unconditional and `--no-verify` becomes the habit"
    );
}
