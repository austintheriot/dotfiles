return {
  {
    -- encourages better vim motion practices (repeating hjkl, etc.)
    'm4xshen/hardtime.nvim',
    dependencies = { 'MunifTanjim/nui.nvim', 'nvim-lua/plenary.nvim' },
    opts = {
      disable_mouse = false,
    },
  },
}
