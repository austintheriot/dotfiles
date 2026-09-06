# Blockers and Build Infrastructure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the four verified fail-open blockers in the leak guard and test
harness, then build the workspace and per-crate stamp infrastructure the Rust
migration needs, without adding any migrated logic yet.

**Architecture:** Two halves. Tasks 1-5 fix gates that currently pass silently;
each separates the IO from the decision it was fused to, which is both the fix
and what makes it testable. Tasks 6-10 convert `crates/` into a cargo workspace
with per-crate stamps, pin the toolchain, and add `config doctor`, all against
the one crate that already exists. No new domain logic lands here.

**Tech Stack:** zsh and POSIX sh (`tests/`, `.scripts/`), bash (test harness),
Rust 2024 with clap + anyhow (`crates/`), git plumbing (`write-tree`,
`rev-parse`), Docker (the pre-push suite).

**Spec:** `docs/superpowers/specs/2026-09-06-rust-migration-design.md`

## Global Constraints

- No em dashes anywhere. Use a comma, a colon, parentheses, or two hyphens.
- No emoji anywhere.
- No single-letter variable names, except numeric loop indices (`i`, `j`, `k`)
  and math or geometry values. Lambda parameters get no exception.
- Comment why, not what. No comment that restates the code.
- Never `--no-verify`. Never disable a test instead of fixing it.
- Never the TypeScript non-null assertion. Not applicable here, listed because
  it is a global rule.
- Pure core, IO at the edges, dependency-injectable. In Rust: the five modules
  `manifest`, `check`, `plan`, `path`, `tree` currently contain **zero**
  references to `std::fs`, `std::process`, `std::env`, `std::io`, or
  `Command::new` (verified). All 37 IO references live in `git.rs` and
  `main.rs`. New code holds that line: decisions are pure functions over
  values, IO only gathers inputs and writes outputs.
- In shell: a function that performs IO must not also decide. It writes to a
  file or stdout and returns a status; the caller reads both. This is the
  direct cause of two of the four blockers.
- Tests inject through existing env seams (`LEAK_PATTERN_FILE`,
  `LEAK_ALLOW_FILE`, `DOTFILES_CONF`, `DOTFILES_ROOT`, `CONFIG_BIN_DIR`,
  `CARGO_TARGET_DIR`). Never read `~/.claude/local/` from a test.
- `tests/leak-check.sh` is `#!/bin/zsh`, so arrays and `pipefail` are
  available there. `setup.sh`, `.scripts/platform.sh`, and
  `.scripts/deps/check-deps.sh` must stay POSIX sh.
- Every task ends green: `~/tests/run-all.sh` passes before the commit.
- Any new or renamed path must match `TRIGGER_PATHS` in `tests/pre-push:38`,
  or the suite that reads it will not run at pre-push.

---

## File Structure

**Modified, Step 0 (blockers):**

| File | Responsibility after this plan |
|---|---|
| `tests/leak-check.sh` | Scan decisions separated from git IO. Fails closed on a failed scan, a missing pattern file, and a path that produced no hunk. |
| `tests/leak-check.test.sh` | Adds the three fail-open regression tests, plus the `SKIP_LEAK_CHECK` truthiness test. |
| `tests/lib.sh` | `finish` fails on zero assertions. `assert_succeeds` reports the real exit code. |
| `tests/githooks-installed.test.sh` | Uses `skip` rather than a bare `printf`, so its absence is counted. |
| Five more `*.test.sh` | Same bare-`printf` fix. |
| `tests/skip-reporting.test.sh` | Adds the zero-assertion verdict test. |

**Modified, Step 2 (infrastructure):**

| File | Responsibility after this plan |
|---|---|
| `crates/Cargo.toml` | New. Workspace root, members list. |
| `crates/Cargo.lock` | New. Shared resolution, moved from the crate. |
| `crates/rust-toolchain.toml` | New. Exact toolchain pin. |
| `crates/config-manifest/src/stamp.rs` | New. Pure stamp folding. No IO. |
| `crates/config-manifest/src/doctor.rs` | New. Pure staleness diagnosis and render. No IO. |
| `crates/config-manifest/src/git.rs` | Gains a `GitRepo` trait so policy above it is testable without a repo. |
| `.scripts/config/config-stamp` | Emits per-crate stamps. |
| `.scripts/config/config-build` | Builds every workspace crate, re-stamps all. |
| `.scripts/config/config-doctor` | New. Wrapper for the doctor subcommand. |
| `tests/pre-push` | Iterates crates rather than checking one. |
| `.claude/rules/dotfiles-tests.md` | Documents the edit-build-test loop. |

---

## Task 1: `leak-check.sh` fails closed when the scan itself fails

The blocker: `added_lines` detects a `git log` failure and calls `exit 2`, but
it runs only inside `$(...)`, so the exit kills the subshell. The parent
captures an empty string, takes the `[ -z "$staged" ] && exit 0` branch, and
reports a clean scan. `changed_paths` has the same shape. Verified by
execution: parent survives, capture empty, final exit 0.

This is an IO-and-decision fusion. The fix separates them.

**Files:**
- Modify: `tests/leak-check.sh:94-121` (both functions), `:129-133` (both callers)
- Test: `tests/leak-check.test.sh`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `SCAN_FAILED` sentinel file path, set beside `TMP_OUT`. Later tasks
  in this file read the same convention. Exit status 2 continues to mean
  "could not scan" to `tests/pre-push:96`.

- [ ] **Step 1: Write the failing test**

Append to `tests/leak-check.test.sh`, before the `finish` call:

```bash
# A git failure mid-scan must block, not pass. The functions that read git run
# inside a command substitution, so an `exit` inside them cannot reach the
# parent; the parent has to observe the failure some other way. Without that,
# the guard prints its own diagnostic and then exits 0, which is the exact
# inversion this asserts against.
shim_dir="$FIXTURES/git-shim-fail"
mkdir -p "$shim_dir"
cat > "$shim_dir/git" <<'SHIM'
#!/bin/sh
# Fail only the `-p` invocation added_lines makes; every other git call works,
# so the range resolves and the path list is produced normally.
for arg in "$@"; do
    if [ "$arg" = "-p" ]; then
        printf 'simulated git failure\n' >&2
        exit 128
    fi
done
exec /usr/bin/git "$@"
SHIM
chmod 755 "$shim_dir/git"

printf 'token = ghp_%s\n' "$(printf 'A%.0s' $(seq 1 24))" > "$repo/planted.txt"
git -C "$repo" add planted.txt
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m 'plant'

output=$(cd "$repo" && PATH="$shim_dir:$PATH" \
    "$LEAK_CHECK" --range 'HEAD~1..HEAD' 2>&1)
status=$?
assert_equals 'a git failure mid-scan exits 2, not 0' '2' "$status"
assert_contains 'the failure names the range' "$output" 'HEAD~1..HEAD'
```

- [ ] **Step 2: Run test to verify it fails**

Run: `~/tests/leak-check.test.sh`
Expected: FAIL on `a git failure mid-scan exits 2, not 0`, reporting `0`
against the expected `2`.

- [ ] **Step 3: Write minimal implementation**

In `tests/leak-check.sh`, add the sentinel beside the existing temp files.
Replace lines 89-91:

```zsh
TMP_OUT=$(mktemp) || exit 2
TMP_ERR=$(mktemp) || exit 2
# A scan failure is detected inside a function that only ever runs in a
# command substitution, and a subshell cannot exit its parent. The sentinel is
# how the failure crosses that boundary: the function creates it, the parent
# checks it before trusting an empty result.
SCAN_FAILED=$(mktemp) || exit 2
rm -f "$SCAN_FAILED"
trap 'rm -f "$TMP_OUT" "$TMP_ERR" "$SCAN_FAILED"' EXIT
```

Replace the `exit 2` in `changed_paths` (line 99) and in `added_lines`
(line 116) with a sentinel write. In `changed_paths`:

```zsh
    if ! git log --format= --name-only "${RANGE_LOG_FLAGS[@]}" "$range" > "$TMP_OUT" 2>"$TMP_ERR"; then
      echo "leak-check: git log failed scanning changed paths for $range" >&2
      cat "$TMP_ERR" >&2
      : > "$SCAN_FAILED"
      return 2
    fi
```

In `added_lines`, also fix the `$?`-after-redirect problem while here: the
status being tested is `xargs`'s, read after an intervening redirect. Use the
pipeline directly as the condition:

```zsh
  if [ "$mode" = push ]; then
    if ! tr '\n' '\0' | xargs -0 git log --format= --no-color -U0 "${RANGE_LOG_FLAGS[@]}" -p "$range" -- \
        > "$TMP_OUT" 2>"$TMP_ERR"; then
      echo "leak-check: git log failed scanning added lines for $range" >&2
      cat "$TMP_ERR" >&2
      : > "$SCAN_FAILED"
      return 2
    fi
    grep '^+' "$TMP_OUT" | grep -v '^+++'
```

Then make both callers check the sentinel before trusting an empty capture.
Replace lines 129-133:

```zsh
scan_paths=$(changed_paths | grep -v '^tests/leak-check\.sh$')
[ -f "$SCAN_FAILED" ] && exit 2
[ -z "$scan_paths" ] && exit 0

staged=$(echo "$scan_paths" | added_lines)
[ -f "$SCAN_FAILED" ] && exit 2
[ -z "$staged" ] && exit 0
```

- [ ] **Step 4: Run test to verify it passes**

Run: `~/tests/leak-check.test.sh`
Expected: PASS, including the two new assertions.

Then confirm nothing regressed:

Run: `~/tests/run-all.sh leak-check`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
config add tests/leak-check.sh tests/leak-check.test.sh
config commit -m "Make the leak guard fail closed when its own scan fails

added_lines and changed_paths detect a git failure and called exit 2, but
both run only inside a command substitution, so the exit killed the subshell
and the parent read an empty capture as \"nothing to scan\" and exited 0. The
guard printed the correct diagnostic and then passed the push.

A subshell cannot exit its parent, so the failure now crosses that boundary
through a sentinel file the parent checks before trusting an empty result.
tests/pre-push:96 already treats status 2 as \"could not scan, push blocked\";
that branch was unreachable from this path.

Also reads the pipeline status directly rather than \$? after an intervening
redirect, which was reporting xargs's status."
```

---

## Task 2: `leak-check.sh` fails closed when the pattern file is missing

The blocker: when `~/.claude/local/leak-patterns.conf` is unreadable, the guard
prints "term rules INACTIVE" to stderr and continues. Layer 1 (credential
shapes) still runs, so an AWS-shaped key is still caught. What deactivates is
Layer 2, the employer and project terms, which is the layer that exists because
this repo is public. Verified by differential test on identical content:
present exits 1, absent exits 0.

The file is untracked by design, so it is absent by default on every fresh
machine, which is exactly when setup work is being committed.

**Files:**
- Modify: `tests/leak-check.sh:167-172`
- Test: `tests/leak-check.test.sh`

**Interfaces:**
- Consumes: the `SCAN_FAILED` convention from Task 1 (not used here, but the
  trap line is shared, so this task edits a file Task 1 already changed).
- Produces: `LEAK_ALLOW_NO_PATTERNS` env seam. Set to `1` to permit a run with
  no pattern file. Tests use it to exercise the permitted path.

- [ ] **Step 1: Write the failing test**

Append to `tests/leak-check.test.sh`:

```bash
# Layer 2 is the only layer defending employer and project terms, and the
# pattern file it reads is untracked on purpose, so it is absent by default on
# every fresh machine. A missing file must therefore block rather than silently
# reduce the guard to its credential rules.
printf 'internal note about %s\n' "$FAKE_TERM" > "$repo/term-only.txt"
git -C "$repo" add term-only.txt

status=0
(cd "$repo" && LEAK_PATTERN_FILE="$FIXTURES/no-such-patterns.conf" \
    "$LEAK_CHECK" >/dev/null 2>&1) || status=$?
assert_equals 'a missing pattern file blocks rather than passing' '2' "$status"

status=0
(cd "$repo" && LEAK_PATTERN_FILE="$FIXTURES/no-such-patterns.conf" \
    LEAK_ALLOW_NO_PATTERNS=1 \
    "$LEAK_CHECK" >/dev/null 2>&1) || status=$?
assert_equals 'the explicit opt-out permits a run with no pattern file' '0' "$status"

git -C "$repo" reset -q HEAD term-only.txt
rm -f "$repo/term-only.txt"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `~/tests/leak-check.test.sh`
Expected: FAIL on `a missing pattern file blocks rather than passing`,
reporting `0` against the expected `2`.

- [ ] **Step 3: Write minimal implementation**

In `tests/leak-check.sh`, replace the `if [ ! -r "$PATTERN_FILE" ]` block at
lines 167-172:

```zsh
if [ ! -r "$PATTERN_FILE" ]; then
  if [ "${LEAK_ALLOW_NO_PATTERNS:-}" = 1 ]; then
    echo "" >&2
    echo "  $hook: term rules INACTIVE, permitted by LEAK_ALLOW_NO_PATTERNS" >&2
    echo "  Generic credential rules still ran." >&2
    echo "" >&2
  else
    # Layer 1 matches credential SHAPES. Layer 2 is the only thing defending
    # employer and project terms, which is why this repo needs a guard at all.
    # The pattern file is untracked on purpose, so "absent" is the default
    # state of a fresh machine rather than an exotic failure, and that is
    # precisely when setup work is being committed. Warning and continuing
    # published the content it exists to stop.
    echo "" >&2
    echo "  $hook: BLOCKED, no readable pattern file at $PATTERN_FILE" >&2
    echo "  The project term rules cannot run, so this scan is incomplete." >&2
    echo "  Restore the file (see ~/DOTFILES-GL.md), or set" >&2
    echo "  LEAK_ALLOW_NO_PATTERNS=1 for a machine with no terms to defend." >&2
    echo "" >&2
    exit 2
  fi
else
```

The `else` branch and everything under it stay exactly as they are.

- [ ] **Step 4: Run test to verify it passes**

Run: `~/tests/leak-check.test.sh`
Expected: PASS.

Confirm the guard still works on this machine, where the file exists:

Run: `cd ~ && GIT_DIR=$HOME/.cfg GIT_WORK_TREE=$HOME sh tests/leak-check.sh; echo $?`
Expected: `0`.

- [ ] **Step 5: Commit**

```bash
config add tests/leak-check.sh tests/leak-check.test.sh
config commit -m "Block rather than warn when the leak pattern file is missing

Layer 1 matches credential shapes. Layer 2 is the only layer defending
employer and project terms, which is the reason a public dotfiles repo needs
this guard. A missing pattern file silently deactivated layer 2 and exited 0.

Verified by differential test before the fix: identical content was blocked
with the file present and allowed with it absent.

The file is untracked on purpose, so absent-by-default is the state of every
fresh machine, and that is exactly when setup work gets committed.
LEAK_ALLOW_NO_PATTERNS=1 is the explicit opt-out for a machine that genuinely
has no terms to defend."
```

---

## Task 3: `leak-check.sh` blocks a path that produced no hunk

The blocker: a path marked `-diff` in `.gitattributes` produces
`Binary files ... differ` with no `+` lines, so the content rules see nothing.
Verified: a file holding a credential-shaped string is blocked normally and
passes once `cred.txt -diff` is committed.

One mechanism closes three recorded gaps at once: this one, the binary-file
gap, and the non-ASCII path gap, because all three have the same signature of
a path in the scan list that yielded no scannable content.

**Files:**
- Modify: `tests/leak-check.sh` (after the `staged` capture, around line 133)
- Test: `tests/leak-check.test.sh`

**Interfaces:**
- Consumes: `SCAN_FAILED` from Task 1, `scan_paths` and `TMP_OUT` from the
  existing script.
- Produces: `unscannable_paths()`, a pure comparison over two newline-separated
  lists. It performs no git call: the caller supplies both the expected path
  list and the diff text, which is what makes it testable without a repo.

- [ ] **Step 1: Write the failing test**

Append to `tests/leak-check.test.sh`:

```bash
# A one-line .gitattributes entry makes an ordinary text file unscannable:
# git prints "Binary files differ" and there are no + lines for the content
# rules to read. The guard must notice that a path it listed produced no
# hunk, rather than treating silence as cleanliness.
printf 'token = ghp_%s\n' "$(printf 'B%.0s' $(seq 1 24))" > "$repo/hidden.txt"
git -C "$repo" add hidden.txt

status=0
(cd "$repo" && "$LEAK_CHECK" >/dev/null 2>&1) || status=$?
assert_equals 'a credential in a plain file is blocked' '1' "$status"

printf 'hidden.txt -diff\n' > "$repo/.gitattributes"
git -C "$repo" add .gitattributes hidden.txt

output=$(cd "$repo" && "$LEAK_CHECK" 2>&1)
status=$?
assert_equals 'a -diff marked path does not pass silently' '2' "$status"
assert_contains 'the block names the unscannable path' "$output" 'hidden.txt'

git -C "$repo" reset -q HEAD hidden.txt .gitattributes
rm -f "$repo/hidden.txt" "$repo/.gitattributes"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `~/tests/leak-check.test.sh`
Expected: FAIL on `a -diff marked path does not pass silently`, reporting `0`
against the expected `2`.

- [ ] **Step 3: Write minimal implementation**

In `tests/leak-check.sh`, add the pure comparison next to the other function
definitions, after `added_lines`:

```zsh
# The paths that were scanned but produced no readable hunk.
#
# Pure by construction: both inputs are supplied by the caller, so this makes
# no git call and can be exercised without a repository. $1 is the newline
# separated path list the scan covered; $2 is the raw diff text produced for
# it.
#
# A path with no hunk is not a clean path. Three separate evasions share this
# signature: a .gitattributes `-diff` marking on ordinary text, a genuinely
# binary file, and a path whose name git quotes (every non-ASCII path), which
# matches no pathspec when fed back. Treating "no + lines" as "nothing to
# find" is what let all three through.
unscannable_paths() {
  local path_list=$1 diff_text=$2
  local path
  printf '%s\n' "$path_list" | while IFS= read -r path; do
    [ -n "$path" ] || continue
    if ! printf '%s\n' "$diff_text" | grep -qF -- "$path"; then
      printf '%s\n' "$path"
    fi
  done
}
```

Then call it after the `staged` capture, replacing the block Task 1 left at
lines 129-135:

```zsh
scan_paths=$(changed_paths | grep -v '^tests/leak-check\.sh$')
[ -f "$SCAN_FAILED" ] && exit 2
[ -z "$scan_paths" ] && exit 0

staged=$(echo "$scan_paths" | added_lines)
[ -f "$SCAN_FAILED" ] && exit 2

unscannable=$(unscannable_paths "$scan_paths" "$(cat "$TMP_OUT" 2>/dev/null)")
if [ -n "$unscannable" ]; then
  echo "" >&2
  echo "  $hook: BLOCKED, these paths produced no readable diff:" >&2
  printf '    %s\n' ${(f)unscannable} >&2
  echo "" >&2
  echo "  A path with no hunk was not scanned. Causes: a .gitattributes" >&2
  echo "  -diff marking, a binary file, or a path name git quotes." >&2
  echo "  Remove the marking, or move the content out of the repo." >&2
  echo "" >&2
  exit 2
fi

[ -z "$staged" ] && exit 0
```

Note `${(f)unscannable}` is zsh's split-on-newline flag; this file is
`#!/bin/zsh`, so it is available.

- [ ] **Step 4: Run test to verify it passes**

Run: `~/tests/leak-check.test.sh`
Expected: PASS.

Then confirm the guard still passes on real staged content, since this task
adds a way for it to block:

Run: `~/tests/run-all.sh leak-check`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
config add tests/leak-check.sh tests/leak-check.test.sh
config commit -m "Block paths the leak scan could not read

A path marked -diff in .gitattributes produces \"Binary files differ\" with no
+ lines, so the content rules saw nothing and the guard exited 0. Verified
before the fix: a credential-shaped string was blocked in a plain file and
allowed once one .gitattributes line marked that file.

One mechanism closes three recorded gaps, because all three have the same
signature of a listed path that yielded no scannable content: the -diff
marking, genuinely binary files, and paths whose names git quotes (every
non-ASCII path, which matches no pathspec when fed back).

unscannable_paths takes both the path list and the diff text as arguments, so
it makes no git call and is exercisable without a repository."
```

---

## Task 4: `SKIP_LEAK_CHECK` only skips on a true value

`tests/leak-check.sh:60` tests `[ -n "$SKIP_LEAK_CHECK" ]`, so
`SKIP_LEAK_CHECK=0` and `SKIP_LEAK_CHECK=false` both disable the guard.
Verified for `1`, `0`, and `false`. This is the sanctioned bypass of the repo's
primary control, so its semantics should not surprise.

**Files:**
- Modify: `tests/leak-check.sh:60`
- Test: `tests/leak-check.test.sh`

**Interfaces:**
- Consumes: nothing.
- Produces: nothing later tasks use.

- [ ] **Step 1: Write the failing test**

Append to `tests/leak-check.test.sh`:

```bash
# The sanctioned bypass of the repo's primary control should not fire on a
# value that reads as "do not skip".
printf 'token = ghp_%s\n' "$(printf 'C%.0s' $(seq 1 24))" > "$repo/skip-probe.txt"
git -C "$repo" add skip-probe.txt

status=0
(cd "$repo" && SKIP_LEAK_CHECK=1 "$LEAK_CHECK" >/dev/null 2>&1) || status=$?
assert_equals 'SKIP_LEAK_CHECK=1 skips' '0' "$status"

status=0
(cd "$repo" && SKIP_LEAK_CHECK=0 "$LEAK_CHECK" >/dev/null 2>&1) || status=$?
assert_equals 'SKIP_LEAK_CHECK=0 does not skip' '1' "$status"

status=0
(cd "$repo" && SKIP_LEAK_CHECK=false "$LEAK_CHECK" >/dev/null 2>&1) || status=$?
assert_equals 'SKIP_LEAK_CHECK=false does not skip' '1' "$status"

git -C "$repo" reset -q HEAD skip-probe.txt
rm -f "$repo/skip-probe.txt"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `~/tests/leak-check.test.sh`
Expected: FAIL on `SKIP_LEAK_CHECK=0 does not skip`, reporting `0` against the
expected `1`.

- [ ] **Step 3: Write minimal implementation**

In `tests/leak-check.sh`, replace line 60:

```zsh
# Matched against true values rather than tested for non-emptiness: `[ -n ]`
# is true for the string "0", so SKIP_LEAK_CHECK=0 disabled the guard for
# anyone who meant the opposite.
case "${SKIP_LEAK_CHECK:-}" in
  1|true|TRUE|yes|YES)
    echo "$hook: leak check SKIPPED via SKIP_LEAK_CHECK" >&2
    exit 0
    ;;
esac
```

- [ ] **Step 4: Run test to verify it passes**

Run: `~/tests/leak-check.test.sh`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
config add tests/leak-check.sh tests/leak-check.test.sh
config commit -m "Skip the leak check only on a true SKIP_LEAK_CHECK value

[ -n ] is true for the string \"0\", so SKIP_LEAK_CHECK=0 and
SKIP_LEAK_CHECK=false both disabled the guard. Verified for 1, 0 and false
before the fix. This is the sanctioned bypass of the repo's primary control,
so its semantics should not surprise the person reaching for it."
```

---

## Task 5: A suite that asserts nothing fails

The blocker: `tests/lib.sh:264` ends `finish` with `[ "$failed" -eq 0 ]`, which
is true when nothing ran. Under `run-all.sh -q`, which is what the pre-push
hook shows, that is byte-identical to a real pass. Verified: a suite whose body
is only `finish` prints `0 passed, 0 failed` and exits 0.

The live instance is `tests/githooks-installed.test.sh:26-30`, which uses a
bare `printf` rather than `skip`, then `finish; exit 0`. Verified under an
isolated `DOTFILES_ROOT`: `0 passed, 0 failed`, exit 0.
`.github/workflows/test-suite.yml:139` sets `DOTFILES_ROOT` to the checkout
workspace, which has `.git` and not `.cfg`, and the container has no `.cfg`
either. So the suite whose purpose is catching "the hooks are not installed"
runs its 7 assertions on one machine only.

Six suites bypass `skip` this way (confirmed by grep): `githooks-installed`,
`alacritty-platform-split`, `workflow-labels`, `zshrc-node-startup`,
`zshrc-python-startup`, `zshrc-platform-split`.

**Files:**
- Modify: `tests/lib.sh:224-233` (`assert_succeeds`), `:259-265` (`finish`)
- Modify: `tests/githooks-installed.test.sh:26-30`
- Modify: `tests/alacritty-platform-split.test.sh:126`,
  `tests/workflow-labels.test.sh:25`, `tests/zshrc-node-startup.test.sh:40,44,66`,
  `tests/zshrc-python-startup.test.sh:38,42`,
  `tests/zshrc-platform-split.test.sh:113`
- Test: `tests/skip-reporting.test.sh`

**Interfaces:**
- Consumes: nothing.
- Produces: `finish` returns non-zero when `passed + failed + skipped` is 0,
  and prints `<name>: no assertions ran`. `run-all.sh` needs no change, because
  it already keys its verdict on the suite's exit status.

Note on purity: `passed`, `failed`, and `skipped` are module-global counters
that `finish` reads, so `finish` cannot be a pure function without rewriting
the harness's state model. That rewrite is out of scope (the harness migrates
last, per the spec's non-goals). The counters stay; only the verdict changes.

- [ ] **Step 1: Write the failing test**

Append to `tests/skip-reporting.test.sh`, before its `finish` call:

```bash
# A suite that runs no assertions is not a passing suite. Under run-all.sh -q,
# which is what the pre-push hook shows, "0 passed, 0 failed" was
# indistinguishable from a real pass, and githooks-installed.test.sh reached
# exactly that state on CI and in the container.
empty_suite="$FIXTURES/empty.test.sh"
cat > "$empty_suite" <<EOF
. "$DOTFILES_ROOT/tests/lib.sh"
finish
EOF
chmod 755 "$empty_suite"

output=$(bash "$empty_suite" 2>&1)
status=$?
assert_equals 'a zero-assertion suite exits non-zero' '1' "$status"
assert_contains 'the verdict says no assertions ran' "$output" 'no assertions ran'

# A suite whose only outcome is a skip still passes: a skip is a green
# statement that a check could not run here, which is different from silence.
skip_suite="$FIXTURES/skip-only.test.sh"
cat > "$skip_suite" <<EOF
. "$DOTFILES_ROOT/tests/lib.sh"
skip 'nothing to check in this environment'
finish
EOF
chmod 755 "$skip_suite"

output=$(bash "$skip_suite" 2>&1)
status=$?
assert_equals 'a skip-only suite still passes' '0' "$status"

# assert_succeeds must report the real exit code. failed=$((failed + 1)) runs
# before the printf reads $?, so every failure across 170+ call sites reported
# "exited 0", which is the diagnostic a container-only failure depends on.
rc_suite="$FIXTURES/rc.test.sh"
cat > "$rc_suite" <<EOF
. "$DOTFILES_ROOT/tests/lib.sh"
assert_succeeds 'a command that exits 42' sh -c 'exit 42'
EOF
chmod 755 "$rc_suite"

output=$(bash "$rc_suite" 2>&1 || true)
assert_contains 'assert_succeeds reports the real exit code' "$output" 'exited 42'
```

- [ ] **Step 2: Run test to verify it fails**

Run: `~/tests/skip-reporting.test.sh`
Expected: FAIL on `a zero-assertion suite exits non-zero` (reporting `0`) and
on `assert_succeeds reports the real exit code`.

- [ ] **Step 3: Write minimal implementation**

In `tests/lib.sh`, fix `assert_succeeds` at lines 224-233. Capture the status
before anything else runs:

```bash
assert_succeeds() {
    local description=$1; shift
    local status=0
    "$@" >/dev/null 2>&1 || status=$?
    if [ "$status" -eq 0 ]; then
        passed=$((passed + 1))
        printf 'ok: %s\n' "$description"
    else
        # Captured before the counter increment: the arithmetic resets $?, so
        # reading it after the increment reported the increment's status and
        # every failure in the suite claimed "exited 0".
        failed=$((failed + 1))
        printf 'FAIL: %s (exited %d)\n' "$description" "$status"
    fi
}
```

Then replace `finish` at lines 259-265:

```bash
finish() {
    local summary
    summary=$(printf '%s: %d passed, %d failed' "$TEST_NAME" "$passed" "$failed")
    [ "$skipped" -eq 0 ] || summary="$summary, $skipped skipped"
    printf '\n%s\n' "$summary"

    # A suite that ran nothing is not a suite that passed. `skip` already
    # fixed the within-suite case; this is the whole-suite case, and it fired
    # in the container and on CI, where githooks-installed.test.sh reported
    # PASS over 7 assertions that never ran. A skip counts as having run,
    # because a skip is an explicit statement with a reason attached.
    if [ $((passed + failed + skipped)) -eq 0 ]; then
        printf '%s: no assertions ran\n' "$TEST_NAME" >&2
        return 1
    fi

    [ "$failed" -eq 0 ]
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `~/tests/skip-reporting.test.sh`
Expected: PASS.

Now the six bare-`printf` suites will fail, which is the point. Fix each by
replacing the bare `printf` with `skip` calls, one per assertion the branch
skips. In `tests/githooks-installed.test.sh:26-30`:

```bash
if [ ! -d "$CFG_DIR" ]; then
    # One skip per assertion the block below would have run, so the count on
    # the summary line matches what was lost. A bare printf here reported
    # "0 passed, 0 failed" and run-all.sh printed PASS, which is how this
    # suite removed itself from the container and from CI, where
    # DOTFILES_ROOT is a checkout with .git and no .cfg.
    skip "no .cfg repository at $CFG_DIR: core.hooksPath is unset"
    skip "no .cfg repository at $CFG_DIR: pre-commit is a symlink"
    skip "no .cfg repository at $CFG_DIR: pre-commit is executable"
    skip "no .cfg repository at $CFG_DIR: pre-commit points at tests/pre-commit"
    skip "no .cfg repository at $CFG_DIR: pre-push is a symlink"
    skip "no .cfg repository at $CFG_DIR: pre-push is executable"
    skip "no .cfg repository at $CFG_DIR: pre-push points at tests/pre-push"
    finish
    exit 0
fi
```

Read each of the other five files at the cited line, count the assertions the
branch skips, and add that many `skip` calls with the same reason text the
`printf` carried. Do not guess the count: read the block.

Run: `~/tests/run-all.sh`
Expected: PASS, with skip counts now appearing for the affected suites.

- [ ] **Step 5: Commit**

```bash
config add tests/lib.sh tests/skip-reporting.test.sh \
    tests/githooks-installed.test.sh tests/alacritty-platform-split.test.sh \
    tests/workflow-labels.test.sh tests/zshrc-node-startup.test.sh \
    tests/zshrc-python-startup.test.sh tests/zshrc-platform-split.test.sh
config commit -m "Fail a suite that runs no assertions, and report real exit codes

finish ended with [ \$failed -eq 0 ], which is true when nothing ran. Under
run-all.sh -q, which is what the pre-push hook shows, that was identical to a
real pass.

The live instance was githooks-installed.test.sh, which skipped with a bare
printf and then called finish. test-suite.yml sets DOTFILES_ROOT to the
checkout workspace, which has .git and not .cfg, and the container has no .cfg
either, so the suite that exists to catch \"the hooks are not installed\" ran
its 7 assertions on one machine only. Six suites skipped this way; all now use
skip, so the count reaches the summary line.

A skip still passes. A skip is an explicit statement with a reason; silence is
not.

Also captures the command status in assert_succeeds before the counter
increment. The arithmetic reset \$?, so all 170+ call sites reported
\"exited 0\" on failure, losing the one diagnostic a container-only failure
depends on."
```

---

## Task 6: Pin the Rust toolchain

The stamp covers source, not compiler. `Cargo.toml` carries `edition = "2024"`
and no `rust-version`, and `edition` is not a pin: every toolchain from 1.85
onward compiles edition 2024. So mac and linux can produce matching stamps from
different compilers. Verified: no `rust-toolchain.toml` anywhere, and
`rustup show` reports the moving `stable` channel.

This lands before the workspace move so the pin is in place while the stamp is
being redesigned.

**Files:**
- Create: `crates/rust-toolchain.toml`
- Test: `tests/config-manifest-lifecycle.test.sh`

**Interfaces:**
- Consumes: nothing.
- Produces: a pinned toolchain that `crates/`-scoped cargo invocations honor.
  Task 7 relies on the file living inside `crates/` so the existing
  `git add -- crates` in the stamp covers it.

- [ ] **Step 1: Write the failing test**

Append to `tests/config-manifest-lifecycle.test.sh`, before `finish`:

```bash
# The stamp covers source, not compiler. edition = "2024" is not a pin: every
# toolchain from 1.85 onward compiles it, so mac and linux could produce
# matching stamps from different compilers and the gate would see no problem.
TOOLCHAIN_FILE="$DOTFILES_ROOT/crates/rust-toolchain.toml"

assert_succeeds 'the toolchain is pinned' test -f "$TOOLCHAIN_FILE"
assert_succeeds 'the pin names an exact version, not a channel' \
    grep -qE '^channel = "1\.[0-9]+\.[0-9]+"' "$TOOLCHAIN_FILE"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `~/tests/config-manifest-lifecycle.test.sh`
Expected: FAIL on `the toolchain is pinned`.

- [ ] **Step 3: Write minimal implementation**

Find the current version, then write the file with that exact value:

```bash
rustc --version
```

Create `crates/rust-toolchain.toml`, substituting the version reported above:

```toml
# An exact pin, not a channel.
#
# The build stamp is a git tree id over the crate source, so it says nothing
# about which compiler produced the binary. `edition = "2024"` is not a pin
# either: every toolchain from 1.85 onward compiles it. Without this file mac
# and linux can push matching stamps from different compilers, and the
# pre-push gate cannot see the difference.
#
# This file lives inside crates/ so the stamp's `git add -- crates` covers it
# and a pin change invalidates every binary, which is the intended behavior.
[toolchain]
channel = "1.94.0"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 4: Run test to verify it passes**

Run: `~/tests/config-manifest-lifecycle.test.sh`
Expected: PASS.

Confirm the pinned toolchain still builds the crate:

Run: `~/.scripts/config/config-build`
Expected: `config-build: installed ... (stamp ...)`.

- [ ] **Step 5: Commit**

```bash
config add crates/rust-toolchain.toml tests/config-manifest-lifecycle.test.sh
config commit -m "Pin the Rust toolchain to an exact version

The build stamp is a git tree id over the crate source, so it says nothing
about which compiler produced the binary, and edition = \"2024\" is not a pin:
every toolchain from 1.85 onward compiles it. mac and linux could push
matching stamps from different compilers with the gate seeing no difference.

The file lives inside crates/ so the stamp's existing \`git add -- crates\`
covers it, which means a pin change correctly invalidates every binary."
```

---

## Task 7: Untrack the proptest regressions file

`crates/config-manifest/proptest-regressions/plan.txt` is tracked inside the
stamped crate directory, so a proptest failure seed written by a test run
changes the stamp and forces a rebuild that produces a byte-identical binary.
Over-declared inputs are the tax that makes people bypass a gate.

**Files:**
- Delete from tracking: `crates/config-manifest/proptest-regressions/plan.txt`
- Modify: `crates/config-manifest/.gitignore`
- Test: `tests/config-manifest-lifecycle.test.sh`

**Interfaces:**
- Consumes: nothing.
- Produces: nothing later tasks use.

- [ ] **Step 1: Write the failing test**

Append to `tests/config-manifest-lifecycle.test.sh`:

```bash
# A proptest seed is an output of running the tests, not an input to the
# build. Tracked inside the stamped tree it moves the stamp, so a test run
# forces a rebuild that produces a byte-identical binary.
assert_equals 'no proptest regressions file is tracked in the stamped tree' '' \
    "$(cd "$DOTFILES_ROOT" && git --git-dir="$DOTFILES_ROOT/.cfg" \
        --work-tree="$DOTFILES_ROOT" ls-files \
        'crates/*/proptest-regressions/*' 2>/dev/null)"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `~/tests/config-manifest-lifecycle.test.sh`
Expected: FAIL, reporting `crates/config-manifest/proptest-regressions/plan.txt`
against the expected empty string.

- [ ] **Step 3: Write minimal implementation**

```bash
config rm --cached crates/config-manifest/proptest-regressions/plan.txt
```

Append to `crates/config-manifest/.gitignore`:

```
# Proptest writes a failure seed here when a property fails. That is an output
# of running the tests, not an input to the build, and this directory sits
# inside the tree the build stamp covers: a tracked seed moves the stamp and
# forces a rebuild whose output is byte-identical.
proptest-regressions/
```

- [ ] **Step 4: Run test to verify it passes**

Run: `~/tests/config-manifest-lifecycle.test.sh`
Expected: PASS.

Confirm the stamp changed as expected and rebuild:

Run: `~/.scripts/config/config-build`
Expected: a new stamp value, since the crate tree no longer contains that file.

- [ ] **Step 5: Commit**

```bash
config add crates/config-manifest/.gitignore \
    tests/config-manifest-lifecycle.test.sh
config commit -m "Untrack the proptest regressions file from the stamped tree

A proptest seed is an output of running the tests, not an input to the build.
Tracked inside crates/config-manifest it sits in the tree the stamp covers, so
a test run moved the stamp and forced a rebuild whose output is byte-identical.

Over-declared build inputs are the tax that makes a gate get bypassed."
```

---

## Task 8: Convert `crates/` to a cargo workspace

One shared `Cargo.lock`, one resolution, one build. This lands before the stamp
redesign so the stamp is written against the layout it has to support.

**Files:**
- Create: `crates/Cargo.toml`
- Move: `crates/config-manifest/Cargo.lock` to `crates/Cargo.lock`
- Modify: `crates/config-manifest/Cargo.toml`
- Modify: `.scripts/config/config-build`
- Test: `tests/config-manifest-lifecycle.test.sh`

**Interfaces:**
- Consumes: the toolchain pin from Task 6.
- Produces: `crates/Cargo.toml` with `[workspace] members`. Task 9 reads that
  members list to enumerate crates for stamping. The binary still installs to
  `$CONFIG_BIN_DIR/config-manifest` and `--stamp` still prints one value, so
  `tests/pre-push` keeps working until Task 9 changes it.

- [ ] **Step 1: Write the failing test**

Append to `tests/config-manifest-lifecycle.test.sh`:

```bash
# One workspace, one lockfile, one resolution. The members list is also what
# the stamp enumerates, so it is the single place that says which crates exist.
WORKSPACE_MANIFEST="$DOTFILES_ROOT/crates/Cargo.toml"

assert_succeeds 'a workspace root exists' test -f "$WORKSPACE_MANIFEST"
assert_succeeds 'the workspace declares members' \
    grep -q '^members = \[' "$WORKSPACE_MANIFEST"
assert_succeeds 'the shared lockfile is at the workspace root' \
    test -f "$DOTFILES_ROOT/crates/Cargo.lock"
assert_equals 'no per-crate lockfile remains' '' \
    "$(cd "$DOTFILES_ROOT" && git --git-dir="$DOTFILES_ROOT/.cfg" \
        --work-tree="$DOTFILES_ROOT" ls-files \
        'crates/*/Cargo.lock' 2>/dev/null)"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `~/tests/config-manifest-lifecycle.test.sh`
Expected: FAIL on `a workspace root exists`.

- [ ] **Step 3: Write minimal implementation**

Create `crates/Cargo.toml`:

```toml
# Workspace root for the dotfiles crates.
#
# One lockfile and one resolution, so a dependency exists at one version
# across every binary. The members list is also what `config stamp` and
# `config build` enumerate, which makes this the single place that says
# which crates exist.
[workspace]
resolver = "3"
members = ["config-manifest"]

# Dependencies shared by more than one member are declared here and
# referenced with `workspace = true`, so a bump happens once.
[workspace.dependencies]
anyhow = "1.0.104"
clap = { version = "4.6.6", features = ["derive", "env"] }
assert_cmd = "2.2.2"
proptest = "1.11.0"
tempfile = "3.27.0"
```

Move the lockfile:

```bash
mv crates/config-manifest/Cargo.lock crates/Cargo.lock
config rm --cached crates/config-manifest/Cargo.lock
```

Rewrite `crates/config-manifest/Cargo.toml` to inherit:

```toml
[package]
name = "config-manifest"
version = "0.1.0"
edition = "2024"

[dependencies]
anyhow = { workspace = true }
clap = { workspace = true }

[dev-dependencies]
assert_cmd = { workspace = true }
proptest = { workspace = true }
tempfile = { workspace = true }
```

In `.scripts/config/config-build`, point cargo at the workspace root. Replace
the `cargo build` invocation and the `CRATE_DIR` assignment:

```sh
ROOT=${DOTFILES_ROOT:-$HOME}
WORKSPACE_DIR="$ROOT/crates"
BIN_DIR=${CONFIG_BIN_DIR:-$HOME/.local/bin}
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/config-manifest/target}"

stamp=$("$ROOT/.scripts/config/config-stamp")

# option_env! reads this at compile time, so changing it rebuilds the binary.
CONFIG_MANIFEST_STAMP="$stamp" \
    cargo build --release --locked --quiet \
        --manifest-path "$WORKSPACE_DIR/Cargo.toml"
```

- [ ] **Step 4: Run test to verify it passes**

Run: `~/tests/config-manifest-lifecycle.test.sh`
Expected: PASS.

Confirm the workspace builds and the binary still works:

Run: `~/.scripts/config/config-build && config-manifest --stamp`
Expected: an install line, then a 40-character tree id.

Run: `~/tests/run-all.sh`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
config add crates/Cargo.toml crates/Cargo.lock \
    crates/config-manifest/Cargo.toml .scripts/config/config-build \
    tests/config-manifest-lifecycle.test.sh
config commit -m "Convert crates/ to a cargo workspace

One lockfile and one resolution, so a dependency exists at one version across
every binary, and one cargo build rather than N. Shared dependency versions
move to [workspace.dependencies] so a bump happens in one place.

The members list is also what the stamp will enumerate in the next commit,
which makes crates/Cargo.toml the single place that says which crates exist.

The binary still installs to the same path and --stamp still prints one value,
so tests/pre-push keeps working unchanged until the stamp becomes per-crate."
```

---

## Task 9: Per-crate stamps

A single workspace-wide stamp would be coarser than any one binary's input
set: editing crate A would mark crate B stale, and `pre-push` would refuse a
push over a binary byte-identical to what its own sources produce. False
refusals are how a gate gets bypassed, which is the same reasoning
`config-stamp`'s header uses to reject a HEAD-based stamp.

Verified before writing this plan: one `write-tree` over `crates/` produced
root tree `623787b34617522f3db88c908d4d3e20a8d75323`, and
`rev-parse "${root}:crates/config-manifest"` produced
`e7b9aa22ab52dd83fd6d1346708a3fd921637b36`, byte-identical to what
`config-stamp` prints today.

**Files:**
- Create: `crates/config-manifest/src/stamp.rs`
- Modify: `crates/config-manifest/src/lib.rs`, `src/main.rs`
- Modify: `.scripts/config/config-stamp`
- Modify: `tests/pre-push:130-144`
- Test: `crates/config-manifest/src/stamp.rs` (unit, in-module),
  `tests/config-manifest-lifecycle.test.sh`, `tests/pre-push-multi-ref.test.sh`

**Interfaces:**
- Consumes: `crates/Cargo.toml` members list from Task 8.
- Produces:
  - `stamp::fold(crate_tree: &str, lock_blob: &str, workspace_blob: &str) -> String`
    Pure. No IO. Returns the stamp for one crate.
  - `stamp::parse_members(manifest_text: &str) -> Vec<String>`
    Pure. No IO. Reads the members list out of workspace manifest text.
  - `config stamp` with no argument prints `<crate> <stamp>` lines.
  - `config stamp <crate>` prints one stamp, for scripting.

- [ ] **Step 1: Write the failing test**

Create `crates/config-manifest/src/stamp.rs` with only its tests:

```rust
//! Build-stamp folding.
//!
//! Pure by construction: every input is a value supplied by the caller, so
//! nothing here spawns git or touches the filesystem. The IO that produces
//! those values lives in `git.rs`, which keeps every folding rule testable
//! as a table.

#[cfg(test)]
mod tests {
    use super::*;

    // A crate's stamp must change when its own source changes.
    #[test]
    fn a_different_crate_tree_yields_a_different_stamp() {
        let first = fold("aaa", "lock", "workspace");
        let second = fold("bbb", "lock", "workspace");
        assert_ne!(first, second);
    }

    // The lockfile is folded in because a shared lockfile means a dependency
    // bump changes what the binary is, without touching the crate's own tree.
    #[test]
    fn a_different_lockfile_yields_a_different_stamp() {
        let first = fold("aaa", "lock-one", "workspace");
        let second = fold("aaa", "lock-two", "workspace");
        assert_ne!(first, second);
    }

    #[test]
    fn a_different_workspace_manifest_yields_a_different_stamp() {
        let first = fold("aaa", "lock", "workspace-one");
        let second = fold("aaa", "lock", "workspace-two");
        assert_ne!(first, second);
    }

    #[test]
    fn the_same_inputs_yield_the_same_stamp() {
        assert_eq!(fold("aaa", "lock", "ws"), fold("aaa", "lock", "ws"));
    }

    #[test]
    fn members_are_read_from_a_workspace_manifest() {
        let text = "[workspace]\nresolver = \"3\"\nmembers = [\"config-manifest\", \"config-deps\"]\n";
        assert_eq!(
            parse_members(text),
            vec!["config-manifest".to_string(), "config-deps".to_string()]
        );
    }

    #[test]
    fn a_manifest_with_no_members_yields_none() {
        assert_eq!(parse_members("[workspace]\n"), Vec::<String>::new());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd ~/crates && cargo test --locked -p config-manifest stamp`
Expected: FAIL to compile, `cannot find function fold in this scope`.

- [ ] **Step 3: Write minimal implementation**

Prepend to `crates/config-manifest/src/stamp.rs`, above the test module:

```rust
use std::fmt::Write as _;

/// The stamp for one crate.
///
/// Folds the crate's own tree id together with the shared lockfile and the
/// workspace manifest, because under a shared lockfile a dependency bump
/// changes what the binary is without touching the crate's own subtree. A
/// stamp that ignored the lockfile would claim currency across such a bump.
///
/// Per-crate rather than one id over the whole workspace: a single workspace
/// stamp is coarser than any one binary's input set, so editing one crate
/// would mark every binary stale and make pre-push refuse a push over a
/// binary identical to what its sources produce. A false refusal is how a
/// gate gets bypassed.
pub fn fold(crate_tree: &str, lock_blob: &str, workspace_blob: &str) -> String {
    let mut folded = String::new();
    // Written as a delimited record rather than concatenated, so two
    // different field splits cannot produce the same string.
    write!(folded, "{crate_tree}:{lock_blob}:{workspace_blob}")
        .expect("writing to a String cannot fail");
    folded
}

/// The member crate names declared by a workspace manifest.
///
/// Reads the `members = [...]` line without a TOML parser, because this runs
/// in the same binary the stamp guards and adding a parse dependency to the
/// stamp path would put that dependency inside its own input set.
pub fn parse_members(manifest_text: &str) -> Vec<String> {
    for line in manifest_text.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("members") else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim();
        let Some(inner) = rest.strip_prefix('[').and_then(|value| value.strip_suffix(']')) else {
            continue;
        };
        return inner
            .split(',')
            .map(|entry| entry.trim().trim_matches('"').to_string())
            .filter(|entry| !entry.is_empty())
            .collect();
    }
    Vec::new()
}
```

Register the module in `crates/config-manifest/src/lib.rs`:

```rust
pub mod stamp;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd ~/crates && cargo test --locked -p config-manifest stamp`
Expected: PASS, 6 tests.

- [ ] **Step 5: Commit**

```bash
config add crates/config-manifest/src/stamp.rs \
    crates/config-manifest/src/lib.rs
config commit -m "Add pure per-crate stamp folding

Folds a crate's tree id with the shared lockfile and workspace manifest,
because under one lockfile a dependency bump changes what a binary is without
touching that crate's subtree.

Per-crate rather than one workspace-wide id: a single stamp is coarser than
any one binary's input set, so editing crate A would mark crate B stale and
pre-push would refuse a push over a binary identical to what its own sources
produce. False refusals are how a gate gets bypassed, which is the reasoning
config-stamp's own header already uses to reject a HEAD-based stamp.

Every input is a caller-supplied value, so this module spawns no git and
touches no filesystem, which keeps each folding rule a table test."
```

- [ ] **Step 6: Write the failing shell test for the emitter**

Append to `tests/config-manifest-lifecycle.test.sh`:

```bash
# config stamp now enumerates crates. The per-crate form is what pre-push
# iterates; the single-crate form is for scripting.
output=$("$DOTFILES_ROOT/.scripts/config/config-stamp")
assert_succeeds 'config stamp names config-manifest' \
    grep -q '^config-manifest [0-9a-f]\{40,\}' <<<"$output"

one=$("$DOTFILES_ROOT/.scripts/config/config-stamp" config-manifest)
assert_succeeds 'config stamp <crate> prints a bare id' \
    grep -qE '^[0-9a-f]{40,}$' <<<"$one"

assert_succeeds 'an unknown crate is an error' \
    test 2 -eq "$("$DOTFILES_ROOT/.scripts/config/config-stamp" no-such-crate \
        >/dev/null 2>&1; echo $?)"
```

- [ ] **Step 7: Run it to verify it fails**

Run: `~/tests/config-manifest-lifecycle.test.sh`
Expected: FAIL on `config stamp names config-manifest`.

- [ ] **Step 8: Rewrite `config-stamp`**

Replace the body of `.scripts/config/config-stamp` below its usage block:

```sh
ROOT=${DOTFILES_ROOT:-$HOME}
WORKSPACE=crates

if [ -d "$ROOT/.cfg" ]; then
    git_cmd() { git --git-dir="$ROOT/.cfg" --work-tree="$ROOT" "$@"; }
else
    git_cmd() { git -C "$ROOT" "$@"; }
fi

index=$(mktemp "${TMPDIR:-/tmp}/config-stamp-XXXXXX")
rm -f "$index"
trap 'rm -f "$index"' EXIT INT TERM HUP
export GIT_INDEX_FILE="$index"

git_cmd read-tree --empty
(cd "$ROOT" && git_cmd add -- "$WORKSPACE")
root_tree=$(git_cmd write-tree)

# One write-tree for the whole workspace, then one rev-parse per crate. The
# lockfile and workspace manifest are folded into every crate's stamp because
# they are shared inputs: a bump to either changes what each binary is.
lock_blob=$(git_cmd rev-parse "$root_tree:$WORKSPACE/Cargo.lock")
ws_blob=$(git_cmd rev-parse "$root_tree:$WORKSPACE/Cargo.toml")

members=$(git_cmd show "$root_tree:$WORKSPACE/Cargo.toml" \
    | sed -n 's/^members = \[\(.*\)\]$/\1/p' \
    | tr ',' '\n' \
    | tr -d ' "' \
    | grep -v '^$')

stamp_for() {
    crate_tree=$(git_cmd rev-parse "$root_tree:$WORKSPACE/$1")
    printf '%s:%s:%s' "$crate_tree" "$lock_blob" "$ws_blob"
}

if [ "$#" -gt 0 ]; then
    for member in $members; do
        if [ "$member" = "$1" ]; then
            stamp_for "$1"
            printf '\n'
            exit 0
        fi
    done
    printf 'config-stamp: no such workspace crate: %s\n' "$1" >&2
    exit 2
fi

for member in $members; do
    printf '%s %s\n' "$member" "$(stamp_for "$member")"
done
```

- [ ] **Step 9: Run it to verify it passes**

Run: `~/tests/config-manifest-lifecycle.test.sh`
Expected: PASS.

- [ ] **Step 10: Update `config-build` and `pre-push` to the per-crate stamp**

In `.scripts/config/config-build`, stamp each crate rather than passing one
value. Replace the stamp and build block:

```sh
# Each crate is built with its own stamp, so a rebuild of one does not claim
# currency for another.
for member in $("$ROOT/.scripts/config/config-stamp" | cut -d' ' -f1); do
    stamp=$("$ROOT/.scripts/config/config-stamp" "$member")
    CONFIG_MANIFEST_STAMP="$stamp" \
        cargo build --release --locked --quiet \
            --manifest-path "$WORKSPACE_DIR/Cargo.toml" -p "$member"
    mkdir -p "$BIN_DIR"
    cp "$CARGO_TARGET_DIR/release/$member" "$BIN_DIR/$member"
    built=$("$BIN_DIR/$member" --stamp)
    printf 'config-build: installed %s (stamp %s)\n' "$BIN_DIR/$member" "$built"
done
```

In `tests/pre-push`, replace the single-binary stamp check at lines 130-144:

```sh
    # Every workspace crate is checked against its own subtree. A single
    # workspace-wide stamp would refuse a push over a binary whose own sources
    # did not change, and a false refusal is how this gate gets bypassed.
    for ref in $push_refs; do
        pushed_tree=$(git rev-parse --verify --quiet "$ref:crates" 2>/dev/null || true)
        [ -n "$pushed_tree" ] || continue

        for member in $("$HOME/.scripts/config/config-stamp" | cut -d' ' -f1); do
            if ! command -v "$member" >/dev/null 2>&1; then
                printf 'pre-push: %s is not on PATH; run ~/.scripts/config/config-build\n' \
                    "$member" >&2
                exit 1
            fi
            expected=$("$HOME/.scripts/config/config-stamp" "$member")
            built=$("$member" --stamp)
            if [ "$built" != "$expected" ]; then
                printf 'pre-push: %s is stale for %s (built %s, expected %s)\n' \
                    "$member" "$ref" "$built" "$expected" >&2
                printf 'pre-push: run ~/.scripts/config/config-build\n' >&2
                exit 1
            fi
            printf 'pre-push: %s stamp matches the pushed crate for %s\n' \
                "$member" "$ref"
        done
    done
```

- [ ] **Step 11: Add the false-refusal regression test**

Append to `tests/pre-push-multi-ref.test.sh`:

```bash
# The reason the stamp is per-crate. With a single workspace-wide stamp,
# editing one crate marks every binary stale, and the gate refuses a push over
# a binary byte-identical to what its own sources produce. A false refusal is
# how a gate gets bypassed, so this asserts the refusal is scoped.
ws="$FIXTURES/stamp-scope"
mkdir -p "$ws/crates/crate-one" "$ws/crates/crate-two"
printf '[workspace]\nmembers = ["crate-one", "crate-two"]\n' > "$ws/crates/Cargo.toml"
printf 'lock\n' > "$ws/crates/Cargo.lock"
printf 'one\n' > "$ws/crates/crate-one/src.rs"
printf 'two\n' > "$ws/crates/crate-two/src.rs"

git -C "$ws" init -q -b main
git -C "$ws" -c user.email=t@t -c user.name=t add -A
git -C "$ws" -c user.email=t@t -c user.name=t commit -q -m init

before_two=$(DOTFILES_ROOT="$ws" "$DOTFILES_ROOT/.scripts/config/config-stamp" crate-two)

printf 'one changed\n' > "$ws/crates/crate-one/src.rs"
git -C "$ws" -c user.email=t@t -c user.name=t add -A
git -C "$ws" -c user.email=t@t -c user.name=t commit -q -m 'edit crate-one'

after_two=$(DOTFILES_ROOT="$ws" "$DOTFILES_ROOT/.scripts/config/config-stamp" crate-two)
after_one=$(DOTFILES_ROOT="$ws" "$DOTFILES_ROOT/.scripts/config/config-stamp" crate-one)

assert_equals 'editing one crate leaves the other stamp unchanged' \
    "$before_two" "$after_two"
assert_succeeds 'editing one crate changes its own stamp' \
    test "$after_one" != "$before_two"
```

- [ ] **Step 12: Verify everything passes**

Run: `~/tests/pre-push-multi-ref.test.sh`
Expected: PASS.

Run: `~/.scripts/config/config-build`
Expected: one install line per member.

Run: `~/tests/run-all.sh`
Expected: PASS.

- [ ] **Step 13: Commit**

```bash
config add .scripts/config/config-stamp .scripts/config/config-build \
    tests/pre-push tests/config-manifest-lifecycle.test.sh \
    tests/pre-push-multi-ref.test.sh
config commit -m "Stamp and verify each workspace crate separately

config stamp now enumerates the workspace: one write-tree for crates/, then
one rev-parse per member, with the shared lockfile and workspace manifest
folded into each crate's stamp. Verified that this reproduces the previous
single-crate stamp byte for byte.

pre-push iterates members and checks each binary against its own subtree. A
single workspace-wide stamp would be coarser than any one binary's input set,
so editing crate A would refuse a push over crate B's unchanged binary.
pre-push-multi-ref.test.sh now asserts that scoping directly, because a false
refusal is how this gate gets bypassed.

The hook still never compiles. That property is load-bearing: a hook that
compiles gets bypassed."
```

---

## Task 10: `config doctor`

Reports which installed binaries do not match their crate, and prints the exact
fix. Silent when current, so it is safe to run habitually.

`doctor` was chosen after checking collisions: `status` shadows a git verb and
`check` is already the drift check. Verified both.

**Files:**
- Create: `crates/config-manifest/src/doctor.rs`
- Create: `.scripts/config/config-doctor`
- Modify: `crates/config-manifest/src/lib.rs`, `src/main.rs`
- Modify: `.claude/rules/dotfiles-tests.md`
- Test: `crates/config-manifest/src/doctor.rs` (unit), `tests/config.test.sh`

**Interfaces:**
- Consumes: `stamp::fold` and `stamp::parse_members` from Task 9.
- Produces:
  - `doctor::Finding { crate_name: String, installed: Option<String>, expected: String }`
  - `doctor::diagnose(installed: &[(String, Option<String>)], expected: &[(String, String)]) -> Vec<Finding>`
    Pure. No IO.
  - `doctor::render(findings: &[Finding]) -> Rendered { stdout, stderr, exit_code }`
    Pure. No IO. Mirrors `check::render`'s existing shape.

- [ ] **Step 1: Write the failing test**

Create `crates/config-manifest/src/doctor.rs` with only its tests:

```rust
//! Binary staleness diagnosis.
//!
//! Pure by construction. `diagnose` compares two caller-supplied lists and
//! `render` turns findings into a value; neither spawns a process nor reads
//! the filesystem. Gathering the installed stamps is the caller's job, which
//! is what lets "one crate stale, two current" be a unit test rather than a
//! subprocess test.

#[cfg(test)]
mod tests {
    use super::*;

    fn expected() -> Vec<(String, String)> {
        vec![
            ("config-manifest".to_string(), "stamp-a".to_string()),
            ("config-deps".to_string(), "stamp-b".to_string()),
        ]
    }

    #[test]
    fn everything_current_yields_no_findings() {
        let installed = vec![
            ("config-manifest".to_string(), Some("stamp-a".to_string())),
            ("config-deps".to_string(), Some("stamp-b".to_string())),
        ];
        assert!(diagnose(&installed, &expected()).is_empty());
    }

    #[test]
    fn a_stale_binary_is_reported_and_the_current_one_is_not() {
        let installed = vec![
            ("config-manifest".to_string(), Some("old".to_string())),
            ("config-deps".to_string(), Some("stamp-b".to_string())),
        ];
        let findings = diagnose(&installed, &expected());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].crate_name, "config-manifest");
    }

    #[test]
    fn a_missing_binary_is_reported() {
        let installed = vec![
            ("config-manifest".to_string(), None),
            ("config-deps".to_string(), Some("stamp-b".to_string())),
        ];
        let findings = diagnose(&installed, &expected());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].installed, None);
    }

    #[test]
    fn a_clean_render_is_silent_and_exits_zero() {
        let rendered = render(&[]);
        assert_eq!(rendered.stdout, "");
        assert_eq!(rendered.stderr, "");
        assert_eq!(rendered.exit_code, 0);
    }

    #[test]
    fn a_stale_render_names_the_crate_and_the_fix() {
        let findings = vec![Finding {
            crate_name: "config-manifest".to_string(),
            installed: Some("old".to_string()),
            expected: "new".to_string(),
        }];
        let rendered = render(&findings);
        assert!(rendered.stderr.contains("config-manifest"));
        assert!(rendered.stderr.contains("config build"));
        assert_eq!(rendered.exit_code, 1);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd ~/crates && cargo test --locked -p config-manifest doctor`
Expected: FAIL to compile, `cannot find function diagnose in this scope`.

- [ ] **Step 3: Write minimal implementation**

Prepend to `crates/config-manifest/src/doctor.rs`:

```rust
use std::fmt::Write as _;

/// One crate whose installed binary does not match its source.
#[derive(Debug, PartialEq, Eq)]
pub struct Finding {
    pub crate_name: String,
    /// `None` when no binary is installed for this crate.
    pub installed: Option<String>,
    pub expected: String,
}

/// A rendered result, as a value rather than as writes to stdout.
///
/// Mirrors `check::Rendered` so both subcommands are testable without a
/// subprocess and only `main` mentions process exit.
#[derive(Debug, PartialEq, Eq)]
pub struct Rendered {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: u8,
}

/// The crates whose installed binary does not match the expected stamp.
///
/// Both lists are supplied by the caller, so this makes no process call and
/// reads no file.
pub fn diagnose(
    installed: &[(String, Option<String>)],
    expected: &[(String, String)],
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (crate_name, want) in expected {
        let Some((_, have)) = installed.iter().find(|(name, _)| name == crate_name) else {
            findings.push(Finding {
                crate_name: crate_name.clone(),
                installed: None,
                expected: want.clone(),
            });
            continue;
        };
        if have.as_deref() != Some(want.as_str()) {
            findings.push(Finding {
                crate_name: crate_name.clone(),
                installed: have.clone(),
                expected: want.clone(),
            });
        }
    }
    findings
}

/// Renders findings. Silent and zero when everything is current, so this is
/// safe to run habitually.
pub fn render(findings: &[Finding]) -> Rendered {
    if findings.is_empty() {
        return Rendered {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: 0,
        };
    }

    let mut stderr = String::new();
    stderr.push_str("config doctor: installed binaries do not match their source\n\n");
    for finding in findings {
        match &finding.installed {
            None => {
                writeln!(stderr, "  {}: not installed", finding.crate_name)
                    .expect("writing to a String cannot fail");
            }
            Some(have) => {
                writeln!(
                    stderr,
                    "  {}: installed {}, source {}",
                    finding.crate_name, have, finding.expected
                )
                .expect("writing to a String cannot fail");
            }
        }
    }
    stderr.push_str("\n  Fix: config build\n");

    Rendered {
        stdout: String::new(),
        stderr,
        exit_code: 1,
    }
}
```

Register it in `crates/config-manifest/src/lib.rs`:

```rust
pub mod doctor;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd ~/crates && cargo test --locked -p config-manifest doctor`
Expected: PASS, 5 tests.

- [ ] **Step 5: Wire the subcommand and the wrapper**

Add a `Doctor` variant to the clap `Subcommand` enum in
`crates/config-manifest/src/main.rs`, following the existing `Check` and `Sync`
variants' shape. Extend the existing import to reach the new module:

```rust
use config_manifest::{check, doctor, git, manifest};
```

`main.rs` reaches modules through the implicit lib target (`use
config_manifest::...`), so registering `pub mod doctor;` in `lib.rs` is what
makes this resolve.

The handler gathers the installed stamps by running each binary's `--stamp`,
reads the expected stamps by running `config-stamp`, calls `doctor::diagnose`,
then writes `doctor::render`'s value to the real streams and returns its
`exit_code`. All process spawning stays in `main.rs`, where the other 10 IO
references already live, so `doctor.rs` keeps its zero.

Create `.scripts/config/config-doctor`:

```sh
#!/bin/sh
# help: Report installed binaries that do not match their source
#
# Reports which workspace binaries were built from source that has since
# changed, and names the command that fixes it. Silent when everything is
# current, so it is safe to run habitually.
#
# There is no runtime freshness check anywhere else, by measurement: computing
# a stamp costs about 124ms, and the hot path runs on every shell prompt. The
# guarantee is at pre-push instead, and this command is how you ask early.
#
# usage: config doctor
#
# Takes no options.
#
# Environment:
#   DOTFILES_ROOT   Repo root. Default $HOME.

set -eu

. "$(dirname "$(readlink -f "$0")")/usage.sh"
usage_if_requested "${1:-}"

exec config-manifest doctor "$@"
```

```bash
chmod 755 .scripts/config/config-doctor
```

- [ ] **Step 6: Write the shell test**

Append to `tests/config.test.sh`:

```bash
# doctor is silent when current, so it is safe to run habitually. `status`
# would shadow a git verb and `check` is already the drift check, which is why
# the verb is `doctor`.
assert_succeeds 'config doctor exists and is executable' \
    test -x "$CONFIG_DIR/config-doctor"

assert_equals 'doctor is silent when every binary is current' '' \
    "$(cd "$DOTFILES_ROOT" && "$CONFIG_DIR/config-doctor" 2>&1)"

assert_equals 'doctor does not shadow a git verb' '' \
    "$(git --list-cmds=main,others 2>/dev/null | grep -x doctor || true)"
```

- [ ] **Step 7: Run the tests**

Run: `~/tests/config.test.sh`
Expected: PASS.

Run: `~/.scripts/config/config-build && config doctor; echo "exit=$?"`
Expected: install lines, then no doctor output and `exit=0`.

Now confirm it detects staleness. Edit a source file, do not rebuild:

```bash
printf '\n// staleness probe\n' >> crates/config-manifest/src/stamp.rs
config doctor; echo "exit=$?"
```
Expected: a line naming `config-manifest` and `config build`, with `exit=1`.

Then revert and rebuild:

```bash
config checkout -- crates/config-manifest/src/stamp.rs
~/.scripts/config/config-build
```

- [ ] **Step 8: Document the loop**

Add to `.claude/rules/dotfiles-tests.md`, after the pre-push section:

```markdown
## Rebuild after editing a crate

Editing a crate under `crates/` does not change what the installed binary
does. Run `config build` after any crate edit; it builds every workspace
member and re-stamps each one.

`config doctor` reports which installed binaries no longer match their source
and names the fix. It is silent when everything is current, so it is safe to
run habitually.

There is deliberately no runtime freshness check. Two were measured and
rejected: a binary that verifies its own stamp on startup costs about 124ms
per invocation, on a path that runs before every shell prompt, and a rebuild
triggered from a prompt hook serializes every pane behind cargo's build lock
(a no-op release build measures 0.7 to 1.5 seconds). The guarantee is at
pre-push, which refuses a push when any binary is stale, and `config doctor`
is how you ask before then.
```

- [ ] **Step 9: Verify and commit**

Run: `~/tests/run-all.sh`
Expected: PASS.

```bash
config add crates/config-manifest/src/doctor.rs \
    crates/config-manifest/src/lib.rs crates/config-manifest/src/main.rs \
    .scripts/config/config-doctor tests/config.test.sh \
    .claude/rules/dotfiles-tests.md
config commit -m "Add config doctor to report stale binaries

Reports which installed binaries were built from source that has since
changed, and names the fix. Silent when current, so it is safe to run
habitually.

diagnose and render are pure over caller-supplied lists, so \"one crate stale,
two current\" is a unit test rather than a subprocess test. render returns a
value carrying stdout, stderr and an exit code, mirroring check::render, so
only main mentions process exit.

The verb is doctor because status shadows a git verb and check is already the
drift check; both verified against git --list-cmds.

Documents the edit-build-test loop, including why there is no runtime
freshness check: a self-checking binary measures about 124ms per invocation on
a path that runs before every prompt, and a prompt-triggered rebuild
serializes every pane behind cargo's build lock."
```

---

## Self-Review

**Spec coverage.** Walking the spec's Step 0 through Step 2:

| Spec item | Task |
|---|---|
| Blocker: subshell fail-open | 1 |
| Blocker: pattern file fail-open | 2 |
| Blocker: `.gitattributes` blinding | 3 |
| Blocker: zero-assertion PASS | 5 |
| Six bare-`printf` skips | 5 |
| `assert_succeeds` exit code | 5 |
| `SKIP_LEAK_CHECK` truthiness | 4 |
| Toolchain pin (6.2) | 6 |
| proptest-regressions untracked (step 2) | 7 |
| Workspace layout (6) | 8 |
| Per-crate stamp (6.1) | 9 |
| pre-push iterates crates (6.3) | 9 |
| `config build` all crates (8.1) | 9 |
| `config doctor` (8.2) | 10 |
| Documentation (8.3) | 10 |
| False-refusal regression test (9.4) | 9 |

Not covered here, and deliberately: `--describe` on the dispatcher (spec step 1)
is a prerequisite for *porting*, and this plan ports nothing, so it moves to
the Step 3 plan where the first port happens. The non-ASCII path and
`setup.sh` word-splitting gaps are closed structurally by Task 3's
hunk-coverage check rather than by separate path-quoting work; the remaining
`setup.sh` item stays in `TODO-AGENTS.md`, since it is a bootstrap fix
unrelated to these two steps.

**Placeholder scan.** No `TBD`, no "add error handling", no "similar to Task
N". Task 5 contains one instruction to read a file rather than showing its
content ("read each of the other five files at the cited line, count the
assertions"). That is deliberate: the correct number of `skip` calls depends on
each file's branch, and inventing counts here would be worse than directing the
implementer to read. The one file whose count matters most,
`githooks-installed.test.sh`, has its full replacement written out.

**Type consistency.** `stamp::fold` and `stamp::parse_members` are defined in
Task 9 and consumed by Task 10 with matching signatures. `doctor::Rendered`
mirrors the field names of the existing `check::Rendered` (`stdout`, `stderr`,
`exit_code`), verified against `check.rs:31-36`. The `SCAN_FAILED` sentinel is
introduced in Task 1 and reused by Task 3 under the same name.

---

## Notes for the executor

- Tasks 1 through 5 are independent of 6 through 10 and can ship in either
  order. Within each half, order matters: Tasks 1, 2, and 3 all edit
  `tests/leak-check.sh` and Task 3 depends on Task 1's sentinel; Task 9
  depends on Task 8's workspace.
- Task 5 will make six suites fail before it fixes them. That is the intended
  red state, not a regression.
- Every commit runs the pre-commit leak guard, which Tasks 1 through 4 modify.
  If a commit in that range is blocked by your own change, that is the test
  telling you something: read the block before reaching for
  `SKIP_LEAK_CHECK=1`.
- Do not push. Pushing is a separate decision, and `config push-all` sends mac
  and linux atomically for reasons documented in
  `.claude/rules/dotfiles-tests.md`.
