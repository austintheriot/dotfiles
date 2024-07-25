-- Set <space> as the leader key
-- See `:help mapleader`
--  NOTE: Must happen before plugins are required (otherwise wrong leader will be used)
vim.g.mapleader = ' '
vim.g.maplocalleader = ' '

-- must be disabled early on in init.lua to prevent race conditions with nvim-tree
vim.g.loaded_netrw = 1
vim.g.loaded_netrwPlugin = 1

-- Install package manager
--    https://github.com/folke/lazy.nvim
--    `:help lazy.nvim.txt` for more info
local lazypath = vim.fn.stdpath 'data' .. '/lazy/lazy.nvim'
if not vim.loop.fs_stat(lazypath) then
  vim.fn.system {
    'git',
    'clone',
    '--filter=blob:none',
    'https://github.com/folke/lazy.nvim.git',
    '--branch=stable', -- latest stable release
    lazypath,
  }
end
vim.opt.rtp:prepend(lazypath)

require('lazy').setup({
  {
    'jose-elias-alvarez/null-ls.nvim',
    lazy = false,
    config = function()
      local null_ls = require('null-ls')
      null_ls.setup({
        sources = {
          null_ls.builtins.formatting.prettierd,
        },
      })
    end
  },

  require 'plugins.icons',
  require 'plugins.telescope',
  require 'plugins.treesitter',
  require 'plugins.addons_windows',
  require 'plugins.addons_keys',
  require 'plugins.indicators',
  require 'plugins.statusline',
  require 'plugins.file_explorer',
  require 'plugins.theme',
  require 'plugins.autocompletion',
  require 'plugins.git',
  require 'plugins.lsp',
}, {})

require 'keymaps'
require 'settings'

-- Setup neovim lua configuration
require('neodev').setup()

-- The line beneath this is called `modeline`. See `:help modeline`
-- vim: ts=2 sts=2 sw=2 et
