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

// `mod.rs` wires `gather` into a `run_to_fixpoint` call in a later commit.
// `expect` rather than `allow`, so the lint fires again the moment a caller
// lands and this attribute has to be deleted rather than quietly outliving
// its reason.
#![cfg_attr(
    not(test),
    expect(dead_code, reason = "mod.rs wires gather into run_to_fixpoint in a later commit")
)]

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

/// `command -v <name>`: present if `name` resolves on `PATH`.
///
/// `PATH` itself is not a `PathRoot`, so this probe has no root to fail to
/// resolve; an absent command is always `Absent`, never `Unresolvable`.
fn probe_command(name: &str) -> Observation {
    let found = std::env::var_os("PATH").is_some_and(|path_var| {
        std::env::split_paths(&path_var).any(|directory| directory.join(name).is_file())
    });
    to_observation(found)
}

/// `python3 -c "import <module>"`: present if the interpreter imports it.
///
/// A missing `python3` interpreter and a missing module are the same
/// distinction rule 2 draws for a root: the shell's `sh -c "$check"`
/// (`check-deps.sh:523`) collapsed "no interpreter" and "no module" into one
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
    use super::*;
    use deps_core::{Manifest, Observations, parse_manifest};
    use dotfiles_path::CheckRelPath;

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
                PathRoot::Home | PathRoot::MacApplications => {
                    Some(std::env::temp_dir())
                }
            }
        }
    }

    fn a_path_in(name: &str) -> CheckPath {
        CheckPath::new(PathRoot::Home, CheckRelPath::parse(name).expect("a valid rel path"))
    }

    /// Build a one-entry manifest whose sole check is a `[ -f "$HOME/<name>" ]`
    /// leaf, the shape `parse_manifest` accepts and `a_path_in` names.
    ///
    /// A manifest string round-tripped through the real parser, rather than
    /// a hand-built `Manifest`, because `Manifest` has no public
    /// constructor: its only way into existence is `parse_manifest`, which
    /// is itself part of what this test exercises indirectly.
    fn a_manifest_with_leaf(name: &str) -> Manifest {
        let raw = format!("one_dependency|[ -f \"$HOME/{name}\" ]|https://example.invalid/docs\n");
        parse_manifest(&raw, deps_core::ConfKind::PlatformSelected).expect("a parseable manifest")
    }

    /// Build a one-entry manifest whose sole check is `test -f A -o -f B`,
    /// the `AnyOf` shape `deps.conf:26` uses.
    fn a_manifest_with_any_of(first_name: &str, second_name: &str) -> Manifest {
        let raw = format!(
            "one_dependency|test -f \"$HOME/{first_name}\" -o -f \"$HOME/{second_name}\"|https://example.invalid/docs\n"
        );
        parse_manifest(&raw, deps_core::ConfKind::PlatformSelected).expect("a parseable manifest")
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
    /// both (`check-deps.sh:523`). This port exists partly to stop that.
    #[test]
    fn a_failed_probe_is_unresolvable_rather_than_absent() {
        let check = Check::FileExists(CheckPath::new(
            PathRoot::BrewPrefix,
            CheckRelPath::parse("bin/definitely-not-installed").expect("a valid path"),
        ));
        let raw = "one_dependency|[ -f \"$(brew --prefix 2>/dev/null)/bin/definitely-not-installed\" ]|https://example.invalid/docs\n";
        let manifest =
            parse_manifest(raw, deps_core::ConfKind::PlatformSelected).expect("a parseable manifest");

        let observations = gather_with_unresolvable_brew(&manifest);

        assert!(
            matches!(observations.observe(&check), Observation::Unresolvable { .. }),
            "an unresolvable root is a different fact from an absent file"
        );
    }
}
