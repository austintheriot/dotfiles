#!/bin/bash
#
# Gates the Lua under .config/nvim: formatting with stylua, correctness with
# selene.
#
# WHY THIS EXISTS. Nothing checked the Lua at all.
# crates/config-cli/tests/shellcheck_lint.rs covers 25+ shell scripts, and
# the ~20 Lua files had no equivalent, so the
# only feedback on a Lua mistake was nvim failing at runtime -- which is how
# the NvimTree and mason-version regressions both shipped.
#
# This is the cheapest half of that gap and it costs nothing to run: stylua
# is already a tracked tool (it is in mason's ensure_installed and pinned in
# mason-lock.json), and .config/nvim/.stylua.toml already states the settings.
# The only thing missing was something that fails when the files drift from
# them.
#
# FORMATTING ONLY, deliberately. A formatter cannot catch a wrong API call,
# and the two regressions above were both wrong API calls. The lint half
# (luacheck or selene, which know the `vim` global) is a separate and larger
# change; this one is a ratchet that keeps the diff noise down so the real
# review can see the logic.
#
# SKIPS WHEN stylua IS ABSENT, and that is a narrow exception rather than a
# habit: stylua arrives through mason on first nvim launch, so a machine that
# has bootstrapped but never opened nvim legitimately lacks it. The CI legs
# that install it are where this has teeth, and the assertion below fails
# rather than skips once the binary exists.
#
# Usage: ~/tests/nvim-lua-format.test.sh

. "$(dirname "$0")/lib.sh"

NVIM_DIR="$DOTFILES_ROOT/.config/nvim"
STYLUA_CONFIG="$NVIM_DIR/.stylua.toml"

assert_succeeds 'the nvim config directory exists' test -d "$NVIM_DIR"
assert_succeeds 'the repo states its own stylua settings' test -f "$STYLUA_CONFIG"

# Mason's bin directory is not on a non-login PATH, which is the same reason
# the deps engine prepends it: a tool installed by this repo has to be
# findable by this repo's own tests.
stylua_bin=$(command -v stylua 2>/dev/null \
    || printf '%s' "$HOME/.local/share/nvim/mason/bin/stylua")

if [ ! -x "$stylua_bin" ]; then
    skip 'stylua is not installed: it arrives through mason on first nvim launch'
else
    # `--check` exits non-zero and prints a unified diff per offending file.
    # The output is captured so a failure names the files rather than only
    # the exit code.
    format_output=$("$stylua_bin" --check "$NVIM_DIR" 2>&1)
    offenders=$(printf '%s\n' "$format_output" | grep -oE '^Diff in .*' | sed 's/^Diff in //' | sort -u)

    assert_equals 'every Lua file matches the repo stylua settings' '' "$offenders"

    # Positive control. `stylua --check` on a directory with no Lua files
    # exits 0 and prints nothing, which is indistinguishable from success, so
    # assert it actually saw files.
    lua_count=$(find "$NVIM_DIR" -name '*.lua' -type f | grep -c . || true)
    assert_succeeds 'the check covered a non-trivial number of Lua files' \
        test "$lua_count" -gt 10
fi

# --- selene: the correctness half --------------------------------------------
#
# stylua above cannot catch a wrong API call, and both regressions that
# shipped this week were wrong API calls. selene can: given a file with an
# unused local and an undefined call it reports both.
#
# THE PART THAT TOOK RESEARCH. selene ships standard libraries for plain Lua
# and Roblox and NOTHING for Neovim -- the upstream request for an equivalent
# of luacheck's `globals vim` is still open. Without a `vim` declaration it
# reports "`vim` is not defined" on the first real line of every file, which
# is a gate that gets muted within a day. .config/nvim/vim.yml supplies it,
# in the shape mason.nvim and plenary.nvim converged on.
SELENE_CONFIG="$NVIM_DIR/selene.toml"
SELENE_STD="$NVIM_DIR/vim.yml"

assert_succeeds 'the repo states its own selene settings' test -f "$SELENE_CONFIG"
assert_succeeds 'the vim standard library ships here' test -f "$SELENE_STD"

# The std chain is the load-bearing line: `lua51+vim` resolves `vim` as a
# file beside selene.toml. A bare `lua51` would report every vim call.
assert_succeeds 'selene chains the vim standard library' \
    grep -qE '^std = "lua5[0-9]\+vim"' "$SELENE_CONFIG"

selene_bin=$(command -v selene 2>/dev/null \
    || printf '%s' "$HOME/.local/share/nvim/mason/bin/selene")

if [ ! -x "$selene_bin" ]; then
    skip 'selene is not installed: it arrives through mason on first nvim launch'
else
    # Run from the config directory: selene resolves `+vim` relative to the
    # working directory, not to the file being linted.
    lint_output=$(cd "$NVIM_DIR" && "$selene_bin" . 2>&1)
    lint_errors=$(printf '%s\n' "$lint_output" | grep -cE '^error\[' || true)

    assert_equals 'selene reports no errors' '0' "$lint_errors"

    # Positive control. selene on a directory it cannot read exits 0 with no
    # findings, which is indistinguishable from a clean run, so assert it
    # actually produced a report.
    assert_succeeds 'selene produced a report' \
        test -n "$(printf '%s\n' "$lint_output" | grep -E 'Results:' || true)"

    # And that `vim` is genuinely declared: a missing vim.yml would surface
    # as undefined_variable errors, which the assertion above catches, but
    # this names the cause rather than the count.
    assert_equals 'the vim global is not reported undefined' '' \
        "$(printf '%s\n' "$lint_output" | grep -oE '`vim` is not defined' | head -1)"
fi
