//! The CLI against a throwaway tmux server: tags land on the pane, and
//! `send` reads them back. Asserts tmux state, never command strings.

mod support;

use thumper_core::wire::{Request, decode_request};
use dotfiles_test_support::tmux_server::Server;
use support::{ReplyMode, Sandbox, listen};

struct Pane {
    server: Server,
    id: String,
    _directory: tempfile::TempDir,
}

impl Pane {
    fn new(label: &str) -> Self {
        let server = Server::new(label);
        let directory = tempfile::tempdir().expect("a directory");
        server.new_session("main", directory.path());
        server.tmux(&["rename-window", "-t", "main", "code/feature-x"]);
        let id = server.stdout(&["display-message", "-p", "-t", "main", "#{pane_id}"]);
        Pane { server, id, _directory: directory }
    }

    fn option(&self, name: &str) -> String {
        self.server.stdout(&["show-options", "-p", "-q", "-v", "-t", &self.id, name])
    }

    fn command(&self, sandbox: &Sandbox, arguments: &[&str]) -> std::process::Command {
        let mut command = sandbox.command(arguments);
        command
            .env("TMUX", format!("{},{},0", self.server.socket_path().display(), self.server.pid()))
            .env("TMUX_PANE", &self.id);
        command
    }
}

fn payload(event: &str, session: &str) -> String {
    format!(r#"{{"session_id":"{session}","prompt_id":"prompt-1","cwd":"/tmp","hook_event_name":"{event}","source":"startup"}}"#)
}

#[test]
fn hooks_bind_and_unbind_the_pane_tags() {
    let pane = Pane::new("an-tags");
    let sandbox = Sandbox::new();
    let hook = |event: &str, session: &str| {
        let output = sandbox.run(&mut pane.command(&sandbox, &["hook", "claude-code"]), &payload(event, session));
        assert!(output.status.success(), "{event}: {output:?}");
    };

    hook("SessionStart", "session-a");
    assert_eq!(pane.option("@thumper_session"), "session-a");
    assert_eq!(pane.option("@thumper_prompt"), "");

    hook("UserPromptSubmit", "session-a");
    assert_eq!(pane.option("@thumper_prompt"), "prompt-1");

    hook("SessionEnd", "session-b");
    assert_eq!(pane.option("@thumper_session"), "session-a", "another session's end leaves the tag");

    hook("SessionEnd", "session-a");
    assert_eq!(pane.option("@thumper_session"), "");
    assert_eq!(pane.option("@thumper_prompt"), "");
}

#[test]
fn a_prompt_submit_with_an_invalid_prompt_id_clears_the_old_prompt_tag() {
    let pane = Pane::new("an-badprompt");
    let sandbox = Sandbox::new();
    let hook = |stdin: &str| {
        let output = sandbox.run(&mut pane.command(&sandbox, &["hook", "claude-code"]), stdin);
        assert!(output.status.success(), "{stdin}: {output:?}");
    };
    hook(&payload("SessionStart", "session-a"));
    hook(&payload("UserPromptSubmit", "session-a"));
    assert_eq!(pane.option("@thumper_prompt"), "prompt-1", "a positive control: the first prompt landed");
    hook(r#"{"session_id":"session-a","prompt_id":"a:b.c","cwd":"/tmp","hook_event_name":"UserPromptSubmit"}"#);
    assert_eq!(pane.option("@thumper_prompt"), "");
    assert_eq!(pane.option("@thumper_session"), "session-a");
}

#[test]
fn ids_that_start_with_a_dash_land_as_tag_values() {
    let pane = Pane::new("an-dash");
    let sandbox = Sandbox::new();
    let stdin = |event: &str| format!(r#"{{"session_id":"-u","prompt_id":"-q","cwd":"/tmp","hook_event_name":"{event}","source":"startup"}}"#);
    for event in ["SessionStart", "UserPromptSubmit"] {
        let output = sandbox.run(&mut pane.command(&sandbox, &["hook", "claude-code"]), &stdin(event));
        assert!(output.status.success(), "{event}: {output:?}");
    }
    assert_eq!(pane.option("@thumper_session"), "-u");
    assert_eq!(pane.option("@thumper_prompt"), "-q");
    assert!(!sandbox.log_text().contains("tag write failed"), "{}", sandbox.log_text());
}

#[test]
fn a_non_ascii_window_name_survives_a_c_locale() {
    let pane = Pane::new("an-utf8");
    pane.server.tmux(&["rename-window", "-t", "main", "café"]);
    let sandbox = Sandbox::new();
    let received = listen(&sandbox.socket, ReplyMode::Ack);
    let output = sandbox.run(pane.command(&sandbox, &["send", "--kind", "info", "--body", "Report ready"]).env("LC_ALL", "C"), "");
    assert!(output.status.success(), "{output:?}");
    let line = received.join().expect("listener thread").expect("a request line");
    let Ok(Request::Agent { origin, .. }) = decode_request(line.trim_end()) else {
        panic!("expected an agent request: {line}");
    };
    assert_eq!(origin.window.map(|window| window.name.as_str().to_string()), Some("café".to_string()));
}

/// The daemon keys every record by the socket path the CLI sends and reads
/// tmux with `#{socket_path}`, so the two must match byte for byte with no
/// canonicalization on either side.
#[test]
fn the_socket_path_from_tmux_env_is_byte_equal_to_the_servers_socket_path() {
    let pane = Pane::new("an-sockpath");
    let directory = tempfile::tempdir().expect("a directory");
    let recorded = directory.path().join("tmux-env");
    let script = format!("printf %s \"$TMUX\" > '{}.tmp' && mv '{0}.tmp' '{0}'; exec sleep 600", recorded.display());
    let window = pane.server.stdout(&["new-window", "-d", "-t", "main", "-P", "-F", "#{pane_id}", "/bin/sh", "-c", &script]);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !recorded.exists() {
        assert!(std::time::Instant::now() < deadline, "the pane never wrote $TMUX");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let tmux_value = std::fs::read_to_string(&recorded).expect("the recorded $TMUX");
    let sandbox = Sandbox::new();
    let received = listen(&sandbox.socket, ReplyMode::Ack);
    let mut command = sandbox.command(&["send", "--kind", "info", "--body", "Report ready"]);
    command.env("TMUX", &tmux_value).env("TMUX_PANE", &window);
    let output = sandbox.run(&mut command, "");
    assert!(output.status.success(), "{output:?}");
    let line = received.join().expect("listener thread").expect("a request line");
    let Ok(Request::Agent { origin, .. }) = decode_request(line.trim_end()) else {
        panic!("expected an agent request: {line}");
    };
    let sent = origin.pane.expect("the pane facts").server.socket_path;
    let reported = pane.server.stdout(&["display-message", "-p", "#{socket_path}"]);
    assert_eq!(sent.as_path().as_os_str().as_encoded_bytes(), reported.as_bytes(), "$TMUX was {tmux_value:?}");
}

#[test]
fn send_carries_the_pane_tags_and_the_window() {
    let pane = Pane::new("an-send");
    let sandbox = Sandbox::new();
    sandbox.run(&mut pane.command(&sandbox, &["hook", "claude-code"]), &payload("SessionStart", "session-a"));
    sandbox.run(&mut pane.command(&sandbox, &["hook", "claude-code"]), &payload("UserPromptSubmit", "session-a"));

    let received = listen(&sandbox.socket, ReplyMode::Ack);
    let output = sandbox.run(&mut pane.command(&sandbox, &["send", "--kind", "info", "--body", "Report ready"]), "");
    assert!(output.status.success(), "{output:?}");
    let line = received.join().expect("listener thread").expect("a request line");

    let Ok(Request::Agent { origin, .. }) = decode_request(line.trim_end()) else {
        panic!("expected an agent request: {line}");
    };
    let window_index: u32 = pane.server.stdout(&["display-message", "-p", "-t", &pane.id, "#{window_index}"]).parse().expect("an index");
    assert_eq!(origin.session.map(|session| session.as_str().to_string()), Some("session-a".to_string()));
    assert_eq!(origin.prompt.map(|prompt| prompt.as_str().to_string()), Some("prompt-1".to_string()));
    let window = origin.window.expect("a window");
    assert_eq!((window.index, window.name.as_str()), (window_index, "code/feature-x"));
    assert!(sandbox.fallback_calls().is_empty(), "an acknowledged send shows no fallback");
}
