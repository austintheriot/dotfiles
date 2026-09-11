//! Integration tests for `.scripts/tmux-start.sh`, converted from
//! `tests/tmux-start.test.sh`.
//!
//! The script is sourced from zsh in real use (`alias s`), never executed:
//! its early `return` statements need a calling shell to return to. So it
//! is sourced from zsh here too, and `$TMUX` is cleared because the script
//! bails out immediately when it is set.
//!
//! What this suite is about: the script decides "does this session already
//! exist" and then prints a name for the alias to attach to. Those two
//! answers have to agree. When the existence test says yes about a session
//! that is not there, the alias attaches to nothing and the user gets an
//! error from tmux about a session they did not name.
//!
//! tmux target matching is the whole subtlety, and it is not obvious:
//!
//!     -t dev        matches dev-tool     (prefix match)
//!     -t '=dev'     matches only dev     (exact match)
//!
//! So `has-session -t "$name"` is NOT sufficient on its own, which is worth
//! stating because it looks sufficient. Measured on tmux 3.4: with only
//! `dev-tool` running, `has-session -t dev` succeeds and
//! `has-session -t =dev` fails.

mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use support::{Server, repo_root};

/// The tracked script under test.
fn script_path() -> PathBuf {
    repo_root().join(".scripts/tmux-start.sh")
}

/// A `tmux` on `PATH` that redirects every call to the throwaway server.
///
/// The script calls a bare `tmux`, with no socket seam of its own, so the
/// seam has to be `PATH`. Without it the script would create sessions on
/// the developer's live server, which is what made the shell tmux suites
/// flaky in the first place.
struct TmuxShim {
    directory: tempfile::TempDir,
}

impl TmuxShim {
    /// Writes a shim that prepends `-L <socket>` and sets `TMUX_TMPDIR`.
    fn new(server: &Server) -> Self {
        let directory = tempfile::Builder::new()
            .prefix("tt-shim-")
            .tempdir_in("/tmp")
            .expect("a shim directory");
        let shim = directory.path().join("tmux");
        let real = which_tmux();
        std::fs::write(
            &shim,
            format!(
                "#!/bin/sh\nexport TMUX_TMPDIR='{}'\nexec '{}' -L '{}' \"$@\"\n",
                server.socket_dir().display(),
                real.display(),
                server.socket()
            ),
        )
        .expect("the shim is written");
        set_executable(&shim);
        TmuxShim { directory }
    }

    /// Sources the script from zsh with `$TMUX` cleared, the way the alias
    /// reaches it from a bare shell, and returns the run.
    ///
    /// stdout is the contract: the alias attaches to whatever is printed,
    /// so it is captured separately from stderr.
    fn start(&self, argument: Option<&str>) -> Output {
        let mut command = Command::new("zsh");
        command
            .args(["-c", "source \"$1\" \"${2-}\"", "zsh"])
            .arg(script_path());
        if let Some(argument) = argument {
            command.arg(argument);
        }
        command
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.directory.path().display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .env_remove("TMUX")
            .output()
            .expect("zsh runs")
    }
}

/// The real `tmux` the shim execs, resolved before `PATH` is shadowed.
fn which_tmux() -> PathBuf {
    let run = Command::new("sh")
        .args(["-c", "command -v tmux"])
        .output()
        .expect("command -v runs");
    PathBuf::from(String::from_utf8_lossy(&run.stdout).trim())
}

/// Sets the owner-execute bit so `PATH` lookup can run the shim.
fn set_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)
        .expect("the shim exists")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("the shim is executable");
}

/// Whether a session of exactly this name exists, by EXACT match.
fn session_exists(server: &Server, name: &str) -> bool {
    server
        .tmux(&["has-session", "-t", &format!("={name}")])
        .status
        .success()
}

/// What the script printed on stdout, trimmed.
fn printed(run: &Output) -> String {
    String::from_utf8_lossy(&run.stdout).trim().to_string()
}

/// A named session that does not exist yet is created, and its name is
/// printed for the alias to attach to.
#[test]
fn a_new_named_session_is_created_and_printed() {
    let server = Server::new("start-fresh");
    // A server has to exist before the shim's calls, or the first
    // has-session would start one outside this fixture's directory.
    server.tmux(&["new-session", "-d", "-s", "anchor"]);
    let shim = TmuxShim::new(&server);

    assert!(
        !session_exists(&server, "fresh"),
        "the control must start with no session named fresh"
    );

    let run = shim.start(Some("fresh"));
    assert_eq!(
        printed(&run),
        "fresh",
        "a new named session is printed for the alias to attach to"
    );
    assert!(
        session_exists(&server, "fresh"),
        "the new named session must actually be created"
    );

    server.shutdown();
}

/// An existing session is printed unchanged and is not re-split.
///
/// Re-running must not create a second session or fail. The printed name is
/// the same either way, so the observable difference is the window count: a
/// second `new-session -d` on a live name errors, and re-running the layout
/// split would add panes to a session the user has already arranged.
#[test]
fn an_existing_session_is_reused_rather_than_recreated() {
    let server = Server::new("start-reuse");
    server.tmux(&["new-session", "-d", "-s", "anchor"]);
    let shim = TmuxShim::new(&server);

    shim.start(Some("reuse"));
    assert!(
        session_exists(&server, "reuse"),
        "the control must have created the session on the first run"
    );

    let windows_before = server
        .stdout(&["list-windows", "-t", "=reuse", "-F", "#{window_id}"])
        .lines()
        .count();
    assert!(windows_before > 0, "the control must find at least one window");

    let run = shim.start(Some("reuse"));
    let windows_after = server
        .stdout(&["list-windows", "-t", "=reuse", "-F", "#{window_id}"])
        .lines()
        .count();

    assert_eq!(
        printed(&run),
        "reuse",
        "an existing session is printed unchanged"
    );
    assert_eq!(
        windows_before, windows_after,
        "an existing session must not be re-split"
    );

    server.shutdown();
}

/// A session whose name is a PREFIX of a live one is still created.
///
/// The bug this suite exists for. With `dev-tool` running, `s dev` used to
/// find it (`tmux ls | rg dev` matches the line, and `has-session -t dev`
/// matches by prefix), skip creation, and print `dev`, so the alias then
/// ran `tmux attach -t dev` against a session that was never created.
#[test]
fn a_name_that_is_a_prefix_of_a_live_session_is_still_created() {
    let server = Server::new("start-prefix");
    server.tmux(&["new-session", "-d", "-s", "anchor"]);
    let shim = TmuxShim::new(&server);

    server.tmux(&["new-session", "-d", "-s", "dev-tool"]);
    // Positive control: the prefix-matching hazard only exists while the
    // longer session is live, so a test that lost it would pass trivially.
    assert!(
        session_exists(&server, "dev-tool"),
        "the control must have the longer session running"
    );

    let run = shim.start(Some("dev"));
    assert_eq!(
        printed(&run),
        "dev",
        "a session named as a prefix of a live one is printed"
    );
    assert!(
        session_exists(&server, "dev"),
        "a session named as a prefix of a live one must actually be created"
    );
    assert!(
        session_exists(&server, "dev-tool"),
        "the longer session it is a prefix of must be left alone"
    );

    server.shutdown();
}

/// A dash-leading name produces no ripgrep diagnostic.
///
/// `tmux ls | rg $SESSION_NAME` was unquoted, so a name beginning with a
/// dash was read by ripgrep as a flag rather than as a pattern. Measured
/// against the old form with a dash-leading name: `rg: unrecognized flag
/// --`. A name like that is not one anyone types on purpose, but the
/// failure the user saw came from a tool they never invoked and named a
/// flag they never passed, which is the worst shape a diagnostic can take.
///
/// tmux itself restricts session names, so this does not assert the session
/// is created. Whatever tmux decides is tmux's answer to give.
#[test]
fn a_dash_leading_name_produces_no_ripgrep_diagnostic() {
    let server = Server::new("start-dashed");
    server.tmux(&["new-session", "-d", "-s", "anchor"]);
    let shim = TmuxShim::new(&server);

    let run = shim.start(Some("-dashed"));
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        !said.contains("rg:"),
        "a dash-leading name must produce no ripgrep diagnostic, got {said:?}"
    );

    server.shutdown();
}

/// The default (no-argument) path tests for an EXACT session name.
///
/// `s` with no argument uses the fixed name `zsh`. That path had the same
/// prefix-matching bug in a quieter form: `has-session -t zsh` matches
/// `zsh-other`, so a user with any zsh-prefixed session running would have
/// creation skipped and then attach to a `zsh` that does not exist.
///
/// Not run against the real `zsh` session, which would touch the
/// developer's own layout. The assertion is on the source text instead: the
/// exact-match spelling is what makes the difference, and it is cheap to
/// pin.
#[test]
fn the_default_path_tests_for_an_exact_session_name() {
    let source = std::fs::read_to_string(script_path()).expect("the script is readable");
    assert!(
        source.contains("has-session -t '=zsh'"),
        "the default path must test for an exact session name"
    );
}

/// No substring session test survives in the script.
///
/// The mechanism, not just its symptoms: `tmux ls` piped to a matcher is
/// the shape that cannot answer "does a session with exactly this name
/// exist", so its absence is the assertion.
///
/// Comment lines are stripped first. The fix's own comment explains the old
/// `tmux ls` form, so a match over the raw file finds the explanation and
/// reports the bug as still present, which is what happened when this
/// assertion was written with a `\s` class that BRE does not support.
#[test]
fn no_substring_session_test_survives() {
    let source = std::fs::read_to_string(script_path()).expect("the script is readable");

    // Positive control: stripping comments must not strip the whole file,
    // or the absence assertion below would hold for an empty string.
    let code: String = source
        .lines()
        .map(|line| line.split('#').next().unwrap_or(""))
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        code.contains("has-session"),
        "the control must find the script's own code after stripping comments"
    );

    assert!(
        !code.contains("tmux ls"),
        "the script must not decide session existence by matching tmux ls output, got {code:?}"
    );
}
