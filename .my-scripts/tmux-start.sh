# check for session already open
SESSION_NAME='code'
if [[ "$(tmux ls | rg $SESSION_NAME)" = "" ]]; then
  # open new session
  tmux new-session -d -s $SESSION_NAME
  tmux split-window -h
  tmux split-window -v 
  tmux select-pane -L
  tmux attach -t $SESSION_NAME
else tmux new-session -d -s $SESSION_NAME
  # open existing session
  tmux attach -t $SESSION_NAME
fi


