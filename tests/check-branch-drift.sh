#!/bin/sh
#
# Checks that every path listed in .sync-manifest is identical between two
# git refs. Prints each diverged path and exits non-zero if any are found.
#
# Usage:
#   check-branch-drift.sh [ref-a] [ref-b]
#
# Defaults to comparing origin/mac against origin/linux inside
# $DOTFILES_ROOT. Reads the manifest from ref-a, since both refs are
# expected to carry an identical copy of it.
#
# A manifest line starting with "!" excludes that path from every check
# (e.g. a mac-only test file that lives inside an otherwise-shared
# directory), regardless of where the "!" line sits relative to the
# directory it excludes from.

set -u

DOTFILES_ROOT=${DOTFILES_ROOT:-$HOME}
REF_A=${1:-origin/mac}
REF_B=${2:-origin/linux}

# The real dotfiles repo is a bare repo at $DOTFILES_ROOT/.cfg with no .git
# inside $DOTFILES_ROOT, so plain `git -C "$DOTFILES_ROOT"` can't discover it
# at all -- it only ever "worked" against test fixtures, which are normal
# `git init` repos with .git inside them. Detect which shape this is.
if [ -d "$DOTFILES_ROOT/.cfg" ]; then
    git_cmd() { git --git-dir="$DOTFILES_ROOT/.cfg" --work-tree="$DOTFILES_ROOT" "$@"; }
else
    git_cmd() { git -C "$DOTFILES_ROOT" "$@"; }
fi

manifest=$(git_cmd show "$REF_A:.sync-manifest" 2>/dev/null) || {
    printf 'check-branch-drift: could not read .sync-manifest from %s\n' "$REF_A" >&2
    exit 1
}

diverged_count=0
checked_count=0

# A line starting with "!" excludes that path from every other check, via
# git's pathspec exclusion magic -- not just the entry immediately above it.
# Collected in a first pass so line order in the manifest doesn't matter.
excludes=''
old_ifs=$IFS
IFS='
'
for line in $manifest; do
    IFS=$old_ifs
    case $line in
        '!'*) excludes="$excludes :(exclude)${line#!}" ;;
    esac
    IFS='
'
done
IFS=$old_ifs

# Splitting on newline in a for-loop, not the usual pipe-into-while: a piped
# while runs in a subshell in POSIX sh, and these counters need to survive
# the loop.
old_ifs=$IFS
IFS='
'
for path in $manifest; do
    IFS=$old_ifs
    case $path in
        ''|'#'*|'!'*) continue ;;
    esac
    checked_count=$((checked_count + 1))

    if ! git_cmd diff --quiet "$REF_A" "$REF_B" -- "$path" $excludes; then
        printf 'diverged: %s\n' "$path"
        diverged_count=$((diverged_count + 1))
    fi
    IFS='
'
done
IFS=$old_ifs

if [ "$diverged_count" -gt 0 ]; then
    printf '\ncheck-branch-drift: %s and %s diverged on %d of %d shared path(s)\n' \
        "$REF_A" "$REF_B" "$diverged_count" "$checked_count" >&2
    exit 1
fi

printf 'check-branch-drift: %s and %s match on all %d shared path(s)\n' \
    "$REF_A" "$REF_B" "$checked_count"
exit 0
