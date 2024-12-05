local shared_prettier_config = {
  'prettier',
  'prettierd',
  -- You can use 'stop_after_first' to run the first available formatter from the list
  stop_after_first = true,
}

return {
  { -- Autoformat
    'stevearc/conform.nvim',
    event = { 'BufWritePre' },
    cmd = { 'ConformInfo' },
    keys = {
      {
        '<leader>f',
        function()
          require('conform').format { async = true, lsp_fallback = true }
        end,
        mode = '',
        desc = '[F]ormat buffer',
      },
    },
    opts = {
      -- Conform will notify you when a formatter errors
      notify_on_error = true,
      -- Conform will notify you when no formatters are available for the buffer
      notify_no_formatters = true,
      -- format_on_save = function(bufnr)
      --   -- Disable "format_on_save lsp_fallback" for languages that don't
      --   -- have a well standardized coding style. You can add additional
      --   -- languages here or re-enable it for the disabled ones.
      --   local disable_filetypes = { c = true, cpp = true }
      --   return {
      --     timeout_ms = 500,
      --     lsp_fallback = not disable_filetypes[vim.bo[bufnr].filetype],
      --   }
      -- end,
      formatters_by_ft = {
        -- Conform can also run multiple formatters sequentially
        -- python = { "isort", "black" },
        lua = { 'stylua' },
        javascript = shared_prettier_config,
        typescript = shared_prettier_config,
        javascriptreact = shared_prettier_config,
        typescriptreact = shared_prettier_config,
        svelte = shared_prettier_config,
        css = shared_prettier_config,
        html = shared_prettier_config,
        json = shared_prettier_config,
        yaml = shared_prettier_config,
        markdown = shared_prettier_config,
        graphql = shared_prettier_config,
        liquid = shared_prettier_config,
      },
    },
  },
}
