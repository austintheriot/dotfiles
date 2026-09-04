#!/bin/sh
#
# Runs ~/tests/run-all.sh inside a throwaway Linux container, so the suite's
# side effects never touch this machine.
#
# The suite mutates $HOME by design: fixture git repos, tmux sessions on the
# default tmux server, and (before the install-path fix in
# .scripts/deps/check-deps.sh) a real ~/.oh-my-zsh created on a machine
# that does not use oh-my-zsh. Inside the container all of that is discarded
# with the container.
#
# The build context is `git archive` of the current branch, overlaid with the
# working tree's tests/, .scripts/, .claude/, README.md, and the zshrc
# files. That covers uncommitted
# edits -- the point of a local loop -- without copying the rest of $HOME into
# a Docker image. The tree is COPYed, never bind-mounted read-write: a mount
# would reintroduce the side effects this exists to contain.
#
# Usage:
#   ~/tests/run-in-docker.sh              # the whole suite
#   ~/tests/run-in-docker.sh check-deps   # one suite, by name
#
# $DOTFILES_TEST_REF overrides which ref is archived (default: the
# checked-out branch). The working-tree overlay is skipped when it names
# anything else, so the container tests that ref as committed.
#
# This does not replace ~/tests/run-all.sh on macOS. notify.test.sh is
# macOS-only (aerospace and osascript) and is absent from the image, so a
# green container run does not cover it.

set -eu

IMAGE=dotfiles-tests
GIT_DIR_PATH="$HOME/.cfg"
WORK_TREE_PATH="$HOME"

git_cmd() {
    git --git-dir="$GIT_DIR_PATH" --work-tree="$WORK_TREE_PATH" "$@"
}

# $DOTFILES_TEST_REF names the ref to archive. The pre-push hook sets it to
# the ref being pushed, which is not always the checked-out branch: pushing
# linux from a worktree while $HOME sits on mac is how this repo normally
# ships a linux change. Archiving the checked-out branch there would test
# mac's code and report a pass for a linux push.
# An explicitly-passed ref is validated here, before the daemon probe, so a
# typo reports the typo rather than a stopped daemon. The default is left to
# resolve later: this script also runs inside the test image, where there is
# no .cfg repository to resolve anything against, and making resolution a
# hard prerequisite there breaks the docker-guard assertions in
# container.test.sh.
if [ -n "${DOTFILES_TEST_REF:-}" ]; then
    branch=$DOTFILES_TEST_REF
    if ! git_cmd rev-parse --verify --quiet "$branch" >/dev/null 2>&1; then
        printf 'run-in-docker: %s is not a ref in this repository.\n' "$branch" >&2
        exit 1
    fi
else
    branch=$(git_cmd branch --show-current 2>/dev/null || true)
fi

if ! command -v docker >/dev/null 2>&1; then
    printf 'run-in-docker: docker is not on PATH. Install Docker and run this again.\n' >&2
    printf 'run-in-docker: to run the suite directly on this machine instead: ~/tests/run-all.sh\n' >&2
    exit 1
fi

if ! docker info >/dev/null 2>&1; then
    printf 'run-in-docker: the Docker daemon is not responding. Start Docker and run this again.\n' >&2
    exit 1
fi

if [ -z "$branch" ]; then
    printf 'run-in-docker: HEAD is detached, so there is no branch to archive.\n' >&2
    printf 'run-in-docker: check out mac or linux, or set $DOTFILES_TEST_REF.\n' >&2
    exit 1
fi

workdir=$(mktemp -d "${TMPDIR:-/tmp}/dotfiles-tests-XXXXXX")
trap 'rm -rf "$workdir"' EXIT INT TERM HUP

git_cmd archive "$branch" | tar -x -C "$workdir"

# Overlay the working tree over the archive. Without this the container
# silently tests the last commit rather than the edit under test, which
# defeats the purpose of a local iteration loop -- and an entirely untracked
# file (a new workflow, a new test) would be missing from the image whether
# or not it is the thing under test.
#
# Skipped when archiving a ref other than the checked-out branch: $HOME's
# working tree belongs to a different branch then, and overlaying it would
# mix the two. The pre-push hook wants the committed ref exactly as it will
# land on the remote, so that is the correct behavior there.
current_branch=$(git_cmd branch --show-current 2>/dev/null || true)
if [ "$branch" = "$current_branch" ]; then
    for tree in tests .scripts .claude .github .config/tmux; do
        if [ -d "$HOME/$tree" ]; then
            rm -rf "$workdir/$tree"
            mkdir -p "$(dirname "$workdir/$tree")"
            cp -R "$HOME/$tree" "$workdir/$tree"
        fi
    done

    for file in .sync-manifest README.md .zshrc .zshrc-mac .zshrc-linux; do
        if [ -f "$HOME/$file" ]; then
            cp "$HOME/$file" "$workdir/$file"
        fi
    done
else
    printf 'run-in-docker: testing ref %s (no working-tree overlay)\n' "$branch"
fi

# .claude carries machine-local state that has no business in an image layer:
# credentials, plugin caches, session transcripts, and the work environment
# files the leak guard exists to keep out of this repo. The suite only needs
# scripts/ and hooks/.
if [ -d "$workdir/.claude" ]; then
    find "$workdir/.claude" -mindepth 1 -maxdepth 1 \
        ! -name scripts ! -name hooks -exec rm -rf {} +
fi

# No `docker build -q`: on a failure the suppressed build log is exactly the
# output needed to diagnose it.
docker build \
    -f "$workdir/tests/docker/Dockerfile" \
    -t "$IMAGE" \
    "$workdir"

# --init so a wedged tmux server is reaped rather than orphaning the run.
docker run --rm --init "$IMAGE" "$@"
