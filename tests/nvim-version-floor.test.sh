#!/bin/bash
#
# Tests that an old Neovim gets an explanation instead of a Lua traceback.
#
# The failure this exists to catch, observed on a real Pop!_OS machine after a
# clean bootstrap:
#
#   Error detected while processing /home/austin/.config/nvim/init.lua:
#   E5113: Error while calling lua chunk: .../init.lua:6: attempt to index
#   field 'uv' (a nil value)
#
# `vim.uv` arrived in Neovim 0.10. Before that the same libuv handle was
# `vim.loop`. Apt ships 0.6.1 on Pop!_OS 22.04 and 0.9.5 on 24.04, and the
# dependency manifest's check is `command -v nvim` -- a presence test with no
# version floor -- so both satisfy the bootstrap and then crash the editor.
#
# `lua/dotfiles/health.lua` already carried the correct floor check. It could
# never fire on the machines that needed it, for two independent reasons, and
# BOTH are asserted below because fixing either one alone leaves the crash:
#
#   1. init.lua indexed `vim.uv` at line 6, so startup died before
#      `:checkhealth` could ever be typed.
#   2. health.lua called `vim.uv.os_uname()` ABOVE its own version check, so
#      even a reachable `:checkhealth` crashed before reporting the cause.
#
# This is the same shape `nvim-mason-runtimes.test.sh` was written against: a
# health check that never runs reports nothing at all. A version guard placed
# below the code it guards is not a guard.
#
# Usage: ~/tests/nvim-version-floor.test.sh

. "$(dirname "$0")/lib.sh"

NVIM_DIR="$DOTFILES_ROOT/.config/nvim"
INIT="$NVIM_DIR/init.lua"
HEALTH="$NVIM_DIR/lua/dotfiles/health.lua"

assert_succeeds 'the nvim entrypoint exists' test -f "$INIT"
assert_succeeds 'the health module exists' test -f "$HEALTH"

# --- the guard runs before anything it guards ---------------------------
#
# Asserted as line ORDER rather than as the mere presence of a guard. A
# correct check sitting below the first 0.10+ call is exactly the bug this
# suite exists for, and a presence-only assertion passes on that bug.
#
# `vim.version` and `vim.notify` are the older API (both predate 0.7), so the
# guard itself runs on the versions it rejects.
#
# Two spellings are excluded from the search, and leaving either one in
# produced a false failure while this suite was being written:
#
#   - Lua comments. Both files now EXPLAIN the 0.10 floor in a comment above
#     the guard, so a raw grep for `vim.uv` matched prose and reported the
#     explanation as the violation it was describing.
#   - `vim.uv or vim.loop`, the compatibility spelling, anywhere on the line
#     rather than as the whole match. `(vim.uv or vim.loop).fs_stat(...)` is
#     the real call site in init.lua and is not a 0.10+ access.
first_modern_use() {
    sed 's/--.*//' "$1" | grep -n 'vim\.uv' | grep -v 'vim\.uv or vim\.loop' \
        | head -1 | cut -d: -f1
}
first_guard() {
    sed 's/--.*//' "$1" | grep -n 'vim\.version\.ge\|vim\.version()' \
        | head -1 | cut -d: -f1
}

for target in "$INIT" "$HEALTH"; do
    name=$(basename "$target")
    modern=$(first_modern_use "$target")
    guard=$(first_guard "$target")

    if [ -z "$modern" ]; then
        # No bare `vim.uv` at all is a valid way to satisfy this: the
        # compatibility spelling alone needs no ordering.
        skip "$name has no bare vim.uv access, so no ordering to check"
        continue
    fi

    assert_succeeds "$name has a version guard at all" test -n "$guard"
    [ -n "$guard" ] || continue
    assert_succeeds "$name guards the version before its first vim.uv use (guard line $guard, use line $modern)" \
        test "$guard" -lt "$modern"
done

# --- the guard reports through an API that exists on old versions -------
#
# `vim.health` is only reachable under `:checkhealth`, and a startup guard
# that used it would print nothing on the crash path this suite is about.
assert_contains 'init.lua reports the floor through vim.notify, which runs at startup' \
    'vim.notify' "$(cat "$INIT")"

# The message has to name the required version. "This config needs a newer
# Neovim" tells the reader nothing they can act on; the whole remedy here is
# knowing which version to get.
assert_contains 'the startup message names the required version' \
    '0.10' "$(cat "$INIT")"

# --- it actually behaves that way ---------------------------------------
#
# Everything above reads the source. This runs the real config with `vim.uv`
# removed, which is what an old Neovim looks like from Lua's side, and is the
# only assertion here that would catch a guard that reads correctly and
# throws anyway.
#
# The version is what gets faked, not `vim.uv`. Nilling the field alone leaves
# `vim.version()` reporting the real (modern) Neovim, so the guard correctly
# passes and the run proceeds into lazy.nvim -- which then clones and compiles
# the entire plugin set over the network. Faking both is what reproduces an
# old Neovim: the guard must trip on the version and return before anything
# touches the missing field.
if ! command -v nvim >/dev/null 2>&1; then
    skip 'nvim is not installed, so the live old-version simulation cannot run'
else
    # XDG_CONFIG_HOME rather than `-u "$INIT"`. `-u` loads the file without
    # putting the config's lua/ directory on the runtimepath, so the run dies
    # at line 1 on `require 'settings'` -- a rig artifact that reports a
    # failure having never reached the code under test.
    #
    # A scratch HOME keeps the run off the real plugin directory: lazy.nvim
    # would otherwise clone on a cold cache and make this suite need the
    # network.
    sim_home=$(mktemp -d)
    mkdir -p "$sim_home/.config"
    ln -s "$NVIM_DIR" "$sim_home/.config/nvim"
    output=$(HOME="$sim_home" XDG_CONFIG_HOME="$sim_home/.config" \
        nvim --headless \
        --cmd 'lua vim.uv = nil; local v = setmetatable({major=0,minor=9,patch=5}, {__tostring=function() return "0.9.5" end}); vim.version = setmetatable({ge=function() return false end}, {__call=function() return v end})' \
        -c 'q' 2>&1)
    rm -rf "$sim_home"

    case $output in
        *E5113*|*"attempt to index"*) crashed=yes ;;
        *) crashed=no ;;
    esac
    assert_equals 'a Neovim without vim.uv does not crash on the config' 'no' "$crashed"
    assert_contains 'a Neovim without vim.uv is told the required version' \
        '0.10' "$output"
fi

finish
