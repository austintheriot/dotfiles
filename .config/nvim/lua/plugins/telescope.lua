return {
  -- Fuzzy Finder Algorithm which requires local dependencies to be built.
  -- Only load if `make` is available. Make sure you have the system
  -- requirements installed.
  {
    'nvim-telescope/telescope-fzf-native.nvim',
    -- NOTE: If you are having trouble with this installation,
    --       refer to the README for telescope-fzf-native for more instructions.
    build = 'make',
    cond = function()
      return vim.fn.executable 'make' == 1
    end,
  },

  -- Fuzzy Finder (files, lsp, etc)
  {
    'nvim-telescope/telescope.nvim',
    branch = '0.1.x',
    dependencies = { 'nvim-lua/plenary.nvim' },
    config = function()
      -- [[ Configure Telescope ]]
      -- See `:help telescope` and `:help telescope.setup()`
      require('telescope').setup {
        defaults = {
          mappings = {
            i = {
              ['<C-u>'] = false,
              ['<C-d>'] = false,
            },
          },
        },
      }

      -- Enable telescope fzf native, if installed
      pcall(require('telescope').load_extension, 'fzf')

      -- See `:help telescope.builtin`
      vim.keymap.set('n', '<leader>so', require('telescope.builtin').oldfiles,
        { desc = '[S]earch Recently [O]pened Files' })
      vim.keymap.set('n', '<leader>sb', require('telescope.builtin').buffers, { desc = '[S]earch Existing [B]uffers' })
      vim.keymap.set('n', '<leader>sr', require('telescope.builtin').resume, { desc = '[S]earch [R]esume' })
      vim.keymap.set('n', '<leader>/', function()
        -- You can pass additional configuration to telescope to change theme, layout, etc.
        require('telescope.builtin').current_buffer_fuzzy_find(require('telescope.themes').get_dropdown {
          winblend = 10,
          previewer = false,
        })
      end, { desc = '[/] Fuzzily search in current buffer' })

      -- MOST HELPFUL TELESCOPE FEATURES:
      -- Fuzzy search through the output of git ls-files command, respects .gitignore
      -- This is the equivalent of fuzzy finding by file in VS Code
      vim.keymap.set('n', '<leader>sf', require('telescope.builtin').git_files, { desc = '[S]earch (git) [F]iles' })
      -- Search for a string in your current working directory and get results live as you type, respects .gitignore. (Requires ripgrep)
      -- This is the equivalent of global "find" in VS Code
      vim.keymap.set('n', '<leader>sg', require('telescope.builtin').live_grep, { desc = '[S]earch by [G]rep' })
      -- This is the equivalent of global "find" in VS Code, but searching ignored files
      vim.keymap.set('n', '<leader>sG',
        function()
          require('telescope.builtin').live_grep({ additional_args = { '-u' } })
        end
        , { desc = '[S]earch Hidden Files by [G]rep' })
      -- Searches for the string under your cursor or selection in your current working directory
      -- This is the equivalent to VS Code's per-file search feature, with the added convenience of auto-searching
      -- the word that is under the cursor or currently selected with visual mode
      vim.keymap.set('n', '<leader>sw', require('telescope.builtin').grep_string, { desc = '[S]earch current [W]ord' })

      -- MORE OBSCURE TELESCOPE FEATURES (may delete later if not needed):
      -- Lists files in your current working directory, respects .gitignore
      -- This includes files normally hidden from search results such as node_modules, etc.
      vim.keymap.set('n', '<leader>sF', require('telescope.builtin').find_files, { desc = '[S]earch All [F]iles' })
      vim.keymap.set('n', '<leader>sh', require('telescope.builtin').help_tags, { desc = '[S]earch [H]elp' })
      vim.keymap.set('n', '<leader>sd', require('telescope.builtin').diagnostics, { desc = '[S]earch [D]iagnostics' })
    end
  },

}
