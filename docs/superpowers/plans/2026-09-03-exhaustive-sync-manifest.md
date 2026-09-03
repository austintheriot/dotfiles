# Exhaustive .sync-manifest Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `tests/check-branch-drift.sh` fail when a tracked file on either branch matches no `.sync-manifest` rule, so a file added on one branch cannot silently escape the drift check.

**Architecture:** A second phase is added to the existing script, after the current path comparison. It unions the tracked file lists from both refs, tests each file against every manifest rule (bare path = shared, `!path` = excluded from a shared path, `~path` = per-branch), and reports every file matching nothing. A new `~` sigil is introduced so a deliberately per-branch path can be declared rather than merely omitted. The manifest is labeled exhaustively first, so the new check is green the moment it is switched on.

**Tech Stack:** POSIX `sh` for `tests/check-branch-drift.sh` (matches the existing script), `bash` for `tests/check-branch-drift.test.sh` (repo convention for test files), the existing `tests/lib.sh` harness, GitHub Actions for `.github/workflows/branch-drift.yml`.

**Spec:** `docs/superpowers/specs/2026-09-03-exhaustive-sync-manifest-design.md`

## Global Constraints

- This repo is public. The pre-commit leak guard (`tests/leak-check.sh`) must pass on every commit; never bypass it with `--no-verify` or by disabling it.
- Run `~/tests/run-in-docker.sh` and confirm it passes before pushing any change to `tests/`, `.my-scripts/`, or `.github/workflows/`, per `.claude/rules/dotfiles-tests.md`. The pre-push hook runs the suite in Docker and blocks the push on failure.
- `tests/check-branch-drift.sh` uses `#!/bin/sh` (POSIX). It must pass both `sh -n` and `dash -n`: `/bin/sh` is bash on macOS and accepts bashisms dash rejects, so `sh -n` alone would pass a script that breaks on the `linux` branch. Test files use `#!/bin/bash`.
- `.sync-manifest`, `tests/`, and `.github/workflows/` are themselves listed in `.sync-manifest`, so every change in this plan must land byte-identically on both `mac` and `linux`. Commit on `mac`, then mirror to `linux` via a worktree, then verify `./tests/check-branch-drift.sh mac linux` reports a match before pushing either branch.
- Push `linux` before `mac`. The `branch-drift` Action compares `origin/mac` against `origin/linux` and fires on each push, so pushing the branch that is behind first avoids a spurious red run against a stale counterpart.
- Commit messages: lowercase, imperative, no period on the summary line, with a body paragraph when the summary is not self-evident.
- Never use `git add <directory>` on `.claude` or the repo root. Always name exact files.
- The manifest lists itself as shared, so both branches carry identical rules. A `~` rule naming a path that does not exist on the current branch is correct and expected, not an error.

---

### Task 1: Capture stderr in the workflow

A live defect, independent of the feature. `.github/workflows/branch-drift.yml` runs `output=$(tests/check-branch-drift.sh)`, and command substitution captures stdout only. The script writes its failure summary to stderr, so a real drift failure puts `diverged: tests/` in the step summary and drops the `diverged on N of M shared path(s)` line entirely. Fixing this first means every message Task 4 adds is actually visible.

**Files:**
- Modify: `.github/workflows/branch-drift.yml:27`

**Interfaces:**
- Consumes: nothing.
- Produces: a workflow whose `$output` variable contains both streams. Task 4's new messages depend on this; without it they are written to stderr and discarded.

- [ ] **Step 1: Reproduce the loss in a throwaway repo**

```bash
cd "$(mktemp -d)"
git init -q . && git config user.email t@t && git config user.name t
mkdir tests
printf 'tests/\n' > .sync-manifest
printf 'a\n' > tests/f.sh
git add -A && git commit -qm base && git branch linux
printf 'b\n' > tests/f.sh && git commit -qam diverge

# Exactly what the workflow does today:
output=$(DOTFILES_ROOT=$PWD ~/tests/check-branch-drift.sh master linux)
echo "--- what CI would show: ---"
echo "$output"
```

Expected: the output contains `diverged: tests/` but NOT the `diverged on 1 of 1 shared path(s)` summary line. That missing line is the bug.

- [ ] **Step 2: Confirm the redirect fixes it, in the same repo**

```bash
output=$(DOTFILES_ROOT=$PWD ~/tests/check-branch-drift.sh master linux 2>&1)
echo "$output"
```

Expected: both `diverged: tests/` and `check-branch-drift: master and linux diverged on 1 of 1 shared path(s)` are present.

- [ ] **Step 3: Apply the redirect**

In `.github/workflows/branch-drift.yml`, change the single line:

```yaml
          output=$(tests/check-branch-drift.sh)
```

to:

```yaml
          # 2>&1 because the script writes its failure summary to stderr,
          # and command substitution captures stdout only. Without this the
          # step summary shows the diverged paths but silently drops the
          # count line that says how many of how many.
          output=$(tests/check-branch-drift.sh 2>&1)
```

- [ ] **Step 4: Verify the YAML still parses**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/branch-drift.yml')); print('parses')"`
Expected: `parses`

- [ ] **Step 5: Run the full suite**

Run: `~/tests/run-in-docker.sh`
Expected: `all 15 suite(s) passed`

- [ ] **Step 6: Commit, mirror to linux, push**

```bash
cd ~
config add .github/workflows/branch-drift.yml
config commit -m "capture stderr from the drift check in CI

The workflow ran output=\$(tests/check-branch-drift.sh), and command
substitution captures stdout only while the script writes its summary to
stderr. A real drift failure put the diverged paths in the step summary and
dropped the line saying how many of how many diverged."

W=$(mktemp -d)
config worktree add "$W" linux
git -C "$W" checkout mac -- .github/workflows/branch-drift.yml
git -C "$W" commit -q -m "capture stderr from the drift check in CI

Mirrors mac. .github/workflows/ is a shared path."
./tests/check-branch-drift.sh mac linux    # expect: match on all 11
git -C "$W" push origin linux
config push origin mac
config worktree remove "$W" --force
```

Expected: the drift check reports `match on all 11 shared path(s)` before either push.

---

### Task 2: Teach the parser the `~` sigil

The `~` sigil marks a path as tracked but deliberately never compared. This task only makes the parser skip `~` lines without treating them as paths to compare; the exhaustiveness check that consumes them arrives in Task 4. Splitting it this way keeps the manifest labeling in Task 3 from being blocked on the new check.

**Files:**
- Modify: `tests/check-branch-drift.sh:64-70` (the comparison loop's `case` filter)
- Modify: `tests/check-branch-drift.sh:12-16` (the header comment)
- Test: `tests/check-branch-drift.test.sh`

**Interfaces:**
- Consumes: nothing.
- Produces: a manifest grammar where a line beginning with `~` is parsed and ignored by the comparison phase rather than treated as a literal path. Task 3 writes such lines; Task 4 reads them.

- [ ] **Step 1: Write the failing test**

Append to `tests/check-branch-drift.test.sh`, immediately before the final `finish` line:

```bash
# --- a ~ line is parsed, not compared -----------------------------------
#
# `~path` means "tracked on purpose, never compared". The comparison phase
# must not treat the line as a literal path named "~something", which would
# both count toward the total and always report as matching (git diff on a
# nonexistent path is empty).

repo=$(make_manifest_repo repo-tilde)
printf '# comment, ignored\n\nshared.txt\n~per-branch.txt\n' > "$repo/.sync-manifest"
printf 'mac version\n' > "$repo/per-branch.txt"
git -C "$repo" add .sync-manifest per-branch.txt
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "add tilde rule"
git -C "$repo" checkout -q linux
printf 'linux version\n' > "$repo/per-branch.txt"
git -C "$repo" add per-branch.txt
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "diverge per-branch file"
git -C "$repo" checkout -q mac

output=$(run_check "$repo" mac linux)
status=$?
assert_equals 'a tilde path does not fail the check' '0' "$status"
assert_contains 'a tilde path is not counted as shared' \
    'match on all 1 shared path' "$output"
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `~/tests/check-branch-drift.test.sh`
Expected: FAIL on `a tilde path is not counted as shared`, reporting `match on all 2 shared path(s)` instead of 1. The `~per-branch.txt` line is being counted as a path.

- [ ] **Step 3: Write the minimal implementation**

In `tests/check-branch-drift.sh`, in the comparison loop, change:

```sh
    case $path in
        ''|'#'*|'!'*) continue ;;
    esac
```

to:

```sh
    case $path in
        ''|'#'*|'!'*|'~'*) continue ;;
    esac
```

- [ ] **Step 4: Document the sigil in the header comment**

In `tests/check-branch-drift.sh`, after the existing paragraph describing `!`, add:

```sh
# A manifest line starting with "~" marks a path as tracked on purpose but
# never compared, because it is meant to differ per branch (.zshrc, a
# platform-only config). It is distinct from "!": "!" means "inside a shared
# path, skip this one", while "~" means "accounted for, never compared".
# Only "~" satisfies the exhaustiveness check below, which is what stops a
# new file from escaping by simply not being mentioned.
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `~/tests/check-branch-drift.test.sh`
Expected: `check-branch-drift: 12 passed, 0 failed`

- [ ] **Step 6: Verify the script still parses under both shells**

Run: `sh -n tests/check-branch-drift.sh && dash -n tests/check-branch-drift.sh && echo "both parse"`
Expected: `both parse`

- [ ] **Step 7: Prove the assertion can fail**

Revert the `case` line to omit `'~'*`, run the test, and confirm `a tilde path is not counted as shared` fails again. Then restore it.

Expected: the assertion fails with the mutation and passes without it. If it passes both ways, the assertion is not testing what it claims and must be fixed before continuing.

- [ ] **Step 8: Run the full suite**

Run: `~/tests/run-in-docker.sh`
Expected: `all 15 suite(s) passed`

- [ ] **Step 9: Commit, mirror to linux, push**

```bash
cd ~
config add tests/check-branch-drift.sh tests/check-branch-drift.test.sh
config commit -m "parse a ~ prefix as a per-branch manifest path

~path marks a path as tracked on purpose but never compared, for a file
meant to differ per branch. Distinct from !path, which means \"inside a
shared path, skip this one\". Only the comparison phase changes here; the
exhaustiveness check that consumes these lines comes next."

W=$(mktemp -d)
config worktree add "$W" linux
git -C "$W" checkout mac -- tests/check-branch-drift.sh tests/check-branch-drift.test.sh
git -C "$W" commit -q -m "parse a ~ prefix as a per-branch manifest path

Mirrors mac. tests/ is a shared path."
./tests/check-branch-drift.sh mac linux    # expect: match on all 11
git -C "$W" push origin linux
config push origin mac
config worktree remove "$W" --force
```

---

### Task 3: Label every currently tracked path

Labeling comes before the check is switched on, so the check is green the moment it exists. All 39 shared candidates are already byte-identical, verified in the spec, so marking them shared does not turn CI red.

**Files:**
- Modify: `.sync-manifest`

**Interfaces:**
- Consumes: the `~` grammar from Task 2.
- Produces: a manifest under which every tracked file on both refs matches some rule. Task 4's check depends on this; without it, enabling the check would immediately fail on 48 files.

- [ ] **Step 1: List the files that currently match no rule**

```bash
cd ~
python3 - <<'PY'
import subprocess
def git(*a):
    return subprocess.run(['git','--git-dir=/Users/austin/.cfg',
                           '--work-tree=/Users/austin',*a],
                          capture_output=True,text=True).stdout.splitlines()
manifest=[l.strip() for l in git('show','mac:.sync-manifest')
          if l.strip() and not l.startswith('#')]
include=[m for m in manifest if not m.startswith(('!','~'))]
files=sorted(set(git('ls-tree','-r','--name-only','mac')
                 + git('ls-tree','-r','--name-only','linux')))
unc=[f for f in files if not any(f==i or f.startswith(i) for i in include)]
print('unlabeled: %d' % len(unc))
for f in unc: print('  ', f)
PY
```

Expected: 49 files (48 on `mac`, plus `.zshrc-linux` which exists only on `linux`).

- [ ] **Step 2: Confirm the shared candidates really are identical**

```bash
cd ~
for p in .config/nvim .agents .claude/data DOTFILES.md; do
  if config diff --quiet mac linux -- "$p"; then
    echo "IDENTICAL: $p"
  else
    echo "DIFFERS:   $p   <- do not mark shared"
  fi
done
```

Expected: all four report `IDENTICAL`. If any reports `DIFFERS`, stop: either mirror it first or label it `~` instead, and note the deviation.

- [ ] **Step 3: Rewrite the manifest**

Replace the whole of `.sync-manifest` with:

```
# Every tracked file must match a rule in this file. A file matching nothing
# fails tests/check-branch-drift.sh, which is what stops a file added on one
# branch from silently escaping the comparison.
#
# Three kinds of rule:
#   path     shared: must be byte-identical on both branches
#   !path    excluded from an enclosing shared path
#   ~path    per-branch: tracked on purpose, never compared
#
# A trailing slash means "this directory and everything under it". Without
# one, the rule matches that exact path only, so DOTFILES.md does not cover
# DOTFILES.md.bak.
#
# This file lists itself, so both branches carry identical rules. A ~ rule
# naming a path that does not exist on this branch is normal, not an error:
# ~.zshrc-mac is listed on linux too.

# --- shared -------------------------------------------------------------

.sync-manifest
.github/workflows/
.claude/agents/
.claude/rules/
.claude/skills/
.claude/scripts/
.claude/data/
.claude/CLAUDE.md
tests/
.my-scripts/
docs/research/
docs/superpowers/
.config/tmux/tmux-common.conf
.config/nvim/
.agents/
DOTFILES.md

# --- excluded from a shared path ----------------------------------------

# macOS-only: drives aerospace and osascript, so it cannot pass on linux.
!tests/notify.test.sh
# Per-branch by design: each machine's own extra dependencies.
!.my-scripts/deps/deps-local.conf

# --- per-branch by design -----------------------------------------------

~.zshrc
~.zshrc-mac
~.zshrc-linux
~.claude/hooks/notify.sh
~.config/aerospace/
~.config/alacritty/
~.config/iterm-profiles/
~.config/tmux/tmux.conf
~README.md
~TODO-AGENTS.md
```

- [ ] **Step 4: Verify nothing is left unlabeled**

Re-run the Step 1 script.
Expected: `unlabeled: 0`

- [ ] **Step 5: Verify the comparison still passes**

Run: `./tests/check-branch-drift.sh mac linux`
Expected: `check-branch-drift: mac and linux match on all 16 shared path(s)` — 16 rather than 11, because `.claude/data/`, `docs/superpowers/`, `.config/nvim/`, `.agents/`, and `DOTFILES.md` are now compared. If any of those five reports `diverged`, Step 2's check was wrong; mirror the file or relabel it `~`.

- [ ] **Step 6: Run the full suite**

Run: `~/tests/run-in-docker.sh`
Expected: `all 15 suite(s) passed`

- [ ] **Step 7: Commit, mirror to linux, push**

```bash
cd ~
config add .sync-manifest
config commit -m "label every tracked path in the sync manifest

Adds the ~ per-branch section and brings the previously unlabeled paths
under a rule. 39 files were already byte-identical on both branches and
kept in sync by hand with nothing enforcing it: .config/nvim/ (32 files),
.agents/ (5), .claude/data/ (1) and DOTFILES.md. Those become shared, so the
first unmirrored change fails instead of drifting quietly.

The nine genuinely platform-specific paths become ~. Labeling lands before
the exhaustiveness check so the check is green the moment it is enabled."

W=$(mktemp -d)
config worktree add "$W" linux
git -C "$W" checkout mac -- .sync-manifest
git -C "$W" commit -q -m "label every tracked path in the sync manifest

Mirrors mac. .sync-manifest lists itself as shared."
./tests/check-branch-drift.sh mac linux    # expect: match on all 16
git -C "$W" push origin linux
config push origin mac
config worktree remove "$W" --force
```

---

### Task 4: Add the exhaustiveness check

**Files:**
- Modify: `tests/check-branch-drift.sh` (new phase after the comparison loop, before the final exit)
- Test: `tests/check-branch-drift.test.sh`

**Interfaces:**
- Consumes: the `~` grammar from Task 2, the fully labeled manifest from Task 3.
- Produces: `check-branch-drift.sh` exits 1 when any tracked file on either ref matches no rule, listing each such file one per line with a `(on mac)` / `(on linux)` / `(on mac, linux)` annotation, followed by the three labeling options. The `branch-drift` workflow surfaces this through the stderr capture from Task 1.

- [ ] **Step 1: Write the failing tests**

Append to `tests/check-branch-drift.test.sh`, immediately before the final `finish` line:

```bash
# --- a tracked file matching no rule fails the check ---------------------
#
# This is the gap the exhaustiveness phase closes. A new file inside a
# listed directory is already caught, because rules are path prefixes; a new
# path outside every rule was previously invisible, and the check reported
# "match on all" while the branches genuinely differed.

repo=$(make_manifest_repo repo-unlabeled)
printf 'brand new\n' > "$repo/brand-new.txt"
git -C "$repo" add brand-new.txt
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "add unlabeled file"

output=$(run_check "$repo" mac linux 2>&1)
status=$?
assert_equals 'an unlabeled file exits non-zero' '1' "$status"
assert_contains 'names the unlabeled file' 'brand-new.txt' "$output"
assert_contains 'says how many are unlabeled' 'match no .sync-manifest rule' "$output"
assert_contains 'offers the shared label' 'shared: must be identical' "$output"
assert_contains 'offers the per-branch label' 'per-branch: tracked, never compared' "$output"

# --- the annotation names the branch the file is really on ---------------
#
# A file present on one branch only usually tells the reader which label it
# wants, so the branch is reported rather than left to be guessed.

assert_contains 'annotates the branch the file is on' '(on mac)' "$output"

# --- a file unlabeled on the SECOND ref is caught too --------------------
#
# The file lists must be unioned across both refs. Scanning ref-a alone
# would let a file added only on ref-b escape, which is the same class of
# bug this phase exists to close.

repo=$(make_manifest_repo repo-unlabeled-b)
git -C "$repo" checkout -q linux
printf 'linux only\n' > "$repo/linux-only.txt"
git -C "$repo" add linux-only.txt
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "add linux-only file"
git -C "$repo" checkout -q mac

output=$(run_check "$repo" mac linux 2>&1)
status=$?
assert_equals 'an unlabeled file on ref-b exits non-zero' '1' "$status"
assert_contains 'names the ref-b file' 'linux-only.txt' "$output"
assert_contains 'annotates it as linux' '(on linux)' "$output"

# --- a ~ label satisfies the check --------------------------------------

repo=$(make_manifest_repo repo-tilde-labeled)
printf '# comment, ignored\n\nshared.txt\n~per-branch.txt\n' > "$repo/.sync-manifest"
printf 'mac version\n' > "$repo/per-branch.txt"
git -C "$repo" add .sync-manifest per-branch.txt
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "label per-branch"

output=$(run_check "$repo" mac linux 2>&1)
status=$?
assert_equals 'a labeled per-branch file passes' '0' "$status"

# --- a new file under a shared directory needs no manifest edit ----------
#
# Rules ending in "/" cover everything beneath them, so adding a test or an
# nvim plugin does not require touching the manifest. Only a genuinely new
# top-level path does.

repo=$(make_manifest_repo repo-under-dir)
printf '# comment, ignored\n\nsub/\n' > "$repo/.sync-manifest"
mkdir -p "$repo/sub"
printf 'x\n' > "$repo/sub/thing.txt"
git -C "$repo" add .sync-manifest sub/thing.txt
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "add file under shared dir"
git -C "$repo" branch -f linux mac

output=$(run_check "$repo" mac linux 2>&1)
status=$?
assert_equals 'a file under a shared directory passes' '0' "$status"

# --- a non-directory rule does not match by prefix -----------------------
#
# "DOTFILES.md" must not cover "DOTFILES.md.bak". Without the trailing-slash
# requirement, a rule would silently absorb every path that merely starts
# with its name.

repo=$(make_manifest_repo repo-prefix)
printf '# comment, ignored\n\nfile.txt\n' > "$repo/.sync-manifest"
printf 'a\n' > "$repo/file.txt"
printf 'b\n' > "$repo/file.txt.bak"
git -C "$repo" add .sync-manifest file.txt file.txt.bak
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "add lookalike path"

output=$(run_check "$repo" mac linux 2>&1)
status=$?
assert_equals 'a lookalike path is not absorbed by prefix' '1' "$status"
assert_contains 'names the lookalike path' 'file.txt.bak' "$output"
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `~/tests/check-branch-drift.test.sh`
Expected: FAIL on `an unlabeled file exits non-zero` (exit 0, not 1) and on every assertion that looks for the new message. The exhaustiveness phase does not exist yet.

- [ ] **Step 3: Write the minimal implementation**

In `tests/check-branch-drift.sh`, insert this phase after the comparison loop's `IFS=$old_ifs` and before the `if [ "$diverged_count" -gt 0 ]` block:

```sh
# --- every tracked file must match a rule -------------------------------
#
# The comparison above only inspects paths the manifest names. A file
# outside every rule was invisible to it: the check reported "match on all"
# while the branches genuinely differed. This phase closes that by requiring
# each tracked file to match some rule.
#
# The file lists are unioned across both refs on purpose. The branches do
# not carry identical file sets, so scanning ref-a alone would let a file
# added only on ref-b escape -- the same bug in a different place.

files_a=$(git_cmd ls-tree -r --name-only "$REF_A" 2>/dev/null)
files_b=$(git_cmd ls-tree -r --name-only "$REF_B" 2>/dev/null)

# Reports which refs a path is on. A file present on one branch only is
# usually a file that wants the ~ label, so saying so saves the reader a
# lookup.
branches_for() {
    found=''
    printf '%s\n' "$files_a" | grep -qxF "$1" && found=$REF_A
    if printf '%s\n' "$files_b" | grep -qxF "$1"; then
        if [ -n "$found" ]; then found="$found, $REF_B"; else found=$REF_B; fi
    fi
    printf '%s' "$found"
}

# A rule ending in "/" covers everything beneath it. Any other rule matches
# that exact path only, so a rule named DOTFILES.md must not absorb
# DOTFILES.md.bak.
is_covered() {
    file=$1
    inner_ifs=$IFS
    IFS='
'
    for rule in $manifest; do
        IFS=$inner_ifs
        case $rule in
            ''|'#'*) IFS='
'; continue ;;
            '!'*|'~'*) rule=${rule#?} ;;
        esac
        [ -n "$rule" ] || { IFS='
'; continue; }
        case $file in
            "$rule") return 0 ;;
            "$rule"*) case $rule in */) return 0 ;; esac ;;
        esac
        IFS='
'
    done
    IFS=$inner_ifs
    return 1
}

unlabeled=''
unlabeled_count=0
old_ifs=$IFS
IFS='
'
for file in $(printf '%s\n%s\n' "$files_a" "$files_b" | sort -u); do
    IFS=$old_ifs
    [ -n "$file" ] || { IFS='
'; continue; }
    if ! is_covered "$file"; then
        unlabeled="$unlabeled$file	$(branches_for "$file")
"
        unlabeled_count=$((unlabeled_count + 1))
    fi
    IFS='
'
done
IFS=$old_ifs

if [ "$unlabeled_count" -gt 0 ]; then
    printf '\ncheck-branch-drift: %d tracked file(s) match no .sync-manifest rule\n\n' \
        "$unlabeled_count" >&2
    printf '%s' "$unlabeled" | while IFS='	' read -r file refs; do
        [ -n "$file" ] || continue
        printf '  %-44s (on %s)\n' "$file" "$refs" >&2
    done
    printf '\nEvery tracked file must match a rule, so a file added on one branch\n' >&2
    printf 'cannot escape this check. Add one of these to .sync-manifest:\n\n' >&2
    printf '  %-44s shared: must be identical on both branches\n' 'path/to/file' >&2
    printf '  %-44s per-branch: tracked, never compared\n' '~path/to/file' >&2
    printf '  %-44s excluded from an enclosing shared path\n\n' '!path/to/file' >&2
    exit 1
fi
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `~/tests/check-branch-drift.test.sh`
Expected: `check-branch-drift: 25 passed, 0 failed` (10 existing, plus 2 from Task 2, plus the 13 above)

- [ ] **Step 5: Verify the script parses under both shells**

Run: `sh -n tests/check-branch-drift.sh && dash -n tests/check-branch-drift.sh && echo "both parse"`
Expected: `both parse`

- [ ] **Step 6: Verify against the real repository**

Run: `./tests/check-branch-drift.sh mac linux`
Expected: `check-branch-drift: mac and linux match on all 16 shared path(s)`, exit 0. Task 3 labeled everything, so nothing should be reported unlabeled. If a file is reported, add its label to `.sync-manifest` in this commit and note why it was missed.

- [ ] **Step 7: Prove each new assertion can fail**

Run each mutation, confirm the named assertion fails, then restore the file:

1. Change `files_b=$(git_cmd ls-tree ...)` to `files_b=''`. Expected: `an unlabeled file on ref-b exits non-zero` fails. This is the union assertion; if it still passes, the union is not being tested.
2. Remove the `case $rule in */) return 0 ;; esac` guard so any prefix matches. Expected: `a lookalike path is not absorbed by prefix` fails.
3. Change `exit 1` in the new block to `exit 0`. Expected: `an unlabeled file exits non-zero` fails.

Expected: each mutation fails exactly the assertion named and no others. An assertion that passes under its mutation is not testing its claim and must be fixed before continuing.

- [ ] **Step 8: Confirm the CI output reads clearly**

```bash
cd "$(mktemp -d)"
git init -q . && git config user.email t@t && git config user.name t
mkdir tests && printf 'tests/\n' > .sync-manifest && printf 'a\n' > tests/f.sh
printf 'x\n' > unlabeled-thing.toml
git add -A && git commit -qm base && git branch linux
output=$(DOTFILES_ROOT=$PWD ~/tests/check-branch-drift.sh master linux 2>&1)
echo "$output"
```

Expected: the output names `unlabeled-thing.toml`, annotates it `(on master, linux)`, and lists all three label forms. Read it as someone who has not seen this plan: if it does not say which file and what to do, revise the wording before committing.

- [ ] **Step 9: Run the full suite**

Run: `~/tests/run-in-docker.sh`
Expected: `all 15 suite(s) passed`

- [ ] **Step 10: Commit, mirror to linux, push**

```bash
cd ~
config add tests/check-branch-drift.sh tests/check-branch-drift.test.sh
config commit -m "fail the drift check on a tracked file matching no rule

The comparison phase only inspects paths the manifest names, so a file
outside every rule was invisible: the check reported \"match on all\" while
the branches genuinely differed. Measured at 48 of 251 tracked files.

The file lists are unioned across both refs because the branches do not
carry identical file sets, so scanning one would let a file added only on
the other escape. Rules ending in / cover everything beneath them; any other
rule matches its exact path only, so DOTFILES.md does not absorb
DOTFILES.md.bak.

The report names every unlabeled file one per line with the branches it is
on, then the three label forms, because a gate that says only \"something is
unlabeled\" moves the investigation to whoever reads the red build."

W=$(mktemp -d)
config worktree add "$W" linux
git -C "$W" checkout mac -- tests/check-branch-drift.sh tests/check-branch-drift.test.sh
git -C "$W" commit -q -m "fail the drift check on a tracked file matching no rule

Mirrors mac. tests/ is a shared path."
./tests/check-branch-drift.sh mac linux    # expect: match on all 16
git -C "$W" push origin linux
config push origin mac
config worktree remove "$W" --force
```

---

### Task 5: Separate the two failure modes in the step summary

Drift and unlabeled paths are different problems with different fixes. The step summary currently drops both into one code fence under a single heading, so a reader cannot tell at a glance which one fired.

**Files:**
- Modify: `.github/workflows/branch-drift.yml:29-40` (the step-summary block)

**Interfaces:**
- Consumes: the stderr capture from Task 1 and the message format from Task 4.
- Produces: nothing later tasks depend on. This is the last task.

- [ ] **Step 1: Read the current step-summary block**

Run: `sed -n '21,45p' .github/workflows/branch-drift.yml`
Expected: a `run:` block that assigns `output`, echoes it, and appends a single `### Branch drift check` heading plus a fenced copy of `$output` to `$GITHUB_STEP_SUMMARY`.

- [ ] **Step 2: Rewrite the step-summary block**

Replace the body of the `Check for drift` step with:

```yaml
        run: |
          # No `set -e`: the assignment below must survive a non-zero exit so
          # the output and step summary are still written before exiting.
          #
          # 2>&1 because the script writes its failure summary to stderr, and
          # command substitution captures stdout only.
          output=$(tests/check-branch-drift.sh 2>&1)
          status=$?
          echo "$output"
          if [ -n "$GITHUB_STEP_SUMMARY" ]; then
            {
              if [ "$status" -eq 0 ]; then
                echo '### Branch drift check passed'
              elif printf '%s' "$output" | grep -q 'match no .sync-manifest rule'; then
                # Two distinct failures with two distinct fixes. Naming which
                # one fired saves the reader from reading the log to find out.
                echo '### Unlabeled files'
                echo
                echo 'A tracked file matches no `.sync-manifest` rule, so it'
                echo 'is invisible to the drift comparison. Label each file'
                echo 'listed below.'
              else
                echo '### Branch drift'
                echo
                echo 'A shared path differs between `mac` and `linux`.'
              fi
              echo
              echo '```'
              echo "$output"
              echo '```'
            } >> "$GITHUB_STEP_SUMMARY"
          fi
          exit "$status"
```

- [ ] **Step 3: Verify the YAML parses**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/branch-drift.yml')); print('parses')"`
Expected: `parses`

- [ ] **Step 4: Verify the heading selection logic**

```bash
# The unlabeled branch:
out='check-branch-drift: 1 tracked file(s) match no .sync-manifest rule'
printf '%s' "$out" | grep -q 'match no .sync-manifest rule' \
  && echo "-> Unlabeled files heading" || echo "-> WRONG"

# The drift branch:
out='diverged: tests/'
printf '%s' "$out" | grep -q 'match no .sync-manifest rule' \
  && echo "-> WRONG" || echo "-> Branch drift heading"
```

Expected: `-> Unlabeled files heading` then `-> Branch drift heading`.

- [ ] **Step 5: Run the full suite**

Run: `~/tests/run-in-docker.sh`
Expected: `all 15 suite(s) passed`

- [ ] **Step 6: Commit, mirror to linux, push**

```bash
cd ~
config add .github/workflows/branch-drift.yml
config commit -m "give drift and unlabeled paths separate step summaries

Two failures with two different fixes were landing under one heading in one
code fence, so a reader could not tell which had fired without reading the
log."

W=$(mktemp -d)
config worktree add "$W" linux
git -C "$W" checkout mac -- .github/workflows/branch-drift.yml
git -C "$W" commit -q -m "give drift and unlabeled paths separate step summaries

Mirrors mac. .github/workflows/ is a shared path."
./tests/check-branch-drift.sh mac linux    # expect: match on all 16
git -C "$W" push origin linux
config push origin mac
config worktree remove "$W" --force
```

- [ ] **Step 7: Confirm both Actions pass on both branches**

```bash
gh run list --repo austintheriot/dotfiles --limit 6
```

Expected: `Branch drift (mac vs linux)` green on `mac` and `linux`. A red `linux` run whose timestamp is seconds before the `mac` push is the known ordering race: confirm with `./tests/check-branch-drift.sh origin/mac origin/linux` after a fetch, and re-run the stale job rather than changing code.

- [ ] **Step 8: Verify the gate really fires end to end**

On a scratch branch, add an unlabeled file, commit, and confirm the pre-push hook blocks the push:

```bash
cd ~
printf 'x\n' > scratch-unlabeled.toml
config add scratch-unlabeled.toml
config commit -m "temp: prove the gate fires"
config push origin mac        # expect: blocked, naming scratch-unlabeled.toml
config reset --hard HEAD~1
rm -f scratch-unlabeled.toml
```

Expected: the push is refused and the message names `scratch-unlabeled.toml`. Then confirm `config status --porcelain` is clean and `config log --oneline -1` no longer shows the temp commit.

---

## Self-Review

**Spec coverage:**

| Spec section | Task |
|---|---|
| Manifest format, `~` sigil | 2, 3 |
| Exhaustiveness check, both-ref union | 4 |
| CI output: capture stderr | 1 |
| CI output: name every unlabeled path | 4 |
| CI output: separate failure modes | 5 |
| Testing: all five listed cases | 2, 4 |
| Migration: label first, then enable | 3 before 4 |
| Consequence: `docs/` partly covered | 3 (adds `docs/superpowers/`) |

**Placeholder scan:** No `TBD`, `TODO`, or "add appropriate handling". Every code step carries the literal text to write; every verification step carries the command and its expected output.

**Type consistency:** `is_covered()`, `branches_for()`, `files_a`, `files_b`, `unlabeled`, `unlabeled_count` are defined in Task 4 Step 3 and used only there. `manifest`, `git_cmd`, `REF_A`, `REF_B`, `old_ifs` are pre-existing in the script and reused with their existing meanings. `is_covered` uses `inner_ifs` rather than `old_ifs` because it is called from inside a loop that already owns `old_ifs`; reusing the name would clobber the caller's saved value.

**Note on the expected counts:** Task 3 Step 5 and later tasks expect `match on all 16 shared path(s)`. That is 11 existing rules plus `.claude/data/`, `docs/superpowers/`, `.config/nvim/`, `.agents/`, and `DOTFILES.md`. If the real count differs when Task 3 runs, the manifest gained or lost a rule since this plan was written: recount from the manifest rather than assuming the plan is right.
