#!/bin/sh
#
# Builds and runs `config deps install --yes` against fresh ubuntu and
# archlinux containers, so the manifest and the engine can be iterated on
# without waiting on GitHub Actions. Mirrors .github/workflows/deps-check.yml's
# arch leg: same Dockerfile, same command, same exit-code contract. The CI
# ubuntu leg runs on a native runner rather than in this image.
#
# Builds from a clean `git archive` of the current branch, not $HOME
# directly. $HOME holds plenty of untracked content that has no business in
# a Docker build context.
#
# Each container runs `config deps install --yes`, which exits non-zero only
# when a dependency with an automated install command still fails its check
# afterward. A dependency with no automated install path is reported and
# does not fail the run. This script exits non-zero if either container
# does.
#
# Usage: ~/.scripts/deps/test-local.sh

set -eu

GIT_DIR_PATH="$HOME/.cfg"
WORK_TREE_PATH="$HOME"

git_cmd() {
    git --git-dir="$GIT_DIR_PATH" --work-tree="$WORK_TREE_PATH" "$@"
}

branch=$(git_cmd branch --show-current)
if [ -z "$branch" ]; then
    printf 'test-local: HEAD is detached, so there is no branch to archive.\n' >&2
    printf 'test-local: check out main and run this again.\n' >&2
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

# The binary the containers run is built by one shared script, not by a copy
# per harness. See .scripts/deps/docker/build-seed-binary.sh for why it must
# be a container build and why the builder image is pinned to bookworm: the
# glibc floor of the build host becomes the floor of the binary, and three
# bootstrap images are older than the CI runner.
build_seed_binary() {
    "$HOME/.scripts/deps/docker/build-seed-binary.sh" "$1" "$HOME" "$2"
}

workdir=$(mktemp -d "${TMPDIR:-/tmp}/depcheck-docker-XXXXXX")
trap 'rm -rf "$workdir"' EXIT INT TERM HUP

# Archive the committed tree, then overlay the working tree's own copy of
# the deps directory. Without the overlay this harness silently tests the
# last commit instead of the edit under test, which defeats the point of a
# local iteration loop.
git_cmd archive "$branch" | tar -x -C "$workdir"
rm -rf "$workdir/.scripts/deps"
mkdir -p "$workdir/.scripts"
cp -R "$HOME/.scripts/deps" "$workdir/.scripts/deps"

seed="$workdir/seed"
mkdir -p "$seed"
cp "$HOME/.scripts/deps/docker/seed-prebuilt.sh" "$seed/seed-prebuilt.sh"
cp "$HOME/.scripts/deps/docker/deps-image-entrypoint.sh" \
    "$seed/deps-image-entrypoint.sh"

# Both images are linux/amd64 in practice -- archlinux publishes no arm64
# image, and matching the two keeps one seed binary valid for both.
if ! build_seed_binary "$seed" '--platform=linux/amd64'; then
    printf '\ntest-local: could not build config-cli for the containers.\n' >&2
    exit 1
fi

status=0

for image in ubuntu arch; do
    printf '\n=== %s ===\n' "$image"

    # archlinux publishes no arm64 image, so the arch leg needs an explicit
    # amd64 platform to build at all on an Apple Silicon machine. It runs
    # under emulation there, which is slow but correct. Ubuntu is
    # multi-arch, so it builds natively.
    # linux/amd64 for both, not only arch. archlinux publishes no arm64
    # image, and the seed binary is built once: an arm64 ubuntu container
    # could not execute an amd64 binary, so matching the two is what lets
    # one seed serve both. Both run under emulation on Apple Silicon.
    platform_args='--platform=linux/amd64'

    # No `docker build -q`: on a failure the suppressed build log is
    # exactly the output needed to diagnose it.
    # shellcheck disable=SC2086
    if ! docker build $platform_args \
        -f "$workdir/.scripts/deps/docker/Dockerfile.$image" \
        -t "depcheck-$image" \
        "$workdir"
    then
        printf '\n=== %s: BUILD FAILED ===\n' "$image" >&2
        status=1
        continue
    fi

    # Redirected to a file, then printed, so the container's own exit status
    # stays authoritative and the summary line can still be inspected after
    # the run.
    #
    # Not a `| tee` pipeline, which would show the output live but hand back
    # tee's status instead of the container's, and PIPESTATUS is not POSIX.
    # The exit code is what decides a pass here, so it wins over liveness;
    # the log is on disk if a run needs watching from another terminal.
    run_log="$workdir/$image.log"
    # shellcheck disable=SC2086
    if docker run --rm $platform_args \
        -v "$seed:/seed:ro" \
        -e BOOTSTRAP_PREBUILT_BIN=/seed/config-cli \
        "depcheck-$image" > "$run_log" 2>&1
    then
        cat "$run_log"
    else
        cat "$run_log"
        printf '\n=== %s: config deps install exited non-zero ===\n' "$image" >&2
        status=1
        continue
    fi

    # A run that considered NO dependencies exits 0 and reads as a pass.
    #
    # That is not hypothetical: the first container run after the engine
    # became a binary reported "0 entries" and this harness called it a clean
    # bootstrap, because the engine resolved the manifest against HOME while
    # the image copies it to /dotfiles. The exit code was 0 throughout.
    #
    # So the count is the assertion, not the status. An image whose manifest
    # the engine cannot find fails here instead of passing silently.
    if grep -qE ': 0 entries' "$run_log"; then
        printf '\n=== %s: the run considered 0 dependencies ===\n' "$image" >&2
        printf 'test-local: the engine found no manifest in this image.\n' >&2
        printf 'test-local: check DOTFILES_ROOT against where the deps tree is copied.\n' >&2
        status=1
    fi
done

printf '\n'
if [ "$status" -ne 0 ]; then
    printf 'test-local: at least one container failed. See the sections above.\n' >&2
    exit 1
fi

printf 'test-local: ubuntu and arch both bootstrapped cleanly.\n'
