#!/bin/bash
#
# Tests the suite filter in ~/tests/run-all.sh.
#
# `run-all.sh <suite>`, `run-in-docker.sh <suite>` and `config test --docker
# <suite>` all documented a suite name, and all three ran the whole suite:
# run-all.sh read only $1 == -q and dropped everything else. Asking for one
# suite and getting 28 is slow rather than wrong, so nothing failed and the
# gap stayed open.
#
# The runner is driven against a fixture tests directory rather than the real
# one. Pointing it at ~/tests would make this suite run every other suite,
# including itself.
#
# Usage: ~/tests/run-all-filter.test.sh

. "$(dirname "$0")/lib.sh"

RUN_ALL="$DOTFILES_ROOT/tests/run-all.sh"

# A fixture tests directory holding three trivial suites. Each prints a line
# naming itself, so the runner's output says exactly which ones ran.
fixture_tests="$FIXTURES/tests"
mkdir -p "$fixture_tests"
cp "$RUN_ALL" "$fixture_tests/run-all.sh"
chmod 755 "$fixture_tests/run-all.sh"

for name in alpha beta gamma; do
    printf '#!/bin/sh\nprintf "ran:%s\\n"\n' "$name" > "$fixture_tests/$name.test.sh"
    chmod 755 "$fixture_tests/$name.test.sh"
done

# A fixture root with no .claude, .scripts or crates, so the python and cargo
# discovery finds nothing and the run is the three suites above.
fixture_root="$FIXTURES/root"
mkdir -p "$fixture_root"

run_all() {
    DOTFILES_ROOT="$fixture_root" bash "$fixture_tests/run-all.sh" "$@" 2>&1
}

ran() {
    printf '%s' "$1" | grep -c "ran:$2" || true
}

# --- no argument runs everything --------------------------------------------

output=$(run_all)
status=$?
assert_equals 'a bare run runs every suite' '0' "$status"
assert_equals 'alpha ran' '1' "$(ran "$output" alpha)"
assert_equals 'beta ran' '1' "$(ran "$output" beta)"
assert_equals 'gamma ran' '1' "$(ran "$output" gamma)"

# --- a suite name runs only that suite --------------------------------------

output=$(run_all beta)
status=$?
assert_equals 'a named suite exits 0 when it passes' '0' "$status"
assert_equals 'the named suite ran' '1' "$(ran "$output" beta)"
assert_equals 'a suite that was not named did not run' '0' "$(ran "$output" alpha)"
assert_equals 'the other suite that was not named did not run' '0' "$(ran "$output" gamma)"

# The count in the [n/total] prefix has to describe the filtered run. Printing
# [1/3] while running one suite is the runner lying about what it is doing.
assert_contains 'the progress count describes the filtered run' '[1/1]' "$output"

# --- the name is accepted in the shapes a caller has --------------------------

# `config test <suite>` passes a bare name, and tab-completing a path gives
# the filename. Both name the same suite, so both must work rather than one
# silently running everything.
for spelling in beta beta.test.sh; do
    output=$(run_all "$spelling")
    assert_equals "the '$spelling' spelling runs the named suite" \
        '1' "$(ran "$output" beta)"
    assert_equals "the '$spelling' spelling runs nothing else" \
        '0' "$(ran "$output" alpha)"
done

# --- an unknown name fails loudly -------------------------------------------

# The failure this closes is a run that reports success having tested nothing.
# A typo must not do that.
output=$(run_all nosuchsuite)
status=$?
assert_equals 'an unknown suite name exits non-zero' '1' "$status"
assert_contains 'the failure names the suite that was asked for' \
    'nosuchsuite' "$output"
assert_equals 'an unknown suite name runs no suite' '0' "$(ran "$output" alpha)"

# A name that matches nothing must not be read as a prefix or a pattern. `bet`
# is not `beta`, and running beta for it would be a surprise in the direction
# that matters: the caller believes they tested something they did not name.
output=$(run_all bet)
status=$?
assert_equals 'a partial name exits non-zero rather than guessing' '1' "$status"
assert_equals 'a partial name does not run the suite it resembles' \
    '0' "$(ran "$output" beta)"

# --- the filter composes with -q ---------------------------------------------

# -q is documented as whole-suite-only by config test, which rejects the
# combination. run-all.sh is also called directly, and there the two are
# independent: quiet is about output, the filter is about which suites.
output=$(run_all -q beta)
status=$?
assert_equals 'a quiet filtered run exits 0' '0' "$status"
# -q suppresses a passing suite's own output, so the per-suite `ran:` line is
# gone by design. The progress line is what quiet mode still prints, and it
# names the suite, so that is what says which one ran.
assert_contains 'a quiet filtered run still runs the named suite' \
    'beta.test.sh' "$output"
assert_equals 'a quiet filtered run runs nothing else' '0' "$(ran "$output" alpha)"
assert_equals 'a quiet filtered run names no other suite' '' \
    "$(printf '%s' "$output" | grep -o 'alpha.test.sh' || true)"

# --- a filtered run does not claim a tool is missing --------------------------

# The python and cargo runs are skipped for a named suite on purpose. Saying
# "cargo not found" there would be false on a machine that has cargo, and a
# false skip message is how a real missing tool stops being noticed.
output=$(run_all beta)
assert_equals 'a filtered run does not report cargo as missing' '' \
    "$(printf '%s' "$output" | grep -F 'cargo not found' || true)"
assert_equals 'a filtered run does not report python3 as missing' '' \
    "$(printf '%s' "$output" | grep -F 'python3 not found' || true)"

# --- a failing named suite still fails ---------------------------------------

printf '#!/bin/sh\nprintf "ran:delta\\n"\nexit 1\n' > "$fixture_tests/delta.test.sh"
chmod 755 "$fixture_tests/delta.test.sh"

output=$(run_all delta)
status=$?
assert_equals 'a failing named suite exits non-zero' '1' "$status"
assert_contains 'the summary names the failing suite' 'delta' "$output"

# --- the cargo run covers the whole workspace, not one crate ----------------

# run-all.sh used to point --manifest-path at crates/config-manifest/Cargo.toml,
# which runs only that crate's tests. The workspace has had a second member
# since dotfiles-path landed, so 12 tests were silently outside the suite, and
# every crate added later would be too. Measured at the time: the per-crate
# command ran 4 test binaries where the workspace ran 6.
#
# Asserted against the real run-all.sh rather than the fixture copy, because
# the defect is in which path the script names, not in how it filters.

# Asserted by RUNNING cargo the way run-all.sh runs it, not by grepping the
# script's text. A text assertion here matched this comment's own explanation
# and reported a defect that was already fixed, which is the unanchored-pattern
# trap the plan records twice already.
# The cargo step must name the workspace root, not one member. run-all.sh used
# to point at crates/config-manifest/Cargo.toml, so `cargo test` ran that crate
# alone: dotfiles-path's tests were outside the suite from the day it landed,
# and every crate added later would have been too. Measured at the time of the
# fix: 4 test binaries against the per-crate manifest, 6 against the workspace.
#
# Asserted on the literal path this script names, which is the one thing the
# defect actually was. Three richer versions of this assertion were tried and
# discarded, each recorded because each failed in a way worth not repeating:
# grepping for "manifest-path" matched the fix's own explanatory comment;
# measuring cargo inside crates/ passed against the broken script because it
# tested cargo rather than the script; and eval-ing the extracted assignment
# resolved DOTFILES_ROOT to empty, so the path did not exist and the guarded
# assertion silently skipped while reporting a pass.

# Python test discovery must skip ephemeral scratch. `.claude/jobs/<id>/` is
# a background job's workspace: it holds third-party checkouts a job cloned to
# read, and their test suites are neither this repo's code nor this repo's
# responsibility. Six such suites reported red during this work and misled two
# separate readers into checking whether they had broken something.
#
# `*/plugins/*` was already excluded for the same reason. This adds the other
# ephemeral tree.

discovery_line=$(grep -n "name 'test_\*\.py'" -A2 "$RUN_ALL" | tr -d '\n' || true)

# Positive control: the find invocation must be locatable, or the assertion
# below compares two empty strings.
assert_succeeds 'run-all.sh has a python discovery find' test -n "$discovery_line"

assert_succeeds 'python discovery excludes ephemeral job workspaces' \
    sh -c 'printf "%s" "$1" | grep -q "jobs"' _ "$discovery_line"

manifest_line=$(grep -E '^cargo_manifest=' "$RUN_ALL" || true)

# Positive control: the assignment must exist, or the assertion below is
# comparing two empty strings and proves nothing.
assert_succeeds 'run-all.sh assigns a cargo manifest path' test -n "$manifest_line"

assert_equals 'the cargo manifest is the workspace root, not one crate' \
    'cargo_manifest="$cargo_dir/Cargo.toml"' "$manifest_line"

finish
