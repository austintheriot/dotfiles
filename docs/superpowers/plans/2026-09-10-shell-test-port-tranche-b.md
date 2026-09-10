# Shell Test Port, Tranche B Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the seven suites whose subject is the shell itself onto a Rust harness, so Rust drives them while `.zshrc` and the ZLE widgets stay the thing under test.

**Architecture:** Each suite becomes an integration test under `crates/config-cli/tests/` that spawns a real `zsh` through `std::process::Command` and asserts on its output. The shell is the subject, so it is not translated: the tests keep invoking it and only the harness changes. One suite, `zshrc-platform-split`, has its contracts **re-derived** rather than ported, because the 2026-09-06 branch collapse made two of them vacuous.

**Tech Stack:** Rust 2024, `dotfiles-test-support` (`skip`, `repo::root`), `tempfile` for fixture `$HOME`s, `std::process::Command` for `zsh -f` and `zsh -i`. No new crates.

**Spec:** `docs/superpowers/specs/2026-09-07-shell-test-port-design.md`, Tranche B at section 3, the skip mechanism at 4b.1. Tranche A is done (`fd3b4435`). Tranche C and the two excluded gate suites have their own plans.

## Global Constraints

Copied from Tranche A's plan, where every one of these was learned by executing it. Every task's requirements implicitly include this section.

- **All Rust invocations run from inside `crates/`, never with `--manifest-path`.** Spec 4a.1: "That exact mistake has failed CI once already."
- **`cargo test --locked --quiet` stays the gate command.** Do not add `--nocapture`.
- **`#[ignore]` must not be used for a runtime skip.** Use `dotfiles_test_support::skip(reason)` and return.
- **A test must never call `skip()` to prove skipping works.** `skip()` reads the ambient `DOTFILES_SKIP_LOG` the gate sets, so such a test records a phantom skip.
- **Positive controls are required.** Any assertion expecting an empty result must first prove its pipeline produced something.
- **Locate tracked files with `dotfiles_test_support::repo::root()`**, never `CARGO_MANIFEST_DIR` alone. That is a compile-time constant and reads the wrong root under the gate, which cost two rounds in Tranche A.
- **`crates/Cargo.lock` goes in every commit that touches a manifest**, or `cargo test --locked` fails. Use `cargo update -w` (drop `--offline` if a crate is not cached).
- **Every test file opens with a `//!` module doc**, or `missing_docs` plus clippy `-D warnings` fails the gate.
- **Adding a workspace member requires five edits to `tests/docker/Dockerfile`** (COPY manifest, mkdir src, stub lib.rs, rm -rf, COPY src). `tests/container.test.sh` enforces it. This plan adds no member.
- **`tests/rust-checks.sh` archives from a git ref**, so run it AFTER committing, not before.
- **One suite per commit.** A reviewer must be able to reject one conversion while accepting its neighbour.
- **Delete each `.test.sh` in the same commit as its Rust replacement**, so no assertion is owned twice.
- **The repo is public.** No employer or product names in tracked files.
- **Never `--no-verify`.** Never disable a test instead of fixing it.

## The seven suites, measured

| Suite | Assertions | Shape |
|---|---|---|
| `zshrc-git-aliases` | 20 | Spawns zsh, asserts alias expansions |
| `zshrc-platform-split` | 15 | **Re-derivation, not a port.** See Task 2 |
| `zshrc-node-startup` | 14 | Startup path resolution, PATH ordering |
| `zshrc-python-startup` | 12 | Startup path resolution, pyenv shim |
| `zsh-git-widgets` | 5 | ZLE widgets; must run in-process |
| `zshrc-alias-isolation` | 2 | Alias namespace |
| `zshrc-startup-budget` | 2 | Wall-clock budget |

70 assertions total. The order below is easiest-first so the zsh-spawning helper settles before the hard cases.

---

### Task 1: The zsh fixture helper

Every suite in this tranche spawns zsh. Building that once beats seven copies, and the two-caller rule is satisfied immediately.

**Files:**
- Modify: `crates/dotfiles-test-support/src/lib.rs` (add `pub mod zsh`)
- Create: `crates/dotfiles-test-support/tests/zsh.rs`

**Interfaces:**
- Consumes: `repo::root` from Tranche A.
- Produces: `dotfiles_test_support::zsh::available() -> bool`,
  `zsh::run(script: &str) -> Output`, and
  `zsh::run_in_home(home: &Path, script: &str) -> Output`.

**CORRECTED 2026-09-10 during execution. There is no `run`.** The first
draft specified one as "a login shell in the repo itself, so the tracked
`.zshrc` loads". **Zsh does not source `.zshrc` for a non-interactive login
shell**, only for an interactive one. Measured:

```
env -i HOME=$HOME PATH=/usr/bin:/bin TERM=xterm zsh -l -c 'whence -w parse_git_dirty'
  -> absent            # .zshrc NOT loaded

env -i HOME=$HOME PATH=/usr/bin:/bin TERM=xterm zsh -i -c 'whence -w parse_git_dirty'
  -> function          # .zshrc loaded
```

So nothing in this tranche can use a non-interactive login shell, because
nothing in `.zshrc` runs in one. Both entry points are `-l -i -c`, matching
`tests/zshrc-node-startup.test.sh:193`, which already spells both flags.

**And both must isolate the environment.** The first draft passed the
developer's environment through, so `DOTFILES_PLATFORM=mac` and `NVM_DIR`
read as set purely by inheritance and any assertion of the form "`.zshrc`
produced X" could be satisfied by the caller. Three shell suites already
guard against this deliberately: `zshrc-node-startup.test.sh:189` says
"`env -i` is load-bearing" in its own comment, `zshrc-platform-split.test.sh:96`
uses `env -i`, and `zshrc-python-startup.test.sh:110` pins `STARTUP_PATH`.
Clear the environment and pass only `HOME`, `PATH` and `TERM`.

- [ ] **Step 1: Write the failing test**

`crates/dotfiles-test-support/tests/zsh.rs`:

```rust
//! The zsh fixture the Tranche B suites share.
//!
//! Tranche B's subject is the shell, so these tests spawn a real zsh rather
//! than reimplementing its behaviour. The helper exists so seven suites do
//! not each carry their own `Command` construction.

/// The fixture loads the repo's `.zshrc`. This is the property every suite
/// in the tranche depends on: if the fixture does not load the config, every
/// assertion about the config passes vacuously.
///
/// **The probe must be something only `.zshrc` defines.** An earlier draft
/// used `$ZSH_VERSION`, which is a zsh BUILT-IN parameter: `zsh -f -c 'echo
/// $ZSH_VERSION'` prints 5.9 with every startup file skipped, so that
/// assertion passed with the config entirely unloaded. The plan's own
/// sabotage step caught it. `parse_git_dirty` (`.zshrc:243`) is a function
/// the config defines and nothing else does.
#[test]
fn a_login_shell_loads_the_repo_zshrc() {
    if !dotfiles_test_support::zsh::available() {
        // Not a skip call: this test proves the fixture works, and a skip
        // here would hide the fixture being broken. See the Global
        // Constraints on why a test must not call skip() to prove skipping.
        eprintln!("zsh absent; the fixture cannot be exercised");
        return;
    }
    let output = dotfiles_test_support::zsh::run("whence -w parse_git_dirty");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("function"),
        "the fixture did not load .zshrc: parse_git_dirty is not defined. \
         Got {text:?}"
    );
}

/// An interactive shell is where aliases and ZLE widgets exist. A
/// non-interactive shell defines neither, so a suite that used the wrong one
/// would assert over an empty namespace and pass.
#[test]
fn an_interactive_shell_defines_aliases() {
    if !dotfiles_test_support::zsh::available() {
        eprintln!("zsh absent; the fixture cannot be exercised");
        return;
    }
    // `.zshrc-mac:46` and `.zshrc-linux:37` each echo a banner line, so
    // stdout is "Loaded mac configuration\n      11\n". Parse the LAST line,
    // and panic on an unparseable count rather than defaulting to zero: an
    // earlier draft used `.unwrap_or(0)` and reported "no aliases at all"
    // for a shell that had defined eleven.
    let output = dotfiles_test_support::zsh::run("alias | wc -l");
    let text = String::from_utf8_lossy(&output.stdout);
    let last = text.lines().last().unwrap_or_default().trim();
    let count: usize = last
        .parse()
        .unwrap_or_else(|_| panic!("expected a count, got {text:?}"));
    assert!(count > 0, "an interactive shell defined no aliases at all");
}

/// A fixture home isolates the shell from the developer's real config, which
/// is what lets a test assert "this variant is NOT loaded" without depending
/// on the machine it runs on.
#[test]
fn a_fixture_home_replaces_the_real_one() {
    if !dotfiles_test_support::zsh::available() {
        eprintln!("zsh absent; the fixture cannot be exercised");
        return;
    }
    let home = tempfile::Builder::new()
        .prefix("zsh-home-")
        .tempdir_in("/tmp")
        .expect("a temp home");
    let output = dotfiles_test_support::zsh::run_in_home(home.path(), "echo $HOME");
    let text = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        text.trim(),
        home.path().to_string_lossy(),
        "the fixture home did not take effect"
    );
}
```

- [ ] **Step 2: Run it to verify it fails**

Run from `~/crates`: `cargo test -p dotfiles-test-support --locked --test zsh`

Expected: FAIL to compile with `could not find 'zsh' in 'dotfiles_test_support'`.

- [ ] **Step 3: Implement the module**

Add to `crates/dotfiles-test-support/src/lib.rs`, alongside `pub mod repo`:

```rust
/// Spawning the shell that Tranche B's suites take as their subject.
///
/// `expect` is allowed here for the same reason `repo` allows it: a helper
/// whose only caller is a test with no recovery path should panic with a
/// stated reason rather than thread a `Result` no one can act on.
#[expect(
    clippy::expect_used,
    reason = "a broken fixture is a panic, not a recoverable error"
)]
pub mod zsh {
    use std::path::Path;
    use std::process::{Command, Output};

    /// Whether a zsh is on PATH at all.
    #[must_use]
    pub fn available() -> bool {
        Command::new("zsh")
            .arg("-c")
            .arg("exit 0")
            .output()
            .is_ok_and(|output| output.status.success())
    }

    fn run(arguments: &[&str], home: &Path, script: &str) -> Output {
        Command::new("zsh")
            .args(arguments)
            .arg(script)
            .env("HOME", home)
            .env_remove("ZDOTDIR")
            .output()
            .expect("zsh spawns")
    }

    /// A login shell in the repo itself, so the tracked `.zshrc` loads.
    #[must_use]
    pub fn run(script: &str) -> Output {
        run(&["-l", "-c"], &super::repo::root(), script)
    }

    /// An interactive shell, where aliases and ZLE widgets exist.
    #[must_use]
    pub fn run_interactive(script: &str) -> Output {
        run(&["-i", "-c"], &super::repo::root(), script)
    }

    /// An interactive shell with a fixture `$HOME`.
    #[must_use]
    pub fn run_in_home(home: &Path, script: &str) -> Output {
        run(&["-i", "-c"], home, script)
    }
}
```

- [ ] **Step 4: Run it to verify it passes**

Run from `~/crates`: `cargo test -p dotfiles-test-support --locked --test zsh`

Expected: PASS, 3 tests. Then `cargo clippy --locked --all-targets -- -D warnings`, expected clean.

- [ ] **Step 5: Sabotage the fixture**

Change `run` to pass `-c` without `-l`. Re-run.

Expected: `a_login_shell_loads_the_repo_zshrc` goes red, because a non-login shell does not source the config. Restore and confirm green. That test is the tranche's own positive control: if it can pass with the config unloaded, every suite built on it is vacuous.

- [ ] **Step 6: Commit**

```bash
cd ~
config add crates/dotfiles-test-support
config commit -F - <<'MSG'
tests: add the zsh fixture Tranche B's suites share

Tranche B's subject is the shell, so its suites spawn a real zsh rather
than reimplementing it. Three entry points, because the distinction
matters: run sources the tracked .zshrc, run_interactive is where
aliases and ZLE widgets exist at all, and run_in_home isolates from the
developer's real config so a test can assert a variant is NOT loaded.

The login test is the tranche's positive control. Sabotaged by dropping
-l: it goes red, because a non-login shell sources nothing, which is the
shape that would make every suite built on this fixture vacuous.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
```

---

### Task 2: Re-derive `zshrc-platform-split`, do not port it

This suite goes second, not last, because its contracts must be settled before anything else asserts about the platform variants. The spec is explicit that this one needs "**re-derivation, not a port**".

**The problem, measured.** `tests/zshrc-platform-split.test.sh:19` records contract 3 as "both variants exist here, so neither can drift unseen." That was a **cross-branch** guarantee: when `mac` and `linux` were separate branches, both variants shipping on both branches was what made drift visible. The 2026-09-06 collapse left one branch, and one branch cannot drift from itself, so the guarantee now holds trivially and the assertion tests a mechanism that no longer exists.

Assertions 37 and 38 (`the mac variant ships here`, `the linux variant ships here`) still check something real: the files exist. What is gone is the *reason* recorded for them. Re-derivation means writing the contract that is true now, not deleting the checks.

**Files:**
- Create: `crates/config-cli/tests/zshrc_platform_split.rs`
- Delete: `tests/zshrc-platform-split.test.sh`

**Interfaces:**
- Consumes: `zsh` and `repo` from Task 1 and Tranche A.
- Produces: nothing other tasks depend on.

- [ ] **Step 1: Inventory all 15 assertions and classify each**

Read `tests/zshrc-platform-split.test.sh` end to end. For each of the 15 assertions write one sentence saying what it claims, then mark it:

- **Still true, same reason.** Port as-is.
- **Still true, different reason.** Port, and rewrite the doc comment to the reason that holds today.
- **Vacuous.** State why, and what assertion replaces it.

Report the classification before writing any Rust. Do not skip this: the whole point of this task is that a mechanical port would preserve two claims that are no longer about anything.

- [ ] **Step 2: Write the re-derived contract as doc comments first**

The contract that holds on one branch, to be stated at the top of the Rust file:

1. The shared `.zshrc` exists and contains no platform-specific path.
2. Both variants (`.zshrc-mac`, `.zshrc-linux`) ship, so a checkout on either platform finds its own. **This is the re-derived reason:** not "so neither can drift unseen" (cross-branch, now vacuous) but "so one checkout serves both platforms", which is what the collapse actually bought.
3. The shared file selects its variant at runtime through the platform helper, rather than through a hardcoded source chain.
4. No hardcoded home directory appears in the shared file.
5. **New, replacing the vacuous drift claim:** the two variants do not both load. Assert with a fixture `$HOME` that exactly one variant is sourced for a given platform, which is the property runtime selection is supposed to deliver and which nothing currently checks.

- [ ] **Step 3: Write the failing tests**

One `#[test]` per contract item, each doc-commented with the sentence from Step 2. Contract 5 is the new one and needs the fixture:

```rust
/// Exactly one variant loads, not both.
///
/// This replaces the suite's old contract 3 ("both variants exist here, so
/// neither can drift unseen"), which was a cross-branch guarantee: two
/// branches each carrying both variants is what made drift visible. The
/// 2026-09-06 collapse left one branch, and one branch cannot drift from
/// itself, so that claim now holds trivially and checks nothing.
///
/// What runtime selection actually promises is this: on a given platform the
/// shared file loads that platform's variant and NOT the other one. Nothing
/// asserted that before.
#[test]
fn exactly_one_platform_variant_loads() {
    if !dotfiles_test_support::zsh::available() {
        dotfiles_test_support::skip("no zsh here, so variant selection cannot be observed");
        return;
    }
    let output = dotfiles_test_support::zsh::run(
        "print -l ${(k)functions} >/dev/null; echo ${DOTFILES_PLATFORM_VARIANT:-unset}",
    );
    let loaded = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_ne!(
        loaded, "unset",
        "positive control: the shared zshrc set no variant marker, so this \
         test cannot tell which variant loaded"
    );
    assert!(
        loaded == "mac" || loaded == "linux",
        "exactly one variant must load; got {loaded:?}"
    );
}
```

**CORRECTED 2026-09-10: a marker already exists, and no shell config needs
changing.** The first draft said the variant that loaded was unobservable and
instructed adding `DOTFILES_PLATFORM_VARIANT` to each variant file. That was
wrong on the second half: `.zshrc-mac:46` echoes `Loaded mac configuration`
and `.zshrc-linux:37` echoes `Loaded linux configuration`. Exactly one prints
per shell, on stdout, which is directly observable and is what the old shell
suite's fixture imitated.

So assert against existing behaviour. **Do not modify `.zshrc-mac` or
`.zshrc-linux`**: a test that changes its subject to become testable is a
worse trade than one that reads what is already there.

- [ ] **Step 4: Run to verify failure, then implement, then verify pass**

Run from `~/crates`: `cargo test -p config-cli --locked --test zshrc_platform_split`

Expected first: FAIL. Then PASS once the tests and any `.zshrc` marker are in place.

- [ ] **Step 5: Sabotage each contract**

Per contract item, break its subject and confirm that test alone goes red: delete `.zshrc-mac`; add a hardcoded `/Users/` path to the shared file; replace the platform-helper call with a hardcoded source chain; make both variants load. Restore after each and report which sabotage proved which test.

- [ ] **Step 6: Delete the shell suite and commit**

```bash
cd ~
config rm tests/zshrc-platform-split.test.sh
config add crates/config-cli/tests/zshrc_platform_split.rs
config commit -F - <<'MSG'
tests: re-derive the zshrc platform-split contract, not port it

The spec calls for re-derivation here, and the reason is at
tests/zshrc-platform-split.test.sh:19: contract 3 read "both variants
exist here, so neither can drift unseen". That was a cross-branch
guarantee. Two branches each carrying both variants is what made drift
visible; the 2026-09-06 collapse left one branch, and one branch cannot
drift from itself, so the claim now holds trivially and checks nothing.

The files-exist assertions survive with a re-derived reason: both
variants ship so one checkout serves both platforms, which is what the
collapse actually bought.

Replacing the vacuous claim: exactly one variant loads, not both. That is
what runtime selection promises and nothing asserted it before.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
```

---

### Task 3: The four startup and alias suites

`zshrc-alias-isolation` (2), `zshrc-startup-budget` (2), `zshrc-python-startup` (12), `zshrc-node-startup` (14). Ascending size, so the pattern settles on the two-assertion suites first.

**Files, per suite:**
- Create: `crates/config-cli/tests/<suite_name>.rs`
- Delete: `tests/<suite-name>.test.sh`

**Interfaces:**
- Consumes: `zsh` from Task 1.
- Produces: nothing.

- [ ] **Step 1: Convert `zshrc-alias-isolation` (2 assertions)**

Inventory both assertions, one sentence each. Write the tests, watch them fail, implement, watch them pass, sabotage each, delete the shell file, commit. Use `zsh::run_interactive`, because aliases do not exist in a non-interactive shell.

- [ ] **Step 2: Convert `zshrc-startup-budget` (2 assertions)**

This one asserts a wall-clock budget, so it needs care the others do not. Read what the shell suite actually measures before writing anything: if it times `zsh -i -c exit` against a threshold, the Rust version must measure the same thing the same way, and the threshold must be carried over verbatim rather than re-derived. A timing test that is stricter than its predecessor will flake on a loaded machine, and this repo has already been bitten by a timing-dependent test (`tmux-update-window-names`, fixed in `8591f242`).

Report the threshold you found and the one you used. If the budget is measured with `$EPOCHREALTIME`, note that it needs `zmodload zsh/datetime`.

- [ ] **Step 3: Convert `zshrc-python-startup` (12 assertions)**

Inventory all 12. Two known traps from this repo's history, both recorded in `.claude/rules/dotfiles-tests.md` or prior commits:

- The `python3` on PATH is a pyenv shim that re-execs, so a test asserting on resolution must know which layer it is asserting about.
- `tests/zshrc-python-startup.test.sh` sets `STARTUP_PATH='/usr/bin:/bin:/usr/sbin:/sbin'` for its two resolution assertions specifically. Carry that scoping over; a test that resolves against the developer's full PATH asserts about their machine, not the config.

- [ ] **Step 4: Convert `zshrc-node-startup` (14 assertions)**

Inventory all 14. Node arrives through nvm rather than a package manager, so the assertions are about PATH ordering and lazy initialisation. Sabotage by reordering PATH entries, not by removing node.

- [ ] **Step 5: Verify the tranche boundary**

Run from `~`: `ls tests/zshrc-*.test.sh 2>/dev/null | wc -l`

Expected: 1, only `zshrc-git-aliases.test.sh`, which is Task 4.

---

### Task 4: `zshrc-git-aliases` (20 assertions)

Largest of the tranche and last, so the fixture is proven before the biggest inventory.

**Files:**
- Create: `crates/config-cli/tests/zshrc_git_aliases.rs`
- Delete: `tests/zshrc-git-aliases.test.sh`

- [ ] **Step 1: Inventory all 20 assertions**

One sentence each, as doc comments. Group by subject (alias existence, alias expansion, alias not shadowing a real git subcommand) so the sabotage in Step 4 can be grouped the same way.

- [ ] **Step 2: Write the failing tests, then implement, then pass**

`zsh::run_interactive` throughout. A positive control per group: assert the alias namespace is non-empty before asserting any particular alias is absent.

- [ ] **Step 3: Run both cargo checks**

Run from `~/crates`: `cargo test -p config-cli --locked --test zshrc_git_aliases`, then `cargo clippy --locked --all-targets -- -D warnings`.

- [ ] **Step 4: Sabotage by group**

Remove an alias definition; redefine one to expand differently; add an alias shadowing a real git subcommand. Confirm the right group alone goes red per sabotage. Report which sabotage proved which group.

- [ ] **Step 5: Delete the shell suite, run all gates, commit**

```bash
cd ~
config rm tests/zshrc-git-aliases.test.sh
config add crates/config-cli/tests/zshrc_git_aliases.rs
```

Then run, in this order: `bash tests/rust-gate.test.sh`, `bash tests/run-all.sh -q`, commit, and finally `tests/rust-checks.sh` (after the commit, because it archives from a ref).

---

### Task 5: `zsh-git-widgets` (5 assertions)

Separated from Task 4 because ZLE widgets are the one thing in this tranche that **cannot** be tested by spawning a shell and reading stdout. A widget runs in-process inside the line editor.

**Files:**
- Create: `crates/config-cli/tests/zsh_git_widgets.rs`
- Delete: `tests/zsh-git-widgets.test.sh` (only if all five assertions carry over; see Step 2)

- [ ] **Step 1: Inventory all 5 assertions, and determine what is testable**

Read `tests/zsh-git-widgets.test.sh` and the spec's section 3 note: "a ZLE widget must run in-process." For each assertion, decide whether it asserts (a) the widget is *defined* and bound, which a spawned interactive shell can check with `zle -l` and `bindkey`, or (b) the widget's *behaviour* when invoked, which needs the line editor.

Report the split before writing Rust. This matters: the spec's tmux-and-zsh spec section 3.2 already decided the widget's logic belongs in a binary with the widget only assigning the result, so behaviour assertions may already be covered by `tmux-tools`' own tests.

- [ ] **Step 2: Convert what is testable, and stop if something is not**

If all five are definition-and-binding assertions, convert all five and delete the shell file.

If any assertion genuinely needs the line editor, **stop and report**. Do not weaken it into a definition check that would pass with the widget's body deleted, and do not delete the shell file while it owns an assertion Rust cannot hold. That is the one case in this tranche where a suite legitimately stays shell, and the spec's own framing ("shell stays the subject") anticipates it.

- [ ] **Step 3: Sabotage**

Unbind the widget; delete its definition; if behaviour is covered, change what it produces. Confirm the right test alone goes red.

- [ ] **Step 4: Commit, then run every gate**

`bash tests/rust-gate.test.sh`, `bash tests/run-all.sh -q`, commit, then `tests/rust-checks.sh`.

Report the final suite count. Expected: 41 minus however many suites this tranche converted, which is 7 if Task 5 converts fully and 6 if it correctly stops.

---

## Self-Review

**1. Spec coverage.** Tranche B names "the six `zshrc-*` suites plus `zsh-git-widgets.test.sh`" and the repo has seven `zshrc-*` files, one more than the spec's count: `zshrc-alias-isolation`, `zshrc-git-aliases`, `zshrc-node-startup`, `zshrc-platform-split`, `zshrc-python-startup`, `zshrc-startup-budget`. That is six, so the spec's count is right and my earlier reading of seven `zshrc-*` was wrong; with `zsh-git-widgets` the tranche is seven suites total. All seven have a task. The re-derivation the spec demands for `zshrc-platform-split` is Task 2 and is the only task that changes what is asserted rather than where.

**2. Placeholder scan.** No TBD, no "implement later". Tasks 3, 4 and 5 say "inventory all N assertions" rather than listing them, which is deliberate and different from a placeholder: the inventory is the task's first deliverable and listing 48 assertions here would duplicate a file the executor must read anyway. Every task states what to do when the inventory surprises the executor.

**3. Type consistency.** `zsh::available`, `run`, `run_interactive`, `run_in_home` keep the same signatures across Tasks 1 through 5. `dotfiles_test_support::skip` matches Tranche A's. `repo::root` is unchanged.

**One risk this plan does not eliminate.** Task 5 may find that a widget's behaviour cannot be asserted from Rust, in which case one suite stays shell and the tranche is 6 of 7. That is an acceptable outcome the spec anticipates, not a failure, but it means "Tranche B is done" may need an asterisk. The plan says to stop and report rather than to weaken the assertion, and that is the right trade even though it leaves the tranche visibly incomplete.

**A second risk, checked and cleared.** Every test here spawns a real zsh,
so the container leg matters. Verified 2026-09-10: `tests/docker/Dockerfile`
installs zsh and its own comment at line 84 calls tmux and zsh "the
load-bearing ones", naming `zsh-git-widgets.test.sh` as a reason. So the
tranche will run in the container rather than skipping, and `available()`
plus `skip` is a guard for an unprovisioned developer machine rather than
for the gate.

One consequence worth noting: that Dockerfile comment names
`zsh-git-widgets.test.sh` as why zsh is installed. If Task 5 converts that
suite, the comment becomes a stale justification pointing at a deleted file,
which is the exact defect corrected in `fd3b4435` for `python3-yaml`. Update
it in Task 5's commit.
