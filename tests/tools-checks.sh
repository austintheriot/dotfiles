#!/bin/sh
#
# Runs the thumper checks against a ref, in a throwaway snapshot of that
# ref: the UI build, typecheck and tests, clippy and cargo test on the daemon
# workspace, and the installer tests.
#
# tools/thumper is its own Cargo workspace with a Bun UI, so
# rust-checks.sh, which runs from crates/, never builds it. tests/pre-push
# calls this only for a push that touches TOOLS_TRIGGER_PATHS, because a cold
# Tauri build takes minutes. .github/workflows/thumper.yml runs the same
# checks in CI, and crates/config-cli/tests/tools_gate.rs holds the two in
# step. Usable by hand as `tests/tools-checks.sh [REF]`.
#
# A snapshot rather than the working tree for the reason rust-checks.sh gives:
# the ref being pushed is not always what $HOME has checked out.

set -u

ref=${1:-HEAD}

GIT_DIR_PATH="${GIT_DIR:-$HOME/.cfg}"
WORK_TREE_PATH="${GIT_WORK_TREE:-$HOME}"

git_cmd() {
    git -C "$WORK_TREE_PATH" --git-dir="$GIT_DIR_PATH" --work-tree="$WORK_TREE_PATH" "$@"
}

for tool in cargo bun; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        printf 'SKIP  thumper checks (%s not found)\n' "$tool"
        exit 0
    fi
done

# The app links AppKit, and the Linux shell does not exist yet.
if [ "$(uname -s)" != Darwin ]; then
    printf 'SKIP  thumper checks (the daemon builds on macOS only)\n'
    exit 0
fi

if ! git_cmd rev-parse --verify --quiet "$ref" >/dev/null 2>&1; then
    printf 'tools-checks: %s is not a ref in this repository.\n' "$ref" >&2
    exit 1
fi

if ! git_cmd rev-parse --verify --quiet "$ref:tools/thumper/Cargo.toml" >/dev/null 2>&1; then
    printf 'tools-checks: %s carries no tools/thumper, nothing to check\n' "$ref"
    exit 0
fi

snapshot=$(mktemp -d "${TMPDIR:-/tmp}/tools-checks.XXXXXX") || {
    printf 'tools-checks: cannot create a snapshot directory\n' >&2
    exit 1
}
trap 'rm -rf "$snapshot"' EXIT

# crates/ because the workspace takes thumper-core and
# dotfiles-test-support as path dependencies, and the installer tests read it.
archive_paths='tools/thumper'
if git_cmd rev-parse --verify --quiet "$ref:crates" >/dev/null 2>&1; then
    archive_paths="$archive_paths crates"
fi

# shellcheck disable=SC2086 # archive_paths is a space-separated pathspec list.
if ! git_cmd archive "$ref" $archive_paths | tar -x -C "$snapshot"; then
    printf 'tools-checks: cannot archive %s from %s\n' "$archive_paths" "$ref" >&2
    exit 1
fi

# The installer's target directory, so the Tauri dependencies stay warm.
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/thumper/target}"
export CARGO_TARGET_DIR

status=0

# Runs one check from a directory under the snapshot's tools/thumper.
# A failure is recorded and the next check still runs, so one push names
# every problem. A status above 128 is a signal death, Ctrl-C included, and
# stops the gate with that status.
#
# git's environment is cleared because the daemon tests build fixture
# repositories, and a hook exports GIT_DIR at the real one.
run_check() {
    directory=$1
    shift
    printf 'tools-checks: %s in %s (%s)\n' "$*" "${directory:-.}" "$ref"
    check_status=0
    (
        cd "$snapshot/tools/thumper/$directory" || exit 1
        env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE -u GIT_PREFIX "$@"
    ) || check_status=$?
    if [ "$check_status" -gt 128 ]; then
        printf 'tools-checks: interrupted (%s), stopping\n' "$check_status" >&2
        exit "$check_status"
    fi
    if [ "$check_status" -ne 0 ]; then
        printf 'tools-checks: %s failed\n' "$*" >&2
        status=1
    fi
}

run_check ui bun install --frozen-lockfile
run_check ui bun run build
run_check ui bun run typecheck
run_check ui bun test
run_check '' cargo clippy --locked --all-targets -- -D warnings
run_check '' cargo test --locked --quiet
run_check '' bun test

exit "$status"
