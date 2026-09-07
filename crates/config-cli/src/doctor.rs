//! `config doctor`: report installed binaries that do not match their source.
//!
//! Ported from `config-manifest`'s `doctor` subcommand. Every decision still
//! lives in `config_manifest::doctor`, which is a pure library module; this
//! file is the gather and the exit code. `.scripts/config/config-doctor` stays
//! as a shim so the dispatcher still resolves the verb: `.scripts/config/config`
//! finds a verb only by testing `[ -x "$here/config-<verb>" ]` and otherwise
//! falls through to git, with no `config-cli` fallback, so deleting the file
//! would turn `config doctor` into `git doctor` permanently.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use config_manifest::{doctor, git};

use crate::stamps::{built_stamp_for, dotfiles_root, has_installed_binary};

/// `config doctor`'s arguments: the worktree to diagnose, and nothing else.
///
/// Reports which workspace binaries were built from source that has since
/// changed, and names the command that fixes it. Silent when everything is
/// current, so it is safe to run habitually.
///
/// There is no runtime freshness check anywhere else, by measurement: a binary
/// verifying its own stamp costs about 124ms per invocation and the hot path
/// runs before every shell prompt, while a rebuild triggered from a prompt
/// hook serializes every pane behind cargo's build lock. The guarantee is at
/// pre-push, and this command is how you ask earlier.
#[derive(clap::Args)]
// `name` rather than the derived line: clap renders subcommand help under
// `<parent bin_name> <subcommand>`, which would say `config-cli doctor`. The
// dispatcher's own name is `config`, and the shim's `# usage:` block says
// `config doctor`, so the two must agree.
#[command(name = "config doctor", override_usage = "config doctor")]
pub(crate) struct DoctorArgs {
    /// The dotfiles worktree to diagnose. Defaults to the home directory.
    #[arg(long, env = "DOTFILES_ROOT", value_name = "dir")]
    root: Option<PathBuf>,
}

/// Run `config doctor`.
///
/// Exit 0 is silence and 1 is a report, which `tests/config.test.sh` pins in
/// both directions. Exit 1 also covers a failed gather, because a diagnosis
/// that could not be reached is not a clean bill of health.
pub(crate) fn run(arguments: DoctorArgs) -> ExitCode {
    match gather_and_report(arguments.root.as_ref()) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("config doctor: {error:#}");
            ExitCode::from(1)
        }
    }
}

/// Gathers both sides and renders `doctor`'s report.
///
/// `doctor` owns its whole gather rather than shelling out to `config-stamp`:
/// `git::workspace_stamps` reads the expected side directly out of the
/// worktree, and this function probes each installed binary itself. A crate
/// with no `src/main.rs` is a library with no `--stamp` to report, so it is
/// dropped from the expected side before diagnosing; otherwise every run would
/// report the library-only crate `NotInstalled`, which is not a defect since
/// nothing was ever going to install it.
fn gather_and_report(root: Option<&PathBuf>) -> anyhow::Result<u8> {
    let root = dotfiles_root(root)?;
    let repo = git::Git::discover(&root);
    let all_expected = repo.workspace_stamps()?;

    let expected: BTreeMap<doctor::CrateName, doctor::Stamp> = all_expected
        .into_iter()
        .filter(|(crate_name, _)| has_installed_binary(&root, crate_name.as_str()))
        .collect();

    let installed: BTreeMap<doctor::CrateName, doctor::Stamp> = expected
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
