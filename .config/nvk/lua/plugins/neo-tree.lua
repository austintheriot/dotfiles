-- Neo-tree is a Neovim plugin to browse the file system
-- https://github.com/nvim-neo-tree/neo-tree.nvim

return {
  'nvim-neo-tree/neo-tree.nvim',
  version = '*',
  lazy = false,
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
    close_if_last_window = true,
    filesystem = {
      -- when opening neovim with `nvim .`, it shows the big file view
      hijack_netrw_behavior = 'open_current',
      -- reveals the currently open file in neo-tree
      follow_current_file = {
        enabled = true, -- This will find and focus the file in the active buffer every time
        --               -- the current file is changed while the tree is open.
        leave_dirs_open = false, -- `false` closes auto expanded dirs, such as with `:Neotree reveal`
      },
      window = {
        mappings = {
          ['<leader>o'] = 'close_window',
          ['-'] = 'navigate_up',
        },
      },
    },
  },
}
