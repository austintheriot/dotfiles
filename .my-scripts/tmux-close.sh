#!/bin/sh

# close all tmux panes except the currently active pane

# do not try to close tmux panes if no session is attached
if [ -z "$TMUX" ]; then
    echo "There is no tmux session currently open"
    return
fi

# Get the ID of the currently active pane
current_pane=$(tmux display-message -p '#{pane_id}')

# Get the current session ID
current_session=$(tmux display-message -p '#{session_id}')

# Get the list of all pane IDs in the current session
all_panes=$(tmux list-panes -F '#{session_id} #{pane_id}')

# Iterate over each pane ID
for pane_info in ${(f)all_panes}; do
  pane_session=$(echo $pane_info | awk '{print $1}')
  pane_id=$(echo $pane_info | awk '{print $2}')

  # If the pane is in the current session and it's not the current pane, kill it
  if [[ $pane_session == $current_session && $pane_id != $current_pane ]]; then
    tmux kill-pane -t $pane_id
  fi
done
