//! `tmux-setup.sh` appends its windows to the session rather than aiming at
//! an occupied window index.
//!
//! The bug this pins: `create_window` targeted `-t $SESSION_NAME`, and a bare
//! target is ambiguous. tmux matches it against window NAMES as well as
//! session names, and the `after-new-window` hook in `.config/tmux/tmux.conf`
//! names every worktree window after its branch, so each one is called
//! `code/<branch>`. That name has the session name as a prefix, tmux resolved
//! the target to the window, and `new-window` then tried to create a window
//! at that window's index. Every call failed with "create window failed:
//! index N in use", the window never existed, and the four `split-window`
//! calls behind it each reported "can't find window: N". The whole session
//! build failed this way, once per window, 21 times.
//!
//! A trailing colon (`-t code:`) forces session-only resolution.
//!
//! Runs against a throwaway tmux server, never the developer's live one.

mod support;

use support::{Server, repo_root};

/// The name the hook gives a worktree window: the session name is a prefix
/// of it, which is the whole precondition for the bug.
const COLLIDING_WINDOW_NAME: &str = "code/cc/some-branch";
const SESSION: &str = "code";

/// A window whose name starts with the session name does not capture the
/// session target.
///
/// THE TARGET ASSERTION. Asserted against real tmux with a real colliding
/// window name rather than by grepping the script, because the script's text
/// is not its behaviour and tmux's target resolution is the thing under test.
#[test]
fn a_window_named_after_the_session_does_not_capture_the_target() {
    let server = Server::new("setup-target");
    let home = repo_root();
    server.new_session(SESSION, &home);
    server.tmux(&[
        "rename-window",
        "-t",
        &format!("{SESSION}:1"),
        COLLIDING_WINDOW_NAME,
    ]);

    let created = server.tmux(&[
        "new-window",
        "-d",
        "-t",
        &format!("{SESSION}:"),
        "-c",
        home.to_str().expect("a utf-8 home"),
    ]);
    let complaint = String::from_utf8_lossy(&created.stderr);
    assert!(
        created.status.success() && complaint.is_empty(),
        "new-window against a session whose window is named {COLLIDING_WINDOW_NAME} \
         failed: {complaint}"
    );

    let windows = server.stdout(&["list-windows", "-t", SESSION, "-F", "#{window_index}"]);
    let indexes: Vec<&str> = windows.split_whitespace().collect();
    assert_eq!(
        indexes,
        vec!["1", "2"],
        "the window was not appended to the session: {windows}"
    );

    // The panes the script splits into the new window have to find it too:
    // the original failure surfaced as "can't find window: 2" four times per
    // window, after new-window had already failed.
    let split = server.tmux(&[
        "split-window",
        "-d",
        "-t",
        &format!("{SESSION}:2"),
        "-c",
        home.to_str().expect("a utf-8 home"),
    ]);
    assert!(
        split.status.success(),
        "split-window could not find the new window: {}",
        String::from_utf8_lossy(&split.stderr)
    );
}

/// The script still carries the trailing colon.
///
/// The behavioural assertion above proves tmux's rule. This one proves the
/// script uses it, so restoring the bare target fails here rather than
/// silently reintroducing a 21-window failure nothing catches until the next
/// `se`.
#[test]
fn the_setup_script_targets_the_session_not_a_window() {
    let script = repo_root().join(".scripts/tmux-setup.sh");
    let text = std::fs::read_to_string(&script)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", script.display()));

    assert!(
        text.contains("new-window -t $SESSION_NAME:"),
        "tmux-setup.sh no longer targets `$SESSION_NAME:`; a bare session \
         name is captured by any window whose name starts with it"
    );
}
