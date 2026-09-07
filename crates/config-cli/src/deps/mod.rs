//! Everything inside this module knows `deps_core`. Nothing outside it does.
//!
//! This is the whole edge. The process environment is read here, exactly
//! once, into [`selection::Environment`]; the manifest is loaded here; the
//! manager, the elevation and the root resolver are resolved here; and the
//! two output streams are written here. Every module beneath this one takes
//! what it needs as an argument, which is what makes them testable without a
//! process environment no two tests may share.
//!
//! Nothing here computes an exit code. `deps_core::exit_status` owns the
//! verb-to-code mapping and this module reaches it only through
//! `Rendered::exit_code`, which is what stops status 1 from coming to mean
//! eight things again.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use deps_core::{
    Elevation, Manifest, PathRoot, Planning, Rendered, Requirements, Verb, describe, reconcile,
    render, run_to_fixpoint,
};

use crate::{DepsArgs, DepsVerb};

pub mod catalog;
pub mod elevation;
pub mod gather;
pub mod installer;
pub mod selection;

/// The exit code every caller error reports.
///
/// A named constant because three different failures reach it (a manifest
/// that will not load, a selection naming an unknown dependency, a
/// requirement edge naming one) and `deps-docs.test.sh` uses the number as
/// its oracle for "the caller got something wrong".
const CALLER_ERROR: u8 = 2;

/// Where the shipped conf files live, relative to the dotfiles root.
///
/// `retired-check-deps:37` resolves `deps.conf` against its own directory.
/// `selection::conf_paths` has no script directory and yields the bare name,
/// so this module supplies the directory that convention means. Without it a
/// run from any working directory but `.scripts/deps` would read no manifest
/// and report an empty, passing check.
const SHIPPED_CONF_DIR: &str = ".scripts/deps";

/// Run one `deps` verb.
///
/// Returns the exit code `deps_core::exit_status` decided, never one computed
/// here.
pub(crate) fn run(verb: DepsVerb) -> ExitCode {
    let (arguments, wanted) = match &verb {
        DepsVerb::Check(arguments) => (arguments, Verb::Check),
        DepsVerb::Install(arguments) => (arguments, Verb::Install),
    };
    // `--dry-run` is its own verb to `deps_core`, not a flag on the other
    // two: a preview that reported as `Install` would call a machine that is
    // merely not ready an install failure.
    let effective = if arguments.dry_run { Verb::DryRun } else { wanted };

    match execute(arguments, effective) {
        Ok(rendered) => {
            print!("{}", rendered.stdout);
            eprint!("{}", rendered.stderr);
            ExitCode::from(rendered.exit_code)
        }
        Err(message) => {
            eprintln!("config-cli: {message}");
            ExitCode::from(CALLER_ERROR)
        }
    }
}

/// Assemble the run and produce its two streams and its exit code.
///
/// `Err` is a caller error and nothing else: every failure the run itself
/// can report travels inside the `Report` this returns. Splitting them means
/// the `CALLER_ERROR` above is reached from one place rather than from every
/// branch that could have picked a different number.
fn execute(arguments: &DepsArgs, verb: Verb) -> Result<Rendered, String> {
    let environment = read_environment();
    let sources = selection::conf_paths(&environment);
    let root = dotfiles_root();
    let manifest = load_manifest(&sources, root.as_deref())?;

    let selection = selection::selection_from(&arguments.only, &manifest)
        .map_err(|error| describe_plan_error(&error))?;
    let requirements = requirements()?;
    let packages = catalog::packages();
    let manager = selection::resolve_manager();
    let elevation = resolve_elevation()?;

    let planning = Planning {
        manifest: &manifest,
        manager,
        selection: &selection,
        requirements: &requirements,
        elevation,
        packages: &packages,
    };
    let resolver = HostRoots { manager };

    match verb {
        // A check installs nothing, so it reaches no installer at all rather
        // than reaching one wired to refuse. A refusing installer would put
        // `NotAutomatable` on every missing row, which reads as "this machine
        // cannot install it" when the truth is that nobody asked.
        Verb::Check => Ok(check(&manifest, &resolver)),
        Verb::DryRun => dry_run(&planning, &resolver, &manifest, manager),
        Verb::Install => {
            let approval = if arguments.yes {
                installer::Approval::Assumed
            } else {
                installer::Approval::Ask
            };
            let installers = installer::wire(manager, elevation, approval);
            let (report, _events) =
                run_to_fixpoint(&planning, &installers, || gather::gather(&manifest, &resolver))
                    .map_err(|error| describe_plan_error(&error))?;
            Ok(render(&report, Verb::Install))
        }
    }
}

/// Report the world as it stands, changing nothing.
///
/// One gather and a `reconcile` over no outcomes. `reconcile` covers every
/// manifest entry rather than only the planned steps, so the report names the
/// whole manifest even though this verb plans nothing.
fn check(manifest: &Manifest, resolver: &HostRoots) -> Rendered {
    let observations = gather::gather(manifest, resolver);
    let (report, _events) = reconcile(manifest, &[], &observations);
    render(&report, Verb::Check)
}

/// Preview the plan without reaching the fixpoint loop.
///
/// `describe` over the plan, never `run_to_fixpoint` with an installer whose
/// `perform` does nothing. An installer that secretly performs nothing is
/// precisely the shape that lets a dry run drift from the real run: the two
/// would share a type and not a code path, and nothing would notice when one
/// changed.
///
/// The report comes from `reconcile` over one gather with no outcomes, which
/// is the world as it stands. That is what the exit code must describe: a dry
/// run reports readiness, and readiness is a fact about the machine now.
fn dry_run(
    planning: &Planning<'_>,
    resolver: &HostRoots,
    manifest: &Manifest,
    manager: deps_core::PackageManager,
) -> Result<Rendered, String> {
    let observations = gather::gather(manifest, resolver);
    let (built, _events) = deps_core::plan(
        planning.manifest,
        planning.manager,
        planning.selection,
        planning.requirements,
        &observations,
        planning.elevation,
        planning.packages,
    )
    .map_err(|error| describe_plan_error(&error))?;

    // The same `Spawning` type the real run wires, so the previewed argv is
    // built by the code that would spawn it. Only `describe` is called here,
    // and `describe` is pure, so no separate preview-only installer exists to
    // drift from the one that runs.
    let describing = installer::Spawning::executing(manager, installer::Approval::Assumed);
    let descriptions = describe(&describing, &built);

    let (report, _reconcile_events) = reconcile(manifest, &[], &observations);
    let mut rendered = render(&report, Verb::DryRun);
    rendered.stdout.push_str(&render_descriptions(&descriptions));
    Ok(rendered)
}

/// Render each previewed action, one block per step.
///
/// Privilege and trust-root changes are disclosed on their own lines rather
/// than left for a reader to infer from the command text. Grepping a rendered
/// command for `sudo` is the string sniff this design exists to remove.
fn render_descriptions(descriptions: &[deps_core::ActionDescription]) -> String {
    use std::fmt::Write;

    let mut text = String::new();
    for description in descriptions {
        // `Write for String` is infallible, so the discarded `Err` describes
        // a case that cannot happen.
        let _ = writeln!(text, "  would    {}", description.summary);
        if description.privilege == deps_core::PrivilegeRequirement::Root {
            let _ = writeln!(text, "    needs root");
        }
        if description.changes_trust_root {
            let _ = writeln!(text, "    adds a package trust root");
        }
        if let Some(preview) = &description.command_preview {
            let _ = writeln!(text, "    {preview}");
        }
    }
    text
}

/// Read the process environment this run depends on, once.
///
/// The only `std::env` read of `DEPS_CONF` and `DEPS_LOCAL_CONF` in the
/// binary. Every branch beneath this reads the returned struct, so a test can
/// reach every one of them without mutating process-global state that would
/// make two tests order-dependent on each other.
fn read_environment() -> selection::Environment {
    selection::Environment {
        deps_conf: std::env::var_os("DEPS_CONF"),
        deps_local_conf: std::env::var_os("DEPS_LOCAL_CONF"),
        platform: host_platform(),
    }
}

/// The platform this machine is, by the rule `platform.sh` uses.
///
/// `std::env::consts::OS` rather than spawning `uname -s`: the values are
/// decided at compile time for the target this binary was built for, which is
/// the same fact `uname` reports and one fewer process to spawn.
fn host_platform() -> selection::Platform {
    match std::env::consts::OS {
        "macos" => selection::Platform::MacOs,
        "linux" => selection::Platform::Linux,
        _ => selection::Platform::Unknown,
    }
}

/// The dotfiles root, matching `config-manifest`'s own resolution.
///
/// `DOTFILES_ROOT` wins, then `HOME`. Returns `None` when neither is set,
/// which leaves a relative conf path relative to the working directory rather
/// than silently rooting it somewhere the caller did not choose.
fn dotfiles_root() -> Option<PathBuf> {
    std::env::var_os("DOTFILES_ROOT")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// Load the manifest, qualifying each relative conf path against the root.
///
/// An absolute path is used as given: `DEPS_CONF=/tmp/x/deps.conf` names one
/// file and must not be reinterpreted. Only the bare defaults
/// `selection::conf_paths` produces are joined onto `<root>/.scripts/deps`,
/// which is the directory `retired-check-deps:37` resolves against.
fn load_manifest(
    sources: &selection::ManifestSources,
    root: Option<&Path>,
) -> Result<Manifest, String> {
    // Mutated in place rather than rebuilt as a struct literal.
    // `ManifestSources` carries a private field recording whether the
    // manifest choice was explicit, and that fact is `conf_paths`'s to decide.
    // Rewriting the paths must not be able to change it.
    let mut qualified = sources.clone();
    for path in &mut qualified.paths {
        *path = qualify(path, root);
    }
    selection::load_manifest(&qualified).map_err(|error| describe_load_error(&error))
}

/// Root one conf path against the shipped conf directory, if it needs it.
fn qualify(path: &Path, root: Option<&Path>) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    match root {
        Some(root) => root.join(SHIPPED_CONF_DIR).join(path),
        None => path.to_path_buf(),
    }
}

/// The requirement graph, validated against every shipped dependency.
///
/// A bad edge is a caller error in the same sense a bad flag is: the table is
/// this crate's own, so an edge naming a dependency no conf file holds is a
/// mistake in the source rather than a fact about the machine. Every
/// offending edge is reported, not the first, so one run names every typo.
fn requirements() -> Result<Requirements, String> {
    catalog::requirements(&catalog::every_shipped_dependency()).map_err(|edges| {
        let listed = edges
            .iter()
            .map(|edge| {
                format!("{} requires unknown {}", edge.dependent.as_str(), edge.unknown.as_str())
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("the requirement table names dependencies no conf file holds: {listed}")
    })
}

/// This process's elevation, or a caller error naming why it is unknown.
///
/// `retired-check-deps:175` guarded the same `id -u` call with a fallback that
/// assumed non-root. Assuming here would wire a privileged installer that
/// cannot elevate, so the failure is reported instead.
fn resolve_elevation() -> Result<Elevation, String> {
    elevation::resolve().map_err(|error| format!("cannot determine elevation: {error}"))
}

/// Resolves the roots a manifest check can name, against this machine.
///
/// Holds the manager because `PathRoot::BrewPrefix` is only answerable where
/// brew is the manager. A machine without brew resolves that root to `None`,
/// which `gather` reports as `Observation::Unresolvable` rather than as an
/// absent file: "brew is not installed" and "the file brew would have
/// provided is missing" have different remedies.
struct HostRoots {
    manager: deps_core::PackageManager,
}

impl gather::RootResolver for HostRoots {
    fn resolve(&self, root: PathRoot) -> Option<PathBuf> {
        match root {
            PathRoot::Home => std::env::var_os("HOME").map(PathBuf::from),
            PathRoot::MacApplications => Some(PathBuf::from("/Applications")),
            PathRoot::BrewPrefix => self.brew_prefix(),
        }
    }
}

impl HostRoots {
    /// `brew --prefix`, or `None` where brew is not this machine's manager.
    ///
    /// Spawned rather than hardcoded to `/opt/homebrew` or `/usr/local`,
    /// because the two differ by architecture and a wrong guess would report
    /// every brew-rooted check absent on the other one.
    fn brew_prefix(&self) -> Option<PathBuf> {
        if self.manager != deps_core::PackageManager::Brew {
            return None;
        }
        let output = std::process::Command::new("brew").arg("--prefix").output().ok()?;
        if !output.status.success() {
            return None;
        }
        let prefix = String::from_utf8(output.stdout).ok()?;
        let trimmed = prefix.trim();
        if trimmed.is_empty() {
            return None;
        }
        Some(PathBuf::from(trimmed))
    }
}

/// Explain a `PlanError` in one line, for stderr.
fn describe_plan_error(error: &deps_core::PlanError) -> String {
    use deps_core::PlanError;

    match error {
        PlanError::UnknownDependency { name, did_you_mean } => match did_you_mean {
            Some(suggestion) => format!(
                "no dependency named {}; did you mean {}?",
                name.as_str(),
                suggestion.as_str()
            ),
            None => format!("no dependency named {}", name.as_str()),
        },
        PlanError::MalformedSelector { raw } => {
            format!("--only value is not a dependency name: {}", raw.as_str())
        }
        PlanError::ManifestParse { path, detail } => {
            format!("{} does not parse: {detail:?}", path.as_str())
        }
        PlanError::ManifestVersion { found, supported } => {
            format!("manifest declares version {found}; this build understands {supported}")
        }
        other => format!("{other:?}"),
    }
}

/// Explain a `selection::LoadError` in one line, for stderr.
fn describe_load_error(error: &selection::LoadError) -> String {
    match error {
        selection::LoadError::Parse { detail } => {
            format!("the manifest does not parse: {detail:?}")
        }
        selection::LoadError::Io { path, message } => {
            format!("cannot read {}: {message}", path.display())
        }
    }
}
