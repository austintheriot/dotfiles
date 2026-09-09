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
    for sub in install-hooks prereqs install build; do
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

# Order is the contract, not an accident, and it encodes one rule: a thin
# shell layer installs only what is needed to RUN the Rust engine, then the
# engine installs everything else.
#
#   install-hooks  puts `config` on PATH, so anything a later step or a later
#                  shell resolves through it can be found.
#   prereqs        installs cc and rustup, in shell, because these are the two
#                  things the engine cannot install without already existing.
#   build          compiles config-cli with the toolchain prereqs just placed.
#   install        runs the engine, which needs the binary build produced.
#
# BUILD BEFORE INSTALL is the correction. The previous order was
# hooks, install, build, on the stated reasoning that "install places the Rust
# toolchain that build then compiles with". That was true while the deps
# engine was shell. It stopped being true when the engine became config-cli:
# the install step is `exec config-cli deps install`
# (.scripts/config/config-install:13), and config-cli is what build PRODUCES,
# so install depended on the output of a step that ran after it.
#
# Reproduced against the README one-liner in clean debian:bookworm and
# ubuntu:24.04 before this changed:
#   config init: [3/4] install the missing tracked dependencies
#   /root/.scripts/config/config-install: 13: exec: config-cli: not found
assert_equals 'the steps run in order: hooks, prereqs, build, install' \
    'install-hooks prereqs build install' \
    "$(printf '%s\n' "$calls" | awk '{print $1}' | tr '\n' ' ' | sed 's/ $//')"

# The engine cannot be the thing that installs the engine's own toolchain.
# Asserted as a position, not only as membership: prereqs after build is the
# bug in a different costume.
assert_succeeds 'prereqs runs before build' \
    test "$(printf '%s\n' "$calls" | awk '{print $1}' | grep -n '^prereqs$' | cut -d: -f1)" \
    -lt "$(printf '%s\n' "$calls" | awk '{print $1}' | grep -n '^build$' | cut -d: -f1)"
assert_succeeds 'build runs before install' \
    test "$(printf '%s\n' "$calls" | awk '{print $1}' | grep -n '^build$' | cut -d: -f1)" \
    -lt "$(printf '%s\n' "$calls" | awk '{print $1}' | grep -n '^install$' | cut -d: -f1)"

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
assert_contains 'dry run accounts for the prereqs step' 'toolchain' "$output"
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

# --- a failing build step is fatal ------------------------------------------
#
# THIS INVERTS AN EARLIER CONTRACT, deliberately, and the old reasoning is
# kept here because it reads as sound and should not be reinstated by
# someone who only sees the new assertions.
#
# The build step used to run LAST and was the one step allowed to fail
# without failing the bootstrap. The argument: rustup is a deps.toml entry
# with no automated install on some managers, so "no cargo yet" is a real
# state on a fresh remote box, a container can omit the toolchain on
# purpose, and everything above the build step already left a working shell.
# Exiting non-zero would have reported a usable machine as a failed
# bootstrap.
#
# Two facts changed, and each on its own is enough:
#   1. `config prereqs` now runs BEFORE the build and installs cc and rustup,
#      so "no cargo" is no longer a state the build step can be reached in.
#      A build failure past that point is a compile failure, not a missing
#      toolchain.
#   2. The build step now runs BEFORE the install step, because the install
#      step is `exec config-cli deps install` and config-cli is what the
#      build produces. Tolerating a build failure would walk straight into
#      "exec: config-cli: not found", which is the original bug with a worse
#      message.
home=$(make_init_home buildfails)
cat > "$home/.scripts/config/config-build" <<'STUB'
#!/bin/sh
# help: stub
# usage: config build
printf 'build %s\n' "$*" >> "$HOME/.calls"
printf 'error: could not compile `config-cli`\n' >&2
exit 1
STUB
chmod +x "$home/.scripts/config/config-build"

output=$(cd "$home" && HOME="$home" "$home/.scripts/config/config" init --yes 2>&1)
status=$?
assert_equals 'a failing build step exits non-zero' '1' "$status"
assert_succeeds 'the failure names the build step' \
    printf '%s' "$output" | grep -q 'build step failed'

# The message must not send the reader back to the package manager: prereqs
# already succeeded, so a compile failure is not a missing dependency.
assert_succeeds 'the failure says the toolchain is already installed' \
    printf '%s' "$output" | grep -q 'cc and rustup are installed'

# The steps before it must still have completed.
calls=$(calls_of "$home")
assert_contains 'the hooks were still linked' 'install-hooks' "$calls"
assert_contains 'the toolchain step still ran' 'prereqs' "$calls"
actual=$(git --git-dir="$home/.cfg" config --get status.showUntrackedFiles)
assert_equals 'the git setting was still written' 'no' "$actual"

# And the install step must NOT have run: it execs the binary this step
# failed to produce.
assert_equals 'the install step does not run after a failed build' '' \
    "$(printf '%s\n' "$calls" | awk '{print $1}' | grep -x 'install' || true)"


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
printf 'deps: 11 automated install(s) did not satisfy their check\n' >&2
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


# --- the build step sees a toolchain the install step just placed -----------
#
# Reported from a bare container: the run ended with "done, except the build
# step / run `config build` once cargo is on PATH" on a machine where the
# install step had just installed rustup successfully. rustup writes to
# ~/.cargo/bin, which is not on a default non-login PATH, so config-build
# could not see the cargo that now existed.
#
# The deps engine already solves this for itself -- it prepends ~/.local/bin and
# ~/.cargo/bin so a `command -v` check does not fail on the line after its own
# install succeeded. config init needs the same, because it is the caller that
# runs an install and a build in one process.
home=$(make_init_home cargopath)

# A cargo that exists ONLY in ~/.cargo/bin, exactly as rustup leaves it, and a
# config-build that fails unless it can find it.
mkdir -p "$home/.cargo/bin"
printf '#!/bin/sh\nexit 0\n' > "$home/.cargo/bin/cargo"
chmod +x "$home/.cargo/bin/cargo"

cat > "$home/.scripts/config/config-build" <<'STUB'
#!/bin/sh
# help: stub
# usage: config build
printf 'build %s\n' "$*" >> "$HOME/.calls"
command -v cargo >/dev/null 2>&1 || exit 1
printf 'build-saw-cargo\n' >> "$HOME/.calls"
STUB
chmod +x "$home/.scripts/config/config-build"

# A PATH without ~/.cargo/bin, which is what a fresh non-login shell has.
output=$(cd "$home" && HOME="$home" PATH="/usr/bin:/bin" \
    "$home/.scripts/config/config" init --yes 2>&1)
status=$?

calls=$(calls_of "$home")
assert_contains 'the build step finds a cargo in ~/.cargo/bin' \
    'build-saw-cargo' "$calls"
assert_equals 'the run succeeds rather than deferring the build' '0' "$status"
assert_equals 'it does not tell the reader to run config build later' '' \
    "$(printf '%s\n' "$output" | grep 'once cargo is on PATH' || true)"

# ~/.local/bin too: config-install-hooks puts `config` there and config-build
# installs config-cli there, and both are resolved by name afterwards.
#
# Driven rather than grepped. An earlier version grepped config-init for
# ".local/bin", which passed on a comment mentioning the path and would have
# kept passing with the export deleted.
home=$(make_init_home localbinpath)
mkdir -p "$home/.local/bin"
printf '#!/bin/sh\nexit 0\n' > "$home/.local/bin/only-in-local-bin"
chmod +x "$home/.local/bin/only-in-local-bin"

cat > "$home/.scripts/config/config-build" <<'STUB'
#!/bin/sh
# help: stub
# usage: config build
command -v only-in-local-bin >/dev/null 2>&1 && printf 'saw-local-bin\n' >> "$HOME/.calls"
STUB
chmod +x "$home/.scripts/config/config-build"

(cd "$home" && HOME="$home" PATH="/usr/bin:/bin" \
    "$home/.scripts/config/config" init --yes >/dev/null 2>&1)
assert_contains 'the steps inherit ~/.local/bin' \
    'saw-local-bin' "$(calls_of "$home")"


# --- it says how to reach config in the shell that just ran it --------------
#
# Reported from a bare container: `config init: done`, and then
# `config st` was "command not found". install-hooks links config into
# ~/.local/bin, but no process can change the PATH of the shell that started
# it, so the shell running the bootstrap can never see the new command by
# itself.
#
# Reporting success while leaving the reader with an uncallable command is a
# dead end. The closing output has to name the one line that fixes the current
# shell, and say where new shells get it from.
home=$(make_init_home pathhint)
output=$(cd "$home" && HOME="$home" "$home/.scripts/config/config" init --yes 2>&1)

assert_contains 'the closing output names the export' \
    'export PATH=' "$output"
assert_contains 'the export names local bin' '.local/bin' "$output"

# It must also say that new shells are already handled, or the reader assumes
# they have to run that export forever.
assert_succeeds 'it says where new shells pick it up' \
    grep -qiE 'profile|zshrc|new shell' <<<"$output"

# Only when config is not already reachable. On a machine where ~/.local/bin
# is already on PATH the hint is noise, and noise in a success message is how
# real warnings get ignored.
home=$(make_init_home pathhintquiet)
mkdir -p "$home/.local/bin"
output=$(cd "$home" && HOME="$home" PATH="$home/.local/bin:/usr/bin:/bin" \
    "$home/.scripts/config/config" init --yes 2>&1)
assert_equals 'no hint when local bin is already on PATH' '' \
    "$(printf '%s\n' "$output" | grep 'export PATH=' || true)"


# --- prereqs is the one step whose failure is fatal before anything else ----

# A machine with no cc and no rustup cannot build the engine, and every step
# after prereqs needs the engine. Continuing past a prereqs failure would
# reach `config-install` and reproduce the original
# "exec: config-cli: not found", which is a worse message for the same
# problem.
home=$(make_init_home prereqfail)
cat > "$home/.scripts/config/config-prereqs" <<'STUB'
#!/bin/sh
# help: stub
# usage: config prereqs
printf 'prereqs %s\n' "$*" >> "$FIXTURE_CALLS"
printf 'config prereqs: no known package manager here\n' >&2
exit 1
STUB
sed -i.bak "s|\$FIXTURE_CALLS|$home/.calls|" "$home/.scripts/config/config-prereqs"
rm -f "$home/.scripts/config/config-prereqs.bak"
chmod +x "$home/.scripts/config/config-prereqs"

output=$(cd "$home" && HOME="$home" "$home/.scripts/config/config" init --yes 2>&1)
status=$?

assert_equals 'a failing prereqs step exits non-zero' '1' "$status"
assert_succeeds 'the failure names the toolchain step' \
    printf '%s' "$output" | grep -q 'toolchain'

# Nothing after it may have run: those steps need what prereqs did not place.
calls=$(calls_of "$home")
assert_equals 'no step after prereqs runs' '' \
    "$(printf '%s\n' "$calls" | awk '{print $1}' | grep -E '^(build|install)$' || true)"

# --- the finished setup hands over to the new login shell ---------------
#
# The feature: after a successful `config init`, the reader is dropped into
# zsh rather than left in the bash they started in. Without it, the very
# last thing a fresh-machine bootstrap does is print "done" into a shell
# that has none of the configuration it just installed -- no prompt, no
# aliases, no PATH entry for `config`.
#
# THE CONSTRAINT THAT MAKES THIS DELICATE: an unattended run must NOT exec a
# shell. The documented entry point is `curl ... | sh`, CI runs the same
# script, and the Docker bootstrap harnesses run it too. Exec'ing an
# interactive zsh there replaces the bootstrap process with a shell reading
# a closed pipe, which either hangs the leg until its timeout or exits in a
# way that looks like a bootstrap failure.
#
# So the handover is gated on the same condition every other prompt in this
# script is gated on: a terminal on stdin. `--yes`, a pipe, and CI all take
# the quiet path and simply return.

assert_succeeds 'the init script names the login shell handover' \
    grep -q 'exec .*zsh\|handover\|hand over' "$INIT"

# The gate, asserted against the code rather than a comment: the handover
# must be inside a terminal test. A handover that runs unattended is the
# hang described above.
handover_line=$(grep -n 'exec .*zsh' "$INIT" | head -1 | cut -d: -f1)
assert_succeeds 'the handover exists as a line of code' test -n "$handover_line"

# Walk back from the handover to find its guard. `-t 0` (or -t 1) must
# appear above it: that is the "somebody is actually here" test.
guard_above=$(head -n "$handover_line" "$INIT" | grep -c '\-t 0\|\-t 1')
assert_succeeds 'the handover is guarded by a terminal test' \
    test "$guard_above" -gt 0

# A dry run changes nothing, including replacing the reader's shell.
dry_output=$("$INIT" --dry-run 2>&1 || true)
assert_succeeds 'a dry run does not exec a shell' \
    test "$(printf '%s' "$dry_output" | grep -c '^config init: done')" -eq 0

# The unattended path must reach its end and RETURN, which is what proves
# it did not exec. A run with no terminal on stdin prints the closing
# "done" and exits; if it exec'd zsh instead, this call would not return.
#
# Piped, not --yes: --yes is about declining prompts, and the point here is
# the absence of a terminal, which is the condition the guard reads.
piped_status=0
printf '' | "$INIT" --dry-run >/dev/null 2>&1 || piped_status=$?
assert_equals 'an unattended run returns rather than exec-ing a shell' \
    '0' "$piped_status"

# SHELL unset is the normal state in a bare container's non-login shell,
# which is the machine this handover exists for. `set -u` is on, so an
# unguarded $SHELL expansion aborts the script and turns a finished
# bootstrap into an error. Asserted against the code, because the abort
# happens only on a machine where getent is ALSO absent -- a combination
# this suite cannot produce on either CI platform.
assert_succeeds 'the SHELL fallback is guarded against being unset' \
    grep -q 'login_shell=${SHELL:-}' "$INIT"

unguarded=$(grep -c 'login_shell=\$SHELL$' "$INIT" || true)
assert_equals 'no unguarded SHELL expansion in the handover' '0' "$unguarded"

finish
