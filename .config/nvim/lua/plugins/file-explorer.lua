return {
  {
    'nvim-tree/nvim-tree.lua',
    lazy = false,
    keys = { { '<leader>o', '<cmd>NvimTreeToggle<CR>', desc = 'Toggle file explorer' } },
    opts = {
      disable_netrw = true,
      respect_buf_cwd = true,
      hijack_directories = { enable = true, auto_open = true },
      update_focused_file = { enable = true },
      view = { number = true, relativenumber = true, width = 60 },
      actions = { open_file = { quit_on_open = true } },
    },
    config = function(_, opts)
      require('nvim-tree').setup(opts)
    end,
  },
}
