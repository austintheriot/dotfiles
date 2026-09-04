mod check;
mod git;
mod manifest;
mod path;
mod tree;

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const USAGE: &str = "usage: config-manifest --version | check [ref-a] [ref-b]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") => {
            println!("config-manifest {VERSION}");
            ExitCode::SUCCESS
        }
        Some("check") => match run_check(&args[1..]) {
            Ok(code) => ExitCode::from(code),
            Err(error) => {
                eprintln!("check-branch-drift: {error:#}");
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
