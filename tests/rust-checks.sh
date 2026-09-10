#!/bin/sh
#
# Runs the workspace's Rust checks against a ref, in a throwaway snapshot of
# that ref.
#
# Called by tests/pre-push before it hands off to Docker, and usable by hand
# as `tests/rust-checks.sh [REF]`.
#
# Why a snapshot rather than the working tree: $HOME is not always on the
# branch being pushed (a push from a worktree leaves $HOME on whatever
# branch it was already on), so checking the working tree would report a
# pass for code that is not being pushed. run-in-docker.sh solves the same
# hazard the same way.
#
# Why these checks are safe on the host, unlike the shell suites: they
# mutate nothing. deps-core and dotfiles-path are pure and deps-core
# enforces it with a purity test over its own sources; every config-manifest
# test builds its own temporary directory and passes explicit --git-dir and
# --work-tree. Verified by running the workspace with HOME pointed at an
# empty directory: the real home was unchanged and the fake home was still
# empty afterwards. This is NOT the host fallback tests/pre-push rejects: a
# fallback substitutes a weaker check when Docker is down, while this always
# runs and never replaces the container leg.

set -u

ref=${1:-HEAD}

# The repository being pushed, not always the dotfiles repository.
#
# Git exports GIT_DIR and GIT_WORK_TREE into a hook, naming the repo whose
# push is being gated, so honouring them is what makes this script correct
# for any caller. Falling back to ~/.cfg keeps a bare `tests/rust-checks.sh`
# working by hand, where nothing is exported.
#
# Hardcoding ~/.cfg was wrong and CI caught it: leak-check.test.sh drives the
# hook with GIT_DIR pointed at a throwaway fixture repo, so a ref that exists
# only there could not be archived out of ~/.cfg, the archive failed, and the
# hook blocked a push it should have allowed. The host run passed because the
# ref happened to exist in both.
GIT_DIR_PATH="${GIT_DIR:-$HOME/.cfg}"
WORK_TREE_PATH="${GIT_WORK_TREE:-$HOME}"

git_cmd() {
    git --git-dir="$GIT_DIR_PATH" --work-tree="$WORK_TREE_PATH" "$@"
}

# Loud, and exit 0. A machine with no Rust toolchain can still push a shell
# change, and a gate that stays silent when it skips is indistinguishable
# from one that is not installed.
if ! command -v cargo >/dev/null 2>&1; then
    printf 'SKIP  rust checks (cargo not found)\n'
    exit 0
fi

if ! git_cmd rev-parse --verify --quiet "$ref" >/dev/null 2>&1; then
    printf 'rust-checks: %s is not a ref in this repository.\n' "$ref" >&2
    exit 1
fi

# No crates at this ref means nothing to check. Asked before archiving so an
# older ref does not fail on an empty snapshot.
if ! git_cmd rev-parse --verify --quiet "$ref:crates/Cargo.toml" >/dev/null 2>&1; then
    printf 'rust-checks: %s carries no crates/Cargo.toml, nothing to check\n' "$ref"
    exit 0
fi

snapshot=$(mktemp -d "${TMPDIR:-/tmp}/rust-checks.XXXXXX") || {
    printf 'rust-checks: cannot create a snapshot directory\n' >&2
    exit 1
}
# Trapped on EXIT rather than removed at the end, so an early exit or a
# failing check still cleans up.
trap 'rm -rf "$snapshot"' EXIT

# crates/ plus every tracked path a crate reads at COMPILE time.
#
# config-cli's catalog embeds the dependency manifests with
# `include_str!("../../../../deps/*.conf")` and `.../*.toml`, which escapes
# crates/. Whole-directory entries rather than per-file ones, which is why
# adding the TOML form needed no change here.
# Archiving crates/ alone left those paths absent and the crate could not
# compile, while a developer-machine build succeeded because the real files
# were simply there. That is this repo's dominant bug class, the environment
# compensating for a gap the engine has, and this gate exists to catch it.
#
# A path that no longer exists at $ref is skipped rather than fatal, so an
# older ref that predates a directory still checks the rest.
# One entry today. Kept as a list because the next crate that embeds a
# tracked file will add to it, and a list makes that a one-word change
# rather than a restructure.
# `deps` is embedded at compile time. `README.md`, `.scripts` and
# `.github` are read at RUN time by the converted suites (config_docs reads
# the README and the config-* scripts, workflow_labels reads the workflows).
#
# Widening the snapshot rather than pointing those tests at $HOME is
# deliberate, and it is this gate's own argument: the comment above says the
# dominant bug class here is "the environment compensating for a gap the
# engine has, and this gate exists to catch it." A test that reads the live
# tree while the gate checks a ref cannot catch that. A test that skips when
# the file is absent checks nothing at all.
COMPILE_TIME_PATHS='deps README.md .scripts .github'

archive_paths='crates'
for compile_time_path in $COMPILE_TIME_PATHS; do
    if git_cmd rev-parse --verify --quiet "$ref:$compile_time_path" >/dev/null 2>&1; then
        archive_paths="$archive_paths $compile_time_path"
    fi
done

# Word splitting is intended: archive_paths is a space-separated pathspec
# list built above, not a single path.
# shellcheck disable=SC2086
if ! git_cmd archive "$ref" $archive_paths | tar -x -C "$snapshot"; then
    printf 'rust-checks: cannot archive %s from %s\n' "$archive_paths" "$ref" >&2
    exit 1
fi

# The shared cache, not a fresh directory under the snapshot. Measured on
# this repo: a cold target directory takes over 120 seconds and the shared
# one takes 7, and a 120-second gate is one somebody switches off. Same
# default run-all.sh uses.
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/config-manifest/target}"
export CARGO_TARGET_DIR

# Run from inside crates/, never with --manifest-path: rustup honours
# crates/rust-toolchain.toml only when the working directory is under
# crates/, so a run from the snapshot root would declare the 1.94.0 pin
# without applying it. Verified that the archived tree carries
# rust-toolchain.toml and reports cargo 1.94.0 from inside crates/.
status=0

# git's environment is CLEARED before cargo runs, and this is load-bearing
# rather than defensive. Git exports GIT_DIR and GIT_WORK_TREE into every
# hook; config-manifest's tests build throwaway git repositories as
# fixtures; and with those variables set a fixture's `git add .` operates on
# the real dotfiles repo instead of the fixture. Observed exactly that: a
# fixture took ~/.cfg/index.lock and blocked every `config` command until the
# process was killed. tests/pre-push guards its Docker call the same way and
# says the same thing.
run_in_snapshot() {
    (
        cd "$snapshot/crates" || exit 1
        env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE -u GIT_PREFIX "$@"
    )
}

# The skip log. Rust's harness has no runtime skip, so a test that cannot run
# here records one line to this file and the count is reported below. See
# docs/superpowers/specs/2026-09-07-shell-test-port-design.md section 4b.1.
#
# Truncated rather than deleted: a stale count from a previous run would
# overstate missing coverage, and a missing file is indistinguishable from a
# run where nothing skipped.
DOTFILES_SKIP_LOG="${TMPDIR:-/tmp}/dotfiles-rust-skips.jsonl"
export DOTFILES_SKIP_LOG
: > "$DOTFILES_SKIP_LOG"

printf 'rust-checks: cargo test (%s)\n' "$ref"
if ! run_in_snapshot cargo test --locked --quiet; then
    printf 'rust-checks: cargo test failed\n' >&2
    status=1
fi

# Report the skips the way run-all.sh does for the shell suites, because a
# gate that says nothing when it skips is indistinguishable from one that is
# not installed. Silent when nothing skipped: a trailing "0 skipped" on every
# run is noise, and noise is what a reader learns to scan past.
if [ -s "$DOTFILES_SKIP_LOG" ]; then
    skipped=$(wc -l < "$DOTFILES_SKIP_LOG" | tr -d ' ')
    # One test that skips can appear more than once: cargo builds an
    # integration target per lib and bin target of the crate under test, so
    # the binary runs twice and each run records its own skip. The count is
    # honest -- both runs really could not run -- but it is not a count of
    # DISTINCT skipped checks. Read the reasons, not the number.
    printf 'rust-checks: %s skipped\n' "$skipped"
    sed -n 's/.*"reason":"\(.*\)"}/  skip: \1/p' "$DOTFILES_SKIP_LOG"
fi

printf 'rust-checks: cargo clippy (%s)\n' "$ref"
if ! run_in_snapshot cargo clippy --locked --all-targets -- -D warnings; then
    printf 'rust-checks: cargo clippy failed\n' >&2
    status=1
fi

# Both checks run even when the first fails, so one push reports every
# problem rather than making the developer discover them one at a time.
exit "$status"
