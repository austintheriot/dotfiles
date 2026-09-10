--- The linters whose command is present on this machine, in declared order.
---
--- `is_executable` is injected (the config passes `vim.fn.executable`), so
--- the filter is a pure function of its inputs. Running a linter whose
--- binary is absent errored once per buffer; skipping it is the fix that
--- shipped, and this is that fix as a testable function.
local M = {}

---@param linters { name: string, cmd: string }[]
---@param is_executable fun(cmd: string): boolean
---@return string[]
function M.runnable(linters, is_executable)
  local present = vim.tbl_filter(function(linter)
    return is_executable(linter.cmd)
  end, linters)
  return vim.tbl_map(function(linter)
    return linter.name
  end, present)
end

return M
