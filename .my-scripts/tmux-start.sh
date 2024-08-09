# #!/bin/sh

# start or attach a tmux session and configure the desired pane locations

# Return early if already inside a tmux session
# We don't want to nest tmux sessions!
if [ -n "$TMUX" ]; then
    echo "A tmux session is already open."
    return
fi

# check if session already exists
# if it doesn't exist yet, create it
SESSION_NAME=$1
if [[ "$(tmux ls | rg $SESSION_NAME)" = "" ]]; then
  # open new session 
  tmux new-session -d -s $SESSION_NAME
  # split windows--use same argument as the name of the session
  source ~/.my-scripts/tmux-split.sh
fi 

# attach the session
tmux attach -t $SESSION_NAME

