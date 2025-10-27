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

# Create new session with first window (Notability)
tmux new-session -d -s $SESSION_NAME -n "1 - Free" -c ~/Documents/Code/Notability

# Create additional windows
tmux new-window -t $SESSION_NAME -n "2 - Free" -c ~/Documents/Code/Notability-My-Work-2
tmux new-window -t $SESSION_NAME -n "3 - Free" -c ~/Documents/Code/Notability-My-Work-3
tmux new-window -t $SESSION_NAME -n "4 - Free" -c ~/Documents/Code/Notability-My-Work-4
tmux new-window -t $SESSION_NAME -n "Staging" -c ~/Documents/Code/Notability-Staging
tmux new-window -t $SESSION_NAME -n "Reviews" -c ~/Documents/Code/Notability-Reviews
tmux new-window -t $SESSION_NAME -n "Config" -c ~

# Select the first window
tmux select-window -t $SESSION_NAME:1

# Attach to the session
tmux attach -t $SESSION_NAME
