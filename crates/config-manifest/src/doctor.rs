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

#[derive(Debug, PartialEq, Eq)]
pub enum NameError {
    Empty,
    NotAName { raw: String },
}

impl CrateName {
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

#[derive(Debug, PartialEq, Eq)]
pub enum StampError {
    WrongFieldCount { found: usize },
    BadId(IdError),
}

impl Stamp {
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
    NotInstalled { crate_name: CrateName, expected: Stamp },
    Stale { crate_name: CrateName, installed: Stamp, expected: Stamp },
    Orphaned { crate_name: CrateName, installed: Stamp },
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
                writeln!(report, "  {}: not installed", crate_name.as_str())
                    .expect("writing to a String cannot fail");
            }
            Finding::Stale { crate_name, installed, expected } => {
                writeln!(
                    report,
                    "  {}: installed {}, source {}",
                    crate_name.as_str(),
                    installed.as_display(),
                    expected.as_display()
                )
                .expect("writing to a String cannot fail");
            }
            Finding::Orphaned { crate_name, .. } => {
                writeln!(
                    report,
                    "  {}: installed but no longer a workspace member",
                    crate_name.as_str()
                )
                .expect("writing to a String cannot fail");
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
