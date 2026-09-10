--- The `ensure_installed` list for mason-tool-installer, from the lockfile.
---
--- Joins three inputs so no second hand-written table can drift: the names
--- the config declares (lspconfig names for servers, mason names for tools),
--- the registry's lspconfig-to-mason mapping, and the pinned versions.
---
--- A pinned tool is a `{ name, version = ... }` TABLE, never `name@version`:
--- mason-tool-installer destructures item[1] and item.version and passes the
--- name straight to the registry, which does not parse a suffix. The string
--- form failed at first launch with `Cannot find package "rust-analyzer@..."`.
local M = {}

---@param names string[]
---@param mason_names table<string, string> lspconfig name -> mason package
---@param lock_packages table<string, string> mason package -> version
---@return (string|{ [1]: string, version: string })[]
function M.ensure_list(names, mason_names, lock_packages)
  return vim.tbl_map(function(name)
    local package_name = mason_names[name] or name
    local version = lock_packages[package_name]
    if version then
      return { package_name, version = version }
    end
    return package_name
  end, names)
end

return M
