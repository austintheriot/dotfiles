#!/bin/bash
#
# Confirms the tmux.conf / tmux-<platform>.conf split preserves the config's
# actual behavior: the file parses cleanly, the shared keybindings and hooks
# resolve, and the platform variant's clipboard binding is the one in effect.
#
# tmux.conf is the shared, platform-neutral file. tmux-mac.conf and
# tmux-linux.conf hold the clipboard commands and both ship on the single
# branch, sourced by platform at runtime.
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

# TMUX_TMPDIR under the fixture directory, which lib.sh removes at exit. tmux
# 3.4 does not unlink a socket on kill-server, so without this every run left
# its socket file in the shared /tmp/tmux-<uid>/ forever. The EXIT trap kills
# through this same wrapper, so it reaches the real server rather than a path
# that no longer exists.
# A SHORT directory directly under /tmp, not under $FIXTURES. A Unix socket
# path is capped at 104 bytes on macOS (sizeof sun_path, measured), and
# $FIXTURES lives under the long /var/folders/.../T/ prefix, so a socket
# there came to 111 bytes and tmux failed with "File name too long". Removed
# in the EXIT trap after the kill, so the server dies before its directory.
SOCKET_DIR=$(mktemp -d /tmp/tmux-test-XXXXXX)
tmux_t() { TMUX_TMPDIR="$SOCKET_DIR" tmux -L "$SOCKET" "$@"; }

extra_cleanup() {
    tmux_t kill-server 2>/dev/null
    rm -rf "$SOCKET_DIR"
    cleanup
}
trap extra_cleanup EXIT
trap 'extra_cleanup; exit 130' INT
trap 'extra_cleanup; exit 143' TERM HUP

parse_errors=$(tmux_t -f "$CONFIG" new-session -d -s "$SESSION" -c "$FIXTURES" 2>&1 >/dev/null)
assert_equals 'the split config parses with no errors' '' "$parse_errors"

# PageUp, not an arrow key: Claude Code binds Up/Down to prompt history, so
# an arrow translation cycles prompts instead of scrolling. Assert the page
# key so a revert to arrows fails here rather than in a live pane.
# S-Up, not PageUp. tmux.conf:92-95 states the reason: a page key moves a
# half screen per notch, so the wheel is bound to Shift+Up and Shift+Down as
# scroll-by-one-line. This assertion named PageUp and went stale when that
# changed, failing against a config that was behaving as designed.
assert_contains 'wheel-scroll forwarding survives the split' 'S-Up' \
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

# Both variants ship on the single branch, so the one for the other platform
# must be here too. This assertion is what now catches one going missing.
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

# --- the suite leaves no socket file behind --------------------------------
#
# tmux 3.4 does NOT unlink its socket on kill-server (verified: new-session,
# kill-server, the file remains). So every `tmux -L <unique>` a test run
# created stayed in the shared directory forever: 1435 dead sockets were
# found there, six per run across this suite and the tmux-tools tests, for
# weeks. Killed explicitly here so the assertion has something to observe;
# the EXIT trap's kill becomes a no-op.
tmux_t kill-server 2>/dev/null
assert_succeeds 'the suite socket is not left in the shared tmux directory' \
    test ! -e "/tmp/tmux-$(id -u)/$SOCKET"

finish
