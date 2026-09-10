//! Drives `tmux-tools name-windows` against a throwaway tmux server.
//!
//! A dedicated socket, not the developer's server: the suite already learned
//! that spawning sessions on the default server is what made the shell tmux
//! tests flaky on a developer machine.

mod support;

use std::process::Command;

use support::Server;

/// A window in a repository directory gets named "repo/branch".
#[test]
fn it_names_a_repository_window() {
    let server = Server::new("name-windows");
    let repo = tempfile::tempdir().expect("tempdir");
    // Fixture git calls clear git's ambient environment; see
    // .agents/PAPERCUTS.md for what happens otherwise.
    for arguments in [
        vec!["init", "-q", "-b", "main"],
        vec!["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "first"],
    ] {
        let status = Command::new("git")
            .args(&arguments)
            .current_dir(repo.path())
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_PREFIX")
            .status()
            .expect("git runs");
        assert!(status.success());
    }

    server.new_session("probe", repo.path());

    let binary = env!("CARGO_BIN_EXE_tmux-tools");
    let run = Command::new(binary)
        .args(["name-windows", "-s", "probe"])
        .env("TMUX_TOOLS_SOCKET", server.socket())
        .env("TMUX_TMPDIR", server.socket_dir())
        .output()
        .expect("the binary runs");
    assert!(run.status.success(), "stderr: {}", String::from_utf8_lossy(&run.stderr));

    let names = server.stdout(&["list-windows", "-t", "probe", "-F", "#{window_name}"]);

    // Positive control: the probe session must exist and report a window, or
    // the assertion below would hold for an empty listing.
    assert!(!names.trim().is_empty(), "the control must list a window");

    let expected = repo.path().file_name().expect("a name").to_string_lossy();
    assert!(
        names.contains(&format!("{expected}/main")),
        "expected a repo/branch name, got {names:?}"
    );

    server.shutdown();
}
