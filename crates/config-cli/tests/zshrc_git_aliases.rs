//! The git alias installation in `.zshrc`.
//!
//! The aliases are installed from `.zshrc` rather than a tracked
//! `.gitconfig`, so that the real `~/.gitconfig` (which carries credentials
//! and per-machine settings) never has to live in this repo. That is the
//! right call. The cost was how it was done: one `git config --global --get`
//! per alias, on every shell startup, to discover that all nine were already
//! installed. Nine subprocesses, measured at 140ms, to change nothing.
//!
//! The contract:
//!
//! 1. An already-installed set costs no git subprocess at startup.
//! 2. A missing alias is still installed.
//! 3. An alias whose value was changed by hand is left alone.
//!
//! Point 3 is the one a sentinel can get wrong. A guard that skips the whole
//! block once any alias exists must not then be a guard that reinstalls
//! every alias whenever one is missing, or it would overwrite a deliberate
//! local edit.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The aliases the block is expected to install.
const EXPECTED_ALIASES: [&str; 9] = [
    "co",
    "br",
    "cm",
    "st",
    "p",
    "pl",
    "lg",
    "pr",
    "change-commits",
];

fn zshrc_path() -> PathBuf {
    dotfiles_test_support::repo::root().join(".zshrc")
}

fn zshrc_text() -> String {
    let path = zshrc_path();
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is unreadable: {error}", path.display()))
}

/// The installer block, from the `# GIT ALIASES` banner to the
/// `# ENVIRONMENT-SPECIFIC` one that follows it.
///
/// Extracted and run in isolation because everything else in `.zshrc` (nvm,
/// pyenv, compinit) is irrelevant here and slow.
fn installer_block() -> String {
    let text = zshrc_text();
    text.lines()
        .skip_while(|line| !line.starts_with("# GIT ALIASES"))
        .take_while(|line| !line.starts_with("# ENVIRONMENT-SPECIFIC"))
        .collect::<Vec<&str>>()
        .join("\n")
}

/// A throwaway `$HOME` with its own gitconfig, so the developer's real
/// global config is never read or written.
struct GitFixture {
    home: tempfile::TempDir,
    installer: PathBuf,
}

impl GitFixture {
    fn new() -> Self {
        let home = tempfile::Builder::new()
            .prefix("git-aliases-")
            .tempdir()
            .expect("a temp home");
        let installer = home.path().join("installer.zsh");
        std::fs::write(&installer, installer_block()).expect("the installer block is written");
        Self { home, installer }
    }

    fn config_path(&self) -> PathBuf {
        self.home.path().join(".gitconfig")
    }

    /// Runs the installer block in a non-interactive zsh.
    ///
    /// Non-interactive on purpose, and it is not the fixture's `run`: this
    /// sources ONLY the extracted block, so nothing else in `.zshrc` runs
    /// and no interactive shell is needed for a block that defines no
    /// aliases or widgets.
    fn install(&self) {
        self.install_with_path(&std::env::var("PATH").unwrap_or_default());
    }

    fn install_with_path(&self, path: &str) {
        let status = Command::new("zsh")
            .arg("-c")
            .arg(format!(". {}", shell_quote(&self.installer.to_string_lossy())))
            .env("HOME", self.home.path())
            .env("GIT_CONFIG_GLOBAL", self.config_path())
            .env("PATH", path)
            .output()
            .expect("zsh spawns");
        assert!(
            status.status.success() || status.status.code() == Some(1),
            "the installer block failed: {}",
            String::from_utf8_lossy(&status.stderr)
        );
    }

    fn git(&self, arguments: &[&str]) -> String {
        let output = Command::new("git")
            .args(arguments)
            .env("HOME", self.home.path())
            .env("GIT_CONFIG_GLOBAL", self.config_path())
            .output()
            .expect("git spawns");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn alias_value(&self, name: &str) -> String {
        self.git(&["config", "--global", "--get", &format!("alias.{name}")])
    }

    fn core_value(&self, name: &str) -> String {
        self.git(&["config", "--global", "--get", &format!("core.{name}")])
    }

    fn reset(&self) {
        let _ = std::fs::remove_file(self.config_path());
    }
}

/// A single-quoted shell word, so a path with a space cannot split.
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// Assertions 1 and 2. The file exists, and startup spends no subprocess per
/// alias.
///
/// Asserted against the source text, the same way the nvm suite does it: an
/// unconditional `git config --get` per alias at top level is the shape
/// being removed, and it is visible without running a shell.
#[test]
fn startup_spends_no_subprocess_per_alias() {
    assert!(
        zshrc_path().is_file(),
        "{} is not a file",
        zshrc_path().display()
    );
    let offenders: Vec<String> = zshrc_text()
        .lines()
        .enumerate()
        .filter(|(_, line)| line.starts_with("if ! git config --global --get"))
        .map(|(index, line)| format!("{}: {line}", index + 1))
        .collect();
    assert!(
        offenders.is_empty(),
        "a per-alias `git config --get` runs at top level, which is the nine \
         subprocesses at 140ms per shell that this block exists to avoid: \
         {offenders:?}"
    );
}

/// Assertions 3 through 6. A fresh gitconfig gets every alias, with its
/// arguments intact.
#[test]
fn a_fresh_gitconfig_gets_every_alias() {
    let fixture = GitFixture::new();
    assert!(
        !installer_block().trim().is_empty(),
        "the alias block was not extracted from .zshrc, so every assertion \
         below would run an empty installer"
    );

    fixture.reset();
    fixture.install();

    let missing: Vec<&str> = EXPECTED_ALIASES
        .iter()
        .copied()
        .filter(|name| fixture.alias_value(name).is_empty())
        .collect();
    assert!(
        missing.is_empty(),
        "a fresh gitconfig did not get every alias installed: {missing:?}"
    );

    assert_eq!(fixture.alias_value("co"), "checkout", "co is not checkout");
    // An alias with arguments is the one a naive installer truncates.
    assert_eq!(
        fixture.alias_value("lg"),
        "log --oneline",
        "lg did not keep its arguments"
    );
}

/// Assertions 7 through 9. A rerun changes nothing, a missing alias comes
/// back, and a hand-edited one survives.
///
/// The third is the failure a sentinel guard introduces: skip when
/// everything is present, but reinstall everything when one is missing,
/// clobbering a local edit. The block reads all the aliases at once rather
/// than sentinelling on one, which is what keeps each installed
/// independently.
#[test]
fn a_rerun_installs_what_is_missing_and_touches_nothing_else() {
    let fixture = GitFixture::new();
    fixture.reset();
    fixture.install();

    let before = std::fs::read_to_string(fixture.config_path()).expect("a gitconfig");
    fixture.install();
    let after = std::fs::read_to_string(fixture.config_path()).expect("a gitconfig");
    assert_eq!(
        before, after,
        "a second run did not leave the config byte-identical"
    );

    fixture.git(&["config", "--global", "alias.co", "checkout --guess"]);
    fixture.git(&["config", "--global", "--unset", "alias.st"]);
    fixture.install();

    assert_eq!(
        fixture.alias_value("st"),
        "status",
        "the missing alias was not reinstalled"
    );
    assert_eq!(
        fixture.alias_value("co"),
        "checkout --guess",
        "the hand-edited alias was overwritten, which is what a sentinel \
         guard does when it reinstalls the whole set because one alias is \
         missing"
    );
}

/// Assertions 10 and 11. The core settings install too.
///
/// `core.editor` and `core.excludeFile` are installed in their own read:
/// they are a different config section, and one regexp over both would match
/// every `core.*` the user has set for their own reasons.
#[test]
fn the_core_settings_are_installed() {
    let fixture = GitFixture::new();
    fixture.reset();
    fixture.install();

    assert_eq!(
        fixture.core_value("editor"),
        "nvim",
        "core.editor is not installed"
    );
    assert!(
        !fixture.core_value("excludeFile").is_empty(),
        "core.excludeFile is not installed"
    );
}

/// Assertions 12 through 14. A fully-installed config costs two git reads
/// and nothing else.
///
/// **This is the assertion that would otherwise have stayed green while the
/// shell paid a subprocess on every startup.** git lowercases a key when it
/// reports it, so `core.excludeFile` reads back as `core.excludefile`. A
/// matcher comparing against the camel-cased name never finds it, and every
/// startup rewrites the setting it just found.
///
/// Comparing the file before and after cannot see that: rewriting a value
/// with the same value leaves the bytes identical. The write has to be
/// COUNTED, so git is stubbed and its invocations logged.
#[test]
fn a_fully_installed_config_costs_two_git_reads_and_nothing_else() {
    let fixture = GitFixture::new();
    fixture.reset();
    fixture.install();

    let stub_directory = fixture.home.path().join("git-stub");
    std::fs::create_dir_all(&stub_directory).expect("a stub directory");
    let call_log = fixture.home.path().join("git-calls");
    let real_git = which_git();
    write_git_stub(&stub_directory.join("git"), &real_git, &call_log);
    std::fs::write(&call_log, "").expect("an empty call log");

    let path = format!(
        "{}:{}",
        stub_directory.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    fixture.install_with_path(&path);

    let calls = std::fs::read_to_string(&call_log).unwrap_or_default();
    let lines: Vec<&str> = calls.lines().filter(|line| !line.trim().is_empty()).collect();

    let writes: Vec<&&str> = lines
        .iter()
        .filter(|line| {
            line.contains("config --global alias.") || line.contains("config --global core.")
        })
        .collect();
    assert!(
        writes.is_empty(),
        "a fully-installed config triggered a git write at startup, which is \
         the camel-case rewrite bug a byte comparison cannot see: {writes:?}"
    );

    let reads = lines
        .iter()
        .filter(|line| line.contains("--get-regexp"))
        .count();
    assert_eq!(
        reads, 2,
        "the whole block should cost two git reads, one for the aliases and \
         one for the core settings; calls were {lines:?}"
    );
    assert_eq!(
        lines.len(),
        2,
        "the block made a git call beyond the two reads; calls were {lines:?}"
    );
}

/// The real git, resolved the way a shell does.
fn which_git() -> PathBuf {
    let output = Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .expect("sh spawns");
    PathBuf::from(String::from_utf8_lossy(&output.stdout).trim())
}

/// A `git` that logs its arguments and then execs the real one.
fn write_git_stub(path: &Path, real_git: &Path, call_log: &Path) {
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> {log}\nexec {git} \"$@\"\n",
        log = shell_quote(&call_log.to_string_lossy()),
        git = shell_quote(&real_git.to_string_lossy()),
    );
    std::fs::write(path, script).expect("the git stub is written");
    make_executable(path);
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("the git stub is executable");
}

/// Assertion 15. The installed set is listed in one place.
///
/// Two copies (a sentinel list and an installer list) is how an alias gets
/// added to one and not the other, and the block then reports itself
/// complete while an alias is missing.
#[test]
fn each_alias_value_appears_once() {
    let occurrences = zshrc_text()
        .lines()
        .filter(|line| line.contains("git config --global alias."))
        .count();
    assert!(
        occurrences <= EXPECTED_ALIASES.len(),
        "`git config --global alias.` appears {occurrences} times, more than \
         the {} aliases installed, so the set is listed in more than one \
         place and an alias can be added to one list and not the other",
        EXPECTED_ALIASES.len()
    );
}

/// Assertions 16 through 20. `parse_git_dirty` asks git for a machine
/// format.
///
/// It runs inside `PS1`, so it costs its full runtime on every prompt in
/// every pane. A bare `git status` measured 299ms in a 24k-file worktree;
/// the porcelain form measured 44.6ms.
///
/// Asserted on the source rather than by timing, because a timing assertion
/// here would be measuring the machine's git rather than this change.
#[test]
fn parse_git_dirty_asks_git_for_a_machine_format() {
    let text = zshrc_text();
    assert!(
        text.lines().any(|line| line.starts_with("parse_git_dirty()")),
        "parse_git_dirty is no longer defined, but it is what colours the \
         prompt on every draw"
    );

    let body: String = text
        .lines()
        .skip_while(|line| !line.starts_with("parse_git_dirty()"))
        .take_while(|line| !line.starts_with('}'))
        .collect::<Vec<&str>>()
        .join("\n");
    assert!(
        !body.trim().is_empty(),
        "the parse_git_dirty body was not extracted, so the assertions below \
         would run over an empty string"
    );

    assert!(
        body.contains("--porcelain"),
        "parse_git_dirty does not ask git for a machine format, so it pays \
         the 299ms human-readable path on every prompt draw"
    );
    assert!(
        body.contains("-uno"),
        "parse_git_dirty walks untracked files, which is the expensive half \
         of `git status` in a large worktree"
    );

    let prose: Vec<&str> = [
        "Changes to be committed",
        "Changes not staged",
        "Untracked files",
    ]
    .into_iter()
    .filter(|needle| body.contains(needle))
    .collect();
    assert!(
        prose.is_empty(),
        "parse_git_dirty still matches human-readable git prose, which \
         breaks under any locale but English and means it is not reading the \
         porcelain format: {prose:?}"
    );
}
