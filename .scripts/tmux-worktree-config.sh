#!/bin/sh

# Shared layout constants for the tmux code-session scripts.
# Sourced (dot-sourced by tmux-setup.sh:17), never executed: the whole point
# is putting WORKTREE_COUNT into the calling shell's scope for its own
# arithmetic. A subprocess cannot set a variable in its parent shell, so
# this file still performs the assignment itself, reading the number from
# the binary rather than hardcoding it twice.

# Number of numbered Notability worktree windows (1..N)
# shellcheck disable=SC2034  # sourced by .scripts/tmux-setup.sh, which reads it
WORKTREE_COUNT="$(tmux-tools worktree-config)"
