#!/bin/sh
#
# Builds and runs check-deps.sh against fresh ubuntu and archlinux
# containers, so the manifest and the engine can be iterated on without
# waiting on GitHub Actions. Mirrors .github/workflows/deps-check.yml's
# ubuntu and arch legs: same Dockerfiles, same command, same exit-code
# contract.
#
# Builds from a clean `git archive` of the current branch, not $HOME
# directly. $HOME holds plenty of untracked content that has no business in
# a Docker build context.
#
# Each container runs `check-deps.sh --fix --yes`, which exits non-zero only
# when a dependency with an automated install command still fails its check
# afterward. A dependency with no automated install path is reported and
# does not fail the run. This script exits non-zero if either container
# does.
#
# Usage: ~/.my-scripts/deps/test-local.sh

set -eu

GIT_DIR_PATH="$HOME/.cfg"
WORK_TREE_PATH="$HOME"

git_cmd() {
    git --git-dir="$GIT_DIR_PATH" --work-tree="$WORK_TREE_PATH" "$@"
}

branch=$(git_cmd branch --show-current)
if [ -z "$branch" ]; then
    printf 'test-local: HEAD is detached, so there is no branch to archive.\n' >&2
    printf 'test-local: check out mac or linux and run this again.\n' >&2
    exit 1
fi

if ! command -v docker >/dev/null 2>&1; then
    printf 'test-local: docker is not on PATH. Install Docker and run this again.\n' >&2
    exit 1
fi

if ! docker info >/dev/null 2>&1; then
    printf 'test-local: the Docker daemon is not responding. Start Docker and run this again.\n' >&2
    exit 1
fi

workdir=$(mktemp -d "${TMPDIR:-/tmp}/depcheck-docker-XXXXXX")
trap 'rm -rf "$workdir"' EXIT INT TERM HUP

# Archive the committed tree, then overlay the working tree's own copy of
# the deps directory. Without the overlay this harness silently tests the
# last commit instead of the edit under test, which defeats the point of a
# local iteration loop.
git_cmd archive "$branch" | tar -x -C "$workdir"
rm -rf "$workdir/.my-scripts/deps"
mkdir -p "$workdir/.my-scripts"
cp -R "$HOME/.my-scripts/deps" "$workdir/.my-scripts/deps"

status=0

for image in ubuntu arch; do
    printf '\n=== %s ===\n' "$image"

    # archlinux publishes no arm64 image, so the arch leg needs an explicit
    # amd64 platform to build at all on an Apple Silicon machine. It runs
    # under emulation there, which is slow but correct. Ubuntu is
    # multi-arch, so it builds natively.
    platform_args=''
    if [ "$image" = arch ]; then
        platform_args='--platform=linux/amd64'
    fi

    # No `docker build -q`: on a failure the suppressed build log is
    # exactly the output needed to diagnose it.
    # shellcheck disable=SC2086
    if ! docker build $platform_args \
        -f "$workdir/.my-scripts/deps/docker/Dockerfile.$image" \
        -t "depcheck-$image" \
        "$workdir"
    then
        printf '\n=== %s: BUILD FAILED ===\n' "$image" >&2
        status=1
        continue
    fi

    # shellcheck disable=SC2086
    if ! docker run --rm $platform_args "depcheck-$image"; then
        printf '\n=== %s: check-deps.sh exited non-zero ===\n' "$image" >&2
        status=1
    fi
done

printf '\n'
if [ "$status" -ne 0 ]; then
    printf 'test-local: at least one container failed. See the sections above.\n' >&2
    exit 1
fi

printf 'test-local: ubuntu and arch both bootstrapped cleanly.\n'
