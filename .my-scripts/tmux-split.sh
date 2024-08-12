#!/bin/sh

# split the attached tmux session into pre-configured panes

# Function to split tmux panes vertical-terminal-style
split_tmux_terminal_panes() {
  if [[ $1 = "2" ]]; then
    tmux split-window -v
    tmux select-pane -U
  elif [[ $1 = "4" ]]; then
    tmux split-window -v
    tmux split-window -v
    tmux select-pane -D
    tmux split-window -v
    tmux select-pane -U
  else
    echo "Invalid number of splits: $NUM_SPLITS"
  fi
}


SPLIT_TYPE=$1

# Fallback to 4
NUM_SPLITS=${2:-4}

# split into 4 vertically stacked terminals, select the top-most one
if [[ $SPLIT_TYPE = "terms" ]]; then
  split_tmux_terminal_panes "$NUM_SPLITS"
fi

 # splits pane into left code editor and two side terminals, select the leftmost one
if [[ $SPLIT_TYPE = "code" ]]; then
  tmux split-window -h
  split_tmux_terminal_panes "$NUM_SPLITS"
  tmux select-pane -L
fi 




