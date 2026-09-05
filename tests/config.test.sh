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

EXPECTED_SUBCOMMANDS='build stamp install-hooks check sync install test reload help'

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

# shellcheck disable=SC2012  # config-<sub> names cannot contain spaces
actual=$(cd "$CONFIG_DIR" && ls config-* 2>/dev/null | sed 's/^config-//' | sort | tr '\n' ' ' | sed 's/ $//')
# shellcheck disable=SC2086  # deliberate split of the space-separated list
expected=$(printf '%s\n' $EXPECTED_SUBCOMMANDS | sort | tr '\n' ' ' | sed 's/ $//')
assert_equals 'the config-<sub> set equals the allowlist' "$expected" "$actual"

not_executable=''
for sub in $EXPECTED_SUBCOMMANDS; do
    [ -x "$CONFIG_DIR/config-$sub" ] || not_executable="$not_executable $sub"
done
assert_equals 'every listed subcommand is executable' '' "$not_executable"

# A config-<sub> that shares a git verb makes that verb unreachable by name,
# so each one is a deliberate choice rather than an accident. `config -- <verb>`
# is the escape hatch that keeps the git command reachable; the passthrough
# section below proves it works for every name listed here.
SHADOWED_ON_PURPOSE='help'

git_commands=$(git --list-cmds=main,others)
shadowing=''
for sub in $EXPECTED_SUBCOMMANDS; do
    printf '%s\n' "$SHADOWED_ON_PURPOSE" | grep -qw "$sub" && continue
    printf '%s\n' "$git_commands" | grep -qx "$sub" && shadowing="$shadowing $sub"
done
assert_equals 'no config-<sub> shadows a git command by accident' '' "$shadowing"

# The reverse: a name on the deliberate list that no longer shadows anything
# (or never did) is stale, and would silently excuse a future accident.
not_actually_shadowing=''
for sub in $SHADOWED_ON_PURPOSE; do
    printf '%s\n' "$git_commands" | grep -qx "$sub" \
        || not_actually_shadowing="$not_actually_shadowing $sub"
done
assert_equals 'every deliberately-shadowed name really is a git command' '' \
    "$not_actually_shadowing"

# A name is only worth shadowing if a sibling script actually claims it.
without_script=''
for sub in $SHADOWED_ON_PURPOSE; do
    [ -x "$CONFIG_DIR/config-$sub" ] || without_script="$without_script $sub"
done
assert_equals 'every deliberately-shadowed name has a config-<sub>' '' \
    "$without_script"

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

mkdir -p "$sibling_dir/config-x"
printf '#!/bin/sh\nprintf "escaped\\n"\n' > "$sibling_dir/config-x/go"
chmod 755 "$sibling_dir/config-x/go"
output=$(HOME="$home" "$sibling_dir/config" x/go 2>&1)
status=$?
assert_equals 'a slash-shaped subcommand does not exec a file inside a config-<sub> directory' \
    '' "$(printf '%s' "$output" | grep -F 'escaped' || true)"
assert_equals 'a slash-shaped subcommand falls through to git and fails' '1' "$status"
assert_contains 'the failure is git own message' 'is not a git command' "$output"

output=$(HOME="$home" "$sibling_dir/config" -x 2>&1)
assert_contains 'a dash-shaped subcommand falls through to git rather than probing a sibling' \
    'unknown option' "$output"

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

chmod o+w "$home/tests"
output=$(HOME="$home" "$install_dir/config" install-hooks 2>&1)
status=$?
assert_equals 'install-hooks refuses a world-writable tests directory' '1' "$status"
assert_contains 'the refusal names the tests directory' "$home/tests" "$output"
chmod o-w "$home/tests"

# --- thin wrappers ----------------------------------------------------------

shim_dir="$FIXTURES/shims"
mkdir -p "$shim_dir"
printf '#!/bin/sh\nprintf "manifest:%%s\\n" "$@"\n' > "$shim_dir/config-manifest"
chmod 755 "$shim_dir/config-manifest"

actual=$(PATH="$shim_dir:$PATH" "$CONFIG" check origin/mac origin/linux)
assert_equals 'config check execs config-manifest check with its args' \
    'manifest:check
manifest:origin/mac
manifest:origin/linux' "$actual"

actual=$(PATH="$shim_dir:$PATH" "$CONFIG" sync --dry-run --to linux)
assert_equals 'config sync execs config-manifest sync with its args' \
    'manifest:sync
manifest:--dry-run
manifest:--to
manifest:linux' "$actual"

home=$(make_fixture_home wrappers)
mkdir -p "$home/.scripts/deps" "$home/tests"
printf '#!/bin/sh\nprintf "deps:%%s\\n" "$@"\n' > "$home/.scripts/deps/check-deps.sh"
printf '#!/bin/sh\n[ "$#" -eq 0 ] && printf "all:(none)\\n" || printf "all:%%s\\n" "$@"\n' > "$home/tests/run-all.sh"
printf '#!/bin/sh\n[ "$#" -eq 0 ] && printf "docker:(none)\\n" || printf "docker:%%s\\n" "$@"\n' > "$home/tests/run-in-docker.sh"
chmod 755 "$home/.scripts/deps/check-deps.sh" "$home/tests/run-all.sh" "$home/tests/run-in-docker.sh"

actual=$(HOME="$home" "$CONFIG" install --yes --dry-run)
assert_equals 'config install wraps check-deps --fix and passes flags through' \
    'deps:--fix
deps:--yes
deps:--dry-run' "$actual"

actual=$(HOME="$home" "$CONFIG" test)
assert_equals 'config test runs the host suite' 'all:(none)' "$actual"

actual=$(HOME="$home" "$CONFIG" test -q)
assert_equals 'config test -q passes -q through' 'all:-q' "$actual"

actual=$(HOME="$home" "$CONFIG" test --docker)
assert_equals 'config test --docker runs the whole suite in docker' 'docker:(none)' "$actual"

actual=$(HOME="$home" "$CONFIG" test --docker check-deps)
assert_equals 'config test --docker <suite> passes the suite name' 'docker:check-deps' "$actual"

output=$(HOME="$home" "$CONFIG" test --bogus 2>&1)
status=$?
assert_equals 'config test rejects an unknown flag with exit 2' '2' "$status"
assert_contains 'the rejection prints usage' 'usage: config test' "$output"

output=$(HOME="$home" "$CONFIG" test a b 2>&1)
status=$?
assert_equals 'config test rejects a second positional argument with exit 2' '2' "$status"
assert_contains 'the second-positional-argument rejection prints usage' 'usage: config test' "$output"

HOME="$home" "$CONFIG" test -q --docker >/dev/null 2>&1
status=$?
assert_equals 'config test rejects -q together with --docker with exit 2' '2' "$status"

HOME="$home" "$CONFIG" test -q some-suite >/dev/null 2>&1
status=$?
assert_equals 'config test rejects -q together with a suite name with exit 2' '2' "$status"

output=$(HOME="$home" "$CONFIG" test --watch --docker 2>/dev/null)
status=$?
assert_equals 'config test rejects --watch together with --docker with exit 2' '2' "$status"
assert_equals 'the rejection never runs the docker stub' '' "$output"

watch_home=$(make_fixture_home watch)
mkdir -p "$watch_home/tests"
printf '#!/bin/sh\nprintf "run\\n" >> "%s/runs"\n' "$watch_home" > "$watch_home/tests/run-all.sh"
chmod 755 "$watch_home/tests/run-all.sh"
( HOME="$watch_home" "$CONFIG" test --watch >/dev/null 2>&1 & echo $! > "$watch_home/watch.pid" )
sleep 2
runs_before=$(grep -c '' "$watch_home/runs" 2>/dev/null || echo 0)
printf 'changed\n' >> "$watch_home/tracked.txt"
git --git-dir="$watch_home/.cfg" --work-tree="$watch_home" add tracked.txt
sleep 3
runs_after=$(grep -c '' "$watch_home/runs" 2>/dev/null || echo 0)
watch_pid=$(cat "$watch_home/watch.pid")
kill "$watch_pid" 2>/dev/null || true
pkill -P "$watch_pid" 2>/dev/null || true
assert_equals 'watch runs the suite once at start' '1' "$runs_before"
[ "$runs_after" -gt "$runs_before" ] && reran=yes || reran="no ($runs_before -> $runs_after)"
assert_equals 'watch reruns the suite when a tracked file changes' 'yes' "$reran"

for _attempt in 1 2 3 4 5 6 7 8 9 10; do
    kill -0 "$watch_pid" 2>/dev/null || break
    sleep 0.2
done
kill -0 "$watch_pid" 2>/dev/null && still_running=yes || still_running=no
assert_equals 'the watch loop is gone after kill' 'no' "$still_running"

# --- help -------------------------------------------------------------------

# `config help`, `config --help` and `config -h` all list the subcommands.
# The listing is generated from the `# help:` line in each config-<sub>, so a
# new utility that lands beside the dispatcher documents itself; there is no
# second list to update, and no way for the two to drift.

for help_form in help --help -h; do
    output=$(HOME="$home" "$CONFIG" "$help_form" 2>&1)
    status=$?
    assert_equals "config $help_form exits 0" '0' "$status"
    missing=''
    for sub in $EXPECTED_SUBCOMMANDS; do
        printf '%s\n' "$output" | grep -q "[^-]$sub" || missing="$missing $sub"
    done
    assert_equals "config $help_form lists every subcommand" '' "$missing"
done

output=$(HOME="$home" "$CONFIG" help 2>&1)
assert_contains 'help says unknown verbs go to git' 'git' "$output"

# Every subcommand carries its own one-line description, and help prints it.
undescribed=''
for sub in $EXPECTED_SUBCOMMANDS; do
    line=$(sed -n 's/^# help: //p' "$CONFIG_DIR/config-$sub" | head -1)
    [ -n "$line" ] || undescribed="$undescribed $sub"
done
assert_equals 'every config-<sub> carries a "# help:" description' '' "$undescribed"

unprinted=''
for sub in $EXPECTED_SUBCOMMANDS; do
    line=$(sed -n 's/^# help: //p' "$CONFIG_DIR/config-$sub" | head -1)
    [ -n "$line" ] || continue
    printf '%s\n' "$output" | grep -qF "$line" || unprinted="$unprinted $sub"
done
assert_equals 'help prints each subcommand own description' '' "$unprinted"

# A help listing that scrolls off the screen is not read. The dispatcher has
# eight subcommands; this is a ceiling, not a target.
help_lines=$(printf '%s\n' "$output" | grep -c '')
[ "$help_lines" -le 30 ] && under_30=yes || under_30="no ($help_lines lines)"
assert_equals 'the help listing stays under 30 lines' 'yes' "$under_30"

# help must not reach the git passthrough, which would print git usage.
assert_equals 'help does not fall through to git' '' \
    "$(printf '%s' "$output" | grep -F 'usage: git' || true)"

# --- explicit git passthrough with -- ---------------------------------------

# `config -- <verb>` sends <verb> to git without consulting the sibling
# scripts. Without it, a config-<sub> that shares a name with a git command
# makes that git command unreachable through the dispatcher; `config help` is
# the first such name, and any future one lands the same way.

output=$(HOME="$home" "$CONFIG" -- help 2>&1)
status=$?
assert_equals 'config -- help exits 0' '0' "$status"
assert_contains 'config -- help reaches git, not the help listing' \
    'usage: git' "$output"
assert_equals 'config -- help does not print the subcommand listing' '' \
    "$(printf '%s' "$output" | grep -F 'config <command>' || true)"

expected=$(git --git-dir="$home/.cfg" --work-tree="$home" rev-parse HEAD)
actual=$(HOME="$home" "$CONFIG" -- rev-parse HEAD)
assert_equals 'config -- passes ordinary git verbs through unchanged' \
    "$expected" "$actual"

# The separator is consumed, not forwarded. Passing it on would turn
# `config -- log <path>` into `git log -- <path>`, a pathspec, which is a
# different command.
actual=$(HOME="$home" "$CONFIG" -- status --porcelain --untracked-files=no 2>&1)
assert_equals 'config -- status behaves like config status' '' "$actual"

# A verb that has no sibling script is unaffected by the separator.
output=$(HOME="$home" "$CONFIG" -- definitely-not-a-command 2>&1)
status=$?
assert_equals 'config -- with an unknown git verb still fails' '1' "$status"
assert_contains 'the failure is git own message' \
    "git: 'definitely-not-a-command' is not a git command" "$output"

# A bare `config --` has nothing to pass through. git treats it as no verb
# and prints its usage, which is the honest answer.
HOME="$home" "$CONFIG" -- >/dev/null 2>&1
status=$?
assert_equals 'a bare config -- does not succeed silently' '1' "$status"

# Only the FIRST argument is the separator. A later -- is a git pathspec and
# must survive untouched.
printf 'content\n' > "$home/passthrough.txt"
git --git-dir="$home/.cfg" --work-tree="$home" add passthrough.txt
actual=$(HOME="$home" "$CONFIG" diff --cached --name-only -- passthrough.txt)
assert_equals 'a -- later in the line stays a git pathspec' \
    'passthrough.txt' "$actual"
git --git-dir="$home/.cfg" --work-tree="$home" reset -q

# Sibling dispatch must not be reachable through the separator.
actual=$(HOME="$home" "$sibling_dir/config" -- probe one 2>&1 || true)
assert_equals 'config -- does not dispatch to a sibling script' '' \
    "$(printf '%s' "$actual" | grep -F 'probe:' || true)"

# --- reload -----------------------------------------------------------------

printf '#!/bin/sh\nprintf "tmux:%%s\\n" "$@"\n' > "$shim_dir/tmux"
chmod 755 "$shim_dir/tmux"

home=$(make_fixture_home reload)
actual=$(env -u TMUX HOME="$home" PATH="$shim_dir:$PATH" "$CONFIG" reload 2>/dev/null)
assert_equals 'reload outside tmux only prints the zsh line' 'source ~/.zshrc' "$actual"

actual=$(TMUX=fake HOME="$home" PATH="$shim_dir:$PATH" "$CONFIG" reload 2>/dev/null)
assert_equals 'reload inside tmux sources the tmux config first' \
    "tmux:source
tmux:$home/.config/tmux/tmux.conf
reloaded tmux config
source ~/.zshrc" "$actual"

assert_equals 'the per-shell tmux source is gone from .zshrc' '' \
    "$(grep -n 'tmux source' "$DOTFILES_ROOT/.zshrc" || true)"

finish
