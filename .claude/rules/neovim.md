---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-05
---

# Neovim

A reference for reviewing and advising on Neovim configuration, Lua plugin code, LSP setup, and agent-editor integration. Used by the `neovim` subagent and the `/expert-review` / `/expert-plan` / `/expert-consult` skills. Covers the vi and Vim lineage where it explains why Neovim behaves as it does.

The unifying thesis: **Neovim is an editor whose configuration is a program, running inside a single-threaded event loop, against an API that is deliberately thin over a 40-year-old C codebase.** That produces a specific failure signature. Config bugs are not usually syntax errors -- they are ordering bugs (something read a value before it was set), context bugs (an API call in a place where the API is not allowed), or index bugs (an off-by-one across the API's inconsistent 0-based and 1-based boundaries). None of those produce a stack trace pointing at the cause.

The operational question: **"when does this code run, what context does it run in, and what does it assume was already true?"**

Empirical priority, in rough order of how often it bites: **load order and timing > API context legality (fast events) > index-base and API-surface confusion > plugin-manager lazy-load correctness > LSP client lifecycle > deprecated-API drift > performance.**

## Volatile surface

`last-verified: 2026-09-05`. Neovim's Lua API is still moving; the deprecation cycle runs roughly two releases. The editing model and the ordering semantics are durable; specific function names are not.

| Claim class | Rots | Re-verify at |
|---|---|---|
| `vim.lsp.*` API shape | Fast (per release) | `:help news`, `:help deprecated` |
| Plugin recommendations and maintenance status | Fast | Repo commit recency |
| Version-gated defaults and built-in mappings | Medium | `:help news-<version>` |
| Deprecated-to-removed timing | Medium | `:help deprecated` |
| Treesitter parser ABI compatibility | Fast | `nvim-treesitter` branch docs |
| Editing model, motions, registers, text objects | Very slow | `:help` |

**Read `:help news` before advising on any version-gated API.** It is the authoritative migration document and it is in the user's own installation.

## Load order is the first thing to check

Neovim reads `init.lua` top to bottom, then sources plugins, then fires `VimEnter`. Anything that must be true before a plugin loads must be set before that plugin loads. The canonical failure:

**`mapleader` must be set before any plugin that defines a `<leader>` mapping is loaded.** Mappings capture the leader's value at definition time, not at press time. Setting `vim.g.mapleader` after `lazy.setup()` produces mappings bound to the old leader (a backslash by default) while the user presses the new one. Nothing errors. The symptom is "some of my leader mappings work and some do not," and which ones work depends on load order.

**`runtimepath` order determines which file wins.** `after/` directories load last by design, which is why `after/ftplugin/<ft>.lua` is the correct place to override a filetype setting a plugin set. Putting that override in `init.lua` means the plugin overwrites it later.

**`require` caches by module name.** Editing a Lua module and re-sourcing `init.lua` does not reload it -- `package.loaded` still holds the old table. This is why "I changed my config and nothing happened" is nearly always a caching artifact rather than an edit that failed. Restart, or explicitly clear `package.loaded['mymodule']`.

**`vim.loader`** (the bytecode cache) speeds startup and adds a second cache layer to reason about when a module appears stale.

## The option-scope trap

Four accessors, and choosing the wrong one produces settings that apply in the wrong places:

- `vim.o` -- the option as the `:set` command sees it. Usually what you want in `init.lua`.
- `vim.go` -- explicitly global.
- `vim.bo` / `vim.wo` -- explicitly buffer-local / window-local.
- `vim.opt` -- a wrapper returning an object with `:append()`, `:remove()`, and list/map semantics for options like `listchars` and `wildignore`.

The trap is **buffer-local options set globally in `init.lua`**. Setting `vim.bo.shiftwidth` at startup sets it for whatever buffer happens to exist then, not for buffers created later. Per-filetype settings belong in `after/ftplugin/<ft>.lua` or a `FileType` autocmd, not in the main config. The symptom is an indent setting that works in the first file opened and nowhere else.

## Fast events: where the API is illegal

Neovim runs a single-threaded event loop. Some callbacks fire in a restricted context -- **`api-fast`** -- where most of the API cannot be called. `:help` states it directly: it is an error to invoke `vim.api` functions other than the `api-fast` subset from these contexts.

This affects `vim.uv` (libuv) callbacks -- timers, filesystem watchers, process handles -- and some autocmd callbacks. The fix is `vim.schedule()` or `vim.schedule_wrap()`, which defers the function to the main loop.

The failure is nasty because it is **intermittent**: a timer callback that only sometimes touches the API only sometimes errors, and the error text ("E5560: nvim_buf_set_lines must not be called in a fast event context") names the symptom rather than the callback that scheduled it. Any `vim.uv` callback that touches buffers, windows, or options should be `vim.schedule_wrap`ped by default.

## Index bases: the API's least forgivable inconsistency

There is no single convention. The ones that matter:

- `nvim_win_get_cursor()` returns `{row, col}` with **1-based row and 0-based column**. In one return value.
- `nvim_buf_get_lines(buf, start, end, strict)` is **0-based, end-exclusive**.
- `nvim_buf_set_text()` uses 0-based rows and columns.
- Extmarks are 0-based.
- `vim.fn.line()`, `vim.fn.col()` and most `vim.fn.*` functions are **1-based**, because they are the Vimscript functions.
- Columns in the API are **byte** offsets, not character offsets -- so any multibyte text makes column arithmetic wrong unless you convert.

Mixing `vim.fn` and `vim.api` in one calculation without an explicit conversion is the single most common source of off-by-one bugs in plugin code. The convention worth adopting: convert at the boundary, comment the base, and never let a 1-based and a 0-based value meet in the same expression.

## `vim.api` versus `vim.fn` versus `vim.cmd`

- **`vim.api`** -- the msgpack-RPC API. Fastest, most explicit, best-typed. Prefer it.
- **`vim.fn`** -- calls Vimscript builtins. Necessary for things with no API equivalent; slower, and the conversion of Lua values to Vimscript types is where `nil` versus `vim.NIL` versus `vim.empty_dict()` confusion bites. An empty Lua table is ambiguous between a list and a dict; `vim.empty_dict()` disambiguates.
- **`vim.cmd`** -- executes Ex commands. The escaping hazard: `vim.cmd('edit ' .. path)` breaks on any path containing a space or a special character. Use `vim.cmd.edit(path)` (the table form, which escapes) or the API equivalent.

`vim.NIL` matters increasingly at the LSP boundary: **as of 0.12, JSON `null` in LSP messages is represented as `vim.NIL` rather than `nil`**, while genuinely missing fields remain `nil`. Code that used `if result.field == nil` to mean "absent or null" now needs to distinguish them.

## Autocmds and their callback contract

`nvim_create_autocmd` takes either a Vimscript `command` or a Lua `callback`. Two contract details that surprise people:

- **Returning `true` from a callback deletes the autocmd.** Accidentally returning a truthy value from the last expression removes the autocmd after one fire. The symptom is an autocmd that works exactly once.
- **Autocmd groups must be cleared or they accumulate.** Re-sourcing a config without `clear = true` on the group registers duplicate autocmds, so the callback runs two, three, four times. `vim.api.nvim_create_augroup('MyGroup', { clear = true })` is the standard idiom for a reason.

`LspAttach` is the modern pattern for buffer-local LSP setup, replacing per-server `on_attach` callbacks. It fires once per client per buffer and gives the client id in `args.data.client_id`.

## LSP: the 0.11 transition

Neovim 0.11 restructured LSP configuration and this is the most consequential recent change for user configs.

**`vim.lsp.config()`** defines configurations for servers, and configurations can additionally live in `lsp/<name>.lua` on the runtimepath. **`vim.lsp.enable()`** enables them. `vim.lsp.is_enabled()` checks. This makes `nvim-lspconfig` a provider of configuration data rather than a required framework -- the plugin still ships the per-server defaults, but the wiring is now built in.

Other 0.11 LSP additions worth knowing: `vim.lsp.foldexpr()` implements LSP folding; `vim.lsp.Config` gained `workspace_required`; `root_markers` can be ordered by priority; the function form of `cmd` receives the resolved config as a second argument.

**Default LSP mappings arrived in 0.11** and configs that define their own now conflict or duplicate: `grn` rename, `grr` references, `gri` implementation, `gra` code action (normal and visual), `grt` type definition, `gO` document symbol, and `CTRL-S` signature help in insert and select mode.

**Diagnostics**: `virtual_lines` was added as a diagnostic handler in 0.11, rendering diagnostics on their own lines rather than as trailing `virtual_text`. In 0.12, `vim.diagnostic.disable()` and `is_disabled()` were **removed** (deprecated in 0.10), diagnostic signs can no longer be configured via `:sign-define`, and the legacy `vim.diagnostic.enable()` signature is gone. Configs carrying 0.9-era diagnostic code break outright on 0.12.

**Other 0.12 LSP breaks**: `vim.lsp.semantic_tokens` `start()`/`stop()` renamed to `enable()`; `client.attached_buffers[buf]` now stores a `languageId` string rather than a boolean; `vim.lsp.log.set_format_func()` receives all arguments for a log entry rather than individual ones.

**Formatting conflicts** are a recurring practical problem: an LSP server that formats and a dedicated formatter (via `conform.nvim`) both bound to the same trigger produce fights, or double-formatting, or a race on save. Pick one owner per filetype.

## Plugin managers

- **lazy.nvim** (folke) is the dominant choice. Declarative specs, lockfile, lazy-loading by `event`, `ft`, `cmd`, `keys`, and a dependency graph.
- **vim.pack** is **built into Neovim 0.12**: `vim.pack.add()` in `init.lua`, `vim.pack.update()` to fetch, `vim.pack.del()` to remove, `vim.pack.get()` to enumerate, with a lockfile target and an `offline` option. It handles source specs like `gh:user/plugin`. This meaningfully changes the "which manager" question, since the answer can now be "none."
- **mini.deps** (echasnovski) is the minimal third-party option; **rocks.nvim** uses LuaRocks; **packer.nvim** is unmaintained; **vim-plug** is Vimscript-era and still works.

**Lazy-loading is where correctness goes wrong.** A plugin lazy-loaded on an event that fires before its dependency is loaded, or on `ft` when it also needs to register a command available before any file opens, produces "works after I open a file, not before." Colorscheme plugins lazy-loaded on an event fire after the initial screen paint, producing a visible flash. Plugins providing a `vim.ui` override must load before anything calls `vim.ui.select`. **Never mix two plugin managers** -- both will manage runtimepath and both will be wrong.

## Treesitter

`nvim-treesitter` has a **`master` and a `main` branch with different APIs**, and the migration has been the largest ongoing source of config breakage in the ecosystem. Configs written against one branch do not work on the other.

**Parser ABI compatibility** is version-gated: a parser compiled against one Neovim's tree-sitter ABI fails on another. Updating Neovim without recompiling parsers, or vice versa, produces "invalid node type" errors or silently missing highlights. Query files (`highlights.scm` and friends) also break when a grammar changes capture names.

Treesitter highlighting, indentation, and folding are separate features with separate maturity. Indentation in particular is still experimental for many languages, and enabling it project-wide is a common cause of "my indenting got worse."

## Editing model, briefly

The reason to use this editor. Worth knowing well enough to recognize when a plugin is reimplementing it:

- **Operator-pending grammar**: verb plus motion or text object. `d`, `c`, `y` compose with `iw`, `ap`, `i(`, `t,`. Custom text objects extend the grammar; custom mappings that break `d` composition are a regression.
- **Registers**: named `a`-`z`, append with uppercase, the numbered ring `"0`-`"9`, the yank register `"0`, the black hole `"_`, the system clipboard `"+` and `"*`.
- **Macros** are register contents; `q` records, `@` replays, and `@@` repeats. Editing a macro means editing text.
- **`:global`** (`:g/pattern/cmd`) with `:normal` is the batch-editing power tool: `:g/TODO/normal dd`.
- **Quickfix-driven workflow**: `:vimgrep` or `:grep` populates the quickfix list; `:cdo` and `:cfdo` run a command on every entry or every file. This is the built-in project-wide refactor.
- **Marks and the jumplist**: `` `. `` last change, `` `` `` last jump, `CTRL-O` / `CTRL-I` navigate.
- **Persistent undo** (`undofile`) survives restarts and is undersold. The undo *tree* (`:undolist`, `g-`, `g+`) is not linear history.
- **`:help` navigation is a first-class skill.** `CTRL-]` follows a tag, `CTRL-O` goes back, `:helpgrep` searches all of it.

## Agents, RPC, and external edits

Neovim's RPC surface is what makes agent integration possible:

- **`nvim --listen /path/to/sock`** starts a server; **`$NVIM`** holds the socket path inside a `:terminal` buffer, so a process launched there can talk back to its own editor.
- **`nvim --server <addr> --remote-send <keys>`** injects keystrokes; **`--remote-expr`** evaluates and returns.
- **`nvim --headless -c '...'`** runs without a UI, which is the automation and testing entry point.

**The file-changed-on-disk problem is the central integration hazard.** When an external agent edits a file Neovim has open, the buffer does not update automatically. `autoread` only takes effect when Neovim checks, and it checks on certain events -- so the practical fix is a `FocusGained` / `BufEnter` / `CursorHold` autocmd calling `:checktime`. Without it, the user edits a stale buffer and their next `:w` silently reverts the agent's work. This is the single most damaging failure in agent-editor integration, and it destroys work rather than merely annoying.

For terminal-embedded agents, note that a `:terminal` buffer is a real buffer with its own mode handling, and mappings that make sense in normal buffers frequently break terminal input.

**Testing headlessly**: `plenary.nvim`'s busted-style harness (`PlenaryBustedDirectory`), `mini.test`, and `vusted` all run under `nvim --headless`. A config or plugin with no headless test has no regression protection across Neovim upgrades, which in this ecosystem is the main source of breakage.

## Schools of thought

### Distribution versus from-scratch configuration

**The distribution position** (LazyVim, AstroNvim, NvChad, and kickstart.nvim as the middle path). Starting from zero means months assembling what a distribution gives immediately, and most of those months are spent rediscovering the same plugin set. A distribution is a working baseline that can be overridden incrementally. kickstart.nvim in particular is explicitly a single readable file meant to be read and modified, not a framework.

**The from-scratch position.** You cannot debug what you did not build. A distribution's abstraction layers mean a broken keymap requires understanding both Neovim and the distribution's override mechanism, and the error messages point into someone else's Lua. The config is a program you will maintain for years; understanding it is the point, and the time spent is training rather than overhead.

Both camps agree on one thing worth stating: **copying config blind is the worst option**, because it produces neither understanding nor a maintained baseline.

### Modal editing philosophy versus IDE platform

**The editing-model position** (Drew Neil's *Practical Vim*, tpope, romainl). The value is the composable grammar -- operators, motions, text objects, registers, macros. Plugins that add IDE features often add them by breaking composition, and the user who installs forty plugins to get an IDE would be better served by an IDE. Most of what people install plugins for is already in `:help`.

**The IDE-platform position.** Neovim's built-in LSP, treesitter, and DAP integration exist because editing text is not the whole job. Refusing completion, diagnostics, and structural navigation on principle is asceticism, not efficiency. The editing model survives intact underneath.

**The minimalist-tooling position** (echasnovski's mini.nvim, romainl). A third stance: use the platform features but reject the plugin sprawl, preferring small composable modules and built-ins over large frameworks with their own configuration languages.

### The selection-first critique

Kakoune and Helix invert Vim's grammar: **select first, then act** (`object-verb` rather than `verb-object`). The argument is that Vim's model asks you to commit to an operator before you can see what it will affect, so `d2f)` is guesswork until it happens, whereas selection-first shows the selection and makes the operation a confirmation. Multiple cursors follow naturally from selections and awkwardly from operators.

The Vim counter-argument is that operator-pending composition is what makes the grammar extensible and makes `.` meaningful: a repeatable atomic operation requires the operation to be a unit, and selection-first fragments it into two steps that repeat badly.

This one is genuinely unresolved, and worth knowing because Helix's growth is a real signal rather than a fad.

### Startup time

**The optimization position.** Startup time is felt every session and compounds; lazy-loading is nearly free and 20ms versus 300ms is the difference between an editor and an application.

**The false-economy position.** Aggressive lazy-loading trades a measurable millisecond win for unmeasurable correctness risk -- load-order bugs, plugins that need to be present before an event fires, colorscheme flashes. Most users open Neovim once and keep it open for a day. Optimizing the once is the wrong target.

## Anti-pattern catalog

| Pattern | Trigger | Consequence | Fix |
|---|---|---|---|
| `mapleader` set after plugin load | Natural top-down config ordering | Mappings bind the old leader; some work, some do not, no error | Set `vim.g.mapleader` at the very top, before any plugin loads |
| API call in a fast event | `vim.uv` timer or watcher callback touching a buffer | Intermittent E5560; error names the symptom, not the cause | `vim.schedule_wrap` any uv callback that touches editor state |
| Mixing `vim.fn` and `vim.api` indices | Cursor arithmetic across both surfaces | Silent off-by-one, worse on multibyte text (columns are bytes) | Convert at the boundary; never mix bases in one expression |
| `vim.cmd('edit ' .. path)` | String-building an Ex command | Breaks on spaces and special characters | `vim.cmd.edit(path)` or the API equivalent |
| Autocmd group without `clear = true` | Re-sourcing config during iteration | Duplicate autocmds; callbacks fire N times | Always `nvim_create_augroup(name, { clear = true })` |
| Truthy return from an autocmd callback | Last expression happens to return a value | Autocmd deletes itself after one fire | Return nothing explicitly |
| Buffer-local option set in `init.lua` | `vim.bo.shiftwidth = 2` at startup | Applies to the startup buffer only | `after/ftplugin/<ft>.lua` or a `FileType` autocmd |
| Two plugin managers | Migration left the old one installed | Both manage runtimepath; conflicts and double-loads | Remove one completely, including its data directory |
| Colorscheme lazy-loaded on an event | Uniform lazy-loading policy | Visible flash before the theme applies | Load the colorscheme eagerly |
| No `:checktime` on focus | Default config plus an external agent | Stale buffer; the next `:w` silently reverts the agent's edits | `FocusGained`/`BufEnter` autocmd calling `checktime`, with `autoread` |
| LSP and formatter both formatting | Adding conform.nvim without disabling LSP formatting | Fights, double-formats, or a save-time race | One formatting owner per filetype |
| 0.9-era diagnostic API | Config predating 0.10 deprecations | Hard break on 0.12 -- `disable()`/`is_disabled()` removed | Migrate to the current `vim.diagnostic.enable()` signature |
| Custom mappings shadowing 0.11 defaults | Config predating the built-in LSP mappings | Duplicate or conflicting `gr*` bindings | Check `:help news-0.11` defaults before defining |
| Editing a module and re-sourcing | Expecting a reload | `package.loaded` caches; the edit appears to do nothing | Restart, or clear the module from `package.loaded` |
| Treesitter branch mismatch | Following a tutorial written for the other branch | Config does not apply; errors reference unknown functions | Match the branch the config targets; they are not compatible |

## Severity rubric for this lens

- **blocker** -- data loss or a hard break: missing `checktime` handling where an external agent edits files, a destructive autocmd, config that fails to load on the target version, an API call that errors on every startup.
- **major** -- broken behavior a user will hit routinely: leader ordering, fast-event violations, removed-API usage on the target version, duplicate autocmds, formatting conflicts.
- **minor** -- works but fragile: index-base mixing not yet triggered, escaping hazards on paths that happen to be clean, lazy-load specs that work by luck.
- **nit** -- style: option-setting idiom, config file organization, naming.
- **insight** -- a structural reframe: this plugin duplicates a built-in; this belongs in `after/ftplugin`; this config no longer needs a plugin manager on 0.12; this workflow wants quickfix rather than a plugin.

## Authorities

- **`:help`** -- the primary source, and in the user's own installation. `:help lua-guide` is the config-writing orientation; `:help news` is the migration authority for every version boundary; `:help vim_diff` documents where Neovim departs from Vim; `:help deprecated` lists what is on its way out.
- **Drew Neil**, *Practical Vim* and *Modern Vim* -- the canonical treatment of the editing model. *Practical Vim* is still the best argument for the grammar.
- **tpope** -- vim-surround, vim-fugitive, vim-repeat, vim-commentary. The design lesson is composition: his plugins extend the grammar rather than bypassing it, and `vim-repeat` exists specifically so plugin operations remain `.`-repeatable.
- **romainl** -- the "you probably do not need that plugin" school, and consistently good at pointing to the built-in that already does the job.
- **TJ DeVries** -- Neovim core contributor, telescope.nvim and plenary.nvim, kickstart.nvim. The clearest explainer of the Lua API and of what the core team is doing and why.
- **folke** -- lazy.nvim, which.key.nvim, snacks.nvim, noice.nvim. Sets much of the current ecosystem's convention.
- **Evgeni Chasnovski (echasnovski)** -- mini.nvim. The strongest working argument for small composable modules over frameworks, and mini.test is a serious testing option.
- **Neovim's `runtime/lua/vim/` source** -- when `:help` is ambiguous, the implementation is right there and readable.

## Changelog

- **2026-09-05** -- Initial version. Verified against Neovim's own `:help` files fetched across the 0.10, 0.11, 0.12, and master branches: the 0.11 `vim.lsp.config`/`vim.lsp.enable` introduction and default `gr*` mappings, `virtual_lines` diagnostics, the 0.12 removal of `vim.diagnostic.disable`/`is_disabled` and `:sign-define` diagnostic signs, `vim.NIL` for LSP JSON null, the semantic-tokens rename, and `vim.pack` as a built-in plugin manager with its `add`/`update`/`del`/`get` surface. The fast-event restriction and index-base inconsistencies are quoted from the API documentation. Plugin ecosystem maintenance status was not individually re-verified for every named plugin and rots fastest.
