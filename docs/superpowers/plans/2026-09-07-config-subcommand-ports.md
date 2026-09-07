# `config-*` Subcommand Ports Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the three `config-*` scripts that can move into `config-cli`
subcommands, delete the one that is already a redundant shim, and close the
two-binary window by making `config-manifest` library-only.

**Architecture:** Each port is one atomic commit that removes a script and
installs its replacement together, because the `config` dispatcher falls
through to `git` for any unmatched verb. `config-help` keeps asking its
siblings rather than embedding a list. `config-build` and `config-stamp`
deliberately stay shell, on the same pre-toolchain and circularity reasoning
that already keeps `config-init` shell.

**Tech Stack:** Rust 2024 edition, toolchain pinned to 1.94.0 by
`crates/rust-toolchain.toml`. `clap` 4.6.6 with `derive`. POSIX sh for the
surviving scripts.

**Spec:** `docs/superpowers/specs/2026-09-07-config-subcommand-ports-design.md`.
Read sections 1, 2, 3.1, 3.2a, 3.3 and 5 before Task 1. Section 3.2's
decision (keep `config-build` shell) is confirmed rather than a preference,
and this plan implements it as decided.

**Depends on:** `2026-09-07-config-cli-adapter-design.md` and its plan
`2026-09-07-config-cli-adapter.md`. `config-cli` must exist and be reachable
through the dispatcher before a subcommand can move into it.

## Scope, measured

The spec's own correction is the number to work from. Nine `config-*` scripts
exist totalling 742 lines, but the parent spec already decided four of them
stay shell and section 3.2 keeps a fifth:

| Script | Lines | Disposition |
|---|---|---|
| `config-init` | 212 | Stays shell. Runs before a toolchain exists. |
| `config-stamp` | 168 | Stays shell. `config-build` calls it to decide what to compile, so a Rust owner is circular. |
| `config-install-hooks` | 61 | Stays shell. Runs before a toolchain exists. |
| `config-install` | 16 | Stays shell. A shim to `config deps install`, handled by the adapter plan. |
| `config-build` | 78 | **Stays shell** per 3.2: it compiles the binary it would become, so a machine with no binary could not build one. |
| `config-reload` | 30 | **Ports.** Task 1. |
| `config-test` | 91 | **Ports.** Task 2. |
| `config-help` | 60 | **Ports**, with the golden fixture as its gate. Task 3. |
| `config-doctor` | 26 | **Deleted.** Already a shim whose target moves. Task 4. |

**So this step is 113 lines across three scripts plus a 26-line deletion**,
not 742 across nine. The larger figure is what "the remaining `config-*`
subcommands" implies before the parent's own decisions are applied, and it is
6.5x the real surface.

## Global Constraints

Copied verbatim from the spec and from `~/.claude/CLAUDE.md`.

- **Each port is ONE atomic commit that removes the script and installs the
  subcommand together.** This is a correctness requirement, not tidiness.
  The dispatcher falls through to `git` for any unmatched verb, so a window
  where `config-reload` is gone before its replacement is installed makes
  `config reload` become `git reload`. That one errors confusingly; **a
  subcommand name that IS a git verb would silently do something else.**
- **`config help` output must stay byte-for-byte identical.**
  `tests/config-usage.test.sh` pins it against
  `tests/fixtures/config-help-before-describe.txt` (18 lines, 937 bytes),
  captured before any `--describe` work existed. That fixture is the
  acceptance test for Task 3.
- **`config-help` must keep enumerating `config-*` siblings and asking each
  one `--describe`. It must NOT embed a compiled-in list.** An embedded list
  is a second copy of the same facts, and per the file's own comment "the one
  that nobody edits is the one that goes stale."
- **`--help` and `-h` must be byte-identical**, and help text must contain
  the literal `config <sub>`, not `config-cli <sub>`
  (`config-usage.test.sh:69-82`). Clap prints `#[command(name = ...)]`, so
  each subcommand's name must render as `config <sub>`.
- **`--help` must not execute anything** (`config-usage.test.sh:94`). The
  incident: `config install-hooks --help` once linked the hooks and rewrote
  `~/.local/bin/config` before printing help.
- **`tests/scripts-dir-name.test.sh:199` pins the script count at exactly
  `38`.** Every deletion changes that count and must update it in the same
  commit. An exact count rather than "more than zero" is deliberate: a
  partial add is the failure it catches.
- **Exit 2 is every caller error**, matching the repo-wide convention and the
  oracle `deps-docs.test.sh` relies on.
- **`config-build:64` already skips installation for a member with no
  `src/main.rs`** and prints "is a library, nothing to install", so deleting
  `config-manifest/src/main.rs` is sufficient there. Verify rather than
  assume.
- **Every Rust invocation runs from inside `crates/`, never with
  `--manifest-path`.** rustup honours the toolchain pin only when the working
  directory is under `crates/`.
- **Any change under `crates/` moves the workspace build stamp**, which the
  live pre-push gate BLOCKS if the installed binary is stale. After each task
  run `config build`, then confirm `config doctor` exits 0 silently. Note
  Task 4 changes what `config doctor` even is, so read that task's steps
  carefully.
- **Keep tests hermetic.** Drive IO against a fixture directory the test
  owns. The host-side Rust gate depends on workspace members not mutating the
  real `$HOME` or the real repository.
- **Clear git's environment in any test that shells out to git**:
  `.env_remove("GIT_DIR")`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`,
  `GIT_PREFIX`. See `.agents/PAPERCUTS.md`.
- **Use `config commit -F <file>` with a heredoc, NEVER `config commit -m`.**
- **Do NOT run `config status -uall` and do NOT run `config stash`.**
- NEVER `--no-verify`. NEVER disable a test instead of fixing it. NEVER
  commit a red suite.
- NO em dashes anywhere. NO emoji.
- NO single-letter variable names except numeric loop indices `i`, `j`, `k`.
- Comment WHY not WHAT. Doc comments on public items, with `# Errors` where
  they apply. No `unwrap()`/`expect()` in non-test code without a proven
  invariant.
- Every empty-expected assertion needs a positive control FIRST.
- **An assertion is not a test until you have observed it fail against the
  unfixed code.** Every task has an explicit red step.

## CORRECTION, added after Task 1 shipped: the scripts become SHIMS, not deletions

Task 1 disagreed with this plan and was right. Verified before accepting it.

`.scripts/config/config:29` resolves a verb ONLY by testing
`[ -x "$here/config-$1" ]`, and line 34 falls through to `git` otherwise.
**There is no `config-cli` fallback in the dispatcher.** Confirmed
empirically: `config no-such-verb` answers
`git: 'no-such-verb' is not a git command`.

So deleting `config-reload` would not open a commit-boundary window, the
thing this plan's atomicity argument is about. It would break `config
reload` **permanently** into `git reload`. I wrote that argument without
ever checking whether the dispatcher had a fallback, and it does not.

The spec agrees: its disposition table lists `config-reload`, `config-test`
and `config-help` as **"Ports"**, and reserves **"Delete it"** for
`config-doctor` alone, which is a different case (a pure exec shim with no
logic to move, whose target moves in Task 4).

**So Tasks 1, 2 and 3 leave a two-line shim behind:**

```sh
exec config-cli <verb> "$@"
```

matching the existing `config-deps` and `config-install` pattern, and keep
their `# usage:` and `# help:` headers so `config help` and the doc suites
still see them.

**Consequences for the guard.** `tests/scripts-dir-name.test.sh` counts
committed scripts and lists executables explicitly. Tasks 1 through 3 delete
nothing, so **the count does not change** for them. Task 1 correctly left it
at 40. Only Task 4 removes a script, so only Task 4 updates the count and the
execute-bit allowlist. Every "decrement to 37/36/35/34" instruction below is
wrong and superseded by this paragraph.

## The atomic-swap pattern

Every port task follows the same five moves. Task 1 establishes it on the
lowest-risk subject and later tasks refer back to it.

1. Add the subcommand to `config-cli` with its tests, and confirm both the
   old script and the new subcommand work.
2. Confirm the new subcommand's behavior matches the script's, by running
   both and comparing output.
3. In ONE commit: delete the script, update every reference to it, update
   `scripts-dir-name.test.sh`'s count, and rebuild.
4. Verify the dispatcher resolves the verb to the binary, not to git.
5. Run the full suite.

**The danger to watch for.** After step 3 and before `config build` completes
in step 3's rebuild, the verb resolves to nothing. Do not push between those
moves, and if a step fails midway, restore the script before doing anything
else.

---

## Task 1: `config reload`

30 lines, no caveat. Ports first specifically to prove the atomic-swap
pattern on the lowest-risk subject: `reload` is not a git verb, so a
mid-swap window errors visibly rather than silently doing something else.

**Files:**
- Create: `crates/config-cli/src/reload.rs`
- Modify: `crates/config-cli/src/main.rs`
- Delete: `.scripts/config/config-reload`
- Modify: `tests/scripts-dir-name.test.sh` (the count)

**Interfaces:**
- Consumes: the `config-cli` binary from the adapter plan.
- Produces: `config-cli reload`, and `config reload` through the dispatcher.

- [ ] **Step 1: Read the script and record its exact behavior**

```sh
cd ~ && cat .scripts/config/config-reload
```

Four behaviors to preserve, and the third and fourth are the subtle ones:

1. When inside tmux (`$TMUX` non-empty), `tmux source ~/.config/tmux/tmux.conf`
   and print `reloaded tmux config`.
2. Run `~/.scripts/alacritty-platform.sh` and, on success, print
   `regenerated alacritty platform config`.
3. Print `a child process cannot re-source the calling shell; run the line below`
   to **stderr**, then `source ~/.zshrc` to **stdout**. The stream split is
   the point: the instruction is commentary, the line is the payload, so
   `$(config reload)` yields something runnable.
4. `usage_if_requested "${1:-}"` handles `--help`, and the help text must not
   execute anything.

Record all four verbatim in your report, including which stream each print
goes to.

- [ ] **Step 2: Write the failing test**

```rust
//! `config reload` behavior, asserted against the built binary.

use std::process::Command;

/// The zsh line goes to stdout and the explanation goes to stderr.
///
/// The split is the contract, not formatting: a caller runs
/// `$(config reload)` and must get a runnable line, so the explanation
/// cannot be on stdout with it.
#[test]
fn the_runnable_line_is_on_stdout_and_the_explanation_on_stderr() {
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .arg("reload")
        .env_remove("TMUX")
        .output()
        .expect("the binary runs");

    let stdout = String::from_utf8_lossy(&run.stdout);
    let stderr = String::from_utf8_lossy(&run.stderr);

    // Positive control: both streams must carry something, or the
    // assertions below hold for a command that printed nothing.
    assert!(!stdout.is_empty(), "stdout must carry the runnable line");
    assert!(!stderr.is_empty(), "stderr must carry the explanation");

    assert!(
        stdout.contains("source ~/.zshrc"),
        "the runnable line belongs on stdout: {stdout:?}"
    );
    assert!(
        !stdout.contains("cannot re-source"),
        "the explanation must not pollute stdout, or $(config reload) breaks: {stdout:?}"
    );
    assert!(
        stderr.contains("cannot re-source"),
        "the explanation belongs on stderr: {stderr:?}"
    );
}

/// Outside tmux, the tmux half is skipped rather than failing.
#[test]
fn outside_tmux_the_tmux_half_is_skipped() {
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .arg("reload")
        .env_remove("TMUX")
        .output()
        .expect("the binary runs");

    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        !stdout.contains("reloaded tmux config"),
        "no TMUX means no tmux reload was attempted: {stdout:?}"
    );
}

/// `--help` prints usage and executes nothing.
///
/// The incident this guards: `config install-hooks --help` once linked the
/// hooks and rewrote ~/.local/bin/config before printing help.
#[test]
fn help_executes_nothing() {
    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["reload", "--help"])
        .output()
        .expect("the binary runs");

    assert_eq!(run.status.code(), Some(0), "--help exits 0");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        stdout.contains("config reload"),
        "help must name the command as `config reload`, not `config-cli reload`: {stdout:?}"
    );
    assert!(
        !stdout.contains("source ~/.zshrc"),
        "--help must not print the payload, which would mean it ran: {stdout:?}"
    );
}
```

- [ ] **Step 3: Run the tests to verify they fail**

```sh
cd ~/crates && cargo test --locked -p config-cli reload
```

Expected: FAIL, because `reload` is not a subcommand: clap rejects it and
exits 2, so `status.code()` is `Some(2)` and the stdout assertions fail.
Record the actual output.

- [ ] **Step 4: Implement `reload.rs`**

Preserve all four behaviors from Step 1. Use
`#[command(name = "config reload")]` so help renders `config reload` rather
than `config-cli reload`, which `config-usage.test.sh:69-82` pins.

The alacritty call and the tmux call are both effects: put them in this
module and keep them obvious. There is no pure core worth extracting from 30
lines of "run two commands and print three strings".

- [ ] **Step 5: Run the tests and compare against the script**

```sh
cd ~/crates && cargo test --locked -p config-cli
cd ~/crates && cargo clippy --locked --all-targets -- -D warnings; echo "clippy=$?"
```

Then compare the two implementations directly, which is the pattern's step 2:

```sh
cd ~
env -u TMUX .scripts/config/config-reload > /tmp/reload.old.out 2> /tmp/reload.old.err
env -u TMUX ./crates/target/release/config-cli reload > /tmp/reload.new.out 2> /tmp/reload.new.err \
  || cargo run --locked --manifest-path /dev/null 2>/dev/null || true
diff /tmp/reload.old.out /tmp/reload.new.out && echo "stdout identical"
diff /tmp/reload.old.err /tmp/reload.new.err && echo "stderr identical"
```

If the release binary is not built yet, use
`cd ~/crates && cargo run --locked -p config-cli -- reload` and capture from
that instead. Expected: both streams identical. Any difference is either a
behavior change to justify in your report or a bug to fix.

- [ ] **Step 6: The atomic swap, in one commit**

```sh
cd ~
config rm .scripts/config/config-reload
```

Update `tests/scripts-dir-name.test.sh:199`'s count from `38` to `37`. Read
the surrounding comment first: the exact count is deliberate.

Then find and update every remaining reference:

```sh
cd ~ && export GIT_DIR=$HOME/.cfg GIT_WORK_TREE=$HOME
git grep -rn -I 'config-reload' -- . | grep -v '^docs/'
```

Expected after your edits: no reference that expects the script to exist.
References in `docs/` describing history may stay.

- [ ] **Step 7: Rebuild, then verify the dispatcher resolves to the binary**

```sh
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
cd ~ && env -u TMUX config reload; echo "reload_exit=$?"
cd ~ && config reload --help 2>&1 | head -3
```

Expected: `doctor=0`; `config reload` runs the binary and prints the same
output as before; help names `config reload`. **If you see a git error, the
verb fell through to git and the swap is incomplete.**

- [ ] **Step 8: Run the suites this touches**

```sh
cd ~ && bash tests/config-usage.test.sh 2>&1 | tail -2
cd ~ && bash tests/scripts-dir-name.test.sh 2>&1 | tail -2
cd ~ && bash tests/config.test.sh 2>&1 | tail -2
```

Expected: all PASS. `scripts-dir-name` fails loudly if the count is wrong,
which is what it is for.

- [ ] **Step 9: Verify the test binds**

```sh
cd ~/crates
cp config-cli/src/reload.rs /tmp/reload.rs.good
python3 - <<'PY'
import pathlib
path = pathlib.Path("config-cli/src/reload.rs")
text = path.read_text()
# Move the explanation to stdout, which is the defect that breaks
# $(config reload) for every caller.
needle = "eprintln!"
assert needle in text, "adjust this sabotage to the real print calls"
path.write_text(text.replace(needle, "println!", 1))
PY
cargo test --locked -p config-cli reload 2>&1 | grep -cE 'FAILED|panicked'
cp /tmp/reload.rs.good config-cli/src/reload.rs && rm /tmp/reload.rs.good
cargo test --locked -p config-cli reload 2>&1 | tail -1
```

Expected: the sabotaged run fails
`the_runnable_line_is_on_stdout_and_the_explanation_on_stderr`; the restored
run passes.

- [ ] **Step 10: Commit**

```sh
cd ~
cat > /tmp/msg.txt <<'MSG'
Port config reload into config-cli, atomically

One commit that deletes the script and installs the subcommand, because the
dispatcher falls through to git for any unmatched verb. reload is not a git
verb, so a mid-swap window errors visibly, which is why this subject goes
first: it proves the pattern where the failure mode is loud.

The stream split is preserved and tested. The explanation goes to stderr and
the runnable line to stdout, because a caller runs $(config reload) and must
get something runnable. A test fails when the explanation moves to stdout.

Help renders "config reload" rather than "config-cli reload", which
config-usage.test.sh pins, and --help prints no payload: the incident behind
that assertion is config install-hooks --help once linking the hooks and
rewriting ~/.local/bin/config before printing help.

scripts-dir-name.test.sh's exact count drops by one in this same commit. The
count is exact rather than a lower bound on purpose, so a deletion that
forgets it fails a test.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add -A
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

`config add -A` is used here because the commit spans a deletion and edits
across several files and must be atomic. Run `config diff --cached --stat`
first and confirm the file list is what you expect.

---

## Task 2: `config test`

91 lines, self-contained, no caveat. Same atomic-swap pattern as Task 1.

**Files:**
- Create: `crates/config-cli/src/test_runner.rs`
- Modify: `cratests/config-cli/src/main.rs`
- Delete: `.scripts/config/config-test`
- Modify: `tests/scripts-dir-name.test.sh` (the count, again)

**Interfaces:**
- Consumes: nothing from Task 1 beyond the established pattern.
- Produces: `config-cli test`, and `config test` through the dispatcher.

**A naming note.** The module is `test_runner.rs`, not `test.rs`: a module
named `test` inside a crate that also has `#[cfg(test)]` modules is
needlessly confusing, and `mod test` shadows nothing but reads as if it
might.

- [ ] **Step 1: Read the script and record its behavior**

```sh
cd ~ && cat .scripts/config/config-test
```

Record: what it runs, how it forwards arguments, what it does with a named
suite versus no argument, and its exit-code behavior. `config test <suite>`
almost certainly forwards to `tests/run-all.sh`, which has its own
named-suite handling, so the subcommand may be a thin forwarder. If it is
thin, say so: a thin forwarder is the right answer and does not need
elaboration.

- [ ] **Step 2: Write the failing test**

Assert the behaviors Step 1 found. At minimum:

```rust
/// A named suite is forwarded, not swallowed.
#[test]
fn a_named_suite_reaches_the_runner() {
    let run = std::process::Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["test", "definitely-not-a-real-suite"])
        .output()
        .expect("the binary runs");

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );

    // Positive control: the command must say something, or the naming
    // assertion below holds for a silent no-op.
    assert!(!combined.is_empty(), "a bad suite name must be reported");
    assert!(
        combined.contains("definitely-not-a-real-suite"),
        "the suite name must reach the runner and be named back: {combined:?}"
    );
    assert_ne!(run.status.code(), Some(0), "an unknown suite is not success");
}

/// `--help` executes no tests.
#[test]
fn help_runs_no_tests() {
    let run = std::process::Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["test", "--help"])
        .output()
        .expect("the binary runs");

    assert_eq!(run.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(stdout.contains("config test"), "help names `config test`: {stdout:?}");
    assert!(
        !stdout.contains("suite(s) passed"),
        "--help must not have run the suite: {stdout:?}"
    );
}
```

- [ ] **Step 3: Run the tests to verify they fail**

```sh
cd ~/crates && cargo test --locked -p config-cli test_runner
```

Expected: FAIL with clap rejecting `test`. Record it.

- [ ] **Step 4: Implement, following Task 1's pattern**

`#[command(name = "config test")]`. Forward arguments through to the runner
without reinterpreting them: the runner owns suite selection, and a second
copy of that logic here is the drift shape this whole migration is against.

- [ ] **Step 5: Compare against the script before swapping**

```sh
cd ~
.scripts/config/config-test definitely-not-a-real-suite > /tmp/test.old.out 2>&1; echo "old=$?"
cd ~/crates && cargo run --locked -q -p config-cli -- test definitely-not-a-real-suite \
  > /tmp/test.new.out 2>&1; echo "new=$?"
diff /tmp/test.old.out /tmp/test.new.out && echo "identical"
```

Expected: identical output and matching exit codes. Report both.

- [ ] **Step 6: The atomic swap**

```sh
cd ~ && config rm .scripts/config/config-test
```

Update the count in `tests/scripts-dir-name.test.sh` (now to `36`), then:

```sh
cd ~ && export GIT_DIR=$HOME/.cfg GIT_WORK_TREE=$HOME
git grep -rn -I 'config-test' -- . | grep -v '^docs/'
```

Update every reference. **Watch for `.github/workflows/`**: if a workflow
calls `config test`, it keeps working, but if it calls the script by path it
breaks, and CI is where you would find out.

- [ ] **Step 7: Rebuild and verify**

```sh
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
cd ~ && config test --help 2>&1 | head -3
cd ~ && bash tests/config-usage.test.sh 2>&1 | tail -2
cd ~ && bash tests/scripts-dir-name.test.sh 2>&1 | tail -2
```

Expected: all PASS, help names `config test`.

- [ ] **Step 8: Run the whole suite through the new path**

```sh
cd ~ && config test 2>&1 | tail -4
```

Expected: the full suite runs and reports its usual totals. This is the
subcommand doing its actual job, so a discrepancy in the suite count here is
a real finding.

- [ ] **Step 9: Commit**

```sh
cd ~
cat > /tmp/msg.txt <<'MSG'
Port config test into config-cli, atomically

Same one-commit swap as config reload, for the same dispatcher-fallthrough
reason.

The subcommand forwards arguments to the runner without reinterpreting
them. The runner owns suite selection, and a second copy of that logic in
the subcommand is exactly the drift shape this migration exists to remove.

The module is test_runner.rs rather than test.rs: a module named test in a
crate full of #[cfg(test)] modules reads as if it might be one.

scripts-dir-name.test.sh's exact count drops again in this same commit.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add -A
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 3: `config help`, with the golden fixture as the gate

60 lines, and the one caveat that makes this task different: the ported
version must **keep asking its siblings** rather than embedding a list, and
`config help` output is pinned byte-for-byte against an 18-line fixture
captured before any `--describe` work existed.

**Files:**
- Create: `crates/config-cli/src/help.rs`
- Modify: `crates/config-cli/src/main.rs`
- Delete: `.scripts/config/config-help`
- Modify: `tests/scripts-dir-name.test.sh` (the count)

**Interfaces:**
- Consumes: the `--describe` convention every `config-*` script implements.
- Produces: `config-cli help`, and `config help` through the dispatcher,
  byte-identical to the fixture.

- [ ] **Step 1: Read the script, the fixture, and the pinning test**

```sh
cd ~ && cat .scripts/config/config-help
cd ~ && cat tests/fixtures/config-help-before-describe.txt
cd ~ && grep -n "config-help-before-describe" -B5 -A20 tests/config-usage.test.sh
```

Record: how the script enumerates siblings, how it calls `--describe`, how it
sorts and formats, and exactly what the pinning test compares. The fixture is
18 lines and 937 bytes; the formatting is part of the contract, including
column alignment and ordering.

**Note the ordering hazard.** The script enumerates `.scripts/config/config-*`
from the filesystem. Tasks 1 and 2 deleted two of those scripts, and this
task deletes a third, so the set of things being enumerated is changing
underneath this port. The fixture predates all of it. **Work out now whether
the fixture still matches today's output**, before you change anything:

```sh
cd ~ && config help > /tmp/help.today.txt 2>&1
diff tests/fixtures/config-help-before-describe.txt /tmp/help.today.txt \
  && echo "fixture matches today" || echo "fixture ALREADY differs"
```

If it already differs, the earlier tasks changed it and the fixture was
updated then, or should have been. Report which, and if a previous task
should have updated the fixture and did not, fix that here and say so.

- [ ] **Step 2: Write the failing test**

The golden comparison is the gate, and it belongs in the shell suite that
already owns it. Add to the Rust side only what the fixture cannot express:

```rust
/// The listing is built by asking siblings, not from an embedded list.
///
/// An embedded list is a second copy of the same facts, and per the shell
/// version's own comment, "the one that nobody edits is the one that goes
/// stale." Asserted by pointing the enumeration at a fixture directory
/// containing one script with a known description: an embedded list cannot
/// know about it.
#[test]
fn the_listing_asks_its_siblings_rather_than_embedding_them() {
    let fixture = tempfile::tempdir().expect("tempdir");
    let script = fixture.path().join("config-invented");
    std::fs::write(
        &script,
        "#!/bin/sh\n[ \"${1:-}\" = --describe ] && printf 'An invented subcommand\\n'\n",
    )
    .expect("write");
    let mut permissions = std::fs::metadata(&script).expect("metadata").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(&script, permissions).expect("chmod");

    let run = std::process::Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .arg("help")
        .env("CONFIG_SUBCOMMAND_DIR", fixture.path())
        .output()
        .expect("the binary runs");

    let stdout = String::from_utf8_lossy(&run.stdout);

    // Positive control: the listing must be non-empty, or "contains" below
    // holds for a command that printed nothing.
    assert!(!stdout.is_empty(), "help must list something");
    assert!(
        stdout.contains("invented"),
        "a subcommand present only in the fixture directory must appear, \
         which an embedded list could not do: {stdout:?}"
    );
    assert!(
        stdout.contains("An invented subcommand"),
        "the description must come from --describe: {stdout:?}"
    );
}
```

`CONFIG_SUBCOMMAND_DIR` is a test seam. It must default to the real
`.scripts/config/` directory when unset, and it exists because this property
is otherwise untestable without mutating the real script directory.

- [ ] **Step 3: Run the test to verify it fails**

```sh
cd ~/crates && cargo test --locked -p config-cli help
```

Expected: FAIL, clap rejecting `help` (or, if clap reserves `help`,
something else entirely: **if clap intercepts the `help` subcommand, say so
now**, because that changes the implementation and `disable_help_subcommand`
in `main.rs` is how the adapter plan already prepared for it).

- [ ] **Step 4: Implement `help.rs`**

Enumerate `CONFIG_SUBCOMMAND_DIR` (defaulting to `.scripts/config/`), find
`config-*` entries, run each with `--describe`, and format exactly as the
fixture requires. Sort the way the script sorted.

**The binary's own subcommands must appear too**, now that `reload`, `test`
and `help` are no longer scripts. That is the part the fixture will notice:
after this task, three verbs exist only inside the binary. Read the fixture
and decide whether it needs updating for that, and if it does, update it in
this commit and explain in your report why the new bytes are correct rather
than merely different.

**A fixture updated without an argument is how a golden test stops being a
gate.** State the reason each changed line changed.

- [ ] **Step 5: Compare byte-for-byte against the script before swapping**

```sh
cd ~
.scripts/config/config-help > /tmp/help.old.txt 2>&1
cd ~/crates && cargo run --locked -q -p config-cli -- help > /tmp/help.new.txt 2>&1
diff /tmp/help.old.txt /tmp/help.new.txt && echo "byte identical"
```

Expected: identical, **except** for verbs that are now binary subcommands
rather than scripts. Any other difference is a bug. Report the diff verbatim.

- [ ] **Step 6: The atomic swap**

```sh
cd ~ && config rm .scripts/config/config-help
```

Update the count in `tests/scripts-dir-name.test.sh` (now `35`), update the
fixture if Step 4 concluded it must change, and update references:

```sh
cd ~ && export GIT_DIR=$HOME/.cfg GIT_WORK_TREE=$HOME
git grep -rn -I 'config-help' -- . | grep -v '^docs/'
```

- [ ] **Step 7: Rebuild and run the golden gate**

```sh
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
cd ~ && bash tests/config-usage.test.sh 2>&1 | tail -3
cd ~ && bash tests/scripts-dir-name.test.sh 2>&1 | tail -2
cd ~ && config help
```

Expected: `config-usage.test.sh` PASSES, which is this task's acceptance
test. If it fails on the fixture comparison, either the format drifted or the
fixture needs a justified update; do not update it to match without stating
why the new bytes are right.

- [ ] **Step 8: Verify the sibling-asking property binds**

```sh
cd ~/crates
cp config-cli/src/help.rs /tmp/help.rs.good
python3 - <<'PY'
import pathlib
path = pathlib.Path("config-cli/src/help.rs")
text = path.read_text()
needle = "--describe"
assert needle in text, "adjust this sabotage to how the module asks siblings"
path.write_text(text.replace(needle, "--describe-disabled", 1))
PY
cargo test --locked -p config-cli help 2>&1 | grep -cE 'FAILED|panicked'
cp /tmp/help.rs.good config-cli/src/help.rs && rm /tmp/help.rs.good
cargo test --locked -p config-cli help 2>&1 | tail -1
```

Expected: the sabotaged run fails
`the_listing_asks_its_siblings_rather_than_embedding_them`; the restored run
passes.

- [ ] **Step 9: Commit**

```sh
cd ~
cat > /tmp/msg.txt <<'MSG'
Port config help, still asking its siblings

The ported listing enumerates config-* siblings and runs --describe on each,
rather than embedding a compiled-in list. An embedded list is a second copy
of the same facts, and per the shell version's own comment, the one nobody
edits is the one that goes stale. There is a test that fails when the
enumeration stops asking: it points the search at a fixture directory
holding an invented subcommand, which an embedded list could not know about.

config-usage.test.sh's golden fixture is the acceptance test for this port
and it passes. Any fixture line that changed is explained in the task
report, because a golden fixture updated without an argument has stopped
being a gate.

scripts-dir-name.test.sh's exact count drops again in this same commit.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add -A
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 4: Delete `config-doctor`, close the two-binary window

Two changes that must land together: `config-manifest`'s two binary
consumers move into `config-cli`, and then `config-manifest/src/main.rs` is
deleted so the crate is library-only. That deletion is the exit condition the
adapter spec's 3a named for the two-binary window.

**Files:**
- Create: `crates/config-cli/src/doctor.rs`,
  `crates/config-cli/src/verify_stamps.rs`
- Modify: `crates/config-cli/src/main.rs`
- Delete: `.scripts/config/config-doctor`,
  `crates/config-manifest/src/main.rs`
- Modify: `crates/config-manifest/Cargo.toml` (drop the `[[bin]]` if one is
  declared), `tests/pre-push`, `tests/config-manifest-lifecycle.test.sh`,
  `tests/scripts-dir-name.test.sh`, `tests/config-usage.test.sh`
- Modify: `tests/docker/Dockerfile` (the runtime stage copies the
  `config-manifest` binary; that binary is going away)

**Interfaces:**
- Consumes: `config_manifest`'s library API for doctor and stamp
  verification.
- Produces: `config-cli doctor` and `config-cli verify-stamps`.

- [ ] **Step 1: Verify the two consumers, and find any third**

The spec says exactly two. Verify rather than trust, and look for a third:

```sh
cd ~ && export GIT_DIR=$HOME/.cfg GIT_WORK_TREE=$HOME
git grep -rn -I 'config-manifest' -- . | grep -v '^docs/' | grep -vE '^crates/config-manifest/'
```

Expected: `.scripts/config/config-doctor:26` (`exec config-manifest doctor`),
`tests/pre-push:194` (`verify-stamps --ref`), plus the `tests/docker/`
runtime copy, plus `config-build`/`config-stamp` mentions, plus
`tests/config-manifest-lifecycle.test.sh:67`'s `--version` assertion.
**Record the full list**; anything calling the binary must move or be updated
in this one commit.

- [ ] **Step 2: Confirm `config-build` handles a library-only member**

The spec says `config-build:64` already skips a member with no
`src/main.rs`. Verify:

```sh
cd ~ && sed -n '55,75p' .scripts/config/config-build
```

Expected: a guard on `src/main.rs` existing, printing "is a library, nothing
to install". If it is not there, this task must add it, and say so.

- [ ] **Step 3: Write the failing tests**

```rust
/// `config-cli doctor` reports what the config-manifest binary reported.
#[test]
fn doctor_reports_installed_binary_drift() {
    let run = std::process::Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .arg("doctor")
        .output()
        .expect("the binary runs");

    // On a clean machine doctor exits 0 and prints nothing, which is the
    // contract the pre-push gate relies on. Positive control: the command
    // must at least run.
    assert!(
        run.status.code().is_some(),
        "doctor must exit, not be killed by a signal"
    );
    if run.status.code() == Some(0) {
        assert!(
            run.stdout.is_empty(),
            "a clean doctor prints nothing, or the gate's silence check breaks"
        );
    }
}

/// `verify-stamps` rejects a ref whose stamps do not match.
#[test]
fn verify_stamps_rejects_a_mismatch() {
    let run = std::process::Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["verify-stamps", "--ref", "definitely-not-a-ref"])
        .output()
        .expect("the binary runs");

    assert_ne!(run.status.code(), Some(0), "an unresolvable ref is not success");
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(!stderr.is_empty(), "a failure must explain itself");
}
```

- [ ] **Step 4: Run the tests to verify they fail**

```sh
cd ~/crates && cargo test --locked -p config-cli doctor
cd ~/crates && cargo test --locked -p config-cli verify_stamps
```

Expected: FAIL, clap rejecting both subcommands. Record it.

- [ ] **Step 5: Implement both subcommands over the library API**

Read `crates/config-manifest/src/main.rs` to see what its `doctor` and
`verify-stamps` arms do, then call the same library functions from
`config-cli`. **Do not reimplement the logic**: `config_manifest::doctor` is
the in-repo precedent for the module shape and it is a library module
already.

`verify-stamps` reads stamps from stdin, per `tests/pre-push:191-194`, which
pipes `config-stamp --ref` output into it. Preserve that interface exactly:
the hook depends on it.

- [ ] **Step 6: Move both consumers, in this commit**

`tests/pre-push`: change `$config_manifest_bin` to point at `config-cli` and
the argument to `verify-stamps`. Read the surrounding comment: it explains
that the binary is resolved from the hook's own location rather than `$HOME`
so a test can drive the hook against a fixture repo. Preserve that property.

Delete `.scripts/config/config-doctor`. The dispatcher then needs `doctor` to
resolve: confirm whether it reaches `config-cli doctor` automatically or
needs a `config-cli` entry in the dispatcher's table, and wire it either way.

- [ ] **Step 7: Delete `config-manifest`'s binary and fix its fallout**

```sh
cd ~ && config rm crates/config-manifest/src/main.rs
```

Then:

- `tests/config-manifest-lifecycle.test.sh:67` asserts
  `config-manifest 0.1.0` from `config-manifest --version`. That binary will
  not exist. Move the assertion to `config-cli --version` or remove it, and
  say which and why.
- `tests/docker/Dockerfile`'s runtime stage copies
  `/build/target/release/config-manifest` onto `PATH`. That binary is gone;
  copy `config-cli` instead, and update the comment, which currently says
  the suite's shell tests call `config-manifest` by name.
- `tests/config-usage.test.sh` asserts the `config-doctor` shim's `# help:`
  line matches `config-manifest --describe`. Both sides of that assertion are
  going away; remove it, which the test's own comment anticipates.
- Update `tests/scripts-dir-name.test.sh`'s count (now `34`).

- [ ] **Step 8: Verify the whole gate chain still works**

This task touches the pre-push gate itself, so exercise it directly rather
than by pushing:

```sh
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
cd ~ && head=$(config rev-parse HEAD)
printf 'refs/heads/main %s refs/heads/main %s\n' "$head" "$(config rev-parse HEAD~1)" \
    | ./tests/pre-push origin https://example.invalid/repo.git 2>&1 | grep -E 'stamp|Rust|leak'
```

Expected: the leak scan, the stamp gate and (if the rust-gate plan landed)
the Rust checks all report passing. **If the stamp gate fails, `verify-stamps`
moved incorrectly and a push would be blocked**, which is the worst outcome
of this task.

- [ ] **Step 9: Run everything**

```sh
cd ~/crates && cargo test --locked
cd ~/crates && cargo clippy --locked --all-targets -- -D warnings; echo "clippy=$?"
cd ~ && bash tests/run-all.sh 2>&1 | tail -5
cd ~ && bash tests/config-manifest-lifecycle.test.sh 2>&1 | tail -2
cd ~ && bash tests/container.test.sh 2>&1 | tail -2
```

Expected: all green. `container.test.sh` matters here because the Dockerfile
changed.

- [ ] **Step 10: Confirm the window is actually closed**

```sh
cd ~ && ls crates/config-manifest/src/main.rs 2>/dev/null && echo "STILL PRESENT" || echo "library-only"
cd ~ && command -v config-manifest && echo "BINARY STILL INSTALLED" || echo "binary gone"
```

Expected: `library-only`, and no `config-manifest` on `PATH` after a
rebuild. If the binary is still installed, `config build` did not remove a
stale install; remove it explicitly and note that `config build` does not
clean up a member that stops being a binary, which is worth a papercut entry.

- [ ] **Step 11: Commit**

```sh
cd ~
cat > /tmp/msg.txt <<'MSG'
Close the two-binary window, and delete the config-doctor shim

config-manifest's binary had exactly two consumers, verified rather than
trusted: config-doctor's exec line and pre-push's verify-stamps call. Both
are now config-cli subcommands over the same library functions, so no logic
was reimplemented.

config-manifest/src/main.rs is deleted and the crate is library-only, which
is the exit condition the adapter spec named for the window where both
binaries were on PATH. config-build already skips installation for a member
with no src/main.rs and says "is a library, nothing to install", so the
deletion is sufficient there.

config-doctor was 26 lines whose body was an exec. Once its target moved the
shim had no purpose, and deleting it removes the string duplication that
config-usage.test.sh asserted away: the shim's help line and
config-manifest --describe were two copies of one description, and the
assertion tying them together existed only because both existed. The test's
own comment anticipated this.

Three fallout sites land in the same commit: the lifecycle suite's
--version assertion, the test image's runtime binary copy, and
scripts-dir-name.test.sh's exact count.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add -A
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Self-Review

**Spec coverage.** Section 1's scope table drives the task list: the four
"stays shell" entries produce no task (correct, they are decisions not work),
`config-build` produces no task per 3.2's confirmed decision, and the
remaining four map to Tasks 1 through 4. Section 2's atomicity rule is the
Global Constraint every task's swap step implements. Section 3.1 is Task 3.
Section 3.2a is Task 4. Section 3.3 is Task 4. Section 4's order is the task
order exactly. Section 5's four "must not break" items each have a check:
the golden fixture in Task 3 Step 7, the `config <sub>` help rendering in
every port's test, the "`--help` executes nothing" assertion in Tasks 1 and
2, and the script count in every deletion.

**One decision this plan makes that the spec left open.** Section 3.2 says
`config-build` stays shell "pending the plan's confirmation". **Confirmed**,
for the spec's own two reasons: the circularity is identical to
`config-stamp`'s, which parent 8.1 already resolved this way, and a fallback
path would let the gate pass having exercised the path being deleted. The
future direction (bootstrapping from a release artifact) is recorded in
`TODO-AGENTS.md` and is explicitly out of scope here.

**Placeholder scan.** No TBD, no "similar to Task N". Tasks 2 and 4 tell the
implementer to read the script and record its behavior before writing tests,
which is deliberate: `config-test`'s forwarding contract and
`config-manifest`'s two subcommand bodies are short enough to read and too
specific to restate accurately from outside.

**Type consistency.** `CONFIG_SUBCOMMAND_DIR` is introduced in Task 3 Step 2
and defined in Step 4. `#[command(name = "config <sub>")]` is used
identically in all four tasks, which is what satisfies
`config-usage.test.sh:69-82`. The script count decrements once per deletion:
38 to 37 to 36 to 35 to 34, and each task names its own number.

**Three risks worth naming.**

1. **Task 3's fixture may already be stale** by the time it runs, because
   Tasks 1 and 2 change what `config help` enumerates. Step 1 checks for that
   explicitly before any edit and tells the implementer to report which case
   they found rather than silently updating.
2. **Task 4 touches the pre-push gate itself.** Step 8 drives the hook
   directly rather than pushing, and names the failure mode: a broken
   `verify-stamps` move blocks every future push.
3. **`clap` may reserve `help` as a subcommand name.** Task 3 Step 3 tells
   the implementer to say so immediately if it does; the adapter plan already
   sets `disable_help_subcommand = true` in anticipation.

**Ordering.** Strictly sequential, and the spec's order is the reason: Task 1
is the lowest-risk subject and proves the pattern, Task 2 is self-contained,
Task 3 changes what Task 1 and 2's deletions made observable, and Task 4 is
last because it closes a window the others widen.
