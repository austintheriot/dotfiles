#!/bin/zsh

# close all tmux panes except the currently active pane, in the current window
#
# Sourced from .zshrc (`alias c`), never executed: the early `return` below
# needs a calling shell to return to.

# do not try to close tmux panes if no session is attached
if [ -z "$TMUX" ]; then
    echo "There is no tmux session currently open"
    return
fi

# `kill-pane -a` kills every pane in the window except the target, which is the
# whole job. It stays inside the current window, so other windows and other
# sessions are untouched.
tmux kill-pane -a -t "$(tmux display-message -p '#{pane_id}')"
