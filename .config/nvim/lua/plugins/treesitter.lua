-- treesitter parses your code into a syntax tree for accurate highlighting,
-- indentation, and incremental selection. it's much more precise than vim's
-- built-in regex-based highlighting.
-- run :TSUpdate to update parsers, :TSInstallInfo to see what's installed

return {
  {
    'nvim-treesitter/nvim-treesitter',
    build = ':TSUpdate',
    config = function()
      require('nvim-treesitter').setup {
        -- parsers listed here are always installed; others are installed on first open
        ensure_installed = {
          'bash', 'c', 'diff', 'html', 'lua', 'luadoc',
          'markdown', 'markdown_inline', 'query', 'vim', 'vimdoc',
          'rust', 'typescript', 'javascript', 'tsx', 'css', 'json', 'toml', 'svelte',
        },
        auto_install = true,
        highlight = {
          enable = true,
          -- ruby requires vim's regex highlighting for correct indentation
          additional_vim_regex_highlighting = { 'ruby' },
        },
        indent = { enable = true, disable = { 'ruby' } },
        incremental_selection = {
          enable = true,
          keymaps = {
            -- press v to expand selection by treesitter node, V to shrink
            node_incremental = 'v',
            node_decremental = 'V',
          },
        },
      }
    end,
  },
}
