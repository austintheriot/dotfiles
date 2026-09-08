# Runs ~/tests/run-all.sh against a throwaway Linux HOME, so the suite's side
# effects never touch the developer's machine.
#
# The suite mutates $HOME by design: it writes fixture git repos, spawns tmux
# sessions on the default tmux server, and -- before the install-path fix in
# the deps engine -- once created a real
# ~/.oh-my-zsh/custom/plugins/zsh-autosuggestions on a machine that does not
# use oh-my-zsh. Every one of those lands in the container instead.
#
# Debian rather than the Ubuntu image used by deps/docker: this
# image tests the dotfiles suite, not the dependency manifest, so it wants a
# small base with the tooling available in one apt index. Digest-pinned so a
# base-image republish cannot change a passing suite underneath us.
# Builder stage: compiles the crates workspace so the runtime image below
# carries only the binary and stays Rust-free. Digest-pinned like the runtime
# base. The dependency layer is built first from a stub main.rs so a source
# edit does not recompile dependencies; a single COPY of the whole crate would
# silently defeat that cache and the measured 0.8s/2.6s rebuilds rest on it.
FROM rust:1.94-slim-bookworm@sha256:cf9dd0ec73e75f827fe59123fff9dc65af1a1c8363c3c31ee8d7f8ad0b6a5fb2 AS builder
# /build/crates, not /build, so the builder's directory depth matches the
# real repository's. config-cli's catalog embeds the dependency manifests
# with `include_str!("../../../../deps/*.conf")`, and that escape
# resolves relative to the crate. Under /build it landed on /deps at the
# filesystem root and the crate could not compile; under /build/crates it
# lands on /build/deps, which is inside the build context and mirrors
# how the path resolves from ~/crates. Matching the layout is what makes one
# relative path correct in both places, rather than each consumer carrying a
# different fix.
WORKDIR /build/crates
COPY crates/Cargo.toml crates/Cargo.lock ./
# One COPY per workspace member, and container.test.sh asserts this list
# matches what cargo reports. Adding deps-core without these three lines
# failed the build with "failed to read /build/deps-core/Cargo.toml" during a
# push, so the guard exists to make the next crate fail in a test instead.
COPY crates/config-cli/Cargo.toml config-cli/
COPY crates/config-manifest/Cargo.toml config-manifest/
COPY crates/dotfiles-path/Cargo.toml dotfiles-path/
COPY crates/deps-core/Cargo.toml deps-core/
COPY crates/tmux-core/Cargo.toml tmux-core/
COPY crates/tmux-tools/Cargo.toml tmux-tools/
# The warm-up build: stub sources so the dependency compile lands in a layer
# that survives an edit to any real source.
#
# EVERY stub is removed afterwards, not only the binaries'. A stub left behind
# is overwritten by the COPY below, but cargo fingerprints what it already
# compiled: the real source arrives and the rlib built from the empty stub is
# still considered fresh, so the final build fails with "no `X` in the root"
# for every item the library genuinely exports. The three library crates used
# to be left in place, which kept this one cache-bust away from failing that
# way, and it did fail that way the first time the stub line itself changed.
RUN mkdir config-cli/src config-manifest/src dotfiles-path/src deps-core/src tmux-core/src tmux-tools/src \
    && echo 'fn main() {}' > config-cli/src/main.rs \
    && echo '' > config-manifest/src/lib.rs \
    && echo '' > dotfiles-path/src/lib.rs \
    && echo '' > deps-core/src/lib.rs \
    && echo '' > tmux-core/src/lib.rs \
    && echo 'fn main() {}' > tmux-tools/src/main.rs \
    && cargo build --release --locked --quiet \
    && rm -rf config-cli/src config-manifest/src dotfiles-path/src deps-core/src \
        tmux-core/src tmux-tools/src \
        target/release/config-cli target/release/tmux-tools \
        target/release/deps/config_cli* target/release/deps/config_manifest* \
        target/release/deps/dotfiles_path* target/release/deps/deps_core* \
        target/release/deps/tmux_core* target/release/deps/tmux_tools* \
        target/release/deps/libconfig_manifest* target/release/deps/libdotfiles_path* \
        target/release/deps/libdeps_core* target/release/deps/libtmux_core*
# The tracked files the catalog embeds at compile time. Copied to
# /build/deps so the crate-relative escape above resolves.
COPY deps ../deps

COPY crates/config-cli/src ./config-cli/src
COPY crates/config-manifest/src ./config-manifest/src
COPY crates/dotfiles-path/src ./dotfiles-path/src
COPY crates/deps-core/src ./deps-core/src
COPY crates/tmux-core/src ./tmux-core/src
COPY crates/tmux-tools/src ./tmux-tools/src
RUN cargo build --release --locked --quiet

FROM debian:bookworm-slim@sha256:88200866dfff7ea7f5cbcb6ec7c8a701889efe6fe859fe64d6990e4b07ea4171

# The tools the suite shells out to. tmux and zsh are the load-bearing ones:
# lib.sh drives real tmux sessions, and zsh-git-widgets.test.sh sources the
# widget file in a real zsh. fzf, ripgrep, and git are asserted as present by
# that suite. dash is /bin/sh here, and the deps tests use it to catch
# bashisms that macOS's bash-flavored /bin/sh accepts. python3 runs the
# unittest suites under .claude/scripts.
#
# ca-certificates is not for the suite itself but for any git operation that
# reaches a remote; without it those fail with an opaque TLS error.
# python3-yaml because deps-harness.test.sh parses the workflow YAML with it
# rather than grepping the raw text, so a reformat cannot produce a false
# failure.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        tmux \
        zsh \
        git \
        python3 \
        python3-yaml \
        dash \
        fzf \
        ripgrep \
        shellcheck \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# tmux-update-window-names.sh is now a shim that execs this binary, so the
# suite fails with "exec: tmux-tools: not found" without it. The builder
# already compiles every workspace member; only the binaries the suite calls
# by name need to reach the runtime stage, which is what keeps this image
# Rust-free while still exercising ported code.
COPY --from=builder /build/crates/target/release/tmux-tools /usr/local/bin/tmux-tools
# config-deps execs this, and deps-docs.test.sh probes every documented
# flag against it. Without it those probes exit 127, which the suite reads
# as a rejected flag, so its harvest finds nothing and passes vacuously
# until the count assertion catches it.
#
# It also carries the doctor and verify-stamps subcommands that
# config-manifest's own binary used to own, which the shell suites and
# pre-push call by name. config-manifest is library-only and installs
# nothing, so there is no second binary to copy.
COPY --from=builder /build/crates/target/release/config-cli /usr/local/bin/config-cli

# A UTF-8 locale is load-bearing, not cosmetic. With no LANG set, tmux treats
# a tab as non-printable and substitutes "_" in display-message output.
# .scripts/tmux-update-window-names.sh uses a tab as its field separator
# and splits the result with parameter expansion, so every field parse breaks
# and 31 of that suite's 33 assertions fail with names like
# "stale_stale___0_/path - repo". Verified by byte-comparing tmux output with
# and without LANG: "a\tb" versus "a_b".
ENV LANG=C.UTF-8
ENV LC_ALL=C.UTF-8

# A real HOME, because the suite treats $HOME as the dotfiles work tree and
# tmux-update-window-names.test.sh asserts that a window opened in $HOME is
# named "~". Running as root is fine here: the container is single-purpose and
# thrown away after each run.
ENV HOME=/root
WORKDIR /root

# The tree is COPYed rather than bind-mounted. A read-write mount of the real
# $HOME would reintroduce exactly the side effects this image exists to
# contain. run-in-docker.sh builds the context from `git archive` plus the
# working tree's tests/ and .scripts/, so uncommitted edits are covered
# without exposing the rest of $HOME.
COPY tests /root/tests
COPY .scripts /root/.scripts
COPY .claude /root/.claude
# tmux-conf-split.test.sh loads the real config on a throwaway tmux server.
COPY .config/tmux /root/.config/tmux
# alacritty-platform-split.test.sh reads the shared config and both variants
# to check the import wiring. No Alacritty binary is needed -- the assertions
# are about which file imports which.
COPY .config/alacritty /root/.config/alacritty
# nvim-mason-runtimes.test.sh reads the Mason ensure_installed list and the
# health module. No nvim binary is needed -- the assertions are about which
# runtimes the config asks for and whether the health module is discoverable.
COPY .config/nvim /root/.config/nvim
# deps-harness.test.sh asserts on the workflow's triggers, permissions, and
# job definitions by parsing this file.
COPY .github /root/.github
# deps-docs.test.sh reads the "Dependency checking" section of the home
# README and resolves every file path the docs cite. Without this the suite
# fails on its own first assertion rather than on anything it means to check.
COPY README.md /root/README.md
# depcheck-hook.test.sh asserts the hook is sourced from .zshrc, and
# deps-manifest.test.sh reads the zshrc files to decide whether this branch
# needs oh-my-zsh. Both silently lose those assertions without this.
# The glob covers both platform variants (.zshrc-mac and .zshrc-linux), which
# both ship on the single branch and are selected at runtime.
COPY .zshrc* /root/
# setup.sh is the bootstrap entry point, and three suites read it:
# setup.test.sh drives it against a fixture HOME, bootstrap-harness.test.sh
# asserts it is at the repo root and documents a reachable raw URL, and
# deps-docs.test.sh resolves it as a path cited by the docs. A root-level
# file needs its own COPY line here -- the COPYs above are per-path, so a new
# one at the repo root is absent from this image until it is listed, and the
# suites that read it fail in the container while passing on the host.
COPY setup.sh /root/setup.sh
# profile-path.test.sh sources this in a real sh and bash to check that
# ~/.local/bin lands on PATH. Another root-level file needing its own COPY
# line, caught by the guard in container.test.sh rather than by CI this time.
COPY .profile /root/.profile

# notify.test.sh drives aerospace and osascript, both macOS-only, so it can
# never pass here. Removing it is honest: run-all.sh globs tests/*.test.sh,
# so a skip-by-absence keeps the runner's suite count accurate instead of
# reporting a failure that means nothing on Linux.
RUN rm -f /root/tests/notify.test.sh

# `docker run` with no arguments runs the whole suite and exits with its
# status, which is what makes this usable from a pre-push hook or CI. Pass a
# single suite name to run just that one.
ENTRYPOINT ["/bin/bash", "/root/tests/run-all.sh"]
