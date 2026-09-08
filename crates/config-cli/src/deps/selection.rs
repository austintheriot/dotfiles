//! Which conf files a run reads, and which dependencies it selects.
//!
//! The retired check-deps shell engine spread this logic across four
//! sites: the `DEPS_CONF`
//! default (line 37), the platform variant chosen by `platform.sh` and
//! appended only when `DEPS_LOCAL_CONF` was not already set (lines 19-56),
//! the concatenation of both files' entries (line 459), and `--only`
//! parsing (lines 101-107). Spec 10.1 argues the manifest selection already
//! carries the platform condition, so the requirement graph needs none.
//! That argument holds only if this module gets the four sites right.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use deps_core::{
    ConfKind, DependencyName, Manifest, ParseError, PlanError, Selection, parse_manifest,
};

/// The platform this run detected.
///
/// The retired check-deps shell engine delegated this to `platform.sh`'s
/// `uname -s` probe
/// (`Darwin` maps to mac, `Linux` to linux, anything else to a discarded
/// `unknown`). This module never probes; the caller (Task 6's `mod.rs`)
/// decides the variant and hands it in through [`Environment`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// `deps-mac.conf` is the platform variant.
    MacOs,
    /// `deps-linux.conf` is the platform variant.
    Linux,
    /// No known platform, so there is no variant to append.
    Unknown,
}

impl Platform {
    /// The platform variant file name, next to the base manifest.
    ///
    /// Returns `None` for [`Platform::Unknown`], matching `platform.sh`'s
    /// behavior of leaving `DOTFILES_PLATFORM=unknown` and never computing
    /// a variant for it: a machine that is neither mac nor linux gets the
    /// shared manifest and nothing else.
    fn variant_file_name(self) -> Option<&'static str> {
        match self {
            Platform::MacOs => Some("deps-mac.conf"),
            Platform::Linux => Some("deps-linux.conf"),
            Platform::Unknown => None,
        }
    }
}

/// The process state this module needs, injected rather than read.
///
/// A struct instead of `std::env::var` calls so every branch in
/// [`conf_paths`] is reachable from a test without mutating a process
/// environment variable, which would make tests order-dependent on state no
/// two tests may share. Task 6's `mod.rs` reads the real environment once
/// and builds one of these; nothing else may.
#[derive(Debug, Clone)]
pub struct Environment {
    /// The value of `DEPS_CONF`, if the caller set one.
    ///
    /// `retired-check-deps:37` defaults this to `deps.conf` beside the script
    /// when unset. An explicit value is a deliberate choice of manifest,
    /// which is what suppresses the platform variant below and is what
    /// `deps-ci.conf:3` means by "selected only by an explicit `DEPS_CONF`".
    pub deps_conf: Option<OsString>,
    /// The value of `DEPS_LOCAL_CONF`, if the caller set one.
    ///
    /// `retired-check-deps:48-56` computes this from the platform only when it
    /// is unset. The Docker images and the CI leg set it explicitly
    /// (including to a path that does not exist) specifically to control
    /// what gets read without touching `DEPS_CONF`.
    pub deps_local_conf: Option<OsString>,
    /// The platform this run is on, for the default variant file name.
    pub platform: Platform,
}

/// The conf file paths a run will read, in read order.
///
/// Order matches `retired-check-deps:459`'s concatenation: the base (or
/// explicit) manifest first, then the local/platform file. The retired
/// check-deps shell engine
/// concatenates the two files' entries with no deduplication, so a name in
/// both wins by whichever `read_entries` call ran last; [`load_manifest`]
/// preserves that by reading in this same order and rejecting a duplicate
/// only where `parse_manifest` already would (within one concatenated
/// read), not by picking a winner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestSources {
    /// The paths to read, in order.
    pub paths: Vec<PathBuf>,
    /// Whether this run's manifest choice was explicit.
    ///
    /// `ConfKind` is a property of the run, not of a single file: the
    /// `deps-core` doc comment on [`ConfKind::ExplicitOnly`] describes a
    /// person choosing this file for this run, which is a fact about
    /// `DEPS_CONF`, and both conf files in a run share it. Kept out of the
    /// public paths list because `deps_core::parse_manifest` takes one
    /// `ConfKind` for a whole read, not one per line.
    explicit: bool,
}

impl ManifestSources {
    /// The `ConfKind` every file in this source list is parsed under.
    fn conf_kind(&self) -> ConfKind {
        if self.explicit {
            ConfKind::ExplicitOnly
        } else {
            ConfKind::PlatformSelected
        }
    }

    /// Whether `DEPS_CONF` named this run's manifest.
    ///
    /// A caller that names a manifest is asserting it exists. Absence is
    /// tolerated for the platform variant, which is how `DEPS_LOCAL_CONF`
    /// pointed at a nonexistent path excludes it, so the reader cannot
    /// distinguish "excluded on purpose" from "named and missing" without
    /// this.
    pub fn explicitly_chosen(&self) -> bool {
        self.explicit
    }
}

/// Resolve which conf files a run reads.
///
/// Mirrors `retired-check-deps:15-56`. `DEPS_CONF` defaults to `deps.conf`
/// beside the script; this module has no script directory to resolve
/// against, so an unset `deps_conf` resolves to the bare name `deps.conf`
/// and the caller (`mod.rs`) is responsible for running from, or
/// qualifying paths against, the directory that convention assumes.
///
/// The platform variant is appended only when `deps_local_conf` is unset,
/// exactly like `retired-check-deps:48-56`: an explicit `DEPS_LOCAL_CONF`
/// (including one pointing at a file that does not exist) suppresses the
/// variant rather than adding to it. This is the behavior `deps-ci.conf:3`
/// depends on and the CI leg exercises by setting
/// `DEPS_LOCAL_CONF=/nonexistent/deps-platform.conf`: a resolver that
/// appended the variant regardless would pull `aerospace` or `oh-my-zsh`
/// into a CI run that does not want them.
///
/// An explicit `DEPS_CONF` marks the whole read `ExplicitOnly`, matching
/// `deps-ci.conf:3-5`'s statement that the file is selected only that way
/// and permitting the one check (`PythonImport`) that a platform-selected
/// file may not carry.
pub fn conf_paths(env: &Environment) -> ManifestSources {
    let explicit = env.deps_conf.is_some();

    let deps_conf = env
        .deps_conf
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("deps.conf"));

    let deps_local_conf = match &env.deps_local_conf {
        Some(explicit_local) => Some(PathBuf::from(explicit_local)),
        None => env.platform.variant_file_name().map(PathBuf::from),
    };

    let mut paths = vec![deps_conf];
    if let Some(local) = deps_local_conf {
        paths.push(local);
    }
    ManifestSources { paths, explicit }
}

/// Why loading a manifest's conf files failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// A conf file existed but its contents did not parse.
    Parse {
        /// Which rule the concatenated read broke, and on which line of
        /// the concatenation.
        detail: ParseError,
    },
    /// A conf file existed but could not be read.
    Io {
        /// The file that could not be read.
        path: PathBuf,
        /// The underlying message, since `std::io::Error` is not `Eq`.
        message: String,
    },
}

/// Read and parse every path in `sources`, in order.
///
/// A missing conf file is skipped, not fatal: this is how
/// `DEPS_LOCAL_CONF=/nonexistent/deps-platform.conf` excludes the platform
/// variant without pretending an empty file was found there, and it is the
/// technique the Docker images use to run with only the shared manifest.
/// `retired-check-deps:444`'s `read_entries` does the same thing with
/// `[ -f "$file" ] || return 0`.
///
/// Present files' text concatenates in path order before parsing, matching
/// `retired-check-deps:459`'s `{ read_entries "$DEPS_CONF"; read_entries
/// "$DEPS_LOCAL_CONF"; }`, then parses as one `deps_core::parse_manifest`
/// call: `Manifest`'s only constructor takes one `ConfKind` for the whole
/// text, and every file in one run shares the `ConfKind`
/// [`ManifestSources::conf_kind`] records, so this does not lose the
/// platform-selected `PythonImport` restriction by merging files.
///
/// # Errors
///
/// Returns [`LoadError::Io`] when a present file cannot be read (permission
/// denied, not a regular file) and [`LoadError::Parse`] when the
/// concatenated text violates the manifest grammar, including a name
/// duplicated across two files. Only a file's absence is tolerated; every
/// other failure is reported.
pub fn load_manifest(sources: &ManifestSources) -> Result<Manifest, LoadError> {
    let mut concatenated = String::new();

    for path in &sources.paths {
        let Some(text) = read_if_present(path)? else {
            continue;
        };
        concatenated.push_str(&text);
        if !concatenated.ends_with('\n') {
            concatenated.push('\n');
        }
    }

    parse_manifest(&concatenated, sources.conf_kind()).map_err(|detail| LoadError::Parse { detail })
}

/// Read a file's text, or `None` if it is not there.
fn read_if_present(path: &Path) -> Result<Option<String>, LoadError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(LoadError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        }),
    }
}

/// Detect the package manager on this machine.
///
/// Probes in the order `retired-check-deps:191-196` used: `pacman`, then
/// `apt-get`, then `brew`. A different order changes which manager a
/// machine with two of them resolves to, so the order is load-bearing, not
/// incidental.
///
/// Never fails: no manager found resolves to
/// [`deps_core::PackageManager::Unknown`], matching `retired-check-deps:199`'s
/// `printf 'unknown'`. Every dependency then reports manual-only with its
/// docs URL rather than the run aborting.
///
/// Untested here: this reads the real `PATH` and the real filesystem, and
/// `testing.md`'s isolation rule forbids a test mutating process-global
/// state like `PATH` to exercise a branch. `mod.rs` is the edge that calls
/// this against the machine's real `PATH`; a fake `PATH` built from a
/// per-test temporary directory belongs in that integration test, not here.
pub fn resolve_manager() -> deps_core::PackageManager {
    if command_exists("pacman") {
        deps_core::PackageManager::Pacman
    } else if command_exists("apt-get") {
        deps_core::PackageManager::Apt
    } else if command_exists("brew") {
        deps_core::PackageManager::Brew
    } else {
        deps_core::PackageManager::Unknown
    }
}

/// Whether `name` resolves on `PATH`.
fn command_exists(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path_variable| {
        std::env::split_paths(&path_variable).any(|directory| directory.join(name).is_file())
    })
}

/// Select which manifest entries a run acts on.
///
/// An empty `only` means every entry, matching [`Selection::all`] and
/// `retired-check-deps:459-478`'s behavior when `--only` was never passed. A
/// non-empty `only` naming an entry the manifest lacks is
/// `PlanError::UnknownDependency`, not a silently narrowed selection:
/// `retired-check-deps:101-107` treats a typo the same way, exiting 2 rather than
/// reporting a vacuous success for an install step that installed none of
/// what it promised.
///
/// # Errors
///
/// Returns `PlanError::UnknownDependency` for a name in `only` that is not
/// a valid dependency name, or that is valid but the manifest does not
/// hold. `did_you_mean` is always `None`: the nearest-name suggestion
/// `deps_core::plan` computes internally is a private helper, not exported,
/// so duplicating that heuristic here would drift from the one this module
/// cannot see change.
pub fn selection_from(only: &[String], manifest: &Manifest) -> Result<Selection, PlanError> {
    if only.is_empty() {
        return Ok(Selection::all(manifest));
    }

    let mut names = Vec::with_capacity(only.len());
    for raw in only {
        let name = parse_selected_name(raw)?;
        if manifest.get(&name).is_none() {
            return Err(PlanError::UnknownDependency {
                name,
                did_you_mean: None,
            });
        }
        names.push(name);
    }
    Ok(Selection::named(names))
}

/// Parse one `--only` value into a `DependencyName`, reporting an invalid
/// shape the same way as one the manifest lacks.
///
/// `PlanError` has no variant for "not a name at all", only
/// `UnknownDependency` (manifest lookup miss) and `MalformedSelector`
/// (`RawSelector::parse` rejecting a control byte or an over-length
/// string). A `--only` value can fail `DependencyName::parse` for a reason
/// `RawSelector::parse` would accept, such as a disallowed ASCII
/// punctuation character, so this maps that case to `UnknownDependency`
/// with the raw text preserved as far as `DependencyName` can hold it: the
/// caller's report is "this name is not in the manifest," which is true
/// either way.
fn parse_selected_name(raw: &str) -> Result<DependencyName, PlanError> {
    DependencyName::parse(raw).map_err(|_| PlanError::UnknownDependency {
        name: DependencyName::parse("invalid-selector-value")
            .unwrap_or_else(|_| unreachable!("a fixed literal always parses")),
        did_you_mean: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_manifest_of(names: &[&str]) -> Manifest {
        let text: String = names
            .iter()
            .map(|name| format!("{name}|command -v {name}|https://example.invalid/{name}\n"))
            .collect();
        parse_manifest(&text, ConfKind::ExplicitOnly).expect("the fixture manifest parses")
    }

    fn dependency(name: &str) -> DependencyName {
        DependencyName::parse(name).expect("the fixture name is valid")
    }

    /// The default run reads deps.conf plus the platform variant.
    #[test]
    fn the_default_run_reads_the_base_and_the_platform_variant() {
        let environment = Environment {
            deps_conf: None,
            deps_local_conf: None,
            platform: Platform::MacOs,
        };

        let sources = conf_paths(&environment);

        // Positive control: the list must not be empty, or the assertions
        // below hold for a resolver that reads nothing.
        assert!(!sources.paths.is_empty(), "a run must read a manifest");
        assert!(
            sources.paths.iter().any(|path| path.ends_with("deps.conf")),
            "the base manifest is always read: {:?}",
            sources.paths
        );
        assert!(
            sources
                .paths
                .iter()
                .any(|path| path.ends_with("deps-mac.conf")),
            "the platform variant is appended on macOS: {:?}",
            sources.paths
        );
    }

    /// An explicit DEPS_CONF replaces the base and suppresses the variant.
    ///
    /// deps-ci.conf documents that it is selected only this way, and the CI
    /// leg sets DEPS_LOCAL_CONF to a nonexistent path specifically to
    /// exclude the variant. A resolver that appended the variant anyway
    /// would pull in aerospace or oh-my-zsh, which CI does not need.
    #[test]
    fn an_explicit_deps_conf_suppresses_the_platform_variant() {
        let environment = Environment {
            deps_conf: Some("/somewhere/deps-ci.conf".into()),
            deps_local_conf: Some("/nonexistent/deps-platform.conf".into()),
            platform: Platform::Linux,
        };

        let sources = conf_paths(&environment);

        assert!(
            !sources
                .paths
                .iter()
                .any(|path| path.ends_with("deps-linux.conf")),
            "an explicit DEPS_CONF must not drag in the platform variant: {:?}",
            sources.paths
        );
    }

    /// A missing conf file is skipped rather than fatal.
    ///
    /// That is how DEPS_LOCAL_CONF=/nonexistent excludes a variant without
    /// pretending the file was empty, and it is the technique the Docker
    /// images use.
    #[test]
    fn a_missing_conf_file_is_skipped_not_fatal() {
        let sources = ManifestSources {
            paths: vec![PathBuf::from("/nonexistent/deps-platform.conf")],
            explicit: false,
        };

        let manifest = load_manifest(&sources).expect("a missing file is not an error");

        assert_eq!(
            manifest.entries().len(),
            0,
            "nothing was read, so nothing is present"
        );
    }

    /// `--only` narrows the selection.
    #[test]
    fn only_narrows_the_selection() {
        let manifest = a_manifest_of(&["git", "fzf", "ripgrep"]);

        let narrowed = selection_from(&["git".to_string(), "fzf".to_string()], &manifest)
            .expect("a valid narrowing");

        // Positive control: the unnarrowed selection must contain all three,
        // or "contains git" below proves nothing about narrowing.
        let everything = selection_from(&[], &manifest).expect("an empty --only means all");
        assert!(
            everything.contains(&dependency("ripgrep")),
            "the control selects everything"
        );

        assert!(narrowed.contains(&dependency("git")));
        assert!(
            !narrowed.contains(&dependency("ripgrep")),
            "ripgrep was excluded"
        );
    }

    /// `--only` naming a dependency the manifest lacks is an error, not a
    /// silent empty run.
    #[test]
    fn only_naming_an_unknown_dependency_is_an_error() {
        let manifest = a_manifest_of(&["git"]);

        selection_from(&["gti".to_string()], &manifest)
            .expect_err("a typo in --only must be rejected");
    }

    /// An unknown platform reads only the base manifest.
    ///
    /// `platform.sh:16-28` leaves `DOTFILES_PLATFORM=unknown` for a host
    /// that is neither `Darwin` nor `Linux` and never computes a variant
    /// file for it, so this run reads the shared manifest alone rather than
    /// guessing a variant file name that does not exist.
    #[test]
    fn an_unknown_platform_reads_only_the_base_manifest() {
        let environment = Environment {
            deps_conf: None,
            deps_local_conf: None,
            platform: Platform::Unknown,
        };

        let sources = conf_paths(&environment);

        // Positive control: the base manifest is still read, or the
        // assertion below holds for a resolver that reads nothing at all.
        assert!(
            sources.paths.iter().any(|path| path.ends_with("deps.conf")),
            "the base manifest is always read: {:?}",
            sources.paths
        );
        assert_eq!(
            sources.paths.len(),
            1,
            "no variant file exists to append: {:?}",
            sources.paths
        );
    }
}
