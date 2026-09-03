#!/bin/bash
#
# Tests that this machine's git hooks are installed, not merely present in
# the work tree.
#
# The hook scripts (tests/pre-commit, tests/pre-push) are tracked, so they
# travel with the repo. The symlinks under .cfg/hooks that make git actually
# run them do not travel, and nothing recreates them on a new machine. This
# machine had tests/pre-push tracked and executable for a full day while
# .cfg/hooks/pre-push did not exist, so every push skipped the drift check
# and the test suite silently. container.test.sh asserts the hook file's
# contents; only this file asserts git will run it.
#
# Skipped entirely where there is no .cfg repository: the test image carries
# the tracked files but not the bare repo they came from, and a machine
# without .cfg has no hooks to install.
#
# Usage: ~/tests/githooks-installed.test.sh

. "$(dirname "$0")/lib.sh"

CFG_DIR="$DOTFILES_ROOT/.cfg"

# `finish` reports the tally and returns a status; it does not exit. An early
# skip has to exit explicitly, or every assertion below still runs.
if [ ! -d "$CFG_DIR" ]; then
    printf '      skipped: no .cfg repository at %s\n' "$CFG_DIR"
    finish
    exit 0
fi

# core.hooksPath overrides $GIT_DIR/hooks wholesale. If it is ever set, the
# symlinks this file checks stop being the thing git runs, and every
# assertion below would pass while no hook fired.
hooks_path=$(git --git-dir="$CFG_DIR" --work-tree="$DOTFILES_ROOT" \
    config --get core.hooksPath 2>/dev/null || true)
assert_equals 'core.hooksPath is unset, so .cfg/hooks is what git runs' \
    '' "$hooks_path"

for hook in pre-commit pre-push; do
    installed="$CFG_DIR/hooks/$hook"
    tracked="$DOTFILES_ROOT/tests/$hook"

    assert_succeeds "$hook is installed in .cfg/hooks" test -e "$installed"

    # Executability is what git checks before running a hook. A present but
    # non-executable hook is skipped without a word.
    assert_succeeds "$hook is executable" test -x "$installed"

    # The installed hook must be the tracked one, so editing tests/$hook
    # changes what runs. A copy would drift silently.
    assert_equals "$hook resolves to the tracked script" \
        "$tracked" "$(readlink "$installed" 2>/dev/null || printf '%s' "$installed")"
done

finish
