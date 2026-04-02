-- telescope is a fuzzy finder — use it to search files, text, LSP symbols, and more
-- while inside a picker: <C-/> (insert) or ? (normal) shows all available keymaps

local function in_git_repo()
  local handle = io.popen 'git rev-parse --is-inside-work-tree 2>/dev/null'
  if not handle then return false end
  local result = handle:read '*a'
  handle:close()
  return result:match 'true'
end

return {
  {
    'nvim-telescope/telescope.nvim',
    event = 'VimEnter',
    branch = '0.1.x',
    dependencies = {
      'nvim-lua/plenary.nvim',
      -- native fzf sorting for much better performance
      { 'nvim-telescope/telescope-fzf-native.nvim', build = 'make', cond = function() return vim.fn.executable 'make' == 1 end },
      -- replaces vim.ui.select (used for code actions, etc.) with telescope
      'nvim-telescope/telescope-ui-select.nvim',
      { 'nvim-tree/nvim-web-devicons', enabled = vim.g.have_nerd_font },
    },
    config = function()
      local actions = require 'telescope.actions'

      local function move_up(n)
        return function(opts)
          for _ = 1, n do actions.move_selection_previous(opts) end
        end
      end

      local function move_down(n)
        return function(opts)
          for _ = 1, n do actions.move_selection_next(opts) end
        end
      end

      local shared_mappings = {
        -- refine results with a second fuzzy search
        ['<C-r>'] = 'to_fuzzy_refine',
        -- confirm selection (remapped from <CR> to match autocomplete muscle memory)
        ['<C-y>'] = 'select_default',
        ['<CR>'] = false,
        -- disable arrow keys inside pickers
        ['<left>'] = false,
        ['<right>'] = false,
        ['<up>'] = false,
        ['<down>'] = false,
        -- jump 5 items at a time
        ['<C-u>'] = move_up(5),
        ['<C-d>'] = move_down(5),
      }

      require('telescope').setup {
        defaults = {
          -- when a path is too long, drop leading segments rather than truncating the filename
          path_display = { truncate = 3 },
          -- vertical splits the window into preview (top) and results+prompt (bottom)
          layout_strategy = 'vertical',
          layout_config = {
            vertical = {
              -- <1.0 is treated as a percentage; 1.0 exactly would be 1 column/row
              width = 0.999,
              height = 0.999,
              preview_height = 0.6,
            },
          },
          -- standard box-drawing border so the window extends flush to screen edges
          borderchars = { '─', '│', '─', '│', '┌', '┐', '┘', '└' },
          mappings = { i = shared_mappings, n = shared_mappings },
        },
        pickers = {
          -- fname_width controls the path column width in results; default is ~30 which truncates deeply nested paths
          live_grep = { fname_width = 70 },
          grep_string = { fname_width = 70 },
          lsp_references = { fname_width = 70 },
          lsp_definitions = { fname_width = 70 },
          lsp_implementations = { fname_width = 70 },
        },
        extensions = {
          ['ui-select'] = {
            require('telescope.themes').get_dropdown { cache_picker = false },
          },
        },
      }

      pcall(require('telescope').load_extension, 'fzf')
      pcall(require('telescope').load_extension, 'ui-select')

      -- nvim 0.12 removed these from nvim-treesitter; shim them so telescope doesn't crash
      local ok, parsers = pcall(require, 'nvim-treesitter.parsers')
      if ok and not parsers.ft_to_lang then
        parsers.ft_to_lang = function(ft) return ft end
      end
      package.loaded['nvim-treesitter.configs'] = package.loaded['nvim-treesitter.configs'] or {
        is_enabled = function() return false end,
      }

      local builtin = require 'telescope.builtin'
      vim.keymap.set('n', '<leader>sb', builtin.buffers, { desc = '[S]earch [B]uffers' })
      vim.keymap.set('n', '<leader>sd', builtin.diagnostics, { desc = '[S]earch [D]iagnostics' })
      -- prefer git files when in a repo (respects .gitignore), fall back to all files
      vim.keymap.set('n', '<leader>sf', function()
        return in_git_repo() and builtin.git_files() or builtin.find_files()
      end, { desc = '[S]earch [F]iles' })
      vim.keymap.set('n', '<leader>sF', builtin.find_files, { desc = '[S]earch All [F]iles' })
      vim.keymap.set('n', '<leader>sg', builtin.live_grep, { desc = '[S]earch by [G]rep' })
      -- -u tells ripgrep to include hidden/ignored files
      vim.keymap.set('n', '<leader>sG', function()
        builtin.live_grep { additional_args = { '-u' } }
      end, { desc = '[S]earch Hidden Files by [G]rep' })
      vim.keymap.set('n', '<leader>sh', builtin.help_tags, { desc = '[S]earch [H]elp' })
      vim.keymap.set('n', '<leader>sk', builtin.keymaps, { desc = '[S]earch [K]eymaps' })
      vim.keymap.set('n', '<leader>so', builtin.oldfiles, { desc = '[S]earch [O]ld Files' })
      vim.keymap.set('n', '<leader>sr', builtin.resume, { desc = '[S]earch [R]esume' })
      vim.keymap.set('n', '<leader>ss', builtin.builtin, { desc = '[S]earch [S]elect Telescope' })
      vim.keymap.set('n', '<leader>sw', builtin.grep_string, { desc = '[S]earch current [W]ord' })
      vim.keymap.set('n', '<leader>/', function()
        builtin.current_buffer_fuzzy_find(require('telescope.themes').get_dropdown {
          winblend = 10,
          previewer = false,
        })
      end, { desc = '[/] Fuzzily search in current buffer' })
      vim.keymap.set('n', '<leader>s/', function()
        builtin.live_grep { grep_open_files = true, prompt_title = 'Live Grep in Open Files' }
      end, { desc = '[S]earch [/] in Open Files' })
      vim.keymap.set('n', '<leader>sn', function()
        builtin.find_files { cwd = vim.fn.stdpath 'config' }
      end, { desc = '[S]earch [N]eovim files' })
    end,
  },
}
