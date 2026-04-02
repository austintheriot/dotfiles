-- marks.nvim shows vim marks as signs in the sign column
-- use m<letter> to set a mark, '<letter> to jump to it
-- use zg to add a word to the spell dictionary, z= for suggestions

return {
  {
    'chentoast/marks.nvim',
    opts = {
      default_mappings = true,
      builtin_marks = { '.', '<', '>', '^' },
      cyclic = true,
      force_write_shada = false,
      refresh_interval = 250,
      sign_priority = { lower = 10, upper = 15, builtin = 8, bookmark = 20 },
      excluded_filetypes = {},
      bookmark_0 = { sign = '⚑', virt_text = 'hello world', annotate = false },
      mappings = {},
    },
  },
}
