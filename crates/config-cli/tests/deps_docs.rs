//! The dependency-checking documentation: `deps/README.md` and the
//! "Dependency checking" section of `~/README.md`.
//!
//! Scope is deliberately narrow. These assert only the facts that rot
//! silently: a cited file path that gets renamed, a documented flag the
//! argument parser stops accepting, an alias definition that drifts from the
//! hook, a dependency list that falls behind `deps.toml`. Prose, wording and
//! section order are not asserted, because freezing those would make every
//! edit to the writing a test failure.
//!
//! Converted whole from `tests/deps-docs.test.sh`, which ran **25**
//! assertions on this machine from 21 `assert_*` call sites: one sits in a
//! loop over the two platform variants, and three more are reached once per
//! probed flag.
//!
//! The engine is the oracle for every flag question, not its source. The
//! shell version grepped a `case` statement, which a compiled binary has no
//! equivalent of, and `--help` is the better oracle regardless: it is what
//! the parser publishes, so a flag the parser accepts but never lists is a
//! documentation defect on its own.
//!
//! Reached through `CARGO_BIN_EXE_config-cli` rather than through a
//! `config-cli` found on `PATH`. The shell suite used the latter and made
//! the absent-program case a real hazard, which is why it had to grow a
//! third outcome after 127 read as acceptance and every flag passed against
//! a program that was not there. A path the harness builds cannot be absent,
//! so that whole failure mode is gone rather than guarded.

use dotfiles_test_support::repo::root as repo_root;
use std::path::{Path, PathBuf};
use std::process::Command;

fn deps_dir() -> PathBuf {
    repo_root().join("deps")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()))
}

fn deps_readme() -> String {
    read(&deps_dir().join("README.md"))
}

/// Both documents' text, which most assertions below search together.
fn docs_text() -> String {
    format!("{}\n{}", deps_readme(), read(&repo_root().join("README.md")))
}

/// Runs the engine and returns its exit status code.
///
/// `DEPS_LOCAL_CONF` points at a path that does not exist so a platform
/// variant cannot answer for the file under test.
fn engine_status(arguments: &[&str], environment: &[(&str, &str)]) -> Option<i32> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_config-cli"));
    command
        .args(arguments)
        .env("DOTFILES_ROOT", repo_root())
        .env("DEPS_LOCAL_CONF", "/nonexistent/deps-platform.toml");
    for (key, value) in environment {
        command.env(key, value);
    }
    command.output().expect("the engine spawns").status.code()
}

/// Every substring of `text` wrapped in single backticks.
fn backticked(text: &str) -> Vec<String> {
    let mut found: Vec<String> = text
        .split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect();
    found.sort();
    found.dedup();
    found
}

#[test]
fn both_documents_exist_with_content_in_them() {
    for path in [deps_dir().join("README.md"), repo_root().join("README.md")] {
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{} must ship here: {error}", path.display()));
        assert!(
            !text.is_empty(),
            "{} is empty, so every assertion that reads it would compare nothing",
            path.display()
        );
    }
}

/// Every repo-relative path the docs cite resolves.
///
/// Paths are extracted from the prose rather than listed here. A hardcoded
/// list would pass while the docs cited something else entirely.
///
/// A citation is either rooted (a leading `.` or `~/`, resolved against the
/// repo root) or bare (`test-local.sh`, resolved against `deps/`, which is
/// how the deps README refers to its own neighbours).
#[test]
fn every_file_path_the_docs_cite_resolves() {
    let text = docs_text();
    let cited: Vec<String> = backticked(&text)
        .into_iter()
        .filter(|candidate| {
            let bare = candidate.trim_start_matches("~/").trim_start_matches("./");
            !bare.is_empty()
                && bare.chars().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, '.' | '_' | '/' | '-')
                })
                && (bare.ends_with(".sh")
                    || bare.ends_with(".conf")
                    || bare.ends_with(".yml")
                    || bare.ends_with(".md")
                    || bare.rsplit_once('.').is_some_and(|(stem, extension)| {
                        stem == "Dockerfile" && extension.chars().all(|byte| byte.is_ascii_lowercase())
                    }))
        })
        .collect();

    // The positive control. Without it an extractor that matched nothing
    // would report every cited path as existing.
    assert!(
        !cited.is_empty(),
        "the docs cite no file path at all, so the resolution check below \
         would pass having checked nothing"
    );

    let missing: Vec<&String> = cited
        .iter()
        .filter(|candidate| {
            let bare = candidate.trim_start_matches("~/").trim_start_matches("./");
            !repo_root().join(bare).exists() && !deps_dir().join(bare).exists()
        })
        .collect();
    assert!(
        missing.is_empty(),
        "the docs cite paths that do not resolve against the repo root or \
         deps/: {missing:?}"
    );
}

/// The README states the alias expansion once, as a literal
/// `alias depcheck=...` line copied from the hook. Matching that exact line
/// is what makes the assertion fail when the hook's definition changes.
/// Searching the whole file instead would match the same command spelled out
/// in the flag reference below it, which lets the alias drift unnoticed.
#[test]
fn the_documented_depcheck_alias_matches_the_hook() {
    let hook = read(&deps_dir().join("depcheck-hook.sh"));
    let definition = hook
        .lines()
        .find(|line| line.starts_with("alias depcheck="))
        .expect("the hook defines a depcheck alias");
    assert!(
        definition.len() > "alias depcheck=".len(),
        "the hook's depcheck alias expands to nothing, so matching it in the \
         README would prove nothing"
    );

    assert!(
        deps_readme().contains(definition),
        "the deps README does not carry the hook's exact alias line \
         ({definition:?}), so the documented expansion has drifted from the \
         hook"
    );
}

/// Every flag the docs name must be accepted, and every flag the parser
/// accepts must be documented. The engine exits 2 on an unknown argument, so
/// the parser itself is the oracle.
///
/// Only flags on a line that also names `config deps` or `depcheck` count.
/// Both READMEs document other tools whose flags this test must not try to
/// feed to the argument parser.
#[test]
fn the_documented_flags_are_exactly_the_flags_the_parser_accepts() {
    let text = docs_text();
    let mut documented: Vec<String> = text
        .lines()
        .filter(|line| line.contains("config deps") || line.contains("depcheck"))
        .flat_map(flags_in)
        .collect();
    documented.sort();
    documented.dedup();
    assert!(
        !documented.is_empty(),
        "the docs name no flag at all, so both directions of this check would \
         compare empty lists"
    );

    // A flag that takes a value cannot be probed bare: it exits 2 on purpose,
    // because selecting the empty set and reporting success would be worse.
    // The value must name a real dependency, since --only rejects a name that
    // matches no entry, also on purpose, so a typo in a workflow cannot
    // install nothing and pass.
    //
    // --dry-run is appended so a probe never mutates this machine, except
    // when the flag under probe IS --dry-run: the engine rejects a repeated
    // flag, so passing it twice would report the parser refusing its own
    // flag.
    let rejected: Vec<&String> = documented
        .iter()
        .filter(|flag| {
            let mut arguments = vec!["deps", "check", flag.as_str()];
            if flag.as_str() == "--only" {
                arguments.push("git");
            }
            if flag.as_str() != "--dry-run" {
                arguments.push("--dry-run");
            }
            engine_status(&arguments, &[]) == Some(2)
        })
        .collect();
    assert!(
        rejected.is_empty(),
        "the engine rejects flags the docs tell a reader to use: {rejected:?}"
    );

    // The arity guard itself, which the probe above deliberately steps
    // around. Without it `--only` with nothing after it selects the empty set
    // and reports a vacuous success.
    assert_eq!(
        engine_status(&["deps", "check", "--only"], &[]),
        Some(2),
        "`--only` with no value did not exit 2, so it selects the empty set \
         and reports success having checked nothing"
    );

    // The other direction, harvested from --help. Both verbs are harvested:
    // they take the same flags today, and a flag added to one alone would be
    // a surface the docs cannot describe consistently.
    let mut published: Vec<String> = ["check", "install"]
        .iter()
        .flat_map(|verb| {
            let output = Command::new(env!("CARGO_BIN_EXE_config-cli"))
                .args(["deps", verb, "--help"])
                .output()
                .expect("the engine spawns");
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| line.starts_with("  ") && line.trim_start().starts_with("--"))
                .flat_map(flags_in)
                .collect::<Vec<String>>()
        })
        .collect();
    published.sort();
    published.dedup();
    assert!(
        !published.is_empty(),
        "the --help harvest found no flag, so the undocumented-flag check \
         below would compare against an empty list"
    );

    let readme = deps_readme();
    let undocumented: Vec<&String> = published
        .iter()
        .filter(|flag| !readme.contains(flag.as_str()))
        .collect();
    assert!(
        undocumented.is_empty(),
        "the parser accepts flags the deps README never names: {undocumented:?}"
    );
}

/// Every `--flag` token on one line.
fn flags_in(line: &str) -> Vec<String> {
    line.split("--")
        .skip(1)
        .filter_map(|rest| {
            let name: String = rest
                .chars()
                .take_while(|byte| byte.is_ascii_lowercase() || *byte == '-')
                .collect();
            let name = name.trim_end_matches('-');
            (!name.is_empty() && name.starts_with(|byte: char| byte.is_ascii_lowercase()))
                .then(|| format!("--{name}"))
        })
        .collect()
}

/// The docs name specific dependencies when explaining the non-binary and
/// platform-tolerant checks. Those names are the ones that go stale when an
/// entry is renamed or moved.
///
/// Only names on the guarded list are checked, so ordinary prose in backticks
/// (a command name, a package manager) is not mistaken for a dependency
/// claim. And a guarded name the docs never mention is never reached, so it
/// would contribute nothing while making the guard list look broader than it
/// is: `ripgrep` sat in bare prose, and renaming its `deps.toml` entry left
/// the shell suite green. Requiring every guarded name to be extractable is
/// what keeps the list honest as the prose is edited.
#[test]
fn every_dependency_the_docs_name_is_really_tracked() {
    const GUARDED: [&str; 9] = [
        "zsh-autosuggestions",
        "tpm",
        "nvm",
        "rustup",
        "zoxide",
        "alacritty",
        "neovim",
        "ripgrep",
        "fzf",
    ];

    let shared = read(&deps_dir().join("deps.toml"));
    let tracked: Vec<String> = shared
        .lines()
        .map(|line| line.split('#').next().unwrap_or(""))
        .filter_map(|line| {
            let inner = line.trim_end().strip_prefix('[')?.strip_suffix(']')?;
            inner
                .chars()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == '-')
                .then(|| inner.to_string())
        })
        .collect();
    assert!(
        !tracked.is_empty(),
        "positive control: deps.toml yielded no entry names, so every check \
         below would compare against an empty list"
    );

    let mentioned = backticked(&docs_text());
    let unreached: Vec<&str> = GUARDED
        .iter()
        .copied()
        .filter(|guarded| !mentioned.iter().any(|name| name == guarded))
        .collect();
    assert!(
        unreached.is_empty(),
        "guarded dependency names never appear in the docs in backticks, so \
         they are never checked and the guard list is wider than it looks: \
         {unreached:?}"
    );

    let absent: Vec<&String> = mentioned
        .iter()
        .filter(|name| GUARDED.contains(&name.as_str()))
        .filter(|name| !tracked.contains(name))
        .collect();
    assert!(
        absent.is_empty(),
        "the docs name dependencies that deps.toml does not track: {absent:?}"
    );
}

/// oh-my-zsh is documented as not being in the shared file, and really is
/// not. It lives in `deps-linux.toml`, because the mac machine does not use
/// it.
#[test]
fn oh_my_zsh_is_documented_as_not_shared_and_really_is_not() {
    let readme = deps_readme();
    assert!(
        readme.contains("oh-my-zsh"),
        "the deps README no longer mentions oh-my-zsh, so its claim about the \
         shared file is gone and the check below asserts nothing anyone reads"
    );

    let shared = read(&deps_dir().join("deps.toml"));
    assert!(
        !shared.lines().any(|line| line.trim_end() == "[oh-my-zsh]"),
        "deps.toml carries an [oh-my-zsh] entry, which the deps README says \
         it does not, so a mac machine would be asked to install it"
    );
}

/// `deps-local.conf` used to hold one branch's own list. The
/// `deps-mac.toml` / `deps-linux.toml` pair replaced it precisely so both
/// variants ship together and the platform check decides which is read.
///
/// The retirement check reads the FILE. The shell version passed the deps
/// README's TEXT where grep expects a path, so grep warned about a filename
/// too long, printed nothing, and the empty-expected assertion passed no
/// matter what the README said. Verified before that fix by appending a
/// `deps-local.conf` mention and watching the assertion still report ok.
#[test]
fn both_platform_variants_ship_and_the_retired_conf_is_unmentioned() {
    for platform in ["mac", "linux"] {
        let path = deps_dir().join(format!("deps-{platform}.toml"));
        assert!(
            path.is_file(),
            "{} does not ship here, so the platform check has nothing to \
             select on a {platform} machine",
            path.display()
        );
    }

    let readme = deps_readme();
    assert!(
        !readme.is_empty(),
        "positive control: the deps README read as empty, so its silence \
         about deps-local.conf would prove nothing"
    );
    let mentions: Vec<(usize, &str)> = readme
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains("deps-local.conf"))
        .map(|(index, line)| (index + 1, line))
        .collect();
    assert!(
        mentions.is_empty(),
        "the deps README still describes the retired deps-local.conf: \
         {mentions:?}"
    );
}

/// The docs describe a TOML schema, and that description is only true while
/// the parser actually refuses what the docs say is invalid.
///
/// Asserted against the parser's behaviour rather than against its source,
/// for the reason the pipe-format version of this pair gave: a grep for
/// source text could never have proved the behaviour, and a compiled binary
/// has no `IFS='|' read` line to grep for anyway.
///
/// The probe is an entry naming TWO check kinds. That is the rule serde
/// cannot express, since both fields deserialize independently, so it is the
/// one the crate's own `TryFrom` has to enforce and the one most worth
/// pinning from outside. Exit 2 is the caller-error code for a manifest that
/// does not load.
#[test]
fn the_documented_schema_constraint_is_one_the_parser_enforces() {
    let fixtures = tempfile::tempdir().expect("a fixture root");

    let two_kinds = fixtures.path().join("schema-probe.toml");
    std::fs::write(
        &two_kinds,
        "[probe]\ncommand = \"sh\"\ndir = \"$HOME/.config\"\ndocs = \"https://example.invalid\"\n",
    )
    .expect("the probe manifest writes");

    // The positive control comes with it. Without one, the assertion above
    // passes against an engine that rejects every manifest, including a
    // well-formed one.
    //
    // The control names `sh`, which is present wherever this test can run, so
    // a well-formed manifest exits 0. A dependency that is merely absent
    // exits 1, which is not a parse failure and would not distinguish the two
    // outcomes this pair exists to compare.
    let one_kind = fixtures.path().join("schema-control.toml");
    std::fs::write(
        &one_kind,
        "[probe]\ncommand = \"sh\"\ndocs = \"https://example.invalid\"\n",
    )
    .expect("the control manifest writes");

    let absent_local = fixtures.path().join("no-such-local.toml");
    let probe_status = engine_status(
        &["deps", "check", "--dry-run"],
        &[
            ("DEPS_CONF", &two_kinds.display().to_string()),
            ("DEPS_LOCAL_CONF", &absent_local.display().to_string()),
        ],
    );
    let control_status = engine_status(
        &["deps", "check", "--dry-run"],
        &[
            ("DEPS_CONF", &one_kind.display().to_string()),
            ("DEPS_LOCAL_CONF", &absent_local.display().to_string()),
        ],
    );

    assert_eq!(
        control_status,
        Some(0),
        "positive control: a well-formed one-kind manifest did not load, so \
         the rejection below would not distinguish the schema rule from an \
         engine that refuses everything"
    );
    assert_eq!(
        probe_status,
        Some(2),
        "an entry naming two check kinds was accepted, so the rule the deps \
         README describes is not one the parser enforces"
    );

    // The phrase the probe above proves true. Asserting the warning still
    // states the rule is what keeps the prose and the parser in step.
    assert!(
        deps_readme().contains("exactly three fields"),
        "the deps README no longer warns about the field constraint the probe \
         above proves the parser enforces"
    );
}
