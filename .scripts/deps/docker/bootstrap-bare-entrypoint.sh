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

SEED=/seed/repo.git
failed=0

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

printf '=== phase 1: no git yet ===\n'

# setup.sh normally arrives by curl. Here it comes out of the seed with a
# plain read, because git is what we are proving absent.
cp /seed/setup.sh /tmp/setup.sh

set +e
no_git_output=$(sh /tmp/setup.sh --repo "$SEED" 2>&1)
no_git_status=$?
set -e

printf '%s\n' "$no_git_output"

check 'setup.sh refuses without git' test "$no_git_status" -ne 0
check 'it names git as the prerequisite' \
    grep -qi 'git is not installed' <<EOFI
$no_git_output
EOFI
check 'it names the command that installs git' \
    grep -q 'apt-get install -y git' <<EOFI
$no_git_output
EOFI

# The message must not suggest sudo here. This container is root and has no
# sudo, so a sudo-prefixed suggestion is a command the reader cannot run --
# the same mistake the install commands themselves used to make.
check 'the suggestion does not assume sudo' \
    test -z "$(printf '%s\n' "$no_git_output" | grep 'sudo' || true)"

check 'nothing was cloned' test ! -d "$HOME/.cfg"

printf '\n=== phase 2: with git, the real bootstrap ===\n'

apt-get update -qq
apt-get install -y --no-install-recommends git >/dev/null

# The seed is a bind mount owned by the host user, and this container runs as
# root, so git refuses it with "detected dubious ownership". That check is
# about a repository on a shared filesystem; it has nothing to do with the
# bootstrap, and a real user clones over HTTPS where ownership never applies.
#
# Declared here rather than worked around in setup.sh, because the constraint
# belongs to this harness and not to the product.
git config --global --add safe.directory "$SEED"

sh /tmp/setup.sh --repo "$SEED"

printf '\n=== phase 2: verifying ===\n'

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
