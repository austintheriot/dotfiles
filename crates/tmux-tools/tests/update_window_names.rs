//! Integration tests for `tmux-tools name-windows`, converted from
//! `tests/tmux-update-window-names.test.sh`.
//!
//! Test windows run an inert command, never a shell. `.zshrc`'s precmd
//! calls the naming path on every prompt, so a shell-backed window renames
//! ITSELF a beat after creation and overwrites the name an assertion is
//! about to read. That was fixed once in `8591f242` and it presents as
//! flakiness rather than as a failure, so the property is enforced by the
//! fixture (`support::Server::new_window`) and asserted in
//! `support_fixture.rs` rather than left to each test to remember.

mod support;

use std::path::{Path, PathBuf};
use std::process::Command;

use support::Server;

/// The naming run, the way the shell script's `-w` flag reached it.
struct Namer {
    server: Server,
    /// Held only so the directory outlives the tests that use it. Every
    /// read goes through `resolved_home`.
    #[allow(dead_code, reason = "the TempDir's Drop is the point")]
    home: tempfile::TempDir,
    /// The home directory as tmux reports it.
    ///
    /// `/tmp` is a symlink to `/private/tmp` on macOS, and tmux reports a
    /// pane's cwd as the resolved path while `$HOME` keeps the symlinked
    /// one. The binary compares those two as strings, so the tilde case
    /// needs the resolved form or it names the window after the temp
    /// directory instead.
    resolved_home: PathBuf,
}

impl Namer {
    /// Starts a server with one session, and a `$HOME` of its own.
    ///
    /// The binary reads a fallback bare-repo pattern file under `$HOME`.
    /// Pointing `$HOME` at an empty directory is what makes a developer
    /// machine that carries one and the container that does not produce
    /// the same names.
    fn new(label: &str, session_directory: &Path) -> Self {
        let server = Server::new(label);
        let home = tempfile::Builder::new()
            .prefix("tt-home-")
            .tempdir_in("/tmp")
            .expect("a home directory");
        server.new_session("main", session_directory);
        let resolved_home = home.path().canonicalize().expect("the home resolves");
        Namer {
            server,
            home,
            resolved_home,
        }
    }

    /// Runs `name-windows` with the given arguments.
    fn run(&self, arguments: &[&str], extra_environment: &[(&str, &str)]) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tmux-tools"));
        command
            .arg("name-windows")
            .args(arguments)
            .env("TMUX_TOOLS_SOCKET", self.server.socket())
            .env("TMUX_TMPDIR", self.server.socket_dir())
            .env("HOME", &self.resolved_home)
            .env_remove("TMUX_PANE");
        for (name, value) in extra_environment {
            command.env(name, value);
        }
        let run = command.output().expect("the binary runs");
        assert!(
            run.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&run.stderr)
        );
    }

    /// Names one window and returns the name it ended up with.
    fn name_window(&self, window: &str) -> String {
        self.run(&["-w", window], &[]);
        self.window_name(window)
    }

    /// The window's current `#{window_name}`.
    fn window_name(&self, window: &str) -> String {
        self.server
            .stdout(&["display-message", "-p", "-t", window, "#{window_name}"])
    }

    /// Marks a window stale, so a later assertion can tell "renamed" from
    /// "was already right".
    fn make_stale(&self, window: &str) {
        self.server.tmux(&["rename-window", "-t", window, "stale"]);
        self.server
            .tmux(&["set", "-w", "-t", window, "@wname_auto", "stale"]);
    }
}

/// Creates a git repository at `path` on `branch`, and returns the path.
fn make_repo(parent: &Path, name: &str, branch: &str) -> PathBuf {
    let path = parent.join(name);
    std::fs::create_dir_all(&path).expect("the repo directory");
    for arguments in [
        vec!["init", "-q", "-b", branch],
        vec![
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
    ] {
        run_git(&path, &arguments);
    }
    path
}

/// Adds a worktree of `repo` on a new `branch`, at `parent/name`.
fn make_worktree(repo: &Path, parent: &Path, branch: &str, name: &str) -> PathBuf {
    let path = parent.join(name);
    run_git(
        repo,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            branch,
            path.to_str().expect("a utf-8 path"),
        ],
    );
    path
}

/// Runs one git command, with git's ambient environment cleared.
///
/// A caller that exports a git environment, which a pre-commit or pre-push
/// hook does, would otherwise redirect every fixture `git init` and
/// `git commit` at the dotfiles repository itself.
fn run_git(directory: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .args(arguments)
        .current_dir(directory)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_PREFIX")
        .status()
        .expect("git runs");
    assert!(status.success(), "git {arguments:?} failed in {}", directory.display());
}

/// A fixture directory under `/tmp`, short enough for a socket path.
fn fixtures() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("tt-fixtures-")
        .tempdir_in("/tmp")
        .expect("a fixture directory")
}

/// Tier 1: a non-repository directory yields its basename, and `$HOME`
/// yields a tilde.
#[test]
fn a_plain_directory_is_named_by_its_basename() {
    let fixtures = fixtures();
    let plain = fixtures.path().join("not-a-repo");
    std::fs::create_dir_all(&plain).expect("the plain directory");
    let namer = Namer::new("wname-plain", fixtures.path());

    let window = namer.server.new_window("main", &plain, &[]);
    assert_eq!(
        namer.name_window(&window),
        "not-a-repo",
        "a non-repository cwd yields its basename"
    );

    // $HOME is the namer's own, so the tilde case does not depend on the
    // developer's home being a particular shape.
    let home_window = namer
        .server
        .new_window("main", &namer.resolved_home, &[]);
    assert_eq!(
        namer.name_window(&home_window),
        "~",
        "the home directory yields a tilde"
    );

    namer.server.shutdown();
}

/// Tier 2: a git branch supersedes the cwd, slashes survive, the name
/// follows a branch change, and a detached HEAD yields a short sha.
#[test]
fn a_repository_is_named_by_repo_and_branch() {
    let fixtures = fixtures();
    let repo_main = make_repo(fixtures.path(), "repo-main", "main");
    let repo_feature = make_repo(fixtures.path(), "repo-feature", "feature/login");
    let namer = Namer::new("wname-repo", fixtures.path());

    let window = namer.server.new_window("main", &repo_main, &[]);
    assert_eq!(
        namer.name_window(&window),
        "repo-main/main",
        "a git repository yields repo/branch, not the basename"
    );

    let slashed = namer.server.new_window("main", &repo_feature, &[]);
    assert_eq!(
        namer.name_window(&slashed),
        "repo-feature/feature/login",
        "a branch name with a slash is preserved"
    );

    run_git(&repo_main, &["checkout", "-q", "-b", "renamed"]);
    assert_eq!(
        namer.name_window(&window),
        "repo-main/renamed",
        "an owned window follows branch changes"
    );

    run_git(&repo_main, &["checkout", "-q", "--detach"]);
    let short_sha = String::from_utf8_lossy(
        &Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(&repo_main)
            .env_remove("GIT_DIR")
            .output()
            .expect("git runs")
            .stdout,
    )
    .trim()
    .to_string();
    assert!(!short_sha.is_empty(), "the control must resolve a short sha");
    assert_eq!(
        namer.name_window(&window),
        format!("repo-main/({short_sha})"),
        "a detached HEAD yields a short sha"
    );

    namer.server.shutdown();
}

/// Tier 3: a manual rename supersedes everything, and an empty name hands
/// ownership back.
#[test]
fn a_manual_rename_is_not_clobbered() {
    let fixtures = fixtures();
    let repo_main = make_repo(fixtures.path(), "repo-main", "main");
    let namer = Namer::new("wname-manual", fixtures.path());

    let window = namer.server.new_window("main", &repo_main, &[]);
    assert_eq!(
        namer.name_window(&window),
        "repo-main/main",
        "the control must have taken ownership first"
    );

    namer
        .server
        .tmux(&["rename-window", "-t", &window, "MyName"]);
    assert_eq!(
        namer.name_window(&window),
        "MyName",
        "a manual rename is not clobbered"
    );

    run_git(&repo_main, &["checkout", "-q", "-b", "another"]);
    assert_eq!(
        namer.name_window(&window),
        "MyName",
        "a manual rename survives a branch change"
    );

    namer.server.tmux(&["rename-window", "-t", &window, ""]);
    assert_eq!(
        namer.name_window(&window),
        "repo-main/another",
        "an empty name reverts to automatic naming"
    );

    namer.server.shutdown();
}

/// A window created with an explicit name counts as manually named.
#[test]
fn an_explicitly_named_window_is_left_alone() {
    let fixtures = fixtures();
    let repo_main = make_repo(fixtures.path(), "repo-main", "main");
    let namer = Namer::new("wname-preset", fixtures.path());

    let window = namer
        .server
        .new_window("main", &repo_main, &["-n", "Preset"]);
    assert_eq!(
        namer.name_window(&window),
        "Preset",
        "a window created with an explicit name is left alone"
    );

    namer.server.shutdown();
}

/// `@wname_label` prefixes the automatic name, in every tier, and a manual
/// rename still beats it.
#[test]
fn a_label_prefixes_the_automatic_name() {
    let fixtures = fixtures();
    let repo_feature = make_repo(fixtures.path(), "repo-feature", "feature/login");
    let plain = fixtures.path().join("not-a-repo");
    std::fs::create_dir_all(&plain).expect("the plain directory");
    let namer = Namer::new("wname-label", fixtures.path());

    let window = namer.server.new_window("main", &repo_feature, &[]);
    namer
        .server
        .tmux(&["set", "-w", "-t", &window, "@wname_label", "Reviews"]);
    assert_eq!(
        namer.name_window(&window),
        "Reviews - repo-feature/feature/login",
        "a label prefixes the automatic name"
    );
    assert_eq!(
        namer.name_window(&window),
        "Reviews - repo-feature/feature/login",
        "a labelled window stays owned across runs"
    );

    namer
        .server
        .tmux(&["rename-window", "-t", &window, "Override"]);
    assert_eq!(
        namer.name_window(&window),
        "Override",
        "a manual rename beats the label"
    );

    namer.server.tmux(&["rename-window", "-t", &window, ""]);
    assert_eq!(
        namer.name_window(&window),
        "Reviews - repo-feature/feature/login",
        "an empty name restores the labelled name"
    );

    let plain_window = namer.server.new_window("main", &plain, &[]);
    namer
        .server
        .tmux(&["set", "-w", "-t", &plain_window, "@wname_label", "Config"]);
    assert_eq!(
        namer.name_window(&plain_window),
        "Config - not-a-repo",
        "a label prefixes a cwd-derived name"
    );

    namer.server.shutdown();
}

/// A worktree is named for the MAIN repository, not the worktree directory.
#[test]
fn a_worktree_uses_the_main_repository_name() {
    let fixtures = fixtures();
    let repo_feature = make_repo(fixtures.path(), "repo-feature", "feature/login");
    let namer = Namer::new("wname-worktree", fixtures.path());

    let window = namer.server.new_window("main", &repo_feature, &[]);
    assert_eq!(
        namer.name_window(&window),
        "repo-feature/feature/login",
        "the control must name the main repository first"
    );

    let worktree = make_worktree(&repo_feature, fixtures.path(), "side-branch", "personal-wt");
    let worktree_window = namer.server.new_window("main", &worktree, &[]);
    assert_eq!(
        namer.name_window(&worktree_window),
        "repo-feature/side-branch",
        "a worktree uses the main repo name, not the worktree directory"
    );

    let labelled = namer.server.new_window("main", &repo_feature, &[]);
    namer
        .server
        .tmux(&["set", "-w", "-t", &labelled, "@wname_label", "Side"]);
    assert_eq!(
        namer.name_window(&labelled),
        "Side - repo-feature/feature/login",
        "a label sits in front of repo/branch"
    );

    namer.server.shutdown();
}

/// A repository matching `@wname_bare_repos` is named by branch alone.
///
/// The patterns are set per window rather than relied on as a built-in
/// default. The binary carries no compiled-in list: the names it used to
/// hold now live in an untracked config file, because this repository is
/// public. So a test that asserted the default behaviour would either name
/// them here or pass only on a machine holding that file.
///
/// Setting the option is also the honest test: it exercises the same code
/// path a real user's config reaches, and it works identically in the
/// container, which has no such file.
#[test]
fn a_bare_repo_pattern_drops_the_repository_prefix() {
    const BARE_PATTERNS: &str = "work-app*|work-tool-*|vendor-*";

    let fixtures = fixtures();
    let namer = Namer::new("wname-bare", fixtures.path());

    for name in ["work-app", "work-tool-cli", "vendor-plugins"] {
        let repo = make_repo(fixtures.path(), name, "work-branch");
        let window = namer.server.new_window("main", &repo, &[]);
        namer.server.tmux(&[
            "set",
            "-w",
            "-t",
            &window,
            "@wname_bare_repos",
            BARE_PATTERNS,
        ]);
        assert_eq!(
            namer.name_window(&window),
            "work-branch",
            "{name} is named by branch alone"
        );
    }

    let work_app = fixtures.path().join("work-app");
    let worktree = make_worktree(&work_app, fixtures.path(), "wt-branch", "work-app-2");
    let worktree_window = namer.server.new_window("main", &worktree, &[]);
    namer.server.tmux(&[
        "set",
        "-w",
        "-t",
        &worktree_window,
        "@wname_bare_repos",
        BARE_PATTERNS,
    ]);
    assert_eq!(
        namer.name_window(&worktree_window),
        "wt-branch",
        "a worktree of a bare repo is named by branch alone"
    );

    let labelled = namer.server.new_window("main", &work_app, &[]);
    namer.server.tmux(&[
        "set",
        "-w",
        "-t",
        &labelled,
        "@wname_bare_repos",
        BARE_PATTERNS,
    ]);
    namer
        .server
        .tmux(&["set", "-w", "-t", &labelled, "@wname_label", "Reviews"]);
    assert_eq!(
        namer.name_window(&labelled),
        "Reviews - work-branch",
        "a bare repo with a label stays short"
    );

    namer.server.shutdown();
}

/// The bare-branch pattern is configurable, in both directions.
#[test]
fn the_bare_repo_pattern_is_configurable() {
    let fixtures = fixtures();
    let repo_feature = make_repo(fixtures.path(), "repo-feature", "feature/login");
    let work_app = make_repo(fixtures.path(), "work-app", "work-branch");
    let namer = Namer::new("wname-configurable", fixtures.path());

    let matching = namer.server.new_window("main", &repo_feature, &[]);
    namer
        .server
        .tmux(&["set", "-w", "-t", &matching, "@wname_bare_repos", "repo-*"]);
    assert_eq!(
        namer.name_window(&matching),
        "feature/login",
        "@wname_bare_repos drops the prefix for a matching repo"
    );

    let unmatched = namer.server.new_window("main", &work_app, &[]);
    namer.server.tmux(&[
        "set",
        "-w",
        "-t",
        &unmatched,
        "@wname_bare_repos",
        "nothing-*",
    ]);
    assert_eq!(
        namer.name_window(&unmatched),
        "work-app/work-branch",
        "@wname_bare_repos adds the prefix back when nothing matches"
    );

    namer.server.shutdown();
}

/// The ACTIVE pane's cwd drives the name, and switching panes updates it.
#[test]
fn the_active_pane_drives_the_name() {
    let fixtures = fixtures();
    let repo_main = make_repo(fixtures.path(), "repo-main", "main");
    let repo_feature = make_repo(fixtures.path(), "repo-feature", "feature/login");
    let namer = Namer::new("wname-active", fixtures.path());

    let window = namer.server.new_window("main", &repo_main, &[]);
    namer.server.tmux(&[
        "split-window",
        "-d",
        "-t",
        &window,
        "-c",
        repo_feature.to_str().expect("a utf-8 path"),
        "sleep",
        "86400",
    ]);
    let panes: Vec<String> = namer
        .server
        .stdout(&["list-panes", "-t", &window, "-F", "#{pane_id}"])
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(panes.len(), 2, "the control must have built two panes");

    namer.server.tmux(&["select-pane", "-t", &panes[1]]);
    assert_eq!(
        namer.name_window(&window),
        "repo-feature/feature/login",
        "the active pane's cwd drives the name"
    );

    namer.server.tmux(&["select-pane", "-t", &panes[0]]);
    assert_eq!(
        namer.name_window(&window),
        "repo-main/main",
        "switching the active pane updates the name"
    );

    namer.server.shutdown();
}

/// The four target modes: one window, one session, every session, and the
/// `$TMUX_PANE` default.
#[test]
fn each_target_mode_reaches_exactly_its_windows() {
    let fixtures = fixtures();
    let plain = fixtures.path().join("not-a-repo");
    std::fs::create_dir_all(&plain).expect("the plain directory");
    let repo_feature = make_repo(fixtures.path(), "repo-feature", "feature/login");
    let namer = Namer::new("wname-targets", fixtures.path());

    let here = namer.server.new_window("main", &plain, &[]);
    namer.server.new_session("other", &plain);
    let elsewhere = namer.server.new_window("other", &repo_feature, &[]);

    namer.make_stale(&here);
    namer.make_stale(&elsewhere);

    namer.run(&["-s", "main"], &[]);
    assert_eq!(
        namer.window_name(&here),
        "not-a-repo",
        "session mode updates the named session"
    );
    assert_eq!(
        namer.window_name(&elsewhere),
        "stale",
        "session mode leaves other sessions alone"
    );

    namer.run(&["--all"], &[]);
    assert_eq!(
        namer.window_name(&elsewhere),
        "repo-feature/feature/login",
        "all mode updates every session"
    );

    namer.make_stale(&here);
    let pane = namer
        .server
        .stdout(&["list-panes", "-t", &here, "-F", "#{pane_id}"])
        .lines()
        .next()
        .expect("a pane id")
        .to_string();
    namer.run(&[], &[("TMUX_PANE", pane.as_str())]);
    assert_eq!(
        namer.window_name(&here),
        "not-a-repo",
        "the default target is the window behind $TMUX_PANE"
    );

    namer.make_stale(&here);
    namer.run(&[], &[]);
    assert_eq!(
        namer.window_name(&here),
        "stale",
        "no target outside tmux is a no-op"
    );

    namer.server.shutdown();
}

/// Repeated runs are stable.
#[test]
fn repeated_runs_are_stable() {
    let fixtures = fixtures();
    let repo_main = make_repo(fixtures.path(), "repo-main", "main");
    let namer = Namer::new("wname-stable", fixtures.path());

    let window = namer.server.new_window("main", &repo_main, &[]);
    let first = namer.name_window(&window);
    assert_eq!(first, "repo-main/main", "the control must name it once");

    namer.run(&["-w", &window], &[]);
    namer.run(&["-w", &window], &[]);
    assert_eq!(
        namer.window_name(&window),
        first,
        "repeated runs are stable"
    );

    namer.server.shutdown();
}

/// precmd calls the binary directly, not the sh wrapper.
///
/// Measured 2026-09-09 with zsh's EPOCHREALTIME: the binary alone is 15 to
/// 22ms per prompt, while going through the sh wrapper is 25 to 36ms. The
/// wrapper's exec hop is a third of every prompt in every pane. The wrapper
/// stays for the tmux hooks and the `re` alias, which are async or manual;
/// the hot path goes direct.
///
/// Read from the precmd body only, so the alias and the hooks can keep
/// naming the wrapper without tripping this.
#[test]
fn precmd_calls_the_binary_rather_than_the_wrapper() {
    let zshrc = support::repo_root().join(".zshrc");
    let source = std::fs::read_to_string(&zshrc).expect("the .zshrc is readable");

    let body: String = source
        .lines()
        .skip_while(|line| !line.starts_with("precmd () {"))
        .take_while(|line| *line != "}")
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!body.is_empty(), "the precmd body must be found");

    assert!(
        body.contains("tmux-tools name-windows"),
        "precmd must rename windows through the binary directly, got {body:?}"
    );
    assert!(
        !body.contains("tmux-update-window-names.sh"),
        "precmd must not pay the sh wrapper hop, got {body:?}"
    );
}
