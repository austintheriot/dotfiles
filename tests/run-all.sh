#!/bin/bash
#
# Runs every test in the dotfiles repo: the integration suites in this
# directory, and the unit tests that live next to the code they cover.
#
# Usage:
#   ~/tests/run-all.sh              run everything
#   ~/tests/run-all.sh -q           print only failures and the summary
#   ~/tests/run-all.sh <suite>      run one integration suite, by name
#
# A suite name is matched exactly, with or without the .test.sh suffix. A name
# that matches nothing is an error rather than a run of zero suites: the
# failure worth preventing is a green report from a run that tested nothing,
# and a typo is how that happens.
#
# Naming a suite runs the integration suites only. The python and cargo runs
# are whole-suite concerns with their own runners; `cargo test <filter>` and
# `python3 -m unittest <name>` already exist for a narrower run there.
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

# The suite calls config-manifest by name. config-build installs it to
# ~/.local/bin, which not every invoking shell has on PATH.
case ":$PATH:" in
    *":$HOME/.local/bin:"*) ;;
    *) PATH="$HOME/.local/bin:$PATH" ;;
esac

quiet=0
only=''
for arg in "$@"; do
    case $arg in
        -q) quiet=1 ;;
        -*)
            printf 'run-all: unknown option %s\n' "$arg" >&2
            printf 'usage: run-all.sh [-q] [suite]\n' >&2
            exit 2
            ;;
        *)
            if [ -n "$only" ]; then
                printf 'run-all: name at most one suite (got %s and %s)\n' \
                    "$only" "$arg" >&2
                exit 2
            fi
            # Accepted with or without the suffix: `config test <suite>` passes
            # a bare name, and tab-completing a path gives the filename.
            only=${arg%.test.sh}
            ;;
    esac
done

if [ -n "$only" ] && [ ! -e "$TESTS_DIR/$only.test.sh" ]; then
    printf 'run-all: no suite named %s in %s\n' "$only" "$TESTS_DIR" >&2
    printf 'run-all: available suites:\n' >&2
    for suite in "$TESTS_DIR"/*.test.sh; do
        [ -e "$suite" ] || continue
        printf '  %s\n' "$(basename "$suite" .test.sh)" >&2
    done
    exit 1
fi

# Every discovery loop below asks this rather than testing $only itself, so
# the filter is applied in one place and the [n/total] count cannot disagree
# with what actually runs.
selected() {
    [ -z "$only" ] || [ "$1" = "$only.test.sh" ]
}

suites_run=0
suites_failed=0
skips_total=0
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

    # Read back out of the captured output rather than passed up a variable:
    # each suite runs as its own process, so there is no other channel. The
    # count comes from the summary line `finish` prints, which is the one
    # place a suite states its own totals.
    #
    # This is the line a developer reads. A skipped assertion that only ever
    # appeared in the indented body -- suppressed entirely under -q -- is
    # exactly how the commit-inspection block in scripts-dir-name.test.sh went
    # unnoticed until CI failed on it.
    local skips
    skips=$(printf '%s\n' "$output" \
        | sed -n 's/^[^:]*: [0-9]* passed, [0-9]* failed, \([0-9]*\) skipped$/\1/p' \
        | tail -n1)
    if [ -n "$skips" ] && [ "$skips" -gt 0 ] 2>/dev/null; then
        skips_total=$((skips_total + skips))
    else
        skips=''
    fi

    if [ "$status" -eq 0 ]; then
        if [ -n "$skips" ]; then
            printf 'PASS (%ds, %s skipped)\n' "$elapsed" "$skips"
        else
            printf 'PASS (%ds)\n' "$elapsed"
        fi
        [ "$quiet" -eq 1 ] || printf '%s\n' "$output" | sed 's/^/      /'
    else
        suites_failed=$((suites_failed + 1))
        failed_names+=("$name")
        if [ -n "$skips" ]; then
            printf 'FAIL (%ds, %s skipped)\n' "$elapsed" "$skips"
        else
            printf 'FAIL (%ds)\n' "$elapsed"
        fi
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
    selected "$(basename "$suite")" || continue
    integration_count=$((integration_count + 1))
done

# The real interpreter, not the pyenv shim. lib.sh resolves this the same way
# for the suites; run-all.sh does not source lib.sh, so it repeats the four
# lines rather than sourcing a harness it otherwise has no use for.
#
# The shim is a bash script that execs `pyenv exec python3`, measured at 750ms
# per start against 40ms for the interpreter it reaches. This file gates a
# pre-commit hook.
PYTHON_BIN=''
if command -v pyenv >/dev/null 2>&1; then
    PYTHON_BIN=$(pyenv which python3 2>/dev/null)
    [ -n "$PYTHON_BIN" ] && [ -x "$PYTHON_BIN" ] || PYTHON_BIN=''
fi
[ -n "$PYTHON_BIN" ] || PYTHON_BIN=$(command -v python3 2>/dev/null || true)
export PYTHON_BIN

python_dirs=''
if [ -z "$only" ] && [ -n "$PYTHON_BIN" ]; then
    python_dirs=$(find "$DOTFILES_ROOT/.claude" "$DOTFILES_ROOT/.scripts" \
                     -name 'test_*.py' -type f -not -path '*/plugins/*' \
                     -exec dirname {} + 2>/dev/null | sort -u)
fi

python_count=0
if [ -n "$python_dirs" ]; then
    python_count=$(printf '%s\n' "$python_dirs" | wc -l | tr -d ' ')
fi

cargo_manifest="$DOTFILES_ROOT/crates/config-manifest/Cargo.toml"
cargo_count=0
if [ -z "$only" ] && command -v cargo >/dev/null 2>&1 && [ -f "$cargo_manifest" ]; then
    cargo_count=1
fi

total_suites=$((integration_count + python_count + cargo_count))

# --- integration suites ------------------------------------------------

for suite in "$TESTS_DIR"/*.test.sh; do
    [ -e "$suite" ] || continue
    selected "$(basename "$suite")" || continue
    run_suite "$(basename "$suite")" "$suite"
done

# --- unit tests, discovered next to the code they cover ----------------

if [ -n "$only" ]; then
    : # as above: not a skip for want of python3, but a narrower run.
elif [ -n "$PYTHON_BIN" ]; then
    while IFS= read -r directory; do
        [ -n "$directory" ] || continue
        run_suite "python unit tests in ${directory#"$DOTFILES_ROOT"/}" \
            "$PYTHON_BIN" -m unittest discover -s "$directory" -p 'test_*.py'
    done <<< "$python_dirs"
else
    printf 'SKIP  python unit tests (python3 not found)\n'
fi

# --- Rust unit and integration tests --------------------------------------
#
# Skipped, and said so, where cargo is absent: the Docker runtime image is
# Rust-free by design and gets the binary from a builder stage instead.

if [ "$cargo_count" -eq 1 ]; then
    export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/config-manifest/target}"
    run_suite "cargo test in crates/config-manifest" \
        cargo test --locked --quiet --manifest-path "$cargo_manifest"
elif [ -n "$only" ]; then
    : # a named suite is an integration suite; saying "cargo not found" here
      # would be false, and cargo has its own filter.
else
    printf 'SKIP  cargo test (cargo not found)\n'
fi

# --- summary -----------------------------------------------------------

printf '%s\n' '----------------------------------------'

# Appended only when non-zero, for the reason `finish` gives: a clean run has
# to stay clean, or the phrase stops carrying information.
skip_note=''
if [ "$skips_total" -gt 0 ]; then
    skip_note=$(printf ' (%d assertion(s) skipped)' "$skips_total")
fi

if [ "$suites_failed" -eq 0 ]; then
    printf 'all %d suite(s) passed%s\n' "$suites_run" "$skip_note"
    exit 0
fi

printf '%d of %d suite(s) failed%s:\n' "$suites_failed" "$suites_run" "$skip_note"
for name in "${failed_names[@]}"; do
    printf '  - %s\n' "$name"
done
exit 1
