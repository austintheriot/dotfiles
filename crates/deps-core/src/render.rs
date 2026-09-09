//! Rendering a report to two streams and one exit code.
//!
//! Pure. Follows `config-manifest`'s `stamp::Rendered`, which the parent
//! spec cites as precedent: returning output as a value rather than writing
//! it makes the whole reporting path testable without capturing streams.

use std::fmt::Write as _;

use dotfiles_path::BoundedText;

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

/// Whether rendered output carries ANSI colour.
///
/// A CLOSED SUM DECIDED AT THE IO EDGE, which is the whole design. `render`
/// is pure -- returning the streams rather than writing them is what makes
/// every reporting decision testable without capturing a process's output --
/// and a pure function cannot ask whether stdout is a terminal. So the
/// answer arrives as an argument.
///
/// The two alternatives were worse in ways worth recording. Carrying
/// structured spans instead of a `String` would move "what is a failure" out
/// of the one place that owns it. Painting at the IO edge by pattern-matching
/// the rendered text would re-derive the status from prose after the type
/// that knew it had been discarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// No escapes at all, byte-identical to the output before colour existed.
    ///
    /// This is the contract for every non-terminal caller, not a fallback.
    /// Every gate in this repo greps this output, and an escape in a piped
    /// stream breaks them only in CI, where nobody is watching a terminal.
    Plain,
    /// ANSI SGR colour, for a terminal with a human in front of it.
    Ansi,
}

/// Red: something failed and the run is not going to succeed.
const RED: &str = "\u{1b}[31m";
/// Yellow: something needs a human, but nothing failed.
const YELLOW: &str = "\u{1b}[33m";
/// Green: satisfied, nothing to do.
const GREEN: &str = "\u{1b}[32m";
/// Bold, for the summary heading only.
const BOLD: &str = "\u{1b}[1m";
/// Reset, closing every sequence above.
const RESET: &str = "\u{1b}[0m";

/// Wrap `text` in `colour`, or return it unchanged under `Style::Plain`.
///
/// Takes the word alone rather than a whole line, so the colour covers the
/// status and not the dependency name: a line where everything is red is
/// harder to scan than one where only the verdict is.
fn paint(style: Style, colour: &str, text: &str) -> String {
    match style {
        Style::Plain => text.to_string(),
        Style::Ansi => format!("{colour}{text}{RESET}"),
    }
}

/// Render one report for one verb.
///
/// Pure. The exit code comes from `exit_status`, so this function has no
/// opinion about which number means what: that mapping has exactly one
/// owner, per spec 5.4.
pub fn render(report: &Report, verb: Verb) -> Rendered {
    render_styled(report, verb, Style::Plain)
}

/// Render one report for one verb, with an explicit style.
///
/// Pure. The exit code comes from `exit_status`, so this function has no
/// opinion about which number means what: that mapping has exactly one
/// owner, per spec 5.4.
pub fn render_styled(report: &Report, verb: Verb, style: Style) -> Rendered {
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
            let _ = writeln!(
                stderr,
                "  {} {}",
                paint(style, RED, "BLOCKED  "),
                row.dependency.as_str()
            );
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
            let _ = writeln!(
                stderr,
                "  {} {}",
                paint(style, RED, "FAILED   "),
                row.dependency.as_str()
            );

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

        // SEVERITY PER ROW, chosen from the outcome rather than from the
        // word, so a renamed word cannot silently change its colour.
        //
        //   red     the run failed or cannot proceed
        //   yellow  a human has to act, but nothing failed
        //   green   satisfied, nothing to do
        //
        // The word keeps its column width inside the paint, so a coloured
        // and a plain run align identically -- an escape has no width but a
        // shorter word does.
        let line = match &row.outcome {
            StepOutcome::AlreadyPresent => {
                format!("  {} {}\n", paint(style, GREEN, "present  "), row.dependency.as_str())
            }
            StepOutcome::Installed => {
                format!("  {} {}\n", paint(style, GREEN, "installed"), row.dependency.as_str())
            }
            StepOutcome::NotSelected => {
                format!("  {} {}\n", paint(style, YELLOW, "missing  "), row.dependency.as_str())
            }
            StepOutcome::Blocked { on } => format!(
                "  {} {} (needs {})\n",
                paint(style, YELLOW, "waiting  "),
                row.dependency.as_str(),
                on.as_str()
            ),
            StepOutcome::Unsatisfiable { on } => format!(
                "  {} {} (needs {}, which this run excluded)\n",
                paint(style, RED, "blocked  "),
                row.dependency.as_str(),
                on.as_str()
            ),
            StepOutcome::NotAutomatable { reason } => format!(
                "  {} {} ({reason:?})\n",
                paint(style, YELLOW, "manual   "),
                row.dependency.as_str()
            ),
            StepOutcome::InstallFailed { .. } | StepOutcome::InstalledButCheckStillFails { .. } => {
                format!(
                    "  {} {} (see stderr)\n",
                    paint(style, RED, "failed   "),
                    row.dependency.as_str()
                )
            }
            StepOutcome::Declined => {
                format!("  {} {}\n", paint(style, YELLOW, "declined "), row.dependency.as_str())
            }
        };
        stdout.push_str(&line);
    }

    let heading = match verb {
        Verb::Check => "checked dependencies",
        Verb::Install => "installed dependencies",
        Verb::DryRun => "would install",
    };
    // Bold rather than coloured: the heading is not a severity, and giving
    // it a colour would compete with the words that are. This is the line
    // tests/run-all.sh parses with sed, so the escapes must sit outside the
    // text it matches -- which Style::Plain guarantees for every non-tty
    // caller.
    let summary = format!(
        "{} {} entries\n",
        paint(style, BOLD, &format!("deps {heading}:")),
        report.rows.len()
    );

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
/// The child's own output is reproduced verbatim rather than summarized: it
/// is the only text that distinguishes "no such package" from "no such
/// command" from "permission denied", and every paraphrase this function
/// could write would be a guess about a message it did not produce.
///
/// The stream is named alongside the message so a reader who reruns the
/// command by hand knows which one to watch. stderr wins when both carry
/// text: a script that writes to both puts its diagnostic there and its
/// progress chatter on stdout, and concatenating the two buries the
/// diagnostic under the chatter.
fn describe_cause(outcome: &StepOutcome) -> Option<String> {
    match outcome {
        StepOutcome::InstallFailed { cause, .. } => Some(match cause {
            crate::ExecFailure::NonZeroExit { code, stdout, stderr } => {
                match first_non_empty(&[("stderr", stderr), ("stdout", stdout)]) {
                    Some((stream, message)) => format!("exited {code} ({stream}): {message}"),
                    None => format!("exited {code} with no output on stdout or stderr"),
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

/// The first stream that carries a message, with the stream's name.
///
/// Trimmed before the emptiness test, because a script that exits nonzero
/// after printing only a newline has said nothing: reporting a blank cause
/// reads as a renderer bug rather than as a silent child.
fn first_non_empty<'text>(
    streams: &[(&'static str, &'text BoundedText)],
) -> Option<(&'static str, &'text str)> {
    streams
        .iter()
        .map(|(name, text)| (*name, text.as_str().trim()))
        .find(|(_, message)| !message.is_empty())
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

    /// One failed row carrying the given cause.
    ///
    /// Every failure fixture routes through here so a new `ExecFailure`
    /// variant has one place to be threaded, rather than one per test.
    fn a_report_whose_install_failed_with(cause: crate::ExecFailure) -> Report {
        Report {
            rows: vec![ReportRow {
                dependency: crate::DependencyName::parse("ripgrep").expect("a valid name"),
                outcome: StepOutcome::InstallFailed {
                    action: crate::InstallAction::Package {
                        id: PackageId::parse("ripgrep").expect("a test package id parses"),
                    },
                    cause,
                },
                after: crate::Observation::Absent,
            }],
            check: CheckStatus::NotReady,
            install: InstallStatus::AttemptFailed,
        }
    }

    fn a_report_with_a_failed_install() -> Report {
        a_report_whose_install_failed_with(crate::ExecFailure::NonZeroExit {
            code: 100,
            stdout: BoundedText::truncating(""),
            stderr: BoundedText::truncating("E: Unable to locate package"),
        })
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

    /// A script that explains itself on stdout is still explained.
    ///
    /// The incident this pins, 2026-09-08: four bootstrap legs failed with
    /// `FAILED oh-my-zsh / exited 1 with no output on stderr`. The cause was
    /// oh-my-zsh's installer printing "Zsh is not installed. Please install
    /// zsh first." and exiting 1, and that sentence went to STDOUT. The
    /// engine captured it, discarded it, and told the reader there was no
    /// output. Diagnosing it took three falsified hypotheses and a hand-run
    /// container, because the failing run itself disclosed nothing.
    #[test]
    fn a_cause_written_only_to_stdout_still_reaches_the_reader() {
        let ready = render(&a_ready_report(), Verb::Check);
        assert!(ready.stderr.is_empty(), "the control must leave stderr empty");

        let rendered = render(
            &a_report_whose_install_failed_with(crate::ExecFailure::NonZeroExit {
                code: 1,
                stdout: BoundedText::truncating("Zsh is not installed. Please install zsh first."),
                stderr: BoundedText::truncating(""),
            }),
            Verb::Install,
        );

        assert!(
            rendered.stderr.contains("Zsh is not installed"),
            "a cause on stdout must be reported, not discarded: {}",
            rendered.stderr
        );
        assert!(
            !rendered.stderr.contains("no output"),
            "the engine held the explanation, so it must not claim there was \
             none: {}",
            rendered.stderr
        );
    }

    /// Which stream a message came from is part of the message.
    ///
    /// A reader who reruns the command by hand needs to know where to look.
    /// stderr wins when both carry text, because a script that writes to
    /// both puts its diagnostic there.
    #[test]
    fn the_rendered_cause_names_the_stream_it_came_from() {
        let both = render(
            &a_report_whose_install_failed_with(crate::ExecFailure::NonZeroExit {
                code: 100,
                stdout: BoundedText::truncating("Reading package lists..."),
                stderr: BoundedText::truncating("E: Unable to locate package"),
            }),
            Verb::Install,
        );

        assert!(
            both.stderr.contains("stderr") && both.stderr.contains("E: Unable to locate package"),
            "with both streams populated, the diagnostic is stderr's and the \
             line must say so: {}",
            both.stderr
        );
        assert!(
            !both.stderr.contains("Reading package lists"),
            "stderr wins outright rather than being concatenated with the \
             progress chatter on stdout: {}",
            both.stderr
        );

        let only_stdout = render(
            &a_report_whose_install_failed_with(crate::ExecFailure::NonZeroExit {
                code: 1,
                stdout: BoundedText::truncating("Zsh is not installed."),
                stderr: BoundedText::truncating(""),
            }),
            Verb::Install,
        );

        assert!(
            only_stdout.stderr.contains("stdout"),
            "a message that came from stdout must say stdout, so a reader \
             knows which stream to watch on a rerun: {}",
            only_stdout.stderr
        );
    }

    /// The honest case stays honest.
    ///
    /// "no output" is the right thing to say when both streams really are
    /// empty. This is the positive control for the two tests above: without
    /// it, a renderer that never says "no output" would pass them both.
    #[test]
    fn a_silent_failure_still_reports_that_it_was_silent() {
        let rendered = render(
            &a_report_whose_install_failed_with(crate::ExecFailure::NonZeroExit {
                code: 2,
                stdout: BoundedText::truncating(""),
                stderr: BoundedText::truncating("   \n  "),
            }),
            Verb::Install,
        );

        assert!(
            rendered.stderr.contains("no output"),
            "both streams are empty, so the report must say so rather than \
             printing a blank cause: {}",
            rendered.stderr
        );
        assert!(
            rendered.stderr.contains('2'),
            "the exit code is the only fact a silent failure leaves: {}",
            rendered.stderr
        );
    }

    /// Plain rendering is byte-identical to the uncoloured output.
    ///
    /// THE BREAKAGE CLASS THIS PREVENTS, and it is the one this repo keeps
    /// hitting. Every gate here greps this output: `tests/run-all.sh` parses
    /// the summary line with sed, the deps-check workflow greps for
    /// `present   neovim`, and `tests/container.test.sh` matches assertion
    /// text. An escape sequence in a piped stream breaks all of them, and it
    /// breaks them only in CI, where nobody is watching a terminal.
    ///
    /// So `Style::Plain` is the contract, not a fallback: the caller decides
    /// at the IO edge, and a non-tty caller gets exactly the bytes it got
    /// before colour existed.
    #[test]
    fn plain_style_emits_no_escape_sequences() {
        for report in [
            a_ready_report(),
            a_report_with_a_failed_install(),
            a_report_that_installed_nothing_and_is_not_ready(),
        ] {
            for verb in [Verb::Check, Verb::Install, Verb::DryRun] {
                let rendered = render_styled(&report, verb, Style::Plain);
                assert!(
                    !rendered.stdout.contains('\u{1b}'),
                    "plain stdout must carry no escape: {:?}",
                    rendered.stdout
                );
                assert!(
                    !rendered.stderr.contains('\u{1b}'),
                    "plain stderr must carry no escape: {:?}",
                    rendered.stderr
                );
            }
        }
    }

    /// Plain is exactly what `render` produced before colour existed.
    ///
    /// Stronger than "no escapes": a styling change that reworded a line
    /// would pass that check and still break every grep. This pins the
    /// bytes.
    #[test]
    fn plain_style_matches_the_uncoloured_renderer() {
        let report = a_report_with_a_failed_install();
        let plain = render_styled(&report, Verb::Install, Style::Plain);
        let default = render(&report, Verb::Install);

        assert_eq!(plain.stdout, default.stdout, "render() must stay plain");
        assert_eq!(plain.stderr, default.stderr, "render() must stay plain");
    }

    /// Failures are red, warnings yellow, successes green.
    ///
    /// The severity mapping is asserted per word rather than as a blanket
    /// "output contains colour", because the point is that a reader can tell
    /// severity at a glance: an error that came out yellow is worse than no
    /// colour at all.
    ///
    /// The trailing spaces are inside the escape on purpose: the column
    /// padding belongs to the painted word so a coloured and a plain run
    /// align identically. An escape has no display width, but a word that
    /// lost its padding does.
    #[test]
    fn ansi_style_colours_by_severity() {
        let failed = render_styled(&a_report_with_a_failed_install(), Verb::Install, Style::Ansi);
        assert!(
            failed.stderr.contains(&format!("{RED}FAILED   {RESET}")),
            "a failure must be red on stderr: {:?}",
            failed.stderr
        );
        assert!(
            failed.stdout.contains(&format!("{RED}failed   {RESET}")),
            "the stdout row for a failure must be red too: {:?}",
            failed.stdout
        );

        let ready = render_styled(&a_ready_report(), Verb::Check, Style::Ansi);
        assert!(
            ready.stdout.contains(&format!("{GREEN}present  {RESET}")),
            "a satisfied row must be green: {:?}",
            ready.stdout
        );

        let manual =
            render_styled(&a_report_that_installed_nothing_and_is_not_ready(), Verb::Install, Style::Ansi);
        assert!(
            manual.stdout.contains(&format!("{YELLOW}manual   {RESET}")),
            "a row needing human action is a warning, so it must be yellow: {:?}",
            manual.stdout
        );
    }

    /// Colour is redundant emphasis: the words survive it.
    ///
    /// A reader on a mono terminal, reading a log file, or with
    /// deuteranopia must lose nothing. So every status word still appears in
    /// the coloured output, and stripping the escapes yields the plain bytes
    /// exactly.
    #[test]
    fn colour_never_replaces_a_word() {
        let report = a_report_with_a_failed_install();
        let plain = render_styled(&report, Verb::Install, Style::Plain);
        let ansi = render_styled(&report, Verb::Install, Style::Ansi);

        assert_eq!(
            strip_escapes(&ansi.stdout),
            plain.stdout,
            "stripping colour must give back the plain bytes"
        );
        assert_eq!(
            strip_escapes(&ansi.stderr),
            plain.stderr,
            "stripping colour must give back the plain bytes"
        );
    }

    /// Remove ANSI CSI sequences, for the redundancy assertion above.
    fn strip_escapes(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut chars = text.chars();
        while let Some(character) = chars.next() {
            if character != '\u{1b}' {
                out.push(character);
                continue;
            }
            // Skip "[...m": every sequence this renderer emits is an SGR.
            for skipped in chars.by_ref() {
                if skipped == 'm' {
                    break;
                }
            }
        }
        out
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
