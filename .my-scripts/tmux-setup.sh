#!/bin/sh

# Setup tmux session for code projects with multiple windows

# Return early if already inside a tmux session
if [ -n "$TMUX" ]; then
    echo "Already inside a tmux session. Please detach first."
    return
fi

SESSION_NAME="${1:-code}"

# Check if session already exists
if tmux has-session -t $SESSION_NAME 2>/dev/null; then
    echo "Session '$SESSION_NAME' already exists. Attaching..."
    tmux attach -t $SESSION_NAME
    return
fi

# Helper function to create editor with terminals layout (sp code 4)
setup_code_layout() {
    local window=$1
    tmux split-window -t $SESSION_NAME:$window -h -c "#{pane_current_path}"
    tmux split-window -t $SESSION_NAME:$window -v -c "#{pane_current_path}"
    tmux split-window -t $SESSION_NAME:$window -v -c "#{pane_current_path}"
    tmux split-window -t $SESSION_NAME:$window -v -c "#{pane_current_path}"
    tmux select-pane -t $SESSION_NAME:$window.1  # Focus editor pane
}

# Windows are left unnamed so tmux-update-window-names.sh can name them after
# their git branch. Windows that a keybinding selects by name get a @wname_label
# instead, which keeps a stable prefix in front of the branch.
create_window() {
    local window=$1 dir=$2 label=${3:-}

    if [ "$window" = "1" ]; then
        tmux new-session -d -s $SESSION_NAME -c "$dir"
    else
        tmux new-window -t $SESSION_NAME -c "$dir"
    fi

    [ -z "$label" ] || tmux set -w -t $SESSION_NAME:$window @wname_label "$label"
    setup_code_layout $window
}

# Create the session and its windows (Notability worktrees first)
create_window 1 ~/Documents/code/Notability/1
create_window 2 ~/Documents/code/Notability/2
create_window 3 ~/Documents/code/Notability/3
create_window 4 ~/Documents/code/Notability/4
create_window 5 ~/Documents/code/Notability/5
create_window 6 ~/Documents/code/Notability/6
create_window 7 ~/Documents/code/Notability/7
create_window 8 ~/Documents/code/Notability/8
create_window 9 ~/Documents/code/Notability/9
create_window 10 ~/Documents/code/Notability/reviews "Reviews"
create_window 11 ~/Documents/code/Notability/staging "Staging"
create_window 12 ~ "Config"
create_window 13 ~/Documents/code "Other"
create_window 14 ~/Documents/code/gingerlabs-claude-plugins "Plugins"
create_window 15 ~/Documents/code/notability-dev-tool "DevTool"

# Update window names with git branches
~/.my-scripts/tmux-update-window-names.sh -s $SESSION_NAME

# Select the first window
tmux select-window -t $SESSION_NAME:1

# Attach to the session
tmux attach -t $SESSION_NAME
