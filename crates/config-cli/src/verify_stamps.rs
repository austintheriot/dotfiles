//! `config-cli verify-stamps`: the pre-push comparison of installed binary
//! stamps against the stamps pushed on a ref.
//!
//! Ported from `config-manifest`'s `verify-stamps` subcommand. The comparison
//! policy still lives in `config_manifest::stamp`, which is pure; this file
//! reads stdin, probes the installed binaries, and owns the exit code.
//!
//! Not reachable through the `config` dispatcher, deliberately: `tests/pre-push`
//! is the only caller, and it resolves the binary from the hook's own location
//! rather than from `PATH` so a test can drive the hook against a fixture repo.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context;
use config_manifest::stamp;

use crate::stamps::{built_stamp_for, dotfiles_root, has_installed_binary};

/// `verify-stamps`'s arguments.
///
/// Reads `<crate> <stamp>` lines on stdin, as `config-stamp --ref` prints
/// them. pre-push pipes that output in; this subcommand never re-derives the
/// workspace member list, since config-stamp already owns it.
#[derive(clap::Args)]
pub(crate) struct VerifyStampsArgs {
    /// The ref the pushed stamps on stdin were read from. Used only in
    /// messages; the comparison itself only needs the two stamp lists.
    #[arg(long = "ref", value_name = "ref")]
    reference: String,

    /// The dotfiles worktree the pushed stamps describe. Defaults to the home
    /// directory; pre-push pins it to the pushed repo's working directory.
    #[arg(long, env = "DOTFILES_ROOT", value_name = "dir")]
    root: Option<PathBuf>,
}

/// Run `verify-stamps`.
///
/// The exit code is the gate: pre-push refuses the push on anything non-zero,
/// so a failed gather has to exit 1 rather than 0. A silent success on an
/// unreadable ref would let a stale binary through, which is the one outcome
/// this subcommand exists to prevent.
pub(crate) fn run(arguments: VerifyStampsArgs) -> ExitCode {
    match compare(&arguments) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("config-cli verify-stamps: {error:#}");
            eprintln!(
                "config-cli verify-stamps: refusing push of {}",
                arguments.reference
            );
            ExitCode::from(1)
        }
    }
}

/// Reads pushed stamps from stdin as `config-stamp --ref` prints them: one
/// `<crate> <stamp>` line per crate, where the stamp itself contains colons.
///
/// Splits each line on the first space only and treats the remainder as one
/// opaque string. Splitting on `:` (or taking the second whitespace token)
/// would shred `<crate-tree>:<lock-blob>:<workspace-blob>` into three fields
/// and compare none of them correctly, which is the one way this subcommand
/// can misread every stamp.
fn read_pushed_stamps<Input: Read>(mut input: Input) -> anyhow::Result<Vec<(String, String)>> {
    let mut text = String::new();
    input.read_to_string(&mut text).context("reading pushed stamps from stdin")?;
    Ok(text.lines().filter_map(stamp::parse_stamp_line).collect())
}

/// Compares the pushed side against the installed side and renders the verdict.
fn compare(arguments: &VerifyStampsArgs) -> anyhow::Result<u8> {
    let root = dotfiles_root(arguments.root.as_ref())?;
    let all_pushed = read_pushed_stamps(std::io::stdin())?;

    // The crate list comes from the pushed side, which came from config-stamp,
    // not from a list re-derived here. Library crates are dropped from both
    // sides before comparison, since neither side has a stamp verdict to offer
    // for a crate with no installed binary.
    let pushed: Vec<(String, String)> = all_pushed
        .into_iter()
        .filter(|(crate_name, _)| has_installed_binary(&root, crate_name))
        .collect();
    let built: Vec<(String, String)> = pushed
        .iter()
        .filter_map(|(crate_name, _)| {
            built_stamp_for(&root, crate_name).map(|built_stamp| (crate_name.clone(), built_stamp))
        })
        .collect();

    let rendered = stamp::render(&stamp::verify(&built, &pushed));
    std::io::stdout().write_all(rendered.stdout.as_bytes())?;
    std::io::stderr().write_all(rendered.stderr.as_bytes())?;
    if rendered.exit_code != 0 {
        eprintln!(
            "config-cli verify-stamps: refusing push of {}",
            arguments.reference
        );
    }
    Ok(rendered.exit_code)
}
