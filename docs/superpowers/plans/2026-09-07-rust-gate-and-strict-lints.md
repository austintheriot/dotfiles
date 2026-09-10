# Rust Gate and Strict Lints Implementation Plan

> **STATUS 2026-09-10: ALREADY IMPLEMENTED. Do not execute this plan.**
>
> Every task below describes work that has shipped. Verified against the
> tree on 2026-09-10:
>
> - `tests/rust-checks.sh:139` runs `cargo test --locked --quiet`, and
>   `:145` runs `cargo clippy --locked --all-targets -- -D warnings`.
> - `tests/pre-push:17` documents that the Rust checks run on the host
>   ahead of the container, and `:232` requires the script to be present
>   and executable. `:219` records the original defect in its own comment:
>   pre-push "printed 'SKIP cargo test (cargo not found)' and passed".
> - `.github/workflows/test-suite.yml:163` runs the same clippy command.
> - `crates/Cargo.toml:24` and `:33` carry `[workspace.lints.rust]` and
>   `[workspace.lints.clippy]`, with the measured reasoning about
>   `pedantic` and `must_use_candidate` in the surrounding comment. All six
>   members opt in with `[lints] workspace = true`.
> - `tests/pre-push:237` records a second defect found and fixed after this
>   plan was written: git runs a hook with a minimal PATH omitting
>   `~/.cargo/bin`, so the checks skipped on machines that did have a
>   toolchain.
>
> The 36 unchecked boxes are therefore misleading rather than pending. They
> are left in place rather than ticked, because nobody can now say which
> commit satisfied which box, and inventing that mapping would be worse
> than leaving the record honest. The spec's section 4a is the durable
> description of what was built.


> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give `cargo test` a local pre-push gate and give
`cargo clippy -D warnings` a gate anywhere at all, and move lint strictness
out of a command line into `crates/Cargo.toml` where every invocation reads
it.

**Architecture:** Three independent surfaces, in dependency order. A new
`tests/rust-checks.sh` runs the hermetic Rust checks against the pushed ref
in a `git archive` snapshot, `tests/pre-push` calls it before handing off to
Docker, `tests/run-all.sh` gains a clippy leg beside its existing cargo-test
leg, and `.github/workflows/test-suite.yml` gains the same clippy step.
Then `crates/Cargo.toml` declares the lint policy the gates enforce.

**Tech Stack:** POSIX sh (the hooks are `#!/bin/sh`), bash (`run-all.sh` is
bash and uses `local`), GitHub Actions, Cargo 1.94.0 pinned by
`crates/rust-toolchain.toml`.

**Spec:** `docs/superpowers/specs/2026-09-07-shell-test-port-design.md`,
section 4a. Read 4a.1 and 4a.2 in full before Task 1: they carry the
measurements this plan rests on and the two rejected alternatives.

## Global Constraints

Copied verbatim from the spec and from `~/.claude/CLAUDE.md`.

- **Every Rust invocation runs from inside `crates/`, never with
  `--manifest-path`.** rustup honours `crates/rust-toolchain.toml` only when
  the working directory is under `crates/`, so a run from the repo root
  declares the 1.94.0 pin without applying it. That exact mistake has failed
  CI once already, and it is documented at `run-all.sh:196-199` and in
  `test-suite.yml`.
- **The host Rust checks must run against the PUSHED REF, not the working
  tree.** `$HOME` is not always on the branch being pushed: a push from a
  worktree leaves `$HOME` on whatever branch it was already on.
  `run-in-docker.sh:41-45` documents this hazard for the container leg and
  solves it with `git archive`; the host leg has the same hazard and takes
  the same solution. A gate that tests `$HOME` while reporting on the pushed
  ref is worse than no gate.
- **`CARGO_TARGET_DIR` must be set to the shared cache**
  (`$HOME/.cache/config-manifest/target`, the default `run-all.sh:234`
  already uses). Measured on this repo: a cold target directory takes over
  120 seconds, the shared one takes **7 seconds**. Without this the gate is
  slow enough that someone will disable it.
- **If cargo is absent the check must SKIP LOUDLY**, the way
  `run-all.sh:245` does. A gate that says nothing when it skips is
  indistinguishable from a gate that is not installed.
- **`rust-checks.sh` MUST clear git's environment variables before running
  cargo**, with `env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE -u
  GIT_PREFIX`. This is not hypothetical and not optional. Git exports
  `GIT_DIR` and `GIT_WORK_TREE` into every hook, `config-manifest`'s tests
  build throwaway git repositories as fixtures, and with those variables set
  a fixture's `git add .` operates on the **real dotfiles repo**. Reproduced
  live while writing this plan: a `cargo test` run inherited an exported
  `GIT_DIR=$HOME/.cfg`, and a fixture at
  `/var/folders/.../T/.tmplD2FsJ` ran `git add .` against `~/.cfg`, taking
  `~/.cfg/index.lock` and blocking every subsequent `config` command until
  the process was killed. `tests/pre-push:228-232` already documents this
  hazard for the Docker leg and guards it the same way. The host leg has the
  same exposure and needs the same guard.
- **Do NOT add a host fallback for the Docker suite.** `tests/pre-push`
  rejects that explicitly and the rejection is correct: falling back when the
  daemon is down reintroduces the tmux and fixture-repo flake the container
  exists to contain. This plan ADDS a gate that always runs; it never
  replaces or weakens the container leg.
- **Do NOT put a Rust toolchain in `tests/docker/Dockerfile`'s runtime
  stage.** That is a later step's decision (spec section 4's container-leg
  bullet, for Tranche A onward) and it is not a prerequisite here.
- **Never `--no-verify`.** Never disable a test instead of fixing it. Never
  commit a red suite.
- **Always `cd ~/crates` before any cargo command** in your own verification
  runs, for the same toolchain-pin reason.
- **Any change under `crates/` moves the workspace build stamp**, which makes
  the installed `~/.local/bin/config-manifest` stale, which the live pre-push
  gate BLOCKS. After Task 4 run `config build`, then confirm `config doctor`
  exits 0 and prints nothing.
- **Use `config commit -F <file>` with a heredoc, NEVER `config commit -m`.**
  A message containing double quotes or `$` gets shredded by the shell into
  pathspec errors.
- **Do NOT run `config status -uall` and do NOT run `config stash`.** Both
  walk all of `$HOME` in this bare repo and can leave `.cfg/index.lock` held,
  which blocks `config add`.
- NO em dashes anywhere. Use a comma, a colon, parentheses, or two hyphens.
- NO emoji.
- NO single-letter variable names except numeric loop indices `i`, `j`, `k`.
- Comment WHY not WHAT. No comment restating the code.
- Shell: quote every expansion. `shellcheck` runs over `tests/` in the suite,
  so a new script must pass it.
- **An assertion is not a test until you have observed it fail against the
  unfixed code.** Every task below has an explicit red step. Report the
  observation, with real output.

## Repository Context

This is a **bare** git repo at `~/.cfg` with `$HOME` as the worktree. Plain
`git` does NOT work in `~/crates`. The `config` command (`~/.local/bin/config`)
is git against that bare repo, and tracked paths are home-relative
(`tests/pre-push`, `crates/Cargo.toml`).

The live hooks are symlinks: `~/.cfg/hooks/pre-push -> ~/tests/pre-push` and
`~/.cfg/hooks/pre-commit -> ~/tests/pre-commit`. `core.hooksPath` is not set.
So editing `~/tests/pre-push` changes the live gate immediately, which is why
Task 2's verification drives the script directly rather than by pushing.

## File Structure

| File | Lines now | Change |
|---|---|---|
| `tests/rust-checks.sh` | **new** | Runs `cargo test` and `cargo clippy` against a ref, in an archive snapshot. One responsibility, callable from the hook and by hand. |
| `tests/pre-push` | 243 | Calls `rust-checks.sh` after the stamp gate, before the Docker handoff. |
| `tests/run-all.sh` | 271 | Gains a clippy leg beside the cargo-test leg, counted in `total_suites`. |
| `.github/workflows/test-suite.yml` | 200 | Gains a clippy step after the existing build step. |
| `crates/Cargo.toml` | 20 | Gains `[workspace.lints.rust]` and `[workspace.lints.clippy]`. |
| `crates/*/Cargo.toml` | 3 files | Each gains `[lints] workspace = true`. |
| `crates/*/src/lib.rs`, `main.rs` | 3 files | Each gains one `cfg_attr(not(test), deny(...))` line. |
| `tests/rust-gate.test.sh` | **new** | Asserts the hook wiring and the skip path, so the gate cannot silently stop running. |

**`rust-checks.sh` is its own file rather than inline in the hook.** The hook
is already 243 lines across three gates, `run-all.sh` needs the same clippy
invocation, and a developer wants to run the checks by hand without pushing.
One script with a ref argument serves all three.

---

## Task 1: `tests/rust-checks.sh`, the hermetic Rust check runner

The gate's engine. Takes a ref, materializes it with `git archive` into a
temp directory, and runs `cargo test` and `cargo clippy` from inside its
`crates/`.

Why an archive rather than the working tree: `$HOME` may be on a different
branch than the ref being pushed (`run-in-docker.sh:41-45`), so testing the
working tree would report a pass for code that is not being pushed.

Why this is safe to run on the host at all, which the spec establishes and
you do not need to re-derive: `deps-core` and `dotfiles-path` are pure and
`deps-core` enforces it with a purity test over its own sources, and every
`config-manifest` test builds its own `tempfile::tempdir()` and passes
explicit `--git-dir`/`--work-tree`, so no test touches the real repository.
Verified by running the workspace with `HOME` pointed at an empty directory:
all green, real home unchanged, fake home still empty afterwards.

**Files:**
- Create: `tests/rust-checks.sh`

**Interfaces:**
- Consumes: nothing from earlier tasks; this is first.
- Produces: `tests/rust-checks.sh [REF]`. Exit 0 when both checks pass or
  when cargo is absent (a loud skip). Exit 1 when either check fails or the
  ref cannot be archived. Defaults `REF` to `HEAD`. Task 2's hook and Task 5's
  test both call it.

- [ ] **Step 1: Write the failing test**

Create `tests/rust-gate.test.sh`. It is a suite in the repo's own harness.

> **Note, added after Task 1 shipped.** The code block below still uses
> `describe` and `assert_eq`, which do NOT exist. The implemented file uses
> the real names correctly. The block is left as written so the correction
> above it stays legible; take the names from the paragraph, not the block.

**The real helper names, verified in `tests/lib.sh`:** `assert_equals`
(:205), `assert_contains` (:218), `assert_succeeds` (:234), `skip` (:265),
`finish` (:322). There is **no `assert_eq` and no `describe`** despite what
an earlier draft of this plan said. All of them are description-first, so the
description is the FIRST argument, not the last. Read `tests/lib.sh` and copy
the header shape from an existing small suite such as
`tests/profile-path.test.sh`. The test code below uses the real names; if you
find a further discrepancy, the harness wins and you must say so.

```sh
#!/usr/bin/env bash
#
# Asserts the Rust gate is wired and that its skip path is loud.
#
# The gate exists because a push once printed "SKIP cargo test (cargo not
# found)" and passed: the container leg is Rust-free by design, so the
# container can never run these checks. A test that only asserted the
# checks pass would not catch the gate being removed from the hook, which
# is the failure that actually happened.

set -uo pipefail
# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"

describe "rust-checks.sh exists and is executable"
assert_eq "0" "$([ -x "$DOTFILES_ROOT/tests/rust-checks.sh" ] && echo 0 || echo 1)" \
    "tests/rust-checks.sh must exist and be executable"

describe "pre-push invokes the Rust checks"
# The hook is the only thing that makes these checks a gate rather than a
# script nobody runs. Asserted against the hook text because driving a real
# push from a test would push.
assert_contains "$(cat "$DOTFILES_ROOT/tests/pre-push")" "rust-checks.sh" \
    "tests/pre-push must call rust-checks.sh, or the gate is not a gate"

describe "the Rust checks run before the Docker handoff"
# Ordering matters: the Rust checks are seconds and the Docker suite is
# minutes, so a Rust failure must not wait behind a container build.
hook_text=$(cat "$DOTFILES_ROOT/tests/pre-push")
rust_line=$(printf '%s\n' "$hook_text" | grep -n 'rust-checks.sh' | head -1 | cut -d: -f1)
docker_line=$(printf '%s\n' "$hook_text" | grep -n 'run-in-docker.sh' | head -1 | cut -d: -f1)
assert_eq "0" "$([ "$rust_line" -lt "$docker_line" ] && echo 0 || echo 1)" \
    "rust-checks.sh must be invoked before run-in-docker.sh"

describe "the skip is loud when cargo is absent"
# PATH emptied so `command -v cargo` fails. The script must say so and
# still exit 0: a machine without Rust can still push shell changes.
skip_output=$(env PATH=/nonexistent "$DOTFILES_ROOT/tests/rust-checks.sh" HEAD 2>&1)
skip_status=$?
assert_eq "0" "$skip_status" "an absent cargo must not block the push"
assert_contains "$skip_output" "SKIP" \
    "an absent cargo must print SKIP, not pass silently"

finish
```

- [ ] **Step 2: Run the test to verify it fails**

```sh
cd ~ && bash tests/rust-gate.test.sh
```

Expected: FAIL on the first assertion, because `tests/rust-checks.sh` does
not exist. If instead it fails inside `lib.sh` on an unknown function, a
helper name differs from this plan: read `tests/lib.sh` and use the real
names. Record the actual output.

- [ ] **Step 3: Write `tests/rust-checks.sh`**

```sh
#!/bin/sh
#
# Runs the workspace's Rust checks against a ref, in a throwaway snapshot of
# that ref.
#
# Called by tests/pre-push before it hands off to Docker, and usable by hand
# as `tests/rust-checks.sh [REF]`.
#
# Why a snapshot rather than the working tree: $HOME is not always on the
# branch being pushed (a push from a worktree leaves $HOME on whatever
# branch it was already on), so checking the working tree would report a
# pass for code that is not being pushed. run-in-docker.sh solves the same
# hazard the same way.
#
# Why these checks are safe on the host, unlike the shell suites: they
# mutate nothing. deps-core and dotfiles-path are pure and deps-core
# enforces it with a purity test over its own sources; every config-manifest
# test builds its own temporary directory and passes explicit --git-dir and
# --work-tree. Verified by running the workspace with HOME pointed at an
# empty directory: the real home was unchanged and the fake home was still
# empty afterwards. This is NOT the host fallback tests/pre-push rejects: a
# fallback substitutes a weaker check when Docker is down, while this always
# runs and never replaces the container leg.

set -u

ref=${1:-HEAD}

GIT_DIR_PATH="$HOME/.cfg"
WORK_TREE_PATH="$HOME"

git_cmd() {
    git --git-dir="$GIT_DIR_PATH" --work-tree="$WORK_TREE_PATH" "$@"
}

# Loud, and exit 0. A machine with no Rust toolchain can still push a shell
# change, and a gate that stays silent when it skips is indistinguishable
# from one that is not installed.
if ! command -v cargo >/dev/null 2>&1; then
    printf 'SKIP  rust checks (cargo not found)\n'
    exit 0
fi

if ! git_cmd rev-parse --verify --quiet "$ref" >/dev/null 2>&1; then
    printf 'rust-checks: %s is not a ref in this repository.\n' "$ref" >&2
    exit 1
fi

# No crates at this ref means nothing to check. Asked before archiving so an
# older ref does not fail on an empty snapshot.
if ! git_cmd rev-parse --verify --quiet "$ref:crates/Cargo.toml" >/dev/null 2>&1; then
    printf 'rust-checks: %s carries no crates/Cargo.toml, nothing to check\n' "$ref"
    exit 0
fi

snapshot=$(mktemp -d "${TMPDIR:-/tmp}/rust-checks.XXXXXX") || {
    printf 'rust-checks: cannot create a snapshot directory\n' >&2
    exit 1
}
# Trapped on EXIT rather than removed at the end, so an early exit or a
# failing check still cleans up.
trap 'rm -rf "$snapshot"' EXIT

if ! git_cmd archive "$ref" crates | tar -x -C "$snapshot"; then
    printf 'rust-checks: cannot archive crates/ from %s\n' "$ref" >&2
    exit 1
fi

# The shared cache, not a fresh directory under the snapshot. Measured on
# this repo: a cold target directory takes over 120 seconds and the shared
# one takes 7, and a 120-second gate is one somebody switches off. Same
# default run-all.sh uses.
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/config-manifest/target}"
export CARGO_TARGET_DIR

# Run from inside crates/, never with --manifest-path: rustup honours
# crates/rust-toolchain.toml only when the working directory is under
# crates/, so a run from the snapshot root would declare the 1.94.0 pin
# without applying it. Verified that the archived tree carries
# rust-toolchain.toml and reports cargo 1.94.0 from inside crates/.
status=0

# git's environment is CLEARED before cargo runs, and this is load-bearing
# rather than defensive. Git exports GIT_DIR and GIT_WORK_TREE into every
# hook; config-manifest's tests build throwaway git repositories as
# fixtures; and with those variables set a fixture's `git add .` operates on
# the real dotfiles repo instead of the fixture. Observed exactly that: a
# fixture took ~/.cfg/index.lock and blocked every `config` command until the
# process was killed. tests/pre-push guards its Docker call the same way and
# says the same thing.
run_in_snapshot() {
    (
        cd "$snapshot/crates" || exit 1
        env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE -u GIT_PREFIX "$@"
    )
}

printf 'rust-checks: cargo test (%s)\n' "$ref"
if ! run_in_snapshot cargo test --locked --quiet; then
    printf 'rust-checks: cargo test failed\n' >&2
    status=1
fi

printf 'rust-checks: cargo clippy (%s)\n' "$ref"
if ! run_in_snapshot cargo clippy --locked --all-targets -- -D warnings; then
    printf 'rust-checks: cargo clippy failed\n' >&2
    status=1
fi

# Both checks run even when the first fails, so one push reports every
# problem rather than making the developer discover them one at a time.
exit "$status"
```

Then make it executable:

```sh
chmod +x ~/tests/rust-checks.sh
```

- [ ] **Step 4: Run the test to verify the first three assertions pass**

```sh
cd ~ && bash tests/rust-gate.test.sh
```

Expected: the `rust-checks.sh exists` and skip-path assertions PASS; the two
`pre-push` assertions still FAIL, because Task 2 has not wired the hook yet.
That partial state is correct at this step. Record which assertions passed.

- [ ] **Step 5: Verify the script actually checks the ref, not the tree**

This is the assertion that proves the archive is load-bearing. Break the
working tree, confirm the script still passes against committed `HEAD`, then
restore.

```sh
cd ~/crates
cp deps-core/src/lib.rs /tmp/lib.rs.good
printf '\ncompile error on purpose\n' >> deps-core/src/lib.rs
cd ~ && ./tests/rust-checks.sh HEAD; echo "archive_status=$?"
cd ~/crates && cargo clippy --locked --all-targets 2>&1 | grep -c "error" || true
cp /tmp/lib.rs.good deps-core/src/lib.rs && rm /tmp/lib.rs.good
cd ~ && ./tests/rust-checks.sh HEAD; echo "restored_status=$?"
```

Expected: `archive_status=0` even though the working tree is broken, because
the script checks committed `HEAD`; the bare `cargo clippy` in the middle
reports errors, proving the tree really was broken. `restored_status=0`.
Record both. If `archive_status` is 1, the script is reading the working
tree and the archive is not working; fix that before continuing.

- [ ] **Step 6: Verify the git-environment guard actually holds**

This is the step that proves the guard rather than trusting it. Run the
script with git's variables exported, the way a hook does, and confirm the
real repository's index is untouched afterwards.

```sh
cd ~
config rev-parse HEAD > /tmp/head.before
env GIT_DIR="$HOME/.cfg" GIT_WORK_TREE="$HOME" ./tests/rust-checks.sh HEAD
echo "guarded_status=$?"
ls ~/.cfg/index.lock 2>/dev/null && echo "LOCK LEAKED" || echo "no lock leaked"
config rev-parse HEAD > /tmp/head.after
diff /tmp/head.before /tmp/head.after && echo "HEAD unchanged"
test -z "$(config diff --cached --name-only)" && echo "index clean"
rm -f /tmp/head.before /tmp/head.after
```

Expected: `guarded_status=0`, `no lock leaked`, `HEAD unchanged`, `index
clean`. If `LOCK LEAKED` appears, the `env -u` guard is missing or
misplaced; fix it before continuing, and if a lock is present remove it with
`rm -f ~/.cfg/index.lock` after confirming with `ps aux | grep [g]it` that no
real git process is running.

- [ ] **Step 7: Verify shellcheck is clean**

Use the REPO's invocation, not a bare `shellcheck`. `tests/shellcheck.test.sh`
always supplies `-x` (follow sourced files) and
`-e "SC1091,SC2016"` (the two the repo suppresses deliberately), and without
them a sourced `lib.sh` produces a false-positive SC1091 that is not a real
finding:

```sh
cd ~ && shellcheck -x -e "SC1091,SC2016" tests/rust-checks.sh tests/rust-gate.test.sh
echo "shellcheck=$?"
```

Expected: `shellcheck=0`. Then confirm against the suite that actually gates
it, which is the authority:

```sh
cd ~ && bash tests/shellcheck.test.sh 2>&1 | tail -2
```

Expected: PASS. If the bare invocation and the suite disagree, the suite is
right.

- [ ] **Step 8: Commit**

```sh
cd ~
cat > /tmp/msg.txt <<'MSG'
Add tests/rust-checks.sh, which checks the pushed ref

The Rust checks had no local gate: pre-push runs the suite in Docker and
the test image's runtime stage is Rust-free by design, so cargo is
correctly absent there and the leg skipped. A push printed
"SKIP cargo test (cargo not found)" and passed.

Checks the pushed ref in a git archive snapshot rather than the working
tree, because $HOME is not always on the branch being pushed: a push from a
worktree leaves $HOME on whatever branch it was already on, so checking the
tree would report a pass for code nobody is pushing. run-in-docker.sh
solves the same hazard the same way.

Runs from inside the snapshot's crates/, never with --manifest-path,
because rustup honours crates/rust-toolchain.toml only when the working
directory is under crates/. Verified the archived tree carries the pin file
and reports cargo 1.94.0.

Uses the shared target cache. Measured: a cold target directory takes over
120 seconds and the shared one takes 7, and a two-minute gate is one
somebody switches off.

Both checks run even when the first fails, so one push reports every
problem instead of making the developer find them one at a time.

Clears GIT_DIR, GIT_WORK_TREE, GIT_INDEX_FILE and GIT_PREFIX before running
cargo. Git exports the first two into every hook, config-manifest's tests
build git fixtures, and with those set a fixture's `git add .` operates on
the real dotfiles repo: observed one taking ~/.cfg/index.lock and blocking
every config command until it was killed. pre-push already guards its Docker
call the same way.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add tests/rust-checks.sh tests/rust-gate.test.sh
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 2: Wire the checks into `tests/pre-push`

The script from Task 1 is not a gate until the hook calls it. It goes after
the stamp gate and **before** the Docker handoff, because the Rust checks
take seconds and the container suite takes minutes: a Rust failure must not
wait behind a container build.

**Files:**
- Modify: `tests/pre-push`, inserting between the stamp gate's closing
  `fi` (around `:205`) and the `# --- dotfiles test suite` banner
  (around `:207`)

**Interfaces:**
- Consumes: `tests/rust-checks.sh [REF]` from Task 1, exit 0 on pass or loud
  skip, 1 on failure.
- Produces: nothing later tasks consume. Task 5's suite asserts this wiring.

- [ ] **Step 1: Read the insertion point**

```sh
cd ~ && sed -n '195,215p' tests/pre-push
```

Confirm you can see the end of the stamp-gate block (`printf 'pre-push:
stamp gate passed\n'` then `fi`) and the start of the test-suite block. Your
insertion goes between them. Record the real line numbers, since the file may
have shifted.

- [ ] **Step 2: Insert the block**

The Rust checks run for every pushed ref, like the stamp gate, rather than
only the first: two pushed refs can hold different crate trees, and checking
one would let the other through.

```sh
# --- Rust checks ----------------------------------------------------------
#
# Runs on the HOST, before the Docker handoff below, and this is deliberate.
#
# The container leg cannot run these: tests/docker/Dockerfile's runtime
# stage is Rust-free by design, so `command -v cargo` is correctly false
# inside it and run-all.sh skips the Rust leg. That is how a push once
# printed "SKIP cargo test (cargo not found)" and passed.
#
# This is NOT the host fallback the suite block below rejects. A fallback
# silently substitutes a weaker check when the daemon is down; this always
# runs and never replaces the container leg. The distinction matters because
# the reason the shell suites need the container -- they mutate $HOME, spawn
# tmux sessions, write fixture repos -- does not apply to cargo test. Those
# crates are pure or test against their own temporary directories, verified
# by running the workspace with HOME pointed at an empty directory and
# finding it still empty afterwards.
#
# Before the Docker block because these take seconds and that takes minutes.

if [ ! -x "$HOME/tests/rust-checks.sh" ]; then
    printf 'pre-push: ~/tests/rust-checks.sh is missing or not executable\n' >&2
    exit 1
fi

for ref in $push_refs; do
    if ! "$HOME/tests/rust-checks.sh" "$ref"; then
        printf '\npre-push: Rust checks failed for %s, push blocked.\n' "$ref" >&2
        exit 1
    fi
done

printf 'pre-push: Rust checks passed\n'
```

- [ ] **Step 3: Run the wiring test to verify it now passes**

```sh
cd ~ && bash tests/rust-gate.test.sh
```

Expected: PASS, all assertions, including the two `pre-push` ones that failed
at Task 1 Step 4 and the ordering assertion.

- [ ] **Step 4: Verify the gate actually blocks a push**

Drive the hook directly with git's pre-push stdin protocol rather than
pushing. Break the committed tree in a scratch commit, confirm the hook
refuses, then reset.

```sh
cd ~/crates
cp deps-core/src/lib.rs /tmp/lib.rs.good
printf '\nthis is not valid rust\n' >> deps-core/src/lib.rs
cd ~
config add crates/deps-core/src/lib.rs
config commit -F /dev/stdin <<'MSG'
TEMPORARY: break the crate to prove the gate blocks
MSG
broken=$(config rev-parse HEAD)
printf 'refs/heads/main %s refs/heads/main %s\n' "$broken" "$(config rev-parse HEAD~1)" \
    | ./tests/pre-push origin https://example.invalid/repo.git
echo "hook_status=$?"
```

Expected: nonzero `hook_status`, with `pre-push: Rust checks failed`. That is
the gate working. Then undo the scratch commit:

```sh
cd ~ && config reset --soft HEAD~1
cp /tmp/lib.rs.good crates/deps-core/src/lib.rs && rm /tmp/lib.rs.good
config restore --staged crates/deps-core/src/lib.rs 2>/dev/null || true
cd ~/crates && cargo clippy --locked --all-targets -- -D warnings; echo "clean=$?"
```

Expected: `clean=0` and `config log --oneline -1` no longer shows the
TEMPORARY commit. **`config reset --soft` is the only history rewrite this
plan authorizes, it is undoing a commit you just made in this step, and the
`--soft` keeps the content.** Record both observations.

- [ ] **Step 5: Verify a clean push passes the gate**

```sh
cd ~
head=$(config rev-parse HEAD)
printf 'refs/heads/main %s refs/heads/main %s\n' "$head" "$(config rev-parse HEAD~1)" \
    | ./tests/pre-push origin https://example.invalid/repo.git 2>&1 | grep -E 'rust|Rust'
```

Expected: `rust-checks: cargo test`, `rust-checks: cargo clippy`, and
`pre-push: Rust checks passed`. The Docker leg may then run or fail on the
daemon; that is outside this step.

- [ ] **Step 6: Commit**

```sh
cd ~
cat > /tmp/msg.txt <<'MSG'
Make the Rust checks a pre-push gate

rust-checks.sh was a script nobody ran. The hook now calls it for every
pushed ref, before the Docker handoff.

Every ref rather than the first, matching the stamp gate: two pushed refs
can hold different crate trees, so checking one would let the other
through.

Before the Docker block because the Rust checks take seconds and the
container suite takes minutes, and a Rust failure should not wait behind a
container build.

The comment says at length why this is not the host fallback the suite
block rejects, because the two look identical and are opposites: a fallback
substitutes a weaker check when the daemon is down, while this always runs
and never replaces the container leg.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add tests/pre-push
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 3: A clippy leg in `tests/run-all.sh`

`run-all.sh` runs `cargo test` when cargo is present. It never runs clippy.
Adding the leg here means the suite reports clippy like any other suite, on
the host and in CI, and it will start working inside the container for free
whenever a later step puts a toolchain there.

**Files:**
- Modify: `tests/run-all.sh:197-201` (the `cargo_count` probe) and
  `:228-246` (the Rust leg)

**Interfaces:**
- Consumes: nothing from earlier tasks. Independent of Tasks 1 and 2, which
  is why it is its own task.
- Produces: one more counted suite named `cargo clippy in crates/`.

- [ ] **Step 1: Write the failing test**

Append to `tests/rust-gate.test.sh` from Task 1:

```sh
runner_text=$(cat "$DOTFILES_ROOT/tests/run-all.sh")
assert_contains "run-all.sh must run clippy, or the invariant has no gate in CI" \
    "$runner_text" "cargo clippy"
# Counted, not a bare command: an uncounted check does not appear in the
# suite total, so its absence is invisible in the summary line.
assert_contains "the clippy leg must go through run_suite so it is counted and timed" \
    "$runner_text" 'run_suite "cargo clippy in crates/"'
```

- [ ] **Step 2: Run the test to verify it fails**

```sh
cd ~ && bash tests/rust-gate.test.sh
```

Expected: the two new assertions FAIL, earlier ones still pass. Record it.

- [ ] **Step 3: Widen the suite count**

`run-all.sh:199-202` counts one Rust suite. It becomes two. Read the real
lines first, then replace the `cargo_count=1` assignment:

```sh
cargo_count=0
if [ -z "$only" ] && command -v cargo >/dev/null 2>&1 && [ -f "$cargo_manifest" ]; then
    # Two legs: test and clippy. Counted separately because run_suite counts
    # per call and the summary line must match what actually ran.
    cargo_count=2
fi
```

- [ ] **Step 4: Add the clippy leg**

In the `if [ "$cargo_count" -eq 1 ]` block at `:232`, change the condition to
`-ge 1` (or `-eq 2`, matching whatever the real code reads after Step 3) and
add the clippy call after the existing `cargo test` call, inside the same
block so both share the `CARGO_TARGET_DIR` export:

```sh
    # -D warnings here is the enforcement of the policy crates/Cargo.toml
    # declares, not the policy itself. Task 4 moves the lint set into the
    # manifest so an IDE and a bare `cargo clippy` agree with this gate.
    #
    # From inside crates/ for the same reason as the test leg: rustup honours
    # crates/rust-toolchain.toml only when the working directory is under
    # crates/.
    run_suite "cargo clippy in crates/" \
        sh -c 'cd "$1" && cargo clippy --locked --all-targets -- -D warnings' _ "$cargo_dir"
```

Update the `-eq 1` guard and the `SKIP` message's wording if it names only
`cargo test`, so a skip reports both legs honestly.

- [ ] **Step 5: Run the suite and the test**

```sh
cd ~ && bash tests/rust-gate.test.sh
cd ~ && bash tests/run-all.sh 2>&1 | tail -4
```

Expected: `rust-gate.test.sh` PASSES fully. `run-all.sh` reports one more
suite than before and `all N suite(s) passed`. Record the suite count before
and after.

- [ ] **Step 6: Verify the clippy leg actually fails the suite**

```sh
cd ~/crates
cp deps-core/src/lib.rs /tmp/lib.rs.good
# An unused import is a warning, not an error, so it proves -D warnings is
# what fails the leg rather than a compile error failing the test leg too.
printf '\nuse std::collections::BTreeMap;\n' >> deps-core/src/lib.rs
cd ~ && bash tests/run-all.sh 2>&1 | grep -E "cargo clippy|suite\(s\) failed" | head -3
cp /tmp/lib.rs.good crates/deps-core/src/lib.rs && rm /tmp/lib.rs.good
cd ~ && bash tests/run-all.sh 2>&1 | tail -2
```

Expected: the sabotaged run shows the clippy suite FAILing while the cargo
test leg still passes, proving the new leg is what caught it; the restored
run is green. Record both. (`std::collections` is not on `deps-core`'s
forbidden-path list, so this sabotage does not trip the purity test and
confuse the result.)

- [ ] **Step 7: Commit**

```sh
cd ~
cat > /tmp/msg.txt <<'MSG'
Run clippy as a counted suite in run-all.sh

cargo clippy had no gate anywhere in this repo: zero hits across .github,
.scripts and tests outside prose. The deps-core completion treated
clippy at 0 errors as a standing invariant and held it by hand across
seven tasks, and two implementers tripped it in tasks whose briefs did not
predict it. An invariant no gate enforces is a convention.

Counted through run_suite rather than run as a bare command, because an
uncounted check is absent from the summary line and its absence is
invisible.

Runs from inside crates/ for the same reason as the test leg: rustup
honours crates/rust-toolchain.toml only when the working directory is under
crates/, and a run from the repo root declares the 1.94.0 pin without
applying it.

This also reaches CI, which runs run-all.sh on the runner with cargo
preinstalled, so the invariant is enforced before merge rather than only
before push.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add tests/run-all.sh tests/rust-gate.test.sh
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 4: Declare the lint policy in `crates/Cargo.toml`

The gates from Tasks 2 and 3 enforce `-D warnings`, but nothing says which
lints are on. A command line binds one invocation, so an IDE, a bare
`cargo clippy`, and a teammate's terminal all disagree with the gate.
`[workspace.lints]` puts the policy where every invocation reads it.

**The set below is measured, not chosen.** A trial of `clippy::all` +
`clippy::pedantic` + `rust_2018_idioms` + `missing_docs` + the panic family
over this workspace produced **315 warnings with `--all-targets`**:

| Warning | Count | Disposition |
|---|---|---|
| `expect`/`unwrap`/`panic` | 126, of which **123 are test code** | Not at workspace level; CLAUDE.md permits them in tests |
| `missing_docs` on public variants and fields | 68 | Adopt; CLAUDE.md already requires public-item docs |
| `must_use_candidate` | 41 | Reject as `pedantic` noise |
| `missing_errors_doc` | 5 | Adopt; already required |
| `format_push_string`, `doc_markdown`, `redundant_closure`, `match_same_arms`, `items_after_statements` | ~30 | Adopt, all mechanical |

The three non-test `expect()` calls are `writeln!` into a `String` at
`config-manifest/src/doctor.rs:153,157,167`, which cannot fail. Those are
CLAUDE.md's proven-invariant exception, so `expect_used` at deny would force
an `#[allow]` onto correct code.

**Adoption is staged inside this task**: the lint set lands at `warn`, the
68 missing-doc warnings get fixed, and only then does the count reach zero.
The gates already deny warnings, so landing the set red would break Task 3's
suite. Fix the docs in the same task, because a half-adopted lint set is one
somebody reverts.

**Files:**
- Modify: `crates/Cargo.toml`
- Modify: `crates/config-manifest/Cargo.toml`,
  `crates/deps-core/Cargo.toml`, `crates/dotfiles-path/Cargo.toml`
- Modify: `crates/deps-core/src/lib.rs`,
  `crates/dotfiles-path/src/lib.rs`, `crates/config-manifest/src/main.rs`
  (one `cfg_attr` line each)
- Modify: whichever `crates/*/src/*.rs` files carry the missing docs

**Interfaces:**
- Consumes: Task 3's clippy leg, which is what makes this enforceable.
- Produces: a workspace whose lint policy travels with the code.

- [ ] **Step 1: Measure the real starting count**

Before changing anything, record the baseline so you can tell progress from
noise:

```sh
cd ~/crates && cargo clippy --locked --all-targets -- -D warnings 2>&1 | tail -2
```

Expected: clean, 0 errors. That is today's state with no lint config.

- [ ] **Step 2: Add the workspace lint tables**

In `crates/Cargo.toml`, insert **before** `[workspace.dependencies]`:

```toml
# Lint policy lives here rather than in a command line, because a command
# line binds one invocation: an IDE, a bare `cargo clippy`, and CI would all
# disagree with the gate. The gates' `-D warnings` enforces this policy; it
# is not the policy.
#
# Measured before adopting. clippy::pedantic wholesale produced 41
# must_use_candidate warnings on builders already used positionally, which is
# noise, so pedantic is not enabled as a group.
[workspace.lints.rust]
# forbid, not deny: forbid cannot be lifted by a local #[allow]. Nothing in
# these crates needs unsafe, and deps-core's whole design argument is that it
# holds no capabilities.
unsafe_code = "forbid"
missing_docs = "warn"
unused_qualifications = "warn"
rust_2018_idioms = { level = "warn", priority = -1 }

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
missing_errors_doc = "warn"
format_push_string = "warn"
```

Then in each of the three member manifests, append:

```toml

[lints]
workspace = true
```

- [ ] **Step 3: Run clippy and count what the policy surfaces**

```sh
cd ~/crates && cargo clippy --locked --all-targets 2>&1 \
    | grep -E "^warning" | grep -oE ": .*" | sed 's/^: //' | sort | uniq -c | sort -rn | head -15
```

Expected: roughly 68 missing-doc warnings plus a handful of others, and NO
`must_use_candidate` (that lint is in `pedantic`, which you did not enable).
Record the real breakdown. If `must_use_candidate` appears, `pedantic` got
enabled by accident; remove it.

- [ ] **Step 4: Add the panic-family carve-out**

Cargo cannot express "deny in `src`, allow in `#[cfg(test)]`" through
`[workspace.lints]`, so this goes per crate. Add as the FIRST line of
`crates/deps-core/src/lib.rs`, `crates/dotfiles-path/src/lib.rs`, and
`crates/config-manifest/src/main.rs`:

```rust
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
```

Verified working before this plan was written: with that line on
`deps-core`, clippy reports zero `unwrap_used`/`expect_used` hits and all 77
tests still pass, because test code is exempt exactly where CLAUDE.md permits
the construct.

`config-manifest` will report its three `doctor.rs` `expect()` calls, which
are `writeln!` into a `String` and cannot fail. Do NOT add an `#[allow]`
there and do NOT weaken the lint: rewrite those three call sites to use
`let _ = writeln!(...)` or `core::fmt::Write` without the `expect`, whichever
reads better, and say which you chose and why in your report. If neither is
clean, an `#[allow]` with a one-line why-comment naming the proven invariant
is acceptable as a last resort; say so explicitly.

- [ ] **Step 5: Fix the missing docs**

Add the missing doc comments the policy surfaced. These are real gaps against
a rule CLAUDE.md already states, so write real documentation, not
placeholders: say what the variant or field MEANS, not what its name already
says. A doc comment reading `/// The name.` on a field called `name` is a
plan failure.

Work crate by crate and re-run after each:

```sh
cd ~/crates && cargo clippy --locked --all-targets -p deps-core 2>&1 | grep -c "^warning"
cd ~/crates && cargo clippy --locked --all-targets -p dotfiles-path 2>&1 | grep -c "^warning"
cd ~/crates && cargo clippy --locked --all-targets -p config-manifest 2>&1 | grep -c "^warning"
```

Drive each to 0. `deps-core` doc comments must not name any of `std::fs`,
`std::process`, `std::env`, `std::io`, or `Command::new` as a literal: that
crate's purity test scans its own sources, doc comments included, and a
literal makes it fail its own check.

- [ ] **Step 6: Confirm the gate is green and the tests still pass**

```sh
cd ~/crates && cargo clippy --locked --all-targets -- -D warnings; echo "clippy=$?"
cd ~/crates && cargo test --locked 2>&1 | grep -cE "^test result: ok"
cd ~/crates && cargo test --locked 2>&1 | grep -cE "FAILED"
```

Expected: `clippy=0`, 9 or more `test result: ok` lines, 0 `FAILED`.

- [ ] **Step 7: Confirm the policy binds a bare invocation**

The whole point of the manifest is that it applies without the gate's flags:

```sh
cd ~/crates && cargo clippy --locked --all-targets 2>&1 | grep -c "^warning"
```

Expected: 0. If a bare run reports warnings while `-D warnings` passes, the
lints are on the command line rather than in the manifest.

- [ ] **Step 8: Verify the lint policy actually catches something**

```sh
cd ~/crates
cp deps-core/src/action.rs /tmp/action.rs.good
python3 - <<'PY'
import pathlib, re
path = pathlib.Path("deps-core/src/action.rs")
text = path.read_text()
# Strip one public variant's doc comment: missing_docs must catch it.
marker = "    /// The manager packages it under this name.\n"
assert text.count(marker) == 1
path.write_text(text.replace(marker, ""))
PY
cargo clippy --locked --all-targets 2>&1 | grep -c "missing documentation"
cp /tmp/action.rs.good deps-core/src/action.rs && rm /tmp/action.rs.good
cargo clippy --locked --all-targets -- -D warnings; echo "restored=$?"
```

Expected: the sabotaged run reports at least 1 missing-documentation warning;
`restored=0`. That proves `missing_docs` is live rather than declared.

- [ ] **Step 9: Rebuild the stamp and commit**

Any change under `crates/` moves the build stamp, and the live pre-push gate
blocks a push whose installed binary is stale.

```sh
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
```

Expected: `doctor=0`, no output.

```sh
cd ~
cat > /tmp/msg.txt <<'MSG'
Declare the lint policy in the manifest, not in a command line

The deps-core completion held "clippy at 0 errors" as an invariant across
seven tasks, by hand. A command line binds one invocation, so an IDE, a
bare cargo clippy, and CI all disagreed with the gate. [workspace.lints]
puts the policy where every invocation reads it, and the gates' -D warnings
becomes enforcement rather than the policy itself.

The set is measured, not chosen. A trial of clippy::all plus pedantic plus
rust_2018_idioms plus missing_docs plus the panic family produced 315
warnings, and the breakdown decided the design:

- 123 of 126 expect/unwrap/panic hits are test code, where CLAUDE.md
  permits them, so the panic family is scoped per crate with
  cfg_attr(not(test), deny(...)) rather than set workspace-wide. Cargo
  cannot express "deny in src, allow in cfg(test)" through workspace lints.
- must_use_candidate is 41 warnings of pedantic noise on builders already
  used positionally, so pedantic is not enabled as a group.
- The 68 missing_docs and 5 missing_errors_doc hits are real gaps against
  rules CLAUDE.md already states, and are fixed here rather than allowed.

unsafe_code is forbid rather than deny so a local #[allow] cannot lift it.
Nothing in these crates needs unsafe, and deps-core's design argument is
that it holds no capabilities.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add crates/
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 5: The clippy step in CI

CI runs `run-all.sh` on the runner with cargo preinstalled, so Task 3's leg
already reaches CI. This task makes the check explicit and fast-failing in
the workflow, so a clippy failure is legible in the Actions UI rather than
buried in a suite's captured output.

**Files:**
- Modify: `.github/workflows/test-suite.yml`, after the existing
  `Build config-manifest` step (around `:119-124`)

**Interfaces:**
- Consumes: Task 4's lint policy, which is what makes the step meaningful.
- Produces: nothing later tasks consume.

- [ ] **Step 1: Write the failing test**

`tests/deps-harness.test.sh` already parses workflow YAML with `python3-yaml`
rather than grepping, which is the pattern to follow. Append to
`tests/rust-gate.test.sh`:

```sh
# Parsed rather than grepped, matching deps-harness.test.sh.
# Parsed rather than grepped, matching deps-harness.test.sh: a reformat of
# the workflow must not produce a false pass or a false failure.
ci_has_clippy=$(python3 - "$DOTFILES_ROOT/.github/workflows/test-suite.yml" <<'PY'
import sys, yaml
with open(sys.argv[1]) as handle:
    workflow = yaml.safe_load(handle)
steps = [
    step
    for job in workflow["jobs"].values()
    for step in job.get("steps", [])
]
found = any("cargo clippy" in str(step.get("run", "")) for step in steps)
print("0" if found else "1")
PY
)
assert_equals "test-suite.yml must run cargo clippy, so the invariant gates before merge" \
    "0" "$ci_has_clippy"
```

- [ ] **Step 2: Run the test to verify it fails**

```sh
cd ~ && bash tests/rust-gate.test.sh
```

Expected: the CI assertion FAILS (`1` != `0`). If it fails on a Python
`KeyError`, the workflow's shape differs from this plan; read the real file
and adjust the traversal. Record the output.

- [ ] **Step 3: Add the step**

After the `Build config-manifest` step, matching its `working-directory`
idiom:

```yaml
      # The lint policy lives in crates/Cargo.toml, so this step enforces it
      # rather than defining it. Explicit here as well as inside run-all.sh
      # so a lint failure is legible in the Actions UI instead of buried in a
      # suite's captured output, and so it fails before the slower suites.
      #
      # working-directory rather than --manifest-path, for the reason the
      # build step above already gives: rustup honours
      # crates/rust-toolchain.toml only when the working directory is under
      # crates/.
      - name: Clippy
        working-directory: crates
        run: cargo clippy --locked --all-targets -- -D warnings
```

- [ ] **Step 4: Run the test to verify it passes**

```sh
cd ~ && bash tests/rust-gate.test.sh
```

Expected: PASS, every assertion.

- [ ] **Step 5: Validate the YAML parses and the whole suite is green**

```sh
cd ~ && python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/test-suite.yml')); print('yaml ok')"
cd ~ && bash tests/run-all.sh 2>&1 | tail -3
```

Expected: `yaml ok`, and `all N suite(s) passed`.

- [ ] **Step 6: Commit and push**

```sh
cd ~
cat > /tmp/msg.txt <<'MSG'
Gate clippy in CI as well as pre-push

run-all.sh's clippy leg already reaches CI, which runs the suite on the
runner with cargo preinstalled. This makes it an explicit workflow step so
a lint failure is legible in the Actions UI rather than buried in a suite's
captured output, and so it fails before the slower suites.

working-directory rather than --manifest-path, for the reason the build
step above it already gives: rustup honours crates/rust-toolchain.toml only
when the working directory is under crates/, and building from the repo
root once used the runner's default toolchain instead of the pinned
1.94.0.

The assertion for this is parsed from the YAML rather than grepped, which
is what deps-harness.test.sh already does, so a reformat cannot produce a
false result.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add .github/workflows/test-suite.yml tests/rust-gate.test.sh
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
config push origin main
```

Expected: the push runs the gate you just built. `pre-push: Rust checks
passed` must appear. If it does not, Task 2 is not wired.

---

## Self-Review

**Spec coverage.** Section 4a's numbered decisions map to tasks: 4a.1 step 1
(host checks in pre-push) to Tasks 1 and 2; 4a.1 step 2 (leave the runtime
stage Rust-free) is a non-action, honored by no task touching
`tests/docker/Dockerfile`; 4a.1 step 3 (same checks in CI) to Tasks 3 and 5;
4a.2 (the lint policy and its measured set) to Task 4. The
run-from-inside-`crates/` constraint appears in Tasks 1, 3, 4 and 5, since
each has its own invocation.

**Placeholder scan.** No TBD, no "similar to Task N", no "add error
handling". Every code step carries real code. Task 4 Step 5 deliberately
does not enumerate all 68 doc comments, because their content depends on the
items they document; it states the standard ("say what it MEANS") and a
mechanical exit condition (each crate's warning count reaches 0) instead of
inventing 68 sentences for an implementer to paste.

**Type consistency.** `tests/rust-checks.sh [REF]` has one signature, used
identically in Task 2's loop and Task 1's tests. `run_suite "<name>" <cmd>`
matches the real helper at `run-all.sh:92`. `$push_refs` in Task 2 is the
variable `tests/pre-push` already builds. `cargo_count` becomes 2 in Task 3
Step 3 and the guard in Step 4 is told to match whatever Step 3 produced.

**One risk worth naming.** Task 4 Step 4 may find the three `doctor.rs`
`expect()` calls harder to rewrite than expected. The step says what to do in
that case (an `#[allow]` with a why-comment, declared explicitly) rather than
leaving the implementer to improvise or silently weaken the lint.

**Ordering.** Tasks 1-2 (the local gate) and Task 3 (the runner leg) are
independent and could run in either order. Task 4 must follow Task 3, because
landing a lint policy with no gate is what this plan exists to stop. Task 5
must follow Task 4, or CI goes red on the 68 doc warnings before they are
fixed.
