//! The Lua under `.config/nvim`: formatting with stylua, correctness with
//! selene.
//!
//! WHY THIS EXISTS. Nothing checked the Lua at all. `shellcheck_lint.rs`
//! covers 25+ shell scripts, and the roughly 20 Lua files had no equivalent,
//! so the only feedback on a Lua mistake was nvim failing at runtime, which
//! is how the NvimTree and mason-version regressions both shipped.
//!
//! stylua is already a tracked tool (it is in mason's `ensure_installed` and
//! pinned in `mason-lock.json`), and `.config/nvim/.stylua.toml` already
//! states the settings. The only thing missing was something that fails when
//! the files drift from them.
//!
//! A formatter cannot catch a wrong API call, and the two regressions above
//! were both wrong API calls. selene is the correctness half. selene ships
//! standard libraries for plain Lua and Roblox and NOTHING for Neovim, so
//! without a `vim` declaration it reports "`vim` is not defined" on the first
//! real line of every file, which is a gate that gets muted within a day.
//! `.config/nvim/vim.yml` supplies it, in the shape mason.nvim and
//! plenary.nvim converged on.
//!
//! SKIPS WHEN A LINTER IS ABSENT, and that is a narrow exception rather than
//! a habit: both tools arrive through mason on first nvim launch, so a
//! machine that has bootstrapped but never opened nvim legitimately lacks
//! them. An absent TOOL is a skip; an absent TRACKED FILE is a failure, and
//! the two are kept distinct below. The CI legs that install the tools are
//! where this has teeth.
//!
//! Converted whole from `tests/nvim-lua-format.test.sh`, which ran **10**
//! assertions on this machine, all 10 from distinct `assert_*` call sites
//! with no loops. Both linters are installed here, so neither skip branch
//! fires.
//!
//! THE SHELL SUITE COULD NOT FAIL. It has no `finish` call, and `finish` is
//! what reads the tally and returns the exit status. Measured 2026-09-10: an
//! unformatted local appended to `init.lua` makes the suite print
//! `FAIL: every Lua file matches the repo stylua settings` and then exit
//! **0**. So the gate reported a violation and passed anyway, which is this
//! repo's named dominant bug class, a gate failing open. The Rust harness
//! removes the failure mode structurally: a failing assertion fails the
//! test, with no bookkeeping call to forget.

use dotfiles_test_support::repo::root as repo_root;
use dotfiles_test_support::skip;
use std::path::{Path, PathBuf};
use std::process::Command;

fn nvim_dir() -> PathBuf {
    repo_root().join(".config/nvim")
}

/// Where a mason-installed tool lives, when it is not on PATH.
///
/// Mason's bin directory is not on a non-login PATH, which is the same
/// reason the deps engine prepends it: a tool installed by this repo has to
/// be findable by this repo's own tests.
fn tool_path(name: &str) -> Option<PathBuf> {
    if let Ok(output) = Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {name}"))
        .output()
        && output.status.success()
    {
        let found = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !found.is_empty() {
            return Some(PathBuf::from(found));
        }
    }

    let mason = repo_root()
        .join(".local/share/nvim/mason/bin")
        .join(name);
    is_executable(&mason).then_some(mason)
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|data| data.permissions().mode() & 0o111 != 0)
}

/// Every tracked Lua file under the nvim config.
fn lua_files(directory: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(directory) else {
        return found;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            found.extend(lua_files(&path));
        } else if path.extension().and_then(|value| value.to_str()) == Some("lua") {
            found.push(path);
        }
    }
    found
}

/// The tracked settings files must exist. An absent tracked file is a
/// failure, never a skip: it means the repo stopped stating its own rules.
#[test]
fn the_repo_states_its_own_lua_settings() {
    let directory = nvim_dir();
    assert!(
        directory.is_dir(),
        "no nvim config directory at {}",
        directory.display()
    );
    for (file, purpose) in [
        (".stylua.toml", "the stylua formatting settings"),
        ("selene.toml", "the selene lint settings"),
        ("vim.yml", "the vim standard library selene needs"),
    ] {
        let path = directory.join(file);
        assert!(path.is_file(), "{purpose} are missing at {}", path.display());
    }
}

/// The std chain is the load-bearing line: `lua51+vim` resolves `vim` as a
/// file beside `selene.toml`. A bare `lua51` would report every vim call.
#[test]
fn selene_chains_the_vim_standard_library() {
    let config = nvim_dir().join("selene.toml");
    let text = std::fs::read_to_string(&config)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", config.display()));

    let chained = text.lines().any(|line| {
        let line = line.trim();
        line.starts_with("std")
            && line.contains("+vim")
            && line.split('=').nth(1).is_some_and(|value| {
                let value = value.trim().trim_matches('"');
                value.starts_with("lua5") && value.ends_with("+vim")
            })
    });
    assert!(
        chained,
        "selene.toml does not chain the vim standard library; without \
         `std = \"lua5x+vim\"` selene reports every vim call as undefined, \
         and the gate gets muted within a day. It says: {text}"
    );
}

#[test]
fn every_lua_file_matches_the_repo_stylua_settings() {
    let directory = nvim_dir();
    let Some(stylua) = tool_path("stylua") else {
        skip("stylua is not installed: it arrives through mason on first nvim launch");
        return;
    };

    // Positive control, asserted BEFORE the check runs. `stylua --check` on
    // a directory with no Lua files exits 0 and prints nothing, which is
    // indistinguishable from success.
    let files = lua_files(&directory);
    assert!(
        files.len() > 10,
        "only {} Lua files found under {}; a clean stylua run would prove \
         nothing about a tree this small",
        files.len(),
        directory.display()
    );

    // `--check` exits non-zero and prints a unified diff per offending file.
    // The output is captured so a failure names the files rather than only
    // the exit code.
    let output = Command::new(&stylua)
        .arg("--check")
        .arg(&directory)
        .output()
        .expect("stylua runs");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let mut offenders: Vec<&str> = combined
        .lines()
        .filter_map(|line| line.strip_prefix("Diff in "))
        .collect();
    offenders.sort_unstable();
    offenders.dedup();

    assert!(
        offenders.is_empty(),
        "these Lua files do not match {}/.stylua.toml: {}",
        directory.display(),
        offenders.join(", ")
    );
}

#[test]
fn selene_reports_no_errors() {
    let directory = nvim_dir();
    let Some(selene) = tool_path("selene") else {
        skip("selene is not installed: it arrives through mason on first nvim launch");
        return;
    };

    // Run FROM the config directory: selene resolves `+vim` relative to the
    // working directory, not to the file being linted.
    let output = Command::new(&selene)
        .arg(".")
        .current_dir(&directory)
        .output()
        .expect("selene runs");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Positive control. selene on a directory it cannot read exits 0 with no
    // findings, which is indistinguishable from a clean run.
    assert!(
        combined.contains("Results:"),
        "selene produced no report, so a clean result proves nothing; it \
         said: {combined}"
    );

    // Named separately from the error count so the failure states the cause
    // rather than only the number.
    assert!(
        !combined.contains("`vim` is not defined"),
        "selene reports the vim global as undefined, so vim.yml is not being \
         resolved; every real finding is buried under that noise: {combined}"
    );

    let errors: Vec<&str> = combined
        .lines()
        .filter(|line| line.starts_with("error["))
        .collect();
    assert!(
        errors.is_empty(),
        "selene reports {} errors: {}",
        errors.len(),
        errors.join("\n")
    );
}
