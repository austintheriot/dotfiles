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
    selected "$(basename "$suite")" || continue
    integration_count=$((integration_count + 1))
done

python_dirs=''
if [ -z "$only" ] && command -v python3 >/dev/null 2>&1; then
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
elif command -v python3 >/dev/null 2>&1; then
    while IFS= read -r directory; do
        [ -n "$directory" ] || continue
        run_suite "python unit tests in ${directory#"$DOTFILES_ROOT"/}" \
            python3 -m unittest discover -s "$directory" -p 'test_*.py'
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
if [ "$suites_failed" -eq 0 ]; then
    printf 'all %d suite(s) passed\n' "$suites_run"
    exit 0
fi

printf '%d of %d suite(s) failed:\n' "$suites_failed" "$suites_run"
for name in "${failed_names[@]}"; do
    printf '  - %s\n' "$name"
done
exit 1
