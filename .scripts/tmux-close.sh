#!/bin/zsh

# Close all tmux panes except the currently active pane, in the current window
#
# Sourced from .zshrc (`alias c`), never executed: the early `return` below
# needs a calling shell to return to. This file keeps no return contract
# beyond that early exit: it never returns an explicit value, so falling
# through to the binary call at the end lets the binary's own exit status
# become the interactive shell's $?, the same as tmux itself did before.

# do not try to close tmux panes if no session is attached
if [ -z "$TMUX" ]; then
    echo "There is no tmux session currently open"
    return
fi

tmux-tools close
