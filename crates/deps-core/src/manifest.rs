use std::collections::BTreeSet;

use dotfiles_path::{CommandName, DocsUrl, NameError};

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

/// A presence check.
///
/// Task 8 extends this to the full seven-variant enum of spec 5.2. The two
/// variants here are what Task 7's manifest tests exercise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {
    Command(CommandName),
    DirExists(dotfiles_path::CheckRelPath),
}

/// Why a check expression was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckParseError {
    Unrecognized,
    BadCommandName(NameError),
    BadPath(dotfiles_path::PathError),
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
    PlatformSelected,
    ExplicitOnly,
}

/// One manifest entry.
///
/// `docs` lives here rather than on `NoInstallReason`, per spec 5.1: the URL
/// already lives in the manifest and a second home is a drift shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    pub name: DependencyName,
    pub check: Check,
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
    WrongFieldCount { line: usize, found: usize },
    BadName { line: usize, cause: NameError },
    BadCheck { line: usize, cause: CheckParseError },
    BadDocs { line: usize, cause: NameError },
    DuplicateName { line: usize, name: DependencyName },
    InterpreterCheckInPlatformConf { line: usize },
}

/// Parse manifest text into typed entries.
///
/// Takes `&str`, never a path: the caller reads the file, so this function
/// makes no syscall and the module holds no capability. `parse_manifest` is
/// the boundary spec 3.6 requires, replacing the `sh -c "$check"` at
/// `check-deps.sh:524` with a closed enum that has no shell escape hatch.
///
/// # Errors
///
/// Returns `ParseError` naming the 1-based line and the rule it broke. A
/// line with more than three pipe-separated fields is
/// `WrongFieldCount` rather than a silent truncation, which is the failure
/// `deps.conf:9-13` documents and cannot detect.
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

/// Recognize a check expression.
///
/// Task 8 replaces this body with the full grammar of spec 5.2. It is
/// written here only far enough to parse the two shapes Task 7's tests use,
/// and `kind` is threaded through now so Task 8's `PythonImport` rule has
/// the argument it needs without a signature change.
fn parse_check(raw: &str, kind: ConfKind, line: usize) -> Result<Check, ParseError> {
    let _ = kind;
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix("command -v ") {
        let name = CommandName::parse(rest.trim()).map_err(|cause| ParseError::BadCheck {
            line,
            cause: CheckParseError::BadCommandName(cause),
        })?;
        return Ok(Check::Command(name));
    }
    if let Some(rest) = home_dir_test(trimmed) {
        let path = dotfiles_path::CheckRelPath::parse(rest).map_err(|cause| {
            ParseError::BadCheck { line, cause: CheckParseError::BadPath(cause) }
        })?;
        return Ok(Check::DirExists(path));
    }
    Err(ParseError::BadCheck { line, cause: CheckParseError::Unrecognized })
}

/// Extract the `$HOME`-relative path from `[ -d "$HOME/<rest>" ]`.
fn home_dir_test(raw: &str) -> Option<&str> {
    raw.strip_prefix("[ -d \"$HOME/")?.strip_suffix("\" ]")
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
