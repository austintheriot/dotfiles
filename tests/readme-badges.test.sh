#!/bin/bash
#
# Tests the CI status badges at the top of ~/README.md.
#
# A badge is a URL, and a URL rots silently. Renaming a workflow file, or
# dropping a branch from a workflow's `on: push:` list, leaves a badge that
# still renders but reports on nothing. GitHub serves "no status" rather than
# an error, so the README keeps looking fine while telling the reader less
# than it claims.
#
# The reachable facts are asserted, not the rendered image: the workflow file
# the badge names exists, the branch it queries is one this repo actually
# pushes, and that workflow runs on that branch. The network is never touched,
# so this suite passes offline and in the container.
#
# README.md is per-branch (see .sync-manifest), so this runs against whichever
# branch's README is checked out and both copies must carry badges.
#
# Usage: ~/tests/readme-badges.test.sh

. "$(dirname "$0")/lib.sh"

README="$DOTFILES_ROOT/README.md"
WORKFLOW_DIR="$DOTFILES_ROOT/.github/workflows"

assert_succeeds 'the home README exists' test -f "$README"

# A badge line looks like:
#   [![Label](https://github.com/<owner>/<repo>/actions/workflows/<file>/badge.svg?branch=<branch>)](<link>)
badges=$(grep -o 'actions/workflows/[^)]*badge\.svg[^)]*' "$README" || true)
assert_succeeds 'the README carries at least one CI badge' test -n "$badges"

# Every badge must name a workflow file that exists.
missing_workflow=''
while IFS= read -r badge; do
    [ -n "$badge" ] || continue
    file=${badge#actions/workflows/}
    file=${file%%/badge.svg*}
    [ -f "$WORKFLOW_DIR/$file" ] || missing_workflow="$missing_workflow $file"
done <<EOF
$badges
EOF
assert_equals 'every badge names a workflow file that exists' '' "$missing_workflow"

# Every badge must pin a branch. Without ?branch=, GitHub reports the default
# branch, which is not what a per-branch badge is claiming to show.
unpinned=''
while IFS= read -r badge; do
    [ -n "$badge" ] || continue
    case $badge in
        *'branch='*) ;;
        *) unpinned="$unpinned ${badge%%/badge.svg*}" ;;
    esac
done <<EOF
$badges
EOF
assert_equals 'every badge pins an explicit branch' '' "$unpinned"

# The branch a badge queries must be one this repo actually pushes, and the
# workflow must be triggered on it. A badge for a branch the workflow ignores
# renders as "no status" forever.
BRANCHES='mac linux'
bad_branch=''
not_triggered=''
while IFS= read -r badge; do
    [ -n "$badge" ] || continue
    file=${badge#actions/workflows/}
    file=${file%%/badge.svg*}
    branch=${badge##*branch=}
    branch=${branch%%&*}

    # shellcheck disable=SC2086  # deliberate split of the space-separated list
    printf '%s\n' $BRANCHES | grep -qx "$branch" \
        || bad_branch="$bad_branch $branch"

    [ -f "$WORKFLOW_DIR/$file" ] || continue
    triggers=$(awk '/^on:/{found=1; next} found && /^[a-z]/{exit} found' "$WORKFLOW_DIR/$file")
    printf '%s\n' "$triggers" | grep -q "$branch" \
        || not_triggered="$not_triggered $file:$branch"
done <<EOF
$badges
EOF
assert_equals 'every badge names a branch this repo pushes' '' "$bad_branch"
assert_equals 'every badge workflow is triggered on the branch it reports' '' \
    "$not_triggered"

# Both branches this repo maintains should be visible, not just the one whose
# README is checked out. The point of the badges is a side-by-side comparison.
missing_branch=''
for branch in $BRANCHES; do
    printf '%s\n' "$badges" | grep -q "branch=$branch" \
        || missing_branch="$missing_branch $branch"
done
assert_equals 'both mac and linux are represented in the badges' '' \
    "$missing_branch"

# The badge block belongs above the first section heading, where a reader sees
# it before anything else.
first_badge=$(grep -n 'badge\.svg' "$README" | head -1 | cut -d: -f1)
first_heading=$(grep -n '^## ' "$README" | head -1 | cut -d: -f1)
[ -n "$first_badge" ] && [ -n "$first_heading" ] && [ "$first_badge" -lt "$first_heading" ] \
    && above=yes || above="no (badge line $first_badge, heading line $first_heading)"
assert_equals 'the badges appear above the first section heading' 'yes' "$above"

finish
