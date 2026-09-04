#!/bin/zsh

# start or attach a tmux session and configure the desired pane locations
#
# Sourced from .zshrc (`alias s`), never executed: the early `return` statements
# below need a calling shell to return to.

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
  # split windows--use same argument as the name of the session.
  # Sourcing with no arguments is deliberate: a sourced script inherits the
  # caller's positional parameters, so tmux-split.sh reads $1 as its layout
  # name and $1 here is still the session name. A session name that is not a
  # layout name prints usage and skips the splits.
  source ~/.scripts/tmux-split.sh
fi 

# attach the session
tmux attach -t $SESSION_NAME
