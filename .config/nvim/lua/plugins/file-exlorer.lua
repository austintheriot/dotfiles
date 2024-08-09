-- Neo-tree is a Neovim plugin to browse the file system
-- https://github.com/nvim-neo-tree/neo-tree.nvim

return {
  {
    'nvim-tree/nvim-tree.lua',
    config = function()
      require('nvim-tree').setup {
        respect_buf_cwd = true,
        update_focused_file = {
          enable = true,
        },
        view = {
          number = true,
          relativenumber = true,
          width = 60,
        },
        actions = {
          open_file = {
            quit_on_open = true,
          },
        },
        -- todo: close nvim-tree after selecting file
      }
      vim.keymap.set('n', '<leader>o', ':NvimTreeToggle<CR>')
    end,
  },

  -- neo-tree config (may enable later)
  -- {
  --   'nvim-neo-tree/neo-tree.nvim',
  --   version = '*',
  --   lazy = false,
  --   dependencies = {
  --     'nvim-lua/plenary.nvim',
  --     'nvim-tree/nvim-web-devicons', -- not strictly required, but recommended
  --     'MunifTanjim/nui.nvim',
  --   },
  --   cmd = 'Neotree',
  --   keys = {
  --     -- opens in default position (whatever specified in ops.filesystem.window.position)
  --     { '<leader>o', ':Neotree toggle<CR>', desc = 'NeoTree reveal ' },
  --     -- open in current window
  --     { '<leader>O', ':Neotree toggle position=current<CR>', desc = 'NeoTree reveal (window)' },
  --   },
  --   opts = {
  --     close_if_last_window = true,
  --     filesystem = {
  --       -- when opening neovim with `nvim .`, it shows the big file view
  --       hijack_netrw_behavior = 'open_current',
  --       -- reveals the currently open file in neo-tree
  --       follow_current_file = {
  --         enabled = true, -- This will find and focus the file in the active buffer every time
  --         --               -- the current file is changed while the tree is open.
  --         leave_dirs_open = false, -- `false` closes auto expanded dirs, such as with `:Neotree reveal`
  --       },
  --       window = {
  --         -- make neotree fit width of tiles
  --         auto_expand_width = true,
  --         mappings = {
  --           ['<leader>o'] = 'close_window',
  --           ['-'] = 'navigate_up',
  --         },
  --       },
  --     },
  --   },
  -- },
}
