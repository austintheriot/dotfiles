#!/bin/bash
#
# Tests that a fresh bootstrap can actually install the neovim tooling.
#
# The failure this exists to catch, observed on a real first run:
#
#   eslint-lsp: failed to install
#   css-variables-language-server: failed to install
#   typescript-language-server: failed to install
#   stylua: failed to install
#
# Mason does not build those itself. It shells out to whatever the package's
# registry entry names -- npm for a `pkg:npm/...` entry, a release download for
# a `pkg:github/...` one. When the runtime is missing, every package that needs
# it fails at once, which is why the errors arrive as a block rather than
# singly.
#
# The bootstrap tracks `nvm`, and `nvm` alone satisfies nothing: the check is
# `[ -s "$HOME/.nvm/nvm.sh" ]`, which passes with no node version installed at
# all. So a freshly bootstrapped machine has nvm, no npm, and every npm-backed
# Mason package fails on first launch.
#
# What is asserted here is the CONTRACT rather than a live install:
#
#   1. Every runtime the ensure_installed list needs is a tracked dependency,
#      so `config install` puts it on the machine.
#   2. The nvim health check names those runtimes, so `:checkhealth` says
#      which one is missing instead of leaving the reader with four identical
#      "failed to install" lines and no cause.
#
# Running Mason for real is deliberately NOT what this does. That needs the
# network, a writable share directory, and minutes of wall clock, and it would
# fail for reasons that have nothing to do with this repo. The contract is the
# part this repo controls.
#
# Usage: ~/tests/nvim-mason-runtimes.test.sh

. "$(dirname "$0")/lib.sh"

LSP_CONFIG="$DOTFILES_ROOT/.config/nvim/lua/plugins/lsp.lua"
NVIM_LUA="$DOTFILES_ROOT/.config/nvim/lua"

# Neovim discovers health modules by globbing `lua/**/health.lua` on the
# runtimepath and naming each one after its PARENT directory, which is what
# `:checkhealth <name>` takes. A module at `lua/health.lua` has no parent
# directory inside lua/, so it has no name and is never run -- and it fails
# silently, because a health check that is never invoked reports nothing at
# all. That is what this repo shipped: init.lua did `require "health"`, which
# returns the table and registers nothing.
HEALTH=$(find "$NVIM_LUA" -mindepth 2 -name health.lua 2>/dev/null | head -1)
DEPS_DIR="$DOTFILES_ROOT/deps"

assert_succeeds 'the lsp plugin config exists' test -f "$LSP_CONFIG"
assert_succeeds 'the health module is under a named directory, so checkhealth can find it' \
    test -n "$HEALTH"
assert_equals 'no health module sits directly in lua/, where it would never run' \
    '' "$(ls "$NVIM_LUA/health.lua" 2>/dev/null)"

# --- the ensure_installed list is readable ------------------------------
#
# Everything below reads through this list rather than restating it, so a
# server added to lsp.lua is covered without editing this suite. Parsing it
# is therefore load-bearing: if the shape of the config changes and this
# silently extracts nothing, every assertion below passes vacuously.

servers=$(sed -n '/local servers = {/,/^      }$/p' "$LSP_CONFIG" \
    | sed -n 's/^ *\([a-z_]*\) = .*/\1/p' | sort -u)
assert_succeeds 'the servers table parses' test -n "$servers"

extra_tools=$(sed -n 's/.*ensure_installed = .*vim\.tbl_keys(servers), {\(.*\)}.*/\1/p' "$LSP_CONFIG" \
    | tr -d " '" | tr ',' '\n' | grep -v '^$' | sort -u)
assert_succeeds 'the extra tool list parses' test -n "$extra_tools"

# A count guard on the parse above. `-gt 1` rather than an exact number, so
# adding a server is not a test failure, but a parse that collapses to one
# entry or none is.
server_count=$(printf '%s\n' "$servers" | grep -c .)
assert_succeeds 'the servers table has more than one entry' test "$server_count" -gt 1

# --- every runtime those tools need is tracked --------------------------
#
# npm is the one that actually broke. Mason installs eslint-lsp, ts_ls,
# css_variables and markdownlint from npm, so a machine with no npm fails all
# of them together.
#
# `nvm` being tracked is NOT the same as npm being available: nvm's check is
# satisfied by nvm.sh existing, which says nothing about whether a node
# version was ever installed through it. The dependency that has to exist is
# node itself.
conf_names=$(cat "$DEPS_DIR"/deps.toml "$DEPS_DIR"/deps-mac.toml "$DEPS_DIR"/deps-linux.toml 2>/dev/null \
    | sed -e 's/#.*//' | grep -oE '^\[[a-z][a-z0-9-]*\]' | tr -d '[]' | sort -u)
assert_succeeds 'the dependency manifests parse' test -n "$conf_names"

tracks() { printf '%s\n' "$conf_names" | grep -qx "$1"; }

# The npm-backed entries in the ensure_installed list. Named here because the
# mapping from a server name to its registry backing is Mason's, not this
# repo's, and reading it at test time would need Mason installed.
NPM_BACKED='eslint ts_ls css_variables cssls cssmodules_ls svelte astro markdownlint'

npm_needed=''
for tool in $NPM_BACKED; do
    printf '%s\n%s\n' "$servers" "$extra_tools" | grep -qx "$tool" \
        && npm_needed="$npm_needed $tool"
done
assert_succeeds 'the config really does request npm-backed tools' test -n "$npm_needed"

tracks node && node_tracked=yes \
    || node_tracked="no (npm-backed tools needed:$npm_needed)"
assert_equals 'node is a tracked dependency, so npm-backed Mason tools can install' \
    'yes' "$node_tracked"

# --- the health check names the runtimes --------------------------------
#
# Without this, a missing runtime surfaces only as N identical "failed to
# install" lines with no stated cause, which is the experience that produced
# this suite. `:checkhealth` is where a reader goes to find out why.
# Matched as a table key or a list entry rather than as any occurrence of the
# word, so prose mentioning node in a message does not satisfy the assertion.
# The claim being tested is that the check RUNS on that executable.
health_text=$(cat "$HEALTH")
for runtime in node npm; do
    case $health_text in
        *"$runtime = "*|*"'$runtime'"*) named=yes ;;
        *) named="no" ;;
    esac
    assert_equals "the health check tests for $runtime" 'yes' "$named"
done

# The remedy has to be in the message. "Not found: node" tells a reader what
# is missing and not what to do about it, and the install path here is not
# guessable: node comes from nvm, not from the package manager.
assert_contains 'the health check names the install remedy' \
    'nvm install' "$health_text"

finish
