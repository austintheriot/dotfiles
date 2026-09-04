#!/bin/bash
#
# Guards the name of the user scripts directory.
#
# The directory was renamed from `.my-scripts` to `.scripts`, and the old
# name was referenced from 33 tracked files: shell aliases, tmux hooks, two
# Dockerfile COPY targets, three anchored regexes (the leak-check allow
# list, the pre-push trigger filter, and a CI path filter), and a lot of
# prose. A rename that misses any one of those fails silently rather than
# loudly -- an unmatched tmux hook path just stops renaming windows, and an
# unmatched trigger regex just stops running the test suite before a push.
#
# So this suite asserts the invariant rather than the edit: the directory
# has exactly one name, and that name appears nowhere in its old form.
#
# Every assertion reads through $DOTFILES_ROOT so the container harness and
# the CI checkout exercise the same code. Deliberately no `git` calls: this
# suite runs inside the test container, which has no repository.
#
# Usage: ~/tests/scripts-dir-name.test.sh

. "$(dirname "$0")/lib.sh"

OLD_NAME='.my-scripts'
NEW_NAME='.scripts'

# The bare stem, searched separately from $OLD_NAME. The first sweep of this
# rename matched only the dotted form and left two references behind: a test
# description reading "matches a my-scripts shell edit", and worse, a Python
# assertion whose needle was the literal "my-scripts" -- once the path in its
# input became .scripts, that assertion passed no matter what the code did.
# A stale word in prose is cosmetic; a stale word in an assertion is a test
# that stopped testing. Both forms are checked because of it.
OLD_STEM='my-scripts'

# --- the directory itself carries the new name ---------------------------

assert_succeeds 'the .scripts directory exists' \
    test -d "$DOTFILES_ROOT/$NEW_NAME"
assert_succeeds 'the .my-scripts directory is gone' \
    test '!' -d "$DOTFILES_ROOT/$OLD_NAME"

# --- the move preserved each script's mode ------------------------------
#
# `git mv` preserves the mode; a rename done by copy-and-delete would not.
# The split is not cosmetic: a tmux hook or a CI step that runs a script as
# a command fails silently when the execute bit is missing, and the four
# scripts below are the ones invoked that way.
#
# The remaining scripts are deliberately NOT executable, because .zshrc
# `source`s them (`alias sp='source ~/.scripts/tmux-split.sh'`) to let them
# modify the calling shell. `source` ignores the execute bit, so marking
# them executable would suggest a way of running them that does not work:
# executing tmux-split.sh in a subshell changes that subshell and exits.

EXECUTED_SCRIPTS='deps/check-deps.sh deps/test-local.sh
tmux-update-window-names.sh tmux-worktree-config.sh
config/config-stamp config/config-build'

SOURCED_SCRIPTS='tmux-close.sh tmux-setup.sh tmux-split.sh tmux-start.sh
zsh-git-widgets.sh deps/depcheck-hook.sh'

missing_scripts=''
non_executable=''
for script in $EXECUTED_SCRIPTS; do
    path="$DOTFILES_ROOT/$NEW_NAME/$script"
    if [ ! -f "$path" ]; then
        missing_scripts="$missing_scripts $script"
        continue
    fi
    [ -x "$path" ] || non_executable="$non_executable $script"
done

for script in $SOURCED_SCRIPTS; do
    [ -f "$DOTFILES_ROOT/$NEW_NAME/$script" ] \
        || missing_scripts="$missing_scripts $script"
done

assert_equals 'every script moved to .scripts' '' "$missing_scripts"
assert_equals 'every executed script is still executable' '' "$non_executable"

# Asserted in both directions. Without this, marking every script executable
# would satisfy the check above while breaking nothing loudly and quietly
# advertising a broken way to invoke the sourced ones.
wrongly_executable=''
for script in $SOURCED_SCRIPTS; do
    path="$DOTFILES_ROOT/$NEW_NAME/$script"
    [ -f "$path" ] || continue
    [ -x "$path" ] && wrongly_executable="$wrongly_executable $script"
done
assert_equals 'no sourced script is marked executable' '' "$wrongly_executable"

# --- no tracked file still names the old directory ----------------------
#
# SEARCH_ROOTS is an allowlist of the tracked paths, not a filesystem walk.
# $DOTFILES_ROOT is $HOME on a developer machine, where 253 tracked files sit
# among roughly 67000 on disk: .claude alone holds 783M of vendored plugins
# and 753M of session transcripts. An unscoped find spends minutes reading
# trees that a rename must never rewrite, and the transcripts quote the old
# name as conversation history, so it would also report them forever.
#
# Listing the tracked roots instead is both fast and honest about scope. The
# named .claude children are exactly the tracked ones; the rest of that
# directory is machine-local runtime state.
#
# `git ls-files` would be the better oracle, but this suite also runs in the
# test container, which carries a copy of the tree and no repository.

SEARCH_ROOTS='.agents .config .github tests docs
.claude/agents .claude/data .claude/hooks .claude/rules
.claude/scripts .claude/skills .claude/CLAUDE.md
.sync-manifest .zshrc .zshrc-mac .zshrc-linux
DOTFILES.md README.md TODO-AGENTS.md'

present_roots=''
for root in $SEARCH_ROOTS; do
    [ -e "$DOTFILES_ROOT/$root" ] || continue
    present_roots="$present_roots $DOTFILES_ROOT/$root"
done
assert_succeeds 'at least one search root is present' test -n "$present_roots"

# Two exclusions, both necessary rather than convenient:
#
#   - This suite names the old directory in its own prose and in $OLD_NAME,
#     so without excluding itself the assertion could never pass.
#   - TODO-AGENTS.md quotes history verbatim: the claim line names the
#     directory being renamed, and an item can be a pasted error message
#     ("/Users/austin/.my-scripts/... returned 127"). Rewriting a quote
#     falsifies it. The file is per-branch (`~TODO-AGENTS.md`) and holds no
#     live path this repo resolves, so a stale name there breaks nothing.
#   - __pycache__ holds compiled bytecode that embeds the string from
#     whatever the .py source said when it was last imported. It is
#     regenerated from the source this suite already checks, and rewriting
#     a .pyc is not a thing a rename does.
#
# The new directory is searched so a self-reference left inside a moved
# script is caught, rather than skipped along with its own directory.
# shellcheck disable=SC2086
stale_files=$(grep -rlF "$OLD_NAME" \
    $present_roots "$DOTFILES_ROOT/$NEW_NAME" 2>/dev/null \
    | sed "s|^$DOTFILES_ROOT/||" \
    | grep -v '^tests/scripts-dir-name\.test\.sh$' \
    | grep -v '^TODO-AGENTS\.md$' \
    | grep -v '/__pycache__/' \
    | sort)
assert_equals 'no tracked file still names .my-scripts' '' "$stale_files"

# shellcheck disable=SC2086
stale_stem=$(grep -rlF "$OLD_STEM" \
    $present_roots "$DOTFILES_ROOT/$NEW_NAME" 2>/dev/null \
    | sed "s|^$DOTFILES_ROOT/||" \
    | grep -v '^tests/scripts-dir-name\.test\.sh$' \
    | grep -v '^TODO-AGENTS\.md$' \
    | grep -v '/__pycache__/' \
    | sort)
assert_equals 'no tracked file still names the my-scripts stem' '' "$stale_stem"

# The self-exclusion above is a hole in the sweep, so the one thing this
# suite is allowed to say about itself is asserted directly: it must not
# reference the old directory as a live path, only as a quoted name.
own_path_refs=$(grep -oE '[^A-Za-z0-9_/.-]\.my-scripts/[A-Za-z0-9_/.-]+' \
    "$0" 2>/dev/null | sort -u)
assert_equals 'this suite names no live .my-scripts path' '' "$own_path_refs"

# --- the scripts exist in the commit, not only on disk -------------------
#
# Every assertion above reads the working tree, and that is exactly the hole
# the rename fell through: the commit that renamed the directory recorded 15
# deletions and zero additions, because the files were absent from disk when
# `git add` ran. `git add` on a missing path stages nothing and exits 0, so
# the staged rename collapsed into pure deletions and the commit passed every
# on-disk check in this file. HEAD carried 237 tracked files instead of 252
# and every tmux hook broke with "returned 127".
#
# So this asserts against HEAD rather than the filesystem. Skipped where
# there is no repository: the test container carries a copy of the tree.

if [ -d "$DOTFILES_ROOT/.cfg" ]; then
    git_cmd() {
        git --git-dir="$DOTFILES_ROOT/.cfg" --work-tree="$DOTFILES_ROOT" "$@"
    }
elif [ -d "$DOTFILES_ROOT/.git" ]; then
    git_cmd() { git -C "$DOTFILES_ROOT" "$@"; }
else
    git_cmd() { return 1; }
fi

if git_cmd rev-parse --verify HEAD >/dev/null 2>&1; then
    committed=$(git_cmd ls-tree -r --name-only HEAD 2>/dev/null \
        | grep "^$NEW_NAME/" | sort)
    committed_count=$(printf '%s\n' "$committed" | grep -c . || true)

    # An exact count rather than "more than zero": a partial add is the
    # failure that actually happened, and it leaves some files staged.
    assert_equals 'all 17 scripts are committed, not only on disk' \
        '17' "$committed_count"

    # The execute bits have to survive the commit too. A script committed
    # 100644 fails at runtime on a fresh clone while working on the machine
    # that has the bit set locally, which is the worst shape for this bug.
    committed_exec=$(git_cmd ls-tree -r HEAD "$NEW_NAME" 2>/dev/null \
        | awk '$1 == "100755" { print $4 }' | sort)
    expected_exec=$(printf '%s\n' \
        "$NEW_NAME/config/config-build" \
        "$NEW_NAME/config/config-stamp" \
        "$NEW_NAME/deps/check-deps.sh" \
        "$NEW_NAME/deps/test-local.sh" \
        "$NEW_NAME/tmux-update-window-names.sh" \
        "$NEW_NAME/tmux-worktree-config.sh" | sort)
    assert_equals 'the committed execute bits match the executed scripts' \
        "$expected_exec" "$committed_exec"

    assert_equals 'no .my-scripts path survives in the commit' '' \
        "$(git_cmd ls-tree -r --name-only HEAD 2>/dev/null \
            | grep "^$OLD_NAME/" || true)"
else
    printf 'skip: no repository here, so the commit cannot be inspected\n'
fi

# --- the three anchored regexes point at the new directory --------------
#
# These are the references a plain text sweep gets wrong, because each one
# is an escaped regex rather than a bare path. Asserted individually so a
# failure names which gate stopped matching, instead of only reporting that
# some file somewhere still holds the old string.

assert_succeeds 'the pre-push trigger filter matches .scripts' \
    grep -qF '^\.scripts/.*\.sh$' "$DOTFILES_ROOT/tests/pre-push"

leak_allow="$DOTFILES_ROOT/.claude/local/leak-allow.conf"
if [ -f "$leak_allow" ]; then
    assert_succeeds 'the leak-check allow list matches .scripts' \
        grep -qF '^\.scripts/tmux-.*\.sh$' "$leak_allow"
else
    printf 'skip: leak-allow.conf is machine-local and absent here\n'
fi

assert_succeeds 'the CI path filter watches .scripts' \
    grep -qF "'.scripts/deps/**'" \
    "$DOTFILES_ROOT/.github/workflows/deps-check.yml"

# --- .sync-manifest still accounts for the directory --------------------
#
# The manifest is exhaustive: every tracked file must match a rule. Renaming
# the directory without renaming its rule would leave 15 files unlabeled and
# fail the branch-drift gate on the next push.

manifest="$DOTFILES_ROOT/.sync-manifest"
assert_succeeds 'the manifest shares the .scripts directory' \
    grep -qxF "$NEW_NAME/" "$manifest"
assert_succeeds 'the manifest still excludes the local deps config' \
    grep -qxF "!$NEW_NAME/deps/deps-local.conf" "$manifest"

finish
