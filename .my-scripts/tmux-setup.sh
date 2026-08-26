#!/bin/sh

# Setup tmux session for code projects with multiple windows

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

# Create new session with first window (Notability worktrees)
tmux new-session -d -s $SESSION_NAME -n "1" -c ~/Documents/code/Notability/1
setup_code_layout 1

# Create the remaining numbered worktree windows
worktree=2
while [ $worktree -le $WORKTREE_COUNT ]; do
    tmux new-window -t $SESSION_NAME -n "$worktree" -c ~/Documents/code/Notability/$worktree
    setup_code_layout $worktree
    worktree=$((worktree + 1))
done

# Create the named windows after the numbered worktrees
next_window=$((WORKTREE_COUNT + 1))
add_named_window() {
    tmux new-window -t $SESSION_NAME -n "$1" -c "$2"
    setup_code_layout $next_window
    next_window=$((next_window + 1))
}

add_named_window "Reviews" ~/Documents/code/Notability/reviews
add_named_window "Staging" ~/Documents/code/Notability/staging
add_named_window "Config" ~
add_named_window "Other" ~/Documents/code
add_named_window "Plugins" ~/Documents/code/gingerlabs-claude-plugins
add_named_window "DevTool" ~/Documents/code/notability-dev-tool

# Update window names with git branches
~/.my-scripts/tmux-update-window-names.sh $SESSION_NAME

# Select the first window
tmux select-window -t $SESSION_NAME:1

# Attach to the session
tmux attach -t $SESSION_NAME
