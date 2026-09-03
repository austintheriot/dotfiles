#!/bin/bash
#
# Confirms the tmux.conf / tmux-common.conf split preserves the config's
# actual behavior: the file parses cleanly, and specific keybindings/hooks
# that move into tmux-common.conf still resolve when tmux loads the real,
# split ~/.config/tmux/tmux.conf.
#
# Runs against the real installed config file, on a throwaway tmux SERVER
# (its own -L socket), not the developer's live server. `-f` only takes
# effect when tmux starts a fresh server -- passing it to a client of an
# already-running server (the one this test itself runs inside) is silently
# ignored, so a shared-socket test would assert on stale global state
# instead of the file actually under test.
#
# Usage: ~/tests/tmux-conf-split.test.sh

. "$(dirname "$0")/lib.sh"

CONFIG="$DOTFILES_ROOT/.config/tmux/tmux.conf"
SOCKET="tmux-conf-split-test-$$"
SESSION=$(session_name conf-split)

tmux_t() { tmux -L "$SOCKET" "$@"; }

extra_cleanup() {
    tmux_t kill-server 2>/dev/null
    cleanup
}
trap extra_cleanup EXIT
trap 'extra_cleanup; exit 130' INT
trap 'extra_cleanup; exit 143' TERM HUP

parse_errors=$(tmux_t -f "$CONFIG" new-session -d -s "$SESSION" -c "$FIXTURES" 2>&1 >/dev/null)
assert_equals 'the split config parses with no errors' '' "$parse_errors"

assert_contains 'wheel-scroll forwarding survives the split' 'Up Up Up' \
    "$(tmux_t list-keys -T root)"

assert_contains 'the window-naming after-new-window hook survives the split' \
    'tmux-update-window-names.sh' "$(tmux_t show-hooks -g)"

assert_equals 'mouse mode is on' 'on' "$(tmux_t show-options -g -v mouse)"

yank_binding=$(tmux_t list-keys -T copy-mode-vi | grep "copy-pipe")
assert_contains 'the platform yank command is present' 'xclip' "$yank_binding"

finish
