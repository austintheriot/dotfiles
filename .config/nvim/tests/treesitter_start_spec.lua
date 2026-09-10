local t = require '_harness'
local m = require 'dotfiles.treesitter_start'
local has = function(lang)
  return lang == 'rust'
end
t.eq(m.should_start('rust', has), true, 'a language with a parser starts')
t.eq(m.should_start('NvimTree', has), false, 'a name with no parser is skipped, not errored')
t.eq(m.should_start('', has), false, 'an empty filetype never starts')
t.eq(m.should_start(nil, has), false, 'a nil filetype never starts')
t.finish 'treesitter_start'
