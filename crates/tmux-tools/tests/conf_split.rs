//! Confirms the `tmux.conf` / `tmux-<platform>.conf` split preserves the
//! config's actual behaviour. Converted from
//! `tests/tmux-conf-split.test.sh`.
//!
//! `tmux.conf` is the shared, platform-neutral file. `tmux-mac.conf` and
//! `tmux-linux.conf` hold the clipboard commands and both ship on the
//! single branch, sourced by platform at runtime.
//!
//! Runs against the real installed config on a throwaway tmux server, never
//! the developer's live one. `-f` only takes effect when tmux starts a
//! fresh server: passing it to a client of an already-running server is
//! silently ignored, so a shared socket would assert on stale global state
//! instead of on the file under test.

mod support;

use std::path::{Path, PathBuf};
use std::process::Command;

use support::{Server, repo_root};

/// The tracked `tmux.conf`.
fn config_path() -> PathBuf {
    repo_root().join(".config/tmux/tmux.conf")
}

/// Starts a server against the real config.
fn server_from_config(label: &str, session: &str) -> Server {
    let server = Server::new(label);
    let fixtures = tempfile::Builder::new()
        .prefix("tt-fixtures-")
        .tempdir_in("/tmp")
        .expect("a fixture directory");
    let config = config_path();
    let started = server.tmux(&[
        "-f",
        config.to_str().expect("a utf-8 config path"),
        "new-session",
        "-d",
        "-s",
        session,
        "-c",
        fixtures.path().to_str().expect("a utf-8 directory"),
    ]);
    assert!(
        started.status.success(),
        "the server must start, stderr: {}",
        String::from_utf8_lossy(&started.stderr)
    );
    // The fixture directory only has to outlive the new-session call: tmux
    // resolves the pane's cwd at creation and never reads the path again.
    server
}

/// Whether the config parses, and whatever tmux said about it.
///
/// `source-file` into a server that was started with NO config, rather than
/// the `-f` that started the server under test. Two tmux 3.4 behaviours
/// force that shape, and the shell suite's own
/// `the split config parses with no errors` assertion was vacuous because
/// of the first:
///
/// - Starting a detached server with `-f` against a config holding
///   `not-a-tmux-command` writes nothing to stderr and exits 0. The shell
///   suite read exactly that stderr, so no config could ever fail it.
/// - `source-file` of the file a server was already started from is also
///   silent. Only a server that has not read the file reports
///   `<file>:<line>: unknown command: ...`.
///
/// - `source-file`'s own diagnostics go to the attached client's terminal,
///   not to the calling process. With stdout a pipe, as it is under
///   `cargo test`, stderr comes back EMPTY even for a config that fails.
///   The EXIT STATUS is the signal: 0 for a clean parse, 1 otherwise.
///
/// So the parse check gets a server of its own, started with
/// `-f /dev/null`. Without that flag tmux loads the user's own
/// `~/.config/tmux/tmux.conf` at startup, which on this machine is the file
/// under test, and the second behaviour above silences the check again.
fn parses_cleanly(config: &Path) -> (bool, String) {
    let checker = Server::new("conf-parse");
    checker.tmux(&["-f", "/dev/null", "new-session", "-d", "-s", "parse-check"]);
    let sourced = checker.tmux(&[
        "source-file",
        config.to_str().expect("a utf-8 config path"),
    ]);
    let clean = sourced.status.success();
    let said = String::from_utf8_lossy(&sourced.stderr).trim().to_string();
    checker.shutdown();
    (clean, said)
}

/// This machine's platform name, from `.scripts/platform.sh` rather than
/// from a `cfg!` here.
///
/// The variant files are selected by that script's answer, so asking it is
/// what makes a disagreement between the script and this test visible.
fn platform() -> String {
    let script = repo_root().join(".scripts/platform.sh");
    let run = Command::new("sh")
        .arg("-c")
        .arg(format!(
            ". '{}' && printf '%s' \"$DOTFILES_PLATFORM\"",
            script.display()
        ))
        .env_remove("DOTFILES_PLATFORM")
        .output()
        .expect("platform.sh runs");
    String::from_utf8_lossy(&run.stdout).trim().to_string()
}

/// The variant file `.scripts/platform.sh` would select for `path`.
fn platform_variant(path: &Path) -> PathBuf {
    let script = repo_root().join(".scripts/platform.sh");
    let run = Command::new("sh")
        .arg("-c")
        .arg(format!(
            ". '{}' && platform_variant '{}'",
            script.display(),
            path.display()
        ))
        .env_remove("DOTFILES_PLATFORM")
        .output()
        .expect("platform.sh runs");
    PathBuf::from(String::from_utf8_lossy(&run.stdout).trim())
}

/// The `copy-pipe` command a variant file binds to `y` in copy mode.
fn yank_command(variant: &Path) -> String {
    let contents = std::fs::read_to_string(variant).unwrap_or_default();
    contents
        .lines()
        .find(|line| line.contains("copy-mode-vi 'y'") && line.contains("copy-pipe"))
        .and_then(|line| line.split_once("copy-pipe")?.1.trim().split('"').nth(1))
        .map(str::to_string)
        .unwrap_or_default()
}

/// The split config parses with no errors, and the shared settings survive
/// it.
#[test]
fn the_shared_half_of_the_split_survives() {
    let (parses, said) = parses_cleanly(&config_path());
    assert!(
        parses,
        "the split config must parse with no errors, tmux said {said:?}"
    );

    let server = server_from_config("conf-split", "conf-split");

    // S-Up, not PageUp. tmux.conf states the reason: a page key moves a
    // half screen per notch, so the wheel is bound to Shift+Up and
    // Shift+Down as scroll-by-one-line. This assertion named PageUp once
    // and went stale when that changed, failing against a config that was
    // behaving as designed.
    let root_keys = server.stdout(&["list-keys", "-T", "root"]);
    // Positive control: an empty listing would satisfy nothing below but
    // would satisfy a `contains` on an empty needle elsewhere, and it is
    // the cheapest way to prove the server took the config at all.
    assert!(!root_keys.trim().is_empty(), "the control must list root keys");
    assert!(
        root_keys.contains("S-Up"),
        "wheel-scroll forwarding must survive the split"
    );

    let hooks = server.stdout(&["show-hooks", "-g"]);
    assert!(
        hooks.contains("tmux-update-window-names.sh"),
        "the window-naming after-new-window hook must survive the split"
    );

    assert_eq!(
        server.stdout(&["show-options", "-g", "-v", "mouse"]),
        "on",
        "mouse mode must be on"
    );

    server.shutdown();
}

/// Both variant files ship, and the one for this platform is the one in
/// effect.
///
/// Read from the variant file for THIS platform rather than hardcoding
/// xclip or pbcopy: this file ships on the single branch for both
/// platforms, so naming either platform's command here would recreate the
/// exact drift the whole mechanism exists to catch.
#[test]
fn the_platform_variant_is_the_one_in_effect() {
    let config = config_path();
    let variant = platform_variant(&config);
    assert!(
        variant.is_file(),
        "this platform must have a tmux variant file at {}",
        variant.display()
    );

    // Both variants ship on the single branch, so the one for the other
    // platform must be here too. This is what catches one going missing.
    let this_platform = platform();
    let other_platform = if this_platform == "mac" { "linux" } else { "mac" };
    let other_variant = repo_root().join(format!(".config/tmux/tmux-{other_platform}.conf"));
    assert!(
        other_variant.is_file(),
        "the {other_platform} variant must also ship here, at {}",
        other_variant.display()
    );

    // Guards against the assertion below passing vacuously: a `contains`
    // with an empty needle matches anything, which is exactly what happened
    // when the clipboard bindings moved out of tmux.conf and the test still
    // read from it.
    let expected_yank = yank_command(&variant);
    assert!(
        !expected_yank.is_empty(),
        "the variant must name a yank command, in {}",
        variant.display()
    );

    let server = server_from_config("conf-variant", "conf-variant");
    let copy_keys = server.stdout(&["list-keys", "-T", "copy-mode-vi"]);
    let yank_binding: String = copy_keys
        .lines()
        .filter(|line| line.contains("copy-pipe"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !yank_binding.is_empty(),
        "the control must find a copy-pipe binding"
    );
    assert!(
        yank_binding.contains(&expected_yank),
        "the {this_platform} yank command must be in effect, got {yank_binding:?}"
    );

    // The other platform's command must NOT be bound. Sourcing both
    // variants would leave whichever loaded last in charge, silently, on
    // both machines.
    let other_yank = yank_command(&other_variant);
    assert!(
        !other_yank.is_empty(),
        "the {other_platform} variant must name a yank command too"
    );
    assert!(
        !yank_binding.contains(&other_yank),
        "the {other_platform} yank command must not be bound, got {yank_binding:?}"
    );

    server.shutdown();
}
