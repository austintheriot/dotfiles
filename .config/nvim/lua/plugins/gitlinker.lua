return {
  -- Share permalinks to GitHub/GitLab
  -- <leader>gy
  {
    'ruifm/gitlinker.nvim',
    config = function()
      require('gitlinker').setup()
    end,
  },
}
