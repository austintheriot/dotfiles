#!/bin/sh
# Builds the config-cli that the container legs run, into a seed directory.
#
# Usage: build-seed-binary.sh <seed-dir> <repo-root> [docker-platform-flag]
#
# One owner for "how the container-bound engine is built". Before this
# script, test-local.sh and test-bootstrap.sh each carried a copy and the CI
# workflow carried a third, different, method. The three drifted, and the
# drift shipped: see the GLIBC section below.
#
# WHY A CONTAINER AND NOT A NATIVE BUILD
#
# Two independent reasons, and only the first was documented before:
#
#   1. Architecture. The local harnesses run on macOS, where a native
#      `cargo build` produces a Mach-O binary no Linux container can execute
#      at all. Obvious, and it fails loudly.
#
#   2. The glibc floor. A dynamically linked Rust binary records the minimum
#      glibc version of the machine that built it. GitHub's `ubuntu-latest`
#      runner is Ubuntu 24.04, which is glibc 2.39. Three bootstrap images
#      are `debian:bookworm-slim`, which is glibc 2.36. A binary built on the
#      runner and mounted into those images dies before `main` with:
#
#        config-cli: /lib/x86_64-linux-gnu/libc.so.6:
#        version `GLIBC_2.39' not found (required by config-cli)
#
#      This one fails QUIETLY as far as the workflow is concerned: the
#      bootstrap continues, `config init` reports "the install step failed",
#      and the leg's own assertions fail for reasons that name nothing about
#      glibc. It cost a full CI red to find.
#
# So the builder image is pinned to the `-bookworm` variant, NOT the bare
# `rust:<channel>` tag. The bare tag follows Debian stable and moved to
# trixie (glibc 2.41), which is newer than every image this repo targets and
# reproduces the same failure. Measured: a binary from `rust:1.94.0` dies on
# `debian:bookworm-slim` exactly as the runner-built one does.
#
# The rule this encodes: build against the OLDEST glibc any target image
# ships, which is bookworm's 2.36. A binary with a 2.36 floor runs on
# bookworm (2.36), on Ubuntu 24.04 (2.39), and on Arch (2.44). Raising the
# builder's base raises the floor and silently drops the oldest target.
# When a bootstrap image moves off bookworm, this pin moves with it.
#
# A separate throwaway container is the point either way: rustup is itself a
# manifest entry, so a toolchain installed into an image under test would
# pre-satisfy the very dependency the run exists to exercise.
set -eu

seed_dir=${1:?usage: build-seed-binary.sh <seed-dir> <repo-root> [platform-flag]}
repo_root=${2:?usage: build-seed-binary.sh <seed-dir> <repo-root> [platform-flag]}
seed_platform=${3:-}

# The repo root is a parameter rather than $HOME. $HOME is the worktree on a
# real machine but not on a CI runner, where the checkout lives under
# $GITHUB_WORKSPACE, and a script that assumes the two are the same builds
# from whatever happens to be in the runner's home directory.
if [ ! -f "$repo_root/crates/rust-toolchain.toml" ]; then
    printf 'build-seed-binary: no crates/rust-toolchain.toml under %s\n' "$repo_root" >&2
    exit 1
fi

toolchain=$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$repo_root/crates/rust-toolchain.toml")
if [ -z "$toolchain" ]; then
    printf 'build-seed-binary: no channel in crates/rust-toolchain.toml\n' >&2
    exit 1
fi

mkdir -p "$seed_dir"
printf '=== building config-cli for the containers (rust %s) ===\n' "$toolchain"

# The repo ROOT is mounted, not just crates/. config-cli embeds the four conf
# files with include_str! at `../../../../.scripts/deps/`, four levels up and
# out of the workspace, so a crates-only context fails to compile with
# "couldn't read ... No such file or directory". Measured: this is what the
# first run of the extracted builder did, and how the coupling was found.
#
# --locked, matching every other build gate in this repo: a harness that
# silently updates the lockfile tests a dependency set that was never
# committed.
# shellcheck disable=SC2086
docker run --rm $seed_platform \
    -v "$repo_root/crates:/src/crates:ro" \
    -v "$repo_root/.scripts:/src/.scripts:ro" \
    -v "$seed_dir:/out" \
    -w /src/crates \
    "rust:$toolchain-bookworm" \
    sh -c 'cargo build --release --locked \
            --target-dir /tmp/target -p config-cli \
        && cp /tmp/target/release/config-cli /out/config-cli'

chmod +x "$seed_dir/config-cli"
