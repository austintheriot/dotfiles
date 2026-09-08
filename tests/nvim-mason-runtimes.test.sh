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

# Anchored to the server table's OWN indentation (8 spaces), not to `^ *`.
# The loose pattern matched every `key = ...` line at any depth, so
# `rust_analyzer`'s nested `settings = {` (lsp.lua:78 and :91) was extracted
# as if it were an LSP server name. That put a non-server into the list every
# assertion below reads, and the count guard did not notice because the list
# was too long rather than too short.
#
# Harmless in effect until now only because the npm-backed list below is
# hardcoded and `settings` is not in it. A server actually named for a
# config key would have been silently cross-checked against the manifest.
servers=$(sed -n '/local servers = {/,/^      }$/p' "$LSP_CONFIG" \
    | sed -n 's/^        \([a-z_][a-z_0-9]*\) = .*/\1/p' | sort -u)
assert_succeeds 'the servers table parses' test -n "$servers"

# The parse must not pick up nested table keys. `settings` is the one this
# suite actually got wrong; it is asserted by name because a generic
# "no unexpected names" rule would need its own list of expected names and
# would drift the moment a server is added.
assert_equals 'the parse takes server names only, not nested config keys' '' \
    "$(printf '%s\n' "$servers" | grep -x 'settings' || true)"

extra_tools=$(sed -n 's/.*ensure_installed = .*vim\.tbl_keys(servers), {\(.*\)}.*/\1/p' "$LSP_CONFIG" \
    | tr -d " '" | tr ',' '\n' | grep -v '^$' | sort -u)
assert_succeeds 'the extra tool list parses' test -n "$extra_tools"

# A count guard on the parse above. `-gt 1` rather than an exact number, so
# adding a server is not a test failure, but a parse that collapses to one
# entry or none is.
#
# WHAT THIS GUARD DOES NOT CATCH, stated because it nearly hid a real defect.
# A PARTIAL collapse passes: if an indentation change dropped the range
# anchor and 10 servers parsed down to 2, every assertion below would run
# against a subset and report success. The guard proves the parse produced
# something, not that it produced everything. The `settings` assertion above
# is the other half -- it proves the parse is not over-matching -- and
# together they bound the parse from both sides.
server_count=$(printf '%s\n' "$servers" | grep -c .)
assert_succeeds 'the servers table has more than one entry' test "$server_count" -gt 1

# An INDEPENDENT derivation, deliberately not sharing the sed range above.
#
# A first version of this guard counted the same `sed -n '/start/,/end/p'`
# range and compared the two counts. It was useless, and the sabotage that
# proved it is worth recording: inserting a line matching the range
# terminator before `cssls` truncated the table from 10 servers to 5, and
# all 14 assertions passed -- because BOTH sides read the same truncated
# range and agreed with each other while both were wrong. Two derivations
# that share their failure mode are one derivation.
#
# So this counts server-shaped lines across the WHOLE file and requires the
# ranged parse to find every one of them. A range that stops early now
# leaves the two numbers different.
#
# NOT_A_SERVER is the cost of reading the whole file: `handlers` (lsp.lua:102)
# sits at the same 8-space depth outside the servers table, and this guard
# failed on the healthy config until it was excluded. That is the intended
# behaviour -- a new key at that depth breaks the build and gets classified
# by a human, rather than silently joining the server list the way `settings`
# did.
NOT_A_SERVER='handlers'
declared_count=$(grep -E '^        [a-z_][a-z_0-9]* = (\{|nil|true|false)' "$LSP_CONFIG" \
    | sed -n 's/^        \([a-z_][a-z_0-9]*\) = .*/\1/p' \
    | grep -vxF "$NOT_A_SERVER" | sort -u | grep -c .)
assert_equals 'every server declared in the file survives the ranged parse' \
    "$declared_count" "$server_count"

# --- no plugin build hook depends on lazy.nvim being set up -------------
#
# THE BUG THIS CATCHES, reported 2026-09-08 from a bare ubuntu container:
#
#   markdown-preview.nvim ... build failed
#   Vim:E492: Not an editor command: Lazy load markdown-preview.nvim
#
# `:Lazy` is a USER COMMAND that lazy.nvim creates during its own setup. A
# `build` hook running during the first bootstrap sync can execute before
# that command exists, so the plugin never builds on a fresh machine. It is
# silent afterwards: nothing re-runs the hook, so the missing artifact only
# surfaces when the feature is first used, possibly months later. Confirmed
# not container-specific -- the build artifact is absent on the mac too.
#
# lazy.nvim already loads a plugin before running its build hook, so the
# command was never needed.
#
# Lua comments are stripped before matching. Both specs now EXPLAIN this bug
# in a comment above the build hook, and a raw grep read that prose as the
# violation it was describing -- the same trap the alacritty and workflow
# suites already document.
plugin_code=$(cat "$NVIM_LUA"/plugins/*.lua 2>/dev/null | sed -e 's/--.*//')
assert_succeeds 'the plugin specs were read' test -n "$plugin_code"
assert_equals 'no build hook invokes the :Lazy user command' '' \
    "$(printf '%s\n' "$plugin_code" | grep -n 'vim\.cmd.*Lazy ' || true)"

# The interactive-terminal half of the same defect. markdown-preview's
# `mkdp#util#install()` with no argument routes through
# `mkdp#util#open_terminal` and opens a terminal split, which cannot work in
# a headless or non-interactive bootstrap. Upstream ships
# `mkdp#util#install_sync()` for exactly that case.
#
# Asserted as "if the install function is called at all, it is the sync
# variant", so removing the plugin does not leave an assertion that passes
# by finding nothing.
mkdp_calls=$(printf '%s\n' "$plugin_code" | grep -n "mkdp#util#install" || true)
if [ -n "$mkdp_calls" ]; then
    assert_equals 'the markdown-preview install call is the sync variant' '' \
        "$(printf '%s\n' "$mkdp_calls" | grep -v 'install_sync' || true)"
fi

# --- every linter the config invokes actually gets installed ------------
#
# THE GAP THIS CLOSES, found 2026-09-08 on a freshly bootstrapped container.
# Opening any file printed:
#
#   Error running cspell: ENOENT: no such file or directory
#
# nvim-lint registers cspell with `cmd = 'cspell'` and wires it into
# `linters_by_ft` for 26 filetypes (lint.lua), so it runs on almost every
# buffer. Nothing installs it: it is absent from `ensure_installed`, absent
# from the deps manifests, and therefore absent on every fresh machine.
#
# This suite already existed to catch exactly this shape -- a tool the config
# invokes that no install path provides -- and missed it, because it only ever
# read the mason `ensure_installed` list. A linter registered directly with
# nvim-lint never appears there.
LINT_CONFIG="$NVIM_LUA/plugins/lint.lua"
assert_succeeds 'the lint config exists' test -f "$LINT_CONFIG"

# Every `cmd = '<name>'` nvim-lint is given. That is the exact string it
# execs, so it is the thing that must exist on PATH.
#
# Matched ANYWHERE on the line, not anchored to the line start. A first
# version required `^ *cmd = '...'` and missed a linter declared inline
# (`lint.linters.foo = { cmd = 'foo' }`), which is valid Lua and the shape a
# one-line linter naturally takes. Verified by sabotage: the anchored pattern
# reported 17 passing assertions with an unprovided linter in the config.
lint_commands=$(sed -e 's/--.*//' "$LINT_CONFIG" \
    | grep -oE "cmd = '[a-z][a-z0-9-]*'" \
    | sed -e "s/cmd = '//" -e "s/'//" | sort -u)
assert_succeeds 'the linter commands parse' test -n "$lint_commands"

# The parse must find every linter the config registers, not a subset. A
# `lint.linters.<name> =` assignment is the independent derivation: it does
# not share the `cmd =` pattern above, so a linter added in a shape the
# command parse cannot read leaves the two counts different.
registered_count=$(sed -e 's/--.*//' "$LINT_CONFIG" \
    | grep -cE 'lint\.linters\.[a-z][a-z0-9_-]* *=')
command_count=$(printf '%s\n' "$lint_commands" | grep -c .)
assert_equals 'every registered linter contributes a parsed command' \
    "$registered_count" "$command_count"

# Each one must be provided by mason's ensure_installed or by the manifests.
# Named per tool rather than as one pass/fail, so a failure says WHICH tool
# has no install path.
for tool in $lint_commands; do
    if printf '%s\n%s\n' "$servers" "$extra_tools" | grep -qx "$tool"; then
        provided="mason ensure_installed"
    elif tracks "$tool"; then
        provided="a deps manifest"
    else
        provided="NOTHING -- add it to ensure_installed or to a deps manifest"
    fi
    assert_succeeds "the '$tool' linter has an install path ($provided)" \
        test "$provided" != "NOTHING -- add it to ensure_installed or to a deps manifest"
done

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
