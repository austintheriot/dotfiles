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
      vim.health.error '$VIMRUNTIME is unset, so no runtime file can be found'
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
      vim.health.error(string.format("runtime files are not reachable through runtimepath (VIMRUNTIME='%s')", runtime))
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

    -- Lean is the one language server this config does not pin, so it is the
    -- one whose absence has to be reported rather than inferred from the
    -- lockfile. lean.nvim starts `lake serve` (or `lean --server`), and when
    -- those do not resolve the client simply never attaches: the infoview
    -- stays empty and NOTHING is printed in the UI. This section is the
    -- difference between that silence and a named cause.
    --
    -- WARN, not ERROR, and the distinction is deliberate: node is required by
    -- servers this config installs unconditionally, while Lean tooling only
    -- matters to someone editing Lean. A machine with no Lean is correct.
    vim.health.start 'nvim config: lean'
    local lean_version = vim.version.parse(require('dotfiles.lean_supported').floor)
    if vim.version.ge(vim.version(), lean_version) then
      vim.health.ok(string.format('Neovim is new enough for lean.nvim (needs %s)', require('dotfiles.lean_supported').floor))
    else
      -- The plugin is not loaded at all in this case, by lean.lua's own
      -- guard, so say so rather than leaving the reader to wonder why a
      -- plugin they installed does nothing.
      vim.health.warn(
        string.format(
          "Neovim '%s' is below lean.nvim's floor of %s, so the Lean plugin is not loaded",
          tostring(vim.version()),
          require('dotfiles.lean_supported').floor
        )
      )
    end

    -- `lake` is what lean.nvim actually execs for a project with a lakefile,
    -- and it is an elan SHIM rather than a real binary. Checking elan alone
    -- would pass on a machine whose shims are not on PATH, which is the
    -- state that breaks the server.
    for exe, needed_by in pairs {
      elan = 'the Lean toolchain manager, which provides the two below',
      lake = 'lean.nvim in a project with a lakefile (`lake serve`)',
      lean = 'lean.nvim for a standalone Lean file (`lean --server`)',
    } do
      if vim.fn.executable(exe) == 1 then
        vim.health.ok(string.format("Found: '%s'", exe))
      else
        vim.health.warn(
          string.format(
            "Not found: '%s' -- needed by %s. Install elan (`config deps install`), "
              .. 'then ensure $HOME/.elan/bin is on PATH. The Lean version itself comes '
              .. "from each project's lean-toolchain file, not from this config.",
            exe,
            needed_by
          )
        )
      end
    end

    -- A SECOND CAUSE OF THE SAME SYMPTOM, which is the only reason this block
    -- is worth its length. When a project's lean-toolchain names a version
    -- that is not installed, elan downloads it (hundreds of megabytes) on
    -- first open. Measured 2026-09-18: `info: downloading ...` and
    -- `info: installing ...` both go to STDERR, which Neovim's LSP client
    -- discards, and stdout stays empty until the download finishes.
    --
    -- So the reader sees an empty infoview and no message -- exactly what a
    -- missing elan looks like, with a different cause and a different fix
    -- (wait, rather than install anything). Listing the installed toolchains
    -- is what lets someone tell the two apart, by comparing this list against
    -- the lean-toolchain file in the project they opened.
    if vim.fn.executable 'elan' == 1 then
      local toolchains = vim.fn.systemlist 'elan toolchain list'
      if vim.v.shell_error == 0 and #toolchains > 0 then
        vim.health.info('Installed Lean toolchains: ' .. table.concat(toolchains, ', '))
      else
        -- `executable` passes on a shim whose toolchain directory was
        -- removed, so the binary existing is not the same as elan working.
        vim.health.warn 'elan is on PATH but lists no toolchains, so no Lean version is installed yet'
      end
      vim.health.info(
        'A project whose lean-toolchain names a version not listed above makes elan download it '
          .. 'on first open. That download reports only on stderr, which Neovim discards, so the '
          .. 'infoview stays empty with no message until it finishes.'
      )
    end
  end,
}
