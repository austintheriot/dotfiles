# Leak Check Range Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give `tests/leak-check.sh` a `--range <a>..<b>` mode that scans every commit in a pushed range, run it from `tests/pre-push` before anything else, and cover both modes with tests.

**Architecture:** The existing script scans staged content with two pattern layers (generic credential rules in the file, project term rules from `~/.claude/local/leak-patterns.conf`) and an allow list. The refactor keeps the layers untouched and parameterises only where the added lines come from: `git diff --cached` in staged mode, `git log -p` over the range in range mode. Messages become mode-aware (`COMMIT BLOCKED` versus `PUSH BLOCKED`). pre-push collects one range per pushed ref and scans each before the drift check.

**Tech Stack:** zsh (the script), POSIX sh (the hook), bash + `tests/lib.sh` (the tests), git.

**Spec:** `docs/superpowers/specs/2026-09-04-config-command-and-manifest-crate-design.md`, section 3 (decision "Leak gate for plumbing commits"), section 8 (pre-push row), section 10 step 0.

## Global Constraints

- `tests/leak-check.sh` stays `#!/bin/zsh`. `tests/pre-push` stays `#!/bin/sh` and POSIX.
- Tests are bash, source `tests/lib.sh`, use its `assert_equals` / `assert_contains` / `assert_succeeds` / `finish`, and are discovered by `tests/run-all.sh` by the `*.test.sh` name.
- Fixture files never contain a real credential. Planted values are built from repeated letters or a documented example UUID.
- The project term pattern file and allow list are injected through the existing `LEAK_PATTERN_FILE` and `LEAK_ALLOW_FILE` environment variables. Tests never read `~/.claude/local/`.
- Range mode scans each commit in the range, not the net diff. A secret added then removed inside the range is still in the pushed history and must be caught.
- The test file is scanned by the leak check when it is committed; only
  `tests/leak-check.sh` itself is self-excluded. Every planted value is
  therefore assembled at runtime from fragments, so no source line matches a
  rule. Writing this plan tripped the real pre-commit check on the two
  literals it originally contained, which is how this constraint was found.
- The credential rules never honour the allow list. Only the project term rules do. That is existing behaviour and is pinned by a test.
- Never use `--no-verify`. Commit messages have no em dashes and no emoji.
- `tests/leak-check.sh` is `!`-excluded from nothing: it lives under the shared `tests/` path and must be synced to `linux` before pushing `mac`, or the drift gate blocks the push.

---

## File Structure

| File | Responsibility |
|---|---|
| `tests/leak-check.sh` (modify) | Add argument parsing for `--range`, a mode label, and two source functions (`changed_paths`, `added_lines`) that both layers read from. Layers and messages otherwise unchanged. |
| `tests/pre-push` (modify) | Collect a range per pushed ref inside the existing stdin loop; scan each range first; update the header comment that says the leak guard stays at pre-commit. |
| `tests/leak-check.test.sh` (create) | Fixture repo, injected pattern and allow files, planted fakes. Pins staged mode, then range mode, then the hook wiring. |
| `TODO-AGENTS.md` (modify) | Remove the queued leak-check item when the work lands. |

---

### Task 1: Pin the existing staged-mode behaviour with tests

The script has no tests today. Before changing it, characterise what it does now so the refactor in Task 2 has a safety net. These tests must pass against the unmodified script.

**Files:**
- Create: `tests/leak-check.test.sh`
- Read only: `tests/leak-check.sh`, `tests/lib.sh`

**Interfaces:**
- Consumes: `tests/leak-check.sh` with no arguments, run inside a repo with staged changes; env `LEAK_PATTERN_FILE`, `LEAK_ALLOW_FILE`, `SKIP_LEAK_CHECK`.
- Produces: helper functions `run_leak_check`, `stage_file`, `plant_*` used by Tasks 2 and 3 in the same file.

- [ ] **Step 1: Write the test file with fixtures and staged-mode assertions**

```bash
#!/bin/bash
#
# Tests for tests/leak-check.sh in both modes.
#
# Staged mode is what pre-commit runs. Range mode is what pre-push runs, and
# it exists because `git commit-tree` (used by `config sync`) and
# `git commit --no-verify` never invoke pre-commit, so without a push-time
# scan those commits reach the public repo unscanned.
#
# The project term rules are injected through LEAK_PATTERN_FILE and
# LEAK_ALLOW_FILE. The real files under ~/.claude/local/ are never read, so
# the suite runs on a machine that does not have them, and it never prints
# a real term.
#
# Every planted secret is a fake built from repeated letters or the RFC 4122
# example UUID. Nothing here is a credential.
#
# Usage: ~/tests/leak-check.test.sh

. "$(dirname "$0")/lib.sh"

LEAK_CHECK="$DOTFILES_ROOT/tests/leak-check.sh"

# A term the fake pattern file names. Chosen to match nothing real.
FAKE_TERM='ZZFAKECORPZZ'

PATTERN_FILE="$FIXTURES/leak-patterns.conf"
ALLOW_FILE="$FIXTURES/leak-allow.conf"
printf '# fake project terms for the test suite\n%s\n' "$FAKE_TERM" > "$PATTERN_FILE"
printf '# paths where project terms are allowed\n^docs/allowed\\.md$\n' > "$ALLOW_FILE"
export LEAK_PATTERN_FILE="$PATTERN_FILE"
export LEAK_ALLOW_FILE="$ALLOW_FILE"

repo=$(make_repo leak main)

# Planted fakes. Each matches exactly one layer-1 rule in leak-check.sh.
plant_key()  { printf 'token = ghp_%s\n' "$(printf 'A%.0s' $(seq 1 24))"; }
# Assembled at runtime on purpose. leak-check.sh self-excludes only its own
# path, so this file IS scanned when committed; a literal secret-shaped line
# here would block the commit that adds the test. The key token above is
# built the same way.
plant_pass() { printf 'password = "%s"\n' "$(printf '%s%s' correcthorse battery1234)"; }
plant_uuid() { printf 'registry_token=%s\n' "$(printf '%s-%s-%s-%s-%s' 123e4567 e89b 12d3 a456 426614174000)"; }
plant_term() { printf 'internal note about %s\n' "$FAKE_TERM"; }
clean_text() { printf 'nothing to see here\n'; }

# Writes $2 to $1 inside the fixture and stages it. Overwrites.
stage_file() {
    local path=$1 content=$2
    mkdir -p "$repo/$(dirname "$path")"
    printf '%s' "$content" > "$repo/$path"
    git -C "$repo" add -- "$path"
}

unstage_all() {
    git -C "$repo" reset -q
    git -C "$repo" checkout -q -- . 2>/dev/null || true
    git -C "$repo" clean -qfd
}

# Runs the leak check inside the fixture. Prints stderr, exit code on the
# last line, so one capture gives both.
run_leak_check() {
    (
        cd "$repo" || exit 99
        "$LEAK_CHECK" "$@" 2>&1 >/dev/null
        printf '\n__exit=%s\n' "$?"
    )
}
exit_of() { printf '%s' "$1" | sed -n 's/^__exit=//p' | tail -1; }

# --- staged mode -----------------------------------------------------------

stage_file notes.txt "$(clean_text)"
out=$(run_leak_check)
assert_equals 'staged: clean content passes' '0' "$(exit_of "$out")"
unstage_all

stage_file notes.txt "$(plant_key)"
out=$(run_leak_check)
assert_equals 'staged: a key-prefix token is blocked' '1' "$(exit_of "$out")"
assert_contains 'staged: the key is labelled a possible credential' \
    '[possible credential]' "$out"
assert_contains 'staged: the block message names the commit' \
    'COMMIT BLOCKED' "$out"
unstage_all

stage_file notes.txt "$(plant_pass)"
out=$(run_leak_check)
assert_equals 'staged: a hardcoded password is blocked' '1' "$(exit_of "$out")"
assert_contains 'staged: the password is labelled a secret assignment' \
    '[hardcoded secret assignment]' "$out"
unstage_all

stage_file notes.txt "$(plant_uuid)"
out=$(run_leak_check)
assert_equals 'staged: a bare UUID assignment is blocked' '1' "$(exit_of "$out")"
assert_contains 'staged: the UUID is labelled a possible token' \
    '[bare UUID (possible token)]' "$out"
unstage_all

stage_file notes.txt "$(plant_term)"
out=$(run_leak_check)
assert_equals 'staged: a project term is blocked' '1' "$(exit_of "$out")"
assert_contains 'staged: the term is labelled a project term' \
    '[project term]' "$out"
unstage_all

# The allow list applies to term rules only.
stage_file docs/allowed.md "$(plant_term)"
out=$(run_leak_check)
assert_equals 'staged: a project term in an allowed path passes' '0' "$(exit_of "$out")"
unstage_all

stage_file docs/allowed.md "$(plant_key)"
out=$(run_leak_check)
assert_equals 'staged: a credential in an allowed path is still blocked' '1' "$(exit_of "$out")"
unstage_all

# Placeholders and environment references are not secrets.
stage_file config.sh "$(printf 'password=${DB_PASSWORD}\napi_key = "<your-key-here>"\n')"
out=$(run_leak_check)
assert_equals 'staged: env references and placeholders pass' '0' "$(exit_of "$out")"
unstage_all

# The escape hatch works and says so.
stage_file notes.txt "$(plant_key)"
out=$(SKIP_LEAK_CHECK=1 run_leak_check)
assert_equals 'staged: SKIP_LEAK_CHECK skips the check' '0' "$(exit_of "$out")"
assert_contains 'staged: the skip is announced' 'SKIPPED' "$out"
unstage_all

# The script never scans itself.
stage_file tests/leak-check.sh "$(plant_key)"
out=$(run_leak_check)
assert_equals 'staged: the script excludes its own path' '0' "$(exit_of "$out")"
unstage_all

finish
```

- [ ] **Step 2: Make it executable and run it against the unmodified script**

Run: `chmod +x tests/leak-check.test.sh && bash tests/leak-check.test.sh`
Expected: `leak-check: 15 passed, 0 failed`. These characterise current behaviour; a failure here means the characterisation is wrong, not the script. Fix the test, not the script.

- [ ] **Step 3: Commit**

```bash
cd /Users/austin
/usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin add tests/leak-check.test.sh
/usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin commit -F - <<'EOF'
Pin the leak check's staged-mode behaviour with tests

tests/leak-check.sh gates every commit against leaking a secret to the
public repo and had no test file. This characterises what it does today
before the range-mode refactor: each layer-1 rule catches its planted
fake, the project term rules honour the allow list and the credential
rules do not, placeholders and env references pass, SKIP_LEAK_CHECK
skips, and the script never scans itself.

The term rules are injected through LEAK_PATTERN_FILE and LEAK_ALLOW_FILE
so the suite never reads or prints a real term.
EOF
```

---

### Task 2: Add `--range` mode to the leak check

**Files:**
- Modify: `tests/leak-check.sh:23-43` (header line, source selection), `tests/leak-check.sh:101-102` (term-scope source), `tests/leak-check.sh:113-126` (block message)
- Test: `tests/leak-check.test.sh` (append range-mode section before `finish`)

**Interfaces:**
- Consumes: the helpers from Task 1.
- Produces: `tests/leak-check.sh [--range <a>..<b>]`. With no argument, staged mode, messages say `pre-commit:` and `COMMIT BLOCKED`, exit 0 clean / 1 blocked. With `--range`, scans every commit in `<a>..<b>`, messages say `pre-push:` and `PUSH BLOCKED`, same exit codes. Any other argument: usage on stderr, exit 2. Task 3 depends on exactly this contract.

- [ ] **Step 1: Append the failing range-mode tests**

Insert before the final `finish` line of `tests/leak-check.test.sh`:

```bash
# --- range mode ------------------------------------------------------------
#
# Builds history in the fixture. `base` is the last pushed commit; everything
# after it is what a push would publish.

commit_file() {
    local path=$1 content=$2 message=$3
    stage_file "$path" "$content"
    git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "$message"
    git -C "$repo" rev-parse HEAD
}

unstage_all
base=$(commit_file README.md "$(clean_text)" 'base')

clean_tip=$(commit_file more.txt "$(clean_text)" 'clean change')
out=$(run_leak_check --range "$base..$clean_tip")
assert_equals 'range: clean commits pass' '0' "$(exit_of "$out")"

leaky_tip=$(commit_file notes.txt "$(plant_key)" 'oops')
out=$(run_leak_check --range "$base..$leaky_tip")
assert_equals 'range: a secret in a pushed commit is blocked' '1' "$(exit_of "$out")"
assert_contains 'range: the block message names the push' 'PUSH BLOCKED' "$out"
assert_contains 'range: the credential label is the same as staged mode' \
    '[possible credential]' "$out"

# The secret is added in one commit and removed in the next. The net diff is
# empty, but both commits are pushed, so the secret is published.
removed_tip=$(commit_file notes.txt "$(clean_text)" 'remove it')
out=$(run_leak_check --range "$leaky_tip..$removed_tip")
assert_equals 'range: removing a secret is itself clean' '0' "$(exit_of "$out")"
out=$(run_leak_check --range "$clean_tip..$removed_tip")
assert_equals 'range: a secret added then removed inside the range is still blocked' \
    '1' "$(exit_of "$out")"

# A range that only touches the script itself is not scanned.
self_tip=$(commit_file tests/leak-check.sh "$(plant_key)" 'edit the guard')
out=$(run_leak_check --range "$removed_tip..$self_tip")
assert_equals 'range: the script excludes its own path' '0' "$(exit_of "$out")"

# The allow list still applies to term rules only.
term_tip=$(commit_file docs/allowed.md "$(plant_term)" 'allowed term')
out=$(run_leak_check --range "$self_tip..$term_tip")
assert_equals 'range: a project term in an allowed path passes' '0' "$(exit_of "$out")"
key_tip=$(commit_file docs/allowed.md "$(plant_key)" 'credential in allowed path')
out=$(run_leak_check --range "$term_tip..$key_tip")
assert_equals 'range: a credential in an allowed path is still blocked' '1' "$(exit_of "$out")"

# The escape hatch works in range mode and names it.
out=$(SKIP_LEAK_CHECK=1 run_leak_check --range "$base..$leaky_tip")
assert_equals 'range: SKIP_LEAK_CHECK skips the check' '0' "$(exit_of "$out")"
assert_contains 'range: the skip names pre-push' 'pre-push' "$out"

# Misuse is a distinct exit code.
out=$(run_leak_check --range)
assert_equals 'range: a missing range value is a usage error' '2' "$(exit_of "$out")"
out=$(run_leak_check --bogus)
assert_equals 'an unknown argument is a usage error' '2' "$(exit_of "$out")"

# An empty range is clean.
out=$(run_leak_check --range "$key_tip..$key_tip")
assert_equals 'range: an empty range passes' '0' "$(exit_of "$out")"
```

- [ ] **Step 2: Run the tests to verify the new ones fail**

Run: `bash tests/leak-check.test.sh 2>&1 | grep -E 'FAIL|passed'`
Expected: the 15 staged-mode tests pass; the range-mode tests fail. The unmodified script ignores arguments and scans the (empty) index, so `--range` with a leaky commit exits 0 where 1 is expected, and `--range` alone exits 0 where 2 is expected.

- [ ] **Step 3: Implement range mode in the script**

Replace `tests/leak-check.sh` lines 23 through 43 (from the `# Scans staged content only` comment through `[ -z "$staged" ] && exit 0`) with:

```zsh
# Two modes:
#
#   (no argument)        scans staged content; run by tests/pre-commit
#   --range <a>..<b>     scans every commit in the range; run by tests/pre-push
#
# Range mode scans each commit's own diff, not the net diff of the range. A
# secret added in one commit and removed in the next has an empty net diff
# but is still in the pushed history, so it is still published.
#
# Override for a verified false positive:  SKIP_LEAK_CHECK=1 config commit ...
# Never use --no-verify; it skips every hook, the test suite included.

mode=commit
range=''
case "${1:-}" in
  '') ;;
  --range)
    if [ -z "${2:-}" ]; then
      echo "usage: leak-check.sh [--range <a>..<b>]" >&2
      exit 2
    fi
    mode=push
    range=$2
    ;;
  *)
    echo "usage: leak-check.sh [--range <a>..<b>]" >&2
    exit 2
    ;;
esac

if [ "$mode" = push ]; then
  hook=pre-push
  blocked='PUSH BLOCKED: this repo is PUBLIC and the pushed commits look internal.'
else
  hook=pre-commit
  blocked='COMMIT BLOCKED: this repo is PUBLIC and the staged content looks internal.'
fi

if [ -n "$SKIP_LEAK_CHECK" ]; then
  echo "$hook: leak check SKIPPED via SKIP_LEAK_CHECK" >&2
  exit 0
fi

PATTERN_FILE="${LEAK_PATTERN_FILE:-$HOME/.claude/local/leak-patterns.conf}"
ALLOW_FILE="${LEAK_ALLOW_FILE:-$HOME/.claude/local/leak-allow.conf}"

# The paths the scan covers, one per line, for the current mode.
changed_paths() {
  if [ "$mode" = push ]; then
    git log --format= --name-only --diff-filter=ACMR "$range" | grep -v '^$' | sort -u
  else
    git diff --cached --name-only --diff-filter=ACMR
  fi
}

# The added lines across the given paths (read from stdin, one per line),
# for the current mode. Only '+' lines, never the '+++' file header.
added_lines() {
  tr '\n' '\0' | if [ "$mode" = push ]; then
    xargs -0 git log --format= --no-color -U0 --diff-filter=ACMR -p "$range" -- 2>/dev/null
  else
    xargs -0 git diff --cached --no-color -U0 -- 2>/dev/null
  fi | grep '^+' | grep -v '^+++'
}

# This script's own source contains the generic patterns it searches for, so
# scanning it would always self-trip. Exclude it; it is reviewed by hand.
scan_paths=$(changed_paths | grep -v '^tests/leak-check\.sh$')
[ -z "$scan_paths" ] && exit 0

staged=$(echo "$scan_paths" | added_lines)
[ -z "$staged" ] && exit 0
```

Delete the original `if [ -n "$SKIP_LEAK_CHECK" ]` block (old lines 28 to 31) and the original `PATTERN_FILE` / `ALLOW_FILE` lines (old 33 to 34); they now live inside the block above.

Then replace the term-scope source (old lines 101 to 102):

```zsh
    term_staged=$(echo "$term_scope" | added_lines)
```

Then in the "term rules INACTIVE" message (old line 79) replace the literal `pre-commit:` with `$hook:`.

Then replace the block message (old line 115):

```zsh
  echo "$blocked" >&2
```

and, two lines further, make the skip hint mode-aware by replacing the `SKIP_LEAK_CHECK=1 config commit ...` line with:

```zsh
  if [ "$mode" = push ]; then
    echo "    SKIP_LEAK_CHECK=1 config push ..." >&2
  else
    echo "    SKIP_LEAK_CHECK=1 config commit ..." >&2
  fi
```

Also add `-a` to every `grep -inE` in the layer-1 and layer-2 `report` calls
(old lines 62, 68, 73, 108) so the report prints the matching lines. Without
it, `grep` can decide the concatenated diff is binary and print only
`Binary file (standard input) matches`, which hides the evidence the
message exists to show. This happened on the commit that added this plan.

Also update the header comment: replace line 4 `# Called by tests/pre-commit. Not installed as a hook directly, so that the repo` with `# Called by tests/pre-commit (staged mode) and tests/pre-push (range mode).` and delete the following line `# keeps a single hook entry point.` so the sentence reads cleanly. Delete the standalone `# Scans staged content only, so it judges what you are about to publish.` line if it survived the block replacement.

- [ ] **Step 4: Run the whole test file to verify everything passes**

Run: `bash tests/leak-check.test.sh`
Expected: `leak-check: 29 passed, 0 failed`.

- [ ] **Step 5: Confirm pre-commit still works against the real repo**

Run: `cd /Users/austin && printf 'harmless\n' > /tmp/lc-probe.txt && /usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin add tests/leak-check.sh && /usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin diff --cached --stat && ./tests/leak-check.sh; echo "exit=$?"`
Expected: the staged diff is the script itself, which is self-excluded, so `exit=0`. Remove the probe file: `rm /tmp/lc-probe.txt`.

- [ ] **Step 6: Commit**

```bash
cd /Users/austin
/usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin add tests/leak-check.sh tests/leak-check.test.sh
/usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin commit -F - <<'EOF'
Add a range mode to the leak check

`leak-check.sh --range <a>..<b>` scans every commit in the range with the
same two pattern layers and allow list as staged mode. It exists because
`git commit-tree` (the coming `config sync`) and `git commit --no-verify`
never invoke pre-commit, so without a push-time scan those commits reach
the public repo unscanned.

Range mode scans each commit's own diff via `git log -p`, not the net diff
of the range. A secret added in one commit and removed in the next has an
empty net diff but is still in the pushed history, so it is still
published; the test for that case is the reason for the choice.

Messages are mode-aware (PUSH BLOCKED / pre-push in range mode), an
unknown argument is exit 2, and the layers themselves are untouched: only
the source of the scanned lines changed.
EOF
```

---

### Task 3: Run the range scan from pre-push

**Files:**
- Modify: `tests/pre-push:9-11` (header comment), `tests/pre-push:30-50` (collect ranges in the loop), insert a new block before line 52 (`# --- branch-drift check`)
- Test: `tests/leak-check.test.sh` (append hook-wiring section before `finish`)

**Interfaces:**
- Consumes: `tests/leak-check.sh --range <a>..<b>` from Task 2, exit 0 / 1 / 2.
- Produces: `tests/pre-push` scans every pushed range before the drift check and exits 1 on a hit with its own `pre-push:` line. No other hook behaviour changes.

- [ ] **Step 1: Append the failing hook-wiring test**

Insert before the final `finish` line of `tests/leak-check.test.sh`:

```bash
# --- pre-push wiring -------------------------------------------------------
#
# Drives tests/pre-push directly with git's stdin protocol, pointed at the
# fixture through GIT_DIR / GIT_WORK_TREE. The pushed ref is a feature
# branch and the changed paths match no TRIGGER_PATH, so neither the drift
# check nor the Docker suite runs: the leak scan is the only gate exercised.

PRE_PUSH="$DOTFILES_ROOT/tests/pre-push"

run_pre_push() {
    local local_sha=$1 remote_sha=$2
    (
        cd "$repo" || exit 99
        printf 'refs/heads/feature %s refs/heads/feature %s\n' "$local_sha" "$remote_sha" \
            | GIT_DIR="$repo/.git" GIT_WORK_TREE="$repo" "$PRE_PUSH" origin "file://$repo" 2>&1 >/dev/null
        printf '\n__exit=%s\n' "$?"
    )
}

out=$(run_pre_push "$clean_tip" "$base")
assert_equals 'pre-push: a clean range is allowed through the leak gate' '0' "$(exit_of "$out")"

out=$(run_pre_push "$leaky_tip" "$base")
assert_equals 'pre-push: a range with a secret is blocked' '1' "$(exit_of "$out")"
assert_contains 'pre-push: the leak check reports the block' 'PUSH BLOCKED' "$out"
assert_contains 'pre-push: the hook names the failing gate' \
    'pre-push: leak check failed' "$out"

# A brand-new remote branch has the zero sha; the whole history is the range.
out=$(run_pre_push "$leaky_tip" '0000000000000000000000000000000000000000')
assert_equals 'pre-push: a new remote branch is scanned from the empty tree' '1' "$(exit_of "$out")"
```

- [ ] **Step 2: Run it to verify the new tests fail**

Run: `bash tests/leak-check.test.sh 2>&1 | grep -E 'FAIL|passed'`
Expected: the three blocking assertions fail (`expected: [1]`, `actual: [0]`), because pre-push does not call the leak check yet. The clean-range test passes already.

- [ ] **Step 3: Collect ranges in the pre-push loop and scan them first**

In `tests/pre-push`, replace lines 9 through 11:

```sh
# The leak guard runs twice: at pre-commit on staged content, and here in
# range mode over every commit being pushed. The push-time scan exists
# because `git commit-tree` (used by `config sync`) and `--no-verify` never
# invoke pre-commit, so a commit can exist locally that no scan has seen.
# The branch-drift check and the full suite run here only, so their cost is
# paid once per push, not once per commit.
```

Add one variable after line 28 (`push_ref=''`):

```sh
ranges=''
```

Inside the loop, after the `range_paths=` if/else (after line 47), add:

```sh
    if [ "$remote_sha" = "$zero" ]; then
        ranges="$ranges $empty_tree..$local_sha"
    else
        ranges="$ranges $remote_sha..$local_sha"
    fi
```

Insert this block immediately before the `# --- branch-drift check` comment (before line 52):

```sh
# --- leak scan ----------------------------------------------------------
#
# Runs before anything else: a leak must never reach the public remote,
# and this is the last gate before it does. One scan per pushed ref.

if [ ! -x "$HOME/tests/leak-check.sh" ]; then
    printf 'pre-push: ~/tests/leak-check.sh is missing or not executable\n' >&2
    exit 1
fi

for range in $ranges; do
    if ! "$HOME/tests/leak-check.sh" --range "$range"; then
        printf '\npre-push: leak check failed for %s, push blocked.\n' "$range" >&2
        exit 1
    fi
done
```

- [ ] **Step 4: Run the whole test file to verify everything passes**

Run: `bash tests/leak-check.test.sh`
Expected: `leak-check: 34 passed, 0 failed`.

- [ ] **Step 5: Run the full suite**

Run: `bash tests/run-all.sh 2>&1 | tail -3`
Expected: `all 21 suite(s) passed` (20 existing plus `leak-check.test.sh`). `githooks-installed.test.sh` still passes because the hook file's path and mode did not change.

- [ ] **Step 6: Commit**

```bash
cd /Users/austin
/usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin add tests/pre-push tests/leak-check.test.sh
/usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin commit -F - <<'EOF'
Scan every pushed range for leaks before the drift check

pre-push now collects one range per pushed ref (from the empty tree for a
new remote branch) and runs `leak-check.sh --range` on each before the
branch-drift check and the suite. Any hit blocks the push with the leak
check's own report plus a pre-push line naming the range.

This closes the gap that pre-commit alone leaves: `git commit-tree` (the
coming `config sync`) and `--no-verify` never invoke pre-commit, so a
commit could reach the public remote unscanned. The wiring test drives the
hook with git's stdin protocol against a fixture repo, on a branch and
paths that trigger neither the drift check nor Docker, so the leak gate is
the only thing exercised.
EOF
```

---

### Task 4: Sync to linux, push, and retire the TODO

**Files:**
- Modify: `TODO-AGENTS.md` (remove the leak-check item)
- Sync: `tests/leak-check.sh`, `tests/leak-check.test.sh`, `tests/pre-push` to `linux`

**Interfaces:**
- Consumes: the three commits above on `mac`.
- Produces: both branches pushed and matching on all shared paths; the TODO removed.

- [ ] **Step 1: Confirm the drift gate sees the divergence**

Run: cd /Users/austin && config-manifest check mac linux
Expected: `diverged: tests/` and a non-zero exit. That is correct; the next step resolves it.

- [ ] **Step 2: Sync the three shared files to linux and push it**

```bash
cd /Users/austin
g() { /usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin "$@"; }
g checkout -q linux
g checkout mac -- tests/leak-check.sh tests/leak-check.test.sh tests/pre-push
g status -s -uno
g commit -q -F - <<'EOF'
Sync the leak check range mode and pre-push scan from mac

All three files live under the shared tests/ path.
EOF
config-manifest check mac linux
g push origin linux
g checkout -q mac
```

Expected: `status` lists exactly the three files; the drift check reports `match on all 15 shared path(s)`; the linux push runs the pre-push (including the new leak scan over its own range) and the Docker suite, and succeeds.

- [ ] **Step 3: Remove the TODO item and push mac**

Delete the whole bullet in `TODO-AGENTS.md` that begins `- Give `tests/leak-check.sh` a `--range <a>..<b>` mode,` (through the line ending `and must land before `config sync` exists.`). Then:

```bash
cd /Users/austin
g() { /usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin "$@"; }
grep -c 'leak-check.sh. a .--range' TODO-AGENTS.md   # expect 0
g add TODO-AGENTS.md
g commit -q -m "Drop the leak-check range-mode TODO, landed

Step 0 of docs/superpowers/specs/2026-09-04-config-command-and-manifest-crate-design.md."
g push origin mac
g branch -vv | grep -E '^\*|linux'
```

Expected: both branches show `[origin/<branch>]` with no `ahead`; the mac push runs the new leak scan over its own range as the first gate.

---

## Self-review

**Spec coverage.** Section 3 decision (range mode, pre-push scans `<remote-sha>..<local-sha>`): Tasks 2 and 3. Section 8 pre-push row ("First, `leak-check.sh --range` ... any hit blocks the push"): Task 3 places it before the drift check. Section 10 step 0 (range mode, pre-push call, tests covering both modes in both directions, fixtures under the per-run temp dir, no real credentials, queued TODO done first): Tasks 1 through 4. Section 6.6's statement that the gate covering `sync` is at push time: satisfied by Task 3.

**Placeholder scan.** None. Every code step contains the code.

**Type and name consistency.** `run_leak_check`, `exit_of`, `stage_file`, `unstage_all`, `commit_file`, `plant_*`, `clean_text` are defined in Task 1 and used unchanged in Tasks 2 and 3. `$base`, `$clean_tip`, `$leaky_tip` are defined in Task 2 and used in Task 3. The hook's failure line `pre-push: leak check failed for <range>, push blocked.` in Task 3 step 3 matches the `assert_contains 'pre-push: leak check failed'` in Task 3 step 1. The script's `PUSH BLOCKED` text in Task 2 step 3 matches the assertions in Tasks 2 and 3. Test counts: 15 after Task 1, 29 after Task 2 (14 added), 34 after Task 3 (5 added).

**One thing the executor must know.** `tests/leak-check.sh` is zsh and uses `${1:-}`; running the test file needs zsh installed, which both machines and the Docker image have. The test drives the hook with `GIT_DIR` and `GIT_WORK_TREE` set explicitly because `lib.sh` unsets them at load for every fixture command.
