#!/bin/bash
#
# Guards the third-party action versions pinned in .github/workflows.
#
# GitHub retires the Node runtime an action declares, then forces the action
# onto the current runtime and prints a deprecation warning on every run:
#
#   Node.js 20 is deprecated. The following actions target Node.js 20 but are
#   being forced to run on Node.js 24: actions/checkout@v4.
#
# The warning is not fatal, but "forced onto a runtime the action was never
# tested against" is a real compatibility risk, and a warning nobody acts on
# trains everyone to ignore the CI log.
#
# actions/checkout@v5 and later declare node24. v4 and earlier declare node20
# or older, so this suite fails the build if a known-deprecated major version
# comes back. It checks the pinned text only and makes no network call, so it
# runs offline and cannot flake.
#
# Usage: ~/tests/workflow-action-versions.test.sh

. "$(dirname "$0")/lib.sh"

WORKFLOWS="$DOTFILES_ROOT/.github/workflows"
NEWLINE='
'

assert_succeeds 'the workflows directory is present' \
    test -d "$WORKFLOWS"

# Action major versions that still declare a Node runtime GitHub has retired.
# Extend this list when GitHub announces the next deprecation.
deprecated_pins='actions/checkout@v1
actions/checkout@v2
actions/checkout@v3
actions/checkout@v4'

found=''
for pin in $deprecated_pins; do
    hits=$(grep -rn --include='*.yml' --include='*.yaml' -F "$pin" "$WORKFLOWS" 2>/dev/null)
    if [ -n "$hits" ]; then
        found="${found}${hits}${NEWLINE}"
    fi
done

assert_equals 'no workflow pins an action on a deprecated Node runtime' \
    '' "$(printf '%s' "$found")"

# Every `uses:` of a versioned action must carry an explicit version, so an
# upstream release cannot silently change what CI runs.
unpinned=$(grep -rhoE '^[[:space:]]*-?[[:space:]]*uses:[[:space:]]*[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+[[:space:]]*$' \
    "$WORKFLOWS" 2>/dev/null)

assert_equals 'every action `uses:` carries an explicit version' \
    '' "$(printf '%s' "$unpinned")"

finish
