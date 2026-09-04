#!/bin/bash
#
# Integration tests for the lazy nvm setup in .zshrc-mac.
#
# Sourcing nvm.sh costs 2.4s, `nvm ls v24` about 1s more, and `nvm use 24`
# another 2.4s. Paying all three at every shell startup put interactive zsh at
# 7-9s, and `se` builds 107 panes, so the cumulative cost ran to minutes before
# the terminal was usable.
#
# `nvm use` only manipulates PATH, so putting the newest v24 bin directory on
# PATH directly reproduces its end state for free. nvm itself becomes a lazy
# function, so the cost is paid only when a version is actually switched.
#
# The contract these tests pin down:
#   1. startup runs no nvm command, so it stays fast
#   2. node/npm/npx still resolve to the newest installed v24
#   3. `nvm` is available but not yet loaded
#   4. `nvm use <other>` still overrides the version for that shell
#
# Point 4 is the one worth testing rather than assuming: it works because
# `nvm use` strips any nvm version directory already on PATH before prepending
# its own, so the manual entry is replaced rather than shadowed.
#
# These tests are macOS-only because .zshrc-mac is. Linux carries .zshrc-linux
# and is unaffected.
#
# Usage: ~/tests/zshrc-node-startup.test.sh

. "$(dirname "$0")/lib.sh"

ZSHRC_MAC="$DOTFILES_ROOT/.zshrc-mac"
NVM_NODE_DIR="$HOME/.nvm/versions/node"

if [ "$(uname -s)" != "Darwin" ]; then
    printf 'zshrc-node-startup: skipped, .zshrc-mac is macOS-only\n'
    exit 0
fi

assert_succeeds 'the mac zshrc is present' test -f "$ZSHRC_MAC"

# The whole point is that startup does not pay for nvm. Assert on the source
# text: no unconditional `nvm use`, `nvm ls`, or nvm.sh source at top level.
assert_equals 'startup does not source nvm.sh' \
    '' "$(grep -nE '^[[:space:]]*(\\?\.|source)[[:space:]]+.*nvm\.sh' "$ZSHRC_MAC")"

assert_equals 'startup does not run `nvm use`' \
    '' "$(grep -nE '^[[:space:]]*nvm[[:space:]]+use' "$ZSHRC_MAC")"

assert_equals 'startup does not run `nvm ls`' \
    '' "$(grep -nE '^[[:space:]]*(if[[:space:]]+!?[[:space:]]*)?nvm[[:space:]]+ls' "$ZSHRC_MAC")"

# The newest installed v24, chosen the way the shell code must choose it.
# A plain lexical sort picks v24.9 over v24.15, so the version-aware sort is
# part of the contract, not a detail.
newest_v24=$(ls -d "$NVM_NODE_DIR"/v24.* 2>/dev/null | sort -V | tail -1)

if [ -z "$newest_v24" ]; then
    printf 'zshrc-node-startup: skipped, no v24 installed under %s\n' "$NVM_NODE_DIR"
    finish
fi

# A fresh interactive shell must resolve node to that directory.
report=$(zsh -i -c '
    print -- "--node--";   command -v node
    print -- "--npm--";    command -v npm
    print -- "--npx--";    command -v npx
    print -- "--version--"; node --version
    print -- "--nvm-type--"
    # `nvm` must exist but must NOT be loaded yet. A loaded nvm is a large
    # function; the lazy shim is a small one, so test behaviour, not size:
    # the real nvm defines nvm_ls_current, the shim does not.
    if typeset -f nvm >/dev/null 2>&1; then
        if typeset -f nvm_ls_current >/dev/null 2>&1; then
            print "eager"
        else
            print "lazy"
        fi
    else
        print "missing"
    fi
' 2>/dev/null)

field() { printf '%s' "$report" | sed -n "/^--$1--\$/,/^--/p" | sed '1d;/^--/d' | head -1; }

assert_equals 'node resolves to the newest installed v24' \
    "$newest_v24/bin/node" "$(field node)"

assert_equals 'npm resolves to the newest installed v24' \
    "$newest_v24/bin/npm" "$(field npm)"

assert_equals 'npx resolves to the newest installed v24' \
    "$newest_v24/bin/npx" "$(field npx)"

assert_equals 'node reports the newest installed v24' \
    "${newest_v24##*/}" "$(field version)"

assert_equals 'nvm is defined but not yet loaded' \
    'lazy' "$(field nvm-type)"

# The override path. Pick any installed non-v24 version to switch to; skip if
# the machine only has v24, since then there is nothing to switch to.
other=$(ls -d "$NVM_NODE_DIR"/v* 2>/dev/null | grep -v '/v24\.' | sort -V | tail -1)

if [ -n "$other" ]; then
    other_version=${other##*/}
    override=$(zsh -i -c "
        nvm use ${other_version#v} >/dev/null 2>&1
        print -- '--which--';   command -v node
        print -- '--reports--'; node --version
        print -- '--end--'
    " 2>/dev/null)

    over_field() {
        printf '%s' "$override" | sed -n "/^--$1--\$/,/^--/p" | sed '1d;/^--/d' | head -1
    }

    assert_equals "nvm use $other_version overrides node for that shell" \
        "$other/bin/node" "$(over_field which)"

    assert_equals "nvm use $other_version reports the switched version" \
        "$other_version" "$(over_field reports)"

    # Switching must replace the startup PATH entry, not stack on top of it.
    # Otherwise every switch grows PATH for the life of the shell.
    #
    # `env -i` is load-bearing. A test run inherits the developer's PATH, which
    # already carries nvm directories from whatever that shell did earlier, so
    # counting entries in an inherited environment measures that history rather
    # than this config. Only a pristine login shell isolates the invariant.
    nvm_entries=$(env -i HOME="$HOME" TERM=xterm /bin/zsh -l -i -c "
        nvm use ${other_version#v} >/dev/null 2>&1
        print -- '--count--'
        print -l \${(s/:/)PATH} | grep -c 'nvm/versions/node'
        print -- '--end--'
    " 2>/dev/null | sed -n '/^--count--$/,/^--end--$/p' | sed '1d;/^--/d' | head -1)

    assert_equals 'switching leaves exactly one nvm directory on PATH' \
        '1' "$nvm_entries"
fi

finish
