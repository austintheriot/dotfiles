return {
  {
    'mfussenegger/nvim-lint',
    event = { 'BufReadPre', 'BufNewFile' },
    config = function()
      local lint = require 'lint'

      lint.linters.cspell = {
        cmd = 'cspell',
        stdin = false,
        append_fname = true,
        args = { '--no-progress', '--no-summary', '--no-color' },
        stream = 'both',
        ignore_exitcode = true,
        env = nil,
        parser = function(output, bufnr)
          local diagnostics = {}
          for line in output:gmatch '[^\r\n]+' do
            local _, lnum, col, message = line:match '([^:]+):(%d+):(%d+)%s*%-%s*(.+)'
            if lnum and col and message then
              local word = message:match 'Unknown word %(([^)]+)%)'
              local suggestion = message:match 'fix: %(([^)]+)%)'
              local full_message = word and (suggestion
                and string.format('Unknown word "%s" - did you mean "%s"?', word, suggestion)
                or string.format('Unknown word "%s"', word)) or message

              table.insert(diagnostics, {
                bufnr = bufnr,
                lnum = tonumber(lnum) - 1,
                col = tonumber(col) - 1,
                message = full_message,
                severity = vim.diagnostic.severity.HINT,
                source = 'cspell',
                user_data = word and { word = word, suggestion = suggestion } or nil,
              })
            end
          end
          return diagnostics
        end,
      }

      local cspell_fts = {
        'javascript', 'javascriptreact', 'typescript', 'typescriptreact',
        'python', 'lua', 'rust', 'go', 'c', 'cpp', 'java',
        'html', 'css', 'scss', 'json', 'yaml', 'toml',
        'gitcommit', 'text', 'vim', 'sh', 'bash', 'zsh',
      }
      lint.linters_by_ft = { markdown = { 'markdownlint', 'cspell' } }
      for _, ft in ipairs(cspell_fts) do
        lint.linters_by_ft[ft] = { 'cspell' }
      end

      vim.api.nvim_create_autocmd({ 'BufEnter', 'BufWritePost', 'InsertLeave' }, {
        group = vim.api.nvim_create_augroup('lint', { clear = true }),
        callback = function() lint.try_lint() end,
      })
    end,
  },
}
