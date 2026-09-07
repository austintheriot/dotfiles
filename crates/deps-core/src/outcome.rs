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
    /// `retired-check-deps:589` prints "install did not satisfy the check for
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
    /// In the manifest, absent from the machine, and blocked on a
    /// prerequisite this run excluded from the selection.
    ///
    /// Distinct from `Blocked`, which claims a later wave can unblock it.
    /// Nothing unblocks this one: the selection is fixed before the first
    /// wave plans, so the prerequisite named here is never installed by
    /// this run. Keeping the two apart is what stops a permanently
    /// unsatisfiable step from rendering as `waiting`.
    Unsatisfiable {
        /// The deselected prerequisite this step can never get.
        on: DependencyName,
    },
    /// In the manifest, absent from the machine, and never considered by
    /// this run.
    ///
    /// Distinct from `Blocked`, which names a real prerequisite the step
    /// waits on. Reconcile emits a row per manifest entry, so an entry the
    /// selection excluded has no outcome and no prerequisite to name.
    /// Reporting it as `Blocked { on: <itself> }` said a dependency waits on
    /// itself, which is not a state that exists, and a caller rendering the
    /// `on` field would print it.
    NotSelected,
    /// The step had an automated install and the user declined it.
    ///
    /// Distinct from every neighbour: `NotAutomatable` claims no automated
    /// install exists, `Blocked` claims a later wave can unblock it, and
    /// `InstallFailed` claims an attempt failed. All three are false here,
    /// and reporting a decline as any of them is the kind of false statement
    /// parent 6.2 rejected `DescribeOnly` for.
    Declined,
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
    ///
    /// Carries both summaries, because they answer different questions and
    /// a run can succeed at every attempt while leaving the machine
    /// incomplete. Reporting only the first is how an install exited 0 with
    /// three dependencies missing.
    Install(InstallStatus, CheckStatus),
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
/// The table:
///
/// | Verdict                                          | Code |
/// |---------------------------------------------------|------|
/// | `Check(Ready)`                                     | 0    |
/// | `Check(NotReady)`                                  | 1    |
/// | `DryRun(Ready)`                                    | 0    |
/// | `DryRun(NotReady)`                                 | 1    |
/// | `Install(AllSucceeded, Ready)`                     | 0    |
/// | `Install(AllSucceeded, NotReady)`                  | 4    |
/// | `Install(AttemptFailed, _)`                        | 3    |
/// | `Err(_)`                                           | 2    |
///
/// Exit 2 means every caller error, matching the repo-wide convention that
/// `retired-check-deps:109` and `:116` already use and that `deps-docs.test.sh`
/// relies on as its oracle for "the parser rejected this flag". Narrowing 2
/// to one condition would break that oracle's semantics.
///
/// Exit 4 means every attempted install succeeded and the machine is still
/// not ready, the manual-only case. Distinct from 3, because nothing
/// failed, and distinct from 0, because the caller cannot proceed.
pub fn exit_status(result: Result<Verdict, PlanError>) -> ExitStatus {
    let Ok(verdict) = result else {
        return ExitStatus(2);
    };
    match verdict {
        Verdict::Check(CheckStatus::Ready) => ExitStatus(0),
        Verdict::Check(CheckStatus::NotReady) => ExitStatus(1),
        Verdict::DryRun(CheckStatus::Ready) => ExitStatus(0),
        Verdict::DryRun(CheckStatus::NotReady) => ExitStatus(1),
        Verdict::Install(InstallStatus::AttemptFailed, _) => ExitStatus(3),
        Verdict::Install(InstallStatus::AllSucceeded, CheckStatus::NotReady) => ExitStatus(4),
        Verdict::Install(InstallStatus::AllSucceeded, CheckStatus::Ready) => ExitStatus(0),
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

    // An unsatisfiable row is a definite non-success, so the check must not
    // report ready. `summarize_check` allowlists the ready outcomes rather
    // than matching exhaustively, so a new variant defaults to not-ready and
    // no compiler error would have caught the opposite. This test is what
    // holds that.
    #[test]
    fn an_unsatisfiable_dependency_makes_the_check_not_ready() {
        let outcomes = [StepOutcome::Unsatisfiable {
            on: DependencyName::parse("oh-my-zsh").expect("oh-my-zsh is a name"),
        }];
        assert_eq!(summarize_check(&outcomes), CheckStatus::NotReady);
    }

    // An unsatisfiable step was never handed to an installer, so no attempt
    // failed. The run is not ready and no install was tried, which is a
    // different fact from a failure and must not be reported as one.
    #[test]
    fn an_unsatisfiable_dependency_is_not_an_attempt_failure() {
        let outcomes = [StepOutcome::Unsatisfiable {
            on: DependencyName::parse("oh-my-zsh").expect("oh-my-zsh is a name"),
        }];
        assert_eq!(summarize_install(&outcomes), InstallStatus::AllSucceeded);
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

    /// An install that attempted nothing and left the machine incomplete
    /// must not exit 0.
    ///
    /// The reported incident: a bootstrap ended "no unresolved failures (16
    /// of 18 were already missing)" and exited 0 with three dependencies
    /// absent. summarize_install said AllSucceeded, because NotAutomatable
    /// is not an attempt failure, and Verdict::Install carried no readiness,
    /// so the exit table had no cell that could disagree.
    #[test]
    fn an_install_that_leaves_the_machine_not_ready_does_not_exit_zero() {
        let manual_only = [StepOutcome::NotAutomatable {
            reason: NoInstallReason::UpstreamPublishesNoStableUrl,
        }];

        // Positive controls. Nothing was attempted, so nothing failed, and
        // the machine is not ready. Both must hold or the assertion below
        // is about the wrong inputs.
        assert_eq!(summarize_install(&manual_only), InstallStatus::AllSucceeded);
        assert_eq!(summarize_check(&manual_only), CheckStatus::NotReady);

        let verdict = Verdict::Install(
            summarize_install(&manual_only),
            summarize_check(&manual_only),
        );
        assert_ne!(
            exit_status(Ok(verdict)).code(),
            0,
            "an incomplete machine must not report success"
        );
    }

    /// The success path still exits 0, so the fix does not make every
    /// install look like a failure.
    #[test]
    fn an_install_that_leaves_the_machine_ready_exits_zero() {
        let all_good = [StepOutcome::Installed, StepOutcome::AlreadyPresent];
        assert_eq!(summarize_install(&all_good), InstallStatus::AllSucceeded);
        assert_eq!(summarize_check(&all_good), CheckStatus::Ready);
        assert_eq!(
            exit_status(Ok(Verdict::Install(InstallStatus::AllSucceeded, CheckStatus::Ready)))
                .code(),
            0
        );
    }

    // InstalledButCheckStillFails carries the Check so the report can name
    // which predicate failed rather than saying "the install did not
    // satisfy the check" as retired-check-deps:589 does today.
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
            (Ok(Verdict::Install(InstallStatus::AllSucceeded, CheckStatus::Ready)), 0),
            (Ok(Verdict::Install(InstallStatus::AllSucceeded, CheckStatus::NotReady)), 4),
            (Ok(Verdict::Install(InstallStatus::AttemptFailed, CheckStatus::NotReady)), 3),
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
    // Three misuse conditions exit 2 today (retired-check-deps:109 for --only
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
    // retired-check-deps:600-602 exits 0 unconditionally on --dry-run, pinned by
    // the retired suite's 'dry-run always exits 0'. That is a latent
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
            Ok(Verdict::Install(InstallStatus::AttemptFailed, CheckStatus::NotReady)),
            Ok(Verdict::Install(InstallStatus::AllSucceeded, CheckStatus::NotReady)),
            Ok(Verdict::DryRun(CheckStatus::NotReady)),
        ];
        for verdict in not_ready {
            let code = exit_status(verdict).code();
            assert!(code != 0 && code != 2, "the rule requires nonzero and not 2, got {code}");
        }
    }

    /// An empty run is Ready, and that is deliberate rather than accidental.
    ///
    /// `summarize_check` folds with `all`, which is vacuously true on an
    /// empty slice, so a run with no outcomes reports Ready and exits 0.
    /// Reported as unpinned by the task that wrote the summaries, because the
    /// intended answer was not stated anywhere.
    ///
    /// Ready is right: asking whether nothing is ready, and being told yes,
    /// is the correct answer to the question asked. The case that must NOT
    /// exit 0 is a selection naming something the manifest lacks, and that is
    /// a different code path which returns `PlanError::UnknownDependency`
    /// (mapped to exit 2), pinned by
    /// `plan::tests::a_selected_name_the_manifest_lacks_is_an_error`.
    ///
    /// Without this test, someone tightening the empty case to NotReady would
    /// make `config deps check --only ""` fail on a machine with nothing
    /// wrong with it, and no assertion would object.
    #[test]
    fn an_empty_run_is_ready_and_exits_zero() {
        let none: Vec<StepOutcome> = Vec::new();

        // Positive control: the same fold must report NotReady for a real
        // failure, or "Ready" below would prove nothing about emptiness.
        let failing = vec![StepOutcome::NotAutomatable {
            reason: NoInstallReason::UpstreamPublishesNoStableUrl,
        }];
        assert_eq!(summarize_check(&failing), CheckStatus::NotReady);

        assert_eq!(summarize_check(&none), CheckStatus::Ready);
        assert_eq!(exit_status(Ok(Verdict::Check(CheckStatus::Ready))).code(), 0);

        // The install verb agrees: nothing attempted means nothing failed,
        // and the empty machine is Ready, so it exits 0 rather than 4.
        assert_eq!(summarize_install(&none), InstallStatus::AllSucceeded);
        assert_eq!(
            exit_status(Ok(Verdict::Install(InstallStatus::AllSucceeded, CheckStatus::Ready)))
                .code(),
            0
        );
    }

    /// A declined step leaves the machine not ready, and is not a failure.
    ///
    /// There was no outcome for "the user said no". NotAutomatable is false,
    /// because an automated install exists. Blocked is false, because
    /// nothing unblocks it. InstallFailed is false, because nothing failed.
    /// Reporting a decline as any of the three is a false statement, which
    /// is the argument parent 6.2 used to reject DescribeOnly.
    #[test]
    fn a_declined_step_is_not_ready_and_not_a_failure() {
        let declined = [StepOutcome::Declined];

        // Positive control: the same summaries must call an installed step
        // ready and unfailed, or the assertions below hold for any input.
        assert_eq!(summarize_check(&[StepOutcome::Installed]), CheckStatus::Ready);
        assert_eq!(
            summarize_install(&[StepOutcome::Installed]),
            InstallStatus::AllSucceeded
        );

        assert_eq!(
            summarize_check(&declined),
            CheckStatus::NotReady,
            "the machine is missing a dependency the user chose not to install"
        );
        assert_eq!(
            summarize_install(&declined),
            InstallStatus::AllSucceeded,
            "nothing was attempted, so no attempt failed"
        );

        // And the two together must not exit 0, which is Task 3's table.
        assert_ne!(
            exit_status(Ok(Verdict::Install(
                summarize_install(&declined),
                summarize_check(&declined),
            )))
            .code(),
            0
        );
    }
}
