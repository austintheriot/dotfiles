//! Gather observations at the instant `run_to_fixpoint` asks for them.
//!
//! `deps_core::Observations` (`check.rs`) states four rules a gather must
//! satisfy. This module is the only place in the binary that probes the
//! filesystem or a resolved root for a manifest's checks, and it exists to
//! satisfy those four rules rather than to restate them:
//!
//! 1. Every `Check::AnyOf` leaf is recorded, not just the composite, because
//!    `ObservationMap::observe` answers `Absent` for a key it never saw.
//! 2. A root or interpreter that fails to resolve records
//!    `Observation::Unresolvable`, never `Absent`.
//! 3. `Unresolvable` is recorded and handed to the core; nothing here
//!    decides whether it blocks (`deps_core::plan` decides that).
//! 4. Root resolution happens once per leaf, at the moment that leaf is
//!    probed, through the injected [`RootResolver`]. Nothing here caches a
//!    resolved root across leaves or across calls, so a caller that invokes
//!    [`gather`] once per fixpoint wave sees a fresh resolution each wave.

use std::path::{Path, PathBuf};

use deps_core::{Check, CheckPath, Manifest, Observation, ObservationMap, PathRoot};

/// Resolves a [`PathRoot`] to a directory, or reports why it could not.
///
/// Injected rather than called directly, because the real resolution of
/// [`PathRoot::BrewPrefix`] runs `brew --prefix`, and a gather that called
/// that itself could never be driven into the failure path on a machine
/// that has brew installed. The real edge (wired in a later commit) queries
/// the environment; tests substitute a resolver that fails on demand.
pub trait RootResolver {
    /// The directory `root` names, or `None` if it could not be resolved.
    fn resolve(&self, root: PathRoot) -> Option<PathBuf>;
}

/// Gather observations for every check a manifest names.
///
/// Called once per fixpoint wave by `deps_core::run_to_fixpoint`, which is
/// what rule 4 relies on: `resolver` is asked again on every call, so a
/// directory an earlier wave's install created (`oh-my-zsh`'s clone target,
/// for instance) is visible to the check that reads it in a later wave.
/// Nothing in this function persists a resolved root between calls or
/// between leaves within one call.
pub fn gather(manifest: &Manifest, resolver: &impl RootResolver) -> ObservationMap {
    let pairs = manifest
        .entries()
        .iter()
        .flat_map(|entry| observe_check(&entry.check, resolver))
        .collect();
    ObservationMap::from_pairs(pairs)
}

/// Observe one check and, if it is an alternation, every branch beneath it.
///
/// Recursion is what satisfies rule 1: `Check::AnyOf` carries `first` and a
/// `rest` of further checks, any of which can itself be an `AnyOf`, and a
/// gather that stops at the top level leaves every leaf unrecorded. Each
/// call returns the pair for the check it was given, plus every pair its
/// branches returned, so the composite and every leaf reach the map.
fn observe_check(check: &Check, resolver: &impl RootResolver) -> Vec<(Check, Observation)> {
    let mut pairs = vec![(check.clone(), observe_one(check, resolver))];
    if let Check::AnyOf { first, rest } = check {
        pairs.extend(observe_check(first, resolver));
        for branch in rest {
            pairs.extend(observe_check(branch, resolver));
        }
    }
    pairs
}

/// Observe a single check as its own subject, ignoring any `AnyOf` branches.
///
/// `Check::AnyOf` is itself observed here too (its pair records how the
/// composite reads under `deps_core::evaluate`), which is why this and
/// [`observe_check`] are separate: the composite's own observation and its
/// branches' observations are independent entries in the map, and
/// `evaluate` is what derives one from the other when the core reads them
/// back.
fn observe_one(check: &Check, resolver: &impl RootResolver) -> Observation {
    match check {
        Check::Command(name) => probe_command(name.as_str()),
        Check::CommandVersion { name, floor } => probe_command_version(name.as_str(), *floor),
        Check::DirExists(path) => probe_path(path, resolver, |candidate| candidate.is_dir()),
        Check::FileExists(path) => probe_path(path, resolver, |candidate| candidate.is_file()),
        Check::FileNonEmpty(path) => probe_path(path, resolver, |candidate| {
            std::fs::metadata(candidate).is_ok_and(|metadata| metadata.len() > 0)
        }),
        Check::GlobExists { dir, pattern } => probe_glob(dir, pattern.as_str(), resolver),
        Check::PythonImport(module) => probe_python_import(module.as_str()),
        // The composite has no subject of its own beyond its branches:
        // `deps_core::evaluate` derives its answer from them, so recording
        // `Absent` here is inert as long as every branch is also recorded
        // (rule 1), and `observe_check` guarantees that.
        Check::AnyOf { .. } => Observation::Absent,
    }
}

/// Resolve `path`'s root and test the joined path, or report why not.
///
/// A root that fails to resolve is `Observation::Unresolvable`, never
/// `Observation::Absent`: rule 2 treats "brew is not installed" as a
/// different fact from "the file brew would have provided is missing",
/// because the two have different remedies (install brew; install the
/// package).
fn probe_path(
    path: &CheckPath,
    resolver: &impl RootResolver,
    test: impl Fn(&Path) -> bool,
) -> Observation {
    match resolver.resolve(path.root) {
        Some(root) => to_observation(test(&root.join(path.rest.as_str()))),
        None => Observation::Unresolvable { root: path.root },
    }
}

/// Resolve `dir`'s root and test whether any entry matches `pattern`.
///
/// Matching is a plain prefix/suffix split on the one `*` the manifest's
/// glob patterns carry (`dotfiles_path::GlobPattern` already restricts the
/// shape), which is enough for `ls -d "$dir"/v*`'s style of check without
/// pulling in a glob crate for one wildcard position.
fn probe_glob(dir: &CheckPath, pattern: &str, resolver: &impl RootResolver) -> Observation {
    let Some(root) = resolver.resolve(dir.root) else {
        return Observation::Unresolvable { root: dir.root };
    };
    let joined = root.join(dir.rest.as_str());
    let Ok(read_dir) = std::fs::read_dir(&joined) else {
        return Observation::Absent;
    };
    let matches = read_dir
        .filter_map(Result::ok)
        .any(|dir_entry| glob_matches(pattern, &dir_entry.file_name().to_string_lossy()));
    to_observation(matches)
}

/// Whether `name` matches a single-`*`-wildcard `pattern`.
fn glob_matches(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        Some((prefix, suffix)) => name.starts_with(prefix) && name.ends_with(suffix),
        None => pattern == name,
    }
}

/// The directories a `command -v` check searches, widest first.
///
/// `PATH`, preceded by `~/.local/bin` and `~/.cargo/bin`.
///
/// Those two are prepended because two dependencies install into them and a
/// default non-login `PATH` carries neither: rustup writes `~/.cargo/bin`,
/// and zoxide's own installer writes `~/.local/bin` where no package for it
/// exists. Without them the check fails on the line right after its own
/// install succeeded, so the fixpoint reports a failure for an install that
/// worked.
///
/// Found by the container gate rather than by any unit test. On a bare image
/// rustup installed cleanly into `~/.cargo/bin` and was then reported
/// `failed`. An interactive shell exports both directories already, which is
/// what hides this on a machine already in use -- and is why the retired
/// shell carried the same prepend, and why the deps README documents it.
///
/// Returned rather than written back into this process's `PATH`: the crate
/// forbids `unsafe`, `set_var` needs it, and a search path that is a value
/// can be handed to the child environment as well as to this probe.
pub(crate) fn search_path() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        directories.push(home.join(".local").join("bin"));
        directories.push(home.join(".cargo").join("bin"));
    }
    if let Some(existing) = std::env::var_os("PATH") {
        directories.extend(std::env::split_paths(&existing));
    }
    directories
}

/// `command -v <name>`: present if `name` resolves on the search path.
///
/// The search path is not a `PathRoot`, so this probe has no root to fail to
/// resolve; an absent command is always `Absent`, never `Unresolvable`.
fn probe_command(name: &str) -> Observation {
    let found = search_path().iter().any(|directory| directory.join(name).is_file());
    to_observation(found)
}

/// The first `MAJOR.MINOR[.PATCH]` triple in a `--version` banner.
///
/// Separated from the spawn so the parsing is testable against real banner
/// text without running anything. The tools this is used on print a version
/// on the first line, but none of them agree on the surrounding words:
/// `NVIM v0.12.4`, `git version 2.50.0`, `ripgrep 14.1.1`,
/// `zsh 5.9 (arm64-apple-darwin25.0)`. Scanning for the first triple rather
/// than matching a per-tool layout is what keeps this one function instead
/// of a table of formats.
///
/// What this does NOT solve is a tool that rejects `--version` itself:
/// macOS tmux answers it with a usage message and wants `-V`. Such a tool
/// reads as `Unresolvable`, which is honest, and putting a floor on one
/// would mean teaching the probe a second flag first.
///
/// A leading `v` is skipped because Neovim writes one. A two-component
/// version gets a zero patch, matching `VersionFloor`'s own rule.
fn parse_version_banner(banner: &str) -> Option<(u32, u32, u32)> {
    for token in banner.split(|character: char| {
        !character.is_ascii_digit() && character != '.'
    }) {
        let mut parts = token.split('.');
        let (Some(major), Some(minor)) = (parts.next(), parts.next()) else {
            continue;
        };
        let (Ok(major), Ok(minor)) = (major.parse::<u32>(), minor.parse::<u32>()) else {
            continue;
        };
        let patch = match parts.next() {
            Some(component) => component.parse::<u32>().unwrap_or(0),
            None => 0,
        };
        if parts.next().is_some() {
            continue;
        }
        return Some((major, minor, patch));
    }
    None
}

/// `command -v <name> >=<floor>`: present, and reporting at least `floor`.
///
/// Three outcomes rather than two, and the third is the point:
///
///   - the command is missing: `Absent`, the same as a bare presence check.
///   - the command runs and reports a version below the floor: `Absent`.
///     The dependency is genuinely not satisfied, so the planner installs
///     over it exactly as it would for a missing one.
///   - the command runs and its banner cannot be parsed: `Unresolvable`.
///     Reporting `Absent` there would make the engine reinstall a tool that
///     may well be current on every run, and reporting `Present` would
///     restore the presence-only check this variant replaced. Neither is
///     honest about "the tool is here and I could not read its version".
fn probe_command_version(name: &str, floor: dotfiles_path::VersionFloor) -> Observation {
    let Some(binary) = search_path().iter().map(|directory| directory.join(name)).find(|candidate| candidate.is_file())
    else {
        return Observation::Absent;
    };
    let output = std::process::Command::new(&binary)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .output();
    let Ok(output) = output else {
        return Observation::Unresolvable { root: PathRoot::Home };
    };
    // Some tools write the banner to stderr, so both streams are read.
    let mut banner = String::from_utf8_lossy(&output.stdout).into_owned();
    banner.push('\n');
    banner.push_str(&String::from_utf8_lossy(&output.stderr));
    match parse_version_banner(&banner) {
        Some((major, minor, patch)) => to_observation(floor.is_satisfied_by(major, minor, patch)),
        None => Observation::Unresolvable { root: PathRoot::Home },
    }
}

/// `python3 -c "import <module>"`: present if the interpreter imports it.
///
/// A missing `python3` interpreter and a missing module are the same
/// distinction rule 2 draws for a root: the shell's `sh -c "$check"`
/// (`retired-check-deps:523`) collapsed "no interpreter" and "no module" into one
/// exit code, and this probe keeps them apart by reporting `Unresolvable`
/// when the interpreter itself cannot be spawned.
fn probe_python_import(module: &str) -> Observation {
    let output = std::process::Command::new("python3")
        .arg("-c")
        .arg(format!("import {module}"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    match output {
        Ok(status) => to_observation(status.success()),
        Err(_) => Observation::Unresolvable { root: PathRoot::Home },
    }
}

fn to_observation(present: bool) -> Observation {
    if present { Observation::Present } else { Observation::Absent }
}

#[cfg(test)]
mod tests {

    /// Real `--version` first lines, copied from the tools themselves.
    ///
    /// The formats disagree on everything except carrying a version, which
    /// is why the parser scans for a triple instead of matching a layout.
    #[test]
    fn the_banner_parser_reads_every_format_these_tools_print() {
        let cases = [
            ("NVIM v0.12.4", (0, 12, 4)),
            ("NVIM v0.9.5", (0, 9, 5)),
            ("NVIM v0.6.1", (0, 6, 1)),
            ("git version 2.39.5", (2, 39, 5)),
            // tmux's banner shape. NOT reachable through `--version` on
            // every platform: macOS tmux answers that with a usage error
            // and wants `-V`. The parser handles the text; a floor on a
            // tool like that needs a probe change nothing asks for yet.
            ("tmux 3.4", (3, 4, 0)),
            ("ripgrep 14.1.1", (14, 1, 1)),
            ("zsh 5.9 (arm-apple-darwin24.0)", (5, 9, 0)),
        ];
        for (banner, expected) in cases {
            assert_eq!(
                parse_version_banner(banner),
                Some(expected),
                "failed to read {banner:?}"
            );
        }
    }

    // The floor exists for this comparison specifically: 0.9.5 is above
    // 0.10 as a string and below it as a version, and the string answer is
    // what shipped a broken editor to a machine the bootstrap called ready.
    #[test]
    fn the_floor_rejects_the_version_pop_os_actually_ships() {
        let floor = dotfiles_path::VersionFloor::parse("0.10").expect("the floor parses");
        let (major, minor, patch) =
            parse_version_banner("NVIM v0.9.5").expect("the banner parses");
        assert!(
            !floor.is_satisfied_by(major, minor, patch),
            "Ubuntu 24.04's nvim 0.9.5 must not satisfy a 0.10 floor"
        );

        let (major, minor, patch) =
            parse_version_banner("NVIM v0.6.1").expect("the banner parses");
        assert!(
            !floor.is_satisfied_by(major, minor, patch),
            "Pop!_OS 22.04's nvim 0.6.1 must not satisfy a 0.10 floor"
        );
    }

    // A banner with no version at all must not read as zero. Returning
    // (0,0,0) would compare below every floor and make the engine reinstall
    // the tool on every run forever.
    #[test]
    fn an_unreadable_banner_is_none_rather_than_zero() {
        for banner in ["", "command not found", "no version here"] {
            assert_eq!(
                parse_version_banner(banner),
                None,
                "{banner:?} produced a version"
            );
        }
    }
    use super::*;
    use deps_core::{Manifest, Observations, parse_manifest_toml};
    use dotfiles_path::CheckRelPath;

    /// The search path leads with the two curl-installer directories.
    ///
    /// This is the assertion the container gate had to find for want of a
    /// test: rustup installs into `~/.cargo/bin` and zoxide's own installer
    /// into `~/.local/bin`, neither of which a default non-login `PATH`
    /// carries, so a check that reads `PATH` alone reports a failure for an
    /// install that succeeded.
    ///
    /// Order is asserted, not just membership. A dependency this run just
    /// installed must be found ahead of an older copy earlier on `PATH`.
    #[test]
    fn the_search_path_leads_with_the_curl_installer_directories() {
        let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
            // No HOME is a real state, and `search_path` returns PATH alone
            // there rather than joining onto nothing.
            return;
        };

        let directories = search_path();

        assert_eq!(
            directories.first(),
            Some(&home.join(".local").join("bin")),
            "~/.local/bin leads: {directories:?}"
        );
        assert_eq!(
            directories.get(1),
            Some(&home.join(".cargo").join("bin")),
            "~/.cargo/bin comes second: {directories:?}"
        );

        // The positive control: the real PATH still follows, so this widened
        // the search rather than replacing it.
        if let Some(existing) = std::env::var_os("PATH") {
            for entry in std::env::split_paths(&existing) {
                assert!(
                    directories.contains(&entry),
                    "PATH entry {entry:?} survived the widening"
                );
            }
        }
    }

    /// A resolver that resolves every real root to a real directory.
    ///
    /// Used as the positive control: it proves the harness can produce
    /// `Present` and `Absent` before the negative tests below prove it can
    /// also produce `Unresolvable`. Without a working positive control, a
    /// resolver that always fails would make every test in this file pass
    /// vacuously.
    struct AlwaysResolves {
        home: PathBuf,
    }

    impl RootResolver for AlwaysResolves {
        fn resolve(&self, root: PathRoot) -> Option<PathBuf> {
            match root {
                PathRoot::Home => Some(self.home.clone()),
                PathRoot::MacApplications => Some(self.home.clone()),
                PathRoot::BrewPrefix => Some(self.home.clone()),
                PathRoot::UsrShare => Some(self.home.clone()),
            }
        }
    }

    /// A resolver that fails only for `PathRoot::BrewPrefix`.
    ///
    /// This is the seam the design requires: without an injectable
    /// resolver, rule 2's `Unresolvable` path is untestable on a machine
    /// that has brew, and this machine has brew.
    struct BrewUnresolvable;

    impl RootResolver for BrewUnresolvable {
        fn resolve(&self, root: PathRoot) -> Option<PathBuf> {
            match root {
                PathRoot::BrewPrefix => None,
                PathRoot::Home | PathRoot::MacApplications | PathRoot::UsrShare => {
                    Some(std::env::temp_dir())
                }
            }
        }
    }

    fn a_path_in(name: &str) -> CheckPath {
        CheckPath::new(PathRoot::Home, CheckRelPath::parse(name).expect("a valid rel path"))
    }

    /// Build a one-entry manifest whose sole check is a `$HOME/<name>` file
    /// leaf, the shape `a_path_in` names.
    ///
    /// A manifest string round-tripped through the real parser, rather than
    /// a hand-built `Manifest`, because `Manifest` has no public
    /// constructor: its only way into existence is `parse_manifest_toml`,
    /// which is itself part of what this test exercises indirectly.
    fn a_manifest_with_leaf(name: &str) -> Manifest {
        let raw = format!(
            "[one_dependency]\nfile = \"$HOME/{name}\"\ndocs = \"https://example.invalid/docs\"\n"
        );
        parse_manifest_toml(&raw, deps_core::ConfKind::PlatformSelected)
            .expect("a parseable manifest")
    }

    /// Build a one-entry manifest whose sole check is an `any_of` over two
    /// file leaves, the shape `deps.toml`'s zsh-autosuggestions entry uses.
    fn a_manifest_with_any_of(first_name: &str, second_name: &str) -> Manifest {
        let raw = format!(
            "[one_dependency]\nany_of = [{{ file = \"$HOME/{first_name}\" }}, {{ file = \"$HOME/{second_name}\" }}]\ndocs = \"https://example.invalid/docs\"\n"
        );
        parse_manifest_toml(&raw, deps_core::ConfKind::PlatformSelected)
            .expect("a parseable manifest")
    }

    fn gather_one(manifest: &Manifest, home: &Path) -> ObservationMap {
        let resolver = AlwaysResolves { home: home.to_path_buf() };
        gather(manifest, &resolver)
    }

    fn gather_with_unresolvable_brew(manifest: &Manifest) -> ObservationMap {
        gather(manifest, &BrewUnresolvable)
    }

    /// Positive control: a plain present/absent pair reports correctly.
    ///
    /// Every assertion below this one relies on `gather` being able to
    /// answer `Present` and `Absent` at all. Without this control, a broken
    /// `gather` that answered `Unresolvable` for everything could still
    /// make the `AnyOf` and `Unresolvable`-specific tests below pass for
    /// the wrong reason.
    #[test]
    fn it_reports_present_and_absent_for_a_plain_leaf() {
        let root = tempfile::tempdir().expect("tempdir");
        let present = root.path().join("present.txt");
        std::fs::write(&present, "x").expect("write");

        let present_check = Check::FileExists(a_path_in("present.txt"));
        let absent_check = Check::FileExists(a_path_in("absent.txt"));

        let present_observations = gather_one(&a_manifest_with_leaf("present.txt"), root.path());
        let absent_observations = gather_one(&a_manifest_with_leaf("absent.txt"), root.path());

        assert_eq!(present_observations.observe(&present_check), Observation::Present);
        assert_eq!(absent_observations.observe(&absent_check), Observation::Absent);
    }

    /// Every `AnyOf` leaf is recorded, not just the top-level check.
    ///
    /// `ObservationMap::observe` returns `Absent` for a key it does not
    /// hold, so a gather that records only top-level checks reports every
    /// `AnyOf` dependency missing. Three shipped dependencies use `AnyOf`.
    #[test]
    fn it_records_every_any_of_leaf() {
        let root = tempfile::tempdir().expect("tempdir");
        let present = root.path().join("present.txt");
        std::fs::write(&present, "x").expect("write");

        let check = Check::AnyOf {
            first: Box::new(Check::FileExists(a_path_in("absent.txt"))),
            rest: vec![Check::FileExists(a_path_in("present.txt"))],
        };
        let manifest = a_manifest_with_any_of("absent.txt", "present.txt");

        let observations = gather_one(&manifest, root.path());

        // Positive control: the top-level AnyOf must itself be answered, or
        // the leaf assertions below could hold in a map with one entry.
        assert_eq!(evaluate_for_test(&check, &observations), Observation::Present);

        // Both leaves must be recorded individually.
        assert_eq!(
            observations.observe(&Check::FileExists(a_path_in("present.txt"))),
            Observation::Present,
            "the present leaf must be recorded, not inferred"
        );
        assert_eq!(
            observations.observe(&Check::FileExists(a_path_in("absent.txt"))),
            Observation::Absent,
            "the absent leaf must be recorded too"
        );
    }

    fn evaluate_for_test(check: &Check, observations: &ObservationMap) -> Observation {
        deps_core::evaluate(check, observations)
    }

    /// A failed probe is `Unresolvable`, not `Absent`.
    ///
    /// "The interpreter is missing" and "the module is missing" are
    /// different facts with different remedies, and the shell collapsed
    /// both (`retired-check-deps:523`). This port exists partly to stop that.
    #[test]
    fn a_failed_probe_is_unresolvable_rather_than_absent() {
        let check = Check::FileExists(CheckPath::new(
            PathRoot::BrewPrefix,
            CheckRelPath::parse("bin/definitely-not-installed").expect("a valid path"),
        ));
        let raw = "[one_dependency]\nfile = \"$(brew --prefix 2>/dev/null)/bin/definitely-not-installed\"\ndocs = \"https://example.invalid/docs\"\n";
        let manifest = parse_manifest_toml(raw, deps_core::ConfKind::PlatformSelected)
            .expect("a parseable manifest");

        let observations = gather_with_unresolvable_brew(&manifest);

        assert!(
            matches!(observations.observe(&check), Observation::Unresolvable { .. }),
            "an unresolvable root is a different fact from an absent file"
        );
    }
}
