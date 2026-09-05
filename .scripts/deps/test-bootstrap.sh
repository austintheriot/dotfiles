#!/bin/sh
#
# Builds and runs the full bootstrap in a fresh container, so setup.sh and
# `config init` can be iterated on without waiting on GitHub Actions.
#
# Sibling to test-local.sh, and deliberately separate. test-local.sh runs
# check-deps.sh directly against two images that COPY .scripts/deps; this runs
# the real entry point against an image that clones from a bare repo and has
# nothing but git, curl and sudo installed. Different subject, different
# image, different failure modes.
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
    printf 'test-bootstrap: check out mac or linux and run this again.\n' >&2
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
# The seed carries BOTH platform branches, not just the host's. The container
# is Linux and detects `linux`, so a seed built only on the host's branch --
# `mac` on this machine -- makes setup.sh fail with "Not a valid object name
# linux" on a branch that exists upstream and only appears missing here. That
# is a defect in the harness rather than in setup.sh, and it took a real
# container run to surface, because both platform branches exist on the real
# remote.
#
# Both branches point at the same tree. This harness tests the bootstrap
# mechanism, not the differences between the branches; the drift check
# (`config check`) is what compares their contents.
git -C "$staging" init -q -b "$branch"
git -C "$staging" add -A
git -C "$staging" -c user.email=t@t -c user.name=t commit -q -m "bootstrap test tree"
for platform_branch in mac linux; do
    if [ "$platform_branch" != "$branch" ]; then
        git -C "$staging" branch -q "$platform_branch"
    fi
done
git clone -q --bare "$staging" "$seed/repo.git"

# HEAD in a bare clone points at whatever the source had checked out. The
# container asks for a branch by name, so this only matters for a caller that
# omits one, but leaving HEAD on the host's branch would make the default
# behave differently depending on which machine ran the harness.
git --git-dir="$seed/repo.git" symbolic-ref HEAD "refs/heads/$branch"
cp "$HOME/.scripts/deps/docker/bootstrap-entrypoint.sh" "$seed/bootstrap-entrypoint.sh"
cp "$HOME/.scripts/deps/docker/bootstrap-bare-entrypoint.sh" "$seed/bootstrap-bare-entrypoint.sh"
# The bare image has no git when setup.sh first runs, so it reads the script
# from the seed directory rather than out of the repository.
cp "$staging/setup.sh" "$seed/setup.sh"

printf '=== building %s ===\n' "$IMAGE"

# An empty build context: the Dockerfile copies nothing, by design. Passing
# the staging tree would quietly re-enable a COPY someone adds later.
if ! docker build \
    -f "$HOME/.scripts/deps/docker/Dockerfile.bootstrap" \
    -t "$IMAGE" \
    "$workdir/empty-context" 2>/dev/null
then
    mkdir -p "$workdir/empty-context"
    if ! docker build \
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

if ! docker run --rm -v "$seed:/seed:ro" "$IMAGE" "$@"; then
    printf '\n=== bootstrap: the container reported a failure ===\n' >&2
    status=1
fi

# The bare leg: root, no sudo, no git. A user found two bugs in seconds on a
# plain `docker run debian` that the image above cannot expose, because it
# installs sudo and git and runs as a normal user.
printf '\n=== building %s-bare ===\n' "$IMAGE"

if ! docker build \
    -f "$HOME/.scripts/deps/docker/Dockerfile.bootstrap-bare" \
    -t "$IMAGE-bare" \
    "$workdir/empty-context"
then
    printf '\n=== bare bootstrap: BUILD FAILED ===\n' >&2
    exit 1
fi

printf '\n=== running the bootstrap (root, no sudo, no git) ===\n'

if ! docker run --rm -v "$seed:/seed:ro" "$IMAGE-bare"; then
    printf '\n=== bare bootstrap: the container reported a failure ===\n' >&2
    status=1
fi

if [ "$status" -ne 0 ]; then
    printf '\ntest-bootstrap: at least one container failed. See the sections above.\n' >&2
    exit 1
fi

printf '\ntest-bootstrap: both bootstrap legs completed and verified cleanly.\n'
