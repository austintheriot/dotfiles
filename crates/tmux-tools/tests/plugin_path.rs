//! Confirms tmux and tpm agree on ONE plugin directory, and that the themes
//! `tmux.conf` declares are actually fetched. Converted from
//! `tests/tmux-plugin-path.test.sh`.
//!
//! THE BUG THIS CLOSES. On a fresh container tmux rendered its plain
//! built-in theme even though `tmux.conf` declares
//! `set -g @plugin "nordtheme/tmux"`. Three things were true at once:
//!
//! 1. The deps engine clones tpm to `$HOME/.tmux/plugins/tpm`, and
//!    `tmux.conf`'s last line runs that copy.
//! 2. tmux derives `TMUX_PLUGIN_MANAGER_PATH` from the directory holding
//!    the config file, so with the config at `~/.config/tmux/tmux.conf` it
//!    points at `~/.config/tmux/plugins/`, a DIFFERENT directory from the
//!    one tpm was cloned into.
//! 3. Nothing ever ran tpm's installer, so no declared plugin was fetched
//!    on any machine, by any bootstrap step.
//!
//! The developer machine hid all three: both directories existed, populated
//! by hand over time. Only a fresh container showed the plain theme.
//!
//! The worst part, and the reason the installed-plugin test asserts the
//! DIRECTORY CONTENTS and not the installer's exit code: running tpm's
//! installer with `TMUX_PLUGIN_MANAGER_PATH` unset prints `download
//! success` and exits 0 having cloned nothing, because its `cd "$path"` ran
//! with an empty argument and landed in `$HOME`. A test that trusted the
//! exit code or the word "success" would pass against a machine with no
//! themes at all, which is the exact fail-open shape this repo keeps
//! hitting.
//!
//! Runs against the real installed config on a throwaway tmux server, never
//! the developer's live one: `-f` is silently ignored for a client of an
//! already-running server, so a shared socket would assert on stale global
//! state instead of the file under test.

mod support;

use std::path::{Path, PathBuf};

use support::{Server, repo_root};

/// The tracked `tmux.conf`.
fn config_path() -> PathBuf {
    repo_root().join(".config/tmux/tmux.conf")
}

/// The plugin repositories `tmux.conf` declares, as `owner/name` strings.
///
/// Both quoting styles appear in the file, so both are read.
///
/// Comments are stripped before matching. A first version of this split on
/// `@plugin` anywhere in the line, so commenting every declaration out left
/// the list unchanged and the "declares at least one plugin" control passed
/// against a config that declared none. That is the same vacuous-grep shape
/// Tranche A found twice.
fn declared_plugins(config: &str) -> Vec<String> {
    config
        .lines()
        .map(|line| line.split('#').next().unwrap_or(""))
        .filter_map(|line| {
            let after = line.split_once("@plugin")?.1.trim();
            let quote = after.chars().next()?;
            if quote != '\'' && quote != '"' {
                return None;
            }
            after[1..].split(quote).next().map(str::to_string)
        })
        .collect()
}

/// Reads a value out of `show-environment -g NAME=value`.
fn environment_value(server: &Server, name: &str) -> String {
    server
        .stdout(&["show-environment", "-g", name])
        .split_once('=')
        .map_or(String::new(), |(_, value)| value.to_string())
}

/// tmux and tpm name the same plugin directory.
///
/// Read tmux's own answer rather than restating a path here: the whole bug
/// was two places disagreeing, so the test asks each side what it believes.
#[test]
fn tpm_runs_from_the_directory_tmux_installs_into() {
    let config = config_path();
    let contents = std::fs::read_to_string(&config).expect("tmux.conf is readable");

    let server = Server::new("plugin-path");
    let fixtures = tempfile::Builder::new()
        .prefix("tt-fixtures-")
        .tempdir_in("/tmp")
        .expect("a fixture directory");
    server.tmux(&[
        "-f",
        config.to_str().expect("a utf-8 config path"),
        "new-session",
        "-d",
        "-s",
        "plugin-path",
        "-c",
        fixtures.path().to_str().expect("a utf-8 directory"),
    ]);

    let manager_path = environment_value(&server, "TMUX_PLUGIN_MANAGER_PATH");
    assert!(
        !manager_path.is_empty(),
        "tmux must report a plugin manager path"
    );

    // The `run` line names the tpm that will do the installing. Whatever
    // directory that copy lives in must be the directory tmux hands it.
    let run_line = contents
        .lines()
        .rfind(|line| line.starts_with("run "))
        .expect("tmux.conf carries a tpm run line");

    let tpm_directory = expand_home(run_line[4..].trim().trim_matches('\''));
    let tpm_parent = tpm_directory
        .parent()
        .and_then(Path::parent)
        .expect("the tpm path has a grandparent");

    assert_eq!(
        Path::new(manager_path.trim_end_matches('/')),
        tpm_parent,
        "tpm must run from the directory tmux installs plugins into"
    );

    server.shutdown();
}

/// Every declared plugin is actually present on disk.
///
/// The contents, not the installer's verdict. See the module doc: the
/// installer reports success for plugins it never cloned.
///
/// Skipped where tpm itself is absent, and that is a real distinction
/// rather than a convenience. This asks about INSTALLED STATE, which the
/// deps engine creates by cloning tpm and which the pre-push container
/// therefore does not have: its tree comes from `git archive`, so no plugin
/// was ever cloned into it. A bare existence check there fails for a
/// correct machine.
#[test]
fn every_declared_plugin_is_installed() {
    let config = config_path();
    let contents = std::fs::read_to_string(&config).expect("tmux.conf is readable");

    let declared = declared_plugins(&contents);
    // Positive control: the loop below asserts nothing at all if the config
    // declares nothing, which is how a parser that stopped matching would
    // pass silently.
    assert!(
        !declared.is_empty(),
        "the config must declare at least one plugin"
    );

    let server = Server::new("plugin-installed");
    let fixtures = tempfile::Builder::new()
        .prefix("tt-fixtures-")
        .tempdir_in("/tmp")
        .expect("a fixture directory");
    server.tmux(&[
        "-f",
        config.to_str().expect("a utf-8 config path"),
        "new-session",
        "-d",
        "-s",
        "plugin-installed",
        "-c",
        fixtures.path().to_str().expect("a utf-8 directory"),
    ]);
    let manager_path = environment_value(&server, "TMUX_PLUGIN_MANAGER_PATH");
    let manager_directory = PathBuf::from(manager_path.trim_end_matches('/'));
    server.shutdown();

    if !manager_directory.join("tpm").is_dir() {
        for repository in &declared {
            support::skip(&format!(
                "no tpm at {}: the declared plugin {repository} is installed",
                manager_directory.display()
            ));
        }
        return;
    }

    for repository in &declared {
        let name = repository.rsplit('/').next().expect("a plugin name");
        let installed = manager_directory.join(name);
        assert!(
            installed.is_dir(),
            "the declared plugin {repository} must be installed at {}",
            installed.display()
        );
    }
}

/// The plugin path follows `$HOME`, not the config file's own location.
///
/// The test that actually catches the bug, and it needs its own tmux server
/// with its own `$HOME`.
///
/// An earlier draft asserted nord's colours against the live plugin tree
/// and PASSED with the theme loading disabled entirely, because the plugins
/// were already installed on the developer machine. That is the fail-open
/// this repo keeps hitting: a gate that passes while testing a pre-satisfied
/// path. So `$HOME` points at an empty directory here, and the assertion
/// reads what a FRESH machine would produce rather than what this one has
/// lying around.
#[test]
fn the_plugin_path_follows_home_rather_than_the_config_location() {
    let config = config_path();
    let fresh_home = tempfile::Builder::new()
        .prefix("tt-home-")
        .tempdir_in("/tmp")
        .expect("a fresh home");
    let config_directory = fresh_home.path().join(".config/tmux");
    std::fs::create_dir_all(&config_directory).expect("the fresh config directory");
    let fresh_config = config_directory.join("tmux.conf");
    std::fs::copy(&config, &fresh_config).expect("tmux.conf is copied");

    let server = Server::new("plugin-fresh");
    server.tmux_with_home(
        fresh_home.path(),
        &[
            "-f",
            fresh_config.to_str().expect("a utf-8 config path"),
            "new-session",
            "-d",
            "-s",
            "fresh",
            "-c",
            fresh_home.path().to_str().expect("a utf-8 directory"),
        ],
    );

    let reported = String::from_utf8_lossy(
        &server
            .tmux_with_home(
                fresh_home.path(),
                &["show-environment", "-g", "TMUX_PLUGIN_MANAGER_PATH"],
            )
            .stdout,
    )
    .trim()
    .to_string();
    let reported = reported
        .split_once('=')
        .map_or(String::new(), |(_, value)| value.to_string());

    // Positive control: the fresh server must have answered at all, or the
    // comparison below would be between two empty strings.
    assert!(
        !reported.is_empty(),
        "the fresh server must report a plugin manager path"
    );

    let expected = format!("{}/.tmux/plugins/", fresh_home.path().display());
    assert_eq!(
        reported, expected,
        "the plugin path must follow HOME, not the config file location"
    );

    server.shutdown();
}

/// Expands a leading `~` against `$HOME`.
///
/// One side of the directory comparison is written with a tilde and the
/// other is absolute, and a string compare would call two names for one
/// directory a mismatch.
fn expand_home(raw: &str) -> PathBuf {
    match raw.strip_prefix('~') {
        Some(rest) => {
            let home = std::env::var("HOME").unwrap_or_default();
            PathBuf::from(format!("{home}{rest}"))
        }
        None => PathBuf::from(raw),
    }
}
