use std::collections::BTreeSet;
use std::fmt;

use crate::path::{PathError, RelPath};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathPattern {
    pub path: RelPath,
    pub is_dir: bool,
}

impl PathPattern {
    pub fn parse(raw: &str) -> Result<Self, PathError> {
        let is_dir = raw.ends_with('/');
        let path = RelPath::parse(raw.trim_end_matches('/'))?;
        Ok(PathPattern { path, is_dir })
    }

    pub fn matches(&self, candidate: &RelPath) -> bool {
        if candidate == &self.path {
            return true;
        }
        self.is_dir
            && candidate.as_str().starts_with(self.path.as_str())
            && candidate.as_str()[self.path.as_str().len()..].starts_with('/')
    }
}

impl fmt::Display for PathPattern {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.path.as_str())?;
        if self.is_dir {
            formatter.write_str("/")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rule {
    Shared(PathPattern),
    Excluded(PathPattern),
    PerBranch(PathPattern),
}

impl Rule {
    fn pattern(&self) -> &PathPattern {
        match self {
            Rule::Shared(pattern) | Rule::Excluded(pattern) | Rule::PerBranch(pattern) => pattern,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    Path { line: usize, source: PathError },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::Path { line, source } => {
                write!(formatter, ".sync-manifest line {line}: {source}")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    Shared,
    Excluded,
    PerBranch,
    Unmatched,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedPaths(BTreeSet<RelPath>);

impl SharedPaths {
    pub fn iter(&self) -> impl Iterator<Item = &RelPath> {
        self.0.iter()
    }

    pub fn contains(&self, path: &RelPath) -> bool {
        self.0.contains(path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partition {
    pub shared: SharedPaths,
    pub unmatched: Vec<RelPath>,
}

pub fn parse(text: &str) -> Result<Manifest, ManifestError> {
    let mut rules = Vec::new();
    for (index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line_number = index + 1;
        let (constructor, body): (fn(PathPattern) -> Rule, &str) = match line.as_bytes()[0] {
            b'!' => (Rule::Excluded, &line[1..]),
            b'~' => (Rule::PerBranch, &line[1..]),
            _ => (Rule::Shared, line),
        };
        let pattern = PathPattern::parse(body).map_err(|source| ManifestError::Path {
            line: line_number,
            source,
        })?;
        rules.push(constructor(pattern));
    }
    Ok(Manifest { rules })
}

pub fn print(manifest: &Manifest) -> String {
    let mut out = String::new();
    for rule in &manifest.rules {
        let prefix = match rule {
            Rule::Shared(_) => "",
            Rule::Excluded(_) => "!",
            Rule::PerBranch(_) => "~",
        };
        out.push_str(prefix);
        out.push_str(&rule.pattern().to_string());
        out.push('\n');
    }
    out
}

impl Manifest {
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    pub fn shared_rules(&self) -> impl Iterator<Item = &PathPattern> {
        self.rules.iter().filter_map(|rule| match rule {
            Rule::Shared(pattern) => Some(pattern),
            Rule::Excluded(_) | Rule::PerBranch(_) => None,
        })
    }

    pub fn classify(&self, path: &RelPath) -> Classification {
        let matching = |wanted: fn(&Rule) -> Option<&PathPattern>| {
            self.rules
                .iter()
                .filter_map(wanted)
                .any(|pattern| pattern.matches(path))
        };
        if matching(|rule| match rule {
            Rule::Excluded(pattern) => Some(pattern),
            _ => None,
        }) {
            return Classification::Excluded;
        }
        if matching(|rule| match rule {
            Rule::Shared(pattern) => Some(pattern),
            _ => None,
        }) {
            return Classification::Shared;
        }
        if matching(|rule| match rule {
            Rule::PerBranch(pattern) => Some(pattern),
            _ => None,
        }) {
            return Classification::PerBranch;
        }
        Classification::Unmatched
    }

    pub fn partition<'a>(&self, paths: impl Iterator<Item = &'a RelPath>) -> Partition {
        let mut shared = BTreeSet::new();
        let mut unmatched = Vec::new();
        for path in paths {
            match self.classify(path) {
                Classification::Shared => {
                    shared.insert(path.clone());
                }
                Classification::Unmatched => unmatched.push(path.clone()),
                Classification::Excluded | Classification::PerBranch => {}
            }
        }
        unmatched.sort();
        unmatched.dedup();
        Partition {
            shared: SharedPaths(shared),
            unmatched,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn rel(raw: &str) -> RelPath {
        RelPath::parse(raw).expect("valid test path")
    }

    const SAMPLE: &str = "\
# shared
.sync-manifest
tests/
DOTFILES.md

# excluded
!tests/notify.test.sh

# per-branch
~.zshrc
~docs/superpowers/
";

    #[test]
    fn parse_reads_three_rule_kinds_and_skips_comments_and_blanks() {
        let manifest = parse(SAMPLE).expect("valid manifest");
        assert_eq!(manifest.rules().len(), 6);
        assert_eq!(manifest.shared_rules().count(), 3);
        assert!(matches!(manifest.rules()[3], Rule::Excluded(_)));
        assert!(matches!(manifest.rules()[4], Rule::PerBranch(_)));
    }

    #[test]
    fn a_trailing_slash_marks_a_directory_rule() {
        let manifest = parse(".sync-manifest\ntests/\n").expect("valid");
        let Rule::Shared(pattern) = &manifest.rules()[1] else {
            panic!("expected a shared rule")
        };
        assert!(pattern.is_dir);
        assert_eq!(pattern.path.as_str(), "tests");
        assert_eq!(pattern.to_string(), "tests/");
    }

    #[test]
    fn parse_reports_the_line_number_of_a_bad_path() {
        let err = parse(".sync-manifest\n\n/absolute\n").expect_err("must fail");
        assert_eq!(
            err,
            ManifestError::Path {
                line: 3,
                source: PathError::Absolute("/absolute".to_string())
            }
        );
    }

    #[test]
    fn directory_rules_match_beneath_and_exact_rules_match_exactly() {
        let manifest = parse("tests/\nDOTFILES.md\n").expect("valid");
        assert_eq!(
            manifest.classify(&rel("tests/lib.sh")),
            Classification::Shared
        );
        assert_eq!(manifest.classify(&rel("tests")), Classification::Shared);
        assert_eq!(
            manifest.classify(&rel("DOTFILES.md")),
            Classification::Shared
        );
        assert_eq!(
            manifest.classify(&rel("DOTFILES.md.bak")),
            Classification::Unmatched
        );
        assert_eq!(
            manifest.classify(&rel("testsuite/x")),
            Classification::Unmatched
        );
    }

    #[test]
    fn excluded_wins_over_shared_and_shared_wins_over_per_branch() {
        let manifest = parse(SAMPLE).expect("valid");
        assert_eq!(
            manifest.classify(&rel("tests/notify.test.sh")),
            Classification::Excluded
        );
        assert_eq!(
            manifest.classify(&rel("tests/lib.sh")),
            Classification::Shared
        );
        assert_eq!(manifest.classify(&rel(".zshrc")), Classification::PerBranch);
        assert_eq!(
            manifest.classify(&rel("docs/superpowers/specs/a.md")),
            Classification::PerBranch
        );
        let overlap = parse("dir/\n~dir/local.txt\n").expect("valid");
        assert_eq!(
            overlap.classify(&rel("dir/local.txt")),
            Classification::Shared
        );
    }

    #[test]
    fn partition_splits_shared_from_unmatched_and_drops_the_rest() {
        let manifest = parse(SAMPLE).expect("valid");
        let paths = [
            rel("tests/lib.sh"),
            rel("tests/notify.test.sh"),
            rel(".zshrc"),
            rel("stray.txt"),
            rel(".sync-manifest"),
        ];
        let partition = manifest.partition(paths.iter());
        let shared: Vec<&str> = partition.shared.iter().map(RelPath::as_str).collect();
        assert_eq!(shared, vec![".sync-manifest", "tests/lib.sh"]);
        assert_eq!(partition.unmatched, vec![rel("stray.txt")]);
    }

    #[test]
    fn print_is_canonical_and_round_trips() {
        let manifest = parse(SAMPLE).expect("valid");
        let printed = print(&manifest);
        assert_eq!(
            printed,
            ".sync-manifest\ntests/\nDOTFILES.md\n!tests/notify.test.sh\n~.zshrc\n~docs/superpowers/\n"
        );
        assert_eq!(parse(&printed).expect("valid"), manifest);
    }

    fn arbitrary_segment() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9_.-]{0,6}"
            .prop_filter("no dot-dot", |segment| segment != ".." && segment != ".")
    }

    fn arbitrary_rule_line() -> impl Strategy<Value = String> {
        (
            prop_oneof![Just(""), Just("!"), Just("~")],
            prop::collection::vec(arbitrary_segment(), 1..4),
            any::<bool>(),
        )
            .prop_map(|(prefix, segments, is_dir)| {
                let slash = if is_dir { "/" } else { "" };
                format!("{prefix}{}{slash}", segments.join("/"))
            })
    }

    proptest! {
        #[test]
        fn parse_print_round_trips(lines in prop::collection::vec(arbitrary_rule_line(), 0..8)) {
            let text = lines.join("\n") + "\n";
            let manifest = parse(&text).expect("generated lines are valid");
            prop_assert_eq!(parse(&print(&manifest)).expect("printed form is valid"), manifest);
        }

        #[test]
        fn shared_partition_never_contains_an_excluded_or_per_branch_path(
            lines in prop::collection::vec(arbitrary_rule_line(), 1..8),
            candidates in prop::collection::vec(prop::collection::vec(arbitrary_segment(), 1..4), 0..12),
        ) {
            let manifest = parse(&(lines.join("\n") + "\n")).expect("valid");
            let paths: Vec<RelPath> = candidates
                .iter()
                .map(|segments| RelPath::parse(&segments.join("/")).expect("valid"))
                .collect();
            let partition = manifest.partition(paths.iter());
            for path in partition.shared.iter() {
                prop_assert_eq!(manifest.classify(path), Classification::Shared);
            }
            for path in &partition.unmatched {
                prop_assert_eq!(manifest.classify(path), Classification::Unmatched);
            }
        }
    }
}
