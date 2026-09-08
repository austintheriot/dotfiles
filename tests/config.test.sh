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

EXPECTED_SUBCOMMANDS='build stamp install-hooks init install prereqs test reload help doctor deps'

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
#
# `init` shadows `git init`, and that is the right way round. Reaching
# `git init` through this dispatcher would initialize a repository in the
# current directory using the bare repo's --git-dir, which is never what
# someone typing `config init` means. The bootstrap step is; `config -- init`
# still reaches git for anyone who wants it.
SHADOWED_ON_PURPOSE='help init'

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
# usage.sh is sourced by every config-<sub> to print its `# usage:` block, so
# it is part of the set a subcommand needs beside it, not an optional extra.
# Copying the scripts without it is what a partial install looks like, and the
# assertion below is what makes that visible rather than a line-23 not-found.
cp "$CONFIG_DIR/usage.sh" "$install_dir/usage.sh"
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

# The same refusal has to hold when the directory is reached through a
# symlink, which is the shape the three tests above cannot catch.
#
# `find "$dir" -maxdepth 0 -perm -o+w` stats the LINK, and a symlink is always
# mode lrwxr-xr-x, so the permission test reads the link's own bits and never
# the target's. Measured: for a symlink pointing at a 0777 directory, that
# find prints nothing and check_dir passes; `find -L` prints the path and
# refuses. A synced or restored home is exactly where ~/.local/bin or ~/tests
# arrives as a link, and the file's header calls these four directories the
# trust boundary -- anyone who can write to them runs code as this user.
real_tests=$(readlink -f "$home/tests")
mv "$home/tests" "$home/tests-real"
ln -s "$home/tests-real" "$home/tests"
chmod o+w "$home/tests-real"
output=$(HOME="$home" "$install_dir/config" install-hooks 2>&1)
status=$?
assert_equals 'install-hooks refuses a world-writable tests directory reached through a symlink' \
    '1' "$status"
chmod o-w "$home/tests-real"
rm -f "$home/tests"
mv "$home/tests-real" "$home/tests"
# Named so the variable is used and the restore is verifiable, rather than
# trusting the mv above silently.
assert_equals 'the tests directory is a real directory again' \
    "$real_tests" "$(readlink -f "$home/tests")"

# The other direction, which is the one that bites on Linux: a symlink to a
# SAFE directory must still be accepted. Without -L, GNU find matches the
# link's own lrwxrwxrwx mode and refuses a correct setup, so a fix that only
# tightened the check would trade a macOS hole for a Linux false refusal.
mv "$home/tests" "$home/tests-real"
ln -s "$home/tests-real" "$home/tests"
chmod 755 "$home/tests-real"
output=$(HOME="$home" "$install_dir/config" install-hooks 2>&1)
status=$?
assert_equals 'install-hooks accepts a safe tests directory reached through a symlink' \
    '0' "$status"
rm -f "$home/tests"
mv "$home/tests-real" "$home/tests"

# --- thin wrappers ----------------------------------------------------------

shim_dir="$FIXTURES/shims"
mkdir -p "$shim_dir"

home=$(make_fixture_home wrappers)
mkdir -p "$home/deps" "$home/tests"
printf '#!/bin/sh\nprintf "cli:%%s\\n" "$@"\n' > "$shim_dir/config-cli"
printf '#!/bin/sh\n[ "$#" -eq 0 ] && printf "all:(none)\\n" || printf "all:%%s\\n" "$@"\n' > "$home/tests/run-all.sh"
printf '#!/bin/sh\n[ "$#" -eq 0 ] && printf "docker:(none)\\n" || printf "docker:%%s\\n" "$@"\n' > "$home/tests/run-in-docker.sh"
chmod 755 "$shim_dir/config-cli" "$home/tests/run-all.sh" "$home/tests/run-in-docker.sh"

actual=$(HOME="$home" PATH="$shim_dir:$PATH" "$CONFIG" install --yes --dry-run)
assert_equals 'config install delegates to config-cli deps install and passes flags through' \
    'cli:deps
cli:install
cli:--yes
cli:--dry-run' "$actual"

actual=$(HOME="$home" "$CONFIG" test)
assert_equals 'config test runs the host suite' 'all:(none)' "$actual"

actual=$(HOME="$home" "$CONFIG" test -q)
assert_equals 'config test -q passes -q through' 'all:-q' "$actual"

actual=$(HOME="$home" "$CONFIG" test --docker)
assert_equals 'config test --docker runs the whole suite in docker' 'docker:(none)' "$actual"

actual=$(HOME="$home" "$CONFIG" test --docker deps-manifest)
assert_equals 'config test --docker <suite> passes the suite name' 'docker:deps-manifest' "$actual"

output=$(HOME="$home" "$CONFIG" test --bogus 2>&1)
status=$?
assert_equals 'config test rejects an unknown flag with exit 2' '2' "$status"
# clap prints its own usage line, capitalized ("Usage:"), rather than the
# lowercase "usage:" the retired shell script's hand-written line used.
assert_contains 'the rejection prints usage' 'Usage: config test' "$output"

output=$(HOME="$home" "$CONFIG" test a b 2>&1)
status=$?
assert_equals 'config test rejects a second positional argument with exit 2' '2' "$status"
assert_contains 'the second-positional-argument rejection prints usage' 'Usage: config test' "$output"

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
#
# config-reload is a shim now, like install and deps: every decision moved
# to config-cli, so this only has to prove the shim delegates and passes
# arguments through. The tmux and Alacritty behavior itself is config-cli's
# own contract, asserted against the built binary in
# crates/config-cli/tests/reload_behavior.rs.

home=$(make_fixture_home reload)
actual=$(HOME="$home" PATH="$shim_dir:$PATH" "$CONFIG" reload extra-arg)
assert_equals 'config reload delegates to config-cli reload and passes args through' \
    'cli:reload
cli:extra-arg' "$actual"

assert_equals 'the per-shell tmux source is gone from .zshrc' '' \
    "$(grep -n 'tmux source' "$DOTFILES_ROOT/.zshrc" || true)"

# --- doctor ------------------------------------------------------------

DOCTOR="$CONFIG_DIR/config-doctor"

assert_succeeds 'config doctor exists and is executable' test -x "$DOCTOR"
assert_equals 'doctor does not shadow a git verb' '' \
    "$(git --list-cmds=main,others 2>/dev/null | grep -x doctor || true)"

# Driven against the ambient $DOTFILES_ROOT, not a fixture: doctor gathers its
# expected side by reading the real crates/ tree through git, so it needs a
# real .cfg or .git there. The Docker runner's image carries no repository at
# all (see the "no repository here" skips elsewhere in this suite), so a call
# against $DOTFILES_ROOT would fail there for a reason unrelated to doctor
# itself.
if [ -d "$DOTFILES_ROOT/.cfg" ] || [ -d "$DOTFILES_ROOT/.git" ]; then
    # Status asserted separately from output. A silent failure (the binary not
    # on PATH, a crash before writing) would otherwise read as "silent because
    # everything is current".
    # The build's own status is checked, not discarded. Swallowing it made a
    # failed build read as a doctor defect: on a CI runner config-build
    # installs into $HOME/.local/bin while PATH resolves config-cli to
    # the workflow's own crates/target/release copy, so doctor compared an
    # unstamped binary and was correct to report a mismatch. The assertion
    # blamed doctor for it.
    build_out=$("$DOTFILES_ROOT/.scripts/config/config-build" 2>&1)
    build_status=$?
    # On failure the build's own output is printed, so the reader gets the
    # cause instead of a bare exit code. Printed rather than folded into the
    # assertion's value, because the value has to compare equal to '0'.
    [ "$build_status" -eq 0 ] || printf 'config-build said: %s\n' "$build_out" >&2
    assert_equals 'config-build succeeds before doctor is asked' '0' "$build_status"

    # doctor reads the binary PATH resolves, so the freshly installed one has
    # to be the one it finds. Without this the suite tests whichever copy the
    # environment happened to put first.
    doctor_out=$(PATH="${CONFIG_BIN_DIR:-$HOME/.local/bin}:$PATH" "$DOCTOR" 2>&1)
    doctor_status=$?
    assert_equals 'doctor exits 0 when every binary is current' '0' "$doctor_status"
    assert_equals 'doctor is silent when every binary is current' '' "$doctor_out"

    # doctor must look where config-build installs, even when DOTFILES_ROOT
    # is not $HOME. It defaulted to $DOTFILES_ROOT/.local/bin while
    # config-build:37 defaults to $HOME/.local/bin. Those coincide on a
    # developer machine, where DOTFILES_ROOT IS $HOME, and diverge in CI,
    # where DOTFILES_ROOT is the checkout -- so doctor reported
    # "not installed" for a binary that was installed correctly.
    #
    # Asserted by pointing DOTFILES_ROOT at a copy while leaving the binary
    # where config-build put it. Without the fix this prints
    # "config-cli: not installed".
    split_root="$FIXTURES/split-root"
    mkdir -p "$split_root"
    cp -R "$DOTFILES_ROOT/crates" "$split_root/crates"
    mkdir -p "$split_root/.scripts/config"
    cp "$DOTFILES_ROOT/.scripts/config/config-stamp" "$split_root/.scripts/config/"
    git -C "$split_root" init -q .
    git -C "$split_root" add -A >/dev/null 2>&1
    git -C "$split_root" -c user.email=t@t -c user.name=t commit -q -m 'split-root probe'

    split_out=$(DOTFILES_ROOT="$split_root" "$DOCTOR" 2>&1 || true)
    assert_equals 'doctor does not report a not-installed binary when DOTFILES_ROOT is not HOME' \
        '' "$(printf '%s' "$split_out" | grep 'not installed' || true)"

    # The behavior doctor exists for, asserted rather than checked by hand.
    #
    # The probe edits config-cli, not config-manifest. config-manifest is
    # library-only: it installs no binary, so doctor drops it from both sides
    # of the comparison and an edit there moves nothing. Pointed at
    # config-manifest this block asserted "stale" against a crate doctor was
    # correct to ignore, which is a silent pass rather than a real check.
    probe="$DOTFILES_ROOT/crates/config-cli/src/doctor.rs"
    cp "$probe" "$FIXTURES/doctor.rs.orig"
    printf '\n// staleness probe\n' >> "$probe"

    # Same PATH pinning as the fresh case above. Without it this asserted
    # "stale" against whichever config-cli the environment resolved
    # first, which on CI is the workflow's unstamped build -- so it would
    # have passed for the wrong reason while the fresh case failed.
    doctor_path="${CONFIG_BIN_DIR:-$HOME/.local/bin}:$PATH"
    doctor_out=$(PATH="$doctor_path" "$DOCTOR" 2>&1 || true)
    doctor_status=0
    PATH="$doctor_path" "$DOCTOR" >/dev/null 2>&1 || doctor_status=$?
    assert_equals 'doctor exits 1 when a binary is stale' '1' "$doctor_status"
    assert_contains 'doctor names the stale crate' 'config-cli' "$doctor_out"
    assert_contains 'doctor names the fix' 'config build' "$doctor_out"

    cp "$FIXTURES/doctor.rs.orig" "$probe"
    "$DOTFILES_ROOT/.scripts/config/config-build" >/dev/null 2>&1
else
    skip 'doctor exits 0 when every binary is current (no repository here)'
    skip 'doctor is silent when every binary is current (no repository here)'
    skip 'doctor exits 1 when a binary is stale (no repository here)'
    skip 'doctor names the stale crate (no repository here)'
    skip 'doctor names the fix (no repository here)'
fi

finish
