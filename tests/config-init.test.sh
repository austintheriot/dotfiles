#!/bin/bash
#
# Tests `config init` -- the post-clone half of the bootstrap.
#
# Every test runs against a fixture HOME under $FIXTURES: a bare repo at .cfg
# with a worktree, so nothing here touches the real dotfiles repo. The
# fixture carries its own .scripts/config tree, because `config init` runs
# the sibling config-<sub> scripts and this suite must not run the real
# install-hooks against the real ~/.cfg.
#
# The install and build steps are stubbed. Both shell out to the network and
# to cargo, and this suite's subject is the orchestration: which steps run,
# in what order, under which flags. `--dry-run` is asserted against the real
# scripts, because it is the one path that runs no step at all.
#
# Usage: ~/tests/config-init.test.sh

. "$(dirname "$0")/lib.sh"

CONFIG_DIR="$DOTFILES_ROOT/.scripts/config"
INIT="$CONFIG_DIR/config-init"

assert_succeeds 'config-init exists and is executable' test -x "$INIT"

# A fixture HOME with a bare repo at .cfg and a stubbed .scripts/config tree.
# The stubs record their invocation to $fixture_home/.calls, in order, so a
# test can assert what ran without any step touching the machine.
make_init_home() {
    local fixture_home="$FIXTURES/home-$1"
    mkdir -p "$fixture_home/.scripts/config"

    local seed
    seed=$(make_repo "seed-$1")
    git clone -q --bare "$seed" "$fixture_home/.cfg"

    cp "$CONFIG_DIR/config" "$fixture_home/.scripts/config/config"
    cp "$CONFIG_DIR/usage.sh" "$fixture_home/.scripts/config/usage.sh"
    cp "$INIT" "$fixture_home/.scripts/config/config-init"

    local sub
    for sub in install-hooks install build; do
        cat > "$fixture_home/.scripts/config/config-$sub" <<STUB
#!/bin/sh
# help: stub
# usage: config $sub
printf '%s %s\n' "$sub" "\$*" >> "$fixture_home/.calls"
STUB
        chmod +x "$fixture_home/.scripts/config/config-$sub"
    done

    : > "$fixture_home/.calls"
    printf '%s\n' "$fixture_home"
}

calls_of() {
    cat "$1/.calls" 2>/dev/null
}

# --- the steps it runs ------------------------------------------------------

home=$(make_init_home steps)
output=$(cd "$home" && HOME="$home" "$home/.scripts/config/config" init --yes 2>&1)
status=$?

assert_equals 'config init --yes exits 0 on a fresh fixture' '0' "$status"

calls=$(calls_of "$home")
assert_contains 'install-hooks runs' 'install-hooks' "$calls"
assert_contains 'install runs' 'install' "$calls"
assert_contains 'build runs' 'build' "$calls"

# Order is the contract, not an accident. install-hooks puts `config` on PATH
# and must precede anything a later step or a later shell resolves through it;
# install places the Rust toolchain that build then compiles with, so a build
# that ran first would fail on a machine with no cargo.
assert_equals 'the steps run in order: hooks, install, build' \
    'install-hooks install build' \
    "$(printf '%s\n' "$calls" | awk '{print $1}' | tr '\n' ' ' | sed 's/ $//')"

# --yes must reach the install step, or an unattended bootstrap stops at the
# first prompt with no terminal to answer it.
assert_contains 'install receives --yes' 'install --yes' "$calls"

# --- the git config it sets -------------------------------------------------

# The one setting the Atlassian technique depends on. Without it every
# untracked file in $HOME shows up in `config status`, which is what makes
# the bare-repo-over-home approach usable at all.
actual=$(git --git-dir="$home/.cfg" config --get status.showUntrackedFiles)
assert_equals 'init sets status.showUntrackedFiles to no' 'no' "$actual"

# --- idempotency ------------------------------------------------------------

# The bootstrap is re-run by hand on a machine already set up, and by the
# container suite twice in a row. A second run must converge, not fail.
home=$(make_init_home idempotent)
(cd "$home" && HOME="$home" "$home/.scripts/config/config" init --yes >/dev/null 2>&1)
output=$(cd "$home" && HOME="$home" "$home/.scripts/config/config" init --yes 2>&1)
status=$?
assert_equals 'a second config init --yes also exits 0' '0' "$status"
actual=$(git --git-dir="$home/.cfg" config --get status.showUntrackedFiles)
assert_equals 'status.showUntrackedFiles is still no after a second run' 'no' "$actual"

# --- dry run ----------------------------------------------------------------

# Asserted against the real config-<sub> scripts, not the stubs: --dry-run's
# whole claim is that it runs no step, so a stub proving it would prove
# nothing about the real ones.
home=$(make_init_home dryrun)
rm -f "$home/.scripts/config/config-install-hooks" \
      "$home/.scripts/config/config-install" \
      "$home/.scripts/config/config-build"
git --git-dir="$home/.cfg" config --unset status.showUntrackedFiles 2>/dev/null

output=$(cd "$home" && HOME="$home" "$home/.scripts/config/config" init --dry-run 2>&1)
status=$?
assert_equals 'config init --dry-run exits 0' '0' "$status"
# Each step is named in prose rather than by script name, so these assert
# the step is accounted for without freezing the wording.
assert_contains 'dry run accounts for the git setting' 'showUntrackedFiles' "$output"
assert_contains 'dry run accounts for the hooks step' 'hooks' "$output"
assert_contains 'dry run accounts for the install step' 'dependencies' "$output"
assert_contains 'dry run accounts for the build step' 'build' "$output"
assert_contains 'dry run says it changed nothing' 'nothing was changed' "$output"

# Every line is marked as hypothetical. A dry run whose output reads like a
# transcript of work done is worse than no dry run.
assert_equals 'every step line is marked "would"' '' \
    "$(printf '%s\n' "$output" | grep '^config init: \[' | grep -v 'would' || true)"

# The proof that nothing ran: the setting it would have written is absent,
# and the missing step scripts were never invoked.
actual=$(git --git-dir="$home/.cfg" config --get status.showUntrackedFiles 2>/dev/null || printf 'unset')
assert_equals 'dry run writes no git config' 'unset' "$actual"
assert_equals 'dry run records no calls' '' "$(calls_of "$home")"

# --- usage ------------------------------------------------------------------

output=$(HOME="$FIXTURES" "$INIT" --help 2>&1)
status=$?
assert_equals 'config init --help exits 0' '0' "$status"
assert_contains 'the usage block prints' 'usage: config init' "$output"

# --help must not run the bootstrap. usage_if_requested is called before any
# parsing for exactly this reason.
home=$(make_init_home help)
output=$(cd "$home" && HOME="$home" "$home/.scripts/config/config" init --help 2>&1)
assert_equals '--help runs no step' '' "$(calls_of "$home")"

# --- argument handling ------------------------------------------------------

home=$(make_init_home badarg)
output=$(cd "$home" && HOME="$home" "$home/.scripts/config/config" init --not-a-flag 2>&1)
status=$?
assert_equals 'an unknown flag exits 2' '2' "$status"
assert_equals 'an unknown flag runs no step' '' "$(calls_of "$home")"

# --- a machine with no Rust toolchain ---------------------------------------
#
# The build step is last because it is the only one that needs a compiler, and
# it must not cost the reader everything above it. rustup is a deps.conf entry
# with no automated install on some managers, so "no cargo yet" is a real
# state on a fresh remote box, and a container can omit the toolchain
# deliberately.
#
# The contract: report the gap, name the recovery, and still exit 0, because
# the hooks are linked, the dependencies are installed, and the shell works.
# A non-zero exit here would turn a usable machine into a failed bootstrap and
# would make the CI bootstrap job red for something that is not a bootstrap
# failure.
home=$(make_init_home nocargo)
cat > "$home/.scripts/config/config-build" <<'STUB'
#!/bin/sh
# help: stub
# usage: config build
printf 'build %s\n' "$*" >> "$HOME/.calls"
printf 'error: no such command: `cargo`\n' >&2
exit 1
STUB
chmod +x "$home/.scripts/config/config-build"

output=$(cd "$home" && HOME="$home" "$home/.scripts/config/config" init --yes 2>&1)
status=$?
assert_equals 'a failing build step still exits 0' '0' "$status"
assert_contains 'the failure is reported' 'build' "$output"
assert_contains 'the recovery is named' 'config build' "$output"

# The steps before it must have completed, which is the whole reason to
# tolerate this one.
calls=$(calls_of "$home")
assert_contains 'the hooks were still linked' 'install-hooks' "$calls"
assert_contains 'the dependencies were still installed' 'install --yes' "$calls"
actual=$(git --git-dir="$home/.cfg" config --get status.showUntrackedFiles)
assert_equals 'the git setting was still written' 'no' "$actual"


# --- a failing install step is not a success --------------------------------
#
# Reported from a bare root container: eleven dependencies failed to install
# and `config init` still walked to the end and reported done. The install
# step's exit code was simply not read.
#
# The build step is deliberately tolerant -- a machine with no cargo is a real
# state -- but the install step is not: a bootstrap that installed nothing has
# not bootstrapped anything.
home=$(make_init_home installfail)
cat > "$home/.scripts/config/config-install" <<'STUB'
#!/bin/sh
# help: stub
# usage: config install
printf 'install %s\n' "$*" >> "$HOME/.calls"
printf 'check-deps: 11 automated install(s) did not satisfy their check\n' >&2
exit 1
STUB
chmod +x "$home/.scripts/config/config-install"

output=$(cd "$home" && HOME="$home" "$home/.scripts/config/config" init --yes 2>&1)
status=$?
assert_equals 'a failing install step exits non-zero' '1' "$status"
assert_succeeds 'the failure names the install step' \
    grep -qi 'install' <<<"$output"

# What already succeeded must still be reported, so the reader knows the
# machine is partly set up rather than untouched.
assert_succeeds 'the output says what did succeed' \
    grep -qiE 'hook|checkout|place' <<<"$output"

# And it must not silently claim completion.
assert_equals 'it does not report plain success' '' \
    "$(printf '%s\n' "$output" | grep -x 'config init: done' || true)"


finish
