#!/bin/bash
#
# Pins the query shape of .scripts/tmux-update-window-names.sh.
#
# `-a` and `-s` read each window's state with its own `display-message -p -t`,
# one call per window, so a session with 21 windows paid 21 round trips to the
# tmux server where one `list-windows -F` returns the same fields for all of
# them. Measured at 287ms against 16ms.
#
# The hooks that fire on every pane switch take the single-window path, which
# was already one call and is unchanged at 30ms. This suite is about the
# multi-window path; it is not a claim about input lag.
#
# Behavior is covered by tmux-update-window-names.test.sh. This suite asserts
# only the property that makes the script cheap: the number of tmux
# invocations does not grow with the number of windows. That is not visible in
# the names the script produces, so nothing else can catch its loss, and the
# loop is an easy thing to reintroduce while every behavioral test stays
# green.
#
# tmux is stubbed rather than driven. Counting real invocations means counting
# processes, and the point is the count itself, not what tmux does with them.
#
# Usage: ~/tests/tmux-name-batching.test.sh

. "$(dirname "$0")/lib.sh"

SCRIPT="$DOTFILES_ROOT/.scripts/tmux-update-window-names.sh"

# A tmux stub that logs every invocation and answers the two read commands the
# script issues. Windows are served from $STUB_WINDOWS, one "id<TAB>path" pair
# per line, so a test decides how many exist.
stub_dir="$FIXTURES/stub"
mkdir -p "$stub_dir"
cat > "$stub_dir/tmux" <<'STUB'
#!/bin/sh
printf '%s\n' "$*" >> "$TMUX_CALL_LOG"

format=''
prev=''
for arg in "$@"; do
    [ "$prev" = '-F' ] && format=$arg
    prev=$arg
done

emit_windows() {
    # Answers whatever format was asked for by substituting the two fields
    # the stub tracks and blanking the rest, which is what an unset tmux
    # option expands to.
    while IFS=$(printf '\t') read -r id path; do
        [ -n "$id" ] || continue
        line=$format
        line=$(printf '%s' "$line" | sed \
            -e "s|#{window_id}|$id|g" \
            -e "s|#{pane_current_path}|$path|g" \
            -e "s|#{window_name}|${STUB_WINDOW_NAME:-}|g" \
            -e "s|#{automatic-rename}|1|g" \
            -e "s|#{@wname_auto}|${STUB_WINDOW_NAME:-}|g" \
            -e "s|#{@[a-z_]*}||g")
        printf '%s\n' "$line"
    done < "$STUB_WINDOWS"
}

case ${1:-} in
    list-windows)   emit_windows ;;
    display-message) emit_windows | head -1 ;;
    *) ;;
esac
exit 0
STUB
chmod 755 "$stub_dir/tmux"

# Counts the tmux invocations one run makes against a session of $1 windows.
calls_for() {
    local count=$1 i
    export STUB_WINDOWS="$FIXTURES/windows-$count"
    export TMUX_CALL_LOG="$FIXTURES/calls-$count"
    : > "$STUB_WINDOWS"
    : > "$TMUX_CALL_LOG"
    for ((i = 1; i <= count; i++)); do
        printf '@%d\t%s\n' "$i" "$FIXTURES" >> "$STUB_WINDOWS"
    done
    PATH="$stub_dir:$PATH" "$SCRIPT" -a >/dev/null 2>&1
    grep -c '' "$TMUX_CALL_LOG"
}

# --- the reads are batched --------------------------------------------------

# The read path is the one that ran unconditionally on every hook, so it is
# the one asserted. Writes are per-window by nature -- renaming twenty windows
# is twenty renames -- and only happen when a name actually changed, which the
# steady-state assertion below covers.

few=$(calls_for 2)
assert_succeeds 'the run against 2 windows called tmux at all' test "$few" -gt 0

# Called for the log it writes, not the total it returns: the assertions below
# read $FIXTURES/calls-20 and count verbs, which a total cannot distinguish.
calls_for 20 >/dev/null

reads=$(grep -c '^display-message' "$FIXTURES/calls-20" || true)
assert_equals 'no per-window display-message read survives' '0' "$reads"

lists=$(grep -c '^list-windows' "$FIXTURES/calls-20" || true)
assert_equals 'one list-windows call reads every window' '1' "$lists"

# One read for two windows and one read for twenty is the whole property.
lists_few=$(grep -c '^list-windows' "$FIXTURES/calls-2" || true)
assert_equals 'the read count is the same for two windows as for twenty' \
    "$lists_few" "$lists"

# --- the steady state costs one call ----------------------------------------

# The hooks fire on every pane switch, and almost every one of those finds
# every name already correct. That run is the one whose cost is felt, and it
# is a single tmux call: nothing to rename, nothing to set.
#
# The stub reports each window already carrying the name the script computes,
# which for a non-repository directory is the basename of its path.
export STUB_WINDOWS="$FIXTURES/windows-steady"
export TMUX_CALL_LOG="$FIXTURES/calls-steady"
: > "$STUB_WINDOWS"
: > "$TMUX_CALL_LOG"
steady_dir="$FIXTURES/not-a-repo"
mkdir -p "$steady_dir"
for ((i = 1; i <= 20; i++)); do
    printf '@%d\t%s\n' "$i" "$steady_dir" >> "$STUB_WINDOWS"
done
STUB_WINDOW_NAME=$(basename "$steady_dir") \
    PATH="$stub_dir:$PATH" "$SCRIPT" -a >/dev/null 2>&1

steady=$(grep -c '' "$TMUX_CALL_LOG" || true)
assert_equals 'a run that changes nothing costs one tmux call' '1' "$steady"

# The batched read must actually carry the fields the script needs, or it
# would be one cheap call plus a per-window fallback.
list_call=$(grep -m1 '^list-windows' "$FIXTURES/calls-20")
for field in window_id pane_current_path window_name automatic-rename; do
    assert_contains "the batched read asks for $field" "$field" "$list_call"
done

finish
