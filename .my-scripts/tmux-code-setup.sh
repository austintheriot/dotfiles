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
    tmux split-window -t $SESSION_NAME:$window -h
    tmux split-window -t $SESSION_NAME:$window -v
    tmux split-window -t $SESSION_NAME:$window -v
    tmux split-window -t $SESSION_NAME:$window -v
    tmux select-pane -t $SESSION_NAME:$window.1  # Focus editor pane
}

# Create new session with first window (Notability)
tmux new-session -d -s $SESSION_NAME -n "1 - Free" -c ~/Documents/Code/Notability
setup_code_layout 1

# Create additional windows
tmux new-window -t $SESSION_NAME -n "2 - Free" -c ~/Documents/Code/Notability-My-Work-2
setup_code_layout 2

tmux new-window -t $SESSION_NAME -n "3 - Free" -c ~/Documents/Code/Notability-My-Work-3
setup_code_layout 3

tmux new-window -t $SESSION_NAME -n "4 - Free" -c ~/Documents/Code/Notability-My-Work-4
setup_code_layout 4

tmux new-window -t $SESSION_NAME -n "Staging" -c ~/Documents/Code/Notability-Staging
setup_code_layout 5

tmux new-window -t $SESSION_NAME -n "Reviews" -c ~/Documents/Code/Notability-Reviews
setup_code_layout 6

tmux new-window -t $SESSION_NAME -n "Config" -c ~
setup_code_layout 7

# Select the first window
tmux select-window -t $SESSION_NAME:1

# Attach to the session
tmux attach -t $SESSION_NAME
