local lsp = require('lsp-zero')

lsp.preset("recommended")

lsp.ensure_installed({
	-- this may require having `npm` installed (and possibly typescript?)
	'tsserver',
	'rust_analyzer',
	'eslint',
})

lsp.on_attach(function(client, bufnr)
	-- see :help lsp-zero-keybindings
	-- to learn the available actions
	lsp.default_keymaps({buffer = bufnr})
end)

-- (Optional) Configure lua language server for neovim
require('lspconfig').lua_ls.setup(lsp.nvim_lua_ls())

lsp.setup()
