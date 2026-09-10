//! An old Neovim gets an explanation instead of a Lua traceback.
//!
//! The failure this exists to catch, observed on a real Pop!_OS machine
//! after a clean bootstrap:
//!
//! ```text
//! Error detected while processing /home/austin/.config/nvim/init.lua:
//! E5113: Error while calling lua chunk: .../init.lua:6: attempt to index
//! field 'uv' (a nil value)
//! ```
//!
//! `vim.uv` arrived in Neovim 0.10. Before that the same libuv handle was
//! `vim.loop`. Apt ships 0.6.1 on Pop!_OS 22.04 and 0.9.5 on 24.04, and the
//! dependency manifest's check is `command -v nvim`, a presence test with no
//! version floor, so both satisfy the bootstrap and then crash the editor.
//!
//! `lua/dotfiles/health.lua` already carried the correct floor check. It
//! could never fire on the machines that needed it, for two independent
//! reasons, and BOTH are asserted below because fixing either one alone
//! leaves the crash:
//!
//! 1. `init.lua` indexed `vim.uv` at line 6, so startup died before
//!    `:checkhealth` could ever be typed.
//! 2. `health.lua` called `vim.uv.os_uname()` ABOVE its own version check,
//!    so even a reachable `:checkhealth` crashed before reporting the cause.
//!
//! A health check that never runs reports nothing at all. A version guard
//! placed below the code it guards is not a guard.
//!
//! Converted whole from `tests/nvim-version-floor.test.sh`, which ran **6**
//! assertions and 2 skips on this machine today, from 8 `assert_*` call
//! sites: two sit in a loop over the two Lua files and are conditional, and
//! both files currently use only the compatibility spelling, so the ordering
//! pair skips. The ceiling is 10 when both files hold a bare `vim.uv`.
//!
//! ONE ASSERTION WAS VACUOUS, found by sabotage on 2026-09-10. The live
//! simulation asserted that the faked old Neovim's output contains `0.10`.
//! With the guard deliberately disabled, so that startup falls straight
//! through into the 0.10-only code below it, the assertion STILL PASSED: the
//! run reached lazy.nvim and printed `markdown-preview.nvim ... v0.0.10`,
//! which contains `0.10`. The guard's own message appeared zero times. So
//! the one assertion here that runs real code, and the only one that could
//! catch a guard which reads correctly and does not fire, was satisfied by
//! plugin-clone noise.
//!
//! Two assertions replace it: the output must contain the guard's own
//! message, and it must NOT contain lazy.nvim's clone chatter, because a
//! guard that fired stops startup before lazy.nvim runs at all. Both were
//! confirmed red against the disabled guard.
//!
//! This is a pre-existing weak assertion in the shell suite, not a defect in
//! the shipped config. The tracked guard is correct and fires.

use dotfiles_test_support::repo::root as repo_root;
use dotfiles_test_support::skip;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The distinctive opening of the guard's own message.
///
/// Matched instead of a bare `0.10`, which plugin-clone output also contains.
/// Stops short of the version number so this constant does not have to change
/// when the floor rises; the version is asserted separately.
const GUARD_MESSAGE_PREFIX: &str = "This config needs Neovim";

fn nvim_dir() -> PathBuf {
    repo_root().join(".config/nvim")
}

fn init_lua() -> PathBuf {
    nvim_dir().join("init.lua")
}

fn health_lua() -> PathBuf {
    nvim_dir().join("lua/dotfiles/health.lua")
}

/// The file's lines with Lua comments stripped.
///
/// Both files now EXPLAIN the 0.10 floor in a comment above the guard, so a
/// raw search for `vim.uv` matched prose and reported the explanation as the
/// violation it was describing.
fn code_lines(path: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()));
    text.lines()
        .map(|line| match line.find("--") {
            Some(index) => line[..index].to_string(),
            None => line.to_string(),
        })
        .collect()
}

/// The 1-based line of the first 0.10-only `vim.uv` access, if any.
///
/// `vim.uv or vim.loop` is the compatibility spelling and is excluded
/// wherever it appears on the line: `(vim.uv or vim.loop).fs_stat(...)` is
/// the real call site in `init.lua` and is not a 0.10+ access.
fn first_modern_use(path: &Path) -> Option<usize> {
    code_lines(path)
        .iter()
        .position(|line| line.contains("vim.uv") && !line.contains("vim.uv or vim.loop"))
        .map(|index| index + 1)
}

/// The 1-based line of the first version guard, if any.
///
/// `vim.version` predates 0.7, so the guard itself runs on the versions it
/// rejects.
fn first_guard(path: &Path) -> Option<usize> {
    code_lines(path)
        .iter()
        .position(|line| line.contains("vim.version.ge") || line.contains("vim.version()"))
        .map(|index| index + 1)
}

#[test]
fn the_two_lua_files_exist() {
    assert!(
        init_lua().is_file(),
        "no nvim entrypoint at {}",
        init_lua().display()
    );
    assert!(
        health_lua().is_file(),
        "no health module at {}",
        health_lua().display()
    );
}

/// Asserted as line ORDER rather than as the mere presence of a guard. A
/// correct check sitting below the first 0.10+ call is exactly the bug this
/// suite exists for, and a presence-only assertion passes on that bug.
#[test]
fn each_file_guards_the_version_before_its_first_modern_use() {
    for target in [init_lua(), health_lua()] {
        let name = target
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("the file")
            .to_string();

        let Some(modern) = first_modern_use(&target) else {
            // No bare `vim.uv` at all is a valid way to satisfy this: the
            // compatibility spelling alone needs no ordering.
            skip(&format!(
                "{name} has no bare vim.uv access, so there is no ordering to check"
            ));
            continue;
        };

        let guard = first_guard(&target);
        assert!(
            guard.is_some(),
            "{name} reaches vim.uv at line {modern} with no version guard anywhere"
        );
        let Some(guard) = guard else { continue };
        assert!(
            guard < modern,
            "{name} guards the version at line {guard}, below its first vim.uv \
             use at line {modern}; a guard below the code it guards is not a guard"
        );
    }
}

/// `vim.health` is only reachable under `:checkhealth`, and a startup guard
/// that used it would print nothing on the crash path this suite is about.
#[test]
fn the_startup_guard_reports_through_an_api_old_versions_have() {
    let text = std::fs::read_to_string(init_lua()).expect("init.lua is readable");
    assert!(
        text.contains("vim.notify"),
        "init.lua does not report the floor through vim.notify, which is what \
         runs at startup"
    );
    // The message has to name the required version. "This config needs a
    // newer Neovim" tells the reader nothing they can act on; the whole
    // remedy here is knowing which version to get.
    assert!(
        text.contains("0.10"),
        "init.lua does not name the required version, so the message gives \
         the reader nothing to act on"
    );
}

/// Everything above reads the source. This runs the real config against a
/// faked old Neovim, and is the only assertion here that would catch a guard
/// that reads correctly and throws anyway.
///
/// The VERSION is what gets faked, not only `vim.uv`. Nilling the field alone
/// leaves `vim.version()` reporting the real modern Neovim, so the guard
/// correctly passes and the run proceeds into lazy.nvim, which then clones
/// and compiles the entire plugin set over the network. Faking both is what
/// reproduces an old Neovim: the guard must trip on the version and return
/// before anything touches the missing field.
#[test]
fn a_neovim_without_vim_uv_is_told_the_version_instead_of_crashing() {
    if Command::new("nvim").arg("--version").output().is_err() {
        skip("nvim is not installed, so the live old-version simulation cannot run");
        return;
    }

    // XDG_CONFIG_HOME rather than `-u <init>`. `-u` loads the file without
    // putting the config's lua/ directory on the runtimepath, so the run
    // dies at line 1 on `require 'settings'`: a rig artifact that reports a
    // failure having never reached the code under test.
    //
    // A scratch HOME keeps the run off the real plugin directory: lazy.nvim
    // would otherwise clone on a cold cache and make this suite need the
    // network.
    let scratch = tempfile::tempdir().expect("a scratch home");
    let config = scratch.path().join(".config");
    std::fs::create_dir_all(&config).expect("a scratch config directory");
    std::os::unix::fs::symlink(nvim_dir(), config.join("nvim")).expect("the config links");

    let fake_old = "lua vim.uv = nil; \
         local v = setmetatable({major=0,minor=9,patch=5}, \
         {__tostring=function() return \"0.9.5\" end}); \
         vim.version = setmetatable({ge=function() return false end}, \
         {__call=function() return v end})";

    let output = Command::new("nvim")
        .args(["--headless", "--cmd", fake_old, "-c", "q"])
        .env("HOME", scratch.path())
        .env("XDG_CONFIG_HOME", &config)
        .output()
        .expect("nvim spawns");

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        !combined.contains("E5113") && !combined.contains("attempt to index"),
        "a Neovim without vim.uv crashed on the config: {combined}"
    );

    // The GUARD'S OWN MESSAGE, not the bare string "0.10". The shell suite
    // asserted the latter, and it is satisfied by plugin-clone noise:
    // measured 2026-09-10 with the guard deliberately disabled, the run fell
    // through into lazy.nvim and printed `markdown-preview.nvim ... v0.0.10`,
    // which contains "0.10". So the assertion passed while the guard had
    // fired zero times. See the module doc.
    assert!(
        combined.contains(GUARD_MESSAGE_PREFIX),
        "a Neovim without vim.uv never saw the guard message; expected \
         {GUARD_MESSAGE_PREFIX:?} in: {combined}"
    );
    assert!(
        combined.contains("0.10"),
        "the guard message does not name the required version; it said: {combined}"
    );

    // A guard that fired means the run stopped before lazy.nvim. If plugins
    // are being cloned, the guard did NOT stop startup, whatever else the
    // output happens to contain.
    assert!(
        !combined.contains("Running task clone"),
        "the guard did not stop startup: the run reached lazy.nvim and began \
         cloning plugins, which is the network-dependent path faking the \
         version exists to prevent"
    );
}
