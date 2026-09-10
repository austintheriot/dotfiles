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

# --- assert_succeeds reports the real exit code -------------------------
#
# failed=$((failed + 1)) ran before the printf read $?, so the arithmetic's
# own exit status (always 0) was what got printed. Every failure across
# 170+ call sites reported "exited 0", losing the one diagnostic a
# container-only failure depends on.

rc_suite="$FIXTURES/rc.test.sh"
write_suite "$rc_suite" "assert_succeeds 'a command that exits 42' sh -c 'exit 42'"

output=$(bash "$rc_suite" 2>&1 || true)
assert_contains 'assert_succeeds reports the real exit code' 'exited 42' "$output"

# --- verdict_for is a pure function of three counts ----------------------
#
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
assert_contains 'the verdict says no assertions ran' 'no assertions ran' "$output"

# An assertion inside a command substitution must still count. This is the
# latent bug the tally file fixes: a subshell cannot increment its parent's
# variable, so these vanished silently.
sub_suite="$FIXTURES/subshell.test.sh"
write_suite "$sub_suite" 'result=$(assert_equals "inside a substitution" a a)'
output=$(bash "$sub_suite" 2>&1)
assert_contains 'a subshell assertion reaches the tally' '1 passed' "$output"

# --- the suite's own output colours by severity --------------------------
#
# Same rule as the deps engine: red for a failure, yellow for a skip, green
# for a pass, and NOTHING when the output is not a terminal. The words are
# unchanged, so colour is redundant emphasis rather than the only channel.
#
# THE PIPED CASE IS THE CONTRACT, not a fallback. container.test.sh matches
# this suite's assertion text and run-all.sh parses the summary line with
# sed, so an escape here breaks the gates and breaks them only where nobody
# is watching a terminal.
LIB="$DOTFILES_ROOT/tests/lib.sh"
lib_code=$(sed -e 's/#.*//' "$LIB")
assert_succeeds 'the lib body was read' test -n "$lib_code"

# The decision must be made once, from a tty check plus NO_COLOR, rather
# than per printf.
assert_succeeds 'lib.sh decides colour from a tty' \
    test -n "$(printf '%s\n' "$lib_code" | grep -E '\-t 1' || true)"
assert_succeeds 'lib.sh honours NO_COLOR' \
    test -n "$(printf '%s\n' "$lib_code" | grep -E 'NO_COLOR' || true)"

# And the assertion output must actually be plain through a pipe. Asserted on
# a real run rather than on lib.sh's text, because the text could be right
# while the expansion is wrong.
#
# A DIFFERENT suite is run, not this one: invoking itself recursed until the
# harness timed out, which is a mistake worth leaving recorded rather than
# silently fixing. skip-assertion.test.sh is small, has no side effects, and
# exercises the same reporters.
piped_assertions=$("$DOTFILES_ROOT/tests/nvim-lua-format.test.sh" 2>&1 || true)
assert_succeeds 'a piped suite run produced output' test -n "$piped_assertions"
assert_equals 'piped assertion output carries no ANSI escape' '' \
    "$(printf '%s' "$piped_assertions" | grep -c "$(printf '\033')" | grep -v '^0$' || true)"

finish
