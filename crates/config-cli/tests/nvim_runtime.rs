//! Does the installed Neovim resolve its own runtime tree?
//!
//! THE BUG THIS SUITE EXISTS FOR, reported 2026-09-08. The deps engine
//! installed Neovim from the official release tarball and copied only
//! `bin/nvim` out of the staging directory, discarding `share/nvim/runtime/`.
//! Neovim locates `$VIMRUNTIME` by walking up from its own executable looking
//! for `share/nvim/runtime`, so the orphaned binary searched
//! `~/.local/share/nvim/runtime`, found nothing, and fell back to the paths
//! compiled in on upstream's build machine. The user saw a `require`
//! traceback listing `/home/runner/work/neovim/neovim/.deps/...` and
//! `E484: Can't open file .../syntax/syntax.vim`.
//!
//! WHY NO EXISTING GATE CAUGHT IT, which is what shapes these assertions:
//!
//!   - CI's only Neovim assertion is `nvim --version | head -1` plus
//!     convergence to `present   neovim` (deps-check.yml, the Pop!_OS leg).
//!     An orphaned binary runs and reports its version perfectly well, so
//!     that leg was exercising the broken install and passing.
//!   - The manifest check is `command = "nvim"` with `min_version = "0.10"`,
//!     which asks whether a name resolves on PATH. Many things can put a name
//!     on PATH; only this install can put a working runtime beside it.
//!   - `lua/dotfiles/health.lua` checks the version and external executables
//!     and never asks whether Neovim can find its own runtime.
//!   - `tests/nvim-mason-runtimes.test.sh` greps config text and never runs
//!     Neovim at all.
//!
//! So the assertion here EXECUTES Neovim and reads what it reports about
//! itself. A test that inspects the installer's argv would be an
//! implementation mirror: it would pass on any rewrite that produced the same
//! commands and fail on a correct rewrite that produced different ones.
//!
//! EVERY CHECK IS PAIRED WITH A NEGATIVE CONTROL. `runtime_health` is called
//! twice: once on the real binary, expecting health, and once on a copy
//! deliberately placed away from its `share/` sibling, expecting exactly the
//! reported failure. Without the second call this suite would assert only
//! that a Neovim exists somewhere, which is the pre-satisfied-path shape that
//! produced the bug.

use std::path::{Path, PathBuf};
use std::process::Command;

/// What Neovim reports about its own runtime.
///
/// A struct rather than a bool so a failure names which of the three facts
/// broke. "nvim is unhealthy" sends a reader to the whole install; "runtime
/// dir does not exist" sends them to one directory.
#[derive(Debug)]
struct RuntimeHealth {
    /// The value of `$VIMRUNTIME` as the process itself resolves it.
    vimruntime: String,
    /// Whether that path is a directory that exists.
    runtime_dir_exists: bool,
    /// Whether a known runtime file is reachable through `runtimepath`.
    ///
    /// Asked through `nvim_get_runtime_file` rather than by joining paths in
    /// Rust: that is the same lookup every `require` and every `:syntax on`
    /// performs, so it fails when they would fail.
    syntax_file_found: bool,
}

impl RuntimeHealth {
    fn is_healthy(&self) -> bool {
        !self.vimruntime.is_empty() && self.runtime_dir_exists && self.syntax_file_found
    }
}

/// Ask a specific Neovim executable about its runtime.
///
/// `-u NONE` so the answer is about the INSTALL and not about this repo's
/// config. A config error would otherwise be reported as a broken runtime,
/// which is the wrong layer and the wrong fix.
///
/// Output is written with `io.write` to stdout rather than `print`, because
/// `print` in headless mode routes through the message system and arrives
/// interleaved with any warning Neovim decides to emit.
fn runtime_health(nvim: &Path) -> Result<RuntimeHealth, String> {
    let lua = r#"
        local runtime = vim.env.VIMRUNTIME or ''
        local dir_ok = runtime ~= '' and vim.fn.isdirectory(runtime) == 1
        local found = #vim.api.nvim_get_runtime_file('syntax/syntax.vim', false) > 0
        io.write(runtime .. '\n' .. tostring(dir_ok) .. '\n' .. tostring(found) .. '\n')
    "#;

    let output = Command::new(nvim)
        .args(["--headless", "-u", "NONE", "-c"])
        .arg(format!("lua {lua}"))
        .args(["-c", "qa!"])
        .output()
        .map_err(|error| format!("could not run {}: {error}", nvim.display()))?;

    // Deliberately NOT asserting on the exit status. `nvim --headless -c qa`
    // exits 0 even when startup raised, so status is not evidence of health;
    // the reported facts are.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    if lines.len() < 3 {
        return Err(format!(
            "expected three lines of runtime facts, got {stdout:?} (stderr: {:?})",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(RuntimeHealth {
        vimruntime: lines[0].trim().to_string(),
        runtime_dir_exists: lines[1].trim() == "true",
        syntax_file_found: lines[2].trim() == "true",
    })
}

/// The Neovim this machine would actually use, or `None` when none is installed.
///
/// Resolved through PATH rather than at a hardcoded location, because that is
/// what a shell, a git hook and this repo's own checks all do. macOS installs
/// Neovim through Homebrew and Linux through the release tarball, so a
/// hardcoded `~/.local/bin/nvim` would silently skip on one of the two
/// platforms this suite runs on.
fn installed_nvim() -> Option<PathBuf> {
    let raw = Command::new("sh").args(["-c", "command -v nvim"]).output().ok()?;
    let path = String::from_utf8_lossy(&raw.stdout).trim().to_string();
    if path.is_empty() { None } else { Some(PathBuf::from(path)) }
}

/// The installed Neovim can find its own runtime.
///
/// Skips rather than fails when Neovim is absent, and that is a deliberate
/// and uncomfortable choice: a skip is the shape that let the bootstrap bug
/// hide for months. It is right here only because this suite runs on
/// developer machines that legitimately may not have Neovim yet, and because
/// the machine where absence WOULD be a defect is covered by the deps
/// workflow, which installs Neovim and then asserts convergence. The negative
/// control below is what keeps the skip honest: it proves the assertion can
/// still fail.
#[test]
fn the_installed_neovim_resolves_its_runtime() {
    let Some(nvim) = installed_nvim() else {
        dotfiles_test_support::skip("no nvim on PATH, so there is no install to check");
        return;
    };

    let health = runtime_health(&nvim).expect("the installed nvim answers about its runtime");

    assert!(
        health.is_healthy(),
        "the installed nvim at {} cannot find its runtime tree, which is the \
         $VIMRUNTIME bug: {health:?}",
        nvim.display()
    );
}

/// NEGATIVE CONTROL: a binary separated from its runtime must be reported unhealthy.
///
/// This reproduces the exact defect. The tarball ships `bin/nvim` beside
/// `share/nvim/runtime`, and the old installer copied the first without the
/// second. Copying the real executable into a bare temp directory recreates
/// that, and the check above must call it broken.
///
/// Without this test the suite could pass while `runtime_health` always
/// returned healthy. It is the difference between "we ran an assertion" and
/// "we ran an assertion that discriminates".
///
/// A Homebrew Neovim is skipped here rather than tested: Homebrew's `bin/nvim`
/// is itself a symlink into a versioned Cellar prefix, so copying the LINK
/// target still lands next to the Cellar's `share/`, and the copy stays
/// healthy for a legitimate reason. The tarball install is where the invariant
/// is load-bearing, so the control runs there.
#[test]
fn a_neovim_separated_from_its_runtime_is_reported_broken() {
    let Some(nvim) = installed_nvim() else {
        dotfiles_test_support::skip("no nvim on PATH, so there is nothing to orphan");
        return;
    };

    let resolved = std::fs::canonicalize(&nvim).unwrap_or_else(|_| nvim.clone());
    if resolved.to_string_lossy().contains("/Cellar/") {
        eprintln!(
            "skip: a Homebrew nvim resolves into its Cellar prefix, so an \
             orphaned copy is still beside a real share/ tree"
        );
        return;
    }

    let scratch = tempfile::tempdir().expect("a temp dir for the orphaned copy");
    let orphan = scratch.path().join("nvim");
    std::fs::copy(&resolved, &orphan).expect("the executable copies");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mut permissions = std::fs::metadata(&orphan).expect("the copy has metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&orphan, permissions).expect("the copy is executable");
    }

    // The control's own precondition: the temp dir must NOT contain a runtime
    // tree, or this proves nothing.
    assert!(
        !scratch.path().join("share/nvim/runtime").exists(),
        "the orphan's directory must have no share/ tree for this control to mean anything"
    );

    match runtime_health(&orphan) {
        Ok(health) => assert!(
            !health.is_healthy(),
            "an nvim with no share/ sibling was reported HEALTHY, so the check \
             above cannot detect the bug it exists for: {health:?}"
        ),
        // A binary that cannot start at all is also a detected failure. It is
        // not the failure shape the bug had, so it is accepted but named.
        Err(reason) => eprintln!("the orphaned copy failed to report at all: {reason}"),
    }
}
