require 'settings'
require 'keymaps'
require 'dotfiles.health'

-- Stop here rather than at the first 0.10-only call below. `vim.uv` arrived
-- in 0.10; an older Neovim aborted this chunk with "attempt to index field
-- 'uv' (a nil value)", which names a Lua field and not the actual problem.
-- The floor check in dotfiles.health cannot cover this: :checkhealth is
-- unreachable once startup fails.
--
-- `vim.version` and `vim.notify` both predate 0.7, so this runs on every
-- version it rejects.
if not vim.version.ge(vim.version(), '0.10') then
  vim.notify(string.format("This config needs Neovim 0.10 or newer. Found '%s'. Plugins are disabled.", tostring(vim.version())), vim.log.levels.ERROR)
  return
end

local lazypath = vim.fn.stdpath 'data' .. '/lazy/lazy.nvim'
if not (vim.uv or vim.loop).fs_stat(lazypath) then
  local out = vim.fn.system { 'git', 'clone', '--filter=blob:none', '--branch=stable', 'https://github.com/folke/lazy.nvim.git', lazypath }
  if vim.v.shell_error ~= 0 then
    error('Error cloning lazy.nvim:\n' .. out)
  end
end
vim.opt.rtp:prepend(lazypath)

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

  'tpope/vim-sleuth',
}, {
  ui = {
    icons = vim.g.have_nerd_font and {} or {
      cmd = '⌘',
      config = '🛠',
      event = '📅',
      ft = '📂',
      init = '⚙',
      keys = '🗝',
      plugin = '🔌',
      runtime = '💻',
      require = '🌙',
      source = '📄',
      start = '🚀',
      task = '📌',
      lazy = '💤 ',
    },
  },
})
