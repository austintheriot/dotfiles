#!/bin/sh
#
# Builds and runs the full bootstrap in a fresh container, so setup.sh and
# `config init` can be iterated on without waiting on GitHub Actions.
#
# Sibling to test-local.sh, and deliberately separate. test-local.sh runs
# `config deps install` directly against two images that COPY .scripts/deps;
# this runs the real entry point against an image that clones from a bare
# repo and has nothing but git, curl and sudo installed. Different subject,
# different image, different failure modes.
#
# The container clones from a bare repo built out of the current branch, so
# what it tests is the working tree's setup.sh, not the last push. The bare
# repo is bind-mounted read-only rather than copied into the image, which
# keeps the build context empty and makes the clone a real clone.
#
# Usage: ~/.scripts/deps/test-bootstrap.sh [setup.sh flags]
#
# Any flag is passed to setup.sh inside the container. The default is --yes.

set -eu

GIT_DIR_PATH="$HOME/.cfg"
WORK_TREE_PATH="$HOME"
IMAGE=dotfiles-bootstrap

git_cmd() {
    git --git-dir="$GIT_DIR_PATH" --work-tree="$WORK_TREE_PATH" "$@"
}

branch=$(git_cmd branch --show-current)
if [ -z "$branch" ]; then
    printf 'test-bootstrap: HEAD is detached, so there is no branch to clone.\n' >&2
    printf 'test-bootstrap: check out main and run this again.\n' >&2
    exit 1
fi

if ! command -v docker >/dev/null 2>&1; then
    printf 'test-bootstrap: docker is not on PATH. Install Docker and run this again.\n' >&2
    exit 1
fi

if ! docker info >/dev/null 2>&1; then
    printf 'test-bootstrap: the Docker daemon is not responding. Start Docker and run this again.\n' >&2
    exit 1
fi

# The engine is a Rust binary now, and neither image carries a toolchain:
# rustup is itself a manifest entry, so installing one in the image under
# test would pre-satisfy the very dependency the run exists to exercise.
#
# So the binary is built in a SEPARATE throwaway Rust container and handed to
# each image through a /seed mount. A separate container is the point: the
# toolchain never touches the image whose bootstrap is being measured.
#
# Built for linux, not for the host. This harness runs on macOS, where a
# native `cargo build` produces a Mach-O binary that a Linux container
# cannot execute at all.
build_seed_binary() {
    seed_dir=$1
    seed_platform=$2

    toolchain=$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$HOME/crates/rust-toolchain.toml")
    if [ -z "$toolchain" ]; then
        printf 'test-bootstrap: no channel in crates/rust-toolchain.toml\n' >&2
        return 1
    fi

    printf '=== building config-cli for the containers (rust %s) ===\n' "$toolchain"

    # The repo ROOT is mounted, not just crates/. config-cli embeds the four
    # conf files with include_str! at `../../../../.scripts/deps/`, four
    # levels up and out of the workspace, so a crates-only context fails to
    # compile with "couldn't read ... No such file or directory". Measured:
    # this is what the first run of this function did, and how the coupling
    # was found.
    #
    # --locked, matching every other build gate in this repo: a harness that
    # silently updates the lockfile tests a dependency set that was never
    # committed.
    # shellcheck disable=SC2086
    docker run --rm $seed_platform \
        -v "$HOME/crates:/src/crates:ro" \
        -v "$HOME/.scripts:/src/.scripts:ro" \
        -v "$seed_dir:/out" \
        -w /src/crates \
        "rust:$toolchain" \
        sh -c 'cargo build --release --locked \
                --target-dir /tmp/target -p config-cli \
            && cp /tmp/target/release/config-cli /out/config-cli' \
        || return 1

    chmod +x "$seed_dir/config-cli"
    unset seed_dir seed_platform toolchain
}

workdir=$(mktemp -d "${TMPDIR:-/tmp}/dotfiles-bootstrap-XXXXXX")
trap 'rm -rf "$workdir"' EXIT INT TERM HUP

# The seed the container clones from: a real bare repo whose HEAD is this
# branch's working tree, so an uncommitted edit to setup.sh is what gets
# tested. A plain `clone --bare` of ~/.cfg would carry the last commit
# instead, which is the trap test-local.sh documents for its own archive.
seed="$workdir/seed"
mkdir -p "$seed"
staging="$workdir/staging"
mkdir -p "$staging"

git_cmd archive "$branch" | tar -x -C "$staging"

# Overlay the working tree's copy of everything this bootstrap actually
# drives, so a local edit is under test rather than the last commit.
for path in setup.sh .scripts/deps .scripts/config .scripts/platform.sh; do
    if [ -e "$HOME/$path" ]; then
        # ${path:?} rather than $path: an empty value here would expand to
        # "$staging/" and recursively delete the staging tree. The loop list
        # is literal today, so this is a guard against a future edit rather
        # than a live bug.
        rm -rf "$staging/${path:?}"
        mkdir -p "$staging/$(dirname "$path")"
        cp -R "$HOME/$path" "$staging/$path"
    fi
done

# A throwaway repo whose single commit is that tree, then a bare clone of it.
# The container needs a bare repo to clone from, and building it here rather
# than reusing ~/.cfg keeps the test off the real repository entirely.
#
# This used to create BOTH `mac` and `linux` alongside the host's branch,
# because setup.sh detected a platform and checked out the matching branch, so
# a seed built only on the host's branch made a Linux container fail with "Not
# a valid object name linux". setup.sh now defaults to `main`, so the seed
# needs one branch, and manufacturing the frozen pair would hide a regression
# of the mapping instead of failing on it.
#
# `main` is still created when the host is on a feature branch, because the
# container passes no --branch and so takes setup.sh's default. It points at
# the same tree: this harness tests the bootstrap mechanism, not the contents
# of any particular branch.
git -C "$staging" init -q -b "$branch"
git -C "$staging" add -A
git -C "$staging" -c user.email=t@t -c user.name=t commit -q -m "bootstrap test tree"
if [ "$branch" != 'main' ]; then
    git -C "$staging" branch -q main
fi
git clone -q --bare "$staging" "$seed/repo.git"

# HEAD in a bare clone points at whatever the source had checked out. The
# container asks for a branch by name, so this only matters for a caller that
# omits one, but leaving HEAD on the host's branch would make the default
# behave differently depending on which machine ran the harness.
git --git-dir="$seed/repo.git" symbolic-ref HEAD "refs/heads/$branch"
cp "$HOME/.scripts/deps/docker/bootstrap-entrypoint.sh" "$seed/bootstrap-entrypoint.sh"
cp "$HOME/.scripts/deps/docker/bootstrap-bare-entrypoint.sh" "$seed/bootstrap-bare-entrypoint.sh"
# The seam both entrypoints source. The engine is a Rust binary and neither
# image carries a toolchain, so it arrives through this mount.
cp "$HOME/.scripts/deps/docker/seed-prebuilt.sh" "$seed/seed-prebuilt.sh"
# The bare image has no git when setup.sh first runs, so it reads the script
# from the seed directory rather than out of the repository.
cp "$staging/setup.sh" "$seed/setup.sh"

# Both bootstrap images are linux/amd64 here for the reason test-local.sh
# gives: one seed binary has to be executable by every container that
# mounts it.
if ! build_seed_binary "$seed" '--platform=linux/amd64'; then
    printf '\ntest-bootstrap: could not build config-cli for the containers.\n' >&2
    exit 1
fi

printf '=== building %s ===\n' "$IMAGE"

# An empty build context: the Dockerfile copies nothing, by design. Passing
# the staging tree would quietly re-enable a COPY someone adds later.
if ! docker build --platform=linux/amd64 \
    -f "$HOME/.scripts/deps/docker/Dockerfile.bootstrap" \
    -t "$IMAGE" \
    "$workdir/empty-context" 2>/dev/null
then
    mkdir -p "$workdir/empty-context"
    if ! docker build --platform=linux/amd64 \
        -f "$HOME/.scripts/deps/docker/Dockerfile.bootstrap" \
        -t "$IMAGE" \
        "$workdir/empty-context"
    then
        printf '\n=== bootstrap: BUILD FAILED ===\n' >&2
        exit 1
    fi
fi

printf '\n=== running the bootstrap (non-root, sudo present) ===\n'

if [ "$#" -eq 0 ]; then
    set -- --yes
fi

status=0

if ! docker run --rm --platform=linux/amd64 -v "$seed:/seed:ro" \
    -e BOOTSTRAP_PREBUILT_BIN=/seed/config-cli "$IMAGE" "$@"
then
    printf '\n=== bootstrap: the container reported a failure ===\n' >&2
    status=1
fi

# The bare leg: root, no sudo, no git. A user found two bugs in seconds on a
# plain `docker run debian` that the image above cannot expose, because it
# installs sudo and git and runs as a normal user.
printf '\n=== building %s-bare ===\n' "$IMAGE"

if ! docker build --platform=linux/amd64 \
    -f "$HOME/.scripts/deps/docker/Dockerfile.bootstrap-bare" \
    -t "$IMAGE-bare" \
    "$workdir/empty-context"
then
    printf '\n=== bare bootstrap: BUILD FAILED ===\n' >&2
    exit 1
fi

printf '\n=== running the bootstrap (root, no sudo, no git) ===\n'

if ! docker run --rm --platform=linux/amd64 -v "$seed:/seed:ro" \
    -e BOOTSTRAP_PREBUILT_BIN=/seed/config-cli "$IMAGE-bare"
then
    printf '\n=== bare bootstrap: the container reported a failure ===\n' >&2
    status=1
fi

if [ "$status" -ne 0 ]; then
    printf '\ntest-bootstrap: at least one container failed. See the sections above.\n' >&2
    exit 1
fi

printf '\ntest-bootstrap: both bootstrap legs completed and verified cleanly.\n'
