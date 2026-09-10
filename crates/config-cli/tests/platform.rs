//! The shared platform-detection helper, `.scripts/platform.sh`.
//!
//! Every config file that has a platform variant (`.zshrc`, `tmux.conf`,
//! `alacritty.toml`, `deps.toml`) needs the same answer to "which platform
//! is this". Before this helper existed, each file answered it separately or
//! not at all, which is how the mac and linux branches drifted: a file that
//! never asks the question has to BE two files, and two files drift.
//!
//! The contract:
//!
//! 1. it names exactly one platform, from a closed set
//! 2. the answer is overridable, so a test can drive both platforms from one
//!    machine
//! 3. an unrecognized uname is an explicit "unknown", never a silent guess
//!    at mac
//!
//! Point 3 matters because the variant files are selected by this name. A
//! helper that guessed "mac" on an unrecognized system would source
//! `.zshrc-mac` on a BSD box and fail deep inside a Homebrew path rather
//! than at the point the assumption was made.
//!
//! Converted whole from `tests/platform.test.sh`, which ran **11**
//! assertions from 11 `assert_*` call sites, none in a loop.
//!
//! Driven through `sh -c` rather than by reading the file, because the
//! helper is sourced by `.zshrc` (zsh), `tmux.conf`'s shell-command hooks
//! (sh), and `setup.sh` (sh). POSIX sh is the common denominator it has to
//! work in, so that is what it is tested in.

use dotfiles_test_support::repo::root as repo_root;
use std::path::{Path, PathBuf};
use std::process::Command;

fn platform_sh() -> PathBuf {
    repo_root().join(".scripts/platform.sh")
}

/// Sources the helper under `sh` and prints one expression.
///
/// `PATH` is pinned so a stub `uname` can be prepended without inheriting
/// whatever else the caller's shell carries.
fn sourced(environment: &[(&str, &str)], path: &str, expression: &str) -> String {
    let script = format!(". '{}' && {expression}", platform_sh().display());
    let mut command = Command::new("sh");
    command.arg("-c").arg(script).env("PATH", path);
    for (key, value) in environment {
        command.env(key, value);
    }
    let output = command.output().expect("sh spawns");
    // Trailing newline trimmed the way the shell suite's `$(...)` did:
    // `platform_variant` ends its output with one, and `$DOTFILES_PLATFORM`
    // does not, so the two call shapes would otherwise need different
    // handling for no reason a reader could see.
    String::from_utf8_lossy(&output.stdout)
        .trim_end_matches('\n')
        .to_string()
}

/// The platform the helper reports when `uname -s` says `fake_uname`.
///
/// The stub is a real executable early on `PATH`, so the helper's own call
/// is what reaches it. Nothing about the helper is mocked.
fn detect_with_uname(fake_uname: &str, stub_root: &Path) -> String {
    let stub_dir = stub_root.join(format!("uname-{fake_uname}"));
    std::fs::create_dir_all(&stub_dir).expect("a stub directory");
    let stub = stub_dir.join("uname");
    std::fs::write(&stub, format!("#!/bin/sh\nprintf '%s\\n' \"{fake_uname}\"\n"))
        .expect("the stub writes");
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&stub).expect("the stub exists").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&stub, permissions).expect("the stub is executable");
    }

    let path = format!("{}:/usr/bin:/bin", stub_dir.display());
    sourced(
        &[("DOTFILES_PLATFORM", "")],
        &path,
        r#"printf '%s' "$DOTFILES_PLATFORM""#,
    )
}

/// This machine's real platform name, for the fallback assertion.
fn native_platform() -> String {
    let output = Command::new("uname").arg("-s").output().expect("uname runs");
    let name = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
    if name == "darwin" { "mac".to_string() } else { name }
}

#[test]
fn the_helper_exists() {
    let path = platform_sh();
    assert!(path.is_file(), "no platform helper at {}", path.display());
}

/// It names one platform from a closed set, and an unrecognized system is
/// explicit rather than a guess.
#[test]
fn each_uname_maps_to_exactly_one_platform_name() {
    let stubs = tempfile::tempdir().expect("a stub root");
    for (uname, expected) in [
        ("Darwin", "mac"),
        ("Linux", "linux"),
        ("FreeBSD", "unknown"),
    ] {
        let detected = detect_with_uname(uname, stubs.path());
        assert_eq!(
            detected, expected,
            "uname {uname} reported {detected:?} rather than {expected:?}; an \
             unrecognized system guessing `mac` would source .zshrc-mac on a \
             BSD box and fail deep inside a Homebrew path"
        );
    }
}

/// This is what lets one machine test both platforms' variant files. Without
/// it, the linux variant of every config file would be unreachable from a
/// mac machine's test suite, and nothing would be looking at it.
#[test]
fn a_preset_platform_is_respected() {
    let overridden = sourced(
        &[("DOTFILES_PLATFORM", "linux")],
        "/usr/bin:/bin",
        r#"printf '%s' "$DOTFILES_PLATFORM""#,
    );
    assert_eq!(
        overridden, "linux",
        "a preset DOTFILES_PLATFORM was not respected, so one machine cannot \
         drive both platforms' variant files"
    );
}

/// An override is only honored when it names a platform the helper knows. A
/// typo ("linix") that silently became the platform would select a variant
/// file that does not exist, and every config would fall back to bare shared
/// behavior with no error: the failure mode this helper removes.
#[test]
fn an_unrecognized_override_falls_back_to_detection() {
    let fallback = sourced(
        &[("DOTFILES_PLATFORM", "linix")],
        "/usr/bin:/bin",
        r#"printf '%s' "$DOTFILES_PLATFORM""#,
    );
    assert_eq!(
        fallback,
        native_platform(),
        "the typo `linix` survived as the platform name rather than falling \
         back to detection, so a misspelled override would select a variant \
         file that does not exist"
    );
}

/// `.zshrc` sources it, and so does a script `.zshrc` invokes. Re-sourcing
/// must not append to PATH or otherwise accumulate state, because
/// `source ~/.zshrc` to reload a shell is a normal thing to do here.
#[test]
fn sourcing_it_twice_gives_the_same_answer() {
    let once = sourced(
        &[("DOTFILES_PLATFORM", "")],
        "/usr/bin:/bin",
        r#"printf '%s' "$DOTFILES_PLATFORM""#,
    );
    let twice = sourced(
        &[("DOTFILES_PLATFORM", "")],
        "/usr/bin:/bin",
        &format!(
            ". '{}' && printf '%s' \"$DOTFILES_PLATFORM\"",
            platform_sh().display()
        ),
    );
    assert!(
        !once.is_empty(),
        "positive control: sourcing the helper once produced no platform at \
         all, so comparing two empty answers would prove nothing"
    );
    assert_eq!(
        twice, once,
        "sourcing the helper twice changed the answer, so it accumulates state"
    );
}

/// The naming convention (`base.ext` becomes `base-mac.ext`) is expressed
/// once, in the helper, rather than restated at each of the five call sites.
/// A call site that spelled the suffix itself would be free to spell it
/// differently, which is the drift this replaces.
#[test]
fn the_variant_helper_places_the_suffix_correctly() {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let cases = [
        // A dotted path takes the suffix before the extension.
        ("mac", "/tmp/alacritty.toml", "/tmp/alacritty-mac.toml".to_string()),
        // An extensionless dotfile takes a trailing suffix.
        ("linux", "$HOME/.zshrc", format!("{home}/.zshrc-linux")),
        // A leading-dot filename with no other dot must not be treated as
        // having an extension: ".bashrc" is a name, not "" with extension
        // "bashrc". Getting this wrong produces "-linux.bashrc", which no
        // file is named.
        ("linux", "/etc/.bashrc", "/etc/.bashrc-linux".to_string()),
        // A dot in a parent directory must not be mistaken for the
        // filename's extension. ~/.config/tmux/tmux.conf has dots in both.
        (
            "mac",
            "/home/a/.config/tmux/tmux.conf",
            "/home/a/.config/tmux/tmux-mac.conf".to_string(),
        ),
    ];

    for (platform, input, expected) in cases {
        let produced = sourced(
            &[("DOTFILES_PLATFORM", platform), ("HOME", &home)],
            "/usr/bin:/bin",
            &format!("platform_variant \"{input}\""),
        );
        assert_eq!(
            produced, expected,
            "platform_variant {input} on {platform} produced {produced:?} \
             rather than {expected:?}"
        );
    }
}
