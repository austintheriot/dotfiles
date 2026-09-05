use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use config_manifest::plan::{TargetSnapshot, plan_sync};
use config_manifest::{check, git, manifest};

const SYNC_BRANCHES: [&str; 2] = ["mac", "linux"];

/// Keeps the mac and linux branches of the dotfiles repo in step on the paths
/// the .sync-manifest marks as shared.
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

    /// The dotfiles worktree to operate on. Defaults to the home directory.
    #[arg(long, env = "DOTFILES_ROOT", value_name = "dir", global = true)]
    root: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Report drift between two refs on the shared paths.
    Check(CheckArgs),
    /// Copy the shared paths from the current branch onto the other branch.
    Sync(SyncArgs),
}

#[derive(Args)]
struct CheckArgs {
    /// The ref whose .sync-manifest defines the shared paths.
    #[arg(default_value = "origin/mac")]
    ref_a: String,
    /// The ref to compare against.
    #[arg(default_value = "origin/linux")]
    ref_b: String,
}

#[derive(Args)]
struct SyncArgs {
    /// Print the edits that would be committed and change nothing.
    #[arg(long)]
    dry_run: bool,
    /// The branch to sync onto; must be the other of mac and linux.
    #[arg(long, value_name = "branch")]
    to: Option<String>,
}

fn main() -> ExitCode {
    // `--stamp` and `--version` are called directly by .scripts/config/config-build
    // and compared by the pre-push stamp check, so both stay global flags.
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => return exit_from_clap_error(error),
    };

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
        Some(Command::Check(args)) => match run_check(&args, cli.root.as_ref()) {
            Ok(code) => ExitCode::from(code),
            Err(error) => {
                eprintln!("check-branch-drift: {error:#}");
                ExitCode::from(1)
            }
        },
        Some(Command::Sync(args)) => match run_sync(&args, cli.root.as_ref()) {
            Ok(code) => ExitCode::from(code),
            Err(error) => {
                eprintln!("config-manifest sync: {error:#}");
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

fn run_check(args: &CheckArgs, root: Option<&PathBuf>) -> anyhow::Result<u8> {
    let ref_a = args.ref_a.as_str();
    let ref_b = args.ref_b.as_str();

    let repo = git::Git::discover(&dotfiles_root(root)?);
    let Some(manifest_text) = repo.show_manifest(ref_a)? else {
        eprintln!("check-branch-drift: could not read .sync-manifest from {ref_a}");
        return Ok(1);
    };
    let manifest = manifest::parse(&manifest_text)?;
    let listing_a = repo.ls_tree(ref_a)?;
    let listing_b = repo.ls_tree(ref_b)?;

    let rendered = check::render(
        &check::check(&manifest, &listing_a, &listing_b),
        ref_a,
        ref_b,
    );
    std::io::stdout().write_all(rendered.stdout.as_bytes())?;
    std::io::stderr().write_all(rendered.stderr.as_bytes())?;
    Ok(rendered.exit_code)
}

fn run_sync(parsed: &SyncArgs, root: Option<&PathBuf>) -> anyhow::Result<u8> {
    let repo = git::Git::discover(&dotfiles_root(root)?);

    let Some(source) = repo.current_branch()? else {
        eprintln!("config-manifest sync: HEAD is detached; check out mac or linux first");
        return Ok(1);
    };
    if !SYNC_BRANCHES.contains(&source.as_str()) {
        eprintln!(
            "config-manifest sync: refusing to sync from branch {source}; only mac and linux are synced"
        );
        return Ok(1);
    }
    let target = match parsed.to.clone() {
        Some(named) if !SYNC_BRANCHES.contains(&named.as_str()) || named == source => {
            eprintln!("config-manifest sync: --to must name the other of mac and linux");
            return Ok(2);
        }
        Some(named) => named,
        None => SYNC_BRANCHES
            .iter()
            .find(|name| **name != source)
            .map(|name| name.to_string())
            .unwrap_or_default(),
    };

    if repo.checked_out_branches()?.contains(&target) {
        eprintln!(
            "config-manifest sync: refusing, {target} is checked out in a worktree; sync commits onto it directly and would leave that worktree dirty"
        );
        return Ok(1);
    }

    if repo.has_remote("origin")? {
        repo.fetch("origin")?;
        let local = repo.rev_parse(&target)?;
        let remote = repo.rev_parse(&format!("origin/{target}"))?;
        if local != remote {
            eprintln!(
                "config-manifest sync: refusing, local {target} is not at origin/{target}; the other machine may have pushed work this would overwrite."
            );
            eprintln!(
                "  fetch and integrate first, for example: config pull origin {target}:{target}"
            );
            return Ok(1);
        }
    } else {
        eprintln!(
            "config-manifest sync: no origin remote; skipping the fetch and the origin comparison"
        );
    }

    let Some(manifest_text) = repo.show_manifest(&source)? else {
        eprintln!("config-manifest sync: could not read .sync-manifest from {source}");
        return Ok(1);
    };
    let manifest = manifest::parse(&manifest_text)?;
    let source_listing = repo.ls_tree("HEAD")?;
    let target_listing = repo.ls_tree(&target)?;
    let target_head = repo.rev_parse(&target)?;

    let dirty = repo.dirty_paths()?;
    let dirty_shared = manifest.partition(dirty.iter()).shared;
    if dirty_shared.iter().next().is_some() {
        eprintln!(
            "config-manifest sync: warning, uncommitted changes under shared paths are not synced (sync copies committed content):"
        );
        for path in dirty_shared.iter() {
            eprintln!("  {path}");
        }
    }

    let union: Vec<&config_manifest::path::RelPath> = source_listing
        .paths()
        .chain(target_listing.paths())
        .collect();
    let shared = manifest.partition(union.into_iter()).shared;
    let target_snapshot = TargetSnapshot::new(target_listing, target_head);
    let plan = plan_sync(&shared, &source_listing, &target_snapshot);

    if parsed.dry_run {
        for edit in plan.edits() {
            println!("{edit}");
        }
        return Ok(0);
    }
    if plan.is_empty() {
        println!(
            "config-manifest sync: nothing to sync, {target} already matches {source} on every shared path"
        );
        return Ok(0);
    }

    let short = repo.output_short(&source)?;
    let message = format!("Sync shared paths from {source} ({short})");
    let commit = repo.commit_plan(&plan, &target, &message)?;
    println!(
        "config-manifest sync: committed {} to {target} ({} edit(s))",
        &commit.as_str()[..12],
        plan.edits().len()
    );

    let synced_listing = repo.ls_tree(&target)?;
    let report = check::check(&manifest, &source_listing, &synced_listing);
    let rendered = check::render(&report, &source, &target);
    let diverged = report
        .findings
        .iter()
        .any(|finding| matches!(finding, check::Finding::Diverged { .. }));
    if diverged {
        eprintln!("config-manifest sync: BUG: the branches still differ after syncing");
        std::io::stderr().write_all(rendered.stdout.as_bytes())?;
        std::io::stderr().write_all(rendered.stderr.as_bytes())?;
        return Ok(3);
    }
    if rendered.exit_code != 0 {
        std::io::stdout().write_all(rendered.stdout.as_bytes())?;
        std::io::stderr().write_all(rendered.stderr.as_bytes())?;
        eprintln!(
            "config-manifest sync: committed to {target}, but the manifest does not cover every tracked path; add rules for the unmatched paths above"
        );
        return Ok(1);
    }

    println!("next: config push origin {source} && config push origin {target}");
    Ok(0)
}
