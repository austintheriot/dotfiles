//! The README's config section must name exactly the subcommands that exist.
//!
//! Converted from tests/config-docs.test.sh, whose extraction was
//! `sed -n 's/^- `config \([a-z-]*\)`.*/\1/p'`. That pattern yields nothing
//! when the bullet marker changes from `-` to `*`, which a Markdown linter
//! does, and the suite's `assert_equals '' ''` then passes having checked
//! nothing. A real parser makes that unrepresentable: `Event::Start(Item)`
//! is the same event either way.

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use std::path::{Path, PathBuf};

/// The repository root, at run time.
///
/// `DOTFILES_ROOT` first, then `HOME`, matching `nvim_lua_units.rs` and
/// `nvim_config_load.rs`. The manifest walk-up is the last resort and exists
/// for the `rust-checks.sh` snapshot, where neither variable points at the
/// archived tree.
///
/// Not `CARGO_MANIFEST_DIR` alone: that is a compile-time constant, so a
/// binary built in one tree and run against another reads the wrong root,
/// which is how these tests passed on the host and failed under the gate.
fn repo_root() -> PathBuf {
    for variable in ["DOTFILES_ROOT", "HOME"] {
        if let Some(value) = std::env::var_os(variable) {
            let candidate = PathBuf::from(value);
            if candidate.join("crates/Cargo.toml").is_file() {
                return candidate;
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the repo root is two levels above config-cli")
        .to_path_buf()
}

/// Every subcommand named by a list item in the README, in document order.
///
/// Reads inline code spans inside list items rather than matching a line
/// shape, so the bullet marker, the indentation and the surrounding prose
/// are all free to change.
fn readme_subcommand_bullets(markdown: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut list_depth: usize = 0;
    let mut in_item = false;
    for event in Parser::new(markdown) {
        match event {
            Event::Start(Tag::List(_)) => list_depth += 1,
            Event::End(TagEnd::List(_)) => list_depth = list_depth.saturating_sub(1),
            Event::Start(Tag::Item) => in_item = true,
            Event::End(TagEnd::Item) => in_item = false,
            Event::Code(code) if in_item && list_depth > 0 => {
                if let Some(rest) = code.strip_prefix("config ") {
                    let name = rest.split_whitespace().next().unwrap_or_default();
                    if !name.is_empty() {
                        found.push(name.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    found
}

/// The positive control the spec's section 4 requires: prove the parser
/// found something before asserting anything about what it did not find.
#[test]
fn the_readme_names_at_least_one_subcommand() {
    let readme = std::fs::read_to_string(repo_root().join("README.md")).expect("README.md");
    let named = readme_subcommand_bullets(&readme);
    assert!(
        !named.is_empty(),
        "the parser found no subcommand bullets at all, so every other \
         assertion in this file would pass vacuously"
    );
}

/// The defect the shell version could not catch: a marker change.
#[test]
fn the_parser_survives_a_bullet_marker_change() {
    let with_dash = "- `config build` builds things.\n";
    let with_star = "* `config build` builds things.\n";
    assert_eq!(
        readme_subcommand_bullets(with_dash),
        readme_subcommand_bullets(with_star),
        "the marker is Markdown syntax, not content, so it must not change \
         what the parser sees"
    );
    assert_eq!(readme_subcommand_bullets(with_dash), vec!["build".to_string()]);
}

/// The second half of the same defect: the old pattern's `[a-z-]*` dropped
/// any name containing a digit.
#[test]
fn a_subcommand_name_with_a_digit_is_not_dropped() {
    let markdown = "- `config s3-sync` does a thing.\n";
    assert_eq!(readme_subcommand_bullets(markdown), vec!["s3-sync".to_string()]);
}

/// The assertion the shell suite existed for: no bullet names a subcommand
/// that has been removed.
#[test]
fn the_readme_bullets_name_no_removed_subcommand() {
    let root = repo_root();
    let readme = std::fs::read_to_string(repo_root().join("README.md")).expect("README.md");
    let named = readme_subcommand_bullets(&readme);
    assert!(!named.is_empty(), "positive control: the parser found bullets");

    let script_directory = root.join(".scripts/config");
    let Ok(entries) = std::fs::read_dir(&script_directory) else {
        dotfiles_test_support::skip("no .scripts/config here, so scripts cannot be enumerated");
        return;
    };

    let mut existing: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()
                .and_then(|name| name.strip_prefix("config-"))
                .map(str::to_string)
        })
        .collect();
    // Verbs the binary owns rather than a script. Kept explicit: the listing
    // is the union of scripts and binary subcommands, and a name that is in
    // neither is the defect this test catches.
    existing.extend(["deps".to_string()]);

    let stale: Vec<&String> = named
        .iter()
        .filter(|name| !existing.contains(name) && !name.contains(' '))
        .collect();
    assert!(
        stale.is_empty(),
        "the README names subcommands that no longer exist: {stale:?}"
    );
}

/// The forward direction, and the assertion this plan's first draft omitted:
/// a script that exists but no bullet names. The shell suite had it at
/// tests/config-docs.test.sh:38 with a `grep -qF "config $sub"` over the
/// section, and dropping it in conversion would have lost coverage silently,
/// which is the exact failure this plan names as its weakest link.
///
/// Note the asymmetry with the reverse direction: this one matches the
/// section text rather than the parsed bullets, because a subcommand may be
/// documented in prose (`config status`, `config commit` are named as git
/// passthrough) without being a bullet. The shell suite's grep had the same
/// property.
#[test]
fn every_config_script_appears_in_the_readme_section() {
    let root = repo_root();
    let readme = std::fs::read_to_string(repo_root().join("README.md")).expect("README.md");

    let script_directory = root.join(".scripts/config");
    let Ok(entries) = std::fs::read_dir(&script_directory) else {
        dotfiles_test_support::skip("no .scripts/config here, so scripts cannot be enumerated");
        return;
    };

    let mut scripts: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()
                .and_then(|name| name.strip_prefix("config-"))
                .map(str::to_string)
        })
        .collect();
    scripts.sort();
    assert!(
        !scripts.is_empty(),
        "positive control: no config-* scripts found, so this assertion would \
         pass vacuously"
    );

    let undocumented: Vec<&String> = scripts
        .iter()
        .filter(|name| !readme.contains(&format!("config {name}")))
        .collect();
    assert!(
        undocumented.is_empty(),
        "these subcommands exist but the README never names them: {undocumented:?}"
    );
}

/// The section must point at the generated listing rather than restating it,
/// so descriptions have exactly one home.
#[test]
fn the_readme_points_at_config_help() {
    let readme = std::fs::read_to_string(repo_root().join("README.md")).expect("README.md");
    assert!(
        readme.contains("config help"),
        "the README must send the reader to `config help` for descriptions"
    );
}
