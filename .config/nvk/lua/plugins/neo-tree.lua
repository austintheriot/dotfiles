-- Neo-tree is a Neovim plugin to browse the file system
-- https://github.com/nvim-neo-tree/neo-tree.nvim

return {
  'nvim-neo-tree/neo-tree.nvim',
  version = '*',
  dependencies = {
    'nvim-lua/plenary.nvim',
    'nvim-tree/nvim-web-devicons', -- not strictly required, but recommended
    'MunifTanjim/nui.nvim',
  },
  cmd = 'Neotree',
  keys = {
    -- opens in default position (whatever specified in ops.filesystem.window.position)
    { '<leader>o', ':Neotree toggle<CR>', desc = 'NeoTree reveal ' },
    -- open in current window
    { '<leader>O', ':Neotree toggle position=current<CR>', desc = 'NeoTree reveal (window)' },
  },
  opts = {
    close_if_last_window = false,
    filesystem = {
      window = {
        mappings = {
          ['<leader>o'] = 'close_window',
          ['-'] = 'navigate_up',
        },
      },
    },
  },
}
