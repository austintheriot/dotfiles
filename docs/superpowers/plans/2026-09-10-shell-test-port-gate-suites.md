# Shell Test Port, The Two Gate Suites Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the two suites Tranche A deliberately excluded, `container.test.sh` (**54** assertions, corrected from an undercount of 48: the eight-tool loop expands one call site into eight) and `scripts-dir-name.test.sh` (15), once nothing else depends on them as gates.

**Architecture:** Both become integration tests under `crates/config-cli/tests/`, reading tracked files through `dotfiles_test_support::repo`. Neither has a shell subject, so both convert wholly. The difficulty is not the conversion; it is that each suite is the gate another spec's step relies on, so this plan's first task is proving that dependency has lapsed.

**Tech Stack:** Rust 2024, `dotfiles-test-support`, `std::process::Command` for `git ls-tree` and `dash -n`. **No new crates, and that includes `toml`:** an earlier draft named it for manifest reads, but it is not a dev-dependency of `config-cli` and this plan adds none. Task 3's workspace-members check parses the single-line `members = [...]` array directly, which is the same shape `config-manifest::git::parse_workspace_members` and `.scripts/config/config-stamp` both already require and document.

**Spec:** `docs/superpowers/specs/2026-09-07-shell-test-port-design.md`, section 3's exclusion note. Tranche A is done (`fd3b4435`). This plan is the spec's "those two convert after step 4."

> **STATUS 2026-09-10: IMPLEMENTED.** Both suites converted:
> `bebf8053` (scripts-dir-name, 15 assertions to 13 tests) and `38b52abb`
> (container, 54 assertions to 17 tests). Task 1 confirmed both gate
> dependencies historical.
>
> **Sabotage caught two defects that review did not, both in the conversion
> itself:**
>
> 1. The macOS-exclusion assertion was **inherited vacuous**. With the real
>    `RUN rm -f` deleted and only the comment naming the suite left, it stayed
>    green: the comments-are-not-code defect this plan's own constraints warn
>    about, reproduced faithfully from the shell suite. Fixed to read parsed
>    instructions.
> 2. Ref resolution probed the wrong repository. `run-in-docker.sh` reads
>    `$HOME/.cfg` unconditionally rather than the root the test reads through,
>    so deriving the expectation from the root predicted "no repository" while
>    the runner resolved `HEAD` against the real one.
>
> **Two mandatory sabotages both went red**, as required. The workspace-member
> one needs a real stub directory, because cargo refuses to load a workspace
> whose member has no directory. That is also the honest shape: a new crate
> arrives as a directory. The `$HOME` mount check was confirmed to
> discriminate rather than ban the flag, since a `:ro` mount still passes.
>
> **Step 7, the circularity, is partly unverified.** `container_image.rs`
> skips nothing by design, so the container leg exercises a real contract
> rather than standing down. That intent was not observed inside the
> container: `run-in-docker.sh` was not run directly. The push gate does run
> it, so the next push is the verification.
>
> **Stale in this document:** Task 3's heading and Step 1 still say 48
> assertions where the goal line says the corrected 54, and Task 2's preamble
> repeats the old number. Step 1's group list still omits three of the ten
> groups (docker guard, ref resolution, and the 17-assertion `TRIGGER_PATHS`
> group), so a reader working only from it would under-convert.

## Why these two were held back

The spec's words:

> **Two exclusions, because steps 3b and 4 use them as gates.** `container.test.sh` is what the adapter spec relies on to assert cargo's workspace members against the test Dockerfile, and `scripts-dir-name.test.sh` is what the subcommand spec relies on to count scripts exactly. Converting either while another step depends on it puts two documents in one file for different reasons.

That is a sequencing constraint, not a technical one, and **it must be re-checked rather than assumed lapsed**. Task 1 exists for that. Today's evidence that the dependency is live: Tranche A added a workspace member and `container.test.sh` caught the missing Dockerfile lines, red at `[ dotfiles-test-support]`, which no other suite noticed. That is the gate doing its job three commits ago.

## This plan's first draft broke the gate it converts

Recorded because it is the most useful thing in this document.

The first draft spelled the pre-rename directory name in prose at four
places. `tests/scripts-dir-name.test.sh` sweeps tracked files for exactly
that string and reads `docs/` as a search root, so committing this plan
turned the suite red: 13 passed, 2 failed. The commit reached `origin/main`
because the pre-push gate runs the suite only when a pushed path matches
`TRIGGER_PATHS`, and `docs/` is not in that list.

Two lessons, both binding on every task below:

**Do not write the pre-rename name in any tracked file, including this one.**
Describe it. The Rust conversion assembles its own needle at runtime for the
same reason, so the test file does not match itself.

**A gate that runs only on triggered paths can be broken by an untriggered
path.** That is not a defect in the trigger list, which exists so a
docs-only push does not pay for the whole suite. It is a reason to run
`bash tests/run-all.sh -q` before pushing anything, including a
documentation change, when the change contains text that any suite reads.

## Global Constraints

Every one learned by executing Tranche A. Every task's requirements implicitly include this section.

- **All Rust invocations run from inside `crates/`, never with `--manifest-path`.**
- **`cargo test --locked --quiet` stays the gate command.** Do not add `--nocapture`.
- **`#[ignore]` must not be used for a runtime skip.** Use `dotfiles_test_support::skip(reason)` and return.
- **Positive controls are required.** Any assertion expecting an empty result must first prove its pipeline produced something. Tranche A's `deps-harness` conversion found a `git ls-tree` pathspec bug precisely because the control was there; without it two empty sets would have compared equal and passed.
- **`git ls-tree` needs a rooted pathspec and `--full-name`** when run from a test. A bare relative pathspec resolves against the cwd prefix, which for an integration test is `crates/config-cli`, not the repo root. Use `:/deps`-style pathspecs and `--full-name`, both learned in `f35fe5d4`.
- **Locate tracked files with `dotfiles_test_support::repo::root()`**, never `CARGO_MANIFEST_DIR` alone.
- **Comments are not code.** Tranche A found whole-file greps satisfied by prose in comments: deleting a real check left an assertion green because a comment above it named the thing. Strip comments before matching, as `bootstrap_harness.rs` now does.
- **Join backslash continuations before matching a Dockerfile line.** `^RUN (apt-get|pacman)` matched only `RUN apt-get update \` and could never see the package list, which made an assertion vacuous for the life of the suite. Measured in `faf7ee3a`.
- **Every test file opens with a `//!` module doc.**
- **`crates/Cargo.lock` goes in every commit that touches a manifest.**
- **`tests/rust-checks.sh` archives from a git ref**, so run it AFTER committing.
- **One suite per commit.**
- **The repo is public.** No employer or product names in tracked files.
- **After restoring a sabotage by moving a backup back, `touch` the file.** A
  restore-by-move carries the backup's older mtime, so cargo does not rebuild
  and the **sabotaged binary keeps running against restored source**. That
  produces a false red or a false green in exactly the step these plans
  mandate. Found while executing this plan.
- **Never `--no-verify`.** Never disable a test instead of fixing it.

---

### Task 1: Prove the gate dependency has lapsed, or stop

This task's deliverable is a decision, not code. Do not convert anything until it is answered, and if the answer is "still live", stop and report rather than converting.

**Files:** none modified.

- [ ] **Step 1: Check what still depends on `container.test.sh` as a gate**

Run from `~`:

```bash
grep -rn "container.test.sh" docs/superpowers/ tests/ .github/workflows/ .claude/rules/ | grep -v Binary
```

Then read `docs/superpowers/specs/2026-09-07-config-cli-adapter-design.md` for the step that relies on it (the spec's step 3b), and check that plan's status banner. `plans/2026-09-07-config-cli-adapter.md` was verified SHIPPED on 2026-09-10, so its dependency is historical rather than live, but **confirm that from the banner rather than from this sentence**.

- [ ] **Step 2: Check what still depends on `scripts-dir-name.test.sh`**

Run from `~`:

```bash
grep -rn "scripts-dir-name" docs/superpowers/ tests/ .github/workflows/ .claude/rules/ | grep -v Binary
grep -n "24\|count" tests/scripts-dir-name.test.sh | head
```

`plans/2026-09-07-config-subcommand-ports.md` relied on it to "count scripts exactly", and that plan was verified SHIPPED. Note the count it pins is **24** as of 2026-09-10, and that the subcommand plan's own count instructions (38 to 37 to 36 to 35 to 34) are stale and superseded, which is recorded in that plan's banner. So the pinned number moved for reasons outside either plan, and the conversion must carry the *current* number, read from the file, not any number in a document.

- [ ] **Step 3: Name what these two suites uniquely catch**

Before converting a gate, know what it guards. Write down, from reading them:

- `container.test.sh`: which assertions are the only check on their subject anywhere in the repo. The workspace-members-versus-Dockerfile assertion is one, proven by Tranche A: it was the only suite that caught `dotfiles-test-support` missing from the test image.
- `scripts-dir-name.test.sh`: same question. The staleness assertions (which sweep tracked files for the pre-rename directory name, both its dotted form and its bare stem) and the sourced-versus-executable mode checks look unique.

Any assertion in this set that is the sole guard on its subject must survive the conversion with the same strength or better. Report the list.

- [ ] **Step 4: Decide and report**

If both dependencies are historical, proceed to Task 2. If either is live, stop: converting a gate another in-flight step depends on is what the spec's exclusion exists to prevent, and no amount of care in the conversion fixes that.

---

### Task 2: Convert `scripts-dir-name.test.sh` (15 assertions)

Smaller of the two, and it goes first so the `git ls-tree` and file-mode patterns settle before the 48-assertion suite.

**Files:**
- Create: `crates/config-cli/tests/scripts_dir_name.rs`
- Delete: `tests/scripts-dir-name.test.sh`

**Interfaces:**
- Consumes: `dotfiles_test_support::{skip, repo}`.
- Produces: nothing other tasks depend on.

- [ ] **Step 1: Inventory all 15 assertions**

Read the suite end to end. One sentence per assertion, becoming the doc comments. Known shape from a first read:

1. `.scripts` exists; the pre-rename directory is gone.
2. Every script moved to `.scripts` (a set comparison, so it needs a positive control).
3. Every executed script is still executable.
4. No sourced script is marked executable (the inverse, and a real invariant: a sourced file with the execute bit invites being run directly).
5. At least one search root is present (this **is** a positive control, already).
6. No tracked file still names the pre-rename directory, in either its dotted form or its bare stem. **Two separate assertions, and the stem one exists because of a real defect:** the first rename pass left an assertion whose needle was the bare stem, and once its input path became `.scripts` that assertion passed unconditionally.
7. A committed-script count, pinned as a literal, plus an explicit list of committed execute bits.

Report the real count and classification before writing Rust.

- [ ] **Step 2: Note the skip this suite already has, and why it must survive**

`.claude/rules/dotfiles-tests.md` records this suite as the origin of the repo's skip-reporting rule: its commit-inspection block skipped silently in the container, `finish` counted only passes and failures, and the pre-push Docker gate printed PASS over an assertion that never ran. A stale committed-script count shipped, and both CI platforms caught it instead.

So the container case is a **skip**, and it must be recorded. In Rust:

```rust
/// The committed execute bits, read from the repository.
///
/// This block is the origin of this repo's skip-reporting rule. It used to
/// print one line into the suite's captured output and nothing else, so
/// `finish` counted only passes and failures and the pre-push Docker gate
/// printed PASS over an assertion that never ran. A stale count shipped and
/// CI caught it, not this suite.
///
/// The container tree is built with `git archive` and carries no repository,
/// which is a correct reason to stand down, so this skips there. It must
/// skip LOUDLY: a gate that says nothing when it skips is indistinguishable
/// from a gate that is not installed.
#[test]
fn every_committed_script_has_the_expected_execute_bit() {
    // NOTE: `root.join(".git").exists()` is WRONG here and an earlier draft
    // of this plan showed it. This repo's git directory is `~/.cfg`, bare,
    // so that check is false on the only machine that can run the
    // assertion, and it would skip silently. Ask git, and try both layouts:
    //
    //   for candidate in [root.join(".cfg"), root.join(".git")] {
    //       git --git-dir=<candidate> rev-parse --verify HEAD
    //   }
    //
    // with GIT_DIR and GIT_WORK_TREE removed from the child environment. A
    // `.git` directory git cannot resolve a HEAD from is not a repository
    // this test can inspect.
    let root = dotfiles_test_support::repo::root();
    if git_dir(&root).is_none() {
        dotfiles_test_support::skip(
            "no repository here, so committed execute bits cannot be inspected",
        );
        return;
    }
    // ... git ls-tree with a rooted pathspec and --full-name
}
```

**Determine the real repository test.** This repo's git directory is `~/.cfg`, a bare repo, so `root.join(".git").exists()` is false even on a developer machine. Find the check that actually distinguishes "has a repository" from "archived tree", probably `git rev-parse --git-dir` succeeding, and use that. Report what you used. Getting this wrong in either direction is the defect: a false skip loses the assertion, a false run fails the container leg.

- [ ] **Step 3: Write the failing tests, verify failure, implement, verify pass**

Run from `~/crates`: `cargo test -p config-cli --locked --test scripts_dir_name`

- [ ] **Step 4: Sabotage each assertion group**

Rename a script out of `.scripts`; chmod a sourced script to 755; chmod an executed script to 644; add a tracked file naming the pre-rename directory; change the pinned count. Confirm the right test alone goes red per sabotage, and report which proved which.

- [ ] **Step 5: Verify the skip fires correctly in both directions**

Two runs, both reported:

- On this machine, with a repository: the test runs and asserts. `tests/rust-checks.sh` must NOT report this skip.
- Simulating the container, with `DOTFILES_ROOT` pointed at a tree with no repository: the test skips and the reason appears in the gate's skip list.

This is the assertion whose silent skip shipped a bug once. Proving both directions is not optional here.

- [ ] **Step 6: Delete the shell suite, run every gate, commit**

```bash
cd ~
config rm tests/scripts-dir-name.test.sh
config add crates/config-cli/tests/scripts_dir_name.rs
```

Then `bash tests/rust-gate.test.sh`, `bash tests/run-all.sh -q`, commit, then `tests/rust-checks.sh`.

---

### Task 3: Convert `container.test.sh` (48 assertions)

Last, because it is the largest and because it asserts about the very image the container leg runs in, which makes a mistake here self-concealing.

**Files:**
- Create: `crates/config-cli/tests/container_image.rs`
- Delete: `tests/container.test.sh`

**Interfaces:**
- Consumes: `dotfiles_test_support::{skip, repo}`.
- Produces: nothing.

- [ ] **Step 1: Inventory all 48 assertions**

One sentence each. From a first read the groups are: the Dockerfile and runner exist and are executable; the runner parses as POSIX `sh` and under `dash`; the image installs each named tool; the image sets `HOME`; the entrypoint runs the suite; the workspace members all appear in the builder stage; the image does not bind-mount `$HOME` read-write.

Report the real count and the groups.

- [ ] **Step 2: Preserve the two assertions that are this suite's whole reason to exist**

Two must come through at full strength, and both have already earned it:

**The workspace-members check.** Tranche A added `dotfiles-test-support` to `crates/Cargo.toml` and forgot the five Dockerfile edits. `rust-checks.sh` passed, `rust-gate.test.sh` passed, and only this suite went red with `expected: [] actual: [ dotfiles-test-support]`. It is the sole guard on that invariant. The Rust version must compare the manifest's `members` against the Dockerfile's COPY lines, with a positive control proving both lists are non-empty.

**The no-read-write-HOME-mount check.** This is the one assertion in the repo that keeps the container from being a way to mutate the developer's home directory. `agent-sandboxing`'s lens applies: this is a boundary assertion, so weakening it is a security regression, not a test regression.

- [ ] **Step 3: Apply the two Dockerfile-reading lessons**

Both were vacuous-assertion defects found in Tranche A, and this suite reads Dockerfiles more than any other:

- **Join backslash continuations** before matching. `RUN apt-get update \` with the package list on the next line defeated `^RUN (apt-get|pacman)` for the life of `bootstrap-harness`.
- **Strip comments** before matching. A whole-file grep for a tool name was satisfied by the comment naming it, so deleting the real install line left the assertion green.

Write a single Dockerfile reader that does both, and doc-comment it with those two reasons. Then assert **exactly one** matching instruction per subject where that is the contract: `sed -n` concatenated two `ENV DOTFILES_ROOT=` lines into an ambiguous value in the shell version.

- [ ] **Step 4: Write the failing tests, verify failure, implement, verify pass**

Run from `~/crates`: `cargo test -p config-cli --locked --test container_image`

- [ ] **Step 5: Sabotage by group, with two mandatory cases**

Beyond the per-group sabotage, these two specifically:

- Add a fake member to `crates/Cargo.toml`'s `members` list without touching the Dockerfile. The workspace-members test must go red naming it. Restore.
- Change the runner to bind-mount `$HOME` read-write. The mount test must go red. Restore.

If either stays green, the conversion has lost the assertion that justified excluding this suite in the first place. Report both explicitly.

- [ ] **Step 6: Delete the shell suite and run every gate**

```bash
cd ~
config rm tests/container.test.sh
config add crates/config-cli/tests/container_image.rs
```

Then `bash tests/rust-gate.test.sh`, `bash tests/run-all.sh -q`, commit, then `tests/rust-checks.sh`.

- [ ] **Step 7: Confirm the container leg still checks its own image**

The circularity worth naming: this suite asserts about the image that the container leg runs inside. After conversion the assertions live in a Rust test that runs in that same container.

Verify the Rust test actually executes there rather than skipping, by pushing (or by running `tests/run-in-docker.sh` directly) and reading the skip list. If it skips in the container, the image's own shape is now unchecked in the only place it matters, and that is a finding to report rather than accept.

---

## Self-Review

**1. Spec coverage.** The spec's exclusion note names exactly these two suites and defers them to "after step 4". Task 1 tests whether that condition is met instead of assuming it, which is the part a plan usually gets wrong. Both suites have a conversion task. The spec's step 4 itself (deleting `lib.sh` and `run-all.sh`) is not in this plan: it is gated on "both suites running green side by side across several pushes" and belongs with Tranche C, which owns the harness.

**2. Placeholder scan.** No TBD. Tasks 2 and 3 say "inventory all N assertions" rather than listing them, deliberately: the inventory is the first deliverable and duplicating 63 assertions here would drift from the files. Every task names what to do when the inventory surprises the executor, and Task 2 Step 2 names a specific unknown (how to test for a repository when the git dir is `~/.cfg`) rather than asserting an answer I have not verified.

**3. Type consistency.** `dotfiles_test_support::skip` and `repo::root` match Tranche A's signatures. No new helpers are promoted, because each suite reads a different kind of file and a shared abstraction over "read a Dockerfile" and "read a manifest" would be an abstraction over two things.

**One risk this plan does not eliminate.** Task 3 converts the suite that checks the image the tests run in. If the conversion is wrong in a way that makes the test skip rather than fail inside the container, the failure is invisible: the gate reports a pass and the image's shape goes unchecked. Step 7 is the mitigation and it is a manual verification, not an automated one. A stronger design would have a second suite assert that this one ran, and that is deliberately not proposed here, because a watcher-of-the-watcher is the kind of structure that gets deleted as redundant in six months.

**A second risk, stated because it is the reason for Task 1.** Both plans that depended on these gates are marked SHIPPED, which is why conversion is now reasonable. But "SHIPPED" is a banner I wrote on 2026-09-10 from a verification pass, not a guarantee that no future step will want these gates. If a later spec re-introduces a dependency on either suite, it will find a Rust test rather than a shell one, which is fine, but it will not find the file path its document names. That is a documentation cost, and the mitigation is that each conversion commit names the shell file it replaced.
