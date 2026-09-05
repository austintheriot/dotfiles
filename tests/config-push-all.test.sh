#!/bin/bash
#
# Tests `config push-all`, the subcommand that lands mac and linux in one
# push.
#
# Why the command exists: the branch-drift workflow compares origin/mac
# against origin/linux on every push to either. Pushing the two branches as
# two commands lands them seconds apart, so the run for the first push
# fetches the other branch before its push arrives and fails against a stale
# ref. Measured gaps between the two runs were 23s, 16s, 14s and 15s, and
# every push in that session needed a manual rerun. One `git push --atomic`
# with both refs closes the window at the source: both refs are on the
# server before either workflow run starts, so there is no earlier state for
# a run to observe.
#
# The push itself is never real here. Every assertion runs against a stub
# `git` on PATH that records its arguments, because the behavior worth
# pinning is which refs the command sends and in what shape, and a real push
# would need a remote and would make the suite a network test.
#
# Usage: ~/tests/config-push-all.test.sh

. "$(dirname "$0")/lib.sh"

CONFIG_DIR="$DOTFILES_ROOT/.scripts/config"
CONFIG="$CONFIG_DIR/config"

assert_succeeds 'the push-all subcommand exists' \
    test -x "$CONFIG_DIR/config-push-all"

# The stub stands in for git on PATH. It appends its arguments to a log and
# exits 0, so the assertions read the log rather than a remote.
#
# It must not be a function or an alias: the dispatcher execs the subcommand
# in a new process, so only something on PATH survives the boundary.
stub_dir="$FIXTURES/stub-bin"
mkdir -p "$stub_dir"
push_log="$FIXTURES/push.log"

write_git_stub() {
    local exit_code=${1:-0}
    cat > "$stub_dir/git" <<STUB
#!/bin/sh
printf '%s\n' "\$*" >> "$push_log"
exit $exit_code
STUB
    chmod 755 "$stub_dir/git"
}

run_push_all() {
    : > "$push_log"
    PATH="$stub_dir:$PATH" "$CONFIG" push-all 2>&1
}

# --- the push is atomic and names both branches ------------------------------

write_git_stub 0
output=$(run_push_all)
status=$?
logged=$(cat "$push_log")

assert_equals 'config push-all exits 0 when the push succeeds' '0' "$status"

# --atomic is the whole point. Without it git sends the refs in one
# connection but applies them independently, so a rejected mac still lets
# linux land and the pair is split again -- the exact state the command
# exists to prevent.
assert_contains 'the push is atomic' '--atomic' "$logged"
assert_contains 'the push names the remote' 'origin' "$logged"
assert_contains 'the push sends mac' 'mac' "$logged"
assert_contains 'the push sends linux' 'linux' "$logged"

# One invocation, not two. Two pushes with --atomic on each is still two
# transactions and still races.
assert_equals 'both refs go in a single push' '1' \
    "$(grep -c 'push' "$push_log")"

# --- failure is reported, not swallowed --------------------------------------

# A rejected push must fail the command. `config sync` has already written a
# commit onto the other branch by this point, so a push-all that reported
# success on a rejection would leave the user believing the remote holds a
# pair it does not have.
write_git_stub 1
run_push_all >/dev/null 2>&1
status=$?
assert_equals 'a rejected push fails the command' '1' "$status"

# --- --help does not push ----------------------------------------------------

# The sharpest case in this file, and the reason config-usage.test.sh checks
# the same shape for install-hooks: asking what a command does must not do
# it. A push cannot be taken back once the remote has it.
write_git_stub 0
: > "$push_log"
PATH="$stub_dir:$PATH" "$CONFIG" push-all --help >/dev/null 2>&1
assert_equals 'config push-all --help does not push' '' "$(cat "$push_log")"

output=$(PATH="$stub_dir:$PATH" "$CONFIG" push-all --help 2>&1)
assert_contains 'config push-all --help names the command' \
    'config push-all' "$output"

# --- unknown flags are refused -----------------------------------------------

# An unrecognised flag must not fall through into the push. Silently pushing
# when the user asked for something the command does not understand is the
# failure mode worth spending an assertion on.
write_git_stub 0
: > "$push_log"
PATH="$stub_dir:$PATH" "$CONFIG" push-all --bogus >/dev/null 2>&1
status=$?
assert_equals 'an unknown flag exits non-zero' '2' "$status"
assert_equals 'an unknown flag does not push' '' "$(cat "$push_log")"

finish
