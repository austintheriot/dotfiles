#!/bin/sh
#
# Runs ~/tests/run-all.sh inside a throwaway Linux container, so the suite's
# side effects never touch this machine.
#
# The suite mutates $HOME by design: fixture git repos, tmux sessions on the
# default tmux server, and (before the install-path fix in
# .my-scripts/deps/check-deps.sh) a real ~/.oh-my-zsh created on a machine
# that does not use oh-my-zsh. Inside the container all of that is discarded
# with the container.
#
# The build context is `git archive` of the current branch, overlaid with the
# working tree's tests/, .my-scripts/, .claude/, README.md, and the zshrc
# files. That covers uncommitted
# edits -- the point of a local loop -- without copying the rest of $HOME into
# a Docker image. The tree is COPYed, never bind-mounted read-write: a mount
# would reintroduce the side effects this exists to contain.
#
# Usage:
#   ~/tests/run-in-docker.sh              # the whole suite
#   ~/tests/run-in-docker.sh check-deps   # one suite, by name
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

if ! command -v docker >/dev/null 2>&1; then
    printf 'run-in-docker: docker is not on PATH. Install Docker and run this again.\n' >&2
    printf 'run-in-docker: to run the suite directly on this machine instead: ~/tests/run-all.sh\n' >&2
    exit 1
fi

if ! docker info >/dev/null 2>&1; then
    printf 'run-in-docker: the Docker daemon is not responding. Start Docker and run this again.\n' >&2
    exit 1
fi

branch=$(git_cmd branch --show-current)
if [ -z "$branch" ]; then
    printf 'run-in-docker: HEAD is detached, so there is no branch to archive.\n' >&2
    printf 'run-in-docker: check out mac or linux and run this again.\n' >&2
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
for tree in tests .my-scripts .claude .github .config/tmux; do
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
