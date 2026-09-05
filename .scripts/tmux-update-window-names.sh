#!/bin/sh
#
# Name tmux windows after what they are actually pointed at.
#
# Precedence, lowest to highest:
#   1. basename of the active pane's working directory ("~" for $HOME)
#   2. "repo/branch" when that directory is a git repository, where repo is the
#      main repository's name even inside a linked worktree, and branch is a
#      short commit sha in parentheses when HEAD is detached
#   3. a name you set yourself with `prefix ,`
#
# Repositories matching @wname_bare_repos are named by branch alone, without the
# repo prefix. Those are the ones whose name is already obvious from context.
# The value is a list of glob patterns separated by "|".
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
NEWLINE='
'
OWNERSHIP_OPTION='@wname_auto'
LABEL_OPTION='@wname_label'
BARE_REPOS_OPTION='@wname_bare_repos'
DEFAULT_BARE_REPOS='Notability*|notability-*|gingerlabs-*'

usage() {
    sed -n '3,34p' "$0" | sed 's/^#\{1,2\} \{0,1\}//'
    exit "${1:-0}"
}

# The name of the repository a directory belongs to. Uses the common git dir so
# that a linked worktree reports its main repository rather than the worktree
# directory, which is usually named after the task and not the project.
repository_name() {
    repo_path=${1%/}
    [ "${repo_path##*/}" != '.git' ] || repo_path=${repo_path%/.git}
    repo_path=${repo_path##*/}
    printf '%s' "${repo_path%.git}"
}

matches_any_pattern() {
    subject=$1
    patterns=$2

    # Splitting on "|" leaves the patterns unquoted, so pathname expansion has
    # to be off or a pattern like "Notability*" would expand against the cwd.
    set -f
    saved_ifs=$IFS
    IFS='|'
    for pattern in $patterns; do
        # shellcheck disable=SC2254  # $patterns holds globs; matching them is the point
        case $subject in
            $pattern) IFS=$saved_ifs; set +f; return 0 ;;
        esac
    done
    IFS=$saved_ifs
    set +f
    return 1
}

directory_name() {
    if [ "$1" = "$HOME" ]; then
        printf '~'
    else
        printf '%s' "${1##*/}"
    fi
}

# The automatic part of the name: repo and branch if we are in a repository,
# otherwise the directory basename.
automatic_name() {
    directory=$1
    bare_patterns=$2

    # One rev-parse for both facts. This runs on every pane and window switch.
    # An empty repository exits non-zero here but still prints both lines, so
    # the exit status is deliberately ignored in favour of checking the output.
    info=$(git -C "$directory" rev-parse --path-format=absolute \
        --git-common-dir --abbrev-ref HEAD 2>/dev/null)
    common_dir=${info%%"$NEWLINE"*}
    revision=${info#*"$NEWLINE"}

    # --path-format arrived in git 2.31. Older versions report a path relative
    # to the directory we asked about.
    if [ -z "$info" ]; then
        info=$(git -C "$directory" rev-parse --git-common-dir --abbrev-ref HEAD 2>/dev/null)
        common_dir=${info%%"$NEWLINE"*}
        revision=${info#*"$NEWLINE"}
        case $common_dir in
            ''|/*) ;;
            *) common_dir="$directory/$common_dir" ;;
        esac
    fi

    if [ -z "$info" ] || [ "$common_dir" = "$revision" ]; then
        directory_name "$directory"
        return
    fi

    # "HEAD" means detached, or a repository with no commits yet.
    if [ "$revision" = 'HEAD' ]; then
        revision=$(git -C "$directory" branch --show-current 2>/dev/null)
        if [ -z "$revision" ]; then
            sha=$(git -C "$directory" rev-parse --short HEAD 2>/dev/null)
            if [ -z "$sha" ]; then
                directory_name "$directory"
                return
            fi
            revision="($sha)"
        fi
    fi

    repo_name=$(repository_name "$common_dir")
    if [ -z "$repo_name" ] || matches_any_pattern "$repo_name" "$bare_patterns"; then
        printf '%s' "$revision"
    else
        printf '%s/%s' "$repo_name" "$revision"
    fi
}

# The fields update_window needs, in the order it unpacks them. Named once so
# the batched read and the single-window read cannot ask for different things
# or ask for them in a different order.
STATE_FORMAT="#{window_name}$TAB#{$OWNERSHIP_OPTION}$TAB#{$LABEL_OPTION}$TAB#{$BARE_REPOS_OPTION}$TAB#{automatic-rename}$TAB#{pane_current_path}"

# We own a window's name when we set it last, when it is empty, or when tmux
# is still auto-renaming it. Anything else means the name was set by hand.
#
# Takes the window id and that window's already-read state line, so the caller
# decides how the state was fetched. update_each_window reads every window in
# one call; the single-window path reads just the one.
update_window() {
    window=$1
    state=$2

    [ -n "$state" ] || return 0

    # Split on tabs with parameter expansion rather than `cut`. This runs on
    # every pane and window switch, so each avoided fork is felt. `read` is not
    # usable here: it collapses the runs of tabs that empty fields produce.
    current_name=${state%%"$TAB"*}; state=${state#*"$TAB"}
    owned_name=${state%%"$TAB"*}; state=${state#*"$TAB"}
    label=${state%%"$TAB"*}; state=${state#*"$TAB"}
    bare_repos=${state%%"$TAB"*}; state=${state#*"$TAB"}
    automatic_rename=${state%%"$TAB"*}; directory=${state#*"$TAB"}

    if [ -n "$current_name" ] \
        && [ "$current_name" != "$owned_name" ] \
        && [ "$automatic_rename" != "1" ]; then
        return 0
    fi

    [ -n "$directory" ] || return 0

    [ -n "$bare_repos" ] || bare_repos=$DEFAULT_BARE_REPOS
    computed=$(automatic_name "$directory" "$bare_repos")
    [ -z "$label" ] || computed="$label - $computed"

    if [ "$computed" != "$current_name" ]; then
        tmux rename-window -t "$window" "$computed" 2>/dev/null || return 0
    fi
    if [ "$computed" != "$owned_name" ]; then
        tmux set -w -t "$window" "$OWNERSHIP_OPTION" "$computed" 2>/dev/null
    fi
}

# One tmux call for every window's state, rather than a list call followed by
# a display-message per window. Measured at 287ms across 21 windows against
# 16ms for the single call on this machine.
#
# `list-windows -F` resolves the same format strings, and `pane_current_path`
# in a window context is the active pane's path, which is exactly what the
# per-window read was asking for.
#
# This is the -a and -s path only. The hook that fires on every pane switch
# takes the single-window path below, which was already one call: measured at
# 30ms before and after this change. The saving here is real but it is not on
# the keystroke.
update_each_window() {
    tmux list-windows "$@" -F "#{window_id}$TAB$STATE_FORMAT" 2>/dev/null \
        | while IFS= read -r line; do
            update_window "${line%%"$TAB"*}" "${line#*"$TAB"}"
        done
}

# The single-window path. Still one call, so it stays a display-message rather
# than filtering a list of every window down to one.
update_one_window() {
    state=$(tmux display-message -p -t "$1" -F "$STATE_FORMAT" 2>/dev/null) \
        || return 0
    update_window "$1" "$state"
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
    window)  update_one_window "$target" ;;
    default)
        [ -n "${TMUX_PANE:-}" ] && update_one_window "$TMUX_PANE"
        ;;
esac
