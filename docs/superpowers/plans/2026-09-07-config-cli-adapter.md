# `config-cli` Adapter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `check-deps.sh` with a `config-cli` binary that drives the
already-complete `deps-core` pipeline, so every install command is argv
rather than a shell string and every non-interactivity flag lives in the
engine rather than in an image that compensates for it.

**Architecture:** One new binary crate whose `deps/` module owns every edge
the pure core needs: a const package catalog, conf-file selection, elevation
resolution, per-wave observation gathering, and two installers. Everything
inside `deps/` knows `deps-core`; nothing outside it does. `main.rs` owns
argv and the exit code and nothing else.

**Tech Stack:** Rust 2024 edition, toolchain pinned to 1.94.0 by
`crates/rust-toolchain.toml` (rustup honours it only when the working
directory is under `crates/`). `clap` 4.6.6 with `derive` and `env`, already
a workspace dependency. `deps-core` and `dotfiles-path` by path. No new
third-party dependencies.

**Spec:** `docs/superpowers/specs/2026-09-07-config-cli-adapter-design.md`.
Read sections 4.1 through 4.3, 5, 6, 8 and 9 before Task 1. That document
corrects itself repeatedly and the corrections are the load-bearing parts:
read the corrected text, not the first-draft claim it quotes.

**Depends on:** `2026-09-07-deps-core-completion-design.md`, which is **done
and pushed** (commits `c5ba11d1..8ee16ecf` plus `709df125`). Its seven fixes
are what make this step buildable.

**Should follow:** `2026-09-07-rust-gate-and-strict-lints.md`. That plan puts
a gate on `cargo clippy`, which nothing enforces today. See Global
Constraints for what to do if it has not landed.

## Global Constraints

Copied verbatim from the spec and from `~/.claude/CLAUDE.md`.

- **`DEBIAN_FRONTEND=noninteractive` goes in the child environment the apt
  installer builds, and its test must run with the ambient variable UNSET.**
  This is the repo's sharpest incident: an unattended bootstrap halted at
  tzdata's debconf prompt while every image and CI leg passed, because
  `Dockerfile.ubuntu` set the variable itself. "The environment was quietly
  compensating for a gap in the engine, so the engine looked correct
  everywhere it was tested and failed on a real machine." A test that
  inherits the variable proves nothing. The same rule generalizes to
  `--noconfirm` for pacman and `-y` for apt: the flag belongs in the argv the
  engine builds, and its test runs with the environment stripped rather than
  prepared.
- **Every effect is `Vec<OsString>` argv, spawned without a shell.** Never
  build a command string. Never interpolate a privilege prefix: elevation is
  data on the `Step`, so the driver decides it, and a `${SUDO}` string prefix
  re-encodes that decision in text where a caller can read a command whose
  privilege does not match its step.
- **`--dry-run` is `describe` over the plan, NOT an `Installer` whose
  `perform` does nothing.** An installer that secretly does nothing is the
  shape that lets a dry run drift from the real run.
- **`Installer::describe` takes `&Step`, not `&InstallAction`.** The spec's
  4.1 snippet shows the old signature; it is stale. The `deps-core`
  completion changed it precisely so a dry run can disclose privileged steps
  before the first password prompt, and an installer handed only the action
  cannot re-derive privilege.
- **`Installer::perform` returns only `Installed`, `InstallFailed`,
  `NotAutomatable` or `Declined`.** `AlreadyPresent` and
  `InstalledButCheckStillFails` are `reconcile`'s to produce from the
  post-loop world; an installer returning either bypasses the re-check that
  catches an install reporting success while changing nothing. This is
  documented on the trait method.
- **The gather contract, written on `deps_core::Observations`, has four rules
  the trait cannot enforce.** Read that doc comment before writing
  `gather.rs`: enumerate every `AnyOf` leaf recursively (an absent key reads
  as `Absent`, so a gather recording only top-level checks silently reports
  every `AnyOf` dependency missing); a failed probe is `Unresolvable`, not
  `Absent`; `Unresolvable` does not block; and resolve roots at the instant
  of use, because installing `oh-my-zsh` creates the directory a later check
  reads.
- **`config deps install` needs `--yes`, or CI hangs.** `deps-check.yml:54`
  and `:66` both pin `--fix --yes`, and `check-deps.sh:572-581` prompts per
  dependency reading stdin. A declined step is `StepOutcome::Declined`.
- **The catalog's `Clone { into }` must equal the dependency's check subject
  byte for byte.** Nothing enforces it: `PackageCatalog` is a bare map and
  neither `plan` nor `action_for` compares them. A catalog pointing the clone
  at a directory whose check reads a file inside it produces a non-converging
  fixpoint where the clone succeeds, the re-gather still reports absent, and
  `Attempted` has already retired the step.
- **`Requirements::validated` must be called by the production catalog, and a
  test must assert the production table validates against the union of all
  four shipped conf files.** This is the test the `deps-core` spec named and
  could not write. It catches a typo neither platform's live run would,
  because a misspelled prerequisite is absent everywhere and therefore looks
  exactly like the legitimate macOS-absent case the design depends on.
- **Keep this crate's tests hermetic.** `config-cli` is the first workspace
  member whose tests genuinely need IO. The host-side Rust gate rests on
  today's members not mutating the real `$HOME` or the real repository. Drive
  every effect against a fixture directory the test owns, the way
  `config-manifest` does with `tempfile::tempdir()`.
- **Clear git's environment in any test that shells out to git**:
  `.env_remove("GIT_DIR")`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_PREFIX`.
  See `.agents/PAPERCUTS.md`: a fixture's `git add .` will otherwise operate
  on the real dotfiles repo and take `~/.cfg/index.lock`.
- **`cargo clippy --locked --all-targets -- -D warnings` must stay at 0.** If
  `2026-09-07-rust-gate-and-strict-lints.md` has not landed, no gate enforces
  this and you must hold it by hand: run it after every task and report the
  result.
- **Every Rust invocation runs from inside `crates/`, never with
  `--manifest-path`.** rustup honours the toolchain pin only when the working
  directory is under `crates/`.
- **`tests/container.test.sh` asserts the Docker builder's `COPY` list
  matches cargo's workspace members.** Adding `config-cli` without its
  `COPY` lines fails that test rather than the Docker build. Add the
  `COPY crates/config-cli/Cargo.toml config-cli/` line, the stub-source entry
  in the dependency-cache `RUN`, and `COPY crates/config-cli/src`.
- **`.scripts/config/config-stamp` reads workspace members with `sed`** and
  requires `members` to stay on ONE line in `crates/Cargo.toml`.
- **Exit 2 is every caller error**, matching the repo-wide convention
  `check-deps.sh:109` and `:116` use and that `deps-docs.test.sh` relies on
  as its oracle for "the parser rejected this flag". Narrowing 2 to one
  condition breaks that oracle. `exit_status` in `deps-core` is the sole
  owner of the verb-to-code mapping: `main.rs` returns
  `Rendered::exit_code`, it does not compute a code.
- **Any change under `crates/` moves the workspace build stamp**, which makes
  installed binaries stale, which the live pre-push gate BLOCKS. After each
  task run `config build`, then confirm `config doctor` exits 0 silently.
- **Use `config commit -F <file>` with a heredoc, NEVER `config commit -m`.**
- **Do NOT run `config status -uall` and do NOT run `config stash`.**
- NEVER `--no-verify`. NEVER disable a test instead of fixing it. NEVER
  commit a red suite.
- NO em dashes anywhere. NO emoji.
- NO single-letter variable names except numeric loop indices `i`, `j`, `k`.
  Closure parameters included: write `.map(|entry| entry.name)`, never `|e|`.
- Comment WHY not WHAT. Doc comments on public items, with `# Errors` where
  they apply. No `unwrap()`/`expect()` in non-test code without a proven
  invariant.
- Every empty-expected assertion needs a positive control FIRST.
- **An assertion is not a test until you have observed it fail against the
  unfixed code.** Every task has an explicit red step; report both
  observations with real output.

## Facts measured before this plan was written

Re-measure the first two yourself before Task 8. The spec says so explicitly,
and it has already drifted once inside one session; my own re-measurement
found 129 references where the spec says 128.

| Fact | Value | How |
|---|---|---|
| `check-deps.sh` consumer files | **41** | `git grep -rl -I 'check-deps.sh' -- . \| grep -v '^docs/' \| wc -l` |
| `check-deps.sh` references | **129** | same with `-rn` |
| Install-command code lines | **34** | `grep -vE '^[[:space:]]*(#\|$)' check-deps.sh \| grep -cE 'brew\|apt-get\|pacman\|rustup\|git clone\|curl\|pip'` |
| Dependency entries | **22** | 16 in `deps.conf`, 3 in `deps-ci.conf`, 2 in `deps-linux.conf`, 1 in `deps-mac.conf` |
| Compensating fixture still present | **yes** | `check-deps.sh:89-90` exports it; `Dockerfile.ubuntu:46` still sets `ENV DEBIAN_FRONTEND=noninteractive` |

## The `deps-core` API this crate consumes

Verified against the real source, since the spec's snippets predate the
completion work:

```rust
// driver.rs
pub trait Installer {
    fn describe(&self, step: &Step) -> ActionDescription;   // &Step, not &InstallAction
    fn perform(&self, action: &InstallAction) -> StepOutcome;
}
pub struct Installers<'wiring> {
    pub ordinary: Box<dyn Installer + 'wiring>,
    pub privileged: Option<Box<dyn Installer + 'wiring>>,
}
pub struct Planning<'inputs> {
    pub manifest: &'inputs Manifest,
    pub manager: PackageManager,
    pub selection: &'inputs Selection,
    pub requirements: &'inputs Requirements,
    pub elevation: Elevation,
    pub packages: &'inputs BTreeMap<DependencyName, PackageMap>,
}
pub fn run_to_fixpoint<GatherFn, Gathered>(
    planning: &Planning<'_>,
    installers: &Installers<'_>,
    mut gather: GatherFn,
) -> Result<(Report, Vec<Event>), PlanError>
where GatherFn: FnMut() -> Gathered, Gathered: Observations;

// render.rs
pub fn render(report: &Report, verb: Verb) -> Rendered;
pub struct Rendered { pub stdout: String, pub stderr: String, pub exit_code: u8 }
pub enum Verb { Check, Install, DryRun }
```

**One wrinkle worth knowing before you fight the compiler.**
`Planning.packages` is typed `&Catalog` where `driver.rs:207` declares
`type Catalog = BTreeMap<DependencyName, PackageMap>` **without `pub`**, while
`plan.rs:26` declares `pub type PackageCatalog` for the identical type. They
are interchangeable structurally, so build a `PackageCatalog` and pass a
reference to it; the private alias is a duplicate, not a barrier.

## File Structure

Module boundaries from the spec's section 5, drawn so each answers who
creates, owns, consumes and decides:

| File | Responsibility |
|---|---|
| `crates/config-cli/src/main.rs` | clap dispatch, exit code. Owns argv. Owns nothing else. |
| `crates/config-cli/src/deps/mod.rs` | Assembles `Planning`, calls `run_to_fixpoint`, renders. |
| `crates/config-cli/src/deps/catalog.rs` | Const: the `PackageCatalog` and the `Requirements` table. |
| `crates/config-cli/src/deps/selection.rs` | Edge, once: conf paths, `Selection`, `PackageManager`. |
| `crates/config-cli/src/deps/elevation.rs` | Edge, once: resolve `Elevation`. |
| `crates/config-cli/src/deps/gather.rs` | Edge, per wave: run each `Check`, build `ObservationMap`. |
| `crates/config-cli/src/deps/installer.rs` | Edge, per action: argv construction and spawning. |
| `.scripts/config/config-deps` | New shim: sources `usage.sh`, execs `config-cli deps "$@"`. |
| `.scripts/config/config-install` | Becomes a 3-line shim to `config deps install`. |
| `.scripts/deps/check-deps.sh` | Deleted, last, in one commit with every consumer. |

**`catalog.rs` holds both the package catalog and the requirement table**,
because they are the same kind of fact.

**`selection.rs` is not optional detail: the spec's 10.1 argument depends on
it.** That argument is "the manifest selection already carries the platform
condition, so the graph needs no condition", which is true only if selection
is correct. The shell resolves it across `check-deps.sh:37` (`DEPS_CONF`
default), `:19` (platform variant), `:459` (concatenation) and `:101-107`
(`--only` parsing), with `deps-ci.conf:3` noting that file is selected only
by an explicit `DEPS_CONF`.

---

## Task 1: The crate skeleton, registered everywhere

An empty binary that builds, dispatches, and is registered in all four places
registration can be missed. Nothing ports yet.

**Files:**
- Create: `crates/config-cli/Cargo.toml`, `crates/config-cli/src/main.rs`
- Modify: `crates/Cargo.toml` (members, ONE line)
- Modify: `tests/docker/Dockerfile` (three places)

**Interfaces:**
- Consumes: nothing; this is first.
- Produces: a `config-cli` binary with `deps` as a subcommand stub, exiting 2
  on an unknown verb. Tasks 2 through 7 fill in `deps/`.

- [ ] **Step 1: Write the failing test**

Create `crates/config-cli/tests/cli_surface.rs`:

```rust
//! The argv surface, asserted against the built binary.
//!
//! Exit 2 for every caller error is a repo-wide convention that
//! tests/deps-docs.test.sh relies on as its oracle for "the parser rejected
//! this flag", so these assertions protect that oracle rather than just this
//! binary.

use std::process::Command;

#[test]
fn no_arguments_is_a_usage_error() {
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .output()
        .expect("the binary runs");

    assert_eq!(run.status.code(), Some(2), "no arguments is exit 2");
}

#[test]
fn an_unknown_subcommand_is_a_usage_error_naming_the_offender() {
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .arg("bogus-verb")
        .output()
        .expect("the binary runs");

    assert_eq!(run.status.code(), Some(2), "an unknown subcommand is exit 2");
    let stderr = String::from_utf8_lossy(&run.stderr);

    // Positive control: stderr must carry something, or the naming
    // assertion below would hold for a binary that says nothing at all.
    assert!(!stderr.is_empty(), "a usage error must explain itself");
    assert!(
        stderr.contains("bogus-verb"),
        "the error must name the offending verb, got {stderr:?}"
    );
}

#[test]
fn deps_accepts_the_check_verb() {
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["deps", "check", "--help"])
        .output()
        .expect("the binary runs");

    assert_eq!(run.status.code(), Some(0), "--help on a real verb exits 0");
}
```

- [ ] **Step 2: Run the test to verify it fails**

```sh
cd ~/crates && cargo test --locked -p config-cli
```

Expected: FAIL, `package ID specification 'config-cli' did not match any
packages`. Record it.

- [ ] **Step 3: Create the manifest and register the member**

`crates/config-cli/Cargo.toml`:

```toml
[package]
name = "config-cli"
version = "0.1.0"
edition = "2024"

[dependencies]
clap = { workspace = true }
deps-core = { path = "../deps-core" }
dotfiles-path = { path = "../dotfiles-path" }

[dev-dependencies]
tempfile = { workspace = true }
```

In `crates/Cargo.toml`, extend `members` **keeping it on one line**:

```toml
members = ["config-cli", "config-manifest", "deps-core", "dotfiles-path"]
```

- [ ] **Step 4: Write `main.rs`**

clap `derive`, with `deps` carrying `check` and `install` and the flags the
spec requires. `--yes` is mandatory or CI hangs; `--dry-run` and `--only` come
from `check-deps.sh`'s existing surface.

```rust
//! The adapter that runs the pure dependency core.
//!
//! Owns argv and the exit code. Every decision about what to install lives
//! in `deps_core`, and every capability lives in `deps/`.
//!
//! Exit 2 is every caller error, matching the repo-wide convention
//! `check-deps.sh` established and `deps-docs.test.sh` uses as its oracle.
//! The verb-to-code mapping has exactly one owner, `deps_core::exit_status`,
//! reached here through `Rendered::exit_code`.

use std::process::ExitCode;

use clap::{Parser, Subcommand};

mod deps;

#[derive(Parser)]
#[command(name = "config-cli", disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check or install the dependencies the manifest names.
    Deps {
        #[command(subcommand)]
        verb: DepsVerb,
    },
}

#[derive(Subcommand)]
enum DepsVerb {
    /// Report what is missing, changing nothing.
    Check(DepsArgs),
    /// Install what is missing.
    Install(DepsArgs),
}

#[derive(clap::Args)]
struct DepsArgs {
    /// Print what would happen, spawning nothing.
    #[arg(long)]
    dry_run: bool,
    /// Approve every step without prompting. Required in CI, which pins it.
    #[arg(long)]
    yes: bool,
    /// Restrict the run to these dependencies, comma-separated.
    #[arg(long, value_delimiter = ',')]
    only: Vec<String>,
}

fn main() -> ExitCode {
    // clap exits 2 itself for a parse error, which is the convention this
    // binary must keep.
    let cli = Cli::parse();
    match cli.command {
        Command::Deps { verb } => deps::run(verb),
    }
}
```

Create `crates/config-cli/src/deps/mod.rs` as a stub that compiles:

```rust
//! Everything inside this module knows `deps_core`. Nothing outside it does.

use std::process::ExitCode;

use crate::DepsVerb;

/// Run one `deps` verb.
///
/// Returns the exit code `deps_core::exit_status` decided, never one computed
/// here.
pub fn run(_verb: DepsVerb) -> ExitCode {
    eprintln!("config-cli: deps is not implemented yet");
    ExitCode::from(2)
}
```

Make `DepsVerb` and `DepsArgs` visible to the module with `pub(crate)`.

- [ ] **Step 5: Run the tests to verify they pass**

```sh
cd ~/crates && cargo test --locked -p config-cli
cd ~/crates && cargo clippy --locked --all-targets -- -D warnings; echo "clippy=$?"
```

Expected: three tests PASS, `clippy=0`. `deps_accepts_the_check_verb` passes
because `--help` is handled by clap before reaching the stub.

- [ ] **Step 6: Register in the Docker builder**

Add to `tests/docker/Dockerfile`, matching the existing shape:

```dockerfile
COPY crates/config-cli/Cargo.toml config-cli/
```

Extend the stub-source `RUN` with `config-cli/src` and
`echo 'fn main() {}' > config-cli/src/main.rs`, and add its removal to the
same `rm -rf` list the other binary gets. Then:

```dockerfile
COPY crates/config-cli/src ./config-cli/src
```

Read the real `RUN` block first and preserve what it already removes.

- [ ] **Step 7: Verify both registration guards**

```sh
cd ~ && bash tests/container.test.sh 2>&1 | tail -3
cd ~ && DOTFILES_ROOT="$HOME" .scripts/config/config-stamp | sort
```

Expected: the container suite PASSES, and `config-stamp` prints four members
including `config-cli`. If it prints three, the `members` array went
multi-line and `sed` stopped matching.

- [ ] **Step 8: Rebuild and commit**

```sh
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
cat > /tmp/msg.txt <<'MSG'
Add the config-cli crate skeleton

An empty binary that builds and dispatches. Nothing ports yet, because
registration has four separate places to miss (the one-line members array
config-stamp reads with sed, two Dockerfile COPY groups, and the
dependency-cache RUN), and discovering a missed one during a port task is
the expensive path.

The argv surface ships with its tests: exit 2 for every caller error is a
repo-wide convention that deps-docs.test.sh uses as its oracle for "the
parser rejected this flag", so those assertions protect that oracle rather
than only this binary.

--yes exists from the start because deps-check.yml pins --fix --yes on two
legs, and a binary without it hangs CI rather than failing it.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add crates/ tests/docker/Dockerfile
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 2: `selection.rs`, the conf-file resolution

The spec's 10.1 argument ("the manifest selection already carries the
platform condition, so the graph needs no condition") is true only if
selection is correct. The shell spreads that logic across four sites, and
nothing tests it.

**Files:**
- Create: `crates/config-cli/src/deps/selection.rs`
- Modify: `crates/config-cli/src/deps/mod.rs` (declare the module)

**Interfaces:**
- Consumes: `deps_core::{Manifest, Selection, PackageManager, RawSelector, parse_manifest, ConfKind}`.
- Produces:

```rust
pub struct ManifestSources { pub paths: Vec<PathBuf> }
pub fn conf_paths(env: &Environment) -> ManifestSources;
pub fn load_manifest(sources: &ManifestSources) -> Result<Manifest, LoadError>;
pub fn resolve_manager() -> PackageManager;
pub fn selection_from(only: &[String], manifest: &Manifest) -> Result<Selection, PlanError>;
```

`Environment` is a small injected struct (`deps_conf: Option<OsString>`,
`deps_local_conf: Option<OsString>`, `platform: Platform`) so the resolution
is testable without setting process environment variables. Task 3's `mod.rs`
reads the real environment once and builds it.

- [ ] **Step 1: Read the four shell sites and record what each does**

```sh
cd ~ && sed -n '15,45p;95,115p;455,465p' .scripts/deps/check-deps.sh
cd ~ && head -6 .scripts/deps/deps-ci.conf
```

Write down in your report: what `DEPS_CONF` defaults to, how the platform
variant is chosen and appended, how `DEPS_LOCAL_CONF` participates, how
`--only` is parsed, and the fact `deps-ci.conf` is selected only by an
explicit `DEPS_CONF`. Those five facts are this module's contract.

- [ ] **Step 2: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// The default run reads deps.conf plus the platform variant.
    #[test]
    fn the_default_run_reads_the_base_and_the_platform_variant() {
        let environment = Environment {
            deps_conf: None,
            deps_local_conf: None,
            platform: Platform::MacOs,
        };

        let sources = conf_paths(&environment);

        // Positive control: the list must not be empty, or the assertions
        // below hold for a resolver that reads nothing.
        assert!(!sources.paths.is_empty(), "a run must read a manifest");
        assert!(
            sources.paths.iter().any(|path| path.ends_with("deps.conf")),
            "the base manifest is always read: {:?}",
            sources.paths
        );
        assert!(
            sources.paths.iter().any(|path| path.ends_with("deps-mac.conf")),
            "the platform variant is appended on macOS: {:?}",
            sources.paths
        );
    }

    /// An explicit DEPS_CONF replaces the base and suppresses the variant.
    ///
    /// deps-ci.conf documents that it is selected only this way, and the CI
    /// leg sets DEPS_LOCAL_CONF to a nonexistent path specifically to
    /// exclude the platform variant. A resolver that appended the variant
    /// anyway would pull in aerospace or oh-my-zsh, which CI does not need.
    #[test]
    fn an_explicit_deps_conf_suppresses_the_platform_variant() {
        let environment = Environment {
            deps_conf: Some("/somewhere/deps-ci.conf".into()),
            deps_local_conf: Some("/nonexistent/deps-platform.conf".into()),
            platform: Platform::Linux,
        };

        let sources = conf_paths(&environment);

        assert!(
            !sources.paths.iter().any(|path| path.ends_with("deps-linux.conf")),
            "an explicit DEPS_CONF must not drag in the platform variant: {:?}",
            sources.paths
        );
    }

    /// A missing conf file is skipped rather than fatal.
    ///
    /// That is how DEPS_LOCAL_CONF=/nonexistent excludes a variant without
    /// pretending the file was empty, and it is the technique the Docker
    /// images use.
    #[test]
    fn a_missing_conf_file_is_skipped_not_fatal() {
        let sources = ManifestSources {
            paths: vec![PathBuf::from("/nonexistent/deps-platform.conf")],
        };

        let manifest = load_manifest(&sources).expect("a missing file is not an error");

        assert_eq!(manifest.entries().len(), 0, "nothing was read, so nothing is present");
    }

    /// `--only` narrows the selection.
    #[test]
    fn only_narrows_the_selection() {
        let manifest = a_manifest_of(&["git", "fzf", "ripgrep"]);

        let narrowed = selection_from(&["git".to_string(), "fzf".to_string()], &manifest)
            .expect("a valid narrowing");

        // Positive control: the unnarrowed selection must contain all three,
        // or "contains git" below proves nothing about narrowing.
        let everything = selection_from(&[], &manifest).expect("an empty --only means all");
        assert!(everything.contains(&dependency("ripgrep")), "the control selects everything");

        assert!(narrowed.contains(&dependency("git")));
        assert!(!narrowed.contains(&dependency("ripgrep")), "ripgrep was excluded");
    }

    /// `--only` naming a dependency the manifest lacks is an error, not a
    /// silent empty run.
    #[test]
    fn only_naming_an_unknown_dependency_is_an_error() {
        let manifest = a_manifest_of(&["git"]);

        selection_from(&["gti".to_string()], &manifest)
            .expect_err("a typo in --only must be rejected");
    }
}
```

Write `a_manifest_of` and `dependency` helpers in the test module, using
`deps_core::parse_manifest` over inline conf text so the fixtures exercise
the real parser rather than a hand-built `Manifest`.

- [ ] **Step 3: Run the tests to verify they fail**

```sh
cd ~/crates && cargo test --locked -p config-cli selection
```

Expected: FAIL to build on the missing module and functions. Record it.

- [ ] **Step 4: Implement `selection.rs`**

Follow what Step 1 found, not this plan's summary of it. Key decisions:

- `Environment` is injected, so no test sets a process environment variable.
  Reading the real environment happens once, in `mod.rs`, which is the only
  place that should touch it.
- A missing conf file is skipped, not fatal, and `load_manifest` says so in
  its doc comment with the `DEPS_LOCAL_CONF=/nonexistent` technique named.
- `resolve_manager` probes for `brew`, `apt-get` and `pacman` in whatever
  order the shell used. Read `check-deps.sh` for that order; a different
  order changes which manager a machine with two of them resolves to.
- `selection_from(&[], manifest)` means everything, matching
  `Selection::all`. A non-empty list that names an unknown dependency is
  `PlanError::UnknownDependency`, not an empty selection.

- [ ] **Step 5: Run the tests and clippy**

```sh
cd ~/crates && cargo test --locked -p config-cli
cd ~/crates && cargo clippy --locked --all-targets -- -D warnings; echo "clippy=$?"
```

Expected: PASS, `clippy=0`.

- [ ] **Step 6: Verify the tests bind**

```sh
cd ~/crates
cp config-cli/src/deps/selection.rs /tmp/selection.rs.good
python3 - <<'PY'
import pathlib
path = pathlib.Path("config-cli/src/deps/selection.rs")
text = path.read_text()
# Make an explicit DEPS_CONF also append the platform variant, which is the
# CI-breaking bug the test exists to catch.
needle = "deps_conf.is_none()"
assert needle in text, "adjust this sabotage to the real control flow"
path.write_text(text.replace(needle, "true", 1))
PY
cargo test --locked -p config-cli selection 2>&1 | grep -cE 'FAILED|panicked'
cp /tmp/selection.rs.good config-cli/src/deps/selection.rs && rm /tmp/selection.rs.good
cargo test --locked -p config-cli selection 2>&1 | tail -1
```

Expected: the sabotaged run fails
`an_explicit_deps_conf_suppresses_the_platform_variant`; the restored run
passes. Adapt the sabotage to the real control flow if the needle differs,
and say how.

- [ ] **Step 7: Rebuild and commit**

```sh
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
cat > /tmp/msg.txt <<'MSG'
Resolve the manifest selection, with tests

The spec's 10.1 argument is that the manifest selection already carries the
platform condition, so the requirement graph needs no condition. That is
true only if selection is correct, and the shell spread the logic across
four sites with nothing testing it: the DEPS_CONF default, the platform
variant, the concatenation, and --only parsing.

An explicit DEPS_CONF suppresses the platform variant, which is not a
tidiness choice: the CI leg sets DEPS_LOCAL_CONF to a nonexistent path
specifically to exclude the variant, and a resolver that appended it anyway
would pull in aerospace or oh-my-zsh that CI does not need. There is a test
that fails when that suppression is removed.

A missing conf file is skipped rather than fatal, which is how a nonexistent
path excludes a variant without pretending the file was empty. The Docker
images use the same technique.

The environment is injected rather than read, so no test mutates process
state to exercise a branch. mod.rs reads the real environment once.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add crates/config-cli/
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 3: `catalog.rs`, the 22-entry install table

The largest single piece of knowledge in the port, and the one the spec says
had no file, no owner and no test: `grep PackageCatalog crates/` returns only
the type alias, the parameter, and test helpers. **Nothing in production
builds one.**

This task also discharges two inherited obligations: the union-of-conf-files
validation test, and the clone-target equality the `deps-core` docs require.

**Files:**
- Create: `crates/config-cli/src/deps/catalog.rs`
- Modify: `crates/config-cli/src/deps/mod.rs`

**Interfaces:**
- Consumes: `deps_core::{PackageCatalog, PackageMap, PackageAvailability, Requirements, RequirementEdgeError, DependencyName, PackageManager, CloneSource, KeyringSource, SourceListEntry, ScriptInstaller, NoInstallReason, CheckPath, PathRoot}`.
- Produces:

```rust
pub fn packages() -> PackageCatalog;
pub fn requirements(known: &BTreeSet<DependencyName>) -> Result<Requirements, Vec<RequirementEdgeError>>;
```

- [ ] **Step 1: Read every install site and tabulate it**

```sh
cd ~ && grep -nE 'brew|apt-get|pacman|rustup|git clone|curl|pip' .scripts/deps/check-deps.sh \
  | grep -vE '^[0-9]+:[[:space:]]*#'
cd ~ && cat .scripts/deps/deps.conf .scripts/deps/deps-linux.conf \
  .scripts/deps/deps-mac.conf .scripts/deps/deps-ci.conf
```

Produce a table in your report, one row per dependency: its name, its check
expression, and its install command per manager. 22 dependencies, 34
install-command code lines. **This table IS the task**; the Rust is
transcription once you have it.

Four dependencies need the non-`Named` availabilities the `deps-core`
completion added:

| Dependency | Availability |
|---|---|
| `zsh-autosuggestions` | `Clone { source: CloneSource::ZshAutosuggestions, into: <CheckPath> }` |
| `node` | `ViaNvm` |
| `pyyaml` | `PipDistribution { id, break_system_packages: true }` |
| `gh` on apt | `AptWithSource { keyring: KeyringSource::GithubCli, list: SourceListEntry::GithubCli }` |

- [ ] **Step 2: Write the failing tests**

Three tests, and the first two are the inherited obligations.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// The production requirement table validates against the union of every
    /// shipped conf file.
    ///
    /// This is the test the deps-core completion spec named and could not
    /// write, because the table it validates did not exist yet. It catches a
    /// typo neither platform's live run would: a misspelled prerequisite is
    /// absent on every platform, so it looks exactly like the legitimate
    /// macOS-absent case the design relies on, plans successfully, orders
    /// nothing, and exits 0.
    #[test]
    fn the_production_requirement_table_validates_against_every_conf_file() {
        let known = every_shipped_dependency();

        // Positive control: the union must be non-empty and must contain a
        // dependency from each file, or validation below passes vacuously.
        assert!(known.len() >= 20, "the union must cover every conf file, got {}", known.len());
        assert!(known.contains(&dependency("git")), "deps.conf entries are present");

        requirements(&known).expect("the production requirement table must validate");
    }

    /// Every clone target equals the check subject of its dependency, byte
    /// for byte.
    ///
    /// Nothing in deps-core enforces this: PackageCatalog is a bare map and
    /// neither plan nor action_for compares them. A clone pointed at a
    /// directory whose check reads a file inside it gives a non-converging
    /// fixpoint, where the clone succeeds, the re-gather still reports
    /// absent, and Attempted has already retired the step.
    #[test]
    fn every_clone_target_equals_its_check_subject() {
        let catalog = packages();
        let manifest = the_shipped_manifest();

        let mut checked = 0;
        for (name, entry) in manifest.entries() {
            for manager in every_manager() {
                if let PackageAvailability::Clone { into, .. } =
                    catalog.get(name).expect("every entry has a package map").resolve(manager)
                {
                    let subject = check_subject(&entry.check)
                        .expect("a clone's dependency must have a path-shaped check");
                    assert_eq!(
                        *into, subject,
                        "the clone target and the check subject must be one value for {name:?}"
                    );
                    checked += 1;
                }
            }
        }

        // Positive control: at least one clone must exist, or this test
        // passes by iterating nothing.
        assert!(checked > 0, "the catalog must contain at least one Clone availability");
    }

    /// Every dependency in every shipped conf file has a catalog entry.
    ///
    /// A missing entry is not a compile error, and `resolve` is total, so a
    /// dependency absent from the catalog would plan as NotAutomatable and
    /// report "no automated install" for something that has one.
    #[test]
    fn every_shipped_dependency_has_a_catalog_entry() {
        let catalog = packages();
        let known = every_shipped_dependency();

        assert!(!known.is_empty(), "the control must find dependencies");
        for name in &known {
            assert!(
                catalog.contains_key(name),
                "{name:?} is in a shipped conf file but has no catalog entry"
            );
        }
    }
}
```

`every_shipped_dependency` reads all four conf files with `include_str!` and
parses them with `deps_core::parse_manifest`. `include_str!` reads at compile
time, so this stays hermetic and needs no filesystem access at run time.
`check_subject` is a small test helper matching the path-carrying `Check`
variants.

- [ ] **Step 3: Run the tests to verify they fail**

```sh
cd ~/crates && cargo test --locked -p config-cli catalog
```

Expected: FAIL to build on the missing module. Record it.

- [ ] **Step 4: Implement `catalog.rs` from your Step 1 table**

Write `packages()` and `requirements()`. Notes:

- The clone target for `zsh-autosuggestions` must be built from the SAME
  string the conf file's check uses. Do not retype it: derive it, or write it
  once as a `const` the test can also see. The `deps-core` completion deleted
  `PathRoot::OhMyZshCustom` precisely so this is `PathRoot::Home` plus the
  relative path `deps.conf` writes.
- `requirements()` calls `Requirements::validated` and returns its error
  rather than panicking, so the caller decides. The known edge today is
  `(zsh-autosuggestions, [oh-my-zsh])`.
- `break_system_packages: true` for `pyyaml` overrides PEP 668. It is a named
  field because blast radius belongs in the type.

- [ ] **Step 5: Run the tests and clippy**

```sh
cd ~/crates && cargo test --locked -p config-cli
cd ~/crates && cargo clippy --locked --all-targets -- -D warnings; echo "clippy=$?"
```

Expected: PASS, `clippy=0`.

- [ ] **Step 6: Verify both obligation tests bind**

```sh
cd ~/crates
cp config-cli/src/deps/catalog.rs /tmp/catalog.rs.good
# Plant the typo the union test exists to catch.
python3 - <<'PY'
import pathlib
path = pathlib.Path("config-cli/src/deps/catalog.rs")
text = path.read_text()
assert text.count('"oh-my-zsh"') >= 1, "adjust to the real spelling in your catalog"
path.write_text(text.replace('"oh-my-zsh"', '"oh-my-zhs"', 1))
PY
cargo test --locked -p config-cli the_production_requirement_table 2>&1 | grep -cE 'FAILED|panicked'
cp /tmp/catalog.rs.good config-cli/src/deps/catalog.rs
# Now break the clone target the other test guards.
python3 - <<'PY'
import pathlib
path = pathlib.Path("config-cli/src/deps/catalog.rs")
text = path.read_text()
needle = "zsh-autosuggestions.zsh"
assert needle in text, "adjust to how your catalog spells the clone target"
path.write_text(text.replace(needle, "zsh-autosuggestions", 1))
PY
cargo test --locked -p config-cli every_clone_target 2>&1 | grep -cE 'FAILED|panicked'
cp /tmp/catalog.rs.good config-cli/src/deps/catalog.rs && rm /tmp/catalog.rs.good
cargo test --locked -p config-cli 2>&1 | grep -E "^test result: ok" | tail -2
```

Expected: both sabotages produce failures, and the restored run is green.
**The second sabotage is the important one**: it turns the clone target from
the file the check reads into its parent directory, which is exactly the
non-converging fixpoint the `deps-core` spec's 10.2 describes.

- [ ] **Step 7: Rebuild and commit**

```sh
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
cat > /tmp/msg.txt <<'MSG'
Build the production package catalog, and discharge two obligations

The catalog is the largest single piece of knowledge in this port: 22
dependencies across four conf files, 34 install-command code lines. Before
this commit nothing in production built a PackageCatalog at all, so the
biggest piece of ported knowledge had no file, no owner and no test.

Two inherited obligations land here because this is where the thing they
guard first exists.

The union-of-conf-files test is the one the deps-core completion spec named
and could not write. It asserts the production requirement table validates
against every shipped conf file, which catches a typo neither platform's
live run would: a misspelled prerequisite is absent everywhere, so it looks
exactly like the legitimate macOS-absent case, plans fine, orders nothing
and exits 0.

The clone-target test asserts every Clone availability points at the same
CheckPath its dependency's check reads, byte for byte. Nothing in deps-core
enforces that, and getting it wrong gives a non-converging fixpoint: the
clone succeeds, the re-gather still reports absent, and Attempted has
already retired the step. The sabotage check plants exactly that error.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add crates/config-cli/
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 4: `elevation.rs` and `gather.rs`, the observation edges

Two edges batched together: both are read-only, both are small, and a
reviewer can judge them as one unit. `gather.rs` is the riskiest edge in the
design because a wrong answer there is silent, which is why `deps-core`
wrote its contract onto the `Observations` trait.

**Files:**
- Create: `crates/config-cli/src/deps/elevation.rs`,
  `crates/config-cli/src/deps/gather.rs`
- Modify: `crates/config-cli/src/deps/mod.rs`

**Interfaces:**
- Consumes: `deps_core::{Elevation, Check, CheckPath, PathRoot, Observation, ObservationMap, Observations}`.
- Produces:

```rust
pub fn resolve() -> Elevation;                       // elevation.rs
pub fn gather(manifest: &Manifest) -> ObservationMap; // gather.rs
```

- [ ] **Step 1: Read the gather contract, and quote it in your report**

```sh
cd ~/crates && sed -n '/A gathered world/,/^pub trait Observations/p' deps-core/src/check.rs
```

Four rules bind you. Restate each in your report with how your
implementation satisfies it. The one most likely to be missed is rule 1:
`ObservationMap::observe` returns `Absent` for any key it does not hold, so a
gather that records only top-level checks silently reports every `AnyOf`
dependency absent. Recurse into `AnyOf`'s `first` and `rest`.

- [ ] **Step 2: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Every AnyOf leaf is recorded, not just the top-level check.
    ///
    /// ObservationMap::observe returns Absent for a key it does not hold, so
    /// a gather that records only top-level checks reports every AnyOf
    /// dependency missing. Three shipped dependencies use AnyOf.
    #[test]
    fn it_records_every_any_of_leaf() {
        let root = tempfile::tempdir().expect("tempdir");
        let present = root.path().join("present.txt");
        std::fs::write(&present, "x").expect("write");

        let check = Check::AnyOf {
            first: Box::new(Check::FileExists(a_path_in(root.path(), "absent.txt"))),
            rest: vec![Check::FileExists(a_path_in(root.path(), "present.txt"))],
        };

        let observations = gather_one(&check, root.path());

        // Positive control: the top-level AnyOf must itself be answered, or
        // the leaf assertions below could hold in a map with one entry.
        assert_eq!(observations.observe(&check), Observation::Present);

        // Both leaves must be recorded individually.
        assert_eq!(
            observations.observe(&Check::FileExists(a_path_in(root.path(), "present.txt"))),
            Observation::Present,
            "the present leaf must be recorded, not inferred"
        );
        assert_eq!(
            observations.observe(&Check::FileExists(a_path_in(root.path(), "absent.txt"))),
            Observation::Absent,
            "the absent leaf must be recorded too"
        );
    }

    /// A failed probe is Unresolvable, not Absent.
    ///
    /// "The interpreter is missing" and "the module is missing" are
    /// different facts with different remedies, and the shell collapsed both
    /// (check-deps.sh:523). This port exists partly to stop that.
    #[test]
    fn a_failed_probe_is_unresolvable_rather_than_absent() {
        // A brew-prefix root on a machine without brew cannot resolve.
        let check = Check::FileExists(CheckPath::new(
            PathRoot::BrewPrefix,
            CheckRelPath::parse("bin/definitely-not-installed").expect("a valid path"),
        ));

        let observations = gather_with_unresolvable_brew(&check);

        assert!(
            matches!(observations.observe(&check), Observation::Unresolvable { .. }),
            "an unresolvable root is a different fact from an absent file"
        );
    }
}
```

Write `gather_one` and `a_path_in` helpers that build a one-entry manifest
and call the real `gather`. `gather_with_unresolvable_brew` injects a root
resolver that fails for `BrewPrefix`, which means `gather` must take its root
resolution as a parameter rather than calling `brew --prefix` directly.
**That injection point is a design requirement, not a test convenience**:
without it this rule is untestable on a machine that has brew.

- [ ] **Step 3: Run the tests to verify they fail**

```sh
cd ~/crates && cargo test --locked -p config-cli gather
```

Expected: FAIL to build. Record it.

- [ ] **Step 4: Implement both modules**

`elevation.rs`: resolve `Elevation` once, per what `check-deps.sh` does.
Read it for the real probe; the three states are already-root, sudo
available, and unavailable.

`gather.rs`: walk every manifest entry's `Check`, recursing into `AnyOf`,
recording each leaf AND each composite. Resolve roots **at the instant of
use**, per rule 4: installing `oh-my-zsh` creates the directory a later check
reads, so a root resolved once at the start is stale by the second wave.
`run_to_fixpoint` calls the gather closure per wave, which is what makes that
possible; do not cache across calls.

Root resolution is injected as a parameter so the `Unresolvable` path is
testable.

- [ ] **Step 5: Run the tests and clippy**

```sh
cd ~/crates && cargo test --locked -p config-cli
cd ~/crates && cargo clippy --locked --all-targets -- -D warnings; echo "clippy=$?"
```

Expected: PASS, `clippy=0`.

- [ ] **Step 6: Verify the AnyOf test binds**

```sh
cd ~/crates
cp config-cli/src/deps/gather.rs /tmp/gather.rs.good
python3 - <<'PY'
import pathlib
path = pathlib.Path("config-cli/src/deps/gather.rs")
text = path.read_text()
# Stop recursing into AnyOf, which is the silent-false bug rule 1 exists for.
needle = "Check::AnyOf"
assert needle in text, "adjust this sabotage to the real recursion"
index = text.index(needle)
path.write_text(text[:index] + "Check::AnyOfDisabled" + text[index + len(needle):])
PY
cargo test --locked -p config-cli gather 2>&1 | grep -cE 'FAILED|panicked|error'
cp /tmp/gather.rs.good config-cli/src/deps/gather.rs && rm /tmp/gather.rs.good
cargo test --locked -p config-cli gather 2>&1 | tail -1
```

Expected: the sabotaged run fails; the restored run passes. If the sabotage
produces a compile error rather than a test failure, that still demonstrates
the recursion is load-bearing, but say which you got: a test that only
catches a compile error is weaker than one that catches a wrong answer, and
if so, add an assertion that fails on a non-recursing but compiling
implementation.

- [ ] **Step 7: Rebuild and commit**

```sh
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
cat > /tmp/msg.txt <<'MSG'
Gather observations, honouring the contract deps-core wrote down

gather is the riskiest edge in this design because a wrong answer there is
silent, which is why deps-core put four rules on the Observations trait
instead of leaving them to be rediscovered.

Rule 1 is the one that bites: ObservationMap::observe returns Absent for any
key it does not hold, so a gather recording only top-level checks reports
every AnyOf dependency missing, and three shipped dependencies use AnyOf.
Every leaf is recorded individually, and there is a test that fails when the
recursion stops.

A failed probe is Unresolvable rather than Absent, because "the interpreter
is missing" and "the module is missing" are different facts with different
remedies, and the shell collapsed both. Root resolution is injected as a
parameter rather than called directly, which is a design requirement rather
than a test convenience: without it that rule is untestable on a machine
that has brew.

Roots resolve at the instant of use rather than once per run, because
installing oh-my-zsh creates the directory a later check reads.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add crates/config-cli/
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 5: `installer.rs`, argv and the `DEBIAN_FRONTEND` fix

The task the spec's 4.2 and 4.3 are about. Every effect becomes
`Vec<OsString>` argv spawned without a shell, and the apt installer sets
`DEBIAN_FRONTEND=noninteractive` in the child environment itself.

**Files:**
- Create: `crates/config-cli/src/deps/installer.rs`
- Modify: `crates/config-cli/src/deps/mod.rs`

**Interfaces:**
- Consumes: `deps_core::{Installer, Installers, InstallAction, Step, ActionDescription, StepOutcome, ExecFailure, SpawnError, PrivilegeRequirement}`, and `dotfiles_path::BoundedText`.
- Produces:

```rust
pub struct Spawning { /* privilege, approval, and a spawn seam */ }
impl Installer for Spawning { /* describe(&Step), perform(&InstallAction) */ }
pub fn argv_for(action: &InstallAction, privilege: PrivilegeRequirement) -> Vec<OsString>;
pub fn wire(elevation: Elevation, approval: Approval) -> Installers<'static>;
```

`argv_for` is separated out and **pure**, so every command shape is testable
without spawning anything. That separation is what makes 4.2's "the dry run
and the real run are the same vector" checkable.

- [ ] **Step 1: Write the failing tests**

The `DEBIAN_FRONTEND` test is the one that matters most, and its whole point
is the stripped environment.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// The apt argv carries -y, and the child environment carries
    /// DEBIAN_FRONTEND, with the ambient variable UNSET.
    ///
    /// This is the repo's sharpest incident: an unattended bootstrap halted
    /// at tzdata's debconf prompt while every image and CI leg passed,
    /// because Dockerfile.ubuntu set the variable itself. The environment was
    /// compensating for a gap in the engine, so the engine looked correct
    /// everywhere it was tested and failed on a real machine. A test that
    /// inherits the variable proves nothing, which is the whole lesson.
    #[test]
    fn the_apt_child_environment_carries_debian_frontend_with_the_ambient_unset() {
        // Positive control: prove the ambient variable really is absent, or
        // this test could pass by inheriting it.
        assert!(
            std::env::var_os("DEBIAN_FRONTEND").is_none(),
            "this test must run with DEBIAN_FRONTEND unset; something set it"
        );

        let environment = child_environment_for(&an_apt_action());

        assert_eq!(
            environment.get(OsStr::new("DEBIAN_FRONTEND")).map(OsString::as_os_str),
            Some(OsStr::new("noninteractive")),
            "the engine sets it, not the image"
        );
    }

    /// Every apt install argv carries -y, and every pacman argv carries
    /// --noconfirm.
    #[test]
    fn non_interactivity_flags_live_in_the_argv() {
        let apt = argv_for(&an_apt_action(), PrivilegeRequirement::Root);
        let pacman = argv_for(&a_pacman_action(), PrivilegeRequirement::Root);

        // Positive control: both must be non-empty, or "contains" below
        // holds for an empty vector.
        assert!(!apt.is_empty() && !pacman.is_empty(), "the control builds real argv");
        assert!(apt.iter().any(|word| word == "-y"), "apt needs -y: {apt:?}");
        assert!(
            pacman.iter().any(|word| word == "--noconfirm"),
            "pacman needs --noconfirm: {pacman:?}"
        );
    }

    /// The privilege prefix is never interpolated into a command word.
    ///
    /// Elevation is data on the Step, so the driver decides it. A ${SUDO}
    /// string prefix re-encodes that decision in text, and a caller can then
    /// read a command whose privilege does not match its step.
    #[test]
    fn privilege_is_a_separate_word_never_a_string_prefix() {
        let elevated = argv_for(&an_apt_action(), PrivilegeRequirement::Root);

        assert_eq!(
            elevated.first().map(OsString::as_os_str),
            Some(OsStr::new("sudo")),
            "root means sudo is its own argv word"
        );
        assert!(
            !elevated.iter().any(|word| word.to_string_lossy().contains("${SUDO}")),
            "no word may carry an uninterpolated prefix: {elevated:?}"
        );
        assert!(
            !elevated.iter().any(|word| word.to_string_lossy().contains("sudo apt-get")),
            "sudo must not be glued to the program: {elevated:?}"
        );
    }

    /// A package name with a space survives as one argv word.
    ///
    /// Nothing in the manifest triggers this today, which is exactly why the
    /// string form would have shipped it unnoticed.
    #[test]
    fn a_name_with_a_space_stays_one_word() {
        let action = a_named_action("weird package");

        let argv = argv_for(&action, PrivilegeRequirement::None);

        assert!(
            argv.iter().any(|word| word == "weird package"),
            "the name is one word, not two: {argv:?}"
        );
    }

    /// describe and perform read the same vector.
    ///
    /// 4.2's third defect: with a string, the dry run and the real run were
    /// different code. With argv they are the same value.
    #[test]
    fn describe_renders_the_argv_perform_would_spawn() {
        let step = an_apt_step_needing_root();
        let installer = Spawning::refusing_to_spawn();

        let described = installer.describe(&step);

        assert_eq!(
            described.privilege,
            PrivilegeRequirement::Root,
            "describe reports the step's privilege, not a guess"
        );
        let expected = argv_for(&step.action, step.privilege);
        let preview = described.command_preview.expect("a spawnable action previews its argv");
        assert_eq!(preview, expected, "the preview IS the argv");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```sh
cd ~/crates && cargo test --locked -p config-cli installer
```

Expected: FAIL to build. Record it.

- [ ] **Step 3: Implement `argv_for` first, and keep it pure**

One `match` over `InstallAction`, returning `Vec<OsString>`. Rules:

- `PrivilegeRequirement::Root` prepends `sudo` as its own word. Never glue it
  to the program name and never interpolate a `${SUDO}` placeholder.
- Apt install: `-y`. Pacman: `--noconfirm`. From your Task 3 table, not from
  memory.
- Every dependency-supplied string (`PackageId`, paths, module names) becomes
  exactly one `OsString`, never split and never quoted.

- [ ] **Step 4: Implement `Spawning` with a spawn seam**

`perform` spawns; `describe` renders. Both read `argv_for`. Give `Spawning` a
seam so tests can construct one that refuses to spawn
(`Spawning::refusing_to_spawn()` above), because a test that actually runs
`apt-get` is not a test this repo can run.

`perform` returns only `Installed`, `InstallFailed`, `NotAutomatable` or
`Declined`. On failure, build `ExecFailure` from the real output:
`Output::status.code()` is `Option<i32>` and is `None` for a signal kill, and
`Command::output()` yields `Vec<u8>` with no UTF-8 guarantee while
`BoundedText::truncating` takes `&str`, so convert with
`String::from_utf8_lossy` and bound it.

Approval: when `--yes` is absent and the step needs approval, prompt; a
refusal is `StepOutcome::Declined`. When `--yes` is present, never read
stdin.

- [ ] **Step 5: Implement `wire`**

`Installers { ordinary, privileged }`, where `privileged` is `None` when
elevation is unavailable. That `None` is what makes "this machine cannot
install with root" a property of the wiring rather than a string test on
command text.

- [ ] **Step 6: Run the tests with the environment stripped**

The `DEBIAN_FRONTEND` test asserts its own precondition, but run it stripped
anyway so a leaked ambient value fails loudly rather than being asserted
away:

```sh
cd ~/crates && env -u DEBIAN_FRONTEND cargo test --locked -p config-cli installer
cd ~/crates && cargo clippy --locked --all-targets -- -D warnings; echo "clippy=$?"
```

Expected: PASS, `clippy=0`.

- [ ] **Step 7: Verify the DEBIAN_FRONTEND test binds**

```sh
cd ~/crates
cp config-cli/src/deps/installer.rs /tmp/installer.rs.good
python3 - <<'PY'
import pathlib
path = pathlib.Path("config-cli/src/deps/installer.rs")
text = path.read_text()
needle = "DEBIAN_FRONTEND"
assert text.count(needle) >= 2, "expected the constant and the test to both name it"
# Remove only the production setting, leaving the test's own reference.
first = text.index(needle)
path.write_text(text[:first] + "DEBIAN_FRONTEND_DISABLED" + text[first + len(needle):])
PY
env -u DEBIAN_FRONTEND cargo test --locked -p config-cli the_apt_child_environment 2>&1 | grep -cE 'FAILED|panicked'
cp /tmp/installer.rs.good config-cli/src/deps/installer.rs && rm /tmp/installer.rs.good
env -u DEBIAN_FRONTEND cargo test --locked -p config-cli installer 2>&1 | tail -1
```

Expected: the sabotaged run fails, the restored run passes. If the first
`DEBIAN_FRONTEND` occurrence in the file is the test rather than the
production constant, adapt the sabotage and say so.

- [ ] **Step 8: Rebuild and commit**

```sh
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
cat > /tmp/msg.txt <<'MSG'
Build effects as argv, and set DEBIAN_FRONTEND in the engine

Every effect is a Vec<OsString> spawned without a shell, replacing command
strings built by printf. That removes three defects that are present today.

Elevation stops being a string prefix. It is data on the Step, so the driver
decides it, and interpolating ${SUDO} into a command string re-encoded that
decision in text where a caller could read a command whose privilege did not
match its step. sudo is now its own argv word, asserted.

Quoting stops being impossible. A package name or path containing a space is
now representable, and there is a test for it. Nothing in the manifest
triggers that today, which is exactly why the string form would have shipped
it unnoticed.

The dry run and the real run stop being different code. argv_for is pure and
both describe and perform read it, so the preview IS the vector that would
be spawned.

DEBIAN_FRONTEND=noninteractive now lives in the child environment the apt
installer builds, and its test asserts the ambient variable is UNSET first.
That is the repo's sharpest incident: an unattended bootstrap halted at
tzdata's debconf prompt while every image and CI leg passed, because
Dockerfile.ubuntu set the variable itself. A test that inherits the variable
proves nothing.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add crates/config-cli/
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 6: `mod.rs`, the assembly, and a working `config-cli deps check`

Wire Tasks 2 through 5 into a binary that actually runs. First end-to-end
behavior.

**Files:**
- Modify: `crates/config-cli/src/deps/mod.rs`, `crates/config-cli/src/main.rs`

**Interfaces:**
- Consumes: everything from Tasks 2 through 5.
- Produces: `config-cli deps check` and `config-cli deps install` returning
  `Rendered::exit_code`.

- [ ] **Step 1: Write the failing test**

An integration test driving the built binary against a fixture manifest, so
it needs no real installs:

```rust
/// `deps check` on a manifest whose dependency is present exits 0 and says so.
#[test]
fn check_exits_zero_when_everything_is_present() {
    let fixture = tempfile::tempdir().expect("tempdir");
    let conf = fixture.path().join("deps.conf");
    // A check that is true on any machine: /bin/sh exists everywhere this
    // repo runs.
    std::fs::write(&conf, "sh|command -v sh|https://example.invalid/sh\n").expect("write");

    let run = std::process::Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["deps", "check"])
        .env("DEPS_CONF", &conf)
        .env("DEPS_LOCAL_CONF", "/nonexistent/deps-platform.conf")
        .output()
        .expect("the binary runs");

    let stdout = String::from_utf8_lossy(&run.stdout);

    // Positive control: the run must produce output, or the exit assertion
    // could hold for a binary that did nothing at all.
    assert!(!stdout.is_empty(), "a check must report something");
    assert_eq!(run.status.code(), Some(0), "everything present is exit 0, stdout: {stdout}");
}

/// `deps check` on a manifest with an absent dependency exits 1.
#[test]
fn check_exits_one_when_something_is_missing() {
    let fixture = tempfile::tempdir().expect("tempdir");
    let conf = fixture.path().join("deps.conf");
    std::fs::write(
        &conf,
        "nonexistent-tool|command -v definitely-not-a-real-binary|https://example.invalid/x\n",
    )
    .expect("write");

    let run = std::process::Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["deps", "check"])
        .env("DEPS_CONF", &conf)
        .env("DEPS_LOCAL_CONF", "/nonexistent/deps-platform.conf")
        .output()
        .expect("the binary runs");

    assert_eq!(run.status.code(), Some(1), "a missing dependency is exit 1");
}

/// `--dry-run` spawns nothing.
///
/// Asserted by pointing the run at a dependency whose install would fail
/// loudly if attempted, and requiring exit to reflect readiness rather than
/// an install failure.
#[test]
fn dry_run_spawns_nothing() {
    let fixture = tempfile::tempdir().expect("tempdir");
    let conf = fixture.path().join("deps.conf");
    std::fs::write(
        &conf,
        "nonexistent-tool|command -v definitely-not-a-real-binary|https://example.invalid/x\n",
    )
    .expect("write");

    let run = std::process::Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["deps", "install", "--dry-run", "--yes"])
        .env("DEPS_CONF", &conf)
        .env("DEPS_LOCAL_CONF", "/nonexistent/deps-platform.conf")
        .output()
        .expect("the binary runs");

    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(!stdout.is_empty(), "a dry run must say what it would do");
    assert_ne!(
        run.status.code(),
        Some(3),
        "exit 3 means an attempt failed, and a dry run attempts nothing: {stdout}"
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```sh
cd ~/crates && cargo test --locked -p config-cli --test deps_end_to_end
```

Expected: FAIL, because `deps::run` is still the stub returning 2. Record it.

- [ ] **Step 3: Implement the assembly**

In `mod.rs`: read the environment once, build `Environment`, resolve the conf
paths, load the manifest, resolve the manager and elevation, build the
catalog and requirements, build `Selection` from `--only`, assemble
`Planning`, wire the installers, and call `run_to_fixpoint` with a gather
closure. Then `render(&report, verb)` and return `Rendered::exit_code`.

Write both streams: `print!("{}", rendered.stdout)` and
`eprint!("{}", rendered.stderr)`. `main` drains a value; it decides nothing.

For `--dry-run`, call `describe` over the plan rather than
`run_to_fixpoint`, per 6.2 and the Global Constraint. Do NOT build an
installer whose `perform` does nothing.

Handle `requirements()`'s error by reporting every offending edge and exiting
2: a bad requirement table is a caller error in the same sense a bad flag is.

- [ ] **Step 4: Run everything**

```sh
cd ~/crates && cargo test --locked -p config-cli
cd ~/crates && cargo clippy --locked --all-targets -- -D warnings; echo "clippy=$?"
```

Expected: PASS, `clippy=0`.

- [ ] **Step 5: Exercise it against the real manifest, read-only**

```sh
cd ~/crates && cargo run --locked -p config-cli -- deps check; echo "check_exit=$?"
cd ~/crates && cargo run --locked -p config-cli -- deps install --dry-run --yes; echo "dry_exit=$?"
```

Expected: both produce sensible output on this machine. `check_exit` is 0 or
1 depending on what is installed; `dry_exit` must not be 3. **Neither may
spawn an installer**: `--dry-run` describes, and `check` never installs. If
you see a package manager run, stop and fix it before continuing.

- [ ] **Step 6: Verify the dry run really is `describe`**

```sh
cd ~/crates && grep -n "describe\|run_to_fixpoint" config-cli/src/deps/mod.rs
```

Expected: the `--dry-run` path reaches `describe` and does NOT reach
`run_to_fixpoint`. Quote the lines in your report. A dry run that goes
through the fixpoint loop with a no-op installer is the shape the spec
forbids.

- [ ] **Step 7: Rebuild and commit**

```sh
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
cat > /tmp/msg.txt <<'MSG'
Assemble the adapter, and run deps check end to end

mod.rs reads the environment once, resolves the manifest, the manager and
elevation, builds the catalog, assembles Planning, and hands the whole thing
to run_to_fixpoint with a gather closure. Then it renders and returns the
exit code deps-core decided.

main drains a value rather than computing one. The verb-to-code mapping
keeps exactly one owner, deps_core::exit_status, which is what stops status
1 from coming to mean eight things again.

--dry-run calls describe over the plan rather than running the fixpoint with
a no-op installer. An installer whose perform secretly does nothing is the
shape that lets a dry run drift from the real run, and there is a check in
this task that the dry-run path does not reach run_to_fixpoint at all.

The end-to-end tests drive fixture manifests in a temporary directory, so
they need no real installs and touch neither the real HOME nor the real
repository. That keeps this crate hermetic, which the host-side Rust gate
depends on.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add crates/config-cli/
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 7: The `config deps` shim and the `config install` port

Make the binary reachable through the `config` dispatcher, without touching
`check-deps.sh` yet. Keeping these separate from Task 8 means the retirement
commit contains only deletions and reference updates.

**Files:**
- Create: `.scripts/config/config-deps`
- Modify: `.scripts/config/config-install`

**Interfaces:**
- Consumes: the `config-cli` binary on `PATH`.
- Produces: `config deps <verb>` and a `config install` that delegates.

- [ ] **Step 1: Read the dispatcher contract and an existing subcommand**

```sh
cd ~ && cat .scripts/config/config-doctor
cd ~ && cat .scripts/config/config-install
```

Note the `# usage:` and `# help:` header conventions: `usage.sh` prints the
usage block out of the calling script, and `config help` reads the `# help:`
lines. A new subcommand without those is invisible to `config help` and has
no usage text.

- [ ] **Step 2: Write the failing test**

`tests/config-docs.test.sh` and `tests/deps-docs.test.sh` already harvest
documented flags and probe them. Check what they assert, then add to whichever
suite owns subcommand discovery:

**The harness's real helper names**, verified in `tests/lib.sh`:
`assert_equals` (:205), `assert_contains` (:218), `assert_succeeds` (:234),
`skip` (:265), `finish` (:322). There is **no `assert_eq` and no `describe`**,
and all of them take the DESCRIPTION FIRST. Read `tests/lib.sh` and an
existing small suite before writing; the harness wins over this plan.

```sh
assert_contains "config help must list deps, or the subcommand is invisible" \
    "$(config help 2>&1)" "deps"

usage_output=$(config deps --not-a-real-flag 2>&1); usage_status=$?
assert_equals "a rejected flag is exit 2, the repo-wide convention" \
    "2" "$usage_status"
```

Confirm the argument order against `tests/lib.sh` before running: a helper
called with expected and actual swapped still passes and reports backwards
when it fails, which is worse than failing outright.

- [ ] **Step 3: Run it to verify it fails**

Expected: FAIL, because `config-deps` does not exist and `config deps` falls
through to `git deps`, which errors differently. **Record the actual error**:
that fallthrough is the hazard Task 8's atomicity requirement is about, and
seeing it once here is worth more than reading about it.

- [ ] **Step 4: Write `config-deps`**

```sh
#!/bin/sh
#
# usage: config deps <check|install> [--dry-run] [--yes] [--only a,b]
#
# help: deps      Check or install the dependencies the manifest names
#
# A shim, deliberately. Every decision lives in config-cli, and this file
# exists so `config deps` resolves through the dispatcher rather than falling
# through to git.

set -eu

# shellcheck source=usage.sh
. "$(dirname "$0")/usage.sh"

exec config-cli deps "$@"
```

Match the real header conventions from Step 1; the block above is the shape,
not necessarily the exact comment syntax the other subcommands use.

- [ ] **Step 5: Port `config-install` to a shim**

Per the spec's 7.3, it becomes a three-line delegation. Read it first to see
what else it does; if it does more than dependency installation, delegate
only that part and say what you kept.

- [ ] **Step 6: Verify both**

```sh
cd ~ && config help 2>&1 | grep -i deps
cd ~ && config deps check; echo "deps_check_exit=$?"
cd ~ && config deps --not-a-real-flag 2>&1 | head -3; echo "bad_flag_exit=$?"
cd ~ && bash tests/config-docs.test.sh 2>&1 | tail -2
cd ~ && bash tests/deps-docs.test.sh 2>&1 | tail -2
```

Expected: `deps` appears in `config help`; `config deps check` runs the
binary; a bad flag exits 2; both doc suites PASS.

- [ ] **Step 7: Commit**

```sh
cd ~
cat > /tmp/msg.txt <<'MSG'
Reach config-cli through the dispatcher

config deps resolves to the binary, and config install delegates to it.

Separate from the retirement commit on purpose: keeping the shims here means
that commit contains only deletions and reference updates, so a reviewer can
see the rename as a rename.

The shim carries usage and help headers because usage.sh prints the usage
block out of the calling script and config help reads the help lines. A
subcommand without them is invisible to config help and has no usage text,
and deps-docs.test.sh harvests documented flags and probes them.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add .scripts/config/ tests/
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 8: Retire `check-deps.sh`, atomically, and delete the compensating fixture

One commit. Atomicity is a correctness requirement, not tidiness: the
`config` dispatcher falls through to `git` for any unmatched verb, so a
window where `check-deps.sh` is gone before its replacement is installed
silently reinterprets a command as a git verb.

**Files:**
- Delete: `.scripts/deps/check-deps.sh`
- Modify: every consumer the re-measurement finds
- Modify: `.scripts/deps/docker/Dockerfile.ubuntu` (remove the `ENV` line)

**Interfaces:**
- Consumes: Task 7's working `config deps`.
- Produces: a repository with no `check-deps.sh`.

- [ ] **Step 1: Re-measure, and do not trust any number**

The spec says to re-run these rather than trusting it, and it was right: it
says 128 references and I measured 129 the same day.

```sh
cd ~ && export GIT_DIR=$HOME/.cfg GIT_WORK_TREE=$HOME
git grep -rl -I 'check-deps.sh' -- . | grep -v '^docs/' | wc -l
git grep -rn -I 'check-deps.sh' -- . | grep -v '^docs/' | wc -l
git grep -rl -I 'check-deps.sh' -- . | grep -v '^docs/'
```

Record all three outputs. The file list is your work list.

- [ ] **Step 2: Verify the compensating fixture is still there**

```sh
cd ~ && grep -n "DEBIAN_FRONTEND" .scripts/deps/docker/Dockerfile.ubuntu
```

Expected: `ENV DEBIAN_FRONTEND=noninteractive` at roughly line 46. **This
line is why the original hang passed every gate.** Task 5 moved the variable
into the engine and proved it with the ambient value unset, so this line can
go, and removing it is what makes a regression of that incident visible
again.

Do NOT delete the image. `test-local.sh:61` builds it on every local run and
`deps-harness.test.sh:130` pins its `ENTRYPOINT` and `CMD`. Only the `ENV`
line goes.

- [ ] **Step 3: Update every consumer**

Work through Step 1's file list. Categories, so you can tell a real update
from a doc mention:

- **Callers** that invoke it: replace with `config deps check` or
  `config deps install`, preserving flags. `--fix` becomes `install`.
- **Tests** that assert on it: retarget to the new command. Do NOT weaken an
  assertion to make it pass; if a test cannot be retargeted, say why.
- **Workflows**: `deps-check.yml:54` and `:66` pin `--fix --yes`. The new
  form must keep `--yes`, or the leg hangs rather than failing.
- **Docs and READMEs**: update the command text.

- [ ] **Step 4: Delete the script and confirm nothing references it**

```sh
cd ~ && config rm .scripts/deps/check-deps.sh
git grep -rn -I 'check-deps.sh' -- . | grep -v '^docs/' | wc -l
```

Expected: **0**. A nonzero count means a consumer was missed and the
dispatcher will reinterpret it as a git verb. Do not proceed until it is 0.
References under `docs/` are historical narrative and may stay; if any
`docs/` reference describes current behavior rather than history, update it
and say which.

- [ ] **Step 5: Run absolutely everything**

```sh
cd ~/crates && cargo test --locked
cd ~/crates && cargo clippy --locked --all-targets -- -D warnings; echo "clippy=$?"
cd ~ && bash tests/run-all.sh 2>&1 | tail -5
```

Expected: all green. This is the commit that can break the most, so a red
suite here stops the task.

- [ ] **Step 6: Run the bootstrap gates, which are this step's acceptance test**

The spec's section 9: "the rewrite is not done until a bare image fully
initializes through the documented one-liner."

```sh
cd ~ && bash .scripts/deps/test-local.sh 2>&1 | tail -20
```

Expected: the images build and the bootstrap legs pass. If Docker is not
running, say so and mark this step blocked rather than skipping it silently:
this is the acceptance test, and a plan that reports done without it is
reporting on the wrong thing.

Note the spec's section 8 decision: the binary reaches the images through the
`/seed` mount, which exists for the two curl legs and needs per-image work
for `arch`. If the `arch` leg fails for want of a wrapper entrypoint, that is
section 8.2's known gap; report it and whether you closed it.

- [ ] **Step 7: Commit, atomically**

```sh
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
cat > /tmp/msg.txt <<'MSG'
Retire check-deps.sh, and delete the fixture that hid its worst bug

One commit, because atomicity is a correctness requirement here rather than
tidiness: the config dispatcher falls through to git for any unmatched verb,
so a window where check-deps.sh is gone before its replacement is installed
silently reinterprets a command as a git verb.

Re-measured rather than trusted, which the spec asks for and which was
justified: the spec says 128 references and the count was 129 the same day.
The consumer count has drifted twice inside this project already.

Dockerfile.ubuntu's ENV DEBIAN_FRONTEND=noninteractive is removed. That line
is why the original hang passed every gate: an unattended bootstrap halted
at tzdata's debconf prompt while every image and CI leg went green, because
the image set the variable the engine failed to set. The engine sets it now,
proved with the ambient value unset, so the compensating line can go, and
removing it is what makes a regression of that incident visible again.

The image itself stays. test-local.sh builds it on every local run and
deps-harness.test.sh pins its ENTRYPOINT and CMD, so deleting it would be a
different change with different consequences.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add -A
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

`config add -A` is deliberate here and is the ONE place this plan uses it:
the commit spans deletions and edits across dozens of files and must be
atomic. Run `config diff --cached --stat` before committing and confirm the
file list matches Step 1's work list plus the deletion. **Do not run
`config status -uall`.**

---

## Self-Review

**Spec coverage.** Section 4.1 (`Installer`, one port, two instances) to Task
5. 4.2 (argv, 61 sites, three defects) to Task 5. 4.3 (`DEBIAN_FRONTEND`) to
Tasks 5 and 8. Section 5's module table to Tasks 1 through 6, one module per
task or batched pair. Section 6 (atomic retirement, re-measured) to Task 8.
Section 8's `/seed` seam is touched in Task 8 Step 6, which reports the
`arch` gap rather than pretending it is closed. Section 9's two inherited
obligations to Task 3, and its gate note to the Global Constraints.

Section 7's two corrections are **not** tasks here: both are explicitly
"owned by step 5's spec" and are planned in
`2026-09-07-tmux-and-zsh-scripts.md`.

**Placeholder scan.** No TBD, no "similar to Task N". Task 3 Step 1 tells the
implementer to build the 22-row table by reading the shell rather than
transcribing a table from this plan, and says "this table IS the task". That
is deliberate: 22 dependencies times per-manager commands is more detail than
a plan can carry accurately, the shell is the contract, and a wrong literal
here would be worse than an instruction to go read it.

**Type consistency.** `Environment`/`ManifestSources` defined in Task 2's
Interfaces and used in Tasks 3 and 6. `argv_for(&InstallAction,
PrivilegeRequirement) -> Vec<OsString>` consistent between Task 5's
interface, its tests, and Task 6's dry-run path. `describe(&Step)` is used
everywhere, matching the real post-completion signature rather than the
spec's stale snippet. `packages()`/`requirements()` from Task 3 feed Task 6's
`Planning`.

**Three risks worth naming.**

1. **Task 3 is the biggest single task** and could warrant splitting per
   manager if it runs long. Its tests are the three obligations, so a split
   should keep them together in the final piece.
2. **Task 8 cannot fully verify without Docker.** Step 6 says to report
   blocked rather than skip, because that step is the acceptance test.
3. **The `arch` image lacks a `/seed` wrapper** per section 8.2. Task 8 Step
   6 surfaces it rather than assuming it away.

**Ordering.** Task 1 first. Tasks 2, 3 and 4 are independent of each other.
Task 5 is independent of 2 through 4 but is what Task 6 needs most. Task 6
needs 2 through 5. Task 7 needs 6. Task 8 needs 7 and is last.
