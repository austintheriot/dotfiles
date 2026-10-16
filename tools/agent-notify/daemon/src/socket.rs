//! The daemon's Unix socket: single instance, private permissions, one
//! request per connection, and a bounded line. See the contract in the
//! plan, Task 11.

use std::fs::{self, DirBuilder, File, Permissions, TryLockError};
use std::io;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use agent_notify_core::wire::{MAX_LINE_BYTES, MAX_REPLY_BYTES, Query, Reply, ReplyCode, Request, decode_request, encode_line};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{Semaphore, mpsc, oneshot};
use tokio::time::timeout;

const LOCK_FILE: &str = "agent-notifyd.lock";
const PRIVATE_DIRECTORY: u32 = 0o700;
const PRIVATE_SOCKET: u32 = 0o600;
const WRITABLE_BY_OTHERS: u32 = 0o022;

/// The result of [`bind`].
#[derive(Debug)]
pub enum BindOutcome {
    /// This process owns the socket.
    Bound(Listener),
    /// Another daemon holds the lock. Exit without touching its socket.
    AlreadyRunning,
}

/// A bound socket and the instance lock. Dropping it removes the socket
/// file, then releases the lock.
#[derive(Debug)]
pub struct Listener {
    listener: std::os::unix::net::UnixListener,
    path: PathBuf,
    owner: u32,
    _lock: File,
}

impl Drop for Listener {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.path) {
            tracing::debug!(%error, path = %self.path.display(), "the socket file was already gone");
        }
    }
}

/// Takes the instance lock, replaces a stale socket file and binds.
///
/// A missing socket directory is created with mode 0700. An existing one is
/// used unchanged, and only when this process's user owns it and neither
/// group nor other can write to it.
///
/// # Errors
///
/// An unsafe socket directory, a non-socket file at `path`, or any
/// filesystem or bind error.
pub fn bind(path: &Path) -> io::Result<BindOutcome> {
    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("the socket path {} has no directory", path.display())))?;
    prepare_directory(directory)?;
    let lock = File::options().create(true).truncate(false).write(true).open(directory.join(LOCK_FILE))?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => return Ok(BindOutcome::AlreadyRunning),
        Err(TryLockError::Error(error)) => return Err(error),
    }
    remove_stale_socket(path)?;
    let listener = std::os::unix::net::UnixListener::bind(path)?;
    let bound = Listener { listener, path: path.to_path_buf(), owner: fs::symlink_metadata(path)?.uid(), _lock: lock };
    fs::set_permissions(path, Permissions::from_mode(PRIVATE_SOCKET))?;
    bound.listener.set_nonblocking(true)?;
    check_owner(directory, bound.owner)?;
    Ok(BindOutcome::Bound(bound))
}

fn prepare_directory(directory: &Path) -> io::Result<()> {
    if let Some(above) = directory.parent().filter(|above| !above.as_os_str().is_empty()) {
        fs::create_dir_all(above)?;
    }
    match DirBuilder::new().mode(PRIVATE_DIRECTORY).create(directory) {
        Ok(()) => fs::set_permissions(directory, Permissions::from_mode(PRIVATE_DIRECTORY)),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => check_closed_to_others(directory),
        Err(error) => Err(error),
    }
}

fn check_closed_to_others(directory: &Path) -> io::Result<()> {
    let metadata = fs::metadata(directory)?;
    let mode = metadata.mode() & 0o7777;
    if !metadata.is_dir() {
        return Err(unsafe_directory(directory, mode, "is not a directory"));
    }
    if mode & WRITABLE_BY_OTHERS != 0 {
        return Err(unsafe_directory(directory, mode, "is writable by group or other"));
    }
    Ok(())
}

fn check_owner(directory: &Path, owner: u32) -> io::Result<()> {
    let metadata = fs::metadata(directory)?;
    if metadata.uid() != owner {
        let reason = format!("is owned by uid {}, not this process's uid {owner}", metadata.uid());
        return Err(unsafe_directory(directory, metadata.mode() & 0o7777, &reason));
    }
    Ok(())
}

fn unsafe_directory(directory: &Path, mode: u32, reason: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!("the socket directory {} (mode {mode:04o}) {reason}; refusing to use it", directory.display()),
    )
}

fn remove_stale_socket(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
        Ok(metadata) if metadata.file_type().is_socket() => {
            fs::remove_file(path)?;
            tracing::info!(path = %path.display(), "replaced a stale socket file");
            Ok(())
        }
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} exists and is not a socket; refusing to remove it", path.display()),
        )),
    }
}

/// A decoded request on its way to the daemon.
#[derive(Debug)]
// One value per queue hop, as in the core's `Request`. Boxing would change the actor's match for no gain.
#[allow(clippy::large_enum_variant)]
pub enum Inbound {
    /// An agent or control request. Already acknowledged.
    Request(Request),
    /// A query. The connection waits for `answer`.
    Query {
        /// What is asked.
        query: Query,
        /// Where the answer goes.
        answer: oneshot::Sender<serde_json::Value>,
    },
}

/// Limits for [`serve`].
#[derive(Debug, Clone, Copy)]
pub struct ServeOptions {
    /// How long a client has to send its line.
    pub line_timeout: Duration,
    /// How long a query waits for the actor. The CLI gives a query 1 s for
    /// connect, write and read together, so this stays below that: an
    /// answer that arrives must still have time to reach the CLI.
    pub query_timeout: Duration,
    /// Connections handled at once. More are closed at accept.
    pub max_connections: usize,
}

impl Default for ServeOptions {
    fn default() -> Self {
        ServeOptions { line_timeout: Duration::from_secs(5), query_timeout: Duration::from_millis(800), max_connections: 64 }
    }
}

struct Context {
    stamp: String,
    inbound: mpsc::Sender<Inbound>,
    options: ServeOptions,
    owner: u32,
}

/// Accepts connections until the task running it is dropped. Must run
/// inside a Tokio runtime.
///
/// # Errors
///
/// When the listener cannot join the runtime.
pub async fn serve(listener: Listener, stamp: String, inbound: mpsc::Sender<Inbound>, options: ServeOptions) -> io::Result<()> {
    let accepting = tokio::net::UnixListener::from_std(listener.listener.try_clone()?)?;
    let limit = Arc::new(Semaphore::new(options.max_connections));
    let context = Arc::new(Context { stamp, inbound, options, owner: listener.owner });
    let _owned = listener;
    loop {
        let stream = match accepting.accept().await {
            Ok((stream, _)) => stream,
            Err(error) => {
                tracing::warn!(%error, "accept failed");
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
        };
        let Ok(permit) = Arc::clone(&limit).try_acquire_owned() else {
            tracing::warn!("too many connections; closed one at accept");
            continue;
        };
        let context = Arc::clone(&context);
        tokio::spawn(async move {
            handle(stream, &context).await;
            drop(permit);
        });
    }
}

async fn handle(stream: UnixStream, context: &Context) {
    match stream.peer_cred() {
        Ok(credentials) if credentials.uid() == context.owner => {}
        Ok(credentials) => {
            tracing::warn!(uid = credentials.uid(), "refused a peer of another user");
            return;
        }
        Err(error) => {
            tracing::warn!(%error, "refused a peer whose credentials did not read");
            return;
        }
    }
    let (read_half, mut write_half) = stream.into_split();
    let (bytes, overflowed) = match timeout(context.options.line_timeout, read_line(read_half)).await {
        Ok(Ok(read)) => read,
        Ok(Err(error)) => {
            tracing::debug!(%error, "a client read failed");
            return;
        }
        Err(_) => {
            tracing::info!("a client sent no line in time");
            return;
        }
    };
    let Some(reply) = answer(&bytes, overflowed, context).await else { return };
    let line = match encode_line(&reply) {
        Ok(line) => line,
        Err(error) => {
            tracing::error!(%error, "a reply did not encode");
            return;
        }
    };
    if line.len() > MAX_REPLY_BYTES {
        tracing::error!(bytes = line.len(), limit = MAX_REPLY_BYTES, "a reply is longer than the CLI reads; closed with no reply");
        return;
    }
    if let Ok(Err(error)) = timeout(context.options.line_timeout, write_half.write_all(line.as_bytes())).await {
        tracing::debug!(%error, "a reply write failed");
    }
}

async fn read_line(read_half: tokio::net::unix::OwnedReadHalf) -> io::Result<(Vec<u8>, bool)> {
    let limit = u64::try_from(MAX_LINE_BYTES).unwrap_or(u64::MAX).saturating_add(1);
    let mut bytes = Vec::new();
    BufReader::new(read_half.take(limit)).read_until(b'\n', &mut bytes).await?;
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
        return Ok((bytes, false));
    }
    let overflowed = bytes.len() > MAX_LINE_BYTES;
    Ok((bytes, overflowed))
}

async fn answer(bytes: &[u8], overflowed: bool, context: &Context) -> Option<Reply> {
    match classify(bytes, overflowed) {
        Err(code) => Some(Reply::rejected(code)),
        Ok(Request::Query { query, .. }) => {
            let (sender, receiver) = oneshot::channel();
            enqueue(&context.inbound, Inbound::Query { query, answer: sender })?;
            match timeout(context.options.query_timeout, receiver).await {
                Ok(Ok(data)) => Some(Reply { data: Some(data), ..Reply::accepted(&context.stamp) }),
                Ok(Err(_)) | Err(_) => {
                    tracing::warn!("a query got no answer in time; closed with no reply");
                    None
                }
            }
        }
        Ok(request) => {
            enqueue(&context.inbound, Inbound::Request(request))?;
            Some(Reply::accepted(&context.stamp))
        }
    }
}

fn enqueue(inbound: &mpsc::Sender<Inbound>, item: Inbound) -> Option<()> {
    match inbound.try_send(item) {
        Ok(()) => Some(()),
        Err(TrySendError::Full(_)) => {
            tracing::error!("the intake queue is full; closed with no reply, so the client falls back");
            None
        }
        Err(TrySendError::Closed(_)) => {
            tracing::error!("the intake queue is closed; closed with no reply, so the client falls back");
            None
        }
    }
}

/// Decodes one line, or names the reply code that refuses it.
///
/// # Errors
///
/// The [`ReplyCode`] for an overlong, non-UTF-8 or invalid line.
pub fn classify(bytes: &[u8], overflowed: bool) -> Result<Request, ReplyCode> {
    if overflowed {
        return Err(ReplyCode::LineTooLong);
    }
    let line = std::str::from_utf8(bytes).map_err(|_| ReplyCode::BadRequest)?;
    decode_request(line).map_err(|error| error.reply_code())
}

#[cfg(test)]
mod tests {
    use agent_notify_core::wire::{MAX_LINE_BYTES, ReplyCode};

    use super::*;

    const JUMP: &str = r#"{"type":"control","v":1,"event_id":"000000000000000000000S0CK1","sent_at_ms":1760500000017,"control":{"type":"jump"}}"#;

    #[test]
    fn a_valid_line_classifies_as_its_request() {
        assert!(matches!(classify(JUMP.as_bytes(), false), Ok(Request::Control { .. })));
    }

    #[test]
    fn bad_lines_map_to_reply_codes() {
        assert_eq!(classify(b"not json", false).err(), Some(ReplyCode::BadRequest));
        assert_eq!(classify(&[0xff, 0xfe], false).err(), Some(ReplyCode::BadRequest));
        assert_eq!(classify(&vec![b'x'; MAX_LINE_BYTES + 1], true).err(), Some(ReplyCode::LineTooLong));
        let future = JUMP.replace(r#""v":1"#, r#""v":9"#);
        assert_eq!(classify(future.as_bytes(), false).err(), Some(ReplyCode::UnsupportedVersion));
        let unknown = JUMP.replace(r#""type":"jump""#, r#""type":"teleport""#);
        assert_eq!(classify(unknown.as_bytes(), false).err(), Some(ReplyCode::UnknownKind));
    }

    #[test]
    fn a_query_waits_less_than_the_cli_budget_by_default() {
        let defaults = ServeOptions::default();
        assert_eq!(defaults.query_timeout, Duration::from_millis(800));
        assert_eq!(defaults.line_timeout, Duration::from_secs(5));
        assert_eq!(defaults.max_connections, 64);
    }
}
