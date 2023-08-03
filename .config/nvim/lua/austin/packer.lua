-- This file can be loaded by calling `lua require('plugins')` from your init.vim

-- Only required if you have packer configured as `opt`
vim.cmd [[packadd packer.nvim]]

return require('packer').startup(function(use)
  -- Packer can manage itself
  use 'wbthomason/packer.nvim'

  -- enable telescope plugin
  use {
	  'nvim-telescope/telescope.nvim', tag = '0.1.2',
	  -- or                            , branch = '0.1.x',
	  requires = { {'nvim-lua/plenary.nvim'} }
  }

  -- enable rose color theme
  use({ 'rose-pine/neovim', as = 'rose-pine', config = function() 
	vim.cmd('colorscheme rose-pine')
  end })

  -- enable syntax highlighting - see https://github.com/nvim-treesitter/nvim-treesitter
  use('nvim-treesitter/nvim-treesitter', { run = ':TSUpdate' })

  -- enable harpoon
  use('theprimeagen/harpoon')

  -- for branching undos
  use('mbbill/undotree')

  -- for integrating git with vim
  use('tpope/vim-fugitive')
end)
