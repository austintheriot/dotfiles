require 'settings'
require 'keymaps'
require 'health'

-- bootstrap lazy.nvim (plugin manager) — clones it if not already installed
local lazypath = vim.fn.stdpath 'data' .. '/lazy/lazy.nvim'
if not vim.uv.fs_stat(lazypath) then
  local out = vim.fn.system { 'git', 'clone', '--filter=blob:none', '--branch=stable', 'https://github.com/folke/lazy.nvim.git', lazypath }
  if vim.v.shell_error ~= 0 then
    error('Error cloning lazy.nvim:\n' .. out)
  end
end
vim.opt.rtp:prepend(lazypath)

-- each plugin lives in its own file under lua/plugins/
-- run :Lazy to manage plugins, :Lazy update to update them
require('lazy').setup({
  -- essentials
  require 'plugins.telescope',
  require 'plugins.lsp',
  require 'plugins.autoformat',
  require 'plugins.autocomplete',
  require 'plugins.treesitter',
  require 'plugins.file-explorer',
  require 'plugins.lint',

  -- ui
  require 'plugins.theme',
  require 'plugins.mini',
  require 'plugins.which-key',
  require 'plugins.indent-blankline',
  require 'plugins.todo-comments',

  -- git
  require 'plugins.gitsigns',
  require 'plugins.gitlinker',
  'tpope/vim-fugitive',

  -- editing
  require 'plugins.autopairs',
  require 'plugins.harpoon',
  require 'plugins.undotree',
  require 'plugins.marks',
  require 'plugins.cspell-actions',
  require 'plugins.markdown-preview',

  -- automatically detect tab/space settings from the current file
  'tpope/vim-sleuth',
}, {
  ui = {
    icons = vim.g.have_nerd_font and {} or {
      cmd = '⌘', config = '🛠', event = '📅', ft = '📂', init = '⚙',
      keys = '🗝', plugin = '🔌', runtime = '💻', require = '🌙',
      source = '📄', start = '🚀', task = '📌', lazy = '💤 ',
    },
  },
})
