#!/bin/bash
#
# Tests the per-command help surface: `config <sub> --help` and the flag
# spellings each subcommand accepts.
#
# `config help` answers "which commands exist". It does not answer "what does
# `config test -q` do", and that second question is the one a reader has when
# they are already at the right command. Each config-<sub> answers it for
# itself, from a `# usage:` block at the top of the script, for the same
# reason the one-line listing is generated: a hand-maintained second copy of
# the flags is the copy that goes stale.
#
# The block runs from the first `# usage:` line to the first line that is not
# a comment. Everything in it prints verbatim, so the script's own header is
# the help text and there is nothing to keep in step.
#
# Usage: ~/tests/config-usage.test.sh

. "$(dirname "$0")/lib.sh"

CONFIG_DIR="$DOTFILES_ROOT/.scripts/config"
CONFIG="$CONFIG_DIR/config"

# The subcommands that parse flags of their own, and so owe the reader a
# description of each one. The thin wrappers (check, sync, install) forward to
# a program that prints its own help, and reload and stamp take no flags.
FLAG_TAKING='test'

# Every subcommand, flags or not, answers --help. A reader who types it should
# never get git usage or a stack trace.
ALL_SUBCOMMANDS='build stamp install-hooks check sync install test reload help'

make_fixture_home() {
    fixture_home="$FIXTURES/home-$1"
    mkdir -p "$fixture_home"
    seed=$(make_repo "seed-$1")
    git clone -q --bare "$seed" "$fixture_home/.cfg"
    git --git-dir="$fixture_home/.cfg" config status.showUntrackedFiles no
    printf '%s\n' "$fixture_home"
}

home=$(make_fixture_home usage)

# Stubs for the programs the thin wrappers exec. A wrapper must answer --help
# itself rather than handing it to a program that may not be installed.
shim_dir="$FIXTURES/shims"
mkdir -p "$shim_dir"
printf '#!/bin/sh\nprintf "manifest:%%s\\n" "$@"\n' > "$shim_dir/config-manifest"
chmod 755 "$shim_dir/config-manifest"
mkdir -p "$home/.scripts/deps" "$home/tests"
printf '#!/bin/sh\nprintf "deps:%%s\\n" "$@"\n' > "$home/.scripts/deps/check-deps.sh"
printf '#!/bin/sh\nprintf "all:%%s\\n" "$@"\n' > "$home/tests/run-all.sh"
printf '#!/bin/sh\nprintf "docker:%%s\\n" "$@"\n' > "$home/tests/run-in-docker.sh"
chmod 755 "$home/.scripts/deps/check-deps.sh" "$home/tests/run-all.sh" \
    "$home/tests/run-in-docker.sh"

run_config() {
    HOME="$home" PATH="$shim_dir:$PATH" "$CONFIG" "$@"
}

# --- every subcommand answers --help ----------------------------------------

for sub in $ALL_SUBCOMMANDS; do
    output=$(run_config "$sub" --help 2>&1)
    status=$?
    assert_equals "config $sub --help exits 0" '0' "$status"
    assert_contains "config $sub --help names the command" "config $sub" "$output"
done

# -h is the spelling people try when --help is too long, and it must reach the
# same text rather than a different one.
for sub in $ALL_SUBCOMMANDS; do
    long=$(run_config "$sub" --help 2>&1)
    short=$(run_config "$sub" -h 2>&1)
    assert_equals "config $sub -h prints the same text as --help" "$long" "$short"
done

# --help must not run the command. A subcommand that ran its work first and
# printed help after would rebuild, or push, or start a watch loop.
output=$(run_config test --help 2>&1)
assert_equals 'config test --help does not run the suite' '' \
    "$(printf '%s' "$output" | grep -F 'all:' || true)"

output=$(run_config check --help 2>&1)
assert_equals 'config check --help does not exec config-manifest' '' \
    "$(printf '%s' "$output" | grep -F 'manifest:' || true)"

output=$(run_config install --help 2>&1)
assert_equals 'config install --help does not exec check-deps' '' \
    "$(printf '%s' "$output" | grep -F 'deps:' || true)"

# install-hooks is the sharpest case: before this suite, `config install-hooks
# --help` linked the hooks and rewrote ~/.local/bin/config, because the script
# ignored its arguments entirely. Asking a command what it does must not be
# the same as doing it.
rm -f "$home/.cfg/hooks/pre-commit" "$home/.cfg/hooks/pre-push" \
    "$home/.local/bin/config"
printf '#!/bin/sh\nexit 0\n' > "$home/tests/pre-commit"
printf '#!/bin/sh\nexit 0\n' > "$home/tests/pre-push"
chmod 755 "$home/tests/pre-commit" "$home/tests/pre-push"
run_config install-hooks --help >/dev/null 2>&1
assert_succeeds 'config install-hooks --help does not link pre-commit' \
    test ! -e "$home/.cfg/hooks/pre-commit"
assert_succeeds 'config install-hooks --help does not link the dispatcher' \
    test ! -e "$home/.local/bin/config"

# --- the usage block is the source ------------------------------------------

# A `# usage:` block in the script is what prints, so the help text and the
# script header cannot disagree.
missing_block=''
for sub in $ALL_SUBCOMMANDS; do
    grep -q '^# usage:' "$CONFIG_DIR/config-$sub" || missing_block="$missing_block $sub"
done
assert_equals 'every config-<sub> carries a "# usage:" block' '' "$missing_block"

for sub in $ALL_SUBCOMMANDS; do
    first_line=$(sed -n 's/^# usage: //p' "$CONFIG_DIR/config-$sub" | head -1)
    output=$(run_config "$sub" --help 2>&1)
    # An empty needle would make assert_contains pass for any output, which
    # is exactly the state this suite starts in. Assert the needle first.
    assert_succeeds "config-$sub has a usage line to print" test -n "$first_line"
    assert_contains "config $sub --help prints its own usage line" \
        "$first_line" "$output"
done

# --- the helper is part of the set ------------------------------------------

# Sourcing usage.sh gives every subcommand a runtime dependency on a file
# beside it. The dispatcher's whole design is "exec whatever sits beside me",
# so a copy that takes the config-<sub> scripts and leaves the helper behind
# is a shape that can really happen. Name the dependency here, so it is a
# stated fact rather than something a reader infers from a not-found error.
assert_succeeds 'the usage helper sits beside the subcommands' \
    test -f "$CONFIG_DIR/usage.sh"

# It must not be named config-*, or the dispatcher would treat it as a
# subcommand called `usage.sh` and `config help` would list it.
assert_equals 'the helper is outside the config-<sub> namespace' '' \
    "$(cd "$CONFIG_DIR" && ls config-usage.sh 2>/dev/null || true)"

not_sourcing=''
for sub in $ALL_SUBCOMMANDS; do
    grep -q 'usage\.sh' "$CONFIG_DIR/config-$sub" || not_sourcing="$not_sourcing $sub"
done
assert_equals 'every config-<sub> sources the shared helper' '' "$not_sourcing"

# --- flags are described ----------------------------------------------------

# Every flag the script parses appears in its help text with a description
# beside it. A flag that is accepted but undocumented is the gap this suite
# exists to close.
for sub in $FLAG_TAKING; do
    output=$(run_config "$sub" --help 2>&1)
    parsed=$(grep -o '^\( *\)\(-[a-z]\||--[a-z-]*\))' "$CONFIG_DIR/config-$sub" \
        | grep -o '\-\{1,2\}[a-z][a-z-]*' | sort -u)
    undocumented=''
    while IFS= read -r flag; do
        [ -n "$flag" ] || continue
        printf '%s\n' "$output" | grep -qF -- "$flag" || undocumented="$undocumented $flag"
    done <<INNER
$parsed
INNER
    assert_equals "every flag config $sub parses is described in its help" \
        '' "$undocumented"
done

# --- short and long spellings -----------------------------------------------

# `config test` took -q with no long spelling and --docker/--watch with no
# short one, so a reader had to remember which form each flag came in. Both
# spellings work for all three.
actual=$(run_config test -q)
assert_equals 'config test -q passes -q through' 'all:-q' "$actual"

actual=$(run_config test --quiet)
assert_equals 'config test --quiet is the long spelling of -q' 'all:-q' "$actual"

actual=$(run_config test --docker)
assert_equals 'config test --docker runs the docker suite' 'docker:' "$actual"

actual=$(run_config test -d)
assert_equals 'config test -d is the short spelling of --docker' 'docker:' "$actual"

printf 'run\n' > "$home/tests/run-all.sh"
printf '#!/bin/sh\nprintf "all:%%s\\n" "$@"\n' > "$home/tests/run-all.sh"
chmod 755 "$home/tests/run-all.sh"

# -w/--watch starts a loop, so it is checked for acceptance rather than run:
# pairing it with --docker is a usage error either way, and the error proves
# the flag was recognised rather than falling into the unknown-flag branch.
output=$(run_config test -w --docker 2>&1)
status=$?
assert_equals 'config test -w is the short spelling of --watch' '2' "$status"
assert_contains 'the -w rejection is the watch/docker conflict, not unknown-flag' \
    'usage: config test' "$output"

# An unknown flag still fails, so the aliases did not open the parser up.
run_config test --bogus >/dev/null 2>&1
status=$?
assert_equals 'config test still rejects an unknown flag' '2' "$status"

run_config test -z >/dev/null 2>&1
status=$?
assert_equals 'config test still rejects an unknown short flag' '2' "$status"

finish
