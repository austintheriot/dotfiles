#!/bin/bash
#
# Integration tests for tests/check-branch-drift.sh
#
# The script compares two git refs against every path listed in
# .sync-manifest and reports any that differ. Fixtures are real git
# repositories, not fakes: this is orchestration of `git diff`, and
# stubbing git would only test the stub.
#
# Usage: ~/tests/check-branch-drift.test.sh

. "$(dirname "$0")/lib.sh"

SCRIPT="$DOTFILES_ROOT/tests/check-branch-drift.sh"

# Builds a repo with a "linux" branch (default) and a "mac" branch, both
# starting from the same manifest and the same shared file content.
make_manifest_repo() {
    local repo
    repo=$(make_repo "$1" linux)
    printf '# comment, ignored\n\nshared.txt\n' > "$repo/.sync-manifest"
    printf 'same content\n' > "$repo/shared.txt"
    git -C "$repo" add .sync-manifest shared.txt
    git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "add manifest"
    git -C "$repo" branch mac
    printf '%s' "$repo"
}

run_check() {
    local repo=$1; shift
    DOTFILES_ROOT="$repo" "$SCRIPT" "$@"
}

# --- matching manifest paths pass ---------------------------------------

repo=$(make_manifest_repo repo-match)
output=$(run_check "$repo" mac linux)
status=$?
assert_equals 'matching paths exit 0' '0' "$status"
assert_contains 'reports the match' 'match on all 1 shared path' "$output"

# --- a diverged manifest path fails --------------------------------------

repo=$(make_manifest_repo repo-diverge)
git -C "$repo" checkout -q mac
printf 'different content\n' > "$repo/shared.txt"
git -C "$repo" add shared.txt
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "diverge on mac"
git -C "$repo" checkout -q linux

output=$(run_check "$repo" mac linux)
status=$?
assert_equals 'a diverged path exits non-zero' '1' "$status"
assert_contains 'names the diverged path' 'diverged: shared.txt' "$output"

# --- comments and blank lines in the manifest are ignored ----------------

repo=$(make_repo repo-comments linux)
printf '# just a comment\n\n\n' > "$repo/.sync-manifest"
git -C "$repo" add .sync-manifest
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "empty manifest"
git -C "$repo" branch mac

output=$(run_check "$repo" mac linux)
status=$?
assert_equals 'a manifest with only comments passes' '0' "$status"
assert_contains 'reports zero paths checked' 'match on all 0 shared path' "$output"

# --- excluded paths (! prefix) may differ without counting as drift -----

repo=$(make_repo repo-exclude linux)
mkdir -p "$repo/dir"
printf 'shared\n' > "$repo/dir/common.txt"
printf 'linux-only\n' > "$repo/dir/platform.txt"
printf 'dir/\n!dir/platform.txt\n' > "$repo/.sync-manifest"
git -C "$repo" add .sync-manifest dir
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "add dir with an excluded file"
git -C "$repo" branch mac
git -C "$repo" checkout -q mac
printf 'mac-only\n' > "$repo/dir/platform.txt"
git -C "$repo" add dir/platform.txt
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "diverge the excluded file on mac"
git -C "$repo" checkout -q linux

output=$(run_check "$repo" mac linux)
status=$?
assert_equals 'a diverged but excluded path still passes' '0' "$status"
assert_contains 'reports the directory as matched' 'match on all 1 shared path' "$output"

# --- a missing manifest fails loudly, not silently ------------------------

repo=$(make_repo repo-no-manifest linux)
git -C "$repo" branch mac

output=$(run_check "$repo" mac linux 2>&1)
status=$?
assert_equals 'a missing manifest exits non-zero' '1' "$status"
assert_contains 'names the missing manifest' '.sync-manifest' "$output"

# --- a ~ line is parsed, not compared -----------------------------------
#
# `~path` means "tracked on purpose, never compared". The comparison phase
# must not treat the line as a literal path named "~something", which would
# both count toward the total and always report as matching (git diff on a
# nonexistent path is empty).

repo=$(make_manifest_repo repo-tilde)
git -C "$repo" checkout -q mac
printf '# comment, ignored\n\nshared.txt\n~per-branch.txt\n' > "$repo/.sync-manifest"
printf 'mac version\n' > "$repo/per-branch.txt"
git -C "$repo" add .sync-manifest per-branch.txt
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "add tilde rule"
git -C "$repo" checkout -q linux
printf '# comment, ignored\n\nshared.txt\n~per-branch.txt\n' > "$repo/.sync-manifest"
printf 'linux version\n' > "$repo/per-branch.txt"
git -C "$repo" add .sync-manifest per-branch.txt
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "diverge per-branch file on linux"

output=$(run_check "$repo" mac linux)
status=$?
assert_equals 'a tilde path does not fail the check' '0' "$status"
assert_contains 'a tilde path is not counted as shared' \
    'match on all 1 shared path' "$output"

finish
