#!/bin/bash
#
# Integration tests for the lazy pyenv setup in .zshrc.
#
# `eval "$(pyenv init - zsh)"` costs 380ms at every shell startup, measured on
# this machine. Two things inside it account for nearly all of that: a
# `bash --norc` spawned only to remove the shims directory from PATH before
# putting it back, and a `pyenv rehash` subprocess at 250ms on its own.
#
# Neither is needed to make python3 resolve. `pyenv init` ends with the shims
# directory on PATH and PYENV_SHELL set; putting the shims on PATH directly
# reaches the same end state for free. rehash only regenerates the shim files,
# which change when a version or a package with an entry point is installed --
# a `pyenv install` concern, not a per-shell one.
#
# The contract these tests pin down, mirroring the nvm suite:
#   1. startup runs no pyenv subprocess, so it stays fast
#   2. python3 and pip still resolve through the shims
#   3. `pyenv` is available but not yet loaded
#   4. calling `pyenv` loads the real thing and then behaves normally
#
# Point 4 is the one worth testing rather than assuming. A shim that loads the
# real thing and forgets to re-dispatch swallows its first call silently, and
# every later call then works -- the shape that passes a careless test. Both
# the first call and the second are asserted here for that reason.
#
# This suite is skipped where pyenv is not installed, the same way the nvm
# suite skips a machine with no v24.
#
# Usage: ~/tests/zshrc-python-startup.test.sh

. "$(dirname "$0")/lib.sh"

ZSHRC="$DOTFILES_ROOT/.zshrc"
PYENV_DIR="$HOME/.pyenv"

if [ "$HOME" != "$DOTFILES_ROOT" ] || [ ! -f "$ZSHRC" ]; then
    skip 'HOME is not the repo: startup does not eval `pyenv init`'
    skip 'HOME is not the repo: startup does not run `pyenv rehash`'
    skip 'HOME is not the repo: python3 resolves through the pyenv shims'
    skip 'HOME is not the repo: pip3 resolves through the pyenv shims'
    skip 'HOME is not the repo: PYENV_SHELL is exported at startup'
    skip 'HOME is not the repo: PYENV_ROOT is exported at startup'
    skip 'HOME is not the repo: pyenv is defined but not yet loaded'
    skip 'HOME is not the repo: the expected version was resolved'
    skip 'HOME is not the repo: the first pyenv call returns the right version'
    skip 'HOME is not the repo: pyenv is loaded for real after the first call'
    skip 'HOME is not the repo: a second pyenv call still works'
    skip 'HOME is not the repo: the shims directory appears on PATH exactly once after loading'
    finish
    exit 0
fi
if [ ! -d "$PYENV_DIR" ]; then
    skip "no pyenv at $PYENV_DIR: startup does not eval \`pyenv init\`"
    skip "no pyenv at $PYENV_DIR: startup does not run \`pyenv rehash\`"
    skip "no pyenv at $PYENV_DIR: python3 resolves through the pyenv shims"
    skip "no pyenv at $PYENV_DIR: pip3 resolves through the pyenv shims"
    skip "no pyenv at $PYENV_DIR: PYENV_SHELL is exported at startup"
    skip "no pyenv at $PYENV_DIR: PYENV_ROOT is exported at startup"
    skip "no pyenv at $PYENV_DIR: pyenv is defined but not yet loaded"
    skip "no pyenv at $PYENV_DIR: the expected version was resolved"
    skip "no pyenv at $PYENV_DIR: the first pyenv call returns the right version"
    skip "no pyenv at $PYENV_DIR: pyenv is loaded for real after the first call"
    skip "no pyenv at $PYENV_DIR: a second pyenv call still works"
    skip "no pyenv at $PYENV_DIR: the shims directory appears on PATH exactly once after loading"
    finish
    exit 0
fi

# --- startup pays nothing ---------------------------------------------------

# Asserted on the source text, the same way the nvm suite does: an
# unconditional `pyenv init` at top level is the shape being removed, and it
# is visible without starting a shell.
assert_equals 'startup does not eval `pyenv init`' \
    '' "$(grep -nE '^[[:space:]]*eval[[:space:]]+"\$\(pyenv init' "$ZSHRC")"

assert_equals 'startup does not run `pyenv rehash`' \
    '' "$(grep -nE '^[[:space:]]*(command[[:space:]]+)?pyenv[[:space:]]+rehash' "$ZSHRC")"

# --- the end state is reached anyway ----------------------------------------

# A BASELINE PATH the caller cannot pollute, and this is a fix for a real
# defect rather than hygiene.
#
# `zsh -i` inherits the caller's PATH, so this suite measured the
# ENVIRONMENT IT RAN IN rather than what .zshrc produces. Run from a shell
# whose PATH already had Homebrew ahead of the pyenv shims (Claude Code's own
# environment is one), the two resolution assertions below failed while a
# real login shell on the same machine resolved python3 to the shims
# correctly. Same .zshrc, two answers, decided by the caller.
#
# The guard in .zshrc is what makes the inherited PATH matter: it skips the
# prepend when the shims are on PATH ANYWHERE, so an inherited PATH that
# already carries them further back leaves them there, behind whatever came
# first.
#
# /usr/bin:/bin:/usr/sbin:/sbin is what `path_helper` starts a login shell
# with, so this is the state .zshrc is actually written against. Nothing here
# adds pyenv or Homebrew: the assertions below are about what .zshrc does
# with PATH, and pre-seeding either one would answer the question for it.
#
# USED ONLY FOR THE RESOLUTION BLOCK BELOW, not for the lazy-loading
# assertions further down. Those call the real `pyenv`, which on this machine
# is a Homebrew install at /opt/homebrew/bin/pyenv, so a baseline that
# excludes Homebrew makes them fail with "command not found: pyenv" -- a fact
# about the baseline rather than about the shim. They keep the caller's PATH
# because what they test (does the shim load the real thing and re-dispatch)
# does not depend on PATH ORDER at all.
STARTUP_PATH='/usr/bin:/bin:/usr/sbin:/sbin'

report=$(PATH="$STARTUP_PATH" zsh -i -c '
    print -- "--python3--"; command -v python3
    print -- "--pip3--";    command -v pip3
    print -- "--shell--";   print -r -- "${PYENV_SHELL:-unset}"
    print -- "--root--";    print -r -- "${PYENV_ROOT:-unset}"
    print -- "--pyenv-type--"
    # `pyenv` must exist but must NOT be loaded yet. The real init defines a
    # `pyenv` function too, so size is not the tell. The lazy shim has no
    # completion loaded; the real init sources one, which defines _pyenv.
    if typeset -f pyenv >/dev/null 2>&1; then
        if (( $+functions[_pyenv] )); then
            print "eager"
        else
            print "lazy"
        fi
    else
        print "missing"
    fi
    print -- "--end--"
' 2>/dev/null)

field() { printf '%s' "$report" | sed -n "/^--$1--\$/,/^--/p" | sed '1d;/^--/d' | head -1; }

assert_equals 'python3 resolves through the pyenv shims' \
    "$PYENV_DIR/shims/python3" "$(field python3)"

assert_equals 'pip3 resolves through the pyenv shims' \
    "$PYENV_DIR/shims/pip3" "$(field pip3)"

# `pyenv init` exports both. A shim that skips them leaves `pyenv shell` and
# anything else reading them broken in a way that only shows up later.
assert_equals 'PYENV_SHELL is exported at startup' 'zsh' "$(field shell)"
assert_equals 'PYENV_ROOT is exported at startup' "$PYENV_DIR" "$(field root)"

assert_equals 'pyenv is defined but not yet loaded' 'lazy' "$(field pyenv-type)"

# --- calling pyenv loads it and works ---------------------------------------

# The recursion trap: the real `pyenv init` defines its own `pyenv` function.
# The shim must unfunction itself before evaluating that, or the two
# definitions fight and the first call never returns.
# The `pyenv` call that loads the real thing has to happen in this shell, not
# in a command substitution. `$(pyenv ...)` runs in a subshell, so the real
# init would load there and be discarded, leaving the parent still lazy. That
# is correct behaviour and not what this assertion is about, so the version is
# captured to a file instead.
version_file="$FIXTURES/pyenv-version"
loaded=$(zsh -i -c '
    pyenv version-name > "'"$version_file"'" 2>&1
    print -- "--after--"
    if (( $+functions[_pyenv] )); then print "eager"; else print "lazy"; fi
    print -- "--end--"
' 2>/dev/null)

after_field() { printf '%s' "$loaded" | sed -n "/^--$1--\$/,/^--/p" | sed '1d;/^--/d' | head -1; }

# pyenv is installed by Homebrew here, so $PYENV_ROOT/bin does not exist and
# the binary is found on PATH. Resolving it the way a shell does keeps this
# working under either installation shape.
REAL_PYENV=$(command -v pyenv 2>/dev/null)
[ -n "$REAL_PYENV" ] || REAL_PYENV="$PYENV_DIR/bin/pyenv"
expected_version=$(PYENV_ROOT="$PYENV_DIR" "$REAL_PYENV" version-name 2>/dev/null)
assert_succeeds 'the expected version was resolved' test -n "$expected_version"
assert_equals 'the first pyenv call returns the right version' \
    "$expected_version" "$(cat "$version_file" 2>/dev/null)"

assert_equals 'pyenv is loaded for real after the first call' \
    'eager' "$(after_field after)"

# A second call must still work. A shim that unfunctions itself and forgets to
# re-dispatch leaves the first call silent and every later one fine, which is
# the shape that passes a careless test.
twice=$(zsh -i -c 'pyenv version-name >/dev/null 2>&1; pyenv version-name 2>&1' 2>/dev/null | tail -1)
assert_equals 'a second pyenv call still works' "$expected_version" "$twice"

# --- the shims are not re-added ---------------------------------------------

# `pyenv init` removes the shims directory from PATH before prepending it, to
# avoid a duplicate entry on a re-source. The lazy path prepends directly, so
# a shell that loads pyenv for real must not end up with two.
shim_count=$(zsh -i -c '
    pyenv version-name >/dev/null 2>&1
    print -r -- "${PATH}" | tr ":" "\n" | grep -cx "'"$PYENV_DIR"'/shims"
' 2>/dev/null | tail -1)
assert_equals 'the shims directory appears on PATH exactly once after loading' \
    '1' "$shim_count"

finish
