tmux new-session -d -s code
tmux split-window -h
tmux split-window -v 
tmux select-pane -L
tmux attach -t code
