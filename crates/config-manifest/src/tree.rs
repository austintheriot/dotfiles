use std::collections::BTreeMap;
use std::fmt;

use crate::path::{BlobId, IdError, PathError, RelPath};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileMode {
    Regular,
    Executable,
    Symlink,
}

impl FileMode {
    pub fn as_git_mode(self) -> &'static str {
        match self {
            FileMode::Regular => "100644",
            FileMode::Executable => "100755",
            FileMode::Symlink => "120000",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TreeListing(BTreeMap<RelPath, (FileMode, BlobId)>);

impl TreeListing {
    pub fn get(&self, path: &RelPath) -> Option<&(FileMode, BlobId)> {
        self.0.get(path)
    }

    pub fn paths(&self) -> impl Iterator<Item = &RelPath> {
        self.0.keys()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeError {
    Malformed(String),
    UnsupportedMode(String),
    UnsupportedType(String),
    NotUtf8(Vec<u8>),
    Path(PathError),
    Id(IdError),
}

impl fmt::Display for TreeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TreeError::Malformed(entry) => write!(formatter, "malformed ls-tree entry: {entry}"),
            TreeError::UnsupportedMode(mode) => write!(formatter, "unsupported file mode: {mode}"),
            TreeError::UnsupportedType(kind) => {
                write!(formatter, "unsupported tree entry type: {kind}")
            }
            TreeError::NotUtf8(bytes) => write!(formatter, "ls-tree entry is not UTF-8: {bytes:?}"),
            TreeError::Path(source) => write!(formatter, "ls-tree path: {source}"),
            TreeError::Id(source) => write!(formatter, "ls-tree object id: {source}"),
        }
    }
}

impl std::error::Error for TreeError {}

pub fn parse_ls_tree(bytes: &[u8]) -> Result<TreeListing, TreeError> {
    let mut entries = BTreeMap::new();
    for raw_entry in bytes
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let entry =
            std::str::from_utf8(raw_entry).map_err(|_| TreeError::NotUtf8(raw_entry.to_vec()))?;
        let (meta, path_text) = entry
            .split_once('\t')
            .ok_or_else(|| TreeError::Malformed(entry.to_string()))?;
        let mut fields = meta.split(' ');
        let (Some(mode_text), Some(kind), Some(id_text), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return Err(TreeError::Malformed(entry.to_string()));
        };
        if kind != "blob" {
            return Err(TreeError::UnsupportedType(kind.to_string()));
        }
        let mode = match mode_text {
            "100644" => FileMode::Regular,
            "100755" => FileMode::Executable,
            "120000" => FileMode::Symlink,
            other => return Err(TreeError::UnsupportedMode(other.to_string())),
        };
        let blob = BlobId::parse(id_text).map_err(TreeError::Id)?;
        let path = RelPath::parse(path_text).map_err(TreeError::Path)?;
        entries.insert(path, (mode, blob));
    }
    Ok(TreeListing(entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(raw: &str) -> RelPath {
        RelPath::parse(raw).expect("valid test path")
    }

    // Recorded `git ls-tree -r -z` output: mode, type, id, tab, path, NUL.
    const RECORDED: &[u8] =
        b"100644 blob 4b825dc642cb6eb9a060e54bf8d69288fbee4904\t.sync-manifest\0\
100755 blob e69de29bb2d1d6434b8b29ae775ad8c2e48c5391\ttests/lib.sh\0\
120000 blob 8b137891791fe96927ad78e64b0aad7bded08bdc\t.cfg/hooks/pre-commit\0\
100644 blob 3b18e512dba79e4c8300dd08aeb37f8e728b8dad\tdir/with space.txt\0";

    #[test]
    fn parses_modes_ids_and_paths_with_spaces() {
        let listing = parse_ls_tree(RECORDED).expect("valid listing");
        assert_eq!(listing.paths().count(), 4);
        let (mode, blob) = listing.get(&rel("tests/lib.sh")).expect("present");
        assert_eq!(*mode, FileMode::Executable);
        assert_eq!(blob.as_str(), "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391");
        assert_eq!(
            listing
                .get(&rel(".cfg/hooks/pre-commit"))
                .expect("present")
                .0,
            FileMode::Symlink
        );
        assert!(listing.get(&rel("dir/with space.txt")).is_some());
    }

    #[test]
    fn an_empty_tree_is_an_empty_listing() {
        let listing = parse_ls_tree(b"").expect("valid");
        assert!(listing.is_empty());
    }

    #[test]
    fn rejects_unknown_modes_non_blob_entries_and_malformed_lines() {
        assert_eq!(
            parse_ls_tree(b"160000 commit 4b825dc642cb6eb9a060e54bf8d69288fbee4904\tsub\0"),
            Err(TreeError::UnsupportedType("commit".to_string()))
        );
        assert_eq!(
            parse_ls_tree(b"040000 tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\tdir\0"),
            Err(TreeError::UnsupportedType("tree".to_string()))
        );
        assert_eq!(
            parse_ls_tree(b"100600 blob 4b825dc642cb6eb9a060e54bf8d69288fbee4904\tf\0"),
            Err(TreeError::UnsupportedMode("100600".to_string()))
        );
        assert_eq!(
            parse_ls_tree(b"garbage\0"),
            Err(TreeError::Malformed("garbage".to_string()))
        );
    }

    #[test]
    fn paths_are_returned_in_sorted_order() {
        let listing = parse_ls_tree(RECORDED).expect("valid");
        let paths: Vec<&str> = listing.paths().map(RelPath::as_str).collect();
        assert_eq!(
            paths,
            vec![
                ".cfg/hooks/pre-commit",
                ".sync-manifest",
                "dir/with space.txt",
                "tests/lib.sh"
            ]
        );
    }
}
