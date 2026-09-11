//! The `config` dispatcher and the `config-<sub>` utilities beside it.
//!
//! Every test runs against a fixture `$HOME`: a bare repo at `.cfg` with a
//! worktree, so nothing here touches the real dotfiles repo. The exception is
//! the doctor block, which is driven against the ambient checkout because
//! `config-doctor` gathers its expected side by reading the real `crates/`
//! tree through git, and skips with a stated reason where there is no
//! repository.
//!
//! Converted whole from `tests/config.test.sh`, which ran **80** assertions,
//! measured by running the suite rather than by counting `assert_` call
//! sites. The plan states 76.
//!
//! # Runtime
//!
//! **This conversion does not make the suite faster, and does not claim to.**
//! The shell suite's cost is [`doctor_reports_a_stale_binary_and_names_the_fix`]
//! and its neighbour, which call `config-build` and compile six crates. The
//! Rust version calls the same script for the same reason: doctor's whole
//! subject is whether an installed binary matches its source, so a test that
//! skips the build is testing nothing. Measured while converting: the shell
//! suite is 154 seconds here, not the plan's 105, and this file is 163 to 181
//! seconds across clean runs. The ported build-bearing tests dominate it the
//! same way, so the conversion moves the cost, it does not remove it.
//!
//! Whether the Rust version should still build is a separate question this
//! conversion does not answer.

use dotfiles_test_support::repo::root as repo_root;
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use tempfile::TempDir;

/// Guards the installed `config-cli` binary against concurrent rebuilds.
///
/// Two hazards, and the second was found by running this port.
///
/// 1. The two build-bearing tests share one tree: one makes
///    `crates/config-cli/src/doctor.rs` stale on purpose while the other
///    asserts every binary is current. Run together, whichever loses the race
///    reports the other's edit.
/// 2. **`config-build` reinstalls `~/.local/bin/config-cli` while other tests
///    are executing it.** Every `config help`, `config test` and
///    `config install` shim execs that binary, so a rebuild mid-run hands
///    them a half-written file and they produce no output at all. Measured:
///    `config help` returned an empty string in a full run and the correct
///    listing when run alone.
///
/// The second hazard has a sharper form on macOS, measured while converting
/// this suite. Overwriting the binary in place invalidates the running
/// image's code signature, and the kernel then SIGKILLs it: the binary exits
/// **137** producing no output, and keeps doing so on every later run until
/// it is re-signed (`codesign -f -s -`) or replaced by a rename rather than
/// an in-place write. `codesign -v` still reports the file as valid, so the
/// symptom reads as "the binary silently prints nothing" rather than as a
/// signing problem. Reported as a finding against `config-build`, which
/// installs over the live path; fixing it is out of this conversion's scope.
///
/// So the build-bearing tests take this as a writer and every test that execs
/// a `config-cli`-backed subcommand takes it as a reader. The shell suite got
/// both for free by being sequential; cargo's harness is not.
static CONFIG_CLI_LOCK: std::sync::RwLock<()> = std::sync::RwLock::new(());

/// Exclusive access, for a test that rebuilds and reinstalls the binary.
fn build_lock() -> std::sync::RwLockWriteGuard<'static, ()> {
    CONFIG_CLI_LOCK
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Shared access, for a test that only runs the installed binary.
///
/// A panic in a build-bearing test must not turn every reader into a second
/// failure with an unrelated message, so a poisoned lock is taken anyway.
fn config_cli_lock() -> std::sync::RwLockReadGuard<'static, ()> {
    CONFIG_CLI_LOCK
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The subcommands the dispatcher is expected to carry.
///
/// The single source of truth for this file, as `EXPECTED_SUBCOMMANDS` was
/// for the shell suite. A `config-<sub>` that lands without appearing here
/// fails [`the_config_sub_set_equals_the_allowlist`].
const EXPECTED_SUBCOMMANDS: &[&str] = &[
    "build",
    "stamp",
    "install-hooks",
    "install-repo-hooks",
    "init",
    "install",
    "prereqs",
    "test",
    "reload",
    "help",
    "doctor",
    "deps",
];

/// Names that deliberately shadow a git verb.
///
/// A `config-<sub>` that shares a git verb makes that verb unreachable by
/// name, so each one is a choice rather than an accident. `config -- <verb>`
/// is the escape hatch that keeps the git command reachable.
///
/// `init` shadows `git init`, and that is the right way round: reaching
/// `git init` through this dispatcher would initialize a repository in the
/// current directory using the bare repo's `--git-dir`, which is never what
/// someone typing `config init` means.
const SHADOWED_ON_PURPOSE: &[&str] = &["help", "init"];

fn config_dir() -> PathBuf {
    repo_root().join(".scripts/config")
}

fn config() -> PathBuf {
    config_dir().join("config")
}

fn write_executable(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("the parent directory is creatable");
    }
    fs::write(path, body).expect("the file is writable");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("the file is executable");
}

fn is_executable(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.permissions().mode() & 0o100 != 0)
        .unwrap_or(false)
}

/// A git invocation with any ambient git environment cleared.
fn git_command() -> Command {
    let mut command = Command::new("git");
    for variable in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_PREFIX",
        "GIT_OBJECT_DIRECTORY",
    ] {
        command.env_remove(variable);
    }
    command
}

fn git_in(directory: &Path, arguments: &[&str]) {
    let output = git_command()
        .arg("-C")
        .arg(directory)
        .args(arguments)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_stdout(arguments: &[&str]) -> String {
    let output = git_command().args(arguments).output().expect("git runs");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// Every git command name, `main` and `others`.
fn git_command_names() -> BTreeSet<String> {
    git_stdout(&["--list-cmds=main,others"])
        .lines()
        .map(str::to_owned)
        .collect()
}

/// A fixture `$HOME` holding a bare repo at `.cfg` with a worktree.
struct Home {
    directory: TempDir,
}

/// One run of a script: its exit status and its combined output.
struct Run {
    status: i32,
    stdout: String,
    text: String,
}

impl Run {
    fn assert_status(&self, expected: i32, description: &str) {
        assert_eq!(
            self.status, expected,
            "{description}\noutput:\n{}",
            self.text
        );
    }

    fn assert_contains(&self, needle: &str, description: &str) {
        assert!(
            self.text.contains(needle),
            "{description}\nexpected to contain {needle:?}, output was:\n{}",
            self.text
        );
    }

    fn assert_lacks(&self, needle: &str, description: &str) {
        assert!(
            !self.text.contains(needle),
            "{description}\nexpected NOT to contain {needle:?}, output was:\n{}",
            self.text
        );
    }

    fn stdout_trimmed(&self) -> &str {
        self.stdout.trim_end()
    }
}

impl Home {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let home = Self { directory };

        let seed = home.root().join("seed");
        fs::create_dir_all(&seed).expect("creatable");
        git_in(&seed, &["init", "-q", "-b", "main", "."]);
        git_in(
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

        fs::create_dir_all(home.path()).expect("creatable");
        let output = git_command()
            .arg("clone")
            .arg("-q")
            .arg("--bare")
            .arg(&seed)
            .arg(home.git_dir())
            .output()
            .expect("git runs");
        assert!(
            output.status.success(),
            "the bare clone succeeds: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        home.cfg(&["config", "status.showUntrackedFiles", "no"]);
        home
    }

    fn root(&self) -> &Path {
        self.directory.path()
    }

    fn path(&self) -> PathBuf {
        self.root().join("home")
    }

    fn git_dir(&self) -> PathBuf {
        self.path().join(".cfg")
    }

    /// A git invocation against the fixture's bare repo and worktree, which
    /// is what `config <git verb>` resolves to.
    fn cfg(&self, arguments: &[&str]) -> Output {
        git_command()
            .arg("--git-dir")
            .arg(self.git_dir())
            .arg("--work-tree")
            .arg(self.path())
            .args(arguments)
            .output()
            .expect("git runs")
    }

    fn cfg_stdout(&self, arguments: &[&str]) -> String {
        String::from_utf8_lossy(&self.cfg(arguments).stdout)
            .trim()
            .to_owned()
    }

    /// Runs the real dispatcher against this fixture home.
    fn run(&self, arguments: &[&str]) -> Run {
        self.run_program(&config(), arguments, |_| {})
    }

    fn run_with(&self, arguments: &[&str], configure: impl FnOnce(&mut Command)) -> Run {
        self.run_program(&config(), arguments, configure)
    }

    fn run_program(
        &self,
        program: &Path,
        arguments: &[&str],
        configure: impl FnOnce(&mut Command),
    ) -> Run {
        let mut command = Command::new(program);
        command
            .args(arguments)
            .env("HOME", self.path())
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_PREFIX")
            .env_remove("GIT_OBJECT_DIRECTORY")
            .stdin(Stdio::null());
        configure(&mut command);
        let output = command.output().expect("the script runs");
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Run {
            status: output.status.code().unwrap_or(-1),
            text: format!("{stdout}{stderr}"),
            stdout,
        }
    }
}

// --- passthrough ------------------------------------------------------------

#[test]
fn ordinary_git_verbs_pass_through_to_the_bare_repo() {
    let home = Home::new();

    let expected = home.cfg_stdout(&["rev-parse", "HEAD"]);
    assert!(!expected.is_empty(), "the fixture has a HEAD to compare");
    assert_eq!(
        home.run(&["rev-parse", "HEAD"]).stdout_trimmed(),
        expected,
        "config rev-parse HEAD passes through to the bare repo"
    );

    assert_eq!(
        home.run(&["status", "--porcelain", "--untracked-files=no"])
            .text,
        "",
        "config status passes through with the fixture worktree"
    );
}

#[test]
fn an_unknown_subcommand_falls_through_to_git_and_fails() {
    let home = Home::new();
    let run = home.run(&["definitely-not-a-command"]);
    run.assert_status(1, "an unknown subcommand falls through to git and fails");
    run.assert_contains(
        "git: 'definitely-not-a-command' is not a git command",
        "the failure is git's own message",
    );
}

#[test]
fn no_arguments_passes_through_to_git_usage() {
    let home = Home::new();
    let run = home.run(&[]);
    run.assert_status(1, "no arguments passes through to git usage");
    run.assert_contains("usage: git", "git usage is what prints");
}

// --- name rules -------------------------------------------------------------

#[test]
fn the_config_sub_set_equals_the_allowlist() {
    let found: BTreeSet<String> = fs::read_dir(config_dir())
        .expect("the config directory is readable")
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()
                .and_then(|name| name.strip_prefix("config-"))
                .map(str::to_owned)
        })
        .collect();
    let expected: BTreeSet<String> = EXPECTED_SUBCOMMANDS
        .iter()
        .map(|&sub| sub.to_owned())
        .collect();
    assert_eq!(found, expected, "the config-<sub> set equals the allowlist");
}

#[test]
fn every_listed_subcommand_is_executable() {
    let not_executable: Vec<&str> = EXPECTED_SUBCOMMANDS
        .iter()
        .copied()
        .filter(|sub| !is_executable(&config_dir().join(format!("config-{sub}"))))
        .collect();
    assert!(
        not_executable.is_empty(),
        "every listed subcommand is executable, these are not: {not_executable:?}"
    );
}

/// No `config-<sub>` may shadow a git command by accident, and every name on
/// the deliberate list must really be one.
///
/// The reverse check matters as much as the forward one: a name on the
/// deliberate list that no longer shadows anything is stale, and would
/// silently excuse a future accident.
#[test]
fn shadowing_a_git_command_is_always_deliberate() {
    let git_commands = git_command_names();

    // Positive control: the list must be non-empty, or every check below
    // holds vacuously against a git that answered nothing.
    assert!(
        git_commands.contains("commit"),
        "git listed its commands, so the shadowing checks have something to compare"
    );

    let accidental: Vec<&str> = EXPECTED_SUBCOMMANDS
        .iter()
        .copied()
        .filter(|sub| !SHADOWED_ON_PURPOSE.contains(sub))
        .filter(|sub| git_commands.contains(*sub))
        .collect();
    assert!(
        accidental.is_empty(),
        "no config-<sub> shadows a git command by accident, these do: {accidental:?}"
    );

    let not_actually_shadowing: Vec<&str> = SHADOWED_ON_PURPOSE
        .iter()
        .copied()
        .filter(|sub| !git_commands.contains(*sub))
        .collect();
    assert!(
        not_actually_shadowing.is_empty(),
        "every deliberately-shadowed name really is a git command, these are not: \
         {not_actually_shadowing:?}"
    );

    let without_script: Vec<&str> = SHADOWED_ON_PURPOSE
        .iter()
        .copied()
        .filter(|sub| !is_executable(&config_dir().join(format!("config-{sub}"))))
        .collect();
    assert!(
        without_script.is_empty(),
        "every deliberately-shadowed name has a config-<sub>, these do not: {without_script:?}"
    );
}

/// The dispatcher stays small enough to read in one screen. A front door that
/// grows logic is a front door with behaviour nobody expects.
#[test]
fn the_dispatcher_stays_under_forty_lines() {
    let source = fs::read_to_string(config()).expect("the dispatcher is readable");
    let line_count = source.lines().count();
    assert!(
        line_count < 40,
        "the dispatcher stays under 40 lines, it is {line_count}"
    );
}

// --- sibling dispatch -------------------------------------------------------

/// A copy of the dispatcher in a scratch directory, so a fake `config-<sub>`
/// can sit beside it without being installed in the real tree.
struct SiblingDir {
    directory: PathBuf,
}

impl SiblingDir {
    fn new(root: &Path) -> Self {
        let directory = root.join("sibling-bin");
        fs::create_dir_all(&directory).expect("creatable");
        fs::copy(config(), directory.join("config")).expect("the dispatcher is copyable");
        fs::set_permissions(directory.join("config"), fs::Permissions::from_mode(0o755))
            .expect("executable");
        write_executable(
            &directory.join("config-probe"),
            "#!/bin/sh\nprintf 'probe:%s\\n' \"$@\"\n",
        );
        Self { directory }
    }

    fn dispatcher(&self) -> PathBuf {
        self.directory.join("config")
    }
}

#[test]
fn a_sibling_config_sub_receives_the_remaining_arguments_intact() {
    let home = Home::new();
    let sibling = SiblingDir::new(home.root());

    let run = home.run_program(
        &sibling.dispatcher(),
        &["probe", "one", "two words"],
        |_| {},
    );
    assert_eq!(
        run.stdout_trimmed(),
        "probe:one\nprobe:two words",
        "a sibling config-<sub> receives the remaining args intact"
    );
}

/// The dispatcher resolves siblings relative to its own real path, so a
/// symlink into `~/.local/bin` still finds them. `readlink -f` is what makes
/// that work, and removing it would break every installed machine.
#[test]
fn the_dispatcher_resolves_siblings_through_its_own_symlink() {
    let home = Home::new();
    let sibling = SiblingDir::new(home.root());

    let link_dir = home.root().join("linked-bin");
    fs::create_dir_all(&link_dir).expect("creatable");
    let link = link_dir.join("config");
    std::os::unix::fs::symlink(sibling.dispatcher(), &link).expect("symlinkable");

    let run = home.run_program(&link, &["probe", "via-symlink"], |_| {});
    assert_eq!(
        run.stdout_trimmed(),
        "probe:via-symlink",
        "the dispatcher resolves siblings through its own symlink"
    );
}

/// A slash-shaped subcommand must not exec a file inside a `config-<sub>`
/// directory. Without the character-class guard, `config x/go` would run
/// `config-x/go`, which is arbitrary path traversal through a verb name.
#[test]
fn a_slash_shaped_subcommand_does_not_escape_into_a_directory() {
    let home = Home::new();
    let sibling = SiblingDir::new(home.root());

    write_executable(
        &sibling.directory.join("config-x/go"),
        "#!/bin/sh\nprintf 'escaped\\n'\n",
    );

    let run = home.run_program(&sibling.dispatcher(), &["x/go"], |_| {});
    run.assert_lacks(
        "escaped",
        "a slash-shaped subcommand does not exec a file inside a config-<sub> directory",
    );
    run.assert_status(
        1,
        "a slash-shaped subcommand falls through to git and fails",
    );
    run.assert_contains("is not a git command", "the failure is git's own message");
}

#[test]
fn a_dash_shaped_subcommand_falls_through_to_git() {
    let home = Home::new();
    let sibling = SiblingDir::new(home.root());
    let run = home.run_program(&sibling.dispatcher(), &["-x"], |_| {});
    run.assert_contains(
        "unknown option",
        "a dash-shaped subcommand falls through to git rather than probing a sibling",
    );
}

// --- install-hooks ----------------------------------------------------------

/// A fixture home with `tests/pre-commit`, `tests/pre-push`, and a scratch
/// copy of the dispatcher plus `config-install-hooks`.
struct HooksFixture {
    home: Home,
    install_dir: PathBuf,
}

impl HooksFixture {
    fn new() -> Self {
        let home = Home::new();
        let home_path = home.path();

        fs::create_dir_all(home_path.join("tests")).expect("creatable");
        fs::create_dir_all(home.git_dir().join("hooks")).expect("creatable");
        for hook in ["pre-commit", "pre-push"] {
            write_executable(&home_path.join("tests").join(hook), "#!/bin/sh\nexit 0\n");
        }

        // `config-install-hooks` resolves its own directory with
        // `readlink -f`, which canonicalizes symlinked ancestors
        // (`/var` -> `/private/var` on macOS). The fixture sits under such a
        // symlink, so every assertion below compares against the canonical
        // form the script will actually report.
        let raw = home.root().join("install-bin");
        fs::create_dir_all(&raw).expect("creatable");
        let install_dir = fs::canonicalize(&raw).expect("the install directory canonicalizes");

        fs::copy(config(), install_dir.join("config")).expect("copyable");
        fs::copy(
            config_dir().join("config-install-hooks"),
            install_dir.join("config-install-hooks"),
        )
        .expect("copyable");
        // `usage.sh` is sourced by every config-<sub> to print its `# usage:`
        // block, so it is part of the set a subcommand needs beside it rather
        // than an optional extra. Copying the scripts without it is what a
        // partial install looks like.
        fs::copy(config_dir().join("usage.sh"), install_dir.join("usage.sh")).expect("copyable");
        for script in ["config", "config-install-hooks"] {
            fs::set_permissions(install_dir.join(script), fs::Permissions::from_mode(0o755))
                .expect("executable");
        }

        Self { home, install_dir }
    }

    fn run(&self) -> Run {
        self.home
            .run_program(&self.install_dir.join("config"), &["install-hooks"], |_| {})
    }
}

fn set_mode(path: &Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("the mode is settable");
}

fn mode_of(path: &Path) -> u32 {
    fs::metadata(path).expect("readable").permissions().mode() & 0o7777
}

#[test]
fn install_hooks_links_both_hooks_and_the_dispatcher_and_is_idempotent() {
    let fixture = HooksFixture::new();
    let home_path = fixture.home.path();

    let run = fixture.run();
    run.assert_status(0, "install-hooks succeeds on a clean fixture");

    for hook in ["pre-commit", "pre-push"] {
        assert_eq!(
            fs::read_link(fixture.home.git_dir().join("hooks").join(hook))
                .expect("the hook is a symlink"),
            home_path.join("tests").join(hook),
            "{hook} is linked"
        );
    }
    assert_eq!(
        fs::read_link(home_path.join(".local/bin/config")).expect("the dispatcher is a symlink"),
        fixture.install_dir.join("config"),
        "the dispatcher is linked into ~/.local/bin"
    );

    let again = fixture.run();
    again.assert_status(0, "a second install-hooks succeeds");
    assert_eq!(
        again.text, run.text,
        "a second install-hooks prints the same report"
    );
}

/// The four directories `config-install-hooks` checks are a trust boundary:
/// anyone who can write to them runs code as this user. Each refusal is
/// asserted separately so a check that stops working is named.
#[test]
fn install_hooks_refuses_a_writable_directory_on_the_trust_boundary() {
    let fixture = HooksFixture::new();
    let home_path = fixture.home.path();

    let local_bin = home_path.join(".local/bin");
    fixture.run().assert_status(0, "the fixture starts clean");

    let original = mode_of(&local_bin);
    set_mode(&local_bin, original | 0o002);
    let run = fixture.run();
    run.assert_status(1, "install-hooks refuses a world-writable ~/.local/bin");
    run.assert_contains(
        &local_bin.to_string_lossy(),
        "the refusal names the directory",
    );
    set_mode(&local_bin, original);

    let original = mode_of(&fixture.install_dir);
    set_mode(&fixture.install_dir, original | 0o020);
    fixture.run().assert_status(
        1,
        "install-hooks refuses a group-writable dispatcher directory",
    );
    set_mode(&fixture.install_dir, original);

    let tests = home_path.join("tests");
    let original = mode_of(&tests);
    set_mode(&tests, original | 0o002);
    let run = fixture.run();
    run.assert_status(1, "install-hooks refuses a world-writable tests directory");
    run.assert_contains(
        &tests.to_string_lossy(),
        "the refusal names the tests directory",
    );
    set_mode(&tests, original);

    fixture
        .run()
        .assert_status(0, "the fixture is clean again after each restore");
}

/// The same refusal has to hold when the directory is reached through a
/// symlink, which is the shape the three checks above cannot catch.
///
/// `find "$dir" -maxdepth 0 -perm -o+w` stats the LINK, and a symlink is
/// always mode `lrwxr-xr-x`, so the permission test reads the link's own bits
/// and never the target's. Measured: for a symlink pointing at a 0777
/// directory, that find prints nothing and the check passes; `find -L` prints
/// the path and refuses. A synced or restored home is exactly where
/// `~/.local/bin` or `~/tests` arrives as a link.
///
/// **Both directions are asserted in one test on purpose.** Without `-L`, GNU
/// find matches the link's own `lrwxrwxrwx` mode and refuses a correct setup,
/// so a fix that only tightened the check would trade a macOS hole for a
/// Linux false refusal. The two halves constrain each other.
#[test]
fn install_hooks_reads_a_symlinked_directory_through_the_link() {
    let fixture = HooksFixture::new();
    let home_path = fixture.home.path();
    let tests = home_path.join("tests");
    let real_tests = home_path.join("tests-real");

    let canonical_before = fs::canonicalize(&tests).expect("the tests directory canonicalizes");

    fs::rename(&tests, &real_tests).expect("renamable");
    std::os::unix::fs::symlink(&real_tests, &tests).expect("symlinkable");

    set_mode(&real_tests, 0o777);
    fixture.run().assert_status(
        1,
        "install-hooks refuses a world-writable tests directory reached through a symlink",
    );

    // The other direction, which is the one that bites on Linux.
    set_mode(&real_tests, 0o755);
    fixture.run().assert_status(
        0,
        "install-hooks accepts a safe tests directory reached through a symlink",
    );

    fs::remove_file(&tests).expect("removable");
    fs::rename(&real_tests, &tests).expect("renamable");
    assert_eq!(
        fs::canonicalize(&tests).expect("canonicalizes"),
        canonical_before,
        "the tests directory is a real directory again"
    );
}

// --- thin wrappers ----------------------------------------------------------

/// A fixture home wired with stubs for `config-cli`, `cargo` and
/// `run-in-docker.sh`, so the wrapper tests assert delegation rather than
/// running a real suite.
struct WrapperFixture {
    home: Home,
    shim_dir: PathBuf,
    /// Holds the `cargo` stub and a link to the freshly-built `config-cli`.
    /// Kept apart from `shim_dir` because the `config test` assertions need
    /// the REAL `config-cli`: adding the directory that also shadows it made
    /// every one of them read `cli:test` and prove nothing about the runner
    /// selection.
    cargo_shim_dir: PathBuf,
}

impl WrapperFixture {
    fn new() -> Self {
        let home = Home::new();
        let home_path = home.path();
        let shim_dir = home.root().join("shims");
        fs::create_dir_all(&shim_dir).expect("creatable");
        let cargo_shim_dir = home.root().join("cargo-shim");
        fs::create_dir_all(&cargo_shim_dir).expect("creatable");
        // The `config-test` shim execs `config-cli` off PATH, which would
        // otherwise resolve to whatever `config build` last installed. That
        // binary is stale the moment this crate's source changes, so the
        // assertions below would describe the installed build rather than
        // the one under test. Link the built binary in ahead of it.
        std::os::unix::fs::symlink(
            env!("CARGO_BIN_EXE_config-cli"),
            cargo_shim_dir.join("config-cli"),
        )
        .expect("the built config-cli is linkable");
        fs::create_dir_all(home_path.join("deps")).expect("creatable");
        fs::create_dir_all(home_path.join("tests")).expect("creatable");
        // `config test` runs cargo from $HOME/crates, so the directory has to
        // exist for the spawn to succeed at all.
        fs::create_dir_all(home_path.join("crates")).expect("creatable");

        write_executable(
            &shim_dir.join("config-cli"),
            "#!/bin/sh\nprintf 'cli:%s\\n' \"$@\"\n",
        );
        // `config test` runs `cargo test --locked` from `$HOME/crates`, so
        // the observable is a stub cargo rather than a stub suite script.
        // Printed as one line so a changed argument list is one diff rather
        // than a reordered set.
        write_executable(
            &cargo_shim_dir.join("cargo"),
            "#!/bin/sh\nprintf 'cargo:%s\\n' \"$*\"\nprintf 'cwd:%s\\n' \"$(basename \"$PWD\")\"\n",
        );
        write_executable(
            &home_path.join("tests/run-in-docker.sh"),
            "#!/bin/sh\n[ \"$#\" -eq 0 ] && printf 'docker:(none)\\n' \
             || printf 'docker:%s\\n' \"$@\"\n",
        );

        Self {
            home,
            shim_dir,
            cargo_shim_dir,
        }
    }

    /// Runs the dispatcher with the stub `config-cli` ahead of the real one.
    ///
    /// Only for the shims whose whole subject is delegation: `install` and
    /// `reload` exec `config-cli`, and the stub is what makes the argv
    /// observable.
    fn run_against_stub_cli(&self, arguments: &[&str]) -> Run {
        let path = std::env::var("PATH").unwrap_or_default();
        let shim_dir = self.shim_dir.clone();
        self.home.run_with(arguments, move |command| {
            command.env("PATH", format!("{}:{path}", shim_dir.display()));
        })
    }

    /// Runs the dispatcher against the REAL `config-cli`.
    ///
    /// `config test` is not a pure delegation: `config-cli test` chooses
    /// between cargo and `$HOME/tests/run-in-docker.sh` itself, so the
    /// observable is the stub runner, not a stub `config-cli`. Shadowing
    /// `config-cli` here made every `config test` assertion read `cli:test`
    /// and prove nothing about the runner selection. Found by running the
    /// port.
    ///
    /// `cargo_shim_dir` goes on `PATH` because the cargo stub and the built
    /// `config-cli` both live there. `CARGO` is cleared as well: cargo sets
    /// it for everything it spawns, so this test binary's own parent cargo
    /// would otherwise win over the stub and run a real build.
    fn run_against_real_cli(&self, arguments: &[&str]) -> Run {
        let path = std::env::var("PATH").unwrap_or_default();
        let cargo_shim_dir = self.cargo_shim_dir.clone();
        self.home.run_with(arguments, move |command| {
            command.env("PATH", format!("{}:{path}", cargo_shim_dir.display()));
            command.env_remove("CARGO");
        })
    }
}

#[test]
fn config_install_delegates_to_config_cli_and_passes_flags_through() {
    let _cli = config_cli_lock();
    let fixture = WrapperFixture::new();
    let run = fixture.run_against_stub_cli(&["install", "--yes", "--dry-run"]);
    assert_eq!(
        run.stdout_trimmed(),
        "cli:deps\ncli:install\ncli:--yes\ncli:--dry-run",
        "config install delegates to config-cli deps install and passes flags through"
    );
}

/// `config test` picks between cargo and the docker runner, runs cargo from
/// `crates/` rather than with `--manifest-path` (rustup honours
/// `crates/rust-toolchain.toml` only from inside that directory), and passes
/// `-q` and a suite name through to whichever runner it picked.
#[test]
fn config_test_selects_the_host_suite_or_the_docker_one() {
    let _cli = config_cli_lock();
    let fixture = WrapperFixture::new();

    assert_eq!(
        fixture.run_against_real_cli(&["test"]).stdout_trimmed(),
        "cargo:test --locked\ncwd:crates",
        "config test runs the cargo suite from crates/"
    );
    assert_eq!(
        fixture
            .run_against_real_cli(&["test", "-q"])
            .stdout_trimmed(),
        "cargo:test --locked --quiet\ncwd:crates",
        "config test -q passes quiet through to cargo"
    );
    assert_eq!(
        fixture
            .run_against_real_cli(&["test", "deps_manifest"])
            .stdout_trimmed(),
        "cargo:test --locked deps_manifest\ncwd:crates",
        "config test <name> passes the name to cargo as a test filter"
    );
    assert_eq!(
        fixture
            .run_against_real_cli(&["test", "--docker"])
            .stdout_trimmed(),
        "docker:(none)",
        "config test --docker runs the whole suite in docker"
    );
    assert_eq!(
        fixture
            .run_against_real_cli(&["test", "--docker", "deps-manifest"])
            .stdout_trimmed(),
        "docker:deps-manifest",
        "config test --docker <suite> passes the suite name"
    );
}

/// Exit 2 for every caller error is a repo-wide convention, so these are
/// contract assertions rather than preferences.
///
/// The usage line is capitalized: clap prints `Usage:`, not the lowercase
/// `usage:` the retired shell script's hand-written line used.
#[test]
fn config_test_rejects_bad_argument_combinations_with_exit_two() {
    let _cli = config_cli_lock();
    let fixture = WrapperFixture::new();

    let run = fixture.run_against_real_cli(&["test", "--bogus"]);
    run.assert_status(2, "config test rejects an unknown flag with exit 2");
    run.assert_contains("Usage: config test", "the rejection prints usage");

    let run = fixture.run_against_real_cli(&["test", "a", "b"]);
    run.assert_status(
        2,
        "config test rejects a second positional argument with exit 2",
    );
    run.assert_contains(
        "Usage: config test",
        "the second-positional-argument rejection prints usage",
    );

    fixture
        .run_against_real_cli(&["test", "-q", "--docker"])
        .assert_status(
            2,
            "config test rejects -q together with --docker with exit 2",
        );
    fixture
        .run_against_real_cli(&["test", "-q", "some-suite"])
        .assert_status(
            2,
            "config test rejects -q together with a suite name with exit 2",
        );

    let run = fixture.run_against_real_cli(&["test", "--watch", "--docker"]);
    run.assert_status(
        2,
        "config test rejects --watch together with --docker with exit 2",
    );
    assert_eq!(
        run.stdout, "",
        "the rejection never runs the docker stub, stdout was: {}",
        run.stdout
    );
}

/// `config test --watch` runs the suite once at start and again when a
/// tracked file changes, and stops when killed.
///
/// Driven with a stub `cargo` that appends one line per run, so the count is
/// the observable. The kill is asserted rather than assumed: a watch loop
/// that survives its parent is a process leak in every developer's session.
#[test]
fn config_test_watch_reruns_on_a_tracked_change_and_stops_when_killed() {
    let _cli = config_cli_lock();
    let home = Home::new();
    let home_path = home.path();
    let runs = home_path.join("runs");

    // `config test` runs cargo from $HOME/crates, so the directory has to
    // exist for the spawn to succeed at all.
    fs::create_dir_all(home_path.join("crates")).expect("creatable");
    let shim_dir = home.root().join("shims");
    fs::create_dir_all(&shim_dir).expect("creatable");
    write_executable(
        &shim_dir.join("cargo"),
        &format!("#!/bin/sh\nprintf 'run\\n' >> '{}'\n", runs.display()),
    );

    // As in WrapperFixture: the `config-test` shim execs `config-cli` off
    // PATH, so the built binary is linked in ahead of the installed one,
    // which is stale whenever this crate's source has changed.
    std::os::unix::fs::symlink(
        env!("CARGO_BIN_EXE_config-cli"),
        shim_dir.join("config-cli"),
    )
    .expect("the built config-cli is linkable");

    let path = std::env::var("PATH").unwrap_or_default();
    let mut child = Command::new(config())
        .args(["test", "--watch"])
        .env("HOME", &home_path)
        .env("PATH", format!("{}:{path}", shim_dir.display()))
        // cargo sets CARGO for everything it spawns, so without this the
        // real cargo wins over the stub and the run count never moves.
        .env_remove("CARGO")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the watch loop starts");

    let runs_before = wait_for_lines(&runs, 1);
    assert_eq!(runs_before, 1, "watch runs the suite once at start");

    fs::write(home_path.join("tracked.txt"), "changed\n").expect("writable");
    home.cfg(&["add", "tracked.txt"]);

    let runs_after = wait_for_lines(&runs, runs_before + 1);
    assert!(
        runs_after > runs_before,
        "watch reruns the suite when a tracked file changes ({runs_before} -> {runs_after})"
    );

    kill_tree(&mut child);
    assert!(
        !child_is_running(&mut child),
        "the watch loop is gone after kill"
    );
}

/// Polls `path` until it holds at least `target` lines, or a deadline passes.
///
/// A poll rather than a fixed sleep: the shell suite slept 2 and 3 seconds,
/// which is both slower than it needs to be on a fast machine and flaky on a
/// loaded one.
fn wait_for_lines(path: &Path, target: usize) -> usize {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut seen = 0;
    while std::time::Instant::now() < deadline {
        seen = fs::read_to_string(path)
            .map(|text| text.lines().count())
            .unwrap_or(0);
        if seen >= target {
            return seen;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    seen
}

fn kill_tree(child: &mut std::process::Child) {
    let pid = child.id();
    let _ = Command::new("pkill")
        .arg("-P")
        .arg(pid.to_string())
        .output();
    let _ = child.kill();
    let _ = child.wait();
}

fn child_is_running(child: &mut std::process::Child) -> bool {
    matches!(child.try_wait(), Ok(None))
}

// --- help -------------------------------------------------------------------

/// `config help`, `config --help` and `config -h` all list the subcommands.
///
/// The listing is generated from the `# help:` line in each `config-<sub>`,
/// so a new utility that lands beside the dispatcher documents itself. There
/// is no second list to update, and no way for the two to drift.
#[test]
fn every_help_spelling_lists_every_subcommand() {
    let _cli = config_cli_lock();
    let home = Home::new();

    for spelling in ["help", "--help", "-h"] {
        let run = home.run(&[spelling]);
        run.assert_status(0, &format!("config {spelling} exits 0"));

        let missing: Vec<&str> = EXPECTED_SUBCOMMANDS
            .iter()
            .copied()
            .filter(|sub| !names_subcommand(&run.text, sub))
            .collect();
        assert!(
            missing.is_empty(),
            "config {spelling} lists every subcommand, missing: {missing:?}\n{}",
            run.text
        );
    }
}

/// Whether the listing names `sub` as a verb rather than only inside a longer
/// flag.
///
/// The shell suite spelled this `grep -q "[^-]$sub"`, which requires one
/// character before the name and so silently cannot match a name at the start
/// of a line. This form asks the question directly: the name appears with a
/// non-hyphen boundary on the left.
fn names_subcommand(listing: &str, sub: &str) -> bool {
    listing.match_indices(sub).any(|(index, _)| {
        let before = listing[..index].chars().next_back();
        before != Some('-')
    })
}

#[test]
fn help_says_unknown_verbs_go_to_git_and_does_not_fall_through_to_it() {
    let _cli = config_cli_lock();
    let home = Home::new();
    let run = home.run(&["help"]);
    run.assert_contains("git", "help says unknown verbs go to git");
    run.assert_lacks("usage: git", "help does not fall through to git");
}

/// Every subcommand carries its own one-line description, and help prints it.
///
/// Both halves are asserted, because a description that exists but is never
/// printed is the drift this generated listing exists to prevent.
#[test]
fn every_subcommand_carries_a_help_description_and_help_prints_it() {
    let _cli = config_cli_lock();
    let home = Home::new();
    let listing = home.run(&["help"]).text;

    let descriptions: Vec<(&str, Option<String>)> = EXPECTED_SUBCOMMANDS
        .iter()
        .copied()
        .map(|sub| (sub, help_description(sub)))
        .collect();

    let undescribed: Vec<&str> = descriptions
        .iter()
        .filter(|(_, line)| line.is_none())
        .map(|(sub, _)| *sub)
        .collect();
    assert!(
        undescribed.is_empty(),
        "every config-<sub> carries a \"# help:\" description, these do not: {undescribed:?}"
    );

    let unprinted: Vec<&str> = descriptions
        .iter()
        .filter_map(|(sub, line)| line.as_ref().map(|text| (*sub, text)))
        .filter(|(_, text)| !listing.contains(text.as_str()))
        .map(|(sub, _)| sub)
        .collect();
    assert!(
        unprinted.is_empty(),
        "help prints each subcommand's own description, these are missing: {unprinted:?}\n{listing}"
    );
}

/// The first `# help:` line of a `config-<sub>`, if it has one.
fn help_description(sub: &str) -> Option<String> {
    let source = fs::read_to_string(config_dir().join(format!("config-{sub}"))).ok()?;
    source
        .lines()
        .find_map(|line| line.strip_prefix("# help: "))
        .map(str::to_owned)
        .filter(|line| !line.is_empty())
}

/// A help listing that scrolls off the screen is not read. This is a ceiling,
/// not a target.
#[test]
fn the_help_listing_stays_under_thirty_lines() {
    let _cli = config_cli_lock();
    let home = Home::new();
    let listing = home.run(&["help"]).text;

    // Positive control: an empty listing would satisfy any ceiling.
    assert!(
        listing.lines().count() > 1,
        "the listing has content to measure"
    );
    let line_count = listing.lines().count();
    assert!(
        line_count <= 30,
        "the help listing stays under 30 lines, it is {line_count}"
    );
}

// --- explicit git passthrough with -- ---------------------------------------

/// `config -- <verb>` sends `<verb>` to git without consulting the sibling
/// scripts.
///
/// Without it, a `config-<sub>` that shares a name with a git command makes
/// that git command unreachable through the dispatcher. `config help` is the
/// first such name, and any future one lands the same way.
#[test]
fn the_separator_reaches_git_rather_than_the_help_listing() {
    let home = Home::new();
    let run = home.run(&["--", "help"]);
    run.assert_status(0, "config -- help exits 0");
    run.assert_contains(
        "usage: git",
        "config -- help reaches git, not the help listing",
    );
    run.assert_lacks(
        "config <command>",
        "config -- help does not print the subcommand listing",
    );
}

#[test]
fn the_separator_passes_ordinary_git_verbs_through_unchanged() {
    let home = Home::new();
    let expected = home.cfg_stdout(&["rev-parse", "HEAD"]);
    assert!(!expected.is_empty(), "the fixture has a HEAD to compare");
    assert_eq!(
        home.run(&["--", "rev-parse", "HEAD"]).stdout_trimmed(),
        expected,
        "config -- passes ordinary git verbs through unchanged"
    );

    // The separator is consumed, not forwarded. Passing it on would turn
    // `config -- log <path>` into `git log -- <path>`, a pathspec, which is a
    // different command.
    assert_eq!(
        home.run(&["--", "status", "--porcelain", "--untracked-files=no"])
            .text,
        "",
        "config -- status behaves like config status"
    );
}

#[test]
fn the_separator_does_not_rescue_an_unknown_git_verb() {
    let home = Home::new();
    let run = home.run(&["--", "definitely-not-a-command"]);
    run.assert_status(1, "config -- with an unknown git verb still fails");
    run.assert_contains(
        "git: 'definitely-not-a-command' is not a git command",
        "the failure is git's own message",
    );
}

/// A bare `config --` has nothing to pass through. Git treats it as no verb
/// and prints its usage, which is the honest answer.
#[test]
fn a_bare_separator_does_not_succeed_silently() {
    let home = Home::new();
    home.run(&["--"])
        .assert_status(1, "a bare config -- does not succeed silently");
}

/// Only the FIRST argument is the separator. A later `--` is a git pathspec
/// and must survive untouched.
#[test]
fn a_later_separator_stays_a_git_pathspec() {
    let home = Home::new();
    fs::write(home.path().join("passthrough.txt"), "content\n").expect("writable");
    home.cfg(&["add", "passthrough.txt"]);

    assert_eq!(
        home.run(&["diff", "--cached", "--name-only", "--", "passthrough.txt"])
            .stdout_trimmed(),
        "passthrough.txt",
        "a -- later in the line stays a git pathspec"
    );
}

#[test]
fn the_separator_does_not_dispatch_to_a_sibling_script() {
    let home = Home::new();
    let sibling = SiblingDir::new(home.root());
    let run = home.run_program(&sibling.dispatcher(), &["--", "probe", "one"], |_| {});
    run.assert_lacks("probe:", "config -- does not dispatch to a sibling script");
}

// --- reload -----------------------------------------------------------------

/// `config-reload` is a shim, like install and deps: every decision moved to
/// `config-cli`, so this only has to prove the shim delegates and passes
/// arguments through. The tmux and Alacritty behaviour itself is
/// `config-cli`'s own contract, asserted in `reload_behavior.rs`.
#[test]
fn config_reload_delegates_to_config_cli_and_passes_arguments_through() {
    let _cli = config_cli_lock();
    let fixture = WrapperFixture::new();
    let run = fixture.run_against_stub_cli(&["reload", "extra-arg"]);
    assert_eq!(
        run.stdout_trimmed(),
        "cli:reload\ncli:extra-arg",
        "config reload delegates to config-cli reload and passes args through"
    );
}

#[test]
fn the_per_shell_tmux_source_is_gone_from_zshrc() {
    let zshrc = fs::read_to_string(repo_root().join(".zshrc")).expect(".zshrc is readable");

    // Positive control: an unreadable or empty .zshrc would satisfy the
    // absence claim without asserting anything.
    assert!(!zshrc.trim().is_empty(), ".zshrc has content to search");

    let sourcing: Vec<&str> = zshrc
        .lines()
        .filter(|line| line.contains("tmux source"))
        .collect();
    assert!(
        sourcing.is_empty(),
        "the per-shell tmux source is gone from .zshrc, found: {sourcing:?}"
    );
}

// --- doctor -----------------------------------------------------------------

fn doctor() -> PathBuf {
    config_dir().join("config-doctor")
}

#[test]
fn config_doctor_exists_and_does_not_shadow_a_git_verb() {
    let _cli = config_cli_lock();
    assert!(
        is_executable(&doctor()),
        "config doctor exists and is executable"
    );
    assert!(
        !git_command_names().contains("doctor"),
        "doctor does not shadow a git verb"
    );
}

/// Whether this environment carries a repository doctor can read.
///
/// Doctor gathers its expected side by reading the real `crates/` tree
/// through git, so it needs a real `.cfg` or `.git` at the root. The Docker
/// runner's image carries no repository at all, so a call there would fail
/// for a reason unrelated to doctor.
fn has_repository() -> bool {
    repo_root().join(".cfg").is_dir() || repo_root().join(".git").is_dir()
}

fn config_bin_dir() -> PathBuf {
    std::env::var_os("CONFIG_BIN_DIR").map_or_else(
        || PathBuf::from(std::env::var_os("HOME").expect("HOME is set")).join(".local/bin"),
        PathBuf::from,
    )
}

/// `PATH` with the directory `config-build` installs into placed first.
///
/// Doctor reads the binary `PATH` resolves, so the freshly installed one has
/// to be the one it finds. Without this the test asserts against whichever
/// copy the environment happened to put first, which on CI is the workflow's
/// own unstamped build.
fn doctor_path() -> String {
    let path = std::env::var("PATH").unwrap_or_default();
    format!("{}:{path}", config_bin_dir().display())
}

fn run_build() -> Output {
    Command::new(config_dir().join("config-build"))
        .current_dir(repo_root())
        .output()
        .expect("config-build runs")
}

/// Runs `config-doctor` from the repository root.
///
/// **The working directory is load-bearing, which the shell suite never had
/// to say.** Doctor gathers its expected side with `git add -- crates`
/// through a temp index, and that pathspec resolves against the CURRENT
/// DIRECTORY rather than the work tree, so from any subdirectory doctor
/// reports `pathspec 'crates' did not match any files` instead of a stamp
/// comparison. The shell suite ran from `$HOME` and so never saw it; a cargo
/// test runs from `crates/` and sees it immediately. Found by running this
/// port.
///
/// It fails closed (exit 1, with the git error printed), so this is a
/// misleading diagnosis rather than a fail-open hole. Reported as a finding;
/// fixing `config-doctor` is out of this conversion's scope.
fn run_doctor(configure: impl FnOnce(&mut Command)) -> Run {
    let mut command = Command::new(doctor());
    command
        .current_dir(repo_root())
        .env("PATH", doctor_path())
        .stdin(Stdio::null());
    configure(&mut command);
    let output = command.output().expect("config-doctor runs");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    Run {
        status: output.status.code().unwrap_or(-1),
        text: format!("{stdout}{stderr}"),
        stdout,
    }
}

/// Doctor is silent and exits 0 when every binary is current.
///
/// The build's own status is checked rather than discarded. Swallowing it
/// made a failed build read as a doctor defect: on a CI runner `config-build`
/// installs into `$HOME/.local/bin` while `PATH` resolves `config-cli` to the
/// workflow's own release copy, so doctor compared an unstamped binary and
/// was correct to report a mismatch, while the assertion blamed doctor.
///
/// Status and output are asserted separately. A silent failure (the binary
/// not on `PATH`, a crash before writing) would otherwise read as "silent
/// because everything is current".
///
/// **This test builds six crates**, which is where this file's runtime goes.
/// Skipping the build would leave nothing to compare against.
#[test]
fn doctor_is_silent_and_exits_zero_when_every_binary_is_current() {
    if !has_repository() {
        dotfiles_test_support::skip(
            "doctor exits 0 and is silent when every binary is current: \
             no repository in this environment",
        );
        return;
    }

    let _guard = build_lock();

    let build = run_build();
    assert!(
        build.status.success(),
        "config-build succeeds before doctor is asked, it said: {}{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let run = run_doctor(|_| {});
    run.assert_status(0, "doctor exits 0 when every binary is current");
    assert_eq!(
        run.text, "",
        "doctor is silent when every binary is current"
    );
}

/// Doctor must look where `config-build` installs, even when `DOTFILES_ROOT`
/// is not `$HOME`.
///
/// It defaulted to `$DOTFILES_ROOT/.local/bin` while `config-build` defaults
/// to `$HOME/.local/bin`. Those coincide on a developer machine, where
/// `DOTFILES_ROOT` IS `$HOME`, and diverge in CI, where `DOTFILES_ROOT` is
/// the checkout, so doctor reported "not installed" for a binary that was
/// installed correctly.
///
/// Asserted by pointing `DOTFILES_ROOT` at a copy while leaving the binary
/// where `config-build` put it.
#[test]
fn doctor_does_not_report_not_installed_when_dotfiles_root_is_not_home() {
    if !has_repository() {
        dotfiles_test_support::skip(
            "doctor does not report a not-installed binary when DOTFILES_ROOT is not HOME: \
             no repository in this environment",
        );
        return;
    }

    let scratch = tempfile::tempdir().expect("a temporary directory");
    let split_root = scratch.path().join("split-root");
    fs::create_dir_all(split_root.join(".scripts/config")).expect("creatable");

    copy_tree(&repo_root().join("crates"), &split_root.join("crates"));
    fs::copy(
        config_dir().join("config-stamp"),
        split_root.join(".scripts/config/config-stamp"),
    )
    .expect("copyable");

    git_in(&split_root, &["init", "-q", "."]);
    let _ = git_command()
        .arg("-C")
        .arg(&split_root)
        .args(["add", "-A"])
        .output();
    git_in(
        &split_root,
        &[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "-m",
            "split-root probe",
        ],
    );

    let run = run_doctor(|command| {
        command.env("DOTFILES_ROOT", &split_root);
    });
    run.assert_lacks(
        "not installed",
        "doctor does not report a not-installed binary when DOTFILES_ROOT is not HOME",
    );
}

/// The behaviour doctor exists for, asserted rather than checked by hand.
///
/// The probe edits `config-cli`, not `config-manifest`. `config-manifest` is
/// library-only: it installs no binary, so doctor drops it from both sides of
/// the comparison and an edit there moves nothing. Pointed at
/// `config-manifest`, this block asserted "stale" against a crate doctor was
/// correct to ignore, which is a silent pass rather than a real check.
///
/// **This test builds six crates twice**, once to make the probe stale and
/// once to restore. That is the rest of this file's runtime, and converting
/// the suite does not change it.
#[test]
fn doctor_reports_a_stale_binary_and_names_the_fix() {
    if !has_repository() {
        dotfiles_test_support::skip(
            "doctor exits 1 and names the stale crate and the fix: \
             no repository in this environment",
        );
        return;
    }

    let _guard = build_lock();

    let build = run_build();
    assert!(
        build.status.success(),
        "config-build succeeds before the probe, it said: {}{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let probe = repo_root().join("crates/config-cli/src/doctor.rs");
    let original = fs::read_to_string(&probe).expect("the probe source is readable");

    let restore = ProbeGuard {
        path: probe.clone(),
        original: original.clone(),
    };

    fs::write(&probe, format!("{original}\n// staleness probe\n")).expect("writable");

    let run = run_doctor(|_| {});
    run.assert_status(1, "doctor exits 1 when a binary is stale");
    run.assert_contains("config-cli", "doctor names the stale crate");
    run.assert_contains("config build", "doctor names the fix");

    drop(restore);
}

/// Restores the probe file and rebuilds, even if the test panics first.
///
/// Without this, a failing assertion leaves the tree edited and every later
/// run of this suite (and the developer's next `config doctor`) reports a
/// stale binary the test created.
struct ProbeGuard {
    path: PathBuf,
    original: String,
}

impl Drop for ProbeGuard {
    fn drop(&mut self) {
        let _ = fs::write(&self.path, &self.original);
        let _ = run_build();
    }
}

fn copy_tree(source: &Path, destination: &Path) {
    let output = Command::new("cp")
        .arg("-R")
        .arg(source)
        .arg(destination)
        .output()
        .expect("cp runs");
    assert!(
        output.status.success(),
        "copying {} failed: {}",
        source.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}
