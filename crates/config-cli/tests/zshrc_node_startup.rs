//! The lazy nvm setup in `.zshrc-mac`.
//!
//! Sourcing `nvm.sh` costs 2.4s, `nvm ls v24` about 1s more, and `nvm use
//! 24` another 2.4s. Paying all three at every shell startup put interactive
//! zsh at 7-9s, and `se` builds 107 panes, so the cumulative cost ran to
//! minutes before the terminal was usable.
//!
//! `nvm use` only manipulates `PATH`, so putting the newest v24 bin
//! directory on `PATH` directly reproduces its end state for free. nvm
//! itself becomes a lazy function, so the cost is paid only when a version
//! is actually switched.
//!
//! The contract:
//!
//! 1. `NVM_DIR` is exported before the variant that reads it.
//! 2. Startup runs no nvm command, so it stays fast.
//! 3. `node` / `npm` / `npx` still resolve to the newest installed v24.
//! 4. `nvm` is available but not yet loaded.
//! 5. `nvm use <other>` still overrides the version for that shell.
//!
//! Point 5 is worth testing rather than assuming: it works because `nvm use`
//! strips any nvm version directory already on `PATH` before prepending its
//! own, so the manual entry is replaced rather than shadowed.
//!
//! The runtime half is macOS-only because `.zshrc-mac` is. Linux carries
//! `.zshrc-linux` and is unaffected.

use std::path::{Path, PathBuf};

/// What `path_helper` starts a login shell with. See
/// [`switching_versions_replaces_the_startup_path_entry`] for why the
/// `PATH` count cannot use the caller's.
const STARTUP_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

fn nvm_node_directory() -> PathBuf {
    dotfiles_test_support::repo::root().join(".nvm/versions/node")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} is unreadable: {error}", path.display()))
}

/// The 1-based line number of the first line starting with `prefix`.
fn line_starting_with(text: &str, prefix: &str) -> Option<usize> {
    text.lines()
        .position(|line| line.starts_with(prefix))
        .map(|index| index + 1)
}

/// Contract 1, and the only three assertions here that run everywhere.
///
/// **The bug this catches**, reported 2026-09-09 from a bare Ubuntu.
/// `.zshrc-linux`'s lazy-nvm block globs `"$NVM_DIR"/versions/node/v*(N/)`
/// and says in its own comment that "NVM_DIR comes from the shared .zshrc".
/// It does, but LATER: the variant was sourced before `NVM_DIR` was
/// exported, so the glob ran with an empty `NVM_DIR` and matched nothing.
///
/// Measured in zsh at the time: 0 matches with an empty `NVM_DIR`, 11 with
/// it set.
///
/// The consequence is not a slow shell, it is a missing tool. node is
/// installed under `~/.nvm` and never reaches `PATH`, so every npm-backed
/// mason package fails to install and `<leader>f` reports no formatters. The
/// deps engine reports node PRESENT throughout, because its check globs the
/// same directory the shell failed to read.
///
/// Asserted as line ORDER rather than mere presence, because a correct
/// export sitting below the variant source is exactly the bug.
#[test]
fn nvm_dir_is_exported_before_the_variant_that_reads_it() {
    let text = read(&dotfiles_test_support::repo::root().join(".zshrc"));

    let export = line_starting_with(&text, "export NVM_DIR=");
    let variant = line_starting_with(&text, "platform_source_variant");

    let export = export.expect("the shared .zshrc exports NVM_DIR");
    let variant = variant.expect("the shared .zshrc sources a platform variant");

    assert!(
        export < variant,
        "NVM_DIR is exported at line {export}, below the variant sourced at \
         line {variant}, so the variant's `\"$NVM_DIR\"/versions/node/v*(N/)` \
         glob runs against an empty NVM_DIR and matches nothing. node then \
         never reaches PATH and every npm-backed mason package fails to \
         install, while the deps engine still reports node present."
    );
}

/// Whether this machine can exercise the runtime half.
fn runtime_blocker() -> Option<String> {
    if !cfg!(target_os = "macos") {
        return Some(".zshrc-mac is macOS-only".to_string());
    }
    let root = dotfiles_test_support::repo::root();
    if std::env::var_os("HOME").map(PathBuf::from).as_deref() != Some(root.as_path())
        || !root.join(".zshrc-mac").is_file()
    {
        return Some("HOME is not the repo or .zshrc-mac is absent".to_string());
    }
    if !dotfiles_test_support::zsh::available() {
        return Some("no zsh on this machine".to_string());
    }
    None
}

/// Contract 2. Startup runs no nvm command.
///
/// Asserted on the source text: no unconditional `nvm use`, `nvm ls`, or
/// `nvm.sh` source at top level.
///
/// Note this reads `.zshrc-mac` but does NOT need a mac to do it, since the
/// file is tracked either way. Only the shell-spawning tests below are
/// gated on the platform.
#[test]
fn startup_runs_no_nvm_command() {
    let text = read(&dotfiles_test_support::repo::root().join(".zshrc-mac"));

    for (label, matches) in [
        ("source nvm.sh", offending_lines(&text, sources_nvm)),
        ("run `nvm use`", offending_lines(&text, |line| {
            line.starts_with("nvm use")
        })),
        ("run `nvm ls`", offending_lines(&text, |line| {
            line.trim_start_matches("if ")
                .trim_start_matches("! ")
                .starts_with("nvm ls")
        })),
    ] {
        assert!(
            matches.is_empty(),
            ".zshrc-mac appears to {label} at startup, which is the cost this \
             lazy setup exists to avoid: {matches:?}"
        );
    }
}

/// The numbered lines, trimmed, for which `predicate` holds on the trimmed
/// text. Only top-level lines count: an indented line is inside the lazy
/// `nvm()` function, which is where these calls belong.
fn offending_lines(text: &str, predicate: impl Fn(&str) -> bool) -> Vec<String> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.starts_with(char::is_whitespace) && predicate(line))
        .map(|(index, line)| format!("{}: {line}", index + 1))
        .collect()
}

fn sources_nvm(line: &str) -> bool {
    let trimmed = line.trim_start_matches('\\');
    (trimmed.starts_with("source ") || trimmed.starts_with(". ")) && trimmed.contains("nvm.sh")
}

/// The installed node versions, newest last, sorted the way the shell code
/// must sort them.
///
/// A plain lexical sort picks v24.9 over v24.15, so the version-aware
/// comparison is part of the contract rather than a detail. This machine has
/// both v24.6.0 and v24.15.0 installed, so a lexical sort here would pick
/// the wrong one and the assertions below would fail loudly rather than
/// silently agreeing with a wrong shell.
fn installed_versions() -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(nvm_node_directory()) else {
        return Vec::new();
    };
    let mut found: Vec<(Vec<u64>, PathBuf)> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter_map(|path| Some((version_key(&file_name(&path))?, path)))
        .collect();
    found.sort();
    found.into_iter().map(|(_, path)| path).collect()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string()
}

/// The numeric components of a `vN.N.N` directory name, for ordering.
fn version_key(name: &str) -> Option<Vec<u64>> {
    let digits = name.strip_prefix('v')?;
    digits
        .split('.')
        .map(|part| part.parse().ok())
        .collect::<Option<Vec<u64>>>()
        .filter(|parts| parts.len() == 3)
}

/// The value a probe script printed for `name`, from `KEY=value` lines.
fn field(text: &str, name: &str) -> String {
    text.lines()
        .find_map(|line| line.strip_prefix(&format!("{name}=")))
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Contracts 3 and 4. The end state is reached, and nvm is still lazy.
///
/// The laziness probe tests behaviour rather than size. A loaded nvm is a
/// large function and the lazy shim is a small one, but size is not a
/// contract: the real nvm defines `nvm_ls_current` and the shim does not.
#[test]
fn startup_reaches_nvms_end_state_without_running_it() {
    if let Some(reason) = runtime_blocker() {
        dotfiles_test_support::skip(&format!("{reason}: node startup state"));
        return;
    }
    let versions = installed_versions();
    let Some(newest_v24) = versions.iter().rev().find(|path| {
        file_name(path).starts_with("v24.")
    }) else {
        dotfiles_test_support::skip(&format!(
            "no v24 installed under {}: node startup state",
            nvm_node_directory().display()
        ));
        return;
    };

    let output = dotfiles_test_support::zsh::run(
        r#"
        print -r -- "node=$(command -v node)"
        print -r -- "npm=$(command -v npm)"
        print -r -- "npx=$(command -v npx)"
        print -r -- "version=$(node --version)"
        if typeset -f nvm >/dev/null 2>&1; then
            if typeset -f nvm_ls_current >/dev/null 2>&1; then
                print "kind=eager"
            else
                print "kind=lazy"
            fi
        else
            print "kind=missing"
        fi
        "#,
    );
    let report = String::from_utf8_lossy(&output.stdout);

    for tool in ["node", "npm", "npx"] {
        assert_eq!(
            field(&report, tool),
            newest_v24.join("bin").join(tool).to_string_lossy(),
            "{tool} does not resolve to the newest installed v24 \
             ({}); report was {report:?}",
            newest_v24.display()
        );
    }
    assert_eq!(
        field(&report, "version"),
        file_name(newest_v24),
        "node reports a different version from the directory it resolved to, \
         so PATH points at one install and the binary is another; report was \
         {report:?}"
    );
    assert_eq!(
        field(&report, "kind"),
        "lazy",
        "nvm is not in the lazy state at startup, so either it is missing or \
         the 2.4s nvm.sh source already ran; report was {report:?}"
    );
}

/// Contract 5. Switching overrides the version and does not stack.
///
/// This is the assertion worth having rather than assuming. `nvm use`
/// strips any nvm version directory already on `PATH` before prepending its
/// own, so the startup entry is REPLACED. If it were merely shadowed, every
/// switch would grow `PATH` for the life of the shell.
///
/// The `PATH` count runs against a PINNED baseline, and that is load-bearing
/// rather than tidy. The fixture otherwise passes the caller's `PATH`
/// through, and a `cargo test` process carries nvm directories from whatever
/// the developer's shell did earlier, so counting entries in an inherited
/// environment measures that history rather than this config. Observed while
/// writing this test: the inherited run counted 4 nvm directories where a
/// pristine login shell on the same machine counted 1. The shell suite hit
/// the same wall and its comment at `zshrc-node-startup.test.sh:189` calls
/// `env -i` "load-bearing" for exactly this assertion.
#[test]
fn switching_versions_replaces_the_startup_path_entry() {
    if let Some(reason) = runtime_blocker() {
        dotfiles_test_support::skip(&format!("{reason}: nvm version switching"));
        return;
    }
    let versions = installed_versions();
    let Some(other) = versions
        .iter()
        .rev()
        .find(|path| !file_name(path).starts_with("v24."))
    else {
        dotfiles_test_support::skip(
            "only v24 is installed, so there is no other version to switch to",
        );
        return;
    };
    let other_name = file_name(other);
    let bare = other_name.trim_start_matches('v').to_string();

    let output = dotfiles_test_support::zsh::run_with_path(
        STARTUP_PATH,
        &format!(
            r#"
        nvm use {bare} >/dev/null 2>&1
        print -r -- "which=$(command -v node)"
        print -r -- "reports=$(node --version)"
        print -r -- "entries=$(print -l ${{(s/:/)PATH}} | grep -c 'nvm/versions/node')"
        "#
        ),
    );
    let report = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        field(&report, "which"),
        other.join("bin/node").to_string_lossy(),
        "`nvm use {bare}` did not override node for that shell; report was \
         {report:?}"
    );
    assert_eq!(
        field(&report, "reports"),
        other_name,
        "`nvm use {bare}` did not report the switched version; report was \
         {report:?}"
    );
    assert_eq!(
        field(&report, "entries"),
        "1",
        "switching left more than one nvm directory on PATH, so the startup \
         entry was shadowed rather than replaced and every further switch \
         grows PATH for the life of the shell; report was {report:?}"
    );
}
