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
    local window_name=$2
    local dir=$3

    if tmux list-windows -t $SESSION_NAME -F '#I' | grep -q "^${window_num}$"; then
        local branch=$(get_branch "$dir")
        tmux rename-window -t $SESSION_NAME:$window_num "$window_name - $branch"
    fi
}

update_window_if_exists 1 "1" ~/Documents/code/Notability/1
update_window_if_exists 2 "2" ~/Documents/code/Notability/2
update_window_if_exists 3 "3" ~/Documents/code/Notability/3
update_window_if_exists 4 "4" ~/Documents/code/Notability/4
update_window_if_exists 5 "5" ~/Documents/code/Notability/5
update_window_if_exists 6 "6" ~/Documents/code/Notability/6
update_window_if_exists 7 "7" ~/Documents/code/Notability/7
update_window_if_exists 8 "8" ~/Documents/code/Notability/8
update_window_if_exists 9 "9" ~/Documents/code/Notability/9
update_window_if_exists 10 "Reviews" ~/Documents/code/Notability/reviews
update_window_if_exists 11 "Staging" ~/Documents/code/Notability/staging
