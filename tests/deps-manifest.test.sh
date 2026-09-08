#!/bin/bash
#
# Tests for the dependency MANIFEST: deps.conf, deps-ci.conf and the two
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

DEPS_DIR="$DOTFILES_ROOT/.scripts/deps"
DEPS_CONF_REAL="$DEPS_DIR/deps.conf"
CI_CONF="$DEPS_DIR/deps-ci.conf"

# The positive controls come first. Every assertion below reads one of these
# files, and an unreadable path makes "no bad lines found" and "nothing was
# read" look identical -- a defect this repo has shipped twice.
assert_succeeds 'deps.conf exists' test -f "$DEPS_CONF_REAL"
assert_succeeds 'deps.conf has content' test -s "$DEPS_CONF_REAL"
assert_succeeds 'deps-ci.conf exists' test -f "$CI_CONF"
assert_succeeds 'deps-ci.conf has content' test -s "$CI_CONF"

for platform in mac linux; do
    assert_succeeds "deps-$platform.conf ships here" \
        test -f "$DEPS_DIR/deps-$platform.conf"
done

mkdir -p "$FIXTURES/empty-home"

# --- the shared checks accept either platform's install shape -------------
#
# deps.conf is byte-identical on the mac and linux branches, so each check in
# it has to pass against the macOS shape and the Linux shape of the same
# dependency. These read the checks out of the real file rather than
# restating them, so the test fails if the shipped file regresses.

check_for() {
    grep "^$1|" "$DEPS_CONF_REAL" | cut -d'|' -f2
}

# A Linux machine: the plugin is an oh-my-zsh custom clone, no brew present.
linux_home="$FIXTURES/linux-home"
mkdir -p "$linux_home/.oh-my-zsh/custom/plugins/zsh-autosuggestions"
touch "$linux_home/.oh-my-zsh/custom/plugins/zsh-autosuggestions/zsh-autosuggestions.zsh"
assert_succeeds 'zsh-autosuggestions resolves via the oh-my-zsh plugin path' \
    env HOME="$linux_home" PATH="/usr/bin:/bin" sh -c "$(check_for zsh-autosuggestions)"

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
        env HOME="$FIXTURES/empty-home" sh -c "$(check_for zsh-autosuggestions)"
else
    skip 'zsh-autosuggestions is not installed through brew here'
fi

# A machine with neither shape must still report it missing, or the widened
# check has become a tautology. PATH carries no brew, so the brew operand
# expands to an empty prefix and cannot accidentally match.
bare_home="$FIXTURES/bare-home"
mkdir -p "$bare_home"
if env HOME="$bare_home" PATH="/usr/bin:/bin" sh -c "$(check_for zsh-autosuggestions)" \
        >/dev/null 2>&1; then
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
    env PATH="$alacritty_bin:/usr/bin:/bin" sh -c "$(check_for alacritty)"

if [ -d /Applications/Alacritty.app ]; then
    assert_succeeds 'alacritty resolves via the macOS app bundle' \
        env PATH="/usr/bin:/bin" sh -c "$(check_for alacritty)"
fi

# --- no check command may contain the field delimiter --------------------
#
# read_entries splits on `|`, so a pipe or `||` in a check silently truncates
# it and leaks the rest into the docs URL. The truncated check still evals,
# so this fails as a wrong answer rather than an error.

bad_delimiter=$(awk -F'|' '!/^#/ && NF > 3 { print $1 }' "$DEPS_CONF_REAL")
assert_equals 'no check command contains a pipe' '' "$bad_delimiter"

# A leaked pipe shows up as a docs field that is no longer a bare URL, which
# is the visible symptom of the truncation described above.
malformed_docs=$(grep -v '^#' "$DEPS_CONF_REAL" | grep -v '^$' \
    | awk -F'|' '$3 !~ /^http/ { printf "%s ", $1 }')
assert_equals 'every docs url survives parsing' '' "$malformed_docs"

# --- oh-my-zsh must be tracked on a branch whose shell sources it --------
#
# zsh-autosuggestions is installed two different ways. On this repo's mac
# branch .zshrc-mac sources it from Homebrew's share directory; on the linux
# branch .zshrc-linux sources it from $HOME/.oh-my-zsh/custom/plugins. The
# shared deps.conf check accepts either path, so the check alone cannot say
# whether this machine needs oh-my-zsh -- the branch's own zshrc can.
#
# Moving oh-my-zsh out of the shared deps.conf into deps-linux.conf is
# correct, because the mac machine does not use it. The move is only safe
# while the variant whose zshrc sources from the oh-my-zsh path still lists
# it: drop it there and the shell sources a plugin nothing installs, silently
# losing autosuggestions with every check still reporting success.
#
# Every manifest counts. Both platform variants ship on the single branch, so
# this reads all of them rather than only the one this machine selects --
# which is what lets a mac machine's suite catch a dependency dropped from
# the linux variant.
all_tracked=$(cat "$DEPS_CONF_REAL" \
        "$DOTFILES_ROOT"/.scripts/deps/deps-mac.conf \
        "$DOTFILES_ROOT"/.scripts/deps/deps-linux.conf 2>/dev/null \
    | sed -e 's/#.*//' | cut -d'|' -f1 | grep -E '^[a-z]' | sort -u)

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
# deps-ci.conf holds what the test suite needs and the working environment
# does not: python3, pyyaml, dash. It is never selected by platform detection,
# so `depcheck` on a developer machine does not ask for them.

# No literal pipe in a check field, for the reason the deps.conf version of
# this assertion gives above: a pipe truncates the check and leaks the rest
# into docs_url, silently, with the truncated check still returning an answer.
bad_pipe=''
ci_lines_read=0
while IFS= read -r line; do
    case $line in
        ''|'#'*) continue ;;
    esac
    ci_lines_read=$((ci_lines_read + 1))
    field_count=$(printf '%s\n' "$line" | awk -F'|' '{print NF}')
    [ "$field_count" -eq 3 ] || bad_pipe="$bad_pipe $line"
done < "$CI_CONF"

# The loop above reports an empty list both when every line is well formed
# and when it read no lines at all.
assert_succeeds 'the deps-ci.conf scan read at least one entry' \
    test "$ci_lines_read" -gt 0
assert_equals 'every deps-ci.conf line has exactly three fields' '' "$bad_pipe"

# --- the manifests name what the rest of the repo assumes -------------------
#
# A C toolchain, which the Rust crate needs to link. Nothing installs it on a
# fresh machine unless the manifest carries it.
assert_succeeds 'cc is a tracked dependency' \
    grep -q '^cc|' "$DEPS_CONF_REAL"

# rustup, because the engine is a Rust binary and `config build` cannot run
# without a toolchain. This entry is also why neither deps Docker image may
# carry one: installing rustup into the image would pre-satisfy the very
# dependency the bootstrap exists to exercise.
assert_succeeds 'rustup is a tracked dependency' \
    grep -q '^rustup|' "$DEPS_CONF_REAL"

finish
