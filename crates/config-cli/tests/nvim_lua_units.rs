//! Runs the pure-Lua specs under `.config/nvim/tests/` and asserts each passes.
//!
//! WHY A RUST DRIVER FOR LUA SPECS. The three modules under
//! `lua/dotfiles/` (`treesitter_start`, `lint_runnable`, `mason_ensure`) are
//! the pieces of the nvim config that are real logic rather than declarative
//! tables, and they are the pieces that broke: the NvimTree parser error and
//! the `rust-analyzer@2026-04-06` mason failure both lived in them. Each spec
//! exercises one module with its IO injected as a predicate or table.
//!
//! The specs run under `nvim --headless -l`, which makes nvim the Lua
//! interpreter: `vim.deep_equal`, `vim.inspect` and `vim.tbl_map` are the real
//! ones, and nothing depends on plenary or busted being installed. This file
//! is the gate: a spec that fails exits non-zero, and cargo test turns that
//! into a red push. The assertion that gates the push therefore lives in
//! Rust, as the rest of this repo's nvim hardening does.
//!
//! DISCOVERY, NOT A LIST. Every `*_spec.lua` under `tests/` is run, so a new
//! spec is covered by being added, and a spec deleted by mistake shows up as
//! a count that went down (asserted below), not as silence.

use std::path::{Path, PathBuf};
use std::process::Command;

fn installed_nvim() -> Option<PathBuf> {
    let raw = Command::new("sh").args(["-c", "command -v nvim"]).output().ok()?;
    let path = String::from_utf8_lossy(&raw.stdout).trim().to_string();
    if path.is_empty() { None } else { Some(PathBuf::from(path)) }
}

fn config_dir() -> PathBuf {
    let root = std::env::var("DOTFILES_ROOT")
        .unwrap_or_else(|_| std::env::var("HOME").unwrap_or_default());
    PathBuf::from(root).join(".config/nvim")
}

/// Every `*_spec.lua` under the config's `tests/` directory, sorted.
fn specs(config: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(config.join("tests"))
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.ends_with("_spec.lua"))
                })
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

/// Run one spec with `LUA_PATH` spanning the config's `lua/` and `tests/`.
///
/// `nvim -l` runs the file as a script and exits with its status, so a spec
/// that calls `os.exit(1)` or raises an error is a non-zero exit here.
fn run_spec(nvim: &Path, config: &Path, spec: &Path) -> (bool, String) {
    let lua_path = format!(
        "{c}/lua/?.lua;{c}/tests/?.lua;;",
        c = config.display()
    );
    let output = Command::new(nvim)
        .env("LUA_PATH", lua_path)
        // A scratch XDG home, so the run cannot load the developer's plugins;
        // the specs need only the vim API that ships with the binary.
        .env("XDG_DATA_HOME", std::env::temp_dir().join("nvim-lua-units-data"))
        .env("XDG_STATE_HOME", std::env::temp_dir().join("nvim-lua-units-state"))
        .env("XDG_CACHE_HOME", std::env::temp_dir().join("nvim-lua-units-cache"))
        .env("LANG", "C.UTF-8")
        .args(["--headless", "-l"])
        .arg(spec)
        .output()
        .expect("nvim runs");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.success(), text)
}

#[test]
fn every_lua_spec_passes_under_nvim() {
    let Some(nvim) = installed_nvim() else {
        eprintln!("skipping: no nvim on PATH");
        return;
    };
    let config = config_dir();
    let found = specs(&config);

    // A positive control on discovery. Three modules have specs today; a
    // count below that means a spec was lost, not that there is less to
    // test, and this is the line that says so.
    assert!(
        found.len() >= 3,
        "expected at least three *_spec.lua under {}, found {:?}",
        config.join("tests").display(),
        found
    );

    for spec in &found {
        let (passed, text) = run_spec(&nvim, &config, spec);
        assert!(
            passed,
            "{} failed under nvim -l:\n{}",
            spec.file_name().and_then(|name| name.to_str()).unwrap_or("?"),
            text.trim()
        );
        assert!(
            text.contains(": ok"),
            "{} exited 0 without reporting ok, so it asserted nothing:\n{}",
            spec.display(),
            text.trim()
        );
    }
}
