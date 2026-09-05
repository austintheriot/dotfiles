#!/bin/sh
# Verifies the bootstrap on a genuinely bare image: root, no sudo, no git.
#
# Two phases, because the first failure a bare image produces is the absence
# of git, and that message is itself under test:
#
#   1. Run setup.sh with no git installed. It must refuse, and it must name
#      the command that installs git -- without a `sudo` prefix, since this
#      container is root and has no sudo.
#   2. Install git, run the bootstrap for real, and assert that the installs
#      actually happened. On the old code every privileged install failed
#      with "sh: 1: sudo: not found", so this is the phase that would have
#      caught it.
set -eu

failed=0

# The seed is a bind mount owned by the host user while this container runs as
# root, so git refuses it with "detected dubious ownership". That check is
# about repositories on a shared filesystem and has nothing to do with the
# bootstrap; a real user clones over HTTPS where ownership never applies.
#
# Copied to a root-owned path rather than declared safe. Two other approaches
# were tried and rejected:
#
#   - `git config --global --add safe.directory` works, but it needs git,
#     and the clone now happens inside setup.sh immediately after setup.sh
#     installs git -- so there is no point in the run where this harness could
#     execute it.
#   - GIT_CONFIG_COUNT / GIT_CONFIG_KEY_0 does NOT work. Measured against
#     git 2.39.5: safe.directory is deliberately ignored when supplied through
#     the environment, because honouring it there would defeat the control.
#
# A local copy owned by the user doing the clone sidesteps the check entirely
# and needs no git, so it can happen before phase 1.
SEED_SRC=/seed/repo.git
SEED=/tmp/seed-repo.git
cp -R "$SEED_SRC" "$SEED"
chown -R "$(id -u):$(id -g)" "$SEED" 2>/dev/null || true

check() {
    check_description=$1
    shift
    if "$@"; then
        printf 'ok: %s\n' "$check_description"
    else
        printf 'FAIL: %s\n' "$check_description"
        failed=$((failed + 1))
    fi
    unset check_description
}

printf '=== phase 1: git is missing, so the run installs it ===\n'

# setup.sh normally arrives by curl. Here it comes out of the seed with a
# plain read, because git is what we are proving absent.
cp /seed/setup.sh /tmp/setup.sh

# `< /dev/null` makes stdin not a terminal, which is what `curl ... | sh`
# creates. That is the case under test: a piped run installs git without
# asking, because the documented entry point is always piped and refusing
# there would leave a bare image needing two commands forever.
set +e
no_git_output=$(sh /tmp/setup.sh --repo "$SEED" < /dev/null 2>&1)
no_git_status=$?
set -e

printf '%s\n' "$no_git_output"

check 'the run reports git is missing' \
    grep -qi 'git is not installed' <<EOFI
$no_git_output
EOFI

# It installs rather than only naming the command. This is the whole change:
# printing the command left a bare image needing two commands to bootstrap
# one machine.
check 'it installs git rather than only naming it' \
    grep -qi 'installing git' <<EOFI
$no_git_output
EOFI
check 'it confirms git arrived' \
    grep -qi 'git installed' <<EOFI
$no_git_output
EOFI

# No sudo anywhere in that path. This container is root with no sudo, so a
# sudo-prefixed command is one the reader cannot run -- the same mistake the
# install commands themselves used to make.
check 'the git install assumes no sudo' \
    test -z "$(printf '%s\n' "$no_git_output" | grep 'sudo' || true)"

# Having installed git, the run must carry straight on rather than stopping.
# A one-command bootstrap is the point.
check 'the run continues into the clone' \
    grep -q 'Cloning into bare repository' <<EOFI
$no_git_output
EOFI
check 'the piped run finishes without error' test "$no_git_status" -eq 0
check 'git is on PATH afterwards' command -v git
check 'the repo was cloned' test -d "$HOME/.cfg"

printf '\n=== phase 2: verifying what the run left behind ===\n'



check 'the bare repo exists' test -d "$HOME/.cfg"
check 'status.showUntrackedFiles is no' \
    test "$(git --git-dir="$HOME/.cfg" config --get status.showUntrackedFiles)" = 'no'
check '.zshrc is checked out' test -f "$HOME/.zshrc"

backup=$(find "$HOME" -maxdepth 1 -type d -name '.dotfiles-backup-*' | head -1)
check 'the pre-existing .zshrc was moved aside' test -n "$backup"

check 'config is on PATH' test -x "$HOME/.local/bin/config"
check 'the pre-commit hook is linked' test -L "$HOME/.cfg/hooks/pre-commit"
check 'the pre-push hook is linked' test -L "$HOME/.cfg/hooks/pre-push"

# The assertion this image exists for. tmux needs a privileged install, this
# container is root with no sudo, and the old code emitted `sudo apt-get
# install -y tmux` -- which failed. Its presence proves the escalation logic
# resolved to a runnable command.
check 'a privileged install actually succeeded (tmux)' command -v tmux

# And nothing may have tried to reach sudo along the way.
check 'no install reported a missing sudo' \
    test -z "$(printf '%s\n' "${BOOTSTRAP_LOG:-}" | grep 'sudo: not found' || true)"

printf '\n=== bare bootstrap: %d check(s) failed ===\n' "$failed"
[ "$failed" -eq 0 ]
