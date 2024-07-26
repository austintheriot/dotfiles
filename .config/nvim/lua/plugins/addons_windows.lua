return {
  -- shows available keybinds when <leader> key is pressed
  'folke/which-key.nvim',

  -- undotree - traverse edit graphs in a separate window
  {
    'mbbill/undotree',
    config = function()
      vim.keymap.set('n', '<leader>u', vim.cmd.UndotreeToggle, { desc = "Open [u]ndo tree" })
    end
  },

  -- keeps track of a small list of files that you frequently switch between
  {
    'ThePrimeagen/harpoon',
    config = function(self, opts)
      require('harpoon').setup({
        -- set marks specific to each git branch inside git repository
        mark_branch = true,
      })
      local mark = require('harpoon.mark')
      local ui = require('harpoon.ui')
      -- add this file to harpoon list
      vim.keymap.set("n", "<leader>a", mark.add_file, { desc = "Harpoon [a]dd file" })
      -- open up the harpoon menu
      vim.keymap.set("n", "<leader><leader>", ui.toggle_quick_menu, { desc = "Open Harpoon quick toggle menu" })
      -- 1<leader>h navigates to file 1, and 3<leader>h navigates to file 3, etc.
      vim.keymap.set('n', '<leader>h', '<cmd>lua require("harpoon.ui").nav_file(vim.v.count1)<cr>', opts)
    end
  },
}
