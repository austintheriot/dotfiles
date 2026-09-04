#!/bin/bash
#
# Tests the `config` dispatcher and the config-<sub> utilities beside it.
# Every test runs against a fixture HOME under $FIXTURES: a bare repo at
# .cfg with a worktree, so nothing here touches the real dotfiles repo.
#
# Usage: ~/tests/config.test.sh

. "$(dirname "$0")/lib.sh"

CONFIG_DIR="$DOTFILES_ROOT/.scripts/config"
CONFIG="$CONFIG_DIR/config"

EXPECTED_SUBCOMMANDS='build stamp'

make_fixture_home() {
    fixture_home="$FIXTURES/home-$1"
    mkdir -p "$fixture_home"
    seed=$(make_repo "seed-$1")
    git clone -q --bare "$seed" "$fixture_home/.cfg"
    git --git-dir="$fixture_home/.cfg" config status.showUntrackedFiles no
    printf '%s\n' "$fixture_home"
}

# --- passthrough ------------------------------------------------------------

home=$(make_fixture_home passthrough)
expected=$(git --git-dir="$home/.cfg" --work-tree="$home" rev-parse HEAD)
actual=$(HOME="$home" "$CONFIG" rev-parse HEAD)
assert_equals 'config rev-parse HEAD passes through to the bare repo' \
    "$expected" "$actual"

actual=$(HOME="$home" "$CONFIG" status --porcelain --untracked-files=no 2>&1)
assert_equals 'config status passes through with the fixture worktree' '' "$actual"

output=$(HOME="$home" "$CONFIG" definitely-not-a-command 2>&1)
status=$?
assert_equals 'an unknown subcommand falls through to git and fails' '1' "$status"
assert_contains 'the failure is git own message' \
    "git: 'definitely-not-a-command' is not a git command" "$output"

output=$(HOME="$home" "$CONFIG" 2>&1)
status=$?
assert_equals 'no arguments passes through to git usage' '1' "$status"
assert_contains 'git usage is what prints' 'usage: git' "$output"

# --- name rules -------------------------------------------------------------

actual=$(cd "$CONFIG_DIR" && ls config-* 2>/dev/null | sed 's/^config-//' | sort | tr '\n' ' ' | sed 's/ $//')
expected=$(printf '%s\n' $EXPECTED_SUBCOMMANDS | sort | tr '\n' ' ' | sed 's/ $//')
assert_equals 'the config-<sub> set equals the allowlist' "$expected" "$actual"

not_executable=''
for sub in $EXPECTED_SUBCOMMANDS; do
    [ -x "$CONFIG_DIR/config-$sub" ] || not_executable="$not_executable $sub"
done
assert_equals 'every listed subcommand is executable' '' "$not_executable"

git_commands=$(git --list-cmds=main,others)
shadowing=''
for sub in $EXPECTED_SUBCOMMANDS; do
    printf '%s\n' "$git_commands" | grep -qx "$sub" && shadowing="$shadowing $sub"
done
assert_equals 'no config-<sub> shadows a git command' '' "$shadowing"

line_count=$(grep -c '' "$CONFIG")
[ "$line_count" -lt 40 ] && under_40=yes || under_40="no ($line_count lines)"
assert_equals 'the dispatcher stays under 40 lines' 'yes' "$under_40"

# --- sibling dispatch -------------------------------------------------------

home=$(make_fixture_home sibling)
sibling_dir="$FIXTURES/sibling-bin"
mkdir -p "$sibling_dir"
cp "$CONFIG" "$sibling_dir/config"
printf '#!/bin/sh\nprintf "probe:%%s\\n" "$@"\n' > "$sibling_dir/config-probe"
chmod 755 "$sibling_dir/config-probe"
actual=$(HOME="$home" "$sibling_dir/config" probe one "two words")
assert_equals 'a sibling config-<sub> receives the remaining args intact' \
    'probe:one
probe:two words' "$actual"

link_dir="$FIXTURES/linked-bin"
mkdir -p "$link_dir"
ln -s "$sibling_dir/config" "$link_dir/config"
actual=$(HOME="$home" "$link_dir/config" probe via-symlink)
assert_equals 'the dispatcher resolves siblings through its own symlink' \
    'probe:via-symlink' "$actual"

finish
