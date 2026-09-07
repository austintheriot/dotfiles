//! Rendering a report to two streams and one exit code.
//!
//! Pure. Follows `config-manifest`'s `stamp::Rendered`, which the parent
//! spec cites as precedent: returning output as a value rather than writing
//! it makes the whole reporting path testable without capturing streams.

use std::fmt::Write as _;

use crate::{Report, StepOutcome};

/// Which verb produced a report.
///
/// The wording and the exit code both depend on it: a `NotReady` report is
/// a failure for `check` and an accurate preview for `--dry-run`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    /// `config deps check`.
    Check,
    /// `config deps install`.
    Install,
    /// Either verb with `--dry-run`.
    DryRun,
}

/// Two output streams and an exit code, as a value.
///
/// Follows `config-manifest`'s `stamp::Rendered`. Returning the streams
/// rather than writing them keeps every reporting decision testable without
/// capturing a process's output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// What belongs on stdout.
    pub stdout: String,
    /// What belongs on stderr. Empty when nothing failed.
    pub stderr: String,
    /// The code to hand the process exit.
    pub exit_code: u8,
}

/// Render one report for one verb.
///
/// Pure. The exit code comes from `exit_status`, so this function has no
/// opinion about which number means what: that mapping has exactly one
/// owner, per spec 5.4.
pub fn render(report: &Report, verb: Verb) -> Rendered {
    let mut stdout = String::new();
    let mut stderr = String::new();

    for row in &report.rows {
        // A failed row writes to both streams: stdout keeps the per-row
        // listing complete so its count matches the summary line, and
        // stderr is what a caller greps for.
        // An unsatisfiable row is terminal at plan time, so it reaches
        // stderr the way a failure does. It is not a failure -- nothing was
        // attempted -- so it carries its own word rather than FAILED.
        if let StepOutcome::Unsatisfiable { on } = &row.outcome {
            let _ = writeln!(stderr, "  BLOCKED   {}", row.dependency.as_str());
            let _ = writeln!(
                stderr,
                "              needs {}, which this run excluded from the selection",
                on.as_str()
            );
            let _ = writeln!(
                stderr,
                "              no later wave can install it; name it in --only or drop the filter"
            );
        }

        if matches!(
            row.outcome,
            StepOutcome::InstallFailed { .. } | StepOutcome::InstalledButCheckStillFails { .. }
        ) {
            // Written rather than push_str(&format!(..)): formatting straight
            // into the buffer skips the intermediate String, and `Write for
            // String` is infallible so the discarded Err does not exist.
            let _ = writeln!(stderr, "  FAILED    {}", row.dependency.as_str());

            // The cause, indented under the name it belongs to. Without it
            // the stdout line's "(see stderr)" points at a stream that only
            // repeats the dependency name, and a failed run discloses
            // nothing about why it failed.
            if let Some(cause) = describe_cause(&row.outcome) {
                for line in cause.lines() {
                    let _ = writeln!(stderr, "              {line}");
                }
            }
        }

        let line = match &row.outcome {
            StepOutcome::AlreadyPresent => format!("  present   {}\n", row.dependency.as_str()),
            StepOutcome::Installed => format!("  installed {}\n", row.dependency.as_str()),
            StepOutcome::NotSelected => format!("  missing   {}\n", row.dependency.as_str()),
            StepOutcome::Blocked { on } => {
                format!("  waiting   {} (needs {})\n", row.dependency.as_str(), on.as_str())
            }
            StepOutcome::Unsatisfiable { on } => format!(
                "  blocked   {} (needs {}, which this run excluded)\n",
                row.dependency.as_str(),
                on.as_str()
            ),
            StepOutcome::NotAutomatable { reason } => {
                format!("  manual    {} ({reason:?})\n", row.dependency.as_str())
            }
            StepOutcome::InstallFailed { .. } | StepOutcome::InstalledButCheckStillFails { .. } => {
                format!("  failed    {} (see stderr)\n", row.dependency.as_str())
            }
            StepOutcome::Declined => format!("  declined  {}\n", row.dependency.as_str()),
        };
        stdout.push_str(&line);
    }

    let heading = match verb {
        Verb::Check => "checked dependencies",
        Verb::Install => "installed dependencies",
        Verb::DryRun => "would install",
    };
    let summary = format!("deps {heading}: {} entries\n", report.rows.len());

    let verdict = match verb {
        Verb::Check => crate::Verdict::Check(report.check),
        Verb::DryRun => crate::Verdict::DryRun(report.check),
        Verb::Install => crate::Verdict::Install(report.install, report.check),
    };

    Rendered {
        stdout: format!("{summary}{stdout}"),
        stderr,
        exit_code: crate::exit_status(Ok(verdict)).code(),
    }
}

/// The human-readable reason one failed row failed, if the outcome carries
/// one.
///
/// Returns `None` for an outcome that is not a failure, so the caller can
/// stay a single `if let` rather than repeating the failure match.
///
/// The child's own stderr is reproduced verbatim rather than summarized: it
/// is the only text that distinguishes "no such package" from "no such
/// command" from "permission denied", and every paraphrase this function
/// could write would be a guess about a message it did not produce.
fn describe_cause(outcome: &StepOutcome) -> Option<String> {
    match outcome {
        StepOutcome::InstallFailed { cause, .. } => Some(match cause {
            crate::ExecFailure::NonZeroExit { code, stderr } => {
                let message = stderr.as_str().trim();
                if message.is_empty() {
                    format!("exited {code} with no output on stderr")
                } else {
                    format!("exited {code}: {message}")
                }
            }
            crate::ExecFailure::AuthenticationRefused => {
                String::from("sudo refused authentication")
            }
            crate::ExecFailure::Spawn(error) => match error {
                crate::SpawnError::NotFound => {
                    String::from("the command is not on PATH")
                }
                crate::SpawnError::PermissionDenied => {
                    String::from("the command exists and could not be executed")
                }
                crate::SpawnError::Other => String::from("the command could not be started"),
            },
        }),
        StepOutcome::InstalledButCheckStillFails { check } => {
            Some(format!("the install succeeded and the check still fails: {check:?}"))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CheckStatus, InstallStatus, Report, ReportRow, StepOutcome};
    use dotfiles_path::{BoundedText, PackageId};

    fn a_ready_report() -> Report {
        Report {
            rows: vec![ReportRow {
                dependency: crate::DependencyName::parse("git").expect("a valid name"),
                outcome: StepOutcome::AlreadyPresent,
                after: crate::Observation::Present,
            }],
            check: CheckStatus::Ready,
            install: InstallStatus::AllSucceeded,
        }
    }

    fn a_report_with_a_failed_install() -> Report {
        Report {
            rows: vec![ReportRow {
                dependency: crate::DependencyName::parse("ripgrep").expect("a valid name"),
                outcome: StepOutcome::InstallFailed {
                    action: crate::InstallAction::Package {
                        id: PackageId::parse("ripgrep").expect("a test package id parses"),
                    },
                    cause: crate::ExecFailure::NonZeroExit {
                        code: 100,
                        stderr: BoundedText::truncating("E: Unable to locate package"),
                    },
                },
                after: crate::Observation::Absent,
            }],
            check: CheckStatus::NotReady,
            install: InstallStatus::AttemptFailed,
        }
    }

    /// The incident shape: a manual-only dependency, so nothing was
    /// attempted (`install: AllSucceeded`) and the machine is still not
    /// ready (`check: NotReady`).
    fn a_report_that_installed_nothing_and_is_not_ready() -> Report {
        Report {
            rows: vec![ReportRow {
                dependency: crate::DependencyName::parse("nvm").expect("a valid name"),
                outcome: StepOutcome::NotAutomatable {
                    reason: crate::NoInstallReason::UpstreamPublishesNoStableUrl,
                },
                after: crate::Observation::Absent,
            }],
            check: CheckStatus::NotReady,
            install: InstallStatus::AllSucceeded,
        }
    }

    /// The regression test for the reported incident: a bootstrap ended "no
    /// unresolved failures (16 of 18 were already missing)" and exited 0
    /// with three dependencies absent. That run flowed through `render`,
    /// not through a hand-built `Verdict`, so this test exercises the
    /// `Verb::Install` arm directly rather than the exit table in
    /// `outcome.rs`, which `render` merely calls.
    #[test]
    fn an_install_that_leaves_the_machine_not_ready_renders_exit_code_four() {
        // Positive control: a ready install must still exit 0, or the
        // assertion below would hold for a renderer that returns nonzero
        // for every `Verb::Install` call regardless of readiness.
        assert_eq!(render(&a_ready_report(), Verb::Install).exit_code, 0);

        let rendered = render(&a_report_that_installed_nothing_and_is_not_ready(), Verb::Install);
        assert_eq!(
            rendered.exit_code, 4,
            "every attempt succeeded but the machine is not ready, which is code 4, \
             not 0 (success) and not 3 (an attempt failed)"
        );
    }

    /// A ready check reports success on stdout and exits 0.
    #[test]
    fn a_ready_check_renders_to_stdout_and_exits_zero() {
        let rendered = render(&a_ready_report(), Verb::Check);

        // Positive control: stdout must carry something, or the assertions
        // below would hold for a renderer that writes nothing at all.
        assert!(!rendered.stdout.is_empty(), "a check must say something");
        assert_eq!(rendered.exit_code, 0);
        assert!(rendered.stderr.is_empty(), "nothing failed, so stderr stays empty");
    }

    /// A failed row writes to both streams and still appears in the stdout
    /// listing, so the listing's row count matches the summary line.
    #[test]
    fn a_failed_install_names_the_dependency_on_both_streams() {
        // Positive control: the ready fixture leaves stderr empty (proved
        // in `a_ready_check_renders_to_stdout_and_exits_zero` above), which
        // is what makes a non-empty stderr below mean something rather than
        // holding for a renderer that always writes to stderr.
        let ready = render(&a_ready_report(), Verb::Check);
        assert!(ready.stderr.is_empty(), "the control must leave stderr empty");

        let rendered = render(&a_report_with_a_failed_install(), Verb::Check);

        assert!(
            !rendered.stderr.is_empty(),
            "a failed install must be visible on stderr, which is what a caller greps for"
        );
        assert!(
            rendered.stderr.contains("ripgrep"),
            "stderr must name the dependency that failed, not just say something failed"
        );
        assert!(
            rendered.stdout.contains("ripgrep"),
            "the row must still appear in the stdout listing, or its count desyncs from the summary"
        );
        assert_ne!(
            rendered.exit_code, 0,
            "a NotReady report must not exit 0 on `check`"
        );
    }

    /// stderr carries the CAUSE, not only the name of what failed.
    ///
    /// The defect this pins: every failed row printed
    /// `failed <name> (see stderr)` on stdout while stderr held only
    /// `FAILED <name>`. The line told a reader to look somewhere that
    /// repeated the same fact, and the captured child stderr on
    /// `ExecFailure::NonZeroExit` was never rendered at all.
    ///
    /// Measured cost: a CI run reported 13 dependencies FAILED with no
    /// reason anywhere in the log. Diagnosing it needed a local
    /// reproduction and a `--dry-run` to see the argv, because the failing
    /// run itself disclosed nothing about why.
    #[test]
    fn a_failed_install_puts_the_cause_on_stderr() {
        // Positive control: the ready fixture leaves stderr empty, so a
        // non-empty stderr below is about this report and not about a
        // renderer that always writes.
        let ready = render(&a_ready_report(), Verb::Check);
        assert!(ready.stderr.is_empty(), "the control must leave stderr empty");

        let rendered = render(&a_report_with_a_failed_install(), Verb::Install);

        assert!(
            rendered.stderr.contains("E: Unable to locate package"),
            "stderr must carry the child's own message, which is the only text \
             that says WHY: {}",
            rendered.stderr
        );
        assert!(
            rendered.stderr.contains("100"),
            "stderr must carry the exit code the command reported: {}",
            rendered.stderr
        );
    }

    /// The verb changes the wording, which is what makes the per-verb exit
    /// codes legible: the same NotReady report is a failure for `check` and
    /// a preview for `--dry-run`.
    #[test]
    fn the_verb_changes_the_wording() {
        let report = a_ready_report();
        let checked = render(&report, Verb::Check);
        let previewed = render(&report, Verb::DryRun);

        assert!(!checked.stdout.is_empty(), "the control must produce output");
        assert_ne!(
            checked.stdout, previewed.stdout,
            "a check and a dry run must not read identically"
        );
    }
}
