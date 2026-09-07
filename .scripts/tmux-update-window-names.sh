#!/bin/sh
#
# Name tmux windows after what they are actually pointed at.
#
# The computation moved to the tmux-tools binary: this file's 241 lines
# spawned 17 processes per invocation and ran on every prompt draw. The
# binary's naming precedence, label handling and bare-repo glob list are
# unchanged and documented in crates/tmux-core/src/naming.rs.
#
# Usage:
#   tmux-update-window-names.sh                 the current window, or
#                                               nothing when run outside tmux
#   tmux-update-window-names.sh -a              every window in every session
#   tmux-update-window-names.sh -s <session>    every window in one session

exec tmux-tools name-windows "$@"
