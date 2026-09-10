//! The ZLE widget registration in `.scripts/zsh-git-widgets.sh`.
//!
//! The file registers a ZLE widget and key bindings at shell-init time.
//!
//! **What is asserted here, and why it is not the widget's behaviour.** A
//! ZLE widget runs in-process inside the line editor: driving
//! `fzf-git-branch-widget` needs an interactive zsh AND a human at an fzf
//! picker, which no test can supply. So these check the REGISTRATION
//! contract, which is what actually breaks: a syntax error or a renamed
//! widget silently costing you Ctrl+G.
//!
//! That is not a weakened assertion, because the widget's logic is not in
//! the widget. The listing and formatting stages (the old `git | rg | sed |
//! sed` pipeline) moved into `tmux-tools list-branches`, and
//! `tmux_core::format_branches` is unit-tested directly. What remains in the
//! widget is the `LBUFFER` assignment, which exists there precisely because
//! assigning into the zsh line editor is impossible from another process.
//! The shell suite this replaces drew the same line and stated it in its own
//! header.

use std::path::PathBuf;

fn widget_script() -> PathBuf {
    dotfiles_test_support::repo::root().join(".scripts/zsh-git-widgets.sh")
}

/// A single-quoted shell word, so a path with a space cannot split.
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// The value a probe script printed for `name`, from `KEY=value` lines.
fn field(text: &str, name: &str) -> String {
    text.lines()
        .find_map(|line| line.strip_prefix(&format!("{name}=")))
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Assertions 1 through 6. The widget is defined once and bound in all three
/// keymaps.
///
/// ZLE exists only in an interactive zsh, and starting one costs seconds
/// because `.zshrc` loads pyenv, zoxide and fzf. So the whole registration
/// state is collected in ONE interactive shell and every assertion reads
/// that single report, rather than paying a startup per assertion.
///
/// All three keymaps are checked because binding only `emacs` is the shape
/// that works for the author and silently costs Ctrl+G to anyone in vi mode,
/// and this config selects the vi keymap at startup.
#[test]
fn the_branch_widget_is_registered_and_bound_in_every_keymap() {
    if !dotfiles_test_support::zsh::available() {
        dotfiles_test_support::skip("no zsh here, so ZLE registration cannot be observed");
        return;
    }
    let script = widget_script();
    assert!(script.is_file(), "{} is not a file", script.display());

    let output = dotfiles_test_support::zsh::run(&format!(
        r#"
        source {script} 2>/dev/null
        print -r -- "sourced=$?"
        print -r -- "widgets=$(zle -l | grep -c '^fzf-git-branch-widget$')"
        for keymap in emacs vicmd viins; do
            print -r -- "bind-$keymap=$(bindkey -M $keymap '^G')"
        done
        "#,
        script = shell_quote(&script.to_string_lossy()),
    ));
    let report = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        field(&report, "sourced"),
        "0",
        "sourcing the widget file failed, which is the syntax-error case that \
         silently costs Ctrl+G; report was {report:?}"
    );

    // Counted rather than merely matched, so that zero and one are
    // distinguished: a `grep -c` of zero is what a missing `zle -N` produces
    // while every bindkey still reports the widget name, which is the
    // confusing half of that failure.
    //
    // The count does NOT catch a duplicate registration, and it cannot:
    // `zle -N` on an already-registered widget replaces it, so registering
    // twice still lists once. Verified rather than assumed.
    assert_eq!(
        field(&report, "widgets"),
        "1",
        "the file did not register exactly one fzf-git-branch-widget with \
         ZLE; report was {report:?}"
    );

    for keymap in ["emacs", "vicmd", "viins"] {
        let binding = field(&report, &format!("bind-{keymap}"));
        assert!(
            binding.contains("fzf-git-branch-widget"),
            "Ctrl+G is not bound to the branch widget in the {keymap} keymap, \
             so the binding is missing for anyone using it; got {binding:?}"
        );
    }
}

/// Assertions 7 through 9. The tools the widget shells out to are present.
///
/// The widget's body runs `tmux-tools list-branches | fzf`, and
/// `list-branches` replaced a `git | rg | sed | sed` pipeline. A missing one
/// of these is a Ctrl+G that appears to do nothing, which is
/// indistinguishable at the keyboard from an unregistered widget, so the
/// two causes are separated here.
#[test]
fn the_tools_the_widget_shells_out_to_are_installed() {
    if !dotfiles_test_support::zsh::available() {
        dotfiles_test_support::skip("no zsh here, so tool resolution cannot be observed");
        return;
    }
    let output = dotfiles_test_support::zsh::run(
        r#"
        for tool in git rg fzf; do
            print -r -- "$tool=$(command -v $tool >/dev/null 2>&1 && print yes || print no)"
        done
        "#,
    );
    let report = String::from_utf8_lossy(&output.stdout);

    let missing: Vec<&str> = ["git", "rg", "fzf"]
        .into_iter()
        .filter(|tool| field(&report, tool) != "yes")
        .collect();
    assert!(
        missing.is_empty(),
        "the branch widget shells out to tools that are not installed, so \
         Ctrl+G would appear to do nothing: {missing:?}"
    );
}
