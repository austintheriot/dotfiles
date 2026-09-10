//! The shell-state isolation of `.zshrc` aliases.
//!
//! An alias that runs `source` executes the script in the interactive shell
//! itself, so every `set` / `setopt` the script performs outlives the
//! command. `.scripts/tmux-update-window-names.sh` sets `set -u`, which is
//! correct for a standalone script and wrong for the caller's shell: with
//! `nounset` left on, the next keystroke makes zsh-autosuggestions read the
//! ZLE-only parameter `POSTDISPLAY` outside a widget and print
//!
//! ```text
//! _zsh_autosuggest_highlight_apply:3: POSTDISPLAY: parameter not set
//! ```
//!
//! before every prompt until the shell is restarted.
//!
//! The scripts that legitimately need sourcing are the ones that change the
//! caller's state on purpose (`cd`, tmux client state). Those set no shell
//! options. The rule enforced here: an alias may only `source` a script that
//! sets no shell options.

use std::path::PathBuf;

/// One alias that sources a script, and the script it sources.
#[derive(Debug)]
struct SourcingAlias {
    name: String,
    relative_path: String,
}

/// Every alias in `.zshrc` whose body sources a script from `.scripts`.
///
/// The `source` is matched anywhere in the alias body, not only as the whole
/// body. An alias that wraps the source in a command substitution to capture
/// what the script printed (`s` and `se` do, so the attach can happen in the
/// caller's own terminal) still runs that script's `setopt` calls in the
/// shell, so it still has to be covered. An anchored whole-body pattern
/// dropped both of those the moment the alias grew a wrapper, and a guard
/// that silently stops covering a file is indistinguishable from no guard.
fn sourcing_aliases(text: &str) -> Vec<SourcingAlias> {
    text.lines()
        .filter_map(|line| {
            let body = line.strip_prefix("alias ")?;
            let (name, rest) = body.split_once('=')?;
            let quoted = rest.strip_prefix('\'')?;
            let (inside, _) = quoted.split_once('\'')?;
            let after_source = inside.split("source ~/").nth(1)?;
            let path: String = after_source
                .chars()
                .take_while(|character| {
                    character.is_ascii_alphanumeric() || "_/.-".contains(*character)
                })
                .collect();
            path.starts_with(".scripts/").then(|| SourcingAlias {
                name: name.to_string(),
                relative_path: path,
            })
        })
        .collect()
}

/// Whether a line sets a shell option that would escape into the caller.
///
/// `set -u` / `set -e` and a bare `setopt` all leak; `setopt localoptions
/// ...` is scoped to the enclosing function and is fine.
fn leaks_a_shell_option(line: &str) -> bool {
    let without_comment = line.split('#').next().unwrap_or_default();
    let trimmed = without_comment.trim_start();
    if let Some(rest) = trimmed.strip_prefix("setopt ") {
        return !rest.trim_start().starts_with("localoptions");
    }
    trimmed
        .strip_prefix("set -")
        .is_some_and(|flags| flags.split_whitespace().next().is_some_and(is_leaky_flag))
}

/// Whether a `set -` flag group turns on `nounset` or `errexit`.
fn is_leaky_flag(flags: &str) -> bool {
    flags
        .chars()
        .all(|character| character.is_ascii_alphabetic())
        && flags.chars().any(|character| matches!(character, 'u' | 'e'))
}

fn zshrc_text() -> String {
    let path = dotfiles_test_support::repo::root().join(".zshrc");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is unreadable: {error}", path.display()))
}

/// Assertion 1. The positive control.
///
/// The assertion below expects an EMPTY list of offenders, and an empty list
/// is also what a broken alias parser produces. This proves the pipeline
/// found aliases to inspect in the first place, so "no offenders" means
/// "none among the ones examined" rather than "none examined".
#[test]
fn at_least_one_sourcing_alias_is_present() {
    let found = sourcing_aliases(&zshrc_text());
    assert!(
        !found.is_empty(),
        "no alias in .zshrc was parsed as sourcing a .scripts script, so the \
         isolation assertion below would pass over an empty list"
    );
}

/// Assertion 2. No sourced script sets a shell option.
///
/// The concrete failure this prevents is at the top of this file: `set -u`
/// inside `tmux-update-window-names.sh` leaking into the interactive shell
/// and making zsh-autosuggestions print a parameter error before every
/// prompt until the shell is restarted.
#[test]
fn no_sourcing_alias_points_at_a_script_that_sets_shell_options() {
    let root = dotfiles_test_support::repo::root();
    let mut offenders: Vec<String> = Vec::new();

    for alias in sourcing_aliases(&zshrc_text()) {
        let script: PathBuf = root.join(&alias.relative_path);
        let Ok(body) = std::fs::read_to_string(&script) else {
            continue;
        };
        let leaks: Vec<String> = body
            .lines()
            .enumerate()
            .filter(|(_, line)| leaks_a_shell_option(line))
            .map(|(index, line)| format!("{}: {}", index + 1, line.trim()))
            .take(3)
            .collect();
        if !leaks.is_empty() {
            offenders.push(format!(
                "{} -> {}: {leaks:?}",
                alias.name, alias.relative_path
            ));
        }
    }

    assert!(
        offenders.is_empty(),
        "a sourced script sets a shell option that outlives the command, \
         which is how set -u escaped into the interactive shell and made \
         zsh-autosuggestions error before every prompt: {offenders:?}"
    );
}

