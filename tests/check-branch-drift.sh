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

set -u

DOTFILES_ROOT=${DOTFILES_ROOT:-$HOME}
REF_A=${1:-origin/mac}
REF_B=${2:-origin/linux}

manifest=$(git -C "$DOTFILES_ROOT" show "$REF_A:.sync-manifest" 2>/dev/null) || {
    printf 'check-branch-drift: could not read .sync-manifest from %s\n' "$REF_A" >&2
    exit 1
}

diverged_count=0
checked_count=0

# Splitting on newline in a for-loop, not the usual pipe-into-while: a piped
# while runs in a subshell in POSIX sh, and these counters need to survive
# the loop.
old_ifs=$IFS
IFS='
'
for path in $manifest; do
    IFS=$old_ifs
    case $path in
        ''|'#'*) continue ;;
    esac
    checked_count=$((checked_count + 1))

    if ! git -C "$DOTFILES_ROOT" diff --quiet "$REF_A" "$REF_B" -- "$path"; then
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
