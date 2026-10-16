//! The CLI against a throwaway tmux server: tags land on the pane, and
//! `send` reads them back. Asserts tmux state, never command strings.

mod support;

use agent_notify_core::wire::{Request, decode_request};
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
    assert_eq!(pane.option("@agent_notify_session"), "session-a");
    assert_eq!(pane.option("@agent_notify_prompt"), "");

    hook("UserPromptSubmit", "session-a");
    assert_eq!(pane.option("@agent_notify_prompt"), "prompt-1");

    hook("SessionEnd", "session-b");
    assert_eq!(pane.option("@agent_notify_session"), "session-a", "another session's end leaves the tag");

    hook("SessionEnd", "session-a");
    assert_eq!(pane.option("@agent_notify_session"), "");
    assert_eq!(pane.option("@agent_notify_prompt"), "");
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
