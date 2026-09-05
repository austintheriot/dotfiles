#!/bin/bash
#
# Integration tests for .claude/hooks/notify.sh
#
# The hook shells out to aerospace (to find the frontmost app) and osascript
# (to raise the notification). Both are addressed through AEROSPACE_BIN and
# OSASCRIPT_BIN, so the tests point them at stubs: one that reports whichever
# app the case wants, and one that records the arguments it was called with.
#
# Usage: ~/tests/notify.test.sh

. "$(dirname "$0")/lib.sh"

SCRIPT="$DOTFILES_ROOT/.claude/hooks/notify.sh"
STUB_DIR="$FIXTURES/stubs"
CALLS="$FIXTURES/osascript-calls"
mkdir -p "$STUB_DIR"

cat > "$STUB_DIR/osascript" <<'STUB'
#!/bin/sh
printf '%s\n' "$*" >> "$CALLS"
STUB

cat > "$STUB_DIR/aerospace" <<'STUB'
#!/bin/sh
printf '%s\n' "${FRONTMOST_APP:-}"
STUB

chmod +x "$STUB_DIR/osascript" "$STUB_DIR/aerospace"

# Runs the hook with the stubs wired in and prints whatever osascript received.
# Empty output means the hook suppressed the notification.
run_notify() {
    local frontmost=$1 kind=${2:-stop} dir=${3:-$FIXTURES}
    : > "$CALLS"
    # env wants its options before any assignment, otherwise -u is read as the
    # command name and the run dies with 127.
    ( cd "$dir" && env -u TMUX -u TMUX_PANE \
        AEROSPACE_BIN="$STUB_DIR/aerospace" \
        OSASCRIPT_BIN="$STUB_DIR/osascript" \
        CALLS="$CALLS" \
        FRONTMOST_APP="$frontmost" \
        "$SCRIPT" "$kind" </dev/null >/dev/null 2>&1 )
    cat "$CALLS"
}

# Same, but inside a test tmux pane.
run_notify_in_pane() {
    local frontmost=$1 session=$2 pane=$3
    : > "$CALLS"
    ( cd "$FIXTURES" && in_session "$session" "$pane" env \
        AEROSPACE_BIN="$STUB_DIR/aerospace" \
        OSASCRIPT_BIN="$STUB_DIR/osascript" \
        CALLS="$CALLS" \
        FRONTMOST_APP="$frontmost" \
        "$SCRIPT" stop </dev/null >/dev/null 2>&1 )
    cat "$CALLS"
}

# --- focus suppression outside tmux ------------------------------------

assert_equals 'suppressed when Alacritty is frontmost and we are not in tmux' \
    '' "$(run_notify Alacritty)"
assert_contains 'fires when another app is frontmost' \
    'display notification' "$(run_notify Safari)"
assert_contains 'fires when aerospace reports nothing' \
    'display notification' "$(run_notify '')"

# --- focus suppression inside tmux -------------------------------------

SESSION=$(new_test_session main "$FIXTURES")
window=$(tmux new-window -d -t "$SESSION" -c "$FIXTURES" -P -F '#{window_id}')
tmux split-window -d -t "$window" -c "$FIXTURES"
target_window "$window"
active_pane=$(tmux display-message -p -t "$window" '#{pane_id}')
idle_pane=$(tmux list-panes -t "$window" -F '#{pane_id}' | grep -v "^$active_pane$" | head -n1)

assert_equals 'suppressed in the active pane of the active window' \
    '' "$(run_notify_in_pane Alacritty "$SESSION" "$active_pane")"
assert_contains 'fires from an inactive pane' \
    'display notification' "$(run_notify_in_pane Alacritty "$SESSION" "$idle_pane")"

other_window=$(tmux new-window -d -t "$SESSION" -c "$FIXTURES" -P -F '#{window_id}')
background_pane=$(tmux list-panes -t "$other_window" -F '#{pane_id}' | head -n1)
assert_contains 'fires from a pane in a non-active window' \
    'display notification' "$(run_notify_in_pane Alacritty "$SESSION" "$background_pane")"

# --- titles per kind ---------------------------------------------------

assert_contains 'stop uses the finished title' 'Claude finished' "$(run_notify Safari stop)"
assert_contains 'notification uses the needs-input title' \
    'Claude needs input' "$(run_notify Safari notification)"
assert_contains 'an unknown kind falls back to a plain title' \
    'with title "Claude"' "$(run_notify Safari something-else)"

# --- message body ------------------------------------------------------

named_dir="$FIXTURES/my-project"
mkdir -p "$named_dir"
assert_contains 'the message is the basename of the working directory' \
    'notification "my-project"' "$(run_notify Safari stop "$named_dir")"

quoted_dir="$FIXTURES/say \"hi\""
mkdir -p "$quoted_dir"
assert_contains 'double quotes in the directory name are escaped' \
    '\"hi\"' "$(run_notify Safari stop "$quoted_dir")"

# --- exit status -------------------------------------------------------

( cd "$FIXTURES" && env -u TMUX -u TMUX_PANE AEROSPACE_BIN="$STUB_DIR/aerospace" \
    OSASCRIPT_BIN="$STUB_DIR/osascript" CALLS="$CALLS" FRONTMOST_APP=Safari \
    "$SCRIPT" stop </dev/null >/dev/null 2>&1 )
assert_equals 'exits zero after firing' '0' "$?"

printf 'some hook payload' | ( cd "$FIXTURES" && env -u TMUX -u TMUX_PANE \
    AEROSPACE_BIN="$STUB_DIR/aerospace" OSASCRIPT_BIN="$STUB_DIR/osascript" \
    CALLS="$CALLS" FRONTMOST_APP=Alacritty "$SCRIPT" stop >/dev/null 2>&1 )
assert_equals 'drains stdin and exits zero when suppressed' '0' "$?"

finish
