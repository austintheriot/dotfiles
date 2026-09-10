--- Whether to start treesitter highlighting for a buffer's filetype.
---
--- `has_parser` is injected so this is a pure decision: the FileType
--- callback passes a probe over the runtimepath, and a test passes a table
--- lookup. The guard exists because `language.add` succeeds for a name with
--- no parser and `treesitter.start` then errors per buffer, which is how
--- "Parser could not be created for buffer 1 and language NvimTree" shipped.
local M = {}

---@param lang string|nil the buffer's filetype
---@param has_parser fun(lang: string): boolean
---@return boolean
function M.should_start(lang, has_parser)
  if lang == nil or lang == '' then
    return false
  end
  return has_parser(lang) == true
end

return M
