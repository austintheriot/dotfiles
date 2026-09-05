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

finish
