# check for session already open
SESSION_NAME='code'
if [[ "$(tmux ls | rg $SESSION_NAME)" = "" ]]; then
  # open new session
  tmux new-session -d -s $SESSION_NAME
  source ~/.my-scripts/tmux-split.sh
fi 
tmux attach -t $SESSION_NAME

