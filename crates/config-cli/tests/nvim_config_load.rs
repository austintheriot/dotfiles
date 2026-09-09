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
fn shared_scratch() -> &'static Path {
    static SCRATCH: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    SCRATCH.get_or_init(|| tempfile::tempdir().expect("a scratch XDG home")).path()
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
    assert!(checked >= 6, "the fixture set must actually be exercised, got {checked}");

    // 5. THE PLUGIN BUFFER, which is the case both 2026-09-08 regressions
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
