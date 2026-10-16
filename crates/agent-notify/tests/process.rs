//! The real binary, with stub notifiers and a private socket path. Each test
//! asserts the exit status and whether the fallback fired.

mod support;

use std::os::unix::net::UnixListener;
use std::time::{Duration, Instant};

use agent_notify_core::wire::{AgentEvent, Request, decode_request};
use support::{ReplyMode, Sandbox, listen};

const PERMISSION: &str = r#"{"session_id":"session-a","prompt_id":"prompt-1","cwd":"/tmp","hook_event_name":"Notification","notification_type":"permission_prompt","message":"Allow Bash?"}"#;
const STOP: &str = r#"{"session_id":"session-a","prompt_id":"prompt-1","cwd":"/tmp","hook_event_name":"Stop","last_assistant_message":"Done."}"#;

fn hook(sandbox: &Sandbox, stdin: &str) -> std::process::Output {
    sandbox.run(&mut sandbox.command(&["hook", "claude-code"]), stdin)
}

#[test]
fn no_daemon_permission_prompt_falls_back() {
    let sandbox = Sandbox::new();
    let output = hook(&sandbox, PERMISSION);
    assert!(output.status.success());
    assert_eq!(sandbox.fallback_calls().len(), 1);
    assert!(sandbox.log_text().contains("fallback shown"));
}

#[test]
fn no_daemon_stop_shows_nothing() {
    let sandbox = Sandbox::new();
    assert!(hook(&sandbox, STOP).status.success());
    assert!(sandbox.fallback_calls().is_empty());
}

#[test]
fn a_stale_socket_file_falls_back() {
    let sandbox = Sandbox::new();
    std::fs::write(&sandbox.socket, "").expect("a stale file");
    assert!(hook(&sandbox, PERMISSION).status.success());
    assert_eq!(sandbox.fallback_calls().len(), 1);
}

#[test]
fn a_silent_daemon_times_out_and_falls_back() {
    let sandbox = Sandbox::new();
    let _listener = listen(&sandbox.socket, ReplyMode::Never);
    let started = Instant::now();
    assert!(hook(&sandbox, PERMISSION).status.success());
    assert!(started.elapsed() < Duration::from_millis(1_500), "took {:?}", started.elapsed());
    assert!(sandbox.log_text().contains("reason=timed out"), "{}", sandbox.log_text());
    assert_eq!(sandbox.fallback_calls().len(), 1);
}

#[test]
fn a_rejecting_daemon_falls_back() {
    let sandbox = Sandbox::new();
    let _listener = listen(&sandbox.socket, ReplyMode::Reject);
    assert!(hook(&sandbox, PERMISSION).status.success());
    assert_eq!(sandbox.fallback_calls().len(), 1);
}

#[test]
fn an_acknowledging_daemon_gets_the_request_and_no_fallback() {
    let sandbox = Sandbox::new();
    let received = listen(&sandbox.socket, ReplyMode::Ack);
    assert!(hook(&sandbox, PERMISSION).status.success());
    let line = received.join().expect("listener").expect("a line");
    assert!(matches!(decode_request(line.trim_end()), Ok(Request::Agent { event: AgentEvent::NeedsInput { .. }, .. })));
    assert!(sandbox.fallback_calls().is_empty());
}

#[test]
fn empty_and_garbage_stdin_exit_zero_without_a_notification() {
    for stdin in ["", "not json", "\u{0}\u{1}\u{2}", "{\"hook_event_name\":\"Stop\"}"] {
        let sandbox = Sandbox::new();
        let output = hook(&sandbox, stdin);
        assert!(output.status.success(), "{stdin:?}");
        assert!(sandbox.fallback_calls().is_empty());
    }
}

#[test]
fn an_unknown_hook_event_exits_zero_and_logs_it() {
    let sandbox = Sandbox::new();
    let stdin = r#"{"session_id":"s","cwd":"/tmp","hook_event_name":"PreCompact"}"#;
    assert!(hook(&sandbox, stdin).status.success());
    assert!(sandbox.log_text().contains("PreCompact"));
}

#[test]
fn the_raw_payload_is_logged_only_at_trace() {
    let marker = "raw-payload-marker-7f3a";
    let stdin = format!(r#"{{"session_id":"s","prompt_id":"p","cwd":"/tmp","hook_event_name":"Stop","transcript_path":"/tmp/{marker}"}}"#);
    let log_with = |variable: &str, value: &str| {
        let sandbox = Sandbox::new();
        assert!(sandbox.run(sandbox.command(&["hook", "claude-code"]).env(variable, value), &stdin).status.success());
        sandbox.log_text()
    };
    let debug = log_with("AGENT_NOTIFY_LOG", "debug");
    assert!(debug.contains("sent"), "{debug}");
    assert!(!debug.contains(marker), "{debug}");
    let traced = log_with("AGENT_NOTIFY_TRACE", "1");
    assert!(traced.contains(marker), "{traced}");
}

#[test]
fn bad_hook_arguments_still_exit_zero() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&mut sandbox.command(&["hook", "codex"]), STOP);
    assert!(output.status.success());
}

#[test]
fn send_usage_errors_exit_two_with_a_message() {
    let sandbox = Sandbox::new();
    for arguments in [&["send", "--kind", "info", "--body", "  "][..], &["send", "--kind", "loud", "--body", "x"]] {
        let output = sandbox.run(&mut sandbox.command(arguments), "");
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(!output.stderr.is_empty());
    }
    assert!(sandbox.fallback_calls().is_empty());
}

#[test]
fn a_clap_usage_error_is_one_stderr_line_and_exit_two() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&mut sandbox.command(&["send", "--kind", "nope", "--body", "x"]), "");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stderr.lines().count(), 1, "{stderr:?}");
    assert!(stderr.starts_with("agent-notify: "), "{stderr:?}");
    assert!(!stderr.contains("error: "), "{stderr:?}");
    assert!(stderr.contains("nope"), "{stderr:?}");
}

#[test]
fn help_and_version_still_print_and_exit_zero() {
    let sandbox = Sandbox::new();
    for arguments in [&["--help"][..], &["--version"], &["send", "--help"]] {
        let output = sandbox.run(&mut sandbox.command(arguments), "");
        assert!(output.status.success(), "{arguments:?}");
        assert!(!output.stdout.is_empty(), "{arguments:?}");
    }
}

#[test]
fn quiet_outside_tmux_exits_two() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&mut sandbox.command(&["quiet", "--until", "next-send"]), "");
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("tmux"));
}

#[test]
fn send_without_a_daemon_falls_back_and_says_so() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&mut sandbox.command(&["send", "--kind", "attention", "--body", "CI is red"]), "");
    assert!(output.status.success());
    assert_eq!(sandbox.fallback_calls().len(), 1);
    assert!(String::from_utf8_lossy(&output.stderr).contains("did not answer"));
}

#[test]
fn an_unwritable_log_directory_does_not_block_the_hook() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(sandbox.command(&["hook", "claude-code"]).env("AGENT_NOTIFY_LOG_DIR", "/dev/null/logs"), PERMISSION);
    assert!(output.status.success());
    assert_eq!(sandbox.fallback_calls().len(), 1);
}

#[test]
fn dry_run_prints_the_request_and_contacts_nothing() {
    let sandbox = Sandbox::new();
    let listener = UnixListener::bind(&sandbox.socket).expect("bind");
    listener.set_nonblocking(true).expect("nonblocking");
    let output = sandbox.run(&mut sandbox.command(&["hook", "claude-code", "--dry-run"]), PERMISSION);
    assert!(output.status.success());
    let printed = String::from_utf8_lossy(&output.stdout);
    assert!(decode_request(printed.trim_end()).is_ok(), "{printed}");
    assert!(listener.accept().is_err(), "dry run must not connect");
    assert!(sandbox.fallback_calls().is_empty());
}

#[test]
fn dead_tmux_server_still_falls_back() {
    let sandbox = Sandbox::new();
    let started = Instant::now();
    let output = sandbox.run(
        sandbox.command(&["hook", "claude-code"]).env("TMUX", "/tmp/an-no-such-dir/sock,1,0").env("TMUX_PANE", "%1"),
        PERMISSION,
    );
    assert!(output.status.success());
    assert!(started.elapsed() < Duration::from_secs(2), "took {:?}", started.elapsed());
    assert!(sandbox.log_text().contains("tmux pane unreadable"), "{}", sandbox.log_text());
    assert_eq!(sandbox.fallback_calls().len(), 1);
}

#[test]
fn fallback_passes_text_as_single_arguments() {
    let sandbox = Sandbox::new();
    let stdin = r#"{"session_id":"s","prompt_id":"p","cwd":"/tmp","hook_event_name":"Notification","notification_type":"permission_prompt","message":"-execute touch /tmp/pwned; echo $(id)"}"#;
    assert!(hook(&sandbox, stdin).status.success());
    let calls = sandbox.fallback_calls();
    assert_eq!(calls.len(), 1);
    let call = &calls[0];
    if cfg!(target_os = "macos") {
        let message_at = call.iter().position(|argument| argument == "-message").expect("a -message flag");
        assert_eq!(call[message_at + 1], " -execute touch /tmp/pwned; echo $(id)");
        assert!(!call.iter().any(|argument| argument == "-execute"));
    } else {
        assert!(call.iter().any(|argument| argument.ends_with("-execute touch /tmp/pwned; echo $(id)")));
    }
}

#[test]
fn stamp_answers_without_a_subcommand() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&mut sandbox.command(&["--stamp"]), "");
    assert!(output.status.success());
    assert!(!output.stdout.is_empty());
}
