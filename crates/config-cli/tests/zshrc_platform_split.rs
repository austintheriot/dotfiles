//! The `.zshrc` / `.zshrc-<platform>` split.
//!
//! `.zshrc` used to be a per-branch file. Both branches carried a near-copy
//! of the same 300 lines and they drifted: mac grew a lazy pyenv init worth
//! 380ms that linux never received, linux grew an fzf fallback, a
//! `$HOME`-relative bun path and a stderr redirect that mac never received.
//! None of the four were platform-specific. They were fixes applied on
//! whichever machine hit the problem.
//!
//! So `.zshrc` is now shared, and the genuinely platform-specific lines live
//! in `.zshrc-mac` / `.zshrc-linux`, selected at runtime.
//!
//! **This suite is a re-derivation rather than a port.** The shell suite's
//! header recorded contract 3 as "both variants exist here, so neither can
//! drift unseen". That was a CROSS-BRANCH guarantee: when `mac` and `linux`
//! were separate branches, both variants shipping on both branches is what
//! made drift visible. The 2026-09-06 collapse left one branch, and one
//! branch cannot drift from itself, so the recorded reason no longer
//! describes anything. The two file-exists checks survive with the reason
//! that holds today, stated on each test below.
//!
//! The contract, as it stands on one branch:
//!
//! 1. The shared `.zshrc` exists and names no platform-specific path.
//! 2. Both variants ship, so one checkout serves both platforms.
//! 3. The shared file selects its variant at runtime through the platform
//!    helper, not through a hardcoded source chain.
//! 4. Exactly one variant loads for a given platform.
//! 5. No setting has two owners.
//! 6. No configuration survives for a framework that is not loaded.

use std::path::{Path, PathBuf};

/// The shared file, read once per test that needs it.
fn shared_zshrc() -> String {
    read(&dotfiles_test_support::repo::root().join(".zshrc"))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} is unreadable: {error}", path.display()))
}

/// Every line of `text` containing `needle`, numbered, for a failure message
/// that names where the offending line is rather than only that one exists.
fn lines_containing(text: &str, needle: &str) -> Vec<String> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| line.contains(needle))
        .map(|(index, line)| format!("{}: {line}", index + 1))
        .collect()
}

/// Assertion 1. The shared file is the subject of most of this suite, so its
/// absence would make the rest fail in a way that names the wrong thing.
#[test]
fn the_shared_zshrc_exists() {
    let path = dotfiles_test_support::repo::root().join(".zshrc");
    assert!(path.is_file(), "{} is not a file", path.display());
}

/// Assertions 2 and 3. Both variants ship on the single branch.
///
/// **This is the re-derived reason.** The shell suite recorded it as "both
/// variants exist here, so neither can drift unseen", which was about two
/// branches each carrying both files: that is what made a divergence
/// visible in a diff. One branch cannot drift from itself, so that reason
/// is gone.
///
/// What the checks are about now is what the collapse actually bought: one
/// checkout serves both platforms. A variant that was absent would leave
/// its platform sourcing nothing at all, and `platform_source_variant`
/// treats an absent variant as the non-error case (a platform with nothing
/// extra to say carries no variant file), so nothing else in the repo would
/// notice. These two assertions are that notice.
#[test]
fn both_platform_variants_ship_here() {
    let root = dotfiles_test_support::repo::root();
    for variant in [".zshrc-mac", ".zshrc-linux"] {
        let path = root.join(variant);
        assert!(
            path.is_file(),
            "{variant} is absent, so a {} checkout would source no variant at \
             all and platform_source_variant would treat that as the normal \
             no-variant case",
            variant.trim_start_matches(".zshrc-")
        );
    }
}

/// Assertions 4 and 5. The shared file names no path that BREAKS on the
/// other platform.
///
/// `/Applications` and `/opt/homebrew` are macOS-only, so a shared file
/// naming either holds a line that belongs in `.zshrc-mac`.
///
/// Debian's `/usr/share/doc/fzf` path is deliberately NOT on this list. It
/// sits behind a `[ -f ]` guard that is simply false on mac, so it costs a
/// stat and describes no assumption. The rule is "no path that breaks
/// elsewhere", not "no path that exists in only one place".
#[test]
fn the_shared_zshrc_names_no_platform_specific_path() {
    let text = shared_zshrc();
    for needle in ["/Applications/", "/opt/homebrew"] {
        let found = lines_containing(&text, needle);
        assert!(
            found.is_empty(),
            "the shared .zshrc names {needle}, which is macOS-only and belongs \
             in .zshrc-mac: {found:?}"
        );
    }
}

/// Assertion 6. No hardcoded home directory.
///
/// The concrete drift: mac had `/Users/austin/.bun` and linux had
/// `$HOME/.bun`. Neither shape should name a user or a home layout.
#[test]
fn the_shared_zshrc_hardcodes_no_home_directory() {
    let text = shared_zshrc();
    let found: Vec<String> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            ["/Users/", "/home/"].iter().any(|prefix| {
                line.split(prefix).skip(1).any(|rest| {
                    rest.chars()
                        .next()
                        .is_some_and(|first| first.is_ascii_lowercase())
                })
            })
        })
        .map(|(index, line)| format!("{}: {line}", index + 1))
        .collect();
    assert!(
        found.is_empty(),
        "the shared .zshrc names a home directory, which is the drift that \
         put /Users/austin/.bun on one branch and $HOME/.bun on the other: \
         {found:?}"
    );
}

/// Assertions 7 and 8. Selection goes through the shared helper.
///
/// Spelled via the helper rather than an inline `if [ -f ~/.zshrc-mac ]`
/// chain per platform. The chain is what made adding `.zshrc-wsl` a
/// three-line edit that only ever landed on one branch.
#[test]
fn the_shared_zshrc_selects_its_variant_through_the_platform_helper() {
    let text = shared_zshrc();
    let sources_helper = text
        .lines()
        .any(|line| line.contains(".scripts/platform.sh") && sources(line));
    assert!(
        sources_helper,
        "the shared .zshrc does not source .scripts/platform.sh, so the \
         naming convention has no single owner"
    );
    assert!(
        text.contains("platform_source_variant"),
        "the shared .zshrc does not call platform_source_variant, so it loads \
         no variant through the helper"
    );
}

/// Whether a shell line is a `source` or `.` of something.
fn sources(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("source ") || trimmed.starts_with(". ")
}

/// Assertion 9. The old shape is asserted gone.
///
/// A hardcoded per-platform source chain would still work today and drift
/// again tomorrow, so its absence is checked rather than assumed.
#[test]
fn no_hardcoded_per_platform_source_chain_remains() {
    let text = shared_zshrc();
    let found: Vec<String> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim_start();
            trimmed.starts_with("if [ -f ~/.zshrc-mac ]")
                || trimmed.starts_with("if [ -f ~/.zshrc-linux ]")
        })
        .map(|(index, line)| format!("{}: {line}", index + 1))
        .collect();
    assert!(
        found.is_empty(),
        "a hardcoded per-platform source chain is back in the shared .zshrc: \
         {found:?}"
    );
}

/// Assertions 10 through 13. Exactly one variant loads, and it is this
/// platform's.
///
/// Behavioural, not textual: a real zsh runs against a fixture `$HOME`
/// holding the real variants and only the variant-loading part of the
/// shared file. Both platforms are driven from whichever machine runs the
/// suite, which is the whole reason `DOTFILES_PLATFORM` is overridable.
///
/// The marker is each variant's own `echo "Loaded <platform> configuration"`
/// (`.zshrc-mac:46`, `.zshrc-linux:37`). Nothing is added to either file to
/// make this observable: a test that changes its subject in order to become
/// testable is a worse trade than one that reads what is already there.
///
/// The "does NOT load" half is the load-bearing one. A helper that sourced
/// every variant it found would reintroduce the bug where linux ran mac's
/// Homebrew autosuggestions path, and only this catches it.
#[test]
fn exactly_this_platforms_variant_loads() {
    if !dotfiles_test_support::zsh::available() {
        dotfiles_test_support::skip(
            "no zsh here, so runtime variant selection cannot be observed",
        );
        return;
    }
    for platform in ["mac", "linux"] {
        let home = variant_fixture_home(platform);
        let output = dotfiles_test_support::zsh::run_in_home(home.path(), "true");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let other = if platform == "mac" { "linux" } else { "mac" };
        assert!(
            text.contains(&format!("Loaded {platform} configuration")),
            "a {platform} shell did not load the {platform} variant; output \
             was {text:?}"
        );
        assert!(
            !text.contains(&format!("Loaded {other} configuration")),
            "a {platform} shell also loaded the {other} variant, which is the \
             bug where linux ran mac's Homebrew autosuggestions path; output \
             was {text:?}"
        );
    }
}

/// A fixture `$HOME` holding the real platform helper, the real variants,
/// and only the variant-loading lines of the shared `.zshrc`.
///
/// Sourcing the whole shared file would pull in compinit, zoxide and the
/// deps hook, none of which this test is about and all of which are slow.
/// The shell suite scoped it the same way, by the same section markers.
///
/// `DOTFILES_PLATFORM` is pinned INSIDE the fixture `.zshrc` rather than
/// passed in the script argument, and that placement is load-bearing. The
/// fixture shell is interactive, so zsh sources `~/.zshrc` on startup by
/// itself; a script argument that exported the platform and sourced the file
/// a second time loaded a variant TWICE, once under the host's own detected
/// platform and once under the requested one. Observed while writing this
/// test: the linux case printed both banners and the "does not load the
/// other variant" assertion failed against its own fixture rather than
/// against the helper. One natural load, with the platform decided before
/// the helper runs, is what the assertion is actually about.
fn variant_fixture_home(platform: &str) -> tempfile::TempDir {
    let root = dotfiles_test_support::repo::root();
    let home = tempfile::Builder::new()
        .prefix("zshrc-variant-")
        .tempdir()
        .expect("a temp home");
    let scripts = home.path().join(".scripts");
    std::fs::create_dir_all(&scripts).expect("a fixture .scripts directory");
    copy(&root.join(".scripts/platform.sh"), &scripts.join("platform.sh"));
    for variant in [".zshrc-mac", ".zshrc-linux"] {
        copy(&root.join(variant), &home.path().join(variant));
    }
    let zshrc = format!(
        "export DOTFILES_PLATFORM={platform}\n{}\n",
        variant_loading_section()
    );
    std::fs::write(home.path().join(".zshrc"), zshrc).expect("a fixture .zshrc");
    home
}

fn copy(from: &Path, to: &PathBuf) {
    std::fs::copy(from, to)
        .unwrap_or_else(|error| panic!("copying {} failed: {error}", from.display()));
}

/// The shared `.zshrc` between its `ENVIRONMENT-SPECIFIC` marker and the
/// `# use neovim` line that follows it.
///
/// Panics rather than returning an empty section when either marker moves:
/// an empty fixture `.zshrc` would load no variant and every assertion in
/// `exactly_this_platforms_variant_loads` would fail with a message blaming
/// the helper, which is the wrong subject.
fn variant_loading_section() -> String {
    let text = shared_zshrc();
    let mut lines = text.lines().skip_while(|line| !line.contains("ENVIRONMENT-SPECIFIC"));
    let section: Vec<&str> = lines
        .by_ref()
        .take_while(|line| !line.starts_with("# use neovim"))
        .collect();
    assert!(
        section.iter().any(|line| line.contains("platform_source_variant")),
        "the ENVIRONMENT-SPECIFIC section of .zshrc no longer contains \
         platform_source_variant, so this fixture would load no variant and \
         the failure would name the wrong subject"
    );
    section.join("\n")
}

/// Assertions 14 through 17. No setting has two owners.
///
/// The concrete bug: linux's `.zshrc` set `NVM_DIR` and sourced `nvm.sh`
/// eagerly at the bottom, and `.zshrc-linux` did it again. The eager copy
/// also defeated the lazy shim that exists to keep 107 tmux panes from
/// costing minutes.
///
/// Both variants need `NVM_DIR`, so it belongs in the shared file, and then
/// neither variant may set it again.
#[test]
fn no_setting_has_two_owners() {
    let root = dotfiles_test_support::repo::root();
    let shared = shared_zshrc();

    let eager_nvm: Vec<String> = shared
        .lines()
        .enumerate()
        .filter(|(_, line)| sources(line) && line.contains("nvm.sh"))
        .map(|(index, line)| format!("{}: {line}", index + 1))
        .collect();
    assert!(
        eager_nvm.is_empty(),
        "the shared .zshrc sources nvm.sh eagerly, which defeats the lazy \
         shim both variants define: {eager_nvm:?}"
    );

    let shared_owners = exports_nvm_dir(&shared);
    assert_eq!(
        shared_owners.len(),
        1,
        "the shared .zshrc must set NVM_DIR exactly once, since both variants \
         need it and neither may set it again; found {shared_owners:?}"
    );

    for variant in [".zshrc-mac", ".zshrc-linux"] {
        let found = exports_nvm_dir(&read(&root.join(variant)));
        assert!(
            found.is_empty(),
            "{variant} re-sets NVM_DIR, which the shared .zshrc already owns: \
             {found:?}"
        );
    }
}

/// The numbered lines of `text` that export `NVM_DIR`.
fn exports_nvm_dir(text: &str) -> Vec<String> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| line.trim_start().starts_with("export NVM_DIR="))
        .map(|(index, line)| format!("{}: {line}", index + 1))
        .collect()
}

/// Assertions 18 and 19. No configuration for a framework that is not
/// loaded.
///
/// `.zshrc` carried `plugin=(git)` for years and it did nothing. oh-my-zsh
/// is never sourced (`.zshrc-linux` reaches into
/// `~/.oh-my-zsh/custom/plugins` by absolute path precisely BECAUSE the
/// framework is not loaded) and the variable oh-my-zsh reads is `plugins`,
/// plural. So the line was a misspelled setting for an absent framework, and
/// its comment claimed the git plugin "comes with zsh", which is a third
/// wrong thing: the git aliases in this repo are installed into git config
/// by the block at `.zshrc:101`.
///
/// Asserted rather than just deleted, because the line is the kind that gets
/// re-added from memory of how a normal oh-my-zsh setup looks. If oh-my-zsh
/// is ever genuinely sourced, the first assertion is what says so and this
/// test is what gets revisited.
#[test]
fn no_configuration_survives_for_an_absent_framework() {
    let root = dotfiles_test_support::repo::root();
    for name in [".zshrc", ".zshrc-mac", ".zshrc-linux"] {
        let text = read(&root.join(name));

        let sourced: Vec<String> = text
            .lines()
            .enumerate()
            .filter(|(_, line)| sources(line) && line.contains("oh-my-zsh/oh-my-zsh.sh"))
            .map(|(index, line)| format!("{}: {line}", index + 1))
            .collect();
        assert!(
            sourced.is_empty(),
            "{name} sources the oh-my-zsh framework, so the reasoning behind \
             the plugin-list assertion below needs revisiting: {sourced:?}"
        );

        let declared: Vec<String> = text
            .lines()
            .enumerate()
            .filter(|(_, line)| {
                let trimmed = line.trim_start();
                trimmed.starts_with("plugin=(") || trimmed.starts_with("plugins=(")
            })
            .map(|(index, line)| format!("{}: {line}", index + 1))
            .collect();
        assert!(
            declared.is_empty(),
            "{name} declares an oh-my-zsh plugin list for a framework it never \
             sources: {declared:?}"
        );
    }
}
