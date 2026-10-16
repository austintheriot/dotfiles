//! The Claude plugin's wiring contract: which hook events run, sync or
//! async, and which Notification types reach the CLI.

use std::path::PathBuf;

use dotfiles_test_support::repo;
use serde_json::{Value, json};

const COMMAND: &str = "\"$HOME/.local/bin/agent-notify\" hook claude-code";

fn read_json(relative: &str) -> Value {
    let path: PathBuf = repo::root().join(relative);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} must be readable: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("{} must be JSON: {error}", path.display()))
}

fn hooks_file() -> Value {
    read_json("tools/agent-notify/claude-plugin/hooks/hooks.json")
}

fn first_entry_hooks(event: &str) -> Value {
    hooks_file()["hooks"][event][0]["hooks"].clone()
}

fn matcher_accepts(pattern: &str, notification_type: &str) -> bool {
    pattern
        .split('|')
        .any(|alternative| alternative == notification_type)
}

#[test]
fn binds_exactly_the_events_the_cli_maps() {
    let file = hooks_file();
    let events = file["hooks"].as_object().expect("hooks is an object");
    let mut names: Vec<&str> = events.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "Notification",
            "SessionEnd",
            "SessionStart",
            "Stop",
            "StopFailure",
            "UserPromptSubmit"
        ]
    );
}

#[test]
fn session_start_runs_sync_so_the_pane_tag_lands_first() {
    assert_eq!(
        first_entry_hooks("SessionStart"),
        json!([{ "type": "command", "command": COMMAND, "timeout": 5 }])
    );
}

#[test]
fn user_prompt_submit_runs_sync_so_the_pane_tag_lands_first() {
    assert_eq!(
        first_entry_hooks("UserPromptSubmit"),
        json!([{ "type": "command", "command": COMMAND, "timeout": 5 }])
    );
}

#[test]
fn session_end_runs_sync_so_the_pane_tag_lands_first() {
    assert_eq!(
        first_entry_hooks("SessionEnd"),
        json!([{ "type": "command", "command": COMMAND, "timeout": 5 }])
    );
}

#[test]
fn stop_runs_async() {
    assert_eq!(
        first_entry_hooks("Stop"),
        json!([{ "type": "command", "command": COMMAND, "async": true }])
    );
}

#[test]
fn stop_failure_runs_async() {
    assert_eq!(
        first_entry_hooks("StopFailure"),
        json!([{ "type": "command", "command": COMMAND, "async": true }])
    );
}

#[test]
fn notification_runs_async() {
    assert_eq!(
        first_entry_hooks("Notification"),
        json!([{ "type": "command", "command": COMMAND, "async": true }])
    );
}

#[test]
fn the_notification_matcher_keeps_idle_prompt_out() {
    let file = hooks_file();
    let pattern = file["hooks"]["Notification"][0]["matcher"]
        .as_str()
        .expect("the Notification entry has a matcher");
    for accepted in [
        "permission_prompt",
        "elicitation_dialog",
        "elicitation_url_dialog",
        "elicitation_complete",
    ] {
        assert!(matcher_accepts(pattern, accepted), "{accepted} must match");
    }
    for rejected in ["idle_prompt", "agent_needs_input", "agent_completed"] {
        assert!(
            !matcher_accepts(pattern, rejected),
            "{rejected} must not match"
        );
    }
}

#[test]
fn the_marketplace_points_at_the_plugin_directory() {
    let marketplace = read_json("tools/agent-notify/.claude-plugin/marketplace.json");
    assert_eq!(marketplace["name"], "dotfiles-tools");
    let plugins = marketplace["plugins"]
        .as_array()
        .expect("plugins is an array");
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0]["name"], "agent-notify");
    assert_eq!(plugins[0]["source"], "./claude-plugin");
    let plugin = read_json("tools/agent-notify/claude-plugin/.claude-plugin/plugin.json");
    assert_eq!(plugin["name"], "agent-notify");
}
