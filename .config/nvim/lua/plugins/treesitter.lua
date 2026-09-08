-- Parsers this config installs. A plain list, read by both the build step and
-- the FileType handler below, so the two cannot disagree about the set.
local parsers = {
  'bash',
  'c',
  'css',
  'diff',
  'html',
  'javascript',
  'json',
  'lua',
  'luadoc',
  'markdown',
  'markdown_inline',
  'query',
  'rust',
  'svelte',
  'toml',
  'tsx',
  'typescript',
  'vim',
  'vimdoc',
}

return {
  {
    'nvim-treesitter/nvim-treesitter',
    branch = 'main',
    -- WRITTEN FOR `main`, WHICH IS WHAT WE PIN. This spec used to pass
    -- master-branch options -- ensure_installed, auto_install, highlight,
    -- indent, incremental_selection -- to a plugin pinned on `main`. On
    -- `main`, `require('nvim-treesitter').setup()` forwards to
    -- nvim-treesitter.config, whose default table accepts ONLY `install_dir`,
    -- so every other key was absorbed and ignored with no error and no
    -- warning.
    --
    -- Measured before the fix: 1 of 19 declared parsers installed, and
    -- treesitter highlighting never started on a .ts buffer. The parser that
    -- did exist worked when driven by hand, so nothing looked broken except
    -- the absence of colour.
    --
    -- Staying on `main` is deliberate. Its lua/nvim-treesitter/parsers.lua
    -- ships an exact `revision` SHA per parser (318 parsers; all of the ones
    -- above are present), so pinning the plugin commit in lazy-lock.json
    -- transitively pins every parser SOURCE revision. `master` is in
    -- maintenance and its lockfile moves on a looser cadence.
    --
    -- `install` rather than `:TSUpdate`: on `main`, update only refreshes
    -- parsers that are ALREADY installed, so a fresh machine converges to
    -- nothing. The wait is bounded because a build that returns before the
    -- compile finishes reports success on an empty parser directory.
    build = function()
      require('nvim-treesitter.install').install(parsers, { summary = true }):wait(600000)
    end,
    config = function()
      require('nvim-treesitter').setup()

      -- `main` ships no FileType handler, so starting the highlighter is
      -- config work now. Both of the guards below are load-bearing:
      --
      --   get_lang maps a filetype to a parser name, which are not always
      --   equal (a `.ts` file is filetype `typescript`, and several
      --   filetypes share one parser).
      --
      --   pcall around language.add means a parser that is missing, or
      --   compiled against a different ABI, degrades to regex highlighting
      --   instead of raising on every single buffer open.
      vim.api.nvim_create_autocmd('FileType', {
        group = vim.api.nvim_create_augroup('dotfiles_treesitter', { clear = true }),
        callback = function(args)
          local lang = vim.treesitter.language.get_lang(args.match)
          if not lang or not pcall(vim.treesitter.language.add, lang) then
            return
          end

          vim.treesitter.start(args.buf, lang)

          -- Indent was `indent = { enable = true, disable = { 'ruby' } }`
          -- under master. On main the expression is ours to set, and ruby's
          -- treesitter indent is still the known-bad one that exclusion
          -- existed for.
          if args.match ~= 'ruby' then
            vim.bo[args.buf].indentexpr = "v:lua.require'nvim-treesitter'.indentexpr()"
          end
        end,
      })
    end,
  },
}
