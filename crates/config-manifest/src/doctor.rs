//! Binary staleness diagnosis.
//!
//! Pure by construction. `diagnose` compares two caller-supplied maps and
//! `render` returns a value; neither spawns a process nor reads the
//! filesystem. Gathering the stamps is `git.rs`'s job, which is what lets
//! "one crate stale, one current, one orphaned" be a table test.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::path::{IdError, TreeId};

/// A workspace member's name.
///
/// Smart-constructed because the name becomes a path component and the
/// basename of a binary, and it is read out of a manifest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CrateName(String);

/// Why a string was rejected as a crate name.
#[derive(Debug, PartialEq, Eq)]
pub enum NameError {
    /// The manifest listed a member with nothing between the quotes, which
    /// would resolve to the workspace directory itself rather than a crate.
    Empty,
    /// The name carries a character that cannot appear in a path component or
    /// a binary basename, so accepting it would let a manifest edit decide
    /// which file the installer writes.
    NotAName {
        /// The rejected text, kept verbatim so the error names what the
        /// manifest actually said rather than a sanitized version of it.
        raw: String,
    },
}

impl CrateName {
    /// Accepts a manifest member entry as a crate name.
    ///
    /// This is the only way to obtain a `CrateName`, so every downstream use
    /// of one as a path component or binary basename is already checked.
    ///
    /// # Errors
    ///
    /// Returns [`NameError::Empty`] for the empty string and
    /// [`NameError::NotAName`] when any character falls outside ASCII
    /// alphanumerics, `-`, and `_`.
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        if raw.is_empty() {
            return Err(NameError::Empty);
        }
        if !raw
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
        {
            return Err(NameError::NotAName { raw: raw.to_string() });
        }
        Ok(CrateName(raw.to_string()))
    }

    /// Borrows the name for use as a path component or a binary basename.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A build stamp: one crate tree id and two shared blob ids.
///
/// Parsed rather than held as a String, so a value that reached the binary
/// through `option_env!` is checked at the boundary the gate reads rather
/// than compared as opaque text.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Stamp {
    crate_tree: TreeId,
    lock_blob: TreeId,
    workspace_blob: TreeId,
}

/// Why a colon-separated string was rejected as a build stamp.
#[derive(Debug, PartialEq, Eq)]
pub enum StampError {
    /// The string did not split into exactly the three ids a stamp carries,
    /// which is what a stamp from an older or newer format looks like.
    WrongFieldCount {
        /// How many colon-separated fields were actually present, so the
        /// error distinguishes a truncated stamp from a concatenated one.
        found: usize,
    },
    /// The shape was right but one of the three fields is not a git object
    /// id, so comparing it against a freshly computed stamp would compare
    /// arbitrary text.
    BadId(IdError),
}

impl Stamp {
    /// Accepts the colon-separated form a binary carries as a build stamp.
    ///
    /// Checked at this boundary rather than compared as text, because the
    /// value reaches the binary through the build environment and an
    /// unparseable stamp must read as an error rather than as "stale".
    ///
    /// # Errors
    ///
    /// Returns [`StampError::WrongFieldCount`] unless the input splits into
    /// exactly three colon-separated fields, and [`StampError::BadId`] when
    /// any field is not a git object id.
    pub fn parse(raw: &str) -> Result<Self, StampError> {
        let fields: Vec<&str> = raw.split(':').collect();
        let [crate_tree, lock_blob, workspace_blob] = fields.as_slice() else {
            return Err(StampError::WrongFieldCount { found: fields.len() });
        };
        Ok(Stamp {
            crate_tree: TreeId::parse(crate_tree).map_err(StampError::BadId)?,
            lock_blob: TreeId::parse(lock_blob).map_err(StampError::BadId)?,
            workspace_blob: TreeId::parse(workspace_blob).map_err(StampError::BadId)?,
        })
    }

    /// Renders the stamp back into the colon-separated form the build
    /// embeds, so a report shows the same text a rebuild would produce.
    pub fn as_display(&self) -> String {
        format!(
            "{}:{}:{}",
            self.crate_tree.as_str(),
            self.lock_blob.as_str(),
            self.workspace_blob.as_str()
        )
    }
}

/// One crate whose installed binary does not match its source.
///
/// A sum rather than a struct with nullable fields: `Stale` cannot be
/// constructed without an installed stamp, and `NotInstalled` cannot carry
/// one.
#[derive(Debug, PartialEq, Eq)]
pub enum Finding {
    /// The crate is a workspace member but has no installed binary at all,
    /// so a caller invoking it would reach an older name or nothing.
    NotInstalled {
        /// The member that has no binary on disk.
        crate_name: CrateName,
        /// What a build from the current sources would stamp the binary with,
        /// carried so the report can name the target rather than only the gap.
        expected: Stamp,
    },
    /// A binary exists but was built from different sources, which is the
    /// case the gate exists to catch: the command runs and answers from code
    /// that is no longer in the tree.
    Stale {
        /// The member whose binary is behind its sources.
        crate_name: CrateName,
        /// The stamp compiled into the binary that is actually installed.
        installed: Stamp,
        /// The stamp the current sources would produce, so the two sides of
        /// the mismatch appear together and neither has to be recomputed.
        expected: Stamp,
    },
    /// A binary is installed for a crate the manifest no longer lists, so it
    /// is unreachable from any source in the tree and will never be rebuilt.
    Orphaned {
        /// The name the leftover binary was installed under.
        crate_name: CrateName,
        /// The stamp it carries, which identifies the commit it came from and
        /// is therefore the only remaining record of where it originated.
        installed: Stamp,
    },
}

/// The crates whose installed binary does not match the expected stamp.
///
/// Both maps are supplied by the caller, so this makes no process call and
/// reads no file. Maps rather than parallel slices: a map cannot carry a
/// duplicate name, and absence expresses "not installed" without an extra
/// Option axis in the value.
pub fn diagnose(
    installed: &BTreeMap<CrateName, Stamp>,
    expected: &BTreeMap<CrateName, Stamp>,
) -> Vec<Finding> {
    let mut findings = Vec::new();

    for (crate_name, want) in expected {
        match installed.get(crate_name) {
            None => findings.push(Finding::NotInstalled {
                crate_name: crate_name.clone(),
                expected: want.clone(),
            }),
            Some(have) if have != want => findings.push(Finding::Stale {
                crate_name: crate_name.clone(),
                installed: have.clone(),
                expected: want.clone(),
            }),
            Some(_) => {}
        }
    }

    // Walked in both directions: an installed binary for a crate that is no
    // longer a member would otherwise be invisible.
    for (crate_name, have) in installed {
        if !expected.contains_key(crate_name) {
            findings.push(Finding::Orphaned {
                crate_name: crate_name.clone(),
                installed: have.clone(),
            });
        }
    }

    findings
}

/// The report, or `None` when everything is current.
///
/// `Option` rather than a struct carrying an always-empty stdout and an exit
/// code: this command is silent or it is a list plus one fix line, and a
/// struct would permit combinations that mean nothing.
pub fn render(findings: &[Finding]) -> Option<String> {
    if findings.is_empty() {
        return None;
    }

    let mut report = String::from("config doctor: installed binaries do not match their source\n\n");
    for finding in findings {
        match finding {
            Finding::NotInstalled { crate_name, .. } => {
                // Discarded rather than expect()-ed: `Write for String` is
                // infallible, so the only Err this could produce does not exist.
                let _ = writeln!(report, "  {}: not installed", crate_name.as_str());
            }
            Finding::Stale { crate_name, installed, expected } => {
                let _ = writeln!(
                    report,
                    "  {}: installed {}, source {}",
                    crate_name.as_str(),
                    installed.as_display(),
                    expected.as_display()
                );
            }
            Finding::Orphaned { crate_name, .. } => {
                let _ = writeln!(
                    report,
                    "  {}: installed but no longer a workspace member",
                    crate_name.as_str()
                );
            }
        }
    }
    report.push_str("\n  Fix: config build\n");
    Some(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn stamps(pairs: &[(&str, &str)]) -> BTreeMap<CrateName, Stamp> {
        pairs
            .iter()
            .map(|(name, stamp)| {
                (
                    CrateName::parse(name).expect("test name is valid"),
                    Stamp::parse(stamp).expect("test stamp is valid"),
                )
            })
            .collect()
    }

    const STAMP_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:cccccccccccccccccccccccccccccccccccccccc";
    const STAMP_B: &str = "dddddddddddddddddddddddddddddddddddddddd:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:cccccccccccccccccccccccccccccccccccccccc";

    #[test]
    fn everything_current_yields_no_findings() {
        let expected = stamps(&[("config-manifest", STAMP_A)]);
        let installed = stamps(&[("config-manifest", STAMP_A)]);
        assert!(diagnose(&installed, &expected).is_empty());
    }

    #[test]
    fn a_stale_binary_is_reported_with_both_stamps() {
        let expected = stamps(&[("config-manifest", STAMP_A)]);
        let installed = stamps(&[("config-manifest", STAMP_B)]);
        let findings = diagnose(&installed, &expected);
        assert_eq!(findings.len(), 1);
        assert!(matches!(findings[0], Finding::Stale { .. }));
    }

    #[test]
    fn a_missing_binary_is_reported_as_not_installed() {
        let expected = stamps(&[("config-manifest", STAMP_A)]);
        let installed = BTreeMap::new();
        let findings = diagnose(&installed, &expected);
        assert_eq!(findings.len(), 1);
        assert!(matches!(findings[0], Finding::NotInstalled { .. }));
    }

    // An installed binary for a crate that is no longer a member is invisible
    // to a diagnosis that only walks the expected side, which is a fail-open
    // in a tool whose whole job is reporting what does not match.
    #[test]
    fn an_installed_binary_with_no_member_is_orphaned() {
        let expected = stamps(&[("config-manifest", STAMP_A)]);
        let installed = stamps(&[("config-manifest", STAMP_A), ("config-gone", STAMP_B)]);
        let findings = diagnose(&installed, &expected);
        assert_eq!(findings.len(), 1);
        assert!(matches!(findings[0], Finding::Orphaned { .. }));
    }

    #[test]
    fn a_clean_render_is_none() {
        assert_eq!(render(&[]), None);
    }

    #[test]
    fn a_stale_render_names_the_crate_and_the_fix() {
        let expected = stamps(&[("config-manifest", STAMP_A)]);
        let installed = stamps(&[("config-manifest", STAMP_B)]);
        let text = render(&diagnose(&installed, &expected)).expect("stale renders");
        assert!(text.contains("config-manifest"));
        assert!(text.contains("config build"));
    }

    #[test]
    fn a_malformed_stamp_is_rejected_at_the_boundary() {
        assert!(Stamp::parse("not-a-stamp").is_err());
        assert!(Stamp::parse("").is_err());
    }

    #[test]
    fn a_crate_name_with_a_path_separator_is_rejected() {
        assert!(CrateName::parse("../evil").is_err());
        assert!(CrateName::parse("").is_err());
    }
}
