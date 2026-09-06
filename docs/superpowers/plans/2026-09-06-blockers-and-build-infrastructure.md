# Blockers and Build Infrastructure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the four verified fail-open blockers in the leak guard and test
harness, then build the workspace and per-crate stamp infrastructure the Rust
migration needs, without adding any migrated logic yet.

**Architecture:** Three groups. Tasks 1-4 fix gates that currently pass
silently; each separates the IO from the decision it was fused to, which is
both the fix and what makes it testable. Tasks 5-7 convert `crates/` into a
cargo workspace with per-crate stamps, pin the toolchain, and add
`config doctor`. Tasks 8-9 cut two measured prompt-latency costs that a
research pass found while investigating the Rust question, and that are worth
more than the migration they were found under. No migrated domain logic lands
here.

**Tech Stack:** zsh (`tests/leak-check.sh`), bash (test harness), POSIX sh
(`.scripts/`), Rust 2024 with clap + anyhow (`crates/`), git plumbing
(`write-tree`, `rev-parse`), Docker (the pre-push suite).

**Spec:** `docs/superpowers/specs/2026-09-06-rust-migration-design.md`

**Revision note:** this plan was rewritten after a six-lens `/expert-review`
panel (`fp-types`, `oo-architecture`, `data-flow`, `bug-hunter`,
`code-simplifier`, `test-coverage`) found five blockers in its first version,
including two that would have broken every commit on this machine. Findings
that changed the design are recorded inline at the task that acts on them, so
the reasoning travels with the work. One panel finding was rejected on
evidence and is recorded in the Rejected Findings section.

## Global Constraints

- No em dashes anywhere. Use a comma, a colon, parentheses, or two hyphens.
- No emoji anywhere.
- No single-letter variable names, except numeric loop indices (`i`, `j`, `k`)
  and math or geometry values. Lambda parameters get no exception.
- Comment why, not what. No comment that restates the code.
- Never `--no-verify`. Never disable a test instead of fixing it.
- Pure core, IO at the edges, dependency-injectable. In Rust: the five modules
  `manifest`, `check`, `plan`, `path`, `tree` currently contain **zero**
  references to `std::fs`, `std::process`, `std::env`, `std::io`, or
  `Command::new` (verified). All 37 IO references live in `git.rs` and
  `main.rs`. New code holds that line.
- **A pure Rust module earns its place only if the Rust binary is the sole
  producer of that value.** This rule came out of the panel and is the reason
  the stamp rule lives in shell (Task 6) while `doctor` lives in Rust
  (Task 8). Apply it to every future module.
- In shell: a function that performs IO must not also decide. It returns its
  result to the caller and the caller decides. This fusion is the direct cause
  of two of the four blockers.
- Tests inject through existing env seams (`LEAK_PATTERN_FILE`,
  `LEAK_ALLOW_FILE`, `DEPS_CONF`, `DOTFILES_ROOT`, `CONFIG_BIN_DIR`,
  `CARGO_TARGET_DIR`). Never read `~/.claude/local/` from a test.
- **Every empty-expected assertion needs a positive control.** The panel found
  four in the first draft with none. An `assert_equals 'no X' '' "$(cmd)"`
  passes when `cmd` breaks for an unrelated reason, so assert first that the
  pipeline produced something, then assert the narrow property.
- `tests/leak-check.sh` is `#!/bin/zsh`: arrays and `${(f)}` are available.
  Two zsh facts that already cost this plan a wrong answer, both verified:
  `status` is a **read-only** variable, so a script assigning to it aborts;
  and `${pipestatus[1]}` does not survive a command substitution assignment,
  because the pipeline runs in a subshell. Do not set `pipefail` globally
  there either: six of its nine grep-terminated pipelines return non-zero on a
  clean scan by design. `tests/lib.sh` and `*.test.sh` are bash, with `set -u`
  and NOT `set -e` (verified), where `status` is an ordinary name. `.scripts/config/*`, `setup.sh`, `.scripts/platform.sh`
  and `.scripts/deps/check-deps.sh` are POSIX sh.
- Every task ends green: `~/tests/run-all.sh` passes before the commit.
- Any new or renamed path must match `TRIGGER_PATHS` in `tests/pre-push:38`,
  or the suite that reads it will not run at pre-push.

---

## What is NOT in this plan, and why

The panel's largest structural finding was that the first draft specified two
Rust modules with **zero production callers**, each wrapped in unit tests.
Recording the resolution here so it is not re-proposed:

- **`stamp::fold` in Rust: deleted before it was written.** `config-build`
  calls `config-stamp` to decide what to compile, so the stamp must be
  computable *before* any binary exists. A Rust owner is a circular
  dependency: the binary that computes the stamp is the binary the stamp
  guards. Shell is therefore the only possible owner, and a second Rust
  implementation would be a format that can silently disagree with the one
  that ships. Task 6 puts the rule in shell, once.
- **`stamp::parse_members` in Rust: same.** The live path enumerates members
  in shell, so a Rust copy is a second parser with no shared test.
- **`doctor` stays Rust**, but redesigned. The first draft had the Rust binary
  shell out to `config-stamp` for expected values, which inverted the layering
  and made the module a pass-through. Task 8 has `doctor` own its whole gather
  through `git.rs`, which makes the binary the sole producer and earns the
  module under the rule above.
- **The four sourced `tmux-*.sh` scripts can never be binaries.** `.zshrc`
  sources `platform.sh`, `tmux-close.sh`, `tmux-setup.sh`, `tmux-split.sh`,
  `tmux-start.sh` and `zsh-git-widgets.sh`; `tmux-split` and `tmux-setup`
  define 9 shell functions between them for the calling shell. A separate
  process cannot define a shell function or mutate its parent's environment.
  This is a mechanism, not a judgment. `tmux-update-window-names.sh` is
  *executed* (`.zshrc:221`), holds the real logic, and is the one that ports.
  That belongs to spec step 5, not to this plan.
- **The harness tally redesign is Task 4's own task, not a note.** The first
  draft said "the counters stay" in one document while a companion design
  replaced them. Task 4 resolves the contradiction by shipping the redesign,
  with the `grep -c` bug the panel found already fixed.

---

## File Structure

**Modified, Tasks 1-4 (blockers):**

| File | Responsibility after this plan |
|---|---|
| `tests/leak-check.sh` | Scan decisions separated from git IO. Fails closed on a failed scan, a missing pattern file, and a path that produced no hunk. |
| `tests/leak-check.test.sh` | The three fail-open regression tests plus `SKIP_LEAK_CHECK` truthiness. |
| `tests/lib.sh` | File-backed tally, pure verdict and summary functions, `finish` as the IO edge. Reports the real exit code. |
| `tests/skip-reporting.test.sh` | Unit tests for the pure verdict, plus the zero-assertion and subshell-counting cases. |
| `tests/githooks-installed.test.sh` plus five more | `skip` rather than a bare `printf`. |

**Modified, Tasks 5-8 (infrastructure):**

| File | Responsibility after this plan |
|---|---|
| `crates/Cargo.toml`, `crates/Cargo.lock`, `crates/rust-toolchain.toml` | New. Workspace root, shared resolution, exact toolchain pin. |
| `.scripts/config/config-stamp` | Sole owner of the stamp rule. Emits per-crate stamps, ref-scoped. |
| `.scripts/config/config-build` | Builds every workspace crate, re-stamps all. |
| `.scripts/config/config-doctor` | New. Thin wrapper for the doctor subcommand. |
| `crates/config-manifest/src/doctor.rs` | New. Pure diagnosis and render. No IO. |
| `crates/config-manifest/src/git.rs` | Gains `workspace_stamps`, the temp-index write-tree read. |
| `tests/pre-push` | Iterates crates, ref-scoped, absolute binary paths. |
| `.claude/rules/dotfiles-tests.md` | Documents the edit-build-test loop. |

---

## Task 1: The leak guard observes what its subshells learned

Two blockers, one mechanism. Both `changed_paths` and `added_lines` detect a
failure and call `exit 2`, but both run only inside `$(...)`, so the exit kills
the subshell and the parent reads an empty capture as "nothing to scan" and
exits 0. Verified by execution: parent survives, capture empty, final exit 0.
`tests/pre-push:96` has a branch for status 2 that this path cannot reach.

The panel found the first draft's separate Tasks 1 and 3 shared this mechanism
and the same caller block, with Task 3 rewriting lines Task 1 had just written.
They are one task.

It also found two defects in the draft's hunk-coverage check, both verified:

1. `TMP_OUT` is **never written in staged mode** (`leak-check.sh:120` pipes
   straight to `grep`), so reading it there compares every staged path against
   an empty diff, marks all unscannable, and exits 2 on **every commit**.
   The check must receive the diff the caller actually captured.
2. `grep -F "$path"` against the whole diff is a substring match. Verified:
   `styles.css` reads as scanned because `vendor/styles.css` appears in the
   diff. That is a fail-open hole inside the fix for a fail-open. The
   anchored form `grep -qxF "+++ b/$path"` distinguishes them; verified that
   it also correctly flags a `-diff` path, which produces
   `Binary files ... differ` and no `+++ b/` header.

**Files:**
- Modify: `tests/leak-check.sh` (both scan functions, the caller block, the trap)
- Test: `tests/leak-check.test.sh`

**Interfaces:**
- Produces: `scan_diff` holds the captured diff text; `unscannable_paths` is a
  pure comparison over two caller-supplied strings. Status 2 continues to mean
  "could not scan" to `tests/pre-push:96`.

- [ ] **Step 1: Write the failing tests**

Append to `tests/leak-check.test.sh`, before its `finish`:

```bash
# A git failure mid-scan must block, not pass. The scan functions run inside a
# command substitution, so an `exit` inside them cannot reach the parent; the
# parent has to observe the failure some other way. Without that, the guard
# prints its own diagnostic and then exits 0.
#
# The shim resolves the real git first rather than hardcoding a path: the
# suite's other calls use whatever git is on PATH, and shadowing it with a
# different build mid-suite is its own confusion.
real_git=$(command -v git)
shim_dir="$FIXTURES/git-shim-fail"
mkdir -p "$shim_dir"
cat > "$shim_dir/git" <<SHIM
#!/bin/sh
# Fail only the -p invocation added_lines makes, so the range still resolves
# and the path list is still produced normally.
for arg in "\$@"; do
    if [ "\$arg" = "-p" ]; then
        printf 'simulated git failure\n' >&2
        exit 128
    fi
done
exec "$real_git" "\$@"
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

# A one-line .gitattributes entry makes an ordinary text file unscannable: git
# prints "Binary files differ" and there are no + lines for the content rules
# to read. The guard must notice a path it listed produced no hunk.
printf 'token = ghp_%s\n' "$(printf 'B%.0s' $(seq 1 24))" > "$repo/hidden.txt"
git -C "$repo" add hidden.txt

status=0
(cd "$repo" && "$LEAK_CHECK" >/dev/null 2>&1) || status=$?
assert_equals 'a credential in a plain staged file is blocked' '1' "$status"

printf 'hidden.txt -diff\n' > "$repo/.gitattributes"
git -C "$repo" add .gitattributes hidden.txt

output=$(cd "$repo" && "$LEAK_CHECK" 2>&1)
status=$?
assert_equals 'a -diff marked path does not pass silently' '2' "$status"
assert_contains 'the block names the unscannable path' "$output" 'hidden.txt'

# The substring hazard, in the one shape that distinguishes the two
# implementations. A shorter path must not read as scanned because a longer
# path containing it has a header.
#
# The gitattributes pattern is anchored with a leading slash on purpose: a
# bare `styles.css` matches at ANY depth, so it would unset diff for
# vendor/styles.css too and both paths would be unscannable, which both the
# buggy and the fixed form report identically. Verified with
# `git check-attr diff styles.css vendor/styles.css`.
#
# With only styles.css unset: the substring form finds "styles.css" inside
# "+++ b/vendor/styles.css" and lets the credential through, and the anchored
# form blocks. Verified both directions before writing this test.
git -C "$repo" reset -q HEAD hidden.txt .gitattributes
rm -f "$repo/hidden.txt" "$repo/.gitattributes"
mkdir -p "$repo/vendor"
printf 'token = ghp_%s\n' "$(printf 'D%.0s' $(seq 1 24))" > "$repo/styles.css"
printf 'plain\n' > "$repo/vendor/styles.css"
printf '/styles.css -diff\n' > "$repo/.gitattributes"
git -C "$repo" add styles.css vendor/styles.css .gitattributes

output=$(cd "$repo" && "$LEAK_CHECK" 2>&1)
status=$?
assert_equals 'a -diff path is blocked even when a longer path shares its name' \
    '2' "$status"
assert_contains 'the block names the short path, not the long one' \
    "$output" 'styles.css'

git -C "$repo" reset -q HEAD styles.css vendor/styles.css .gitattributes
rm -rf "$repo/styles.css" "$repo/vendor" "$repo/.gitattributes"
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `~/tests/leak-check.test.sh`
Expected: FAIL on `a git failure mid-scan exits 2, not 0` (reporting `0`) and
on `a -diff marked path does not pass silently` (reporting `0`).

- [ ] **Step 3: Write the implementation**

Do NOT set `pipefail` globally in this file, and do not read `pipestatus`.
Both were in an earlier draft of this plan and both are wrong. Verified:

- **`${pipestatus[1]}` does not work through a command substitution
  assignment.** `out=$(f | grep ...)` runs the pipeline in a subshell, so the
  outer `pipestatus` describes the assignment, not `f`. Measured: it returned
  `0` where the function returned `2`, which silently loses exactly the status
  this task exists to propagate.
- **Global `pipefail` arms a trap in six unrelated pipelines.**
  `tests/leak-check.sh` has nine pipelines ending in `grep`, and six of them
  want no match on a clean scan (the credential rules at lines 152, 158, 163,
  the term scope at 185, the term rules at 197). Under `pipefail` each returns
  non-zero on clean input. They sit inside `$(...)` so nothing checks them
  today, which makes the option harmless now and a false failure the moment
  anyone adds a status check.

The form that works needs neither. Capture the function's output **without a
pipe**, so `$?` is the function's own status, then filter in a separate step.
Verified: `0` on the clean path, `2` on the failing path.

Change both scan functions to return a status instead of exiting, and to
truncate-then-append so multiple `xargs` batches accumulate rather than each
clobbering the previous (BSD `xargs` batches at roughly 5000 arguments, and
the existing single `>` keeps only the final batch):

```zsh
changed_paths() {
  if [ "$mode" = push ]; then
    : > "$TMP_OUT"
    if ! git log --format= --name-only "${RANGE_LOG_FLAGS[@]}" "$range" \
        >> "$TMP_OUT" 2>"$TMP_ERR"; then
      echo "leak-check: git log failed scanning changed paths for $range" >&2
      cat "$TMP_ERR" >&2
      return 2
    fi
    grep -v '^$' "$TMP_OUT" | sort -u
  else
    git diff --cached --name-only --diff-filter=ACMR
  fi
}

# The diff text for the given paths (read from stdin, one per line).
#
# Returns the whole diff rather than only the added lines, because the caller
# needs both: the content rules read the + lines, and the hunk-coverage check
# needs the file headers. Returning one value that serves both keeps a single
# git invocation and removes the TMP_OUT aliasing the two callers had.
scan_diff() {
  if [ "$mode" = push ]; then
    : > "$TMP_OUT"
    if ! tr '\n' '\0' | xargs -0 git log --format= --no-color -U0 \
        "${RANGE_LOG_FLAGS[@]}" -p "$range" -- >> "$TMP_OUT" 2>"$TMP_ERR"; then
      echo "leak-check: git log failed scanning added lines for $range" >&2
      cat "$TMP_ERR" >&2
      return 2
    fi
    cat "$TMP_OUT"
  else
    tr '\n' '\0' | xargs -0 git diff --cached --no-color -U0 -- 2>/dev/null
  fi
}

# The scanned paths that produced no file header in the diff.
#
# Pure: both inputs are caller-supplied, so this makes no git call and is
# exercisable without a repository.
#
# Matched against the anchored `+++ b/<path>` header rather than by searching
# the diff for the path anywhere. A free-text search reports a path as scanned
# when a longer path containing it appears (styles.css satisfied by
# vendor/styles.css, verified), which is a hole in the shape this check exists
# to close.
#
# A path with no header was not scanned. Two evasions share that signature: a
# .gitattributes `-diff` marking on ordinary text, and a genuinely binary
# file. Both produce "Binary files ... differ" and no header.
unscannable_paths() {
  local path_list=$1 diff_text=$2
  local headers
  # The header set is extracted once, then compared as sets. Re-piping the
  # whole diff through a fresh grep per path is quadratic: measured at 3.7
  # seconds for 400 paths against a small diff, and pre-push runs this per
  # pushed range with two refs on a `config push-all`.
  #
  # Both header spellings are kept: `+++ b/path` is the default, and
  # `+++ path` is what --no-prefix output produces.
  headers=$(printf '%s\n' "$diff_text" \
    | sed -n -e 's|^+++ b/\(.*\)$|\1|p' -e 's|^+++ \([^b].*\)$|\1|p' \
    | grep -v '^/dev/null$' \
    | sort -u)
  printf '%s\n' "$path_list" | sort -u | comm -23 - <(printf '%s\n' "$headers")
}
```

Then rewrite the caller block. Each capture is unpiped so `$?` is the
function's own status, and the self-exclusion filter is a separate step. No
sentinel file, no global option, no `pipestatus`.

Note `scan_status` rather than `status`: **`status` is a read-only variable in
zsh** (verified), so assigning to it aborts the script. That matters only in
this file, since `tests/lib.sh` and the test suites are bash where `status` is
an ordinary name.

```zsh
raw_paths=$(changed_paths)
scan_status=$?
[ "$scan_status" -eq 0 ] || exit 2

# The self-exclusion is a separate step, not a pipe on the capture above:
# piping would make $? describe grep rather than changed_paths.
#
# This script's own source contains the generic patterns it searches for, so
# scanning it would always self-trip. It is the only path excluded in either
# mode, and a change to this file is therefore unguarded and depends on human
# review, so do not add another file here without the same tradeoff in mind.
scan_paths=$(printf '%s\n' "$raw_paths" | grep -v '^tests/leak-check\.sh$')
[ -z "$scan_paths" ] && exit 0

diff_text=$(printf '%s\n' "$scan_paths" | scan_diff)
scan_status=$?
[ "$scan_status" -eq 0 ] || exit 2

unscannable=$(unscannable_paths "$scan_paths" "$diff_text")
if [ -n "$unscannable" ]; then
  echo "" >&2
  echo "  $hook: BLOCKED, these paths produced no readable diff:" >&2
  printf '    %s\n' "${(@f)unscannable}" >&2
  echo "" >&2
  echo "  A path with no diff header was not scanned. Causes: a" >&2
  echo "  .gitattributes -diff marking, or a binary file." >&2
  echo "  Remove the marking, or move the content out of the repo." >&2
  echo "" >&2
  exit 2
fi

staged=$(printf '%s\n' "$diff_text" | grep '^+' | grep -v '^+++')
[ -z "$staged" ] && exit 0
```

Note `"${(@f)unscannable}"` is quoted so a path containing a glob character is
not expanded, which the panel flagged in the unquoted form.

Also extend the trap to the signals `lib.sh` already covers. The existing trap
is EXIT only, so a Ctrl+C mid-scan leaks both temp files:

```zsh
trap 'rm -f "$TMP_OUT" "$TMP_ERR"' EXIT
trap 'rm -f "$TMP_OUT" "$TMP_ERR"; exit 130' INT
trap 'rm -f "$TMP_OUT" "$TMP_ERR"; exit 143' TERM HUP
```

Finally, the term-scope scan later in the file calls `added_lines` a second
time. Replace that call with a reuse of `$diff_text` filtered to the term
scope, so there is one git invocation and no temp-file aliasing between the
two readers.

- [ ] **Step 4: Run tests to verify they pass**

Run: `~/tests/leak-check.test.sh`
Expected: PASS, including all five new assertions.

Confirm the guard still passes on real staged content in both modes:

Run: `cd ~ && GIT_DIR=$HOME/.cfg GIT_WORK_TREE=$HOME zsh tests/leak-check.sh; echo $?`
Expected: `0`.

Run: `~/tests/run-all.sh leak-check`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
config add tests/leak-check.sh tests/leak-check.test.sh
config commit -m "Make the leak guard fail closed, and block what it cannot read

Two fail-opens with one cause: the scan functions detected a git failure and
called exit 2, but both run only inside a command substitution, so the exit
killed the subshell and the parent read an empty capture as \"nothing to
scan\" and exited 0. The guard printed the correct diagnostic and then passed
the push. tests/pre-push:96 already treats status 2 as \"could not scan, push
blocked\"; that branch was unreachable.

The functions now return a status and the caller captures their output without
a pipe, so \$? is the function's own status and no sentinel file is needed.

Two forms were tried and rejected on measurement. \`\${pipestatus[1]}\` does
not survive a command substitution assignment: it reported 0 where the
function returned 2, which loses the very status this fixes. A global
\`pipefail\` would arm six unrelated pipelines whose clean-scan outcome is
grep finding nothing.

Also blocks a path that produced no diff header, which closes the
.gitattributes -diff evasion and the binary-file gap with one mechanism.
Verified before the fix: a credential-shaped string was blocked in a plain
file and allowed once one .gitattributes line marked that file.

The header match is anchored to \`+++ b/<path>\` rather than searching the
diff for the path. A free-text search reported styles.css as scanned because
vendor/styles.css appeared in the diff, which would have left a hole in
exactly the shape this check exists to close.

Truncates-then-appends so multiple xargs batches accumulate instead of each
clobbering the previous. BSD xargs batches at roughly 5000 arguments, so the
single redirect kept only the final batch's diff. Extends the trap to INT,
TERM and HUP, matching tests/lib.sh."
```

---

## Task 2: The guard's skip policy fails closed

Two single-line predicate changes, no shared state with Task 1.

**Blocker: a missing pattern file deactivates Layer 2.** Layer 1 matches
credential shapes; Layer 2 is the only layer defending employer and project
terms, which is the reason a public repo needs this guard. Verified by
differential test on identical content: pattern file present exits 1, absent
exits 0. The file is untracked by design, so absent-by-default is the state of
every fresh machine, which is exactly when setup work is committed.

**`SKIP_LEAK_CHECK=0` disables the guard.** `[ -n ]` is true for the string
`0`. Verified for `1`, `0`, and `false`.

The panel noted status 2 would then mean two different things to
`tests/pre-push:96` ("could not scan" and "no pattern file"), with different
remediations. This task uses status 3 for the configuration error and teaches
the hook to distinguish them.

**Files:**
- Modify: `tests/leak-check.sh` (the `SKIP_LEAK_CHECK` test, the pattern-file block)
- Modify: `tests/pre-push` (distinguish status 3)
- Test: `tests/leak-check.test.sh`

**Interfaces:**
- Produces: `LEAK_ALLOW_NO_PATTERNS=1` opt-out. Status 3 means "the guard is
  misconfigured", distinct from status 2 "the guard could not scan".

- [ ] **Step 1: Write the failing tests**

Append to `tests/leak-check.test.sh`:

```bash
# Layer 2 is the only layer defending employer and project terms, and the
# pattern file it reads is untracked on purpose, so it is absent by default on
# every fresh machine. A missing file must block rather than silently reduce
# the guard to its credential rules.
printf 'internal note about %s\n' "$FAKE_TERM" > "$repo/term-only.txt"
git -C "$repo" add term-only.txt

status=0
(cd "$repo" && LEAK_PATTERN_FILE="$FIXTURES/no-such-patterns.conf" \
    "$LEAK_CHECK" >/dev/null 2>&1) || status=$?
assert_equals 'a missing pattern file is a configuration error' '3' "$status"

status=0
(cd "$repo" && LEAK_PATTERN_FILE="$FIXTURES/no-such-patterns.conf" \
    LEAK_ALLOW_NO_PATTERNS=1 \
    "$LEAK_CHECK" >/dev/null 2>&1) || status=$?
assert_equals 'the explicit opt-out permits a run with no pattern file' '0' "$status"

git -C "$repo" reset -q HEAD term-only.txt
rm -f "$repo/term-only.txt"

# The sanctioned bypass of the repo's primary control should not fire on a
# value that reads as "do not skip", and an unrecognized value should say so
# rather than silently declining to skip.
printf 'token = ghp_%s\n' "$(printf 'C%.0s' $(seq 1 24))" > "$repo/skip-probe.txt"
git -C "$repo" add skip-probe.txt

status=0
(cd "$repo" && SKIP_LEAK_CHECK=1 "$LEAK_CHECK" >/dev/null 2>&1) || status=$?
assert_equals 'SKIP_LEAK_CHECK=1 skips' '0' "$status"

status=0
(cd "$repo" && SKIP_LEAK_CHECK=0 "$LEAK_CHECK" >/dev/null 2>&1) || status=$?
assert_equals 'SKIP_LEAK_CHECK=0 does not skip' '1' "$status"

output=$(cd "$repo" && SKIP_LEAK_CHECK=maybe "$LEAK_CHECK" 2>&1 || true)
assert_contains 'an unrecognized skip value is announced' "$output" 'not a recognized'

git -C "$repo" reset -q HEAD skip-probe.txt
rm -f "$repo/skip-probe.txt"
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `~/tests/leak-check.test.sh`
Expected: FAIL on `a missing pattern file is a configuration error` (reporting
`0`), on `SKIP_LEAK_CHECK=0 does not skip` (reporting `0`), and on
`an unrecognized skip value is announced`.

- [ ] **Step 3: Write the implementation**

Replace the `SKIP_LEAK_CHECK` test in `tests/leak-check.sh`:

```zsh
# Matched against true values rather than tested for non-emptiness: `[ -n ]`
# is true for the string "0", so SKIP_LEAK_CHECK=0 disabled the guard for
# anyone who meant the opposite. An unrecognized value is announced rather
# than silently declining, because the person who set it believes the guard is
# off and would not understand the block.
case "${SKIP_LEAK_CHECK:-}" in
  '') ;;
  1|true|TRUE|True|yes|YES|Yes)
    echo "$hook: leak check SKIPPED via SKIP_LEAK_CHECK" >&2
    exit 0
    ;;
  *)
    echo "$hook: SKIP_LEAK_CHECK=$SKIP_LEAK_CHECK is not a recognized true value, scanning anyway" >&2
    ;;
esac
```

Replace the pattern-file block:

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
    #
    # Status 3, not 2: pre-push reports 2 as "could not scan this range",
    # which is a different problem with a different fix.
    echo "" >&2
    echo "  $hook: BLOCKED, no readable pattern file at $PATTERN_FILE" >&2
    echo "  The project term rules cannot run, so this scan is incomplete." >&2
    echo "  Restore the file (see ~/DOTFILES-GL.md), or set" >&2
    echo "  LEAK_ALLOW_NO_PATTERNS=1 for a machine with no terms to defend." >&2
    echo "" >&2
    exit 3
  fi
else
```

In `tests/pre-push`, teach the range loop to name status 3 distinctly:

```sh
    if [ "$leak_status" -eq 3 ]; then
        printf '\npre-push: leak check is misconfigured, push blocked.\n' >&2
        exit 1
    elif [ "$leak_status" -eq 2 ]; then
        printf '\npre-push: leak check could not scan %s, push blocked.\n' "$range" >&2
        exit 1
    elif [ "$leak_status" -ne 0 ]; then
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `~/tests/leak-check.test.sh`
Expected: PASS.

Run: `~/tests/run-all.sh`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
config add tests/leak-check.sh tests/pre-push tests/leak-check.test.sh
config commit -m "Fail closed on a missing leak pattern file and a false skip value

Layer 1 matches credential shapes. Layer 2 is the only layer defending
employer and project terms, which is the reason a public dotfiles repo needs
this guard. A missing pattern file silently deactivated layer 2 and exited 0.
Verified by differential test: identical content was blocked with the file
present and allowed with it absent. The file is untracked on purpose, so
absent-by-default is the state of every fresh machine, and that is exactly
when setup work gets committed.

Status 3 rather than 2, because pre-push reports 2 as \"could not scan this
range\", which is a different problem with a different fix. The hook now names
both.

Also skips only on a recognized true value: [ -n ] is true for the string
\"0\", so SKIP_LEAK_CHECK=0 and =false both disabled the guard. An
unrecognized value is announced rather than silently declining, because the
person who set it believes the guard is off."
```

---

## Task 3: `assert_succeeds` reports the real exit code

`failed=$((failed + 1))` runs before the `printf` reads `$?`, so the
arithmetic's status is what gets printed. Verified: a command exiting 42
reports "exited 0". `assert_succeeds` has 170+ call sites, and the failures
that matter most are the ones from a container run that cannot be reproduced
interactively.

Separated from Task 4 because it is a two-line ordering fix with no dependency
on the state model, and Task 4 rewrites that model.

**Files:**
- Modify: `tests/lib.sh` (`assert_succeeds`)
- Test: `tests/skip-reporting.test.sh`

- [ ] **Step 1: Write the failing test**

Append to `tests/skip-reporting.test.sh`, before its `finish`. Reuse the
existing `write_suite` helper rather than adding a third heredoc idiom to the
same file:

```bash
# assert_succeeds must report the real exit code. failed=$((failed + 1)) ran
# before the printf read $?, so every failure across 170+ call sites reported
# "exited 0", losing the one diagnostic a container-only failure depends on.
See the note below: `write_suite` already exists with the signature
`write_suite <path> <body>`.
```

`write_suite` **already exists** in that file at line 41, with a different
contract: `write_suite <path> <body>`, returning nothing. Seven call sites use
it (lines 53, 75, 85, 98, 111, 131, 133). Verified.

Use the existing one rather than redefining it. The call becomes:

```bash
rc_suite="$FIXTURES/rc.test.sh"
write_suite "$rc_suite" "assert_succeeds 'a command that exits 42' sh -c 'exit 42'"

output=$(bash "$rc_suite" 2>&1 || true)
assert_contains 'assert_succeeds reports the real exit code' "$output" 'exited 42'
```

Do not introduce a second `write_suite` shape. An earlier draft of this plan
said "if it does not exist, define it", which was wrong: following that
literally writes a file named `rc` into the repo root and then runs
`bash ""`.

- [ ] **Step 2: Run test to verify it fails**

Run: `~/tests/skip-reporting.test.sh`
Expected: FAIL on `assert_succeeds reports the real exit code`.

- [ ] **Step 3: Write the implementation**

In `tests/lib.sh`:

```bash
assert_succeeds() {
    local description=$1; shift
    local status=0
    "$@" >/dev/null 2>&1 || status=$?
    if [ "$status" -eq 0 ]; then
        record_outcome pass
        printf 'ok: %s\n' "$description"
    else
        # Captured before anything else runs: the counter increment reset $?,
        # so reading it afterwards reported the increment's status and every
        # failure in the suite claimed "exited 0".
        record_outcome fail
        printf 'FAIL: %s (exited %d)\n' "$description" "$status"
    fi
}
```

`record_outcome` arrives in Task 4. Until then use `passed=$((passed + 1))`
and `failed=$((failed + 1))` as today; the ordering fix is independent.

- [ ] **Step 4: Run test to verify it passes**

Run: `~/tests/skip-reporting.test.sh`
Expected: PASS.

Run: `~/tests/run-all.sh`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
config add tests/lib.sh tests/skip-reporting.test.sh
config commit -m "Report the real exit code from assert_succeeds

The counter increment ran before the printf read \$?, so the arithmetic's
status was printed and all 170+ call sites reported \"exited 0\" on failure.
Verified: a command exiting 42 reported 0.

That is the one diagnostic a container-only failure depends on, since the
pre-push suite runs in Docker and cannot be reproduced interactively."
```

---

## Task 4: A pure verdict, and a suite that asserts nothing fails

`tests/lib.sh:264` ends `finish` with `[ "$failed" -eq 0 ]`, true when nothing
ran. Under `run-all.sh -q`, which the pre-push hook shows, that is identical to
a real pass. Verified: a suite whose body is only `finish` prints
`0 passed, 0 failed` and exits 0.

The live instance is `githooks-installed.test.sh:26-30`: a bare `printf`
instead of `skip`, then `finish; exit 0`. `.github/workflows/test-suite.yml:139`
sets `DOTFILES_ROOT` to the checkout workspace, which has `.git` and not
`.cfg`, and the container has no `.cfg` either. So the suite that exists to
catch "the hooks are not installed" runs its 7 assertions on one machine only.
Six suites skip this way (confirmed by grep).

This task also replaces the three module-global counters, which is what makes
the verdict testable. The panel's findings, all folded in:

- The counters are private to `lib.sh` (verified: nothing outside reads them),
  so the state model can change without touching 49 suites.
- A file substrate is right, and not only for purity: assertions run inside
  command substitutions in several suites, and a subshell cannot increment its
  parent's variable, so the current counters silently lose those. `SESSION_LIST`
  sets the precedent for the same reason.
- **`grep -c` prints `0` and exits 1**, so `$(grep -c ... || printf '0')`
  yields the two-line string `"0\n0"` and breaks the arithmetic. Verified.
  The file always exists after `: > "$TALLY"`, so no fallback is needed.
- The verdict stays a **string**, not an exit status. A status-returning
  verdict reintroduces the exact bug Task 3 fixes: `$?` is destroyed by any
  intervening command, so one inserted line would silently turn "empty" into
  "pass". A string on stdout cannot be clobbered.
- The `case` needs a default arm. Shell has no exhaustiveness check, so
  without it a typo in `verdict_for` falls through and returns 0, which is the
  fail-open shape this whole plan closes.
- **`finish` must never truncate the tally.** 10+ suites call it more than
  once and `run-all.sh:119` reads `tail -n1`, so the cumulative reading is the
  existing contract. Verified by probe: a second `finish` reports growing
  totals.

**Files:**
- Modify: `tests/lib.sh` (tally, `verdict_for`, `summary_for`, `finish`, all
  four assertion functions, `skip`)
- Modify: `tests/githooks-installed.test.sh` plus five more
- Test: `tests/skip-reporting.test.sh`

**Interfaces:**
- Produces:
  - `record_outcome <pass|fail|skip>` appends one line to `$TALLY`.
  - `verdict_for <pass> <fail> <skip>` prints `pass`, `fail`, or `empty`. Pure.
  - `summary_for <name> <pass> <fail> <skip>` prints the summary line. Pure,
    and the single owner of the format `run-all.sh:119` parses.
  - `finish` reads the tally, prints, returns 0 or 1. The only impure piece.

- [ ] **Step 1: Write the failing tests**

Append to `tests/skip-reporting.test.sh`:

```bash
# The verdict is now a pure function of three counts, so these are table tests
# with no subprocess and no fixture. That is the thing the module-global
# counters made impossible.
assert_equals 'a passing tally is pass'        'pass'  "$(verdict_for 1 0 0)"
assert_equals 'any failure is fail'            'fail'  "$(verdict_for 3 1 0)"
assert_equals 'a skip-only tally still passes' 'pass'  "$(verdict_for 0 0 1)"
assert_equals 'nothing at all is empty'        'empty' "$(verdict_for 0 0 0)"

# The format run-all.sh parses lives in one function. Asserted through the
# exact expression run-all.sh uses, so the two cannot drift apart silently.
summary=$(summary_for 'probe' 1 2 3)
assert_equals 'the summary names all three counts' \
    'probe: 1 passed, 2 failed, 3 skipped' "$summary"
assert_equals 'run-all.sh can extract the skip count from it' '3' \
    "$(printf '%s\n' "$summary" \
        | sed -n 's/^[^:]*: [0-9]* passed, [0-9]* failed, \([0-9]*\) skipped$/\1/p')"
assert_equals 'a clean summary omits the skip count' \
    'probe: 1 passed, 0 failed' "$(summary_for 'probe' 1 0 0)"

# A suite that runs no assertions is not a passing suite.
empty_suite="$FIXTURES/empty.test.sh"
write_suite "$empty_suite" ''
output=$(bash "$empty_suite" 2>&1)
status=$?
assert_equals 'a zero-assertion suite exits non-zero' '1' "$status"
assert_contains 'the verdict says no assertions ran' "$output" 'no assertions ran'

# An assertion inside a command substitution must still count. This is the
# latent bug the tally file fixes: a subshell cannot increment its parent's
# variable, so these vanished silently.
sub_suite="$FIXTURES/subshell.test.sh"
write_suite "$sub_suite" 'result=$(assert_equals "inside a substitution" a a)'
output=$(bash "$sub_suite" 2>&1)
assert_contains 'a subshell assertion reaches the tally' "$output" '1 passed'
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `~/tests/skip-reporting.test.sh`
Expected: FAIL to find `verdict_for`, then FAIL on the zero-assertion and
subshell assertions.

- [ ] **Step 3: Write the implementation**

In `tests/lib.sh`, replace the three counters at lines 50-52:

```bash
# One line per assertion outcome: "pass", "fail" or "skip".
#
# A file rather than three counters because assertions run inside command
# substitutions in several suites, and a subshell cannot increment its
# parent's variable, so those outcomes vanished silently. This is the same
# reason SESSION_LIST is a file, and the same failure it prevents: a count
# that quietly stops rising.
TALLY="$FIXTURES/.tally"
: > "$TALLY"

record_outcome() {
    printf '%s\n' "$1" >> "$TALLY"
}
```

Add the two pure functions next to `finish`:

```bash
# The verdict for a tally, as a value on stdout. Reads no globals, performs no
# IO, and is callable directly from a test.
#
# A string rather than an exit status on purpose. A status-returning verdict
# would reintroduce the bug assert_succeeds had for years: $? is destroyed by
# any intervening command, so one line inserted between the call and the read
# would silently turn "empty" into "pass". A string cannot be clobbered.
#
# "empty" is a distinct verdict rather than a kind of pass, because a suite
# that ran nothing is not a suite that passed. `skip` already fixed the
# within-suite case; this is the whole-suite case.
verdict_for() {
    local pass_count=$1 fail_count=$2 skip_count=$3
    if [ $((pass_count + fail_count + skip_count)) -eq 0 ]; then
        printf 'empty'
    elif [ "$fail_count" -gt 0 ]; then
        printf 'fail'
    else
        printf 'pass'
    fi
}

# The summary line for a tally, as a value.
#
# The single owner of the format run-all.sh parses with a regex. A test
# asserts that regex against this output, so the two cannot drift apart
# silently.
#
# The skip count is appended only when there is one. A trailing "0 skipped" on
# every clean suite is noise, and noise is what a reader learns to scan past,
# which is the habit this mechanism exists to interrupt.
summary_for() {
    local name=$1 pass_count=$2 fail_count=$3 skip_count=$4
    local line
    line=$(printf '%s: %d passed, %d failed' "$name" "$pass_count" "$fail_count")
    [ "$skip_count" -eq 0 ] || line="$line, $skip_count skipped"
    printf '%s' "$line"
}
```

Replace `finish`:

```bash
# Reads the tally, prints the summary, returns the exit status. The IO edge,
# and the only function here that touches state.
#
# Read-only: several suites call finish more than once, in an early-skip
# branch and again at the end, and run-all.sh reads the last summary line. So
# the tally is truncated exactly once, at source time, and finish never
# truncates it. Calling finish twice prints growing cumulative totals, which
# is the existing behavior and what run-all.sh's `tail -n1` expects.
#
# No `|| printf '0'` on the counts: grep -c prints 0 and exits 1 when there
# are no matches, so the fallback would append a second 0 and break the
# arithmetic. The file always exists after the truncate above.
finish() {
    local pass_count fail_count skip_count summary outcome
    pass_count=$(grep -c '^pass$' "$TALLY")
    fail_count=$(grep -c '^fail$' "$TALLY")
    skip_count=$(grep -c '^skip$' "$TALLY")

    summary=$(summary_for "$TEST_NAME" "$pass_count" "$fail_count" "$skip_count")
    printf '\n%s\n' "$summary"

    outcome=$(verdict_for "$pass_count" "$fail_count" "$skip_count")
    case $outcome in
        pass) return 0 ;;
        fail) return 1 ;;
        empty)
            printf '%s: no assertions ran\n' "$TEST_NAME" >&2
            return 1
            ;;
        # Shell has no exhaustiveness check, so without this arm a typo in
        # verdict_for falls through and finish returns 0, which is the
        # fail-open shape this change exists to close.
        *)
            printf '%s: lib.sh bug, unknown verdict: %s\n' "$TEST_NAME" "$outcome" >&2
            return 2
            ;;
    esac
}
```

Replace all nine counter increments in `assert_equals`, `assert_contains`,
`assert_succeeds` and `skip` with `record_outcome pass` / `fail` / `skip`.

Then fix the six bare-`printf` skips. In `githooks-installed.test.sh:26-30`:

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

For the other five (`alacritty-platform-split.test.sh:126`,
`workflow-labels.test.sh:25`, `zshrc-node-startup.test.sh:40,44,66`,
`zshrc-python-startup.test.sh:38,42`, `zshrc-platform-split.test.sh:113`),
read each block, count the assertions it skips, and add that many `skip` calls
carrying the reason text the `printf` had. Do not guess the counts: read the
blocks.

- [ ] **Step 4: Run tests to verify they pass**

Run: `~/tests/skip-reporting.test.sh`
Expected: PASS.

Run: `~/tests/run-all.sh`
Expected: PASS, with skip counts now on the affected suites.

Run: `~/tests/run-in-docker.sh`
Expected: PASS. This is the environment where the six suites were silently
contributing nothing, so it is the run that proves the fix.

- [ ] **Step 5: Commit**

```bash
config add tests/lib.sh tests/skip-reporting.test.sh \
    tests/githooks-installed.test.sh tests/alacritty-platform-split.test.sh \
    tests/workflow-labels.test.sh tests/zshrc-node-startup.test.sh \
    tests/zshrc-python-startup.test.sh tests/zshrc-platform-split.test.sh
config commit -m "Fail a suite that runs no assertions, and make the verdict pure

finish ended with [ \$failed -eq 0 ], which is true when nothing ran. Under
run-all.sh -q, which is what the pre-push hook shows, that was identical to a
real pass.

The live instance was githooks-installed.test.sh, which skipped with a bare
printf and then called finish. test-suite.yml sets DOTFILES_ROOT to the
checkout workspace, which has .git and not .cfg, and the container has no .cfg
either, so the suite that exists to catch \"the hooks are not installed\" ran
its 7 assertions on one machine only. Six suites skipped this way; all now use
skip, so the count reaches the summary line. A skip still passes: a skip is an
explicit statement with a reason, and silence is not.

The three module-global counters become a tally file, which is what makes the
verdict a pure function of three numbers and therefore a table test. It also
fixes a latent bug: assertions inside command substitutions could not
increment their parent's variable, so those outcomes vanished silently. That
is the same reason SESSION_LIST is a file.

The verdict is a string, not an exit status. A status would reintroduce the
bug assert_succeeds had for years, where \$? is destroyed by any intervening
command. The case has a default arm because shell has no exhaustiveness check,
and without it a typo would return 0.

finish never truncates the tally: several suites call it twice and
run-all.sh reads the last summary line, so cumulative reading is the existing
contract."
```

---

## Task 5: Workspace, toolchain pin, and a narrower stamped tree

Three changes that all narrow the build's declared inputs to its true inputs.
The panel found the first draft's separate tasks for these were a full 5-step
cycle each for one file creation and one `git rm --cached`, so they are folded.

**Toolchain pin.** The stamp covers source, not compiler. `Cargo.toml` has
`edition = "2024"` and no `rust-version`, and `edition` is not a pin: every
toolchain from 1.85 onward compiles it. So mac and linux can produce matching
stamps from different compilers.

**Untrack the proptest seed.** `crates/config-manifest/proptest-regressions/plan.txt`
is tracked inside the stamped tree, so a failure seed written by a test run
moves the stamp and forces a rebuild whose output is byte-identical.
Over-declared inputs are the tax that makes a gate get bypassed.

**Workspace.** One lockfile, one resolution, one build.

**Files:**
- Create: `crates/Cargo.toml`, `crates/rust-toolchain.toml`
- Move: `crates/config-manifest/Cargo.lock` to `crates/Cargo.lock`
- Modify: `crates/config-manifest/Cargo.toml`, `crates/config-manifest/.gitignore`
- Modify: `.scripts/config/config-build`
- Test: `tests/config-manifest-lifecycle.test.sh`

**Interfaces:**
- Produces: `crates/Cargo.toml` with `[workspace] members`. Task 6 reads that
  list to enumerate crates. The binary still installs to the same path and
  `--stamp` still prints one value, so `tests/pre-push` keeps working until
  Task 6 changes it.

- [ ] **Step 1: Write the failing tests**

Append to `tests/config-manifest-lifecycle.test.sh`, before `finish`. Note
every empty-expected assertion is preceded by a positive control, per the
Global Constraints:

```bash
# The stamp covers source, not compiler. edition = "2024" is not a pin: every
# toolchain from 1.85 onward compiles it, so mac and linux could produce
# matching stamps from different compilers with the gate seeing no difference.
TOOLCHAIN_FILE="$DOTFILES_ROOT/crates/rust-toolchain.toml"
assert_succeeds 'the toolchain is pinned' test -f "$TOOLCHAIN_FILE"
assert_succeeds 'the pin names an exact version, not a channel' \
    grep -qE '^channel = "1\.[0-9]+\.[0-9]+"' "$TOOLCHAIN_FILE"

pinned=$(sed -n 's/^channel = "\(.*\)"/\1/p' "$TOOLCHAIN_FILE")
installed=$(rustc --version | awk '{print $2}')
assert_equals 'the pinned toolchain is the one installed' "$pinned" "$installed"

# One workspace, one lockfile, one resolution. The members list is also what
# the stamp enumerates, so it is the single place that says which crates exist.
WORKSPACE_MANIFEST="$DOTFILES_ROOT/crates/Cargo.toml"
assert_succeeds 'a workspace root exists' test -f "$WORKSPACE_MANIFEST"
assert_succeeds 'the workspace declares members' \
    grep -q '^members = \[' "$WORKSPACE_MANIFEST"
assert_succeeds 'the shared lockfile is at the workspace root' \
    test -f "$DOTFILES_ROOT/crates/Cargo.lock"

cfg_git() {
    git --git-dir="$DOTFILES_ROOT/.cfg" --work-tree="$DOTFILES_ROOT" "$@"
}

# Positive control first. An empty-expected assertion passes when its pipeline
# breaks for an unrelated reason, so prove the pipeline reaches the repo before
# asserting the narrow property.
assert_succeeds 'ls-files reaches the crates tree' \
    test -n "$(cfg_git ls-files 'crates/*')"
assert_equals 'no per-crate lockfile remains' '' \
    "$(cfg_git ls-files 'crates/*/Cargo.lock')"
assert_equals 'no proptest regressions file is tracked' '' \
    "$(cfg_git ls-files 'crates/*/proptest-regressions/*')"
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `~/tests/config-manifest-lifecycle.test.sh`
Expected: FAIL on `the toolchain is pinned`, `a workspace root exists`,
`no per-crate lockfile remains`, and `no proptest regressions file is tracked`.

- [ ] **Step 3: Write the implementation**

Find the current toolchain:

```bash
rustc --version
```

Create `crates/rust-toolchain.toml`, substituting the version reported:

```toml
# An exact pin, not a channel.
#
# The build stamp is a git tree id over the crate source, so it says nothing
# about which compiler produced the binary, and `edition = "2024"` is not a
# pin either: every toolchain from 1.85 onward compiles it. Without this file
# mac and linux can push matching stamps from different compilers, and the
# pre-push gate cannot see the difference.
#
# Lives inside crates/ so the stamp's `git add -- crates` covers it and a pin
# change invalidates every binary, which is the intended behavior.
[toolchain]
channel = "1.94.0"
components = ["rustfmt", "clippy"]
```

Create `crates/Cargo.toml`:

```toml
# Workspace root for the dotfiles crates.
#
# One lockfile and one resolution, so a dependency exists at one version
# across every binary. The members list is also what `config stamp` and
# `config build` enumerate, which makes this the single place that says which
# crates exist.
#
# Keep members on one line. `.scripts/config/config-stamp` reads this list
# with sed, because it must work before any binary exists, and a multi-line
# array would silently yield no members. config-stamp aborts on an empty list
# rather than iterating zero crates.
[workspace]
resolver = "3"
members = ["config-manifest"]

[workspace.dependencies]
anyhow = "1.0.104"
clap = { version = "4.6.6", features = ["derive", "env"] }
assert_cmd = "2.2.2"
proptest = "1.11.0"
tempfile = "3.27.0"
```

Move the lockfile and untrack the seed:

```bash
mv crates/config-manifest/Cargo.lock crates/Cargo.lock
config rm --cached crates/config-manifest/Cargo.lock
config rm --cached crates/config-manifest/proptest-regressions/plan.txt
```

Append to `crates/config-manifest/.gitignore`:

```
# Proptest writes a failure seed here when a property fails. That is an output
# of running the tests, not an input to the build, and this directory sits
# inside the tree the build stamp covers: a tracked seed moved the stamp and
# forced a rebuild whose output is byte-identical.
proptest-regressions/
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

In `.scripts/config/config-build`, point cargo at the workspace root:

```sh
ROOT=${DOTFILES_ROOT:-$HOME}
WORKSPACE_DIR="$ROOT/crates"
BIN_DIR=${CONFIG_BIN_DIR:-$HOME/.local/bin}
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/config-manifest/target}"

stamp=$("$ROOT/.scripts/config/config-stamp")

CONFIG_MANIFEST_STAMP="$stamp" \
    cargo build --release --locked --quiet \
        --manifest-path "$WORKSPACE_DIR/Cargo.toml"
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `~/tests/config-manifest-lifecycle.test.sh`
Expected: PASS.

Run: `~/.scripts/config/config-build && config-manifest --stamp`
Expected: an install line, then a tree id.

Run: `~/tests/run-all.sh`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
config add crates/Cargo.toml crates/Cargo.lock crates/rust-toolchain.toml \
    crates/config-manifest/Cargo.toml crates/config-manifest/.gitignore \
    .scripts/config/config-build tests/config-manifest-lifecycle.test.sh
config commit -m "Convert crates/ to a workspace, pin the toolchain, untrack the seed

Three changes that all narrow the build's declared inputs to its true inputs.

The workspace gives one lockfile and one resolution, so a dependency exists at
one version across every binary, and one cargo build rather than N. The
members list becomes the single place that says which crates exist.

The toolchain pin closes a real gap: the stamp is a git tree id over source,
so it says nothing about the compiler, and edition = \"2024\" is not a pin
because every toolchain from 1.85 onward compiles it. mac and linux could push
matching stamps from different compilers.

The proptest seed is an output of running the tests, not an input to the
build, and it sat inside the stamped tree, so a test run moved the stamp and
forced a rebuild whose output was byte-identical.

The binary still installs to the same path and --stamp still prints one value,
so tests/pre-push keeps working unchanged until the stamp becomes per-crate."
```

---

## Task 6: Per-crate, ref-scoped stamps

A single workspace-wide stamp would be coarser than any one binary's input
set: editing crate A would mark crate B stale, and `pre-push` would refuse a
push over a binary byte-identical to what its own sources produce. False
refusals are how a gate gets bypassed, which is the reasoning `config-stamp`'s
own header already uses to reject a HEAD-based stamp.

Verified before this plan was written: one `write-tree` over `crates/` produced
root tree `623787b34617522f3db88c908d4d3e20a8d75323`, and
`rev-parse "${root}:crates/config-manifest"` produced
`e7b9aa22ab52dd83fd6d1346708a3fd921637b36`, byte-identical to what
`config-stamp` prints today.

**Shell owns the stamp rule, and there is no Rust counterpart.** `config-build`
calls `config-stamp` to decide what to compile, so the stamp must be computable
before any binary exists. A Rust implementation would be a circular dependency
and a second format that can silently disagree with the one that ships.

Four panel findings changed this task from its first draft, all verified:

1. **The gate must stay ref-scoped.** The draft moved `expected` from
   `git rev-parse "$ref:crates/config-manifest"` (the committed pushed ref) to
   `config-stamp` (the worktree), dropping `$ref` entirely while still looping
   over refs and printing "for %s". That would false-refuse a legitimate push
   of a committed older ref and false-pass a push diverging from the worktree,
   defeating the mac-versus-linux check the hook documents at lines 59-62.
2. **Never execute a name read from a manifest via PATH.** The draft ran
   `command -v "$member"` and `"$member" --stamp`, so a member named `rm`, or
   one shadowed on PATH, would run inside a pre-push hook. Binaries are
   invoked by absolute path and member names are validated.
3. **An empty member list must abort.** If the `sed` misses, both loops iterate
   zero times and the stamp gate silently stops gating while exiting 0.
4. **A failed `rev-parse` inside a command substitution does not trip `set -e`.**
   Verified: it prints a truncated stamp and exits 0. Each id is captured into
   a variable with an explicit failure check before being formatted.

**Files:**
- Modify: `.scripts/config/config-stamp` (rewrite, including its usage header)
- Modify: `.scripts/config/config-build`
- Modify: `tests/pre-push`
- Test: `tests/config-manifest-lifecycle.test.sh`, `tests/pre-push-multi-ref.test.sh`

**Interfaces:**
- Produces:
  - `config stamp` prints `<crate> <stamp>` lines for the worktree.
  - `config stamp <crate>` prints one stamp, for scripting.
  - `config stamp --ref <ref> <crate>` prints one stamp for a committed ref.
    This is what `pre-push` uses, and it is why the gate stays ref-scoped.
  - Stamp format: `<crate-tree>:<lock-blob>:<workspace-blob>`. Shell is the
    sole owner of this format.

- [ ] **Step 1: Write the failing tests**

Append to `tests/config-manifest-lifecycle.test.sh`:

```bash
STAMP_CMD="$DOTFILES_ROOT/.scripts/config/config-stamp"

# config stamp enumerates crates. The per-crate form is what pre-push
# iterates; the single-crate form is for scripting.
output=$("$STAMP_CMD")
assert_succeeds 'config stamp names config-manifest with a folded stamp' \
    grep -qE '^config-manifest [0-9a-f]{40}:[0-9a-f]{40}:[0-9a-f]{40}$' <<<"$output"

one=$("$STAMP_CMD" config-manifest)
assert_succeeds 'config stamp <crate> prints a bare folded stamp' \
    grep -qE '^[0-9a-f]{40}:[0-9a-f]{40}:[0-9a-f]{40}$' <<<"$one"

# The error path names the crate, so a status 2 from an unrelated cause
# (a missing usage.sh, a mktemp failure) does not read as this error.
err=$("$STAMP_CMD" no-such-crate 2>&1 || true)
assert_contains 'an unknown crate is named in the error' "$err" 'no-such-crate'
status=0
"$STAMP_CMD" no-such-crate >/dev/null 2>&1 || status=$?
assert_equals 'an unknown crate exits 2' '2' "$status"

# A ref-scoped stamp is what the push gate needs: it must compare the binary
# against what is being published, not against the worktree.
head_stamp=$("$STAMP_CMD" --ref HEAD config-manifest)
assert_succeeds 'a ref-scoped stamp is well formed' \
    grep -qE '^[0-9a-f]{40}:[0-9a-f]{40}:[0-9a-f]{40}$' <<<"$head_stamp"
```

Append to `tests/pre-push-multi-ref.test.sh`:

```bash
# The reason the stamp is per-crate. With one workspace-wide stamp, editing
# any crate marks every binary stale and the gate refuses a push over a binary
# byte-identical to what its own sources produce. A false refusal is how a gate
# gets bypassed.
ws="$FIXTURES/stamp-scope"
mkdir -p "$ws/crates/crate-one" "$ws/crates/crate-two"
printf '[workspace]\nmembers = ["crate-one", "crate-two"]\n' > "$ws/crates/Cargo.toml"
printf 'lock\n' > "$ws/crates/Cargo.lock"
printf 'one\n' > "$ws/crates/crate-one/src.rs"
printf 'two\n' > "$ws/crates/crate-two/src.rs"

git -C "$ws" init -q -b main
git -C "$ws" -c user.email=t@t -c user.name=t add -A
git -C "$ws" -c user.email=t@t -c user.name=t commit -q -m init

stamp_in_ws() { DOTFILES_ROOT="$ws" "$DOTFILES_ROOT/.scripts/config/config-stamp" "$@"; }

before_two=$(stamp_in_ws crate-two)
# Positive control. Both assertions below compare two stamp outputs, so if
# config-stamp failed and printed nothing they would compare empty to empty
# and pass vacuously.
assert_succeeds 'the fixture stamp is well formed' \
    grep -qE '^[0-9a-f]{40}:' <<<"$before_two"

printf 'one changed\n' > "$ws/crates/crate-one/src.rs"
git -C "$ws" -c user.email=t@t -c user.name=t add -A
git -C "$ws" -c user.email=t@t -c user.name=t commit -q -m 'edit crate-one'

after_two=$(stamp_in_ws crate-two)
after_one=$(stamp_in_ws crate-one)

assert_equals 'editing one crate leaves the other stamp unchanged' \
    "$before_two" "$after_two"
assert_succeeds 'editing one crate changes its own stamp' \
    test "$after_one" != "$before_two"

# An empty member list must abort rather than iterate zero crates. A gate that
# checks nothing and exits 0 is the failure this whole plan closes.
printf '[workspace]\nmembers = []\n' > "$ws/crates/Cargo.toml"
git -C "$ws" -c user.email=t@t -c user.name=t add -A
git -C "$ws" -c user.email=t@t -c user.name=t commit -q -m 'empty members'

status=0
stamp_in_ws >/dev/null 2>&1 || status=$?
assert_equals 'an empty member list is an error, not an empty run' '2' "$status"
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `~/tests/config-manifest-lifecycle.test.sh`
Expected: FAIL on the folded-stamp format assertions.

Run: `~/tests/pre-push-multi-ref.test.sh`
Expected: FAIL on `the fixture stamp is well formed`.

- [ ] **Step 3: Rewrite `config-stamp`**

Replace the whole file. Note the usage header changes too: the old one said
"Print the tree id of crates/config-manifest" and "Takes no options", both of
which become false.

```sh
#!/bin/sh
# help: Print the build stamp of each workspace crate
#
# Prints "<crate> <stamp>" for every member of the crates/ workspace, or one
# bare stamp when given a crate name.
#
# A stamp is <crate-tree>:<lock-blob>:<workspace-blob>. The lockfile and
# workspace manifest are folded into every crate's stamp because they are
# shared inputs: a bump to either changes what each binary is, without
# touching that crate's own subtree.
#
# Per-crate rather than one id over the whole workspace. A single stamp is
# coarser than any one binary's input set, so editing one crate would mark
# every binary stale and pre-push would refuse a push over a binary identical
# to what its own sources produce. False refusals are how a gate gets
# bypassed.
#
# This rule lives in shell and has no Rust counterpart on purpose.
# config-build calls this script to decide what to compile, so the stamp has
# to be computable before any binary exists. A Rust owner would be a circular
# dependency, and a second implementation would be a format that can silently
# disagree with the one that ships.
#
# A HEAD-based stamp would false-refuse the `commit -a` workflow: build while
# dirty, commit that same content, push, refused although the binary matches.
# Reading the worktree through a temp index makes the same content stamp
# identically whether or not it is committed yet, and `git add` honours
# .gitignore, so an ignored target/ never moves the stamp.
#
# usage: config stamp [--ref <ref>] [<crate>]
#
# Options:
#   --ref <ref>   Stamp the crate as committed at <ref> rather than in the
#                 worktree. This is what the pre-push gate uses: it must
#                 compare a binary against what is being published.
#
# Environment:
#   DOTFILES_ROOT   The worktree to read the crates out of. Default $HOME.

set -eu

. "$(dirname "$(readlink -f "$0")")/usage.sh"
usage_if_requested "${1:-}"

ROOT=${DOTFILES_ROOT:-$HOME}
WORKSPACE=crates

ref=''
crate=''
while [ "$#" -gt 0 ]; do
    case $1 in
        --ref)
            [ "$#" -ge 2 ] || { printf 'config-stamp: --ref needs a value\n' >&2; exit 2; }
            ref=$2
            shift 2
            ;;
        -*)
            printf 'config-stamp: unknown option %s\n' "$1" >&2
            exit 2
            ;;
        *)
            crate=$1
            shift
            ;;
    esac
done

if [ -d "$ROOT/.cfg" ]; then
    git_cmd() { git --git-dir="$ROOT/.cfg" --work-tree="$ROOT" "$@"; }
else
    git_cmd() { git -C "$ROOT" "$@"; }
fi

# The root tree the stamps are read out of. The worktree by default, through a
# temp index; a committed ref when --ref is given.
if [ -n "$ref" ]; then
    root_tree=$(git_cmd rev-parse --verify --quiet "$ref^{tree}") || {
        printf 'config-stamp: cannot resolve ref %s\n' "$ref" >&2
        exit 2
    }
else
    index=$(mktemp "${TMPDIR:-/tmp}/config-stamp-XXXXXX")
    rm -f "$index"
    trap 'rm -f "$index"' EXIT INT TERM HUP
    export GIT_INDEX_FILE="$index"

    git_cmd read-tree --empty
    (cd "$ROOT" && git_cmd add -- "$WORKSPACE")
    root_tree=$(git_cmd write-tree)
fi

# Captured into a variable with an explicit check, because a failing
# rev-parse inside a command substitution does not trip `set -e`: it yields an
# empty string and the caller formats a truncated stamp and exits 0.
# Returns 2 rather than exiting, because every caller invokes this inside a
# command substitution and an `exit` there dies in the subshell. An earlier
# draft used `exit 2` and was verified to print a member name with an EMPTY
# stamp and exit 0, which config-build would then embed. That is the same
# defect Task 1 exists to fix, so it must not be reintroduced here.
object_at() {
    if ! object_id=$(git_cmd rev-parse --verify --quiet "$root_tree:$1"); then
        printf 'config-stamp: %s is not in the stamped tree\n' "$1" >&2
        return 2
    fi
    printf '%s' "$object_id"
}

lock_blob=$(object_at "$WORKSPACE/Cargo.lock") || exit 2
ws_blob=$(object_at "$WORKSPACE/Cargo.toml") || exit 2

# Read from the same tree the stamps come from, so the member list and the
# subtrees cannot disagree. `members` must be a single-line array; the
# workspace manifest says so, and an empty result aborts below rather than
# iterating zero crates.
members=$(git_cmd show "$root_tree:$WORKSPACE/Cargo.toml" \
    | sed -n 's/^members = \[\(.*\)\]$/\1/p' \
    | tr ',' '\n' \
    | sed 's/^[[:space:]]*"//; s/"[[:space:]]*$//' \
    | grep -v '^[[:space:]]*$' || true)

if [ -z "$members" ]; then
    printf 'config-stamp: no workspace members found in %s/Cargo.toml\n' "$WORKSPACE" >&2
    printf 'config-stamp: members must be a single-line array; a gate that\n' >&2
    printf 'config-stamp: checks zero crates would pass silently.\n' >&2
    exit 2
fi

# Member names become path components and, in pre-push, the basename of a
# binary to execute. Validated so a manifest cannot name `rm` or `../thing`.
# A `for` over the validated list rather than a piped `while`: an `exit`
# inside a pipeline runs in a subshell and does not abort the script, so a
# piped loop would announce an invalid name and then process it anyway.
# Word splitting is safe here because the loop is what establishes that every
# name is [a-zA-Z0-9_-].
for member in $members; do
    case $member in
        ''|*[!a-zA-Z0-9_-]*)
            printf 'config-stamp: invalid member name: %s\n' "$member" >&2
            exit 2
            ;;
    esac
done

stamp_for() {
    crate_tree=$(object_at "$WORKSPACE/$1") || return 2
    printf '%s:%s:%s' "$crate_tree" "$lock_blob" "$ws_blob"
}

if [ -n "$crate" ]; then
    if ! printf '%s\n' "$members" | grep -qxF -- "$crate"; then
        printf 'config-stamp: no such workspace crate: %s\n' "$crate" >&2
        exit 2
    fi
    stamp_for "$crate"
    printf '\n'
    exit 0
fi

# Each stamp is captured with an explicit check, and the loop runs in the
# parent shell. An earlier draft used `printf '%s %s' "$member"
# "$(stamp_for ...)"` inside a piped `while`, which was verified to print a
# member name with an EMPTY stamp and exit 0 when the lookup failed:
# command substitution swallows the status, and the pipeline subshell
# swallows the exit. config-build would then embed that empty stamp.
for member in $members; do
    stamp=$(stamp_for "$member") || exit 2
    printf '%s %s\n' "$member" "$stamp"
done
```

- [ ] **Step 4: Update `config-build` and `pre-push`**

In `.scripts/config/config-build`, build and stamp each member. The member
list and each stamp come from one `config-stamp` invocation, captured once, so
every crate is stamped from the same snapshot of the worktree:

```sh
# One invocation, one snapshot. Re-invoking per crate would read the worktree
# several times, so crate A could be stamped from one state and crate B from
# another.
stamps=$("$ROOT/.scripts/config/config-stamp")

mkdir -p "$BIN_DIR"
printf '%s\n' "$stamps" | while IFS=' ' read -r member stamp; do
    [ -n "$member" ] || continue
    CONFIG_MANIFEST_STAMP="$stamp" \
        cargo build --release --locked --quiet \
            --manifest-path "$WORKSPACE_DIR/Cargo.toml" -p "$member"
    cp "$CARGO_TARGET_DIR/release/$member" "$BIN_DIR/$member"
    built=$("$BIN_DIR/$member" --stamp)
    printf 'config-build: installed %s (stamp %s)\n' "$BIN_DIR/$member" "$built"
done
```

In `tests/pre-push`, replace the stamp block. Note `--ref "$ref"` and the
absolute binary path:

```sh
    # Every workspace crate is checked against its own subtree, as committed
    # at the ref being pushed. A single workspace-wide stamp would refuse a
    # push over a binary whose own sources did not change, and reading the
    # worktree instead of the ref would compare the binary against whatever
    # happens to be checked out rather than against what is being published.
    BIN_DIR=${CONFIG_BIN_DIR:-$HOME/.local/bin}
    for ref in $push_refs; do
        pushed_stamps=$("$HOME/.scripts/config/config-stamp" --ref "$ref" 2>/dev/null) || {
            printf 'pre-push: cannot read workspace stamps for %s\n' "$ref" >&2
            exit 1
        }
        [ -n "$pushed_stamps" ] || {
            printf 'pre-push: no workspace members for %s, refusing to check nothing\n' "$ref" >&2
            exit 1
        }

        printf '%s\n' "$pushed_stamps" | while IFS=' ' read -r member expected; do
            [ -n "$member" ] || continue
            # Absolute path, never PATH resolution: the member name comes from
            # a manifest in the pushed tree, and executing it by name would run
            # whatever answers to that name.
            if [ ! -x "$BIN_DIR/$member" ]; then
                printf 'pre-push: %s is not installed; run ~/.scripts/config/config-build\n' \
                    "$member" >&2
                exit 1
            fi
            built=$("$BIN_DIR/$member" --stamp)
            if [ "$built" != "$expected" ]; then
                printf 'pre-push: %s is stale for %s (built %s, expected %s)\n' \
                    "$member" "$ref" "$built" "$expected" >&2
                printf 'pre-push: run ~/.scripts/config/config-build\n' >&2
                exit 1
            fi
            printf 'pre-push: %s stamp matches the pushed crate for %s\n' "$member" "$ref"
        done || exit 1
    done
```

The `|| exit 1` after the inner loop matters: the pipeline runs its body in a
subshell, so an `exit 1` inside it would otherwise not stop the hook. Verified
that an `exit 1` inside a piped `while` does make the pipeline return 1.

**This change requires editing `tests/pre-push-multi-ref.test.sh`, not only
appending to it.** That suite injects a fake `config-manifest` through
`PATH="$stub_dir:$PATH"` and never sets `CONFIG_BIN_DIR`. After this rewrite
the hook resolves `${CONFIG_BIN_DIR:-$HOME/.local/bin}/config-manifest`, so
the stub is invisible: the matching-binary assertion goes red for the wrong
reason and the two stale assertions go green for the wrong reason. Pass
`CONFIG_BIN_DIR="$stub_dir"` wherever the suite runs the hook, and keep the
`PATH` entry only if something else still needs it.

Also hoist the installed-stamp gather above the ref loop. The installed binary
does not change between refs, so probing it once per member per ref is 2N
spawns where N suffice, and it makes the two lifetimes legible: the installed
set belongs to the push, the expected set belongs to the ref.

- [ ] **Step 5: Run tests to verify they pass**

Run: `~/tests/config-manifest-lifecycle.test.sh`
Expected: PASS.

Run: `~/tests/pre-push-multi-ref.test.sh`
Expected: PASS.

Run: `~/.scripts/config/config-build`
Expected: one install line per member.

Run: `~/tests/run-all.sh`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
config add .scripts/config/config-stamp .scripts/config/config-build \
    tests/pre-push tests/config-manifest-lifecycle.test.sh \
    tests/pre-push-multi-ref.test.sh
config commit -m "Stamp and verify each workspace crate, scoped to the pushed ref

config stamp now enumerates the workspace: one write-tree for crates/, then
one rev-parse per member, with the shared lockfile and workspace manifest
folded into each crate's stamp. Verified that this reproduces the previous
single-crate stamp byte for byte.

Per-crate rather than one workspace-wide id: a single stamp is coarser than
any one binary's input set, so editing crate A would refuse a push over crate
B's unchanged binary. pre-push-multi-ref.test.sh asserts that scoping
directly, because a false refusal is how a gate gets bypassed.

The gate stays ref-scoped. \`config stamp --ref\` reads a committed tree, so
the hook compares each binary against what is being published rather than
against whatever is checked out. Reading the worktree would have defeated the
mac-versus-linux check the hook exists for.

Binaries are invoked by absolute path and member names are validated: the
member list comes from a manifest in the pushed tree, and executing it by name
would run whatever answers to that name inside a git hook.

An empty member list aborts rather than iterating zero crates, because a gate
that checks nothing and exits 0 is the failure this work exists to close. Each
git id is captured with an explicit check, since a failing rev-parse inside a
command substitution does not trip set -e and would emit a truncated stamp.

The stamp rule lives only in shell. config-build calls config-stamp to decide
what to compile, so the stamp has to exist before any binary does; a Rust
implementation would be a circular dependency and a second format that can
disagree with the one that ships."
```

---

## Task 7: `config doctor` reports stale binaries

Silent when current, so it is safe to run habitually. There is deliberately no
runtime freshness check: a binary verifying its own stamp measures 124ms per
invocation on a path that runs before every shell prompt, and a rebuild
triggered from a prompt hook serializes every pane behind cargo's build lock (a
no-op release build measures 0.7 to 1.5 seconds). Both were measured and
rejected. The guarantee is at pre-push; this is how you ask earlier.

`doctor` was chosen after checking collisions: `status` shadows a git verb and
`check` is already the drift check. Verified both against `git --list-cmds`.

**This module is Rust, and it earns it under the Global Constraints rule.** The
first draft had the binary shell out to `config-stamp` for expected values,
which inverted the layering and made the module a pass-through. Here `doctor`
owns its whole gather through `git.rs`, so the binary is the sole producer.

**Files:**
- Create: `crates/config-manifest/src/doctor.rs`, `.scripts/config/config-doctor`
- Modify: `crates/config-manifest/src/lib.rs`, `src/main.rs`, `src/git.rs`, `src/path.rs`
- Modify: `.claude/rules/dotfiles-tests.md`
- Test: `crates/config-manifest/src/doctor.rs` (unit), `tests/config.test.sh`

**Interfaces:**
- Produces:
  - `path::TreeId` beside the existing `BlobId` and `CommitId`, so a tree id
    and a blob id are not interchangeable.
  - `doctor::Finding` as a sum: `NotInstalled`, `Stale`, `Orphaned`.
  - `doctor::diagnose(installed: &BTreeMap<CrateName, Stamp>, expected: &BTreeMap<CrateName, Stamp>) -> Vec<Finding>`.
    Pure. No IO.
  - `doctor::render(&[Finding]) -> Option<String>`. Pure. `None` means current.
  - `git::workspace_stamps()` gathers the expected stamps. The IO edge.

- [ ] **Step 1: Write the failing tests**

Create `crates/config-manifest/src/doctor.rs` with only its tests:

```rust
//! Binary staleness diagnosis.
//!
//! Pure by construction. `diagnose` compares two caller-supplied maps and
//! `render` returns a value; neither spawns a process nor reads the
//! filesystem. Gathering the stamps is `git.rs`'s job, which is what lets
//! "one crate stale, one current, one orphaned" be a table test.

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn stamps(pairs: &[(&str, &str)]) -> BTreeMap<CrateName, Stamp> {
        pairs
            .iter()
            .map(|(name, stamp)| {
                (
                    CrateName::parse(name).expect("test name is valid"),
                    Stamp::parse(stamp).expect("test stamp is valid"),
                )
            })
            .collect()
    }

    const STAMP_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:cccccccccccccccccccccccccccccccccccccccc";
    const STAMP_B: &str = "dddddddddddddddddddddddddddddddddddddddd:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:cccccccccccccccccccccccccccccccccccccccc";

    #[test]
    fn everything_current_yields_no_findings() {
        let expected = stamps(&[("config-manifest", STAMP_A)]);
        let installed = stamps(&[("config-manifest", STAMP_A)]);
        assert!(diagnose(&installed, &expected).is_empty());
    }

    #[test]
    fn a_stale_binary_is_reported_with_both_stamps() {
        let expected = stamps(&[("config-manifest", STAMP_A)]);
        let installed = stamps(&[("config-manifest", STAMP_B)]);
        let findings = diagnose(&installed, &expected);
        assert_eq!(findings.len(), 1);
        assert!(matches!(findings[0], Finding::Stale { .. }));
    }

    #[test]
    fn a_missing_binary_is_reported_as_not_installed() {
        let expected = stamps(&[("config-manifest", STAMP_A)]);
        let installed = BTreeMap::new();
        let findings = diagnose(&installed, &expected);
        assert_eq!(findings.len(), 1);
        assert!(matches!(findings[0], Finding::NotInstalled { .. }));
    }

    // An installed binary for a crate that is no longer a member is invisible
    // to a diagnosis that only walks the expected side, which is a fail-open
    // in a tool whose whole job is reporting what does not match.
    #[test]
    fn an_installed_binary_with_no_member_is_orphaned() {
        let expected = stamps(&[("config-manifest", STAMP_A)]);
        let installed = stamps(&[("config-manifest", STAMP_A), ("config-gone", STAMP_B)]);
        let findings = diagnose(&installed, &expected);
        assert_eq!(findings.len(), 1);
        assert!(matches!(findings[0], Finding::Orphaned { .. }));
    }

    #[test]
    fn a_clean_render_is_none() {
        assert_eq!(render(&[]), None);
    }

    #[test]
    fn a_stale_render_names_the_crate_and_the_fix() {
        let expected = stamps(&[("config-manifest", STAMP_A)]);
        let installed = stamps(&[("config-manifest", STAMP_B)]);
        let text = render(&diagnose(&installed, &expected)).expect("stale renders");
        assert!(text.contains("config-manifest"));
        assert!(text.contains("config build"));
    }

    #[test]
    fn a_malformed_stamp_is_rejected_at_the_boundary() {
        assert!(Stamp::parse("not-a-stamp").is_err());
        assert!(Stamp::parse("").is_err());
    }

    #[test]
    fn a_crate_name_with_a_path_separator_is_rejected() {
        assert!(CrateName::parse("../evil").is_err());
        assert!(CrateName::parse("").is_err());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd ~/crates && cargo test --locked -p config-manifest doctor`
Expected: FAIL to compile, `cannot find type CrateName in this scope`.

- [ ] **Step 3: Write the implementation**

Add `TreeId` to `crates/config-manifest/src/path.rs`, beside the existing
`BlobId` and `CommitId`, reusing their `parse_object_id`:

```rust
/// A git tree id.
///
/// Distinct from `BlobId` so a tree and a blob cannot be passed
/// interchangeably. The stamp folds one tree id and two blob ids, and a
/// transposition there would produce a well-formed stamp that gates pushes
/// wrongly.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TreeId(String);

impl TreeId {
    pub fn parse(raw: &str) -> Result<Self, IdError> {
        parse_object_id(raw).map(TreeId)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
```

Prepend to `crates/config-manifest/src/doctor.rs`:

```rust
use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::path::{IdError, TreeId};

/// A workspace member's name.
///
/// Smart-constructed because the name becomes a path component and the
/// basename of a binary, and it is read out of a manifest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CrateName(String);

#[derive(Debug, PartialEq, Eq)]
pub enum NameError {
    Empty,
    NotAName { raw: String },
}

impl CrateName {
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        if raw.is_empty() {
            return Err(NameError::Empty);
        }
        if !raw
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
        {
            return Err(NameError::NotAName { raw: raw.to_string() });
        }
        Ok(CrateName(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A build stamp: one crate tree id and two shared blob ids.
///
/// Parsed rather than held as a String, so a value that reached the binary
/// through `option_env!` is checked at the boundary the gate reads rather
/// than compared as opaque text.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Stamp {
    crate_tree: TreeId,
    lock_blob: TreeId,
    workspace_blob: TreeId,
}

#[derive(Debug, PartialEq, Eq)]
pub enum StampError {
    WrongFieldCount { found: usize },
    BadId(IdError),
}

impl Stamp {
    pub fn parse(raw: &str) -> Result<Self, StampError> {
        let fields: Vec<&str> = raw.split(':').collect();
        let [crate_tree, lock_blob, workspace_blob] = fields.as_slice() else {
            return Err(StampError::WrongFieldCount { found: fields.len() });
        };
        Ok(Stamp {
            crate_tree: TreeId::parse(crate_tree).map_err(StampError::BadId)?,
            lock_blob: TreeId::parse(lock_blob).map_err(StampError::BadId)?,
            workspace_blob: TreeId::parse(workspace_blob).map_err(StampError::BadId)?,
        })
    }

    pub fn as_display(&self) -> String {
        format!(
            "{}:{}:{}",
            self.crate_tree.as_str(),
            self.lock_blob.as_str(),
            self.workspace_blob.as_str()
        )
    }
}

/// One crate whose installed binary does not match its source.
///
/// A sum rather than a struct with nullable fields: `Stale` cannot be
/// constructed without an installed stamp, and `NotInstalled` cannot carry
/// one.
#[derive(Debug, PartialEq, Eq)]
pub enum Finding {
    NotInstalled { crate_name: CrateName, expected: Stamp },
    Stale { crate_name: CrateName, installed: Stamp, expected: Stamp },
    Orphaned { crate_name: CrateName, installed: Stamp },
}

/// The crates whose installed binary does not match the expected stamp.
///
/// Both maps are supplied by the caller, so this makes no process call and
/// reads no file. Maps rather than parallel slices: a map cannot carry a
/// duplicate name, and absence expresses "not installed" without an extra
/// Option axis in the value.
pub fn diagnose(
    installed: &BTreeMap<CrateName, Stamp>,
    expected: &BTreeMap<CrateName, Stamp>,
) -> Vec<Finding> {
    let mut findings = Vec::new();

    for (crate_name, want) in expected {
        match installed.get(crate_name) {
            None => findings.push(Finding::NotInstalled {
                crate_name: crate_name.clone(),
                expected: want.clone(),
            }),
            Some(have) if have != want => findings.push(Finding::Stale {
                crate_name: crate_name.clone(),
                installed: have.clone(),
                expected: want.clone(),
            }),
            Some(_) => {}
        }
    }

    // Walked in both directions: an installed binary for a crate that is no
    // longer a member would otherwise be invisible.
    for (crate_name, have) in installed {
        if !expected.contains_key(crate_name) {
            findings.push(Finding::Orphaned {
                crate_name: crate_name.clone(),
                installed: have.clone(),
            });
        }
    }

    findings
}

/// The report, or `None` when everything is current.
///
/// `Option` rather than a struct carrying an always-empty stdout and an exit
/// code: this command is silent or it is a list plus one fix line, and a
/// struct would permit combinations that mean nothing.
pub fn render(findings: &[Finding]) -> Option<String> {
    if findings.is_empty() {
        return None;
    }

    let mut report = String::from("config doctor: installed binaries do not match their source\n\n");
    for finding in findings {
        match finding {
            Finding::NotInstalled { crate_name, .. } => {
                writeln!(report, "  {}: not installed", crate_name.as_str())
                    .expect("writing to a String cannot fail");
            }
            Finding::Stale { crate_name, installed, expected } => {
                writeln!(
                    report,
                    "  {}: installed {}, source {}",
                    crate_name.as_str(),
                    installed.as_display(),
                    expected.as_display()
                )
                .expect("writing to a String cannot fail");
            }
            Finding::Orphaned { crate_name, .. } => {
                writeln!(
                    report,
                    "  {}: installed but no longer a workspace member",
                    crate_name.as_str()
                )
                .expect("writing to a String cannot fail");
            }
        }
    }
    report.push_str("\n  Fix: config build\n");
    Some(report)
}
```

Register both modules in `crates/config-manifest/src/lib.rs`:

```rust
pub mod doctor;
```

Add the gather to `crates/config-manifest/src/git.rs`. `run_checked` already
accepts an optional index path, so the temp-index read is the same mechanism
the crate already uses:

```rust
/// The expected stamp for every workspace member, read out of the worktree
/// through a temp index.
///
/// The IO edge for `doctor`. Everything above it is pure.
pub fn workspace_stamps(&self) -> anyhow::Result<BTreeMap<CrateName, Stamp>> {
    // Implementation: read-tree --empty into a temp index, add -- crates,
    // write-tree for the root, then rev-parse each member subtree plus
    // crates/Cargo.lock and crates/Cargo.toml, folding as
    // <crate-tree>:<lock-blob>:<workspace-blob>. Members come from
    // `git show <root>:crates/Cargo.toml`, parsed for the single-line
    // members array, with an empty result an error rather than an empty map.
}
```

Add a `Doctor` variant to the clap `Subcommand` enum in `src/main.rs` and
extend the import:

```rust
use config_manifest::{check, doctor, git, manifest};
```

`main.rs` reaches modules through the implicit lib target, so registering
`pub mod doctor;` in `lib.rs` is what makes this resolve. The handler calls
`git.workspace_stamps()` for expected values, spawns each installed binary's
`--stamp` and parses it with `Stamp::parse` for installed values, calls
`diagnose`, writes `render`'s `Option<String>` to stderr, and returns 0 or 1.
All process spawning stays in `main.rs`.

Create `.scripts/config/config-doctor`:

```sh
#!/bin/sh
# help: Report installed binaries that do not match their source
#
# Reports which workspace binaries were built from source that has since
# changed, and names the command that fixes it. Silent when everything is
# current, so it is safe to run habitually.
#
# There is no runtime freshness check anywhere else, by measurement: a binary
# verifying its own stamp costs about 124ms per invocation and the hot path
# runs before every shell prompt, while a rebuild triggered from a prompt hook
# serializes every pane behind cargo's build lock. The guarantee is at
# pre-push, and this command is how you ask earlier.
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

- [ ] **Step 4: Run the unit tests**

Run: `cd ~/crates && cargo test --locked -p config-manifest doctor`
Expected: PASS, 8 tests.

- [ ] **Step 5: Write the integration test**

The panel's finding: the unit tests cover `diagnose` and `render`, but nothing
covers the wiring that gathers real stamps and joins them by crate name, which
is where the bug will be. Append to `tests/config.test.sh`:

```bash
DOCTOR="$CONFIG_DIR/config-doctor"

assert_succeeds 'config doctor exists and is executable' test -x "$DOCTOR"
assert_equals 'doctor does not shadow a git verb' '' \
    "$(git --list-cmds=main,others 2>/dev/null | grep -x doctor || true)"

# Status asserted separately from output. A silent failure (the binary not on
# PATH, a crash before writing) would otherwise read as "silent because
# everything is current".
"$DOTFILES_ROOT/.scripts/config/config-build" >/dev/null 2>&1
doctor_out=$("$DOCTOR" 2>&1)
doctor_status=$?
assert_equals 'doctor exits 0 when every binary is current' '0' "$doctor_status"
assert_equals 'doctor is silent when every binary is current' '' "$doctor_out"

# The behavior doctor exists for, asserted rather than checked by hand.
probe="$DOTFILES_ROOT/crates/config-manifest/src/doctor.rs"
cp "$probe" "$FIXTURES/doctor.rs.orig"
printf '\n// staleness probe\n' >> "$probe"

doctor_out=$("$DOCTOR" 2>&1 || true)
doctor_status=0
"$DOCTOR" >/dev/null 2>&1 || doctor_status=$?
assert_equals 'doctor exits 1 when a binary is stale' '1' "$doctor_status"
assert_contains 'doctor names the stale crate' "$doctor_out" 'config-manifest'
assert_contains 'doctor names the fix' "$doctor_out" 'config build'

cp "$FIXTURES/doctor.rs.orig" "$probe"
"$DOTFILES_ROOT/.scripts/config/config-build" >/dev/null 2>&1
```

- [ ] **Step 6: Run it, then document the loop**

Run: `~/tests/config.test.sh`
Expected: PASS.

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
pre-push, which refuses a push when any binary is stale for the ref being
pushed, and `config doctor` is how you ask before then.
```

- [ ] **Step 7: Verify and commit**

Run: `~/tests/run-all.sh`
Expected: PASS.

```bash
config add crates/config-manifest/src/doctor.rs \
    crates/config-manifest/src/path.rs crates/config-manifest/src/git.rs \
    crates/config-manifest/src/lib.rs crates/config-manifest/src/main.rs \
    .scripts/config/config-doctor tests/config.test.sh \
    .claude/rules/dotfiles-tests.md
config commit -m "Add config doctor to report stale binaries

Reports which installed binaries were built from source that has since
changed, and names the fix. Silent when current, so it is safe to run
habitually.

diagnose and render are pure over caller-supplied maps, so \"one stale, one
current, one orphaned\" is a table test rather than a subprocess test. The
gather is git.rs's job, which keeps the binary the sole producer of the values
it reports on.

Finding is a sum, so Stale cannot be constructed without an installed stamp
and NotInstalled cannot carry one. render returns Option<String> rather than a
struct with an always-empty stdout and an exit code, because this command is
silent or it is a list plus one fix line.

diagnose walks both directions: an installed binary for a crate that is no
longer a member would otherwise be invisible, which is a fail-open in a tool
whose job is reporting what does not match.

Adds TreeId beside BlobId so a tree id and a blob id are not interchangeable,
and parses the stamp at the boundary rather than comparing opaque text.

The verb is doctor because status shadows a git verb and check is already the
drift check; both verified against git --list-cmds.

Documents the edit-build-test loop, including why there is no runtime
freshness check."
```

---

## Task 8: The prompt stops running a full `git status`

Found by a research pass while investigating the Rust question, and worth more
than the migration it was found under. `.zshrc:208-213` defines
`parse_git_dirty` and `.zshrc:224` calls it from inside `PS1`, so it runs a
full `git status` every time a prompt is drawn, in every pane.

Measured twice, once by the research agent and once independently, in a
24,453-file worktree:

| Form | Cost |
|---|---|
| `git status` (what it does now) | **299.2 ms** |
| `git -c core.untrackedCache=true status` | 133.4 ms |
| `git status --porcelain -uno --no-renames` | **44.6 ms** |

In `$HOME` it costs only 12ms, because `.cfg/config` sets
`status.showUntrackedFiles=no`. The cost is paid in the project worktrees,
which is where most prompts are drawn. For scale, the tmux naming script this
plan spends two tasks on costs 17.8ms on the same path.

The porcelain form is also a correctness improvement. The current code matches
three `[[ =~ ]]` patterns against human-readable English (`"Changes to be
committed:"`), which depends on git's wording and on the user's locale.
Porcelain codes are a stable machine format.

It drops untracked-file colouring, which is a deliberate behavior decision
rather than a free win. `-uno` is what makes it fast; keeping untracked
detection means keeping most of the cost.

**Files:**
- Modify: `.zshrc:208-213`
- Test: `tests/zshrc-git-aliases.test.sh`

**Interfaces:**
- Produces: `parse_git_dirty` keeps its name, its call site, and its output
  contract (zero or more `%F{colour}` escapes on stdout). Only the mechanism
  and the untracked case change.

- [ ] **Step 1: Write the failing test**

Append to `tests/zshrc-git-aliases.test.sh`, before its `finish`:

```bash
# parse_git_dirty runs inside PS1, so it costs its full runtime on every
# prompt in every pane. A bare `git status` measured 299ms in a 24k-file
# worktree; the porcelain form measured 44.6ms.
#
# Asserted on the source rather than by timing, because a timing assertion
# here would be measuring the machine's git, not this change.
ZSHRC="$DOTFILES_ROOT/.zshrc"

assert_succeeds 'parse_git_dirty is still defined' \
    grep -q '^parse_git_dirty()' "$ZSHRC"

dirty_body=$(sed -n '/^parse_git_dirty()/,/^}/p' "$ZSHRC")
assert_succeeds 'the dirty check was extracted' test -n "$dirty_body"

assert_succeeds 'it asks git for a machine format' \
    grep -q -- '--porcelain' <<<"$dirty_body"
assert_succeeds 'it does not walk untracked files' \
    grep -q -- '-uno' <<<"$dirty_body"
assert_equals 'it no longer matches human-readable git prose' '' \
    "$(grep -o 'Changes to be committed\|Changes not staged\|Untracked files' <<<"$dirty_body" || true)"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `~/tests/zshrc-git-aliases.test.sh`
Expected: FAIL on `it asks git for a machine format` and on
`it no longer matches human-readable git prose`.

- [ ] **Step 3: Write the implementation**

Replace `parse_git_dirty` in `.zshrc`:

```zsh
# Runs inside PS1, so this costs its full runtime on every prompt in every
# pane. A bare `git status` measured 299ms in a 24k-file worktree against
# 44.6ms for this form, and `-uno` is the flag that buys most of it.
#
# Porcelain codes rather than the three matches against human-readable
# English this used before: git's wording is not a contract, and the previous
# form silently stopped colouring anything under a non-English locale.
#
# Untracked files are deliberately not reported. Detecting them is what costs
# the other 255ms, and an untracked file is visible from `config status`
# rather than needing a prompt colour.
parse_git_dirty() {
  local porcelain
  porcelain=$(git status --porcelain -uno --no-renames 2>/dev/null) || return 0
  [ -n "$porcelain" ] || return 0
  # Column 1 is the index, column 2 the worktree. Staged beats unstaged for
  # the colour, matching what the previous form did by check order.
  case $porcelain in
    [MADRC]*) printf '%%F{green}' ;;
    ?[MD]*)   printf '%%F{yellow}' ;;
  esac
}
```

Note the doubled `%%` in `printf`: `%F{green}` is a zsh prompt escape and
`printf` would otherwise consume the `%F`.

- [ ] **Step 4: Run test to verify it passes**

Run: `~/tests/zshrc-git-aliases.test.sh`
Expected: PASS.

Then confirm the prompt still colours correctly, by hand in a real repo:

```bash
cd ~/Documents/code/*/1 2>/dev/null || cd ~
# stage something, confirm green; modify without staging, confirm yellow
```

Run: `~/tests/run-all.sh`
Expected: PASS.

- [ ] **Step 5: Commit**

Note `.zshrc` does **not** match `TRIGGER_PATHS` in `tests/pre-push:38`, so
pushing this file alone runs none of the six `zshrc-*` suites that test it.
That gap is recorded in `TODO-AGENTS.md`. Run the suite locally before
committing; do not rely on the hook here.

```bash
~/tests/run-all.sh
config add .zshrc tests/zshrc-git-aliases.test.sh
config commit -m "Stop running a full git status on every prompt

parse_git_dirty is called from inside PS1, so it ran a full \`git status\`
every time a prompt was drawn, in every pane. Measured twice in a
24,453-file worktree: 299ms for the old form, 44.6ms for this one. In \$HOME
it was only 12ms because .cfg/config sets status.showUntrackedFiles=no, so
the cost was paid in the project worktrees where most prompts are drawn.

For scale, the tmux window-naming script costs 17.8ms on the same path.

Also a correctness fix. The old form matched three patterns against
human-readable English, so it depended on git's wording and on the user's
locale. Porcelain codes are a stable machine format.

Untracked files are no longer coloured. Detecting them is what costs the
other 255ms, and \`config status\` reports them without a prompt colour."
```

---

## Task 9: The `--all` naming path stops spawning git per window

`.scripts/tmux-update-window-names.sh --all` measures **1473 ms**, against
17.8 ms for the default single-window path, so the `-a` path is roughly 80x
the per-prompt cost. It is reached from the `re` alias (`.zshrc:85`) and by
anything renaming every window.

Two causes, both fixable in shell, both measured:

1. **The `--path-format` fallback fires on every non-repo directory.** Lines
   105-113 exist for git older than 2.31, and the comment says so. But the
   condition is `[ -z "$info" ]`, and `$info` is empty for any directory that
   is not a repository, so the fallback runs a *second* wasted `git rev-parse`
   there. Git here is 2.50.0, so the fallback can never fire for its stated
   reason. Measured: 24.27 ms of wasted spawns per non-repo directory, against
   0.0033 ms for a `test -e` guard.
2. **The per-window `git rev-parse` can be a file read.** Verified against
   `git branch --show-current` across all 21 live window directories: 21 of 21
   agree, including the linked worktrees whose `.git` is a file pointing into
   a `worktrees/` directory.

Keep a `git rev-parse` fallback for shapes a file read does not cover
(`gitdir:` chains, `core.worktree`, unusual ref backends). That is the
library-first-with-escape-hatch design starship uses, and 21 live directories
is evidence rather than proof.

**Files:**
- Modify: `.scripts/tmux-update-window-names.sh` (the repo/branch derivation)
- Test: `tests/tmux-update-window-names.test.sh`

**Interfaces:**
- Produces: the derivation keeps its current output contract, a repo name and
  a revision. Only the mechanism changes. The `git rev-parse` path remains as
  the fallback, so a shape the file read cannot handle still resolves.

- [ ] **Step 1: Write the failing equivalence test**

This is the harness the research note asks for. Append to
`tests/tmux-update-window-names.test.sh`:

```bash
# The file-read derivation must agree with git on every shape this repo
# actually produces, including linked worktrees, before it can replace the
# spawn. 21 live window directories agreed when this was measured, which is
# evidence rather than proof, so the fixtures below pin the shapes.
eq_repo=$(make_repo eqmain main)
printf 'x\n' > "$eq_repo/file.txt"
git -C "$eq_repo" -c user.email=t@t -c user.name=t add -A
git -C "$eq_repo" -c user.email=t@t -c user.name=t commit -q -m add

eq_wt="$FIXTURES/eqwork"
git -C "$eq_repo" worktree add -q -b feature/branch "$eq_wt"

not_repo="$FIXTURES/eqplain"
mkdir -p "$not_repo"

# Compare the script's derivation against git, per shape.
for probe in "$eq_repo" "$eq_wt" "$not_repo"; do
    expected=$(git -C "$probe" branch --show-current 2>/dev/null || true)
    actual=$("$SCRIPT" --print-revision "$probe" 2>/dev/null || true)
    assert_equals "the derivation matches git for $(basename "$probe")" \
        "$expected" "$actual"
done

# Non-repo directories must cost no git spawns at all.
assert_equals 'a non-repo directory yields no revision' '' \
    "$("$SCRIPT" --print-revision "$not_repo" 2>/dev/null || true)"
```

`--print-revision` is a new debug-only flag whose sole purpose is making the
derivation testable without driving a tmux server. Add it in Step 3.

- [ ] **Step 2: Run test to verify it fails**

Run: `~/tests/tmux-update-window-names.test.sh`
Expected: FAIL, because `--print-revision` does not exist yet, so every
`actual` is empty while `expected` names a branch.

- [ ] **Step 3: Write the implementation**

In `.scripts/tmux-update-window-names.sh`:

First add the `--print-revision <dir>` flag to the argument parser, printing
the derived revision for one directory and exiting. It exists so the
derivation is testable without a tmux server, which is the reason the current
equivalence claim rests on live windows rather than fixtures.

Then guard the whole derivation, so a non-repo directory costs zero spawns:

```sh
# A directory that is not a repository costs 24.27ms in wasted git spawns
# without this guard, measured, and most windows in a large session are not
# repositories.
[ -e "$directory/.git" ] || return 0
```

Then read the refs directly, keeping `git rev-parse` as the fallback:

```sh
# Read HEAD directly rather than spawning git. Verified to agree with
# `git branch --show-current` on all 21 live window directories, including
# linked worktrees whose .git is a file.
#
# The rev-parse fallback stays for shapes this does not model: a `gitdir:`
# chain more than one level deep, core.worktree, or a ref backend that does
# not store a readable HEAD. Library first, escape hatch behind it.
git_dir=''
if [ -d "$directory/.git" ]; then
    git_dir="$directory/.git"
elif [ -f "$directory/.git" ]; then
    git_dir=$(sed -n 's/^gitdir: //p' "$directory/.git")
    case $git_dir in
        ''|/*) ;;
        *) git_dir="$directory/$git_dir" ;;
    esac
fi

revision=''
if [ -n "$git_dir" ] && [ -f "$git_dir/HEAD" ]; then
    revision=$(sed -n 's|^ref: refs/heads/||p' "$git_dir/HEAD")
fi

# Fall back to git only when the file read did not resolve a branch.
if [ -z "$revision" ]; then
    revision=$(git -C "$directory" branch --show-current 2>/dev/null)
fi
```

Delete the `--path-format` fallback block at lines 105-113. Its stated reason
is git older than 2.31, git here is 2.50.0, and its real effect is a second
wasted spawn on every non-repo directory.

- [ ] **Step 4: Run tests, then measure**

Run: `~/tests/tmux-update-window-names.test.sh`
Expected: PASS.

Measure the path this task exists to fix:

```bash
zsh -c 'zmodload zsh/datetime; s=$EPOCHREALTIME
  ~/.scripts/tmux-update-window-names.sh --all >/dev/null 2>&1
  e=$EPOCHREALTIME; printf "--all: %.0f ms\n" $(( (e-s)*1000 ))'
```
Expected: well under the 1473 ms baseline. The research pass measured 32 ms
for the equivalent POSIX sh implementation.

Run: `~/tests/run-all.sh`
Expected: PASS. Note `tmux-update-window-names.test.sh` is the suite with the
known 25% flake on the live tmux server (recorded in `TODO-AGENTS.md`), so a
single failure there is worth a rerun before treating it as a regression.

- [ ] **Step 5: Commit**

```bash
config add .scripts/tmux-update-window-names.sh \
    tests/tmux-update-window-names.test.sh
config commit -m "Derive repo and branch by reading refs, not by spawning git

--all measured 1473ms against 17.8ms for the single-window path, so the -a
path was roughly 80x the per-prompt cost. Two causes, both measured.

The --path-format fallback fired on every non-repo directory, not only on git
older than 2.31: its condition is an empty \$info, and \$info is empty
whenever the directory is not a repository. Git here is 2.50.0, so it could
never fire for its stated reason, and it cost a second wasted spawn each
time. Measured 24.27ms per non-repo directory against 0.0033ms for a test -e
guard.

The per-window rev-parse is now a direct read of .git, commondir and HEAD.
Verified against git branch --show-current on all 21 live window
directories: 21 of 21 agree, including linked worktrees whose .git is a file.

git rev-parse remains as the fallback for shapes the file read does not
model, so an unusual layout still resolves. Adds --print-revision so the
derivation is testable against fixtures rather than against whatever windows
happen to be open."
```

---

## Self-Review

**Spec coverage.** Every spec item for steps 0 through 2:

| Spec item | Task |
|---|---|
| Blocker: subshell fail-open | 1 |
| Blocker: `.gitattributes` blinding | 1 |
| Blocker: pattern file fail-open | 2 |
| `SKIP_LEAK_CHECK` truthiness | 2 |
| `assert_succeeds` exit code | 3 |
| Blocker: zero-assertion PASS | 4 |
| Six bare-`printf` skips | 4 |
| Toolchain pin (spec 6.2) | 5 |
| proptest-regressions untracked | 5 |
| Workspace layout (spec 6) | 5 |
| Per-crate stamp (spec 6.1) | 6 |
| pre-push iterates crates (spec 6.3) | 6 |
| `config build` all crates (spec 8.1) | 6 |
| `config doctor` (spec 8.2) | 7 |
| Documentation (spec 8.3) | 7 |
| False-refusal regression test (spec 9.4) | 6 |
| `parse_git_dirty` prompt cost (not in spec; research 2026-09-06) | 8 |
| `--all` naming path cost (not in spec; research 2026-09-06) | 9 |

Not covered, deliberately: `--describe` on the dispatcher (spec step 1) is a
prerequisite for *porting*, and this plan ports nothing, so it moves to the
step 3 plan. The `setup.sh` word-splitting gap stays in `TODO-AGENTS.md` as a
bootstrap fix unrelated to these steps.

**Placeholder scan.** One deliberate omission: `git::workspace_stamps`'s body
is described rather than written, because it is a mechanical composition of
`run_checked` calls whose exact shape depends on the `git.rs` helpers in place
when the task runs. Every decision it must make is stated. Task 4 also directs
the implementer to read five files and count assertions rather than inventing
counts; the file whose count matters most has its full replacement written.

**Type consistency.** `CrateName`, `Stamp`, `TreeId`, and `Finding` are defined
in Task 7 and used consistently within it. `record_outcome` is introduced in
Task 4 and referenced by Task 3 with an explicit note that Task 3 keeps the old
increments until Task 4 lands. `config stamp --ref` is defined in Task 6 and
consumed by `pre-push` in the same task.

---

## Rejected panel findings

**`bug-hunter` claimed the non-ASCII path gap is not closed** by the
hunk-coverage check, and that Task 1's commit message therefore ships a false
claim. Rejected on evidence. Verified: `git diff --cached --name-only` emits
`"caf\303\251.txt"` while the diff header emits `"a/caf\303\251.txt"`. The
`a/` prefix sits inside the quotes, so the strings differ, the anchored
`+++ b/` match finds nothing, and the path is correctly reported unscannable.
The agent reasoned about the byte encoding being identical and missed the
prefix. Recorded so it is not re-raised.

Note the current plan's Task 1 does not claim to close the non-ASCII gap in its
commit message, because the anchored match closes it as a side effect of
requiring a header rather than by design. The remaining non-ASCII item in
`TODO-AGENTS.md` is about `setup.sh`, which this plan does not touch.

---

## Notes for the executor

- Tasks 1 through 4 are independent of 5 through 7 and can ship in either
  order. Within each half, order matters: Task 1 rewrites the caller block
  Task 2 leaves alone, Task 4 introduces the `record_outcome` Task 3
  references, and Task 6 depends on Task 5's workspace.
- Task 4 will make six suites fail before it fixes them. That is the intended
  red state.
- Task 4's Step 4 includes a Docker run. That is the environment where those
  six suites were silently contributing nothing, so it is the run that proves
  the fix.
- Every commit runs the pre-commit leak guard, which Tasks 1 and 2 modify. If
  a commit in that range is blocked by your own change, read the block before
  reaching for `SKIP_LEAK_CHECK=1`.
- Do not push. `config push-all` sends mac and linux atomically for reasons
  documented in `.claude/rules/dotfiles-tests.md`, and pushing is a separate
  decision.
