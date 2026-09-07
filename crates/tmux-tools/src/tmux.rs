//! Every `Command::new("tmux")` call the `name-windows` subcommand makes.
//!
//! `list_windows` batches every window's state into one `list-windows -F`
//! call per session rather than one `display-message` per window, which is
//! where the shell script's 442ms-for-21-windows cost came from: see
//! `.scripts/tmux-update-window-names.sh`'s own `update_each_window`
//! comment for the measurement that motivated the same batching there.

use std::process::Command;

/// The per-window option this binary sets after every rename, so a later
/// run can tell "tmux still owns this name" apart from "the user renamed
/// it by hand." Matches the shell script's `OWNERSHIP_OPTION`.
pub const OWNERSHIP_OPTION: &str = "@wname_auto";

/// One window's tmux-reported state, before any git inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowTarget {
    /// The window's `#{window_id}`, stable across renames.
    pub window_id: String,
    /// The active pane's current working directory.
    pub pane_current_path: String,
    /// The window's current name, before any rename this run might make.
    pub current_name: String,
    /// The value this binary last set `@wname_auto` to, empty when never set.
    pub owned_name: String,
    /// The window's own `@wname_label` value, empty when unset.
    pub label: String,
    /// The window's own `@wname_bare_repos` value, empty when unset.
    ///
    /// Read per window, matching the shell script's own field list: the
    /// suite sets this option with `tmux set -w`, not `-g`, so a read
    /// scoped to the server misses every override the tests (and a real
    /// user) make.
    pub bare_repos: String,
    /// Whether tmux's own `automatic-rename` option is still on for this
    /// window. A window the user renamed by hand normally has this off,
    /// which is part of how the caller decides whether it still owns the
    /// name.
    pub automatic_rename: bool,
}

/// The format string shared by every read call, so a field added to one
/// cannot silently go missing from the other.
const STATE_FORMAT: &str = "#{window_id}\t#{pane_current_path}\t#{window_name}\t#{@wname_auto}\t#{@wname_label}\t#{@wname_bare_repos}\t#{automatic-rename}";

/// A running tmux server to address, honouring the test seam.
///
/// A test cannot spawn sessions on the developer's real tmux server without
/// reintroducing the flake the shell test suite already hit
/// (`.agents/PAPERCUTS.md`), so `TMUX_TOOLS_SOCKET` lets a test point every
/// call in this module at a throwaway server instead.
pub struct Server {
    socket: Option<String>,
}

impl Server {
    /// Reads `TMUX_TOOLS_SOCKET` and returns a `Server` bound to it, or to
    /// the default server when the variable is unset.
    #[must_use]
    pub fn from_env() -> Self {
        Server {
            socket: std::env::var("TMUX_TOOLS_SOCKET").ok(),
        }
    }

    /// Builds a `tmux` command, prefixed with `-L <socket>` when the test
    /// seam is set.
    fn command(&self) -> Command {
        let mut command = Command::new("tmux");
        if let Some(socket) = &self.socket {
            command.args(["-L", socket]);
        }
        command
    }

    /// Lists windows for `-a` (every session) or `-s <session>` (one
    /// session) in a single `list-windows` call, batching every window's
    /// state into one round trip rather than one per window.
    #[must_use]
    pub fn list_windows(&self, target: &[&str]) -> Vec<WindowTarget> {
        let mut arguments = vec!["list-windows"];
        arguments.extend_from_slice(target);
        let output = self
            .command()
            .args(&arguments)
            .arg("-F")
            .arg(STATE_FORMAT)
            .output();

        let Ok(output) = output else {
            return Vec::new();
        };
        if !output.status.success() {
            return Vec::new();
        }

        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(parse_window_line)
            .collect()
    }

    /// Reads one window's state via `display-message`, for the single-window
    /// and default-target paths. Still one call: a `list-windows` call here
    /// would filter a list of every window down to the one already known.
    #[must_use]
    pub fn display_message(&self, target: &str) -> Option<WindowTarget> {
        let output = self
            .command()
            .args(["display-message", "-p", "-t", target, "-F", STATE_FORMAT])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .and_then(parse_window_line)
    }

    /// Renames a window. Fires tmux's `after-*` hooks
    /// (`.config/tmux/tmux.conf`), so the caller renames only when the
    /// computed name actually differs from the current one, never
    /// unconditionally.
    pub fn rename(&self, window_id: &str, name: &str) {
        let _ = self
            .command()
            .args(["rename-window", "-t", window_id, name])
            .output();
    }

    /// Records the name this binary computed, so a later run can recognize
    /// its own work and tell it apart from a name the user set by hand.
    pub fn set_owned_name(&self, window_id: &str, name: &str) {
        let _ = self
            .command()
            .args(["set", "-w", "-t", window_id, OWNERSHIP_OPTION, name])
            .output();
    }
}

/// Parses one `list-windows`/`display-message` output line into a
/// `WindowTarget`. Returns `None` for a line with fewer fields than
/// `STATE_FORMAT` produces, which tmux does not emit for a well-formed
/// format string but which an empty or truncated line would.
fn parse_window_line(line: &str) -> Option<WindowTarget> {
    let mut fields = line.splitn(7, '\t');
    let window_id = fields.next()?.to_string();
    let pane_current_path = fields.next()?.to_string();
    let current_name = fields.next()?.to_string();
    let owned_name = fields.next()?.to_string();
    let label = fields.next()?.to_string();
    let bare_repos = fields.next()?.to_string();
    let automatic_rename = fields.next()? == "1";
    Some(WindowTarget {
        window_id,
        pane_current_path,
        current_name,
        owned_name,
        label,
        bare_repos,
        automatic_rename,
    })
}
