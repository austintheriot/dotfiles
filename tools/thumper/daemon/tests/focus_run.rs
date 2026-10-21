//! Running focus plans against a real tmux server. A test cannot attach a
//! tmux client without a terminal, so these cover the no-client path
//! (`AttachNewTerminal`), `SessionGone` and `RaiseOnly`. The final smoke
//! test covers `SwitchClient` by hand.

mod support;

use std::path::Path;

use thumper_core::update::{FocusOutcome, FocusTarget};
use thumper_core::{PaneKey, SessionId};
use thumper_daemon::focus_run::run_focus;
use support::{RecordingRaiser, adapter, first_pane, key, new_window, new_window_in, server_id, socket_of, tag, tmux_server};

fn target(session: &str, pane: PaneKey) -> FocusTarget {
    FocusTarget::Session { session: SessionId::parse(session).expect("id"), pane }
}

#[test]
fn a_tagged_pane_with_no_client_selects_it_and_opens_a_terminal() {
    let server = tmux_server("an-run");
    let pane = new_window(&server);
    tag(&server, &pane, "session-a");
    let raiser = RecordingRaiser::default();
    let tmux = adapter();
    let last_seen = PaneKey { server: server_id(&server), pane: first_pane(&server, "main") };
    let outcome = run_focus(Some(&tmux), &raiser, &target("session-a", last_seen));
    assert_eq!(outcome, FocusOutcome::Done);
    assert_eq!(server.stdout(&["display-message", "-p", "-t", "main", "#{pane_id}"]), pane.as_str(), "the tagged pane is now current");
    let attaches = raiser.attaches.lock().expect("unpoisoned");
    assert_eq!(attaches.len(), 1);
    assert_eq!(attaches[0].session, server.stdout(&["display-message", "-p", "-t", "main", "#{session_id}"]));
    assert_eq!(attaches[0].socket, socket_of(&server));
    assert_eq!(attaches[0].tmux, tmux.binary());
}

#[test]
fn a_session_whose_name_needs_escaping_is_reached_by_its_id() {
    let server = tmux_server("an-odd");
    let odd_name = r"odd\ name x";
    server.new_session(odd_name, Path::new("/tmp"));
    let odd_session = server.stdout(&["list-sessions", "-f", "#{!=:#{session_name},main}", "-F", "#{session_id}"]);
    assert!(odd_session.starts_with('$'), "found the second session: {odd_session:?}");
    let pane = new_window_in(&server, &odd_session);
    tag(&server, &pane, "session-a");
    let raiser = RecordingRaiser::default();
    let outcome = run_focus(Some(&adapter()), &raiser, &target("session-a", key(&server, &pane)));
    assert_eq!(outcome, FocusOutcome::Done);
    assert_eq!(server.stdout(&["display-message", "-p", "-t", &odd_session, "#{pane_id}"]), pane.as_str(), "the tagged pane is now current");
    let attaches = raiser.attaches.lock().expect("unpoisoned");
    assert_eq!(attaches.len(), 1);
    assert_eq!(attaches[0].session, odd_session);
    let attached_name = server.tmux(&["display-message", "-p", "-t", &attaches[0].session, "#{session_name}"]);
    assert!(attached_name.status.success(), "the attach target resolves: {attached_name:?}");
}

#[test]
fn an_untagged_world_is_session_gone() {
    let server = tmux_server("an-gone");
    let pane = first_pane(&server, "main");
    let raiser = RecordingRaiser::default();
    let outcome = run_focus(Some(&adapter()), &raiser, &target("session-a", key(&server, &pane)));
    assert_eq!(outcome, FocusOutcome::SessionGone);
    assert_eq!(*raiser.raises.lock().expect("unpoisoned"), 0);
}

#[test]
fn a_dead_server_is_session_gone() {
    let server = tmux_server("an-dead");
    let pane_key = key(&server, &first_pane(&server, "main"));
    assert!(server.tmux(&["kill-server"]).status.success());
    let outcome = run_focus(Some(&adapter()), &RecordingRaiser::default(), &target("session-a", pane_key));
    assert_eq!(outcome, FocusOutcome::SessionGone);
}

#[test]
fn raise_only_raises_and_reports_a_failed_raise() {
    let raiser = RecordingRaiser::default();
    assert_eq!(run_focus(None, &raiser, &FocusTarget::RaiseOnly), FocusOutcome::Done);
    assert_eq!(*raiser.raises.lock().expect("unpoisoned"), 1);
    let failing = RecordingRaiser { fail: true, ..RecordingRaiser::default() };
    assert_eq!(run_focus(None, &failing, &FocusTarget::RaiseOnly), FocusOutcome::RaiseFailed);
}
