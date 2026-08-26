#!/bin/sh

# Update tmux window names with current git branches

SESSION_NAME="${1:-code}"

# Shared layout constants (WORKTREE_COUNT)
. ~/.my-scripts/tmux-worktree-config.sh

# Check if session exists
if ! tmux has-session -t $SESSION_NAME 2>/dev/null; then
    echo "Session '$SESSION_NAME' does not exist."
    exit 1
fi

# Helper function to get git branch for a directory
get_branch() {
    local dir=$1
    git -C "$dir" branch --show-current 2>/dev/null || echo "Free"
}

update_window_if_exists() {
    local window_num=$1
    local window_name=$2
    local dir=$3

    if tmux list-windows -t $SESSION_NAME -F '#I' | grep -q "^${window_num}$"; then
        local branch=$(get_branch "$dir")
        tmux rename-window -t $SESSION_NAME:$window_num "$window_name - $branch"
    fi
}

worktree=1
while [ $worktree -le $WORKTREE_COUNT ]; do
    update_window_if_exists $worktree "$worktree" ~/Documents/code/Notability/$worktree
    worktree=$((worktree + 1))
done

update_window_if_exists $((WORKTREE_COUNT + 1)) "Reviews" ~/Documents/code/Notability/reviews
update_window_if_exists $((WORKTREE_COUNT + 2)) "Staging" ~/Documents/code/Notability/staging
