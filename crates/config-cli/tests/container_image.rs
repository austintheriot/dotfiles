//! The containerized test runner: `tests/docker/Dockerfile` and
//! `tests/run-in-docker.sh`.
//!
//! The suite mutates `$HOME` by design. It writes fixture repositories, spawns
//! tmux sessions, and once created a real `~/.oh-my-zsh` on a machine that does
//! not use oh-my-zsh. Running it inside a container keeps those side effects
//! off the developer's machine.
//!
//! This asserts the runner's static contracts only. It never builds or runs the
//! image: the gate has to stay fast and must not require a Docker daemon, and a
//! build here would recurse, because the image runs the suite that runs this.
//!
//! Converted whole from `tests/container.test.sh`, which reported 54 assertions
//! from 47 `assert_*` call sites: one of them sits in a loop over eight tools,
//! so that site expands to eight.
//!
//! # The circularity worth naming
//!
//! This file asserts about the image that this file runs inside. A mistake here
//! is self-concealing: if a conversion error made a test skip rather than fail
//! in the container, the gate reports a pass and the image's shape goes
//! unchecked in the only place it matters. Nothing here skips. Every assertion
//! reads tracked files that the image carries, so the container runs all of
//! them, and a failure to read one is a panic rather than a stand-down.
//!
//! # Two ways a Dockerfile assertion goes vacuous
//!
//! Both were measured in this repository, and [`Dockerfile::instructions`]
//! exists to answer both at once:
//!
//! 1. **A comment satisfied a whole-file search.** A search for a tool name was
//!    satisfied by the prose naming it, so deleting the real install line left
//!    the assertion green.
//! 2. **A backslash continuation hid the payload.** `RUN apt-get update \` with
//!    the package list on the next line defeated `^RUN (apt-get|pacman)` for the
//!    life of a suite, because the matched line never contained a package.

use dotfiles_test_support::repo::root as repo_root;
use std::path::PathBuf;
use std::process::Command;

// --- the subjects ---------------------------------------------------------

/// The tools the suite shells out to, each of which the image must install.
///
/// A missing tool does not fail the suite honestly: the tmux helpers and the
/// zsh widget tests would error in ways that read as unrelated breakage.
const REQUIRED_TOOLS: [&str; 8] = [
    "tmux",
    "zsh",
    "git",
    "python3",
    "dash",
    "fzf",
    "ripgrep",
    "shellcheck",
];

/// The root-level files the runner must overlay from the working tree.
const OVERLAID_FILES: [&str; 2] = ["setup.sh", "README.md"];

/// Root-level directories a suite references that are deliberately absent from
/// the runtime image. Each is an exemption with a reason, not a suppression.
///
/// - `.cfg` is the bare repository on a developer machine, and `.git` is the
///   same thing on a CI runner where the root is a checkout. Both are probed to
///   ask "is there a repository here", which is a question rather than a path
///   the image needs. The container's tree comes from `git archive` and has no
///   repository at all, which is the documented reason several checks stand
///   down in there. Copying either would defeat that isolation.
/// - `.config` is copied per subdirectory, so the image carries only what a
///   suite reads.
/// - `.local` is a runtime artifact, untracked, built inside the image.
/// - `crates` is copied into the builder stage, and checked separately by the
///   workspace-member assertion.
///
/// `.git` is on this list because CI caught its absence. The first version was
/// derived from what happens to exist under the root on ONE machine, where the
/// repository is bare and `$HOME` has no `.git`, so the loop skipped it and the
/// list looked complete. An exemption list built from one environment's
/// filesystem is a list that passes there and nowhere else.
const EXEMPT_FROM_IMAGE: [&str; 5] = [".cfg", ".git", ".config", ".local", "crates"];

/// The same exemptions minus `crates`, which IS overlaid: an uncommitted crate
/// edit has to reach the builder or the container tests the last commit's
/// binaries.
const EXEMPT_FROM_OVERLAY: [&str; 4] = [".cfg", ".git", ".config", ".local"];

fn dockerfile_path() -> PathBuf {
    repo_root().join("tests/docker/Dockerfile")
}

fn runner_path() -> PathBuf {
    repo_root().join("tests/run-in-docker.sh")
}

fn read(path: &std::path::Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

// --- reading a Dockerfile -------------------------------------------------

/// A Dockerfile, read as instructions rather than as text.
struct Dockerfile {
    /// Every instruction, comments stripped and continuations joined.
    instructions: Vec<String>,
    /// The instructions from the last `FROM` onward, which is the runtime stage.
    runtime_instructions: Vec<String>,
}

impl Dockerfile {
    /// Parses a Dockerfile into logical instructions.
    ///
    /// Two transformations, each answering a measured vacuous-assertion defect:
    ///
    /// **Comments are dropped.** A whole-file search for a tool name was
    /// satisfied by the comment naming it, so deleting the real install line
    /// left the assertion green. Comments are not code, and an assertion that a
    /// comment can satisfy is an assertion that cannot fail.
    ///
    /// **Backslash continuations are joined.** `RUN apt-get update \` with the
    /// package list on the following line means a per-line match never sees a
    /// package name. One `RUN` spanning thirteen lines becomes one instruction.
    fn parse(text: &str) -> Self {
        let mut instructions: Vec<String> = Vec::new();
        let mut pending = String::new();
        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.starts_with('#') || (line.is_empty() && pending.is_empty()) {
                continue;
            }
            let (body, continues) = match line.strip_suffix('\\') {
                Some(body) => (body.trim_end(), true),
                None => (line, false),
            };
            if pending.is_empty() {
                pending.push_str(body);
            } else {
                pending.push(' ');
                pending.push_str(body);
            }
            if !continues {
                instructions.push(std::mem::take(&mut pending));
            }
        }
        if !pending.is_empty() {
            instructions.push(pending);
        }

        let runtime_start = instructions
            .iter()
            .rposition(|instruction| instruction.starts_with("FROM "))
            .expect("the Dockerfile has a FROM instruction");
        let runtime_instructions = instructions[runtime_start..].to_vec();

        Self {
            instructions,
            runtime_instructions,
        }
    }

    /// Reads the checkout's Dockerfile.
    fn read() -> Self {
        let parsed = Self::parse(&read(&dockerfile_path()));
        assert!(
            !parsed.instructions.is_empty(),
            "positive control: the Dockerfile parsed to no instructions at all"
        );
        parsed
    }

    /// Every instruction of one kind, across all stages.
    fn directives(&self, keyword: &str) -> Vec<&str> {
        let prefix = format!("{keyword} ");
        self.instructions
            .iter()
            .filter(|instruction| instruction.starts_with(&prefix))
            .map(String::as_str)
            .collect()
    }

    /// The `COPY` source arguments across all stages.
    fn copy_sources(&self) -> Vec<String> {
        copy_sources_of(&self.instructions)
    }

    /// The `COPY` source arguments of the runtime stage only.
    ///
    /// A different question from [`Self::copy_sources`], and the difference was
    /// measured. `COPY deps ../deps` in the builder made a whole-file scan
    /// report `deps/` as covered while `/root/deps` did not exist, so the guard
    /// passed on the exact bug it was written for.
    fn runtime_copy_sources(&self) -> Vec<String> {
        copy_sources_of(&self.runtime_instructions)
    }
}

/// The source arguments of every `COPY` in a set of instructions.
///
/// Flags (`--from=builder`) and absolute paths are dropped: the first is not a
/// path and the second names something inside a previous stage rather than a
/// path in the build context. The final argument is the destination.
fn copy_sources_of(instructions: &[String]) -> Vec<String> {
    instructions
        .iter()
        .filter_map(|instruction| instruction.strip_prefix("COPY "))
        .flat_map(|arguments| {
            let fields: Vec<&str> = arguments.split_whitespace().collect();
            let sources = fields.split_last().map_or(&[][..], |(_, rest)| rest);
            sources
                .iter()
                .filter(|field| !field.starts_with("--") && !field.starts_with('/'))
                .map(|field| (*field).to_string())
                .collect::<Vec<String>>()
        })
        .collect()
}

/// Whether a path is covered by a `COPY` source, which may be a glob.
///
/// `.zshrc*` covers `.zshrc-linux`, so this is a glob match rather than a
/// substring test. Only `*` is supported, which is all the Dockerfile uses.
fn copy_source_covers(source: &str, target: &str) -> bool {
    match source.split_once('*') {
        Some((before, after)) => {
            target.len() >= before.len() + after.len()
                && target.starts_with(before)
                && target.ends_with(after)
        }
        None => source == target,
    }
}

// --- reading the runner ---------------------------------------------------

/// The single-word items of a `for <name> in <items>; do` loop in the runner.
///
/// Read from the loop's own list rather than from the whole script, for the
/// same reason the image checks read only `COPY` lines: a mention anywhere else
/// in the file must not satisfy an assertion that the loop covers a path.
fn loop_items(runner_text: &str, variable: &str) -> Vec<String> {
    let opening = format!("for {variable} in ");
    let items: Vec<String> = runner_text
        .lines()
        .map(str::trim)
        .find_map(|line| {
            line.strip_prefix(&opening)?
                .strip_suffix("; do")
                .map(|items| {
                    items
                        .split_whitespace()
                        .map(str::to_string)
                        .collect::<Vec<String>>()
                })
        })
        .unwrap_or_default();
    assert!(
        !items.is_empty(),
        "the runner's `for {variable} in ...` list is no longer parseable"
    );
    items
}

// --- reading the suites' references ---------------------------------------

/// The root-level paths the shell suites reach for through `$DOTFILES_ROOT`.
///
/// Derived from what the suites actually reference rather than from a hand-kept
/// list, so a new root-level dependency is caught without anyone remembering to
/// update this file. That is how the `deps` tree was caught when it moved out
/// from under `.scripts/` and stopped riding along on that `COPY`.
fn referenced_root_paths() -> Vec<String> {
    // Both harnesses, because the suites moved.
    //
    // This read `tests/*.test.sh` alone. The 2026-09-10 port converted 47 of
    // them to Rust, so that glob went from matching every suite to matching
    // three, and the three that remain reference no root-level path. The
    // positive control caught it at the push gate rather than letting the
    // assertion pass over an empty set, which is exactly what it is for.
    //
    // Both directories are read so this keeps working while the last shell
    // suites exist and after they are deleted.
    let root = repo_root();
    let mut found: Vec<String> = [
        (root.join("tests"), ".test.sh"),
        (root.join("crates/config-cli/tests"), ".rs"),
    ]
    .into_iter()
    .filter_map(|(directory, suffix)| {
        std::fs::read_dir(&directory)
            .ok()
            .map(|entries| (entries, suffix))
    })
    .flat_map(|(entries, suffix)| {
        entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(move |path| path.to_str().is_some_and(|path| path.ends_with(suffix)))
            .collect::<Vec<_>>()
    })
    .filter_map(|path| std::fs::read_to_string(path).ok())
    .flat_map(|text| references_in(&text))
    .collect();
    found.sort();
    found.dedup();
    found
}

/// The root-level path segments one suite's text references.
///
/// Only a bare segment counts: `$DOTFILES_ROOT/tests/pre-push` names `tests`, and
/// anything carrying a further slash is a path inside a root-level entry that
/// the entry's own `COPY` already covers.
/// Every root-level segment the harness references.
///
/// Keyed on `DOTFILES_ROOT/`, which both harnesses use: the shell suites
/// wrote `$DOTFILES_ROOT/tests`, and the converted suites carry the same
/// literal in their fixture setup.
///
/// A second idiom, `repo_root().join("setup.sh")`, was added here on
/// 2026-09-10 and then removed: sabotage showed it changed nothing, because
/// `DOTFILES_ROOT/setup.sh` appears in this very file and so the control is
/// satisfied without it. Verified against a simulated deletion of the last
/// three shell suites too. An assertion nothing can break is not an
/// assertion.
fn references_in(text: &str) -> Vec<String> {
    let marker = "DOTFILES_ROOT/";
    text.match_indices(marker)
        .filter_map(|(index, _)| {
            let rest = &text[index + marker.len()..];
            let segment: String = rest
                .chars()
                .take_while(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | '-')
                })
                .collect();
            let terminator = rest[segment.len()..].chars().next();
            // A segment cut short by a further path character names something
            // deeper, which its parent's COPY already covers.
            if segment.is_empty() || matches!(terminator, Some('/')) {
                return None;
            }
            Some(segment)
        })
        .collect()
}

// --- the files exist and parse --------------------------------------------

/// The Dockerfile and the runner both exist, and the runner is executable.
#[test]
fn the_dockerfile_and_runner_exist() {
    assert!(
        dockerfile_path().is_file(),
        "{} is not a file",
        dockerfile_path().display()
    );
    assert!(
        runner_path().is_file(),
        "{} is not a file",
        runner_path().display()
    );
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(runner_path())
        .expect("the runner is readable")
        .permissions()
        .mode();
    assert!(
        mode & 0o111 != 0,
        "the runner is not executable (mode {mode:o})"
    );
}

/// The runner parses as POSIX `sh`, and under `dash` where one is installed.
///
/// Both, not either. `/bin/sh` is bash on macOS and accepts bashisms that dash
/// rejects, so `sh -n` alone would pass a script that breaks on the Linux leg,
/// which is the only leg that runs it.
#[test]
fn the_runner_parses_as_posix_shell() {
    for shell in ["sh", "dash"] {
        let Ok(parsed) = Command::new(shell).arg("-n").arg(runner_path()).output() else {
            // dash is absent on a stock macOS. sh never is, so this cannot
            // silently drop both: the `sh` leg would have panicked already.
            continue;
        };
        assert!(
            parsed.status.success(),
            "the runner does not parse under {shell}: {}",
            String::from_utf8_lossy(&parsed.stderr)
        );
    }
}

// --- the image carries every tool the suite shells out to -----------------

/// The image installs each tool the suite shells out to.
///
/// Read from the `RUN` instructions with continuations joined and comments
/// dropped. The shell version searched the whole file text, which the prose
/// above the install line would satisfy on its own.
#[test]
fn the_image_installs_every_tool_the_suite_needs() {
    let dockerfile = Dockerfile::read();
    let installs = dockerfile.directives("RUN").join("\n");
    assert!(
        installs.contains("apt-get install"),
        "positive control: the Dockerfile has no package install instruction"
    );
    let missing: Vec<&str> = REQUIRED_TOOLS
        .into_iter()
        .filter(|tool| !installs.split_whitespace().any(|field| field == *tool))
        .collect();
    assert!(
        missing.is_empty(),
        "the image installs no such packages: {missing:?}"
    );
}

// --- the container runs as a throwaway HOME -------------------------------

/// The image sets `HOME`, and the suite is the entrypoint.
///
/// The image's job is "run the suite and exit with its status", which is what
/// makes it usable from a pre-push hook or from CI.
///
/// The suite is the Rust one now. The entrypoint ran `tests/run-all.sh` while
/// the shell suites existed; they were ported, the runtime stage gained a
/// toolchain, and the runner became cargo.
///
/// Three assertions rather than one, because "the entrypoint mentions cargo"
/// is satisfied by an entrypoint that gets the run wrong in either of the two
/// ways this repository has already got it wrong:
///
/// 1. **`--locked`**, so a run cannot silently resolve a different dependency
///    graph than the lockfile names. The host gate and both CI legs pass it,
///    and a container leg that did not would disagree with all three.
/// 2. **From inside `crates/`, never `--manifest-path`.** rustup honours
///    `crates/rust-toolchain.toml` only when the working directory is under
///    the tree that carries it, so `--manifest-path` declares the pin without
///    applying it. That exact mistake has failed CI once already, and spec
///    section 4a.1 names it.
#[test]
fn the_image_sets_home_and_runs_the_suite() {
    let dockerfile = Dockerfile::read();
    assert!(
        dockerfile
            .directives("ENV")
            .iter()
            .any(|instruction| instruction.contains("HOME=")),
        "the Dockerfile sets no HOME"
    );
    let entrypoint = [
        dockerfile.directives("ENTRYPOINT").join(" "),
        dockerfile.directives("CMD").join(" "),
    ]
    .join(" ");
    assert!(
        entrypoint.contains("cargo test"),
        "the entrypoint does not run the suite: {entrypoint}"
    );
    assert!(
        entrypoint.contains("--locked"),
        "the entrypoint runs cargo test without --locked, so the container \
         can resolve a dependency graph the lockfile does not name: {entrypoint}"
    );
    assert!(
        !entrypoint.contains("--manifest-path"),
        "the entrypoint reaches the workspace with --manifest-path, which \
         declares the rust-toolchain.toml pin without applying it: {entrypoint}"
    );
    assert!(
        entrypoint.contains("/root/crates"),
        "the entrypoint does not run from the workspace directory, so the \
         toolchain pin does not apply: {entrypoint}"
    );
}

/// The runtime stage carries a Rust toolchain and the workspace it compiles.
///
/// The counterpart to the entrypoint assertion above: an entrypoint that runs
/// `cargo test` in an image with no cargo and no crate sources fails at run
/// time with "cargo: not found", which reads as a broken image rather than as
/// the missing prerequisite it is.
///
/// Read from the RUNTIME stage's instructions, never the whole file. The
/// builder stage is a Rust image that copies every crate manifest, so a
/// whole-file search reports both facts as satisfied while `/root/crates` does
/// not exist and the runtime base is still Rust-free. That is the same
/// measured defect `runtime_copy_sources` exists for: `COPY deps ../deps` in
/// the builder made a whole-file scan report `deps/` as covered while
/// `/root/deps` was absent, so the guard passed on the exact bug it was
/// written for.
#[test]
fn the_runtime_stage_carries_the_toolchain_and_the_workspace() {
    let dockerfile = Dockerfile::read();
    let runtime = dockerfile.runtime_instructions.join("\n");
    assert!(
        runtime.starts_with("FROM "),
        "positive control: the runtime stage does not begin with a FROM"
    );
    assert!(
        runtime.lines().next().is_some_and(|from| from.contains("rust:")),
        "the runtime stage is not built on a Rust base, so the entrypoint's \
         cargo does not exist: {}",
        runtime.lines().next().unwrap_or_default()
    );
    assert!(
        dockerfile
            .runtime_copy_sources()
            .iter()
            .any(|source| copy_source_covers(source, "crates")),
        "the runtime stage copies no crates tree, so there is nothing for \
         cargo test to compile"
    );
}

/// The runtime stage installs the pinned toolchain at build time.
///
/// The base image's default toolchain is NOT the pin. Measured against the
/// digest this Dockerfile names: the image ships 1.94.1 while
/// `crates/rust-toolchain.toml` says 1.94.0, so the first cargo invocation
/// inside `/root/crates` makes rustup download 1.94.0 over the network.
///
/// That turns every `docker run` into a network operation, and it fails
/// outright on a machine with no route out, which is a state the pre-push
/// gate has to survive. Resolving the pin in a build layer is what keeps the
/// run offline.
///
/// Asserted on the rustup install AND on the toolchain file reaching the
/// runtime stage, because either alone is vacuous: an install with no file
/// installs the base image's default, and a file with no install leaves the
/// download to run time.
#[test]
fn the_runtime_stage_installs_the_pinned_toolchain_at_build_time() {
    let dockerfile = Dockerfile::read();
    let runtime = dockerfile.runtime_instructions.join("\n");
    assert!(
        runtime.contains("rustup toolchain install"),
        "the runtime stage never installs the pinned toolchain, so the first \
         cargo run downloads it over the network"
    );
    assert!(
        dockerfile
            .runtime_copy_sources()
            .iter()
            .any(|source| copy_source_covers(source, "crates/rust-toolchain.toml")
                || copy_source_covers(source, "crates")),
        "the runtime stage copies no rust-toolchain.toml, so the install \
         above resolves the base image's default rather than the pin"
    );
}

/// The notify tests reach the platform only through injected stubs, so the
/// image needs no exclusion for them.
///
/// # Why this replaced an exclusion assertion
///
/// The shell harness globbed `tests/*.test.sh`, so a suite that could not pass
/// on Linux had to be DELETED from the image to keep the runner's suite count
/// honest, and this test asserted that `RUN rm -f .../notify.test.sh` was
/// present. The 2026-09-11 harness deletion removed both the glob and the
/// `rm`, so asserting the exclusion would now pin a line that must not exist.
///
/// `notify.rs` addresses aerospace and osascript through `AEROSPACE_BIN` and
/// `OSASCRIPT_BIN` and points both at stubs it writes itself, so it runs on
/// Linux like any other test. THAT is the property worth holding: the moment
/// a notify test shells out to a real macOS binary, it becomes a false
/// negative in this image, and this test goes red instead.
#[test]
fn the_notify_tests_reach_the_platform_only_through_stubs() {
    let source = read(&repo_root().join("crates/config-cli/tests/notify.rs"));
    assert!(
        source.contains("AEROSPACE_BIN") && source.contains("OSASCRIPT_BIN"),
        "positive control: notify.rs no longer names the two indirection \
         variables, so this test is reading the wrong file"
    );

    let direct: Vec<&str> = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//"))
        .filter(|line| line.contains("Command::new(\"osascript\")") || line.contains("Command::new(\"aerospace\")"))
        .collect();
    assert!(
        direct.is_empty(),
        "notify.rs invokes a real macOS binary by name, so it cannot pass in \
         the Linux container and becomes a false negative there: {direct:?}"
    );
}

// --- the runner never mutates the real HOME -------------------------------

/// The runner does not bind-mount `$HOME` read-write.
///
/// THE BOUNDARY ASSERTION. This is the one check in the repository that keeps
/// the container from being a way to mutate the developer's home directory. The
/// tree is copied in, never mounted, because a read-write mount would
/// reintroduce exactly the side effects the image exists to contain. Weakening
/// this is a security regression rather than a test regression.
#[test]
fn the_runner_does_not_bind_mount_home_read_write() {
    let runner = read(&runner_path());
    assert!(
        runner.contains("docker run"),
        "positive control: the runner has no `docker run` invocation to inspect"
    );
    let mounts: Vec<&str> = runner
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .filter(|line| {
            let Some(rest) = line.split_once("-v").map(|(_, rest)| rest.trim_start()) else {
                return false;
            };
            let rest = rest.trim_start_matches('"');
            let mounts_home = rest.starts_with("$HOME") || rest.starts_with("${HOME}");
            // A `:ro` suffix is a read-only mount, which is not what this
            // forbids. Anything else, including a bare mount, is read-write.
            mounts_home && !rest.contains(":ro")
        })
        .collect();
    assert!(
        mounts.is_empty(),
        "the runner bind-mounts HOME read-write: {mounts:?}"
    );
}

// --- the runner fails clearly without Docker ------------------------------

/// Runs the runner with a `PATH` that has no docker on it.
///
/// The guard under test is reached only when docker cannot be found, so the
/// child gets a directory holding just the passthrough tools the runner calls.
fn run_without_docker(extra_environment: &[(&str, &str)]) -> (i32, String) {
    let sandbox = tempfile::Builder::new()
        .prefix("no-docker-bin")
        .tempdir()
        .expect("a temporary directory is creatable");
    for tool in [
        "env", "sh", "bash", "printf", "mktemp", "rm", "cp", "tar", "git", "sed", "grep",
    ] {
        let Some(real) = ["/bin", "/usr/bin"]
            .into_iter()
            .map(|directory| PathBuf::from(directory).join(tool))
            .find(|candidate| candidate.is_file())
        else {
            continue;
        };
        let _ = std::os::unix::fs::symlink(real, sandbox.path().join(tool));
    }
    let mut command = Command::new(runner_path());
    command.env("PATH", sandbox.path()).env_remove("DOTFILES_TEST_REF");
    for (name, value) in extra_environment {
        command.env(name, value);
    }
    let run = command.output().expect("the runner spawns");
    let mut output = String::from_utf8_lossy(&run.stdout).into_owned();
    output.push_str(&String::from_utf8_lossy(&run.stderr));
    (run.status.code().unwrap_or(-1), output)
}

/// A missing docker exits non-zero and explains itself.
///
/// Asserted on the runner's own guard, not on any line mentioning "docker": the
/// shell's own "command not found" would satisfy that even with the guard
/// deleted, which makes it unfalsifiable.
///
/// The alternative it names is the Rust suite. It named `run-all.sh` while the
/// shell suites existed; the container's entrypoint is cargo now, so a message
/// pointing at the old runner would send a developer whose Docker is down to a
/// script that runs a different set of tests than the gate they just failed.
///
/// Asserted on `cargo test` AND on the working directory, for the same reason
/// the entrypoint assertion checks both: a suggestion the developer can paste
/// has to carry the `cd`, because `cargo test` from `$HOME` picks up no
/// workspace at all and `--manifest-path` would declare the pin without
/// applying it.
#[test]
fn a_missing_docker_fails_clearly() {
    let (status, output) = run_without_docker(&[]);
    assert_eq!(status, 1, "a missing docker did not exit 1: {output}");
    assert!(
        output.contains("docker is not on PATH"),
        "a missing docker does not explain itself: {output}"
    );
    assert!(
        output.contains("cargo test"),
        "a missing docker does not name the direct alternative: {output}"
    );
    assert!(
        output.contains("crates"),
        "a missing docker names cargo test without the directory it must run \
         from, so the suggestion does not work as printed: {output}"
    );
}

// --- the runner tests the ref being pushed --------------------------------

/// The runner accepts a ref argument.
///
/// The image is built from `git archive <ref>`, and the pre-push hook can push
/// a ref that is not the checked-out branch. A runner that always archived the
/// checked-out branch would build one branch's code and report a pass for
/// another, which is a false green: strictly worse than the flaky host run this
/// replaces.
#[test]
fn the_runner_accepts_a_ref_argument() {
    let runner = read(&runner_path());
    assert!(
        runner.contains("DOTFILES_TEST_REF"),
        "the runner reads no ref override"
    );
}

/// A ref that does not exist stops the run before the docker probe.
///
/// Asserted by running rather than by reading the source: `git archive
/// "$branch"` never contains the name of the variable it was assigned from, so
/// a source-text assertion passes even when the override is ignored entirely.
///
/// And asserted on the outcome rather than the wording. Matching only the
/// message would still pass if the guard were deleted and the failure deferred
/// to `git archive`, which exits non-zero too, but only after the daemon probe.
#[test]
fn a_ref_that_does_not_exist_never_reaches_the_docker_probe() {
    let (status, output) =
        run_without_docker(&[("DOTFILES_TEST_REF", "refs/heads/no-such-ref")]);
    assert_eq!(status, 1, "a bad ref did not exit 1: {output}");
    assert!(
        !output.contains("docker is not on PATH"),
        "a bad ref fell through to the docker probe: {output}"
    );
}

/// A real ref gets past ref resolution, so the guard is not refusing everything.
///
/// Meaningful only where the repository exists. This also runs inside the test
/// image, whose tree comes from `git archive` and carries no repository, so
/// `HEAD` resolves to nothing there. Rather than skip, the check asks git
/// whether `HEAD` resolves and asserts the guard's behavior for whichever
/// answer it gets: a resolvable HEAD must pass the guard, and an unresolvable
/// one must be refused by it. Both are real contracts, so the container tests
/// one of them instead of standing down.
///
/// The repository is probed where the RUNNER looks for it, which is `$HOME`,
/// not the repository root this test otherwise reads through. The two differ
/// whenever `DOTFILES_ROOT` points somewhere else, and deriving the expectation
/// from the root made this predict "no repository" while the runner resolved
/// `HEAD` against the real one. Asking the same question the subject asks is
/// the whole point of the assertion.
#[test]
fn ref_resolution_accepts_a_real_ref_and_refuses_an_unresolvable_one() {
    let runner_home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let resolvable = Command::new("git")
        .arg(format!("--git-dir={}", runner_home.join("\u{2e}cfg").display()))
        .args(["rev-parse", "--verify", "HEAD"])
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .is_ok_and(|output| output.status.success());
    let (_, output) = run_without_docker(&[("DOTFILES_TEST_REF", "HEAD")]);
    let refused = output.contains("not a ref in this repository");
    assert_eq!(
        refused, !resolvable,
        "HEAD resolves={resolvable} but the guard refused={refused}: {output}"
    );
}

// --- the pre-push hook runs the suite in the container --------------------

/// The pre-push hook runs the suite in the container and never on the host.
///
/// A hook that silently falls back to the host suite when Docker is down
/// reintroduces the flake it exists to avoid, without saying so.
///
/// # Why this stopped asserting on `tests/run-all.sh`
///
/// It used to, and the string was the right one while `run-all.sh` was how a
/// host run happened. It is the wrong one now, and it would have become worse
/// than wrong: once `run-all.sh` is deleted the literal cannot appear in any
/// file, so `!text.contains("tests/run-all.sh")` is a check no edit can ever
/// fail. An assertion that cannot fail is the vacuous shape this suite's
/// module docs catalogue, arrived at from the other direction.
///
/// The contract it was guarding is unchanged: **the hook must not run the
/// suite on this machine.** The host route is now a bare `cargo test`, so that
/// is what this refuses. The `rust-checks.sh` line is deliberately NOT
/// refused: it runs cargo against an archived snapshot of the pushed ref, it
/// mutates nothing (spec section 4a.1 verified that against an empty `$HOME`),
/// and it is an additional gate rather than a substitute for the container
/// one. Refusing it would delete a real gate to satisfy a string match.
///
/// Read from the hook's own command lines, comments dropped, for the reason
/// the Dockerfile checks are read as instructions: the prose above the
/// container call explains why there is no host fallback, and it names the
/// host command while doing so. A whole-file search would read that
/// explanation as the violation it exists to describe.
#[test]
fn the_pre_push_hook_runs_the_suite_in_the_container() {
    let hook = repo_root().join("tests/pre-push");
    assert!(hook.is_file(), "{} is not a file", hook.display());
    let text = read(&hook);
    assert!(
        text.contains("run-in-docker.sh"),
        "the hook does not run the suite in the container"
    );
    assert!(
        text.contains("DOTFILES_TEST_REF"),
        "the hook does not pass the pushed ref to the runner"
    );

    let commands: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    assert!(
        commands
            .iter()
            .any(|line| line.contains("run-in-docker.sh")),
        "positive control: no uncommented line of the hook calls the runner, \
         so the refusals below compare against nothing"
    );
    let host_run: Vec<&&str> = commands
        .iter()
        .filter(|line| line.contains("cargo test") || line.contains("cargo clippy"))
        .collect();
    assert!(
        host_run.is_empty(),
        "the hook runs the suite on this machine instead of in the container, \
         which reintroduces the flake the container leg exists to avoid: \
         {host_run:?}"
    );
}

// --- TRIGGER_PATHS covers every path a suite reads ------------------------

/// The pre-push hook's `TRIGGER_PATHS` pattern.
fn trigger_paths() -> String {
    let text = read(&repo_root().join("tests/pre-push"));
    let pattern = text
        .lines()
        .map(str::trim)
        .find_map(|line| {
            line.strip_prefix("TRIGGER_PATHS='")?
                .strip_suffix('\'')
                .map(str::to_string)
        })
        .unwrap_or_default();
    assert!(
        !pattern.is_empty(),
        "TRIGGER_PATHS is not defined in the hook"
    );
    pattern
}

/// Whether a path matches the trigger pattern, as `grep -E` would judge it.
///
/// Matched against real example paths rather than read as a substring of the
/// hook text, so the assertion fails if the regex stops matching even though the
/// literal text still appears somewhere in the pattern.
fn path_matches_trigger(pattern: &str, path: &str) -> bool {
    let matched = Command::new("grep")
        .args(["-Eq", pattern])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(path.as_bytes())?;
                stdin.write_all(b"\n")?;
            }
            child.wait()
        })
        .expect("grep runs");
    matched.success()
}

/// Every path shape a suite reads triggers the pre-push gate.
///
/// Exhaustiveness requires that editing any of these runs the suite, which is
/// exactly the routine edit that skipped it before each pattern was added: a
/// push touching only workflows ran no tests at all, and `^\.scripts/.*\.sh$`
/// matched none of the extensionless config subcommands, so a push editing only
/// one of them ran zero tests while two suites read it.
///
/// Asserted per path rather than as one pattern check, because each failure was
/// silent and per-path is what makes it legible when it returns.
#[test]
fn trigger_paths_matches_every_path_a_suite_reads() {
    let pattern = trigger_paths();
    let should_match = [
        ".github/workflows/test-suite.yml",
        ".config/nvim/lua/plugins/lsp.lua",
        ".config/tmux/tmux.conf",
        ".scripts/config/config-stamp",
        ".scripts/config/config",
        "setup.sh",
        ".zshrc",
        ".zshrc-mac",
        "deps/deps.toml",
        "deps/docker/Dockerfile.pop",
        "deps/test-local.sh",
        ".scripts/foo.sh",
        ".claude/scripts/foo.py",
        ".claude/hooks/foo.sh",
        "tests/some-suite.test.sh",
    ];
    let unmatched: Vec<&str> = should_match
        .into_iter()
        .filter(|path| !path_matches_trigger(&pattern, path))
        .collect();
    assert!(
        unmatched.is_empty(),
        "TRIGGER_PATHS no longer matches these: {unmatched:?}"
    );
}

/// A docs-only edit still triggers nothing.
///
/// The negative control. Without it a broadened pattern that matched everything
/// would satisfy every assertion above while making the gate meaningless.
///
/// This is also the hole that let a docs commit break a suite: the gate runs
/// only on triggered paths, so a documentation change carrying text that a
/// suite reads reached the remote unchecked. That is not a defect in this list,
/// which exists so a docs-only push does not pay for the whole suite.
#[test]
fn trigger_paths_ignores_a_docs_only_edit() {
    let pattern = trigger_paths();
    assert!(
        !path_matches_trigger(&pattern, "docs/superpowers/plans/some-plan.md"),
        "TRIGGER_PATHS matches a docs-only edit, so the gate triggers on everything"
    );
}

// --- every root-level path a suite reads is in the image ------------------

/// Every root-level FILE the suites read is copied into the image.
///
/// The `COPY` instructions are per-path, so a new file at the repository root is
/// absent from the image until it gets its own line. The failure is nasty: the
/// suite that reads it passes on the host and fails only in the container, so it
/// surfaces at pre-push rather than during development. That is exactly how a
/// bootstrap script shipped broken, with three suites reading it, all three
/// green on the host and all three failing in the container.
#[test]
fn every_root_level_file_the_suites_read_is_in_the_image() {
    let root = repo_root();
    let dockerfile = Dockerfile::read();
    let sources = dockerfile.copy_sources();
    assert!(
        !sources.is_empty(),
        "positive control: the Dockerfile has no COPY sources"
    );

    let mut checked = 0usize;
    let missing: Vec<String> = referenced_root_paths()
        .into_iter()
        .filter(|candidate| root.join(candidate).is_file())
        .inspect(|_| checked += 1)
        .filter(|candidate| {
            !sources
                .iter()
                .any(|source| copy_source_covers(source, candidate))
        })
        .collect();
    assert!(
        checked > 0,
        "positive control: no referenced root-level file exists, so nothing was checked"
    );
    assert!(
        missing.is_empty(),
        "these root-level files the suites read are not COPYed into the image: {missing:?}"
    );
}

/// Every root-level DIRECTORY the suites read is copied into the RUNTIME stage.
///
/// The file check tests for a file, so a top-level directory passes it without
/// ever being looked at. That gap shipped: when the deps tree moved out from
/// under `.scripts/` to the top level, it stopped riding along on that `COPY`
/// and no line replaced it. Six suites read that tree, all six passed on the
/// host, and all six failed in the container.
///
/// Scoped to the runtime stage, which is the other half of the same lesson: a
/// builder-stage `COPY` made a whole-file scan report the tree as covered while
/// the runtime path did not exist.
///
/// The positive control is folded INTO the assertion rather than sitting beside
/// it, and the difference was measured. With a broken root the separate control
/// failed while the assertion printed `ok`, because an empty derived list
/// produces an empty missing-list. A control that reports separately still
/// leaves a green tick next to a check that examined nothing, and a green tick
/// is what a reader scans for.
#[test]
fn every_root_level_directory_the_suites_read_is_in_the_runtime_stage() {
    let root = repo_root();
    let dockerfile = Dockerfile::read();
    let sources = dockerfile.runtime_copy_sources();
    assert!(
        !sources.is_empty(),
        "positive control: the runtime stage has no COPY sources"
    );

    let mut checked = 0usize;
    let missing: Vec<String> = referenced_root_paths()
        .into_iter()
        .filter(|candidate| root.join(candidate).is_dir())
        .filter(|candidate| !EXEMPT_FROM_IMAGE.contains(&candidate.as_str()))
        .inspect(|_| checked += 1)
        .filter(|candidate| {
            !sources
                .iter()
                .any(|source| copy_source_covers(source, candidate))
        })
        .collect();
    assert_eq!(
        (checked > 0, missing.as_slice()),
        (true, &[][..]),
        "checked {checked} root-level directories; these are absent from the runtime stage: {missing:?}"
    );
}

/// The runner overlays every root-level path the image copies.
///
/// A path in the image but absent from the overlay list means the container
/// tests the last commit of it rather than the edit under test, which is the
/// quieter half of the same bug the image checks catch.
#[test]
fn the_runner_overlays_the_root_level_paths_it_copies() {
    let root = repo_root();
    let runner = read(&runner_path());
    let overlaid_trees = loop_items(&runner, "tree");
    let overlaid_files = loop_items(&runner, "file");

    let missing_files: Vec<&str> = OVERLAID_FILES
        .into_iter()
        .filter(|candidate| root.join(candidate).is_file())
        .filter(|candidate| !overlaid_files.iter().any(|item| item == candidate))
        .collect();
    assert!(
        missing_files.is_empty(),
        "the runner does not overlay these root-level files: {missing_files:?}"
    );

    let mut checked = 0usize;
    let missing_trees: Vec<String> = referenced_root_paths()
        .into_iter()
        .filter(|candidate| root.join(candidate).is_dir())
        .filter(|candidate| !EXEMPT_FROM_OVERLAY.contains(&candidate.as_str()))
        .inspect(|_| checked += 1)
        .filter(|candidate| !overlaid_trees.iter().any(|item| item == candidate))
        .collect();
    assert_eq!(
        (checked > 0, missing_trees.as_slice()),
        (true, &[][..]),
        "checked {checked} root-level directories; the runner overlays none of these: {missing_trees:?}"
    );
}

// --- the builder stage knows every workspace member -----------------------

/// The workspace members, parsed from `crates/Cargo.toml`.
///
/// The array is kept on one line deliberately, and both `config-stamp` and
/// `config-manifest`'s own parser already require and document that shape, so
/// this reads it the same way rather than adding a manifest-parsing dependency
/// for one assertion.
fn workspace_members() -> Vec<String> {
    let manifest = read(&repo_root().join("crates/Cargo.toml"));
    let members: Vec<String> = manifest
        .lines()
        .map(str::trim)
        .find_map(|line| {
            let inner = line.strip_prefix("members = [")?.strip_suffix(']')?;
            Some(
                inner
                    .split(',')
                    .map(|entry| entry.trim().trim_matches('"'))
                    .filter(|entry| !entry.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<String>>(),
            )
        })
        .unwrap_or_default();
    assert!(
        !members.is_empty(),
        "positive control: no workspace members were parsed, so nothing is compared. \
         The members array must stay on one line."
    );
    members
}

/// Every workspace member is named in the builder stage.
///
/// THE ASSERTION THAT JUSTIFIED HOLDING THIS SUITE BACK. The builder pre-warms
/// the dependency cache by copying each crate's manifest, stubbing its source,
/// and building once. That list is written out crate by crate, so every new
/// member breaks the image until someone adds three more lines, and the failure
/// is a Docker build error during a push rather than a test.
///
/// It is the sole guard on that invariant, proven rather than asserted: adding
/// `dotfiles-test-support` to the workspace without the Dockerfile edits left
/// the Rust gate green and the lint gate green, and only this check went red.
///
/// Both lists carry a positive control, because comparing two empty lists
/// passes while asserting nothing.
#[test]
fn every_workspace_member_is_named_in_the_builder_stage() {
    let members = workspace_members();
    let dockerfile = Dockerfile::read();
    let builder_text = dockerfile.instructions.join("\n");
    assert!(
        builder_text.contains("Cargo.toml"),
        "positive control: the Dockerfile names no crate manifest at all"
    );

    let unknown: Vec<&String> = members
        .iter()
        .filter(|member| !builder_text.contains(&format!("crates/{member}/Cargo.toml")))
        .collect();
    assert!(
        unknown.is_empty(),
        "these workspace members are absent from tests/docker/Dockerfile's builder stage \
         (add COPY, mkdir and stub lines for each): {unknown:?}"
    );
}
