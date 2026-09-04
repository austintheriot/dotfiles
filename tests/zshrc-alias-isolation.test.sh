#!/bin/bash
#
# Integration tests for the shell-state isolation of .zshrc aliases.
#
# An alias that runs `source` executes the script in the interactive shell
# itself, so every `set`/`setopt` the script performs outlives the command.
# `.scripts/tmux-update-window-names.sh` sets `set -u`, which is correct for a
# standalone script and wrong for the caller's shell: with `nounset` left on,
# the next keystroke makes zsh-autosuggestions read the ZLE-only parameter
# POSTDISPLAY outside a widget and print
#
#   _zsh_autosuggest_highlight_apply:3: POSTDISPLAY: parameter not set
#
# before every prompt until the shell is restarted.
#
# The scripts that legitimately need sourcing are the ones that change the
# caller's state on purpose (cd, tmux client state). Those set no shell
# options. The rule this suite enforces: an alias may only `source` a script
# that sets no shell options.
#
# Usage: ~/tests/zshrc-alias-isolation.test.sh

. "$(dirname "$0")/lib.sh"

ZSHRC="$DOTFILES_ROOT/.zshrc"
TAB=$(printf '\t')
NEWLINE='
'

# Every alias whose body sources a script from .scripts, as "alias<TAB>path".
sourced=$(
    grep -oE "^alias [a-zA-Z0-9_-]+='source ~/\.scripts/[A-Za-z0-9_/.-]+'" "$ZSHRC" \
        | sed -E "s#^alias ([a-zA-Z0-9_-]+)='source ~/(\.scripts/[A-Za-z0-9_/.-]+)'#\1${TAB}\2#"
)

assert_succeeds 'at least one sourcing alias is present' \
    test -n "$sourced"

# A sourced script must not set shell options, because they leak into the
# interactive shell. `set -u`/`set -e` and `setopt` without `localoptions` all
# escape; `setopt localoptions ...` is scoped to the function and is fine.
offenders=''
while IFS="$TAB" read -r alias_name rel_path; do
    [ -n "$rel_path" ] || continue
    script="$DOTFILES_ROOT/$rel_path"
    [ -f "$script" ] || continue

    # Strip comments so prose about `set -u` does not count as a setting of it.
    leaks=$(sed 's/#.*//' "$script" \
        | grep -nE '^[[:space:]]*(set[[:space:]]+-[a-zA-Z]*[ue]|setopt[[:space:]]+)' \
        | grep -vE 'setopt[[:space:]]+localoptions' \
        | head -3)

    if [ -n "$leaks" ]; then
        offenders="${offenders}${alias_name} -> ${rel_path}: $(echo "$leaks" | tr '\n' ' ')${NEWLINE}"
    fi
done <<EOF
$sourced
EOF

assert_equals 'no sourcing alias points at a script that sets shell options' \
    '' "$(printf '%s' "$offenders")"

finish
