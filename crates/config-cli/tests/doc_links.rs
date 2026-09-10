//! File paths cited in this repo's own documentation resolve.
//!
//! `deps_end_to_end.rs` makes this assertion for the two dependency-checking
//! READMEs. It cannot catch a dead path anywhere else, which is how
//! `docs/research/dotfiles-management-landscape.md` kept a citation to a
//! design doc for a full commit after that doc was deleted.
//!
//! Scope is deliberately narrow: only documentation that describes THIS
//! repo. The agent and rules files under `.claude/` are excluded on purpose.
//! They cite paths as illustrations of what to look for in whatever project
//! the agent is reviewing, and those are not claims that the file exists
//! here. Asserting on them yields around 200 false positives and no real
//! findings.
//!
//! Only backticked, repo-rooted paths with a file extension are checked. A
//! bare word in backticks is prose (a command, a flag, a package name), and
//! a URL is somebody else's problem.
//!
//! Converted whole from `tests/doc-links.test.sh`, which had 2 assertions
//! from 2 `assert_*` call sites, neither in a loop.

use dotfiles_test_support::repo::root as repo_root;
use std::path::{Path, PathBuf};

/// The docs that describe this repo, enumerated rather than globbed.
///
/// A glob over `.claude/` would silently pull in every future agent file and
/// reintroduce the false positives this scope exists to avoid. The research
/// directory IS globbed, because every file in it describes this repo.
fn documents(root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = ["README.md", "DOTFILES.md", "deps/README.md", ".claude/rules/dotfiles-tests.md"]
        .iter()
        .map(|relative| root.join(relative))
        .filter(|path| path.is_file())
        .collect();

    if let Ok(entries) = std::fs::read_dir(root.join("docs/research")) {
        let mut research: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path.extension().and_then(|extension| extension.to_str()) == Some("md")
            })
            .collect();
        research.sort();
        found.extend(research);
    }
    found
}

/// Every backticked span in some Markdown text.
fn backticked(text: &str) -> Vec<String> {
    let mut spans = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else {
            break;
        };
        spans.push(after[..close].to_string());
        rest = &after[close + 1..];
    }
    spans
}

/// The extensions that make a backticked span a claim about a file.
const PATH_EXTENSIONS: [&str; 7] = ["sh", "conf", "yml", "yaml", "md", "toml", "py"];

/// Whether a backticked span is a repo-rooted path citation.
///
/// Mirrors the shell suite's three filters: the character class, the
/// extension list, and the requirement of a directory component.
fn is_path_citation(span: &str) -> bool {
    let stripped = span
        .strip_prefix("~/")
        .or_else(|| span.strip_prefix("./"))
        .unwrap_or(span);

    if stripped.is_empty() {
        return false;
    }
    if !stripped
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "._/-".contains(character))
    {
        return false;
    }
    // A bare filename with no directory (`lib.sh`, `tmux.conf`) is a pointer
    // to a file the reader is expected to locate, not a claim about a path.
    // This also keeps the suite branch-neutral: the research doc names
    // mac-only files while discussing what varies per platform.
    if !stripped.contains('/') {
        return false;
    }
    let Some(extension) = Path::new(stripped).extension().and_then(|value| value.to_str()) else {
        return false;
    };
    PATH_EXTENSIONS.contains(&extension)
}

/// The cited path, with the two decorative prefixes removed.
fn normalize(span: &str) -> &str {
    span.strip_prefix("~/")
        .or_else(|| span.strip_prefix("./"))
        .unwrap_or(span)
}

/// Every path citation in one document, sorted and de-duplicated.
fn citations(text: &str) -> Vec<String> {
    let mut cited: Vec<String> = backticked(text)
        .iter()
        .filter(|span| is_path_citation(span))
        .map(|span| normalize(span).to_string())
        .collect();
    cited.sort();
    cited.dedup();
    cited
}

#[test]
fn at_least_one_document_is_checked() {
    let found = documents(&repo_root());
    assert!(
        !found.is_empty(),
        "found no Markdown file describing this repo; the enumeration is stale"
    );
}

#[test]
fn every_cited_path_resolves() {
    let root = repo_root();
    let found = documents(&root);
    assert!(
        !found.is_empty(),
        "positive control: the sweep needs at least one document to read"
    );

    let mut checked = 0_usize;
    let mut dead: Vec<String> = Vec::new();
    for document in &found {
        let Ok(text) = std::fs::read_to_string(document) else {
            continue;
        };
        let parent = document.parent().unwrap_or(&root).to_path_buf();
        for cited in citations(&text) {
            checked += 1;
            // A path may be cited from the root, or relative to the citing
            // file's own directory. Either resolution is a live citation.
            if root.join(&cited).exists() || parent.join(&cited).exists() {
                continue;
            }
            dead.push(format!("{}:{cited}", document.display()));
        }
    }

    assert!(
        checked > 0,
        "positive control: the documents yielded no path citations at all, \
         so an empty dead list would prove nothing"
    );
    assert!(
        dead.is_empty(),
        "these cited paths do not resolve: {}",
        dead.join(" ")
    );
}
