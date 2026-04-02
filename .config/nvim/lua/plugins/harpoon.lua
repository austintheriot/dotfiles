return {
  {
    'ThePrimeagen/harpoon',
    config = function()
      require('harpoon').setup { mark_branch = true }

      local mark = require 'harpoon.mark'
      local ui = require 'harpoon.ui'

      vim.keymap.set('n', '<leader>a', mark.add_file, { desc = 'Harpoon add file' })
      vim.keymap.set('n', '<leader><leader>', ui.toggle_quick_menu, { desc = 'Harpoon quick menu' })
      vim.keymap.set('n', '<leader>h', '<cmd>lua require("harpoon.ui").nav_file(vim.v.count1)<cr>', { desc = 'Harpoon select buffer' })
    end,
  },
}
