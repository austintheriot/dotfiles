return {
  {
    'folke/lazydev.nvim',
    ft = 'lua',
    opts = {
      library = {
        { path = 'luvit-meta/library', words = { 'vim%.uv' } },
      },
    },
  },
  { 'Bilal2453/luvit-meta', lazy = true },
  {
    'neovim/nvim-lspconfig',
    dependencies = {
      { 'williamboman/mason.nvim', config = true },
      'williamboman/mason-lspconfig.nvim',
      'WhoIsSethDaniel/mason-tool-installer.nvim',
      { 'j-hui/fidget.nvim', opts = {} },
      'hrsh7th/cmp-nvim-lsp',
    },
    config = function()
      vim.api.nvim_create_autocmd('LspAttach', {
        group = vim.api.nvim_create_augroup('lsp-attach', { clear = true }),
        callback = function(event)
          local map = function(keys, func, desc)
            vim.keymap.set('n', keys, func, { buffer = event.buf, desc = 'LSP: ' .. desc })
          end

          local tb = require 'telescope.builtin'
          map('gd', tb.lsp_definitions, 'Goto Definition')
          map('gr', tb.lsp_references, 'Goto References')
          map('gI', tb.lsp_implementations, 'Goto Implementation')
          map('<leader>D', tb.lsp_type_definitions, 'Type Definition')
          map('<leader>ds', tb.lsp_document_symbols, 'Document Symbols')
          map('<leader>ws', tb.lsp_dynamic_workspace_symbols, 'Workspace Symbols')
          map('<leader>rn', vim.lsp.buf.rename, 'Rename')
          map('<leader>ca', vim.lsp.buf.code_action, 'Code Action')
          map('gD', vim.lsp.buf.declaration, 'Goto Declaration')

          local client = vim.lsp.get_client_by_id(event.data.client_id)

          if client and client:supports_method(vim.lsp.protocol.Methods.textDocument_documentHighlight) then
            -- only enable document highlighting for smaller files (performance)
            if vim.fn.getfsize(vim.api.nvim_buf_get_name(event.buf)) < 50000 then
              local augroup = vim.api.nvim_create_augroup('lsp-highlight', { clear = false })
              vim.api.nvim_create_autocmd({ 'CursorHold', 'CursorHoldI' }, {
                buffer = event.buf,
                group = augroup,
                callback = vim.lsp.buf.document_highlight,
              })
              vim.api.nvim_create_autocmd({ 'CursorMoved', 'CursorMovedI' }, {
                buffer = event.buf,
                group = augroup,
                callback = vim.lsp.buf.clear_references,
              })
              vim.api.nvim_create_autocmd('LspDetach', {
                group = vim.api.nvim_create_augroup('lsp-detach', { clear = true }),
                callback = function(event2)
                  vim.lsp.buf.clear_references()
                  vim.api.nvim_clear_autocmds { group = 'lsp-highlight', buffer = event2.buf }
                end,
              })
            end
          end

          if client and client:supports_method(vim.lsp.protocol.Methods.textDocument_inlayHint) then
            map('<leader>th', function()
              vim.lsp.inlay_hint.enable(not vim.lsp.inlay_hint.is_enabled { bufnr = event.buf })
            end, 'Toggle Inlay Hints')
          end
        end,
      })

      local capabilities = vim.tbl_deep_extend('force', vim.lsp.protocol.make_client_capabilities(), require('cmp_nvim_lsp').default_capabilities())

      local servers = {
        rust_analyzer = {
          settings = {
            ['rust-analyzer'] = { cargo = { allFeatures = true } },
          },
        },
        eslint = {},
        taplo = {},
        ts_ls = {},
        css_variables = {},
        cssls = {},
        cssmodules_ls = {},
        svelte = {},
        astro = {},
        lua_ls = {
          settings = {
            Lua = { completion = { callSnippet = 'Replace' } },
          },
        },
      }

      -- Versions come from mason-lock.json, so two machines install the
      -- same tools rather than whatever the registry served that day.
      --
      -- The registry pin is the load-bearing half. Without it the same
      -- package NAME resolves to a different recipe over time, which is
      -- worse than an unpinned version: the resolution function changes
      -- with no diff anywhere. The registry release also ships checksums
      -- for its own payload, so pinning the version pins the bytes.
      --
      -- WHAT THIS DOES NOT PIN, stated so nobody reads more into it: the
      -- downloaded artifacts. Mason has no integrity-hash layer for package
      -- payloads, and an npm package's transitive tree floats under a
      -- pinned top-level version. This buys "the same tool versions on both
      -- machines", not byte-identity.
      local lock = vim.json.decode(table.concat(vim.fn.readfile(vim.fn.stdpath 'config' .. '/mason-lock.json'), '\n'))

      require('mason').setup { registries = { lock.registry } }

      -- mason-tool-installer takes `name@version` strings. The lock is
      -- keyed by MASON PACKAGE name, while `servers` is keyed by lspconfig
      -- name, so the two are joined through the registry's own mapping
      -- rather than a second hand-written table.
      local mason_names = require('mason-lspconfig.mappings').get_mason_map().lspconfig_to_package
      -- The join lives in lua/dotfiles/mason_ensure.lua, where it is a pure
      -- function with its own spec, including the rule that a pinned tool is a
      -- { name, version = ... } TABLE and never a `name@version` string.
      local ensure = require('dotfiles.mason_ensure').ensure_list(
        vim.list_extend(vim.tbl_keys(servers), { 'stylua', 'markdownlint', 'cspell', 'prettier', 'prettierd', 'selene' }),
        mason_names,
        lock.packages
      )

      require('mason-tool-installer').setup {
        ensure_installed = ensure,
        -- Drift-by-default is the enemy of a lockfile.
        auto_update = false,
      }
      require('mason-lspconfig').setup {
        handlers = {
          function(server_name)
            local server = servers[server_name] or {}
            server.capabilities = vim.tbl_deep_extend('force', {}, capabilities, server.capabilities or {})
            server.timeout_ms = 3000
            require('lspconfig')[server_name].setup(server)
          end,
        },
      }
    end,
  },
}
