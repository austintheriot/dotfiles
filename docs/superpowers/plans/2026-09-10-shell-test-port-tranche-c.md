# Shell Test Port, Tranche C Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the remaining 32 shell suites to Rust, then delete `tests/lib.sh` and `tests/run-all.sh`, leaving one harness.

**Architecture:** `assert_cmd` plus `tempfile` replaces the `lib.sh` harness. Each suite becomes an integration test under `crates/config-cli/tests/`. Two suites are special because their subject is the harness itself and disappears with it, and one group needs the tmux fixture from spec 4b.2.

**Tech Stack:** Rust 2024, `dotfiles-test-support` (`skip`, `repo`, `zsh` if Tranche B landed), `assert_cmd`, `tempfile`. `assert_cmd` is the one new dependency and it arrives in Task 2.

**Spec:** `docs/superpowers/specs/2026-09-07-shell-test-port-design.md`, Tranche C at section 3, the tmux fixture at 4b.2, the harness deletion at section 5 step 4. Tranche A is done (`fd3b4435`). Tranche B and the two gate suites have their own plans and **should land before this one's Task 6**.

## Read this before starting: the honest case for this tranche

The spec is unusually candid here, and executing this plan without knowing that is a mistake:

> **One claim withdrawn from the parent spec's first draft, recorded so it is not used as justification again.** It said `tempfile` "fixes the fixture-ownership defect where cleanup kills tmux sessions by name pattern on a shared server." The pattern-kill at `lib.sh:87-89` exists but is **PID-scoped** ... So that defect is unreachable, and **tranche C rests on consistency alone.**

And: "The payoff is thin per suite, so this goes last and incrementally."

So this tranche has no defect class behind it, unlike Tranche A, which had a reproduced vacuous-assertion bug and found four more during execution. What it has is a decision, made 2026-09-10: one suite should not live in two harnesses, and one repo should not run two test runners. That is a legitimate reason and it is the only one. **Do not write commit messages claiming this tranche fixes bugs.** If a conversion happens to find one, as Tranche A's did four times, report it as a finding rather than as the tranche's justification.

The corollary: this plan is explicitly incremental and safe to stop between tasks. Ending after Task 3 with 20 suites converted and both harnesses alive is a valid state. Only Task 6 is irreversible.

## Progress, 2026-09-10

**Task 7 done** (`3c34a7ce`): `nvim-mason-runtimes`, 59 executed assertions
against this plan's stated 53. Fourth count error of the day.

**Task 1: 8 of 11 done.** `2dc4c534` doc-links, `ec1f785b`
workflow-shell-quoting, `5b19fcbb` githooks-installed, `89463860`
profile-path, `34a14f21` nvim-version-floor, `12b1046a` nvim-lua-format,
`6f5d1e81` platform, `17401615` rust-gate. Remaining: `deps-manifest`,
`alacritty-platform-split`, `deps-docs`.

**`python3-yaml` is gone** from `tests/docker/Dockerfile`, removed in
`17401615` with `rust-gate`, the second of its two importers.

**`assert_cmd` is a dev-dependency** (`dfa08f35`), so Task 2 needs only its
conversion.

### Three defects the conversions found

Reported as findings, not as this tranche's justification, per the honest
case above.

1. **`nvim-lua-format.test.sh` could not fail.** No `finish` call: its last
   statement was an assertion inside an `if`, and `finish` is what returns
   the exit status. An unformatted Lua file printed `FAIL:` and exited **0**,
   so the stylua gate had been open since the suite was written. The port
   exits 101 on the same sabotage.
2. **`nvim-version-floor.test.sh` had a vacuous assertion.** Its live
   simulation asserted the output contained `0.10`. With the guard disabled
   so startup fell through, it still passed: the run reached lazy.nvim and
   printed `markdown-preview.nvim v0.0.10`, while the guard's own message
   appeared zero times.
3. **`profile-path.test.sh` could not detect a bashism.** `[[ ]]` passes both
   `sh -n` and `dash -n`, because dash parses `[[` as a command word. A parse
   check is not an execution check.

All three are now in `.claude/rules/dotfiles-tests.md`.

### Two process notes for the remaining tasks

- **`config status -uall` walks the whole home directory** and held the index
  for over two minutes here, causing an `index.lock` collision. Do not use it.
- **`config commit` without a pathspec commits everything staged**, including
  another agent's in-flight work. One agent swept a sibling's staged files
  into a commit and recovered with `config reset --soft`. Always commit
  path-scoped when any other agent is running.

## Global Constraints

Every one learned by executing Tranche A. Every task's requirements implicitly include this section.

- **All Rust invocations run from inside `crates/`, never with `--manifest-path`.**
- **`cargo test --locked --quiet` stays the gate command.** Do not add `--nocapture`.
- **`#[ignore]` must not be used for a runtime skip.** Use `dotfiles_test_support::skip(reason)` and return.
- **A test must never call `skip()` to prove skipping works.** It reads the ambient `DOTFILES_SKIP_LOG` the gate sets, so such a test records a phantom skip. This bit Tranche A in the mechanism's own first test.
- **Positive controls are required.** Any assertion expecting an empty result must first prove its pipeline produced something.
- **Locate tracked files with `dotfiles_test_support::repo::root()`**, never `CARGO_MANIFEST_DIR` alone.
- **`git ls-tree` from a test needs a rooted pathspec (`:/deps`) and `--full-name`.** A bare relative pathspec resolves against `crates/config-cli`.
- **Strip comments and join backslash continuations before matching file contents.** Both defects made assertions vacuous in Tranche A: a grep satisfied by a comment, and `^RUN (apt-get|pacman)` that could not see a package list on a continuation line.
- **Every test file opens with a `//!` module doc.**
- **`crates/Cargo.lock` goes in every commit that touches a manifest.** Use `cargo update -w`, dropping `--offline` when a crate is not cached.
- **Adding a workspace member requires five edits to `tests/docker/Dockerfile`.** This plan adds none.
- **`tests/rust-checks.sh` archives from a git ref**, so run it AFTER committing.
- **One suite per commit**, deleting its `.test.sh` in the same commit.
- **The repo is public.** No employer or product names in tracked files.
- **Never `--no-verify`.** Never disable a test instead of fixing it.

## The 32 suites, measured 2026-09-10

**757 assertions across 32 suites**, counted as non-comment lines containing
`assert_`. Two cautions about that number, both from getting it wrong while
writing this plan: `grep -c` counts matching *lines* while a per-occurrence
count is higher, and a first pass omitted a suite entirely. If your total
disagrees with this one, recount before assuming the plan is right.

Grouped by what the conversion needs, because that determines order.

**Group 1, plain file and text reads (11 suites).** Nothing but
`repo::root()` and file reads. `doc-links`, `workflow-shell-quoting`,
`githooks-installed`, `profile-path`, `nvim-version-floor`,
`nvim-lua-format`, `platform`, `rust-gate`, `deps-manifest`,
`alacritty-platform-split`, `deps-docs`.

> **CORRECTED 2026-09-10 during execution: `python-interpreter` moved to
> Group 5.** It was filed here, and it does not belong: every one of its
> assertions is about `PYTHON_BIN`, which after `rust-gate` converts exists
> only in `tests/lib.sh`, `tests/run-all.sh`, and the suite itself. Its
> subject is the harness Task 6 deletes, which makes it Task 5's
> classification problem, not a conversion. The executing agent stopped
> rather than improvising a classification this plan reserves for Task 5,
> which was the right call.

**Group 2, subprocess drivers (7 suites, 207 assertions).** Need `assert_cmd`
to drive a binary or script and assert on exit codes and output. `notify`,
`git-post-checkout-hook`, `depcheck-hook`, `pre-push-multi-ref`,
`config-manifest-lifecycle`, `config-usage`, `config-init`.

**Group 3, tmux (7 suites, 96 assertions).** Need the fixture from spec 4b.2.
`tmux-name-batching`, `tmux-close`, `tmux-plugin-path`, `tmux-conf-split`,
`tmux-start`, `tmux-split`, `tmux-update-window-names`.

**Group 4, the big three (3 suites, 201 assertions).** `config`,
`leak-check`, `setup`. Each is a subsystem's whole contract and each gets its
own commit.

**Group 5, the harness's own tests (3 suites).** `skip-reporting`,
`run-all-filter`, and `python-interpreter` (moved here during execution).
**These are the ordering hazard.** See Task 5.

**Group 6, nvim mason runtimes (1 suite, 53 assertions).** `nvim-mason-runtimes`,
which a first draft of this plan **omitted entirely** and which is the
single largest suite outside Group 4. It is its own group because its subject
is neither a file read nor a subprocess it owns: it asserts that the mason
runtimes are declared, that the health check names them, and that the
install remedy is in the message. It also now carries the headless-gate
assertion added on 2026-09-10, which reads the installed plugin tree and
skips where absent. See Task 7.

Per-suite assertion counts are deliberately not listed here. The first draft
listed them and four of the five group totals were wrong; the counts belong
in each task's inventory step, read from the file at conversion time.

---

### Task 1: Group 1, the twelve plain readers

Cheapest first, so the pattern is established before anything hard. Twelve commits.

**Files, per suite:** create `crates/config-cli/tests/<name>.rs`, delete `tests/<name>.test.sh`.

**Interfaces:**
- Consumes: `dotfiles_test_support::{skip, repo}`.
- Produces: nothing. Do not promote a shared helper from this group until a third caller wants it; two callers is the threshold and Tranche A's promotion already covers YAML.

- [ ] **Step 1: Convert in ascending assertion order**

`doc-links`, `workflow-shell-quoting`, `githooks-installed`, `profile-path`, `nvim-version-floor`, `python-interpreter`, `nvim-lua-format`, `platform`, `rust-gate`, `deps-manifest`, `alacritty-platform-split`, `deps-docs`.

For each: inventory every assertion as one sentence, write the tests, run and observe failure, implement, run and observe pass, sabotage each assertion group, `config rm` the shell file, commit. Report the inventory count per suite.

- [ ] **Step 2: Watch for three specific traps in this group**

- **`rust-gate.test.sh` and `workflow-shell-quoting.test.sh` import the python yaml module.** Converting them is what finally lets `python3-yaml` leave `tests/docker/Dockerfile`. That comment was corrected in `fd3b4435` to name these two as its remaining justification, so when the second of them converts, remove the package and its comment in the same commit. Check the comment before assuming.
- **`nvim-lua-format.test.sh` and any suite that shells to a linter** need `dotfiles_test_support::skip` when the tool is absent, not a failure. An absent tool is a skip; an absent tracked file is a failure.
- **`githooks-installed.test.sh` skips entirely where there is no `.cfg` repository**, which is the container. Preserve that skip and prove it fires, the same way the gate-suites plan requires for `scripts-dir-name`.

- [ ] **Step 3: Report the group's total and stop for review**

After twelve commits, run `bash tests/run-all.sh -q` and `tests/rust-checks.sh`. Report the remaining shell-suite count (expected 20) and the skip list. This is a natural stopping point.

---

### Task 2: `assert_cmd` arrives, and Group 2's smallest suite

The one new dependency, introduced with the smallest suite that needs it so the pattern is proven cheaply.

**Files:**
- Modify: `crates/config-cli/Cargo.toml` (add `assert_cmd` to `[dev-dependencies]`), `crates/Cargo.lock`
- Create: `crates/config-cli/tests/git_post_checkout_hook.rs`
- Delete: `tests/git-post-checkout-hook.test.sh`

**Interfaces:**
- Produces: the `assert_cmd` usage pattern the rest of Group 2 copies.

- [ ] **Step 1: Inventory the 11 assertions of `git-post-checkout-hook.test.sh`**

One sentence each. This suite stubs `tmux-tools` on PATH and asserts the hook invokes it with `-a`, from a worktree too, that install is idempotent, and that a non-repo is refused with exit 2.

- [ ] **Step 2: Write the failing tests**

Use `assert_cmd::Command` for the hook invocation and `tempfile` for the fixture repository. The PATH stub is the interesting part: the test needs a directory containing a fake `tmux-tools` that records its arguments, prepended to `PATH` for the child only.

- [ ] **Step 3: Declare the dependency**

```toml
assert_cmd = "2"
```

Then `cargo update -w` and add `crates/Cargo.lock` to the commit.

- [ ] **Step 4: Verify failure, implement, verify pass, sabotage, commit**

Sabotage: make the hook not call `tmux-tools`; drop the `-a` flag; make install non-idempotent; make a non-repo exit 0. Each should redden its own test.

---

### Task 3: The rest of Group 2 (six suites, 205 assertions)

Ascending order: `notify` (13), `pre-push-multi-ref` (17), `depcheck-hook` (21), `config-manifest-lifecycle` (33), `config-init` (56), `config-usage` (57). Six commits.

- [ ] **Step 1: Convert each, following Task 2's pattern**

Per suite: inventory, red, implement, green, sabotage by group, delete, commit.

- [ ] **Step 2: Two suites need specific care**

- **`config-usage.test.sh` pins `config help` output byte-for-byte** against `tests/fixtures/config-help-before-describe.txt`. The subcommand-ports spec calls that fixture "the acceptance test" for the help port. Carry the byte-for-byte comparison over exactly; do not soften it to a `contains`. And note that this suite exercises odd-shaped `config-*` scripts from a fixture directory via `CONFIG_SUBCOMMAND_DIR`, which is why it can test them without leaving a stray script in the real `.scripts/config` that every other suite counts.
- **`config-init.test.sh` (56 assertions) drives a real init against a fixture `$HOME`.** That is the most stateful suite in this group. Verify it cannot touch the developer's real home: Tranche A's precedent is spec 4a.1's verification, which ran a suite with `HOME` pointed at a throwaway directory and confirmed "the real home directory unchanged at 111 entries, tmux session count unchanged, and the fake home completely empty afterwards. Zero mutation." Do the same and report the same evidence.

- [ ] **Step 3: Report and stop for review**

Expected remaining shell suites: 14.

---

### Task 4: Group 3, the seven tmux suites (124 assertions)

These need the fixture spec 4b.2 decided: promote the `fn tmux` helper that already exists, duplicated, in `crates/tmux-tools/tests/name_windows.rs` and `split.rs`.

**Files:**
- Create: `crates/tmux-tools/tests/common/mod.rs` or a support module (see Step 1)
- Create: seven test files
- Delete: seven `.test.sh` files

- [ ] **Step 1: Promote the tmux fixture, per spec 4b.2**

De-duplicate the existing `fn tmux(socket_dir, socket, arguments)` into one place. Three trap guards become assertions rather than comments, each recorded in `.claude/rules/dotfiles-tests.md` as having cost a debugging cycle on 2026-09-09:

- Create the socket directory before the first tmux call, and assert at the end that the **shared** directory holds no socket of the suite's own. When `TMUX_TMPDIR` names a directory whose parent is missing, tmux falls back to the shared path and exits 0 silently.
- Build the directory directly under `/tmp` with `tempfile::Builder::new().prefix("tt-").tempdir_in("/tmp")`. A Unix socket path is capped at 104 bytes on macOS and a temp dir under `/var/folders/.../T/` overruns it.
- Kill the server in `Drop`, **through the same wrapper that set `TMUX_TMPDIR`**. A teardown that misses this aims at a path that no longer exists, leaves the real server running, and then removes its socket from under it.

Also: `isolate_hooks` becomes an explicit no-op hook at index `[0]`, not an empty string. Setting a hook to `''` leaves the inherited global array entry firing, fixed once already in `8591f242`.

- [ ] **Step 2: Convert in ascending order**

`tmux-name-batching` (6), `tmux-close` (8), `tmux-plugin-path` (8), `tmux-conf-split` (11), `tmux-start` (11), `tmux-split` (19), `tmux-update-window-names` (34).

- [ ] **Step 3: Know the one race already fixed here**

`tmux-update-window-names.test.sh` was fixed in `8591f242`: `.zshrc`'s precmd calls the script under test on every prompt, so a shell-backed window renamed **itself** a beat after creation and overwrote the name the assertion checked. The fix was `$INERT_WINDOW_CMD` instead of a shell. The Rust version must keep that property: test windows run an inert command, not a shell. A conversion that spawns shells in test windows re-introduces a race that took a debugging cycle to find and that presents as flakiness rather than failure.

- [ ] **Step 4: Sabotage, delete, commit per suite; then report**

Expected remaining shell suites: 7.

---

### Task 5: Group 5, the harness's own tests

**This is the task that needs a decision, not just execution.** `skip-reporting.test.sh` (43) and `run-all-filter.test.sh` (28) test `tests/lib.sh` and `tests/run-all.sh`, which Task 6 deletes. Their subject disappears.

- [ ] **Step 1: Classify every assertion in both suites**

For each of the 71 assertions, mark it:

- **About the shell harness only.** Dies with `lib.sh` and `run-all.sh`. Example shape: "`run-all.sh <suite>` filters to one suite."
- **About a property the Rust harness must also have.** Must be re-expressed against the Rust harness, not deleted. Example shape: "a skipped assertion is counted and reported", which is exactly what `dotfiles-test-support`'s skip mechanism and `rust-checks.sh`'s count now provide.

Report the split. This classification is the task's real deliverable.

- [ ] **Step 2: Understand what `skip-reporting` is guarding, from the rules file**

`.claude/rules/dotfiles-tests.md` records the incident: `scripts-dir-name.test.sh`'s commit-inspection block "used to print one line into a suite's captured output and nothing else, so `finish` counted only passes and failures and the pre-push Docker gate printed PASS over an assertion that never ran. A stale committed-script count shipped, and both CI platforms caught it instead." The rule it produced: "a gate that says nothing when it skips is indistinguishable from a gate that is not installed."

That rule now applies to **two** harnesses. Whatever survives from `skip-reporting` must assert it of the Rust one: that `dotfiles_test_support::skip` records, that `rust-checks.sh` reports the count, and that the count is not silently swallowed by `--quiet`. Tranche A verified those by hand; this task is where they become assertions.

- [ ] **Step 3: Write the Rust-harness-property tests**

A new `crates/dotfiles-test-support/tests/skip_reporting.rs`, asserting the properties from Step 1's second category. It can assert the mechanism directly (a skip writes a line, two skips write two lines, an unconfigured log is a no-op) and can assert `rust-checks.sh`'s reporting by running it with a seeded log.

- [ ] **Step 4: Convert `run-all-filter`'s surviving assertions, if any**

Its subject is `run-all.sh`'s filter. If the Rust harness has no equivalent (cargo's own `--test` and filter arguments do the job), then most of its assertions die with the file rather than converting. Say so explicitly per assertion; do not let one vanish unremarked.

- [ ] **Step 5: Commit, and do not delete either shell suite yet**

Both stay until Task 6, because until `lib.sh` exists they are still testing something real. Commit the new Rust tests alone.

---

### Task 6: Group 4 (the big three), then delete the shell harness

Last, and the only irreversible step.

- [ ] **Step 1: Convert `setup` (61), `leak-check` (64), `config` (76), one commit each**

Each is a subsystem contract. Two notes:

- **`leak-check.test.sh` has 64 assertions and guards a public repo's credential gate.** `agent-sandboxing`'s lens applies: these are boundary assertions and weakening one is a security regression. Its layer-2 project-term rules read patterns from an untracked file outside the repo precisely so the terms are not published; the Rust version must preserve that indirection and must not inline a single term.
- **`config.test.sh` is the 105-second suite**, because line 454 calls `config-build` and compiles six crates. Converting it does not fix that, and spec 2a says so. Do not claim it does. Whether the Rust version should still build is a separate question this plan does not answer: report the runtime you observe.

- [ ] **Step 2: Confirm the spec's precondition for deletion is met**

The spec's section 5 step 4: delete `lib.sh` and `run-all.sh` "only after both suites have run green side by side across several pushes." Report the evidence: how many pushes have run both harnesses green. If the answer is fewer than several, stop here. This is the one step where waiting is the correct action and impatience is unrecoverable.

- [ ] **Step 3: Delete `tests/lib.sh`, `tests/run-all.sh`, and the two Group 5 shell suites**

Plus every reference: `tests/pre-push`'s invocation, `tests/run-in-docker.sh`, `.github/workflows/test-suite.yml`, `config test`'s implementation, and `.claude/rules/dotfiles-tests.md`, which describes `run-all.sh` throughout and needs rewriting rather than patching.

- [ ] **Step 4: Verify the gates still gate**

The critical check, and the reason this step is last: after deletion, `tests/pre-push` must still block a push on a failing Rust test and on a leak. Prove both by trying:

- Break a Rust test, attempt a push, confirm refusal, restore.
- Stage a fake credential, attempt a commit, confirm refusal, restore.

A deletion that leaves the hooks calling a missing script fails open, which is this repo's named dominant bug class ("the environment compensating for a gap the engine has").

- [ ] **Step 5: Update the rules file and commit**

`.claude/rules/dotfiles-tests.md` is written around the shell harness: "Run `~/tests/run-all.sh`", the 374-second budget, the skip-reporting section, the `lib.sh` helper guidance. All of it needs rewriting for one harness. That is a documentation deliverable, not an afterthought, and it is what a future reader will trust.

---

### Task 7: Group 6, `nvim-mason-runtimes` (53 assertions)

Omitted from this plan's first draft. Placed after Task 4 because it shares
nothing with the other groups and can run whenever, but before Task 6
because it must not be the thing still outstanding when the harness is
deleted.

**Files:**
- Create: `crates/config-cli/tests/nvim_mason_runtimes.rs`
- Delete: `tests/nvim-mason-runtimes.test.sh`

- [ ] **Step 1: Inventory all 53 assertions**

One sentence each. The groups from a first read: the runtimes are declared as
dependencies; the health check names each runtime; the health check names the
install remedy (`nvm install`, which is not guessable because node comes from
nvm rather than a package manager); `unzip` is tracked so zip-backed mason
tools can install; and the mason-lspconfig headless gate still exists.

- [ ] **Step 2: Preserve the headless-gate assertion exactly**

Added 2026-09-10 in the same commit as the rules-file record. It reads
`mason-lspconfig/lua/mason-lspconfig/init.lua` from the **installed plugin
tree**, which the container does not carry, and skips there with a stated
reason. Its purpose is inverted from a normal assertion: it asserts the gate
still EXISTS, so that when a mason bump removes it, the advice in
`.claude/rules/dotfiles-tests.md` fails loudly instead of quietly becoming
wrong. Carry that intent into the doc comment, or a later reader will "fix"
it into a normal presence check.

- [ ] **Step 3: Red, implement, green, sabotage, delete, commit**

Sabotage: drop a runtime from the manifest; remove a runtime from the health
check; delete the `nvm install` remedy text; untrack `unzip`; change the
headless-gate text. Each should redden its own test.

---

## Self-Review

**1. Spec coverage.** Tranche C is "the remainder", and the 32 suites here are exactly the 41 remaining minus Tranche B's 7 and the 2 gate suites. Verified by set arithmetic rather than by reading, which is how the omission of `nvim-mason-runtimes` was caught: a first draft grouped 31 of 32 and the missing one was the largest suite outside Group 4. Section 5 step 4's harness deletion is Task 6. Spec 4b.2's tmux fixture is Task 4 Step 1. The withdrawn justification is quoted at the top so it cannot be reused as a rationale.

**2. Placeholder scan.** No TBD. Tasks 1, 3, 4 and 6 say "inventory every assertion" rather than listing 715 of them, which is the deliverable rather than a gap. Task 5 has no code samples because its deliverable is a classification, and pre-writing the tests would presuppose the answer.

**3. Type consistency.** `dotfiles_test_support::skip`, `repo::root` and the `zsh` module keep Tranche A's and B's signatures. `assert_cmd` arrives once, in Task 2, and Task 3 copies that pattern rather than redefining it.

**Three risks this plan does not eliminate.**

**The tranche has no defect class behind it.** Every other tranche fixed something measurable. This one is consistency, by the spec's own admission, so its cost-benefit is worse and its value is real but unmeasurable. Stopping early is explicitly fine, and the plan is ordered cheapest-first so that early stopping keeps the best ratio.

**Task 6 is irreversible and its precondition is a judgment call.** "Several pushes" is not a number. An executor eager to finish will read two as several. The plan says to report the evidence and stop if unsure, which is a discipline rather than a gate, and it is the weakest link.

**Converting `leak-check` and `config` moves 140 assertions guarding a public repo's credential gate and its dispatcher.** Tranche A found four vacuous assertions while converting far less security-relevant suites. The same care applies here, and the risk runs both ways: a conversion may find a vacuous assertion, or may introduce one. The sabotage step is the only defense and it must not be skipped under time pressure on precisely these two suites.
