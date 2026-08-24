vim.g.mapleader = ' '
vim.g.maplocalleader = ' '
vim.g.have_nerd_font = true

-- must be set before plugins load to prevent netrw from opening
vim.g.loaded_netrw = 1
vim.g.loaded_netrwPlugin = 1


vim.o.spell = true
vim.o.spelllang = 'en_us'
vim.o.spellsuggest = 'best,9'

vim.opt.number = true
vim.opt.relativenumber = true
vim.opt.mouse = 'a'
vim.opt.showmode = false
vim.opt.breakindent = true
vim.opt.undofile = true
vim.opt.ignorecase = true
vim.opt.smartcase = true
vim.opt.signcolumn = 'yes'
vim.opt.updatetime = 250
vim.opt.timeoutlen = 300
vim.opt.splitright = true
vim.opt.splitbelow = false
vim.opt.list = true
vim.opt.listchars = { tab = '» ', trail = '·', nbsp = '␣' }
vim.opt.inccommand = 'split'
vim.opt.scrolloff = 10
vim.o.foldmethod = 'manual'
vim.o.foldlevel = 99
vim.o.foldenable = true

vim.schedule(function()
  vim.opt.clipboard = 'unnamedplus'
end)

-- only enable cursorline for smaller files (performance)
vim.api.nvim_create_autocmd('BufEnter', {
  callback = function()
    vim.opt.cursorline = vim.fn.getfsize(vim.api.nvim_buf_get_name(0)) < 100000
  end,
})
