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
#
# A `# ---` line ends the block early, so a script can follow its help with a
# note meant for whoever edits it rather than whoever runs it. A bare `#`
# cannot serve as the terminator: the blocks already use one to separate
# their own paragraphs, so it would end every block at its first blank line.
# config-install-hooks needs this: its trust-boundary rationale explains why
# the ownership checks exist, which is a maintainer's question, and printing
# it to someone who asked what the command does buries the answer.

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
        | sed -e '/^[^#]/d' \
        | awk '/^# ---/ { exit } { print }' \
        | sed -e 's/^# \{0,1\}//'
}
