#!/bin/sh
#
# Regression test for tmux-setup.sh window targeting.
#
# The after-new-window hook renames windows to branch-derived names that
# begin with "code". A bare "-t code" target matches that window name before
# the session, so new-window aims at an occupied window index instead of
# appending to the session. The trailing colon in "-t code:" forces
# session-only resolution.

SOCKET=tmux-setup-test
SESSION=code
WINDOW_NAME_COLLIDING=code/cc/some-branch

cleanup() {
    tmux -L "$SOCKET" kill-server 2>/dev/null
}
trap cleanup EXIT
cleanup

failures=0

fail() {
    echo "FAIL: $1"
    failures=$((failures + 1))
}

tmux -L "$SOCKET" new-session -d -s "$SESSION" -c "$HOME"
tmux -L "$SOCKET" rename-window -t "$SESSION:1" "$WINDOW_NAME_COLLIDING"

error=$(tmux -L "$SOCKET" new-window -t "$SESSION:" -c "$HOME" 2>&1)
[ -z "$error" ] || fail "new-window against a colliding window name: $error"

window_count=$(tmux -L "$SOCKET" list-windows -t "$SESSION" | wc -l | tr -d ' ')
[ "$window_count" = "2" ] || fail "expected 2 windows, found $window_count"

error=$(tmux -L "$SOCKET" split-window -t "$SESSION:2" -h -c "$HOME" 2>&1)
[ -z "$error" ] || fail "split-window on the new window: $error"

if grep -q 'new-window -t \$SESSION_NAME -c' "$HOME/.scripts/tmux-setup.sh"; then
    fail "tmux-setup.sh still uses the ambiguous bare -t \$SESSION_NAME target"
fi

if [ "$failures" -eq 0 ]; then
    echo "PASS"
else
    echo "$failures failure(s)"
    exit 1
fi
