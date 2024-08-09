# #!/bin/sh

# split the attached tmux session into pre-configured panes

SPLIT_TYPE=$1

# split into 4 vertically stacked terminals, select the top-most one
if [[ $SPLIT_TYPE = "terms" ]]; then
  tmux split-window -v
  tmux split-window -v
  tmux select-pane -D
  tmux split-window -v
  tmux select-pane -U
fi 

 # splits pane into left code editor and two side terminals, select the leftmost one
if [[ $SPLIT_TYPE = "code" ]]; then
  tmux split-window -h
  tmux split-window -v 
  tmux select-pane -L
fi 




