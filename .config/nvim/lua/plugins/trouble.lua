-- trouble.nvim gives you a nicer panel for diagnostics, LSP references, quickfix, etc.
-- replaces the default <leader>q diagnostic list with a much more readable view

return {
  {
    'folke/trouble.nvim',
    dependencies = { 'nvim-tree/nvim-web-devicons' },
    opts = {},
    keys = {
      { '<leader>q', '<cmd>Trouble diagnostics toggle<cr>', desc = 'Diagnostics (Trouble)' },
      { '<leader>Q', '<cmd>Trouble diagnostics toggle filter.buf=0<cr>', desc = 'Buffer diagnostics (Trouble)' },
    },
  },
}
