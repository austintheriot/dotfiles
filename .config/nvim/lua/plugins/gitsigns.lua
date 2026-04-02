-- gitsigns shows git change indicators in the sign column (the thin strip left of line numbers)
-- and provides hunk-level staging/resetting without leaving nvim

return {
  {
    'lewis6991/gitsigns.nvim',
    opts = {
      on_attach = function(bufnr)
        local gs = require 'gitsigns'
        local function map(mode, l, r, opts)
          vim.keymap.set(mode, l, r, vim.tbl_extend('force', { buffer = bufnr }, opts or {}))
        end

        -- a "hunk" is a contiguous block of changed lines
        map('n', ']c', function()
          if vim.wo.diff then vim.cmd.normal { ']c', bang = true } else gs.nav_hunk 'next' end
        end, { desc = 'Next git change' })
        map('n', '[c', function()
          if vim.wo.diff then vim.cmd.normal { '[c', bang = true } else gs.nav_hunk 'prev' end
        end, { desc = 'Previous git change' })

        -- visual mode hunk actions operate on the selected lines only
        map('v', '<leader>hs', function() gs.stage_hunk { vim.fn.line '.', vim.fn.line 'v' } end, { desc = 'Stage hunk' })
        map('v', '<leader>hr', function() gs.reset_hunk { vim.fn.line '.', vim.fn.line 'v' } end, { desc = 'Reset hunk' })
        map('n', '<leader>hs', gs.stage_hunk, { desc = 'Git stage hunk' })
        map('n', '<leader>hr', gs.reset_hunk, { desc = 'Git reset hunk' })
        map('n', '<leader>hS', gs.stage_buffer, { desc = 'Git stage buffer' })
        map('n', '<leader>hu', gs.undo_stage_hunk, { desc = 'Git undo stage hunk' })
        map('n', '<leader>hR', gs.reset_buffer, { desc = 'Git reset buffer' })
        map('n', '<leader>hp', gs.preview_hunk, { desc = 'Git preview hunk' })
        map('n', '<leader>hb', gs.blame_line, { desc = 'Git blame line' })
        map('n', '<leader>hd', gs.diffthis, { desc = 'Git diff against index' })
        map('n', '<leader>hD', function() gs.diffthis '@' end, { desc = 'Git diff against last commit' })
        map('n', '<leader>tb', gs.toggle_current_line_blame, { desc = 'Toggle git blame line' })
        map('n', '<leader>tD', gs.toggle_deleted, { desc = 'Toggle git show deleted' })
      end,
    },
  },
}
