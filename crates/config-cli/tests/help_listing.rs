//! `config help` listing behavior, asserted against the built binary.
//!
//! The property under test is that the listing is built by asking siblings
//! rather than from a compiled-in list, which no golden fixture can prove:
//! a fixture records what the listing says, not where the text came from.

use std::process::Command;

/// Write an executable stand-in subcommand that answers `--describe`.
///
/// Returns nothing because the caller only needs the file to exist; the
/// directory it lands in is what the enumeration is pointed at.
fn write_describable_script(directory: &std::path::Path, name: &str, description: &str) {
    let script = directory.join(name);
    std::fs::write(
        &script,
        format!("#!/bin/sh\n[ \"${{1:-}}\" = --describe ] && printf '{description}\\n'\n"),
    )
    .expect("write the stand-in subcommand");
    let mut permissions = std::fs::metadata(&script)
        .expect("read the stand-in's metadata")
        .permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(&script, permissions).expect("make the stand-in executable");
}

/// The listing is built by asking siblings, not from an embedded list.
///
/// An embedded list is a second copy of the same facts, and per the shell
/// version's own comment, "the one that nobody edits is the one that goes
/// stale." Asserted by pointing the enumeration at a fixture directory
/// containing one script with a known description: an embedded list cannot
/// know about a subcommand invented in a temporary directory.
#[test]
fn the_listing_asks_its_siblings_rather_than_embedding_them() {
    let fixture = tempfile::tempdir().expect("tempdir");
    write_describable_script(fixture.path(), "config-invented", "An invented subcommand");

    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .arg("help")
        .env("CONFIG_SUBCOMMAND_DIR", fixture.path())
        .output()
        .expect("the binary runs");

    let stdout = String::from_utf8_lossy(&run.stdout);

    // Positive control: the listing must be non-empty, or the "contains"
    // assertions below hold for a command that printed nothing.
    assert!(!stdout.is_empty(), "help must list something");
    assert!(
        stdout.contains("invented"),
        "a subcommand present only in the fixture directory must appear, \
         which an embedded list could not do: {stdout:?}"
    );
    assert!(
        stdout.contains("An invented subcommand"),
        "the description must come from --describe: {stdout:?}"
    );
}

/// A sibling that cannot be executed is listed as `(undocumented)`.
///
/// Only an executable sibling is reachable through the dispatcher, which
/// requires `-x` before it execs. A file the listing names but the dispatcher
/// will not run has no description to ask for, and asking anyway would put a
/// permission error in the column where the description belongs.
#[test]
fn a_non_executable_sibling_is_listed_as_undocumented() {
    let fixture = tempfile::tempdir().expect("tempdir");
    let script = fixture.path().join("config-inert");
    std::fs::write(&script, "#!/bin/sh\n# help: never runs\n").expect("write the inert sibling");

    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .arg("help")
        .env("CONFIG_SUBCOMMAND_DIR", fixture.path())
        .output()
        .expect("the binary runs");

    let stdout = String::from_utf8_lossy(&run.stdout);

    // Positive control: the row has to be present at all before its contents
    // mean anything, and an empty listing would satisfy every negative below.
    assert!(
        stdout.contains("inert"),
        "a non-executable sibling is still a row in the listing: {stdout:?}"
    );
    assert!(
        stdout.contains("(undocumented)"),
        "an unaskable sibling degrades to (undocumented): {stdout:?}"
    );
    assert!(
        !stdout.contains("never runs"),
        "the description comes from asking, not from reading the file, so the \
         `# help:` text of an unaskable sibling must not appear: {stdout:?}"
    );
}

/// The rows are sorted by name, matching the shell glob the listing replaces.
///
/// The shell version enumerated `config-*` through a glob, which the shell
/// expands in sorted order, so the ordering is part of the recorded output
/// rather than an accident of directory iteration. Readdir order is not
/// sorted on any filesystem this runs on.
#[test]
fn the_rows_are_sorted_by_subcommand_name() {
    let fixture = tempfile::tempdir().expect("tempdir");
    for name in ["config-zulu", "config-alpha", "config-mike"] {
        write_describable_script(fixture.path(), name, "a description");
    }

    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .arg("help")
        .env("CONFIG_SUBCOMMAND_DIR", fixture.path())
        .output()
        .expect("the binary runs");

    let stdout = String::from_utf8_lossy(&run.stdout);
    let order: Vec<&str> = ["alpha", "mike", "zulu"]
        .into_iter()
        .filter(|name| stdout.contains(*name))
        .collect();

    // Positive control: all three must be present, or a listing that dropped
    // two of them would trivially be "in order".
    assert_eq!(
        order.len(),
        3,
        "every stand-in must be listed before ordering means anything: {stdout:?}"
    );
    let alpha = stdout.find("alpha").expect("alpha is listed");
    let mike = stdout.find("mike").expect("mike is listed");
    let zulu = stdout.find("zulu").expect("zulu is listed");
    assert!(
        alpha < mike && mike < zulu,
        "rows must be sorted by name: {stdout:?}"
    );
}

/// `--help` still yields the listing rather than clap's own help text.
///
/// `help` is the one subcommand that does not stop at `--help`: the
/// dispatcher routes `config --help` here, and a reader who typed it expects
/// the answer to "what commands exist" rather than a block about `help`
/// itself. The shim prints the usage block and forwards the flag, so the
/// binary's job is to answer with the listing regardless.
#[test]
fn help_with_the_help_flag_still_prints_the_listing() {
    let fixture = tempfile::tempdir().expect("tempdir");
    write_describable_script(fixture.path(), "config-invented", "An invented subcommand");

    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["help", "--help"])
        .env("CONFIG_SUBCOMMAND_DIR", fixture.path())
        .output()
        .expect("the binary runs");

    let stdout = String::from_utf8_lossy(&run.stdout);

    // Positive control: an empty stdout would satisfy the negative assertion
    // below, so require output before characterizing it.
    assert!(!stdout.is_empty(), "--help must print something");
    assert!(
        stdout.contains("An invented subcommand"),
        "--help still prints the listing, because the listing is what help \
         is for: {stdout:?}"
    );
    assert!(
        !stdout.contains("Options:"),
        "clap's own help text must not replace the listing: {stdout:?}"
    );
}

/// `--describe` reaches the binary as a request for the listing, not an error.
///
/// The shim answers `--describe` itself and never forwards it, so this only
/// pins that a direct `config-cli help --describe` is accepted rather than
/// rejected by clap. The one-line contract every subcommand owes the listing
/// is asserted against the shim in `tests/config-usage.test.sh`, where the
/// shim is the thing that answers.
#[test]
fn the_binary_accepts_the_describe_flag_rather_than_rejecting_it() {
    let fixture = tempfile::tempdir().expect("tempdir");
    write_describable_script(fixture.path(), "config-invented", "An invented subcommand");

    let run = Command::new(env!("CARGO_BIN_EXE_config-cli"))
        .args(["help", "--describe"])
        .env("CONFIG_SUBCOMMAND_DIR", fixture.path())
        .output()
        .expect("the binary runs");

    let stdout = String::from_utf8_lossy(&run.stdout);

    // Positive control: a rejected flag exits non-zero with empty stdout, so
    // requiring output first is what makes the success check mean anything.
    assert!(
        run.status.success(),
        "--describe must not be a parse error: {:?}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        stdout.contains("An invented subcommand"),
        "the binary answers with the listing: {stdout:?}"
    );
}
