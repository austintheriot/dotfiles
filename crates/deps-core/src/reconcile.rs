//! Classify each outcome against the world observed after the loop.
//!
//! Pure. The post-loop world arrives as an injected [`Observations`], so
//! this module opens nothing and spawns nothing.

use crate::check::{Observation, Observations, evaluate};
use crate::manifest::{DependencyName, Manifest};
use crate::outcome::{
    CheckStatus, InstallStatus, StepOutcome, summarize_check, summarize_install,
};
use crate::plan::Event;

/// One dependency's final state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportRow {
    /// The dependency this row describes.
    pub dependency: DependencyName,
    /// What the run concluded about it.
    pub outcome: StepOutcome,
    /// What its check observed after the loop finished.
    pub after: Observation,
}

/// What the run concluded, per dependency and in aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// One row per manifest entry, in the manifest's order.
    pub rows: Vec<ReportRow>,
    /// Whether the environment is ready.
    pub check: CheckStatus,
    /// Whether every actionable install succeeded.
    pub install: InstallStatus,
}

/// Classify each outcome against the world observed after the loop.
///
/// Pure: `observations` is the post-loop world, injected. This is where an
/// `Installed` whose check still fails becomes
/// [`StepOutcome::InstalledButCheckStillFails`], carrying the `Check` so the
/// report can name the predicate.
///
/// A dependency the loop never attempted still gets a row, classified from
/// its observation, so the report covers the manifest rather than only the
/// steps.
pub fn reconcile(
    manifest: &Manifest,
    outcomes: &[(DependencyName, StepOutcome)],
    observations: &impl Observations,
) -> (Report, Vec<Event>) {
    let mut rows = Vec::with_capacity(manifest.entries().len());
    let mut events = Vec::new();

    for entry in manifest.entries() {
        let after = evaluate(&entry.check, observations);
        if let Observation::Unresolvable { root } = after {
            events.push(Event::CheckUnanswerable {
                dependency: entry.name.clone(),
                root,
            });
        }

        // The LAST recorded outcome wins, and that is load-bearing rather
        // than defensive. A dependency blocked in wave 1 and installed in
        // wave 2 appears twice in the accumulated outcomes, so taking the
        // first would report every deferred dependency as blocked no matter
        // what a later wave did, which would make the fixpoint invisible in
        // the report.
        let recorded = outcomes
            .iter()
            .rev()
            .find(|(name, _)| name == &entry.name)
            .map(|(_, outcome)| outcome.clone());

        let outcome = match (recorded, after) {
            (Some(StepOutcome::Installed), Observation::Present) => StepOutcome::Installed,
            // The install ran and the predicate is still not satisfied.
            // Carrying the Check is what lets the report say which one.
            (Some(StepOutcome::Installed), _) => StepOutcome::InstalledButCheckStillFails {
                check: entry.check.clone(),
            },
            (Some(other), _) => other,
            (None, Observation::Present) => StepOutcome::AlreadyPresent,
            // No outcome and not present: this run never considered it,
            // so there is no prerequisite to name. This used to report
            // Blocked { on: entry.name }, which told the reader a dependency
            // waits on itself.
            (None, _) => StepOutcome::NotSelected,
        };

        rows.push(ReportRow {
            dependency: entry.name.clone(),
            outcome,
            after,
        });
    }

    let classified: Vec<StepOutcome> = rows.iter().map(|row| row.outcome.clone()).collect();
    let report = Report {
        check: summarize_check(&classified),
        install: summarize_install(&classified),
        rows,
    };
    (report, events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Check, CheckPath, CheckStatus, ConfKind, InstallStatus, PathRoot, parse_manifest,
    };
    use dotfiles_path::CheckRelPath;

    fn dependency(name: &str) -> DependencyName {
        DependencyName::parse(name).expect("a test dependency name parses")
    }

    // The real Linux pair. `deps-linux.conf:11` holds oh-my-zsh; the
    // zsh-autosuggestions check is `deps.conf:26` with its brew branch
    // dropped, because the brew branch is what lets the pair converge in
    // one pass on macOS and this is the Linux case.
    fn oh_my_zsh_manifest() -> Manifest {
        let text = "\
oh-my-zsh|[ -d \"$HOME/.oh-my-zsh\" ]|https://ohmyz.sh/
zsh-autosuggestions|[ -f \"$HOME/.oh-my-zsh/custom/plugins/zsh-autosuggestions/zsh-autosuggestions.zsh\" ]|https://github.com/zsh-users/zsh-autosuggestions
";
        parse_manifest(text, ConfKind::PlatformSelected).expect("the real pair parses")
    }

    fn oh_my_zsh_check() -> Check {
        Check::DirExists(CheckPath::new(
            PathRoot::Home,
            CheckRelPath::parse(".oh-my-zsh").expect("a test path parses"),
        ))
    }

    // The classification the post-loop world buys. An install that ran and
    // whose check still fails is caught here, and it carries the Check so
    // the report names the predicate rather than printing
    // check-deps.sh:589's "install did not satisfy the check for %s".
    #[test]
    fn an_install_whose_check_still_fails_is_reclassified() {
        let manifest = oh_my_zsh_manifest();
        let outcomes = vec![
            (dependency("oh-my-zsh"), StepOutcome::Installed),
            (dependency("zsh-autosuggestions"), StepOutcome::Installed),
        ];
        let after = crate::ObservationMap::from_pairs(vec![(
            oh_my_zsh_check(),
            Observation::Present,
        )]);
        let (report, events) = reconcile(&manifest, &outcomes, &after);

        let row = report
            .rows
            .iter()
            .find(|row| row.dependency == dependency("zsh-autosuggestions"))
            .expect("the second entry has a row");
        assert!(matches!(
            row.outcome,
            StepOutcome::InstalledButCheckStillFails { .. }
        ));
        assert_eq!(report.install, InstallStatus::AttemptFailed);
        assert_eq!(report.check, CheckStatus::NotReady);
        assert!(
            events.is_empty(),
            "no root failed to resolve, so no event: got {events:?}"
        );
    }

    // Positive control for the empty-events assertion above. An
    // unresolvable root does produce an event, so the emptiness there is a
    // fact about this world rather than a function that emits nothing.
    #[test]
    fn an_unresolvable_root_produces_an_event() {
        let text = "zsh-autosuggestions|[ -f \"$(brew --prefix 2>/dev/null)/share/zsh-autosuggestions/zsh-autosuggestions.zsh\" ]|https://github.com/zsh-users/zsh-autosuggestions\n";
        let manifest =
            parse_manifest(text, ConfKind::PlatformSelected).expect("the brew entry parses");
        let brew_check = Check::FileExists(CheckPath::new(
            PathRoot::BrewPrefix,
            CheckRelPath::parse("share/zsh-autosuggestions/zsh-autosuggestions.zsh")
                .expect("a test path parses"),
        ));
        let after = crate::ObservationMap::from_pairs(vec![(
            brew_check,
            Observation::Unresolvable { root: PathRoot::BrewPrefix },
        )]);
        let (_report, events) = reconcile(&manifest, &[], &after);
        assert_eq!(
            events,
            vec![Event::CheckUnanswerable {
                dependency: dependency("zsh-autosuggestions"),
                root: PathRoot::BrewPrefix,
            }]
        );
    }

    // A dependency the loop never attempted still gets a row, so the report
    // covers the manifest rather than only the steps.
    //
    // This test previously asserted `Blocked { on: zsh-autosuggestions }` for
    // the zsh-autosuggestions row: a dependency waiting on itself. That is
    // not a state that exists, and asserting it meant a green test pinned the
    // defect in place. Reproduced through reconcile before the fix with a
    // two-entry manifest and one observation: `fzf -> Blocked { on: fzf }`.
    #[test]
    fn an_unattempted_dependency_still_gets_a_row() {
        let manifest = oh_my_zsh_manifest();
        let after = crate::ObservationMap::from_pairs(vec![(
            oh_my_zsh_check(),
            Observation::Present,
        )]);
        let (report, _events) = reconcile(&manifest, &[], &after);
        assert_eq!(report.rows.len(), 2, "one row per manifest entry");
        assert_eq!(report.rows[0].outcome, StepOutcome::AlreadyPresent);
        assert_eq!(report.rows[1].outcome, StepOutcome::NotSelected);
    }

    /// A dependency nothing considered must not report the machine ready.
    ///
    /// `NotSelected` exists because reconcile has no prerequisite to name for
    /// an entry the selection excluded. It still has to count as not ready:
    /// the entry is in the manifest and absent from the machine, so a run
    /// that reports success would be claiming an environment is complete
    /// while a tracked dependency is missing.
    #[test]
    fn a_not_selected_dependency_is_not_ready() {
        // Positive control: the same summary must call a present dependency
        // ready, or the assertion below would hold for the wrong reason.
        assert_eq!(
            crate::summarize_check(&[StepOutcome::AlreadyPresent]),
            crate::CheckStatus::Ready
        );

        assert_eq!(
            crate::summarize_check(&[StepOutcome::NotSelected]),
            crate::CheckStatus::NotReady
        );

        // And it is not an attempt failure: nothing was attempted.
        assert_eq!(
            crate::summarize_install(&[StepOutcome::NotSelected]),
            crate::InstallStatus::AllSucceeded
        );
    }
}
