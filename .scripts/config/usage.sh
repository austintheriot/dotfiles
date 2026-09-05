# shellcheck shell=sh
# Prints the `# usage:` block out of the calling script.
#
# Sourced, not executed, so the caller keeps its own $0 and the block is read
# out of the script the reader actually asked about.
#
# The block starts at the first `# usage:` line and ends at the first line
# that is not a comment, so a script's header is its help text. A second copy
# maintained beside the parser is the copy that drifts, which is the same
# reason `config help` reads the `# help:` lines rather than a hand-written
# list.

# Prints the block and exits 0 when the first argument is --help or -h.
# Call it before parsing anything else, so asking a command what it does never
# runs the command.
usage_if_requested() {
    case ${1:-} in
        --help|-h) print_usage; exit 0 ;;
    esac
}

print_usage() {
    script=$(readlink -f "$0")
    sed -n '/^# usage:/,/^[^#]/p' "$script" \
        | sed -e '/^[^#]/d' -e 's/^# \{0,1\}//'
}
