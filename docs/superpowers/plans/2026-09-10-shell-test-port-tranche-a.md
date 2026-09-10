# Shell Test Port, Tranche A Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the shell test suites that parse structured formats with `sed`, `grep -oE` and `awk` into Rust tests that use real parsers, starting with the suite whose defect is reproduced.

**Architecture:** A new `dotfiles-test-support` library crate holds the runtime-skip mechanism from spec section 4b.1 and nothing else. Converted suites become integration tests under `crates/config-cli/tests/`, reading tracked files with `serde_yaml`, `toml` and a Markdown parser instead of regex. Each conversion deletes its `.test.sh` in the same commit that adds its Rust replacement, so the two harnesses never both own one assertion.

**Tech Stack:** Rust 2024, `serde` and `serde_yaml` for workflows, `toml` (already in the lock at 0.9.12) for manifests, `pulldown-cmark` for Markdown, `tempfile` (already at 3.27.0), `assert_cmd` where a suite drives a binary.

**Spec:** `docs/superpowers/specs/2026-09-07-shell-test-port-design.md` (Tranche A in section 3, the skip mechanism in 4b.1, the tmux fixture in 4b.2). Tranches B and C are out of scope for this plan; B follows the widget port and C follows each suite's subject.

## Global Constraints

Copied from the spec and from this repo's standing rules. Every task's requirements implicitly include this section.

- **All Rust invocations run from inside `crates/`, never with `--manifest-path`.** Spec 4a.1 states this as a hard invariant: "That exact mistake has failed CI once already."
- **`cargo test --locked --quiet` stays the gate command.** `tests/rust-checks.sh:139`. Do not add `--nocapture` to make skips visible; spec 4b.1 rejected that.
- **`#[ignore]` must not be used for a runtime skip.** Spec section 4: "it hides the count."
- **Positive controls are required.** Spec section 4: every assertion expecting an empty result must first assert that its pipeline produced something. In Rust an empty `Vec` and a failed command are different types, and the test must distinguish them.
- **A skip must be loud.** `.claude/rules/dotfiles-tests.md`: "a gate that says nothing when it skips is indistinguishable from a gate that is not installed."
- **Two suites are excluded from Tranche A.** `container.test.sh` and `scripts-dir-name.test.sh` convert after step 4 of spec section 5, because other steps use them as gates.
- **`config.test.sh` is not in this plan.** Spec 2a: it is in no tranche, and placing it needs the `config-build` question answered first.
- **Lint policy is already enforced.** `crates/Cargo.toml:24` and `:33`; all six members opt in. New crates must add `[lints] workspace = true`.
- **The repo is public.** No employer or product names in tracked files.
- **Never `--no-verify`.** Never disable a test instead of fixing it. Never commit code that does not compile.
- **Commit messages** end with the attribution lines this session uses.
- **`crates/Cargo.lock` is part of every commit that touches a manifest.**
  Adding a member or a dependency changes the lock, and `cargo test --locked`
  refuses to update it, so a commit without the lock fails its own gate. Run
  `cargo update --offline -w` after a manifest edit, and add the lock. Found
  the hard way in Task 1, whose own `config add` list omitted it.
- **Every crate root needs
  `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]`.**
  `tests/rust-gate.test.sh` asserts it per crate, and that suite is not
  mentioned anywhere else in this plan. Without the attribute the suite goes
  to `24 passed, 1 failed` and the push is blocked.
- **Every integration-test file opens with a `//!` module doc.** All 13
  existing ones do, and `missing_docs = "warn"` plus clippy's `-D warnings`
  makes an omission a gate failure rather than a style note.
- **`tests/rust-checks.sh` archives from a git ref, not the working tree.**
  So a gate run before the commit exercises the OLD code. Verify after
  committing, not before, or the run proves nothing about what you wrote.
- **A test must never call `skip()` to prove skipping works.** `skip()` reads
  the ambient `DOTFILES_SKIP_LOG`, which the gate sets, so such a test
  appends a phantom skip and overstates missing coverage. Assert through
  `skip_log_path()` and an explicit path instead. This was a real defect in
  this plan's Task 1, caught during execution.

---

### Task 1: The runtime-skip mechanism -- DONE (`6a162f05`, 2026-09-10)

> Landed with all gates green. The sabotage control worked. `nvim` is on
> PATH here, so the correct outcome was no skip line, and there was none.
>
> Four defects in this task as written, all now fixed in the Global
> Constraints above: the missing lock in the `config add` list, the missing
> `deny(clippy::unwrap_used)` attribute that `rust-gate.test.sh` enforces,
> the missing `//!` module doc, and a real bug in the third test, which
> called `skip()` under a gate that sets the log and so appended a phantom
> skip line.
>
> **Follow-up this task did not cover:** `nvim_runtime.rs:175` has a THIRD
> skip, the Homebrew-Cellar case, still on bare `eprintln!`. On this machine
> that is the branch that fires, so the one real skip here is still invisible
> to the gate. Converted separately.


Spec 4b.1. Everything else in Tranche A depends on this, because a converted suite that cannot record a skip silently drops platform-gated coverage.

**Files:**
- Create: `crates/dotfiles-test-support/Cargo.toml`
- Create: `crates/dotfiles-test-support/src/lib.rs`
- Create: `crates/dotfiles-test-support/tests/skip_log.rs`
- Modify: `crates/Cargo.toml` (add the member)
- Modify: `tests/rust-checks.sh` (truncate before, report after)
- Modify: `tests/pre-push` (nothing; it calls rust-checks.sh already, confirm only)

**Interfaces:**
- Consumes: nothing.
- Produces: `dotfiles_test_support::skip(reason: &str)`, which appends one JSON line to the path in `DOTFILES_SKIP_LOG` and is a no-op when that variable is unset. `dotfiles_test_support::skip_log_path() -> Option<PathBuf>`. Later tasks call `skip` and return early.

- [ ] **Step 1: Write the failing test**

`crates/dotfiles-test-support/tests/skip_log.rs`:

```rust
use std::fs;

/// `skip` appends one line naming the reason, so `rust-checks.sh` can count
/// what did not run. The gate reads this file; a skip that writes nothing is
/// indistinguishable from a pass.
#[test]
fn skip_appends_a_line_naming_the_reason() {
    let directory = tempfile::Builder::new()
        .prefix("skip-log-")
        .tempdir_in("/tmp")
        .expect("a temp dir");
    let log = directory.path().join("skips.jsonl");

    dotfiles_test_support::skip_to(&log, "shellcheck is not installed here");

    let written = fs::read_to_string(&log).expect("the log exists");
    assert_eq!(written.lines().count(), 1, "one skip is one line");
    assert!(
        written.contains("shellcheck is not installed here"),
        "the reason survives into the log: {written}"
    );
}

/// Two skips are two lines. A mechanism that overwrites reports one skip
/// where there were several, which understates missing coverage.
#[test]
fn skips_accumulate_rather_than_overwrite() {
    let directory = tempfile::Builder::new()
        .prefix("skip-log-")
        .tempdir_in("/tmp")
        .expect("a temp dir");
    let log = directory.path().join("skips.jsonl");

    dotfiles_test_support::skip_to(&log, "first reason");
    dotfiles_test_support::skip_to(&log, "second reason");

    let written = fs::read_to_string(&log).expect("the log exists");
    assert_eq!(written.lines().count(), 2, "two skips are two lines");
}

/// With no log path configured, `skip` must not panic and must not create a
/// file. A developer running `cargo test` by hand has no gate to report to.
#[test]
fn an_unconfigured_log_is_a_silent_no_op() {
    dotfiles_test_support::skip("no log configured, so this goes nowhere");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run from `~/crates`: `cargo test -p dotfiles-test-support --locked`

Expected: FAIL. The crate does not exist yet, so cargo reports `error: package ID specification 'dotfiles-test-support' did not match any packages`.

- [ ] **Step 3: Create the crate and the member entry**

`crates/dotfiles-test-support/Cargo.toml`:

```toml
[package]
name = "dotfiles-test-support"
version = "0.1.0"
edition = "2024"

[dependencies]

[dev-dependencies]
tempfile = "3.27"

[lints]
workspace = true
```

In `crates/Cargo.toml`, add the member to the existing list so it reads:

```toml
members = ["config-cli", "config-manifest", "deps-core", "dotfiles-path", "dotfiles-test-support", "tmux-core", "tmux-tools"]
```

- [ ] **Step 4: Write the minimal implementation**

`crates/dotfiles-test-support/src/lib.rs`:

```rust
//! Test-support for the converted shell suites.
//!
//! Holds one thing: a runtime skip that the pre-push gate can count. Rust's
//! harness has no representation for "this check cannot run on this
//! machine": `#[ignore]` is a compile-time decision and hides the count, and
//! `eprintln!` is swallowed by `cargo test --quiet`, which is the command
//! `tests/rust-checks.sh` runs. Measured 2026-09-10: that command reported
//! zero skip lines from `nvim_runtime.rs`, which already skips this way.
//!
//! So a skip is recorded to a file whose path the gate sets, and the gate
//! reports the count. See the spec's section 4b.1.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The environment variable naming the skip log. Set by `tests/rust-checks.sh`.
pub const SKIP_LOG_VARIABLE: &str = "DOTFILES_SKIP_LOG";

/// Where skips are recorded, or `None` when no gate is listening.
#[must_use]
pub fn skip_log_path() -> Option<PathBuf> {
    std::env::var_os(SKIP_LOG_VARIABLE).map(PathBuf::from)
}

/// Records that a check could not run here, and why.
///
/// Call this and return early. It is a no-op when no log is configured, so a
/// developer running `cargo test` by hand needs no setup.
pub fn skip(reason: &str) {
    if let Some(log) = skip_log_path() {
        skip_to(&log, reason);
    }
}

/// Appends one skip to an explicit path. Public so the mechanism is testable
/// without mutating the environment, which parallel tests share.
pub fn skip_to(log: &Path, reason: &str) {
    let escaped = reason.replace('\\', "\\\\").replace('"', "\\\"");
    let line = format!("{{\"reason\":\"{escaped}\"}}\n");
    if let Ok(mut handle) = OpenOptions::new().create(true).append(true).open(log) {
        let _ = handle.write_all(line.as_bytes());
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run from `~/crates`: `cargo test -p dotfiles-test-support --locked`

Expected: PASS, 3 tests.

- [ ] **Step 6: Verify the sabotage control**

The mechanism must be provably load-bearing. Temporarily replace the body of `skip_to` with `let _ = (log, reason);` and re-run.

Run from `~/crates`: `cargo test -p dotfiles-test-support --locked`

Expected: FAIL on `skip_appends_a_line_naming_the_reason` and `skips_accumulate_rather_than_overwrite`, because no file is created. Restore the real body and confirm PASS again. Do not commit the sabotaged version.

- [ ] **Step 7: Teach the gate to truncate and report**

In `tests/rust-checks.sh`, before the `cargo test` invocation at line 139, add:

```sh
# The skip log. Rust's harness has no runtime skip, so a test that cannot run
# here records one line to this file and the count is reported below. See
# docs/superpowers/specs/2026-09-07-shell-test-port-design.md section 4b.1.
#
# Truncated rather than deleted: a stale count from a previous run would
# overstate missing coverage, and a missing file is indistinguishable from a
# run where nothing skipped.
DOTFILES_SKIP_LOG="${TMPDIR:-/tmp}/dotfiles-rust-skips.jsonl"
export DOTFILES_SKIP_LOG
: > "$DOTFILES_SKIP_LOG"
```

After the `cargo test` block succeeds, add:

```sh
# Report the skips the way run-all.sh does for the shell suites, because a
# gate that says nothing when it skips is indistinguishable from one that is
# not installed. Silent when nothing skipped: a trailing "0 skipped" on every
# run is noise, and noise is what a reader learns to scan past.
if [ -s "$DOTFILES_SKIP_LOG" ]; then
    skipped=$(wc -l < "$DOTFILES_SKIP_LOG" | tr -d ' ')
    printf 'rust-checks: %s skipped\n' "$skipped"
    sed -n 's/.*"reason":"\(.*\)"}/  skip: \1/p' "$DOTFILES_SKIP_LOG"
fi
```

- [ ] **Step 8: Verify the gate reports a real skip**

`crates/config-cli/tests/nvim_runtime.rs` already skips at runtime with `eprintln!`. Change its two `eprintln!("skip: ...")` calls to `dotfiles_test_support::skip(...)` plus the same early return, and add `dotfiles-test-support = { path = "../dotfiles-test-support" }` to `crates/config-cli`'s `[dev-dependencies]`.

Run from `~`: `tests/rust-checks.sh`

Expected: on a machine with no `nvim` on PATH, the output ends with `rust-checks: 1 skipped` (or 2) and the reason lines. On a machine with `nvim`, no skip line appears at all. Either outcome is correct; record which one you saw.

- [ ] **Step 9: Commit**

```bash
cd ~
config add crates/Cargo.toml crates/Cargo.lock crates/dotfiles-test-support \
    crates/config-cli/Cargo.toml crates/config-cli/tests/nvim_runtime.rs tests/rust-checks.sh
config commit -F - <<'MSG'
tests: give Rust a runtime skip the gate can count

Rust's harness has no representation for "this check cannot run here".
#[ignore] is compile-time and hides the count, and eprintln is swallowed
by cargo test --quiet, which is what rust-checks.sh runs. Measured: that
command reported zero skip lines from nvim_runtime.rs, which already
skipped that way, so the gate was blind before any conversion.

dotfiles-test-support::skip appends one line to DOTFILES_SKIP_LOG.
rust-checks.sh truncates it before the run and reports "N skipped" plus
the reasons after, which is what run-all.sh already does for the shell
suites. Silent when nothing skipped.

nvim_runtime.rs moves onto it, so the two skips it already had become
visible to the gate.

Spec: docs/superpowers/specs/2026-09-07-shell-test-port-design.md 4b.1

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
```

---

### Task 2: Convert `config-docs.test.sh`, the reproduced defect

Spec section 3 and section 5: this suite goes first "because its defect is the reproduced one". The defect is at `tests/config-docs.test.sh:52`:

```sh
sed -n 's/^- `config \([a-z-]*\)`.*/\1/p'
```

Change the README's bullet marker from `- ` to `* `, the edit a Markdown linter makes, and this yields zero names, zero loop iterations, and `assert_equals '' ''` passes. The pattern also hardcodes backticks and `[a-z-]*`, so a subcommand name with a digit silently drops out.

**Files:**
- Create: `crates/config-cli/tests/config_docs.rs`
- Modify: `crates/config-cli/Cargo.toml` (add `pulldown-cmark` to `[dev-dependencies]`)
- Delete: `tests/config-docs.test.sh`
- Modify: `tests/run-all.sh` if it carries a suite count (check; it discovers by glob, so probably not)

**Interfaces:**
- Consumes: `dotfiles_test_support::skip` from Task 1.
- Produces: `fn readme_subcommand_bullets(markdown: &str) -> Vec<String>`, a private helper in this test file that returns every subcommand named by a bullet in the README's config section. Later Markdown conversions may promote it into `dotfiles-test-support` once a second caller exists; do not promote it now.

- [ ] **Step 1: Write the failing test**

`crates/config-cli/tests/config_docs.rs`:

```rust
//! The README's config section must name exactly the subcommands that exist.
//!
//! Converted from tests/config-docs.test.sh, whose extraction was
//! `sed -n 's/^- `config \([a-z-]*\)`.*/\1/p'`. That pattern yields nothing
//! when the bullet marker changes from `-` to `*`, which a Markdown linter
//! does, and the suite's `assert_equals '' ''` then passes having checked
//! nothing. A real parser makes that unrepresentable: `Event::Start(Item)`
//! is the same event either way.

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    // The workspace lives at <root>/crates, and this test runs with its
    // manifest directory as the working directory.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("the repo root is two levels above config-cli")
        .to_path_buf()
}

/// Every subcommand named by a list item in the README, in document order.
///
/// Reads inline code spans inside list items rather than matching a line
/// shape, so the bullet marker, the indentation and the surrounding prose
/// are all free to change.
fn readme_subcommand_bullets(markdown: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut list_depth: usize = 0;
    let mut in_item = false;
    for event in Parser::new(markdown) {
        match event {
            Event::Start(Tag::List(_)) => list_depth += 1,
            Event::End(TagEnd::List(_)) => list_depth = list_depth.saturating_sub(1),
            Event::Start(Tag::Item) => in_item = true,
            Event::End(TagEnd::Item) => in_item = false,
            Event::Code(code) if in_item && list_depth > 0 => {
                if let Some(rest) = code.strip_prefix("config ") {
                    let name = rest.split_whitespace().next().unwrap_or_default();
                    if !name.is_empty() {
                        found.push(name.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    found
}

/// The positive control the spec's section 4 requires: prove the parser
/// found something before asserting anything about what it did not find.
#[test]
fn the_readme_names_at_least_one_subcommand() {
    let readme = std::fs::read_to_string(repo_root().join("README.md")).expect("README.md");
    let named = readme_subcommand_bullets(&readme);
    assert!(
        !named.is_empty(),
        "the parser found no subcommand bullets at all, so every other \
         assertion in this file would pass vacuously"
    );
}

/// The defect the shell version could not catch: a marker change.
#[test]
fn the_parser_survives_a_bullet_marker_change() {
    let with_dash = "- `config build` builds things.\n";
    let with_star = "* `config build` builds things.\n";
    assert_eq!(
        readme_subcommand_bullets(with_dash),
        readme_subcommand_bullets(with_star),
        "the marker is Markdown syntax, not content, so it must not change \
         what the parser sees"
    );
    assert_eq!(readme_subcommand_bullets(with_dash), vec!["build".to_string()]);
}

/// The second half of the same defect: the old pattern's `[a-z-]*` dropped
/// any name containing a digit.
#[test]
fn a_subcommand_name_with_a_digit_is_not_dropped() {
    let markdown = "- `config s3-sync` does a thing.\n";
    assert_eq!(readme_subcommand_bullets(markdown), vec!["s3-sync".to_string()]);
}

/// The assertion the shell suite existed for: no bullet names a subcommand
/// that has been removed.
#[test]
fn the_readme_bullets_name_no_removed_subcommand() {
    let root = repo_root();
    let readme = std::fs::read_to_string(root.join("README.md")).expect("README.md");
    let named = readme_subcommand_bullets(&readme);
    assert!(!named.is_empty(), "positive control: the parser found bullets");

    let script_directory = root.join(".scripts/config");
    let Ok(entries) = std::fs::read_dir(&script_directory) else {
        dotfiles_test_support::skip("no .scripts/config here, so scripts cannot be enumerated");
        return;
    };

    let mut existing: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()
                .and_then(|name| name.strip_prefix("config-"))
                .map(str::to_string)
        })
        .collect();
    // Verbs the binary owns rather than a script. Kept explicit: the listing
    // is the union of scripts and binary subcommands, and a name that is in
    // neither is the defect this test catches.
    existing.extend(["deps".to_string()]);

    let stale: Vec<&String> = named
        .iter()
        .filter(|name| !existing.contains(name) && !name.contains(' '))
        .collect();
    assert!(
        stale.is_empty(),
        "the README names subcommands that no longer exist: {stale:?}"
    );
}

/// The forward direction, and the assertion this plan's first draft omitted:
/// a script that exists but no bullet names. The shell suite had it at
/// tests/config-docs.test.sh:38 with a `grep -qF "config $sub"` over the
/// section, and dropping it in conversion would have lost coverage silently,
/// which is the exact failure this plan names as its weakest link.
///
/// Note the asymmetry with the reverse direction: this one matches the
/// section text rather than the parsed bullets, because a subcommand may be
/// documented in prose (`config status`, `config commit` are named as git
/// passthrough) without being a bullet. The shell suite's grep had the same
/// property.
#[test]
fn every_config_script_appears_in_the_readme_section() {
    let root = repo_root();
    let readme = std::fs::read_to_string(root.join("README.md")).expect("README.md");

    let script_directory = root.join(".scripts/config");
    let Ok(entries) = std::fs::read_dir(&script_directory) else {
        dotfiles_test_support::skip("no .scripts/config here, so scripts cannot be enumerated");
        return;
    };

    let mut scripts: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()
                .and_then(|name| name.strip_prefix("config-"))
                .map(str::to_string)
        })
        .collect();
    scripts.sort();
    assert!(
        !scripts.is_empty(),
        "positive control: no config-* scripts found, so this assertion would \
         pass vacuously"
    );

    let undocumented: Vec<&String> = scripts
        .iter()
        .filter(|name| !readme.contains(&format!("config {name}")))
        .collect();
    assert!(
        undocumented.is_empty(),
        "these subcommands exist but the README never names them: {undocumented:?}"
    );
}

/// The section must point at the generated listing rather than restating it,
/// so descriptions have exactly one home.
#[test]
fn the_readme_points_at_config_help() {
    let readme = std::fs::read_to_string(repo_root().join("README.md")).expect("README.md");
    assert!(
        readme.contains("config help"),
        "the README must send the reader to `config help` for descriptions"
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run from `~/crates`: `cargo test -p config-cli --test config_docs --locked`

Expected: FAIL at compile time with `unresolved import pulldown_cmark` (or `can't find crate`), because the dependency is not declared yet.

- [ ] **Step 3: Declare the dependency**

In `crates/config-cli/Cargo.toml`, under `[dev-dependencies]`, add:

```toml
pulldown-cmark = { version = "0.13", default-features = false }
```

`default-features = false` drops the crate's own CLI and HTML renderer, which this test does not use.

- [ ] **Step 4: Run the test to verify it passes**

Run from `~/crates`: `cargo test -p config-cli --test config_docs --locked`

Expected: PASS, 6 tests. If `the_readme_bullets_name_no_removed_subcommand` or `every_config_script_appears_in_the_readme_section` fails, the README genuinely disagrees with the tree and the README is what to fix, not the test.

- [ ] **Step 5: Verify the conversion caught what the shell could not**

Prove the new test is strictly stronger. Change one README bullet's marker from `- ` to `* `:

Run from `~`: `bash tests/config-docs.test.sh`

Expected: PASS. The shell suite does not notice.

Run from `~/crates`: `cargo test -p config-cli --test config_docs --locked`

Expected: PASS as well, because the parser is marker-agnostic, which is the point. Now instead add a bullet naming a subcommand that does not exist, `- \`config nonesuch\` does nothing.`:

Run from `~/crates`: `cargo test -p config-cli --test config_docs --locked`

Expected: FAIL on `the_readme_bullets_name_no_removed_subcommand`, naming `nonesuch`. Revert both README edits.

- [ ] **Step 6: Delete the shell suite**

```bash
cd ~
config rm tests/config-docs.test.sh
```

Then check whether anything pins the suite count:

Run from `~`: `grep -rn "config-docs" tests/ .github/workflows/ .claude/rules/ | grep -v Binary`

Expected: no remaining references. If `tests/scripts-dir-name.test.sh` or a workflow carries a literal count of suites, update it in this commit; the spec excludes that suite from *conversion*, not from *maintenance*.

- [ ] **Step 7: Run the full suite both ways**

Run from `~`: `tests/run-all.sh -q`

Expected: all suites pass, one fewer than before, and the summary line reports the new total.

Run from `~`: `tests/rust-checks.sh`

Expected: passes, and reports any skip from Task 1's mechanism.

- [ ] **Step 8: Commit**

```bash
cd ~
config add crates/config-cli/Cargo.toml crates/config-cli/tests/config_docs.rs \
    crates/Cargo.lock tests/config-docs.test.sh
config commit -F - <<'MSG'
tests: convert config-docs to Rust with a real Markdown parser

The shell suite extracted subcommand names with
sed -n 's/^- `config \([a-z-]*\)`.*/\1/p'. Change the README's bullet
marker from - to *, which a Markdown linter does, and it yields zero
names, zero loop iterations, and assert_equals '' '' passes having
checked nothing. The same pattern hardcoded [a-z-]*, so a name with a
digit dropped out silently.

pulldown-cmark reads inline code spans inside list items, so the marker,
the indentation and the surrounding prose are all free to change. Two
tests pin the defects directly: a marker change must not alter what the
parser sees, and s3-sync must not be dropped.

Carries the spec's positive control: a test asserts the parser found
bullets at all, so the stale-subcommand assertion cannot pass vacuously.

Spec: docs/superpowers/specs/2026-09-07-shell-test-port-design.md
section 3, tranche A, first suite by section 5's order.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
```

---

### Task 3: Convert the YAML workflow suites

Spec section 5: "Then the YAML suites, since `serde_yaml` covers seven at once." Nine suites parse `.github/workflows/`, minus the two excluded (`container`, `scripts-dir-name`), leaving seven: `bootstrap-harness`, `check-deps`, `deps-harness`, `readme-badges`, `shellcheck`, `workflow-action-versions`, `workflow-labels`.

Start with `workflow-labels.test.sh`, which spec section 3 calls "the sharpest omission" because line 50 already shells out to an inline Python parser to filter `.yml` and `.yaml`. That is the strongest single argument for this tranche, and it means the suite's own author already reached for a real parser.

**Files:**
- Create: `crates/config-cli/tests/workflow_labels.rs`
- Modify: `crates/config-cli/Cargo.toml` (add `serde_yaml` and `serde` to `[dev-dependencies]`)
- Delete: `tests/workflow-labels.test.sh`

**Interfaces:**
- Consumes: `dotfiles_test_support::skip` from Task 1.
- Produces: `fn workflow_files(root: &Path) -> Vec<PathBuf>` and `fn parse_workflow(text: &str) -> serde_yaml::Value`, private to this test file. Task 4 promotes them into `dotfiles-test-support` once the second YAML suite needs them; do not promote now, because two callers is the threshold and this is one.

- [ ] **Step 1: Read the suite being converted**

Run from `~`: `cat tests/workflow-labels.test.sh`

Do not write the Rust until you can state, in one sentence each, what every assertion in that file claims. Write those sentences as the doc comments of the Rust tests. A conversion that drops an assertion is a regression that no gate will catch, because the shell file is deleted in the same commit.

- [ ] **Step 2: Write the failing tests**

One `#[test]` per assertion in the shell suite, each named for the behavior rather than the mechanism, with the shell suite's own comment as the doc comment where it explains a non-obvious claim. Every test that expects an empty result opens with a positive control, per the Global Constraints.

The YAML reading shape, to use in each:

```rust
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
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
```

- [ ] **Step 3: Run the tests to verify they fail**

Run from `~/crates`: `cargo test -p config-cli --test workflow_labels --locked`

Expected: FAIL at compile time with `unresolved import serde_yaml`.

- [ ] **Step 4: Declare the dependencies**

In `crates/config-cli/Cargo.toml`, under `[dev-dependencies]`:

```toml
serde_yaml = "0.9"
```

`serde` is already a workspace dependency; confirm with `grep -n serde crates/config-cli/Cargo.toml` before adding a second entry.

- [ ] **Step 5: Run the tests to verify they pass**

Run from `~/crates`: `cargo test -p config-cli --test workflow_labels --locked`

Expected: PASS, with one test per assertion the shell suite carried.

- [ ] **Step 6: Verify each conversion is load-bearing**

For each test, break the thing it asserts about and confirm that test alone goes red. A test that stays green when its subject is broken is asserting the wrong thing, and this is the only point in the process where that is cheap to discover. Record which sabotage you used for each; restore after each one.

- [ ] **Step 7: Delete the shell suite and run both gates**

```bash
cd ~
config rm tests/workflow-labels.test.sh
grep -rn "workflow-labels" tests/ .github/workflows/ .claude/rules/ | grep -v Binary
```

Run from `~`: `tests/run-all.sh -q` then `tests/rust-checks.sh`

Expected: both pass.

- [ ] **Step 8: Commit**

```bash
cd ~
config add crates/config-cli/Cargo.toml crates/config-cli/tests/workflow_labels.rs \
    crates/Cargo.lock tests/workflow-labels.test.sh
config commit -F - <<'MSG'
tests: convert workflow-labels to Rust with serde_yaml

The spec calls this suite tranche A's sharpest argument: line 50 already
shelled out to an inline Python parser to filter .yml and .yaml, so its
own author had already reached for a real parser and the shell could not
supply one.

One Rust test per assertion the shell suite carried, each with a positive
control where it expects an empty result, and each verified load-bearing
by breaking its subject and watching that test alone go red.

Spec: docs/superpowers/specs/2026-09-07-shell-test-port-design.md
section 3, tranche A; section 5 order, YAML after config-docs.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
```

---

### Task 4: Promote the shared YAML helpers, then convert the remaining six

Two callers is the threshold for extraction. Task 3 produced one; the second YAML suite is the second, so the helpers move to `dotfiles-test-support` at the start of this task rather than being copied a third time.

**Files:**
- Modify: `crates/dotfiles-test-support/src/lib.rs` (add a `repo` module)
- Modify: `crates/dotfiles-test-support/Cargo.toml` (`serde_yaml` moves to `[dependencies]`)
- Modify: `crates/config-cli/tests/workflow_labels.rs` (use the promoted helpers)
- Create: one test file per remaining suite
- Delete: the six remaining shell suites, one per commit

**Interfaces:**
- Consumes: Task 1's crate, Task 3's helper shapes.
- Produces: `dotfiles_test_support::repo::root() -> PathBuf`, `repo::workflow_files() -> Vec<PathBuf>`, `repo::parse_workflow(&str) -> serde_yaml::Value`.

- [ ] **Step 1: Move the helpers, with a test**

Add to `crates/dotfiles-test-support/src/lib.rs` a `pub mod repo` holding `root`, `workflow_files` and `parse_workflow`, verbatim from Task 3. Add a test in `crates/dotfiles-test-support/tests/repo.rs` asserting `workflow_files` finds both extensions, using a `tempfile` directory containing one `.yml` and one `.yaml`.

- [ ] **Step 2: Run it to verify it fails, then passes**

Run from `~/crates`: `cargo test -p dotfiles-test-support --locked`

Expected: FAIL first (`repo` does not exist), then PASS after the module lands.

- [ ] **Step 3: Point Task 3's file at the promoted helpers**

Delete the three private functions from `crates/config-cli/tests/workflow_labels.rs` and import them. Re-run that suite:

Run from `~/crates`: `cargo test -p config-cli --test workflow_labels --locked`

Expected: PASS, unchanged behavior.

- [ ] **Step 4: Commit the promotion**

```bash
cd ~
config add crates/dotfiles-test-support crates/config-cli/tests/workflow_labels.rs crates/Cargo.lock
config commit -F - <<'MSG'
tests: promote the YAML test helpers on their second caller

Two callers is the extraction threshold. workflow_labels was the first and
the next YAML conversion is the second, so the helpers move now rather
than being copied a third time.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
```

- [x] **Step 5: Convert the remaining suites, one commit each** -- DONE for three of five; the last two are re-scoped below.

> **CORRECTED 2026-09-10 during execution.** This step named six suites.
> Two of those instructions were wrong:
>
> **`check-deps.test.sh` does not exist.** It was deleted in `13df32d5`,
> "Retire check-deps.sh, and delete the fixture that hid its worst bug", on
> 2026-09-07. The spec's section 3 table still lists it and this plan
> inherited the error. So the count is five, not six, and Step 6's "seven
> fewer suites" is five fewer.
>
> **`bootstrap-harness` and `deps-harness` must NOT be converted wholesale.**
> Measured: 484 lines and 77 assertions, of which 9 parse YAML; 526 lines and
> 52 assertions, of which 3 parse YAML. The other ~115 are greps over
> Dockerfiles and shell text, file-mode checks, `git ls-tree` reads, TOML
> greps, and two behaviours driven against fixture `$HOME`s with their own
> bare repos.
>
> Deleting the shell files would drag those ~115 assertions into Rust as the
> same substring matching, gaining nothing against the defect class this
> tranche targets, and would land the fixture-harness work the spec assigns
> to tranche C. The spec says so directly at its lines 111-124: the tranches
> "OVERLAP ... A suite in both gets its parser work in A and its fixture work
> in C."
>
> The tranche-A win in these two is real and still worth taking: both parse
> workflows with `python3`, and both carry python-gated skips
> (`bootstrap-harness.test.sh:132-133`, and `deps-harness`'s 60-line inline
> parser). `yaml_serde` removes the interpreter dependency and those skips.
> That is a **scoped extraction**: move only the YAML assertions to Rust and
> leave each shell suite owning its Dockerfile, fixture and file-mode
> assertions. It is deferred to its own task rather than improvised here,
> because the Global Constraints forbid two harnesses owning one assertion
> and the split needs deciding, not inventing.

Converted, in this order, cheapest first: `workflow-action-versions` (`28a54d83`), `readme-badges` (`a6743f02`), `shellcheck` (`10c83f09`).

> **DECIDED 2026-09-10, after the correction above: convert both suites in
> full.** The owner's call, and the reason is consistency: one suite should
> not live in two harnesses, even when the split would be along disjoint
> assertion sets.
>
> This overrides the scoped-extraction recommendation above. That
> recommendation was right that ~117 of the 129 assertions gain nothing
> technically from Rust (`test -f` becomes `Path::is_file`), and the owner
> has weighed that against a reader having to know two files hold one
> suite's assertions. Consistency won. Recorded so the tradeoff is not
> re-litigated: the cost is known and accepted, not overlooked.
>
> So `bootstrap-harness` (77 assertions, 484 lines) and `deps-harness` (52
> assertions, 526 lines) convert whole, one commit each, and both `.test.sh`
> files are deleted. The 12 python-gated YAML assertions lose their skips.
> The remaining assertions carry over as the same checks in Rust:
> `test -f` to `is_file`, `test -x` to a mode check, `git ls-tree` to a
> `Command`, Dockerfile and shell-text greps to `contains` over a read file.
>
> Tranche C then has nothing left to do for these two, which is the other
> half of the consistency argument.

For each, repeat Task 3's steps 1 through 8 exactly: read the suite and state every assertion in a sentence, write one test per assertion with a positive control, watch it fail, implement, watch it pass, sabotage each test to prove it is load-bearing, delete the shell file, run both gates, commit. Do not batch two suites into one commit; a reviewer must be able to reject one conversion while accepting its neighbor.

Two suites need a skip from Task 1's mechanism rather than an assertion:
- `shellcheck.test.sh` asserts on `shellcheck` output. When the binary is absent, call `dotfiles_test_support::skip("shellcheck is not installed; `config install` adds it")` and return.
- `deps-harness.test.sh` and `bootstrap-harness.test.sh` read the deps workflow's container jobs. Where a job is absent because the workflow was rewritten, that is a failure, not a skip. Only an absent *tool* is a skip.

- [ ] **Step 6: Confirm the tranche's boundary held**

Run from `~`: `ls tests/*.test.sh | wc -l`

Expected: seven fewer suites than before Task 2. `container.test.sh` and `scripts-dir-name.test.sh` must both still exist, per the Global Constraints.

Run from `~`: `grep -rn "sed -n\|grep -oE" tests/*.test.sh | grep -c yml`

Expected: zero remaining YAML extraction by regex outside the two excluded suites.

---

## Self-Review

**1. Spec coverage.** Tranche A's YAML column (9 suites) is covered by Tasks 3 and 4, minus the two the spec excludes. The Markdown column's first suite (`config-docs`) is Task 2; the remaining Markdown and TOML suites are **not** covered by this plan, because spec section 5 orders Tranche A as "config-docs first, then the YAML suites" and stops there. That is a deliberate gap, not an omission: the Markdown and TOML remainder should get its own plan once Tasks 1 through 4 have shown what the helpers actually want to be. Section 4b.1's skip mechanism is Task 1. Section 4b.2's tmux fixture is **not** in this plan, because no Tranche A suite uses tmux; it belongs to the plan that converts the four tmux suites.

**2. Placeholder scan.** No TBD, no "implement later", no "similar to Task N" without the code. Task 4 Step 5 says "repeat Task 3's steps 1 through 8 exactly" and names the per-suite deviations, which is a deliberate reference to a procedure rather than to code; the code it would repeat is the same twelve-line YAML helper already given in full in Task 3 and promoted in Task 4 Step 1.

**3. Type consistency.** `skip` and `skip_to` keep the same signatures across Tasks 1, 2, 3 and 4. `repo_root` in Tasks 2 and 3 becomes `repo::root` in Task 4 Step 3, which is called out explicitly rather than left to drift. `readme_subcommand_bullets` stays private to Task 2's file, as stated in that task's Interfaces block, and is never referenced elsewhere.

**One risk this plan does not eliminate.** Deleting each `.test.sh` in the same commit as its replacement means the shell assertion is gone before anyone has run the Rust version on the other platform. The spec's step 4 keeps `lib.sh` and `run-all.sh` alive until both suites have run green side by side, which covers the harness but not an individual suite's assertions. The mitigation in this plan is Task 3 Step 6 and its repetition in Task 4: every converted test must be proven load-bearing by sabotage before its shell original is deleted. That is a discipline, not a gate, and it is the weakest link in the plan.
