return {
  -- undotree - traverse edit graphs in a separate window
  {
    'mbbill/undotree',
    config = function()
      vim.keymap.set('n', '<leader>u', vim.cmd.UndotreeToggle, { desc = 'Open [u]ndo tree' })
    end,
  },
}
