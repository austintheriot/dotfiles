use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context;
use config_manifest::plan::{TargetSnapshot, plan_sync};
use config_manifest::{check, git, manifest};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const USAGE: &str = "usage: config-manifest --version | --stamp | check [ref-a] [ref-b] | sync [--dry-run] [--to <branch>]";
const SYNC_BRANCHES: [&str; 2] = ["mac", "linux"];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") => {
            println!("config-manifest {VERSION}");
            ExitCode::SUCCESS
        }
        Some("--stamp") => {
            // `option_env!` is read at compile time, so cargo rebuilds when
            // CONFIG_MANIFEST_STAMP changes. The stamp travels inside the
            // binary, so a stale or foreign config-manifest on PATH cannot
            // report a stamp it was not built with.
            println!(
                "{}",
                option_env!("CONFIG_MANIFEST_STAMP").unwrap_or("unstamped")
            );
            ExitCode::SUCCESS
        }
        Some("check") => match run_check(&args[1..]) {
            Ok(code) => ExitCode::from(code),
            Err(error) => {
                eprintln!("check-branch-drift: {error:#}");
                ExitCode::from(1)
            }
        },
        Some("sync") => match run_sync(&args[1..]) {
            Ok(code) => ExitCode::from(code),
            Err(error) => {
                eprintln!("config-manifest sync: {error:#}");
                ExitCode::from(1)
            }
        },
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn dotfiles_root() -> anyhow::Result<PathBuf> {
    if let Some(root) = std::env::var_os("DOTFILES_ROOT") {
        return Ok(PathBuf::from(root));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("neither DOTFILES_ROOT nor HOME is set")
}

fn run_check(args: &[String]) -> anyhow::Result<u8> {
    if args.len() > 2 {
        eprintln!("{USAGE}");
        return Ok(2);
    }
    let ref_a = args.first().map(String::as_str).unwrap_or("origin/mac");
    let ref_b = args.get(1).map(String::as_str).unwrap_or("origin/linux");

    let repo = git::Git::discover(&dotfiles_root()?);
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

struct SyncArgs {
    dry_run: bool,
    to: Option<String>,
}

fn parse_sync_args(args: &[String]) -> Option<SyncArgs> {
    let mut parsed = SyncArgs {
        dry_run: false,
        to: None,
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--dry-run" => parsed.dry_run = true,
            "--to" => {
                index += 1;
                parsed.to = Some(args.get(index)?.clone());
            }
            _ => return None,
        }
        index += 1;
    }
    Some(parsed)
}

fn run_sync(args: &[String]) -> anyhow::Result<u8> {
    let Some(parsed) = parse_sync_args(args) else {
        eprintln!("{USAGE}");
        return Ok(2);
    };
    let repo = git::Git::discover(&dotfiles_root()?);

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
    let target = match parsed.to {
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
    if rendered.exit_code != 0 {
        eprintln!("config-manifest sync: BUG: the branches still differ after syncing");
        std::io::stderr().write_all(rendered.stdout.as_bytes())?;
        std::io::stderr().write_all(rendered.stderr.as_bytes())?;
        return Ok(3);
    }

    println!("next: config push origin {source} && config push origin {target}");
    Ok(0)
}
