//! `setup.sh`, the clone half of the bootstrap.
//!
//! `setup.sh` is the curl-piped entry point: it clones the bare repo, picks
//! the branch, checks out, and then hands off to `config init`. Every test
//! here drives it against a local seed repo and a fixture `$HOME`, so nothing
//! reaches the network or the real `~/.cfg`.
//!
//! The handoff target is stubbed. `config init` has its own suite
//! (`config_init.rs`); what matters here is that `setup.sh` reaches it with
//! the right flags, and that everything before the handoff is correct.
//!
//! Converted whole from `tests/setup.test.sh`, which ran **62** assertions:
//! 61 passing plus one skip, measured by running the suite rather than by
//! counting `assert_` call sites. The plan's stated 61 omitted the skip.
//!
//! Two properties the shell suite had that this file must not lose:
//!
//! - **Text assertions read the code, not the prose.** `setup.sh` documents
//!   the removed platform-to-branch mapping in a comment, so a grep for that
//!   mapping matches the comment recording its removal. Every source-text
//!   assertion here strips comments first.
//! - **The interactive handoff stays unasserted, loudly.** The shell suite
//!   called `skip` with a stated reason rather than leaving a silent gap: two
//!   pty attempts regressed into an EOF and a hang. That skip is preserved.

use dotfiles_test_support::repo::root as repo_root;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use tempfile::TempDir;

fn setup_script() -> PathBuf {
    repo_root().join("setup.sh")
}

/// `setup.sh` with every comment removed.
///
/// The load-bearing helper for this file's source-text assertions. The
/// script's own comments quote the removed `branch=$platform` mapping and
/// name the `work` and `home` branches, so an unstripped match finds the
/// record of the removal rather than the removal's absence.
fn setup_source_without_comments() -> String {
    strip_comments(&fs::read_to_string(setup_script()).expect("setup.sh is readable"))
}

/// Drops everything from the first `#` on each line.
///
/// Naive on purpose: a `#` inside a quoted string would be treated as a
/// comment. `setup.sh` carries none, and the alternative is a shell parser.
fn strip_comments(script: &str) -> String {
    script
        .lines()
        .map(|line| match line.find('#') {
            Some(index) => &line[..index],
            None => line,
        })
        .collect::<Vec<&str>>()
        .join("\n")
}

fn is_executable(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.permissions().mode() & 0o100 != 0)
        .unwrap_or(false)
}

fn write_executable(path: &Path, body: &str) {
    dotfiles_test_support::stub::write(path, body).expect("the stub is writable");
}

/// A git invocation with any ambient git environment cleared.
///
/// A pre-commit hook exports `GIT_DIR` and friends, and every fixture
/// `git init` would otherwise target the developer's repository. `lib.sh`
/// unset the same five variables at source time.
fn git(directory: &Path, arguments: &[&str]) {
    let mut command = Command::new("git");
    command.arg("-C").arg(directory);
    for variable in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_PREFIX",
        "GIT_OBJECT_DIRECTORY",
    ] {
        command.env_remove(variable);
    }
    let output = command.args(arguments).output().expect("git runs");
    assert!(
        output.status.success(),
        "git {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_output(arguments: &[&str]) -> Output {
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
    command.args(arguments).output().expect("git runs")
}

/// A seed repository shaped like the real one: one branch named `main`, plus
/// a `.scripts/config` tree whose `config-init` is a stub recording its flags.
///
/// One branch, deliberately. This used to build `mac` and `linux`, because
/// `setup.sh` mapped a detected platform to a branch of that name. Both refs
/// became frozen history in the 2026-09-06 collapse to `main`, so a seed that
/// still manufactured them would let a reintroduced mapping keep passing.
///
/// The second branch is named `work` as a fixture only: the real `work`
/// branch was archived as a tag on 2026-09-09, and this seed is independent
/// of it. It exists so `--branch` has something no `uname` can imply.
struct Fixture {
    directory: TempDir,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let fixture = Self { directory };
        fixture.build_seed();
        fs::create_dir_all(fixture.home()).expect("the fixture home is creatable");
        fs::write(fixture.calls_path(), "").expect("the call log is writable");
        fixture
    }

    fn root(&self) -> &Path {
        self.directory.path()
    }

    fn seed(&self) -> PathBuf {
        self.root().join("seed")
    }

    fn home(&self) -> PathBuf {
        self.root().join("home")
    }

    fn calls_path(&self) -> PathBuf {
        self.home().join(".calls")
    }

    fn calls(&self) -> String {
        fs::read_to_string(self.calls_path()).unwrap_or_default()
    }

    fn marker(&self) -> String {
        fs::read_to_string(self.home().join(".marker")).unwrap_or_default()
    }

    fn build_seed(&self) {
        let seed = self.seed();
        fs::create_dir_all(seed.join(".scripts/config")).expect("creatable");
        git(&seed, &["init", "-q", "-b", "main"]);

        let real_config_dir = repo_root().join(".scripts/config");
        for script in ["config", "usage.sh"] {
            fs::copy(
                real_config_dir.join(script),
                seed.join(".scripts/config").join(script),
            )
            .expect("the real config scripts are readable");
        }

        write_executable(
            &seed.join(".scripts/config/config-init"),
            "#!/bin/sh\n\
             # help: stub\n\
             # usage: config init\n\
             printf 'init %s\\n' \"$*\" >> \"$HOME/.calls\"\n",
        );

        fs::write(seed.join(".marker"), "main branch marker\n").expect("writable");
        self.commit_seed("main");

        git(&seed, &["checkout", "-q", "-b", "work"]);
        fs::write(seed.join(".marker"), "work branch marker\n").expect("writable");
        self.commit_seed("work");
        git(&seed, &["checkout", "-q", "main"]);
    }

    fn commit_seed(&self, message: &str) {
        let seed = self.seed();
        git(&seed, &["add", "-A"]);
        git(
            &seed,
            &[
                "-c",
                "user.email=t@t",
                "-c",
                "user.name=t",
                "commit",
                "-q",
                "-m",
                message,
            ],
        );
    }

    /// Adds tracked paths that git word-splits or C-quotes, and commits them
    /// on `main`.
    fn seed_odd_paths(&self) {
        let seed = self.seed();
        fs::create_dir_all(seed.join("my notes")).expect("creatable");
        fs::create_dir_all(seed.join("café")).expect("creatable");
        fs::write(seed.join("my notes/marker.txt"), "tracked space\n").expect("writable");
        fs::write(seed.join("café/marker.txt"), "tracked utf8\n").expect("writable");
        self.commit_seed("odd paths");
    }

    /// Runs `setup.sh` against this fixture, with stdin detached so the run is
    /// non-interactive, which is the `curl ... | sh` condition.
    fn run(&self, arguments: &[&str]) -> Run {
        self.run_with(arguments, |_| {})
    }

    fn run_with(&self, arguments: &[&str], configure: impl FnOnce(&mut Command)) -> Run {
        let mut command = Command::new(setup_script());
        command
            .args(arguments)
            .env("HOME", self.home())
            .env("DOTFILES_PLATFORM", "mac")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_PREFIX")
            .env_remove("GIT_OBJECT_DIRECTORY")
            .stdin(Stdio::null());
        configure(&mut command);
        // Through the shared runner: this spawn races the stubs the fixture
        // just wrote, and a lost race is ETXTBSY on exec rather than anything
        // this suite means to assert.
        let output = dotfiles_test_support::stub::run(&mut command).expect("setup.sh runs");
        Run {
            status: output.status.code().unwrap_or(-1),
            text: format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        }
    }

    /// Runs against the local seed with `--yes`, the shape most tests need.
    fn run_yes(&self) -> Run {
        let seed = self.seed();
        self.run(&["--yes", "--repo", seed.to_str().expect("utf-8 path")])
    }

    /// The single `.dotfiles-backup-*` directory in the fixture home.
    fn backup_directory(&self) -> Option<PathBuf> {
        let mut found: Vec<PathBuf> = fs::read_dir(self.home())
            .expect("the fixture home is readable")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_dir()
                    && path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with(".dotfiles-backup-"))
            })
            .collect();
        found.sort();
        found.into_iter().next()
    }
}

struct Run {
    status: i32,
    text: String,
}

// --- a clean clone ----------------------------------------------------------

#[test]
fn setup_sh_exists_and_is_executable() {
    assert!(
        is_executable(&setup_script()),
        "setup.sh is executable at {}",
        setup_script().display()
    );
}

#[test]
fn a_clean_run_clones_bare_checks_out_and_hands_off() {
    let fixture = Fixture::new();
    let run = fixture.run_yes();

    assert_eq!(
        run.status, 0,
        "setup.sh --yes exits 0 against a local seed: {}",
        run.text
    );
    assert!(
        fixture.home().join(".cfg").is_dir(),
        "the bare repo lands at ~/.cfg"
    );
    assert!(
        fixture.home().join(".marker").is_file(),
        "the worktree is checked out into $HOME"
    );
    assert_eq!(
        fixture.marker().trim(),
        "main branch marker",
        "the default branch is what got checked out"
    );
    assert!(
        fixture.calls().contains("init --yes"),
        "it hands off to config init with --yes: {}",
        fixture.calls()
    );

    // The clone must be bare, and the work tree must be $HOME. A non-bare
    // clone into ~/.cfg would put a second worktree there and track nothing
    // in $HOME.
    let bare = git_output(&[
        "--git-dir",
        fixture.home().join(".cfg").to_str().expect("utf-8 path"),
        "config",
        "--get",
        "core.bare",
    ]);
    assert_eq!(
        String::from_utf8_lossy(&bare.stdout).trim(),
        "true",
        "the clone is bare"
    );
}

// --- platform detection survives, branch mapping does not -------------------

/// Platform detection still runs, because `.zshrc-mac` versus `.zshrc-linux`
/// selection needs `DOTFILES_PLATFORM` at run time. What it no longer does is
/// name a ref, so a linux machine and a mac machine reach the same branch.
#[test]
fn every_platform_reaches_the_same_branch() {
    for platform in ["linux", "mac"] {
        let fixture = Fixture::new();
        let seed = fixture.seed();
        fixture.run_with(
            &["--yes", "--repo", seed.to_str().expect("utf-8 path")],
            |command| {
                command.env("DOTFILES_PLATFORM", platform);
            },
        );
        assert_eq!(
            fixture.marker().trim(),
            "main branch marker",
            "a {platform} machine checks out the single branch"
        );
    }
}

/// No platform may map to a branch name.
///
/// This is the assertion whose absence let the 2026-09-06 collapse ship
/// half-done: a stale local ref satisfied the old branch-existence check, so
/// nothing failed while `setup.sh` still bootstrapped every new machine onto
/// frozen history.
///
/// Read from the comment-stripped source. `setup.sh` documents the removed
/// `branch=$platform` mapping in a comment, so an unstripped match reads the
/// record of the removal as the mapping itself. The shell suite anchored to
/// a line start to dodge that; stripping is the stronger form, since a
/// comment can be indented onto a line of its own.
#[test]
fn setup_sh_maps_no_platform_to_a_branch_name() {
    let source = setup_source_without_comments();
    let matched: Vec<&str> = source
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("branch=$platform")
                || trimmed.starts_with("branch=${platform")
                || trimmed.starts_with("branch=\"$platform")
        })
        .collect();
    assert!(
        matched.is_empty(),
        "setup.sh maps no platform to a branch name, found: {matched:?}"
    );
}

/// An explicit `--branch` overrides the default, which is what makes a
/// non-default branch reachable at all: those names are not derivable from
/// `uname`.
#[test]
fn an_explicit_branch_flag_overrides_the_default() {
    let fixture = Fixture::new();
    let seed = fixture.seed();
    fixture.run(&[
        "--yes",
        "--repo",
        seed.to_str().expect("utf-8 path"),
        "--branch",
        "work",
    ]);
    assert_eq!(
        fixture.marker().trim(),
        "work branch marker",
        "--branch overrides the default branch"
    );
}

/// An unrecognized platform must no longer block the clone.
///
/// It used to exit 1, because the platform named the branch and guessing one
/// for an unknown system would check out Homebrew paths onto a machine with
/// no Homebrew. The branch no longer depends on the platform, so the clone
/// half has nothing left to refuse.
///
/// What happens after the clone is deliberately NOT asserted: nothing
/// downstream reports an unknown platform today, so an unrecognized OS gets a
/// successful clone and a silent variant miss. That is a gap, tracked
/// separately rather than papered over here.
#[test]
fn an_unrecognized_platform_still_clones() {
    let fixture = Fixture::new();
    let seed = fixture.seed();
    let run = fixture.run_with(
        &["--yes", "--repo", seed.to_str().expect("utf-8 path")],
        |command| {
            command.env("DOTFILES_PLATFORM", "unknown");
        },
    );

    assert_eq!(
        run.status, 0,
        "an unrecognized platform still clones: {}",
        run.text
    );
    assert_eq!(
        fixture.marker().trim(),
        "main branch marker",
        "it checks out the single branch anyway"
    );
    assert!(
        fixture.calls().contains("init --yes"),
        "it still hands off to config init"
    );
}

// --- pre-existing files -----------------------------------------------------

/// The failure that makes a naive bootstrap script dangerous.
///
/// A fresh machine usually already has a `.zshrc`, and `git checkout` refuses
/// to overwrite it. Losing the user's file is not an option, and neither is
/// aborting with git's raw message, so the file is backed up and the checkout
/// retried.
#[test]
fn a_colliding_file_is_backed_up_and_the_checkout_retried() {
    let fixture = Fixture::new();
    fs::write(fixture.home().join(".marker"), "the user own zshrc\n").expect("writable");
    let run = fixture.run_yes();

    assert_eq!(
        run.status, 0,
        "a colliding file does not fail the bootstrap: {}",
        run.text
    );
    assert_eq!(
        fixture.marker().trim(),
        "main branch marker",
        "the tracked version wins in $HOME"
    );

    let backup = fixture
        .backup_directory()
        .expect("a timestamped backup directory is created");
    assert_eq!(
        fs::read_to_string(backup.join(".marker"))
            .unwrap_or_default()
            .trim(),
        "the user own zshrc",
        "the backup holds the original content under its original name"
    );

    // The backup must not be inside the checkout it is protecting, or the
    // next `config status` reports it and the next checkout collides with it
    // too.
    let backup_name = backup
        .file_name()
        .and_then(|name| name.to_str())
        .expect("utf-8 name");
    let tracked = git_output(&[
        "--git-dir",
        fixture.home().join(".cfg").to_str().expect("utf-8 path"),
        "--work-tree",
        fixture.home().to_str().expect("utf-8 path"),
        "ls-files",
        backup_name,
    ]);
    assert!(
        String::from_utf8_lossy(&tracked.stdout).trim().is_empty(),
        "the backup is not itself tracked"
    );
    assert!(
        run.text.contains("marker"),
        "the output says a file was moved aside: {}",
        run.text
    );
}

/// Colliding paths that git word-splits or C-quotes.
///
/// The move-aside loop was `for path in $(cfg ls-tree -r --name-only ...)`,
/// which word-splits on whitespace and receives git's C-quoted form for any
/// non-ASCII byte. A tracked `my notes/marker.txt` became two words that
/// named nothing, `café/` arrived as `"caf\303\251/"`, neither was moved, and
/// the retried checkout then failed under `set -e` -- the exact failure the
/// loop exists to prevent. No tracked path carries either today, which is why
/// this never fired, and why it is a test rather than an incident.
#[test]
fn colliding_paths_with_a_space_or_a_non_ascii_byte_are_moved_aside() {
    let fixture = Fixture::new();
    fixture.seed_odd_paths();

    let home = fixture.home();
    fs::create_dir_all(home.join("my notes")).expect("creatable");
    fs::create_dir_all(home.join("café")).expect("creatable");
    fs::write(home.join("my notes/marker.txt"), "user space\n").expect("writable");
    fs::write(home.join("café/marker.txt"), "user utf8\n").expect("writable");

    let run = fixture.run_yes();
    assert_eq!(
        run.status, 0,
        "colliding paths with a space and a non-ASCII byte do not fail the bootstrap: {}",
        run.text
    );
    assert_eq!(
        fs::read_to_string(home.join("my notes/marker.txt"))
            .unwrap_or_default()
            .trim(),
        "tracked space",
        "the tracked file wins at the path with a space"
    );
    assert_eq!(
        fs::read_to_string(home.join("café/marker.txt"))
            .unwrap_or_default()
            .trim(),
        "tracked utf8",
        "the tracked file wins at the non-ASCII path"
    );

    let backup = fixture
        .backup_directory()
        .expect("a backup directory is created for the odd paths");
    assert_eq!(
        fs::read_to_string(backup.join("my notes/marker.txt"))
            .unwrap_or_default()
            .trim(),
        "user space",
        "the backup keeps the original at the path with a space"
    );
    assert_eq!(
        fs::read_to_string(backup.join("café/marker.txt"))
            .unwrap_or_default()
            .trim(),
        "user utf8",
        "the backup keeps the original at the non-ASCII path"
    );
}

// --- the fossil branches are no longer advertised ---------------------------

/// `home`, `home-mac` and `work` last moved in 2023 and differed from `main`
/// in nearly every tracked file, yet `--branch work` would check one out onto
/// a fresh machine and the usage text named them as the reason the flag
/// exists. Decided 2026-09-09: archived as tags `archive/<name>`, branches
/// deleted. `--branch` itself stays; what must not survive is the text
/// steering a reader at a 2023 tree.
///
/// **Deliberately NOT comment-stripped**, and this is the one place in this
/// file where stripping would be the bug. `setup.sh`'s usage block IS a
/// comment: `print_usage` sed-extracts the `# usage:` lines and prints them,
/// so the prose a reader sees is the same prose a comment-stripper deletes.
/// Confirmed by sabotage during this conversion: a version of this test that
/// stripped first let a reintroduced "reach `work` or `home`" through the
/// usage block untouched.
///
/// So the assertion is made against the rendered `--help` output as well as
/// the raw source. The rendered check is the stronger one, because it reads
/// exactly what the reader reads, and the raw check catches advertising that
/// lives outside the extracted block.
#[test]
fn setup_sh_no_longer_advertises_the_fossil_branches() {
    let fixture = Fixture::new();
    let rendered = fixture.run(&["--help"]).text;
    let raw = fs::read_to_string(setup_script()).expect("setup.sh is readable");

    // Positive control before the narrow claim: both texts must carry the
    // flag whose advertising is under test, or an empty read would satisfy
    // the claim without asserting anything.
    assert!(
        rendered.contains("--branch"),
        "the rendered usage still documents --branch: {rendered}"
    );
    assert!(raw.contains("--branch"), "the source still carries --branch");

    for (label, text) in [("the rendered usage", &rendered), ("setup.sh", &raw)] {
        let advertised: Vec<&str> = text.lines().filter(|line| advertises(line)).collect();
        assert!(
            advertised.is_empty(),
            "{label} no longer advertises the fossil branches, found: {advertised:?}"
        );
    }
}

/// Whether a line steers the reader at a fossil branch.
///
/// Adjacency is the whole test. `setup.sh` legitimately says "Reaches any
/// feature branch; the 2023 `work` and `home` trees are archived as tags",
/// which names both branches while recording their removal, so a line that
/// merely contains a verb and a name somewhere is not evidence. What must not
/// come back is a verb whose OBJECT is one of those names, which is what
/// "reach `work`" or "checks out home" reads as.
///
/// The shell suite expressed the same adjacency as
/// `reach(es|ing)? \`?(work|home)\`?`. This adds the checkout verbs and the
/// `or` in "reach `work` or `home`", which that regex would have missed on
/// the second name.
fn advertises(line: &str) -> bool {
    let lowered = line.to_lowercase();
    let words: Vec<String> = lowered
        .split_whitespace()
        .map(|word| word.trim_matches(|character: char| !character.is_alphanumeric()))
        .map(str::to_owned)
        .collect();

    words.windows(2).any(|pair| {
        let is_verb = ["reach", "reaches", "reaching"].contains(&pair[0].as_str())
            || pair[0] == "out"
            || pair[0] == "or";
        is_verb && (pair[1] == "work" || pair[1] == "home")
    })
}

// --- refusing to clobber an existing setup ----------------------------------

/// Re-running the clone half on a machine that already has `~/.cfg` must not
/// re-clone over it. That directory is the repo; blowing it away would take
/// any unpushed commit with it.
#[test]
fn a_second_run_refuses_and_points_at_a_runnable_command() {
    let fixture = Fixture::new();
    fixture.run_yes();
    fs::write(fixture.calls_path(), "").expect("writable");

    let run = fixture.run_yes();
    assert_eq!(
        run.status, 1,
        "a second run exits non-zero rather than re-cloning: {}",
        run.text
    );
    assert!(run.text.contains(".cfg"), "it names the existing repo");

    // The advice must be runnable in the reader's CURRENT shell. Reported
    // from a bare container: the message said "run `config init`" and the
    // next line was "bash: config: command not found", because install-hooks
    // symlinks config into ~/.local/bin, which is absent from a default PATH
    // -- and the PATH that `config init` exports does not reach an
    // interactive shell that already existed. So the suggestion has to spell
    // a path, not a bare command name.
    assert!(
        run.text.contains("config init"),
        "it points at config init as the way forward"
    );
    let runnable = run.text.lines().any(|line| {
        (line.contains("$HOME") || line.contains('~') || line.contains('/'))
            && line.contains(".scripts/config/config-init")
            || line.contains(".local/bin/config init")
    });
    assert!(
        runnable,
        "the suggestion is runnable without config on PATH: {}",
        run.text
    );
}

// --- dry run ----------------------------------------------------------------

#[test]
fn a_dry_run_prints_the_branch_and_changes_nothing() {
    let fixture = Fixture::new();
    let seed = fixture.seed();
    let run = fixture.run(&["--dry-run", "--repo", seed.to_str().expect("utf-8 path")]);

    assert_eq!(run.status, 0, "setup.sh --dry-run exits 0: {}", run.text);
    assert!(
        !fixture.home().join(".cfg").exists(),
        "dry run creates no repo"
    );
    assert!(
        !fixture.home().join(".marker").exists(),
        "dry run checks out nothing"
    );
    assert_eq!(fixture.calls(), "", "dry run runs no handoff");
    assert!(
        run.text.contains("main"),
        "dry run names the branch it would use: {}",
        run.text
    );
}

// --- usage ------------------------------------------------------------------

#[test]
fn help_exits_zero_and_prints_the_usage_block() {
    let fixture = Fixture::new();
    let run = fixture.run(&["--help"]);
    assert_eq!(run.status, 0, "setup.sh --help exits 0");
    assert!(
        run.text.contains("usage: setup.sh"),
        "the usage block prints: {}",
        run.text
    );
}

#[test]
fn an_unknown_flag_exits_two_and_clones_nothing() {
    let fixture = Fixture::new();
    let run = fixture.run(&["--not-a-flag"]);
    assert_eq!(run.status, 2, "an unknown flag exits 2: {}", run.text);
    assert!(
        !fixture.home().join(".cfg").exists(),
        "an unknown flag clones nothing"
    );
}

// --- POSIX sh ---------------------------------------------------------------

/// It is fetched by curl and piped to `sh`, so on Debian and Ubuntu it runs
/// under dash. A bashism here fails on exactly the remote-server case the
/// script exists for.
///
/// **A parse check is not an execution check.** `dash -n` accepts `[[ ]]`,
/// because dash parses `[[` as a command word, which is how
/// `profile-path.test.sh` failed to detect a bashism at all. So the parse
/// check is kept (it catches syntax dash cannot read) and a text check for
/// the bashisms dash accepts-and-then-fails-on is added beside it.
#[test]
fn setup_sh_stays_posix_sh() {
    let script = setup_script();
    let head = fs::read_to_string(&script).expect("setup.sh is readable");
    assert!(
        head.starts_with("#!/bin/sh"),
        "the shebang is /bin/sh: {:?}",
        head.lines().next()
    );

    let source = setup_source_without_comments();
    let bashisms: Vec<&str> = source
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("[[ ")
                || trimmed.contains(" [[ ")
                || trimmed.contains("<<<")
                || trimmed.starts_with("function ")
                || trimmed.contains("${BASH_")
                || trimmed.contains("declare -")
                || trimmed.contains("local -")
        })
        .collect();
    assert!(
        bashisms.is_empty(),
        "setup.sh carries no bashism dash would accept and then fail on: {bashisms:?}"
    );

    match Command::new("dash").arg("-n").arg(&script).output() {
        Ok(output) => assert!(
            output.status.success(),
            "setup.sh parses under dash: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
        Err(_) => dotfiles_test_support::skip("dash is not installed"),
    }
}

// --- an unreachable remote --------------------------------------------------

/// Reported from a real run: on a box with no DNS, `curl | sh` died with
/// "Could not resolve host" before `setup.sh` ever started, and once the
/// script was copied over by hand the clone failed with git's own message.
/// Git's wording names the URL, not the cause.
///
/// Driven with a URL whose host cannot resolve, not by breaking DNS: the
/// suite must not depend on network state, and `.test` domains are reserved
/// by RFC 2606 precisely so they never resolve.
#[test]
fn an_unreachable_remote_fails_cleanly_and_leaves_nothing_behind() {
    let fixture = Fixture::new();
    let run = fixture.run(&[
        "--yes",
        "--repo",
        "https://nonexistent.invalid.test/dotfiles.git",
    ]);

    assert_eq!(
        run.status, 1,
        "an unreachable remote exits non-zero: {}",
        run.text
    );
    let lowered = run.text.to_lowercase();
    assert!(
        ["reach", "resolve", "network", "connect"]
            .iter()
            .any(|word| lowered.contains(word)),
        "the failure names the remote as unreachable: {}",
        run.text
    );

    // Nothing half-created. A ~/.cfg left behind by a failed clone would make
    // the next run refuse with "already cloned", which is the worst possible
    // outcome: the reader fixes their DNS and then cannot re-run the script.
    assert!(
        !fixture.home().join(".cfg").exists(),
        "a failed clone leaves no ~/.cfg behind"
    );
    assert_eq!(fixture.calls(), "", "a failed clone runs no handoff");
}

/// A local path must not be probed for reachability. The tests and the
/// container both clone from a path or a bare repo on disk, and treating
/// those as unreachable would break every other assertion in this file.
#[test]
fn a_local_repo_path_is_never_probed_as_a_network_host() {
    let fixture = Fixture::new();
    let run = fixture.run_yes();
    assert_eq!(
        run.status, 0,
        "a local repo path is never probed as a network host: {}",
        run.text
    );
}

// --- a piped run is unattended all the way through --------------------------

/// The curl one-liner needed `sh -s -- --yes`, and the `--yes` was doing work
/// the script can determine for itself: a piped run has stdin bound to the
/// pipe, so there is no terminal to answer any prompt.
///
/// `setup.sh` already skipped its OWN branch prompt on that basis, but handed
/// off to `config init` with no arguments, which then prompts per dependency
/// install with nobody there. So the flag was not cosmetic: without it a
/// piped run stalled or silently declined every install.
#[test]
fn a_non_interactive_run_hands_off_unattended() {
    let fixture = Fixture::new();
    let seed = fixture.seed();
    // No `--yes`. Stdin is null, which is what makes it not a terminal, the
    // same condition `curl ... | sh` creates.
    fixture.run(&["--repo", seed.to_str().expect("utf-8 path")]);

    assert!(
        fixture.calls().contains("init --yes"),
        "a non-interactive run hands off unattended: {}",
        fixture.calls()
    );
    assert_eq!(
        fixture.marker().trim(),
        "main branch marker",
        "a non-interactive run still checks out"
    );
}

/// An explicit `--yes` stays equivalent, so every existing caller (Docker,
/// the CI bootstrap job, the documented one-liner) keeps working unchanged.
#[test]
fn an_explicit_yes_behaves_the_same_way() {
    let fixture = Fixture::new();
    fixture.run_yes();
    assert!(
        fixture.calls().contains("init --yes"),
        "an explicit --yes behaves the same way: {}",
        fixture.calls()
    );
}

/// The reverse must not regress: with a terminal and no `--yes`, the handoff
/// stays interactive so `config init` can prompt per install.
///
/// Deliberately not driven. It needs a pty that stays open long enough to
/// answer one prompt, and two attempts made things worse rather than better:
/// `script -q /dev/null` treats a piped newline as EOF, so the run died at
/// the prompt without reaching the handoff, and a `pty.spawn` whose input
/// callback kept returning a newline fed input forever and hung the whole
/// suite. A test that hangs the gate is far worse than an honest gap.
///
/// Recorded as a counted skip rather than an omission, because a gate that
/// says nothing when it stands down is indistinguishable from a gate that is
/// not installed.
#[test]
fn an_interactive_run_does_not_force_yes() {
    dotfiles_test_support::skip(
        "an interactive run does not force --yes: needs a pty harness; \
         two attempts regressed into EOF and a hang",
    );
}

// --- installing git when it is missing --------------------------------------

/// A `PATH` holding only a stub package manager and the coreutils `setup.sh`
/// needs, so no test installs anything for real.
struct StubPath {
    directory: PathBuf,
    log: PathBuf,
}

impl StubPath {
    /// A stub `apt-get` that records its invocation and then creates a fake
    /// `git`, so the script's post-install `command -v git` succeeds and the
    /// run can continue.
    fn with_apt_get(root: &Path) -> Self {
        let stub = Self::bare(root, "nogit-bin");
        write_executable(
            &stub.directory.join("apt-get"),
            &format!(
                "#!/bin/sh\n\
                 printf 'apt-get %s\\n' \"$*\" >> {log}\n\
                 cat > {bin}/git <<'GITSTUB'\n\
                 #!/bin/sh\n\
                 exit 0\n\
                 GITSTUB\n\
                 chmod +x {bin}/git\n\
                 exit 0\n",
                log = shell_quote(&stub.log),
                bin = shell_quote(&stub.directory),
            ),
        );
        stub
    }

    /// The same `PATH` with no package manager at all.
    fn without_package_manager(root: &Path) -> Self {
        Self::bare(root, "nopm-bin")
    }

    fn bare(root: &Path, name: &str) -> Self {
        let directory = root.join(name);
        fs::create_dir_all(&directory).expect("creatable");
        for passthrough in [
            "sh", "printf", "id", "command", "test", "cat", "chmod", "mkdir", "rm", "sed", "grep",
            "uname", "dirname", "readlink", "date", "mv", "awk",
        ] {
            let Some(real) = which(passthrough) else {
                continue;
            };
            let link = directory.join(passthrough);
            let _ = fs::remove_file(&link);
            let _ = std::os::unix::fs::symlink(real, link);
        }
        let log = root.join(format!("{name}.log"));
        fs::write(&log, "").expect("writable");
        Self { directory, log }
    }

    fn contents(&self) -> String {
        fs::read_to_string(&self.log).unwrap_or_default()
    }
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(program))
        .find(|candidate| is_executable(candidate))
}

/// Reported from a bare root container: the one-liner printed the command
/// that installs git and exited, so a bare image needed two commands. The
/// detection to print that line is the same detection needed to run it, so
/// the script installs git itself.
///
/// Piped installs without asking, because the curl one-liner is ALWAYS piped:
/// refusing there would leave the bare-image case -- the case this script
/// exists for -- needing two commands forever.
#[test]
fn a_piped_run_installs_git_rather_than_only_naming_it() {
    let fixture = Fixture::new();
    let stub = StubPath::with_apt_get(fixture.root());
    let seed = fixture.seed();

    let run = fixture.run_with(
        &["--repo", seed.to_str().expect("utf-8 path")],
        |command| {
            command
                .env("PATH", &stub.directory)
                .env("DOTFILES_PLATFORM", "linux");
        },
    );

    let installed = stub.contents();
    assert!(
        !installed.is_empty(),
        "a piped run installs git rather than only naming it"
    );
    assert!(
        installed.contains("git"),
        "it installs git specifically: {installed}"
    );

    // And it must say so rather than installing silently. A one-liner that
    // mutates the system without a word is worse than one that asks.
    let lowered = run.text.to_lowercase();
    assert!(
        lowered.contains("install") || lowered.contains("git"),
        "it announces the install: {}",
        run.text
    );
}

/// No package manager at all: nothing to run, so it must still refuse with
/// the docs URL rather than pretending.
#[test]
fn with_no_package_manager_it_refuses_and_points_at_the_docs() {
    let fixture = Fixture::new();
    let stub = StubPath::without_package_manager(fixture.root());
    let seed = fixture.seed();

    let run = fixture.run_with(
        &["--repo", seed.to_str().expect("utf-8 path")],
        |command| {
            command
                .env("PATH", &stub.directory)
                .env("DOTFILES_PLATFORM", "linux");
        },
    );

    assert!(
        run.text.to_lowercase().contains("git"),
        "with no package manager it still refuses: {}",
        run.text
    );
    assert!(
        run.text.contains("git-scm.com"),
        "and points at the docs: {}",
        run.text
    );
}

/// `--dry-run` must never install git, since its whole contract is changing
/// nothing.
#[test]
fn a_dry_run_installs_no_git() {
    let fixture = Fixture::new();
    let stub = StubPath::with_apt_get(fixture.root());
    let seed = fixture.seed();

    fixture.run_with(
        &["--dry-run", "--repo", seed.to_str().expect("utf-8 path")],
        |command| {
            command
                .env("PATH", &stub.directory)
                .env("DOTFILES_PLATFORM", "linux");
        },
    );

    assert_eq!(stub.contents(), "", "a dry run installs no git");
}

// --- one URL, one branch ----------------------------------------------------

/// The README documents a single URL for macOS, Linux and WSL. That URL must
/// name the branch that actually receives commits, because it fetches both
/// the script and, through the default, the tree the script checks out.
#[test]
fn the_readme_documents_one_bootstrap_url_naming_main() {
    let readme = fs::read_to_string(repo_root().join("README.md")).expect("README.md is readable");

    let url = readme
        .split_whitespace()
        .find(|word| {
            word.starts_with("https://raw.githubusercontent.com/") && word.ends_with("/setup.sh")
        })
        .expect("the README documents a raw URL");

    // Positive control before the narrow claim: an edit that breaks the
    // split leaves the branch empty, and an empty value would otherwise make
    // a wrong branch name look like the right one.
    let branch = url
        .rsplit_once("/setup.sh")
        .and_then(|(head, _)| head.rsplit_once('/'))
        .map(|(_, branch)| branch)
        .unwrap_or_default();
    assert!(!branch.is_empty(), "the URL yields a branch name: {url}");
    assert_eq!(branch, "main", "the README bootstrap URL names main");

    // The README must not reintroduce a second per-platform URL. That is the
    // duplication this simplification removed, and a well-meaning edit adding
    // "Linux or WSL:" back would make the two copies drift.
    let url_count = readme
        .split_whitespace()
        .filter(|word| {
            word.starts_with("https://raw.githubusercontent.com/austintheriot/dotfiles/")
                && word.ends_with("/setup.sh")
        })
        .count();
    assert_eq!(
        url_count, 1,
        "the README documents exactly one bootstrap URL"
    );
}

/// Every git-backed assertion is guarded on the repository existing. The test
/// container carries a COPY of the tree with no `.cfg` at all, so an
/// unguarded `git --git-dir` there fails on the absence of a repository
/// rather than on anything this suite means to check.
#[test]
fn main_is_a_real_branch_here_and_carries_setup_sh() {
    let git_dir = repo_root().join(".cfg");
    if !git_dir.is_dir() {
        dotfiles_test_support::skip(
            "main is a real branch carrying setup.sh: no repository in this environment",
        );
        return;
    }
    let git_dir = git_dir.to_str().expect("utf-8 path");

    assert!(
        git_output(&[
            "--git-dir",
            git_dir,
            "rev-parse",
            "--verify",
            "--quiet",
            "refs/heads/main",
        ])
        .status
        .success(),
        "main is a real branch here"
    );
    assert!(
        git_output(&["--git-dir", git_dir, "cat-file", "-e", "main:setup.sh"])
            .status
            .success(),
        "main carries setup.sh"
    );
}
