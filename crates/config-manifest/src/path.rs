use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelPath(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    Empty,
    Absolute(String),
    ParentTraversal(String),
}

impl fmt::Display for PathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathError::Empty => write!(formatter, "empty path"),
            PathError::Absolute(raw) => write!(formatter, "absolute path not allowed: {raw}"),
            PathError::ParentTraversal(raw) => write!(formatter, "'..' segment not allowed: {raw}"),
        }
    }
}

impl std::error::Error for PathError {}

impl RelPath {
    pub fn parse(raw: &str) -> Result<Self, PathError> {
        if raw.is_empty() {
            return Err(PathError::Empty);
        }
        if raw.starts_with('/') {
            return Err(PathError::Absolute(raw.to_string()));
        }
        if raw.split('/').any(|segment| segment == "..") {
            return Err(PathError::ParentTraversal(raw.to_string()));
        }
        Ok(RelPath(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdError {
    Length(usize),
    NonHex(String),
}

impl fmt::Display for IdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdError::Length(len) => write!(
                formatter,
                "object id has {len} characters, expected 40 or 64"
            ),
            IdError::NonHex(raw) => write!(formatter, "object id is not lowercase hex: {raw}"),
        }
    }
}

impl std::error::Error for IdError {}

fn parse_object_id(raw: &str) -> Result<String, IdError> {
    let len = raw.len();
    if len != 40 && len != 64 {
        return Err(IdError::Length(len));
    }
    let is_lower_hex = raw
        .chars()
        .all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch));
    if !is_lower_hex {
        return Err(IdError::NonHex(raw.to_string()));
    }
    Ok(raw.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlobId(String);

impl BlobId {
    pub fn parse(raw: &str) -> Result<Self, IdError> {
        parse_object_id(raw).map(BlobId)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommitId(String);

impl CommitId {
    pub fn parse(raw: &str) -> Result<Self, IdError> {
        parse_object_id(raw).map(CommitId)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rel_path_accepts_nested_paths_with_spaces_and_unicode() {
        let path = RelPath::parse("dir/sub dir/fïle.txt").expect("valid");
        assert_eq!(path.as_str(), "dir/sub dir/fïle.txt");
    }

    #[test]
    fn rel_path_rejects_empty_absolute_and_parent_traversal() {
        assert_eq!(RelPath::parse(""), Err(PathError::Empty));
        assert_eq!(
            RelPath::parse("/etc/passwd"),
            Err(PathError::Absolute("/etc/passwd".to_string()))
        );
        assert_eq!(
            RelPath::parse("a/../b"),
            Err(PathError::ParentTraversal("a/../b".to_string()))
        );
        assert_eq!(
            RelPath::parse(".."),
            Err(PathError::ParentTraversal("..".to_string()))
        );
    }

    #[test]
    fn rel_path_allows_a_single_dot_segment_and_dotfiles() {
        assert!(RelPath::parse(".zshrc").is_ok());
        assert!(RelPath::parse("a/./b").is_ok());
    }

    #[test]
    fn ids_accept_40_and_64_lowercase_hex_only() {
        let sha1 = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
        let sha256 = "a".repeat(64);
        assert!(BlobId::parse(sha1).is_ok());
        assert!(BlobId::parse(&sha256).is_ok());
        assert!(CommitId::parse(sha1).is_ok());
        assert_eq!(BlobId::parse("abc"), Err(IdError::Length(3)));
        assert_eq!(
            BlobId::parse(&"G".repeat(40)),
            Err(IdError::NonHex("G".repeat(40)))
        );
        assert_eq!(
            CommitId::parse(&"A".repeat(40)),
            Err(IdError::NonHex("A".repeat(40)))
        );
    }

    #[test]
    fn rel_paths_order_by_string() {
        let first = RelPath::parse("a").expect("valid");
        let second = RelPath::parse("b").expect("valid");
        assert!(first < second);
    }
}
