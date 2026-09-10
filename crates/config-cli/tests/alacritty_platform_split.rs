//! The `alacritty.toml` / `alacritty-<platform>.toml` split.
//!
//! `alacritty.toml` used to be per-branch and it drifted one way in each
//! direction: the mac branch never received the Nord colour palette, and the
//! linux branch never received `option_as_alt` or the Cmd+N binding. Neither
//! loss was deliberate.
//!
//! The awkward part, and the reason this suite exists: **Alacritty has no
//! conditional import.** A shared config that imported both variants would
//! apply both on every machine. So the shared file imports ONE stable path,
//! `alacritty-platform.toml`, which is a generated pointer to this machine's
//! real variant. The pointer is untracked, the only per-machine artifact
//! here, and `.scripts/alacritty-platform.sh` regenerates it.
//!
//! The contract:
//!
//! 1. the shared config imports the stable pointer, never a named platform
//! 2. both real variants ship here
//! 3. generating the pointer selects THIS platform's variant
//! 4. generating is idempotent and self-healing, so a stale or absent
//!    pointer is corrected rather than appended to
//! 5. the pointer is replaced atomically, never truncated in place
//!
//! Converted whole from `tests/alacritty-platform-split.test.sh`, which ran
//! **28** assertions on this machine from 21 `assert_*` call sites: three
//! sit in loops, two over the shared config's two forbidden imports and four
//! over the two platforms' generated pointers.

use dotfiles_test_support::repo::root as repo_root;
use dotfiles_test_support::skip;
use std::path::{Path, PathBuf};
use std::process::Command;

fn alacritty_dir() -> PathBuf {
    repo_root().join(".config/alacritty")
}

fn generator() -> PathBuf {
    repo_root().join(".scripts/alacritty-platform.sh")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()))
}

/// A file's lines with comment lines removed.
///
/// Every content assertion below goes through this. The comment block above
/// the shared config's import names both variants to explain the design, and
/// matching the raw file would read that prose as configuration, which is
/// the vacuous-match shape Tranche A found twice.
fn uncommented(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect()
}

/// Runs the real generator against a fixture directory on a stated platform.
fn run_generator(platform: &str, directory: &Path) {
    let status = Command::new("sh")
        .arg(generator())
        .env("DOTFILES_PLATFORM", platform)
        .env("ALACRITTY_DIR", directory)
        .status()
        .expect("the generator spawns");
    assert!(
        status.success(),
        "the generator exited {:?} for {platform} in {}",
        status.code(),
        directory.display()
    );
}

/// A fixture directory holding both variants, each with distinguishable
/// content, so a pointer can be told apart from the wrong variant's copy.
fn fixture_with_both_variants(root: &Path, name: &str) -> PathBuf {
    let directory = root.join(name);
    std::fs::create_dir_all(&directory).expect("a fixture directory");
    std::fs::write(directory.join("alacritty-mac.toml"), "mac-variant\n").expect("the mac variant");
    std::fs::write(directory.join("alacritty-linux.toml"), "linux-variant\n")
        .expect("the linux variant");
    directory
}

/// The positive controls, plus point 2 of the contract: both real variants
/// ship on the single branch, which is what lets a mac machine's suite catch
/// a linux variant going missing.
#[test]
fn the_shared_config_the_generator_and_both_variants_all_ship_here() {
    for path in [
        alacritty_dir().join("alacritty.toml"),
        generator(),
        alacritty_dir().join("alacritty-mac.toml"),
        alacritty_dir().join("alacritty-linux.toml"),
    ] {
        assert!(
            path.is_file(),
            "{} does not ship here, so every assertion that reads it would \
             compare nothing",
            path.display()
        );
    }
}

/// Point 1 of the contract. Importing a named variant is the bug this design
/// exists to prevent: both would load on both machines, because Alacritty has
/// no conditional import.
///
/// `general.import` is Alacritty 0.14+. On 0.13 it parses as an unknown key
/// and every import is silently dropped: the config loads, nothing errors,
/// and no platform binding is ever applied. The spelling that works on both
/// is pinned.
#[test]
fn the_shared_config_imports_the_pointer_and_never_a_named_platform() {
    let text = read(&alacritty_dir().join("alacritty.toml"));
    let lines = uncommented(&text);

    assert!(
        lines.iter().any(|line| line.contains("alacritty-platform.toml")),
        "the shared config does not import the platform pointer, so no \
         variant is applied on any machine"
    );

    for platform in ["mac", "linux"] {
        let named = format!("alacritty-{platform}.toml");
        assert!(
            !lines.iter().any(|line| line.contains(&named)),
            "the shared config imports {named} by name; Alacritty has no \
             conditional import, so both variants would apply on every machine"
        );
    }

    assert!(
        lines.iter().any(|line| line.starts_with("import = [")),
        "the import is not spelled at top level, so Alacritty 0.13 drops it \
         silently and no platform binding is ever applied"
    );
    assert!(
        !lines.iter().any(|line| line.starts_with("general.import")),
        "the 0.14-only `general.import` spelling is in use, which 0.13 parses \
         as an unknown key and silently ignores"
    );
}

/// A fresh Pop!_OS machine has bash as the login shell, so Alacritty opened
/// there lands in bash: nothing in `.zshrc` is loaded, and the `s` alias does
/// not exist until zsh is started by hand. macOS needs no equivalent, its
/// login shell has been zsh since Catalina.
///
/// Declared in the config rather than by `chsh` deliberately. `chsh` writes
/// `/etc/passwd`, which is system state outside `$HOME` that nothing else in
/// this repo touches, needs a password or root, has no idempotent
/// declaration, and cannot be exercised by a suite.
///
/// `terminal.shell` is the 0.14+ spelling, and on 0.13 it parses as an
/// unknown key and the setting is silently dropped: Alacritty starts, nothing
/// errors, and the login shell runs anyway. Same silent-ignore trap the
/// `general.import` assertion above pins.
#[test]
fn only_the_linux_variant_declares_a_shell_and_it_uses_the_portable_spelling() {
    let linux_text = read(&alacritty_dir().join("alacritty-linux.toml"));
    let linux_lines = uncommented(&linux_text);

    assert!(
        linux_lines.iter().any(|line| line.starts_with(r#"program = "/bin/zsh""#)),
        "the linux variant no longer launches zsh, so an Alacritty opened on \
         a machine whose login shell is bash loads none of .zshrc"
    );
    assert!(
        linux_lines.iter().any(|line| line.starts_with("[shell]")),
        "the shell program is not declared under the 0.13 [shell] table, so \
         0.13 ignores it"
    );
    assert!(
        !linux_lines
            .iter()
            .any(|line| line.contains("terminal.shell") || line.starts_with("[terminal")),
        "the 0.14-only terminal.shell spelling is in use, which 0.13 parses \
         as an unknown key and silently drops"
    );

    // The mac variant must NOT carry it. Hardcoding /bin/zsh on a mac would
    // override a Homebrew zsh the user chose, and the key is unnecessary
    // there.
    //
    // Matched on the [shell] TABLE HEADER, not on a bare `program =` line.
    // The mac variant already has `program = "open"` under
    // [keyboard.bindings.command], and the looser pattern flagged that
    // unrelated key: the assertion failed for the wrong reason before this
    // was narrowed.
    let mac_text = read(&alacritty_dir().join("alacritty-mac.toml"));
    assert!(
        !uncommented(&mac_text)
            .iter()
            .any(|line| line.starts_with("[shell]") || line.starts_with("shell.")),
        "the mac variant declares a shell, which would override a Homebrew \
         zsh the user chose"
    );

    // The shared file must not carry it either: values in the shared config
    // WIN over an imported variant, so a shell key there would apply on
    // macOS too and defeat the split.
    let shared_text = read(&alacritty_dir().join("alacritty.toml"));
    assert!(
        !uncommented(&shared_text)
            .iter()
            .any(|line| line.starts_with("[shell]") || line.starts_with("shell.")),
        "the shared config declares a shell, and shared values win over an \
         imported variant, so it would apply on macOS too"
    );
}

/// Point 3 of the contract, on both platforms, from one machine.
#[test]
fn generating_the_pointer_selects_this_platforms_variant_and_only_that_one() {
    let fixtures = tempfile::tempdir().expect("a fixture root");

    for (platform, other) in [("mac", "linux"), ("linux", "mac")] {
        let directory = fixture_with_both_variants(fixtures.path(), &format!("alac-{platform}"));
        run_generator(platform, &directory);

        let pointer = directory.join("alacritty-platform.toml");
        assert!(
            pointer.is_file(),
            "{platform} got no pointer, so the shared config's one import \
             resolves to nothing"
        );

        let contents = read(&pointer);
        assert!(
            contents.contains(&format!("{platform}-variant")),
            "the {platform} pointer does not resolve to the {platform} \
             variant; it holds {contents:?}"
        );
        assert!(
            !contents.contains(&format!("{other}-variant")),
            "the {platform} pointer carries {other} content, so the wrong \
             platform's config is reachable through the stable import"
        );
    }
}

/// Point 4 of the contract. `config reload` and shell startup both run the
/// generator, so it runs constantly: a generator that appended rather than
/// replaced would grow the file without bound and Alacritty would apply every
/// stale copy. And a pointer left behind by the other platform, from a synced
/// home directory or a branch switch, must be corrected rather than trusted.
#[test]
fn generating_is_idempotent_and_heals_a_stale_pointer() {
    let fixtures = tempfile::tempdir().expect("a fixture root");
    let directory = fixture_with_both_variants(fixtures.path(), "alac-idem");
    let pointer = directory.join("alacritty-platform.toml");

    run_generator("mac", &directory);
    let first = read(&pointer);
    run_generator("mac", &directory);
    let second = read(&pointer);
    assert!(
        !first.is_empty(),
        "positive control: the first run produced an empty pointer, so \
         comparing two empty files would prove nothing"
    );
    assert_eq!(
        second, first,
        "running the generator twice changed the pointer, so it accumulates \
         rather than replacing"
    );

    run_generator("linux", &directory);
    let healed = read(&pointer);
    assert!(
        healed.contains("linux-variant"),
        "a stale mac pointer was not rewritten for the current platform"
    );
    assert!(
        !healed.contains("mac-variant"),
        "the stale mac content survived the rewrite, so the generator appends \
         rather than replacing"
    );
}

/// Point 5 of the contract, asserted behaviourally rather than by looking for
/// `mv` in the script.
///
/// `.zshrc` backgrounds the generator from every shell, so on the first
/// startup after a variant edit all ~107 panes run it at once and the
/// content-equality guard lets every one of them through. Alacritty watches
/// the pointer, so a read landing inside a truncate-then-fill window gets a
/// partial config and Alacritty applies whatever parsed.
///
/// A sampling reader watches the file while a writer loops, and no sample may
/// be shorter than the finished file. Measured on the pre-fix code, 14% of
/// reads saw a partial or zero-length file; the atomic form produced 0 out of
/// 3.49M.
///
/// A MISSING FILE IS DELIBERATELY NOT COUNTED. The writer loop deletes the
/// pointer to force a write past the content-equality guard, so an absence is
/// the harness's own doing and says nothing about how the write happens.
/// Counting it would fail the atomic form too, which is exactly the false
/// result this note exists to prevent. What only a non-atomic write can
/// produce is a file that EXISTS and is INCOMPLETE, so that is the signal.
///
/// A large payload is what makes the window observable at all: the real
/// variants are small enough that a single `write(2)` usually completes
/// between two samples.
#[test]
fn a_reader_never_sees_a_partial_pointer() {
    let fixtures = tempfile::tempdir().expect("a fixture root");
    let directory = fixtures.path().join("alac-atomic");
    std::fs::create_dir_all(&directory).expect("a fixture directory");
    std::fs::write(directory.join("alacritty-mac.toml"), "mac-variant\n").expect("the mac variant");

    use std::fmt::Write as _;
    let mut padded = String::from("linux-variant\n");
    for index in 0..4000 {
        let _ = writeln!(padded, "key{index:04} = \"padding that widens the write window\"");
    }
    std::fs::write(directory.join("alacritty-linux.toml"), &padded).expect("the linux variant");

    // One clean run establishes the finished size the samples are judged
    // against.
    run_generator("linux", &directory);
    let pointer = directory.join("alacritty-platform.toml");
    let full_size = std::fs::metadata(&pointer).expect("the pointer exists").len();
    assert!(
        full_size > 100_000,
        "positive control: the finished pointer is only {full_size} bytes, \
         too small for the write window to be observable, so a zero count \
         below would mean nothing"
    );

    let sampled = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let short = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    let sampler = {
        let (pointer, sampled, short, stop) =
            (pointer.clone(), sampled.clone(), short.clone(), stop.clone());
        std::thread::spawn(move || {
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                if let Ok(data) = std::fs::metadata(&pointer) {
                    sampled.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if data.len() < full_size {
                        short.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }
        })
    };

    // Deleting the pointer is what forces the write: the guard reads the
    // pointer, so an absent one can never match and the generator always
    // reaches the write.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut rewrites = 0_u32;
    while std::time::Instant::now() < deadline {
        let _ = std::fs::remove_file(&pointer);
        run_generator("linux", &directory);
        rewrites += 1;
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    sampler.join().expect("the sampler thread joins");

    // Both positive controls, because a run that rewrote nothing or sampled
    // nothing reports zero short reads just as an atomic writer does.
    assert!(
        rewrites > 1,
        "positive control: only {rewrites} rewrites happened, so the sampler \
         had no write window to observe"
    );
    assert!(
        sampled.load(std::sync::atomic::Ordering::Relaxed) > 0,
        "positive control: the sampler read the pointer zero times, so a zero \
         short-read count says nothing about how the write happens"
    );
    assert_eq!(
        short.load(std::sync::atomic::Ordering::Relaxed),
        0,
        "a reader saw the pointer present but shorter than finished, so the \
         generator truncates in place; Alacritty watches this file and would \
         apply whatever parsed"
    );
}

/// The pointer is machine-local, not tracked. Tracking it would put a
/// per-machine file in the repo, so each machine would fight the last one
/// over its contents on every push.
#[test]
fn the_generated_pointer_is_not_tracked() {
    let root = repo_root();
    let git_dir = root.join(".cfg");
    if !git_dir.is_dir() {
        skip("no .cfg repository here, so the pointer cannot be checked against it");
        return;
    }

    let output = Command::new("git")
        .arg(format!("--git-dir={}", git_dir.display()))
        .arg(format!("--work-tree={}", root.display()))
        .args(["ls-files", "--full-name", "--", ":/.config/alacritty"])
        .output()
        .expect("git spawns");
    assert!(
        output.status.success(),
        "git ls-files failed, so an empty result would not mean the pointer \
         is untracked"
    );
    let listed = String::from_utf8_lossy(&output.stdout);
    let tracked: Vec<&str> = listed.lines().collect();

    // The positive control. Without it, an ls-files that matched nothing at
    // all would satisfy the assertion below.
    assert!(
        tracked.contains(&".config/alacritty/alacritty.toml"),
        "positive control: git ls-files did not list the tracked shared \
         config, so its silence about the pointer proves nothing"
    );
    assert!(
        !tracked.contains(&".config/alacritty/alacritty-platform.toml"),
        "the generated pointer is tracked, so every machine would fight the \
         last one over its contents on every push"
    );
}

/// Requested 2026-09-09: keep the Nord status bar, make the terminal's own
/// background full black. The two are separable because nvim runs with
/// `transparent = true` and Nord's tmux panes use `bg=default`, so both show
/// the terminal through, and only Alacritty paints the ground.
///
/// Pinned because of how the old value got there. `0x2E3440` is Nord's polar
/// night, pasted in as part of the whole Nord palette, and a future palette
/// refresh would paste it back. The palette entries are deliberately NOT
/// pinned: `black = 0x3B4252` is what keeps the bar looking like Nord, and
/// that is wanted.
#[test]
fn the_terminal_ground_is_pure_black() {
    let text = read(&alacritty_dir().join("alacritty.toml"));
    let background = text
        .lines()
        .skip_while(|line| !line.starts_with("[colors.primary]"))
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .find_map(|line| {
            let value = line.trim().strip_prefix("background")?.trim_start();
            let value = value.strip_prefix('=')?.trim();
            value.trim_matches('"').split('"').next().map(str::to_string)
        });

    assert_eq!(
        background.as_deref(),
        Some("0x000000"),
        "the terminal background under [colors.primary] is {background:?} \
         rather than pure black; 0x2E3440 is Nord's polar night, which a \
         palette refresh would paste back"
    );
}
