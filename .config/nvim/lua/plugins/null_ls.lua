return {
  -- used for formatting on <leader>f
  {
    'jose-elias-alvarez/null-ls.nvim',
    lazy = false,
    config = function()
      local null_ls = require('null-ls')
      null_ls.setup({
        sources = {
          null_ls.builtins.formatting.prettierd,
        },
      })
    end
  },
}
