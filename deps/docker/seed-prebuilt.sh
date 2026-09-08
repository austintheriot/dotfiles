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
#
# THE ONE WAY OUT is BOOTSTRAP_NO_PREBUILT=1, which must be set on purpose.
# `config init` now installs cc and rustup in shell (config-prereqs) before
# it builds the crate, so a fresh machine compiles its own engine and needs
# no injected binary at all. A leg that sets this is asserting exactly that,
# and it is the only leg that tests the path a real user takes.
#
# Kept as a separate, explicit variable rather than by letting an unset
# BOOTSTRAP_PREBUILT_BIN mean "self-build". An absent variable is what a
# typo, a renamed step, or a dropped `-e` flag also produces, and those must
# stay loud. Opting out is a sentence someone wrote; forgetting is not.

seed_prebuilt() {
    if [ "${BOOTSTRAP_NO_PREBUILT:-0}" = 1 ]; then
        if [ -n "${BOOTSTRAP_PREBUILT_BIN:-}" ]; then
            printf 'FAIL: BOOTSTRAP_NO_PREBUILT=1 and BOOTSTRAP_PREBUILT_BIN are both set.\n' >&2
            printf 'These ask for opposite things, so the run would silently test\n' >&2
            printf 'only one of them. Set exactly one.\n' >&2
            return 1
        fi
        printf 'harness: no prebuilt binary -- the bootstrap must build its own\n'
        return 0
    fi

    seed_prebuilt_path=${BOOTSTRAP_PREBUILT_BIN:-}

    if [ -z "$seed_prebuilt_path" ]; then
        printf 'FAIL: BOOTSTRAP_PREBUILT_BIN is unset.\n' >&2
        printf 'Either mount a prebuilt config-cli, or set BOOTSTRAP_NO_PREBUILT=1\n' >&2
        printf 'to assert that the bootstrap builds its own engine.\n' >&2
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
