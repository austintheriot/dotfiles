#!/bin/sh

# Update tmux window names with current git branches

SESSION_NAME="${1:-code}"

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

# Update Notability work windows (1-4)
update_window_if_exists() {
    local window_num=$1
    local dir=$2

    if tmux list-windows -t $SESSION_NAME -F '#I' | grep -q "^${window_num}$"; then
        local branch=$(get_branch "$dir")
        tmux rename-window -t $SESSION_NAME:$window_num "$window_num - $branch"
    fi
}

update_window_if_exists 1 ~/Documents/Code/Notability
update_window_if_exists 2 ~/Documents/Code/Notability-My-Work-2
update_window_if_exists 3 ~/Documents/Code/Notability-My-Work-3
update_window_if_exists 4 ~/Documents/Code/Notability-My-Work-4
update_window_if_exists 5 ~/Documents/Code/Notability-Staging
update_window_if_exists 6 ~/Documents/Code/Notability-Reviews
