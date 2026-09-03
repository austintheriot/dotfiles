#!/bin/bash
#
# Runs every test in the dotfiles repo: the integration suites in this
# directory, and the unit tests that live next to the code they cover.
#
# Usage:
#   ~/tests/run-all.sh          run everything
#   ~/tests/run-all.sh -q       print only failures and the summary
#
# Exits non-zero when anything fails, so a pre-commit hook can gate on it.
#
# Each suite is announced as [n/total] before it starts. A suite's own output is
# captured rather than streamed, so without that line a caller sees nothing for
# the whole run and cannot tell a slow suite from a hung one.
#
# Suites run one at a time on purpose. Running them in parallel was measured at
# 26s against 22s serial: every suite drives the same single-threaded tmux
# server, so concurrency buys nothing and costs process overhead.

set -u

TESTS_DIR=$(cd "$(dirname "$0")" && pwd)
DOTFILES_ROOT=${DOTFILES_ROOT:-$HOME}
export DOTFILES_ROOT

quiet=0
[ "${1:-}" = '-q' ] && quiet=1

suites_run=0
suites_failed=0
failed_names=()

report() {
    [ "$quiet" -eq 1 ] || printf '%s\n' "$1"
}

run_suite() {
    local name=$1
    shift
    local output status started elapsed
    suites_run=$((suites_run + 1))

    # Printed before the suite starts, without a newline, so the name is on
    # screen for however long the suite takes. The verdict completes the line.
    printf '[%d/%d] %s ... ' "$suites_run" "$total_suites" "$name"

    started=$SECONDS
    output=$("$@" 2>&1)
    status=$?
    elapsed=$((SECONDS - started))

    if [ "$status" -eq 0 ]; then
        printf 'PASS (%ds)\n' "$elapsed"
        [ "$quiet" -eq 1 ] || printf '%s\n' "$output" | sed 's/^/      /'
    else
        suites_failed=$((suites_failed + 1))
        failed_names+=("$name")
        printf 'FAIL (%ds)\n' "$elapsed"
        printf '%s\n' "$output" | sed 's/^/      /'
    fi
    report ''
}

# --- what is going to run ----------------------------------------------
#
# Both sets are discovered up front, because [n/total] needs the total before
# the first suite starts.

integration_count=0
for suite in "$TESTS_DIR"/*.test.sh; do
    [ -e "$suite" ] || continue
    integration_count=$((integration_count + 1))
done

python_dirs=''
if command -v python3 >/dev/null 2>&1; then
    python_dirs=$(find "$DOTFILES_ROOT/.claude" "$DOTFILES_ROOT/.my-scripts" \
                     -name 'test_*.py' -type f -not -path '*/plugins/*' 2>/dev/null \
                  | xargs -n1 dirname 2>/dev/null | sort -u)
fi

python_count=0
if [ -n "$python_dirs" ]; then
    python_count=$(printf '%s\n' "$python_dirs" | wc -l | tr -d ' ')
fi

total_suites=$((integration_count + python_count))

# --- integration suites ------------------------------------------------

for suite in "$TESTS_DIR"/*.test.sh; do
    [ -e "$suite" ] || continue
    run_suite "$(basename "$suite")" "$suite"
done

# --- unit tests, discovered next to the code they cover ----------------

if command -v python3 >/dev/null 2>&1; then
    while IFS= read -r directory; do
        [ -n "$directory" ] || continue
        run_suite "python unit tests in ${directory#$DOTFILES_ROOT/}" \
            python3 -m unittest discover -s "$directory" -p 'test_*.py'
    done <<< "$python_dirs"
else
    printf 'SKIP  python unit tests (python3 not found)\n'
fi

# --- summary -----------------------------------------------------------

printf '%s\n' '----------------------------------------'
if [ "$suites_failed" -eq 0 ]; then
    printf 'all %d suite(s) passed\n' "$suites_run"
    exit 0
fi

printf '%d of %d suite(s) failed:\n' "$suites_failed" "$suites_run"
for name in "${failed_names[@]}"; do
    printf '  - %s\n' "$name"
done
exit 1
