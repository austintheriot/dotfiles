#!/bin/bash
#
# Tests for the dependency MANIFEST: deps.toml, deps-ci.toml and the two
# platform variants. Not for the engine that reads them -- that is Rust now,
# and `cargo test` covers its exit codes, its install commands, its dry run
# and its --only handling in `crates/deps-core` and `crates/config-cli`.
#
# The split is the point. This suite once drove the retired shell engine
# through a fixture PATH of stubbed package managers, and about two thirds of
# its assertions were about that script's behaviour. Those moved into the
# crates with the code. What did NOT move, and lives here, is everything that
# validates the shipped conf files themselves:
#
#   - every check command parses, on both platforms' shapes of the same
#     dependency;
#   - no check command contains the field delimiter, which truncates it
#     silently;
#   - oh-my-zsh is tracked wherever a shipped zshrc sources from it;
#   - the manifests name the dependencies the rest of the repo assumes.
#
# The conf files are data with no compiler behind them, so a suite that reads
# them is the only thing that catches a malformed line before a bootstrap
# does.
#
# Usage: ~/tests/deps-manifest.test.sh

. "$(dirname "$0")/lib.sh"

DEPS_DIR="$DOTFILES_ROOT/deps"
DEPS_MANIFEST_REAL="$DEPS_DIR/deps.toml"
CI_CONF="$DEPS_DIR/deps-ci.toml"

# The positive controls come first. Every assertion below reads one of these
# files, and an unreadable path makes "no bad lines found" and "nothing was
# read" look identical -- a defect this repo has shipped twice.
assert_succeeds 'deps.toml exists' test -f "$DEPS_MANIFEST_REAL"
assert_succeeds 'deps.toml has content' test -s "$DEPS_MANIFEST_REAL"
assert_succeeds 'deps-ci.toml exists' test -f "$CI_CONF"
assert_succeeds 'deps-ci.toml has content' test -s "$CI_CONF"

for platform in mac linux; do
    assert_succeeds "deps-$platform.toml ships here" \
        test -f "$DEPS_DIR/deps-$platform.toml"
done

mkdir -p "$FIXTURES/empty-home"

# --- the shared checks accept either platform's install shape -------------
#
# deps.toml is byte-identical on the mac and linux branches, so each check in
# it has to pass against the macOS shape and the Linux shape of the same
# dependency. These read the checks out of the real file rather than
# restating them, so the test fails if the shipped file regresses.

# Evaluate one dependency's real check, through the engine that owns it.
#
# The pipe format stored a check as executable shell text, so this used to be
# `grep "^$1|" | cut -d'|' -f2` piped into `sh -c`. TOML stores a TYPED
# check, so there is no shell string to extract -- and re-deriving one would
# rebuild the `sh -c "$check"` the closed `Check` sum exists to remove.
#
# Asking the engine is also the stronger test: it exercises the same code
# path a real `config deps check` takes, rather than a shell approximation of
# it that could agree with a broken engine. Exit 0 means present, 1 means
# missing, per the engine's exit contract.
#
# DEPS_LOCAL_CONF points at a path that does not exist so the platform
# variant is suppressed: these assertions are about the SHARED manifest
# accepting either platform's shape, and pulling in deps-mac.toml would let
# a mac-only entry answer for it.
# The engine binary's own directory is always appended to the PATH under
# test. Without it the narrowed PATH hides `config-cli` itself and every
# assertion fails with exit 127 -- which is indistinguishable from the
# dependency being missing, and would have read as a passing "missing"
# assertion in the bare-machine case below. Measured: that is exactly what
# the first version of this helper did.
#
# Appended, not prepended, so a fixture binary earlier in the PATH still
# wins for the dependency being checked.
ENGINE_BIN_DIR=$(dirname "$(command -v config-cli)")

check_via_engine() {
    env HOME="$2" PATH="${3:-/usr/bin:/bin}:$ENGINE_BIN_DIR" \
        DOTFILES_ROOT="$DOTFILES_ROOT" \
        DEPS_LOCAL_CONF=/nonexistent/deps-platform.toml \
        config-cli deps check --only "$1" >/dev/null 2>&1
}

# A Linux machine: the plugin is an oh-my-zsh custom clone, no brew present.
linux_home="$FIXTURES/linux-home"
mkdir -p "$linux_home/.oh-my-zsh/custom/plugins/zsh-autosuggestions"
touch "$linux_home/.oh-my-zsh/custom/plugins/zsh-autosuggestions/zsh-autosuggestions.zsh"
assert_succeeds 'zsh-autosuggestions resolves via the oh-my-zsh plugin path' \
    check_via_engine zsh-autosuggestions "$linux_home"

# This machine: the plugin comes from brew, and there is no ~/.oh-my-zsh.
#
# Guarded on the plugin file, not on brew being installed. `command -v brew`
# was the wrong question: a macOS CI runner has brew and need not have this
# package, so the guard passed and the assertion then failed on a machine
# where the check command is behaving correctly. It broke the moment
# test-suite.yml stopped naming zsh-autosuggestions in its --only list,
# which it stopped doing because no test needs the package installed --
# except this one, which needs it to prove the brew branch of a two-branch
# check.
#
# Skipped rather than silently absent, per the suite's own rule: a check
# that cannot run must say so, or it is indistinguishable from one that is
# not there.
brew_plugin="$(brew --prefix 2>/dev/null)/share/zsh-autosuggestions/zsh-autosuggestions.zsh"
if [ -f "$brew_plugin" ]; then
    assert_succeeds 'zsh-autosuggestions resolves via the brew share path' \
        check_via_engine zsh-autosuggestions "$FIXTURES/empty-home" "$PATH"
else
    skip 'zsh-autosuggestions is not installed through brew here'
fi

# A machine with neither shape must still report it missing, or the widened
# check has become a tautology. PATH carries no brew, so the brew operand
# expands to an empty prefix and cannot accidentally match.
bare_home="$FIXTURES/bare-home"
mkdir -p "$bare_home"
if check_via_engine zsh-autosuggestions "$bare_home"; then
    bare_result=present
else
    bare_result=missing
fi
assert_equals 'zsh-autosuggestions is missing on a bare machine' \
    'missing' "$bare_result"

# alacritty is a .app bundle on macOS and a PATH binary on Linux.
alacritty_bin="$FIXTURES/alacritty-bin"
mkdir -p "$alacritty_bin"
printf '#!/bin/sh\nexit 0\n' > "$alacritty_bin/alacritty"
chmod +x "$alacritty_bin/alacritty"
assert_succeeds 'alacritty resolves via a binary on PATH' \
    check_via_engine alacritty "$FIXTURES/empty-home" "$alacritty_bin:/usr/bin:/bin"

if [ -d /Applications/Alacritty.app ]; then
    assert_succeeds 'alacritty resolves via the macOS app bundle' \
        check_via_engine alacritty "$FIXTURES/empty-home"
fi

# --- the field-delimiter assertions are gone on purpose -------------------
#
# Two assertions lived here: "no check command contains a pipe" and "every
# docs url survives parsing". Both existed because the pipe format split each
# line on `|`, so a pipe inside a check truncated it and leaked the remainder
# into the docs field -- and the truncated check still evaluated, making it a
# WRONG ANSWER rather than an error.
#
# TOML cannot express that failure. A pipe in a string is a character in a
# string, and the docs value is a named key rather than the third positional
# field. There is nothing left for either assertion to test, so they are
# deleted rather than ported: an assertion whose failure mode is
# unrepresentable is one that passes forever and tells a reader nothing.
#
# The guarantee they were reaching for now lives in
# `crates/deps-core/src/manifest.rs` as `deny_unknown_fields` plus the
# exactly-one-check rule, both with their own tests.

# --- oh-my-zsh must be tracked on a branch whose shell sources it --------
#
# zsh-autosuggestions is installed two different ways. On this repo's mac
# branch .zshrc-mac sources it from Homebrew's share directory; on the linux
# branch .zshrc-linux sources it from $HOME/.oh-my-zsh/custom/plugins. The
# shared deps.toml check accepts either path, so the check alone cannot say
# whether this machine needs oh-my-zsh -- the branch's own zshrc can.
#
# Moving oh-my-zsh out of the shared deps.toml into deps-linux.toml is
# correct, because the mac machine does not use it. The move is only safe
# while the variant whose zshrc sources from the oh-my-zsh path still lists
# it: drop it there and the shell sources a plugin nothing installs, silently
# losing autosuggestions with every check still reporting success.
#
# Every manifest counts. Both platform variants ship on the single branch, so
# this reads all of them rather than only the one this machine selects --
# which is what lets a mac machine's suite catch a dependency dropped from
# the linux variant.
all_tracked=$(cat "$DEPS_MANIFEST_REAL" \
        "$DOTFILES_ROOT"/deps/deps-mac.toml \
        "$DOTFILES_ROOT"/deps/deps-linux.toml 2>/dev/null \
    | sed -e 's/#.*//' | grep -oE '^\[[a-z][a-z0-9-]*\]' | tr -d '[]' | sort -u)

sources_from_oh_my_zsh=0
for zshrc in "$DOTFILES_ROOT"/.zshrc "$DOTFILES_ROOT"/.zshrc-*; do
    [ -f "$zshrc" ] || continue
    grep -q '^[^#]*source.*\.oh-my-zsh' "$zshrc" && sources_from_oh_my_zsh=1
done

if [ "$sources_from_oh_my_zsh" -eq 1 ]; then
    oh_my_zsh_tracked=$(printf '%s\n' "$all_tracked" | grep -cx 'oh-my-zsh')
    assert_equals 'oh-my-zsh is tracked on a branch whose shell sources it' \
        '1' "$oh_my_zsh_tracked"
else
    assert_equals 'no zshrc on this branch sources from oh-my-zsh' \
        '0' "$sources_from_oh_my_zsh"
fi

# --- the CI-only manifest ---------------------------------------------------
#
# deps-ci.toml holds what the test suite needs and the working environment
# does not: python3, pyyaml, dash. It is never selected by platform
# detection, so `depcheck` on a developer machine does not ask for them.

# The whole file parses, asked of the engine rather than field-counted in
# shell.
#
# The assertion this replaces counted three pipe-separated fields per line,
# which was the only way to catch a check corrupted by a literal `|`. TOML
# removes that failure, so a field count has nothing to find. What is still
# worth asserting is that the file the CI workflow names actually loads: a
# manifest that silently reads as empty is this repo's recurring bug, and it
# is what `DEPS_CONF` pointing at the wrong name produced before.
#
# Explicit DEPS_CONF, matching how the workflow selects it -- that is also
# what makes its `python_import` entry legal, which a platform-selected file
# may not carry.
ci_entries=$(env DOTFILES_ROOT="$DOTFILES_ROOT" \
    DEPS_CONF="$(basename "$CI_CONF")" \
    DEPS_LOCAL_CONF=/nonexistent/deps-platform.toml \
    config-cli deps check 2>/dev/null | grep -cE '^  (present|missing) ')
assert_succeeds 'the CI manifest parses to at least one entry' \
    test "$ci_entries" -gt 0

# pyyaml is the entry that proves the ExplicitOnly kind is in force: it is a
# `python_import`, which the parser refuses in any platform-selected file. If
# the workflow ever selected this manifest by platform instead, the load
# would fail rather than quietly dropping the check.
assert_succeeds 'the CI manifest carries the python_import entry' \
    grep -q '^\[pyyaml\]' "$CI_CONF"

# --- the manifests name what the rest of the repo assumes -------------------
#
# A C toolchain, which the Rust crate needs to link. Nothing installs it on a
# fresh machine unless the manifest carries it.
assert_succeeds 'cc is a tracked dependency' \
    grep -q '^\[cc\]' "$DEPS_MANIFEST_REAL"

# rustup, because the engine is a Rust binary and `config build` cannot run
# without a toolchain. This entry is also why neither deps Docker image may
# carry one: installing rustup into the image would pre-satisfy the very
# dependency the bootstrap exists to exercise.
assert_succeeds 'rustup is a tracked dependency' \
    grep -q '^\[rustup\]' "$DEPS_MANIFEST_REAL"

finish
