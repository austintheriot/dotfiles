#!/bin/bash
#
# Tests for the containerized test runner: tests/docker/Dockerfile and
# tests/run-in-docker.sh.
#
# The suite mutates $HOME by design -- it writes fixture repos, spawns tmux
# sessions, and (before the install-path fix) once created a real
# ~/.oh-my-zsh on a machine that does not use oh-my-zsh. Running it inside a
# container keeps those side effects off the developer's machine.
#
# This file asserts the runner's static contracts only. It never builds or
# runs the image: run-all.sh has to stay fast and must not require a Docker
# daemon, and a build here would recurse (the image runs run-all.sh, which
# runs this file).
#
# Usage: ~/tests/container.test.sh

. "$(dirname "$0")/lib.sh"

DOCKERFILE="$DOTFILES_ROOT/tests/docker/Dockerfile"
RUNNER="$DOTFILES_ROOT/tests/run-in-docker.sh"

# Reads a single-line directive out of the Dockerfile, ignoring comments.
directive() {
    grep -E "^$1 " "$DOCKERFILE" | head -1
}

# --- the files exist and parse -------------------------------------------

assert_succeeds 'the Dockerfile exists' test -f "$DOCKERFILE"
assert_succeeds 'the runner exists' test -f "$RUNNER"
assert_succeeds 'the runner is executable' test -x "$RUNNER"

# /bin/sh is bash on macOS and accepts bashisms dash rejects, so `sh -n`
# alone would pass a script that breaks on the linux branch.
assert_succeeds 'the runner parses as POSIX sh' sh -n "$RUNNER"
if command -v dash >/dev/null 2>&1; then
    assert_succeeds 'the runner parses under dash' dash -n "$RUNNER"
fi

# --- the image carries every tool the suite shells out to ----------------
#
# A missing tool does not fail the suite honestly: lib.sh's tmux helpers and
# the zsh widget tests would error in ways that read as unrelated breakage.

for tool in tmux zsh git python3 dash fzf ripgrep; do
    assert_contains "the image installs $tool" "$tool" "$(cat "$DOCKERFILE")"
done

# --- the container runs as a throwaway, non-root-owned HOME --------------

assert_contains 'the image sets HOME' 'ENV HOME=' "$(cat "$DOCKERFILE")"

# --- the suite is the entrypoint ----------------------------------------
#
# The image's job is "run the suite and exit with its status", which is what
# makes it usable from a pre-push hook or CI.

assert_contains 'the entrypoint runs the suite' 'run-all.sh' "$(directive ENTRYPOINT)$(directive CMD)"

# --- macOS-only suites are excluded -------------------------------------
#
# notify.test.sh drives aerospace and osascript. Neither exists on Linux, so
# it has to be skipped rather than left to fail as a false negative.

assert_contains 'the runner or image skips the macOS-only suite' \
    'notify' "$(cat "$DOCKERFILE" "$RUNNER")"

# --- the runner never mutates the real HOME -----------------------------
#
# The whole point. A bind mount of $HOME would defeat it, so the runner must
# copy the tree in rather than mount it read-write.

mounts_home_writable=$(grep -cE -- '-v[[:space:]]*"?\$(HOME|\{HOME\})"?:[^:]*(:rw)?[[:space:]]*\\?$' "$RUNNER" || true)
assert_equals 'the runner does not bind-mount HOME read-write' '0' "$mounts_home_writable"

# --- the runner fails clearly without Docker ----------------------------

no_docker_bin="$FIXTURES/no-docker-bin"
mkdir -p "$no_docker_bin"
for passthrough in env sh bash printf mktemp rm cp tar git sed grep; do
    real=$(command -v "$passthrough" 2>/dev/null) || continue
    ln -sf "$real" "$no_docker_bin/$passthrough"
done

output=$(PATH="$no_docker_bin" "$RUNNER" 2>&1)
status=$?
assert_equals 'a missing docker exits non-zero' '1' "$status"

# Assert on the runner's own guard, not on any line mentioning "docker": the
# shell's own "command not found: docker" would satisfy that even if the
# guard were deleted or reworded, which makes it unfalsifiable.
assert_contains 'a missing docker explains it is not on PATH' \
    'docker is not on PATH' "$output"
assert_contains 'a missing docker names the direct alternative' \
    'run-all.sh' "$output"

# --- the runner tests the ref being pushed, not the checked-out branch ---
#
# The image is built from `git archive <branch>`. The pre-push hook can push
# a ref that is not the checked-out branch: pushing linux from a worktree
# while $HOME sits on mac is the normal way this repo ships a linux change.
# A runner that always archives the checked-out branch would build mac's code
# and report a pass for a linux push, which is a false green -- strictly
# worse than the flaky host run this replaces.

assert_contains 'the runner accepts a ref argument' \
    'DOTFILES_TEST_REF' "$(cat "$RUNNER")"

# The override has to actually change which ref is archived. Grepping the
# source cannot show that: `git archive "$branch"` never contains the string
# `show-current` whatever $branch was assigned from, so a source-text
# assertion passes even when the override is ignored entirely.
#
# Instead run the runner with $DOTFILES_TEST_REF set to a ref that does not
# exist, on a PATH with no docker. The ref is resolved before the docker
# guard, so an honoured override reports the bad ref while an ignored one
# falls through to the docker complaint.
output=$(PATH="$no_docker_bin" DOTFILES_TEST_REF=refs/heads/no-such-ref \
    "$RUNNER" 2>&1)
status=$?
# Assert on the outcome, not the wording: a bad ref must stop the run before
# it can reach docker. Matching only the message would still pass if the
# guard were deleted and the failure deferred to `git archive`, which exits
# non-zero too but after the daemon probe.
assert_equals 'a ref that does not exist exits non-zero' '1' "$status"
assert_equals 'a bad ref never reaches the docker probe' \
    '0' "$(printf '%s' "$output" | grep -c 'docker is not on PATH')"

# And a real ref must get past that check, so the guard is not simply
# refusing everything. Only meaningful where the repository exists: this
# file also runs inside the test image, which carries the tracked files but
# not the .cfg repository they came from.
if [ -d "$DOTFILES_ROOT/.cfg" ]; then
    output=$(PATH="$no_docker_bin" DOTFILES_TEST_REF=HEAD "$RUNNER" 2>&1)
    assert_equals 'a valid ref passes ref resolution' \
        '0' "$(printf '%s' "$output" | grep -c 'not a ref in this repository')"
fi

# --- the pre-push hook runs the suite in the container -------------------
#
# The host suite spawns tmux sessions on the real server and writes fixture
# repos under $HOME. That is what made tmux-update-window-names.test.sh flaky
# enough to block a push whose code was fine.

HOOK="$DOTFILES_ROOT/tests/pre-push"
assert_succeeds 'the pre-push hook exists' test -f "$HOOK"
hook_text=$(cat "$HOOK" 2>/dev/null)

assert_contains 'the hook runs the suite in the container' \
    'run-in-docker.sh' "$hook_text"
assert_contains 'the hook passes the pushed ref to the runner' \
    'DOTFILES_TEST_REF' "$hook_text"

# A hook that silently falls back to the host suite when Docker is down
# reintroduces the flake it exists to avoid, without saying so.
assert_equals 'the hook does not invoke run-all.sh directly' \
    '0' "$(printf '%s' "$hook_text" | grep -c 'tests/run-all\.sh')"

finish
