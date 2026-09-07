#!/bin/sh
#
# Split the current tmux window into a named layout.
#
# Still sourced (`alias sp`), never executed, so the binary's exit status
# becomes the interactive shell's $?, as this script's own `return 1` did
# before. `exec` would replace the interactive shell and close the user's
# terminal, so this file must never use it.
#
# Exit 3 means "that name is not a layout". That is not an error: the caller
# passed a session name that happens not to name a layout, and tmux-start.sh
# treats 3 as "plan no splits". Exit 2 is a real usage error, such as no
# argument at all.

tmux-tools split "$@"
return $?
