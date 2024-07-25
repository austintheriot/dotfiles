return {
  -- Detect tabstop and shiftwidth automatically
  'tpope/vim-sleuth',

  -- utility for applying comments with a keybinding
  -- "gcc" to comment visual lines
  -- "gbc" to comment blocks
  -- :help `comment-nvim` for more information
  { 'numToStr/Comment.nvim', opts = {} },

  -- Share permalinks to GitHub/GitLab
  {
    'ruifm/gitlinker.nvim',
    config = function()
      require "gitlinker".setup()
    end
  },


  {
    "kylechui/nvim-surround",
    version = "*", -- Use for stability; omit to use `main` branch for the latest features
    event = "VeryLazy",
    config = function()
      require("nvim-surround").setup({
        -- Configuration here, or leave empty to use defaults
      })
    end
  },


  -- Enable treesitter to auto-close and auto-update HTML/JSX tags
  'windwp/nvim-ts-autotag',
}
