#!/bin/sh
#
# Name tmux windows after what they are actually pointed at.
#
# Precedence, lowest to highest:
#   1. basename of the active pane's working directory ("~" for $HOME)
#   2. the git branch of that directory, if it is a repository
#      (a short commit sha in parentheses when HEAD is detached)
#   3. a name you set yourself with `prefix ,`
#
# Rename a window to an empty name to drop back to automatic naming.
#
# Set the per-window option @wname_label to keep a stable prefix in front of
# the automatic part:
#
#     tmux set -w @wname_label Reviews     # -> "Reviews - feature/login"
#
# A label keeps `select-window -t <label>` keybindings working while the
# branch behind it still shows.
#
# Usage:
#   tmux-update-window-names.sh                 the current window, or
#                                               nothing when run outside tmux
#   tmux-update-window-names.sh -a              every window in every session
#   tmux-update-window-names.sh -s <session>    every window in one session
#   tmux-update-window-names.sh -w <window>     one window
#   tmux-update-window-names.sh <session>       same as -s, kept for callers
#                                               that predate the flags

set -u

TAB=$(printf '\t')
OWNERSHIP_OPTION='@wname_auto'
LABEL_OPTION='@wname_label'

usage() {
    sed -n '3,28p' "$0" | sed 's/^#\{1,2\} \{0,1\}//'
    exit "${1:-0}"
}

# The automatic part of the name: branch if we are in a repository,
# otherwise the directory basename.
automatic_name() {
    directory=$1

    branch=$(git -C "$directory" branch --show-current 2>/dev/null)
    if [ -n "$branch" ]; then
        printf '%s' "$branch"
        return
    fi

    if git -C "$directory" rev-parse --git-dir >/dev/null 2>&1; then
        sha=$(git -C "$directory" rev-parse --short HEAD 2>/dev/null)
        if [ -n "$sha" ]; then
            printf '(%s)' "$sha"
            return
        fi
    fi

    if [ "$directory" = "$HOME" ]; then
        printf '~'
    else
        printf '%s' "${directory##*/}"
    fi
}

# We own a window's name when we set it last, when it is empty, or when tmux
# is still auto-renaming it. Anything else means the name was set by hand.
update_window() {
    window=$1

    state=$(tmux display-message -p -t "$window" -F \
        "#{window_name}$TAB#{$OWNERSHIP_OPTION}$TAB#{$LABEL_OPTION}$TAB#{automatic-rename}$TAB#{pane_current_path}" \
        2>/dev/null) || return 0
    [ -n "$state" ] || return 0

    # Split on tabs with parameter expansion rather than `cut`. This runs on
    # every pane and window switch, so each avoided fork is felt. `read` is not
    # usable here: it collapses the runs of tabs that empty fields produce.
    current_name=${state%%"$TAB"*}; state=${state#*"$TAB"}
    owned_name=${state%%"$TAB"*}; state=${state#*"$TAB"}
    label=${state%%"$TAB"*}; state=${state#*"$TAB"}
    automatic_rename=${state%%"$TAB"*}; directory=${state#*"$TAB"}

    if [ -n "$current_name" ] \
        && [ "$current_name" != "$owned_name" ] \
        && [ "$automatic_rename" != "1" ]; then
        return 0
    fi

    [ -n "$directory" ] || return 0

    computed=$(automatic_name "$directory")
    [ -z "$label" ] || computed="$label - $computed"

    if [ "$computed" != "$current_name" ]; then
        tmux rename-window -t "$window" "$computed" 2>/dev/null || return 0
    fi
    if [ "$computed" != "$owned_name" ]; then
        tmux set -w -t "$window" "$OWNERSHIP_OPTION" "$computed" 2>/dev/null
    fi
}

update_each_window() {
    tmux list-windows "$@" -F '#{window_id}' 2>/dev/null | while read -r window; do
        update_window "$window"
    done
}

mode=default
target=''

while [ $# -gt 0 ]; do
    case $1 in
        -a|--all)     mode=all; shift ;;
        -s|--session) mode=session; target=${2:-}; shift 2 ;;
        -w|--window)  mode=window; target=${2:-}; shift 2 ;;
        -h|--help)    usage 0 ;;
        -*)           printf 'Unknown option: %s\n\n' "$1" >&2; usage 1 ;;
        *)            mode=session; target=$1; shift ;;
    esac
done

if [ "$mode" = session ] || [ "$mode" = window ]; then
    if [ -z "$target" ]; then
        printf 'Missing target for --%s\n\n' "$mode" >&2
        usage 1
    fi
fi

case $mode in
    all)     update_each_window -a ;;
    session) update_each_window -t "$target" ;;
    window)  update_window "$target" ;;
    default)
        [ -n "${TMUX_PANE:-}" ] && update_window "$TMUX_PANE"
        ;;
esac
