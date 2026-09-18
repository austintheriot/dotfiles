-- Lean 4 support: the language server, the infoview, and unicode
-- abbreviation expansion.
--
-- THIS PLUGIN OWNS ITS OWN LSP, which is why `leanls` appears nowhere in
-- lsp.lua. lean.nvim ships `lsp/leanls.lua` at its root, which is the 0.11+
-- `vim.lsp.config` runtimepath convention, and its plugin/lean.lua calls
-- `vim.lsp.enable 'leanls'` itself. nvim-lspconfig has no leanls at all
-- (only the Lean 3 `lean3ls`), so the mason-lspconfig handler loop in
-- lsp.lua cannot configure it and must not try: that loop passes its own
-- `capabilities` table, which would overwrite lean.nvim's
-- `capabilities.lean.silentDiagnosticSupport`, its `$/lean/fileProgress`
-- handler, its publishDiagnostics override and `init_options.hasWidgets`.
-- The result is not a clean failure -- the server attaches and diagnostics
-- appear while the infoview stays empty, which is far harder to diagnose.
--
-- THE SERVER IS NOT A MASON PACKAGE, and this is the one place this config
-- cannot offer the guarantee it offers everywhere else. Stated plainly
-- rather than papered over:
--
--   mason-registry DOES contain `lean-language-server`, and it is a trap.
--   Its package.yaml reads `languages: [Lean 3]`, `pkg:npm/lean-language-server@3.4.0`.
--   Lean 3 is a different, end-of-life language with an incompatible
--   protocol. Installing it for Lean 4 is worse than installing nothing,
--   so a Rust assertion in crates/config-cli/tests/nvim_lean.rs keeps it
--   out of mason-lock.json permanently.
--
--   The real server is `lake serve` (when the root has a lakefile) or
--   `lean --server` (otherwise: the core lean4 repo, a standalone file, or a
--   directory with a lean-toolchain and no lakefile, since lean-toolchain is
--   itself a root marker). Both are elan SHIMS on $PATH. elan picks the
--   toolchain from the CWD, which is why lean.nvim sets cmd_cwd to the
--   project root; the path argument does not affect the choice.
--
-- So the version is pinned PER PROJECT, by the project, and this repo
-- deliberately does not pin it. elan is tracked in deps/deps.toml so the
-- shims exist; what they resolve to is the Lean project's business.
--
-- THE REASON IS OWNERSHIP, not artifacts. lean-toolchain is checked into the
-- project's source, so a globally pinned Lean would override a decision the
-- project already made, and on a machine with two projects on different
-- toolchains at most one could be right. The cost is not a one-time rebuild:
-- .olean files are not compatible across versions, so a pinned editor server
-- and a `lake build` from a shell would each invalidate the other's output
-- indefinitely.
--
-- WITHOUT elan the failure is quiet. `vim.lsp.rpc.start` is handed a `lake`
-- that does not resolve, so the client never attaches and the infoview
-- simply stays empty with no message in the UI. `:checkhealth dotfiles`
-- names elan for that reason.
local supported = require 'dotfiles.lean_supported'

-- A HIGHER FLOOR THAN THE REST OF THIS CONFIG. init.lua rejects below 0.10
-- (where vim.uv arrived); lean.nvim's own MIN_SUPPORTED_NVIM is 0.11.5, and
-- it only WARNS below that before continuing into API calls that do not
-- exist yet. Declining to load is the honest outcome: `:checkhealth
-- dotfiles` reports the version, rather than a warning scrolling past
-- startup and an unrelated traceback arriving later.
if not supported.is_supported(tostring(vim.version())) then
  return {}
end

return {
  {
    'Julian/lean.nvim',
    -- Sourcing plugin/ is REQUIRED, not incidental. plugin/lean.lua is the
    -- one line that calls `vim.lsp.enable 'leanls'`, so a lazy-load that
    -- skips it yields a plugin that looks loaded and whose server never
    -- starts. lean.nvim ships a health check specifically for this
    -- ("lean.nvim's plugin files have not run"), which is a fair measure of
    -- how often it happens. These two events are the plugin's own
    -- documented spec and do source plugin/.
    event = { 'BufReadPre *.lean', 'BufNewFile *.lean' },

    -- No dependencies. The plugin's rockspec declares `dependencies = {}`,
    -- and the `require 'std.*'` calls throughout its source resolve to
    -- lua/std/ VENDORED INSIDE the plugin, not to plenary. A stale example
    -- in its own .devcontainer still lists plenary; the rockspec and the
    -- current source do not. telescope, vim-matchup, switch.vim and
    -- tcomment are all genuinely optional and are commented out in its
    -- README's own lazy spec.

    -- `vim.g.lean_config`, NOT `opts`. lazy.nvim's `opts` key calls
    -- `require('lean').setup()`, which is marked ---@deprecated in
    -- lua/lean/init.lua with a removal target of v2026.9.1 -- the next
    -- version after the current v2026.4.1. The config table is read lazily
    -- at first use, so setting it here is early enough.
    -- `dependencyBuildMode` IS LEFT ALONE, deliberately. lean.nvim defaults
    -- didOpen to 'never' and uses 'once' only for an explicit restart, which
    -- is the same split VS Code makes: opening a file must not silently start
    -- a build that takes hours on a Mathlib project. Unbuilt imports instead
    -- fail fast with "Imports are out of date", and lean.nvim rewrites that
    -- message and offers a vim.ui.select prompt to rebuild.
    --
    -- DISMISSING THAT PROMPT IS A DEAD END unless you know the way out: the
    -- buffer keeps no semantic information and the prompt does not return.
    -- `:LeanRestartFile` is the recovery, and it is written here because
    -- nothing in the UI names it at the moment it is needed.
    init = function()
      vim.g.lean_config = {
        mappings = true,
        -- Left at the default (true). lean.nvim replaces vim.diagnostic's
        -- signs for its own namespace so it can draw multi-line guides;
        -- nothing else in this config renders diagnostic signs, so there is
        -- no conflict to resolve.
      }
    end,
  },
}
