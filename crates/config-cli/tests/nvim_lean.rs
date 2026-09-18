//! Lean 4 tooling: the one language whose server this repo deliberately does
//! not pin, and the guards that keep that decision honest.
//!
//! Every other language server in this config is reproducible from
//! `mason-lock.json`. Lean is not, and the reason is structural rather than an
//! omission waiting to be fixed:
//!
//!   * The Lean 4 server is `lake serve` (or `lean --server`), which are elan
//!     SHIMS. elan reads the project's own `lean-toolchain` file and
//!     dispatches to the version that project pins, the way rustup reads
//!     rust-toolchain.toml.
//!   * `.olean` build artifacts are not compatible across toolchain versions,
//!     so a globally pinned Lean would be the WRONG version for every project
//!     that disagreed with the pin. The reproducibility story belongs to the
//!     Lean project, not to this editor config.
//!
//! THE TRAP THIS FILE EXISTS FOR. mason-registry is not empty of Lean: it
//! contains `lean-language-server`, whose package.yaml declares
//! `languages: [Lean 3]` and installs `pkg:npm/lean-language-server@3.4.0`.
//! Lean 3 is a different, end-of-life language with an incompatible protocol.
//! So anyone who greps the registry for "lean", finds a hit, and wires it up
//! gets a server that installs cleanly and is wrong. That is a worse failure
//! than a missing one, and a comment alone does not stop it.

use std::path::PathBuf;

/// The repository root, resolved at run time rather than at compile time.
fn root() -> PathBuf {
    dotfiles_test_support::repo::root()
}

/// Reads a tracked file, naming it when it is absent.
fn read_tracked(relative: &str) -> String {
    let path = root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is tracked and readable: {error}", path.display()))
}

/// Lua source with its comments stripped.
///
/// Load-bearing rather than tidy, and this file is the sharpest case of it in
/// the repo: `lean.lua` EXPLAINS the Lean 3 trap in prose, naming
/// `lean-language-server` several times. A raw match would read that
/// explanation as the violation it warns against, and the assertion below
/// would fail on the very comment that prevents the bug.
fn strip_lua_comments(source: &str) -> String {
    source
        .lines()
        .map(|line| match line.find("--") {
            Some(index) => &line[..index],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The Lean 3 mason package never enters this config.
///
/// Asserted against the LOCKFILE and the ensure list rather than against
/// prose, because those are the two places a package name has to appear
/// before mason installs anything.
#[test]
fn the_lean3_mason_package_never_enters_the_config() {
    const LEAN3_PACKAGE: &str = "lean-language-server";

    let lock = read_tracked(".config/nvim/mason-lock.json");
    assert!(
        !lock.contains(LEAN3_PACKAGE),
        "mason-lock.json pins `{LEAN3_PACKAGE}`, which is the LEAN 3 server \
         (pkg:npm/lean-language-server@3.4.0, `languages: [Lean 3]`). Lean 3 is \
         a different end-of-life language with an incompatible protocol, so \
         this installs cleanly and is wrong for Lean 4. The Lean 4 server comes \
         from elan, which deps.toml tracks."
    );

    // The ensure list is built from lsp.lua, so the name must not reach it
    // through the servers table or the extra-tools list either.
    let lsp_code = strip_lua_comments(&read_tracked(".config/nvim/lua/plugins/lsp.lua"));
    assert!(
        !lsp_code.trim().is_empty(),
        "positive control: stripping comments from lsp.lua left no code, so \
         this assertion would pass vacuously"
    );
    assert!(
        !lsp_code.contains(LEAN3_PACKAGE),
        "lsp.lua names `{LEAN3_PACKAGE}`, the Lean 3 server, in code"
    );
}

/// `leanls` is not configured through the mason-lspconfig handler loop.
///
/// THE FAILURE THIS CATCHES IS A PARTIAL ONE, which is why it is asserted
/// rather than left to review. lean.nvim ships its own `lsp/leanls.lua` and
/// calls `vim.lsp.enable 'leanls'` from its `plugin/` directory. The handler
/// loop in lsp.lua passes every server its own `capabilities` table, and doing
/// that to leanls overwrites lean.nvim's
/// `capabilities.lean.silentDiagnosticSupport`, its `$/lean/fileProgress`
/// handler, its publishDiagnostics override and `init_options.hasWidgets`.
///
/// The server still attaches and diagnostics still appear. The infoview stays
/// empty. A reader looking at a working gutter has no reason to suspect the
/// capabilities table, which is what makes this worth a permanent guard.
///
/// nvim-lspconfig has no `leanls` at all (only the Lean 3 `lean3ls`), so there
/// is no correct way to add it here even deliberately.
#[test]
fn leanls_is_not_declared_in_the_mason_lspconfig_loop() {
    let lsp_code = strip_lua_comments(&read_tracked(".config/nvim/lua/plugins/lsp.lua"));
    assert!(
        !lsp_code.trim().is_empty(),
        "positive control: stripping comments from lsp.lua left no code"
    );
    for name in ["leanls", "lean3ls", "lean4ls"] {
        assert!(
            !lsp_code.contains(name),
            "lsp.lua declares `{name}`. lean.nvim owns its own LSP config and \
             enables it itself; routing it through the mason-lspconfig handler \
             loop overwrites its capabilities and handlers, which leaves the \
             server attached and the infoview permanently empty."
        );
    }
}

/// elan has an install path, because the editor execs its shims.
///
/// The same rule every linter and formatter in this config is held to: a
/// binary the config runs has to be installed by something. Without elan,
/// `vim.lsp.rpc.start` is handed a `lake` that does not resolve, the client
/// never attaches, and the infoview simply stays empty with nothing printed
/// in the UI.
#[test]
fn elan_is_a_tracked_dependency() {
    let manifest = read_tracked("deps/deps.toml");
    assert!(
        manifest.contains("[elan]"),
        "deps.toml does not track elan, so nothing installs the Lean 4 server. \
         lean.nvim execs `lake serve` / `lean --server`, which are elan shims."
    );

    // A PATH check alone would report a machine that HAS elan as missing it:
    // elan installs its shims into $HOME/.elan/bin by editing $HOME/.profile,
    // which a non-interactive shell never reads. Verified on this machine,
    // where `command -v elan` fails while $HOME/.elan/bin/elan is a working
    // 5.8MB binary. That is the same shape as the nvm and node entries.
    let elan_section = manifest
        .split("[elan]")
        .nth(1)
        .unwrap_or_default()
        .split("\n[")
        .next()
        .unwrap_or_default()
        .to_string();
    assert!(
        elan_section.contains(".elan/bin/elan"),
        "the elan entry checks only PATH, so it reports a machine that has \
         elan as missing it: elan's shims live in $HOME/.elan/bin, which a \
         non-interactive shell does not have on PATH"
    );
}

/// The health check names the binaries lean.nvim execs.
///
/// THE SILENCE THIS REPLACES. When `lake` does not resolve,
/// `vim.lsp.rpc.start` fails at the RPC layer: the client never attaches, the
/// infoview stays empty, and nothing is printed in the UI. There is no error
/// to search for, which is the worst shape a missing dependency can take.
///
/// `lake` and `lean` are asserted individually rather than via elan alone,
/// because they are elan SHIMS on $PATH. Checking elan and stopping would
/// pass on a machine whose shims are not on PATH -- which is exactly the
/// state of the shell this was written in, where `command -v elan` fails
/// while $HOME/.elan/bin/elan is a working binary.
#[test]
fn the_health_check_names_the_lean_runtimes() {
    let health = read_tracked(".config/nvim/lua/dotfiles/health.lua");
    let code = strip_lua_comments(&health);
    assert!(
        !code.trim().is_empty(),
        "positive control: stripping comments from health.lua left no code"
    );
    for exe in ["elan", "lake", "lean"] {
        assert!(
            code.contains(&format!("{exe} =")),
            "health.lua does not check for `{exe}`, so a missing Lean toolchain \
             presents as an infoview that silently never populates"
        );
    }
}

/// The Lean plugin does not pin a toolchain version, and says why.
///
/// A GUARD AGAINST A PLAUSIBLE FUTURE EDIT rather than against today's code.
/// The natural instinct, in a config where everything else is pinned, is to
/// pin Lean too. Doing so breaks every project whose `lean-toolchain`
/// disagrees, because `.olean` artifacts are not compatible across toolchain
/// versions. This asserts the decision is recorded where the edit would be
/// made.
#[test]
fn the_lean_spec_records_why_the_toolchain_is_unpinned() {
    let spec = read_tracked(".config/nvim/lua/plugins/lean.lua");
    assert!(
        spec.contains("lean-toolchain"),
        "lean.lua does not name `lean-toolchain`, so the reason the server \
         version is unpinned is not recorded where someone would go to pin it"
    );

    let code = strip_lua_comments(&spec);
    assert!(
        !code.trim().is_empty(),
        "positive control: stripping comments from lean.lua left no code"
    );
    // `opts` calls require('lean').setup(), which is marked ---@deprecated in
    // the plugin with a removal target of v2026.9.1 -- the next release after
    // the current v2026.4.1. The supported form is `vim.g.lean_config`.
    assert!(
        code.contains("vim.g.lean_config"),
        "lean.lua does not set vim.g.lean_config, which is the supported \
         configuration path; `opts` routes through the deprecated setup()"
    );
    assert!(
        !code.contains("opts ="),
        "lean.lua passes `opts`, which lazy.nvim forwards to the deprecated \
         require('lean').setup() (removal target v2026.9.1)"
    );

    // plugin/lean.lua is the one line that calls `vim.lsp.enable 'leanls'`.
    // A lazy-load that never sources plugin/ gives a plugin that looks loaded
    // and whose server never starts -- lean.nvim ships a health check
    // specifically for this ("lean.nvim's plugin files have not run"), which
    // is a fair measure of how often it happens.
    assert!(
        code.contains("BufReadPre *.lean"),
        "lean.lua does not lazy-load on a Lean buffer event, so plugin/ may \
         never source and `vim.lsp.enable 'leanls'` never runs"
    );
}

/// The Lean plugin refuses to load below lean.nvim's own floor.
///
/// TWO DIFFERENT FLOORS, which is the whole point. init.lua rejects anything
/// below 0.10, where `vim.uv` arrived. lean.nvim's MIN_SUPPORTED_NVIM is
/// 0.11.5, so 0.10.0 through 0.11.4 satisfy this config and not the plugin.
///
/// lean.nvim only WARNS in that gap and then keeps loading into API calls that
/// do not exist yet. A warning scrolls past during startup and the reader gets
/// an unrelated traceback later, naming neither the plugin nor the version.
#[test]
fn the_lean_spec_gates_on_the_plugins_own_version_floor() {
    let code = strip_lua_comments(&read_tracked(".config/nvim/lua/plugins/lean.lua"));
    assert!(
        code.contains("dotfiles.lean_supported"),
        "lean.lua does not gate on the version module, so it loads on a \
         Neovim that lean.nvim only warns about"
    );

    let module = read_tracked(".config/nvim/lua/dotfiles/lean_supported.lua");
    assert!(
        module.contains("0.11.5"),
        "the floor is not lean.nvim's MIN_SUPPORTED_NVIM (0.11.5)"
    );

    // The spec beside it is what proves the comparison is right, including
    // the prerelease and unparseable cases. Named here so deleting it is a
    // failure rather than a silence; nvim_lua_units.rs runs it.
    assert!(
        root()
            .join(".config/nvim/tests/lean_supported_spec.lua")
            .is_file(),
        "the version-floor module has no spec, so its comparison is unproven"
    );
}
