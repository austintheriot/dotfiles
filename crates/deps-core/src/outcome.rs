//! What happened to each planned step, and what the run concluded.
//!
//! Pure. Nothing here performs IO: the driver in the CLI crate runs the
//! steps and hands the resulting outcomes back for summarizing.

use dotfiles_path::BoundedText;

use crate::action::{InstallAction, NoInstallReason};
use crate::check::Check;
use crate::manifest::DependencyName;
use crate::plan::PlanError;

/// Why a process could not be started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnError {
    /// The command was not on PATH.
    NotFound,
    /// The command was found and could not be executed.
    PermissionDenied,
    /// Any other spawn failure.
    Other,
}

/// Why an install command failed.
///
/// Specified rather than left undefined. `stderr` is `BoundedText` because
/// an unbounded subprocess string in an error type is how a terminal gets a
/// control sequence written to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecFailure {
    /// The command ran and exited nonzero.
    NonZeroExit {
        /// The exit code the process reported.
        code: i32,
        /// Captured stderr, bounded and safe to render.
        stderr: BoundedText,
    },
    /// sudo said no, and every remaining privileged step will too. Knowable
    /// at step 1 of 8 rather than at step 8, which is why
    /// `Elevation::ViaSudo` is a prediction rather than a guarantee: a sudo
    /// binary on PATH does not prove the user is in sudoers.
    AuthenticationRefused,
    /// The process never started.
    Spawn(SpawnError),
}

/// What happened to one planned step.
///
/// `NotAutomatable` appears here and in `InstallAction`, and that is not
/// duplication: they are different propositions. As an action it is the
/// terminal element of the algebra, which is what makes `plan` total. As an
/// outcome it records that the driver performed the step and correctly did
/// nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    /// The check already passed, so nothing ran.
    AlreadyPresent,
    /// The install ran and the check now passes.
    Installed,
    /// Carries the `Check` so the report names which predicate failed.
    /// `check-deps.sh:589` prints "install did not satisfy the check for
    /// %s" and cannot say more, because the predicate was a shell string it
    /// re-ran rather than a value it holds.
    InstalledButCheckStillFails {
        /// The predicate that still does not hold.
        check: Check,
    },
    /// The install ran and failed.
    InstallFailed {
        /// The action the driver attempted.
        action: InstallAction,
        /// Why the attempt failed.
        cause: ExecFailure,
    },
    /// The driver performed the step and correctly did nothing.
    NotAutomatable {
        /// Why this dependency has no automated install here.
        reason: NoInstallReason,
    },
    /// Not in this wave. A later wave can unblock it, which is what
    /// separates this from `NoInstallReason::PrerequisiteNotYetInstalled`:
    /// the latter is what a dry run reports, because a dry run cannot know
    /// what a later wave would do.
    Blocked {
        /// The dependency this step waits on.
        on: DependencyName,
    },
}

/// The result of `deps check`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    /// Every dependency is present.
    Ready,
    /// At least one dependency is not present.
    NotReady,
}

/// The result of `deps install`.
///
/// A separate type from `CheckStatus`, not a shared status enum. The first
/// draft's single `summarize(outcomes, verb)` had a table with two
/// "(unused)" cells, and nothing in the types stopped
/// `summarize(_, Verb::Check)` from returning `AttemptFailed`. Splitting by
/// verb makes "unused" an absent variant rather than a convention upheld by
/// hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStatus {
    /// Every step that had something to attempt succeeded.
    AllSucceeded,
    /// At least one attempted install did not work.
    AttemptFailed,
}

/// Whether the environment is ready.
///
/// Pure. `NotAutomatable` counts toward not-ready, which is the direct fix
/// for today's behavior where a manual-only dependency is invisible to the
/// exit code and `config-init` can report success on a machine that is not
/// ready.
pub fn summarize_check(outcomes: &[StepOutcome]) -> CheckStatus {
    let ready = outcomes.iter().all(|outcome| {
        matches!(outcome, StepOutcome::AlreadyPresent | StepOutcome::Installed)
    });
    if ready { CheckStatus::Ready } else { CheckStatus::NotReady }
}

/// Whether every actionable install succeeded.
///
/// Pure. `NotAutomatable` is not an attempt failure: nothing was attempted,
/// so no attempt failed. The not-ready signal for that case belongs to
/// `summarize_check`, which the driver also calls.
pub fn summarize_install(outcomes: &[StepOutcome]) -> InstallStatus {
    let attempt_failed = outcomes.iter().any(|outcome| {
        matches!(
            outcome,
            StepOutcome::InstallFailed { .. } | StepOutcome::InstalledButCheckStillFails { .. }
        )
    });
    if attempt_failed { InstallStatus::AttemptFailed } else { InstallStatus::AllSucceeded }
}

/// What one run concluded.
///
/// `DryRun` carries a `CheckStatus` rather than its own type, because a dry
/// run answers the same question `deps check` answers: is the environment
/// ready. It is a separate variant only so the table can give it its own
/// column if the codes ever diverge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// A `deps check` run.
    Check(CheckStatus),
    /// A `deps install` run.
    Install(InstallStatus),
    /// A `--dry-run` run.
    DryRun(CheckStatus),
}

/// A process exit code.
///
/// A newtype with a private field, not a `pub enum`. A `pub enum` has public
/// constructors and `#[non_exhaustive]` restrains only other crates, while
/// `config-cli` is in this same workspace, so a `pub enum` cannot make
/// `exit_status` the only constructor. The nine-site provenance regression
/// this prevents is real, so the mechanism has to work rather than be
/// documented.
///
/// The field is private to this module and no `From`, `new`, or `Default`
/// impl exists. [`exit_status`] is the sole way to obtain one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitStatus(u8);

impl ExitStatus {
    /// The code to hand the process exit.
    pub fn code(&self) -> u8 {
        self.0
    }
}

/// A compile-fail witness for `ExitStatus`'s private field.
///
/// The doctests are `compile_fail`, so `cargo test` fails if the field ever
/// becomes public or a public constructor appears. That is the mechanism
/// spec 5.4 requires, and a comment claiming privacy is not.
///
/// ```compile_fail
/// let forged = deps_core::ExitStatus(1);
/// ```
///
/// ```compile_fail
/// let forged: deps_core::ExitStatus = Default::default();
/// ```
#[allow(dead_code)]
fn exit_status_has_no_public_constructor() {}

/// Map one run's verdict to a process exit code.
///
/// The single place this mapping exists. Codes are per-verb disjoint, so a
/// consumer learning "nonzero and not 2 means the environment is not ready"
/// is correct for both verbs permanently, and the unused cells allow growth
/// without renumbering.
///
/// Exit 2 means every caller error, matching the repo-wide convention that
/// `check-deps.sh:109` and `:116` already use and that `deps-docs.test.sh`
/// relies on as its oracle for "the parser rejected this flag". Narrowing 2
/// to one condition would break that oracle's semantics.
pub fn exit_status(result: Result<Verdict, PlanError>) -> ExitStatus {
    let Ok(verdict) = result else {
        return ExitStatus(2);
    };
    match verdict {
        Verdict::Check(CheckStatus::Ready) => ExitStatus(0),
        Verdict::Check(CheckStatus::NotReady) => ExitStatus(1),
        Verdict::DryRun(CheckStatus::Ready) => ExitStatus(0),
        Verdict::DryRun(CheckStatus::NotReady) => ExitStatus(1),
        Verdict::Install(InstallStatus::AllSucceeded) => ExitStatus(0),
        Verdict::Install(InstallStatus::AttemptFailed) => ExitStatus(3),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Check, DependencyName, InstallAction, NoInstallReason, PlanError};
    use dotfiles_path::{BoundedText, CommandName, PackageId};

    fn command_check(name: &str) -> Check {
        Check::Command(CommandName::parse(name).expect("a test command name parses"))
    }

    fn a_package_action() -> InstallAction {
        InstallAction::Package {
            id: PackageId::parse("ripgrep").expect("a test package id parses"),
        }
    }

    #[test]
    fn a_run_where_everything_was_already_present_is_ready() {
        let outcomes = [StepOutcome::AlreadyPresent, StepOutcome::AlreadyPresent];
        assert_eq!(summarize_check(&outcomes), CheckStatus::Ready);
    }

    // The defect spec 5.4 names: today a manual-only dependency is
    // invisible to the exit code, so config-init can report success on a
    // machine that is not ready. NotAutomatable being a variant is what
    // forces this function to decide about it.
    #[test]
    fn a_manual_only_dependency_makes_the_check_not_ready() {
        let outcomes = [
            StepOutcome::AlreadyPresent,
            StepOutcome::NotAutomatable {
                reason: NoInstallReason::UpstreamPublishesNoStableUrl,
            },
        ];
        assert_eq!(
            summarize_check(&outcomes),
            CheckStatus::NotReady,
            "a manual-only dependency must count toward not-ready"
        );
    }

    #[test]
    fn a_blocked_dependency_makes_the_check_not_ready() {
        let outcomes = [StepOutcome::Blocked {
            on: DependencyName::parse("nvm").expect("nvm is a name"),
        }];
        assert_eq!(summarize_check(&outcomes), CheckStatus::NotReady);
    }

    // An install that ran and whose check still fails is an install
    // failure, not a check failure: the tool did something and the
    // something did not work.
    #[test]
    fn an_install_whose_check_still_fails_is_an_attempt_failure() {
        let outcomes = [StepOutcome::InstalledButCheckStillFails {
            check: command_check("rg"),
        }];
        assert_eq!(summarize_install(&outcomes), InstallStatus::AttemptFailed);
    }

    #[test]
    fn an_install_failure_is_an_attempt_failure() {
        let outcomes = [StepOutcome::InstallFailed {
            action: a_package_action(),
            cause: ExecFailure::NonZeroExit {
                code: 100,
                stderr: BoundedText::truncating("E: Unable to locate package"),
            },
        }];
        assert_eq!(summarize_install(&outcomes), InstallStatus::AttemptFailed);
    }

    // summarize_install must NOT report AttemptFailed for a manual-only
    // dependency: nothing was attempted, so no attempt failed. The
    // not-ready signal for that case belongs to summarize_check, which the
    // driver also calls. This is the distinction the single summarize
    // function upheld by hand.
    #[test]
    fn a_manual_only_dependency_is_not_an_attempt_failure() {
        let outcomes = [StepOutcome::NotAutomatable {
            reason: NoInstallReason::RequiresInteractiveApproval,
        }];
        assert_eq!(summarize_install(&outcomes), InstallStatus::AllSucceeded);
        assert_eq!(summarize_check(&outcomes), CheckStatus::NotReady);
    }

    // InstalledButCheckStillFails carries the Check so the report can name
    // which predicate failed rather than saying "the install did not
    // satisfy the check" as check-deps.sh:589 does today.
    #[test]
    fn installed_but_check_still_fails_names_the_predicate() {
        let outcome = StepOutcome::InstalledButCheckStillFails {
            check: command_check("rg"),
        };
        let StepOutcome::InstalledButCheckStillFails { check } = outcome else {
            panic!("the fixture is that variant");
        };
        assert_eq!(check, command_check("rg"));
    }

    // The table in spec 5.4. Every cell, including the reserved ones,
    // asserted as one test so a renumbering cannot slip through per-case.
    #[test]
    fn the_exit_code_table_holds() {
        let cases = [
            (Ok(Verdict::Check(CheckStatus::Ready)), 0),
            (Ok(Verdict::Check(CheckStatus::NotReady)), 1),
            (Ok(Verdict::Install(InstallStatus::AllSucceeded)), 0),
            (Ok(Verdict::Install(InstallStatus::AttemptFailed)), 3),
            (Ok(Verdict::DryRun(CheckStatus::Ready)), 0),
            (Ok(Verdict::DryRun(CheckStatus::NotReady)), 1),
        ];
        for (verdict, expected) in cases {
            assert_eq!(
                exit_status(verdict.clone()).code(),
                expected,
                "the wrong code for {verdict:?}"
            );
        }
    }

    // Exit 2 keeps its repo-wide meaning: the caller made a usage error.
    // Three misuse conditions exit 2 today (check-deps.sh:109 for --only
    // with no value, :116 for an unknown argument, and --only naming a
    // nonexistent dependency), clap exits 2 for its own usage errors
    // deliberately, and deps-docs.test.sh uses exit 2 as its oracle for
    // "the parser rejected this flag". Narrowing 2 breaks that oracle.
    #[test]
    fn every_plan_error_exits_two() {
        let errors = [
            PlanError::UnknownDependency {
                name: DependencyName::parse("ripgpre").expect("a name parses"),
                did_you_mean: DependencyName::parse("ripgrep").ok(),
            },
            PlanError::MalformedSelector {
                raw: crate::RawSelector::parse("a,,b").expect("a bounded selector parses"),
            },
            PlanError::RequirementCycle {
                chain: vec![
                    DependencyName::parse("fzf").expect("a name parses"),
                    DependencyName::parse("ripgrep").expect("a name parses"),
                ],
            },
            PlanError::ManifestVersion { found: 2, supported: 1 },
        ];
        for error in errors {
            assert_eq!(
                exit_status(Err(error.clone())).code(),
                2,
                "every caller error is 2, including {error:?}"
            );
        }
    }

    // The behavior change spec 5.4 makes deliberately.
    // check-deps.sh:600-602 exits 0 unconditionally on --dry-run, pinned by
    // check-deps.test.sh:130 ('dry-run always exits 0'). That is a latent
    // hole: a CI gate on --dry-run passes on a machine with everything
    // missing. "Would install three things" means "three things are
    // missing".
    #[test]
    fn a_dry_run_with_something_missing_does_not_exit_zero() {
        assert_eq!(exit_status(Ok(Verdict::DryRun(CheckStatus::NotReady))).code(), 1);
    }

    // Codes are per-verb disjoint, so a consumer learning "nonzero and not
    // 2 means the environment is not ready" is correct for both verbs
    // permanently.
    #[test]
    fn nonzero_and_not_two_always_means_not_ready() {
        let not_ready = [
            Ok(Verdict::Check(CheckStatus::NotReady)),
            Ok(Verdict::Install(InstallStatus::AttemptFailed)),
            Ok(Verdict::DryRun(CheckStatus::NotReady)),
        ];
        for verdict in not_ready {
            let code = exit_status(verdict).code();
            assert!(code != 0 && code != 2, "the rule requires nonzero and not 2, got {code}");
        }
    }
}
