# shellcheck shell=bash
# Shared harness for the dotfiles integration tests.
#
# A test file sources this, calls `assert_equals` (and friends), and exits with
# `finish`. Every test runs against real tmux sessions and real git repos: the
# scripts under test are almost entirely orchestration of tmux and git, so
# stubbing those out would only test the stubs.
#
# Sourcing this file sets up a per-run temp directory and an EXIT trap that
# tears down anything registered with `track_session`.

set -u

# Fixtures are real git repositories. If a caller exports a git environment (a
# pre-commit hook does exactly this), every fixture `git init` and `git commit`
# would target that repository instead of the fixture.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_OBJECT_DIRECTORY

DOTFILES_ROOT=${DOTFILES_ROOT:-$HOME}

# The real interpreter, resolved once for every suite that sources this.
#
# The `python3` on PATH is a pyenv shim: a bash script that execs
# `pyenv exec python3`, which re-resolves the version and execs again.
# Measured at 750ms per call against 40ms for the interpreter it eventually
# reaches. A suite that spells `python3` pays that on every call, and
# run-all.sh gates a pre-commit hook.
#
# `pyenv which` is itself a shim call, so it is paid once here and only where
# pyenv is actually in use. Everything degrades to whatever `python3` means on
# this machine. /usr/bin/python3 is deliberately not a fallback: on macOS it is
# an xcrun stub that does not run without the Xcode command line tools.
resolve_python() {
    local candidate
    if command -v pyenv >/dev/null 2>&1; then
        candidate=$(pyenv which python3 2>/dev/null)
        if [ -n "$candidate" ] && [ -x "$candidate" ]; then
            printf '%s' "$candidate"
            return
        fi
    fi
    command -v python3 2>/dev/null || command -v python 2>/dev/null || true
}
PYTHON_BIN=$(resolve_python)
export PYTHON_BIN

FIXTURES=$(mktemp -d "${TMPDIR:-/tmp}/dotfiles-test-XXXXXX")
TEST_NAME=$(basename "${BASH_SOURCE[1]:-$0}" .test.sh)

passed=0
failed=0

# Tracked in a file, not an array: `new_test_session` is normally called inside
# a command substitution, and a subshell cannot append to the parent's array.
SESSION_LIST="$FIXTURES/.sessions"
: > "$SESSION_LIST"

# tmux sessions are named with the pid so a test run cannot collide with a live
# session or with a second run happening at the same time.
session_name() {
    printf '%s-%s-%s' "$TEST_NAME" "$$" "${1:-main}"
}

track_session() {
    printf '%s\n' "$1" >> "$SESSION_LIST"
}

cleanup() {
    local session
    while read -r session; do
        [ -z "$session" ] || tmux kill-session -t "$session" 2>/dev/null
    done < "$SESSION_LIST"

    # Belt and braces: kill anything this run named, in case a session was
    # created without being tracked.
    tmux list-sessions -F '#{session_name}' 2>/dev/null \
        | grep "^${TEST_NAME}-$$-" \
        | while read -r session; do tmux kill-session -t "$session" 2>/dev/null; done

    rm -rf "$FIXTURES"
}
# EXIT alone is not enough. A killed run (Ctrl+C, a timeout, a pre-commit hook
# giving up) skips the EXIT trap in bash and leaves its tmux sessions behind,
# and enough orphaned sessions will bog the tmux server down. cleanup is
# idempotent, so running it twice is harmless.
trap cleanup EXIT
trap 'cleanup; exit 130' INT
trap 'cleanup; exit 143' TERM HUP

# The real config installs hooks globally. A test session inherits them, and
# they would race any assertion that reads a window name, so every test session
# overrides them and drives the scripts explicitly instead.
#
# The override is an explicit no-op at index [0], not the empty string. A hook
# set to '' still leaves the inherited global array entry in place, so the
# global hook goes on firing and the isolation this function promises is not
# delivered.
isolate_hooks() {
    local session=$1 hook
    for hook in after-new-window after-split-window after-select-window \
                after-select-pane after-kill-pane client-session-changed; do
        tmux set-hook -t "$session" "${hook}[0]" 'run-shell -b true' 2>/dev/null
    done
}

# The command test windows run instead of an interactive shell.
#
# Hooks are only half of the race. .zshrc's precmd calls
# tmux-update-window-names.sh on every prompt, so a window running a login
# shell renames ITSELF a beat after it is created, with no hook involved and
# nothing a hook override can prevent. That is what made
# `session mode leaves other sessions alone` fail on a developer machine while
# passing in CI, where the container has no such .zshrc.
#
# These suites never need a shell in the window: they assert on window names
# and pane paths, and drive the scripts under test by hand. A process that
# just sits there gives a window with a real cwd and no opinion about its own
# name. `sleep 86400` rather than `cat`, so a window that outlives its test
# cannot sit on a pipe waiting for input.
INERT_WINDOW_CMD=${INERT_WINDOW_CMD:-'sleep 86400'}

# Creates a tracked, hook-isolated, detached session and prints its name.
new_test_session() {
    local suffix=${1:-main} dir=${2:-$FIXTURES}
    local session
    session=$(session_name "$suffix")
    # Unquoted on purpose: the value is a command plus its argument, and
    # tmux needs them as separate words.
    # shellcheck disable=SC2086
    tmux new-session -d -s "$session" -c "$dir" $INERT_WINDOW_CMD
    track_session "$session"
    isolate_hooks "$session"
    printf '%s' "$session"
}

# Runs a command as if it were executing inside the given pane.
#
# Clearing $TMUX is the load-bearing part. A test runs inside the developer's
# real tmux session, so a script that splits or kills "the current pane" would
# otherwise operate on the developer's live pane.
#
# Setting $TMUX_PANE is not enough on its own. With no client attached, tmux
# resolves an unqualified target to the current window of the current session
# and ignores $TMUX_PANE entirely, so a caller must also select the window it
# wants the script to act on. See `target_window` below.
in_pane() {
    local pane=$1
    shift
    env -u TMUX TMUX_PANE="$pane" "$@"
}

# The value tmux exports as $TMUX, pointed at a test session rather than the
# developer's. Scripts that guard on [ -z "$TMUX" ] need this instead of
# `in_pane`, which clears $TMUX entirely.
tmux_env_for() {
    local session=$1 session_id
    session_id=$(tmux display-message -p -t "$session" '#{session_id}')
    printf '%s,%s,%s' \
        "$(tmux display-message -p '#{socket_path}')" \
        "$(tmux display-message -p '#{pid}')" \
        "${session_id#$}"
}

# Runs a command as if it were executing inside a pane of a test session, with
# $TMUX pointed at that session. Combine with `target_window`, since tmux
# resolves an unqualified target to the session's current window.
in_session() {
    local session=$1 pane=$2
    shift 2
    env TMUX="$(tmux_env_for "$session")" TMUX_PANE="$pane" "$@"
}

# Makes a window current so that a script acting on "the current pane" acts on
# this one. Required before any `in_pane` call that splits or kills panes.
target_window() {
    tmux select-window -t "$1"
    tmux select-pane -t "$(tmux list-panes -t "$1" -F '#{pane_id}' | head -n1)"
}

make_repo() {
    local path="$FIXTURES/$1" branch=${2:-main}
    mkdir -p "$path"
    git -C "$path" init -q -b "$branch"
    git -C "$path" -c user.email=t@t -c user.name=t commit -q --allow-empty -m init
    printf '%s' "$path"
}

make_worktree() {
    local repo=$1 branch=$2 path="$FIXTURES/$3"
    git -C "$repo" worktree add -q -b "$branch" "$path"
    printf '%s' "$path"
}

assert_equals() {
    local description=$1 expected=$2 actual=$3
    if [ "$expected" = "$actual" ]; then
        passed=$((passed + 1))
        printf 'ok: %s\n' "$description"
    else
        failed=$((failed + 1))
        printf 'FAIL: %s\n' "$description"
        printf '      expected: [%s]\n' "$expected"
        printf '      actual:   [%s]\n' "$actual"
    fi
}

assert_contains() {
    local description=$1 needle=$2 haystack=$3
    case $haystack in
        *"$needle"*)
            passed=$((passed + 1))
            printf 'ok: %s\n' "$description"
            ;;
        *)
            failed=$((failed + 1))
            printf 'FAIL: %s\n' "$description"
            printf '      expected to contain: [%s]\n' "$needle"
            printf '      actual:              [%s]\n' "$haystack"
            ;;
    esac
}

assert_succeeds() {
    local description=$1; shift
    if "$@" >/dev/null 2>&1; then
        passed=$((passed + 1))
        printf 'ok: %s\n' "$description"
    else
        failed=$((failed + 1))
        printf 'FAIL: %s (exited %d)\n' "$description" "$?"
    fi
}

finish() {
    printf '\n%s: %d passed, %d failed\n' "$TEST_NAME" "$passed" "$failed"
    [ "$failed" -eq 0 ]
}
