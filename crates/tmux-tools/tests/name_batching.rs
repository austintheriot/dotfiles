//! Pins the query shape of `tmux-tools name-windows`, converted from
//! `tests/tmux-name-batching.test.sh`.
//!
//! `-a` and `-s` used to read each window's state with its own
//! `display-message -p -t`, one call per window, so a session with 21
//! windows paid 21 round trips where one `list-windows -F` returns the same
//! fields for all of them. Measured at 287ms against 16ms.
//!
//! Behaviour is covered by `update_window_names.rs`. This file asserts only
//! the property that makes the binary cheap: the number of tmux invocations
//! does not grow with the number of windows. That is not visible in the
//! names produced, so nothing else can catch its loss, and the per-window
//! loop is an easy thing to reintroduce while every behavioural test stays
//! green.
//!
//! tmux is stubbed rather than driven. Counting real invocations means
//! counting processes, and the point is the count itself, not what tmux does
//! with them.

mod support;

use std::path::Path;
use std::process::Command;

/// One stubbed `tmux` on `PATH`, plus the files it reads and writes.
///
/// The stub logs every invocation and answers the two read commands the
/// binary issues. Windows are served from a file of `id<TAB>path` pairs, so
/// a test decides how many exist.
struct Stub {
    directory: tempfile::TempDir,
}

impl Stub {
    /// Writes the stub script and makes it executable.
    fn new() -> Self {
        let directory = tempfile::Builder::new()
            .prefix("tt-stub-")
            .tempdir_in("/tmp")
            .expect("a stub directory");
        let script = directory.path().join("tmux");
        std::fs::write(&script, STUB_SCRIPT).expect("the stub is written");
        set_executable(&script);
        Stub { directory }
    }

    /// Runs `tmux-tools name-windows -a` against a session of `windows`
    /// windows, each pointed at `directory`, and returns every tmux
    /// invocation the run made, one per line.
    ///
    /// `current_name` is what the stub reports each window is already
    /// called, which is how the steady-state case is expressed.
    fn calls_for(&self, windows: usize, directory: &Path, current_name: &str) -> Vec<String> {
        let window_list = self.directory.path().join(format!("windows-{windows}"));
        let call_log = self.directory.path().join(format!("calls-{windows}"));
        let rows: String = (1..=windows)
            .map(|index| format!("@{index}\t{}\n", directory.display()))
            .collect();
        std::fs::write(&window_list, rows).expect("the window list is written");
        std::fs::write(&call_log, "").expect("the call log is truncated");

        let path = format!(
            "{}:{}",
            self.directory.path().display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let run = Command::new(env!("CARGO_BIN_EXE_tmux-tools"))
            .args(["name-windows", "-a"])
            .env("PATH", path)
            .env("STUB_WINDOWS", &window_list)
            .env("TMUX_CALL_LOG", &call_log)
            .env("STUB_WINDOW_NAME", current_name)
            // The binary reads a fallback pattern file under $HOME. Point
            // HOME at the stub directory so a developer machine that has
            // one and the container that does not produce the same calls.
            .env("HOME", self.directory.path())
            .output()
            .expect("the binary runs");
        assert!(
            run.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&run.stderr)
        );

        std::fs::read_to_string(&call_log)
            .expect("the call log is readable")
            .lines()
            .map(str::to_string)
            .collect()
    }
}

/// A `tmux` that records what it was asked and answers the reads.
///
/// It substitutes the two fields it tracks into whatever `-F` format it was
/// given and blanks the rest, which is what an unset tmux option expands to.
const STUB_SCRIPT: &str = r#"#!/bin/sh
printf '%s\n' "$*" >> "$TMUX_CALL_LOG"

format=''
prev=''
for arg in "$@"; do
    [ "$prev" = '-F' ] && format=$arg
    prev=$arg
done

emit_windows() {
    while IFS=$(printf '\t') read -r id path; do
        [ -n "$id" ] || continue
        line=$format
        line=$(printf '%s' "$line" | sed \
            -e "s|#{window_id}|$id|g" \
            -e "s|#{pane_current_path}|$path|g" \
            -e "s|#{window_name}|${STUB_WINDOW_NAME:-}|g" \
            -e "s|#{automatic-rename}|1|g" \
            -e "s|#{@wname_auto}|${STUB_WINDOW_NAME:-}|g" \
            -e "s|#{@[a-z_]*}||g")
        printf '%s\n' "$line"
    done < "$STUB_WINDOWS"
}

case ${1:-} in
    list-windows)   emit_windows ;;
    display-message) emit_windows | head -1 ;;
    *) ;;
esac
exit 0
"#;

/// Sets the owner-execute bit on the stub, so `PATH` lookup can run it.
fn set_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)
        .expect("the stub exists")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("the stub is executable");
}

/// The reads are batched: one `list-windows` covers every window, and no
/// per-window `display-message` survives.
///
/// The read path is the one that ran unconditionally on every hook, so it is
/// the one asserted. Writes are per-window by nature, and only happen when a
/// name actually changed, which the steady-state test below covers.
#[test]
fn the_reads_are_batched() {
    let stub = Stub::new();
    let plain = tempfile::Builder::new()
        .prefix("not-a-repo-")
        .tempdir_in("/tmp")
        .expect("a plain directory");

    // Positive control: the run must call tmux at all, or every count
    // assertion below would hold for a binary that made no calls.
    let few = stub.calls_for(2, plain.path(), "");
    assert!(
        !few.is_empty(),
        "the run against 2 windows must have called tmux at all"
    );

    let many = stub.calls_for(20, plain.path(), "");

    let reads = many
        .iter()
        .filter(|call| call.starts_with("display-message"))
        .count();
    assert_eq!(reads, 0, "no per-window display-message read may survive");

    let lists = many
        .iter()
        .filter(|call| call.starts_with("list-windows"))
        .count();
    assert_eq!(lists, 1, "one list-windows call reads every window");

    let lists_few = few
        .iter()
        .filter(|call| call.starts_with("list-windows"))
        .count();
    assert_eq!(
        lists_few, lists,
        "the read count is the same for two windows as for twenty"
    );
}

/// A run that changes nothing costs one tmux call.
///
/// The hooks fire on every pane switch, and almost every one of those finds
/// every name already correct. That run is the one whose cost is felt.
#[test]
fn the_steady_state_costs_one_call() {
    let stub = Stub::new();
    let plain = tempfile::Builder::new()
        .prefix("not-a-repo-")
        .tempdir_in("/tmp")
        .expect("a plain directory");
    let already_named = plain
        .path()
        .file_name()
        .expect("a basename")
        .to_string_lossy()
        .into_owned();

    let calls = stub.calls_for(20, plain.path(), &already_named);
    assert_eq!(
        calls.len(),
        1,
        "a run that changes nothing costs one tmux call, got {calls:?}"
    );
}

/// The batched read asks for every field the naming needs.
///
/// Without this, the binary could be one cheap `list-windows` plus a
/// per-window fallback for whatever the batched call failed to fetch.
#[test]
fn the_batched_read_carries_every_field() {
    let stub = Stub::new();
    let plain = tempfile::Builder::new()
        .prefix("not-a-repo-")
        .tempdir_in("/tmp")
        .expect("a plain directory");

    let calls = stub.calls_for(20, plain.path(), "");
    let list_call = calls
        .iter()
        .find(|call| call.starts_with("list-windows"))
        .expect("a list-windows call");

    for field in [
        "window_id",
        "pane_current_path",
        "window_name",
        "automatic-rename",
    ] {
        assert!(
            list_call.contains(field),
            "the batched read must ask for {field}, got {list_call:?}"
        );
    }
}
