#!/bin/bash
#
# Tests that the "Repo utilities" section of ~/README.md keeps up with the
# config-<sub> scripts beside the dispatcher.
#
# Scope is deliberately narrow, the same rule deps-docs.test.sh follows:
# assert only the facts that rot silently. A subcommand that ships without a
# README entry, or a README entry for a subcommand that no longer exists.
# Prose and section order are not asserted, because freezing those would make
# every edit to the writing a test failure.
#
# The one-line descriptions themselves are NOT duplicated here. They live in
# the `# help:` line of each config-<sub> and `config help` generates the
# listing from them, so there is no second copy to drift.
#
# Usage: ~/tests/config-docs.test.sh

. "$(dirname "$0")/lib.sh"

CONFIG_DIR="$DOTFILES_ROOT/.scripts/config"
HOME_README="$DOTFILES_ROOT/README.md"

assert_succeeds 'the home README exists' test -f "$HOME_README"

section=$(awk '/^### Repo utilities$/{found=1; next} found && /^### /{exit} found' "$HOME_README")
assert_succeeds 'the README has a "Repo utilities" section' test -n "$section"

# shellcheck disable=SC2012  # config-<sub> names cannot contain spaces
subcommands=$(cd "$CONFIG_DIR" && ls config-* 2>/dev/null | sed 's/^config-//' | sort)

undocumented=''
while IFS= read -r sub; do
    [ -n "$sub" ] || continue
    printf '%s\n' "$section" | grep -qF "config $sub" || undocumented="$undocumented $sub"
done <<EOF
$subcommands
EOF
assert_equals 'every config-<sub> appears in the README section' '' "$undocumented"

# The reverse direction: a bullet naming a subcommand that no longer exists.
# This is the failure deps-docs.test.sh exists to catch for deps.conf.
#
# Only the bulleted list is scanned. The surrounding prose names git verbs on
# purpose (`config status`, `config commit`) to show that passthrough works,
# and those are not claims that a config-<sub> exists.
stale=''
while IFS= read -r named; do
    [ -n "$named" ] || continue
    printf '%s\n' "$subcommands" | grep -qx "$named" || stale="$stale $named"
done <<EOF
$(printf '%s\n' "$section" | sed -n 's/^- `config \([a-z-]*\)`.*/\1/p' | sort -u)
EOF
assert_equals 'the README bullets name no subcommand that has been removed' '' "$stale"

# The section must point the reader at the generated listing rather than
# restating it, so the descriptions have exactly one home.
assert_contains 'the README points at config help' 'config help' "$section"

finish
