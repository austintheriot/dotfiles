#!/bin/bash
#
# Tests for tests/leak-check.sh in both modes.
#
# Staged mode is what pre-commit runs. Range mode is what pre-push runs, and
# it exists because `git commit-tree` and `git commit --no-verify` never
# invoke pre-commit, so without a push-time scan those commits reach the
# public repo unscanned.
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
        # shellcheck disable=SC2069  # stderr-only capture: order is deliberate
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

# Regression for the term-scope filter matching nothing once more than one
# file is staged. term_headers is a multi-line list of "+++ b/<path>" lines;
# feeding it to awk through -v silently produces an empty lookup table
# (awk -v does not accept a value containing a newline), so with a SECOND
# staged file the term rule never sees any hunk as in scope and a leaked term
# passes clean. A single staged file happened to still work, because the
# lookup table being empty and the file's own header both failing to match
# looked the same as "not in scope" either way; two files is the shape that
# tells them apart.
stage_file first.txt "$(clean_text)"
stage_file second.txt "$(plant_term)"
out=$(run_leak_check)
assert_equals 'staged: a term in the second of two staged files is blocked' \
    '1' "$(exit_of "$out")"
assert_contains 'staged: the term is named even when it is not the first file' \
    "$FAKE_TERM" "$out"
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

# An unresolvable range must not fail open. Without validation, git's failure
# inside changed_paths would produce an empty scan_paths and a false-clean 0.
out=$(run_leak_check --range "0123456789abcdef0123456789abcdef01234567..HEAD")
assert_equals 'range: an unresolvable range is a usage error, not clean' \
    '2' "$(exit_of "$out")"
assert_contains 'range: the error names the unresolved range' \
    'cannot resolve range' "$out"

# An empty range is clean.
out=$(run_leak_check --range "$key_tip..$key_tip")
assert_equals 'range: an empty range passes' '0' "$(exit_of "$out")"

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
assert_contains 'the failure names the range' 'HEAD~1..HEAD' "$output"

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
assert_contains 'the block names the unscannable path' 'hidden.txt' "$output"

# A renamed file must not read as unscannable in range mode.
#
# git's rename detection pairs an add and a delete of similar content into ONE
# entry reported under the NEW path, while `--name-only` (which builds the
# path list) still lists the OLD path. So the old path has no `+++ b/<path>`
# header and the guard blocked the push for a path carrying nothing. Nine
# paths blocked this way on the first push of `main`, every one a file moved
# years ago. RANGE_LOG_FLAGS carries --no-renames for this reason.
rename_repo="$FIXTURES/rename-range"
mkdir -p "$rename_repo"
git -C "$rename_repo" init -q .
git -C "$rename_repo" config user.email t@t
git -C "$rename_repo" config user.name t
printf 'seed\n' > "$rename_repo/seed.txt"
git -C "$rename_repo" add seed.txt
git -C "$rename_repo" commit -q -m seed

mkdir -p "$rename_repo/olddir"
printf 'a line\nb line\nc line\nd line\ne line\n' > "$rename_repo/olddir/moved.txt"
git -C "$rename_repo" add olddir/moved.txt
git -C "$rename_repo" commit -q -m 'add the file at its old path'

mkdir -p "$rename_repo/newdir"
git -C "$rename_repo" mv olddir/moved.txt newdir/moved.txt
git -C "$rename_repo" commit -q -m 'move it, which git reports as a rename'

# The range spans both commits, so the old path appears in --name-only and
# the rename collapses its header under the new path.
status=0
(cd "$rename_repo" && "$LEAK_CHECK" --range 'HEAD~2..HEAD' >/dev/null 2>&1) || status=$?
assert_equals 'a renamed path does not read as unscannable' '0' "$status"

# An empty file counts as scanned, not as unscannable.
#
# Git emits a `diff --git` line for a zero-byte file and no hunk, so it never
# produces a `+++` header. It also cannot carry a secret. Blocking on it
# refused a push over a 0-byte file nobody references.
empty_repo="$FIXTURES/empty-range"
mkdir -p "$empty_repo"
git -C "$empty_repo" init -q .
git -C "$empty_repo" config user.email t@t
git -C "$empty_repo" config user.name t
printf 'seed\n' > "$empty_repo/seed.txt"
git -C "$empty_repo" add seed.txt
git -C "$empty_repo" commit -q -m seed

: > "$empty_repo/placeholder"
git -C "$empty_repo" add placeholder
git -C "$empty_repo" commit -q -m 'add a zero-byte file'

status=0
(cd "$empty_repo" && "$LEAK_CHECK" --range 'HEAD~1..HEAD' >/dev/null 2>&1) || status=$?
assert_equals 'an empty file does not read as unscannable' '0' "$status"

# The positive control: a genuinely unreadable file in the same shape must
# still block, or the fix above has made the guard useless rather than
# correct.
printf 'placeholder -diff\n' > "$empty_repo/.gitattributes"
printf 'real content that cannot be read\n' > "$empty_repo/placeholder"
git -C "$empty_repo" add .gitattributes placeholder
git -C "$empty_repo" commit -q -m 'mark a NON-empty file unreadable'

status=0
(cd "$empty_repo" && "$LEAK_CHECK" --range 'HEAD~1..HEAD' >/dev/null 2>&1) || status=$?
assert_equals 'a non-empty unreadable file still blocks' '2' "$status"

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
    'styles.css' "$output"

git -C "$repo" reset -q HEAD styles.css vendor/styles.css .gitattributes
rm -rf "$repo/styles.css" "$repo/vendor" "$repo/.gitattributes"

# --- range mode: merge commits ----------------------------------------------
#
# `git log -p` shows no diff for a merge commit by default, so content that
# exists only in the merge itself (an "evil merge": a conflict resolution, or
# extra content stapled on during the merge) is invisible unless the scan
# passes --diff-merges=first-parent. Side-branch commits are scanned either
# way because they are walked as ordinary commits in the range.

merge_base=$(commit_file base.txt "$(clean_text)" 'merge base')

git -C "$repo" checkout -q -b left "$merge_base"
commit_file left.txt "$(clean_text)" 'left branch change' >/dev/null

git -C "$repo" checkout -q -b right "$merge_base"
commit_file right.txt "$(clean_text)" 'right branch change' >/dev/null

git -C "$repo" checkout -q left
git -C "$repo" merge -q --no-commit --no-ff right >/dev/null 2>&1
stage_file evil.txt "$(plant_key)"
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m 'evil merge'
evil_merge=$(git -C "$repo" rev-parse HEAD)

out=$(run_leak_check --range "$merge_base..$evil_merge")
assert_equals 'range: a secret introduced only in a merge commit is blocked' \
    '1' "$(exit_of "$out")"

git -C "$repo" checkout -q left
git -C "$repo" checkout -q -b left2 "$merge_base"
commit_file left2.txt "$(clean_text)" 'left2 branch change' >/dev/null
git -C "$repo" checkout -q -b right2 "$merge_base"
commit_file right2.txt "$(clean_text)" 'right2 branch change' >/dev/null
git -C "$repo" checkout -q left2
git -C "$repo" merge -q --no-edit right2 >/dev/null 2>&1
clean_merge=$(git -C "$repo" rev-parse HEAD)

out=$(run_leak_check --range "$merge_base..$clean_merge")
assert_equals 'range: an ordinary clean merge passes' '0' "$(exit_of "$out")"

git -C "$repo" checkout -q main

# --- pre-push wiring -------------------------------------------------------
#
# Drives tests/pre-push directly with git's stdin protocol, pointed at the
# fixture through GIT_DIR / GIT_WORK_TREE. The pushed ref is a feature
# branch and the changed paths match no TRIGGER_PATH, so neither the drift
# check nor the Docker suite runs: the leak scan is the only gate exercised.

PRE_PUSH="$DOTFILES_ROOT/tests/pre-push"

# HOME is set alongside GIT_DIR/GIT_WORK_TREE because the hook resolves its
# own scripts as "$HOME/tests/...". On CI, DOTFILES_ROOT is the checkout while
# HOME is the runner home, so without this the hook looks for leak-check.sh in
# the wrong tree and every assertion below fails. The pushed ref is `feature`,
# so the config-manifest stamp block never runs.
run_pre_push() {
    local local_sha=$1 remote_sha=$2
    (
        cd "$repo" || exit 99
        # shellcheck disable=SC2069  # stderr-only capture: order is deliberate
        printf 'refs/heads/feature %s refs/heads/feature %s\n' "$local_sha" "$remote_sha" \
            | HOME="$DOTFILES_ROOT" GIT_DIR="$repo/.git" GIT_WORK_TREE="$repo" \
                "$PRE_PUSH" origin "file://$repo" 2>&1 >/dev/null
        printf '\n__exit=%s\n' "$?"
    )
}

# Like run_pre_push, but captures stdout+stderr combined, for assertions on
# the hook's success-path announcements (which print to stdout).
run_pre_push_combined() {
    local local_sha=$1 remote_sha=$2
    (
        cd "$repo" || exit 99
        printf 'refs/heads/feature %s refs/heads/feature %s\n' "$local_sha" "$remote_sha" \
            | HOME="$DOTFILES_ROOT" GIT_DIR="$repo/.git" GIT_WORK_TREE="$repo" \
                "$PRE_PUSH" origin "file://$repo" 2>&1
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

# The remote sha is a well-formed but unknown object (not the zero sha, the
# "new branch" sentinel). leak-check.sh cannot resolve the range and exits 2;
# pre-push must report that distinctly from an actual leak, not collapse it
# into "leak check failed".
out=$(run_pre_push "$leaky_tip" 'abcdef1234567890abcdef1234567890abcdef12')
assert_equals 'pre-push: an unresolvable range still blocks the push' '1' "$(exit_of "$out")"
assert_contains 'pre-push: an unresolvable range is reported as could-not-scan, not a leak' \
    'could not scan' "$out"

# A delete-only push has the zero LOCAL sha for every ref, so the loop that
# builds "ranges" never runs. The hook must still say so, per its own
# convention that a skipped gate announces itself.
out=$(run_pre_push_combined '0000000000000000000000000000000000000000' "$base")
assert_equals 'pre-push: a delete-only push passes' '0' "$(exit_of "$out")"
assert_contains 'pre-push: a delete-only push announces no range to scan' \
    'no pushed range to scan' "$out"

out=$(run_pre_push_combined "$clean_tip" "$base")
assert_contains 'pre-push: a clean push announces the scanned range count' \
    'leak scan passed for 1 range(s)' "$out"

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

# The subshell keeps the `cd` local and the `|| true` outside the
# substitution rather than inside it: `$(A && B || true)` reads as an
# if-then-else it is not, and shellcheck flags it (SC2015) on the version the
# Docker gate runs.
output=$( (cd "$repo" && SKIP_LEAK_CHECK=maybe "$LEAK_CHECK" 2>&1) ) || true
assert_contains 'an unrecognized skip value is announced' 'not a recognized' "$output"

git -C "$repo" reset -q HEAD skip-probe.txt
rm -f "$repo/skip-probe.txt"

finish
