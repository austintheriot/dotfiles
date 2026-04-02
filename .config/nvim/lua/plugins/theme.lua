return {
  {
    'Mofiqul/vscode.nvim',
    priority = 1000,
    config = function()
      vim.o.background = 'dark'
      local c = require('vscode.colors').get_colors()
      require('vscode').setup {
        transparent = true,
        italic_comments = true,
        disable_nvimtree_bg = true,
        color_overrides = { vscLineNumber = '#FFFFFF' },
        group_overrides = {
          Cursor = { fg = c.vscDarkBlue, bg = c.vscLightGreen, bold = true },
        },
      }
      require('vscode').load()
    end,
  },
}
