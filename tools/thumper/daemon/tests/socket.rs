//! The socket server, end to end over a real Unix socket.

use std::collections::BTreeSet;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use thumper_core::wire::{MAX_LINE_BYTES, MAX_REPLY_BYTES, Reply, encode_line};
use thumper_core::wire::Request;
use thumper_daemon::socket::{BindOutcome, Intake, Listener, PendingQuery, ServeOptions, bind, serve};
use tokio::sync::mpsc;

const JUMP: &str = r#"{"type":"control","v":1,"event_id":"000000000000000000000S0CK1","sent_at_ms":1760500000017,"control":{"type":"jump"}}"#;
const STATUS: &str = r#"{"type":"query","v":1,"event_id":"000000000000000000000S0CK2","query":"status"}"#;
const STAMP: &str = "stamp-1";
const LOCK_FILE: &str = "thumperd.lock";
const SOCKET_FILE: &str = "thumper.sock";

struct World {
    directory: tempfile::TempDir,
    socket: PathBuf,
    requests: mpsc::Receiver<Request>,
    queries: mpsc::Receiver<PendingQuery>,
    server: tokio::task::JoinHandle<io::Result<()>>,
}

/// What a client saw: the read's result (`Ok(0)` when the daemon closed
/// with no reply) and the reply text.
struct Exchange {
    read: Result<usize, io::ErrorKind>,
    reply: String,
}

fn options() -> ServeOptions {
    ServeOptions { line_timeout: Duration::from_millis(300), query_timeout: Duration::from_millis(300), max_connections: 8 }
}

fn scratch() -> tempfile::TempDir {
    tempfile::Builder::new().prefix("an-").tempdir_in("/tmp").expect("a directory under /tmp")
}

fn start() -> World {
    start_with(options())
}

fn start_with(options: ServeOptions) -> World {
    let directory = scratch();
    let socket = directory.path().join("run").join(SOCKET_FILE);
    let BindOutcome::Bound(listener) = bind(&socket).expect("binds") else { panic!("nothing else runs here") };
    start_bound(directory, socket, listener, options, 16)
}

fn start_bound(directory: tempfile::TempDir, socket: PathBuf, listener: Listener, options: ServeOptions, capacity: usize) -> World {
    let (request_sender, requests) = mpsc::channel(capacity);
    let (query_sender, queries) = mpsc::channel(capacity);
    let server = tokio::spawn(serve(listener, STAMP.to_string(), Intake { requests: request_sender, queries: query_sender }, options));
    World { directory, socket, requests, queries, server }
}

fn exchange(socket: &Path, line: &[u8]) -> Exchange {
    let mut stream = UnixStream::connect(socket).expect("connects");
    stream.set_read_timeout(Some(Duration::from_secs(2))).expect("sets a timeout");
    stream.write_all(line).expect("writes");
    let mut reply = String::new();
    let read = BufReader::new(&stream).read_line(&mut reply).map_err(|error| error.kind());
    Exchange { read, reply }
}

async fn exchange_async(socket: PathBuf, line: Vec<u8>) -> Exchange {
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

fn entries(directory: &Path) -> BTreeSet<String> {
    std::fs::read_dir(directory)
        .expect("lists")
        .map(|entry| entry.expect("an entry").file_name().to_string_lossy().into_owned())
        .collect()
}

fn jump_padded_to(length: usize) -> Vec<u8> {
    let open = &JUMP[..JUMP.len() - 1];
    let padding = " ".repeat(length - JUMP.len());
    format!("{open}{padding}}}\n").into_bytes()
}

/// Query data whose reply line, newline included, is exactly `length` bytes.
fn data_for_reply_of(length: usize) -> serde_json::Value {
    let line = |blob: &str| encode_line(&Reply { data: Some(serde_json::json!({ "blob": blob })), ..Reply::accepted(STAMP) }).expect("encodes");
    let blob = "x".repeat(length - line("").len());
    assert_eq!(line(&blob).len(), length);
    serde_json::json!({ "blob": blob })
}

async fn ask_and_answer(world: &mut World, data: serde_json::Value) -> Exchange {
    let asking = tokio::spawn(exchange_async(world.socket.clone(), format!("{STATUS}\n").into_bytes()));
    let Some(PendingQuery { answer, .. }) = world.queries.recv().await else { panic!("a query") };
    answer.send(data).expect("the server waits");
    asking.await.expect("joins")
}

#[tokio::test(flavor = "multi_thread")]
async fn an_agent_line_is_acknowledged_and_forwarded() {
    let mut world = start();
    let reply = exchange_async(world.socket.clone(), format!("{JUMP}\n").into_bytes()).await.reply;
    assert_eq!(serde_json::from_str::<serde_json::Value>(&reply).expect("JSON")["ok"], true, "{reply}");
    assert!(reply.contains(STAMP), "{reply}");
    assert!(world.requests.recv().await.is_some());
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_full_request_queue_closes_with_no_reply_and_a_query_still_gets_through() {
    let directory = scratch();
    let socket = directory.path().join("run").join(SOCKET_FILE);
    let BindOutcome::Bound(listener) = bind(&socket).expect("binds") else { panic!("nothing else runs here") };
    let mut world = start_bound(directory, socket, listener, options(), 1);
    let queued = exchange_async(world.socket.clone(), format!("{JUMP}\n").into_bytes()).await;
    assert!(queued.reply.contains("\"ok\":true"), "{}", queued.reply);
    let refused = exchange_async(world.socket.clone(), format!("{JUMP}\n").into_bytes()).await;
    assert_eq!((refused.read, refused.reply.as_str()), (Ok(0), ""), "a full queue closes with no reply, so the client falls back");
    let asked = ask_and_answer(&mut world, serde_json::json!({ "answer": 7 })).await.reply;
    assert!(asked.contains("\"answer\":7"), "{asked}");
    assert!(world.requests.recv().await.is_some());
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_query_is_answered_with_data() {
    let mut world = start();
    let reply = ask_and_answer(&mut world, serde_json::json!({ "answer": 42 })).await.reply;
    assert_eq!(serde_json::from_str::<serde_json::Value>(&reply).expect("JSON")["data"]["answer"], 42, "{reply}");
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unanswered_query_closes_without_a_reply() {
    let mut world = start();
    let asking = tokio::spawn(exchange_async(world.socket.clone(), format!("{STATUS}\n").into_bytes()));
    let held = world.queries.recv().await;
    let asked = asking.await.expect("joins");
    assert_eq!(asked.read, Ok(0), "the daemon closed the connection, the client did not time out");
    assert_eq!(asked.reply, "");
    drop(held);
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn an_answer_of_exactly_the_reply_limit_is_sent() {
    let mut world = start();
    let asked = ask_and_answer(&mut world, data_for_reply_of(MAX_REPLY_BYTES)).await;
    assert_eq!(asked.read, Ok(MAX_REPLY_BYTES));
    assert!(asked.reply.ends_with('\n') && asked.reply.contains("\"ok\":true"));
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn an_answer_longer_than_the_cli_reads_closes_without_a_reply() {
    let mut world = start();
    let asked = ask_and_answer(&mut world, data_for_reply_of(MAX_REPLY_BYTES + 1)).await;
    assert_eq!(asked.read, Ok(0), "the daemon closed the connection, the client did not time out");
    assert_eq!(asked.reply, "");
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_malformed_line_is_refused_with_bad_request() {
    let world = start();
    let reply = exchange_async(world.socket.clone(), b"{nope\n".to_vec()).await.reply;
    assert!(reply.contains("\"code\":\"bad_request\""), "{reply}");
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn an_oversized_line_is_refused_and_other_clients_are_served() {
    let mut world = start();
    let mut huge = vec![b'x'; MAX_LINE_BYTES + 10];
    huge.push(b'\n');
    let refused = exchange_async(world.socket.clone(), huge).await.reply;
    assert!(refused.contains("\"code\":\"line_too_long\""), "{refused}");
    let served = exchange_async(world.socket.clone(), format!("{JUMP}\n").into_bytes()).await.reply;
    assert!(served.contains("\"ok\":true"), "{served}");
    assert!(world.requests.recv().await.is_some());
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn an_oversized_line_is_refused_before_it_ends() {
    let world = start();
    let unending = vec![b'x'; MAX_LINE_BYTES + 10];
    let refused = exchange_async(world.socket.clone(), unending).await.reply;
    assert!(refused.contains("\"code\":\"line_too_long\""), "the daemon stops reading at the limit: {refused}");
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_line_of_exactly_the_limit_is_accepted_and_one_byte_more_is_not() {
    let mut world = start();
    let at_limit = jump_padded_to(MAX_LINE_BYTES);
    assert_eq!(at_limit.len(), MAX_LINE_BYTES + 1, "the limit plus the newline");
    let accepted = exchange_async(world.socket.clone(), at_limit).await.reply;
    assert!(accepted.contains("\"ok\":true"), "{accepted}");
    assert!(world.requests.recv().await.is_some());
    let refused = exchange_async(world.socket.clone(), jump_padded_to(MAX_LINE_BYTES + 1)).await.reply;
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
    let served = exchange_async(world.socket.clone(), format!("{JUMP}\n").into_bytes()).await.reply;
    assert!(served.contains("\"ok\":true"), "a silent client must not block others: {served}");
    let (read, reply) = silent.await.expect("joins");
    assert_eq!(read, Ok(0), "the daemon closed the connection, it did not wait for the client's 3 s timeout");
    assert_eq!(reply, "");
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn stopping_the_server_stops_its_connections() {
    let mut world = start_with(ServeOptions { line_timeout: Duration::from_secs(5), ..options() });
    let silent = UnixStream::connect(&world.socket).expect("connects");
    let served = exchange_async(world.socket.clone(), format!("{JUMP}\n").into_bytes()).await.reply;
    assert!(served.contains("\"ok\":true"), "the silent client was accepted first: {served}");
    assert!(world.requests.recv().await.is_some());
    world.server.abort();
    let closed = tokio::time::timeout(Duration::from_secs(1), world.requests.recv()).await;
    assert!(matches!(closed, Ok(None)), "the silent client's handler stopped with the server");
    drop(silent);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stale_socket_file_is_replaced() {
    let directory = scratch();
    let run = existing_directory(&directory, 0o700);
    let socket = run.join(SOCKET_FILE);
    drop(std::os::unix::net::UnixListener::bind(&socket).expect("a stale socket"));
    assert!(socket.exists(), "the stale file stays after the listener closes");
    assert!(matches!(bind(&socket).expect("binds"), BindOutcome::Bound(_)));
}

#[test]
fn a_regular_file_at_the_socket_path_is_refused_and_kept() {
    let directory = scratch();
    let run = existing_directory(&directory, 0o700);
    let socket = run.join(SOCKET_FILE);
    std::fs::write(&socket, "not a socket").expect("writes");
    let error = bind(&socket).expect_err("refuses a regular file");
    assert!(error.to_string().contains(&socket.display().to_string()), "{error}");
    assert_eq!(std::fs::read_to_string(&socket).expect("still there"), "not a socket");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_instance_sees_the_first() {
    let world = start();
    assert!(matches!(bind(&world.socket).expect("checks"), BindOutcome::AlreadyRunning));
    let reply = exchange_async(world.socket.clone(), format!("{JUMP}\n").into_bytes()).await.reply;
    assert!(reply.contains("\"ok\":true"), "the first daemon still answers: {reply}");
    world.server.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_socket_its_lock_and_its_directory_are_private() {
    let world = start();
    let run = world.socket.parent().expect("a parent");
    assert_eq!(mode(&world.socket), 0o600);
    assert_eq!(mode(&run.join(LOCK_FILE)), 0o600);
    assert_eq!(mode(run), 0o700);
    drop(world.directory);
}

#[test]
fn bind_leaves_only_the_lock_and_the_socket() {
    let directory = scratch();
    let run = existing_directory(&directory, 0o755);
    let outcome = bind(&run.join(SOCKET_FILE)).expect("binds");
    assert!(matches!(outcome, BindOutcome::Bound(_)));
    assert_eq!(entries(&run), BTreeSet::from([LOCK_FILE.to_string(), SOCKET_FILE.to_string()]));
}

#[test]
fn a_leftover_probe_with_this_pid_does_not_block_bind() {
    let directory = scratch();
    let run = existing_directory(&directory, 0o700);
    let leftover = format!(".thumperd-probe-{}", std::process::id());
    std::fs::write(run.join(&leftover), "").expect("writes");
    let outcome = bind(&run.join(SOCKET_FILE)).expect("binds past a probe a crash left behind");
    assert!(matches!(outcome, BindOutcome::Bound(_)));
    assert_eq!(entries(&run), BTreeSet::from([leftover, LOCK_FILE.to_string(), SOCKET_FILE.to_string()]), "only this bind's own probe is removed");
}

#[test]
fn an_existing_lock_file_open_to_others_is_made_private() {
    let directory = scratch();
    let run = existing_directory(&directory, 0o755);
    let lock = run.join(LOCK_FILE);
    std::fs::write(&lock, "").expect("writes");
    std::fs::set_permissions(&lock, std::fs::Permissions::from_mode(0o644)).expect("sets the mode");
    let outcome = bind(&run.join(SOCKET_FILE)).expect("binds");
    assert!(matches!(outcome, BindOutcome::Bound(_)));
    assert_eq!(mode(&lock), 0o600);
}

#[test]
fn an_existing_directory_closed_to_others_is_used_unchanged() {
    let directory = scratch();
    let run = existing_directory(&directory, 0o755);
    let outcome = bind(&run.join(SOCKET_FILE)).expect("accepts a 0755 directory");
    assert!(matches!(outcome, BindOutcome::Bound(_)));
    assert_eq!(mode(&run), 0o755);
    assert_eq!(mode(&run.join(LOCK_FILE)), 0o600);
}

#[test]
fn an_existing_directory_writable_by_others_is_refused_and_unchanged() {
    let directory = scratch();
    let run = existing_directory(&directory, 0o777);
    let socket = run.join(SOCKET_FILE);
    let error = bind(&socket).expect_err("refuses a 0777 directory");
    let message = error.to_string();
    assert!(message.contains(&run.display().to_string()) && message.contains("0777"), "{message}");
    assert_eq!(mode(&run), 0o777);
    assert!(entries(&run).is_empty(), "nothing was created: {:?}", entries(&run));
}

#[tokio::test(flavor = "multi_thread")]
async fn stopping_the_server_removes_the_socket_file() {
    let world = start();
    world.server.abort();
    let _ = world.server.await;
    assert!(!world.socket.exists());
}
