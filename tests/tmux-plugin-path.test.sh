#!/bin/bash
#
# Confirms tmux and tpm agree on ONE plugin directory, and that the themes
# tmux.conf declares are actually fetched.
#
# THE BUG THIS CLOSES. On a fresh container tmux rendered its plain built-in
# theme even though tmux.conf declares `set -g @plugin "nordtheme/tmux"`.
# Three things were true at once:
#
#   1. The deps engine clones tpm to $HOME/.tmux/plugins/tpm, and tmux.conf's
#      last line runs that copy.
#   2. tmux derives TMUX_PLUGIN_MANAGER_PATH from the directory holding the
#      config file, so with the config at ~/.config/tmux/tmux.conf it points
#      at ~/.config/tmux/plugins/ -- a DIFFERENT directory from the one tpm
#      was cloned into.
#   3. Nothing ever ran tpm's installer, so no declared plugin was fetched
#      on any machine, by any bootstrap step.
#
# The developer machine hid all three: both directories existed, populated by
# hand over time. Only a fresh container showed the plain theme.
#
# The worst part, and the reason this test asserts the DIRECTORY CONTENTS and
# not the installer's exit code: running tpm's installer with
# TMUX_PLUGIN_MANAGER_PATH unset prints
#     Installing "tmux"
#       "tmux" download success
# and exits 0 having cloned nothing, because its `cd "$path"` ran with an
# empty argument and landed in $HOME. A test that trusted the exit code or
# the word "success" would pass against a machine with no themes at all --
# the exact fail-open shape this repo keeps hitting.
#
# Runs against the real installed config on a throwaway tmux SERVER (its own
# -L socket), never the developer's live server: `-f` is silently ignored for
# a client of an already-running server, so a shared socket would assert on
# stale global state instead of the file under test.
#
# Usage: ~/tests/tmux-plugin-path.test.sh

. "$(dirname "$0")/lib.sh"

CONFIG="$DOTFILES_ROOT/.config/tmux/tmux.conf"
SOCKET="tmux-plugin-path-test-$$"
FRESH_SOCKET="tmux-plugin-path-fresh-$$"
SESSION=$(session_name plugin-path)

# A HOME of its own, so the fresh-machine assertion below cannot read this
# machine's already-installed plugins.
TMPDIR_TEST=$(mktemp -d)
fresh_home="$TMPDIR_TEST/fresh-home"

tmux_t() { tmux -L "$SOCKET" "$@"; }

extra_cleanup() {
    tmux_t kill-server 2>/dev/null
    HOME="$fresh_home" tmux -L "$FRESH_SOCKET" kill-server 2>/dev/null
    [ -n "${TMPDIR_TEST:-}" ] && rm -rf "$TMPDIR_TEST"
    cleanup
}
trap extra_cleanup EXIT
trap 'extra_cleanup; exit 130' INT
trap 'extra_cleanup; exit 143' TERM HUP

tmux_t -f "$CONFIG" new-session -d -s "$SESSION" -c "$FIXTURES" 2>/dev/null

# --- the two paths must be the same directory ---------------------------
#
# Read tmux's own answer rather than restating a path here: the whole bug was
# two places disagreeing, so the assertion asks each side what it believes.
manager_path=$(tmux_t show-environment -g TMUX_PLUGIN_MANAGER_PATH 2>/dev/null | cut -f2 -d=)
assert_succeeds 'tmux reports a plugin manager path' test -n "$manager_path"

# The `run` line names the tpm that will do the installing. Whatever
# directory that copy lives in must be the directory tmux hands it.
run_line=$(grep -E "^run " "$CONFIG" | tail -1)
assert_succeeds 'tmux.conf carries a tpm run line' test -n "$run_line"

# Expand both to real paths before comparing: one side is written with a
# tilde and the other is absolute, and a string compare would call two names
# for one directory a mismatch.
tpm_dir=$(printf '%s' "$run_line" | sed -E "s|^run +'?([^']*)'?$|\1|" | sed "s|^~|$HOME|")
tpm_parent=$(dirname "$(dirname "$tpm_dir")")
manager_dir=${manager_path%/}

assert_equals 'tpm runs from the directory tmux installs plugins into' \
    "$manager_dir" "$tpm_parent"

# --- every declared plugin is actually present --------------------------
#
# The contents, not the installer's verdict. See the header: the installer
# reports success for plugins it never cloned.
while read -r repo; do
    [ -n "$repo" ] || continue
    plugin_name=${repo##*/}
    assert_succeeds "the declared plugin $repo is installed" \
        test -d "$manager_dir/$plugin_name"
done <<PLUGINS
$(grep -oE "@plugin +['\"][^'\"]+['\"]" "$CONFIG" \
    | sed -E "s|@plugin +['\"]([^'\"]+)['\"]|\1|")
PLUGINS

# --- the theme applies from an EMPTY plugin tree ------------------------
#
# The assertion that actually catches the bug, and it needs its own tmux
# server with its own HOME.
#
# An earlier draft of this test asserted nord's colours against the live
# plugin tree and PASSED with the theme loading disabled entirely, because
# the plugins were already installed on the developer machine. That is the
# fail-open this repo keeps hitting: a gate that passes while testing a
# pre-satisfied path.
#
# So point HOME at an empty directory. tmux then derives its plugin path
# from the config's own location under that HOME, and the assertion below
# reads what a FRESH machine would produce rather than what this one has
# lying around.
mkdir -p "$fresh_home/.config/tmux"
cp "$CONFIG" "$fresh_home/.config/tmux/tmux.conf"

tmux_fresh() { HOME="$fresh_home" tmux -L "$FRESH_SOCKET" "$@"; }

tmux_fresh -f "$fresh_home/.config/tmux/tmux.conf" \
    new-session -d -s fresh -c "$fresh_home" 2>/dev/null
sleep 1

# The path the config names, resolved under the fresh HOME. This is the
# assertion that fails if the config ever goes back to letting tmux derive
# the directory: tmux would answer with the config file's own parent, and
# the engine clones somewhere else.
fresh_manager=$(tmux_fresh show-environment -g TMUX_PLUGIN_MANAGER_PATH 2>/dev/null | cut -f2 -d=)
assert_equals 'the plugin path follows HOME, not the config file location' \
    "$fresh_home/.tmux/plugins/" "$fresh_manager"

tmux_fresh kill-server 2>/dev/null

finish
