local prettier = { 'prettier', 'prettierd', stop_after_first = true }

return {
  {
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
      notify_on_error = true,
      notify_no_formatters = true,
      formatters_by_ft = {
        lua = { 'stylua' },
        -- rustfmt, not rust-analyzer's own formatting. Without this entry
        -- `<leader>f` on a .rs file fell through to `lsp_fallback` and
        -- whatever the server decided, which is not the same tool CI runs.
        --
        -- No install step is needed: rustup's DEFAULT profile ships rustfmt
        -- beside cargo and clippy, and rustup is a tracked dependency.
        -- Verified in a clean ubuntu:24.04, where
        -- `rustup component list --installed` names rustfmt with no extra
        -- step. That is why this is not in mason-lock.json.
        rust = { 'rustfmt' },
        javascript = prettier,
        typescript = prettier,
        javascriptreact = prettier,
        typescriptreact = prettier,
        svelte = prettier,
        css = prettier,
        html = prettier,
        json = prettier,
        yaml = prettier,
        markdown = prettier,
        graphql = prettier,
        liquid = prettier,
      },
    },
  },
}
