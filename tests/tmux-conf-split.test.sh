#!/bin/bash
#
# Confirms the tmux.conf / tmux-<platform>.conf split preserves the config's
# actual behavior: the file parses cleanly, the shared keybindings and hooks
# resolve, and the platform variant's clipboard binding is the one in effect.
#
# tmux.conf is the shared file, byte-identical on both branches.
# tmux-mac.conf and tmux-linux.conf hold the clipboard commands and both ship
# on both branches, so the drift check covers them too.
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

# --- the platform variant is the one that took effect ------------------------

# Read from the variant file for THIS platform rather than hardcoding xclip
# or pbcopy: this test file is itself shared between the mac and linux
# branches, so naming either platform's command here would recreate the exact
# drift the whole mechanism exists to catch.
. "$DOTFILES_ROOT/.scripts/platform.sh"
VARIANT=$(platform_variant "$CONFIG")

assert_succeeds 'this platform has a tmux variant file' test -f "$VARIANT"

# Both variants ship on both branches, so the one for the other platform must
# be here too -- that is what keeps it inside `config check`.
other_platform=mac; [ "$DOTFILES_PLATFORM" = mac ] && other_platform=linux
assert_succeeds "the $other_platform variant also ships here" \
    test -f "$DOTFILES_ROOT/.config/tmux/tmux-$other_platform.conf"

expected_yank_cmd=$(sed -n "s/.*copy-mode-vi 'y'.*copy-pipe \"\([^\"]*\)\".*/\1/p" "$VARIANT")
# Guards against the assertion below passing vacuously: assert_contains with
# an empty needle matches anything, which is exactly what happened when the
# clipboard bindings moved out of tmux.conf and this still read from it.
assert_succeeds 'the variant names a yank command' test -n "$expected_yank_cmd"

yank_binding=$(tmux_t list-keys -T copy-mode-vi | grep "copy-pipe")
assert_contains 'the platform yank command is in effect' "$expected_yank_cmd" "$yank_binding"

# The other platform's command must NOT be bound. Sourcing both variants
# would leave whichever loaded last in charge, silently, on both machines.
other_yank_cmd=$(sed -n "s/.*copy-mode-vi 'y'.*copy-pipe \"\([^\"]*\)\".*/\1/p" \
    "$DOTFILES_ROOT/.config/tmux/tmux-$other_platform.conf")
assert_equals "the $other_platform yank command is not bound" \
    '' "$(printf '%s' "$yank_binding" | grep -oF "$other_yank_cmd")"

finish
