#!/bin/bash
#
# Integration tests for .my-scripts/zsh-git-widgets.sh
#
# The file registers a ZLE widget and key bindings at shell-init time. Driving
# the widget itself needs an interactive zsh and an fzf prompt, which a test
# cannot supply, so these check the registration contract instead: sourcing the
# file must define the widget and bind it in all three keymaps. That is enough
# to catch the failure that actually happens, which is a syntax error or a
# renamed widget silently costing you Ctrl+G.
#
# Usage: ~/tests/zsh-git-widgets.test.sh

. "$(dirname "$0")/lib.sh"

SCRIPT="$DOTFILES_ROOT/.my-scripts/zsh-git-widgets.sh"

# ZLE is only available in an interactive zsh, and starting one costs about
# four seconds here because .zshrc loads pyenv, zoxide, and fzf. So the whole
# registration state is collected in a single interactive shell and the
# assertions read that one report.
report=$(zsh -i -c "
    source '$SCRIPT' 2>/dev/null
    print -- '--widgets--'
    zle -l | grep fzf-git-branch-widget
    for keymap in emacs vicmd viins; do
        print -- \"--bind-\$keymap--\"
        bindkey -M \$keymap '^G'
    done
" 2>/dev/null)

section() {
    printf '%s' "$report" | sed -n "/^--$1--\$/,/^--/p" | grep -v '^--'
}

assert_equals 'sourcing the file succeeds' '0' \
    "$(zsh -c "source '$SCRIPT'" >/dev/null 2>&1; echo $?)"

assert_contains 'the branch widget is registered with ZLE' 'fzf-git-branch-widget' \
    "$(section widgets)"

assert_equals 'the file defines exactly one widget' '1' \
    "$(section widgets | grep -c fzf-git-branch-widget)"

for keymap in emacs vicmd viins; do
    assert_contains "Ctrl+G is bound in the $keymap keymap" 'fzf-git-branch-widget' \
        "$(section "bind-$keymap")"
done

# --- the dependencies the widget shells out to -------------------------

for tool in git rg fzf; do
    assert_succeeds "$tool is installed, which the widget needs" command -v "$tool"
done

finish
