#!/bin/bash
#
# Integration tests for .my-scripts/tmux-close.sh
#
# The script is sourced from zsh in real use (`alias c`), so it is sourced from
# zsh here rather than executed.
#
# Its scope is the current window, not the current session: `kill-pane -a` kills
# every pane in the target's window and leaves other windows and other sessions
# alone. The bystander assertions below pin that down.
#
# Usage: ~/tests/tmux-close.test.sh

. "$(dirname "$0")/lib.sh"

SCRIPT="$DOTFILES_ROOT/.my-scripts/tmux-close.sh"
SESSION=$(new_test_session main "$FIXTURES")

pane_count() {
    tmux list-panes -t "$1" -F '#{pane_id}' | wc -l | tr -d ' '
}

window_with_panes() {
    local count=$1 window index
    window=$(tmux new-window -d -t "$SESSION" -c "$FIXTURES" -P -F '#{window_id}')
    for ((index = 1; index < count; index++)); do
        tmux split-window -d -t "$window" -c "$FIXTURES"
    done
    printf '%s' "$window"
}

# The script bails out when $TMUX is empty, so it gets a $TMUX pointed at the
# test session rather than the cleared one `in_pane` provides.
run_close() {
    local session=$1 pane=$2
    in_session "$session" "$pane" zsh -c 'source "$1"' zsh "$SCRIPT" >/dev/null 2>&1
}

# --- closes the other panes in the current window ----------------------

window=$(window_with_panes 4)
target_window "$window"
keep=$(tmux list-panes -t "$window" -F '#{pane_id}' | sed -n 2p)
tmux select-pane -t "$keep"
run_close "$SESSION" "$keep"
assert_equals 'closes every pane except the active one' '1' "$(pane_count "$window")"
assert_equals 'the surviving pane is the one that was active' "$keep" \
    "$(tmux list-panes -t "$window" -F '#{pane_id}')"
tmux kill-window -t "$window" 2>/dev/null

# --- a single-pane window is left alone --------------------------------

window=$(window_with_panes 1)
target_window "$window"
only=$(tmux list-panes -t "$window" -F '#{pane_id}')
run_close "$SESSION" "$only"
assert_equals 'a lone pane survives' '1' "$(pane_count "$window")"
assert_equals 'the lone pane is unchanged' "$only" "$(tmux list-panes -t "$window" -F '#{pane_id}')"
tmux kill-window -t "$window" 2>/dev/null

# --- other windows are untouched ---------------------------------------

bystander=$(window_with_panes 3)
window=$(window_with_panes 3)
target_window "$window"
keep=$(tmux list-panes -t "$window" -F '#{pane_id}' | head -n1)
tmux select-pane -t "$keep"
run_close "$SESSION" "$keep"
assert_equals 'the target window is reduced to one pane' '1' "$(pane_count "$window")"
assert_equals 'a different window in the same session keeps its panes' '3' "$(pane_count "$bystander")"
tmux kill-window -t "$window" 2>/dev/null
tmux kill-window -t "$bystander" 2>/dev/null

# --- other sessions are untouched --------------------------------------

OTHER=$(new_test_session other "$FIXTURES")
tmux split-window -d -t "$OTHER" -c "$FIXTURES"
window=$(window_with_panes 3)
target_window "$window"
keep=$(tmux list-panes -t "$window" -F '#{pane_id}' | head -n1)
tmux select-pane -t "$keep"
run_close "$SESSION" "$keep"
assert_equals 'another session keeps its panes' '2' "$(pane_count "$OTHER")"
tmux kill-window -t "$window" 2>/dev/null

# --- refuses to run outside tmux ---------------------------------------

output=$(env -u TMUX -u TMUX_PANE zsh -c 'source "$1"' zsh "$SCRIPT" 2>&1)
assert_contains 'refuses to run with no tmux session' 'no tmux session' "$output"

finish
