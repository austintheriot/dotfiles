#!/bin/zsh

# Setup tmux session for code projects with multiple windows
#
# Sourced from .zshrc (`alias se`), never executed: the early `return`
# statements below need a calling shell to return to.

# Return early if already inside a tmux session
if [ -n "$TMUX" ]; then
    echo "Already inside a tmux session. Please detach first."
    return
fi

SESSION_NAME="${1:-code}"

# Shared layout constants (WORKTREE_COUNT)
. ~/.my-scripts/tmux-worktree-config.sh

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

next_window=$((WORKTREE_COUNT + 1))

# The named windows follow the numbered worktrees, so their indexes move with
# WORKTREE_COUNT.
create_named_window() {
    create_window $next_window "$1" "$2"
    next_window=$((next_window + 1))
}

# Create the session and its windows (Notability worktrees first)
worktree=1
while [ $worktree -le $WORKTREE_COUNT ]; do
    create_window $worktree ~/Documents/code/Notability/$worktree
    worktree=$((worktree + 1))
done

create_named_window ~/Documents/code/Notability/reviews "Reviews"
create_named_window ~/Documents/code/Notability/staging "Staging"
create_named_window ~ "Config"
create_named_window ~/Documents/code "Other"
create_named_window ~/Documents/code/gingerlabs-claude-plugins "Plugins"
create_named_window ~/Documents/code/notability-dev-tool "DevTool"

# Update window names with git branches
~/.my-scripts/tmux-update-window-names.sh -s $SESSION_NAME

# Select the first window
tmux select-window -t $SESSION_NAME:1

# Attach to the session
tmux attach -t $SESSION_NAME
