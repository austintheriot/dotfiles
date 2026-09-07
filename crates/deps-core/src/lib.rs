#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
//! The dependency-check core. No IO of any kind.
//!
//! Every function here is a function of its arguments. Parsing takes text,
//! planning takes an observation map, and the fixpoint loop takes its
//! effects as arguments: `gather` is a closure and the installers are a
//! parameter, so the loop's shape lives here while every capability stays
//! at the edge. The crate boundary is what makes that compiler-enforced
//! rather than a discipline (spec 7.1), and `config_manifest::doctor` is
//! the in-repo precedent for the module shape.
//!
//! # Reading the `retired-check-deps:NNN` citations
//!
//! Comments throughout this crate and `config-cli` cite line numbers in the
//! shell engine this code replaced. That engine lived under `.scripts/deps/`
//! and was deleted when the port completed, so its line numbers resolve
//! through git history rather than the working tree. The citations are kept
//! because they carry the reasoning: each one names the behaviour a decision
//! here preserves, or the defect it deliberately does not.

mod action;
mod check;
mod driver;
mod manifest;
mod outcome;
mod plan;
mod reconcile;
mod render;

pub use check::{
    Check, CheckParseError, CheckPath, Observation, ObservationMap, Observations, PathRoot,
    evaluate, parse_check_expression,
};
pub use action::{
    BrewKind, CloneSource, InstallAction, KeyringSource, NoInstallReason, PackageAvailability,
    PackageManager, PackageMap, ScriptInstaller, SourceListEntry, TapName,
};
pub use manifest::{
    ConfKind, DependencyName, Manifest, ManifestEntry, ParseError, parse_manifest,
};
pub use outcome::{
    CheckStatus, ExecFailure, ExitStatus, InstallStatus, SpawnError, StepOutcome, Verdict,
    exit_status, summarize_check, summarize_install,
};
pub use driver::{
    ActionDescription, Attempted, Installer, Installers, Planning, describe, perform_all,
    run_to_fixpoint,
};
pub use plan::{
    Elevation, Event, PackageCatalog, Plan, PlanError, PrivilegeRequirement, RawSelector,
    RequirementEdgeError, Requirements, Selection, Step, plan,
};
pub use reconcile::{Report, ReportRow, reconcile};
pub use render::{Rendered, Verb, render};

#[cfg(test)]
mod purity {
    /// The crate source must name no IO capability.
    ///
    /// `include_str!` reads at compile time, so this test spawns nothing and
    /// opens nothing at runtime. It is the mechanism spec 7.1 says the crate
    /// boundary buys, made checkable inside the crate as well: a `deps-core`
    /// that grows a filesystem call fails here before it fails a review.
    ///
    /// Each later task adds its own module to `sources` in the same commit
    /// that adds the module. A module absent from this array is unchecked.
    #[test]
    fn no_module_names_an_io_capability() {
        let sources = [
            ("action.rs", include_str!("action.rs")),
            ("lib.rs", include_str!("lib.rs")),
            ("check.rs", include_str!("check.rs")),
            ("driver.rs", include_str!("driver.rs")),
            ("manifest.rs", include_str!("manifest.rs")),
            ("outcome.rs", include_str!("outcome.rs")),
            ("plan.rs", include_str!("plan.rs")),
            ("reconcile.rs", include_str!("reconcile.rs")),
            ("render.rs", include_str!("render.rs")),
        ];
        for (file_name, source) in sources {
            for forbidden in forbidden_capabilities() {
                assert!(
                    !source.contains(&forbidden),
                    "{file_name} names {forbidden}; deps-core performs no IO"
                );
            }
        }
    }

    /// Positive control for the assertion above.
    ///
    /// The purity test asserts an absence, so it passes vacuously if
    /// `include_str!` ever yields empty text or a needle stops matching.
    /// This plants a line that names an IO capability and proves the same
    /// `contains` check finds it.
    ///
    #[test]
    fn the_purity_check_detects_a_forbidden_string() {
        for forbidden in forbidden_capabilities() {
            let planted = format!("fn reach_out() {{ {forbidden}::whatever() }}");
            assert!(
                planted.contains(&forbidden),
                "a needle the purity test relies on does not match a source that names it"
            );
        }
        for source in [
            include_str!("action.rs"),
            include_str!("lib.rs"),
            include_str!("check.rs"),
            include_str!("driver.rs"),
            include_str!("manifest.rs"),
            include_str!("outcome.rs"),
            include_str!("plan.rs"),
            include_str!("reconcile.rs"),
            include_str!("render.rs"),
        ] {
            assert!(
                !source.is_empty(),
                "include_str! yielded empty text, so the purity test proves nothing"
            );
        }
    }

    /// The capability paths no module may name.
    ///
    /// Assembled from segments rather than written as literals, because this
    /// file is one of the files the check scans and a literal here would make
    /// the crate fail its own purity test. The first version of this module
    /// did exactly that, which is the evidence the check is not vacuous.
    fn forbidden_capabilities() -> Vec<String> {
        ["fs", "process", "env", "io"]
            .into_iter()
            .map(|capability| format!("std::{capability}"))
            .collect()
    }
}
