#!/bin/bash
#
# Tests the git alias installation in .zshrc.
#
# The aliases are installed from .zshrc rather than a tracked .gitconfig, so
# that the real ~/.gitconfig -- which carries credentials and per-machine
# settings -- never has to live in this repo. That is the right call. The cost
# was how it was done: one `git config --global --get` per alias, on every
# shell startup, to discover that all eight were already installed. Eight
# subprocesses, measured at 140ms, to change nothing.
#
# The contract:
#   1. an already-installed set costs no git subprocess at startup
#   2. a missing alias is still installed
#   3. an alias whose value was changed by hand is left alone
#
# Point 3 is the one a sentinel can get wrong. A guard that skips the whole
# block once any alias exists must not then be a guard that reinstalls every
# alias whenever one is missing, or it would overwrite a deliberate local
# edit.
#
# Usage: ~/tests/zshrc-git-aliases.test.sh

. "$(dirname "$0")/lib.sh"

ZSHRC="$DOTFILES_ROOT/.zshrc"

assert_succeeds 'the zshrc exists' test -f "$ZSHRC"

# --- startup spends no subprocess on an installed set -----------------------

# Asserted against the source text, the same way the nvm suite does it: an
# unconditional `git config` per alias at top level is the shape being
# removed, and it is visible without running a shell.
top_level_gets=$(grep -c '^if ! git config --global --get' "$ZSHRC" || true)
assert_equals 'no per-alias git config --get runs at top level' '0' "$top_level_gets"

# --- the aliases still get installed ----------------------------------------

# Driven against a throwaway HOME with its own gitconfig, so the developer's
# real global config is never read or written.
fixture_home="$FIXTURES/git-home"
mkdir -p "$fixture_home"

# The installer block, extracted from .zshrc and run in isolation. Everything
# else in .zshrc (nvm, pyenv, oh-my-zsh) is irrelevant here and slow.
installer="$FIXTURES/installer.sh"
awk '/^# GIT ALIASES/{f=1} f && /^# ENVIRONMENT-SPECIFIC/{exit} f' \
    "$ZSHRC" > "$installer"
assert_succeeds 'the alias block was extracted' test -s "$installer"

EXPECTED_ALIASES='co br cm st p pl lg pr change-commits'

run_installer() {
    HOME="$fixture_home" GIT_CONFIG_GLOBAL="$fixture_home/.gitconfig" \
        zsh -c ". '$installer'" 2>&1
}

alias_value() {
    HOME="$fixture_home" GIT_CONFIG_GLOBAL="$fixture_home/.gitconfig" \
        git config --global --get "alias.$1" 2>/dev/null
}

rm -f "$fixture_home/.gitconfig"
run_installer >/dev/null

missing=''
for name in $EXPECTED_ALIASES; do
    [ -n "$(alias_value "$name")" ] || missing="$missing $name"
done
assert_equals 'a fresh gitconfig gets every alias installed' '' "$missing"

assert_equals 'co is checkout' 'checkout' "$(alias_value co)"
assert_equals 'lg keeps its arguments' 'log --oneline' "$(alias_value lg)"

# --- a second run changes nothing -------------------------------------------

before=$(cat "$fixture_home/.gitconfig")
run_installer >/dev/null
after=$(cat "$fixture_home/.gitconfig")
assert_equals 'a second run leaves the config byte-identical' "$before" "$after"

# --- a hand-edited alias survives -------------------------------------------

# The failure a sentinel guard can introduce: skip when everything is present,
# but reinstall everything when one is missing, clobbering a local edit.
HOME="$fixture_home" GIT_CONFIG_GLOBAL="$fixture_home/.gitconfig" \
    git config --global alias.co 'checkout --guess'
HOME="$fixture_home" GIT_CONFIG_GLOBAL="$fixture_home/.gitconfig" \
    git config --global --unset alias.st

run_installer >/dev/null

assert_equals 'the missing alias is reinstalled' 'status' "$(alias_value st)"
assert_equals 'the hand-edited alias is not overwritten' \
    'checkout --guess' "$(alias_value co)"

# --- the core settings ------------------------------------------------------

# core.editor and core.excludeFile are installed the same way, in their own
# read: they are a different config section, and one regexp over both would
# match every core.* the user has set for their own reasons.
core_value() {
    HOME="$fixture_home" GIT_CONFIG_GLOBAL="$fixture_home/.gitconfig" \
        git config --global --get "core.$1" 2>/dev/null
}

rm -f "$fixture_home/.gitconfig"
run_installer >/dev/null

assert_equals 'core.editor is installed' 'nvim' "$(core_value editor)"
assert_succeeds 'core.excludeFile is installed' test -n "$(core_value excludeFile)"

# git lowercases a key when it reports it, so core.excludeFile reads back as
# core.excludefile. A matcher that compares against the camel-cased name never
# finds it, and every startup rewrites the setting it just found.
#
# Comparing the file before and after cannot see this: rewriting a value with
# the same value leaves the bytes identical. The write has to be counted, so
# git is stubbed and its invocations logged. This is the assertion that would
# have stayed green while the shell paid a subprocess on every startup.
git_stub_dir="$FIXTURES/git-stub"
mkdir -p "$git_stub_dir"
cat > "$git_stub_dir/git" <<'STUB'
#!/bin/sh
printf '%s\n' "$*" >> "$GIT_CALL_LOG"
exec /usr/bin/env -u PATH_STUB "$REAL_GIT" "$@"
STUB
chmod 755 "$git_stub_dir/git"

REAL_GIT=$(command -v git)
export REAL_GIT
export GIT_CALL_LOG="$FIXTURES/git-calls"
: > "$GIT_CALL_LOG"
HOME="$fixture_home" GIT_CONFIG_GLOBAL="$fixture_home/.gitconfig" \
    PATH="$git_stub_dir:$PATH" zsh -c ". '$installer'" >/dev/null 2>&1

writes=$(grep -c 'config --global alias\.\|config --global core\.' "$GIT_CALL_LOG" || true)
assert_equals 'a fully-installed config triggers no git write at startup' \
    '0' "$writes"

reads=$(grep -c -- '--get-regexp' "$GIT_CALL_LOG" || true)
assert_equals 'the whole block costs two git reads' '2' "$reads"

total=$(grep -c '' "$GIT_CALL_LOG" || true)
assert_equals 'and no other git call' '2' "$total"

# --- the installed set is one place -----------------------------------------

# The names and their values must be listed once. Two copies -- a sentinel
# list and an installer list -- is how an alias gets added to one and not the
# other, and the block then reports itself complete while an alias is missing.
value_lines=$(grep -c "git config --global alias\." "$ZSHRC" || true)
[ "$value_lines" -le 8 ] && single_source=yes || single_source="no ($value_lines lines)"
assert_equals 'each alias value appears once' 'yes' "$single_source"

finish
