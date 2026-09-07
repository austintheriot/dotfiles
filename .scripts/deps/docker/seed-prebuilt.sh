#!/bin/sh
# Copies the prebuilt config-cli out of the seed mount and onto PATH, for the
# bootstrap images.
#
# Sourced by every bootstrap entrypoint rather than duplicated into each one.
# The four entrypoints already share the property that made this necessary,
# and a divergent copy per image is how one leg quietly stops checking
# something the others still do.
#
# Why the seam exists at all: `config init` installs dependencies BEFORE it
# builds the crate, because rustup is itself a manifest entry and cargo does
# not exist until the install step places it. The engine that performs that
# install is now config-cli, so on a genuinely fresh machine config-cli must
# arrive before the step that would otherwise build it. These images cannot
# compile it: they carry no toolchain, and giving them one would pre-satisfy
# the rustup entry the run exists to exercise.
#
# BOOTSTRAP_PREBUILT_BIN is REQUIRED, not optional. It defaulted to empty
# while the engine was shell and the run could proceed against whatever
# was on PATH. That is a fail-open: a gate that falls back can pass having
# tested a path that no longer ships. An unset variable now fails the run.

seed_prebuilt() {
    seed_prebuilt_path=${BOOTSTRAP_PREBUILT_BIN:-}

    if [ -z "$seed_prebuilt_path" ]; then
        printf 'FAIL: BOOTSTRAP_PREBUILT_BIN is unset.\n' >&2
        printf 'This image carries no Rust toolchain, so the caller must build\n' >&2
        printf 'config-cli and mount it into the seed.\n' >&2
        return 1
    fi

    if [ ! -x "$seed_prebuilt_path" ]; then
        printf 'FAIL: a prebuilt binary was named but does not exist: %s\n' \
            "$seed_prebuilt_path" >&2
        return 1
    fi

    mkdir -p "$HOME/.local/bin"
    cp "$seed_prebuilt_path" "$HOME/.local/bin/"
    printf 'harness: seeded a prebuilt %s\n' "${seed_prebuilt_path##*/}"
    unset seed_prebuilt_path
}
