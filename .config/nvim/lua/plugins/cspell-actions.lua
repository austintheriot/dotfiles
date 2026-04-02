return {
  {
    'kosayoda/nvim-lightbulb',
    event = 'LspAttach',
    opts = { autocmd = { enabled = false } },
    config = function(_, opts)
      require('nvim-lightbulb').setup(opts)

      vim.api.nvim_create_autocmd('LspAttach', {
        callback = function()
          local function get_cspell_actions()
            local row, col = unpack(vim.api.nvim_win_get_cursor(0))
            row = row - 1
            local actions = {}

            for _, diagnostic in ipairs(vim.diagnostic.get(0, { lnum = row })) do
              if diagnostic.source == 'cspell' and diagnostic.user_data then
                local word = diagnostic.user_data.word
                local suggestion = diagnostic.user_data.suggestion
                if col >= diagnostic.col and col < diagnostic.col + #word then
                  if suggestion then
                    table.insert(actions, {
                      title = string.format('Replace "%s" with "%s"', word, suggestion),
                      action = function()
                        local line = vim.api.nvim_buf_get_lines(0, diagnostic.lnum, diagnostic.lnum + 1, false)[1]
                        local new_line = line:sub(1, diagnostic.col) .. suggestion .. line:sub(diagnostic.col + #word + 1)
                        vim.api.nvim_buf_set_lines(0, diagnostic.lnum, diagnostic.lnum + 1, false, { new_line })
                      end,
                    })
                  end
                  table.insert(actions, {
                    title = string.format('Add "%s" to dictionary', word),
                    action = function() vim.cmd('CSpellAddWord ' .. word) end,
                  })
                end
              end
            end
            return actions
          end

          local original_code_action = vim.lsp.buf.code_action
          vim.lsp.buf.code_action = function(options)
            local cspell_actions = get_cspell_actions()
            if #cspell_actions == 0 then
              return original_code_action(options)
            end

            local params = vim.lsp.util.make_range_params()
            vim.lsp.buf_request_all(0, 'textDocument/codeAction', params, function(results)
              local all_actions = vim.deepcopy(cspell_actions)
              for client_id, result in pairs(results) do
                for _, action in ipairs(result.result or {}) do
                  table.insert(all_actions, {
                    title = action.title,
                    client_id = client_id,
                    action = function()
                      if action.edit then vim.lsp.util.apply_workspace_edit(action.edit, 'utf-8') end
                      if action.command then vim.lsp.buf.execute_command(action.command) end
                    end,
                  })
                end
              end

              if #all_actions > 0 then
                vim.ui.select(all_actions, {
                  prompt = 'Code actions:',
                  format_item = function(item) return item.title end,
                }, function(choice)
                  if choice then choice.action() end
                end)
              else
                vim.notify('No code actions available', vim.log.levels.INFO)
              end
            end)
          end

          vim.api.nvim_create_user_command('CSpellAddWord', function(cmd_opts)
            local word = cmd_opts.args ~= '' and cmd_opts.args or vim.fn.expand '<cword>'
            local config_path = vim.fn.expand '~/.config/nvim/cspell.json'
            local ok, content = pcall(vim.fn.readfile, config_path)
            if not ok then
              vim.notify('Could not read cspell.json', vim.log.levels.ERROR)
              return
            end

            local config = vim.fn.json_decode(table.concat(content, '\n'))
            config.words = config.words or {}
            if vim.tbl_contains(config.words, word) then
              vim.notify(string.format('"%s" already in dictionary', word), vim.log.levels.INFO)
              return
            end

            table.insert(config.words, word)
            table.sort(config.words)
            local json = vim.fn.system('python -m json.tool', vim.fn.json_encode(config))
            vim.fn.writefile(vim.fn.split(json, '\n'), config_path)
            vim.notify(string.format('Added "%s" to dictionary', word), vim.log.levels.INFO)

            local bufnr = vim.api.nvim_get_current_buf()
            local ns = vim.api.nvim_create_namespace 'nvim-lint'
            local filtered = vim.tbl_filter(function(d)
              return d.source ~= 'cspell' or (d.user_data and d.user_data.word ~= word)
            end, vim.diagnostic.get(bufnr, { namespace = ns }))
            vim.diagnostic.set(ns, bufnr, filtered)

            vim.defer_fn(function()
              if vim.api.nvim_buf_is_valid(bufnr) then
                require('lint').try_lint 'cspell'
              end
            end, 100)
          end, { nargs = '?', desc = 'Add word to cspell dictionary' })
        end,
      })
    end,
  },
}
