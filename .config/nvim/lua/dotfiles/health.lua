return {
  check = function()
    vim.health.start 'nvim config'

    -- Version first, and via the compatibility spelling. `vim.uv` arrived in
    -- 0.10, so reporting the system through it above this check crashed
    -- :checkhealth on precisely the out-of-date versions the check exists to
    -- report.
    if vim.version.ge(vim.version(), '0.10-dev') then
      vim.health.ok(string.format("Neovim version: '%s'", tostring(vim.version())))
    else
      vim.health.error(string.format("Neovim out of date: '%s'. This config needs 0.10 or newer", tostring(vim.version())))
    end

    local uv = vim.uv or vim.loop
    vim.health.info('System: ' .. vim.inspect(uv.os_uname()))

    for _, exe in ipairs { 'git', 'make', 'unzip', 'rg' } do
      if vim.fn.executable(exe) == 1 then
        vim.health.ok(string.format("Found: '%s'", exe))
      else
        vim.health.warn(string.format("Not found: '%s'", exe))
      end
    end

    -- Mason shells out to these rather than building anything itself, so a
    -- missing one fails every package that needs it at once. Without this
    -- block that arrives as several identical "failed to install" lines with
    -- no stated cause, which is the experience this check exists to replace.
    vim.health.start 'nvim config: mason runtimes'
    for exe, needed_by in pairs {
      node = 'npm-backed servers (eslint-lsp, typescript-language-server, css-variables-language-server)',
      npm = 'the same npm-backed servers; ships with node',
    } do
      if vim.fn.executable(exe) == 1 then
        vim.health.ok(string.format("Found: '%s'", exe))
      else
        vim.health.error(string.format("Not found: '%s' -- required by %s. Install a node version through nvm: nvm install --lts", exe, needed_by))
      end
    end
  end,
}
