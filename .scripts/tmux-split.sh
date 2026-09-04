#!/bin/zsh

# tmux Layout Manager
# Creates pre-configured tmux pane layouts with customizable splits
#
# Sourced from .zshrc (`alias sp`) and from tmux-start.sh, never executed:
# it splits the caller's own pane, and `return` below has to reach the calling
# shell rather than killing it.

# Constants
DEFAULT_VERTICAL_SPLITS=2
DEFAULT_HORIZONTAL_SPLITS=2

# --- Layout Functions ---

# Creates vertical splits (default: 2 panes)
create_vertical_splits() {
  local count=${1:-$DEFAULT_VERTICAL_SPLITS}
  for ((i=1; i<count; i++)); do
    tmux split-window -v
  done
  tmux select-pane -U  # Return to top pane
}

# Creates horizontal splits (default: 2 panes)
create_horizontal_splits() {
  local count=${1:-$DEFAULT_HORIZONTAL_SPLITS}
  for ((i=1; i<count; i++)); do
    tmux split-window -h
  done
}

# Creates a main area above and two panes below (default: 2x2)
create_main_above_two_below() {
  local vertical_splits=${1:-$DEFAULT_VERTICAL_SPLITS}
  local horizontal_splits=${2:-$DEFAULT_HORIZONTAL_SPLITS}

  # Create main area (vertical splits)
  create_vertical_splits "$vertical_splits"

  # Create bottom area (horizontal splits)
  tmux select-pane -D
  create_horizontal_splits "$horizontal_splits"

  # Return to main area
  tmux select-pane -U
}

# Creates multiple vertical terminals (default: 2 panes)
create_vertical_terminals() {
  local splits=${1:-$DEFAULT_VERTICAL_SPLITS}
  create_vertical_splits "$splits"
}

# Creates editor on left with terminals on right (default: 2 terminals)
create_editor_with_terminals() {
  local vertical_splits=${1:-$DEFAULT_VERTICAL_SPLITS}
  tmux split-window -h
  create_vertical_terminals "$vertical_splits"
  tmux select-pane -L  # Focus editor pane
}

# --- Main Script ---

# Validate input and show help if no arguments provided
show_usage() {
  echo "Usage: $0 <layout_type> [vertical_splits] [horizontal_splits]"
  echo "Available layouts:"
  echo "  terms    - Vertical terminals (default: $DEFAULT_VERTICAL_SPLITS)"
  echo "  | or \\   - Editor with terminals (default: $DEFAULT_VERTICAL_SPLITS)"
  echo "  - or _   - Main above with panes below (default: ${DEFAULT_VERTICAL_SPLITS}x${DEFAULT_HORIZONTAL_SPLITS})"
  # `return`, not `exit`: this script is sourced, so `exit` would close the
  # caller's interactive shell on a typo'd layout name.
  return 1
}

# Parse arguments
LAYOUT_TYPE=${1:-}
VERTICAL_SPLITS=${2:-$DEFAULT_VERTICAL_SPLITS}
HORIZONTAL_SPLITS=${3:-$DEFAULT_HORIZONTAL_SPLITS}

# Create requested layout
case $LAYOUT_TYPE in
  "terms")    create_vertical_terminals "$VERTICAL_SPLITS" ;;
  "\\"|"|")   create_editor_with_terminals "$VERTICAL_SPLITS" ;;
  "-"|"_")    create_main_above_two_below "$VERTICAL_SPLITS" "$HORIZONTAL_SPLITS" ;;
  "code")     create_editor_with_terminals "$VERTICAL_SPLITS" ;;  # Alias for editor layout
  *)          show_usage ;;
esac
