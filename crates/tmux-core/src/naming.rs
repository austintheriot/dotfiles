//! Window-name computation, ported from `tmux-update-window-names.sh`.
//!
//! The precedence, lowest to highest, is the script's own header:
//!
//! 1. basename of the active pane's working directory, `~` for `$HOME`
//! 2. `repo/branch` when that directory is a git repository, where repo is
//!    the main repository's name even inside a linked worktree, and branch
//!    is a short commit sha in parentheses when HEAD is detached
//! 3. a name the user set with `prefix ,`
//!
//! `@wname_label` prefixes the automatic part, and `@wname_bare_repos`
//! drops the `repo/` prefix for repositories matching one of its glob
//! patterns.

/// The facts one tmux window contributes to its own name.
///
/// The caller (the `tmux-tools` binary) gathers these from tmux and git,
/// since reading either is IO and this crate performs none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowFacts {
    /// The basename of the active pane's current working directory.
    ///
    /// Used only when the directory is neither `$HOME` nor a repository.
    pub directory_basename: String,

    /// Whether the active pane's current working directory is `$HOME`.
    ///
    /// Outranks `repository`: `$HOME` can itself be a git repository (a
    /// bare-repo worktree, for one), and the tilde must still win so the
    /// window is not named after that repository.
    pub is_home: bool,

    /// The repository facts, when the directory is inside a git repository.
    ///
    /// `None` when the directory is not a repository, or when git could not
    /// determine a revision for it (an empty repository with no commits and
    /// no branch, in the script's own words).
    pub repository: Option<RepositoryFacts>,

    /// A name the user set with `prefix ,`, if tmux still owns the window.
    ///
    /// `Some(String::new())` means the user renamed the window to an empty
    /// string, which the script treats as "drop back to automatic naming"
    /// rather than as a manual name of `""`.
    pub manual_name: Option<String>,

    /// The `@wname_label` window option, if the user set one.
    ///
    /// Prefixed onto the automatic part as `"<label> - <automatic>"` so
    /// `select-window -t <label>` keybindings keep working while the
    /// branch behind the label still shows.
    pub label: Option<String>,
}

/// The repository facts for a window whose directory is a git repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryFacts {
    /// The name of the main repository, even inside a linked worktree.
    ///
    /// Derived from the common git directory rather than the worktree
    /// directory, because a linked worktree's own directory is usually
    /// named after the task rather than the project.
    pub main_repo_name: String,

    /// The state of HEAD: on a branch, or detached at a specific commit.
    pub head: HeadState,
}

/// The state of a repository's HEAD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadState {
    /// HEAD points at a branch, named here.
    Branch(String),

    /// HEAD is detached, at the commit whose short sha is given.
    Detached {
        /// The short form of the detached commit's sha, as `git rev-parse
        /// --short HEAD` prints it.
        short_sha: String,
    },
}

/// Computes the name a window should have, given its facts.
///
/// `bare_repo_patterns` is the `@wname_bare_repos` option already split on
/// `|` by the caller: splitting the raw option string is the caller's job,
/// because reading a tmux option is IO. A repository whose name matches any
/// pattern is named by branch alone, without the `repo/` prefix.
#[must_use]
pub fn window_name(facts: &WindowFacts, bare_repo_patterns: &[String]) -> String {
    let automatic = automatic_name(facts, bare_repo_patterns);
    let automatic = match &facts.label {
        Some(label) => format!("{label} - {automatic}"),
        None => automatic,
    };

    match &facts.manual_name {
        Some(manual) if !manual.is_empty() => manual.clone(),
        _ => automatic,
    }
}

/// The automatic part of the name: repo and branch, the directory basename,
/// or the tilde, before any label prefix or manual override is applied.
fn automatic_name(facts: &WindowFacts, bare_repo_patterns: &[String]) -> String {
    // is_home outranks repository. The home directory can itself be a
    // bare-repo worktree with no .git file for the shell script to find,
    // so a Rust version that inspects repositories properly would start
    // naming $HOME after that repository unless the tilde is checked first.
    if facts.is_home {
        return "~".to_string();
    }

    match &facts.repository {
        Some(repository) => repository_name(repository, bare_repo_patterns),
        None => facts.directory_basename.clone(),
    }
}

/// Formats a repository's contribution to the automatic name: `repo/branch`,
/// or just the branch when the repository matches a bare-repo pattern.
fn repository_name(repository: &RepositoryFacts, bare_repo_patterns: &[String]) -> String {
    let revision = match &repository.head {
        HeadState::Branch(branch) => branch.clone(),
        HeadState::Detached { short_sha } => format!("({short_sha})"),
    };

    let matches_bare_pattern = bare_repo_patterns
        .iter()
        .any(|pattern| matches_glob(&repository.main_repo_name, pattern));

    if matches_bare_pattern {
        revision
    } else {
        format!("{}/{revision}", repository.main_repo_name)
    }
}

/// Reports whether `subject` matches `pattern`, where `pattern` may use `*`
/// (any run of characters, including none) and `?` (exactly one character).
///
/// Hand-rolled rather than a dependency: the patterns come from a tmux
/// option the user writes, and the syntax needed is small.
fn matches_glob(subject: &str, pattern: &str) -> bool {
    let subject_chars: Vec<char> = subject.chars().collect();
    let pattern_chars: Vec<char> = pattern.chars().collect();
    matches_glob_from(&subject_chars, &pattern_chars)
}

/// The recursive walk behind [`matches_glob`], operating on character
/// slices so a `*` match can recurse without re-slicing a `str`.
fn matches_glob_from(subject: &[char], pattern: &[char]) -> bool {
    match pattern.first() {
        None => subject.is_empty(),
        Some('*') => {
            matches_glob_from(subject, &pattern[1..])
                || (!subject.is_empty() && matches_glob_from(&subject[1..], pattern))
        }
        Some('?') => !subject.is_empty() && matches_glob_from(&subject[1..], &pattern[1..]),
        Some(literal) => {
            subject.first() == Some(literal) && matches_glob_from(&subject[1..], &pattern[1..])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_repo_window() -> WindowFacts {
        WindowFacts {
            directory_basename: "crates".to_string(),
            is_home: false,
            repository: Some(RepositoryFacts {
                main_repo_name: "dotfiles".to_string(),
                head: HeadState::Branch("main".to_string()),
            }),
            manual_name: None,
            label: None,
        }
    }

    /// A repository window is named "repo/branch", which is precedence
    /// level 2 from the script's own header.
    #[test]
    fn a_repository_window_is_named_repo_slash_branch() {
        let name = window_name(&a_repo_window(), &[]);

        // Positive control: the name must not be empty, or the equality
        // below would hold for a function that returns nothing at all.
        assert!(!name.is_empty(), "a window must get a name");
        assert_eq!(name, "dotfiles/main");
    }

    /// A manual name set with `prefix ,` outranks everything, which is
    /// precedence level 3.
    #[test]
    fn a_manual_name_outranks_the_repository() {
        let mut facts = a_repo_window();
        facts.manual_name = Some("Reviews".to_string());

        assert_eq!(window_name(&facts, &[]), "Reviews");
    }

    /// An empty manual name drops back to automatic naming, which the
    /// script's header states explicitly.
    #[test]
    fn an_empty_manual_name_falls_back_to_automatic() {
        let mut facts = a_repo_window();
        facts.manual_name = Some(String::new());

        assert_eq!(window_name(&facts, &[]), "dotfiles/main");
    }

    /// A directory that is not a repository is named by basename, which is
    /// precedence level 1.
    #[test]
    fn a_plain_directory_is_named_by_basename() {
        let facts = WindowFacts {
            directory_basename: "Downloads".to_string(),
            is_home: false,
            repository: None,
            manual_name: None,
            label: None,
        };

        assert_eq!(window_name(&facts, &[]), "Downloads");
    }

    /// $HOME renders as "~".
    ///
    /// The existing shell suite asserts this, and the spec warns the
    /// assertion currently passes for the wrong reason: /Users/austin is
    /// itself a bare-repo worktree with no .git, so the old and new
    /// implementations skip the repository branch identically. This test
    /// pins the tilde on its own facts rather than on that coincidence.
    #[test]
    fn home_renders_as_a_tilde() {
        let facts = WindowFacts {
            directory_basename: "austin".to_string(),
            is_home: true,
            repository: None,
            manual_name: None,
            label: None,
        };

        assert_eq!(window_name(&facts, &[]), "~");
    }

    /// A bare-repo worktree at $HOME must still render as "~", even though
    /// it IS a repository.
    ///
    /// This is the case the spec says a converted implementation needs of
    /// its own: the shell version skips it only because it looks for `.git`
    /// and a bare-repo worktree has none, so a Rust version that inspects
    /// repositories more thoroughly would start naming $HOME "dotfiles/main"
    /// and break the assertion for a new reason.
    #[test]
    fn a_bare_repo_worktree_at_home_still_renders_as_a_tilde() {
        let facts = WindowFacts {
            directory_basename: "austin".to_string(),
            is_home: true,
            repository: Some(RepositoryFacts {
                main_repo_name: "dotfiles".to_string(),
                head: HeadState::Branch("main".to_string()),
            }),
            manual_name: None,
            label: None,
        };

        assert_eq!(
            window_name(&facts, &[]),
            "~",
            "is_home outranks the repository, or $HOME gets named after the dotfiles repo"
        );
    }

    /// A detached HEAD shows a short sha in parentheses.
    #[test]
    fn a_detached_head_shows_a_short_sha_in_parentheses() {
        let mut facts = a_repo_window();
        facts.repository = Some(RepositoryFacts {
            main_repo_name: "dotfiles".to_string(),
            head: HeadState::Detached {
                short_sha: "16c874a5".to_string(),
            },
        });

        assert_eq!(window_name(&facts, &[]), "dotfiles/(16c874a5)");
    }

    /// A repository matching @wname_bare_repos is named by branch alone.
    #[test]
    fn a_bare_repo_pattern_drops_the_repo_prefix() {
        let patterns = vec!["dotfiles".to_string()];

        assert_eq!(window_name(&a_repo_window(), &patterns), "main");
    }

    /// The glob list is glob patterns, not literals.
    #[test]
    fn a_bare_repo_pattern_matches_as_a_glob() {
        let patterns = vec!["dot*".to_string()];

        // Positive control: a pattern that cannot match must leave the
        // prefix on, or this test would pass for a function that always
        // drops it.
        assert_eq!(
            window_name(&a_repo_window(), &["nomatch*".to_string()]),
            "dotfiles/main"
        );
        assert_eq!(window_name(&a_repo_window(), &patterns), "main");
    }

    /// A label keeps a stable prefix in front of the automatic part, so
    /// `select-window -t <label>` keybindings keep working.
    #[test]
    fn a_label_prefixes_the_automatic_name() {
        let mut facts = a_repo_window();
        facts.label = Some("Reviews".to_string());

        assert_eq!(window_name(&facts, &[]), "Reviews - dotfiles/main");
    }
}
