#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
//! The `config-manifest` binary: the git-sync and stamp subcommands the
//! `config` shell command dispatches to.
//!
//! Argument parsing and stream writing live here; every decision the
//! subcommands make lives in the library, so the interesting behaviour is
//! testable without spawning this binary.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use config_manifest::{doctor, git, stamp};

/// Reports on the workspace crates' build stamps.
#[derive(Parser)]
// `subcommand_required` cannot coexist with the exclusive `--stamp` flag, so
// the missing-subcommand case is handled after parsing instead.
#[command(name = "config-manifest", version, disable_help_subcommand = true)]
struct Cli {
    /// Print the build-time stamp of the crate this binary was built from.
    ///
    /// Not `exclusive`: --root is env-backed, so a shell that exports
    /// DOTFILES_ROOT (config-build does) makes clap treat --root as supplied,
    /// and an exclusive --stamp would collide with it and exit 2.
    #[arg(long)]
    stamp: bool,

    /// Print the one-line description `config help` lists this command under.
    ///
    /// Not `exclusive`, for the same reason --stamp is not: --root is
    /// env-backed, so a shell that exports DOTFILES_ROOT makes clap treat
    /// --root as supplied, and an exclusive --describe would collide with it
    /// and exit 2.
    #[arg(long)]
    describe: bool,

    /// The dotfiles worktree to operate on. Defaults to the home directory.
    #[arg(long, env = "DOTFILES_ROOT", value_name = "dir", global = true)]
    root: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Compare installed binary stamps against the stamps pushed on a ref.
    ///
    /// Reads `<crate> <stamp>` lines on stdin, as `config-stamp --ref`
    /// prints them. pre-push pipes that output in; this subcommand never
    /// re-derives the workspace member list, since config-stamp already
    /// owns it.
    VerifyStamps(VerifyStampsArgs),
    /// Report installed binaries that do not match their source.
    ///
    /// Silent when every binary is current. This subcommand owns its whole
    /// gather: it reads the expected stamps out of the worktree itself and
    /// probes each installed binary's `--stamp`, rather than being handed
    /// both sides on stdin.
    Doctor,
}

#[derive(Args)]
struct VerifyStampsArgs {
    /// The ref the pushed stamps on stdin were read from. Used only in
    /// messages; the comparison itself only needs the two stamp lists.
    #[arg(long = "ref", value_name = "ref")]
    reference: String,
}

fn main() -> ExitCode {
    // `--stamp` and `--version` are called directly by .scripts/config/config-build
    // and compared by the pre-push stamp check, so both stay global flags.
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => return exit_from_clap_error(error),
    };

    // config-help builds its listing by running `config-<sub> --describe` and
    // formatting each answer with `printf '  %-14s %s\n'`, so the contract is
    // exactly one line on stdout and exit 0. Handled before every other
    // branch, because asking a command what it does must not run it.
    //
    // Unreached today: `config doctor` is a shim whose own `# help:` line is
    // what the listing shows. This exists for the port that removes the shim,
    // which leaves no shell script to hold the description.
    if cli.describe {
        println!("Report installed binaries that do not match their source");
        return ExitCode::SUCCESS;
    }

    if cli.stamp {
        // `option_env!` is read at compile time, so cargo rebuilds when
        // CONFIG_MANIFEST_STAMP changes. The stamp travels inside the
        // binary, so a stale or foreign config-manifest on PATH cannot
        // report a stamp it was not built with.
        println!(
            "{}",
            option_env!("CONFIG_MANIFEST_STAMP").unwrap_or("unstamped")
        );
        return ExitCode::SUCCESS;
    }

    match cli.command {
        Some(Command::VerifyStamps(args)) => match run_verify_stamps(&args, cli.root.as_ref()) {
            Ok(code) => ExitCode::from(code),
            Err(error) => {
                eprintln!("config-manifest verify-stamps: {error:#}");
                ExitCode::from(1)
            }
        },
        Some(Command::Doctor) => match run_doctor(cli.root.as_ref()) {
            Ok(code) => ExitCode::from(code),
            Err(error) => {
                eprintln!("config doctor: {error:#}");
                ExitCode::from(1)
            }
        },
        None => {
            // A bare invocation is a usage error, so the help goes to stderr
            // and the exit code stays 2, matching every other usage error.
            let mut command = <Cli as clap::CommandFactory>::command();
            let _ = command.write_help(&mut std::io::stderr());
            ExitCode::from(2)
        }
    }
}

/// Clap exits 2 on a usage error and 0 on `--help` or `--version`, but writes
/// help to stdout and errors to stderr; this keeps that split while returning
/// the exit codes config-build and the shell suite already expect.
fn exit_from_clap_error(error: clap::Error) -> ExitCode {
    if error.use_stderr() {
        let _ = error.print();
        ExitCode::from(2)
    } else {
        let _ = error.print();
        ExitCode::SUCCESS
    }
}

fn dotfiles_root(root: Option<&PathBuf>) -> anyhow::Result<PathBuf> {
    if let Some(root) = root {
        return Ok(root.clone());
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("neither DOTFILES_ROOT nor HOME is set")
}

/// Reads pushed stamps from stdin as `config-stamp --ref` prints them: one
/// `<crate> <stamp>` line per crate, where the stamp itself contains colons.
///
/// Splits each line on the first space only and treats the remainder as one
/// opaque string. Splitting on `:` (or taking the second whitespace token)
/// would shred `<crate-tree>:<lock-blob>:<workspace-blob>` into three fields
/// and compare none of them correctly, which is the one way this subcommand
/// can misread every stamp.
fn read_pushed_stamps<R: Read>(mut input: R) -> anyhow::Result<Vec<(String, String)>> {
    let mut text = String::new();
    input.read_to_string(&mut text).context("reading pushed stamps from stdin")?;
    Ok(text.lines().filter_map(stamp::parse_stamp_line).collect())
}

/// Whether a workspace crate has an installed binary to gate at all.
///
/// A crate with no `src/main.rs` is a library: it has no `--stamp` to
/// report and nothing pre-push can execute. Its content is already covered
/// by the stamp of whichever binary crate depends on it, the same
/// reasoning pre-push used in shell before this subcommand existed. Such a
/// crate must not enter `verify()` on either side, since entering it only
/// on the pushed side would render it `NotBuilt` for a crate no binary was
/// ever going to report.
fn has_installed_binary(root: &Path, crate_name: &str) -> bool {
    root.join("crates").join(crate_name).join("src/main.rs").is_file()
}

/// Probes the installed binary for one crate's build-time stamp.
fn built_stamp_for(root: &Path, crate_name: &str) -> Option<String> {
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
    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}

fn run_verify_stamps(args: &VerifyStampsArgs, root: Option<&PathBuf>) -> anyhow::Result<u8> {
    let root = dotfiles_root(root)?;
    let all_pushed = read_pushed_stamps(std::io::stdin())?;

    // The crate list comes from the pushed side, which came from
    // config-stamp, not from a list re-derived here. Library crates are
    // dropped from both sides before comparison, since neither side has a
    // stamp verdict to offer for a crate with no installed binary.
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
        eprintln!("config-manifest verify-stamps: refusing push of {}", args.reference);
    }
    Ok(rendered.exit_code)
}

/// Gathers both sides and renders `doctor`'s report.
///
/// `doctor` owns its whole gather rather than shelling out to
/// `config-stamp`: `git::workspace_stamps` reads the expected side directly
/// out of the worktree, and this function probes each installed binary
/// itself. A crate with no `src/main.rs` is a library with no `--stamp` to
/// report, so it is dropped from the expected side before diagnosing;
/// otherwise every push would report the library-only crate `NotInstalled`,
/// which is not a defect since nothing was ever going to install it.
fn run_doctor(root: Option<&PathBuf>) -> anyhow::Result<u8> {
    let root = dotfiles_root(root)?;
    let repo = git::Git::discover(&root);
    let all_expected = repo.workspace_stamps()?;

    let expected: std::collections::BTreeMap<doctor::CrateName, doctor::Stamp> = all_expected
        .into_iter()
        .filter(|(crate_name, _)| has_installed_binary(&root, crate_name.as_str()))
        .collect();

    let installed: std::collections::BTreeMap<doctor::CrateName, doctor::Stamp> = expected
        .keys()
        .filter_map(|crate_name| {
            let stamp_text = built_stamp_for(&root, crate_name.as_str())?;
            let stamp = doctor::Stamp::parse(&stamp_text).ok()?;
            Some((crate_name.clone(), stamp))
        })
        .collect();

    let findings = doctor::diagnose(&installed, &expected);
    match doctor::render(&findings) {
        Some(report) => {
            eprint!("{report}");
            Ok(1)
        }
        None => Ok(0),
    }
}
