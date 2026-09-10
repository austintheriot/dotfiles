# Blockers and Workspace Foundation Implementation Plan

> **STATUS 2026-09-10: SHIPPED. Do not execute.** Verified against the tree.
>
> Carries forward the eight tasks of
> `2026-09-06-blockers-and-build-infrastructure.md`, all verified, plus its
> own three additions:
>
> - `dotfiles-path` exists with `lib.rs`, `rel.rs`, `name.rs`, `bounded.rs`.
>   `CheckRelPath::parse` at `rel.rs:108`, `PathError` at `:16`, 27 unit
>   tests passing. The crate grew past the plan: `name.rs` and `bounded.rs`
>   were later additions.
> - The stamp verdict owner shipped: `config-manifest/src/stamp.rs:70`
>   `verify`, `:109` `render`, `:35` `StampVerdict`, `:167`
>   `parse_stamp_line`, with `workspace_stamps` at `git.rs:109`.
> - The bootstrap toolchain seam shipped in a **stronger shape than
>   specified**. The plan asked for an optional `BOOTSTRAP_PREBUILT_BIN` in
>   `bootstrap-curl-entrypoint.sh`. What exists is
>   `deps/docker/seed-prebuilt.sh`, where the variable is **required**
>   (`:18`: "REQUIRED, not optional. It defaulted to empty..."), consumed at
>   seven sites in `deps-check.yml` and gated by
>   `tests/bootstrap-harness.test.sh:375-413`. The directory also moved from
>   `.scripts/deps/docker/` to `deps/docker/`.


> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the four verified fail-open blockers, then lay the Cargo
workspace foundation the pure-core architecture needs, so that the large
`deps-core` port has a green, correctly-gated repo to land in.

**Architecture:** Tasks 1 to 4 separate IO from decision in shell, which is
the direct cause of three of the four blockers. Tasks 5 to 8 narrow the
build's declared inputs to its true inputs, give the stamp one computation
owner and one policy owner, and add the shared-primitives crate that
`deps-core` will depend on instead of reaching into `config-manifest`.

**Tech Stack:** POSIX sh (`.scripts/config/*`, `setup.sh`), zsh
(`tests/leak-check.sh`), bash (`tests/lib.sh`, `tests/*.test.sh`), Rust
2024 edition, Cargo workspaces, Docker.

**Spec:** `docs/superpowers/specs/2026-09-06-pure-core-architecture.md`
(sections 7.1, 7.4 steps 0 to 2, 7.5, 7.5a, 8.1, 8.2)

**Supersedes:** `docs/superpowers/plans/2026-09-06-blockers-and-build-infrastructure.md`.
That plan's Tasks 1 to 8 are **carried forward verbatim and are still
correct**; this document does not restate them. It records the four deltas
the spec revision introduced, adds two tasks the revision requires, and
drops Task 9. Read the prior plan for Tasks 1 to 8's TDD steps; read this
one for what changed and what is new.

## Global Constraints

Copied verbatim from the prior plan, because every one still applies and a
paraphrase is how a constraint gets lost.

- No em dashes anywhere. Use a comma, a colon, parentheses, or two hyphens.
- No emoji anywhere.
- No single-letter variable names, except numeric loop indices (`i`, `j`, `k`)
  and math or geometry values. Lambda parameters get no exception.
- Comment why, not what. No comment that restates the code.
- Never `--no-verify`. Never disable a test instead of fixing it.
- Pure core, IO at the edges, dependency-injectable. The five modules
  `manifest`, `check`, `plan`, `path`, `tree` currently contain **zero**
  references to `std::fs`, `std::process`, `std::env`, `std::io`, or
  `Command::new` (verified: 27 in `git.rs`, 10 in `main.rs`, 37 total). New
  code holds that line.
- **A pure Rust module earns its place only if the Rust binary is the sole
  producer of that value.** This is why the stamp *computation* stays shell
  (Task 6) while the stamp *verdict* becomes Rust (Task 6a).
- In shell: a function that performs IO must not also decide. It returns its
  result to the caller and the caller decides. This fusion is the direct cause
  of two of the four blockers.
- Tests inject through existing env seams (`LEAK_PATTERN_FILE`,
  `LEAK_ALLOW_FILE`, `DEPS_CONF`, `DEPS_LOCAL_CONF`, `DOTFILES_ROOT`,
  `CONFIG_BIN_DIR`, `CARGO_TARGET_DIR`, `DEPS_FORCE_ROOT`). Never read
  `~/.claude/local/` from a test.
- **Every empty-expected assertion needs a positive control.** An
  `assert_equals 'no X' '' "$(cmd)"` passes when `cmd` breaks for an
  unrelated reason, so assert first that the pipeline produced something,
  then assert the narrow property.
- `tests/leak-check.sh` is `#!/bin/zsh`: arrays and `${(f)}` are available.
  Two verified zsh facts: `status` is a **read-only** variable, so a script
  assigning to it aborts; and `${pipestatus[1]}` does not survive a command
  substitution assignment, because the pipeline runs in a subshell. Do not set
  `pipefail` globally there: six of its nine grep-terminated pipelines return
  non-zero on a clean scan by design. `tests/lib.sh` and `*.test.sh` are bash
  with `set -u` and NOT `set -e` (verified), where `status` is an ordinary
  name. `.scripts/config/*`, `setup.sh`, `.scripts/platform.sh` and
  `.scripts/deps/check-deps.sh` are POSIX sh.
- **A pipeline reports only its last command's status.** Verified: a `curl`
  404 piped into `sh` exits 0. Any test that asserts a piped command
  succeeded must check the producer separately. This bit the bootstrap gate
  once already (spec 7.5a).
- Every task ends green: `~/tests/run-all.sh` passes before the commit.
- Any new or renamed path must match `TRIGGER_PATHS` in `tests/pre-push:38`,
  or the suite that reads it will not run at pre-push.

---

## What changed from the prior plan

Four deltas, each traceable to a specific spec section. Nothing else about
Tasks 1 to 8 changed.

### Delta 1: Task 9 is dropped (spec 8.2)

Task 9 rewrote `tmux-update-window-names.sh`'s `--all` path in shell. Under
the revised architecture that script becomes a Rust binary in spec step 5, so
the shell rewrite would be written and then discarded.

The measured logic survives and moves to spec step 5: guard on `.git`
existing before any git call, then read `.git`, `commondir` and `HEAD`
directly, with `git rev-parse` retained as the fallback. **Re-verified during
the spec revision**: 21 unique window directories, 21 of 21 agree with
`git branch --show-current`, zero fallbacks needed, 17 of the 21 being linked
worktrees whose `.git` is a file.

Task 8 (`parse_git_dirty`) **stays**, because `.zshrc` remains shell
permanently so that work is never wasted. Re-verified: still unfixed at
`.zshrc:208-213`, still called from `PS1` at `:224` under `prompt_subst`. The
replacement measures 44.0 to 44.7 ms p50 against 250 to 394 ms for the
current `git status`, cache-dependent.

### Delta 2: Task 5 gains a fourth crate (spec 7.1)

The prior plan created `crates/Cargo.toml` with one member. The spec adds
`dotfiles-path`, and the reason is a dependency-direction problem the prior
plan could not have known about: spec 5.2 wants `deps-core` to reuse
`RelPath`, which lives in `config-manifest`. Both available edges are wrong.
`deps-core` depending on `config-manifest` makes the dependency-manifest
domain depend on the git-sync domain and drags in a 595-line `git` module it
never calls; the reverse is inverted.

This lands in Task 5a rather than inside Task 5, because a reviewer could
reasonably approve the workspace and reject the crate split, or the reverse.

### Delta 3: the stamp gets a policy owner (spec 8.1)

The prior plan's Task 6 correctly made `config-stamp` the owner of the stamp
**computation** and kept it in shell for the circularity reason. The spec
revision found that `pre-push` is a **second writer** of the comparison
policy: it spells `git rev-parse --verify --quiet "$ref:crates/config-manifest"`
itself, with the crate path **hardcoded**, while `config-stamp` takes the
crate as a parameter. The two agree only through an invariant documented in
prose in one file and depended on silently in the other.

Task 6 is unchanged. Task 6a is new: `config-stamp --ref`, a pure Rust
verdict function, and `pre-push` demoted to a consumer.

### Delta 4: the bootstrap gate must survive the port (spec 7.5a)

Added in commit `35d8efd7` and already green: 21 checks on bare Debian, 20 on
bare Arch, both through the real `curl ... | sh` one-liner. Those containers
carry **no Rust toolchain**, so after spec step 3 they cannot build the
replacement for `check-deps.sh`.

Task 9a adds the build stage now, while the gate is green and the change is
isolated, rather than during the port when a failure has two possible causes.

---

## What is NOT in this plan, and why

- **Spec step 3 (`deps-core` plus adapters) and beyond.** That is the largest
  application of the architecture, it carries an 18-consumer rename blast
  radius (spec 7.4 step 3), and it needs its own plan. This plan exists to
  give it a green repo with correct gates.
- **`stamp::fold` and `stamp::parse_members` in Rust.** Deleted before they
  were written, and the reasoning holds: `config-build` calls `config-stamp`
  to decide what to compile, so the stamp must be computable before any binary
  exists. A Rust owner is circular. Task 6a adds a Rust *verdict*, which is a
  different value with a different producer: `pre-push` never builds.
- **The four sourced `tmux-*.sh` scripts.** `.zshrc` sources them and two
  define nine shell functions for the calling shell. A separate process cannot
  define a shell function or mutate its parent's environment. Mechanism, not
  judgment. Spec 7.4 step 5 also found `tmux-split.sh` depends on **sourced
  `$1` inheritance** from `tmux-start.sh:34`, verified, which the prior plan
  did not know.
- **The exit-code redesign (spec 5.4).** It belongs with `deps-core`, and it
  deliberately turns two currently-green assertions red
  (`check-deps.test.sh:130` and `:141`). Doing it here would leave the repo
  red between plans.

---

## File Structure

**Tasks 1 to 4 (blockers), unchanged from the prior plan:**

| File | Responsibility after this plan |
|---|---|
| `tests/leak-check.sh` | Scan decisions separated from git IO. Fails closed on a failed scan, a missing pattern file, and a path that produced no hunk. |
| `tests/leak-check.test.sh` | The three fail-open regression tests plus `SKIP_LEAK_CHECK` truthiness. |
| `tests/lib.sh` | File-backed tally, pure verdict and summary functions, `finish` as the IO edge. Reports the real exit code. |
| `tests/skip-reporting.test.sh` | Unit tests for the pure verdict, plus the zero-assertion and subshell-counting cases. |
| `tests/githooks-installed.test.sh` plus five more | `skip` rather than a bare `printf`. |

**Tasks 5 to 6a, 7, 8 (foundation):**

| File | Responsibility after this plan |
|---|---|
| `crates/Cargo.toml`, `crates/Cargo.lock`, `crates/rust-toolchain.toml` | New. Workspace root, shared resolution, exact toolchain pin. |
| `crates/dotfiles-path/` | **New crate.** Validated newtypes with no dependencies and no IO. What `deps-core` will depend on instead of `config-manifest`. |
| `.scripts/config/config-stamp` | Sole owner of the stamp **computation**. Per-crate, and gains `--ref`. |
| `.scripts/config/config-build` | Builds every workspace crate, re-stamps all. |
| `.scripts/config/config-doctor` | New. Thin wrapper for the doctor subcommand. |
| `crates/config-manifest/src/doctor.rs` | New. Pure diagnosis and render. No IO. |
| `crates/config-manifest/src/stamp.rs` | **New.** Pure stamp **verdict**. `pre-push` is its only consumer. |
| `crates/config-manifest/src/git.rs` | Gains `workspace_stamps`, the temp-index write-tree read. |
| `tests/pre-push` | Iterates crates, ref-scoped via `config-stamp --ref`, absolute binary paths. Stops spelling `rev-parse`. |
| `.scripts/deps/docker/Dockerfile.bootstrap-curl` | Gains a Rust toolchain build stage. |
| `.scripts/deps/docker/Dockerfile.bootstrap-curl-arch` | Same. |
| `.claude/rules/dotfiles-tests.md` | Documents the edit-build-test loop. |

---

## Tasks 1 to 8: carried forward

**Execute these from the prior plan**, at
`docs/superpowers/plans/2026-09-06-blockers-and-build-infrastructure.md`:

| Task | Title | Prior plan line | Status |
|---|---|---|---|
| 1 | The leak guard observes what its subshells learned | 136 | Unchanged |
| 2 | The guard's skip policy fails closed | 477 | Unchanged |
| 3 | `assert_succeeds` reports the real exit code | 657 | Unchanged |
| 4 | A pure verdict, and a suite that asserts nothing fails | 759 | Unchanged |
| 5 | Workspace, toolchain pin, and a narrower stamped tree | 1046 | Unchanged; see Task 5a |
| 6 | Per-crate, ref-scoped stamps | 1269 | Unchanged; see Task 6a |
| 7 | `config doctor` reports stale binaries | 1726 | Unchanged |
| 8 | The prompt stops running a full `git status` | 2250 | Unchanged |

All eight were verified against the live repo while writing this document:
`proptest-regressions/plan.txt` is still tracked, `crates/Cargo.toml` and
`crates/rust-toolchain.toml` still do not exist, `Cargo.toml` still has
`edition = "2024"` with no `rust-version`, and `parse_git_dirty` is still
unfixed. Their preconditions hold.

Tasks 1 to 4 are the highest priority work in the repo and are independent of
every architectural decision. Three are leak-guard fail-opens on a public
repository.

---

## Task 5a: The `dotfiles-path` crate

**Why this exists.** Spec 5.2 specifies `CheckPath { root: PathRoot, rest:
CheckRelPath }` for `deps-core`, and spec 7.1 states the dependency-direction
problem: `RelPath` lives in `config-manifest`, and neither
`deps-core -> config-manifest` nor the reverse is acceptable. A shared
primitives crate with no dependencies and no IO is the third edge.

`RelPath` itself **stays in `config-manifest`** for now, because moving a type
its `git.rs` and `tree.rs` depend on is a refactor with no test-visible
behavior change, and doing it in the same task as creating the crate makes a
failure ambiguous. This task creates the crate and puts `CheckRelPath` in it,
which is the type `deps-core` actually needs. `RelPath` moves in the
`deps-core` plan if that plan still wants it.

Spec 5.2's reasoning for a separate type, verified: `RelPath::parse` rejects
exactly three things (`Empty`, `Absolute`, and any `..` segment), and its own
tests at `path.rs:115-141` assert that `a/./b` and `dir/sub dir/fïle.txt` are
accepted. It does not reject backslashes, NUL or control bytes, a leading `~`,
or a Windows drive prefix. Composed with a variable root, those gaps matter.

**Files:**
- Create: `crates/dotfiles-path/Cargo.toml`
- Create: `crates/dotfiles-path/src/lib.rs`
- Create: `crates/dotfiles-path/src/rel.rs`
- Modify: `crates/Cargo.toml` (add the member)
- Test: inline `#[cfg(test)]` in `rel.rs`, run by `cargo test`

**Interfaces:**
- Consumes: `crates/Cargo.toml` with `[workspace] members` from Task 5.
- Produces:
  - `dotfiles_path::CheckRelPath` with
    `pub fn parse(raw: &str) -> Result<CheckRelPath, PathError>` and
    `pub fn as_str(&self) -> &str`.
  - `dotfiles_path::PathError` implementing `std::error::Error` and
    `fmt::Display`, with variants `Empty`, `Absolute`, `ParentTraversal`,
    `Backslash`, `ControlByte`, `HomePrefix`, `DrivePrefix`, `TooLong`.
  - The `deps-core` plan consumes both.

- [ ] **Step 1: Write the failing test**

Create `crates/dotfiles-path/src/rel.rs` containing only the test module, so
the test fails to compile for the right reason (a missing type, not a missing
file):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_plain_relative_path() {
        let parsed = CheckRelPath::parse("share/zsh-autosuggestions/x.zsh")
            .expect("a plain relative path parses");
        assert_eq!(parsed.as_str(), "share/zsh-autosuggestions/x.zsh");
    }

    #[test]
    fn rejects_empty() {
        assert!(matches!(CheckRelPath::parse(""), Err(PathError::Empty)));
    }

    #[test]
    fn rejects_absolute() {
        assert!(matches!(
            CheckRelPath::parse("/Applications/Alacritty.app"),
            Err(PathError::Absolute)
        ));
    }

    #[test]
    fn rejects_parent_traversal() {
        assert!(matches!(
            CheckRelPath::parse("a/../../etc/passwd"),
            Err(PathError::ParentTraversal)
        ));
    }

    // The four rejections RelPath::parse does NOT make. Each is a real
    // shape once the path is composed with a variable root: a backslash
    // traverses on a Windows-ish target, a control byte can rewrite a
    // terminal when the path is rendered in an error, `~` is expanded by a
    // shell but not by Rust so it would be taken literally, and a drive
    // prefix is absolute without a leading slash.
    #[test]
    fn rejects_backslash() {
        assert!(matches!(
            CheckRelPath::parse("a\\..\\..\\etc"),
            Err(PathError::Backslash)
        ));
    }

    #[test]
    fn rejects_control_bytes() {
        assert!(matches!(
            CheckRelPath::parse("a/\u{1b}[2Jb"),
            Err(PathError::ControlByte)
        ));
        assert!(matches!(
            CheckRelPath::parse("a/\u{0}b"),
            Err(PathError::ControlByte)
        ));
    }

    #[test]
    fn rejects_home_prefix() {
        assert!(matches!(
            CheckRelPath::parse("~/.nvm/nvm.sh"),
            Err(PathError::HomePrefix)
        ));
    }

    #[test]
    fn rejects_drive_prefix() {
        assert!(matches!(
            CheckRelPath::parse("C:\\Users"),
            Err(PathError::DrivePrefix)
        ));
    }

    #[test]
    fn rejects_over_length() {
        let long_path = "a/".repeat(2049);
        assert!(matches!(
            CheckRelPath::parse(&long_path),
            Err(PathError::TooLong { .. })
        ));
    }

    // A single-dot segment is ACCEPTED, matching RelPath's existing tests, so
    // the two types do not disagree about a shape that appears in neither
    // manifest.
    #[test]
    fn accepts_single_dot_segment() {
        assert!(CheckRelPath::parse("a/./b").is_ok());
    }

    // Non-ASCII is accepted: the conf files are UTF-8 and a path with an
    // accent is not a traversal.
    #[test]
    fn accepts_non_ascii() {
        assert!(CheckRelPath::parse("dir/fïle.txt").is_ok());
    }

    // The error must render without the raw bytes, because an error string
    // reaches a terminal and the input is untrusted.
    #[test]
    fn control_byte_error_does_not_echo_the_input() {
        let rendered = CheckRelPath::parse("a/\u{1b}[2Jb")
            .expect_err("a control byte is rejected")
            .to_string();
        assert!(
            !rendered.contains('\u{1b}'),
            "the error rendered the escape byte: {rendered:?}"
        );
    }
}
```

Create `crates/dotfiles-path/Cargo.toml`:

```toml
[package]
name = "dotfiles-path"
edition = "2024"
version = "0.1.0"
publish = false

# No dependencies, deliberately. This crate exists so deps-core and
# config-manifest can share validated newtypes without one depending on the
# other, and a dependency here would be inherited by both.
[dependencies]
```

Create `crates/dotfiles-path/src/lib.rs`:

```rust
//! Validated path and name primitives shared between the dotfiles crates.
//!
//! This crate has no dependencies and performs no IO. It exists so that
//! `deps-core` and `config-manifest` can share parse-don't-validate newtypes
//! without either depending on the other: `RelPath` was written first and
//! happens to live in `config-manifest`, but a validated relative path is not
//! a git-sync concept.

mod rel;

pub use rel::{CheckRelPath, PathError};
```

Add the member to `crates/Cargo.toml`:

```toml
[workspace]
resolver = "3"
members = ["config-manifest", "dotfiles-path"]
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd crates && cargo test -p dotfiles-path`
Expected: FAIL to compile, `cannot find type CheckRelPath in this scope`.

- [ ] **Step 3: Write the minimal implementation**

Prepend to `crates/dotfiles-path/src/rel.rs`, above the test module:

```rust
use std::fmt;

/// The maximum byte length of a parsed path.
///
/// PATH_MAX is 1024 on darwin and 4096 on Linux. 4096 is the smaller
/// surprise: a path this crate accepts must be usable on both, and a value
/// above the platform limit fails at the syscall instead of at the parse.
const MAX_LEN: usize = 4096;

/// Why a candidate path was refused.
///
/// One variant per rule so a caller can report the cause rather than
/// "invalid path", and so a new rule cannot be folded into an existing
/// variant without a diff that names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    Empty,
    Absolute,
    ParentTraversal,
    Backslash,
    ControlByte,
    HomePrefix,
    DrivePrefix,
    TooLong { len: usize, max: usize },
}

impl fmt::Display for PathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // No variant renders the offending input. These errors reach a
        // terminal, and the input is untrusted: a rejected control byte
        // written into the message would run as an escape sequence, which is
        // the reason ControlByte exists in the first place.
        match self {
            PathError::Empty => write!(formatter, "the path is empty"),
            PathError::Absolute => {
                write!(formatter, "the path is absolute, and a root is supplied separately")
            }
            PathError::ParentTraversal => {
                write!(formatter, "the path contains a `..` segment")
            }
            PathError::Backslash => write!(formatter, "the path contains a backslash"),
            PathError::ControlByte => {
                write!(formatter, "the path contains a control byte")
            }
            PathError::HomePrefix => {
                write!(formatter, "the path starts with `~`, which is not expanded here")
            }
            PathError::DrivePrefix => {
                write!(formatter, "the path starts with a drive letter")
            }
            PathError::TooLong { len, max } => {
                write!(formatter, "the path is {len} bytes, over the {max}-byte limit")
            }
        }
    }
}

impl std::error::Error for PathError {}

/// A relative path safe to join onto a `PathRoot`.
///
/// Stricter than `config_manifest::RelPath`, which rejects only empty,
/// absolute, and `..` segments. The extra rules exist because this type is
/// joined onto a root resolved at runtime, so the escape shapes a
/// worktree-relative path never sees are reachable here.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CheckRelPath(String);

impl CheckRelPath {
    pub fn parse(raw: &str) -> Result<Self, PathError> {
        if raw.is_empty() {
            return Err(PathError::Empty);
        }
        if raw.len() > MAX_LEN {
            return Err(PathError::TooLong { len: raw.len(), max: MAX_LEN });
        }
        // Control bytes first: every later rule reports a shape, and a
        // message about a shape is less useful than one about a byte that
        // would corrupt the message itself.
        if raw.chars().any(|character| character.is_control()) {
            return Err(PathError::ControlByte);
        }
        if raw.starts_with('/') {
            return Err(PathError::Absolute);
        }
        if raw.starts_with('~') {
            return Err(PathError::HomePrefix);
        }
        // A drive prefix is absolute without a leading slash, so the
        // Absolute check above does not catch it.
        let mut characters = raw.chars();
        if let (Some(first), Some(':')) = (characters.next(), characters.next())
            && first.is_ascii_alphabetic()
        {
            return Err(PathError::DrivePrefix);
        }
        if raw.contains('\\') {
            return Err(PathError::Backslash);
        }
        if raw.split('/').any(|segment| segment == "..") {
            return Err(PathError::ParentTraversal);
        }
        Ok(CheckRelPath(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CheckRelPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd crates && cargo test -p dotfiles-path`
Expected: PASS, 12 tests.

- [ ] **Step 5: Verify the whole workspace still builds and the stamp moved**

Run: `cd crates && cargo test`
Expected: PASS for both crates.

Run: `~/.scripts/config/config-stamp`
Expected: a stamp per member, and `config-manifest`'s value is **unchanged**
from before this task, because nothing in that crate's tree changed. A changed
`config-manifest` stamp means the new crate landed inside its tree.

- [ ] **Step 6: Run the full suite**

Run: `~/tests/run-all.sh`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
config add crates/dotfiles-path crates/Cargo.toml crates/Cargo.lock
config commit -m "Add dotfiles-path, a shared crate for validated primitives

deps-core needs a validated relative path, and RelPath lives in
config-manifest. deps-core depending on config-manifest would make the
dependency-manifest domain depend on the git-sync domain and inherit a
595-line git module it never calls; the reverse edge is inverted. A crate
with no dependencies and no IO is the third edge.

CheckRelPath is stricter than RelPath rather than a copy of it. RelPath
rejects empty, absolute, and .. segments, and its own tests accept a/./b.
Composed with a runtime-resolved root, the shapes it does not reject become
reachable: backslashes, control bytes, a leading ~, and a drive prefix.

No error variant renders the offending input, because these errors reach a
terminal and a rejected escape byte would run."
```

---

## Task 6a: One computation owner, one policy owner

**Why this exists.** Spec 8.1. After Task 6, `config-stamp` owns the stamp
computation, but `tests/pre-push:131` still computes a pushed stamp itself:

```sh
pushed_tree=$(git rev-parse --verify --quiet "$ref:crates/config-manifest" 2>/dev/null || true)
```

Two problems, both verified. The crate path is **hardcoded** where
`config-stamp` takes it as a parameter, so the two spellings agree only
through an invariant documented in prose in one file. And with the workspace
now holding three crates, the stamp is a **set**, so `pre-push` would need the
crate list too, making it a third place the same fact lives.

`pre-push` never builds, so the circularity argument that keeps the
computation in shell does not apply to the comparison. That decision is
`plan`-shaped by spec 8.3's own criterion: it consumes small values and
produces a refusal or a next step.

**Files:**
- Modify: `.scripts/config/config-stamp` (add `--ref`)
- Create: `crates/config-manifest/src/stamp.rs`
- Modify: `crates/config-manifest/src/lib.rs` (export the module)
- Modify: `crates/config-manifest/src/main.rs` (add the `verify-stamps` subcommand)
- Modify: `tests/pre-push:128-144`
- Test: inline `#[cfg(test)]` in `stamp.rs`; `tests/config-manifest-lifecycle.test.sh`

**Interfaces:**
- Consumes: `config-stamp` emitting `<crate> <tree-id>` per line from Task 6.
- Produces:
  - `config_manifest::stamp::StampVerdict` with variants
    `Fresh`, `Stale { crate_name: String, built: String, pushed: String }`,
    `NotBuilt { crate_name: String }`, `Unknown { crate_name: String }`.
  - `config_manifest::stamp::verify(built: &[(String, String)], pushed: &[(String, String)]) -> Vec<StampVerdict>`, pure.
  - `config_manifest::stamp::render(verdicts: &[StampVerdict]) -> check::Rendered`, pure.
  - `config-stamp --ref <ref>` printing committed stamps for that ref.
  - `config-manifest verify-stamps --ref <ref>` for `pre-push`.

- [ ] **Step 1: Write the failing test**

Create `crates/config-manifest/src/stamp.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn pair(name: &str, tree: &str) -> (String, String) {
        (name.to_string(), tree.to_string())
    }

    #[test]
    fn matching_stamps_are_fresh() {
        let verdicts = verify(
            &[pair("config-manifest", "aaa"), pair("dotfiles-path", "bbb")],
            &[pair("config-manifest", "aaa"), pair("dotfiles-path", "bbb")],
        );
        assert_eq!(verdicts, vec![StampVerdict::Fresh, StampVerdict::Fresh]);
    }

    #[test]
    fn a_differing_stamp_is_stale_and_names_the_crate() {
        let verdicts = verify(
            &[pair("config-manifest", "aaa")],
            &[pair("config-manifest", "zzz")],
        );
        assert_eq!(
            verdicts,
            vec![StampVerdict::Stale {
                crate_name: "config-manifest".to_string(),
                built: "aaa".to_string(),
                pushed: "zzz".to_string(),
            }]
        );
    }

    // The whole reason this is a set rather than one value: editing one crate
    // must not mark another stale, and a verdict has to say which.
    #[test]
    fn one_stale_crate_does_not_taint_its_neighbour() {
        let verdicts = verify(
            &[pair("config-manifest", "aaa"), pair("dotfiles-path", "bbb")],
            &[pair("config-manifest", "aaa"), pair("dotfiles-path", "zzz")],
        );
        assert_eq!(verdicts[0], StampVerdict::Fresh);
        assert!(matches!(
            &verdicts[1],
            StampVerdict::Stale { crate_name, .. } if crate_name == "dotfiles-path"
        ));
    }

    // A binary that reported no stamp is NotBuilt, not Stale. pre-push tells
    // the reader to run `config build` in that case, and "stale" would send
    // them looking for a source change that does not exist.
    #[test]
    fn a_missing_built_stamp_is_not_built() {
        let verdicts = verify(&[], &[pair("config-manifest", "aaa")]);
        assert_eq!(
            verdicts,
            vec![StampVerdict::NotBuilt { crate_name: "config-manifest".to_string() }]
        );
    }

    // A crate present in the build but absent from the pushed ref is Unknown
    // rather than Fresh. Fail closed: this is what a new crate pushed without
    // its sources looks like, and treating it as Fresh would pass the gate.
    #[test]
    fn a_crate_absent_from_the_ref_is_unknown() {
        let verdicts = verify(&[pair("brand-new", "aaa")], &[]);
        assert_eq!(
            verdicts,
            vec![StampVerdict::Unknown { crate_name: "brand-new".to_string() }]
        );
    }

    // Order must not decide the verdict: config-stamp iterates the member
    // list and git iterates the tree, and nothing guarantees they agree.
    #[test]
    fn input_order_does_not_change_the_verdict() {
        let verdicts = verify(
            &[pair("dotfiles-path", "bbb"), pair("config-manifest", "aaa")],
            &[pair("config-manifest", "aaa"), pair("dotfiles-path", "bbb")],
        );
        assert!(verdicts.iter().all(|verdict| *verdict == StampVerdict::Fresh));
    }

    #[test]
    fn an_empty_build_list_is_not_silently_fresh() {
        // Both empty is the shape a broken member-list sed produces. It must
        // not render as a pass.
        let rendered = render(&verify(&[], &[]));
        assert_ne!(rendered.exit_code, 0, "an empty comparison passed the gate");
    }

    #[test]
    fn fresh_renders_exit_zero() {
        let rendered = render(&[StampVerdict::Fresh]);
        assert_eq!(rendered.exit_code, 0);
    }

    #[test]
    fn stale_renders_nonzero_and_names_the_rebuild_command() {
        let rendered = render(&[StampVerdict::Stale {
            crate_name: "config-manifest".to_string(),
            built: "aaa".to_string(),
            pushed: "zzz".to_string(),
        }]);
        assert_ne!(rendered.exit_code, 0);
        assert!(
            rendered.stderr.contains("config build"),
            "the reader was not told how to fix it: {:?}",
            rendered.stderr
        );
        assert!(rendered.stderr.contains("config-manifest"));
    }

    #[test]
    fn not_built_renders_nonzero_and_names_the_rebuild_command() {
        let rendered = render(&[StampVerdict::NotBuilt {
            crate_name: "config-manifest".to_string(),
        }]);
        assert_ne!(rendered.exit_code, 0);
        assert!(rendered.stderr.contains("config build"));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd crates && cargo test -p config-manifest stamp::`
Expected: FAIL to compile, `cannot find function verify in this scope`.

- [ ] **Step 3: Write the minimal implementation**

Prepend to `crates/config-manifest/src/stamp.rs`:

```rust
//! The stamp comparison policy, as a pure function.
//!
//! The stamp *computation* lives in `.scripts/config/config-stamp` and must
//! stay there: `config-build` calls it to decide what to compile, so a Rust
//! owner would be the binary the stamp guards. The comparison is a different
//! value with a different producer. `pre-push` never builds; it consumes two
//! lists and produces a refusal or a pass, which is plan-shaped.

use std::collections::BTreeMap;

use crate::check::Rendered;

/// The freshness of one crate's installed binary against a pushed ref.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StampVerdict {
    Fresh,
    Stale { crate_name: String, built: String, pushed: String },
    /// The installed binary reported no stamp for this crate.
    NotBuilt { crate_name: String },
    /// The build knows this crate and the pushed ref does not.
    Unknown { crate_name: String },
}

/// Compares built stamps against the stamps committed on a ref.
///
/// Keyed by crate name rather than by position, because `config-stamp`
/// iterates the workspace member list while git iterates a tree, and nothing
/// makes those orders agree.
pub fn verify(
    built: &[(String, String)],
    pushed: &[(String, String)],
) -> Vec<StampVerdict> {
    let built_by_name: BTreeMap<&str, &str> = built
        .iter()
        .map(|(name, tree)| (name.as_str(), tree.as_str()))
        .collect();
    let pushed_by_name: BTreeMap<&str, &str> = pushed
        .iter()
        .map(|(name, tree)| (name.as_str(), tree.as_str()))
        .collect();

    let mut names: Vec<&str> = built_by_name.keys().copied().collect();
    names.extend(pushed_by_name.keys().copied());
    names.sort_unstable();
    names.dedup();

    names
        .into_iter()
        .map(|name| match (built_by_name.get(name), pushed_by_name.get(name)) {
            (Some(built_tree), Some(pushed_tree)) if built_tree == pushed_tree => {
                StampVerdict::Fresh
            }
            (Some(built_tree), Some(pushed_tree)) => StampVerdict::Stale {
                crate_name: name.to_string(),
                built: (*built_tree).to_string(),
                pushed: (*pushed_tree).to_string(),
            },
            (None, Some(_)) => StampVerdict::NotBuilt { crate_name: name.to_string() },
            // Fail closed. A crate the build knows and the ref does not is
            // what pushing a new crate without its sources looks like.
            (Some(_), None) => StampVerdict::Unknown { crate_name: name.to_string() },
            (None, None) => unreachable!("a name came from one of the two maps"),
        })
        .collect()
}

/// Renders verdicts to two streams and an exit code.
pub fn render(verdicts: &[StampVerdict]) -> Rendered {
    // An empty comparison is a broken caller, not a pass. This is the shape a
    // member-list parse failure produces, and the whole point of the gate is
    // that it refuses when it cannot answer.
    if verdicts.is_empty() {
        return Rendered {
            stdout: String::new(),
            stderr: "pre-push: no crates were compared, so the stamp gate cannot answer\n"
                .to_string(),
            exit_code: 2,
        };
    }

    let mut stdout = String::new();
    let mut stderr = String::new();

    for verdict in verdicts {
        match verdict {
            StampVerdict::Fresh => {}
            StampVerdict::Stale { crate_name, built, pushed } => stderr.push_str(&format!(
                "pre-push: {crate_name} is stale (built {built}, pushed {pushed}); run `config build`\n"
            )),
            StampVerdict::NotBuilt { crate_name } => stderr.push_str(&format!(
                "pre-push: {crate_name} reported no stamp; run `config build`\n"
            )),
            StampVerdict::Unknown { crate_name } => stderr.push_str(&format!(
                "pre-push: {crate_name} is built here but absent from the pushed ref\n"
            )),
        }
    }

    if stderr.is_empty() {
        stdout.push_str(&format!(
            "pre-push: every stamp matches the pushed ref ({} crate(s))\n",
            verdicts.len()
        ));
        return Rendered { stdout, stderr, exit_code: 0 };
    }
    Rendered { stdout, stderr, exit_code: 1 }
}
```

Export it in `crates/config-manifest/src/lib.rs`:

```rust
pub mod stamp;
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd crates && cargo test -p config-manifest stamp::`
Expected: PASS, 10 tests.

- [ ] **Step 5: Add `--ref` to `config-stamp`**

The shell keeps the computation. `--ref` reads the committed tree for a ref
instead of the worktree, so `pre-push` stops spelling `rev-parse`.

Add to `.scripts/config/config-stamp`, after the existing argument parsing:

```sh
# --ref reads the committed tree for a ref rather than the worktree.
#
# pre-push used to spell `git rev-parse "$ref:crates/config-manifest"` itself,
# with the crate path hardcoded where this script takes it as a parameter. Two
# spellings of one rule agree only by an invariant nothing checks, and with a
# three-crate workspace pre-push would have needed the member list too. One
# owner, parameterised.
if [ -n "$stamp_ref" ]; then
    for member in $members; do
        member_tree=$(git_cmd rev-parse --verify --quiet \
            "$stamp_ref:crates/$member" 2>/dev/null || true)
        # A member absent from the ref prints nothing rather than an empty
        # field. The Rust verdict treats a missing entry as Unknown and fails
        # closed; an empty field would compare equal to another empty field.
        [ -n "$member_tree" ] || continue
        printf '%s %s\n' "$member" "$member_tree"
    done
    exit 0
fi
```

- [ ] **Step 6: Rewrite the `pre-push` stamp block**

Replace `tests/pre-push:128-144` with:

```sh
    # The stamp gate. pre-push compares and refuses; it does not compute.
    #
    # `config-stamp --ref` owns the committed side and the installed binary
    # owns the built side, so this hook holds no copy of the stamp rule and no
    # copy of the crate list. Both were duplicated here before, and the crate
    # path was hardcoded where config-stamp took it as a parameter.
    for ref in $push_refs; do
        pushed_stamps=$("$DOTFILES_ROOT/.scripts/config/config-stamp" --ref "$ref" 2>/dev/null || true)
        # No stamps for this ref means it carries no crates, so there is
        # nothing to gate.
        [ -n "$pushed_stamps" ] || continue

        if [ ! -x "$config_manifest_bin" ]; then
            printf 'pre-push: config-manifest is not built; run %s\n' \
                "$DOTFILES_ROOT/.scripts/config/config-build" >&2
            exit 1
        fi

        if ! printf '%s\n' "$pushed_stamps" \
            | "$config_manifest_bin" verify-stamps --ref "$ref"; then
            exit 1
        fi
    done
```

- [ ] **Step 7: Wire the subcommand**

Add to `crates/config-manifest/src/main.rs`, in the subcommand match:

```rust
        Command::VerifyStamps { reference } => {
            // Built stamps come from this binary's own embedded stamps, and
            // pushed stamps arrive on stdin from config-stamp --ref. The
            // binary is the sole producer of the built side, which is what
            // earns this module under the plan's own rule.
            let pushed = stamp_pairs_from_stdin()?;
            let built = built_stamps();
            let rendered = stamp::render(&stamp::verify(&built, &pushed));
            write_rendered(&rendered)?;
            let _ = reference;
            Ok(ExitCode::from(rendered.exit_code))
        }
```

- [ ] **Step 8: Run the lifecycle test**

Run: `bash tests/config-manifest-lifecycle.test.sh`
Expected: PASS.

Then verify the gate refuses a real stale binary:

```bash
cd crates && touch config-manifest/src/plan.rs && cd -
printf '%s\n' "$(~/.scripts/config/config-stamp --ref HEAD)" \
  | ~/.local/bin/config-manifest verify-stamps --ref HEAD; echo "exit: $?"
```

Expected: the worktree stamp now differs from `HEAD`, so this prints a stale
line naming `config-manifest` and exits 1. Rebuild with
`~/.scripts/config/config-build` and confirm it returns to exit 0.

- [ ] **Step 9: Run the full suite**

Run: `~/tests/run-all.sh`
Expected: PASS.

- [ ] **Step 10: Commit**

```bash
config add .scripts/config/config-stamp crates/config-manifest/src/stamp.rs \
  crates/config-manifest/src/lib.rs crates/config-manifest/src/main.rs tests/pre-push
config commit -m "Give the stamp one computation owner and one policy owner

config-stamp already owned the computation. pre-push owned a second copy of
the comparison policy: it spelled git rev-parse itself with the crate path
HARDCODED, where config-stamp takes it as a parameter. Two spellings of one
rule agree only through an invariant documented in prose in one file and
depended on silently in the other, and a three-crate workspace would have
made pre-push carry the member list too.

config-stamp gains --ref so it owns both sides of the computation. A pure
Rust verify() owns the verdict, which is legitimate under the rule that keeps
the computation in shell: pre-push never builds, so there is no circularity.

verify() keys by crate name rather than position, because config-stamp
iterates the member list while git iterates a tree. A crate built here but
absent from the pushed ref is Unknown rather than Fresh, and an empty
comparison exits 2 rather than passing: both fail closed, since a gate that
cannot answer must refuse."
```

---

## Task 9a: The bootstrap containers gain a Rust toolchain

**Why this exists.** Spec 7.5a and 7.4 step 3. The bootstrap gate added in
`35d8efd7` is green today: 21 checks on bare Debian, 20 on bare Arch, both
through the real `curl ... | sh` one-liner. Those images carry no Rust
toolchain, and `config init` currently builds `config-manifest` inside them
successfully **because `rustup` is a dependency entry that installs during
the run**.

Verified from a live run: `config-build: installed
/root/.local/bin/config-manifest (stamp e7b9aa22...)`. So the toolchain
arrives already. What breaks after spec step 3 is different: `check-deps.sh`
itself becomes the Rust binary, so it must exist **before** the dependency
install that places rustup. That is a genuine ordering problem, and it is
cheaper to solve while the gate is green.

**The fix is a build stage, not a preinstalled toolchain.** Preinstalling
rustup in the image would delete the coverage that `rustup` installs
correctly, which is one of the 22 dependencies under test.

**Files:**
- Modify: `.scripts/deps/docker/Dockerfile.bootstrap-curl`
- Modify: `.scripts/deps/docker/Dockerfile.bootstrap-curl-arch`
- Modify: `.scripts/deps/docker/bootstrap-curl-entrypoint.sh`
- Test: `tests/bootstrap-harness.test.sh`

**Interfaces:**
- Consumes: the entrypoint and both Dockerfiles from `35d8efd7`.
- Produces: a `BOOTSTRAP_PREBUILT_BIN` environment seam the entrypoint honors,
  so the `deps-core` plan can point the gate at a prebuilt binary without
  editing the harness again.

- [ ] **Step 1: Write the failing test**

Add to `tests/bootstrap-harness.test.sh`, before `finish`:

```bash
# --- the toolchain seam ------------------------------------------------------
#
# After spec step 3, check-deps.sh IS the Rust binary, so it must exist before
# the dependency install that places rustup. The images cannot compile it
# themselves: the build context is only .scripts/deps and there is no
# toolchain. A seam now means the deps-core plan points the gate at a prebuilt
# binary rather than editing this harness under time pressure.
assert_succeeds 'the curl-pipe entrypoint honors a prebuilt binary seam' \
    grep -q 'BOOTSTRAP_PREBUILT_BIN' "$CURL_ENTRYPOINT"

# The seam must be optional, or today's green run breaks: the whole point is
# that the shell path still works until step 3 lands.
assert_succeeds 'the prebuilt seam is optional' \
    grep -qE 'BOOTSTRAP_PREBUILT_BIN:-' "$CURL_ENTRYPOINT"

# And it must not silently accept a path that is not there. A gate that
# ignores a misspelled seam tests the wrong binary and passes.
assert_succeeds 'a set-but-missing prebuilt binary is refused' \
    grep -q 'prebuilt binary was named but does not exist' "$CURL_ENTRYPOINT"
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `bash tests/bootstrap-harness.test.sh`
Expected: FAIL, three assertions, `BOOTSTRAP_PREBUILT_BIN` not found.

- [ ] **Step 3: Add the seam to the entrypoint**

Insert into `.scripts/deps/docker/bootstrap-curl-entrypoint.sh`, after the
seed copy and before the HTTP server starts:

```sh
# An optional prebuilt binary, for after the deps-core port.
#
# Today check-deps.sh is shell and runs on a bare image with no toolchain.
# After spec step 3 it is a Rust binary that must exist BEFORE the dependency
# install that places rustup, and this image cannot compile it: the build
# context is only .scripts/deps. So the caller may hand one in.
#
# Optional on purpose. The shell path is what ships until step 3 lands, and a
# required seam would break the green run this gate currently provides.
prebuilt=${BOOTSTRAP_PREBUILT_BIN:-}
if [ -n "$prebuilt" ]; then
    if [ ! -x "$prebuilt" ]; then
        printf 'FAIL: a prebuilt binary was named but does not exist: %s\n' \
            "$prebuilt" >&2
        exit 1
    fi
    mkdir -p "$HOME/.local/bin"
    cp "$prebuilt" "$HOME/.local/bin/"
    printf 'harness: seeded a prebuilt %s\n' "${prebuilt##*/}"
fi
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `bash tests/bootstrap-harness.test.sh`
Expected: PASS.

- [ ] **Step 5: Verify both legs still pass end to end**

The seam is unset, so this must reproduce today's result exactly.

```bash
rm -rf /tmp/seed-curl && mkdir -p /tmp/seed-curl
git clone --bare --quiet "$HOME/.cfg" /tmp/seed-curl/repo.git
cp ~/setup.sh /tmp/seed-curl/setup.sh
cp ~/.scripts/deps/docker/bootstrap-curl-entrypoint.sh /tmp/seed-curl/
mkdir -p /tmp/empty-ctx

docker build -q -f ~/.scripts/deps/docker/Dockerfile.bootstrap-curl \
  -t dotfiles-bootstrap-curl /tmp/empty-ctx
docker run --rm -v /tmp/seed-curl:/seed:ro dotfiles-bootstrap-curl
```

Expected: `=== curl-pipe bootstrap: 0 check(s) failed ===`, 21 ok lines.

Then the Arch leg:

```bash
docker build --platform linux/amd64 -q \
  -f ~/.scripts/deps/docker/Dockerfile.bootstrap-curl-arch \
  -t dotfiles-bootstrap-curl-arch /tmp/empty-ctx
docker run --rm --platform linux/amd64 \
  -v /tmp/seed-curl:/seed:ro dotfiles-bootstrap-curl-arch
```

Expected: `=== curl-pipe bootstrap: 0 check(s) failed ===`.

- [ ] **Step 6: Verify the seam refuses a bad path**

```bash
docker run --rm -v /tmp/seed-curl:/seed:ro \
  -e BOOTSTRAP_PREBUILT_BIN=/nope/config-manifest \
  dotfiles-bootstrap-curl; echo "exit: $?"
```

Expected: `FAIL: a prebuilt binary was named but does not exist` and exit 1.
A gate that ignored the misspelling would test the wrong binary and pass.

- [ ] **Step 7: Run the full suite and clean up**

Run: `~/tests/run-all.sh`
Expected: PASS.

```bash
rm -rf /tmp/seed-curl /tmp/empty-ctx
```

- [ ] **Step 8: Commit**

```bash
config add .scripts/deps/docker/bootstrap-curl-entrypoint.sh tests/bootstrap-harness.test.sh
config commit -m "Give the bootstrap gate a prebuilt-binary seam

After spec step 3, check-deps.sh IS the Rust binary, so it must exist before
the dependency install that places rustup. The bootstrap images cannot
compile it: the build context is only .scripts/deps and there is no
toolchain.

The seam is optional, so today's green run is unchanged and the shell path
keeps shipping until step 3 lands. A set-but-missing path is refused rather
than ignored: a gate that silently tests the wrong binary is worse than one
that is absent.

Not a preinstalled toolchain. rustup is one of the 22 dependencies under
test, and preinstalling it would delete that coverage."
```

---

## Self-Review

**1. Spec coverage.** Walked the in-scope sections.

| Spec section | Task |
|---|---|
| 7.4 step 0 (four blockers) | Tasks 1 to 4, prior plan |
| 7.4 step 1 (`--describe`, `usage.sh`) | **Gap, see below** |
| 7.4 step 2 (workspace, pin, stamps) | Tasks 5, 6, prior plan |
| 7.1 (`dotfiles-path` crate) | Task 5a |
| 8.1 (stamp policy owner, binary resolution) | Task 6a; binary resolution partially |
| 8.2 (Task 9 dropped, Task 8 stays) | Delta 1 |
| 7.5a (bootstrap gate survives the port) | Task 9a |

**Gap found: spec 7.4 step 1 has no task.** `config-help` reads descriptions
with `sed -n 's/^# help: //p'` over source text, so the first ported
subcommand lists as `(undocumented)`, and `usage.sh:32-35` extracts the
`# usage:` block the same way and breaks at the same moment. The prior plan
covers neither.

I am **not** adding it here, and the reason is a real dependency rather than
scope trimming: nothing in this plan ports a subcommand to Rust, so nothing
in this plan can make a subcommand list as `(undocumented)`. Adding
`--describe` before its first consumer exists would ship an interface with no
caller, which is the failure the prior plan's own "What is NOT in this plan"
section records twice. It belongs at the head of the `deps-core` plan, whose
first task creates the first Rust subcommand. Recorded in that plan's
prerequisites, not dropped.

Spec 8.1's binary-resolution finding (four owners today) is likewise deferred:
Task 6a removes `pre-push`'s duplicate *stamp* rule, but making the dispatcher
the sole owner of *binary resolution* only matters once `config deps` resolves
through it.

**2. Placeholder scan.** No TBD, no "add error handling", no "similar to Task
N". Every code step carries the actual code. Task 5a and 6a spell out all
types they introduce; Task 9a's shell block is complete.

**3. Type consistency.** Checked the names crossing task boundaries:

- `CheckRelPath::parse` and `PathError` in Task 5a are used by name in the
  Interfaces block and nowhere else in this plan, so nothing can disagree.
  The `deps-core` plan consumes them.
- `StampVerdict`'s four variants are spelled identically in the test, the
  implementation, and `render`'s match. `verify` and `render` signatures match
  their Interfaces block.
- `Rendered { stdout, stderr, exit_code }` matches `check.rs:31-36` as
  verified, so `stamp::render` returns the type the crate already uses.
- `config-stamp --ref` output format (`<crate> <tree-id>` per line) is
  produced in Task 6a step 5 and consumed in step 6, same format.
- `BOOTSTRAP_PREBUILT_BIN` is spelled identically in the test, the
  implementation, and the verification command.

One inconsistency found and fixed while reviewing: Task 6a's `main.rs`
snippet originally called `stamp::verify` with the arguments reversed relative
to the signature (`pushed, built` against `built, pushed`), which would have
inverted every Stale message. The step now passes `&built, &pushed`.

---

## Notes for the executor

- **Tasks 1 to 4 first, and they are independent.** Three are leak-guard
  fail-opens on a public repository. They need no architectural decision and
  they are the only items here that let content reach a public remote.
- **Read the prior plan for Tasks 1 to 8.** This document deliberately does
  not restate 2,300 lines of correct TDD steps. Its job is the four deltas and
  the three new tasks.
- **Task 5a depends on Task 5** for `crates/Cargo.toml`. Task 6a depends on
  Task 6 for per-crate `config-stamp` output. Task 9a depends on neither and
  can land any time.
- **The bootstrap gate is a release gate, not a unit test.** Re-run both legs
  after Task 9a, and again after the `deps-core` port. Passing today validates
  the shell implementation, which is the pre-rewrite baseline.
- **`tmux-conf-split.test.sh` currently fails** on a wheel-scroll assertion
  from commit `e5a78efa`, unrelated to this plan. Do not treat it as caused by
  a task here, and do not fix it inside one.
