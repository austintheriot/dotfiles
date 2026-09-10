local t = require '_harness'
local m = require 'dotfiles.lint_runnable'
local exe = function(cmd)
  return cmd == 'selene' or cmd == 'markdownlint'
end
t.eq(
  m.runnable({ { name = 'selene', cmd = 'selene' }, { name = 'cspell', cmd = 'cspell' }, { name = 'md', cmd = 'markdownlint' } }, exe),
  { 'selene', 'md' },
  'keeps only linters whose command is executable, in order'
)
t.eq(m.runnable({}, exe), {}, 'nothing in, nothing out')
t.eq(m.runnable({ { name = 'cspell', cmd = 'cspell' } }, exe), {}, 'all absent yields an empty list, not nil')
t.finish 'lint_runnable'
