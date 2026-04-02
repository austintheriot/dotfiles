return {
  -- lazydev improves Lua LSP completions specifically for your nvim config
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
      -- mason installs language servers automatically
      { 'williamboman/mason.nvim', config = true },
      'williamboman/mason-lspconfig.nvim',
      'WhoIsSethDaniel/mason-tool-installer.nvim',
      -- fidget shows LSP loading progress in the bottom right
      { 'j-hui/fidget.nvim', opts = {} },
      'hrsh7th/cmp-nvim-lsp',
    },
    config = function()
      -- this runs every time an LSP attaches to a buffer (i.e. when you open a file)
      vim.api.nvim_create_autocmd('LspAttach', {
        group = vim.api.nvim_create_augroup('lsp-attach', { clear = true }),
        callback = function(event)
          local map = function(keys, func, desc)
            vim.keymap.set('n', keys, func, { buffer = event.buf, desc = 'LSP: ' .. desc })
          end

          local tb = require 'telescope.builtin'
          -- jump to where a symbol is defined
          map('gd', tb.lsp_definitions, 'Goto Definition')
          -- find all usages of a symbol
          map('gr', tb.lsp_references, 'Goto References')
          map('gI', tb.lsp_implementations, 'Goto Implementation')
          map('<leader>D', tb.lsp_type_definitions, 'Type Definition')
          map('<leader>ds', tb.lsp_document_symbols, 'Document Symbols')
          map('<leader>ws', tb.lsp_dynamic_workspace_symbols, 'Workspace Symbols')
          -- rename a symbol across the whole project
          map('<leader>rn', vim.lsp.buf.rename, 'Rename')
          -- show available fixes or refactors at the cursor position
          map('<leader>ca', vim.lsp.buf.code_action, 'Code Action')
          -- declaration is different from definition (e.g. header files in C)
          map('gD', vim.lsp.buf.declaration, 'Goto Declaration')

          local client = vim.lsp.get_client_by_id(event.data.client_id)

          -- highlight all occurrences of the symbol under the cursor when idle
          -- skipped for large files because it can be slow
          if client and client.supports_method(vim.lsp.protocol.Methods.textDocument_documentHighlight) then
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

          -- inlay hints show inline type info (e.g. variable types in Rust)
          if client and client.supports_method(vim.lsp.protocol.Methods.textDocument_inlayHint) then
            map('<leader>th', function()
              vim.lsp.inlay_hint.enable(not vim.lsp.inlay_hint.is_enabled { bufnr = event.buf })
            end, 'Toggle Inlay Hints')
          end
        end,
      })

      -- broadcast nvim-cmp's extra completion capabilities to all language servers
      local capabilities = vim.tbl_deep_extend('force', vim.lsp.protocol.make_client_capabilities(), require('cmp_nvim_lsp').default_capabilities())

      -- add/remove servers here — mason will install them automatically
      -- run :Mason to see what's installed, :LspInfo to see active servers
      local servers = {
        rust_analyzer = {
          settings = {
            ['rust-analyzer'] = { cargo = { allFeatures = true } },
          },
        },
        eslint = {},
        taplo = {},   -- TOML
        ts_ls = {},   -- TypeScript/JavaScript
        css_variables = {},
        cssls = {},
        cssmodules_ls = {},
        svelte = {},
        lua_ls = {
          settings = {
            Lua = { completion = { callSnippet = 'Replace' } },
          },
        },
      }

      require('mason').setup()
      require('mason-tool-installer').setup {
        ensure_installed = vim.list_extend(vim.tbl_keys(servers), { 'stylua' }),
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
