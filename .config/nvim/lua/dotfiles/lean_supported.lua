--- Whether this Neovim is new enough for lean.nvim.
---
--- A SECOND, HIGHER FLOOR than the one init.lua enforces, which is the whole
--- reason this module exists. init.lua rejects anything below 0.10, because
--- that is where `vim.uv` arrived. lean.nvim's own MIN_SUPPORTED_NVIM is
--- 0.11.5 (lua/lean/init.lua), so 0.10.0 through 0.11.4 satisfy this config
--- and not lean.nvim.
---
--- lean.nvim only WARNS on that gap and then keeps loading:
---
---   if vim.version.lt(nvim_version, MIN_SUPPORTED_NVIM) then
---     vim.notify(..., vim.log.levels.WARN)
---
--- A warning scrolls past during startup, and what the reader gets instead is
--- a later error from an API that does not exist on their version, which
--- names neither lean.nvim nor the version. So the decision is made here and
--- the plugin is not loaded at all below the floor.
local M = {}

--- lean.nvim's MIN_SUPPORTED_NVIM, as of v2026.4.1.
M.floor = '0.11.5'

---@param version string|nil a version string, as `tostring(vim.version())` gives
---@return boolean
function M.is_supported(version)
  if type(version) ~= 'string' or version == '' then
    return false
  end
  -- `parse` has TWO failure modes and they are not the same. It raises on a
  -- non-string argument (`error(err_msg(version))`, runtime/lua/vim/version.lua)
  -- and returns nil for a string it cannot read -- measured on 0.12.4, where
  -- 'not a version' and '' both give nil. The type guard above covers the
  -- first, so the pcall is defence in depth and the nil check is what
  -- actually fires. Either way an unreadable version is unsupported, which
  -- fails towards not loading the plugin rather than loading it blind.
  local ok, parsed = pcall(vim.version.parse, version)
  if not ok or parsed == nil then
    return false
  end
  return vim.version.ge(parsed, M.floor)
end

return M
