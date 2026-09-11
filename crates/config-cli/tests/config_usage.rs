//! The per-command help surface: `config <sub> --help`, `--describe`, and the
//! flag spellings each subcommand accepts.
//!
//! `config help` answers "which commands exist". It does not answer "what does
//! `config test -q` do", and that second question is the one a reader has when
//! they are already at the right command. Each `config-<sub>` answers it for
//! itself, from a `# usage:` block at the top of the script, for the same
//! reason the one-line listing is generated: a hand-maintained second copy of
//! the flags is the copy that goes stale.
//!
//! The block runs from the first `# usage:` line to the first line that is not
//! a comment, or to a `# ---` terminator. Everything in it prints verbatim, so
//! the script's own header is the help text and there is nothing to keep in
//! step.
//!
//! Converted whole from `tests/config-usage.test.sh`, which ran **191**
//! assertions, measured by running it. The plan for this tranche says 57,
//! which is a count of `assert_` call sites rather than of executed
//! assertions: almost every one of them sits inside a loop over the twelve
//! subcommands. Sixth count error of the day, and the largest.
//!
//! Two things this file must keep exactly as they were:
//!
//! - The `config help` listing is pinned BYTE-FOR-BYTE against
//!   `tests/fixtures/config-help-before-describe.txt`. The column is built
//!   with `printf '  %-14s %s\n'`, so a description that gained a trailing
//!   newline or a second line would shift every row after it and no other
//!   assertion here would notice. A `contains` check would not see that.
//! - The odd-shaped `config-*` scripts (a binary, a non-executable sibling)
//!   are driven from a FIXTURE directory through `CONFIG_SUBCOMMAND_DIR`,
//!   which `config-help` exports from its own `$0`. That is how they are
//!   tested without leaving a stray `config-*` in the real `.scripts/config`
//!   that other assertions count.

use dotfiles_test_support::repo::root as repo_root;
use dotfiles_test_support::skip;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn config_dir() -> PathBuf {
    repo_root().join(".scripts/config")
}

fn dispatcher() -> PathBuf {
    config_dir().join("config")
}

/// Every subcommand, discovered from the directory rather than listed by hand.
///
/// A hand-written list is a second copy of the same facts, and a subcommand
/// missing from it is not reported as a failure: it is simply never checked,
/// which is the quietest way for this suite to stop covering something.
fn all_subcommands() -> Vec<String> {
    let mut found: Vec<String> = fs::read_dir(config_dir())
        .expect("the config directory is readable")
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter_map(|name| name.strip_prefix("config-").map(str::to_string))
        .collect();
    found.sort();
    assert!(
        !found.is_empty(),
        "no config-<sub> scripts were discovered, so every loop below would \
         run zero times and pass vacuously"
    );
    found
}

fn write_executable(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("the parent directory is creatable");
    }
    fs::write(path, body).expect("the file is writable");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("the file is executable");
}

fn git(directory: &Path, arguments: &[&str]) {
    let mut command = Command::new("git");
    command.arg("-C").arg(directory);
    for variable in ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_PREFIX"] {
        command.env_remove(variable);
    }
    let output = command.args(arguments).output().expect("git runs");
    assert!(
        output.status.success(),
        "git {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A fixture `$HOME` with a bare `.cfg`, plus a shim directory whose
/// `config-cli` records the argv it was handed.
///
/// `help` is the exception the shim forwards rather than echoes. Every other
/// verb is asserted on for argv forwarding, which an echo answers, but the
/// listing's CONTENT is what this suite checks about `config help`, and an
/// echo would make those assertions pass against `deps:help` while proving
/// nothing.
struct Fixture {
    directory: TempDir,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let home = directory.path().join("home");
        fs::create_dir_all(home.join("deps")).expect("creatable");
        fs::create_dir_all(home.join("tests")).expect("creatable");

        let seed = directory.path().join("seed");
        fs::create_dir_all(&seed).expect("creatable");
        git(&seed, &["init", "-q", "-b", "main"]);
        git(
            &seed,
            &[
                "-c",
                "user.email=t@t",
                "-c",
                "user.name=t",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "init",
            ],
        );
        let clone = Command::new("git")
            .args(["clone", "-q", "--bare"])
            .arg(&seed)
            .arg(home.join(".cfg"))
            .output()
            .expect("git clone runs");
        assert!(
            clone.status.success(),
            "the bare clone failed: {}",
            String::from_utf8_lossy(&clone.stderr)
        );

        let real_config_cli = which("config-cli")
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        write_executable(
            &directory.path().join("shims/config-cli"),
            &format!(
                "#!/bin/sh\n\
                 if [ \"${{1:-}}\" = help ] && [ -x '{real_config_cli}' ]; then\n\
                 \x20   exec '{real_config_cli}' \"$@\"\n\
                 fi\n\
                 printf 'deps:%s\\n' \"$@\"\n"
            ),
        );

        Self { directory }
    }

    fn home(&self) -> PathBuf {
        self.directory.path().join("home")
    }

    fn shim_dir(&self) -> PathBuf {
        self.directory.path().join("shims")
    }

    /// Runs the dispatcher under the fixture home with the shim first on
    /// `PATH`.
    fn run(&self, arguments: &[&str]) -> Run {
        let output = Command::new(dispatcher())
            .args(arguments)
            .env("HOME", self.home())
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.shim_dir().display(),
                    std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_string())
                ),
            )
            .output()
            .expect("the dispatcher runs");
        Run {
            status: output.status.code().unwrap_or(-1),
            stdout: trimmed(&output.stdout),
            stderr: trimmed(&output.stderr),
        }
    }

    /// The same, with stdout and stderr merged the way `2>&1` merges them.
    fn run_combined(&self, arguments: &[&str]) -> Run {
        let run = self.run(arguments);
        let combined = if run.stderr.is_empty() {
            run.stdout.clone()
        } else if run.stdout.is_empty() {
            run.stderr.clone()
        } else {
            format!("{}\n{}", run.stdout, run.stderr)
        };
        Run {
            status: run.status,
            stdout: combined,
            stderr: run.stderr,
        }
    }
}

struct Run {
    status: i32,
    stdout: String,
    stderr: String,
}

fn trimmed(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches('\n')
        .to_string()
}

/// The first `PATH` entry holding an executable of this name.
fn which(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")?
        .to_str()?
        .split(':')
        .map(|directory| Path::new(directory).join(name))
        .find(|candidate| {
            fs::metadata(candidate).is_ok_and(|data| data.permissions().mode() & 0o111 != 0)
        })
}

/// The first `# <field>: ` value in a script, or an empty string.
fn header_field(sub: &str, field: &str) -> String {
    let prefix = format!("# {field}: ");
    fs::read_to_string(config_dir().join(format!("config-{sub}")))
        .unwrap_or_default()
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or_default()
        .to_string()
}

/// Every subcommand answers `--help`, names itself, and `-h` is the same text.
#[test]
fn every_subcommand_answers_help_under_both_spellings() {
    let fixture = Fixture::new();
    for sub in all_subcommands() {
        let long = fixture.run_combined(&[&sub, "--help"]);
        assert_eq!(
            long.status, 0,
            "config {sub} --help exited {}: {}",
            long.status, long.stdout
        );
        assert!(
            long.stdout.contains(&format!("config {sub}")),
            "config {sub} --help does not name the command: {}",
            long.stdout
        );

        // -h is the spelling people try when --help is too long, and it must
        // reach the same text rather than a different one.
        let short = fixture.run_combined(&[&sub, "-h"]);
        assert_eq!(
            short.stdout, long.stdout,
            "config {sub} -h printed different text from --help"
        );
    }
}

/// `--help` must not run the command.
///
/// A subcommand that ran its work first and printed help after would rebuild,
/// or push, or start a watch loop. `install-hooks` is the sharpest case:
/// before the shell suite existed, `config install-hooks --help` linked the
/// hooks and rewrote `~/.local/bin/config`, because the script ignored its
/// arguments entirely. Asking a command what it does must not be the same as
/// doing it.
#[test]
fn help_does_not_run_the_command() {
    let fixture = Fixture::new();

    assert!(
        !fixture.run_combined(&["test", "--help"]).stdout.contains("all:"),
        "config test --help ran the suite"
    );
    assert!(
        !fixture
            .run_combined(&["install", "--help"])
            .stdout
            .contains("deps:"),
        "config install --help exec'd the deps engine"
    );
    assert!(
        !fixture
            .run_combined(&["deps", "--help"])
            .stdout
            .contains("deps:"),
        "config deps --help exec'd the deps engine"
    );

    for verb in ["--help", "--describe"] {
        let fixture = Fixture::new();
        write_executable(&fixture.home().join("tests/pre-commit"), "#!/bin/sh\nexit 0\n");
        write_executable(&fixture.home().join("tests/pre-push"), "#!/bin/sh\nexit 0\n");
        let _ = fs::remove_file(fixture.home().join(".cfg/hooks/pre-commit"));
        let _ = fs::remove_file(fixture.home().join(".local/bin/config"));

        fixture.run_combined(&["install-hooks", verb]);
        assert!(
            !fixture.home().join(".cfg/hooks/pre-commit").exists(),
            "config install-hooks {verb} linked pre-commit"
        );
        assert!(
            !fixture.home().join(".local/bin/config").exists(),
            "config install-hooks {verb} rewrote the dispatcher"
        );
    }
}

/// The `# usage:` block in the script is what prints, so the help text and the
/// script header cannot disagree.
#[test]
fn the_usage_block_is_the_source_of_the_help_text() {
    let fixture = Fixture::new();
    for sub in all_subcommands() {
        let first_line = header_field(&sub, "usage");
        // An empty needle would make the containment check pass for any
        // output, which is exactly the state this suite starts in. Assert the
        // needle first.
        assert!(
            !first_line.is_empty(),
            "config-{sub} carries no `# usage:` line to print"
        );
        let output = fixture.run_combined(&[&sub, "--help"]).stdout;
        assert!(
            output.contains(&first_line),
            "config {sub} --help does not print its own usage line \
             {first_line:?}: {output}"
        );
    }
}

/// The shared helper sits beside the subcommands, outside their namespace, and
/// every one of them sources it.
///
/// Sourcing `usage.sh` gives every subcommand a runtime dependency on a file
/// beside it. The dispatcher's whole design is "exec whatever sits beside me",
/// so a copy that takes the `config-<sub>` scripts and leaves the helper
/// behind is a shape that can really happen.
#[test]
fn the_usage_helper_is_part_of_the_set() {
    assert!(
        config_dir().join("usage.sh").is_file(),
        "the usage helper does not sit beside the subcommands"
    );
    // It must not be named config-*, or the dispatcher would treat it as a
    // subcommand called `usage.sh` and `config help` would list it.
    assert!(
        !config_dir().join("config-usage.sh").exists(),
        "the helper is inside the config-<sub> namespace, so `config help` \
         would list it as a subcommand"
    );

    for sub in all_subcommands() {
        let text = fs::read_to_string(config_dir().join(format!("config-{sub}")))
            .unwrap_or_else(|_| panic!("config-{sub} is readable"));
        assert!(
            text.contains("usage.sh"),
            "config-{sub} does not source the shared helper"
        );
    }
}

/// A `# ---` line ends the block, and a bare `#` does not.
///
/// `config-install-hooks` follows its help with a trust-boundary rationale
/// that answers a maintainer's question. Printing it to someone who asked what
/// the command does buries the actual answer. The terminator must not be a
/// bare `#`, because every block uses one to separate its own paragraphs, so a
/// bare `#` would truncate each block at its first blank line.
#[test]
fn the_block_terminator_cuts_in_the_right_place() {
    let fixture = Fixture::new();

    let output = fixture.run_combined(&["install-hooks", "--help"]).stdout;
    assert!(
        output.contains("Takes no options"),
        "install-hooks help lost the text above the terminator: {output}"
    );
    assert!(
        !output.contains("trust boundary"),
        "install-hooks help printed past the terminator: {output}"
    );
    assert!(
        !output.lines().any(|line| line.trim() == "---"),
        "the terminator line itself printed: {output}"
    );

    // config test has four paragraphs; the last one has to survive.
    let output = fixture.run_combined(&["test", "--help"]).stdout;
    assert!(
        output.contains("Exits 2 on a usage error"),
        "a multi-paragraph block was cut at its first blank line: {output}"
    );
}

/// Every flag the script parses appears in its help text.
///
/// A flag that is accepted but undocumented is the gap this assertion exists
/// to close. Only `config test` parses flags of its own; the thin wrappers
/// forward to a program that prints its own help, and `reload` and `stamp`
/// take none.
#[test]
fn every_parsed_flag_is_described() {
    let fixture = Fixture::new();
    // `test` is the only subcommand that parses flags of its own. The thin
    // wrappers forward to a program that prints its own help, and `reload` and
    // `stamp` take none.
    {
        let sub = "test";
        let source = fs::read_to_string(config_dir().join(format!("config-{sub}")))
            .unwrap_or_else(|_| panic!("config-{sub} is readable"));
        let output = fixture.run_combined(&[sub, "--help"]).stdout;

        let mut parsed: Vec<String> = source
            .lines()
            .filter(|line| line.trim_end().ends_with(')'))
            .flat_map(|line| {
                line.split(['|', ')', '('])
                    .map(str::trim)
                    .filter(|word| {
                        word.starts_with('-')
                            && word.len() > 1
                            && word
                                .trim_start_matches('-')
                                .chars()
                                .all(|byte| byte.is_ascii_lowercase() || byte == '-')
                    })
                    .map(str::to_string)
                    .collect::<Vec<String>>()
            })
            .collect();
        parsed.sort();
        parsed.dedup();

        let undocumented: Vec<&String> = parsed
            .iter()
            .filter(|flag| !output.contains(flag.as_str()))
            .collect();
        assert!(
            undocumented.is_empty(),
            "config {sub} parses flags its help does not describe: \
             {undocumented:?}"
        );
    }
}

/// `config test` forwards every flag spelling to `config-cli` unparsed.
///
/// It is a shim now: it execs `config-cli test "$@"`, and `config-cli` owns
/// both spellings of every flag. The fixture's stub prints `deps:<arg>` per
/// argument, so these assertions check what REACHES the binary rather than
/// what a shell case statement used to do with it.
#[test]
fn config_test_forwards_every_flag_spelling() {
    let fixture = Fixture::new();
    for (arguments, expected) in [
        (vec!["test", "-q"], "deps:test\ndeps:-q"),
        (vec!["test", "--quiet"], "deps:test\ndeps:--quiet"),
        (vec!["test", "--docker"], "deps:test\ndeps:--docker"),
        (vec!["test", "-d"], "deps:test\ndeps:-d"),
        (
            vec!["test", "-w", "--docker"],
            "deps:test\ndeps:-w\ndeps:--docker",
        ),
    ] {
        let run = fixture.run(&arguments);
        assert_eq!(
            run.stdout, expected,
            "config {arguments:?} did not reach config-cli unchanged"
        );
    }
}

/// `--describe` prints exactly one line, on stdout, matching the `# help:`
/// comment, and nothing else.
///
/// `config-help` builds its listing by asking each sibling. It used to run
/// `sed -n 's/^# help: //p'` over each subcommand's SOURCE. Pointed at a
/// compiled binary, that sed writes "RE error: illegal byte sequence" to
/// stderr and the pipeline still exits 0, because `head -1` is the last stage
/// and supplies the status. So the description silently became empty and a
/// linter-style error leaked into the listing.
#[test]
fn describe_prints_exactly_the_help_comment() {
    let fixture = Fixture::new();
    for sub in all_subcommands() {
        let run = fixture.run(&[&sub, "--describe"]);
        assert_eq!(run.status, 0, "config {sub} --describe exited {}", run.status);

        // An empty description would make the shape assertions below vacuous,
        // so assert the string exists before asserting its shape.
        assert!(
            !run.stdout.is_empty(),
            "config {sub} --describe printed nothing"
        );
        assert_eq!(
            run.stdout.lines().count(),
            1,
            "config {sub} --describe printed more than one line: {:?}",
            run.stdout
        );
        // config-help formats with `printf '  %-14s %s\n'`, so leading
        // whitespace would break the column.
        assert_eq!(
            run.stdout,
            run.stdout.trim_start(),
            "config {sub} --describe has leading whitespace"
        );

        // stdout carries the description; stderr carries nothing. The leaked
        // sed error on the shared terminal is the failure this contract exists
        // to remove.
        assert_eq!(
            run.stderr, "",
            "config {sub} --describe wrote to stderr: {:?}",
            run.stderr
        );

        // The `# help:` comment stays the single home of the string. A
        // subcommand that grew a second, hand-written copy would be free to
        // disagree with the comment the README points contributors at.
        let comment = header_field(&sub, "help");
        assert!(
            !comment.is_empty(),
            "config-{sub} carries no `# help:` line to describe from"
        );
        assert_eq!(
            run.stdout, comment,
            "config {sub} --describe does not match its own `# help:` line"
        );
    }
}

/// `print_describe` reads the file it is given, not `$0`.
///
/// Without the argument the only way to test it against a named file is to
/// copy a script under a new name, and a stray `config-*` left in the real
/// directory changes what `config help` lists for every other suite.
#[test]
fn print_describe_reads_the_file_it_is_given() {
    let fixtures = tempfile::tempdir().expect("a temporary directory");
    let subject = fixtures.path().join("describe-subject");
    fs::write(
        &subject,
        "#!/bin/sh\n# help: A description read from an argument\n",
    )
    .expect("the subject is writable");

    let output = Command::new("sh")
        .arg("-c")
        .arg(". \"$1/usage.sh\"; print_describe \"$2\"")
        .arg("_")
        .arg(config_dir())
        .arg(&subject)
        .output()
        .expect("sh runs");
    assert_eq!(
        trimmed(&output.stdout),
        "A description read from an argument",
        "print_describe did not read the file it was given"
    );
}

/// The listing asks each subcommand rather than reading it, and every
/// description reaches the column.
#[test]
fn the_listing_consumes_describe() {
    let fixture = Fixture::new();
    let listing = fixture.run(&["help"]).stdout;
    assert!(
        !listing.is_empty(),
        "config help printed no listing, so the checks below would be vacuous"
    );

    let mut undescribed = Vec::new();
    for sub in all_subcommands() {
        let described = fixture.run(&[&sub, "--describe"]).stdout;
        if described.is_empty() || !listing.contains(&described) {
            undescribed.push(sub);
        }
    }
    assert!(
        undescribed.is_empty(),
        "config help omits these subcommands' descriptions: {undescribed:?}"
    );
    assert!(
        !listing.contains("(undocumented)"),
        "the listing has undocumented entries of its own: {listing}"
    );
}

/// The odd-shaped siblings, driven from a FIXTURE directory.
///
/// `config-help` resolves its siblings from its own `$0` and exports that path
/// as `CONFIG_SUBCOMMAND_DIR`, so a copy of the shim in a fixture directory
/// lists THAT directory's siblings. This is how a binary subcommand and a
/// non-executable one are exercised without leaving a stray `config-*` in the
/// real `.scripts/config` that every other assertion counts.
#[test]
fn the_listing_handles_odd_shaped_siblings() {
    let fixtures = tempfile::tempdir().expect("a temporary directory");
    let listing_dir = fixtures.path().join("listing-dir");
    fs::create_dir_all(&listing_dir).expect("the listing directory is creatable");
    for file in ["config-help", "usage.sh"] {
        fs::copy(config_dir().join(file), listing_dir.join(file))
            .unwrap_or_else(|_| panic!("{file} is copyable"));
        let _ = fs::set_permissions(listing_dir.join(file), fs::Permissions::from_mode(0o755));
    }

    let run_listing = || -> String {
        let output = Command::new(listing_dir.join("config-help"))
            .output()
            .expect("the listing shim runs");
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    };

    // A BINARY subcommand, standing in for the first ported one. A copy of a
    // real binary rather than a crafted file, so the bytes are whatever a
    // compiler actually emits. /bin/echo answers --describe by printing
    // "--describe", which is one line on stdout with exit 0: that satisfies
    // the contract, which is what makes it usable here. The assertion is that
    // the listing READS STDOUT and emits no sed error, not what the text says.
    if Path::new("/bin/echo").exists() {
        let binary = listing_dir.join("config-fixturebin");
        fs::copy("/bin/echo", &binary).expect("the binary is copyable");
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))
            .expect("the binary is executable");

        let output = run_listing();
        assert!(
            output.contains("fixturebin"),
            "config help does not list a binary subcommand: {output}"
        );
        assert!(
            !output.contains("illegal byte sequence"),
            "a binary subcommand produced a sed error in the listing: {output}"
        );
        assert!(
            !output.contains("sed:"),
            "a binary subcommand produced a sed error at all: {output}"
        );
        fs::remove_file(&binary).expect("the binary is removable");
    } else {
        skip("/bin/echo is missing, so there is no binary to stand in for a ported subcommand");
    }

    // A config-* sibling that is NOT executable cannot be reached through the
    // dispatcher, so the listing cannot execute it either. It must degrade to
    // (undocumented) rather than emitting a not-found error into the column.
    let inert = listing_dir.join("config-fixtureinert");
    fs::write(&inert, "#!/bin/sh\n# help: never runs\n").expect("the sibling is writable");
    fs::set_permissions(&inert, fs::Permissions::from_mode(0o644))
        .expect("the sibling is non-executable");

    let output = run_listing();
    assert!(
        output.contains("fixtureinert"),
        "config help does not list a non-executable sibling: {output}"
    );
    assert!(
        output.contains("(undocumented)"),
        "a non-executable sibling is not marked undocumented: {output}"
    );
    let lowered = output.to_lowercase();
    assert!(
        !lowered.contains("permission denied") && !lowered.contains("not found"),
        "a non-executable sibling produced an exec error: {output}"
    );
    // The `# help:` line IS present and would be found by a source grep, so
    // the assertion above only means anything while the description comes from
    // asking rather than from reading.
    assert!(
        !output.contains("never runs"),
        "the non-executable sibling was described by READING it, so the \
         listing still greps source: {output}"
    );
}

/// `print_usage` does not degrade silently on a file it cannot read.
///
/// It reads its subject, so it cannot delegate to a subprocess the way the
/// listing does: the whole point is that it prints the block out of the script
/// the reader asked about. What it can stop doing is printing an empty block
/// plus a sed error. A shell subcommand rewritten as a binary without porting
/// its help surface is the mistake this catches.
#[test]
fn print_usage_does_not_degrade_silently() {
    let run_print_usage = |subject: &Path| -> (i32, String) {
        let output = Command::new("sh")
            .arg("-c")
            .arg(". \"$1/usage.sh\"; print_usage \"$2\"")
            .arg("_")
            .arg(config_dir())
            .arg(subject)
            .output()
            .expect("sh runs");
        (
            output.status.code().unwrap_or(-1),
            format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        )
    };

    // Positive control: pointed at a text script, print_usage prints that
    // script's block. Without this, the failure assertions below pass when the
    // call is simply broken.
    let (_, text_output) = run_print_usage(&config_dir().join("config-help"));
    assert!(
        text_output.contains("usage: config help"),
        "print_usage did not print the block of the file it was given: \
         {text_output}"
    );

    if !Path::new("/bin/echo").exists() {
        skip("/bin/echo is missing, so there is no binary to point print_usage at");
        return;
    }
    let fixtures = tempfile::tempdir().expect("a temporary directory");
    let binary = fixtures.path().join("binary-usage-subject");
    fs::copy("/bin/echo", &binary).expect("the binary is copyable");
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))
        .expect("the binary is executable");

    let (status, output) = run_print_usage(&binary);
    assert!(
        !output.contains("illegal byte sequence"),
        "print_usage on a binary emitted a sed error: {output}"
    );
    assert!(
        output.contains("not a text file"),
        "print_usage on a binary did not say what went wrong: {output}"
    );
    assert_eq!(
        status, 1,
        "print_usage on a binary returned {status} rather than 1"
    );
}

/// The `config help` listing renders BYTE-FOR-BYTE as recorded.
///
/// The point of the `--describe` migration is that nothing about `config help`
/// changed. A recorded expectation is the only assertion that proves it: the
/// column is built with `printf '  %-14s %s\n'`, so a description that gained
/// a trailing newline or a second line would shift every row after it and no
/// other assertion in this file would notice. The expectation was captured
/// before the first `--describe` commit, so a match proves the migration is
/// invisible in the output rather than merely self-consistent.
///
/// Deliberately an exact comparison, never a `contains`. Regenerate it only
/// for a deliberate edit to the listing's own format, to a `# help:` line, or
/// when a subcommand is added or removed.
///
/// Read from the REAL `.scripts/config` rather than through a fixture home:
/// the expectation records the real set of siblings.
#[test]
fn config_help_renders_exactly_the_recorded_listing() {
    let expectation = repo_root().join("tests/fixtures/config-help-before-describe.txt");
    assert!(
        expectation.is_file(),
        "the recorded listing is missing at {}",
        expectation.display()
    );

    let expected = fs::read_to_string(&expectation).expect("the recorded listing is readable");
    // An empty expectation would make the comparison pass against an equally
    // empty listing, so assert the recorded text exists first.
    assert!(
        !expected.trim().is_empty(),
        "the recorded listing is empty, so the comparison below proves nothing"
    );

    let actual = Command::new(config_dir().join("config-help"))
        .output()
        .expect("config-help runs");
    let actual = String::from_utf8_lossy(&actual.stdout).to_string();

    assert_eq!(
        actual.trim_end_matches('\n'),
        expected.trim_end_matches('\n'),
        "config help does not render the recorded listing. Regenerate with: \
         .scripts/config/config-help > tests/fixtures/config-help-before-describe.txt"
    );
}

/// `config deps` resolves through the dispatcher rather than falling through
/// to git.
///
/// Written before `config-deps` existed: at that point `config deps` fell
/// through to `git deps`, which failed with git's own "not a git command"
/// message and exit 1. That fallthrough is the hazard a missing shim creates.
#[test]
fn config_deps_resolves_through_the_dispatcher() {
    let fixture = Fixture::new();

    let listing = Command::new(dispatcher())
        .arg("help")
        .env("HOME", fixture.home())
        .output()
        .expect("the dispatcher runs");
    let listing = format!(
        "{}{}",
        String::from_utf8_lossy(&listing.stdout),
        String::from_utf8_lossy(&listing.stderr)
    );
    assert!(
        listing.contains("deps"),
        "config help does not list deps, so the subcommand is invisible: \
         {listing}"
    );

    let bad_flag = Command::new(dispatcher())
        .args(["deps", "--not-a-real-flag"])
        .env("HOME", fixture.home())
        .output()
        .expect("the dispatcher runs");
    let message = format!(
        "{}{}",
        String::from_utf8_lossy(&bad_flag.stdout),
        String::from_utf8_lossy(&bad_flag.stderr)
    );
    assert_eq!(
        bad_flag.status.code(),
        Some(2),
        "config deps did not reject an unrecognized flag with exit 2, the \
         repo-wide convention: {message}"
    );
    assert!(
        !message.contains("is not a git command"),
        "the rejection fell through to git: {message}"
    );
}
