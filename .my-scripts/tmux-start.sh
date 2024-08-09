#!/bin/sh

# start or attach a tmux session and configure the desired pane locations

# Return early if already inside a tmux session
# We don't want to nest tmux sessions!
if [ -n "$TMUX" ]; then
    echo "A tmux session is already open."
    return
fi

SESSION_NAME=$1

# if no session specified, just spin up the default tmux session
if [ "$SESSION_NAME" = "" ]; then
  # open new session with default name and no splits
  tmux new-session -A -s zsh
  return
fi

# check if session already exists
# if it doesn't exist yet, create it
if [ "$(tmux ls | rg $SESSION_NAME)" = "" ]; then
  # open new session 
  tmux new-session -d -s $SESSION_NAME
  # split windows--use same argument as the name of the session
  source ~/.my-scripts/tmux-split.sh
fi 

# attach the session
tmux attach -t $SESSION_NAME
