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

# Prints the block and exits 0 when the first argument is --help or -h, or the
# one-line description and exits 0 when it is --describe.
# Call it before parsing anything else, so asking a command what it does never
# runs the command.
usage_if_requested() {
    case ${1:-} in
        --help|-h) print_usage; exit 0 ;;
        --describe) print_describe; exit 0 ;;
    esac
}

# Prints the `# usage:` block out of a script, defaulting to the caller's own.
#
# The subject is an argument with a default rather than a bare read of $0, so
# the function is callable against a named file and therefore testable without
# a second copy of the script under a new name.
#
# Unlike the one-line description, this cannot delegate to
# `<script> --describe`: print_usage runs inside the very script whose block
# it prints, and reading that script is the point. So the text-file assumption
# stays, and the guard below is what keeps it from failing silently. A shell
# subcommand rewritten as a binary that still sources this helper is the
# mistake worth naming: without the guard, sed writes "RE error: illegal byte
# sequence" to stderr and the reader gets empty help.
#
# `grep -qI ''` is the text-file probe. -I makes grep treat a binary file as a
# non-match, and the empty pattern matches every line of any file that has
# lines, so a match means "text and non-empty". LC_ALL=C pins the binary
# determination so it does not vary with the caller's locale.
print_usage() {
    usage_script=$(readlink -f "${1:-$0}")
    if ! LC_ALL=C grep -qI '' "$usage_script" 2>/dev/null; then
        printf '%s: not a text file, so it has no "# usage:" block to print\n' \
            "$usage_script" >&2
        return 1
    fi
    sed -n '/^# usage:/,/^[^#]/p' "$usage_script" \
        | sed -e '/^[^#]/d' \
        | awk '/^# ---/ { exit } { print }' \
        | sed -e 's/^# \{0,1\}//'
}

# Prints the `# help:` line out of a script, without its marker. Defaults to
# the calling script, so a subcommand describes itself with no argument.
#
# config-help formats this with `printf '  %-14s %s\n'`, so the contract is
# exactly one line with no leading whitespace: a second line, or an indented
# one, breaks the column the listing is read in. `head -1` holds the first
# half and the substitution's own anchor holds the second.
#
# Read out of the script rather than assigned in each one, so the `# help:`
# comment stays the single home of the string. config.test.sh asserts every
# subcommand carries that comment and the README tells contributors to add
# one, so a second copy here would be the copy that drifts.
print_describe() {
    describe_script=$(readlink -f "${1:-$0}")
    sed -n 's/^# help: //p' "$describe_script" | head -1
}
