-- <space> as the leader key — must be set before plugins load
vim.g.mapleader = ' '
vim.g.maplocalleader = ' '

-- set to true if you have a Nerd Font installed in your terminal
vim.g.have_nerd_font = true

-- disable netrw (vim's built-in file browser) in favor of nvim-tree
-- must happen before any plugin loads
vim.g.loaded_netrw = 1
vim.g.loaded_netrwPlugin = 1

vim.o.spell = true
vim.o.spelllang = 'en_us'
vim.o.spellsuggest = 'best,9'

vim.opt.number = true
-- relative line numbers make jumping easier (e.g. 5j, 12k)
vim.opt.relativenumber = true
vim.opt.mouse = 'a'
-- mode is already shown in the statusline
vim.opt.showmode = false
-- wrapped lines keep their indentation
vim.opt.breakindent = true
-- persist undo history across sessions
vim.opt.undofile = true
vim.opt.ignorecase = true
-- case-sensitive only when the search includes a capital letter
vim.opt.smartcase = true
-- always show the sign column to avoid layout shifts on diagnostics/gitsigns
vim.opt.signcolumn = 'yes'
-- ms before CursorHold fires (affects LSP highlights, gitsigns)
vim.opt.updatetime = 250
-- ms to wait for a key sequence (affects which-key popup speed)
vim.opt.timeoutlen = 300
-- vertical splits open to the right
vim.opt.splitright = true
vim.opt.splitbelow = false
vim.opt.list = true
vim.opt.listchars = { tab = '» ', trail = '·', nbsp = '␣' }
-- show a live preview of :substitute in a split
vim.opt.inccommand = 'split'
-- keep at least 10 lines visible above/below the cursor
vim.opt.scrolloff = 10
vim.o.foldmethod = 'manual'
-- start with all folds open
vim.o.foldlevel = 99
vim.o.foldenable = true

-- clipboard sync is deferred to avoid slowing down startup
vim.schedule(function()
  vim.opt.clipboard = 'unnamedplus'
end)

-- cursorline highlighting is expensive on large files
vim.api.nvim_create_autocmd('BufEnter', {
  callback = function()
    vim.opt.cursorline = vim.fn.getfsize(vim.api.nvim_buf_get_name(0)) < 100000
  end,
})
