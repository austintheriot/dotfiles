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
fn an_unknown_hook_event_exits_zero_and_logs_it_without_stdin_values_at_info() {
    let stdin = r#"{"session_id":"s","cwd":"/tmp","hook_event_name":"PreCompact"}"#;
    let info_log = hook_log(stdin, "info");
    assert!(info_log.contains("nothing sent"), "{info_log}");
    assert!(!info_log.contains("PreCompact"), "{info_log}");
    let debug_log = hook_log(stdin, "debug");
    assert!(debug_log.contains("PreCompact"), "{debug_log}");
}

#[test]
fn the_nothing_sent_line_names_the_session_and_its_own_reason() {
    let unmapped = r#"{"session_id":"session-a","cwd":"/tmp","hook_event_name":"Notification","notification_type":"idle_prompt"}"#;
    let info_log = hook_log(unmapped, "info");
    let line = info_log.lines().find(|line| line.contains("nothing sent")).unwrap_or_else(|| panic!("a nothing-sent line in {info_log}"));
    for field in ["event_id=", r#"session_id="session-a""#, "pane=", r#"reason="notification type not mapped""#] {
        assert!(line.contains(field), "{field} missing from {line}");
    }
    assert!(!info_log.contains("idle_prompt"), "{info_log}");
}

/// Runs one hook at log `level` in its own sandbox and returns the log. The
/// sandbox drops before the next one starts.
fn hook_log(stdin: &str, level: &str) -> String {
    let sandbox = Sandbox::new();
    assert!(sandbox.run(sandbox.command(&["hook", "claude-code"]).env("AGENT_NOTIFY_LOG", level), stdin).status.success());
    sandbox.log_text()
}

#[test]
fn the_sent_line_names_its_fields_and_keeps_the_request_at_debug() {
    let marker = "last-message-marker-91c2";
    let stdin = format!(r#"{{"session_id":"session-a","prompt_id":"prompt-1","cwd":"/tmp","hook_event_name":"Stop","last_assistant_message":"Done {marker}."}}"#);
    let info_log = hook_log(&stdin, "info");
    let sent = info_log.lines().find(|line| line.contains(" sent")).unwrap_or_else(|| panic!("a sent line in {info_log}"));
    for field in ["event_id=", r#"event="turn_ended""#, r#"session_id="session-a""#, "pane=", "delivered=false", "reason="] {
        assert!(sent.contains(field), "{field} missing from {sent}");
    }
    assert!(!info_log.contains(marker), "{info_log}");
    let debug_log = hook_log(&stdin, "debug");
    assert!(debug_log.contains(marker), "{debug_log}");
}

#[test]
fn the_fallback_line_names_its_fields() {
    let sandbox = Sandbox::new();
    assert!(hook(&sandbox, PERMISSION).status.success());
    let log = sandbox.log_text();
    let shown = log.lines().find(|line| line.contains("fallback shown")).unwrap_or_else(|| panic!("a fallback line in {log}"));
    for field in ["event_id=", r#"event="needs_input""#, r#"session_id="session-a""#, "pane="] {
        assert!(shown.contains(field), "{field} missing from {shown}");
    }
}

#[test]
fn session_start_and_prompt_submit_hooks_print_nothing_on_stdout() {
    let start = r#"{"session_id":"session-a","cwd":"/tmp","hook_event_name":"SessionStart","source":"startup"}"#;
    let submit = r#"{"session_id":"session-a","prompt_id":"prompt-1","cwd":"/tmp","hook_event_name":"UserPromptSubmit","prompt":"hi"}"#;
    for stdin in [start, submit] {
        let sandbox = Sandbox::new();
        let output = hook(&sandbox, stdin);
        assert!(output.status.success(), "{stdin}");
        assert!(output.stdout.is_empty(), "stdout goes into the model context: {:?}", String::from_utf8_lossy(&output.stdout));
        assert!(sandbox.log_text().contains(" sent"), "a positive control: the hook ran to the send in {}", sandbox.log_text());
    }
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
    for (arguments, named) in [(&["send", "--kind", "nope", "--body", "x"][..], "nope"), (&["send", "--kind", "info"], "--body")] {
        let output = sandbox.run(&mut sandbox.command(arguments), "");
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(stderr.lines().count(), 1, "{stderr:?}");
        assert!(stderr.starts_with("agent-notify: "), "{stderr:?}");
        assert!(!stderr.contains("error: "), "{stderr:?}");
        assert!(stderr.contains(named), "{stderr:?}");
    }
}

#[test]
fn a_hanging_notifier_does_not_hold_the_hook() {
    let sandbox = Sandbox::new();
    for name in ["terminal-notifier", "notify-send"] {
        sandbox.stub(name, "#!/bin/sh\nexec sleep 30\n");
    }
    let started = Instant::now();
    assert!(hook(&sandbox, PERMISSION).status.success());
    assert!(started.elapsed() < Duration::from_secs(6), "took {:?}", started.elapsed());
    assert!(sandbox.log_text().contains("fallback failed"), "{}", sandbox.log_text());
}

#[test]
fn a_hanging_tmux_does_not_hold_the_hook() {
    let sandbox = Sandbox::new();
    sandbox.stub("tmux", "#!/bin/sh\nexec sleep 30\n");
    let started = Instant::now();
    let output = sandbox.run(
        sandbox.command(&["hook", "claude-code"]).env("TMUX", "/tmp/an-wedged/sock,1,0").env("TMUX_PANE", "%1"),
        PERMISSION,
    );
    assert!(output.status.success());
    assert!(started.elapsed() < Duration::from_secs(5), "took {:?}", started.elapsed());
    assert!(sandbox.log_text().contains("tmux pane unreadable"), "{}", sandbox.log_text());
    assert_eq!(sandbox.fallback_calls().len(), 1);
}

#[test]
fn closed_output_streams_do_not_crash_a_dry_run_hook() {
    let sandbox = Sandbox::new();
    let (stdout_reader, stdout_writer) = std::io::pipe().expect("a pipe");
    let (stderr_reader, stderr_writer) = std::io::pipe().expect("a pipe");
    drop((stdout_reader, stderr_reader));
    let mut command = sandbox.command(&["hook", "claude-code", "--dry-run"]);
    command.stdout(stdout_writer).stderr(stderr_writer);
    let output = sandbox.run_with_own_output(&mut command, PERMISSION);
    assert!(output.success(), "{output:?}");
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

const FOCUS_IN: [&str; 10] = ["focus-changed", "--socket", "/tmp/tmux-501/default", "--pid", "77", "--start-time", "1700000000", "--pane", "%3", "--in"];

#[test]
fn focus_changed_sends_a_control_and_prints_nothing() {
    let sandbox = Sandbox::new();
    let received = listen(&sandbox.socket, ReplyMode::Ack);
    let output = sandbox.run(&mut sandbox.command(&FOCUS_IN), "");
    assert!(output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty() && output.stderr.is_empty(), "{output:?}");
    let line = received.join().expect("listener thread").expect("a request line");
    let request: serde_json::Value = serde_json::from_str(&line).expect("JSON");
    assert_eq!(request["control"]["type"], "focus_changed");
    assert_eq!(request["control"]["pane"]["pane"], "%3");
    assert_eq!(request["control"]["pane"]["server"]["socket_path"], "/tmp/tmux-501/default");
    assert_eq!(request["control"]["focused"], true);
}

#[test]
fn focus_changed_without_a_daemon_is_silent_and_succeeds() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&mut sandbox.command(&FOCUS_IN), "");
    assert!(output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty() && output.stderr.is_empty(), "{output:?}");
    assert!(sandbox.fallback_calls().is_empty());
}

#[test]
fn focus_changed_does_not_read_the_current_pane() {
    let sandbox = Sandbox::new();
    sandbox.stub("tmux", "#!/bin/sh\nexec sleep 30\n");
    let output = sandbox.run(sandbox.command(&FOCUS_IN).env("TMUX", "/tmp/an-wedged/sock,1,0").env("TMUX_PANE", "%1"), "");
    assert!(output.status.success(), "{output:?}");
    let log = sandbox.log_text();
    assert!(log.contains(" sent"), "a positive control: the command ran to the send in {log}");
    assert!(!log.contains("tmux pane unreadable"), "{log}");
}

#[test]
fn dnd_on_without_a_daemon_fails_and_names_doctor() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&mut sandbox.command(&["dnd", "on"]), "");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("agent-notify doctor"), "{output:?}");
    assert!(sandbox.fallback_calls().is_empty());
}

#[test]
fn dnd_on_with_a_daemon_confirms() {
    let sandbox = Sandbox::new();
    let received = listen(&sandbox.socket, ReplyMode::Ack);
    let output = sandbox.run(&mut sandbox.command(&["dnd", "on", "--for-minutes", "30"]), "");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "do-not-disturb is on");
    let line = received.join().expect("listener thread").expect("a request line");
    assert!(line.contains("\"for_minutes\":30"), "{line}");
}

#[test]
fn a_rejected_jump_fails_and_names_the_reason_and_doctor() {
    let sandbox = Sandbox::new();
    let _listener = listen(&sandbox.socket, ReplyMode::Reject);
    let output = sandbox.run(&mut sandbox.command(&["jump"]), "");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("refused") && stderr.contains("unknown_kind") && stderr.contains("agent-notify doctor"), "{stderr}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(sandbox.fallback_calls().is_empty());
}

fn report_reply(dnd_on: bool) -> String {
    let report = serde_json::json!({
        "daemon": { "stamp": "stamp-9", "started_ms": 0, "tools": [] },
        "now_ms": 60_000,
        "dnd": { "on": dnd_on, "until_ms": null },
        "sessions": [],
        "decisions": []
    });
    serde_json::json!({ "ok": true, "daemon": "stamp-9", "data": report }).to_string()
}

fn status_tmux_log(level: &str) -> String {
    let sandbox = Sandbox::new();
    let received = listen(&sandbox.socket, ReplyMode::Reply(report_reply(false)));
    let output = sandbox.run(sandbox.command(&["status", "--tmux"]).env("AGENT_NOTIFY_LOG", level), "");
    assert!(output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty(), "an answered status with DND off prints nothing: {output:?}");
    let line = received.join().expect("listener thread").expect("a request line");
    assert!(line.contains(r#""query":"status""#), "{line}");
    sandbox.log_text()
}

#[test]
fn an_answered_status_tmux_writes_nothing_at_info() {
    let info_log = status_tmux_log("info");
    assert_eq!(info_log.lines().count(), 0, "{info_log}");
    let debug_log = status_tmux_log("debug");
    assert!(debug_log.contains(" sent") && debug_log.contains("delivered=true"), "a positive control: the query was logged at debug in {debug_log}");
}

#[test]
fn doctor_without_a_daemon_fails_and_names_the_check() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&mut sandbox.command(&["doctor"]), "");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("FAIL  daemon answers"), "{stdout}");
    assert!(stdout.lines().all(|line| line.starts_with("ok    ") || line.starts_with("warn  ") || line.starts_with("FAIL  ")), "{stdout}");
}

#[test]
fn doctor_with_a_daemon_sends_a_status_query_and_prints_its_stamp() {
    let sandbox = Sandbox::new();
    let received = listen(&sandbox.socket, ReplyMode::Reply(report_reply(false)));
    let output = sandbox.run(&mut sandbox.command(&["doctor"]), "");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ok    daemon answers: stamp stamp-9"), "{stdout}");
    let line = received.join().expect("listener thread").expect("a request line");
    assert!(line.contains("\"query\":\"status\""), "{line}");
}

#[test]
fn doctor_warns_about_a_large_cli_log() {
    let sandbox = Sandbox::new();
    std::fs::create_dir_all(&sandbox.logs).expect("log dir");
    let log = std::fs::File::create(sandbox.logs.join("cli.log")).expect("a log file");
    log.set_len(50 * 1024 * 1024).expect("a sparse file");
    let output = sandbox.run(&mut sandbox.command(&["doctor"]), "");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("warn  cli log:"), "{stdout}");
}

#[test]
fn status_tmux_prints_nothing_without_a_socket_and_offline_with_a_stale_one() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&mut sandbox.command(&["status", "--tmux"]), "");
    assert!(output.status.success());
    assert!(output.stdout.is_empty(), "{output:?}");
    std::fs::write(&sandbox.socket, "").expect("a stale regular file");
    let output = sandbox.run(&mut sandbox.command(&["status", "--tmux"]), "");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "#[fg=red]notify offline#[default] ");
}

#[test]
fn status_prints_the_daemon_report() {
    let sandbox = Sandbox::new();
    let received = listen(&sandbox.socket, ReplyMode::Reply(report_reply(true)));
    let output = sandbox.run(&mut sandbox.command(&["status"]), "");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("agent-notifyd stamp-9, up 1m"), "{stdout}");
    assert!(stdout.contains("do-not-disturb: on"), "{stdout}");
    let line = received.join().expect("listener thread").expect("a request line");
    assert!(line.contains("\"query\":\"status\""), "{line}");
}

#[test]
fn status_json_and_dnd_status_and_a_down_daemon() {
    let sandbox = Sandbox::new();
    let received = listen(&sandbox.socket, ReplyMode::Reply(report_reply(true)));
    let output = sandbox.run(&mut sandbox.command(&["status", "--json"]), "");
    assert!(output.status.success(), "{output:?}");
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON on stdout");
    assert_eq!(parsed["daemon"]["stamp"], "stamp-9");
    received.join().expect("listener thread");
    std::fs::remove_file(&sandbox.socket).expect("remove the socket");
    let received = listen(&sandbox.socket, ReplyMode::Reply(report_reply(true)));
    let output = sandbox.run(&mut sandbox.command(&["dnd", "status"]), "");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "on\n", "{output:?}");
    received.join().expect("listener thread");
    std::fs::remove_file(&sandbox.socket).expect("remove the socket");
    let output = sandbox.run(&mut sandbox.command(&["status"]), "");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("the daemon is not running"), "{output:?}");
}
