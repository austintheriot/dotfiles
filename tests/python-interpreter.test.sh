#!/bin/bash
#
# Tests that the suite runs python through the real interpreter, not the
# pyenv shim.
#
# The `python3` on PATH is a pyenv shim: a bash script that execs
# `pyenv exec python3`, which re-resolves the version and execs again.
# Measured here at 750ms per call against 40ms for the interpreter the shim
# eventually reaches. The suite starts python five times across run-all.sh,
# deps-harness and workflow-labels, so it pays roughly 3.5 seconds to arrive
# at the same interpreter -- and run-all.sh gates a pre-commit hook.
#
# lib.sh resolves it once and exports PYTHON_BIN. Every suite sources lib.sh,
# so one resolution serves all of them, and a suite that needs python asks for
# the variable rather than spelling `python3` and paying the shim again.
#
# The resolution has to degrade rather than fail. `pyenv which` is itself a
# shim call, so it is only paid when pyenv is actually in use, and a machine
# with no pyenv keeps whatever `python3` means there. /usr/bin/python3 is
# specifically not a fallback: on macOS it is an xcrun stub that does not run
# without the Xcode command line tools installed.
#
# Usage: ~/tests/python-interpreter.test.sh

. "$(dirname "$0")/lib.sh"

# --- lib.sh exports a resolved interpreter ----------------------------------

assert_succeeds 'lib.sh exports PYTHON_BIN' test -n "${PYTHON_BIN:-}"

if [ -z "${PYTHON_BIN:-}" ]; then
    finish
    exit 1
fi

assert_succeeds 'PYTHON_BIN runs' "$PYTHON_BIN" -c 'pass'

version=$("$PYTHON_BIN" -c 'import sys; print(sys.version_info[0])' 2>/dev/null)
assert_equals 'PYTHON_BIN is python 3' '3' "$version"

# --- it is not the shim -----------------------------------------------------

# Only assertable where a shim is actually in play. A machine without pyenv
# has nothing to avoid, and asserting there would fail for the wrong reason.
if [ -d "$HOME/.pyenv/shims" ] && command -v python3 >/dev/null 2>&1 \
    && [ "$(command -v python3)" = "$HOME/.pyenv/shims/python3" ]; then

    assert_equals 'PYTHON_BIN is not the pyenv shim' '' \
        "$(printf '%s' "$PYTHON_BIN" | grep -F '/.pyenv/shims/' || true)"

    # The shim is a text script; the real interpreter is a binary. That is the
    # difference that costs 700ms, so assert it directly rather than trusting
    # the path to look right.
    assert_equals 'PYTHON_BIN is a binary, not a shell script' '' \
        "$(head -c 2 "$PYTHON_BIN" 2>/dev/null | grep -F '#!' || true)"

    # The whole point is speed. A generous bound: the shim measured 750ms and
    # the interpreter 40ms, so anything under a quarter second is unambiguous
    # about which one this is, without being tight enough to flake on a busy
    # machine.
    started=$(date +%s%N 2>/dev/null || echo '')
    if [ -n "$started" ]; then
        "$PYTHON_BIN" -c 'pass' 2>/dev/null
        elapsed=$(( ($(date +%s%N) - started) / 1000000 ))
        [ "$elapsed" -lt 250 ] && fast=yes || fast="no (${elapsed}ms)"
        assert_equals 'PYTHON_BIN starts without the shim penalty' 'yes' "$fast"
    fi
else
    printf 'ok: no pyenv shim on this machine, nothing to avoid\n'
fi

# --- the suites use it ------------------------------------------------------

# A suite that spells `python3` directly is back on the shim, so the
# resolution has to be used rather than merely available.
bare=''
for suite in "$DOTFILES_ROOT"/tests/*.test.sh; do
    name=$(basename "$suite")
    # This file names the shim in its prose and its assertions.
    [ "$name" = 'python-interpreter.test.sh' ] && continue
    # run-all-filter asserts on the runner's "python3 not found" message.
    [ "$name" = 'run-all-filter.test.sh' ] && continue
    # These read what python3 resolves to; that is the subject, not a call.
    [ "$name" = 'zshrc-python-startup.test.sh' ] && continue
    [ "$name" = 'container.test.sh' ] && continue
    grep -qE '^[[:space:]]*("?\$?\{?)?python3?[}"]?[[:space:]]+-' "$suite" \
        && bare="$bare $name"
done
assert_equals 'no suite starts python by the bare name' '' "$bare"

# --- run-all.sh uses it too --------------------------------------------------

# run-all.sh does not source lib.sh, so it resolves the interpreter the same
# way rather than inheriting the variable.
RUN_ALL="$DOTFILES_ROOT/tests/run-all.sh"
assert_succeeds 'run-all.sh resolves an interpreter rather than calling python3' \
    grep -q 'PYTHON_BIN' "$RUN_ALL"

assert_equals 'run-all.sh does not run the unittest discover through python3' '' \
    "$(grep -nE '^[[:space:]]*python3[[:space:]]+-m[[:space:]]+unittest' "$RUN_ALL" || true)"

finish
