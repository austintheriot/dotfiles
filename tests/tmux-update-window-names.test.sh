#!/bin/bash
#
# Integration tests for .scripts/tmux-update-window-names.sh
#
# Usage: ~/tests/tmux-update-window-names.test.sh

. "$(dirname "$0")/lib.sh"

SCRIPT="$DOTFILES_ROOT/.scripts/tmux-update-window-names.sh"

window_name() {
    tmux display-message -p -t "$1" '#{window_name}'
}

# Pane ids are the only reliable way to target a pane by position: "@12.1"
# is not valid pane-target syntax.
pane_id() {
    tmux list-panes -t "$1" -F '#{pane_id}' | sed -n "$2p"
}

# Windows created without -n keep tmux's automatic-rename flag on, which is
# what a window made by a keybinding looks like.
# $INERT_WINDOW_CMD rather than the default shell: a login shell's precmd
# calls the very script under test, so a shell-backed window renames itself
# a beat after creation and races every assertion here. See lib.sh.
new_window() {
    local dir=$1
    # shellcheck disable=SC2086  # command plus argument, split on purpose
    tmux new-window -d -t "$SESSION" -c "$dir" -P -F '#{window_id}' $INERT_WINDOW_CMD
}

# --- setup -------------------------------------------------------------

repo_main=$(make_repo repo-main main)
repo_feature=$(make_repo repo-feature feature/login)
plain_dir="$FIXTURES/not-a-repo"
mkdir -p "$plain_dir"

SESSION=$(new_test_session main "$FIXTURES")

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
assert_equals 'git repo yields repo/branch, not basename' 'repo-main/main' "$(window_name "$win_repo")"

win_slash=$(new_window "$repo_feature")
"$SCRIPT" -w "$win_slash"
assert_equals 'branch name with slash is preserved' 'repo-feature/feature/login' "$(window_name "$win_slash")"

git -C "$repo_main" checkout -q -b renamed
"$SCRIPT" -w "$win_repo"
assert_equals 'owned window follows branch changes' 'repo-main/renamed' "$(window_name "$win_repo")"

git -C "$repo_main" checkout -q --detach
detached_sha=$(git -C "$repo_main" rev-parse --short HEAD)
"$SCRIPT" -w "$win_repo"
assert_equals 'detached HEAD yields short sha' "repo-main/($detached_sha)" "$(window_name "$win_repo")"
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
assert_equals 'empty name reverts to automatic naming' 'repo-main/another' "$(window_name "$win_repo")"

# --- label prefix ------------------------------------------------------

win_label=$(new_window "$repo_feature")
tmux set -w -t "$win_label" @wname_label 'Reviews'
"$SCRIPT" -w "$win_label"
assert_equals 'label prefixes the automatic name' 'Reviews - repo-feature/feature/login' "$(window_name "$win_label")"

"$SCRIPT" -w "$win_label"
assert_equals 'labelled window stays owned across runs' 'Reviews - repo-feature/feature/login' "$(window_name "$win_label")"

tmux rename-window -t "$win_label" 'Override'
"$SCRIPT" -w "$win_label"
assert_equals 'manual rename beats the label' 'Override' "$(window_name "$win_label")"

tmux rename-window -t "$win_label" ''
"$SCRIPT" -w "$win_label"
assert_equals 'empty name restores the labelled name' 'Reviews - repo-feature/feature/login' "$(window_name "$win_label")"

win_label_plain=$(new_window "$plain_dir")
tmux set -w -t "$win_label_plain" @wname_label 'Config'
"$SCRIPT" -w "$win_label_plain"
assert_equals 'label prefixes a cwd-derived name' 'Config - not-a-repo' "$(window_name "$win_label_plain")"

# --- repo name prefixes the branch outside the work repos ---

win_prefixed=$(new_window "$repo_feature")
"$SCRIPT" -w "$win_prefixed"
assert_equals 'personal repo is named repo/branch' 'repo-feature/feature/login' "$(window_name "$win_prefixed")"

personal_worktree=$(make_worktree "$repo_feature" side-branch personal-wt)
win_personal_wt=$(new_window "$personal_worktree")
"$SCRIPT" -w "$win_personal_wt"
assert_equals 'worktree uses the main repo name, not the worktree directory' \
    'repo-feature/side-branch' "$(window_name "$win_personal_wt")"

win_prefixed_label=$(new_window "$repo_feature")
tmux set -w -t "$win_prefixed_label" @wname_label 'Side'
"$SCRIPT" -w "$win_prefixed_label"
assert_equals 'label sits in front of repo/branch' 'Side - repo-feature/feature/login' \
    "$(window_name "$win_prefixed_label")"

# --- work repos are named by branch alone ---

for work_repo in Notability notability-dev-tool gingerlabs-claude-plugins; do
    work_path=$(make_repo "$work_repo" work-branch)
    win_work=$(new_window "$work_path")
    "$SCRIPT" -w "$win_work"
    assert_equals "$work_repo is named by branch alone" 'work-branch' "$(window_name "$win_work")"
done

work_worktree=$(make_worktree "$FIXTURES/Notability" wt-branch Notability-2)
win_work_wt=$(new_window "$work_worktree")
"$SCRIPT" -w "$win_work_wt"
assert_equals 'work worktree is named by branch alone' 'wt-branch' "$(window_name "$win_work_wt")"

win_work_label=$(new_window "$FIXTURES/Notability")
tmux set -w -t "$win_work_label" @wname_label 'Reviews'
"$SCRIPT" -w "$win_work_label"
assert_equals 'work repo with a label stays short' 'Reviews - work-branch' "$(window_name "$win_work_label")"

# --- the bare-branch pattern is configurable ---

win_configurable=$(new_window "$repo_feature")
tmux set -w -t "$win_configurable" @wname_bare_repos 'repo-*'
"$SCRIPT" -w "$win_configurable"
assert_equals '@wname_bare_repos drops the prefix for a matching repo' 'feature/login' \
    "$(window_name "$win_configurable")"

win_unmatched=$(new_window "$FIXTURES/Notability")
tmux set -w -t "$win_unmatched" @wname_bare_repos 'nothing-*'
"$SCRIPT" -w "$win_unmatched"
assert_equals '@wname_bare_repos adds the prefix back when nothing matches' 'Notability/work-branch' \
    "$(window_name "$win_unmatched")"

# --- an explicitly named window counts as manual ---

# shellcheck disable=SC2086  # command plus argument, split on purpose
win_named=$(tmux new-window -d -t "$SESSION" -n 'Preset' -c "$repo_main" -P -F '#{window_id}' $INERT_WINDOW_CMD)
"$SCRIPT" -w "$win_named"
assert_equals 'window created with an explicit name is left alone' 'Preset' "$(window_name "$win_named")"

# --- active pane drives the name ---------------------------------------

win_multi=$(new_window "$repo_main")
tmux split-window -d -t "$win_multi" -c "$repo_feature"
tmux select-pane -t "$(pane_id "$win_multi" 2)"
"$SCRIPT" -w "$win_multi"
assert_equals 'active pane cwd drives the name' 'repo-feature/feature/login' "$(window_name "$win_multi")"
tmux select-pane -t "$(pane_id "$win_multi" 1)"
"$SCRIPT" -w "$win_multi"
assert_equals 'switching active pane updates the name' 'repo-main/another' "$(window_name "$win_multi")"

# --- target selection --------------------------------------------------

SESSION_B=$(new_test_session other "$plain_dir")
# shellcheck disable=SC2086  # command plus argument, split on purpose
other=$(tmux new-window -d -t "$SESSION_B" -c "$repo_feature" -P -F '#{window_id}' $INERT_WINDOW_CMD)

tmux rename-window -t "$win" 'stale'
tmux set -w -t "$win" @wname_auto 'stale'
tmux rename-window -t "$other" 'stale'
tmux set -w -t "$other" @wname_auto 'stale'

"$SCRIPT" -s "$SESSION"
assert_equals 'session mode updates the named session' 'not-a-repo' "$(window_name "$win")"
assert_equals 'session mode leaves other sessions alone' 'stale' "$(window_name "$other")"

"$SCRIPT" --all
assert_equals 'all mode updates every session' 'repo-feature/feature/login' "$(window_name "$other")"

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

finish
