#!/bin/bash
#
# Integration tests for the branch-drift check (shell script and config-manifest binary)
#
# The script compares two git refs against every path listed in
# .sync-manifest and reports any that differ. Fixtures are real git
# repositories, not fakes: this is orchestration of `git diff`, and
# stubbing git would only test the stub.
#
# Usage: ~/tests/check-branch-drift.test.sh

. "$(dirname "$0")/lib.sh"

# CHECK_CMD selects the implementation under test. The default is the shell
# script; `CHECK_CMD='config-manifest check'` runs the same 25 assertions
# against the Rust binary. Word-splitting the unquoted expansion is the
# intent: the value is a command plus its subcommand.
CHECK_CMD=${CHECK_CMD:-$DOTFILES_ROOT/tests/check-branch-drift.sh}

# Builds a repo with a "linux" branch (default) and a "mac" branch, both
# starting from the same manifest and the same shared file content.
make_manifest_repo() {
    local repo
    repo=$(make_repo "$1" linux)
    printf '# comment, ignored\n\n.sync-manifest\nshared.txt\n' > "$repo/.sync-manifest"
    printf 'same content\n' > "$repo/shared.txt"
    git -C "$repo" add .sync-manifest shared.txt
    git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "add manifest"
    git -C "$repo" branch mac
    printf '%s' "$repo"
}

run_check() {
    local repo=$1; shift
    DOTFILES_ROOT="$repo" $CHECK_CMD "$@"
}

# --- matching manifest paths pass ---------------------------------------

repo=$(make_manifest_repo repo-match)
output=$(run_check "$repo" mac linux)
status=$?
assert_equals 'matching paths exit 0' '0' "$status"
assert_contains 'reports the match' 'match on all 2 shared path' "$output"

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
printf '# just a comment\n\n.sync-manifest\n' > "$repo/.sync-manifest"
git -C "$repo" add .sync-manifest
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "empty manifest"
git -C "$repo" branch mac

output=$(run_check "$repo" mac linux)
status=$?
assert_equals 'a manifest with only comments passes' '0' "$status"
assert_contains 'reports only the self-listed manifest as checked' \
    'match on all 1 shared path' "$output"

# --- excluded paths (! prefix) may differ without counting as drift -----

repo=$(make_repo repo-exclude linux)
mkdir -p "$repo/dir"
printf 'shared\n' > "$repo/dir/common.txt"
printf 'linux-only\n' > "$repo/dir/platform.txt"
printf '.sync-manifest\ndir/\n!dir/platform.txt\n' > "$repo/.sync-manifest"
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
assert_contains 'reports the directory as matched' 'match on all 2 shared path' "$output"

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
printf '# comment, ignored\n\n.sync-manifest\nshared.txt\n~per-branch.txt\n' > "$repo/.sync-manifest"
printf 'mac version\n' > "$repo/per-branch.txt"
git -C "$repo" add .sync-manifest per-branch.txt
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "add tilde rule"
git -C "$repo" checkout -q linux
printf '# comment, ignored\n\n.sync-manifest\nshared.txt\n~per-branch.txt\n' > "$repo/.sync-manifest"
printf 'linux version\n' > "$repo/per-branch.txt"
git -C "$repo" add .sync-manifest per-branch.txt
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "diverge per-branch file on linux"

output=$(run_check "$repo" mac linux)
status=$?
assert_equals 'a tilde path does not fail the check' '0' "$status"
assert_contains 'a tilde path is not counted as shared' \
    'match on all 2 shared path' "$output"

# --- a tracked file matching no rule fails the check ---------------------
#
# This is the gap the exhaustiveness phase closes. A new file inside a
# listed directory is already caught, because rules are path prefixes; a new
# path outside every rule was previously invisible, and the check reported
# "match on all" while the branches genuinely differed.

repo=$(make_manifest_repo repo-unlabeled)
git -C "$repo" checkout -q mac
printf 'brand new\n' > "$repo/brand-new.txt"
git -C "$repo" add brand-new.txt
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "add unlabeled file"
git -C "$repo" checkout -q linux

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

# --- the unmatched block's exact bytes, not just a substring -------------
#
# assert_contains above proves the right words appear somewhere in the
# output; it cannot catch a whitespace or stream-routing regression (a
# missing blank line, output on the wrong stream, a shifted padding width).
# This assertion pins the unmatched block byte for byte, built with the same
# printf format the implementation uses, so the padding comes from %-44s
# itself rather than a hand-counted literal.

stderr_only=$(run_check "$repo" mac linux 2>&1 >/dev/null)
expected_stderr=$(
    printf '\ncheck-branch-drift: 1 tracked file(s) match no .sync-manifest rule\n\n'
    printf '  %-44s (on %s)\n' 'brand-new.txt' 'mac'
    printf '\nEvery tracked file must match a rule, so a file added on one branch\n'
    printf 'cannot escape this check. Add one of these to .sync-manifest:\n\n'
    printf '  %-44s shared: must be identical on both branches\n' 'path/to/file'
    printf '  %-44s per-branch: tracked, never compared\n' '~path/to/file'
    printf '  %-44s excluded from an enclosing shared path\n\n' '!path/to/file'
)
assert_equals 'the unmatched block matches byte for byte' "$expected_stderr" "$stderr_only"

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
printf '# comment, ignored\n\n.sync-manifest\nshared.txt\n~per-branch.txt\n' > "$repo/.sync-manifest"
printf 'mac version\n' > "$repo/per-branch.txt"
git -C "$repo" add .sync-manifest per-branch.txt
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "label per-branch"
git -C "$repo" branch -f mac linux

output=$(run_check "$repo" mac linux 2>&1)
status=$?
assert_equals 'a labeled per-branch file passes' '0' "$status"

# --- a new file under a shared directory needs no manifest edit ----------
#
# Rules ending in "/" cover everything beneath them, so adding a test or an
# nvim plugin does not require touching the manifest. Only a genuinely new
# top-level path does.

repo=$(make_manifest_repo repo-under-dir)
printf '# comment, ignored\n\n.sync-manifest\nshared.txt\nsub/\n' > "$repo/.sync-manifest"
mkdir -p "$repo/sub"
printf 'x\n' > "$repo/sub/thing.txt"
git -C "$repo" add .sync-manifest sub/thing.txt
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "add file under shared dir"
git -C "$repo" branch -f mac linux

output=$(run_check "$repo" mac linux 2>&1)
status=$?
assert_equals 'a file under a shared directory passes' '0' "$status"

# --- a non-directory rule does not match by prefix -----------------------
#
# "DOTFILES.md" must not cover "DOTFILES.md.bak". Without the trailing-slash
# requirement, a rule would silently absorb every path that merely starts
# with its name.

repo=$(make_manifest_repo repo-prefix)
printf '# comment, ignored\n\n.sync-manifest\nshared.txt\nfile.txt\n' > "$repo/.sync-manifest"
printf 'a\n' > "$repo/file.txt"
printf 'b\n' > "$repo/file.txt.bak"
git -C "$repo" add .sync-manifest file.txt file.txt.bak
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "add lookalike path"
git -C "$repo" branch -f mac linux

output=$(run_check "$repo" mac linux 2>&1)
status=$?
assert_equals 'a lookalike path is not absorbed by prefix' '1' "$status"
assert_contains 'names the lookalike path' 'file.txt.bak' "$output"

# --- a bare path sharing a directory rule's name is not absorbed by it ----
#
# `PathPattern::parse("dir/")` strips the trailing slash before storing the
# path, so an unconditional exact-equality check would match the bare file
# "dir" too. The reference behavior compares against the unstripped rule
# "dir/", which never matches a path that isn't strictly beneath it.

repo=$(make_manifest_repo repo-dir-bare-name)
printf '# comment, ignored\n\n.sync-manifest\nshared.txt\ndir/\n' > "$repo/.sync-manifest"
git -C "$repo" add .sync-manifest
printf 'x\n' > "$repo/dir"
git -C "$repo" add dir
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "add dir rule and a bare file named dir"
git -C "$repo" branch -f mac linux

output=$(run_check "$repo" mac linux 2>&1)
status=$?
assert_equals 'a bare path sharing a directory rule name exits non-zero' '1' "$status"
assert_contains 'names the bare path' 'dir' "$output"
assert_contains 'reports it as unmatched' 'match no .sync-manifest rule' "$output"

finish
