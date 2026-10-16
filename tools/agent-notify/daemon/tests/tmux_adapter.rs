//! The tmux adapter against a real, throwaway tmux server.

mod support;

use agent_notify_core::status::{PANE_STATE_OPTION, WINDOW_STATE_OPTION};
use agent_notify_core::update::PaneState;
use agent_notify_core::{PaneKey, SessionId};
use agent_notify_daemon::tmux::TmuxError;
use support::{adapter, first_pane, key, new_window, pane_option, server_id, socket, split, tag, tmux_server, window_option};

#[test]
fn the_server_id_matches_what_tmux_reports() {
    let server = tmux_server("an-id");
    assert_eq!(adapter().server_id(&socket(&server)).expect("reads"), server_id(&server));
}

#[test]
fn the_world_finds_a_tagged_pane_by_session_id() {
    let server = tmux_server("an-world");
    let pane = new_window(&server);
    tag(&server, &pane, "session-a");
    let world = adapter().focus_world(&server_id(&server)).expect("reads");
    let tagged: Vec<_> = world.panes.iter().filter(|entry| entry.session_tag.is_some()).collect();
    assert_eq!(tagged.len(), 1);
    assert_eq!(tagged[0].pane, pane);
    assert_eq!(tagged[0].tmux_session, server.stdout(&["display-message", "-p", "-t", "main", "#{session_id}"]));
    assert!(tagged[0].tmux_session.starts_with('$'), "a session id, not a name: {}", tagged[0].tmux_session);
    assert!(world.clients.is_empty(), "no client is attached in a test");
}

#[test]
fn pane_state_writes_the_pane_and_the_windows_most_urgent_state() {
    let server = tmux_server("an-state");
    let first = first_pane(&server, "main");
    let second = split(&server, "main");
    let tmux = adapter();
    tmux.set_pane_state(&key(&server, &first), PaneState::Working).expect("writes");
    tmux.set_pane_state(&key(&server, &second), PaneState::NeedsYou).expect("writes");
    assert_eq!(pane_option(&server, &first, PANE_STATE_OPTION), "working");
    assert_eq!(pane_option(&server, &second, PANE_STATE_OPTION), "needs-you");
    assert_eq!(window_option(&server, &first, WINDOW_STATE_OPTION), "needs-you");
    tmux.set_pane_state(&key(&server, &second), PaneState::Idle).expect("writes");
    assert_eq!(pane_option(&server, &second, PANE_STATE_OPTION), "");
    assert_eq!(window_option(&server, &first, WINDOW_STATE_OPTION), "working");
    tmux.set_pane_state(&key(&server, &first), PaneState::Idle).expect("writes");
    assert_eq!(window_option(&server, &first, WINDOW_STATE_OPTION), "");
}

#[test]
fn a_restarted_server_reads_as_gone() {
    let server = tmux_server("an-restart");
    let old = server_id(&server);
    let old_pane = first_pane(&server, "main");
    assert!(server.tmux(&["kill-server"]).status.success());
    server.new_session("main", std::path::Path::new("/tmp"));
    let new_pane = first_pane(&server, "main");
    assert_eq!(new_pane, old_pane, "the new server reuses the pane id, which is the hazard");
    let tmux = adapter();
    let stale = PaneKey { server: old.clone(), pane: old_pane };
    assert!(matches!(tmux.set_pane_state(&stale, PaneState::Working), Err(TmuxError::ServerGone)));
    assert_eq!(pane_option(&server, &new_pane, PANE_STATE_OPTION), "", "nothing was written to the new pane");
    assert!(matches!(tmux.focus_world(&old), Err(TmuxError::ServerGone)));
    assert!(matches!(tmux.active_pane(&old), Ok(None) | Err(TmuxError::ServerGone)));
    assert!(matches!(tmux.session_tag(&stale), Err(TmuxError::ServerGone)));
}

#[test]
fn a_snapshot_after_a_restart_names_the_new_server() {
    let server = tmux_server("an-snap");
    let pane = first_pane(&server, "main");
    tag(&server, &pane, "session-a");
    let tmux = adapter();
    let before = tmux.pane_snapshot(&socket(&server)).expect("reads");
    assert_eq!(before.server, server_id(&server));
    assert_eq!(before.panes.len(), 1);
    assert_eq!(before.panes[0].session, Some(SessionId::parse("session-a").expect("id")));
    assert!(server.tmux(&["kill-server"]).status.success());
    server.new_session("main", std::path::Path::new("/tmp"));
    let after = tmux.pane_snapshot(&socket(&server)).expect("reads");
    assert_ne!(after.server, before.server, "the snapshot names the server it read");
    assert_eq!(after.server, server_id(&server));
    assert_eq!(after.panes.len(), 1);
    assert_eq!(after.panes[0].session, None, "the new server's pane carries no tag");
}

#[test]
fn a_missing_server_is_gone_not_failed() {
    let server = tmux_server("an-missing");
    let id = server_id(&server);
    assert!(server.tmux(&["kill-server"]).status.success());
    let tmux = adapter();
    assert!(matches!(tmux.focus_world(&id), Err(TmuxError::ServerGone)));
    assert!(matches!(tmux.pane_snapshot(&id.socket_path), Err(TmuxError::ServerGone)));
    assert!(matches!(tmux.server_id(&id.socket_path), Err(TmuxError::ServerGone)));
}

#[test]
fn the_session_tag_reads_back() {
    let server = tmux_server("an-tag");
    let pane = first_pane(&server, "main");
    let tmux = adapter();
    assert_eq!(tmux.session_tag(&key(&server, &pane)).expect("reads"), None);
    tag(&server, &pane, "session-b");
    assert_eq!(tmux.session_tag(&key(&server, &pane)).expect("reads"), Some(SessionId::parse("session-b").expect("id")));
}
