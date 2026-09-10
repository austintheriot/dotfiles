//! The dependency MANIFEST: `deps.toml`, `deps-ci.toml` and the two platform
//! variants. Not the engine that reads them, which is Rust already and has
//! its own tests in `deps-core` and this crate.
//!
//! The conf files are data with no compiler behind them, so a suite that
//! reads them is the only thing that catches a malformed entry before a
//! bootstrap does. Every check is evaluated by asking the engine, not by
//! re-deriving a shell string from the file: TOML stores a TYPED check, so
//! there is no shell text to extract, and asking the engine exercises the
//! same code path `config deps check` takes rather than an approximation of
//! it that could agree with a broken engine.
//!
//! Converted whole from `tests/deps-manifest.test.sh`, which ran **20**
//! assertions on this machine from 22 `assert_*` call sites: two sit in a
//! loop over the two platforms, and two call sites are in branches only one
//! of which runs per machine.
//!
//! A FOURTH GATE THAT COULD NOT FAIL, found by this conversion and reported
//! as a finding rather than as this tranche's justification. The shell
//! suite called `finish` at line 247 with **four assertions still below
//! it**: the whole piped-output block, which pins that no ANSI escape
//! reaches a pipe and that the summary and present rows stay greppable.
//! `finish` is what returns the exit status, so those four ran, printed
//! their result, and could never fail the suite. Measured 2026-09-10:
//! sabotaging `the piped run produced output` printed `FAIL:` and the suite
//! exited **0**. Same shape as `nvim-lua-format.test.sh`. All four are real
//! gates here.
//!
//! The engine is reached through `CARGO_BIN_EXE_config-cli` rather than
//! through a `config-cli` found on `PATH`. The shell suite used the latter
//! and therefore tested whatever binary was last installed, which can lag
//! the tree under test by any amount.

use dotfiles_test_support::repo::root as repo_root;
use dotfiles_test_support::skip;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn deps_dir() -> PathBuf {
    repo_root().join("deps")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()))
}

/// One dependency's real check, evaluated by the engine that owns it.
///
/// `DEPS_LOCAL_CONF` points at a path that does not exist so the platform
/// variant is suppressed: these assertions are about the SHARED manifest
/// accepting either platform's shape, and pulling in `deps-mac.toml` would
/// let a mac-only entry answer for it.
///
/// Exit 0 means present, 1 means missing, per the engine's exit contract.
fn check_via_engine(dependency: &str, home: &Path, path: &str) -> bool {
    Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["deps", "check", "--only", dependency])
        .env("HOME", home)
        .env("PATH", path)
        .env("DOTFILES_ROOT", repo_root())
        .env("DEPS_LOCAL_CONF", "/nonexistent/deps-platform.toml")
        .output()
        .expect("the engine spawns")
        .status
        .success()
}

/// Every table header in every shipped manifest, comments stripped first.
///
/// All three files, not only the one this machine selects: both platform
/// variants ship on the single branch, so reading all of them is what lets a
/// mac machine's suite catch a dependency dropped from the linux variant.
fn all_tracked_dependencies() -> Vec<String> {
    let mut names: Vec<String> = ["deps.toml", "deps-mac.toml", "deps-linux.toml"]
        .iter()
        .map(|name| read(&deps_dir().join(name)))
        .flat_map(|text| table_headers(&text))
        .collect();
    names.sort();
    names.dedup();
    names
}

/// The `[name]` table headers of one manifest, with comments removed.
///
/// Comments are stripped before matching because a `# [example]` in the
/// prose above an entry would otherwise read as a tracked dependency, which
/// is the vacuous-match shape Tranche A found twice.
fn table_headers(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| line.split('#').next().unwrap_or(""))
        .filter_map(|line| {
            let trimmed = line.trim_end();
            let inner = trimmed.strip_prefix('[')?.strip_suffix(']')?;
            let starts_lower = inner.starts_with(|byte: char| byte.is_ascii_lowercase());
            let plain = inner
                .chars()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == '-');
            (starts_lower && plain).then(|| inner.to_string())
        })
        .collect()
}

/// The positive controls. Every assertion below reads one of these files, and
/// an unreadable path makes "no bad lines found" and "nothing was read" look
/// identical, a defect this repo has shipped twice.
#[test]
fn every_manifest_ships_here_with_content_in_it() {
    for name in ["deps.toml", "deps-ci.toml", "deps-mac.toml", "deps-linux.toml"] {
        let path = deps_dir().join(name);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{} must ship here: {error}", path.display()));
        assert!(
            !text.is_empty(),
            "{} is empty, so every assertion that reads it would compare nothing",
            path.display()
        );
    }
}

/// `deps.toml` is byte-identical on both platforms' machines, so each check
/// in it has to pass against the macOS shape and the Linux shape of the same
/// dependency. Read out of the real file through the engine rather than
/// restated here, so the test fails if the shipped file regresses.
#[test]
fn the_shared_check_accepts_either_platforms_shape() {
    let fixtures = tempfile::tempdir().expect("a fixture root");

    // A Linux machine: the plugin is an oh-my-zsh custom clone, no brew.
    let linux_home = fixtures.path().join("linux-home");
    let plugin = linux_home.join(".oh-my-zsh/custom/plugins/zsh-autosuggestions");
    std::fs::create_dir_all(&plugin).expect("the plugin directory");
    std::fs::write(plugin.join("zsh-autosuggestions.zsh"), "").expect("the plugin file");
    assert!(
        check_via_engine("zsh-autosuggestions", &linux_home, "/usr/bin:/bin"),
        "the shared check did not resolve zsh-autosuggestions through the \
         oh-my-zsh plugin path, so a Linux machine's install would report \
         missing after a successful install"
    );

    let empty_home = fixtures.path().join("empty-home");
    std::fs::create_dir_all(&empty_home).expect("an empty home");

    // This machine: the plugin comes from brew, and there is no ~/.oh-my-zsh.
    //
    // Guarded on the plugin FILE, not on brew being installed. `command -v
    // brew` was the wrong question: a macOS CI runner has brew and need not
    // have this package, so the guard passed and the assertion then failed
    // on a machine where the check is behaving correctly.
    match brew_plugin_path() {
        Some(brew_plugin) if brew_plugin.is_file() => {
            let inherited = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_string());
            assert!(
                check_via_engine("zsh-autosuggestions", &empty_home, &inherited),
                "the shared check did not resolve zsh-autosuggestions through \
                 the brew share path, so the two-branch check has lost a branch"
            );
        }
        _ => skip("zsh-autosuggestions is not installed through brew here"),
    }

    // A machine with neither shape must still report it missing, or the
    // widened check has become a tautology. PATH carries no brew, so the brew
    // operand expands to an empty prefix and cannot accidentally match.
    let bare_home = fixtures.path().join("bare-home");
    std::fs::create_dir_all(&bare_home).expect("a bare home");
    assert!(
        !check_via_engine("zsh-autosuggestions", &bare_home, "/usr/bin:/bin"),
        "the zsh-autosuggestions check reported present on a machine carrying \
         neither shape, so it is a tautology that can never say missing"
    );
}

/// Homebrew's share path for the plugin, or `None` where brew is absent.
fn brew_plugin_path() -> Option<PathBuf> {
    let output: Output = Command::new("brew").arg("--prefix").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!prefix.is_empty())
        .then(|| PathBuf::from(prefix).join("share/zsh-autosuggestions/zsh-autosuggestions.zsh"))
}

/// alacritty is a `.app` bundle on macOS and a PATH binary on Linux, and one
/// shared check has to accept both.
#[test]
fn the_shared_alacritty_check_accepts_both_install_shapes() {
    let fixtures = tempfile::tempdir().expect("a fixture root");
    let empty_home = fixtures.path().join("empty-home");
    std::fs::create_dir_all(&empty_home).expect("an empty home");

    let stub_dir = fixtures.path().join("alacritty-bin");
    std::fs::create_dir_all(&stub_dir).expect("a stub directory");
    let stub = stub_dir.join("alacritty");
    std::fs::write(&stub, "#!/bin/sh\nexit 0\n").expect("the stub writes");
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&stub).expect("the stub exists").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&stub, permissions).expect("the stub is executable");
    }

    let path = format!("{}:/usr/bin:/bin", stub_dir.display());
    assert!(
        check_via_engine("alacritty", &empty_home, &path),
        "the shared alacritty check did not resolve a binary on PATH, which \
         is the only shape a Linux machine has"
    );

    if Path::new("/Applications/Alacritty.app").is_dir() {
        assert!(
            check_via_engine("alacritty", &empty_home, "/usr/bin:/bin"),
            "the shared alacritty check did not resolve the macOS app bundle, \
             so this machine's real install would report missing"
        );
    } else {
        skip("no /Applications/Alacritty.app here, so the bundle branch cannot run");
    }
}

/// zsh-autosuggestions is installed two different ways, and the shared check
/// accepts either path, so the check alone cannot say whether this machine
/// needs oh-my-zsh. The branch's own zshrc can.
///
/// Moving oh-my-zsh out of the shared `deps.toml` into `deps-linux.toml` is
/// correct, because the mac machine does not use it. The move is only safe
/// while the variant whose zshrc sources from the oh-my-zsh path still lists
/// it: drop it there and the shell sources a plugin nothing installs,
/// silently losing autosuggestions with every check still reporting success.
#[test]
fn oh_my_zsh_is_tracked_wherever_a_shipped_zshrc_sources_it() {
    let root = repo_root();
    let mut zshrcs = vec![root.join(".zshrc")];
    if let Ok(entries) = std::fs::read_dir(&root) {
        let mut variants: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(".zshrc-"))
            })
            .collect();
        variants.sort();
        zshrcs.extend(variants);
    }

    let sourcing: Vec<&PathBuf> = zshrcs
        .iter()
        .filter(|path| path.is_file())
        .filter(|path| {
            read(path).lines().any(|line| {
                let code = line.split('#').next().unwrap_or("");
                code.contains("source") && code.contains(".oh-my-zsh")
            })
        })
        .collect();

    if sourcing.is_empty() {
        // Asserted rather than skipped: "no zshrc sources it" is a real
        // state of the repo, and the check below would then be vacuous.
        assert!(
            !all_tracked_dependencies().is_empty(),
            "positive control: no manifest entry was read at all, so \
             concluding anything about oh-my-zsh would prove nothing"
        );
        return;
    }

    let tracked = all_tracked_dependencies();
    assert!(
        tracked.iter().any(|name| name == "oh-my-zsh"),
        "{} sources zsh-autosuggestions from the oh-my-zsh plugin path, but \
         no shipped manifest tracks oh-my-zsh, so that shell would source a \
         plugin nothing installs while every check reports success",
        sourcing
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<String>>()
            .join(", ")
    );
}

/// `deps-ci.toml` holds what the test suite needs and the working
/// environment does not. It is never selected by platform detection, so
/// `depcheck` on a developer machine does not ask for them.
///
/// The assertion is that the file the CI workflow names actually loads: a
/// manifest that silently reads as empty is this repo's recurring bug, and
/// it is what `DEPS_CONF` pointing at the wrong name produced before.
#[test]
fn the_ci_manifest_loads_and_carries_its_explicit_only_entry() {
    let output = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["deps", "check"])
        .env("DOTFILES_ROOT", repo_root())
        .env("DEPS_CONF", "deps-ci.toml")
        .env("DEPS_LOCAL_CONF", "/nonexistent/deps-platform.toml")
        .output()
        .expect("the engine spawns");
    let listed = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.starts_with("  present ") || line.starts_with("  missing "))
        .count();
    assert!(
        listed > 0,
        "the CI manifest parsed to zero entries, so a `config deps check` \
         against it would report success having checked nothing"
    );

    // pyyaml is the entry that proves the ExplicitOnly kind is in force: it
    // is a `python_import`, which the parser refuses in any platform-selected
    // file. If the workflow ever selected this manifest by platform instead,
    // the load would fail rather than quietly dropping the check.
    let text = read(&deps_dir().join("deps-ci.toml"));
    assert!(
        table_headers(&text).iter().any(|name| name == "pyyaml"),
        "deps-ci.toml no longer carries the pyyaml entry, which is the only \
         `python_import` proving the ExplicitOnly kind is still in force"
    );
}

/// The manifests name what the rest of the repo assumes: a C toolchain the
/// Rust crate needs to link, and the rustup `config build` cannot run
/// without. Nothing installs either on a fresh machine unless the manifest
/// carries it.
///
/// The rustup entry is also why neither deps Docker image may carry one:
/// installing rustup into the image would pre-satisfy the very dependency
/// the bootstrap exists to exercise.
#[test]
fn the_shared_manifest_names_the_toolchain_the_repo_assumes() {
    let headers = table_headers(&read(&deps_dir().join("deps.toml")));
    assert!(
        !headers.is_empty(),
        "positive control: deps.toml yielded no table headers at all, so the \
         two checks below would compare against an empty list"
    );
    for required in ["cc", "rustup"] {
        assert!(
            headers.iter().any(|name| name == required),
            "deps.toml no longer tracks `{required}`, so a fresh machine \
             would reach `config build` without it"
        );
    }
}

/// THE BREAKAGE CLASS THIS GUARDS, stated because it is the one this repo
/// keeps hitting: the gates grep the engine's output. The deps-check
/// workflow greps for `present   neovim` and the container suite matches
/// assertion text. An escape sequence in a piped stream breaks all of them,
/// and it breaks them ONLY in CI, where nobody is watching a terminal.
///
/// So this asserts the bytes, through a pipe, the same shape every gate
/// uses. The `deps-core` tests assert that `Style::Plain` emits no escapes;
/// this asserts that a piped process actually chooses it.
///
/// **This whole block sat below `finish` in the shell suite and could not
/// fail it.** See the module doc.
#[test]
fn a_piped_run_stays_plain_and_greppable() {
    let output = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["deps", "check"])
        .env("DOTFILES_ROOT", repo_root())
        .output()
        .expect("the engine spawns");
    let mut piped = String::from_utf8_lossy(&output.stdout).to_string();
    piped.push_str(&String::from_utf8_lossy(&output.stderr));

    assert!(
        !piped.is_empty(),
        "positive control: the piped run produced no output at all, so the \
         three assertions below would pass against an empty string"
    );
    assert!(
        !piped.contains('\u{1b}'),
        "an ANSI escape reached a pipe, which breaks every gate that greps \
         this output and breaks it only in CI"
    );
    assert!(
        piped.lines().any(|line| {
            line.strip_prefix("deps checked dependencies: ")
                .and_then(|rest| rest.split_whitespace().next())
                .is_some_and(|count| count.parse::<u32>().is_ok())
        }),
        "the summary line is no longer greppable as `deps checked \
         dependencies: <n> entries`, which run-all.sh's parser and the \
         workflow both match"
    );
    assert!(
        piped.lines().any(|line| {
            line.strip_prefix("  present   ")
                .is_some_and(|rest| rest.starts_with(|byte: char| byte.is_ascii_lowercase()))
        }),
        "a present row is no longer greppable as `  present   <name>`, which \
         the deps-check workflow matches"
    );
}
