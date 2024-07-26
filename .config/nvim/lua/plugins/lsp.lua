-- LSP Configuration & Plugins
--
-- Use your language server to automatically format your code on save.
-- Adds additional commands as well to manage the behavior

-- [[ Configure LSP ]]

--  This function gets run when an LSP connects to a particular buffer.
local on_attach = function(client, bufnr)
  -- NOTE: Remember that lua is a real programming language, and as such it is possible
  -- to define small helper and utility functions so you don't have to repeat yourself
  -- many times.
  --
  -- In this case, we create a function that lets us more easily define mappings specific
  -- for LSP related items. It sets the mode, buffer and description for us each time.
  local nmap = function(keys, func, desc, default_mode)
    if desc then
      desc = 'LSP: ' .. desc
    end

    local mode = default_mode or 'n'
    vim.keymap.set(mode, keys, func, { buffer = bufnr, desc = desc })
  end

  nmap('<leader>rn', vim.lsp.buf.rename, '[R]e[n]ame')
  nmap('<leader>ca', vim.lsp.buf.code_action, '[C]ode [A]ction')

  nmap('gd', vim.lsp.buf.definition, '[G]oto [D]efinition')
  nmap('gr', require('telescope.builtin').lsp_references, '[G]oto [R]eferences')
  nmap('gI', vim.lsp.buf.implementation, '[G]oto [I]mplementation')
  nmap('<leader>D', vim.lsp.buf.type_definition, 'Type [D]efinition')
  nmap('<leader>ds', require('telescope.builtin').lsp_document_symbols, '[D]ocument [S]ymbols')
  nmap('<leader>ws', require('telescope.builtin').lsp_dynamic_workspace_symbols, '[W]orkspace [S]ymbols')

  -- See `:help K` for why this keymap
  nmap('K', vim.lsp.buf.hover, 'Hover Documentation')
  -- in normal mode, <C-k> is already taken by switching vim windows
  nmap('<leader>k', vim.lsp.buf.signature_help, 'Signature Documentation', 'n')
  nmap('<C-k>', vim.lsp.buf.signature_help, 'Signature Documentation', 'i')

  -- Lesser used LSP functionality
  nmap('gD', vim.lsp.buf.declaration, '[G]oto [D]eclaration')
  nmap('<leader>wa', vim.lsp.buf.add_workspace_folder, '[W]orkspace [A]dd Folder')
  nmap('<leader>wr', vim.lsp.buf.remove_workspace_folder, '[W]orkspace [R]emove Folder')
  nmap('<leader>wl', function()
    print(vim.inspect(vim.lsp.buf.list_workspace_folders()))
  end, '[W]orkspace [L]ist Folders')


  local run_format = function(local_bufnr)
    local file_path = vim.fn.expand('%:p')
    -- use null-ls to format in Notability repo,
    -- since it's necessary to use prettier config
    if (string.find(file_path, "Notability")) then
      vim.lsp.buf.format({
        bufnr = local_bufnr,
        filter = function(local_client)
          return local_client.name == "null-ls"
        end
      })
      print("Formatted with null-ls & pretterd")
    else
      vim.lsp.buf.format({ timeout_ms = 10000 })
      print("Formatted with default LSP formatter")
    end
  end

  -- Create a command `:Format` local to the LSP buffer
  vim.api.nvim_buf_create_user_command(bufnr, 'Format', function(_)
    run_format(bufnr)
  end, { desc = 'Format current buffer with LSP' })

  -- assign keymap for formatting in normal mode
  vim.keymap.set("n", "<Leader>f", run_format, { desc = "[lsp] format" })

  -- allow auto-formatting on save
  if client.supports_method("textDocument/formatting") then
    vim.api.nvim_create_autocmd("BufWritePre", {
      callback = function()
        run_format(bufnr)
      end,
      group = vim.api.nvim_create_augroup("lsp_document_format", { clear = true }),
      buffer = 0
    })
  else
    vim.schedule(function()
      print('No formatter')
    end)
  end
end

return {
  'neovim/nvim-lspconfig',
  dependencies = {
    -- Automatically install LSPs to stdpath for neovim
    { 'williamboman/mason.nvim', config = true },
    'williamboman/mason-lspconfig.nvim',

    -- Useful status updates for LSP
    -- NOTE: `opts = {}` is the same as calling `require('fidget').setup({})`
    { 'j-hui/fidget.nvim',       tag = 'legacy', opts = {} },

    -- Additional lua configuration, makes nvim stuff amazing!
    'folke/neodev.nvim',
  },
  config = function()
    -- Enable the following language servers
    -- They will automatically be installed.
    --
    --  Add any additional override configuration in the following tables. They will be passed to
    --  the `settings` field of the server config. You must look up that documentation yourself.
    --
    --  If you want to override the default filetypes that your language server will attach to you can
    --  define the property 'filetypes' to the map in question.
    local servers = {
      rust_analyzer = {},
      tsserver = {},
      eslint = {},
      html = { filetypes = { 'html', 'twig', 'hbs' } },
      pylsp = {
        configurationSources = { 'flake8' },
        plugins = {
          -- we don't care about these, we use ruff (3rd-party_
          pycodestyle = { enabled = false },
          mccabe = { enabled = false },
          pyflakes = { enabled = false },
          flake8 = {
            enabled = false,
          },
        }
      },
      -- TOML
      taplo = {},
      ruff_lsp = {
        -- Any extra CLI arguments for `ruff` can go here.
        args = {},
      },
      lua_ls = {
        Lua = {
          workspace = { checkThirdParty = false },
          telemetry = { enable = false },
        },
      },
    }

    -- nvim-cmp supports additional completion capabilities, so broadcast that to servers
    local capabilities = vim.lsp.protocol.make_client_capabilities()
    capabilities = require('cmp_nvim_lsp').default_capabilities(capabilities)

    -- Ensure the servers above are installed
    local mason_lspconfig = require 'mason-lspconfig'

    mason_lspconfig.setup {
      ensure_installed = vim.tbl_keys(servers),
    }

    mason_lspconfig.setup_handlers {
      function(server_name)
        require('lspconfig')[server_name].setup {
          capabilities = capabilities,
          on_attach = on_attach,
          settings = servers[server_name],
          filetypes = (servers[server_name] or {}).filetypes,
        }
      end
    }
  end,
}
