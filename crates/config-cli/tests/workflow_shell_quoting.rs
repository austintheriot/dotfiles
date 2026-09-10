//! The single-quoted `sh -c '...'` bodies in `.github/workflows`.
//!
//! THE BUG THIS EXISTS FOR, 2026-09-08. A job passes a whole script to a
//! container as one single-quoted argument:
//!
//! ```text
//! depcheck-pop -c '
//!   set -eu
//!   ...
//! '
//! ```
//!
//! Any single quote inside that body closes the argument early. A runtime
//! assertion added to the Pop!_OS leg wrapped its Lua in single quotes and
//! included the word "repo's" in a comment, and the step died with
//! `Process completed with exit code 127` and no output whatsoever: not a
//! failed assertion, an unparseable command. 127 is "command not found", so
//! the job reported a missing binary for what was really a quoting error,
//! and the assertion the step existed to run never executed.
//!
//! WHY A GUARD RATHER THAN CARE. The failure is invisible to every cheap
//! check: the YAML parses, `actionlint` does not read into the string, and
//! the body only breaks when the shell splits it. It cost a full red CI
//! round to find, and the natural fix (an apostrophe in an English comment)
//! reintroduces it silently.
//!
//! WHAT IS ASSERTED. Each single-quoted `-c '...'` body must contain no
//! single quote at all, and must parse as a shell script under `sh -n`. The
//! second half is what catches an unbalanced construct that happens to avoid
//! apostrophes.
//!
//! Converted whole from `tests/workflow-shell-quoting.test.sh`, which had 2
//! `assert_*` call sites. The second folded every body's verdict into one
//! compared string; here each body is its own assertion, so a failure names
//! the body rather than appearing inside a concatenated blob.
//!
//! This suite and `rust-gate` were the last two readers of the container's
//! `python3-yaml` package. This one converts first, so the package stays
//! until `rust-gate` follows.

use dotfiles_test_support::repo::{self, root as repo_root};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// One `-c '...'` body, with enough context to name where it lives.
struct Body {
    /// `<file>:<job>:step<index>`, the shell suite's own header format.
    location: String,
    text: String,
    /// Whether the quote-only line that closes the argument was never found.
    unterminated: bool,
}

/// Every `-c '<body>'` argument in a workflow's steps.
///
/// THE BODY ENDS AT A LINE WHOSE ONLY CONTENT IS A QUOTE, not at the next
/// quote character. A first version of the shell extractor split on the
/// first `'` it found, which made the guard blind to the very bug it exists
/// for: an apostrophe in a mid-body comment terminated the extracted body
/// there, so the quote was never inside the text being checked and the
/// sabotage passed. The shell does not stop at the first quote of a line, so
/// neither may the extractor.
fn bodies_in(path: &Path) -> Vec<Body> {
    let workflow = repo::read_workflow(path);
    let file = repo::file_name(path);
    let mut found = Vec::new();

    for (job_name, job) in repo::jobs_of(&workflow) {
        let steps = job
            .get("steps")
            .and_then(yaml_serde::Value::as_sequence)
            .map(Vec::as_slice)
            .unwrap_or_default();

        for (index, step) in steps.iter().enumerate() {
            let Some(run) = step.get("run").and_then(yaml_serde::Value::as_str) else {
                continue;
            };
            if !run.contains("-c '") {
                continue;
            }

            let mut collecting = false;
            let mut lines: Vec<&str> = Vec::new();
            for line in run.lines() {
                if !collecting {
                    if line.trim_end().ends_with("-c '") {
                        collecting = true;
                        lines.clear();
                    }
                    continue;
                }
                if line.trim() == "'" {
                    if lines.iter().any(|entry| !entry.trim().is_empty()) {
                        found.push(Body {
                            location: format!("{file}:{job_name}:step{index}"),
                            text: lines.join("\n"),
                            unterminated: false,
                        });
                    }
                    collecting = false;
                    continue;
                }
                lines.push(line);
            }
            // An unterminated body is itself a defect: the quote-only line
            // the shell needs to close the argument is missing.
            if collecting && lines.iter().any(|entry| !entry.trim().is_empty()) {
                found.push(Body {
                    location: format!("{file}:{job_name}:step{index}"),
                    text: lines.join("\n"),
                    unterminated: true,
                });
            }
        }
    }
    found
}

/// Every body across every workflow in this checkout.
fn all_bodies() -> Vec<Body> {
    repo::workflow_files()
        .iter()
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("yml"))
        .flat_map(|path| bodies_in(path))
        .collect()
}

/// Whether `sh -n` accepts the text as a shell script.
fn parses_as_shell(text: &str) -> bool {
    let Ok(mut child) = Command::new("sh")
        .arg("-n")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }
    child.wait().is_ok_and(|status| status.success())
}

#[test]
fn the_workflow_directory_exists() {
    let directory = repo_root().join(".github/workflows");
    assert!(
        directory.is_dir(),
        "no workflow directory at {}",
        directory.display()
    );
}

#[test]
fn every_single_quoted_body_is_quote_free_and_parses() {
    let bodies = all_bodies();

    // The count is a positive control rather than a separate concern. An
    // extractor that silently found nothing would otherwise report zero
    // violations and pass while checking nothing, which is the exact failure
    // shape this repo keeps hitting: a guard whose derivation broke compares
    // empty to empty and reports success.
    assert!(
        !bodies.is_empty(),
        "the extractor found no `sh -c` bodies at all, so a clean result \
         would prove nothing; the workflows or the extractor changed"
    );

    for body in &bodies {
        assert!(
            !body.unterminated,
            "{}: the quote-only line that closes the sh -c argument is missing",
            body.location
        );
        assert!(
            !body.text.contains('\''),
            "{}: contains a single quote, which closes the sh -c argument early",
            body.location
        );
        assert!(
            parses_as_shell(&body.text),
            "{}: does not parse as a shell script under `sh -n`",
            body.location
        );
    }
}
