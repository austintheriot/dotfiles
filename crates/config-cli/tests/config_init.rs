//! `config init`, the post-clone half of the bootstrap.
//!
//! Every test runs against a fixture `$HOME`: a bare repo at `.cfg` with a
//! worktree, so nothing here touches the real dotfiles repo. The fixture
//! carries its own `.scripts/config` tree, because `config init` runs the
//! sibling `config-<sub>` scripts and this suite must not run the real
//! `install-hooks` against the real `~/.cfg`.
//!
//! The install and build steps are stubbed. Both shell out to the network and
//! to cargo, and this suite's subject is the ORCHESTRATION: which steps run,
//! in what order, under which flags. `--dry-run` is asserted against the real
//! scripts, because it is the one path that runs no step at all.
//!
//! Converted whole from `tests/config-init.test.sh`, which ran **56**
//! assertions, measured by running it rather than by counting `assert_` call
//! sites.
//!
//! Zero mutation of the developer's real home is the property this file must
//! not lose, and [`the_fixture_home_absorbs_every_write`] asserts it directly
//! rather than leaving it to review: it counts the real home's entries and the
//! live tmux sessions before and after a full `--yes` run, and asserts the
//! fixture home holds everything that was written.

use dotfiles_test_support::repo::root as repo_root;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn config_dir() -> PathBuf {
    repo_root().join(".scripts/config")
}

fn init_script() -> PathBuf {
    config_dir().join("config-init")
}

fn write_executable(path: &Path, body: &str) {
    dotfiles_test_support::stub::write(path, body).expect("the stub is writable");
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

/// A fixture `$HOME` with a bare repo at `.cfg` and a stubbed
/// `.scripts/config` tree.
///
/// The stubs record their invocation to `.calls`, in order, so a test can
/// assert what ran without any step touching the machine.
struct InitHome {
    directory: TempDir,
}

impl InitHome {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let home = directory.path();
        fs::create_dir_all(home.join(".scripts/config")).expect("creatable");

        let seed = home.join("seed");
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

        for file in ["config", "usage.sh", "config-init"] {
            fs::copy(config_dir().join(file), home.join(".scripts/config").join(file))
                .unwrap_or_else(|_| panic!("{file} is copyable"));
            let _ = fs::set_permissions(
                home.join(".scripts/config").join(file),
                fs::Permissions::from_mode(0o755),
            );
        }

        let fixture = Self { directory };
        for sub in ["install-hooks", "prereqs", "install", "build"] {
            fixture.stub_step(sub);
        }
        fs::write(fixture.calls_path(), "").expect("the call log is writable");
        fixture
    }

    fn path(&self) -> &Path {
        self.directory.path()
    }

    fn calls_path(&self) -> PathBuf {
        self.path().join(".calls")
    }

    /// The default stub: records its name and argv, then succeeds.
    fn stub_step(&self, sub: &str) {
        write_executable(
            &self.path().join(".scripts/config").join(format!("config-{sub}")),
            &format!(
                "#!/bin/sh\n# help: stub\n# usage: config {sub}\n\
                 printf '%s %s\\n' '{sub}' \"$*\" >> \"$HOME/.calls\"\n"
            ),
        );
    }

    /// Replaces one step's stub with an arbitrary body.
    fn replace_step(&self, sub: &str, body: &str) {
        write_executable(
            &self.path().join(".scripts/config").join(format!("config-{sub}")),
            body,
        );
    }

    fn remove_step(&self, sub: &str) {
        let _ = fs::remove_file(self.path().join(".scripts/config").join(format!("config-{sub}")));
    }

    fn calls(&self) -> String {
        fs::read_to_string(self.calls_path())
            .unwrap_or_default()
            .trim_end_matches('\n')
            .to_string()
    }

    /// The step names that ran, in order.
    fn step_order(&self) -> Vec<String> {
        self.calls()
            .lines()
            .filter_map(|line| line.split_whitespace().next())
            .map(str::to_string)
            .collect()
    }

    /// Runs `config init` under this fixture home.
    fn run_init(&self, arguments: &[&str], path: Option<&str>) -> (i32, String) {
        let mut command = Command::new(self.path().join(".scripts/config/config"));
        command
            .arg("init")
            .args(arguments)
            .current_dir(self.path())
            .env("HOME", self.path());
        if let Some(path) = path {
            command.env("PATH", path);
        }
        // Through the shared runner: this spawn races the stubs the fixture
        // just wrote, and a lost race is ETXTBSY on exec rather than anything
        // this suite means to assert.
        let output = dotfiles_test_support::stub::run(&mut command).expect("the dispatcher runs");
        (
            output.status.code().unwrap_or(-1),
            format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        )
    }

    /// The value of one `.cfg` git setting, or `None` when unset.
    fn cfg_setting(&self, key: &str) -> Option<String> {
        let output = Command::new("git")
            .arg(format!("--git-dir={}", self.path().join(".cfg").display()))
            .args(["config", "--get", key])
            .output()
            .expect("git config runs");
        output.status.success().then(|| {
            String::from_utf8_lossy(&output.stdout)
                .trim_end_matches('\n')
                .to_string()
        })
    }
}

/// The script exists and is runnable.
#[test]
fn config_init_is_executable() {
    assert!(
        fs::metadata(init_script())
            .is_ok_and(|data| data.permissions().mode() & 0o111 != 0),
        "{} is missing or not executable",
        init_script().display()
    );
}

/// The steps run, in the order the bootstrap depends on, and `--yes` reaches
/// the install step.
///
/// Order is the contract, not an accident, and it encodes one rule: a thin
/// shell layer installs only what is needed to RUN the Rust engine, then the
/// engine installs everything else.
///
/// BUILD BEFORE INSTALL is the correction. The previous order was hooks,
/// install, build, on the stated reasoning that "install places the Rust
/// toolchain that build then compiles with". That stopped being true when the
/// engine became `config-cli`: the install step is `exec config-cli deps
/// install`, and `config-cli` is what build PRODUCES, so install depended on
/// the output of a step that ran after it. Reproduced in clean
/// `debian:bookworm` and `ubuntu:24.04` as
/// `exec: config-cli: not found`.
#[test]
fn the_steps_run_in_the_order_the_bootstrap_depends_on() {
    let home = InitHome::new();
    let (status, _) = home.run_init(&["--yes"], None);
    assert_eq!(status, 0, "config init --yes did not exit 0 on a fresh fixture");

    let order = home.step_order();
    assert_eq!(
        order,
        vec!["install-hooks", "prereqs", "build", "install"],
        "the steps did not run in order: hooks, prereqs, build, install"
    );

    // Asserted as POSITIONS too, not only as a whole-list match: prereqs after
    // build is the bug in a different costume, and a whole-list comparison
    // that someone later loosens would lose that.
    let position = |name: &str| {
        order
            .iter()
            .position(|step| step == name)
            .unwrap_or_else(|| panic!("{name} never ran"))
    };
    assert!(
        position("prereqs") < position("build"),
        "prereqs ran after build, so the engine would install its own toolchain"
    );
    assert!(
        position("build") < position("install"),
        "build ran after install, so install would exec a binary build had \
         not produced yet"
    );

    // --yes must reach the install step, or an unattended bootstrap stops at
    // the first prompt with no terminal to answer it.
    assert!(
        home.calls().contains("install --yes"),
        "the install step did not receive --yes: {}",
        home.calls()
    );

    // The one setting the bare-repo-over-home technique depends on. Without
    // it every untracked file in $HOME shows up in `config status`.
    assert_eq!(
        home.cfg_setting("status.showUntrackedFiles").as_deref(),
        Some("no"),
        "init did not set status.showUntrackedFiles to no"
    );
}

/// A second run converges rather than failing.
///
/// The bootstrap is re-run by hand on a machine already set up, and by the
/// container suite twice in a row.
#[test]
fn a_second_run_is_idempotent() {
    let home = InitHome::new();
    home.run_init(&["--yes"], None);
    let (status, output) = home.run_init(&["--yes"], None);

    assert_eq!(status, 0, "a second config init --yes did not exit 0: {output}");
    assert_eq!(
        home.cfg_setting("status.showUntrackedFiles").as_deref(),
        Some("no"),
        "status.showUntrackedFiles is not `no` after a second run"
    );
}

/// `--dry-run` accounts for every step, marks each line hypothetical, and runs
/// nothing.
///
/// Asserted against the REAL `config-<sub>` scripts, not the stubs:
/// `--dry-run`'s whole claim is that it runs no step, so a stub proving it
/// would prove nothing about the real ones.
#[test]
fn a_dry_run_accounts_for_every_step_and_changes_nothing() {
    let home = InitHome::new();
    for sub in ["install-hooks", "install", "build"] {
        home.remove_step(sub);
    }
    let _ = Command::new("git")
        .arg(format!("--git-dir={}", home.path().join(".cfg").display()))
        .args(["config", "--unset", "status.showUntrackedFiles"])
        .output();

    let (status, output) = home.run_init(&["--dry-run"], None);
    assert_eq!(status, 0, "config init --dry-run did not exit 0: {output}");

    // Each step is named in prose rather than by script name, so these assert
    // the step is accounted for without freezing the wording.
    for (needle, step) in [
        ("showUntrackedFiles", "the git setting"),
        ("hooks", "the hooks step"),
        ("dependencies", "the install step"),
        ("build", "the build step"),
        ("toolchain", "the prereqs step"),
        ("nothing was changed", "the closing claim"),
    ] {
        assert!(
            output.contains(needle),
            "the dry run does not account for {step}: {output}"
        );
    }

    // Every step line is marked as hypothetical. A dry run whose output reads
    // like a transcript of work done is worse than no dry run.
    let unmarked: Vec<&str> = output
        .lines()
        .filter(|line| line.starts_with("config init: [") && !line.contains("would"))
        .collect();
    assert!(
        unmarked.is_empty(),
        "these dry-run step lines are not marked `would`: {unmarked:?}"
    );

    // The proof that nothing ran: the setting it would have written is
    // absent, and the missing step scripts were never invoked.
    assert_eq!(
        home.cfg_setting("status.showUntrackedFiles"),
        None,
        "the dry run wrote git config"
    );
    assert_eq!(home.calls(), "", "the dry run recorded calls");
}

/// `--help` prints the usage block, exits 0, and runs no step.
///
/// `usage_if_requested` is called before any parsing for exactly this reason.
#[test]
fn help_prints_the_usage_block_and_runs_nothing() {
    let fixtures = tempfile::tempdir().expect("a temporary directory");
    let output = Command::new(init_script())
        .arg("--help")
        .env("HOME", fixtures.path())
        .output()
        .expect("config-init runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "config init --help did not exit 0"
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("usage: config init"),
        "the usage block did not print"
    );

    let home = InitHome::new();
    home.run_init(&["--help"], None);
    assert_eq!(home.calls(), "", "--help ran a step");
}

/// An unknown flag exits 2 and runs no step.
#[test]
fn an_unknown_flag_exits_two_and_runs_nothing() {
    let home = InitHome::new();
    let (status, output) = home.run_init(&["--not-a-flag"], None);
    assert_eq!(status, 2, "an unknown flag did not exit 2: {output}");
    assert_eq!(home.calls(), "", "an unknown flag ran a step");
}

/// A failing build step is fatal, and the install step does not run after it.
///
/// THIS INVERTS AN EARLIER CONTRACT, deliberately. The build step used to run
/// LAST and was the one step allowed to fail without failing the bootstrap,
/// because "no cargo yet" was a real state on a fresh box. Two facts changed,
/// and each alone is enough: `config prereqs` now runs BEFORE the build and
/// installs cc and rustup, so a build failure past that point is a compile
/// failure rather than a missing toolchain; and the build runs BEFORE the
/// install, so tolerating a build failure would walk straight into
/// `exec: config-cli: not found`.
#[test]
fn a_failing_build_step_is_fatal() {
    let home = InitHome::new();
    home.replace_step(
        "build",
        "#!/bin/sh\n# help: stub\n# usage: config build\n\
         printf 'build %s\\n' \"$*\" >> \"$HOME/.calls\"\n\
         printf 'error: could not compile `config-cli`\\n' >&2\nexit 1\n",
    );

    let (status, output) = home.run_init(&["--yes"], None);
    assert_eq!(status, 1, "a failing build step did not exit 1: {output}");
    assert!(
        output.contains("build step failed"),
        "the failure does not name the build step: {output}"
    );
    // The message must not send the reader back to the package manager:
    // prereqs already succeeded, so a compile failure is not a missing
    // dependency.
    assert!(
        output.contains("cc and rustup are installed"),
        "the failure does not say the toolchain is already installed: {output}"
    );

    // The steps before it must still have completed.
    let order = home.step_order();
    assert!(
        order.contains(&"install-hooks".to_string()),
        "the hooks were not linked before the build failed: {order:?}"
    );
    assert!(
        order.contains(&"prereqs".to_string()),
        "the toolchain step did not run before the build failed: {order:?}"
    );
    assert_eq!(
        home.cfg_setting("status.showUntrackedFiles").as_deref(),
        Some("no"),
        "the git setting was not written before the build failed"
    );

    // And the install step must NOT have run: it execs the binary this step
    // failed to produce.
    assert!(
        !order.contains(&"install".to_string()),
        "the install step ran after a failed build: {order:?}"
    );
}

/// A failing install step is not a success.
///
/// Reported from a bare root container: eleven dependencies failed to install
/// and `config init` still walked to the end and reported done. The install
/// step's exit code was simply not read.
#[test]
fn a_failing_install_step_is_not_a_success() {
    let home = InitHome::new();
    home.replace_step(
        "install",
        "#!/bin/sh\n# help: stub\n# usage: config install\n\
         printf 'install %s\\n' \"$*\" >> \"$HOME/.calls\"\n\
         printf 'deps: 11 automated install(s) did not satisfy their check\\n' >&2\n\
         exit 1\n",
    );

    let (status, output) = home.run_init(&["--yes"], None);
    assert_eq!(status, 1, "a failing install step did not exit 1: {output}");
    assert!(
        output.to_lowercase().contains("install"),
        "the failure does not name the install step: {output}"
    );

    // What already succeeded must still be reported, so the reader knows the
    // machine is partly set up rather than untouched.
    let lowered = output.to_lowercase();
    assert!(
        lowered.contains("hook") || lowered.contains("checkout") || lowered.contains("place"),
        "the output does not say what did succeed: {output}"
    );
    assert!(
        !output.lines().any(|line| line.trim() == "config init: done"),
        "it reported plain success after a failed install: {output}"
    );
}

/// A failing prereqs step is fatal before anything else.
///
/// A machine with no cc and no rustup cannot build the engine, and every step
/// after prereqs needs the engine. Continuing would reach `config-install` and
/// reproduce `exec: config-cli: not found`, a worse message for the same
/// problem.
#[test]
fn a_failing_prereqs_step_stops_everything_after_it() {
    let home = InitHome::new();
    home.replace_step(
        "prereqs",
        "#!/bin/sh\n# help: stub\n# usage: config prereqs\n\
         printf 'prereqs %s\\n' \"$*\" >> \"$HOME/.calls\"\n\
         printf 'config prereqs: no known package manager here\\n' >&2\nexit 1\n",
    );

    let (status, output) = home.run_init(&["--yes"], None);
    assert_eq!(status, 1, "a failing prereqs step did not exit 1: {output}");
    assert!(
        output.contains("toolchain"),
        "the failure does not name the toolchain step: {output}"
    );

    let order = home.step_order();
    let after: Vec<&String> = order
        .iter()
        .filter(|step| *step == "build" || *step == "install")
        .collect();
    assert!(
        after.is_empty(),
        "steps ran after a failed prereqs: {after:?}"
    );
}

/// The steps see a `~/.cargo/bin` and a `~/.local/bin` the caller's `PATH`
/// does not carry.
///
/// Reported from a bare container: the run ended with "run `config build` once
/// cargo is on PATH" on a machine where the install step had just installed
/// rustup successfully. rustup writes to `~/.cargo/bin`, which is not on a
/// default non-login `PATH`, so `config-build` could not see the cargo that
/// now existed.
///
/// DRIVEN rather than grepped. An earlier version of this check grepped
/// `config-init` for `.local/bin`, which passed on a comment mentioning the
/// path and would have kept passing with the export deleted.
#[test]
fn the_steps_inherit_the_directories_the_bootstrap_writes_to() {
    let home = InitHome::new();
    // A cargo that exists ONLY in ~/.cargo/bin, exactly as rustup leaves it.
    write_executable(&home.path().join(".cargo/bin/cargo"), "#!/bin/sh\nexit 0\n");
    home.replace_step(
        "build",
        "#!/bin/sh\n# help: stub\n# usage: config build\n\
         printf 'build %s\\n' \"$*\" >> \"$HOME/.calls\"\n\
         command -v cargo >/dev/null 2>&1 || exit 1\n\
         printf 'build-saw-cargo\\n' >> \"$HOME/.calls\"\n",
    );

    // A PATH without ~/.cargo/bin, which is what a fresh non-login shell has.
    let (status, output) = home.run_init(&["--yes"], Some("/usr/bin:/bin"));
    assert!(
        home.calls().contains("build-saw-cargo"),
        "the build step did not find a cargo in ~/.cargo/bin: {}",
        home.calls()
    );
    assert_eq!(
        status, 0,
        "the run did not succeed, so it deferred the build: {output}"
    );
    assert!(
        !output.contains("once cargo is on PATH"),
        "it told the reader to run config build later: {output}"
    );

    // ~/.local/bin too: config-install-hooks puts `config` there and
    // config-build installs config-cli there, and both are resolved by name
    // afterwards.
    let home = InitHome::new();
    write_executable(
        &home.path().join(".local/bin/only-in-local-bin"),
        "#!/bin/sh\nexit 0\n",
    );
    home.replace_step(
        "build",
        "#!/bin/sh\n# help: stub\n# usage: config build\n\
         command -v only-in-local-bin >/dev/null 2>&1 \
         && printf 'saw-local-bin\\n' >> \"$HOME/.calls\"\n",
    );
    home.run_init(&["--yes"], Some("/usr/bin:/bin"));
    assert!(
        home.calls().contains("saw-local-bin"),
        "the steps did not inherit ~/.local/bin: {}",
        home.calls()
    );
}

/// The closing output says how to reach `config` in the shell that just ran
/// it, and only when it is not already reachable.
///
/// Reported from a bare container: `config init: done`, and then `config st`
/// was "command not found". `install-hooks` links config into `~/.local/bin`,
/// but no process can change the `PATH` of the shell that started it.
///
/// The hint appears only when it is needed. On a machine where `~/.local/bin`
/// is already on `PATH` the hint is noise, and noise in a success message is
/// how real warnings get ignored.
#[test]
fn the_closing_output_names_the_path_export_only_when_needed() {
    let home = InitHome::new();
    let (_, output) = home.run_init(&["--yes"], None);
    assert!(
        output.contains("export PATH="),
        "the closing output does not name the export: {output}"
    );
    assert!(
        output.contains(".local/bin"),
        "the export does not name local bin: {output}"
    );
    // It must also say that new shells are already handled, or the reader
    // assumes they have to run that export forever.
    let lowered = output.to_lowercase();
    assert!(
        lowered.contains("profile") || lowered.contains("zshrc") || lowered.contains("new shell"),
        "it does not say where new shells pick it up: {output}"
    );

    let quiet = InitHome::new();
    fs::create_dir_all(quiet.path().join(".local/bin")).expect("creatable");
    let path = format!("{}/.local/bin:/usr/bin:/bin", quiet.path().display());
    let (_, output) = quiet.run_init(&["--yes"], Some(&path));
    assert!(
        !output.contains("export PATH="),
        "the hint printed though local bin is already on PATH: {output}"
    );
}

/// The finished setup hands over to the new login shell, and never unattended.
///
/// The feature: after a successful `config init`, the reader is dropped into
/// zsh rather than left in the bash they started in. Without it, the last
/// thing a fresh-machine bootstrap does is print "done" into a shell with none
/// of the configuration it just installed.
///
/// THE CONSTRAINT THAT MAKES THIS DELICATE: an unattended run must NOT exec a
/// shell. The documented entry point is `curl ... | sh`, CI runs the same
/// script, and the Docker bootstrap harnesses run it too. Exec'ing an
/// interactive zsh there replaces the bootstrap process with a shell reading a
/// closed pipe, which hangs the leg until its timeout.
///
/// The guard and the `${SHELL:-}` fallback are asserted against the CODE,
/// because the abort they prevent happens only on a machine where `getent` is
/// also absent, a combination this suite cannot produce on either CI platform.
/// The unattended path is asserted by DRIVING it: a run that returns is a run
/// that did not exec.
#[test]
fn the_handover_is_gated_on_a_terminal() {
    let source = fs::read_to_string(init_script()).expect("config-init is readable");

    // Comments are stripped FIRST. `config-init` explains the handover in
    // prose above it ("`exec`, not a subshell, so the reader's shell IS zsh"),
    // and a search over raw lines matches that comment at line 260 rather than
    // the `exec zsh -l` at line 301. The shell suite this replaces had the
    // same bug and passed only because a `-t 1` happened to sit above the
    // comment it found. A grep satisfied by a comment is the vacuity class
    // `.claude/rules/dotfiles-tests.md` records twice.
    let code: Vec<&str> = source
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') { "" } else { line }
        })
        .collect();

    let handover_line = code
        .iter()
        .position(|line| line.contains("exec") && line.contains("zsh"))
        .expect("config-init has no `exec ... zsh` handover line outside a comment");

    // Walk back from the handover to find its guard. `-t 0` (or `-t 1`) must
    // appear above it: that is the "somebody is actually here" test.
    let guarded = code
        .iter()
        .take(handover_line + 1)
        .any(|line| line.contains("-t 0") || line.contains("-t 1"));
    assert!(
        guarded,
        "the handover at line {} is not guarded by a terminal test, so an \
         unattended run would exec a shell reading a closed pipe",
        handover_line + 1
    );

    // `set -u` is on, and SHELL is genuinely unset in a bare container's
    // non-login shell, which is the machine this handover exists for. An
    // unguarded expansion turns a finished bootstrap into an error.
    assert!(
        code.iter().any(|line| line.contains("login_shell=${SHELL:-}")),
        "the SHELL fallback is not guarded against being unset"
    );
    assert!(
        !code.iter().any(|line| line.trim() == "login_shell=$SHELL"),
        "an unguarded SHELL expansion remains in the handover"
    );

    // A dry run changes nothing, including replacing the reader's shell.
    let dry = Command::new(init_script())
        .arg("--dry-run")
        .output()
        .expect("config-init runs");
    let dry_output = String::from_utf8_lossy(&dry.stdout);
    assert!(
        !dry_output.lines().any(|line| line == "config init: done"),
        "a dry run reported done, so it took the completion path: {dry_output}"
    );

    // The unattended path must reach its end and RETURN, which is what proves
    // it did not exec. Piped, not --yes: --yes is about declining prompts, and
    // the point here is the ABSENCE of a terminal, which is the condition the
    // guard reads.
    let piped = Command::new(init_script())
        .arg("--dry-run")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("config-init runs");
    assert_eq!(
        piped.status.code(),
        Some(0),
        "an unattended run did not return 0, so it may have exec'd a shell"
    );
}

/// A full `--yes` run mutates the fixture home and NOTHING in the real one.
///
/// The evidence spec 4a.1 set for Tranche A, asserted here rather than left to
/// a reviewer: the real home's entry count and the live tmux session count are
/// read before and after, and the fixture home is shown to hold the writes.
/// A test that pointed `HOME` at a fixture but leaked a write through an
/// absolute path would pass every other assertion in this file and fail this
/// one.
#[test]
fn the_fixture_home_absorbs_every_write() {
    let real_home = PathBuf::from(std::env::var_os("HOME").expect("HOME is set"));

    let count_entries = |directory: &Path| -> usize {
        fs::read_dir(directory)
            .map(|entries| entries.filter_map(Result::ok).count())
            .unwrap_or(0)
    };
    let count_tmux_sessions = || -> usize {
        Command::new("tmux")
            .args(["list-sessions", "-F", "#{session_name}"])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).lines().count())
            .unwrap_or(0)
    };

    let entries_before = count_entries(&real_home);
    let sessions_before = count_tmux_sessions();
    assert!(
        entries_before > 0,
        "the real home listed zero entries, so the comparison below would \
         hold zero against zero and prove nothing"
    );

    let home = InitHome::new();
    let (status, output) = home.run_init(&["--yes"], None);
    assert_eq!(status, 0, "the fixture run did not succeed: {output}");

    let entries_after = count_entries(&real_home);
    let sessions_after = count_tmux_sessions();

    assert_eq!(
        entries_after, entries_before,
        "the real home gained or lost entries during a fixture run: {} then {}",
        entries_before, entries_after
    );
    assert_eq!(
        sessions_after, sessions_before,
        "the live tmux session count changed during a fixture run: {} then {}",
        sessions_before, sessions_after
    );

    // The positive half. Zero mutation of the real home is only meaningful
    // alongside proof that the run wrote SOMEWHERE, or a run that did nothing
    // at all would satisfy the two assertions above.
    assert!(
        !home.calls().is_empty(),
        "the fixture run recorded no calls, so `zero mutation` above is \
         satisfied by a run that did nothing"
    );
    assert_eq!(
        home.cfg_setting("status.showUntrackedFiles").as_deref(),
        Some("no"),
        "the fixture run did not write its git setting, so the writes went \
         somewhere this test did not look"
    );
}
