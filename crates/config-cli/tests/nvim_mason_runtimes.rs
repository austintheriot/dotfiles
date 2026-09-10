//! Can a fresh bootstrap actually install the Neovim tooling?
//!
//! THE FAILURE THIS EXISTS TO CATCH, observed on a real first run:
//!
//!   eslint-lsp: failed to install
//!   css-variables-language-server: failed to install
//!   typescript-language-server: failed to install
//!   stylua: failed to install
//!
//! Mason does not build those itself. It shells out to whatever the
//! package's registry entry names: npm for a `pkg:npm/...` entry, a release
//! download for a `pkg:github/...` one. When the runtime is missing, every
//! package that needs it fails at once, which is why the errors arrive as a
//! block rather than singly.
//!
//! The bootstrap tracks `nvm`, and `nvm` alone satisfies nothing. Its check
//! is `[ -s "$HOME/.nvm/nvm.sh" ]`, which passes with no node version
//! installed at all. So a freshly bootstrapped machine has nvm, no npm, and
//! every npm-backed Mason package fails on first launch.
//!
//! WHAT IS ASSERTED HERE IS THE CONTRACT rather than a live install:
//!
//!   1. Every runtime the ensure_installed list needs is a tracked
//!      dependency, so `config install` puts it on the machine.
//!   2. The nvim health check names those runtimes, so `:checkhealth` says
//!      which one is missing instead of leaving the reader with four
//!      identical "failed to install" lines and no cause.
//!
//! Running Mason for real is deliberately NOT what this does. That needs the
//! network, a writable share directory, and minutes of wall clock, and it
//! would fail for reasons that have nothing to do with this repo. The
//! contract is the part this repo controls.

use std::collections::BTreeSet;
use std::path::PathBuf;

/// The repository root, resolved at run time rather than at compile time.
fn root() -> PathBuf {
    dotfiles_test_support::repo::root()
}

/// Reads a tracked file, naming it when it is absent.
///
/// A missing tracked file is a failure rather than a skip: every path this
/// suite reads is committed, so absence means the checkout is broken.
fn read_tracked(relative: &str) -> String {
    let path = root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()))
}

/// Lua source with its comments stripped.
///
/// Load-bearing rather than tidy. Several of the files below EXPLAIN a bug
/// in a comment above the code that fixes it, and a raw match reads that
/// prose as the violation it is describing. The alacritty and workflow
/// suites document the same trap, and it is how three vacuous assertions
/// survived in this repo.
fn strip_lua_comments(source: &str) -> String {
    source
        .lines()
        .map(|line| match line.find("--") {
            Some(index) => &line[..index],
            None => line,
        })
        .collect::<Vec<&str>>()
        .join("\n")
}

/// The `.config/nvim/lua` directory.
fn nvim_lua() -> PathBuf {
    root().join(".config/nvim/lua")
}

/// The health module Neovim would actually run, if there is one.
///
/// Neovim discovers health modules by globbing `lua/**/health.lua` on the
/// runtimepath and naming each one after its PARENT directory, which is what
/// `:checkhealth <name>` takes. A module at `lua/health.lua` has no parent
/// directory inside `lua/`, so it has no name and is never run. It fails
/// SILENTLY, because a health check that is never invoked reports nothing at
/// all. That is what this repo shipped: `init.lua` did `require "health"`,
/// which returns the table and registers nothing.
///
/// So the search starts one level down, which is exactly the shape Neovim
/// requires.
fn health_module() -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    let Ok(entries) = std::fs::read_dir(nvim_lua()) else {
        return None;
    };
    for entry in entries.filter_map(Result::ok) {
        let directory = entry.path();
        if directory.is_dir() {
            let candidate = directory.join("health.lua");
            if candidate.is_file() {
                found.push(candidate);
            }
        }
    }
    found.sort();
    found.into_iter().next()
}

/// Every dependency name the manifests declare, across all three files.
///
/// Comments are stripped before the section headers are matched, for the
/// reason `strip_lua_comments` documents: a commented-out `[foo]` section
/// would otherwise read as a tracked dependency.
fn manifest_names() -> BTreeSet<String> {
    let deps = root().join("deps");
    let mut names = BTreeSet::new();
    for file in ["deps.toml", "deps-mac.toml", "deps-linux.toml"] {
        let Ok(text) = std::fs::read_to_string(deps.join(file)) else {
            continue;
        };
        for line in text.lines() {
            let code = match line.find('#') {
                Some(index) => &line[..index],
                None => line,
            };
            let trimmed = code.trim_end();
            if let Some(name) = trimmed.strip_prefix('[').and_then(|rest| rest.strip_suffix(']'))
                && !name.is_empty()
                && name
                    .chars()
                    .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-')
                && name.starts_with(|character: char| character.is_ascii_lowercase())
            {
                names.insert(name.to_string());
            }
        }
    }
    names
}

/// The LSP server names `lsp.lua` declares.
///
/// Anchored to the server table's OWN indentation (8 spaces), not to `^ *`.
/// A loose pattern matched every `key = ...` line at any depth, so
/// `rust_analyzer`'s nested `settings = {` was extracted as if it were an
/// LSP server name. That put a non-server into the list every assertion
/// below reads, and the count guard did not notice, because the list was too
/// long rather than too short.
fn servers(lsp_config: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut inside = false;
    for line in lsp_config.lines() {
        if line.contains("local servers = {") {
            inside = true;
            continue;
        }
        if inside && line == "      }" {
            break;
        }
        if inside && let Some(name) = server_shaped_key(line, "        ") {
            names.insert(name);
        }
    }
    names
}

/// A `<indent><name> = ...` key, when the line has exactly that indent.
fn server_shaped_key(line: &str, indent: &str) -> Option<String> {
    let rest = line.strip_prefix(indent)?;
    if rest.starts_with(char::is_whitespace) {
        return None;
    }
    let (name, _) = rest.split_once(" = ")?;
    let valid = !name.is_empty()
        && name.starts_with(|character: char| character.is_ascii_lowercase() || character == '_')
        && name
            .chars()
            .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_');
    valid.then(|| name.to_string())
}

/// The extra (non-server) tools `lsp.lua` adds to the ensure list.
///
/// Anchored on `vim.tbl_keys(servers), {` rather than on `ensure_installed`.
/// The list moved inside a `for` when versions started coming from
/// `mason-lock.json`, and an `ensure_installed`-anchored parse silently
/// returned nothing, which the count guard below caught, as designed.
fn extra_tools(lsp_config: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in lsp_config.lines() {
        let Some((_, after)) = line.split_once("vim.tbl_keys(servers), {") else {
            continue;
        };
        let Some((inside, _)) = after.split_once('}') else {
            continue;
        };
        for piece in inside.split(',') {
            let name = piece.trim().trim_matches('\'').trim();
            if !name.is_empty() {
                names.insert(name.to_string());
            }
        }
    }
    names
}

/// The formatter names conform receives.
///
/// Quoted strings inside a BRACE LIST, which is the only place a formatter
/// name appears. A looser match over every quoted string in the file picked
/// up `conform` from `require('conform')` and reported the plugin itself as
/// an uninstalled formatter, the same over-match that read `settings` as an
/// LSP server.
fn formatters(format_code: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in format_code.lines() {
        let mut rest = line;
        while let Some(open) = rest.find('{') {
            rest = &rest[open + 1..];
            let end = rest.find('}').unwrap_or(rest.len());
            let group = &rest[..end];
            let mut cursor = group;
            let mut first = true;
            while let Some(quote) = cursor.find('\'') {
                let after = &cursor[quote + 1..];
                let Some(close) = after.find('\'') else { break };
                let candidate = &after[..close];
                let before = cursor[..quote].trim();
                let adjacent = if first { before.is_empty() } else { before == "," };
                let lowercase = !candidate.is_empty()
                    && candidate.starts_with(|character: char| character.is_ascii_lowercase())
                    && candidate.chars().all(|character| {
                        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_' || character == '-'
                    });
                if adjacent && lowercase {
                    names.insert(candidate.to_string());
                } else if !adjacent {
                    break;
                }
                first = false;
                cursor = &after[close + 1..];
            }
        }
    }
    names
}

#[test]
fn the_lsp_plugin_config_exists() {
    assert!(root().join(".config/nvim/lua/plugins/lsp.lua").is_file());
}

#[test]
fn the_health_module_sits_under_a_named_directory() {
    assert!(
        health_module().is_some(),
        "no lua/<name>/health.lua exists, so :checkhealth has no name to run"
    );
    assert!(
        !nvim_lua().join("health.lua").exists(),
        "a health module directly in lua/ has no name and is never run"
    );
}

#[test]
fn the_servers_table_parses() {
    let lsp_config = read_tracked(".config/nvim/lua/plugins/lsp.lua");
    let parsed = servers(&lsp_config);
    assert!(!parsed.is_empty(), "the servers table parsed to nothing");
    assert!(
        !parsed.contains("settings"),
        "the parse took a nested config key as a server name"
    );
    assert!(parsed.len() > 1, "the servers table has one entry or none");
}

/// Every server declared in the file survives the ranged parse.
///
/// An INDEPENDENT derivation, deliberately not sharing the range above.
///
/// A first version of this guard counted the same range and compared the two
/// counts. It was useless, and the sabotage that proved it is worth
/// recording: inserting a line matching the range terminator before `cssls`
/// truncated the table from 10 servers to 5, and every assertion passed,
/// because BOTH sides read the same truncated range and agreed with each
/// other while both were wrong. Two derivations that share their failure
/// mode are one derivation.
///
/// So this counts server-shaped lines across the WHOLE file and requires the
/// ranged parse to find every one of them. A range that stops early now
/// leaves the two sets different.
///
/// `NOT_A_SERVER` is the cost of reading the whole file: `handlers` and
/// `auto_update` sit at the same 8-space depth outside the servers table.
/// That is the intended behaviour and it has now happened twice. A new key
/// at that depth breaks the build and gets classified by a human, rather
/// than silently joining the server list the way `settings` did.
#[test]
fn every_declared_server_survives_the_ranged_parse() {
    const NOT_A_SERVER: [&str; 2] = ["handlers", "auto_update"];
    let lsp_config = read_tracked(".config/nvim/lua/plugins/lsp.lua");

    let declared: BTreeSet<String> = lsp_config
        .lines()
        .filter_map(|line| server_shaped_key(line, "        "))
        .filter(|name| {
            let value = lsp_config
                .lines()
                .find(|line| server_shaped_key(line, "        ").as_ref() == Some(name))
                .and_then(|line| line.split_once(" = "))
                .map(|(_, value)| value.trim())
                .unwrap_or_default();
            value.starts_with('{') || value == "nil" || value == "true" || value == "false"
        })
        .filter(|name| !NOT_A_SERVER.contains(&name.as_str()))
        .collect();

    assert!(
        !declared.is_empty(),
        "positive control: the whole-file derivation found no servers, so this \
         comparison would pass against any ranged parse"
    );
    assert_eq!(
        declared,
        servers(&lsp_config),
        "the ranged parse and the whole-file derivation disagree, so the range \
         stops early or over-matches"
    );
}

/// Mason is pinned by a lockfile.
///
/// Without this, 14 tools were declared by NAME ONLY and the mason registry
/// was unpinned, so two machines bootstrapped a week apart got different
/// versions with no diff anywhere. The registry pin is the load-bearing
/// half: an unpinned registry changes the RESOLUTION FUNCTION, so the same
/// name means something different over time.
#[test]
fn the_mason_lockfile_pins_the_registry_and_every_tool() {
    const ENGINE_OWNED_PINS: [&str; 1] = ["tree-sitter-cli"];
    let lock_text = read_tracked(".config/nvim/mason-lock.json");
    let lock: yaml_serde::Value = yaml_serde::from_str(&lock_text)
        .unwrap_or_else(|error| panic!("mason-lock.json parses: {error}"));

    let registry = lock
        .get("registry")
        .and_then(yaml_serde::Value::as_str)
        .unwrap_or_default();
    assert!(
        registry.contains('@'),
        "the lockfile leaves the mason registry at the moving default: {registry:?}"
    );

    let lsp_config = read_tracked(".config/nvim/lua/plugins/lsp.lua");
    // tree-sitter-cli is pinned in the lock but installed by the deps
    // engine, so it is counted here even though mason does not install it.
    // The pin has to survive the move, or the lockfile silently stops
    // covering the one tool whose version must match what compiled the
    // parsers.
    let declared = servers(&lsp_config).len()
        + extra_tools(&lsp_config).len()
        + ENGINE_OWNED_PINS.len();
    let locked = lock
        .get("packages")
        .and_then(yaml_serde::Value::as_mapping)
        .map(yaml_serde::Mapping::len)
        .unwrap_or_default();
    // Compared as a COUNT against the declared set, so a lockfile that
    // silently lost entries fails rather than passing on the subset it
    // still covers.
    assert_eq!(
        declared, locked,
        "{declared} tools are declared but {locked} are pinned"
    );
}

/// The config reads the lockfile rather than carrying a second copy of the
/// versions. Two sources for one fact is how they drift.
#[test]
fn the_config_reads_the_lockfile_and_pins_the_registry() {
    let lsp_code = strip_lua_comments(&read_tracked(".config/nvim/lua/plugins/lsp.lua"));
    assert!(
        !lsp_code.trim().is_empty(),
        "positive control: stripping comments from lsp.lua left no code, so \
         every assertion here would pass vacuously"
    );

    assert!(lsp_code.contains("mason-lock.json"), "the config does not read the lockfile");
    assert!(lsp_code.contains("registries"), "the config does not pin the registry from the lockfile");
    assert!(
        lsp_code.contains("auto_update = false"),
        "auto_update is not disabled, so the lock gets overwritten"
    );

    // mason-tool-installer takes `{ 'name', version = '...' }` TABLE
    // entries. It passes the name straight to mason-registry.get_package,
    // which does not parse a `name@version` string, so the string form fails
    // at runtime with `Cannot find package "rust-analyzer@2026-04-06"`. The
    // `name@version` spelling is mason-LSPCONFIG's syntax, not this plugin's,
    // and mixing them up is silent until first launch.
    assert!(
        !lsp_code.contains("package_name .. '@'"),
        "the tool-installer entries use the name@version string form"
    );
    assert!(
        lsp_code.contains("require('dotfiles.mason_ensure')"),
        "lsp.lua does not build ensure_installed through the mason_ensure module"
    );

    // The join moved out of lsp.lua into a pure module with its own spec, so
    // the table-field rule is asserted where the code is.
    let ensure_code = strip_lua_comments(&read_tracked(".config/nvim/lua/dotfiles/mason_ensure.lua"));
    assert!(
        ensure_code.contains("version = version"),
        "the tool-installer entries do not pass version as a table field"
    );
}

/// Every formatter the config invokes actually gets installed.
///
/// THE BUG THIS CATCHES, reported 2026-09-09 from a bare Ubuntu: pressing
/// `<leader>f` on a TypeScript file printed `Formatters unavailable for
/// typescript file`. conform names prettier and prettierd for typescript,
/// javascript, svelte, css, html, json and more, and NEITHER was installed
/// by anything: not mason-lock.json, not ensure_installed, not a deps
/// manifest entry.
///
/// THE SAME CLASS AS THE LINTER GAP fixed the day before, and it walked
/// straight back in because that fix gated LINTERS only. A formatter is the
/// same shape: a binary the config execs that some install path has to
/// provide. Gating both is what stops a third recurrence with a different
/// noun.
#[test]
fn every_formatter_has_an_install_path() {
    // rustfmt has a THIRD install path and it is neither of the first two.
    // rustup's default profile ships rustfmt beside cargo and clippy, so the
    // tracked `rustup` entry is what provides it. Verified in a clean
    // ubuntu:24.04. Listed by tool name rather than inferred, so a formatter
    // that only LOOKS toolchain-provided still has to be justified by a
    // human.
    const TOOLCHAIN_PROVIDED: [&str; 1] = ["rustfmt"];

    assert!(
        root().join(".config/nvim/lua/plugins/autoformat.lua").is_file(),
        "the formatter config does not exist"
    );
    let format_code = strip_lua_comments(&read_tracked(".config/nvim/lua/plugins/autoformat.lua"));
    let parsed = formatters(&format_code);
    assert!(!parsed.is_empty(), "the formatter list parsed to nothing");
    assert!(
        !parsed.contains("conform"),
        "the parse took the plugin name as a formatter"
    );

    let lsp_config = read_tracked(".config/nvim/lua/plugins/lsp.lua");
    let mason: BTreeSet<String> = servers(&lsp_config)
        .into_iter()
        .chain(extra_tools(&lsp_config))
        .collect();
    let tracked = manifest_names();

    for tool in &parsed {
        let provided = if mason.contains(tool) {
            "mason ensure_installed"
        } else if tracked.contains(tool) {
            "a deps manifest"
        } else if TOOLCHAIN_PROVIDED.contains(&tool.as_str()) {
            "the rust toolchain (rustup default profile)"
        } else {
            "NOTHING"
        };
        assert_ne!(
            provided, "NOTHING",
            "the '{tool}' formatter has no install path -- add it to \
             ensure_installed or to a deps manifest"
        );
    }
}

/// Every language with an LSP server also names a formatter.
///
/// THE GAP THIS CATCHES, reported 2026-09-09: `rust` had no entry in
/// `formatters_by_ft` at all, so `<leader>f` on a .rs file fell through to
/// `lsp_fallback` and whatever rust-analyzer chose to do. rustfmt was on the
/// machine the whole time, so this was purely a missing config line.
///
/// Asserted per language rather than as a count, so the failure names the
/// language that is missing rather than only that one is.
///
/// The list is the languages this config declares an LSP server for AND that
/// have a canonical formatter. A language whose formatter is genuinely the
/// language server's own job does not belong here; `lua_ls`/stylua and
/// `rust_analyzer`/rustfmt both have a separate standard tool.
#[test]
fn every_language_with_a_server_names_a_formatter() {
    let format_code = strip_lua_comments(&read_tracked(".config/nvim/lua/plugins/autoformat.lua"));
    for language in ["lua", "rust"] {
        let declared = format_code
            .lines()
            .any(|line| line.trim_start().starts_with(&format!("{language} = ")));
        assert!(
            declared,
            "the {language} filetype names no formatter, so leader-f falls \
             through to lsp_fallback"
        );
    }
}

/// The treesitter spec matches the branch it is pinned to.
///
/// THE BUG THIS CATCHES, found 2026-09-08. nvim-treesitter is pinned to the
/// `main` branch (the rewrite) in lazy-lock.json, but the spec was written
/// for `master` and passed `ensure_installed`, `auto_install`, `highlight`,
/// `indent` and `incremental_selection`. On `main`,
/// `require('nvim-treesitter').setup()` forwards to nvim-treesitter.config,
/// whose default table accepts ONLY `install_dir`, so every other key is
/// absorbed and ignored. No error, no warning.
///
/// MEASURED CONSEQUENCE: 1 of 19 declared parsers installed, and treesitter
/// highlighting did not auto-start on a .ts buffer. `main` ships no FileType
/// handler, so starting the highlighter is config work now.
///
/// Asserted against the PINNED BRANCH rather than assumed, so switching back
/// to `master` deliberately makes these assertions flip rather than lie.
#[test]
fn the_treesitter_spec_matches_its_pinned_branch() {
    assert!(
        root().join(".config/nvim/lua/plugins/treesitter.lua").is_file(),
        "the treesitter spec does not exist"
    );
    // THE LOCKFILE MUST BE TRACKED, not merely present. It was gitignored
    // from the original config import until 2026-09-08, which meant the 37
    // exact plugin commits it records existed ONLY on the machine that wrote
    // them: a fresh clone got no lockfile and resolved every plugin to
    // whatever was current that day. The pinning everyone assumed was in
    // place was local state. Caught by the pre-push container, where this
    // file's absence made these assertions fail while they passed on the
    // host, which is the one shape this repo's gates are built to notice.
    assert!(
        root().join(".config/nvim/lazy-lock.json").is_file(),
        "the lazy lockfile does not exist"
    );

    let lazy_lock: yaml_serde::Value =
        yaml_serde::from_str(&read_tracked(".config/nvim/lazy-lock.json"))
            .unwrap_or_else(|error| panic!("lazy-lock.json parses: {error}"));
    let branch = lazy_lock
        .get("nvim-treesitter")
        .and_then(|entry| entry.get("branch"))
        .and_then(yaml_serde::Value::as_str)
        .unwrap_or_default();
    assert!(
        !branch.is_empty(),
        "the lockfile records no branch for nvim-treesitter"
    );

    let spec = strip_lua_comments(&read_tracked(".config/nvim/lua/plugins/treesitter.lua"));
    assert!(
        !spec.trim().is_empty(),
        "positive control: stripping comments from treesitter.lua left no code"
    );

    if branch != "main" {
        return;
    }

    // Options that only exist on master. Each one is silently discarded on
    // main, which is why their presence is a defect rather than a style
    // question.
    for option in ["ensure_installed", "auto_install", "highlight", "incremental_selection"] {
        assert!(
            !spec.contains(option),
            "the spec passes the master-only '{option}' option, which main \
             silently discards"
        );
    }
    // `:TSUpdate` only updates ALREADY-INSTALLED parsers on main, so a fresh
    // machine converges to nothing. The build has to install explicitly.
    assert!(
        !spec.contains("TSUpdate"),
        "the build step relies on :TSUpdate, which installs nothing on main"
    );
    // main ships no FileType autocmd, so the spec must start the highlighter.
    assert!(spec.contains("vim.treesitter.start"), "the spec does not start treesitter itself");
    assert!(
        spec.contains("pcall") && spec.contains("language.add"),
        "the spec does not guard the parser load"
    );
    // `pcall(language.add, lang)` is NOT sufficient on its own. get_lang
    // falls back to returning the filetype itself when nothing maps it, and
    // language.add SUCCEEDS for a name with no parser: it registers the
    // language rather than loading a parser. treesitter.start then throws
    // `Parser could not be created for buffer 1 and language "NvimTree"`,
    // reported from a real session on a plugin buffer. The predicate that
    // actually answers the question is whether a parser FILE exists on
    // runtimepath.
    assert!(
        spec.contains("nvim_get_runtime_file"),
        "the spec does not require an installed parser before starting"
    );

    // THE TREE-SITTER CLI IS A HARD REQUIREMENT ON `main`. `master` compiled
    // parsers by invoking cc directly; `main` shells out to
    // `tree-sitter build`. Measured in ubuntu:24.04 with gcc present and the
    // CLI absent: `Installed 0/19 languages`, which is a silent outcome --
    // nothing fails at startup, there is simply no highlighting.
    //
    // THE ENGINE INSTALLS IT, NOT MASON, and the distinction is the whole
    // fix. It was a mason package first, and that failed on a fresh machine:
    // the treesitter build hook runs during the same `Lazy sync` that asks
    // mason to install the CLI, so it was not on PATH yet and only 3 of 19
    // parsers compiled. Anything the editor needs during its own first run
    // cannot be installed by the editor.
    let lsp_config = read_tracked(".config/nvim/lua/plugins/lsp.lua");
    let mason: BTreeSet<String> = servers(&lsp_config)
        .into_iter()
        .chain(extra_tools(&lsp_config))
        .collect();
    assert!(
        !mason.contains("tree-sitter-cli"),
        "tree-sitter-cli is left in mason ensure_installed, but the editor \
         needs it during its own first run"
    );
}

/// No plugin build hook depends on lazy.nvim being set up.
///
/// THE BUG THIS CATCHES, reported 2026-09-08 from a bare ubuntu container:
///
///   markdown-preview.nvim ... build failed
///   Vim:E492: Not an editor command: Lazy load markdown-preview.nvim
///
/// `:Lazy` is a USER COMMAND that lazy.nvim creates during its own setup. A
/// `build` hook running during the first bootstrap sync can execute before
/// that command exists, so the plugin never builds on a fresh machine. It is
/// silent afterwards: nothing re-runs the hook, so the missing artifact only
/// surfaces when the feature is first used, possibly months later. Confirmed
/// not container-specific: the build artifact is absent on the mac too.
///
/// lazy.nvim already loads a plugin before running its build hook, so the
/// command was never needed.
#[test]
fn no_build_hook_depends_on_lazy_being_set_up() {
    let plugins = root().join(".config/nvim/lua/plugins");
    let mut specs: Vec<PathBuf> = std::fs::read_dir(&plugins)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", plugins.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("lua"))
        .collect();
    specs.sort();

    // Lua comments are stripped before matching. Both specs now EXPLAIN this
    // bug in a comment above the build hook, and a raw match read that prose
    // as the violation it was describing, the same trap the alacritty and
    // workflow suites already document.
    let plugin_code: String = specs
        .iter()
        .map(|path| {
            strip_lua_comments(
                &std::fs::read_to_string(path)
                    .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display())),
            )
        })
        .collect::<Vec<String>>()
        .join("\n");
    assert!(
        !plugin_code.trim().is_empty(),
        "positive control: the plugin specs stripped to nothing"
    );

    let lazy_invocations: Vec<&str> = plugin_code
        .lines()
        .filter(|line| line.contains("vim.cmd") && line.contains("Lazy "))
        .collect();
    assert!(
        lazy_invocations.is_empty(),
        "a build hook invokes the :Lazy user command, which does not exist \
         yet during the first bootstrap sync: {lazy_invocations:?}"
    );

    // The interactive-terminal half of the same defect.
    // markdown-preview's `mkdp#util#install()` with no argument routes
    // through `mkdp#util#open_terminal` and opens a terminal split, which
    // cannot work in a headless or non-interactive bootstrap. Upstream ships
    // `mkdp#util#install_sync()` for exactly that case.
    //
    // Asserted as "if the install function is called at all, it is the sync
    // variant", so removing the plugin does not leave an assertion that
    // passes by finding nothing.
    let mkdp_calls: Vec<&str> = plugin_code
        .lines()
        .filter(|line| line.contains("mkdp#util#install"))
        .collect();
    let async_calls: Vec<&&str> = mkdp_calls
        .iter()
        .filter(|line| !line.contains("install_sync"))
        .collect();
    assert!(
        async_calls.is_empty(),
        "the markdown-preview install call is not the sync variant, so it \
         opens a terminal split a headless bootstrap cannot provide: \
         {async_calls:?}"
    );
}

/// Every linter the config invokes actually gets installed.
///
/// THE GAP THIS CLOSES, found 2026-09-08 on a freshly bootstrapped
/// container. Opening any file printed `Error running cspell: ENOENT: no
/// such file or directory`. nvim-lint registers cspell with `cmd = 'cspell'`
/// and wires it into `linters_by_ft` for 26 filetypes, so it runs on almost
/// every buffer. Nothing installed it.
///
/// This suite already existed to catch exactly this shape -- a tool the
/// config invokes that no install path provides -- and missed it, because it
/// only ever read the mason `ensure_installed` list.
#[test]
fn every_linter_has_an_install_path() {
    assert!(
        root().join(".config/nvim/lua/plugins/lint.lua").is_file(),
        "the lint config does not exist"
    );
    let lint_code = strip_lua_comments(&read_tracked(".config/nvim/lua/plugins/lint.lua"));
    assert!(
        !lint_code.trim().is_empty(),
        "positive control: stripping comments from lint.lua left no code"
    );

    // THE BUG THIS CATCHES, captured verbatim from a PTY render on a freshly
    // bootstrapped machine:
    //
    //   Error running markdownlint: ENOENT: no such file or directory
    //   Error running cspell: ENOENT: no such file or directory
    //   Press ENTER or type command to continue
    //
    // Every markdown open blocked on a prompt. The linters ARE declared in
    // mason, so this is ordering, not a missing declaration: nvim-lint runs
    // on BufEnter while mason installs asynchronously, and mason's
    // ensure_installed does not run headlessly at all. A first launch
    // therefore always has a window where the binaries are absent.
    //
    // Unlike tree-sitter-cli these are not needed at build time, so the fix
    // is to tolerate their absence. nvim-lint has no built-in guard, so the
    // check is ours.
    assert!(
        lint_code.contains("vim.fn.executable"),
        "the lint callback does not check the linter exists before running it"
    );

    // Every `cmd = '<name>'` nvim-lint is given. That is the exact string it
    // execs, so it is the thing that must exist on PATH.
    //
    // Matched ANYWHERE on the line, not anchored to the line start. A first
    // version required `^ *cmd = '...'` and missed a linter declared inline
    // (`lint.linters.foo = { cmd = 'foo' }`), which is valid Lua and the
    // shape a one-line linter naturally takes. Verified by sabotage: the
    // anchored pattern reported 17 passing assertions with an unprovided
    // linter in the config.
    let commands: BTreeSet<String> = lint_code
        .lines()
        .filter_map(|line| {
            let (_, after) = line.split_once("cmd = '")?;
            let (name, _) = after.split_once('\'')?;
            let valid = !name.is_empty()
                && name.starts_with(|character: char| character.is_ascii_lowercase())
                && name.chars().all(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
                });
            valid.then(|| name.to_string())
        })
        .collect();
    assert!(!commands.is_empty(), "the linter commands parsed to nothing");

    // The parse must find every linter the config registers, not a subset. A
    // `lint.linters.<name> =` assignment is the independent derivation: it
    // does not share the `cmd =` pattern above, so a linter added in a shape
    // the command parse cannot read leaves the two counts different.
    let registered = lint_code
        .lines()
        .filter(|line| {
            line.split_once("lint.linters.").is_some_and(|(_, after)| {
                let name: String = after
                    .chars()
                    .take_while(|character| {
                        character.is_ascii_lowercase() || character.is_ascii_digit() || *character == '_' || *character == '-'
                    })
                    .collect();
                !name.is_empty() && after[name.len()..].trim_start().starts_with('=')
            })
        })
        .count();
    assert_eq!(
        registered,
        commands.len(),
        "{registered} linters are registered but {} commands parsed",
        commands.len()
    );

    let lsp_config = read_tracked(".config/nvim/lua/plugins/lsp.lua");
    let mason: BTreeSet<String> = servers(&lsp_config)
        .into_iter()
        .chain(extra_tools(&lsp_config))
        .collect();
    let tracked = manifest_names();

    // Named per tool rather than as one pass/fail, so a failure says WHICH
    // tool has no install path.
    for tool in &commands {
        assert!(
            mason.contains(tool) || tracked.contains(tool),
            "the '{tool}' linter has no install path -- add it to \
             ensure_installed or to a deps manifest"
        );
    }
}

/// Every runtime those tools need is tracked.
///
/// npm is the one that actually broke. Mason installs eslint-lsp, ts_ls,
/// css_variables and markdownlint from npm, so a machine with no npm fails
/// all of them together.
///
/// `nvm` being tracked is NOT the same as npm being available: nvm's check
/// is satisfied by nvm.sh existing, which says nothing about whether a node
/// version was ever installed through it. The dependency that has to exist
/// is node itself.
#[test]
fn every_runtime_the_mason_tools_need_is_tracked() {
    // The npm-backed entries in the ensure_installed list. Named here
    // because the mapping from a server name to its registry backing is
    // Mason's, not this repo's, and reading it at test time would need Mason
    // installed.
    const NPM_BACKED: [&str; 8] = [
        "eslint", "ts_ls", "css_variables", "cssls", "cssmodules_ls", "svelte", "astro", "markdownlint",
    ];

    let tracked = manifest_names();
    assert!(!tracked.is_empty(), "the dependency manifests parsed to nothing");

    // The other half of the tree-sitter-cli ownership pair: the engine must
    // install it, so it has to be a manifest entry.
    assert!(
        tracked.contains("tree-sitter-cli"),
        "tree-sitter-cli is not a tracked dependency, but the engine must \
         install it before the editor's first run"
    );
    // rustfmt needs NO manifest entry of its own, and this states why so
    // nobody adds a redundant one: rustup's default profile installs rustfmt
    // alongside cargo and clippy. So the formatter's install path is the
    // rustup entry.
    assert!(
        tracked.contains("rustup"),
        "rustup is not tracked, and it is what provides rustfmt"
    );

    let lsp_config = read_tracked(".config/nvim/lua/plugins/lsp.lua");
    let mason: BTreeSet<String> = servers(&lsp_config)
        .into_iter()
        .chain(extra_tools(&lsp_config))
        .collect();
    let needed: Vec<&str> = NPM_BACKED
        .into_iter()
        .filter(|tool| mason.contains(*tool))
        .collect();
    // A positive control on the assertion below: node only has to be tracked
    // because something npm-backed is requested.
    assert!(
        !needed.is_empty(),
        "the config requests no npm-backed tools, so the node assertion below \
         would pass without proving anything"
    );
    assert!(
        tracked.contains("node"),
        "node is not a tracked dependency, so these npm-backed Mason tools \
         cannot install: {needed:?}"
    );

    // unzip is the second runtime, and it is the one that was still missing.
    //
    // Mason's own health check names it, because a `pkg:github/...` entry
    // whose release asset is a .zip is extracted with unzip. stylua ships
    // exactly that way, so a bare machine installs every npm-backed tool
    // successfully and stylua alone fails, which reads as a stylua problem
    // rather than a missing archiver.
    //
    // deps.toml already RECORDED that a bare ubuntu:24.04 has neither xz nor
    // unzip, in the comment justifying the xz-utils entry. The observation
    // was written down and only half acted on.
    assert!(
        tracked.contains("unzip"),
        "unzip is not a tracked dependency, so zip-backed Mason tools such as \
         stylua cannot install"
    );
}

/// The health check names the runtimes, and the remedy.
///
/// Without this, a missing runtime surfaces only as N identical "failed to
/// install" lines with no stated cause, which is the experience that
/// produced this suite. `:checkhealth` is where a reader goes to find out
/// why.
#[test]
fn the_health_check_names_each_runtime_and_the_remedy() {
    let path = health_module().expect("a lua/<name>/health.lua exists");
    let health_text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()));

    // Matched as a table key or a list entry rather than as any occurrence
    // of the word, so prose mentioning node in a message does not satisfy
    // the assertion. The claim being tested is that the check RUNS on that
    // executable.
    for runtime in ["node", "npm", "unzip"] {
        assert!(
            health_text.contains(&format!("{runtime} = ")) || health_text.contains(&format!("'{runtime}'")),
            "the health check does not test for {runtime}, so a machine \
             missing it gets no stated cause"
        );
    }

    // The remedy has to be in the message. "Not found: node" tells a reader
    // what is missing and not what to do about it, and the install path here
    // is not guessable: node comes from nvm, not from the package manager.
    assert!(
        health_text.contains("nvm install"),
        "the health check does not name the install remedy, so a reader is \
         told what is missing and not how to get it"
    );
}

/// mason-lspconfig still gates `ensure_installed` on an attached UI.
///
/// THIS ASSERTION IS INVERTED FROM A NORMAL PRESENCE CHECK, and a later
/// reader will "fix" it into one unless this comment stops them.
///
/// mason-lspconfig gates `ensure_installed` on an attached user interface,
/// so a headless test that asserts the servers installed is asserting THE
/// GATE: nothing was attempted, and the assertion passes on a machine where
/// mason is entirely broken. `.claude/rules/dotfiles-tests.md` records that
/// trap and tells a reader to drive the install directly instead.
///
/// What is asserted here is that the gate STILL EXISTS at the pinned commit.
/// That is deliberately not a check that the gate is good or wanted. It is a
/// tripwire on the rules file: when a mason bump REMOVES the gate, a
/// headless install test becomes possible and that advice becomes wrong, so
/// this must fail loudly rather than let the record quietly rot. A failure
/// here is an instruction to update `.claude/rules/dotfiles-tests.md`, not a
/// bug in this repo's code.
///
/// Read from the INSTALLED PLUGIN TREE, which is not tracked and which the
/// container does not carry, so it skips there rather than failing or
/// passing vacuously.
#[test]
fn mason_lspconfig_still_gates_ensure_installed_on_an_attached_ui() {
    let Some(path) = mason_lspconfig_init() else {
        dotfiles_test_support::skip(
            "mason-lspconfig is not installed here, so the headless gate cannot be read",
        );
        return;
    };
    let init = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()));
    assert!(
        init.contains("not platform.is_headless and #settings.current.ensure_installed"),
        "the mason-lspconfig headless gate is gone from {}. A headless install \
         test is now possible, so the advice in \
         .claude/rules/dotfiles-tests.md is wrong and needs updating.",
        path.display()
    );
}

/// The installed mason-lspconfig entry point, when the plugin tree is here.
///
/// Separate from the test so the absent-tree path is one `None` rather than
/// a branch inside the assertion.
fn mason_lspconfig_init() -> Option<PathBuf> {
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| root().join(".local/share"));
    let path = data_home.join("nvim/lazy/mason-lspconfig.nvim/lua/mason-lspconfig/init.lua");
    path.is_file().then_some(path)
}
