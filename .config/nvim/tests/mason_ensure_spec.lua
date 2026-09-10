local t = require '_harness'
local m = require 'dotfiles.mason_ensure'
local mason_names = { lua_ls = 'lua-language-server', rust_analyzer = 'rust-analyzer' }
local lock = { ['lua-language-server'] = '3.18.1', stylua = 'v2.4.1' }
t.eq(
  m.ensure_list({ 'lua_ls', 'stylua', 'rust_analyzer', 'prettier' }, mason_names, lock),
  { { 'lua-language-server', version = '3.18.1' }, { 'stylua', version = 'v2.4.1' }, 'rust-analyzer', 'prettier' },
  'lspconfig names map to mason names; a pinned tool is a {name, version=} table; an unpinned one is a bare string'
)
t.eq(m.ensure_list({}, mason_names, lock), {}, 'nothing in, nothing out')
t.finish 'mason_ensure'
