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

      -- The highlight QUERIES, without which treesitter reports itself
      -- active and captures nothing. On `main` they live in `runtime/queries`
      -- rather than the top-level `queries/` that `master` shipped, and
      -- lazy.nvim puts only a plugin's TOP LEVEL on the runtimepath. So
      -- nvim_get_runtime_file('queries/typescript/highlights.scm') found
      -- exactly one file: vscode.nvim's `after/queries` overlay, which
      -- REFINES a base query rather than standing in for one.
      --
      -- The symptom is not an absence of colour, which is what makes it hard
      -- to recognise. Strings, comments and types still resolve through the
      -- overlay and the LSP, so a buffer looks nearly right while `const`,
      -- `await`, `export` and `return` render as plain foreground text.
      -- Measured: 0 captures before this line, 47 after, on the same buffer.
      vim.opt.runtimepath:append(vim.fn.stdpath 'data' .. '/lazy/nvim-treesitter/runtime')

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
          if not lang then
            return
          end

          -- A PARSER FILE MUST EXIST, and checking that is not the same as
          -- calling language.add. get_lang returns the FILETYPE ITSELF when
          -- nothing maps it, and language.add then succeeds for that name
          -- because it registers a language rather than loading a parser.
          -- vim.treesitter.start is the call that finally throws:
          --
          --   Parser could not be created for buffer 1 and language "NvimTree"
          --
          -- Reported from a real session opening the file explorer, whose
          -- buffer has filetype NvimTree. Every plugin buffer is this shape.
          local has_parser = function(name)
            return #vim.api.nvim_get_runtime_file('parser/' .. name .. '.so', false) > 0
          end
          if not require('dotfiles.treesitter_start').should_start(lang, has_parser) then
            return
          end

          -- Still guarded: a parser that exists can be ABI-stale against the
          -- running Neovim, and that failure arrives from start() rather
          -- than from the file check above.
          if not pcall(vim.treesitter.language.add, lang) then
            return
          end
          if not pcall(vim.treesitter.start, args.buf, lang) then
            return
          end

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
