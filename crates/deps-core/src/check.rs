use std::collections::BTreeMap;

use dotfiles_path::{CheckRelPath, CommandName, GlobPattern, ModuleName};

/// A root a check path is joined onto.
///
/// A closed sum, which is what deletes all shell expansion from the check
/// field. `deps.conf:26` embeds `$(brew --prefix 2>/dev/null)`, and on a
/// machine with no brew that substitution is empty, so the real test runs
/// against the filesystem root. A root that fails to resolve is
/// `Observation::Unresolvable`, not false.
///
/// `MacApplications` rather than an open `Absolute` variant: the only
/// absolute path in the whole corpus is `/Applications/Alacritty.app`
/// (`deps.conf:24`), and a named variant per need makes each addition a
/// reviewable decision that states its own blast radius.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PathRoot {
    /// The invoking user's home directory, written `$HOME/` in the manifest.
    Home,
    /// `/Applications/`, the macOS bundle directory.
    MacApplications,
    /// Homebrew's prefix, written `$(brew --prefix 2>/dev/null)/`.
    BrewPrefix,
    /// oh-my-zsh's custom directory, written
    /// `${ZSH_CUSTOM:-$HOME/.oh-my-zsh/custom}/`.
    OhMyZshCustom,
}

/// A path to check: a closed root plus a validated relative remainder.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CheckPath {
    /// The root the remainder is joined onto.
    pub root: PathRoot,
    /// The remainder, relative to `root`.
    pub rest: CheckRelPath,
}

impl CheckPath {
    /// Pair a root with a validated remainder.
    pub fn new(root: PathRoot, rest: CheckRelPath) -> Self {
        CheckPath { root, rest }
    }
}

/// A presence check.
///
/// There is no `Shell` variant. All 22 real checks fit these seven, verified
/// by reading the four conf files, and a named variant per need is strictly
/// better than an escape hatch that grants all future blast radius at once.
///
/// `AnyOf` carries `first` and `rest` rather than one `Vec`, because
/// `AnyOf(vec![])` is a well-typed value that evaluates false under any
/// rule, and a manifest entry parsing to it would report a dependency
/// permanently missing with no diagnostic. All three real `AnyOf` entries
/// have exactly two branches.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Check {
    /// `command -v <name>`: the name resolves on `PATH`.
    Command(CommandName),
    /// `[ -d <path> ]`: the path is a directory.
    DirExists(CheckPath),
    /// `[ -f <path> ]`: the path is a file.
    FileExists(CheckPath),
    /// `[ -s <path> ]`: the path is a file with a non-zero size.
    ///
    /// `deps.conf:36` uses `-s`, not `-f`: a truncated `nvm.sh` passes `-f`
    /// and sources to nothing.
    FileNonEmpty(CheckPath),
    /// `ls -d <dir>/<pattern>`: at least one entry in `dir` matches.
    GlobExists {
        /// The directory the pattern is matched inside.
        dir: CheckPath,
        /// The filename pattern, which cannot itself contain a separator.
        pattern: GlobPattern,
    },
    /// `python3 -c "import <module>"`: the interpreter imports the module.
    PythonImport(ModuleName),
    /// At least one branch is present.
    AnyOf {
        /// The first branch, so the alternation cannot be empty.
        first: Box<Check>,
        /// The remaining branches, in the order the manifest wrote them.
        rest: Vec<Check>,
    },
}

/// What was observed about one check.
///
/// Three-state rather than boolean. A report must distinguish "brew is
/// absent so this check is unanswerable" from "the file is missing", because
/// they have different remedies, and a boolean collapses the first into the
/// second silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observation {
    /// The check's subject is there.
    Present,
    /// The check's subject is not there.
    Absent,
    /// The check could not be answered, because `root` did not resolve.
    Unresolvable {
        /// The root that failed to resolve.
        root: PathRoot,
    },
}

/// What the edge observed, injected into the core.
///
/// A trait rather than a concrete map so a test can script a world as a
/// small struct without enumerating every leaf, and so the core never holds
/// the capability that produced the answers. The gather step implements this
/// at the edge; nothing in this crate does.
pub trait Observations {
    /// The observation recorded for `check`.
    fn observe(&self, check: &Check) -> Observation;
}

/// An observation map built from explicit pairs.
///
/// Absence means `Absent`: the gather step runs at the edge over the selected
/// manifest, and a leaf it did not record is a leaf whose subject is not
/// there.
#[derive(Debug, Clone, Default)]
pub struct ObservationMap(BTreeMap<Check, Observation>);

impl ObservationMap {
    /// Build a map from explicit pairs.
    ///
    /// A later pair for the same check wins, which is the `collect` default
    /// and is why the constructor takes a `Vec` rather than a map: the caller
    /// writes the pairs in one place and the last write is visible there.
    pub fn from_pairs(pairs: Vec<(Check, Observation)>) -> Self {
        ObservationMap(pairs.into_iter().collect())
    }
}

impl Observations for ObservationMap {
    fn observe(&self, check: &Check) -> Observation {
        self.0.get(check).copied().unwrap_or(Observation::Absent)
    }
}

/// Evaluate a check against an injected observation.
///
/// Pure: the observation comes in as an argument, so this function opens no
/// file and spawns no process. `AnyOf` short-circuits on the first
/// `Present`, and otherwise prefers a reported `Unresolvable` over `Absent`,
/// because an unanswerable branch collapsed to `Absent` is exactly the
/// silent-false bug `deps.conf:26` has today.
pub fn evaluate(check: &Check, observed: &impl Observations) -> Observation {
    let Check::AnyOf { first, rest } = check else {
        return observed.observe(check);
    };

    let mut unresolved_root = None;
    for branch in std::iter::once(first.as_ref()).chain(rest.iter()) {
        match evaluate(branch, observed) {
            Observation::Present => return Observation::Present,
            Observation::Unresolvable { root } => {
                unresolved_root = unresolved_root.or(Some(root));
            }
            Observation::Absent => {}
        }
    }
    match unresolved_root {
        Some(root) => Observation::Unresolvable { root },
        None => Observation::Absent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotfiles_path::{CheckRelPath, CommandName};

    fn command(name: &str) -> Check {
        Check::Command(CommandName::parse(name).expect("a test command name parses"))
    }

    fn home_file(rest: &str) -> CheckPath {
        CheckPath::new(
            PathRoot::Home,
            CheckRelPath::parse(rest).expect("a test path parses"),
        )
    }

    fn brew_file(rest: &str) -> CheckPath {
        CheckPath::new(
            PathRoot::BrewPrefix,
            CheckRelPath::parse(rest).expect("a test path parses"),
        )
    }

    // deps.conf:26, the real zsh-autosuggestions check, as the two-branch
    // AnyOf it decomposes to.
    fn real_zsh_autosuggestions_check() -> Check {
        Check::AnyOf {
            first: Box::new(Check::FileExists(home_file(
                ".oh-my-zsh/custom/plugins/zsh-autosuggestions/zsh-autosuggestions.zsh",
            ))),
            rest: vec![Check::FileExists(brew_file(
                "share/zsh-autosuggestions/zsh-autosuggestions.zsh",
            ))],
        }
    }

    #[test]
    fn any_of_is_present_when_the_first_branch_is_present() {
        let check = real_zsh_autosuggestions_check();
        let Check::AnyOf { first, .. } = &check else {
            panic!("the fixture is an AnyOf");
        };
        let observed =
            ObservationMap::from_pairs(vec![((**first).clone(), Observation::Present)]);
        assert_eq!(evaluate(&check, &observed), Observation::Present);
    }

    #[test]
    fn any_of_is_present_when_a_later_branch_is_present() {
        let check = real_zsh_autosuggestions_check();
        let Check::AnyOf { first, rest } = &check else {
            panic!("the fixture is an AnyOf");
        };
        let observed = ObservationMap::from_pairs(vec![
            ((**first).clone(), Observation::Absent),
            (rest[0].clone(), Observation::Present),
        ]);
        assert_eq!(evaluate(&check, &observed), Observation::Present);
    }

    // The bug spec 5.2 fixes. On a machine with no brew the shell
    // substitution at deps.conf:26 is empty, so the second operand tests
    // /share/... at the filesystem root. Present is the wrong answer and
    // Absent is also wrong: the branch is unanswerable, and the remedy for
    // "brew is missing" differs from the remedy for "the file is missing".
    #[test]
    fn any_of_reports_unresolvable_when_no_branch_is_present_and_one_root_is_unresolvable() {
        let check = real_zsh_autosuggestions_check();
        let Check::AnyOf { first, rest } = &check else {
            panic!("the fixture is an AnyOf");
        };
        let observed = ObservationMap::from_pairs(vec![
            ((**first).clone(), Observation::Absent),
            (
                rest[0].clone(),
                Observation::Unresolvable { root: PathRoot::BrewPrefix },
            ),
        ]);
        assert_eq!(
            evaluate(&check, &observed),
            Observation::Unresolvable { root: PathRoot::BrewPrefix },
            "an unanswerable branch must not collapse to Absent"
        );
    }

    #[test]
    fn any_of_is_absent_only_when_every_branch_is_absent() {
        let check = real_zsh_autosuggestions_check();
        let Check::AnyOf { first, rest } = &check else {
            panic!("the fixture is an AnyOf");
        };
        let observed = ObservationMap::from_pairs(vec![
            ((**first).clone(), Observation::Absent),
            (rest[0].clone(), Observation::Absent),
        ]);
        assert_eq!(evaluate(&check, &observed), Observation::Absent);
    }

    // A leaf nobody observed is Absent, not a panic. gather runs at the
    // edge and a leaf it skipped is a leaf whose subject is not there.
    #[test]
    fn an_unobserved_leaf_is_absent() {
        let observed = ObservationMap::from_pairs(vec![]);
        assert_eq!(evaluate(&command("git"), &observed), Observation::Absent);
    }

    // deps.conf:36 uses -s, not -f. A truncated nvm.sh passes -f and
    // sources to nothing, so collapsing the two variants would introduce a
    // bug during the port.
    #[test]
    fn file_non_empty_is_a_distinct_check_from_file_exists() {
        let path = home_file(".nvm/nvm.sh");
        let non_empty = Check::FileNonEmpty(path.clone());
        let exists = Check::FileExists(path);
        assert_ne!(non_empty, exists);
        let observed = ObservationMap::from_pairs(vec![(exists, Observation::Present)]);
        assert_eq!(
            evaluate(&non_empty, &observed),
            Observation::Absent,
            "a satisfied -f must not satisfy a -s"
        );
    }
}
