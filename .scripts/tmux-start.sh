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
  # `-t '=zsh'` and not `-t zsh`: tmux target matching is by PREFIX, so the
  # bare form succeeds for any session whose name starts with "zsh". A user
  # with `zsh-other` running would have creation skipped here and the alias
  # would then attach to a `zsh` that does not exist. Measured on tmux 3.4.
  if ! tmux has-session -t '=zsh' 2>/dev/null; then
    tmux new-session -d -s zsh
  fi
  echo zsh
  return
fi

# check if session already exists
# if it doesn't exist yet, create it
#
# `has-session -t '=name'` rather than matching `tmux ls` output. The old form
# was `[ "$(tmux ls | rg $SESSION_NAME)" = "" ]`, which got this wrong three
# ways at once:
#
#   - it matched a SUBSTRING of the whole listing line, so `s dev` found a
#     running `dev-tool`, skipped creation, and printed `dev` for the alias to
#     attach to. tmux then reported no such session, naming one the user had
#     not typed.
#   - it treated the name as a regex, so any name with regex metacharacters
#     matched something other than itself.
#   - $SESSION_NAME was unquoted, so a name beginning with a dash was read by
#     ripgrep as a flag and the user got `rg: oo: No such file or directory`
#     from a tool they never invoked.
#
# The `=` prefix is required and is the part that is easy to miss: plain
# `-t "$SESSION_NAME"` still matches by prefix, so it fixes the regex and
# quoting problems while leaving the original bug in place.
if ! tmux has-session -t "=$SESSION_NAME" 2>/dev/null; then
  # open new session
  tmux new-session -d -s "$SESSION_NAME"

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
echo "$SESSION_NAME"
