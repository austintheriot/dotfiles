#!/bin/bash
#
# Tests setup.sh -- the clone half of the bootstrap.
#
# setup.sh is the curl-piped entry point: it clones the bare repo, picks the
# branch, checks out, and then hands off to `config init`. This suite drives
# it against a local seed repo and a fixture HOME, so no test reaches the
# network or the real ~/.cfg.
#
# The handoff target is stubbed. `config init` has its own suite
# (config-init.test.sh); what matters here is that setup.sh reaches it with
# the right flags, and that everything it does before the handoff is correct.
#
# Usage: ~/tests/setup.test.sh

. "$(dirname "$0")/lib.sh"

SETUP="$DOTFILES_ROOT/setup.sh"

assert_succeeds 'setup.sh exists and is executable' test -x "$SETUP"

# A seed repo shaped like the real one: two platform branches, and a
# .scripts/config tree whose config-init is a stub that records its flags.
# setup.sh checks out from this, so the stub is what the checkout delivers.
make_seed() {
    local seed="$FIXTURES/seed-$1"
    mkdir -p "$seed/.scripts/config"
    git -C "$seed" init -q -b mac

    cp "$DOTFILES_ROOT/.scripts/config/config" "$seed/.scripts/config/config"
    cp "$DOTFILES_ROOT/.scripts/config/usage.sh" "$seed/.scripts/config/usage.sh"

    cat > "$seed/.scripts/config/config-init" <<'STUB'
#!/bin/sh
# help: stub
# usage: config init
printf 'init %s\n' "$*" >> "$HOME/.calls"
STUB
    chmod +x "$seed/.scripts/config/config-init"

    printf 'mac branch marker\n' > "$seed/.marker"
    git -C "$seed" add -A
    git -C "$seed" -c user.email=t@t -c user.name=t commit -q -m 'mac'

    git -C "$seed" checkout -q -b linux
    printf 'linux branch marker\n' > "$seed/.marker"
    git -C "$seed" add -A
    git -C "$seed" -c user.email=t@t -c user.name=t commit -q -m 'linux'
    git -C "$seed" checkout -q mac

    printf '%s' "$seed"
}

new_home() {
    local fixture_home="$FIXTURES/home-$1"
    mkdir -p "$fixture_home"
    : > "$fixture_home/.calls"
    printf '%s' "$fixture_home"
}

# --- a clean clone ----------------------------------------------------------

seed=$(make_seed clean)
home=$(new_home clean)
output=$(HOME="$home" DOTFILES_PLATFORM=mac "$SETUP" --yes --repo "$seed" 2>&1)
status=$?

assert_equals 'setup.sh --yes exits 0 against a local seed' '0' "$status"
assert_succeeds 'the bare repo lands at ~/.cfg' test -d "$home/.cfg"
assert_succeeds 'the worktree is checked out into $HOME' test -f "$home/.marker"
assert_equals 'the detected branch is what got checked out' \
    'mac branch marker' "$(cat "$home/.marker" 2>/dev/null)"
assert_contains 'it hands off to config init with --yes' 'init --yes' "$(cat "$home/.calls")"

# The clone must be bare, and the work tree must be $HOME. A non-bare clone
# into ~/.cfg would put a second worktree there and track nothing in $HOME.
assert_equals 'the clone is bare' 'true' \
    "$(git --git-dir="$home/.cfg" config --get core.bare)"

# --- platform detection -----------------------------------------------------

seed=$(make_seed detect)
home=$(new_home detect)
HOME="$home" DOTFILES_PLATFORM=linux "$SETUP" --yes --repo "$seed" >/dev/null 2>&1
assert_equals 'a linux platform checks out the linux branch' \
    'linux branch marker' "$(cat "$home/.marker" 2>/dev/null)"

# An explicit --branch overrides detection, which is what makes a work or
# home branch reachable at all: those names are not derivable from uname.
seed=$(make_seed override)
home=$(new_home override)
HOME="$home" DOTFILES_PLATFORM=mac "$SETUP" --yes --repo "$seed" --branch linux >/dev/null 2>&1
assert_equals '--branch overrides platform detection' \
    'linux branch marker' "$(cat "$home/.marker" 2>/dev/null)"

# A platform uname cannot name must not be guessed into a branch. platform.sh
# already returns "unknown" rather than defaulting to mac; setup.sh must stop
# there and ask rather than checking out a branch for the wrong machine.
seed=$(make_seed unknown)
home=$(new_home unknown)
output=$(HOME="$home" DOTFILES_PLATFORM=unknown "$SETUP" --yes --repo "$seed" 2>&1)
status=$?
assert_equals 'an undetectable platform exits non-zero' '1' "$status"
assert_contains 'it says the platform could not be detected' 'branch' "$output"
assert_equals 'it runs no handoff' '' "$(cat "$home/.calls")"

# --- pre-existing files -----------------------------------------------------

# The failure that makes a naive bootstrap script dangerous. A fresh machine
# usually already has a .zshrc, and `git checkout` refuses to overwrite it.
# Losing the user's file is not an option, and neither is aborting with git's
# raw message, so the file is backed up and the checkout retried.
seed=$(make_seed collide)
home=$(new_home collide)
printf 'the user own zshrc\n' > "$home/.marker"
output=$(HOME="$home" DOTFILES_PLATFORM=mac "$SETUP" --yes --repo "$seed" 2>&1)
status=$?

assert_equals 'a colliding file does not fail the bootstrap' '0' "$status"
assert_equals 'the tracked version wins in $HOME' \
    'mac branch marker' "$(cat "$home/.marker" 2>/dev/null)"

backup_dir=$(find "$home" -maxdepth 1 -type d -name '.dotfiles-backup-*' 2>/dev/null | head -1)
assert_succeeds 'a timestamped backup directory is created' test -n "$backup_dir"
assert_equals 'the backup holds the original content under its original name' \
    'the user own zshrc' "$(cat "$backup_dir/.marker" 2>/dev/null)"

# The backup must not be inside the checkout it is protecting, or the next
# `config status` reports it and the next checkout collides with it too.
assert_succeeds 'the backup is not itself tracked' \
    test -z "$(git --git-dir="$home/.cfg" --work-tree="$home" ls-files "$(basename "$backup_dir")")"
assert_contains 'the output says a file was moved aside' 'marker' "$output"

# --- refusing to clobber an existing setup ----------------------------------

# Re-running the clone half on a machine that already has ~/.cfg must not
# re-clone over it. That directory is the repo; blowing it away would take
# any unpushed commit with it.
seed=$(make_seed existing)
home=$(new_home existing)
HOME="$home" DOTFILES_PLATFORM=mac "$SETUP" --yes --repo "$seed" >/dev/null 2>&1
: > "$home/.calls"
output=$(HOME="$home" DOTFILES_PLATFORM=mac "$SETUP" --yes --repo "$seed" 2>&1)
status=$?
assert_equals 'a second run exits non-zero rather than re-cloning' '1' "$status"
assert_contains 'it names the existing repo' '.cfg' "$output"
assert_contains 'it points at config init as the way forward' 'config init' "$output"

# --- dry run ----------------------------------------------------------------

seed=$(make_seed dryrun)
home=$(new_home dryrun)
output=$(HOME="$home" DOTFILES_PLATFORM=mac "$SETUP" --dry-run --repo "$seed" 2>&1)
status=$?
assert_equals 'setup.sh --dry-run exits 0' '0' "$status"
assert_succeeds 'dry run creates no repo' test ! -d "$home/.cfg"
assert_succeeds 'dry run checks out nothing' test ! -f "$home/.marker"
assert_equals 'dry run runs no handoff' '' "$(cat "$home/.calls")"
assert_contains 'dry run names the branch it would use' 'mac' "$output"

# --- usage ------------------------------------------------------------------

output=$("$SETUP" --help 2>&1)
status=$?
assert_equals 'setup.sh --help exits 0' '0' "$status"
assert_contains 'the usage block prints' 'usage: setup.sh' "$output"

home=$(new_home badarg)
output=$(HOME="$home" "$SETUP" --not-a-flag 2>&1)
status=$?
assert_equals 'an unknown flag exits 2' '2' "$status"
assert_succeeds 'an unknown flag clones nothing' test ! -d "$home/.cfg"

# --- POSIX sh ---------------------------------------------------------------

# It is fetched by curl and piped to sh. A bashism here fails on a box whose
# /bin/sh is dash, which is every Debian and Ubuntu machine -- exactly the
# remote-server case this script exists for.
assert_contains 'the shebang is /bin/sh' '#!/bin/sh' "$(head -1 "$SETUP")"

if command -v dash >/dev/null 2>&1; then
    assert_succeeds 'setup.sh parses under dash' dash -n "$SETUP"
else
    skip 'setup.sh parses under dash' 'dash is not installed'
fi

# --- an unreachable remote ---------------------------------------------------
#
# Reported from a real run: on a box with no DNS, `curl | sh` died with
# "Could not resolve host" before setup.sh ever started, and once the script
# was copied over by hand the clone failed with git's own message. Git's
# wording names the URL, not the cause, so the reader learns that a clone
# failed rather than that the machine cannot resolve anything.
#
# A remote that cannot be reached is the single most likely failure on a
# freshly provisioned box, which is exactly the machine this script is for, so
# it is worth naming.
#
# Driven with a URL whose host cannot resolve, not by breaking DNS: the suite
# must not depend on network state, and .test domains are reserved by RFC 2606
# precisely so they never resolve.
seed=$(make_seed unreachable)
home=$(new_home unreachable)
output=$(HOME="$home" DOTFILES_PLATFORM=mac "$SETUP" --yes \
    --repo 'https://nonexistent.invalid.test/dotfiles.git' 2>&1)
status=$?

assert_equals 'an unreachable remote exits non-zero' '1' "$status"
assert_succeeds 'the failure names the remote as unreachable' \
    grep -qiE 'reach|resolve|network|connect' <<<"$output"

# Nothing half-created. A ~/.cfg left behind by a failed clone would make the
# next run refuse with "already cloned", which is the worst possible outcome:
# the reader fixes their DNS and then cannot re-run the script.
assert_succeeds 'a failed clone leaves no ~/.cfg behind' test ! -d "$home/.cfg"
assert_equals 'a failed clone runs no handoff' '' "$(cat "$home/.calls")"

# A local path must not be probed for reachability. The tests and the
# container both clone from a path or a bare repo on disk, and treating those
# as unreachable would break every other assertion in this file.
seed=$(make_seed localpath)
home=$(new_home localpath)
status=0
HOME="$home" DOTFILES_PLATFORM=mac "$SETUP" --yes --repo "$seed" >/dev/null 2>&1 || status=$?
assert_equals 'a local repo path is never probed as a network host' '0' "$status"

# --- a piped run is unattended all the way through --------------------------
#
# The curl one-liner needed `sh -s -- --yes`, and the --yes was doing work
# that the script can determine for itself: a piped run has stdin bound to
# the pipe, so there is no terminal to answer any prompt.
#
# setup.sh already skipped its OWN branch prompt on that basis, but handed off
# to `config init` with no arguments, which then prompts per dependency
# install with nobody there. So the flag was not cosmetic -- without it a
# piped run stalled or silently declined every install.
#
# No terminal therefore implies unattended, and the one-liner needs no flags.
seed=$(make_seed piped)
home=$(new_home piped)

# `< /dev/null` is what makes stdin not a terminal, which is the same
# condition `curl ... | sh` creates.
HOME="$home" DOTFILES_PLATFORM=mac "$SETUP" --repo "$seed" < /dev/null > /dev/null 2>&1

assert_contains 'a non-interactive run hands off unattended' \
    'init --yes' "$(cat "$home/.calls")"

# And it must still have checked out, rather than stopping at the prompt it
# skipped.
assert_equals 'a non-interactive run still checks out' \
    'mac branch marker' "$(cat "$home/.marker" 2>/dev/null)"

# An explicit --yes stays equivalent, so every existing caller (Docker, the
# CI bootstrap job, the documented one-liner) keeps working unchanged.
seed=$(make_seed pipedexplicit)
home=$(new_home pipedexplicit)
HOME="$home" DOTFILES_PLATFORM=mac "$SETUP" --yes --repo "$seed" < /dev/null > /dev/null 2>&1
assert_contains 'an explicit --yes behaves the same way' \
    'init --yes' "$(cat "$home/.calls")"

# The reverse must not regress: with a terminal and no --yes, the handoff
# stays interactive so `config init` can prompt per install. Driven only
# where a pty is available, because that is what makes stdin a terminal.
# The interactive path is deliberately NOT driven here.
#
# It needs a pty that stays open long enough to answer one prompt, and two
# attempts made things worse rather than better: `script -q /dev/null` treats
# a piped newline as EOF, so the run died at the prompt without reaching the
# handoff, and a pty.spawn whose input callback kept returning a newline fed
# input forever and hung the whole suite. A test that hangs run-all.sh is far
# worse than an honest gap.
#
# What matters most is covered above and does not need a pty: a run with no
# terminal hands off unattended, and an explicit --yes is equivalent. The
# interactive branch prompt was verified by hand under `script`, which does
# print "detected mac. Branch to check out [mac]:" as intended.
skip 'an interactive run does not force --yes' \
    'needs a pty harness; two attempts regressed into EOF and a hang'

# --- installing git when it is missing --------------------------------------
#
# Reported from a bare root container: the one-liner printed the command that
# installs git and exited, so a bare image needed two commands. The detection
# to print that line is the same detection needed to run it, so the script now
# installs git itself.
#
# Piped installs without asking, because the curl one-liner is ALWAYS piped:
# refusing there would leave the bare-image case -- the case this script exists
# for -- needing two commands forever. With a terminal it asks first, since a
# developer machine missing git is more likely a surprise than an intent.
#
# A stub package manager on PATH, so no test installs anything for real.
git_seed=$(make_seed gitinstall)
git_home=$(new_home gitinstall)

nogit_bin="$FIXTURES/nogit-bin"
mkdir -p "$nogit_bin"

# apt-get records its invocation and then creates a fake git, so the script's
# post-install `command -v git` succeeds and the run can continue.
cat > "$nogit_bin/apt-get" <<STUB
#!/bin/sh
printf 'apt-get %s\n' "\$*" >> "$FIXTURES/apt-git.log"
cat > "$nogit_bin/git" <<'GITSTUB'
#!/bin/sh
exit 0
GITSTUB
chmod +x "$nogit_bin/git"
exit 0
STUB
chmod +x "$nogit_bin/apt-get"

for passthrough in sh printf id command test cat mkdir rm sed grep uname dirname readlink; do
    real=$(command -v "$passthrough" 2>/dev/null) || continue
    ln -sf "$real" "$nogit_bin/$passthrough"
done

: > "$FIXTURES/apt-git.log"

# Piped (no terminal): installs without asking.
PATH="$nogit_bin" HOME="$git_home" DOTFILES_PLATFORM=linux \
    "$SETUP" --repo "$git_seed" < /dev/null > "$FIXTURES/gitinstall.out" 2>&1 || true

apt_git_log=$(cat "$FIXTURES/apt-git.log" 2>/dev/null)
assert_succeeds 'a piped run installs git rather than only naming it' \
    test -n "$apt_git_log"
assert_contains 'it installs git specifically' 'git' "$apt_git_log"

# And it must say so rather than installing silently. A one-liner that mutates
# the system without a word is worse than one that asks.
assert_succeeds 'it announces the install' \
    grep -qiE 'install|git' "$FIXTURES/gitinstall.out"

# No package manager at all: nothing to run, so it must still refuse with the
# docs URL rather than pretending.
nopm_bin="$FIXTURES/nopm-bin"
mkdir -p "$nopm_bin"
for passthrough in sh printf id command test cat sed grep uname dirname readlink; do
    real=$(command -v "$passthrough" 2>/dev/null) || continue
    ln -sf "$real" "$nopm_bin/$passthrough"
done

nopm_home=$(new_home nopm)
output=$(PATH="$nopm_bin" HOME="$nopm_home" DOTFILES_PLATFORM=linux \
    "$SETUP" --repo "$git_seed" < /dev/null 2>&1 || true)
assert_succeeds 'with no package manager it still refuses' \
    grep -qi 'git' <<<"$output"
assert_contains 'and points at the docs' 'git-scm.com' "$output"

# --dry-run must never install git, since its whole contract is changing
# nothing.
: > "$FIXTURES/apt-git.log"
dry_home=$(new_home gitdry)
PATH="$nogit_bin" HOME="$dry_home" DOTFILES_PLATFORM=linux \
    "$SETUP" --dry-run --repo "$git_seed" < /dev/null > /dev/null 2>&1 || true
assert_equals 'a dry run installs no git' '' "$(cat "$FIXTURES/apt-git.log" 2>/dev/null)"


finish
