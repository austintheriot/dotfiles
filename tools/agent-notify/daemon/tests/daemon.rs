//! The daemon end to end: socket in, tmux and presenter out.

mod support;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use agent_notify_core::EpochMs;
use agent_notify_core::state::Dnd;
use agent_notify_core::status::PANE_STATE_OPTION;
use agent_notify_core::update::UiIntent;
use agent_notify_core::wire::Kind;
use agent_notify_daemon::actor::DaemonConfig;
use agent_notify_daemon::dnd_store::DndStore;
use serde_json::json;
use support::{
    DaemonSetup, RecordingRaiser, SlowPresence, agent_line, control_line, eventually, first_pane, new_window, one_daemon_at_a_time, pane_option, query_line, send_line, spawn_daemon,
    spawn_daemon_in, spawn_daemon_setup, status_sessions, status_until, tag, tmux_stub,
};

#[tokio::test(flavor = "multi_thread")]
async fn a_finished_turn_shows_an_info_card_and_marks_the_pane() {
    let _serial = one_daemon_at_a_time().await;
    let world = spawn_daemon("an-turn");
    let pane = first_pane(&world.server, "main");
    let started = send_line(&world.socket, agent_line(&world.server, &pane, 1, "session-a", "prompt-1", json!({ "type": "turn_started" }))).await;
    assert_eq!(started["ok"], true, "{started}");
    let server = &world.server;
    let read_state = || pane_option(server, &pane, PANE_STATE_OPTION);
    tokio::task::block_in_place(|| eventually("working", read_state));
    send_line(&world.socket, agent_line(&world.server, &pane, 2, "session-a", "prompt-1", json!({ "type": "turn_ended", "last_message": "Done." }))).await;
    let view = tokio::task::block_in_place(|| world.presenter.wait_for(|view| !view.cards.is_empty()));
    assert_eq!(view.cards[0].kind, Kind::Info);
    assert_eq!(view.cards[0].body.as_deref(), Some("Done."));
    tokio::task::block_in_place(|| eventually("finished", read_state));
    world.daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ticks_that_change_nothing_push_no_view() {
    let _serial = one_daemon_at_a_time().await;
    let base = DaemonSetup::default();
    let world = spawn_daemon_setup("an-quiet", DaemonSetup { config: DaemonConfig { tick: Duration::from_millis(20), ..base.config }, ..base });
    tokio::task::block_in_place(|| world.presenter.wait_for(|view| view.cards.is_empty()));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(world.presenter.pushes(), 1, "about 15 ticks changed nothing on screen");
    world.daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn status_reports_the_session_and_its_decisions() {
    let _serial = one_daemon_at_a_time().await;
    let world = spawn_daemon("an-status");
    let pane = first_pane(&world.server, "main");
    send_line(&world.socket, agent_line(&world.server, &pane, 1, "session-a", "prompt-1", json!({ "type": "turn_started" }))).await;
    let reply = status_until(&world.socket, |reply| reply["data"]["decisions"].as_array().is_some_and(|rows| !rows.is_empty())).await;
    assert_eq!(reply["ok"], true, "{reply}");
    assert_eq!(reply["data"]["daemon"]["stamp"], "stamp-test");
    assert_eq!(reply["data"]["sessions"][0]["turn"], "working", "{reply}");
    assert_eq!(reply["data"]["decisions"].as_array().map(Vec::len), Some(1), "{reply}");
    assert_eq!(reply["data"]["decisions"][0]["outcome"], "no_card", "{reply}");
    world.daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_click_runs_the_focus_plan_on_the_worker() {
    let _serial = one_daemon_at_a_time().await;
    let world = spawn_daemon("an-click");
    let pane = new_window(&world.server);
    tag(&world.server, &pane, "session-a");
    send_line(&world.socket, agent_line(&world.server, &pane, 1, "session-a", "prompt-1", json!({ "type": "turn_started" }))).await;
    send_line(&world.socket, agent_line(&world.server, &pane, 2, "session-a", "prompt-1", json!({ "type": "turn_ended" }))).await;
    let view = tokio::task::block_in_place(|| world.presenter.wait_for(|view| !view.cards.is_empty()));
    world.daemon.send_ui(UiIntent::Clicked { card_id: view.cards[0].card_id.clone() });
    let raiser = world.raiser.clone();
    tokio::task::block_in_place(|| eventually("1", || raiser.attaches.lock().expect("unpoisoned").len().to_string()));
    let session_id = world.server.stdout(&["display-message", "-p", "-t", "main", "#{session_id}"]);
    assert_eq!(world.raiser.attaches.lock().expect("unpoisoned")[0].session, session_id);
    world.daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn dnd_hides_cards_and_survives_a_restart() {
    let _serial = one_daemon_at_a_time().await;
    let world = spawn_daemon("an-dnd");
    let reply = send_line(&world.socket, control_line(1, json!({ "type": "dnd", "command": { "type": "on" } }))).await;
    assert_eq!(reply["ok"], true, "{reply}");
    let view = tokio::task::block_in_place(|| world.presenter.wait_for(|view| view.dnd));
    assert!(view.cards.is_empty());
    let dnd_path = world.dnd_path.clone();
    tokio::task::block_in_place(|| eventually("true", || DndStore::new(dnd_path.clone()).load().eq(&Dnd::On { until: None }).to_string()));
    let scratch = world.scratch;
    world.daemon.stop().await;
    let restarted = spawn_daemon_in("an-dnd2", scratch);
    let model = restarted.daemon.view_model().await.expect("the actor answers");
    assert!(model.dnd, "DND was loaded from disk");
    restarted.daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn stopping_frees_the_socket_for_the_next_daemon() {
    let _serial = one_daemon_at_a_time().await;
    let world = spawn_daemon("an-stop");
    let socket = world.socket.clone();
    world.daemon.stop().await;
    assert!(!socket.exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_query_is_answered_while_focus_facts_are_slow() {
    let _serial = one_daemon_at_a_time().await;
    let presence = Arc::new(SlowPresence { delay: Duration::from_secs(2), ..SlowPresence::default() });
    let world = spawn_daemon_setup("an-slow", DaemonSetup { presence: presence.clone(), ..DaemonSetup::default() });
    let pane = first_pane(&world.server, "main");
    send_line(&world.socket, agent_line(&world.server, &pane, 1, "session-a", "prompt-1", json!({ "type": "turn_ended", "last_message": "Done." }))).await;
    let asked = Instant::now();
    let reply = send_line(&world.socket, query_line(2, "status")).await;
    let waited = asked.elapsed();
    assert_eq!(reply["ok"], true, "the query got no answer: {reply}");
    assert!(waited < Duration::from_millis(800), "the query waited {waited:?} behind focus gathering");
    tokio::task::block_in_place(|| world.presenter.wait_for(|view| !view.cards.is_empty()));
    assert_eq!(presence.reads.load(Ordering::SeqCst), 1);
    world.daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn presence_is_read_only_for_events_that_can_show_a_card() {
    let _serial = one_daemon_at_a_time().await;
    let presence = Arc::new(SlowPresence::default());
    let world = spawn_daemon_setup("an-presence", DaemonSetup { presence: presence.clone(), ..DaemonSetup::default() });
    let pane = first_pane(&world.server, "main");
    send_line(&world.socket, agent_line(&world.server, &pane, 1, "session-a", "prompt-1", json!({ "type": "turn_started" }))).await;
    send_line(&world.socket, control_line(2, json!({ "type": "jump" }))).await;
    let decided = |count: usize| move |reply: &serde_json::Value| reply["data"]["decisions"].as_array().map(Vec::len) == Some(count);
    status_until(&world.socket, decided(2)).await;
    assert_eq!(presence.reads.load(Ordering::SeqCst), 0, "a turn start and a control show no card");
    send_line(&world.socket, agent_line(&world.server, &pane, 3, "session-a", "prompt-1", json!({ "type": "turn_ended" }))).await;
    status_until(&world.socket, decided(3)).await;
    assert_eq!(presence.reads.load(Ordering::SeqCst), 1, "a turn end can show a card");
    world.daemon.stop().await;
}

fn snapshotting() -> DaemonSetup {
    let base = DaemonSetup::default();
    DaemonSetup { config: DaemonConfig { tick: Duration::from_millis(100), snapshot_every: Duration::from_millis(200), ..base.config }, ..base }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_restarted_on_the_same_socket_keeps_the_new_servers_sessions() {
    let _serial = one_daemon_at_a_time().await;
    let world = spawn_daemon_setup("an-reborn", snapshotting());
    let old_pane = first_pane(&world.server, "main");
    tag(&world.server, &old_pane, "session-a");
    send_line(&world.socket, agent_line(&world.server, &old_pane, 1, "session-a", "prompt-1", json!({ "type": "turn_started" }))).await;
    status_until(&world.socket, |reply| status_sessions(reply) == ["session-a"]).await;
    assert!(world.server.tmux(&["kill-server"]).status.success());
    world.server.new_session("main", std::path::Path::new("/tmp"));
    let new_pane = first_pane(&world.server, "main");
    tag(&world.server, &new_pane, "session-b");
    send_line(&world.socket, agent_line(&world.server, &new_pane, 2, "session-b", "prompt-1", json!({ "type": "turn_started" }))).await;
    status_until(&world.socket, |reply| status_sessions(reply) == ["session-b"]).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let later = send_line(&world.socket, query_line(3, "status")).await;
    assert_eq!(status_sessions(&later), ["session-b"], "later snapshots keep the new server's session: {later}");
    world.daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_that_is_gone_ends_its_sessions() {
    let _serial = one_daemon_at_a_time().await;
    let world = spawn_daemon_setup("an-gone", snapshotting());
    let pane = first_pane(&world.server, "main");
    tag(&world.server, &pane, "session-a");
    send_line(&world.socket, agent_line(&world.server, &pane, 1, "session-a", "prompt-1", json!({ "type": "turn_started" }))).await;
    status_until(&world.socket, |reply| status_sessions(reply) == ["session-a"]).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let alive = send_line(&world.socket, query_line(2, "status")).await;
    assert_eq!(status_sessions(&alive), ["session-a"], "a live tagged pane survives snapshots: {alive}");
    assert!(world.server.tmux(&["kill-server"]).status.success());
    status_until(&world.socket, |reply| reply["ok"] == true && status_sessions(reply).is_empty()).await;
    world.daemon.stop().await;
}

fn wait_for_reads(presence: &SlowPresence, count: u32) {
    tokio::task::block_in_place(|| eventually(&count.to_string(), || presence.reads.load(Ordering::SeqCst).to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_full_request_queue_refuses_at_the_socket_while_a_query_answers() {
    let _serial = one_daemon_at_a_time().await;
    let presence = Arc::new(SlowPresence { delay: Duration::from_secs(2), ..SlowPresence::default() });
    let base = DaemonSetup::default();
    let world = spawn_daemon_setup("an-full", DaemonSetup { presence: presence.clone(), config: DaemonConfig { intake_capacity: 1, ..base.config }, ..base });
    let pane = first_pane(&world.server, "main");
    let [first, second, third] = [1, 2, 3].map(|number| agent_line(&world.server, &pane, number, "session-a", "prompt-1", json!({ "type": "turn_ended" })));
    assert_eq!(send_line(&world.socket, first).await["ok"], true);
    wait_for_reads(&presence, 1);
    assert_eq!(send_line(&world.socket, second).await["ok"], true, "one request waits in the queue");
    let refused = send_line(&world.socket, third).await;
    assert_eq!(refused, serde_json::Value::Null, "a full queue must close with no reply, so the client falls back: {refused}");
    let status = send_line(&world.socket, query_line(4, "status")).await;
    assert_eq!(status["ok"], true, "a query still answers: {status}");
    world.daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn stop_applies_a_control_acknowledged_just_before_it() {
    let _serial = one_daemon_at_a_time().await;
    let presence = Arc::new(SlowPresence { delay: Duration::from_secs(1), ..SlowPresence::default() });
    let world = spawn_daemon_setup("an-drain", DaemonSetup { presence: presence.clone(), ..DaemonSetup::default() });
    let pane = first_pane(&world.server, "main");
    send_line(&world.socket, agent_line(&world.server, &pane, 1, "session-a", "prompt-1", json!({ "type": "turn_ended" }))).await;
    wait_for_reads(&presence, 1);
    let reply = send_line(&world.socket, control_line(2, json!({ "type": "dnd", "command": { "type": "on" } }))).await;
    assert_eq!(reply["ok"], true, "{reply}");
    world.daemon.stop().await;
    assert_eq!(DndStore::new(world.dnd_path.clone()).load(), Dnd::On { until: None }, "an acknowledged DND change survives stop");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lost_pane_state_write_heals_on_the_next_snapshot() {
    let _serial = one_daemon_at_a_time().await;
    let stubs = tempfile::Builder::new().prefix("an-").tempdir_in("/tmp").expect("a directory under /tmp");
    let fail_first_write = r#"if [ "$4" = set-option ] && [ ! -e "$DIR/failed-once" ]; then : > "$DIR/failed-once"; exit 1; fi"#;
    let world = spawn_daemon_setup("an-heal", DaemonSetup { tmux_binary: Some(tmux_stub(stubs.path(), fail_first_write)), ..snapshotting() });
    let pane = first_pane(&world.server, "main");
    tag(&world.server, &pane, "session-a");
    send_line(&world.socket, agent_line(&world.server, &pane, 1, "session-a", "prompt-1", json!({ "type": "turn_started" }))).await;
    let server = &world.server;
    tokio::task::block_in_place(|| eventually("working", || pane_option(server, &pane, PANE_STATE_OPTION)));
    assert!(stubs.path().join("failed-once").exists(), "the first write failed, so a later write healed it");
    world.daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_snapshot_keeps_the_sessions() {
    let _serial = one_daemon_at_a_time().await;
    let stubs = tempfile::Builder::new().prefix("an-").tempdir_in("/tmp").expect("a directory under /tmp");
    let fail_snapshots = r#"if [ "$4 $5" = "list-panes -a" ]; then echo x >> "$DIR/snapshots"; echo "boom" >&2; exit 1; fi"#;
    let world = spawn_daemon_setup("an-failsnap", DaemonSetup { tmux_binary: Some(tmux_stub(stubs.path(), fail_snapshots)), ..snapshotting() });
    let pane = first_pane(&world.server, "main");
    send_line(&world.socket, agent_line(&world.server, &pane, 1, "session-a", "prompt-1", json!({ "type": "turn_started" }))).await;
    status_until(&world.socket, |reply| status_sessions(reply) == ["session-a"]).await;
    let snapshots = stubs.path().join("snapshots");
    let attempted = || std::fs::read_to_string(&snapshots).map_or(0, |text| text.lines().count()) >= 2;
    tokio::task::block_in_place(|| eventually("true", || attempted().to_string()));
    let later = send_line(&world.socket, query_line(2, "status")).await;
    assert_eq!(status_sessions(&later), ["session-a"], "a snapshot that failed reaps nothing: {later}");
    world.daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn snapshots_keep_their_pace_when_the_wall_clock_steps_back() {
    let _serial = one_daemon_at_a_time().await;
    let wall = Arc::new(std::sync::atomic::AtomicI64::new(1_760_500_000_000));
    let clock = Arc::clone(&wall);
    let world = spawn_daemon_setup("an-clock", DaemonSetup { clock: Arc::new(move || EpochMs(clock.fetch_sub(1_000, Ordering::SeqCst))), ..snapshotting() });
    let pane = first_pane(&world.server, "main");
    tag(&world.server, &pane, "session-a");
    send_line(&world.socket, agent_line(&world.server, &pane, 1, "session-a", "prompt-1", json!({ "type": "turn_started" }))).await;
    status_until(&world.socket, |reply| status_sessions(reply) == ["session-a"]).await;
    assert!(world.server.tmux(&["kill-server"]).status.success());
    status_until(&world.socket, |reply| reply["ok"] == true && status_sessions(reply).is_empty()).await;
    world.daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn stop_returns_while_a_worker_job_still_runs() {
    let _serial = one_daemon_at_a_time().await;
    let raiser = Arc::new(RecordingRaiser { delay: Duration::from_secs(8), ..RecordingRaiser::default() });
    let world = spawn_daemon_setup("an-hang", DaemonSetup { raiser: raiser.clone(), ..DaemonSetup::default() });
    let pane = new_window(&world.server);
    tag(&world.server, &pane, "session-a");
    send_line(&world.socket, agent_line(&world.server, &pane, 1, "session-a", "prompt-1", json!({ "type": "turn_ended" }))).await;
    let view = tokio::task::block_in_place(|| world.presenter.wait_for(|view| !view.cards.is_empty()));
    world.daemon.send_ui(UiIntent::Clicked { card_id: view.cards[0].card_id.clone() });
    tokio::task::block_in_place(|| eventually("1", || raiser.attaches.lock().expect("unpoisoned").len().to_string()));
    let stopping = Instant::now();
    world.daemon.stop().await;
    let waited = stopping.elapsed();
    assert!(waited < Duration::from_millis(4_500), "stop waited {waited:?} for a hung job");
    assert!(waited >= Duration::from_millis(2_500), "stop gave the job its 3 s: {waited:?}");
    assert!(!world.socket.exists(), "the socket is gone even though the worker is still busy");
}
