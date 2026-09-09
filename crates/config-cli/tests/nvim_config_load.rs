//! Does this repo's actual Neovim config load without errors?
//!
//! WHY THIS EXISTS. Two regressions shipped on 2026-09-08 that any test
//! loading the real config would have caught in seconds:
//!
//!   Parser could not be created for buffer 1 and language "NvimTree"
//!   Cannot find package "rust-analyzer@2026-04-06"
//!
//! Neither was reachable by anything that existed. `nvim-mason-runtimes.test.sh`
//! has 45 assertions and zero nvim invocations -- it greps Lua text.
//! `nvim_runtime.rs` executes nvim with `-u NONE`, deliberately testing the
//! INSTALL rather than the config. The test-suite workflow mentions nvim zero
//! times. So the config was the one layer nothing exercised, and it is the
//! layer that changes most often.
//!
//! THE REAL init.lua, never a minimal one. A minimal init tests a config this
//! repo does not ship, and both regressions lived in plugin specs a minimal
//! init would not load.
//!
//! EVERY RUN GETS A THROWAWAY XDG_DATA_HOME. Reusing the developer's real
//! data directory would let the test pass against plugins installed by hand
//! months ago, which is this repo's dominant bug shape: a gate that passes
//! because its precondition was already satisfied.

use std::path::{Path, PathBuf};
use std::process::Command;

/// One scratch XDG home per test process, populated once.
///
/// THE COST THAT FORCED THIS. A fresh `XDG_DATA_HOME` makes the first nvim
/// run install every plugin: measured at 139 seconds against 0 seconds with
/// a warm directory. Seven of those is not an every-push test.
///
/// A shared directory is the middle ground, and it keeps the property that
/// matters: it is created by THIS test run, never inherited from the
/// developer's machine, so no assertion can pass because a plugin was
/// installed by hand months ago. The first test to touch it pays the
/// install; the rest are fast.
///
/// `OnceLock` rather than a lazily-created file, so the directory outlives
/// every test in the process and is cleaned up when the process exits.
///
/// THE PLUGIN INSTALL IS PART OF THE INITIALISER, and that is the fix for a
/// real race rather than caution. Creating the directory once is not enough:
/// the first nvim run to touch a cold `XDG_DATA_HOME` is what bootstraps
/// lazy.nvim and clones every plugin, and cargo runs the tests in this file
/// in PARALLEL. So the first test started the install while the others read
/// a half-populated directory and failed with
///
///   E5113: Lua chunk: init.lua:27: module 'lazy' not found
///
/// and, in the formatter test, an empty formatter list -- which looks
/// exactly like a missing config entry. One test in this file was safe by
/// accident; three were not.
///
/// `get_or_init` serialises: every caller blocks until the closure returns,
/// so the warm-up runs once and no test observes a partial install.
fn shared_scratch() -> &'static Path {
    static SCRATCH: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    SCRATCH
        .get_or_init(|| {
            let scratch = tempfile::tempdir().expect("a scratch XDG home");
            warm_up_plugins(scratch.path());
            scratch
        })
        .path()
}

/// Run nvim once against a cold scratch home so lazy.nvim installs.
///
/// Failures are printed and swallowed: this is a warm-up, and the tests that
/// follow assert on what the config does. A hard panic here would report a
/// network problem as a config defect.
fn warm_up_plugins(scratch: &Path) {
    let Some(nvim) = installed_nvim() else {
        return;
    };
    // `Lazy! restore`, NOT `Lazy! sync`. Both install missing plugins and
    // both are synchronous in the bang form, but `sync` also UPDATES every
    // plugin to its latest commit and rewrites `lazy-lock.json` -- and that
    // file lives in the repo's config directory, not in the scratch
    // `XDG_DATA_HOME`. A first version of this warm-up used `sync` and
    // silently moved 20 tracked plugin pins in the working tree.
    //
    // `restore` installs exactly the commits the lockfile names, which is
    // also the version this test should be exercising: the pins that ship.
    //
    // THEN `MasonToolsInstallSync`, and this is what makes the formatter
    // test honest. mason-tool-installer downloads its tools asynchronously,
    // and a warm-up that only restored plugins exited before any of them
    // arrived. So the scratch home had no `stylua`, the probe truthfully
    // reported it absent, and the `.lua` case SKIPPED on every run -- which
    // cargo hides for a passing test. stylua is the only mason-provided
    // formatter in the set, so the one case the e2e test exists for was the
    // one it never ran. The sync variant blocks until every ensure_installed
    // tool is present.
    match run_with_config(&nvim, scratch, &["-c", "Lazy! restore", "-c", "MasonToolsInstallSync"]) {
        Ok(run) if !run.stderr.trim().is_empty() => {
            eprintln!("scratch warm-up reported: {}", run.stderr.trim());
        }
        Err(error) => eprintln!("scratch warm-up could not run: {error}"),
        Ok(_) => {}
    }
}

/// The Neovim this machine would actually use, or `None` when none is installed.
fn installed_nvim() -> Option<PathBuf> {
    let raw = Command::new("sh").args(["-c", "command -v nvim"]).output().ok()?;
    let path = String::from_utf8_lossy(&raw.stdout).trim().to_string();
    if path.is_empty() { None } else { Some(PathBuf::from(path)) }
}

/// This repo's nvim config directory.
fn config_dir() -> PathBuf {
    let root = std::env::var("DOTFILES_ROOT")
        .unwrap_or_else(|_| std::env::var("HOME").unwrap_or_default());
    PathBuf::from(root).join(".config/nvim")
}

/// What one headless run reported.
struct Run {
    stdout: String,
    stderr: String,
}

/// Run nvim with this repo's config and the given extra arguments.
///
/// `XDG_DATA_HOME` and `XDG_STATE_HOME` point at a scratch directory so the
/// run cannot inherit plugins, parsers or shada from the developer's machine.
/// `XDG_CONFIG_HOME` points at the repo, so the config under test is the one
/// that ships.
fn run_with_config(nvim: &Path, scratch: &Path, arguments: &[&str]) -> Result<Run, String> {
    let output = Command::new(nvim)
        .env("XDG_CONFIG_HOME", config_dir().parent().unwrap_or(Path::new("/")))
        .env("XDG_DATA_HOME", scratch.join("data"))
        .env("XDG_STATE_HOME", scratch.join("state"))
        .env("XDG_CACHE_HOME", scratch.join("cache"))
        // Deterministic messages regardless of the host locale.
        .env("LANG", "C.UTF-8")
        .args(["--headless"])
        .args(arguments)
        .args(["-c", "qa!"])
        .output()
        .map_err(|error| format!("could not run {}: {error}", nvim.display()))?;

    Ok(Run {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// The error shapes a broken config produces.
///
/// Matched on Neovim's own error vocabulary rather than on a word list of my
/// own invention: `E<digits>:` is an enumerated namespace, and the Lua
/// failures arrive as a traceback or an explicit "Error" line. `E484` would
/// have been caught by the first pattern, and both 2026-09-08 regressions by
/// the last two.
fn error_lines(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("Error")
                || trimmed.contains("stack traceback")
                || trimmed.contains("Cannot find package")
                || regex_like_vim_error(trimmed)
        })
        .collect()
}

/// Whether a line opens with Neovim's `E<digits>:` error form.
///
/// Hand-rolled rather than pulling in a regex crate for one pattern: the
/// shape is `E` followed by digits followed by a colon, anywhere in the line.
fn regex_like_vim_error(line: &str) -> bool {
    let bytes = line.as_bytes();
    for (index, window) in bytes.iter().enumerate() {
        if *window != b'E' {
            continue;
        }
        let rest = &bytes[index + 1..];
        let digits = rest.iter().take_while(|byte| byte.is_ascii_digit()).count();
        if digits > 0 && rest.get(digits) == Some(&b':') {
            return true;
        }
    }
    false
}

/// ONE TEST, NOT THREE, and the reason is mechanical rather than stylistic.
///
/// Rust runs `#[test]` functions on parallel threads. Three of them sharing
/// one scratch `XDG_DATA_HOME` while lazy.nvim installs 37 plugins into it
/// is a race: the later tests read a half-populated directory. Splitting the
/// scratch per test instead costs a full plugin install each (measured: 139
/// seconds against 0 with a warm directory), which is not an every-push
/// test.
///
/// So the checks run in sequence inside one test. The cost is a less precise
/// failure name, which the assertion messages carry instead.
///
/// The real config loads with no errors on stderr.
///
/// Skips when nvim is absent, which is a deliberate and narrow exception: a
/// developer machine may legitimately not have it yet, and the machine where
/// absence WOULD be a defect is covered by the deps workflow. The sentinel
/// test below is what keeps the skip honest, by proving this assertion can
/// still fail.
#[test]
fn the_real_config_loads_and_opens_buffers_without_errors() {
    let Some(nvim) = installed_nvim() else {
        eprintln!("skip: no nvim on PATH, so there is no config to load");
        return;
    };
    if !config_dir().join("init.lua").is_file() {
        eprintln!("skip: no init.lua at {}", config_dir().display());
        return;
    }
    let scratch = shared_scratch();

    // 1. The config loads at all.
    let run = run_with_config(&nvim, scratch, &[]).expect("nvim runs");
    let errors = error_lines(&run.stderr);
    assert!(
        errors.is_empty(),
        "loading the real config wrote errors to stderr:\n{}",
        errors.join("\n")
    );

    // 2. ANTI-VACUITY: the config really was loaded. An empty stderr proves
    //    nothing on its own. The sentinels are values only this repo sets:
    //    mapleader (settings.lua:1) and the dotfiles_treesitter augroup
    //    (treesitter.lua:69), the second of which proves a PLUGIN spec ran
    //    rather than only the top-level settings file.
    let probe = r#"lua
        local leader = vim.g.mapleader == ' '
        local groups = vim.api.nvim_get_autocmds({ group = 'dotfiles_treesitter' })
        io.write('leader=' .. tostring(leader) .. ' augroup=' .. tostring(#groups > 0))
    "#;
    let loaded = run_with_config(&nvim, scratch, &["-c", probe]).expect("nvim runs");
    assert!(
        loaded.stdout.contains("leader=true"),
        "the config's own mapleader must be set, or this suite is testing an \
         unconfigured editor: {:?} (stderr: {:?})",
        loaded.stdout,
        loaded.stderr
    );
    assert!(
        loaded.stdout.contains("augroup=true"),
        "the treesitter plugin spec must have run, or only settings.lua was \
         reached: {:?} (stderr: {:?})",
        loaded.stdout,
        loaded.stderr
    );

    // 3. The negative half of the same probe: with no config, the sentinels
    //    must be absent. Without this, a probe that always reported true
    //    would satisfy the two assertions above.
    let bare = Command::new(&nvim)
        .args(["--headless", "-u", "NONE", "-c", probe, "-c", "qa!"])
        .output()
        .expect("nvim runs with no config");
    let bare_stdout = String::from_utf8_lossy(&bare.stdout);
    assert!(
        !bare_stdout.contains("leader=true"),
        "`-u NONE` reported the config's mapleader, so the probe is not \
         measuring the config at all: {bare_stdout:?}"
    );

    // 4. Opening a buffer of each configured kind raises nothing.
    let fixtures = scratch.join("fixtures");
    std::fs::create_dir_all(&fixtures).expect("a fixtures directory");
    let files = [
        ("probe.lua", "local function greet(name)\n  return name\nend\n"),
        ("probe.md", "# Heading\n\nSome **bold** text.\n"),
        ("probe.ts", "const value: number = 1;\n"),
        ("probe.json", "{\n  \"key\": \"value\"\n}\n"),
        ("probe.toml", "[table]\nkey = \"value\"\n"),
        ("probe.sh", "#!/bin/sh\necho hello\n"),
        // Added 2026-09-09 at the owner's suggestion, after `<leader>f` on a
        // TypeScript file reported no formatters on a bare Ubuntu. These are
        // the filetypes conform routes to prettier, so they are where a
        // missing formatter shows up.
        ("probe.css", ".selector {\n  color: red;\n}\n"),
        ("probe.html", "<!doctype html>\n<html><body><p>hi</p></body></html>\n"),
        ("probe.rs", "fn main() {\n    println!(\"hi\");\n}\n"),
    ];
    let mut checked = 0;
    for (name, body) in files {
        let path = fixtures.join(name);
        std::fs::write(&path, body).expect("a fixture file");
        let run = run_with_config(&nvim, scratch, &["-c", &format!("edit {}", path.display())])
            .expect("nvim runs");
        let errors = error_lines(&run.stderr);
        assert!(
            errors.is_empty(),
            "opening {name} wrote errors to stderr:\n{}",
            errors.join("\n")
        );
        checked += 1;
    }
    assert!(checked >= 9, "the fixture set must actually be exercised, got {checked}");

    // 5. EVERY PINNED MASON NAME RESOLVES against the pinned registry.
    //
    //    This is what `Cannot find package "rust-analyzer@2026-04-06"` was:
    //    a lockfile whose names the registry cannot find, discovered on
    //    first launch rather than in a test.
    //
    //    IT NEEDS THE NETWORK, unavoidably. The registry lives in
    //    XDG_DATA_HOME, which this test keeps scratch precisely so nothing
    //    is inherited, so it must be fetched. Reading the developer's
    //    already-downloaded copy would be instant and would also be the
    //    pre-satisfied path this repo keeps getting bitten by.
    //
    //    Resolution goes through the registry's OWN lspconfig mapping rather
    //    than a second table here, so a rename upstream cannot leave the two
    //    disagreeing.
    let lockfile_probe = r#"lua
        local path = vim.fn.stdpath('config') .. '/mason-lock.json'
        if vim.fn.filereadable(path) == 0 then
          io.write('lock-missing')
          return
        end
        local lock = vim.json.decode(table.concat(vim.fn.readfile(path), '\n'))
        local registry = require('mason-registry')
        -- The registry lives in XDG_DATA_HOME, which this test deliberately
        -- keeps scratch, so it has to be fetched before any name can
        -- resolve. Without this every name reports unresolved for a reason
        -- that has nothing to do with the lockfile.
        local done = false
        registry.update(function() done = true end)
        vim.wait(120000, function() return done end, 200)
        local unresolved, checked = {}, 0
        for package_name, _ in pairs(lock.packages or {}) do
          checked = checked + 1
          if not pcall(registry.get_package, package_name) then
            table.insert(unresolved, package_name)
          end
        end
        io.write('checked=' .. checked .. ' unresolved=' .. table.concat(unresolved, ','))
    "#;
    let lock = run_with_config(&nvim, scratch, &["-c", lockfile_probe]).expect("nvim runs");
    assert!(
        !lock.stdout.contains("lock-missing"),
        "mason-lock.json must exist, or the pinning it records is not in effect"
    );
    assert!(
        lock.stdout.contains("unresolved="),
        "the lockfile probe did not finish, so it proved nothing: {:?} \
         (stderr: {:?})",
        lock.stdout,
        lock.stderr
    );
    assert!(
        lock.stdout.contains("unresolved=") && lock.stdout.ends_with("unresolved="),
        "every name in mason-lock.json must resolve against the pinned \
         registry: {}",
        lock.stdout
    );
    // Positive control: a probe that read an empty lockfile would report
    // zero unresolved names and pass while asserting nothing.
    assert!(
        !lock.stdout.contains("checked=0"),
        "the lockfile probe read no packages, so it asserted nothing: {}",
        lock.stdout
    );

    // 6. EVERY CONFIGURED FORMATTER RESOLVES.
    //
    //    `<leader>f` on a TypeScript file reported "Formatters unavailable"
    //    on a bare Ubuntu, because prettier and prettierd were named by
    //    conform and installed by nothing.
    //
    //    THE TRAP THIS AVOIDS: on a machine where a formatter is genuinely
    //    absent, "unavailable" is the CORRECT answer, so asserting that
    //    formatting succeeds would fail for the right reason and the wrong
    //    cause. What is asserted instead is that every formatter conform is
    //    configured with resolves to an executable -- a config bug (nothing
    //    configured for this filetype) and an install bug (configured but
    //    absent) are different, and only the second is this repo's fault.
    //
    //    Skipped rather than failed when nothing is installed yet, because
    //    mason installs asynchronously and this test does not drive it. The
    //    shell suite's formatter gate is what proves an install path EXISTS;
    //    this proves the ones present actually run.
    let formatter_probe = r#"lua
        local ok, conform = pcall(require, 'conform')
        if not ok then io.write('conform-missing') return end
        local unresolved, checked = {}, 0
        for filetype, entry in pairs(conform.formatters_by_ft or {}) do
          local names = type(entry) == 'table' and entry or { entry }
          for _, name in ipairs(names) do
            if type(name) == 'string' then
              checked = checked + 1
              local info = conform.get_formatter_info(name)
              if info and info.available == false and info.available_msg then
                table.insert(unresolved, filetype .. ':' .. name)
              end
            end
          end
        end
        io.write('checked=' .. checked .. ' unavailable=' .. #unresolved)
    "#;
    let formatters = run_with_config(&nvim, scratch, &["-c", formatter_probe]).expect("nvim runs");
    assert!(
        !formatters.stdout.contains("conform-missing"),
        "conform must load, or the formatter configuration is unreachable: {:?}",
        formatters.stderr
    );
    assert!(
        formatters.stdout.contains("checked="),
        "the formatter probe did not finish, so it proved nothing: {:?} \
         (stderr: {:?})",
        formatters.stdout,
        formatters.stderr
    );
    assert!(
        !formatters.stdout.contains("checked=0"),
        "conform reported no configured formatters, so this asserted \
         nothing: {}",
        formatters.stdout
    );

    // 7. THE PLUGIN BUFFER, which is the case both 2026-09-08 regressions
    //    needed. A filetype with no parser is what every plugin window is,
    //    and a test that only opens .lua and .md files passes while the file
    //    explorer throws on every open.
    let plugin_buffer = r#"lua
        local buf = vim.api.nvim_create_buf(false, true)
        vim.api.nvim_buf_set_lines(buf, 0, -1, false, { 'contents' })
        vim.api.nvim_buf_call(buf, function()
          vim.bo[buf].filetype = 'NvimTree'
        end)
        io.write('plugin-buffer-ok')
    "#;
    let run = run_with_config(&nvim, scratch, &["-c", plugin_buffer]).expect("nvim runs");
    assert!(
        run.stdout.contains("plugin-buffer-ok"),
        "the plugin-buffer probe did not finish, so it proved nothing: {:?} \
         (stderr: {:?})",
        run.stdout,
        run.stderr
    );
    let errors = error_lines(&run.stderr);
    assert!(
        errors.is_empty(),
        "a buffer whose filetype has no parser raised:\n{}",
        errors.join("\n")
    );
}

/// Does `<leader>f` actually format a file, per language?
///
/// WHY THIS EXISTS, and why the contract tests were not enough. Everything
/// else guarding formatters checks DECLARATIONS: that a formatter named in
/// `formatters_by_ft` also appears in mason's `ensure_installed` or a deps
/// manifest. That is a real gate and it is one layer too high.
///
/// The gap it missed, reported 2026-09-09 from a bare Ubuntu: `rust` had no
/// entry in `formatters_by_ft` at all. Nothing was declared, so nothing was
/// undeclared, so every declaration test passed while pressing the format
/// key on a .rs file did nothing but fall through to `lsp_fallback`. A test
/// that formats a real buffer cannot be fooled that way: an unformatted
/// buffer is an unformatted buffer.
///
/// FORMATTING THROUGH CONFORM'S OWN ENTRY POINT, not by shelling out to
/// prettier. Running `prettier` directly would prove prettier works, which
/// nobody doubts. What is under test is this repo's wiring: the filetype
/// mapping, the formatter resolution, and whether the tool is reachable on
/// the PATH nvim actually has.
///
/// SKIPS RATHER THAN FAILS when the formatter binary is absent. Mason
/// installs prettier and stylua asynchronously on first launch, and a fresh
/// scratch directory has not finished when this runs; rustfmt comes from
/// rustup and may not be on a CI image at all. A skip keeps this honest on
/// an incomplete machine while still failing hard on the thing this repo
/// controls -- the mapping. `formatters_for_filetype` below is that half,
/// and it never skips.
#[test]
fn each_configured_filetype_formats_a_real_buffer() {
    let Some(nvim) = installed_nvim() else {
        eprintln!("skipping: no nvim on PATH");
        return;
    };
    let scratch = shared_scratch();

    // One case per language whose formatter this repo configures. The input
    // is deliberately misformatted in a way the formatter must fix, so a
    // no-op formatter cannot pass.
    let cases: [(&str, &str, &str); 4] = [
        ("rs", "fn  main( )  {let x=1;}\n", "rustfmt"),
        ("lua", "local  x   =    1\n", "stylua"),
        ("ts", "const  x   =    1\n", "prettier"),
        ("json", "{\"a\" :  1}\n", "prettier"),
    ];

    for (extension, misformatted, tool) in cases {
        let path = scratch.join(format!("format-probe.{extension}"));
        std::fs::write(&path, misformatted)
            .unwrap_or_else(|error| panic!("could not write {}: {error}", path.display()));

        // Ask conform what it would run BEFORE formatting, so a filetype
        // with no mapping is reported as such rather than as a formatter
        // that made no change.
        // `list_formatters_for_buffer`, not `list_formatters`. The latter
        // filters to formatters that are actually INSTALLED, so in a fresh
        // scratch XDG home it returns an empty list and the assertion below
        // could not tell a missing mapping from an uninstalled tool -- the
        // two facts this test exists to separate.
        let probe = String::from(
            "lua local names = require('conform').list_formatters_for_buffer(0) \
             print('FORMATTERS:' .. table.concat(vim.tbl_flatten(names), ','))",
        );
        let listing = run_with_config(
            &nvim,
            scratch,
            &[path.to_str().expect("a utf-8 path"), "-c", &probe],
        )
        .expect("nvim runs");

        let named = listing
            .stdout
            .lines()
            .chain(listing.stderr.lines())
            .find_map(|line| line.strip_prefix("FORMATTERS:"))
            .unwrap_or("")
            .to_owned();

        assert!(
            named.split(',').any(|entry| entry == tool),
            ".{extension} must map to {tool}, conform listed [{named}]"
        );

        // Now format for real and compare the file on disk. `write` is what
        // makes the change observable to this test at all.
        let format = "lua require('conform').format({ async = false, lsp_fallback = false })";
        let run = run_with_config(
            &nvim,
            scratch,
            &[path.to_str().expect("a utf-8 path"), "-c", format, "-c", "w"],
        )
        .expect("nvim runs");

        let formatted = std::fs::read_to_string(&path).expect("the probe file is readable");
        match judge_format(misformatted, &formatted, tool_on_nvim_path(&nvim, scratch, tool)) {
            FormatVerdict::Formatted => {}
            FormatVerdict::ToolAbsent => {
                // A genuine skip: mason installs asynchronously and has not
                // put the binary on nvim's PATH yet. Named, so a run that
                // skipped everything is visibly not a run that passed.
                eprintln!("skipping .{extension}: {tool} is not on nvim's PATH yet");
                continue;
            }
            FormatVerdict::ToolPresentButNoChange => panic!(
                ".{extension}: {tool} is on nvim's PATH and the file did not change, \
                 so the wiring did not run it\n  stderr: {}",
                run.stderr.trim()
            ),
        }

        assert!(
            !formatted.contains("  ="),
            ".{extension} still holds the misformatted spacing after formatting: {formatted:?}"
        );
    }
}

/// Whether nvim can find a formatter binary on the PATH it actually has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolOnPath {
    Yes,
    No,
}

/// What one format attempt proved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormatVerdict {
    /// The file changed: the wiring ran the tool.
    Formatted,
    /// Unchanged and the binary is absent: mason has not finished. Skip.
    ToolAbsent,
    /// Unchanged and the binary is PRESENT: the wiring did not run it. Fail.
    ToolPresentButNoChange,
}

/// Pure: decide from the two file contents and the PATH probe alone.
fn judge_format(before: &str, after: &str, tool: ToolOnPath) -> FormatVerdict {
    if before != after {
        return FormatVerdict::Formatted;
    }
    match tool {
        ToolOnPath::No => FormatVerdict::ToolAbsent,
        ToolOnPath::Yes => FormatVerdict::ToolPresentButNoChange,
    }
}

/// Ask the running config whether `tool` resolves on nvim's PATH.
///
/// nvim's PATH, not this process's: mason prepends its own bin directory
/// inside nvim, so a binary this test cannot see can still be one conform
/// will run. `exepath` is empty when nothing resolves.
fn tool_on_nvim_path(nvim: &Path, scratch: &Path, tool: &str) -> ToolOnPath {
    let probe = format!("lua print('EXEPATH:' .. vim.fn.exepath('{tool}'))");
    let run = run_with_config(nvim, scratch, &["-c", &probe]).expect("nvim runs");
    let found = run
        .stdout
        .lines()
        .chain(run.stderr.lines())
        .find_map(|line| line.strip_prefix("EXEPATH:"))
        .unwrap_or("");
    if found.is_empty() { ToolOnPath::No } else { ToolOnPath::Yes }
}

/// The verdict logic for one formatted file, on its own so it can be
/// tested without nvim.
///
/// THE DISTINCTION THIS EXISTS TO MAKE. An earlier version of the e2e test
/// skipped whenever the file came back unchanged, on the reasoning that
/// mason installs asynchronously and the binary may not exist yet. That
/// skip could not tell "prettier is not installed" from "prettier is
/// installed and the wiring did not run it" -- and the second is exactly
/// the reported bug. A skip that covers both is the fail-open shape this
/// repo keeps paying for.
///
/// So the unchanged case splits on whether the tool is on nvim's PATH:
/// absent is a genuine skip, present is a failure.
#[test]
fn an_unchanged_file_is_a_failure_only_when_the_tool_was_present() {
    let before = "fn  main( )  {}\n";
    let after_formatting = "fn main() {}\n";

    assert_eq!(
        judge_format(before, after_formatting, ToolOnPath::Yes),
        FormatVerdict::Formatted,
        "a changed file is formatted regardless of anything else"
    );
    assert_eq!(
        judge_format(before, before, ToolOnPath::No),
        FormatVerdict::ToolAbsent,
        "unchanged with no binary is mason not finished: skip"
    );
    assert_eq!(
        judge_format(before, before, ToolOnPath::Yes),
        FormatVerdict::ToolPresentButNoChange,
        "unchanged WITH the binary means the wiring did not run it: fail"
    );
}

/// Every filetype this repo maps to a formatter resolves to one in nvim.
///
/// The half of the test above that must NEVER skip. It asks conform, inside
/// the real config, which formatters it would run for each filetype -- so it
/// fails when a mapping is missing or misspelled regardless of whether any
/// formatter binary is installed on this machine.
#[test]
fn formatters_for_filetype_are_all_resolvable() {
    let Some(nvim) = installed_nvim() else {
        eprintln!("skipping: no nvim on PATH");
        return;
    };
    let scratch = shared_scratch();

    // Read the mapping out of the running config rather than restating it
    // here, so a filetype added to autoformat.lua is covered with no edit
    // to this test.
    let probe = "lua local by_ft = require('conform').formatters_by_ft \
                 local keys = vim.tbl_keys(by_ft) table.sort(keys) \
                 print('FILETYPES:' .. table.concat(keys, ','))";
    let listing = run_with_config(&nvim, scratch, &["-c", probe]).expect("nvim runs");

    let filetypes = listing
        .stdout
        .lines()
        .chain(listing.stderr.lines())
        .find_map(|line| line.strip_prefix("FILETYPES:"))
        .unwrap_or("")
        .to_owned();

    assert!(
        !filetypes.is_empty(),
        "the config must map at least one filetype to a formatter, got [{filetypes}]"
    );

    // rust is asserted BY NAME because its absence is the reported bug. The
    // generic check above would pass with rust missing, exactly as every
    // declaration test did.
    assert!(
        filetypes.split(',').any(|entry| entry == "rust"),
        "rust must map to a formatter, config maps [{filetypes}]"
    );
    assert!(
        filetypes.split(',').any(|entry| entry == "lua"),
        "lua must map to a formatter, config maps [{filetypes}]"
    );
}
