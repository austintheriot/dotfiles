return {
  check = function()
    vim.health.start 'nvim config'
    vim.health.info('System: ' .. vim.inspect(vim.uv.os_uname()))

    if vim.version.ge(vim.version(), '0.10-dev') then
      vim.health.ok(string.format("Neovim version: '%s'", tostring(vim.version())))
    else
      vim.health.error(string.format("Neovim out of date: '%s'. Upgrade to latest stable", tostring(vim.version())))
    end

    for _, exe in ipairs { 'git', 'make', 'unzip', 'rg' } do
      if vim.fn.executable(exe) == 1 then
        vim.health.ok(string.format("Found: '%s'", exe))
      else
        vim.health.warn(string.format("Not found: '%s'", exe))
      end
    end
  end,
}
