#!/bin/bash
#
# Guards the visibility of a skipped assertion.
#
# A suite may legitimately decline to run part of itself: the git-dependent
# block in scripts-dir-name.test.sh needs a repository, and the container
# harness builds its tree with `git archive`, so there is none. That skip is
# correct. What was wrong is that it cost nothing to ignore.
#
# The skip printed one line into a suite's captured output, `finish` counted
# only passes and failures, and run-all.sh printed PASS. The pre-push Docker
# gate therefore reported a clean run while the commit-count assertion in
# scripts-dir-name.test.sh had not executed at all. Adding config-help moved
# that count from 24 to 25, the local gate passed, and both CI platforms
# failed instead (runs 33934561395 and 33934578084, 2026-09-05). CI caught it
# only because actions/checkout gives the runner a repository.
#
# The pre-push hook already states the rule this suite enforces one level
# down: "A hook that says nothing when it skips is indistinguishable from a
# hook that is not installed at all." The same is true of a test.
#
# So a skip stays permitted and stays green. It just has to be counted and
# said, on the line a developer reads, at every level that summarizes.
#
# Driven against fixture suites rather than the real ones, for the reason
# run-all-filter.test.sh gives: pointing the runner at ~/tests would make
# this suite run every other suite, including itself.
#
# Usage: ~/tests/skip-reporting.test.sh

. "$(dirname "$0")/lib.sh"

LIB="$DOTFILES_ROOT/tests/lib.sh"
RUN_ALL="$DOTFILES_ROOT/tests/run-all.sh"

# --- the harness counts a skip ------------------------------------------
#
# A fixture suite that sources the real lib.sh, so these assertions are about
# the harness under test and not a reimplementation of it.

write_suite() {
    local path=$1 body=$2
    {
        printf '#!/bin/bash\n'
        printf '. "%s"\n' "$LIB"
        printf '%s\n' "$body"
        printf 'finish\n'
    } > "$path"
    chmod 755 "$path"
}

only_skips="$FIXTURES/only-skips.test.sh"
write_suite "$only_skips" \
    "skip 'no repository here, so the commit cannot be inspected'"

output=$(bash "$only_skips" 2>&1)
status=$?

# Green, still. A skip is a statement that an assertion did not run, not a
# claim that it would have failed. Turning these red would push the container
# harness to fake a repository just to get a passing gate, which trades a
# quiet gap for a lying one.
assert_equals 'a skipped assertion does not fail the suite' '0' "$status"

assert_contains 'the skip names its reason' 'no repository here' "$output"
assert_contains 'the summary counts the skip' '1 skipped' "$output"

# --- a clean suite says nothing about skips -----------------------------
#
# The count has to be absent, not zero. "0 skipped" on every one of 30 suites
# is noise, and noise is what the eye learns to skip past -- which is the
# failure this whole suite exists to prevent.

no_skips="$FIXTURES/no-skips.test.sh"
write_suite "$no_skips" "assert_equals 'a real assertion' 'x' 'x'"

output=$(bash "$no_skips" 2>&1)
assert_equals 'a suite with no skips exits 0' '0' "$?"
assert_equals 'a suite with no skips does not mention skipping' '' \
    "$(printf '%s' "$output" | grep -o 'skipped' || true)"

# --- skips and real assertions coexist ----------------------------------

mixed="$FIXTURES/mixed.test.sh"
write_suite "$mixed" "$(printf '%s\n' \
    "assert_equals 'first' 'x' 'x'" \
    "skip 'the second needs a repository'" \
    "assert_equals 'third' 'y' 'y'")"

output=$(bash "$mixed" 2>&1)
assert_equals 'a mixed suite exits 0' '0' "$?"
assert_contains 'the passes are still counted' '2 passed' "$output"
assert_contains 'the skips are counted beside them' '1 skipped' "$output"

# Two skips are two, not one. A count that saturates would hide the second
# block going quiet in a suite that already skips one.
two_skips="$FIXTURES/two-skips.test.sh"
write_suite "$two_skips" "$(printf '%s\n' \
    "skip 'the first reason'" \
    "skip 'the second reason'")"

output=$(bash "$two_skips" 2>&1)
assert_contains 'each skip is counted' '2 skipped' "$output"

# --- a failing suite still reports its failure --------------------------
#
# The skip count must not displace the verdict. A suite that both skips and
# fails is failing.

skip_and_fail="$FIXTURES/skip-and-fail.test.sh"
write_suite "$skip_and_fail" "$(printf '%s\n' \
    "skip 'a reason'" \
    "assert_equals 'a real failure' 'x' 'y'")"

output=$(bash "$skip_and_fail" 2>&1)
status=$?
assert_equals 'a suite that skips and fails still fails' '1' "$status"
assert_contains 'the failure is still counted' '1 failed' "$output"
assert_contains 'the skip is counted too' '1 skipped' "$output"

# --- the runner surfaces the skip on the verdict line -------------------
#
# This is the line the developer actually reads. A suite's own output is
# indented under it and is exactly what nobody scans on a green run, which is
# where the original skip line was living.

fixture_tests="$FIXTURES/tests"
mkdir -p "$fixture_tests"
cp "$RUN_ALL" "$fixture_tests/run-all.sh"
chmod 755 "$fixture_tests/run-all.sh"
write_suite "$fixture_tests/alpha.test.sh" \
    "skip 'alpha needs a repository'"
write_suite "$fixture_tests/beta.test.sh" \
    "assert_equals 'beta is clean' 'x' 'x'"

fixture_root="$FIXTURES/root"
mkdir -p "$fixture_root"

run_all() {
    DOTFILES_ROOT="$fixture_root" bash "$fixture_tests/run-all.sh" "$@" 2>&1
}

output=$(run_all)
status=$?
assert_equals 'a run containing a skip still exits 0' '0' "$status"

alpha_line=$(printf '%s\n' "$output" | grep -F 'alpha.test.sh')
assert_contains 'the skipping suite is still a pass' 'PASS' "$alpha_line"
assert_contains 'the verdict line says it skipped' 'skipped' "$alpha_line"

beta_line=$(printf '%s\n' "$output" | grep -F 'beta.test.sh')
assert_equals 'a clean suite verdict says nothing about skips' '' \
    "$(printf '%s' "$beta_line" | grep -o 'skipped' || true)"

# --- the run summary surfaces it too ------------------------------------
#
# -q exists so a passing run prints only verdict lines and the summary. A
# developer running the gate quietly must still be told that part of it did
# not execute, so the count belongs in the summary and not only per-suite.

output=$(run_all -q)
assert_contains 'a quiet run still reports the skip' 'skipped' "$output"

# "assertion(s) skipped" rather than a bare count: at this level the reader is
# looking at a list of suites, and "1 skipped" there would read as one suite
# having been skipped. run-all.sh already prints `SKIP` lines for a whole
# runner it declined to start (python, cargo), so the two must not collide.
summary=$(printf '%s\n' "$output" | grep -F 'suite(s) passed')
assert_contains 'the summary counts the skipped assertions' \
    '1 assertion(s) skipped' "$summary"

# The all-clear must stay clean. If every green run ends in "0 skipped", the
# phrase stops carrying information.
output=$(run_all -q beta)
summary=$(printf '%s\n' "$output" | grep -F 'suite(s) passed')
assert_equals 'a summary with no skips does not mention them' '' \
    "$(printf '%s' "$summary" | grep -o 'skipped' || true)"

finish
