#!/bin/bash
#
# Tests the shared platform-detection helper.
#
# Every config file that has a platform variant (.zshrc, tmux.conf,
# alacritty.toml, deps.conf) needs the same answer to "which platform is
# this". Before this helper existed, each file answered it separately or not
# at all, which is how the mac and linux branches drifted: a file that never
# asks the question has to BE two files, and two files drift.
#
# The contract:
#   1. it names exactly one platform, from a closed set
#   2. the answer is overridable, so a test can drive both branches from
#      one machine
#   3. an unrecognized uname is an explicit "unknown", never a silent
#      guess at mac
#
# Point 3 matters because the variant files are selected by this name. A
# helper that guessed "mac" on an unrecognized system would source
# .zshrc-mac on a BSD box and fail deep inside a Homebrew path rather than
# at the point the assumption was made.
#
# Usage: ~/tests/platform.test.sh

. "$(dirname "$0")/lib.sh"

PLATFORM_SH="$DOTFILES_ROOT/.scripts/platform.sh"

assert_succeeds 'the helper exists' test -f "$PLATFORM_SH"

# Driven through `sh -c` rather than sourced into this bash process, because
# the helper is sourced by .zshrc (zsh), tmux.conf's shell-command hooks
# (sh), and check-deps.sh (sh). POSIX sh is the common denominator it has to
# work in, so that is what it is tested in.
detect_with_uname() {
    local fake_uname=$1
    local stub_dir="$FIXTURES/uname-$fake_uname"
    mkdir -p "$stub_dir"
    printf '#!/bin/sh\nprintf %%s\\\\n "%s"\n' "$fake_uname" > "$stub_dir/uname"
    chmod +x "$stub_dir/uname"
    env DOTFILES_PLATFORM= PATH="$stub_dir:/usr/bin:/bin" \
        sh -c ". '$PLATFORM_SH' && printf '%s' \"\$DOTFILES_PLATFORM\""
}

# --- it names one platform from a closed set --------------------------------

assert_equals 'Darwin is mac' 'mac' "$(detect_with_uname Darwin)"
assert_equals 'Linux is linux' 'linux' "$(detect_with_uname Linux)"

# --- an unrecognized system is explicit, not a guess -------------------------

assert_equals 'FreeBSD is unknown, not mac' 'unknown' "$(detect_with_uname FreeBSD)"

# --- the answer is overridable ----------------------------------------------

# This is what lets one machine test both branches' variant files. Without
# it, the linux variant of every config file would be unreachable from the
# mac branch's test suite, and the drift check would be the only thing
# looking at it.
overridden=$(env DOTFILES_PLATFORM=linux sh -c \
    ". '$PLATFORM_SH' && printf '%s' \"\$DOTFILES_PLATFORM\"")
assert_equals 'a preset DOTFILES_PLATFORM is respected' 'linux' "$overridden"

# An override is only honored when it names a platform the helper knows. A
# typo ("linix") that silently became the platform would select a variant
# file that does not exist, and every config would fall back to bare shared
# behavior with no error -- the failure mode this whole refactor removes.
bad_override=$(env DOTFILES_PLATFORM=linix PATH="/usr/bin:/bin" sh -c \
    ". '$PLATFORM_SH' && printf '%s' \"\$DOTFILES_PLATFORM\"")
assert_equals 'an unrecognized override falls back to detection' \
    "$(uname -s | tr '[:upper:]' '[:lower:]' | sed 's/darwin/mac/')" "$bad_override"

# --- sourcing it twice is harmless ------------------------------------------

# .zshrc sources it, and so does a script .zshrc invokes. Re-sourcing must
# not append to PATH or otherwise accumulate state, because `source ~/.zshrc`
# to reload a shell is a normal thing to do here.
twice=$(env DOTFILES_PLATFORM= sh -c \
    ". '$PLATFORM_SH' && . '$PLATFORM_SH' && printf '%s' \"\$DOTFILES_PLATFORM\"")
once=$(env DOTFILES_PLATFORM= sh -c \
    ". '$PLATFORM_SH' && printf '%s' \"\$DOTFILES_PLATFORM\"")
assert_equals 'sourcing twice gives the same answer' "$once" "$twice"

# --- it exposes the variant-path helper -------------------------------------

# The naming convention (base.ext -> base-mac.ext) is expressed once, here,
# rather than restated at each of the five call sites. A call site that
# spelled the suffix itself would be free to spell it differently, which is
# the drift this replaces.
variant=$(env DOTFILES_PLATFORM=mac sh -c \
    ". '$PLATFORM_SH' && platform_variant /tmp/alacritty.toml")
assert_equals 'a dotted path takes the suffix before the extension' \
    '/tmp/alacritty-mac.toml' "$variant"

variant=$(env DOTFILES_PLATFORM=linux sh -c \
    ". '$PLATFORM_SH' && platform_variant \"\$HOME/.zshrc\"")
assert_equals 'an extensionless dotfile takes a trailing suffix' \
    "$HOME/.zshrc-linux" "$variant"

# A leading-dot filename with no other dot must not be treated as having an
# extension: ".zshrc" is a name, not "" with extension "zshrc". Getting this
# wrong produces "-linux.zshrc", which no file is named.
variant=$(env DOTFILES_PLATFORM=linux sh -c \
    ". '$PLATFORM_SH' && platform_variant /etc/.bashrc")
assert_equals 'a leading dot is not an extension separator' \
    '/etc/.bashrc-linux' "$variant"

# A dot in a parent directory must not be mistaken for the filename's
# extension. ~/.config/tmux/tmux.conf has dots in both places.
variant=$(env DOTFILES_PLATFORM=mac sh -c \
    ". '$PLATFORM_SH' && platform_variant /home/a/.config/tmux/tmux.conf")
assert_equals 'a dot in a parent directory is not the extension' \
    '/home/a/.config/tmux/tmux-mac.conf' "$variant"

finish
