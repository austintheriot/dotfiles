use std::fmt;

/// The maximum byte length of a parsed name.
///
/// 64 bytes covers every name in the four conf files with room to spare, and
/// a bound is what keeps a name out of an unbounded allocation when it
/// arrives from a file that reaches this crate without passing pre-commit
/// (spec 3.6).
const MAX_NAME_LEN: usize = 64;

/// The maximum byte length of a parsed documentation URL.
const MAX_URL_LEN: usize = 512;

/// Why a candidate name was refused.
///
/// One variant per rule, matching `PathError`'s shape, so a caller reports
/// the cause rather than "invalid name" and a new rule cannot hide inside an
/// existing variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameError {
    /// Nothing was supplied. An empty name reaches a process spawn or an
    /// `import` statement as a syntax error at the far end rather than as a
    /// rejection here.
    Empty,
    /// The name is longer than this crate accepts. Checked before any
    /// per-character rule, so text arriving from a file that skipped
    /// pre-commit cannot force an unbounded scan.
    TooLong {
        /// The rejected length in bytes, so the caller sees how far over the
        /// bound the input was rather than only that it was over.
        len: usize,
        /// The bound in force. Names and URLs have different bounds, so the
        /// message carries the one that actually applied.
        max: usize,
    },
    /// A control byte appears. These names are reported to a terminal, so a
    /// control byte would become an escape sequence inside the very message
    /// meant to reject it.
    ControlByte,
    /// A separator appears in a command name. Such a name bypasses PATH
    /// lookup entirely and reaches a specific file, which turns a conf-file
    /// edit into a choice of which binary runs.
    PathSeparator,
    /// A command name begins with `-`, so the spawned program's argument
    /// parser reads it as an option rather than as the command.
    LeadingDash,
    /// A module name is not a bare identifier, so placing it after `import`
    /// would either fail to parse or import something other than the name.
    NotAnIdentifier,
    /// A byte outside the printable ASCII range appears. Such a name renders
    /// differently in a report than it resolves on disk.
    NotPrintable,
    /// A documentation URL carries no `http://` or `https://` scheme, so it
    /// is a bare string a browser would resolve against nothing.
    NoScheme,
    /// A version floor is not `MAJOR.MINOR` or `MAJOR.MINOR.PATCH` in
    /// decimal. A floor that parsed loosely would gate on a version nobody
    /// chose, which is worse than refusing the line.
    NotAVersion,
}

impl fmt::Display for NameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // No variant renders the offending input, for the reason
        // PathError's Display states: these errors reach a terminal and a
        // rejected control byte written into the message would run as an
        // escape sequence.
        match self {
            NameError::Empty => write!(formatter, "the name is empty"),
            NameError::TooLong { len, max } => {
                write!(formatter, "the name is {len} bytes, over the {max}-byte limit")
            }
            NameError::ControlByte => write!(formatter, "the name contains a control byte"),
            NameError::PathSeparator => {
                write!(formatter, "the name contains a path separator, which would bypass PATH lookup")
            }
            NameError::LeadingDash => {
                write!(formatter, "the name starts with `-`, which reads as an option")
            }
            NameError::NotAnIdentifier => {
                write!(formatter, "the name is not a bare identifier")
            }
            NameError::NotPrintable => write!(formatter, "the name contains a non-printable byte"),
            NameError::NoScheme => write!(formatter, "the URL has no `http://` or `https://` scheme"),
            NameError::NotAVersion => {
                write!(formatter, "the version floor is not MAJOR.MINOR or MAJOR.MINOR.PATCH")
            }
        }
    }
}

impl std::error::Error for NameError {}

fn reject_common(raw: &str, max: usize) -> Result<(), NameError> {
    if raw.is_empty() {
        return Err(NameError::Empty);
    }
    if raw.len() > max {
        return Err(NameError::TooLong { len: raw.len(), max });
    }
    // Control bytes first: a message about a shape is less useful than one
    // about a byte that would corrupt the message itself.
    if raw.chars().any(|character| character.is_control()) {
        return Err(NameError::ControlByte);
    }
    Ok(())
}

/// The name of a program looked up on PATH.
///
/// # Errors
///
/// Returns `NameError::PathSeparator` for a name containing `/`, because
/// such a name bypasses PATH lookup entirely once it reaches a process
/// spawn, and `NameError::LeadingDash` for a name an argument parser in the
/// spawned program would read as an option.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CommandName(String);

impl CommandName {
    /// # Errors
    ///
    /// See the type's documentation for the rules and their variants.
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        reject_common(raw, MAX_NAME_LEN)?;
        if raw.contains('/') || raw.contains('\\') {
            return Err(NameError::PathSeparator);
        }
        if raw.starts_with('-') {
            return Err(NameError::LeadingDash);
        }
        if raw.chars().any(|character| !character.is_ascii_graphic()) {
            return Err(NameError::NotPrintable);
        }
        Ok(CommandName(raw.to_string()))
    }

    /// The validated name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CommandName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// A Python module name, safe to place after `import`.
///
/// # Errors
///
/// Returns `NameError::NotAnIdentifier` for anything but a bare identifier.
/// A dotted or punctuated value would let manifest text reach an
/// interpreter as code, which is the escape spec 5.2 closes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ModuleName(String);

impl ModuleName {
    /// # Errors
    ///
    /// See the type's documentation for the rules and their variants.
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        reject_common(raw, MAX_NAME_LEN)?;
        let mut characters = raw.chars();
        let starts_well = characters
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == '_');
        let rest_is_well_formed = characters
            .all(|character| character.is_ascii_alphanumeric() || character == '_');
        if !starts_well || !rest_is_well_formed {
            return Err(NameError::NotAnIdentifier);
        }
        Ok(ModuleName(raw.to_string()))
    }

    /// The validated module name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModuleName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// A filename glob pattern, matched inside one already-validated directory.
///
/// # Errors
///
/// Returns `NameError::PathSeparator` for a pattern containing `/`, so the
/// pattern cannot widen the directory the caller chose.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GlobPattern(String);

impl GlobPattern {
    /// # Errors
    ///
    /// See the type's documentation for the rules and their variants.
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        reject_common(raw, MAX_NAME_LEN)?;
        if raw.contains('/') || raw.contains('\\') {
            return Err(NameError::PathSeparator);
        }
        if raw.chars().any(|character| !character.is_ascii_graphic()) {
            return Err(NameError::NotPrintable);
        }
        Ok(GlobPattern(raw.to_string()))
    }

    /// The validated pattern.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GlobPattern {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// A package name passed to a package manager.
///
/// # Errors
///
/// Returns `NameError::LeadingDash` so a package name cannot arrive at a
/// manager as a flag, and `NameError::NotPrintable` for shell-active bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageId(String);

impl PackageId {
    /// # Errors
    ///
    /// See the type's documentation for the rules and their variants.
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        reject_common(raw, MAX_NAME_LEN)?;
        if raw.starts_with('-') {
            return Err(NameError::LeadingDash);
        }
        let allowed = |character: char| {
            character.is_ascii_alphanumeric()
                || matches!(character, '-' | '_' | '.' | '+' | ':' | '@')
        };
        if !raw.chars().all(allowed) {
            return Err(NameError::NotPrintable);
        }
        Ok(PackageId(raw.to_string()))
    }

    /// The validated package name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PackageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// A minimum acceptable version, written `MAJOR.MINOR` or `MAJOR.MINOR.PATCH`.
///
/// Three numeric components rather than a string, because the comparison
/// this exists for is the one a string gets wrong: "0.9" sorts above "0.10"
/// lexically and below it as a version. That is not hypothetical. Neovim
/// 0.9.5 is what Ubuntu 24.04 ships and 0.10 is what the nvim config needs,
/// so a string comparison would report the machine satisfied.
///
/// A missing patch component means zero, so `0.10` and `0.10.0` are the same
/// floor. Pre-release suffixes are rejected rather than ordered: `0.10-dev`
/// has no total order against `0.10` that everyone agrees on, and a manifest
/// is not the place to litigate it.
///
/// # Errors
///
/// Returns `NameError::NotAVersion` for anything that is not two or three
/// decimal components separated by `.`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VersionFloor {
    major: u32,
    minor: u32,
    patch: u32,
}

impl VersionFloor {
    /// # Errors
    ///
    /// See the type's documentation for the rules and their variants.
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        reject_common(raw, MAX_NAME_LEN)?;
        let mut parts = raw.split('.');
        let mut next_component = || -> Result<u32, NameError> {
            parts
                .next()
                .ok_or(NameError::NotAVersion)?
                .parse::<u32>()
                .map_err(|_| NameError::NotAVersion)
        };
        let major = next_component()?;
        let minor = next_component()?;
        let patch = match parts.next() {
            Some(component) => component.parse::<u32>().map_err(|_| NameError::NotAVersion)?,
            None => 0,
        };
        if parts.next().is_some() {
            return Err(NameError::NotAVersion);
        }
        Ok(VersionFloor { major, minor, patch })
    }

    /// The floor's three components, patch defaulted to zero.
    pub fn components(&self) -> (u32, u32, u32) {
        (self.major, self.minor, self.patch)
    }

    /// Whether an observed version is at or above this floor.
    pub fn is_satisfied_by(&self, major: u32, minor: u32, patch: u32) -> bool {
        (major, minor, patch) >= (self.major, self.minor, self.patch)
    }
}

impl fmt::Display for VersionFloor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// A documentation URL, printed for a human and never fetched.
///
/// Scheme-agnostic across `http` and `https` because 21 of the 22 manifest
/// entries are `https://` and `deps-ci.conf:23` is
/// `http://gondor.apana.org.au/~herbert/dash/`. An https-only type cannot
/// parse the manifest this repo ships.
///
/// # Errors
///
/// Returns `NameError::NoScheme` for a value with neither scheme, so a
/// bare word cannot be rendered to a reader as a link.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DocsUrl(String);

impl DocsUrl {
    /// # Errors
    ///
    /// See the type's documentation for the rules and their variants.
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        reject_common(raw, MAX_URL_LEN)?;
        if !raw.starts_with("https://") && !raw.starts_with("http://") {
            return Err(NameError::NoScheme);
        }
        if raw.chars().any(|character| !character.is_ascii_graphic()) {
            return Err(NameError::NotPrintable);
        }
        Ok(DocsUrl(raw.to_string()))
    }

    /// The validated URL.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DocsUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A name containing `/` would bypass PATH lookup entirely if the value
    // ever reached Command::new, which is why the rule is at parse time
    // rather than at use (spec 5.2, "Names are validated at parse time").
    #[test]
    fn command_name_rejects_a_path_separator() {
        assert!(matches!(
            CommandName::parse("../../bin/sh"),
            Err(NameError::PathSeparator)
        ));
    }

    #[test]
    fn command_name_rejects_a_leading_dash() {
        assert!(matches!(CommandName::parse("-rf"), Err(NameError::LeadingDash)));
    }

    #[test]
    fn command_name_rejects_control_bytes() {
        assert!(matches!(
            CommandName::parse("git\u{1b}[2J"),
            Err(NameError::ControlByte)
        ));
    }

    #[test]
    fn command_name_rejects_over_64_bytes() {
        let long_name = "a".repeat(65);
        assert!(matches!(
            CommandName::parse(&long_name),
            Err(NameError::TooLong { len: 65, max: 64 })
        ));
    }

    // Every command name in the four conf files, verified by reading them:
    // git gh alacritty zsh nvim fzf rg zoxide tmux shellcheck cc rustup
    // node aerospace xclip python3.
    #[test]
    fn command_name_accepts_every_real_manifest_command() {
        let real_commands = [
            "git", "gh", "alacritty", "zsh", "nvim", "fzf", "rg", "zoxide",
            "tmux", "shellcheck", "cc", "rustup", "node", "aerospace",
            "xclip", "python3",
        ];
        for command in real_commands {
            assert!(
                CommandName::parse(command).is_ok(),
                "a real manifest command was rejected: {command}"
            );
        }
    }

    // PythonImport's surface is `python3 -c "import <this>"`, so the
    // argument must be an identifier and nothing else (spec 5.2).
    #[test]
    fn module_name_rejects_anything_but_an_identifier() {
        assert!(matches!(
            ModuleName::parse("yaml; import os"),
            Err(NameError::NotAnIdentifier)
        ));
        assert!(matches!(ModuleName::parse("os.path"), Err(NameError::NotAnIdentifier)));
        assert_eq!(
            ModuleName::parse("yaml").expect("a bare module name parses").as_str(),
            "yaml"
        );
    }

    // deps-ci.conf:23 is http://, not https://, so a scheme-agnostic type
    // is required. An HttpsUrl cannot parse the manifest this repo ships.
    #[test]
    fn docs_url_accepts_both_schemes_and_rejects_neither() {
        assert!(DocsUrl::parse("https://git-scm.com/downloads").is_ok());
        assert!(
            DocsUrl::parse("http://gondor.apana.org.au/~herbert/dash/").is_ok(),
            "deps-ci.conf:23 must parse"
        );
        assert!(matches!(
            DocsUrl::parse("git-scm.com/downloads"),
            Err(NameError::NoScheme)
        ));
    }

    #[test]
    fn a_version_floor_parses_two_and_three_component_forms() {
        assert_eq!(
            VersionFloor::parse("0.10").expect("two components parse").components(),
            (0, 10, 0)
        );
        assert_eq!(
            VersionFloor::parse("0.10.4").expect("three components parse").components(),
            (0, 10, 4)
        );
    }

    // The manifest field is written by hand, so a floor that silently
    // parsed to something else would gate on a version nobody chose.
    #[test]
    fn a_version_floor_rejects_what_is_not_a_version() {
        for raw in ["", "0", "v0.10", "0.10.4.1", "0.x", "0.10-dev", "latest"] {
            assert!(
                VersionFloor::parse(raw).is_err(),
                "{raw:?} parsed as a version floor"
            );
        }
    }

    // A floor is compared against a version read out of `<tool> --version`,
    // and comparing those component-wise as numbers is the whole point:
    // "0.9" is greater than "0.10" as a string and lower as a version.
    #[test]
    fn a_version_floor_orders_by_number_not_by_string() {
        let floor = VersionFloor::parse("0.10").expect("the floor parses");
        assert!(!floor.is_satisfied_by(0, 9, 5), "0.9.5 must not satisfy 0.10");
        assert!(floor.is_satisfied_by(0, 10, 0), "0.10.0 satisfies 0.10");
        assert!(floor.is_satisfied_by(0, 12, 4), "0.12.4 satisfies 0.10");
        assert!(floor.is_satisfied_by(1, 0, 0), "1.0.0 satisfies 0.10");
        assert!(!floor.is_satisfied_by(0, 6, 1), "0.6.1 must not satisfy 0.10");
    }

    // An error string reaches a terminal and the input is untrusted, which
    // is the rule rel.rs:29-32 already states for PathError.
    #[test]
    fn name_error_does_not_echo_the_input() {
        let rendered = CommandName::parse("git\u{1b}[2J")
            .expect_err("a control byte is rejected")
            .to_string();
        assert!(
            !rendered.contains('\u{1b}'),
            "the error rendered the escape byte: {rendered:?}"
        );
    }
}
