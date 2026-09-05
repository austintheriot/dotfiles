#!/bin/sh
# Runs the bootstrap inside Dockerfile.bootstrap, then asserts the state a
# finished bootstrap must leave behind.
#
# Mounted into the container beside the bare repo it clones from, rather than
# baked into the image, so iterating on the assertions costs no rebuild.
#
# Two halves, and the split matters:
#
#   1. Run setup.sh exactly as a remote server would, from the seed repo.
#   2. Check the things a "success" exit code does not prove. setup.sh can
#      exit 0 having cloned, checked out, and hooked up nothing useful, and a
#      container that only checks the exit code would call that a pass.
#
# Usage: passed straight to setup.sh. Default is --yes.
set -eu

SEED=/seed/repo.git

printf '=== bootstrap: cloning from %s ===\n' "$SEED"

# setup.sh normally arrives by curl. Here it comes out of the seed repo with
# `git show`, which is the same content the curl URL serves and needs no
# network. The pipe to sh is deliberate: it exercises the curl-piped path,
# where stdin is the script itself, so setup.sh must not read from stdin
# expecting a human.
git --git-dir="$SEED" show "HEAD:setup.sh" > /tmp/setup.sh
sh /tmp/setup.sh --repo "$SEED" "$@"

printf '\n=== bootstrap: verifying the result ===\n'

failed=0

# Takes the predicate as arguments and runs it, rather than taking $? from a
# test on the previous line. The $? form reads as though the status belongs to
# the description, and any statement inserted between the two silently breaks
# it.
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

check 'the bare repo exists at ~/.cfg' test -d "$HOME/.cfg"

check 'status.showUntrackedFiles is no' \
    test "$(git --git-dir="$HOME/.cfg" config --get status.showUntrackedFiles)" = 'no'

# The worktree actually landed. .zshrc is tracked on every branch, so its
# presence with tracked content is the checkout's proof.
check '.zshrc is checked out' test -f "$HOME/.zshrc"

# The pre-existing .zshrc the image planted must have been kept, not
# destroyed. This is the assertion that distinguishes a careful bootstrap
# from a destructive one.
backup=$(find "$HOME" -maxdepth 1 -type d -name '.dotfiles-backup-*' | head -1)
check 'the pre-existing .zshrc was moved aside' test -n "$backup"
if [ -n "$backup" ]; then
    check 'the original .zshrc content survived' \
        grep -q 'predating the bootstrap' "$backup/.zshrc"
fi

# config on PATH is what `config install-hooks` exists to do, and it is what
# every later command in a real session depends on.
check 'config is on PATH at ~/.local/bin' test -x "$HOME/.local/bin/config"

# The hooks are the pre-commit leak check and the pre-push suite. A bootstrap
# that skipped them leaves a machine that can push unverified.
check 'the pre-commit hook is linked' test -L "$HOME/.cfg/hooks/pre-commit"
check 'the pre-push hook is linked' test -L "$HOME/.cfg/hooks/pre-push"

# A dependency that came from the bootstrap and not from the image. The image
# installs git, curl, sudo and ca-certificates only, so tmux on PATH can have
# arrived only through check-deps.sh.
check 'tmux was installed by the bootstrap' command -v tmux

# Idempotency, which the container is the only place to test honestly: a
# second `config init` on a machine the first one finished.
printf '\n=== bootstrap: re-running config init ===\n'
check 'a second config init succeeds' "$HOME/.local/bin/config" init --yes

# And setup.sh must now refuse, because ~/.cfg exists. Re-cloning over a repo
# that may hold unpushed commits is the one thing it must never do.
if sh /tmp/setup.sh --repo "$SEED" --yes >/dev/null 2>&1; then
    printf 'FAIL: setup.sh re-ran over an existing ~/.cfg\n'
    failed=$((failed + 1))
else
    printf 'ok: setup.sh refuses to clobber an existing ~/.cfg\n'
fi

printf '\n=== bootstrap: %d check(s) failed ===\n' "$failed"
[ "$failed" -eq 0 ]
