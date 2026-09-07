//! Drives `tmux-tools name-windows` against a throwaway tmux server.
//!
//! A dedicated socket, not the developer's server: the suite already learned
//! that spawning sessions on the default server is what made the shell tmux
//! tests flaky on a developer machine.

use std::process::Command;

fn tmux(socket: &str, arguments: &[&str]) -> std::process::Output {
    Command::new("tmux")
        .args(["-L", socket])
        .args(arguments)
        .output()
        .expect("tmux runs")
}

/// A window in a repository directory gets named "repo/branch".
#[test]
fn it_names_a_repository_window() {
    let socket = format!("tmux-tools-test-{}", std::process::id());
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

    tmux(&socket, &["new-session", "-d", "-s", "probe", "-c", repo.path().to_str().expect("utf-8")]);

    let binary = env!("CARGO_BIN_EXE_tmux-tools");
    let run = Command::new(binary)
        .args(["name-windows", "-s", "probe"])
        .env("TMUX_TOOLS_SOCKET", &socket)
        .output()
        .expect("the binary runs");
    assert!(run.status.success(), "stderr: {}", String::from_utf8_lossy(&run.stderr));

    let listed = tmux(&socket, &["list-windows", "-t", "probe", "-F", "#{window_name}"]);
    let names = String::from_utf8_lossy(&listed.stdout);

    // Positive control: the probe session must exist and report a window, or
    // the assertion below would hold for an empty listing.
    assert!(!names.trim().is_empty(), "the control must list a window");

    let expected = repo.path().file_name().expect("a name").to_string_lossy();
    assert!(
        names.contains(&format!("{expected}/main")),
        "expected a repo/branch name, got {names:?}"
    );

    tmux(&socket, &["kill-server"]);
}
