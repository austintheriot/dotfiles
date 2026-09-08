#!/bin/bash
#
# Integration tests for .scripts/tmux-start.sh
#
# The script is sourced from zsh in real use (`alias s`), never executed: its
# early `return` statements need a calling shell to return to. So it is sourced
# from zsh here too, and $TMUX is cleared because the script bails out
# immediately when it is set.
#
# What this suite is about: the script decides "does this session already
# exist" and then prints a name for the alias to attach to. Those two answers
# have to agree. When the existence test says yes about a session that is not
# there, the alias attaches to nothing and the user gets an error from tmux
# about a session they did not name.
#
# tmux target matching is the whole subtlety, and it is not obvious:
#
#   -t dev        matches dev-tool     (prefix match)
#   -t '=dev'     matches only dev     (exact match)
#
# So `has-session -t "$name"` is NOT sufficient on its own, which is worth
# stating because it looks sufficient. Measured on tmux 3.4: with only
# `dev-tool` running, `has-session -t dev` succeeds and `has-session -t =dev`
# fails.
#
# Usage: ~/tests/tmux-start.test.sh

. "$(dirname "$0")/lib.sh"

SCRIPT="$DOTFILES_ROOT/.scripts/tmux-start.sh"

assert_succeeds 'the script exists' test -f "$SCRIPT"

# Sourced with $TMUX cleared, the way the alias reaches it from a bare shell.
# stdout is the contract (the alias attaches to whatever is printed), so it is
# captured separately from stderr.
run_start() {
    env -u TMUX zsh -c 'source "$1" "${2-}"' zsh "$SCRIPT" "${1-}" 2>/dev/null
}

session_exists() {
    tmux has-session -t "=$1" 2>/dev/null
}

# --- a named session that does not exist yet is created ---------------------

fresh=$(session_name fresh)
track_session "$fresh"
printed=$(run_start "$fresh")
assert_equals 'a new named session is printed for the alias to attach to' \
    "$fresh" "$printed"
assert_succeeds 'the new named session was actually created' \
    session_exists "$fresh"

# --- an existing session is reused, not recreated ---------------------------

# Re-running must not create a second session or fail. The printed name is the
# same either way, so the observable difference is the window count: a second
# `new-session -d` on a live name errors, and re-running the layout split would
# add panes to a session the user has already arranged.
windows_before=$(tmux list-windows -t "=$fresh" -F '#{window_id}' | wc -l | tr -d ' ')
printed=$(run_start "$fresh")
windows_after=$(tmux list-windows -t "=$fresh" -F '#{window_id}' | wc -l | tr -d ' ')
assert_equals 'an existing session is printed unchanged' "$fresh" "$printed"
assert_equals 'an existing session is not re-split' \
    "$windows_before" "$windows_after"

# --- a session whose name is a PREFIX of a live one is still created --------

# The bug this suite exists for. With `dev-tool` running, `s dev` used to find
# it (`tmux ls | rg dev` matches the line, and `has-session -t dev` matches by
# prefix), skip creation, and print `dev` -- so the alias then ran
# `tmux attach -t dev` against a session that was never created.
#
# Reproduced before the fix: creation skipped, `dev` absent, and the alias
# left with a name it cannot attach to.
prefix_of=$(session_name prefixof)
longer="${prefix_of}-tool"
track_session "$longer"
tmux new-session -d -s "$longer" -c "$FIXTURES"
track_session "$prefix_of"

printed=$(run_start "$prefix_of")
assert_equals 'a session named as a prefix of a live one is printed' \
    "$prefix_of" "$printed"
assert_succeeds 'a session named as a prefix of a live one is actually created' \
    session_exists "$prefix_of"
assert_succeeds 'the longer session it is a prefix of is left alone' \
    session_exists "$longer"

# --- a name that would break the old ripgrep call --------------------------

# `tmux ls | rg $SESSION_NAME` was unquoted, so a name beginning with a dash
# was read by ripgrep as a flag rather than as a pattern. Measured against the
# old form with a dash-leading name: `rg: unrecognized flag --`. A name like
# that is not one anyone types on purpose, but the failure the user saw came
# from a tool they never invoked and named a flag they never passed, which is
# the worst shape a diagnostic can take.
#
# tmux itself restricts session names, so this does not assert the session is
# created -- only that the script does not fail inside a matcher first.
# Whatever tmux decides is tmux's answer to give.
dashed=$(session_name dashed)
track_session "-$dashed"
output=$(env -u TMUX zsh -c 'source "$1" "$2"' zsh "$SCRIPT" "-$dashed" 2>&1)
assert_equals 'a dash-leading name produces no ripgrep diagnostic' \
    '' "$(printf '%s' "$output" | grep -o 'rg:.*' || true)"

# --- the default (no argument) path ----------------------------------------

# `s` with no argument uses the fixed name `zsh`. That path had the same
# prefix-matching bug in a quieter form: `has-session -t zsh` matches
# `zsh-other`, so a user with any zsh-prefixed session running would have
# creation skipped and then attach to a `zsh` that does not exist.
#
# Not run against the real `zsh` session, which would touch the developer's
# own layout. The assertion is on the source text instead: the exact-match
# spelling is what makes the difference, and it is cheap to pin.
assert_contains 'the default path tests for an exact session name' \
    "has-session -t '=zsh'" "$(cat "$SCRIPT")"

# --- no substring session test survives in the script ----------------------

# The mechanism, not just its symptoms: `tmux ls` piped to a matcher is the
# shape that cannot answer "does a session with exactly this name exist", so
# its absence is the assertion.
# Comment lines are stripped first. The fix's own comment explains the old
# `tmux ls` form, so a grep over the raw file matches the explanation and
# reports the bug as still present -- which is what happened when this
# assertion was written with a `\s` class that BRE does not support.
assert_equals 'the script does not decide session existence by matching tmux ls output' \
    '' "$(sed 's/#.*//' "$SCRIPT" | grep -n 'tmux ls' || true)"

finish
