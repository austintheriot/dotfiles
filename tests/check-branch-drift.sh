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
#
# A manifest line starting with "~" marks a path as tracked on purpose but
# never compared, because it is meant to differ per branch (.zshrc, a
# platform-only config). It is distinct from "!": "!" means "inside a shared
# path, skip this one", while "~" means "accounted for, never compared".
# Only "~" satisfies the exhaustiveness check below, which is what stops a
# new file from escaping by simply not being mentioned.

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
        ''|'#'*|'!'*|'~'*) continue ;;
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

# --- every tracked file must match a rule -------------------------------
#
# The comparison above only inspects paths the manifest names. A file
# outside every rule was invisible to it: the check reported "match on all"
# while the branches genuinely differed. This phase closes that by requiring
# each tracked file to match some rule.
#
# The file lists are unioned across both refs on purpose. The branches do
# not carry identical file sets, so scanning ref-a alone would let a file
# added only on ref-b escape -- the same bug in a different place.

files_a=$(git_cmd ls-tree -r --name-only "$REF_A" 2>/dev/null)
files_b=$(git_cmd ls-tree -r --name-only "$REF_B" 2>/dev/null)

# Reports which refs a path is on. A file present on one branch only is
# usually a file that wants the ~ label, so saying so saves the reader a
# lookup.
branches_for() {
    found=''
    printf '%s\n' "$files_a" | grep -qxF "$1" && found=$REF_A
    if printf '%s\n' "$files_b" | grep -qxF "$1"; then
        if [ -n "$found" ]; then found="$found, $REF_B"; else found=$REF_B; fi
    fi
    printf '%s' "$found"
}

# A rule ending in "/" covers everything beneath it. Any other rule matches
# that exact path only, so a rule named DOTFILES.md must not absorb
# DOTFILES.md.bak.
is_covered() {
    file=$1
    inner_ifs=$IFS
    IFS='
'
    for rule in $manifest; do
        IFS=$inner_ifs
        case $rule in
            ''|'#'*) IFS='
'; continue ;;
            '!'*|'~'*) rule=${rule#?} ;;
        esac
        [ -n "$rule" ] || { IFS='
'; continue; }
        case $file in
            "$rule") return 0 ;;
            "$rule"*) case $rule in */) return 0 ;; esac ;;
        esac
        IFS='
'
    done
    IFS=$inner_ifs
    return 1
}

unlabeled=''
unlabeled_count=0
old_ifs=$IFS
IFS='
'
for file in $(printf '%s\n%s\n' "$files_a" "$files_b" | sort -u); do
    IFS=$old_ifs
    [ -n "$file" ] || { IFS='
'; continue; }
    if ! is_covered "$file"; then
        unlabeled="$unlabeled$file	$(branches_for "$file")
"
        unlabeled_count=$((unlabeled_count + 1))
    fi
    IFS='
'
done
IFS=$old_ifs

if [ "$unlabeled_count" -gt 0 ]; then
    printf '\ncheck-branch-drift: %d tracked file(s) match no .sync-manifest rule\n\n' \
        "$unlabeled_count" >&2
    printf '%s' "$unlabeled" | while IFS='	' read -r file refs; do
        [ -n "$file" ] || continue
        printf '  %-44s (on %s)\n' "$file" "$refs" >&2
    done
    printf '\nEvery tracked file must match a rule, so a file added on one branch\n' >&2
    printf 'cannot escape this check. Add one of these to .sync-manifest:\n\n' >&2
    printf '  %-44s shared: must be identical on both branches\n' 'path/to/file' >&2
    printf '  %-44s per-branch: tracked, never compared\n' '~path/to/file' >&2
    printf '  %-44s excluded from an enclosing shared path\n\n' '!path/to/file' >&2
    exit 1
fi

if [ "$diverged_count" -gt 0 ]; then
    printf '\ncheck-branch-drift: %s and %s diverged on %d of %d shared path(s)\n' \
        "$REF_A" "$REF_B" "$diverged_count" "$checked_count" >&2
    exit 1
fi

printf 'check-branch-drift: %s and %s match on all %d shared path(s)\n' \
    "$REF_A" "$REF_B" "$checked_count"
exit 0
