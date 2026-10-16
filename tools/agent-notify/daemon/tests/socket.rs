//! The socket server, end to end over a real Unix socket.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use agent_notify_core::wire::{MAX_LINE_BYTES, MAX_REPLY_BYTES};
use agent_notify_daemon::socket::{BindOutcome, Inbound, ServeOptions, bind, serve};
use tokio::sync::mpsc;

const JUMP: &str = r#"{"type":"control","v":1,"event_id":"000000000000000000000S0CK1","sent_at_ms":1760500000017,"control":{"type":"jump"}}"#;
const STATUS: &str = r#"{"type":"query","v":1,"event_id":"000000000000000000000S0CK2","query":"status"}"#;

struct World {
    directory: tempfile::TempDir,
    socket: PathBuf,
    inbound: mpsc::Receiver<Inbound>,
    server: tokio::task::JoinHandle<std::io::Result<()>>,
}

fn options() -> ServeOptions {
    ServeOptions { line_timeout: Duration::from_millis(300), query_timeout: Duration::from_millis(300), max_connections: 8 }
}

fn scratch() -> tempfile::TempDir {
    tempfile::Builder::new().prefix("an-").tempdir_in("/tmp").expect("a directory under /tmp")
}

fn start() -> World {
    let directory = scratch();
    let socket = directory.path().join("run/agent-notify.sock");
    let BindOutcome::Bound(listener) = bind(&socket).expect("binds") else { panic!("nothing else runs here") };
    let (sender, inbound) = mpsc::channel(16);
    let server = tokio::spawn(serve(listener, "stamp-1".to_string(), sender, options()));
    World { directory, socket, inbound, server }
}

fn exchange(socket: &Path, line: &[u8]) -> String {
    let mut stream = UnixStream::connect(socket).expect("connects");
    stream.set_read_timeout(Some(Duration::from_secs(2))).expect("sets a timeout");
    stream.write_all(line).expect("writes");
    let mut reply = String::new();
    let _ = BufReader::new(&stream).read_line(&mut reply);
    reply
}

async fn exchange_async(socket: PathBuf, line: Vec<u8>) -> String {
    tokio::task::spawn_blocking(move || exchange(&socket, &line)).await.expect("joins")
}

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path).expect("exists").permissions().mode() & 0o777
}

fn existing_directory(scratch: &tempfile::TempDir, permissions: u32) -> PathBuf {
    let directory = scratch.path().join("run");
    std::fs::create_dir(&directory).expect("creates");
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(permissions)).expect("sets the mode");
    directory
}

fn jump_padded_to(length: usize) -> Vec<u8> {
    let open = &JUMP[..JUMP.len() - 1];
    let padding = " ".repeat(length - JUMP.len());
    format!("{open}{padding}}}\n").into_bytes()
}

#[tokio::test(flavor = "multi_thread")]
async fn an_agent_line_is_acknowledged_and_forwarded() {
    let mut world = start();
    let reply = exchange_async(world.socket.clone(), format!("{JUMP}\n").into_bytes()).await;
    assert_eq!(serde_json::from_str::<serde_json::Value>(&reply).expect("JSON")["ok"], true, "{reply}");
    assert!(reply.contains("stamp-1"), "{reply}");
    assert!(matches!(world.inbound.recv().await, Some(Inbound::Request(_))));
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_query_is_answered_with_data() {
    let mut world = start();
    let asking = tokio::spawn(exchange_async(world.socket.clone(), format!("{STATUS}\n").into_bytes()));
    let Some(Inbound::Query { answer, .. }) = world.inbound.recv().await else { panic!("a query") };
    answer.send(serde_json::json!({ "answer": 42 })).expect("the server waits");
    let reply = asking.await.expect("joins");
    assert_eq!(serde_json::from_str::<serde_json::Value>(&reply).expect("JSON")["data"]["answer"], 42, "{reply}");
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unanswered_query_closes_without_a_reply() {
    let mut world = start();
    let asking = tokio::spawn(exchange_async(world.socket.clone(), format!("{STATUS}\n").into_bytes()));
    let held = world.inbound.recv().await;
    assert_eq!(asking.await.expect("joins"), "");
    drop(held);
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn an_answer_longer_than_the_cli_reads_closes_without_a_reply() {
    let mut world = start();
    let asking = tokio::spawn(exchange_async(world.socket.clone(), format!("{STATUS}\n").into_bytes()));
    let Some(Inbound::Query { answer, .. }) = world.inbound.recv().await else { panic!("a query") };
    answer.send(serde_json::json!({ "blob": "x".repeat(MAX_REPLY_BYTES) })).expect("the server waits");
    assert_eq!(asking.await.expect("joins"), "");
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_malformed_line_is_refused_with_bad_request() {
    let world = start();
    let reply = exchange_async(world.socket.clone(), b"{nope\n".to_vec()).await;
    assert!(reply.contains("\"code\":\"bad_request\""), "{reply}");
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn an_oversized_line_is_refused_and_other_clients_are_served() {
    let mut world = start();
    let mut huge = vec![b'x'; MAX_LINE_BYTES + 10];
    huge.push(b'\n');
    let refused = exchange_async(world.socket.clone(), huge).await;
    assert!(refused.contains("\"code\":\"line_too_long\""), "{refused}");
    let served = exchange_async(world.socket.clone(), format!("{JUMP}\n").into_bytes()).await;
    assert!(served.contains("\"ok\":true"), "{served}");
    assert!(world.inbound.recv().await.is_some());
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn an_oversized_line_is_refused_before_it_ends() {
    let world = start();
    let unending = vec![b'x'; MAX_LINE_BYTES + 10];
    let refused = exchange_async(world.socket.clone(), unending).await;
    assert!(refused.contains("\"code\":\"line_too_long\""), "the daemon stops reading at the limit: {refused}");
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_line_of_exactly_the_limit_is_accepted_and_one_byte_more_is_not() {
    let mut world = start();
    let at_limit = jump_padded_to(MAX_LINE_BYTES);
    assert_eq!(at_limit.len(), MAX_LINE_BYTES + 1, "the limit plus the newline");
    let accepted = exchange_async(world.socket.clone(), at_limit).await;
    assert!(accepted.contains("\"ok\":true"), "{accepted}");
    assert!(matches!(world.inbound.recv().await, Some(Inbound::Request(_))));
    let refused = exchange_async(world.socket.clone(), jump_padded_to(MAX_LINE_BYTES + 1)).await;
    assert!(refused.contains("\"code\":\"line_too_long\""), "{refused}");
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_silent_client_is_dropped_after_the_timeout() {
    let world = start();
    let socket = world.socket.clone();
    let silent = tokio::task::spawn_blocking(move || {
        let stream = UnixStream::connect(&socket).expect("connects");
        stream.set_read_timeout(Some(Duration::from_secs(3))).expect("sets a timeout");
        let mut reply = String::new();
        let read = BufReader::new(&stream).read_line(&mut reply);
        (read.map_err(|error| error.kind()), reply)
    });
    let served = exchange_async(world.socket.clone(), format!("{JUMP}\n").into_bytes()).await;
    assert!(served.contains("\"ok\":true"), "a silent client must not block others: {served}");
    let (read, reply) = silent.await.expect("joins");
    assert_eq!(read, Ok(0), "the daemon closed the connection, it did not wait for the client's 3 s timeout");
    assert_eq!(reply, "");
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stale_socket_file_is_replaced() {
    let directory = scratch();
    let run = existing_directory(&directory, 0o700);
    let socket = run.join("agent-notify.sock");
    drop(std::os::unix::net::UnixListener::bind(&socket).expect("a stale socket"));
    assert!(socket.exists(), "the stale file stays after the listener closes");
    assert!(matches!(bind(&socket).expect("binds"), BindOutcome::Bound(_)));
}

#[test]
fn a_regular_file_at_the_socket_path_is_refused_and_kept() {
    let directory = scratch();
    let run = existing_directory(&directory, 0o700);
    let socket = run.join("agent-notify.sock");
    std::fs::write(&socket, "not a socket").expect("writes");
    let error = bind(&socket).expect_err("refuses a regular file");
    assert!(error.to_string().contains(&socket.display().to_string()), "{error}");
    assert_eq!(std::fs::read_to_string(&socket).expect("still there"), "not a socket");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_instance_sees_the_first() {
    let world = start();
    assert!(matches!(bind(&world.socket).expect("checks"), BindOutcome::AlreadyRunning));
    let reply = exchange_async(world.socket.clone(), format!("{JUMP}\n").into_bytes()).await;
    assert!(reply.contains("\"ok\":true"), "the first daemon still answers: {reply}");
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_socket_and_its_directory_are_private() {
    let world = start();
    assert_eq!(mode(&world.socket), 0o600);
    assert_eq!(mode(world.socket.parent().expect("a parent")), 0o700);
    drop(world.directory);
}

#[test]
fn an_existing_directory_closed_to_others_is_used_unchanged() {
    let directory = scratch();
    let run = existing_directory(&directory, 0o755);
    let outcome = bind(&run.join("agent-notify.sock")).expect("accepts a 0755 directory");
    assert!(matches!(outcome, BindOutcome::Bound(_)));
    assert_eq!(mode(&run), 0o755);
}

#[test]
fn an_existing_directory_writable_by_others_is_refused_and_unchanged() {
    let directory = scratch();
    let run = existing_directory(&directory, 0o777);
    let socket = run.join("agent-notify.sock");
    let error = bind(&socket).expect_err("refuses a 0777 directory");
    let message = error.to_string();
    assert!(message.contains(&run.display().to_string()) && message.contains("0777"), "{message}");
    assert_eq!(mode(&run), 0o777);
    assert!(!socket.exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn stopping_the_server_removes_the_socket_file() {
    let world = start();
    world.server.abort();
    let _ = world.server.await;
    assert!(!world.socket.exists());
}
