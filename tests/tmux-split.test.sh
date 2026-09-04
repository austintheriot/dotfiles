#!/bin/bash
#
# Integration tests for .scripts/tmux-split.sh
#
# The script is sourced, not executed, and splits whatever pane $TMUX_PANE
# points at. Each case sources it against a fresh single-pane window and counts
# the panes that result.
#
# Every layout defaults to DEFAULT_VERTICAL_SPLITS (2). An unknown layout must
# leave the sourcing shell alive, which is why show_usage returns rather than
# exits.
#
# Usage: ~/tests/tmux-split.test.sh

. "$(dirname "$0")/lib.sh"

SCRIPT="$DOTFILES_ROOT/.scripts/tmux-split.sh"
SESSION=$(new_test_session main "$FIXTURES")

pane_count() {
    tmux list-panes -t "$1" -F '#{pane_id}' | wc -l | tr -d ' '
}

fresh_window() {
    tmux new-window -d -t "$SESSION" -c "$FIXTURES" -P -F '#{window_id}'
}

first_pane() {
    tmux list-panes -t "$1" -F '#{pane_id}' | head -n1
}

# The script uses `for ((...))`, which is bash and zsh syntax but not POSIX sh,
# so it is sourced from zsh to match how .zshrc actually loads it.
split_in_new_window() {
    local window pane count
    window=$(fresh_window)
    target_window "$window"
    pane=$(first_pane "$window")
    # Arguments go through as positionals, not interpolated into the command
    # string: layout names include "|" and "\", which the shell would otherwise
    # read as operators.
    in_pane "$pane" zsh -c 'source "$1" "${@:2}"' zsh "$SCRIPT" "$@" >/dev/null 2>&1
    count=$(pane_count "$window")
    tmux kill-window -t "$window" 2>/dev/null
    printf '%s' "$count"
}

# --- layouts -----------------------------------------------------------

assert_equals 'terms with no count gives 2 panes' '2' "$(split_in_new_window terms)"
assert_equals 'terms honours an explicit count' '3' "$(split_in_new_window terms 3)"
assert_equals 'terms with a count of 1 does not split' '1' "$(split_in_new_window terms 1)"

assert_equals 'editor layout with no count gives 1 editor plus 2 terminals' \
    '3' "$(split_in_new_window '|')"
assert_equals 'backslash is an alias for the editor layout' '3' "$(split_in_new_window '\')"
assert_equals 'code is an alias for the editor layout' '3' "$(split_in_new_window code)"
assert_equals 'editor layout honours an explicit terminal count' \
    '5' "$(split_in_new_window '|' 4)"

assert_equals 'main-above-two-below builds 3 panes' '3' "$(split_in_new_window - 2 2)"
assert_equals 'underscore is an alias for main-above-two-below' '3' "$(split_in_new_window _ 2 2)"
assert_equals 'main-above-two-below honours explicit counts' '5' "$(split_in_new_window - 3 3)"

# --- the editor layout leaves focus on the editor ----------------------

window=$(fresh_window)
target_window "$window"
pane=$(first_pane "$window")
in_pane "$pane" zsh -c 'source "$1" "$2"' zsh "$SCRIPT" '|' >/dev/null 2>&1
assert_equals 'editor layout focuses the leftmost pane' "$pane" \
    "$(tmux display-message -p -t "$window" '#{pane_id}')"
tmux kill-window -t "$window" 2>/dev/null

# --- unknown layouts ---------------------------------------------------

window=$(fresh_window)
target_window "$window"
pane=$(first_pane "$window")
usage=$(in_pane "$pane" zsh -c 'source "$1" "$2"' zsh "$SCRIPT" bogus-layout 2>&1)
status=$?
assert_equals 'an unknown layout exits non-zero' '1' "$status"
assert_contains 'an unknown layout prints usage' 'Available layouts' "$usage"
assert_equals 'an unknown layout splits nothing' '1' "$(pane_count "$window")"
tmux kill-window -t "$window" 2>/dev/null

window=$(fresh_window)
target_window "$window"
pane=$(first_pane "$window")
in_pane "$pane" zsh -c 'source "$1"' zsh "$SCRIPT" >/dev/null 2>&1
assert_equals 'no layout argument splits nothing' '1' "$(pane_count "$window")"
tmux kill-window -t "$window" 2>/dev/null

# --- usage must not take the sourcing shell down with it ---------------
#
# The script is sourced into an interactive shell, so `exit` in show_usage
# would close the user's terminal on a typo'd layout name. These two check that
# the shell reaches the statement after the `source`.

window=$(fresh_window)
target_window "$window"
pane=$(first_pane "$window")
survived=$(in_pane "$pane" zsh -c \
    'source "$1" "$2" >/dev/null 2>&1; print -r -- SURVIVED' zsh "$SCRIPT" bogus-layout)
assert_equals 'an unknown layout leaves the sourcing shell alive' 'SURVIVED' "$survived"

survived=$(in_pane "$pane" zsh -c \
    'source "$1" >/dev/null 2>&1; print -r -- SURVIVED' zsh "$SCRIPT")
assert_equals 'no layout argument leaves the sourcing shell alive' 'SURVIVED' "$survived"
tmux kill-window -t "$window" 2>/dev/null

finish
