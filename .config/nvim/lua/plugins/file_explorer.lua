return {
  {
    "nvim-tree/nvim-tree.lua",
    config = function()
      require('nvim-tree').setup({
        respect_buf_cwd = true,
        update_focused_file = {
          enable = true,
        },
        view = {
          number = true,
          relativenumber = true,
          width = 60
        },
        actions = {
          open_file = {
            quit_on_open = true
          }
        }
        -- todo: close nvim-tree after selecting file
      })
      vim.keymap.set("n", "<leader>o", ':NvimTreeToggle<CR>')
    end
  },
}
