#!/bin/bash
#
# Tests for tmux-update-window-names.sh
#
# Runs against a throwaway tmux session and throwaway git repos under a
# temp dir. Never touches live sessions.
#
# Usage: ./tests/tmux-update-window-names.test.sh

set -u

SCRIPT="$(cd "$(dirname "$0")/.." && pwd)/tmux-update-window-names.sh"
SESSION="wname-test-$$"
FIXTURES="$(mktemp -d "${TMPDIR:-/tmp}/wname-test-XXXXXX")"

passed=0
failed=0

cleanup() {
    tmux kill-session -t "$SESSION" 2>/dev/null
    tmux kill-session -t "${SESSION}-b" 2>/dev/null
    rm -rf "$FIXTURES"
}
trap cleanup EXIT

fail() {
    failed=$((failed + 1))
    printf 'FAIL: %s\n' "$1"
    printf '      expected: [%s]\n' "$2"
    printf '      actual:   [%s]\n' "$3"
}

assert_equals() {
    local description=$1 expected=$2 actual=$3
    if [ "$expected" = "$actual" ]; then
        passed=$((passed + 1))
        printf 'ok: %s\n' "$description"
    else
        fail "$description" "$expected" "$actual"
    fi
}

window_name() {
    tmux display-message -p -t "$1" '#{window_name}'
}

# Pane ids are the only reliable way to target a pane by position: "@12.1"
# is not valid pane-target syntax.
pane_id() {
    tmux list-panes -t "$1" -F '#{pane_id}' | sed -n "$2p"
}

# The real config installs these hooks globally. A test session inherits them
# and would race the assertions, so each test session overrides them with a
# no-op and drives the script explicitly instead.
isolate_hooks() {
    local session=$1 hook
    for hook in after-new-window after-split-window after-select-window \
                after-select-pane after-kill-pane client-session-changed; do
        tmux set-hook -t "$session" "$hook" '' 2>/dev/null
    done
}

make_repo() {
    local path="$FIXTURES/$1" branch=$2
    mkdir -p "$path"
    git -C "$path" init -q -b "$branch"
    git -C "$path" -c user.email=t@t -c user.name=t commit -q --allow-empty -m init
    printf '%s' "$path"
}

# Windows created without -n keep tmux's automatic-rename flag on, which is
# what a window made by a keybinding looks like.
new_window() {
    local dir=$1
    tmux new-window -d -t "$SESSION" -c "$dir" -P -F '#{window_id}'
}

# --- setup -------------------------------------------------------------

repo_main=$(make_repo repo-main main)
repo_feature=$(make_repo repo-feature feature/login)
plain_dir="$FIXTURES/not-a-repo"
mkdir -p "$plain_dir"

tmux new-session -d -s "$SESSION" -n placeholder -c "$FIXTURES"
isolate_hooks "$SESSION"

# --- tier 1: cwd basename ----------------------------------------------

win=$(new_window "$plain_dir")
"$SCRIPT" -w "$win"
assert_equals 'non-repo cwd yields basename' 'not-a-repo' "$(window_name "$win")"

win_home=$(new_window "$HOME")
"$SCRIPT" -w "$win_home"
assert_equals 'home directory yields tilde' '~' "$(window_name "$win_home")"

# --- tier 2: git branch supersedes cwd ---------------------------------

win_repo=$(new_window "$repo_main")
"$SCRIPT" -w "$win_repo"
assert_equals 'git repo yields branch, not basename' 'main' "$(window_name "$win_repo")"

win_slash=$(new_window "$repo_feature")
"$SCRIPT" -w "$win_slash"
assert_equals 'branch name with slash is preserved' 'feature/login' "$(window_name "$win_slash")"

git -C "$repo_main" checkout -q -b renamed
"$SCRIPT" -w "$win_repo"
assert_equals 'owned window follows branch changes' 'renamed' "$(window_name "$win_repo")"

git -C "$repo_main" checkout -q --detach
detached_sha=$(git -C "$repo_main" rev-parse --short HEAD)
"$SCRIPT" -w "$win_repo"
assert_equals 'detached HEAD yields short sha' "($detached_sha)" "$(window_name "$win_repo")"
git -C "$repo_main" checkout -q renamed

# --- tier 3: manual rename supersedes ----------------------------------

tmux rename-window -t "$win_repo" 'MyName'
"$SCRIPT" -w "$win_repo"
assert_equals 'manual rename is not clobbered' 'MyName' "$(window_name "$win_repo")"

git -C "$repo_main" checkout -q -b another
"$SCRIPT" -w "$win_repo"
assert_equals 'manual rename survives a branch change' 'MyName' "$(window_name "$win_repo")"

# --- revert: empty name reclaims ownership -----------------------------

tmux rename-window -t "$win_repo" ''
"$SCRIPT" -w "$win_repo"
assert_equals 'empty name reverts to automatic naming' 'another' "$(window_name "$win_repo")"

# --- label prefix ------------------------------------------------------

win_label=$(new_window "$repo_feature")
tmux set -w -t "$win_label" @wname_label 'Reviews'
"$SCRIPT" -w "$win_label"
assert_equals 'label prefixes the automatic name' 'Reviews - feature/login' "$(window_name "$win_label")"

"$SCRIPT" -w "$win_label"
assert_equals 'labelled window stays owned across runs' 'Reviews - feature/login' "$(window_name "$win_label")"

tmux rename-window -t "$win_label" 'Override'
"$SCRIPT" -w "$win_label"
assert_equals 'manual rename beats the label' 'Override' "$(window_name "$win_label")"

tmux rename-window -t "$win_label" ''
"$SCRIPT" -w "$win_label"
assert_equals 'empty name restores the labelled name' 'Reviews - feature/login' "$(window_name "$win_label")"

win_label_plain=$(new_window "$plain_dir")
tmux set -w -t "$win_label_plain" @wname_label 'Config'
"$SCRIPT" -w "$win_label_plain"
assert_equals 'label prefixes a cwd-derived name' 'Config - not-a-repo' "$(window_name "$win_label_plain")"

# --- an explicitly named window counts as manual ---

win_named=$(tmux new-window -d -t "$SESSION" -n 'Preset' -c "$repo_main" -P -F '#{window_id}')
"$SCRIPT" -w "$win_named"
assert_equals 'window created with an explicit name is left alone' 'Preset' "$(window_name "$win_named")"

# --- active pane drives the name ---------------------------------------

win_multi=$(new_window "$repo_main")
tmux split-window -d -t "$win_multi" -c "$repo_feature"
tmux select-pane -t "$(pane_id "$win_multi" 2)"
"$SCRIPT" -w "$win_multi"
assert_equals 'active pane cwd drives the name' 'feature/login' "$(window_name "$win_multi")"
tmux select-pane -t "$(pane_id "$win_multi" 1)"
"$SCRIPT" -w "$win_multi"
assert_equals 'switching active pane updates the name' 'another' "$(window_name "$win_multi")"

# --- target selection --------------------------------------------------

tmux new-session -d -s "${SESSION}-b" -n placeholder -c "$plain_dir"
isolate_hooks "${SESSION}-b"
other=$(tmux new-window -d -t "${SESSION}-b" -c "$repo_feature" -P -F '#{window_id}')

tmux rename-window -t "$win" 'stale'
tmux set -w -t "$win" @wname_auto 'stale'
tmux rename-window -t "$other" 'stale'
tmux set -w -t "$other" @wname_auto 'stale'

"$SCRIPT" -s "$SESSION"
assert_equals 'session mode updates the named session' 'not-a-repo' "$(window_name "$win")"
assert_equals 'session mode leaves other sessions alone' 'stale' "$(window_name "$other")"

"$SCRIPT" --all
assert_equals 'all mode updates every session' 'feature/login' "$(window_name "$other")"

tmux rename-window -t "$win" 'stale'
tmux set -w -t "$win" @wname_auto 'stale'
TMUX_PANE=$(pane_id "$win" 1) "$SCRIPT"
assert_equals 'default target is the current pane window' 'not-a-repo' "$(window_name "$win")"

tmux rename-window -t "$win" 'stale'
tmux set -w -t "$win" @wname_auto 'stale'
env -u TMUX_PANE "$SCRIPT"
assert_equals 'no target outside tmux is a no-op' 'stale' "$(window_name "$win")"
"$SCRIPT" -w "$win"

# --- idempotence -------------------------------------------------------

before=$(window_name "$win_repo")
"$SCRIPT" -w "$win_repo"
"$SCRIPT" -w "$win_repo"
assert_equals 'repeated runs are stable' "$before" "$(window_name "$win_repo")"

# --- summary -----------------------------------------------------------

printf '\n%d passed, %d failed\n' "$passed" "$failed"
[ "$failed" -eq 0 ]
