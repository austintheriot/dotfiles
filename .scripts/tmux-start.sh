#!/bin/zsh

# start a tmux session and configure the desired pane locations
#
# Sourced from .zshrc (`alias s`), never executed: the early `return`
# statements below need a calling shell to return to.
#
# The `tmux attach` this script used to end with now lives in the alias. A
# subprocess that attaches attaches itself, and the alias runs in the
# caller's own terminal, which is the process that has to take over the TTY.
# This script prints the session to attach to on stdout and the alias
# attaches to whatever it printed, so the alias attaches under exactly the
# conditions this script used to: never when already inside tmux, and
# otherwise to the session named here.

# Return early if already inside a tmux session
# We don't want to nest tmux sessions!
if [ -n "$TMUX" ]; then
    echo "A tmux session is already open." >&2
    return
fi

SESSION_NAME=$1

# if no session specified, just spin up the default tmux session
if [ "$SESSION_NAME" = "" ]; then
  # The old `new-session -A -s zsh` both created and attached. The alias
  # does the attaching now, so creation is conditional here and `-A`'s
  # "attach if it exists" half becomes the alias attaching either way.
  if ! tmux has-session -t zsh 2>/dev/null; then
    tmux new-session -d -s zsh
  fi
  echo zsh
  return
fi

# check if session already exists
# if it doesn't exist yet, create it
if [ "$(tmux ls | rg $SESSION_NAME)" = "" ]; then
  # open new session
  tmux new-session -d -s $SESSION_NAME

  # Explicit argument, not positional-parameter inheritance. The old version
  # sourced tmux-split.sh with no arguments so its ${1:-} read THIS script's
  # $1, which is invisible at the call site and is why the parent spec called
  # this conversion unsafe. Exit 3 means the session name is not a layout,
  # which is the ordinary case and not a failure.
  tmux-tools split "$SESSION_NAME"
  split_status=$?
  if [ "$split_status" -ne 0 ] && [ "$split_status" -ne 3 ]; then
      printf 'tmux-start: splitting failed with status %s\n' "$split_status" >&2
      return "$split_status"
  fi
fi

# The alias attaches to whatever this prints.
echo $SESSION_NAME
