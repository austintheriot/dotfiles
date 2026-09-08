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

    -- Can Neovim find its own runtime? Reported 2026-09-08 as a require()
    -- traceback listing upstream's build-machine paths plus
    -- `E484: Can't open file .../syntax/syntax.vim`.
    --
    -- The cause was an install that copied `bin/nvim` out of the release
    -- tarball and discarded `share/nvim/runtime/` beside it. Neovim finds
    -- $VIMRUNTIME by walking up from its own executable, so the orphaned
    -- binary searched a directory nothing had created.
    --
    -- Checked here because every other signal available to a reader passes on
    -- that install: `nvim --version` prints normally, the deps manifest check
    -- (`command = "nvim"`) resolves, and the version guard above is satisfied.
    -- The first symptom was a wall of Lua stack traces on an unrelated action.
    --
    -- KNOWN LIMIT, measured rather than assumed. This block catches a
    -- PARTIALLY broken runtime (a runtimepath that no longer reaches the
    -- files, a moved or half-extracted tree). It cannot catch a FULLY
    -- orphaned binary: with no runtime at all, Neovim fails to load its own
    -- Lua standard library, so `require 'dotfiles.health'` raises before
    -- this function is ever called and :checkhealth reports nothing.
    -- Verified in ubuntu:24.04 against a binary copied away from its
    -- share/ tree -- the traceback is upstream's `vim/_init_packages:71`,
    -- which is exactly what the 2026-09-08 report showed.
    -- The assertion that survives that case lives outside the process, in
    -- crates/config-cli/tests/nvim_runtime.rs.
    local runtime = vim.env.VIMRUNTIME or ''
    if runtime == '' then
      vim.health.error('$VIMRUNTIME is unset, so no runtime file can be found')
    elseif vim.fn.isdirectory(runtime) ~= 1 then
      vim.health.error(
        string.format(
          "$VIMRUNTIME points at '%s', which is not a directory. The nvim binary is "
            .. 'probably separated from its share/nvim/runtime tree -- reinstall with '
            .. '`config install`',
          runtime
        )
      )
    elseif #vim.api.nvim_get_runtime_file('syntax/syntax.vim', false) == 0 then
      -- Asked through nvim_get_runtime_file rather than by joining paths,
      -- because that is the same lookup every `require` and `:syntax on`
      -- performs. A directory that exists but is not on runtimepath fails
      -- here and would pass a plain isdirectory check.
      vim.health.error(
        string.format("runtime files are not reachable through runtimepath (VIMRUNTIME='%s')", runtime)
      )
    else
      vim.health.ok(string.format("Runtime files found: '%s'", runtime))
    end

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
