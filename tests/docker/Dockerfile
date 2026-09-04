# Runs ~/tests/run-all.sh against a throwaway Linux HOME, so the suite's side
# effects never touch the developer's machine.
#
# The suite mutates $HOME by design: it writes fixture git repos, spawns tmux
# sessions on the default tmux server, and -- before the install-path fix in
# .scripts/deps/check-deps.sh -- once created a real
# ~/.oh-my-zsh/custom/plugins/zsh-autosuggestions on a machine that does not
# use oh-my-zsh. Every one of those lands in the container instead.
#
# Debian rather than the Ubuntu image used by .scripts/deps/docker: this
# image tests the dotfiles suite, not the dependency manifest, so it wants a
# small base with the tooling available in one apt index. Digest-pinned so a
# base-image republish cannot change a passing suite underneath us.
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
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

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
COPY .sync-manifest /root/.sync-manifest
# deps-harness.test.sh asserts on the workflow's triggers, permissions, and
# job definitions by parsing this file.
COPY .github /root/.github
# deps-docs.test.sh reads the "Dependency checking" section of the home
# README and resolves every file path the docs cite. Without this the suite
# fails on its own first assertion rather than on anything it means to check.
COPY README.md /root/README.md
# depcheck-hook.test.sh asserts the hook is sourced from .zshrc, and
# check-deps.test.sh reads the zshrc files to decide whether this branch
# needs oh-my-zsh. Both silently lose those assertions without this.
# The glob covers the per-branch companion (.zshrc-mac or .zshrc-linux)
# without naming either, so this line works unchanged on both branches.
COPY .zshrc* /root/

# notify.test.sh drives aerospace and osascript, both macOS-only, so it can
# never pass here. It is already excluded from .sync-manifest for the same
# reason. Removing it is honest: run-all.sh globs tests/*.test.sh, so a
# skip-by-absence keeps the runner's suite count accurate instead of
# reporting a failure that means nothing on Linux.
RUN rm -f /root/tests/notify.test.sh

# `docker run` with no arguments runs the whole suite and exits with its
# status, which is what makes this usable from a pre-push hook or CI. Pass a
# single suite name to run just that one.
ENTRYPOINT ["/bin/bash", "/root/tests/run-all.sh"]
