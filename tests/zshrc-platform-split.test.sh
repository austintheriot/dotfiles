#!/bin/bash
#
# Tests the .zshrc / .zshrc-<platform> split.
#
# .zshrc used to be a per-branch file. Both branches carried a near-copy of
# the same 300 lines, and they drifted: the mac branch grew a lazy pyenv init
# worth 380ms of startup that linux never received, while the linux branch
# grew an fzf fallback, a $HOME-relative bun path and a stderr redirect that
# mac never received. None of those four were platform-specific. They were
# just fixes applied on whichever machine hit the problem.
#
# So .zshrc is now shared and drift-checked, and the genuinely
# platform-specific lines live in .zshrc-mac / .zshrc-linux -- both of which
# ship on both branches and are drift-checked too.
#
# The contract:
#   1. .zshrc names no platform-specific absolute path of its own
#   2. it loads the variant for THIS platform, via the shared helper
#   3. both variants exist here, so neither can drift unseen
#   4. a variant sets no value the shared file also sets (nvm twice was the
#      concrete bug: linux set NVM_DIR eagerly in .zshrc AND in .zshrc-linux)
#
# Usage: ~/tests/zshrc-platform-split.test.sh

. "$(dirname "$0")/lib.sh"

ZSHRC="$DOTFILES_ROOT/.zshrc"
ZSHRC_MAC="$DOTFILES_ROOT/.zshrc-mac"
ZSHRC_LINUX="$DOTFILES_ROOT/.zshrc-linux"

assert_succeeds 'the shared zshrc exists' test -f "$ZSHRC"

# --- both variants ship on both branches ------------------------------------

# This is what puts them inside the drift check. A variant that existed only
# on its own branch would be exempt from `config check`, which is how the
# per-branch files drifted in the first place.
assert_succeeds 'the mac variant ships here' test -f "$ZSHRC_MAC"
assert_succeeds 'the linux variant ships here' test -f "$ZSHRC_LINUX"

# --- the shared file carries no platform-specific paths ---------------------

# /Applications and /opt/homebrew are macOS-only: a shared file naming either
# is a line that belongs in .zshrc-mac.
#
# Debian's /usr/share/doc/fzf path is deliberately NOT on this list. It sits
# behind a `[ -f ]` guard that is simply false on mac, so it costs a stat and
# describes no assumption. The rule being enforced is "no path that BREAKS on
# the other platform", not "no path that only exists on one".
for needle in '/Applications/' '/opt/homebrew'; do
    assert_equals "the shared zshrc has no $needle path" \
        '' "$(grep -n -- "$needle" "$ZSHRC")"
done

# The hardcoded home directory that drifted: mac had /Users/austin/.bun,
# linux had $HOME/.bun. Neither branch should name a user or a home layout.
assert_equals 'the shared zshrc hardcodes no home directory' \
    '' "$(grep -nE '/(Users|home)/[a-z]+' "$ZSHRC")"

# --- it loads this platform's variant through the shared helper -------------

# Spelled via the helper rather than an inline `if [ -f ~/.zshrc-mac ]` chain
# per platform. The chain is what made adding .zshrc-wsl a three-line edit
# that only ever landed on one branch.
assert_succeeds 'the shared zshrc sources the platform helper' \
    grep -qE '(source|\.)[[:space:]]+.*\.scripts/platform\.sh' "$ZSHRC"

assert_succeeds 'the shared zshrc loads its platform variant' \
    grep -q 'platform_source_variant' "$ZSHRC"

# The old shape, asserted gone: a hardcoded per-platform source chain would
# still work today and drift again tomorrow.
assert_equals 'no hardcoded per-platform source chain remains' \
    '' "$(grep -nE '^[[:space:]]*if \[ -f ~/\.zshrc-(mac|linux) \]' "$ZSHRC")"

# --- a real shell loads the right variant -----------------------------------

# Behavioral, not textual: run zsh with a fake HOME containing the real
# shared .zshrc and a marker variant, and confirm the marker ran. Both
# platforms are driven from whichever machine runs the suite, which is the
# whole reason DOTFILES_PLATFORM is overridable.
if command -v zsh >/dev/null 2>&1; then
    for platform in mac linux; do
        fake_home="$FIXTURES/home-$platform"
        mkdir -p "$fake_home/.scripts"
        cp "$DOTFILES_ROOT/.scripts/platform.sh" "$fake_home/.scripts/platform.sh"

        # Only the variant-loading section is exercised. Sourcing the whole
        # .zshrc would pull in compinit, zoxide and the deps hook, none of
        # which this test is about and all of which are slow.
        sed -n '/ENVIRONMENT-SPECIFIC/,/^# use neovim/p' "$ZSHRC" \
            > "$fake_home/.zshrc"

        printf 'print -r -- "loaded-%s"\n' "$platform" \
            > "$fake_home/.zshrc-$platform"

        output=$(env -i HOME="$fake_home" DOTFILES_PLATFORM="$platform" \
            PATH="/usr/bin:/bin" zsh -c ". '$fake_home/.zshrc'" 2>&1)
        assert_contains "a $platform shell loads the $platform variant" \
            "loaded-$platform" "$output"

        # The other platform's variant must NOT run. A helper that sourced
        # every variant it found would reintroduce the bug where linux ran
        # mac's Homebrew autosuggestions path.
        other=mac; [ "$platform" = mac ] && other=linux
        printf 'print -r -- "loaded-%s"\n' "$other" > "$fake_home/.zshrc-$other"
        output=$(env -i HOME="$fake_home" DOTFILES_PLATFORM="$platform" \
            PATH="/usr/bin:/bin" zsh -c ". '$fake_home/.zshrc'" 2>&1)
        assert_equals "a $platform shell does not load the $other variant" \
            '' "$(printf '%s' "$output" | grep -o "loaded-$other")"
    done
else
    printf 'zshrc-platform-split: skipped shell assertions, zsh not installed\n'
fi

# --- no setting is made twice ------------------------------------------------

# The concrete bug: linux's .zshrc set NVM_DIR and sourced nvm.sh eagerly at
# the bottom, and .zshrc-linux did it again. The eager copy also defeated the
# lazy shim that exists to keep 107 tmux panes from costing minutes.
assert_equals 'the shared zshrc does not source nvm.sh' \
    '' "$(grep -nE '^[[:space:]]*(\\?\.|source)[[:space:]]+.*nvm\.sh' "$ZSHRC")"

# NVM_DIR is set in exactly one place. Both variants need it, so it belongs
# in the shared file -- but then neither variant may set it again.
shared_nvm_dir=$(grep -cE '^[[:space:]]*export NVM_DIR=' "$ZSHRC" || true)
assert_equals 'the shared zshrc sets NVM_DIR exactly once' '1' "$shared_nvm_dir"

for variant in "$ZSHRC_MAC" "$ZSHRC_LINUX"; do
    assert_equals "$(basename "$variant") does not re-set NVM_DIR" \
        '' "$(grep -nE '^[[:space:]]*export NVM_DIR=' "$variant")"
done

finish
