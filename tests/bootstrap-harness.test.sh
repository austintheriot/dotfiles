#!/bin/bash
#
# Tests the bootstrap container harness: Dockerfile.bootstrap,
# bootstrap-entrypoint.sh, test-bootstrap.sh, and the `bootstrap` job in
# .github/workflows/deps-check.yml.
#
# Builds and runs nothing, for the same reason deps-harness.test.sh does not:
# the container is the slow, network-dependent, daemon-dependent part, and
# run-all.sh gates a pre-commit hook. Exercising the image is
# test-bootstrap.sh's job and CI's job.
#
# What is left is the set of static contracts that can rot silently.
#
# Usage: ~/tests/bootstrap-harness.test.sh

. "$(dirname "$0")/lib.sh"

DOCKERFILE="$DOTFILES_ROOT/.scripts/deps/docker/Dockerfile.bootstrap"
ENTRYPOINT="$DOTFILES_ROOT/.scripts/deps/docker/bootstrap-entrypoint.sh"
HARNESS="$DOTFILES_ROOT/.scripts/deps/test-bootstrap.sh"
WORKFLOW="$DOTFILES_ROOT/.github/workflows/deps-check.yml"
SETUP="$DOTFILES_ROOT/setup.sh"

assert_succeeds 'Dockerfile.bootstrap exists' test -f "$DOCKERFILE"
assert_succeeds 'the entrypoint exists and is executable' test -x "$ENTRYPOINT"
assert_succeeds 'test-bootstrap.sh exists and is executable' test -x "$HARNESS"

# --- the image installs nothing the bootstrap should install ----------------
#
# The whole value of this image is that it starts with almost nothing. An
# apt-get line that quietly grows tmux or zsh turns the bootstrap assertions
# into assertions about the image, and they would keep passing while the
# bootstrap itself broke.
image_installs=$(grep -E '^\s*(RUN\s+)?apt-get install|^\s+sudo curl|^\s+.*--no-install-recommends' "$DOCKERFILE" || true)
for tool in tmux zsh fzf ripgrep neovim shellcheck zoxide; do
    assert_equals "the image does not preinstall $tool" '' \
        "$(printf '%s\n' "$image_installs" | grep -w "$tool" || true)"
done

# git is the one prerequisite setup.sh cannot install, so it must be there.
assert_contains 'the image installs git' 'git' "$image_installs"

# --- it must not run as root ------------------------------------------------
#
# Root hides two real failures: config install-hooks refuses a ~/.local/bin it
# does not own, and every `sudo` in an install command is a no-op for root.
assert_succeeds 'the image switches to a non-root user' \
    grep -qE '^USER +[a-z]' "$DOCKERFILE"
assert_succeeds 'the non-root user has passwordless sudo' \
    grep -q 'NOPASSWD' "$DOCKERFILE"

# --- the collision fixture --------------------------------------------------
#
# A pre-existing dotfile is the single most likely way a naive bootstrap
# fails, and the image plants one so the run has to handle it. Without this,
# the container tests a path no real machine takes.
assert_succeeds 'the image plants a pre-existing .zshrc' \
    grep -q '\.zshrc' "$DOCKERFILE"
assert_succeeds 'the entrypoint asserts the planted file survived' \
    grep -q 'predating the bootstrap' "$ENTRYPOINT"

# --- digest pinning ---------------------------------------------------------
#
# Same decision as Dockerfile.ubuntu, and for the same reason: ubuntu:24.04 is
# republished for every point release, so a tag-only reference makes the image
# change silently under a passing test. Dockerfile.arch is deliberately the
# opposite; deps-harness.test.sh owns that assertion.
assert_succeeds 'the base image is pinned by digest' \
    grep -qE '^FROM .*@sha256:[0-9a-f]{64}' "$DOCKERFILE"

# --- an empty build context -------------------------------------------------
#
# The image copies nothing: the repo arrives through a real clone from a
# bind-mounted bare repo. A COPY here would make the container test a file
# transfer instead of a clone, which is the exact gap this image exists to
# close.
assert_equals 'the Dockerfile copies nothing into the image' '' \
    "$(grep -E '^\s*COPY ' "$DOCKERFILE" || true)"
assert_contains 'the entrypoint clones from the mounted seed' '/seed' "$(cat "$ENTRYPOINT")"

# --- what the entrypoint verifies -------------------------------------------
#
# An exit code alone proves too little: setup.sh can exit 0 having hooked up
# nothing useful. These are the facts a finished bootstrap must leave behind.
for claim in 'status.showUntrackedFiles' '.local/bin/config' 'pre-commit' 'pre-push' 'tmux'; do
    assert_succeeds "the entrypoint verifies $claim" \
        grep -qF "$claim" "$ENTRYPOINT"
done

# Both re-run behaviors, which are opposite on purpose: config init converges,
# setup.sh refuses. A harness that checked only one would let the other regress
# into either a failure or a clobber.
assert_succeeds 'the entrypoint re-runs config init' \
    grep -q 'config.* init --yes' "$ENTRYPOINT"
assert_succeeds 'the entrypoint asserts setup.sh refuses to re-clone' \
    grep -q 'refuses to clobber' "$ENTRYPOINT"

# --- the local harness and CI agree -----------------------------------------

assert_contains 'the local harness names the same image file' \
    'Dockerfile.bootstrap' "$(cat "$HARNESS")"

# The harness must test the working tree, not the last commit. test-local.sh
# documents this trap for its own archive; the same one applies here, and the
# overlay is what avoids it.
assert_contains 'the local harness overlays the working tree' \
    'setup.sh' "$(grep -A3 'for path in' "$HARNESS" || true)"

# --- the CI job -------------------------------------------------------------
#
# Parsed with python3 rather than grepped, matching deps-harness.test.sh, so
# reindenting the workflow cannot fail this suite.
if [ -z "$PYTHON_BIN" ]; then
    skip 'the deps-check workflow declares a bootstrap job' 'no python3'
else
    jobs=$("$PYTHON_BIN" -c "
import yaml, sys
spec = yaml.safe_load(open('$WORKFLOW'))
print(' '.join(sorted(spec.get('jobs', {}))))
" 2>/dev/null)
    assert_contains 'the deps-check workflow declares a bootstrap job' 'bootstrap' "$jobs"

    # The full bootstrap installs over the network on every run, so it must
    # not fire for every commit: an upstream outage would fail work that has
    # nothing to do with dependencies. This workflow's answer is a
    # path-filtered push plus a schedule, which keeps the exposure
    # proportional -- a commit that touches none of the dependency files
    # cannot break the bootstrap. An unfiltered push trigger is the
    # regression to catch.
    triggers=$("$PYTHON_BIN" -c "
import yaml
spec = yaml.safe_load(open('$WORKFLOW'))
on = spec.get('on', spec.get(True, {}))
print(' '.join(sorted(on)) if isinstance(on, dict) else str(on))
" 2>/dev/null)
    assert_contains 'the workflow runs on a schedule' 'schedule' "$triggers"
    assert_contains 'the workflow can be dispatched by hand' 'workflow_dispatch' "$triggers"

    push_paths=$("$PYTHON_BIN" -c "
import yaml
spec = yaml.safe_load(open('$WORKFLOW'))
on = spec.get('on', spec.get(True, {}))
push = (on or {}).get('push') or {}
print(' '.join(push.get('paths', [])))
" 2>/dev/null)
    assert_succeeds 'the push trigger is path-filtered, not unconditional' \
        test -n "$push_paths"

    # setup.sh and the config scripts are now part of what this workflow
    # verifies, so a change to either must reach the filter. Without this the
    # bootstrap job exists but never runs on the commits most likely to break
    # it.
    assert_contains 'the filter covers setup.sh' 'setup.sh' "$push_paths"
    assert_contains 'the filter covers the config scripts' '.scripts/config' "$push_paths"
fi

# --- setup.sh is reachable as a raw URL -------------------------------------
#
# The curl one-liner is the point of the split, so setup.sh must live at the
# repo root where a raw URL can reach it, and must be executable.
assert_succeeds 'setup.sh is at the repo root' test -f "$SETUP"
assert_succeeds 'setup.sh is executable' test -x "$SETUP"

# The URL in its own usage block must name a path that exists. A one-liner
# documented against a moved file is a broken one-liner.
documented_url=$(grep -o 'https://raw.githubusercontent.com/[^ ]*setup.sh' "$SETUP" | head -1)
assert_succeeds 'the usage block documents a raw URL' test -n "$documented_url"
assert_contains 'the documented URL ends at setup.sh' 'setup.sh' "$documented_url"

# --- the bare-root leg ------------------------------------------------------
#
# A user ran the documented one-liner in a plain `docker run debian` and found
# two bugs in seconds that the image above cannot expose, because it installs
# sudo and git and runs as an ordinary user:
#
#   - every install command carried a hardcoded `sudo`, so all eleven
#     privileged installs failed with "sh: 1: sudo: not found";
#   - setup.sh said only "install git, then run this again".
#
# This leg varies exactly those axes and nothing else, so the coverage cannot
# quietly drift back to the comfortable shape.
BARE_DOCKERFILE="$DOTFILES_ROOT/.scripts/deps/docker/Dockerfile.bootstrap-bare"
BARE_ENTRYPOINT="$DOTFILES_ROOT/.scripts/deps/docker/bootstrap-bare-entrypoint.sh"

assert_succeeds 'the bare Dockerfile exists' test -f "$BARE_DOCKERFILE"
assert_succeeds 'the bare entrypoint is executable' test -x "$BARE_ENTRYPOINT"

bare_text=$(cat "$BARE_DOCKERFILE")

# The three axes. Each one is the difference between this image and the other,
# and each is why one of the reported bugs was reachable here.
assert_equals 'the bare image installs no sudo' '' \
    "$(grep -E '^RUN apt-get install|--no-install-recommends' "$BARE_DOCKERFILE" | grep -w 'sudo' || true)"
assert_equals 'the bare image installs no git' '' \
    "$(grep -E '^RUN apt-get install|--no-install-recommends' "$BARE_DOCKERFILE" | grep -w 'git' || true)"
assert_succeeds 'the bare image runs as root' \
    grep -qE '^USER +root' "$BARE_DOCKERFILE"
assert_equals 'the bare image creates no unprivileged user' '' \
    "$(printf '%s\n' "$bare_text" | grep 'useradd' || true)"

# curl IS expected, and that is deliberate rather than an oversight: the
# documented entry point is `curl ... | sh`, so an image without curl cannot
# reach setup.sh at all, and two dependencies install through `curl | sh`.
# Measured: omitting it produced two false failures.
assert_succeeds 'the bare image installs curl, which the one-liner requires' \
    grep -q 'curl' "$BARE_DOCKERFILE"

# Digest-pinned like the others, so a republished tag cannot change a passing
# test underneath us.
assert_succeeds 'the bare base image is pinned by digest' \
    grep -qE '^FROM .*@sha256:[0-9a-f]{64}' "$BARE_DOCKERFILE"

# Copies nothing: the repo arrives by a real clone from the mounted seed.
assert_equals 'the bare Dockerfile copies nothing in' '' \
    "$(grep -E '^\s*COPY ' "$BARE_DOCKERFILE" || true)"

entry_text=$(cat "$BARE_ENTRYPOINT")

# Phase 1 covers the missing-git path, which was the second reported bug. It
# used to assert a printed suggestion; the script now installs git itself,
# because the documented entry point is always piped and refusing there left a
# bare image needing two commands. These assertions follow that change.
assert_contains 'the entrypoint runs setup.sh before git is installed' \
    'phase 1' "$entry_text"
assert_contains 'it asserts git gets installed rather than only named' \
    'installs git rather than only naming it' "$entry_text"
assert_contains 'it asserts the install is confirmed' \
    'it confirms git arrived' "$entry_text"
assert_contains 'it asserts the git install assumes no sudo' \
    'assumes no sudo' "$entry_text"

# The one-command claim: having installed git, the run must carry on into the
# clone rather than stopping. Without this the leg could pass on a script that
# installed git and then exited, which is the behavior being replaced.
assert_contains 'it asserts the run continues into the clone' \
    'continues into the clone' "$entry_text"

# Piped is the case under test, so the entrypoint must drive it that way. A
# run with a terminal takes the prompt path instead and would prove nothing
# about the one-liner.
assert_contains 'the entrypoint drives the piped path' \
    '/dev/null' "$entry_text"

# Phase 2's load-bearing assertion: a PRIVILEGED install succeeded. Without
# this the leg could pass on an image where nothing needing root was ever
# attempted, which is precisely the hole the reported bug lived in.
assert_contains 'it proves a privileged install succeeded' \
    'privileged install actually succeeded' "$entry_text"
assert_contains 'it checks nothing reported a missing sudo' \
    'sudo: not found' "$entry_text"

# The local harness must drive both legs, or the bare one only ever runs in CI
# and nobody sees it fail before pushing.
harness_text=$(cat "$HARNESS")
assert_contains 'the local harness builds the bare image' \
    'Dockerfile.bootstrap-bare' "$harness_text"
# The harness names the bare image by interpolating "$IMAGE-bare" rather than
# spelling it out, so the assertion is on the docker run reaching it, not on a
# literal string that never appears in the source.
assert_succeeds 'the local harness runs the bare image' \
    grep -qE 'docker run .*\$IMAGE-bare' "$HARNESS"

# And CI must declare it as its own job.
if [ -n "$PYTHON_BIN" ]; then
    ci_jobs=$("$PYTHON_BIN" -c "
import yaml
spec = yaml.safe_load(open('$WORKFLOW'))
print(' '.join(sorted(spec.get('jobs', {}))))
" 2>/dev/null)
    assert_contains 'the workflow declares the bare bootstrap job' \
        'bootstrap-bare' "$ci_jobs"
else
    skip 'the workflow declares the bare bootstrap job' 'no python3'
fi


finish
