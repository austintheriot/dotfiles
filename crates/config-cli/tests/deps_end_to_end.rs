//! `deps check` and `deps install` driven against the built binary.
//!
//! Every test writes its own manifest into a temporary directory and points
//! `DEPS_CONF` at it, so no test reads the repository's real conf files and
//! none of them can install anything. That is what keeps this crate
//! hermetic, which the host-side Rust gate depends on: a test that mutated
//! the real `$HOME` would make the gate's result depend on the machine it
//! ran on.

use std::path::Path;
use std::process::{Command, Output};

/// Run the built binary with a fixture manifest and no platform variant.
///
/// `DEPS_LOCAL_CONF` is set to a path that does not exist on purpose. An
/// explicit value is what suppresses the platform variant, so a fixture run
/// cannot pull `deps-mac.toml` or `deps-linux.toml` into a manifest the test
/// wrote by hand.
fn run_against(conf: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(arguments)
        .env("DEPS_CONF", conf)
        .env("DEPS_LOCAL_CONF", "/nonexistent/deps-platform.conf")
        .output()
        .expect("the binary runs")
}

/// Write a manifest of command checks into a fresh temporary directory.
///
/// Takes `(dependency, command)` pairs and renders the TOML, so no call site
/// below writes manifest syntax. When the format changed from pipe-delimited
/// to TOML, this one function absorbed it and the seven call sites did not
/// move -- which is the point of a fixture builder over a literal.
fn fixture_manifest(entries: &[(&str, &str)]) -> (tempfile::TempDir, std::path::PathBuf) {
    use std::fmt::Write;

    let mut text = String::new();
    for (dependency, command) in entries {
        // `writeln!` into the buffer rather than push_str(&format!(..)):
        // formatting straight in skips the intermediate String, and
        // `Write for String` is infallible so the discarded Err does not
        // exist. Same shape as render.rs.
        let _ = write!(
            text,
            "[{dependency}]\ncommand = \"{command}\"\ndocs = \"https://example.invalid/{dependency}\"\n\n"
        );
    }
    fixture_manifest_text(&text)
}

/// Write verbatim manifest text into a fresh temporary directory.
///
/// For the one test that needs text which does NOT parse, where a builder
/// that always emits valid TOML is the wrong tool.
fn fixture_manifest_text(text: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let fixture = tempfile::tempdir().expect("tempdir");
    let conf = fixture.path().join("deps.toml");
    std::fs::write(&conf, text).expect("write");
    (fixture, conf)
}

/// `deps check` on a manifest whose dependency is present exits 0 and says so.
#[test]
fn check_exits_zero_when_everything_is_present() {
    // A check that is true on any machine: `sh` is on PATH everywhere this
    // repo runs, and the test asserts that below rather than assuming it.
    let (_fixture, conf) = fixture_manifest(&[("sh", "sh")]);

    let run = run_against(&conf, &["deps", "check"]);
    let stdout = String::from_utf8_lossy(&run.stdout);

    // Positive control: the run must produce output, or the exit assertion
    // could hold for a binary that did nothing at all.
    assert!(!stdout.is_empty(), "a check must report something");
    assert_eq!(
        run.status.code(),
        Some(0),
        "everything present is exit 0, stdout: {stdout}"
    );
    assert!(
        stdout.contains("present   sh"),
        "the report must name the present dependency, got {stdout:?}"
    );
}

/// `deps check` on a manifest with an absent dependency exits 1.
#[test]
fn check_exits_one_when_something_is_missing() {
    let (_fixture, conf) = fixture_manifest(&[("nonexistent-tool", "definitely-not-a-real-binary")]);

    let run = run_against(&conf, &["deps", "check"]);
    let stdout = String::from_utf8_lossy(&run.stdout);

    // Positive control before the exit assertion, so exit 1 cannot come from
    // a binary that produced nothing and failed for an unrelated reason.
    assert!(!stdout.is_empty(), "a check must report something");
    assert_eq!(
        run.status.code(),
        Some(1),
        "a missing dependency is exit 1, stdout: {stdout}"
    );
}

/// `--dry-run` spawns nothing.
///
/// Asserted by pointing the run at a dependency whose install would fail
/// loudly if attempted, and requiring exit to reflect readiness rather than
/// an install failure.
#[test]
fn dry_run_spawns_nothing() {
    let (_fixture, conf) = fixture_manifest(&[("nonexistent-tool", "definitely-not-a-real-binary")]);

    let run = run_against(&conf, &["deps", "install", "--dry-run", "--yes"]);
    let stdout = String::from_utf8_lossy(&run.stdout);

    assert!(!stdout.is_empty(), "a dry run must say what it would do");
    assert_ne!(
        run.status.code(),
        Some(3),
        "exit 3 means an attempt failed, and a dry run attempts nothing: {stdout}"
    );
}

/// A dry run names the command it would run, rather than only a count.
///
/// The disclosure is the point of `--dry-run`: a preview that reported only
/// "1 entry" would pass `dry_run_spawns_nothing` while telling the reader
/// nothing about what is about to happen on their machine.
#[test]
fn dry_run_discloses_the_command_it_would_run() {
    let (_fixture, conf) = fixture_manifest(&[("ripgrep", "rg")]);

    let run = run_against(&conf, &["deps", "install", "--dry-run", "--yes"]);
    let stdout = String::from_utf8_lossy(&run.stdout);

    assert!(!stdout.is_empty(), "a dry run must say what it would do");
    assert!(
        stdout.contains("would install"),
        "a dry run must announce itself as a preview, got {stdout:?}"
    );
}

/// An `--only` value the manifest does not hold is a caller error, exit 2.
///
/// `retired-check-deps:101-107` treated a typo the same way. A silently narrowed
/// selection would let an install report success having installed none of
/// what the caller named.
#[test]
fn an_unknown_only_value_is_a_caller_error() {
    let (_fixture, conf) = fixture_manifest(&[("sh", "sh")]);

    let run = run_against(&conf, &["deps", "check", "--only", "not-in-this-manifest"]);
    let stderr = String::from_utf8_lossy(&run.stderr);

    // Positive control: exit 2 with a silent stderr would be indistinguishable
    // from a crash, so the message must exist before its code is trusted.
    assert!(!stderr.is_empty(), "a caller error must explain itself");
    assert_eq!(
        run.status.code(),
        Some(2),
        "an unknown --only value is exit 2, stderr: {stderr}"
    );
    assert!(
        stderr.contains("not-in-this-manifest"),
        "the error must name the offending selector, got {stderr:?}"
    );
}

/// A manifest file that does not parse is a caller error, exit 2.
#[test]
fn an_unparsable_manifest_is_a_caller_error() {
    // Genuinely broken TOML. The pipe-format version of this test used "this
    // line has no pipes at all", which TOML would reject too but for the
    // wrong reason -- it reads as a bare key with no value. An unclosed
    // table header cannot be mistaken for anything else.
    let (_fixture, conf) = fixture_manifest_text("[unclosed\ncommand = \"sh\"\n");

    let run = run_against(&conf, &["deps", "check"]);
    let stderr = String::from_utf8_lossy(&run.stderr);

    assert!(!stderr.is_empty(), "a caller error must explain itself");
    assert_eq!(
        run.status.code(),
        Some(2),
        "an unparsable manifest is exit 2, stderr: {stderr}"
    );
}

/// `--only` narrows what a dry run plans, not what the report covers.
///
/// `deps_core::reconcile` walks the whole manifest by design, so every entry
/// keeps a row and the readiness verdict still covers the machine rather than
/// the selection. What `--only` decides is which steps the plan emits, which
/// is what a dry run discloses.
#[test]
fn only_narrows_which_steps_a_dry_run_plans() {
    let (_fixture, conf) = fixture_manifest(&[
        ("ripgrep", "definitely-not-ripgrep"),
        ("fd", "definitely-not-fd"),
    ]);

    // Positive control for the absence asserted below: unnarrowed, the dry
    // run does plan a step for `fd`, so its absence afterwards is the
    // selection taking effect and not a preview that plans nothing at all.
    let full = run_against(&conf, &["deps", "install", "--dry-run", "--yes"]);
    let full_stdout = String::from_utf8_lossy(&full.stdout);
    assert!(
        full_stdout.contains("would    "),
        "an unnarrowed dry run must plan steps, got {full_stdout:?}"
    );
    let full_steps = full_stdout.matches("would    ").count();
    assert_eq!(full_steps, 2, "both entries get a step, got {full_stdout:?}");

    let narrowed = run_against(&conf, &["deps", "install", "--dry-run", "--yes", "--only", "ripgrep"]);
    let narrowed_stdout = String::from_utf8_lossy(&narrowed.stdout);
    let narrowed_steps = narrowed_stdout.matches("would    ").count();

    assert_eq!(
        narrowed_steps, 1,
        "--only leaves one step planned, got {narrowed_stdout:?}"
    );
}

/// `--only` scopes the verdict to what the caller named.
///
/// CI narrows to the dependencies a test run needs and deliberately excludes
/// the rest, so a verdict covering the whole manifest made every such run
/// exit 4 on entries nobody asked about. Regression for that: the first real
/// CI run after the port failed exactly this way.
#[test]
fn only_scopes_the_verdict_to_the_named_dependencies() {
    let fixture = tempfile::tempdir().expect("tempdir");
    // Must match SHIPPED_CONF_DIR in config-cli's deps module. Built from
    // one literal, not joined segment by segment: the segmented form is what
    // a repo-wide path substitution cannot see, and it survived the move of
    // this tree out of .scripts/ by silently producing an empty manifest --
    // "0 entries" rather than a missing-file error, because a conf file that
    // is absent is tolerated by design.
    let conf_dir = fixture.path().join("deps");
    std::fs::create_dir_all(&conf_dir).expect("conf dir");
    std::fs::write(
        conf_dir.join("deps.toml"),
        r#"
[sh]
command = "sh"
docs = "https://example.invalid/sh"

[absent-tool]
command = "definitely-not-a-real-binary"
docs = "https://example.invalid/x"
"#,
    )
    .expect("write");

    let run = |only: &[&str]| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_config-cli"));
        command
            .args(["deps", "check"])
            .env("DOTFILES_ROOT", fixture.path())
            .env("DEPS_LOCAL_CONF", "/nonexistent/deps-platform.conf");
        if !only.is_empty() {
            command.args(["--only", &only.join(",")]);
        }
        command.output().expect("the binary runs")
    };

    // Positive control: unnarrowed, the absent tool must still make the run
    // report not-ready, or the assertion below would hold for a binary that
    // never reports anything missing.
    let whole = run(&[]);
    let whole_stdout = String::from_utf8_lossy(&whole.stdout).into_owned();
    // Checked before the status, because it names the cause rather than the
    // symptom. A conf file the engine cannot find is tolerated by design, so
    // a fixture written to the wrong directory yields an EMPTY manifest and
    // this test then fails on an exit code with no hint why. That is exactly
    // what happened when the tree moved out of .scripts/ and this fixture's
    // path did not: "0 entries", and a stared-at status mismatch.
    assert!(
        !whole_stdout.contains("0 entries"),
        "the fixture manifest was not read -- is conf_dir still SHIPPED_CONF_DIR? got: {whole_stdout}"
    );
    assert_eq!(
        whole.status.code(),
        Some(1),
        "an absent dependency must fail an unnarrowed check: {whole_stdout}"
    );

    let narrowed = run(&["sh"]);
    assert_eq!(
        narrowed.status.code(),
        Some(0),
        "--only sh must not report on absent-tool, which nobody asked about: {}",
        String::from_utf8_lossy(&narrowed.stdout)
    );
}
