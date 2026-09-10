local M = {}
local failures = 0
-- print, not io.stderr:write: selene's lua51 std does not model the stderr
-- handle, and the Rust driver captures both streams anyway.
function M.eq(actual, expected, label)
  if vim.deep_equal(actual, expected) then
    return
  end
  failures = failures + 1
  print(('FAIL %s\n  expected: %s\n  actual:   %s'):format(label, vim.inspect(expected), vim.inspect(actual)))
end
function M.finish(name)
  if failures > 0 then
    print(('%s: %d failed'):format(name, failures))
    os.exit(1)
  end
  print(name .. ': ok')
end
return M
