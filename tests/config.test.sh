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

EXPECTED_SUBCOMMANDS='build stamp install-hooks'

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

# --- install-hooks ----------------------------------------------------------

home=$(make_fixture_home hooks)
mkdir -p "$home/tests" "$home/.cfg/hooks"
printf '#!/bin/sh\nexit 0\n' > "$home/tests/pre-commit"
printf '#!/bin/sh\nexit 0\n' > "$home/tests/pre-push"
chmod 755 "$home/tests/pre-commit" "$home/tests/pre-push"
install_dir="$FIXTURES/install-bin"
mkdir -p "$install_dir"
# config-install-hooks resolves its own directory with `readlink -f`, which
# canonicalizes symlinked ancestors (/var -> /private/var on macOS). $FIXTURES
# sits under such a symlink, so the assertions below compare against the same
# canonical form the script will actually report, not the pre-canonical path.
install_dir=$(readlink -f "$install_dir")
cp "$CONFIG" "$install_dir/config"
cp "$CONFIG_DIR/config-install-hooks" "$install_dir/config-install-hooks"
chmod 755 "$install_dir/config" "$install_dir/config-install-hooks"

output=$(HOME="$home" "$install_dir/config" install-hooks 2>&1)
status=$?
assert_equals 'install-hooks succeeds on a clean fixture' '0' "$status"
assert_equals 'pre-commit is linked' "$home/tests/pre-commit" "$(readlink "$home/.cfg/hooks/pre-commit")"
assert_equals 'pre-push is linked' "$home/tests/pre-push" "$(readlink "$home/.cfg/hooks/pre-push")"
assert_equals 'the dispatcher is linked into ~/.local/bin' "$install_dir/config" "$(readlink "$home/.local/bin/config")"

output_again=$(HOME="$home" "$install_dir/config" install-hooks 2>&1)
status=$?
assert_equals 'a second install-hooks succeeds' '0' "$status"
assert_equals 'a second install-hooks prints the same report' "$output" "$output_again"

chmod o+w "$home/.local/bin"
output=$(HOME="$home" "$install_dir/config" install-hooks 2>&1)
status=$?
assert_equals 'install-hooks refuses a world-writable ~/.local/bin' '1' "$status"
assert_contains 'the refusal names the directory' "$home/.local/bin" "$output"
chmod o-w "$home/.local/bin"

chmod g+w "$install_dir"
output=$(HOME="$home" "$install_dir/config" install-hooks 2>&1)
status=$?
assert_equals 'install-hooks refuses a group-writable dispatcher directory' '1' "$status"
chmod g-w "$install_dir"

finish
