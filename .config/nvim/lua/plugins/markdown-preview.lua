return {
  {
    'iamcco/markdown-preview.nvim',
    cmd = { 'MarkdownPreviewToggle', 'MarkdownPreview', 'MarkdownPreviewStop' },
    ft = { 'markdown' },
    -- A COMMAND STRING, not a Lua function, and that distinction is the fix.
    --
    -- lazy.nvim calls `Loader.load(plugin)` before running a build only for
    -- the string form (lazy/manage/task/plugin.lua:20-26, `B.cmd`). A Lua
    -- build function gets no such load, so the plugin's `autoload/` is not
    -- yet on runtimepath and `mkdp#util#*` cannot resolve.
    --
    -- Two failures came from getting this wrong, both measured in a bare
    -- ubuntu:24.04 container:
    --   1. Upstream's README snippet, `vim.cmd [[Lazy load markdown-preview.nvim]]`
    --      inside a Lua build, raised
    --      `Vim:E492: Not an editor command: Lazy` -- the `:Lazy` user command
    --      does not exist yet during the first bootstrap sync.
    --   2. Dropping that line and calling the function directly raised
    --      `Vim:E117: Unknown function: mkdp#util#install_sync`, because the
    --      autoload file was never sourced. So the `:Lazy load` line WAS
    --      load-bearing; it was solving the right problem the wrong way.
    --
    -- `install_sync` rather than `install`: the bare `mkdp#util#install()`
    -- routes through `mkdp#util#open_terminal` (autoload/mkdp/util.vim:153)
    -- and opens an interactive terminal split, which no bootstrap has.
    -- `install_sync` runs the same installer through `execute '!'`.
    --
    -- The installer downloads a prebuilt binary with curl (app/install.sh,
    -- wget fallback), so this needs network but no node toolchain.
    build = ':call mkdp#util#install_sync()',
  },
}
