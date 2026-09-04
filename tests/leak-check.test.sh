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

finish
