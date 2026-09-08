#!/bin/sh
# Runs `config-cli deps install` inside Dockerfile.ubuntu and Dockerfile.arch.
#
# These two images used to name a shell engine as their ENTRYPOINT directly,
# because that engine ran on a bare image with no toolchain. The engine is a Rust binary now, so the images need something
# that can receive one. That is this file's whole job.
#
# Shared between both images rather than one wrapper each, for the reason
# the curl entrypoints already state: the assertions are about the engine's
# contract, not about the distribution, and a divergent copy per image is
# how one leg quietly stops checking something the other still does.
#
# Deliberately NOT a fallback to shell. A gate that falls back can pass
# having tested a path that no longer ships.
#
# Usage: arguments are passed to `config-cli deps`. The images pin
# `install --yes` as their CMD.
set -eu

. /seed/seed-prebuilt.sh
seed_prebuilt

PATH="$HOME/.local/bin:$PATH"
export PATH

exec config-cli deps "$@"
