use std::fmt;

/// The maximum byte length of a parsed path.
///
/// PATH_MAX is 1024 on darwin and 4096 on Linux. 4096 is the smaller
/// surprise: a path this crate accepts must be usable on both, and a value
/// above the platform limit fails at the syscall instead of at the parse.
const MAX_LEN: usize = 4096;

/// Why a candidate path was refused.
///
/// One variant per rule so a caller can report the cause rather than
/// "invalid path", and so a new rule cannot be folded into an existing
/// variant without a diff that names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    Empty,
    Absolute,
    ParentTraversal,
    Backslash,
    ControlByte,
    HomePrefix,
    DrivePrefix,
    TooLong { len: usize, max: usize },
}

impl fmt::Display for PathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // No variant renders the offending input. These errors reach a
        // terminal, and the input is untrusted: a rejected control byte
        // written into the message would run as an escape sequence, which is
        // the reason ControlByte exists in the first place.
        match self {
            PathError::Empty => write!(formatter, "the path is empty"),
            PathError::Absolute => {
                write!(formatter, "the path is absolute, and a root is supplied separately")
            }
            PathError::ParentTraversal => {
                write!(formatter, "the path contains a `..` segment")
            }
            PathError::Backslash => write!(formatter, "the path contains a backslash"),
            PathError::ControlByte => {
                write!(formatter, "the path contains a control byte")
            }
            PathError::HomePrefix => {
                write!(formatter, "the path starts with `~`, which is not expanded here")
            }
            PathError::DrivePrefix => {
                write!(formatter, "the path starts with a drive letter")
            }
            PathError::TooLong { len, max } => {
                write!(formatter, "the path is {len} bytes, over the {max}-byte limit")
            }
        }
    }
}

impl std::error::Error for PathError {}

/// A relative path safe to join onto a `PathRoot`.
///
/// Stricter than `config_manifest::RelPath`, which rejects only empty,
/// absolute, and `..` segments. The extra rules exist because this type is
/// joined onto a root resolved at runtime, so the escape shapes a
/// worktree-relative path never sees are reachable here.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CheckRelPath(String);

impl CheckRelPath {
    pub fn parse(raw: &str) -> Result<Self, PathError> {
        if raw.is_empty() {
            return Err(PathError::Empty);
        }
        if raw.len() > MAX_LEN {
            return Err(PathError::TooLong { len: raw.len(), max: MAX_LEN });
        }
        // Control bytes first: every later rule reports a shape, and a
        // message about a shape is less useful than one about a byte that
        // would corrupt the message itself.
        if raw.chars().any(|character| character.is_control()) {
            return Err(PathError::ControlByte);
        }
        if raw.starts_with('/') {
            return Err(PathError::Absolute);
        }
        if raw.starts_with('~') {
            return Err(PathError::HomePrefix);
        }
        // A drive prefix is absolute without a leading slash, so the
        // Absolute check above does not catch it.
        let mut characters = raw.chars();
        if let (Some(first), Some(':')) = (characters.next(), characters.next())
            && first.is_ascii_alphabetic()
        {
            return Err(PathError::DrivePrefix);
        }
        if raw.contains('\\') {
            return Err(PathError::Backslash);
        }
        if raw.split('/').any(|segment| segment == "..") {
            return Err(PathError::ParentTraversal);
        }
        Ok(CheckRelPath(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CheckRelPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_plain_relative_path() {
        let parsed = CheckRelPath::parse("share/zsh-autosuggestions/x.zsh")
            .expect("a plain relative path parses");
        assert_eq!(parsed.as_str(), "share/zsh-autosuggestions/x.zsh");
    }

    #[test]
    fn rejects_empty() {
        assert!(matches!(CheckRelPath::parse(""), Err(PathError::Empty)));
    }

    #[test]
    fn rejects_absolute() {
        assert!(matches!(
            CheckRelPath::parse("/Applications/Alacritty.app"),
            Err(PathError::Absolute)
        ));
    }

    #[test]
    fn rejects_parent_traversal() {
        assert!(matches!(
            CheckRelPath::parse("a/../../etc/passwd"),
            Err(PathError::ParentTraversal)
        ));
    }

    // The four rejections RelPath::parse does NOT make. Each is a real
    // shape once the path is composed with a variable root: a backslash
    // traverses on a Windows-ish target, a control byte can rewrite a
    // terminal when the path is rendered in an error, `~` is expanded by a
    // shell but not by Rust so it would be taken literally, and a drive
    // prefix is absolute without a leading slash.
    #[test]
    fn rejects_backslash() {
        assert!(matches!(
            CheckRelPath::parse("a\\..\\..\\etc"),
            Err(PathError::Backslash)
        ));
    }

    #[test]
    fn rejects_control_bytes() {
        assert!(matches!(
            CheckRelPath::parse("a/\u{1b}[2Jb"),
            Err(PathError::ControlByte)
        ));
        assert!(matches!(
            CheckRelPath::parse("a/\u{0}b"),
            Err(PathError::ControlByte)
        ));
    }

    #[test]
    fn rejects_home_prefix() {
        assert!(matches!(
            CheckRelPath::parse("~/.nvm/nvm.sh"),
            Err(PathError::HomePrefix)
        ));
    }

    #[test]
    fn rejects_drive_prefix() {
        assert!(matches!(
            CheckRelPath::parse("C:\\Users"),
            Err(PathError::DrivePrefix)
        ));
    }

    #[test]
    fn rejects_over_length() {
        let long_path = "a/".repeat(2049);
        assert!(matches!(
            CheckRelPath::parse(&long_path),
            Err(PathError::TooLong { .. })
        ));
    }

    // A single-dot segment is ACCEPTED, matching RelPath's existing tests, so
    // the two types do not disagree about a shape that appears in neither
    // manifest.
    #[test]
    fn accepts_single_dot_segment() {
        assert!(CheckRelPath::parse("a/./b").is_ok());
    }

    // Non-ASCII is accepted: the conf files are UTF-8 and a path with an
    // accent is not a traversal.
    #[test]
    fn accepts_non_ascii() {
        assert!(CheckRelPath::parse("dir/fïle.txt").is_ok());
    }

    // The error must render without the raw bytes, because an error string
    // reaches a terminal and the input is untrusted.
    #[test]
    fn control_byte_error_does_not_echo_the_input() {
        let rendered = CheckRelPath::parse("a/\u{1b}[2Jb")
            .expect_err("a control byte is rejected")
            .to_string();
        assert!(
            !rendered.contains('\u{1b}'),
            "the error rendered the escape byte: {rendered:?}"
        );
    }
}
