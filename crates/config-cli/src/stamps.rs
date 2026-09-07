//! The stamp gather both `doctor` and `verify-stamps` need.
//!
//! Moved here from `config-manifest`'s binary, which owned both subcommands
//! before they became `config-cli` subcommands. The two share every part of
//! "which crates have an installed binary, and what stamp does each one
//! report", and they disagree only about where the expected side comes from:
//! `verify-stamps` is handed it on stdin, `doctor` reads it out of the
//! worktree. Keeping the shared half in one module is what stops the two from
//! drifting into two answers for "is this crate gated at all".

use std::path::{Path, PathBuf};

use anyhow::Context;

/// The worktree the stamps are read from.
///
/// `DOTFILES_ROOT` is how every test and the Docker image point the commands
/// at a repo other than the home directory, and `pre-push` pins it to the
/// pushed repo's working directory.
pub(crate) fn dotfiles_root(root: Option<&PathBuf>) -> anyhow::Result<PathBuf> {
    if let Some(root) = root {
        return Ok(root.clone());
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("neither DOTFILES_ROOT nor HOME is set")
}

/// Whether a workspace crate has an installed binary to gate at all.
///
/// A crate with no `src/main.rs` is a library: it has no `--stamp` to report
/// and nothing pre-push can execute. Its content is already covered by the
/// stamp of whichever binary crate depends on it, the same reasoning pre-push
/// used in shell before this check existed. Such a crate must not enter
/// `verify()` on either side, since entering it only on the pushed side would
/// render it `NotBuilt` for a crate no binary was ever going to report.
pub(crate) fn has_installed_binary(root: &Path, crate_name: &str) -> bool {
    root.join("crates").join(crate_name).join("src/main.rs").is_file()
}

/// Probes the installed binary for one crate's build-time stamp.
///
/// Returns `None` when the binary is absent, unrunnable, or answers with
/// nothing, which the callers read as "not installed" rather than as an
/// error: a missing binary is a finding, not a failure to reach one.
pub(crate) fn built_stamp_for(root: &Path, crate_name: &str) -> Option<String> {
    // Defaults to $HOME/.local/bin, matching config-build:37, which is the
    // script that decides where binaries go. This read used to default to
    // `root.join(".local/bin")`, and $DOTFILES_ROOT is $HOME on a developer
    // machine, so the two agreed there and disagreed everywhere else. In CI,
    // DOTFILES_ROOT is the checkout, so doctor looked in
    // <checkout>/.local/bin, found nothing, and reported "not installed" for
    // a binary config-build had just installed correctly.
    let bin_dir = std::env::var_os("CONFIG_BIN_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/bin")))
        .unwrap_or_else(|| root.join(".local/bin"));
    let binary = bin_dir.join(crate_name);
    let output = std::process::Command::new(&binary).arg("--stamp").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stamp = String::from_utf8(output.stdout).ok()?;
    let trimmed = stamp.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
