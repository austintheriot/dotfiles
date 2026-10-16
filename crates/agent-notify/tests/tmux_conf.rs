//! The tracked agent-notify tmux file loads on a throwaway server and sets
//! the hooks, the binding and the status formats.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use dotfiles_test_support::repo;
use dotfiles_test_support::stub;
use dotfiles_test_support::tmux_server::Server;

fn conf() -> PathBuf {
    repo::root().join(".config/tmux/agent-notify.conf")
}

fn loaded(label: &str) -> Server {
    let server = Server::new(label);
    let dir = tempfile::tempdir().expect("a temp dir");
    server.new_session("main", dir.path());
    let path = conf();
    let sourced = server.tmux(&["source-file", path.to_str().expect("utf-8")]);
    assert!(sourced.status.success(), "{}", String::from_utf8_lossy(&sourced.stderr));
    server
}

#[test]
fn focus_hooks_report_in_and_out_with_the_server_identity() {
    let server = loaded("an-conf-hooks");
    let hooks = server.stdout(&["show-hooks", "-gw"]);
    for direction in ["--in", "--out"] {
        let line = hooks
            .lines()
            .find(|line| line.contains("agent-notify focus-changed") && line.contains(direction))
            .unwrap_or_else(|| panic!("no focus hook with {direction} in:\n{hooks}"));
        for field in ["--socket", "#{q:socket_path}", "--pid", "#{pid}", "--start-time", "#{start_time}", "--pane", "#{pane_id}"] {
            assert!(line.contains(field), "{direction} hook lacks {field}: {line}");
        }
        assert!(line.contains("run-shell -b"), "the hook must not block tmux: {line}");
    }
}

#[test]
fn focus_hooks_need_focus_events_so_the_file_turns_them_on() {
    let server = loaded("an-conf-focus");
    assert_eq!(server.stdout(&["show-options", "-gv", "focus-events"]), "on");
}

#[test]
fn meta_n_jumps_without_blocking() {
    let server = loaded("an-conf-jump");
    let keys = server.stdout(&["list-keys", "-T", "root", "M-n"]);
    assert!(keys.contains("run-shell -b") && keys.contains("agent-notify jump"), "{keys}");
}

#[test]
fn window_formats_and_status_right_gain_the_marker_and_segment() {
    let server = loaded("an-conf-formats");
    for option in ["window-status-format", "window-status-current-format"] {
        let value = server.stdout(&["show-options", "-gv", option]);
        assert!(value.contains("@agent_notify_marker"), "{option}: {value}");
    }
    let right = server.stdout(&["show-options", "-gv", "status-right"]);
    assert!(right.contains("agent-notify status --tmux"), "{right}");
}

#[test]
fn the_marker_names_each_window_state() {
    let server = loaded("an-conf-marker");
    for (state, marker) in [("needs-you", " !"), ("failed", " x"), ("finished", " +"), ("working", " ~"), ("", "")] {
        let set = if state.is_empty() {
            server.tmux(&["set-option", "-wu", "-t", "main:", "@agent_notify_window_state"])
        } else {
            server.tmux(&["set-option", "-w", "-t", "main:", "@agent_notify_window_state", state])
        };
        assert!(set.status.success());
        let shown = server.stdout(&["display-message", "-p", "-t", "main:", "#{E:@agent_notify_marker}"]);
        assert_eq!(shown.trim_end_matches('\n'), marker, "state {state:?}");
    }
}

#[test]
fn sourcing_twice_does_not_duplicate_the_segment() {
    let server = loaded("an-conf-twice");
    let path = conf();
    let again = server.tmux(&["source-file", path.to_str().expect("utf-8")]);
    assert!(again.status.success());
    let right = server.stdout(&["show-options", "-gv", "status-right"]);
    assert_eq!(right.matches("agent-notify status --tmux").count(), 1, "{right}");
    let format = server.stdout(&["show-options", "-gv", "window-status-format"]);
    assert_eq!(format.matches("@agent_notify_marker").count(), 1, "{format}");
    let current = server.stdout(&["show-options", "-gv", "window-status-current-format"]);
    assert_eq!(current.matches("@agent_notify_marker").count(), 1, "{current}");
}

fn wait_for_lines(path: &Path) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(text) = std::fs::read_to_string(path) {
            return text.lines().map(str::to_string).collect();
        }
        assert!(Instant::now() < deadline, "the stub never wrote {}", path.display());
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn the_focus_hook_hands_a_socket_path_with_a_space_over_as_one_argument() {
    let server = Server::at_socket_in_directory_prefixed("an-conf-space", "an conf ");
    let session_directory = tempfile::tempdir().expect("a temp dir");
    server.new_session("main", session_directory.path());
    let conf = conf();
    let sourced = server.tmux(&["source-file", conf.to_str().expect("utf-8")]);
    assert!(sourced.status.success(), "{}", String::from_utf8_lossy(&sourced.stderr));

    let home = server.socket_dir();
    let argv = home.join("argv");
    let partial = home.join("argv.partial");
    let script = format!(
        "#!/bin/sh\nfor argument in \"$@\"; do printf '%s\\n' \"$argument\"; done > '{partial}'\nmv '{partial}' '{argv}'\n",
        partial = partial.display(),
        argv = argv.display()
    );
    stub::write(&home.join(".local/bin/agent-notify"), &script).expect("a stub");
    let home_text = home.to_str().expect("utf-8");
    assert!(server.tmux(&["set-environment", "-g", "HOME", home_text]).status.success());

    let hook = server.stdout(&["show-options", "-gv", "pane-focus-in[42]"]);
    assert!(hook.contains("run-shell -b"), "the hook must not block tmux: {hook}");
    let pane = server.stdout(&["display-message", "-p", "#{pane_id}"]);
    let fired = server.tmux(&["if-shell", "-F", "-t", &pane, "1", &hook]);
    assert!(fired.status.success(), "{}", String::from_utf8_lossy(&fired.stderr));

    let lines = wait_for_lines(&argv);
    let socket_path = server.socket_dir().join(server.socket());
    let socket = socket_path.to_str().expect("utf-8");
    let pid = server.stdout(&["display-message", "-p", "#{pid}"]);
    let start_time = server.stdout(&["display-message", "-p", "#{start_time}"]);
    let expected = ["focus-changed", "--socket", socket, "--pid", &pid, "--start-time", &start_time, "--pane", &pane, "--in"];
    assert_eq!(lines, expected);
}
