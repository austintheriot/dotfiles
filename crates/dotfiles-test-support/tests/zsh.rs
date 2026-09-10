//! The zsh fixture the Tranche B suites share.
//!
//! Tranche B's subject is the shell, so these tests spawn a real zsh rather
//! than reimplementing its behaviour. The helper exists so seven suites do
//! not each carry their own `Command` construction.

/// The fixture loads the repo's `.zshrc`. This is the property every suite in
/// the tranche depends on: if the fixture does not load the config, every
/// assertion about the config passes vacuously.
///
/// **The probe must be something only `.zshrc` defines.** An earlier draft
/// used `$ZSH_VERSION`, which is a zsh BUILT-IN parameter: `zsh -f -c 'echo
/// $ZSH_VERSION'` prints 5.9 with every startup file skipped, so that
/// assertion passed with the config entirely unloaded. `parse_git_dirty`
/// (`.zshrc:243`) is a function the config defines and nothing else does.
///
/// **The shell must be interactive.** Zsh does not source `.zshrc` for a
/// non-interactive login shell. Measured 2026-09-10: `zsh -l -c 'whence -w
/// parse_git_dirty'` reports `none`, and `zsh -i -c` reports `function`.
#[test]
fn the_fixture_shell_loads_the_repo_zshrc() {
    if !dotfiles_test_support::zsh::available() {
        // Not a skip call: this test proves the fixture works, and a skip
        // here would hide the fixture being broken. See the Global
        // Constraints on why a test must not call skip() to prove skipping.
        eprintln!("zsh absent; the fixture cannot be exercised");
        return;
    }
    let output = dotfiles_test_support::zsh::run("whence -w parse_git_dirty");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("function"),
        "the fixture did not load .zshrc: parse_git_dirty is not defined. \
         Got {text:?}"
    );
}

/// An interactive shell is where aliases and ZLE widgets exist. A
/// non-interactive shell defines neither, so a suite that used the wrong one
/// would assert over an empty namespace and pass.
#[test]
fn the_fixture_shell_defines_aliases() {
    if !dotfiles_test_support::zsh::available() {
        eprintln!("zsh absent; the fixture cannot be exercised");
        return;
    }
    // `.zshrc-mac:46` and `.zshrc-linux:37` each echo a banner line, so
    // stdout is "Loaded mac configuration\n      11\n". Parse the LAST line,
    // and panic on an unparseable count rather than defaulting to zero: an
    // earlier draft used `.unwrap_or(0)` and reported "no aliases at all"
    // for a shell that had defined eleven.
    let output = dotfiles_test_support::zsh::run("alias | wc -l");
    let text = String::from_utf8_lossy(&output.stdout);
    let last = text.trim().lines().last().unwrap_or_default().trim();
    let count: usize = last
        .parse()
        .unwrap_or_else(|_| panic!("expected a count, got {text:?}"));
    assert!(count > 0, "an interactive shell defined no aliases at all");
}

/// A fixture home isolates the shell from the developer's real config, which
/// is what lets a test assert "this variant is NOT loaded" without depending
/// on the machine it runs on.
#[test]
fn a_fixture_home_replaces_the_real_one() {
    if !dotfiles_test_support::zsh::available() {
        eprintln!("zsh absent; the fixture cannot be exercised");
        return;
    }
    let home = tempfile::Builder::new()
        .prefix("zsh-home-")
        .tempdir()
        .expect("a temp home");
    let output = dotfiles_test_support::zsh::run_in_home(home.path(), "echo $HOME");
    let text = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        text.trim(),
        home.path().to_string_lossy(),
        "the fixture home did not take effect"
    );
}

/// The fixture clears the caller's environment, passing only `HOME`, `PATH`
/// and `TERM`.
///
/// This is the assertion that makes every other one in the tranche mean
/// something. `DOTFILES_PLATFORM` and `NVM_DIR` are both set in the
/// developer's own shell, so without isolation an assertion of the form
/// "`.zshrc` produced X" is satisfied by inheritance from whoever ran
/// `cargo test`. Three shell suites already guard against exactly this:
/// `zshrc-node-startup.test.sh:189` calls `env -i` "load-bearing" in its own
/// comment, `zshrc-platform-split.test.sh:96` uses it, and
/// `zshrc-python-startup.test.sh:110` pins `STARTUP_PATH`.
///
/// The probe is `CARGO_PKG_NAME`, which cargo exports into every test
/// process it runs and which no shell config sets. It cannot be absent from
/// this process, so an inherited environment is directly visible rather than
/// inferred, and the test needs no environment mutation of its own (the
/// workspace forbids `unsafe`, and `std::env::set_var` is unsafe in Rust
/// 2024).
#[test]
fn the_fixture_does_not_inherit_the_callers_environment() {
    if !dotfiles_test_support::zsh::available() {
        eprintln!("zsh absent; the fixture cannot be exercised");
        return;
    }
    assert!(
        std::env::var_os("CARGO_PKG_NAME").is_some(),
        "positive control: cargo did not export CARGO_PKG_NAME into this test \
         process, so its absence downstream would prove nothing"
    );
    let output = dotfiles_test_support::zsh::run("echo \"[${CARGO_PKG_NAME:-clean}]\"");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("[clean]"),
        "the caller's environment reached the fixture shell, so any assertion \
         about what .zshrc produced can be satisfied by the developer's own \
         shell instead. Got {text:?}"
    );
}
