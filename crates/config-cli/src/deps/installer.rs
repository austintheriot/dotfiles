//! Every effect is argv, and the apt installer sets `DEBIAN_FRONTEND`.
//!
//! The retired check-deps shell engine built each install as a command
//! string and ran it through `sh -c`. Three defects came with that form,
//! and every one of them is
//! removed by making the effect a `Vec<OsString>` spawned with no shell:
//!
//! 1. `${SUDO}` was a string prefix (`retired-check-deps:164-168`). Elevation is
//!    data on the [`Step`], so the driver decides it, and interpolating the
//!    decision back into command text let a caller read a command whose
//!    privilege did not match its step. Here `sudo` is its own argv word.
//! 2. No quoting was possible. A package name or path containing a space was
//!    not representable safely. Nothing in the manifest triggers that today,
//!    which is exactly why the string form would have shipped it unnoticed.
//! 3. The dry run and the real run were different code. [`argv_for`] is pure
//!    and both `describe` and `perform` read it, so the preview IS the
//!    vector that would be spawned.
//!
//! `DEBIAN_FRONTEND=noninteractive` lives in the child environment this
//! module builds, never in an image. `retired-check-deps:66-88` records why: an
//! unattended bootstrap halted at tzdata's debconf prompt, "Please select
//! the geographic area in which you live", waiting for a keypress nobody was
//! there to supply. Every image and CI leg passed anyway, because
//! `Dockerfile.ubuntu` set the variable itself. The environment was quietly
//! compensating for a gap in the engine, so the engine looked correct
//! everywhere it was tested and failed on a real machine. The same rule
//! generalizes to `-y` and `--noconfirm`: non-interactivity belongs in the
//! argv the engine builds.

// `mod.rs` wires `wire` into a `run_to_fixpoint` call in a later commit.
// `expect` rather than `allow`, so the lint fires again the moment a caller
// lands and this attribute has to be deleted rather than quietly outliving
// its reason.
#![cfg_attr(
    not(test),
    expect(dead_code, reason = "mod.rs wires these into run_to_fixpoint in a later commit")
)]

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::{IsTerminal, Write};
use std::process::{Command, Output};

use deps_core::{
    ActionDescription, BrewKind, CloneSource, Elevation, ExecFailure, InstallAction, Installer,
    Installers, KeyringSource, NoInstallReason, PackageManager, PathRoot, PrivilegeRequirement,
    ScriptInstaller, SourceListEntry, SpawnError, Step, StepOutcome,
};
use dotfiles_path::BoundedText;

/// The debconf frontend an unattended run must use.
///
/// Named as a constant so the sabotage probe in this task's plan has one
/// production occurrence to remove, and so a reader sees the value the
/// engine sets rather than a literal buried in a builder.
const DEBIAN_FRONTEND: &str = "DEBIAN_FRONTEND";

/// The value that stops apt from asking a question.
const NONINTERACTIVE: &str = "noninteractive";

/// The URL each script installer is fetched from.
///
/// Hardcoded here rather than carried on [`ScriptInstaller`], because a
/// manifest-supplied URL would make one edited line an arbitrary-code
/// vector. `ScriptInstaller` is a closed set of identities for that reason,
/// and this function is where an identity becomes a URL.
fn script_url(installer: ScriptInstaller) -> &'static str {
    match installer {
        ScriptInstaller::Rustup => "https://sh.rustup.rs",
        ScriptInstaller::OhMyZsh => {
            "https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh"
        }
        ScriptInstaller::Zoxide => {
            "https://raw.githubusercontent.com/ajeetdsouza/zoxide/main/install.sh"
        }
    }
}

/// The repository each clone source names.
fn clone_url(source: CloneSource) -> &'static str {
    match source {
        CloneSource::Tpm => "https://github.com/tmux-plugins/tpm",
        CloneSource::ZshAutosuggestions => "https://github.com/zsh-users/zsh-autosuggestions",
    }
}

/// The keyring file each trust root is written to.
fn keyring_path(keyring: KeyringSource) -> &'static str {
    match keyring {
        KeyringSource::GithubCli => "/etc/apt/keyrings/githubcli-archive-keyring.gpg",
    }
}

/// The URL each keyring is fetched from.
fn keyring_url(keyring: KeyringSource) -> &'static str {
    match keyring {
        KeyringSource::GithubCli => "https://cli.github.com/packages/githubcli-archive-keyring.gpg",
    }
}

/// The source-list file each entry is appended to.
fn source_list_path(list: SourceListEntry) -> &'static str {
    match list {
        SourceListEntry::GithubCli => "/etc/apt/sources.list.d/github-cli.list",
    }
}

/// The environment a child process for `manager` runs with.
///
/// Only the variables this engine sets. The child inherits the rest of the
/// process environment through [`Command`]'s default, so this map is the
/// engine's own contribution and nothing else, which is what makes the
/// `DEBIAN_FRONTEND` test able to assert an absence in the ambient
/// environment and a presence here.
///
/// Set for apt only, because apt is the only manager that reads it. The
/// shell set it globally and said so; keying it to the manager makes the
/// reason visible instead of relying on brew and pacman ignoring a variable
/// they never read.
fn child_environment_for(manager: PackageManager) -> BTreeMap<OsString, OsString> {
    let mut environment = BTreeMap::new();
    if manager == PackageManager::Apt {
        environment.insert(OsString::from(DEBIAN_FRONTEND), OsString::from(NONINTERACTIVE));
    }
    environment
}

/// Every argv one action runs, in order.
///
/// A sequence rather than a single vector, because several real actions are
/// several commands. `retired-check-deps:235` chains eight commands with `&&` to
/// install `gh` on apt, and `retired-check-deps:266-270` taps before it installs
/// a cask. Collapsing those to one vector would need a shell to hold the
/// `&&`, which is the form this module exists to remove.
///
/// Returns an empty sequence for an action with no automated install. That
/// emptiness is what [`Spawning::perform`] reads to report
/// [`StepOutcome::NotAutomatable`] rather than spawning nothing and claiming
/// success.
#[must_use]
pub fn argv_sequence_for(
    action: &InstallAction,
    manager: PackageManager,
    privilege: PrivilegeRequirement,
) -> Vec<Vec<OsString>> {
    match action {
        InstallAction::Package { id } => match manager {
            // `-qq` on update and `-y` on install, matching
            // `retired-check-deps:435`. The `-y` is the argv half of the same
            // rule DEBIAN_FRONTEND is the environment half of: a question
            // apt would otherwise ask an unattended run.
            PackageManager::Apt => vec![
                elevated(privilege, ["apt-get", "update", "-qq"]),
                elevated_with(privilege, ["apt-get", "install", "-y"], [id.as_str()]),
            ],
            // `-Sy` syncs and installs in one command, so pacman is one
            // argv where apt is two (`retired-check-deps:437`).
            PackageManager::Pacman => {
                vec![elevated_with(privilege, ["pacman", "-Sy", "--noconfirm"], [id.as_str()])]
            }
            PackageManager::Brew => vec![words(["brew", "install", id.as_str()])],
            // An undetected manager has no install command in the shell
            // either. `plan` reports it manual-only before reaching here.
            PackageManager::Unknown => Vec::new(),
        },
        // Brew never elevates: it refuses to run as root and says so, which
        // is why `action_for` gives every brew action
        // `PrivilegeRequirement::None` and why no word here is `sudo`.
        InstallAction::Brew { kind, id, tap } => {
            let mut sequence = Vec::new();
            if let Some(tap_name) = tap {
                // `brew tap` is idempotent, so re-running costs a no-op
                // (`retired-check-deps:266-270`). Tapping is not enough on its
                // own: current Homebrew refuses to load a cask from an
                // untrusted tap, and `brew trust` is a recent subcommand, so
                // its failure is tolerated for an older Homebrew that does
                // not have it and does not need it.
                sequence.push(words(["brew", "tap", tap_name.as_str()]));
                sequence.push(words(["brew", "trust", tap_name.as_str()]));
            }
            sequence.push(match kind {
                BrewKind::Formula => words(["brew", "install", id.as_str()]),
                BrewKind::Cask => words(["brew", "install", "--cask", id.as_str()]),
            });
            sequence
        }
        // `retired-check-deps:235`, as argv. The shell wrote the keyring with
        // `wget -O- | sudo tee`, which needs a pipe; `curl -o <path>` under
        // the same privilege writes the same bytes to the same place with no
        // shell, and the privilege stays with the write either way.
        InstallAction::AptSource { keyring, list } => vec![
            elevated(privilege, ["apt-get", "update", "-qq"]),
            elevated(privilege, ["apt-get", "install", "-y", "curl"]),
            elevated(privilege, ["mkdir", "-p", "-m", "755", "/etc/apt/keyrings"]),
            elevated(
                privilege,
                ["curl", "-fsSL", "-o", keyring_path(*keyring), keyring_url(*keyring)],
            ),
            elevated(privilege, ["chmod", "go+r", keyring_path(*keyring)]),
            elevated_with(privilege, ["tee", source_list_path(*list)], []),
            elevated(privilege, ["apt-get", "update", "-qq"]),
            elevated(privilege, ["apt-get", "install", "-y", "gh"]),
        ],
        // pip installs into the user site directory, so no elevation. The
        // override is a separate word rather than part of a flag string,
        // because it overrides PEP 668 and blast radius belongs where a
        // reader sees it (`retired-check-deps:410`).
        InstallAction::Pip { id, break_system_packages } => {
            let mut argv = words(["python3", "-m", "pip", "install"]);
            if *break_system_packages {
                argv.push(OsString::from("--break-system-packages"));
            }
            argv.push(OsString::from(id.as_str()));
            vec![argv]
        }
        // The shell piped curl into sh. Fetching to a temporary file and
        // running it would need a filesystem this function does not have, so
        // `perform` runs the fetch and the interpreter as two spawns with a
        // pipe between them, and this sequence names both.
        InstallAction::Script { installer } => script_argv(*installer),
        InstallAction::GitClone { source, into } => {
            vec![vec![
                OsString::from("git"),
                OsString::from("clone"),
                OsString::from(clone_url(*source)),
                clone_destination(into),
            ]]
        }
        // nvm is a shell function, not a binary, so it is sourced first.
        // That is the one action a shell genuinely owns, and it is spawned
        // as `sh -c` over a fixed string with no interpolated data:
        // `NvmInstall` carries no payload precisely so no manifest text can
        // reach this word (`retired-check-deps:387`).
        InstallAction::NvmInstall => {
            vec![words(["sh", "-c", ". \"$HOME/.nvm/nvm.sh\" && nvm install --lts"])]
        }
        InstallAction::NotAutomatable { .. } => Vec::new(),
    }
}

/// The argv that installs, which is the last command in the sequence.
///
/// Pure, so every command shape is testable without spawning anything. That
/// purity is what makes "the dry run and the real run are the same vector"
/// checkable rather than asserted: [`Spawning::describe`] renders what this
/// returns and [`Spawning::perform`] spawns it.
///
/// Returns an empty vector for an action with no automated install.
#[must_use]
pub fn argv_for(
    action: &InstallAction,
    manager: PackageManager,
    privilege: PrivilegeRequirement,
) -> Vec<OsString> {
    argv_sequence_for(action, manager, privilege).pop().unwrap_or_default()
}

/// Where a fetched installer script is written before it is run.
///
/// One path per installer, so two installers in the same run cannot overwrite
/// each other's script.
fn script_path(installer: ScriptInstaller) -> OsString {
    let name = match installer {
        ScriptInstaller::Rustup => "rustup-init.sh",
        ScriptInstaller::OhMyZsh => "oh-my-zsh-install.sh",
        ScriptInstaller::Zoxide => "zoxide-install.sh",
    };
    let mut path = std::env::temp_dir();
    path.push(name);
    path.into_os_string()
}

/// The fetch-and-run pair one installer script needs.
///
/// The fetch writes the script to a FILE with `curl -o`, and the run executes
/// that file. It does not pipe.
///
/// The retired shell wrote `curl ... | sh`, and the first port of it kept the
/// two commands but dropped the pipe, because this module's whole design is
/// that an effect is argv and never a shell string -- and a pipe is not
/// expressible as argv. The result ran `curl` with its output discarded and
/// then `sh -s --` with nothing on stdin, which exits 0 having installed
/// nothing. The check then failed, and the run reported an install failure
/// whose real cause was that no install had been attempted.
///
/// Found by the container gate: rustup, zoxide and oh-my-zsh all failed on a
/// bare image while every unit test passed, because the tests assert the
/// planned argv and the argv was individually correct.
///
/// A file is what lets this stay argv. `AptSource` already took the same
/// route for the same reason: the shell wrote the keyring with
/// `wget -O- | sudo tee`, and this module writes it with `curl -o` instead.
fn script_argv(installer: ScriptInstaller) -> Vec<Vec<OsString>> {
    let path = script_path(installer);

    let mut fetch = words(["curl", "--proto", "=https", "--tlsv1.2", "-sSf", "-o"]);
    fetch.push(path.clone());
    fetch.push(OsString::from(script_url(installer)));

    // Each installer's own flags, from `retired-check-deps:308`, `:324` and
    // `:367`. oh-my-zsh takes `--keep-zshrc` because its installer otherwise
    // overwrites `~/.zshrc` with its template and moves the real one aside.
    //
    // `sh <path> --` rather than `sh -s --`: `-s` reads the script from
    // stdin, which is exactly what no longer arrives.
    let mut run = words(["sh"]);
    run.push(path);
    match installer {
        ScriptInstaller::Rustup => run.extend(words(["-y"])),
        ScriptInstaller::OhMyZsh => run.extend(words(["--unattended", "--keep-zshrc"])),
        ScriptInstaller::Zoxide => {}
    }

    vec![fetch, run]
}

/// Where a clone lands, as one `OsString`.
///
/// One word even when the path contains a space, which is the second defect
/// the string form could not represent. `PathRoot::Home` is read from the
/// environment here rather than passed in, because a clone destination that
/// did not resolve against the running user's home would land the clone
/// somewhere nothing ever reads.
fn clone_destination(into: &deps_core::CheckPath) -> OsString {
    let root = match into.root {
        PathRoot::Home => std::env::var_os("HOME").unwrap_or_default(),
        PathRoot::MacApplications => OsString::from("/Applications"),
        // A clone never targets the brew prefix, and guessing a prefix here
        // would write outside any directory this process owns. An empty root
        // yields a relative destination, which git creates under the working
        // directory rather than somewhere unexpected.
        PathRoot::BrewPrefix => OsString::new(),
    };
    let mut destination = root;
    if !destination.is_empty() {
        destination.push("/");
    }
    destination.push(into.rest.as_str());
    destination
}

/// Build an argv from string words, one `OsString` per word.
fn words<const COUNT: usize>(parts: [&str; COUNT]) -> Vec<OsString> {
    parts.into_iter().map(OsString::from).collect()
}

/// Prepend `sudo` as its own word when the step needs root.
///
/// Its own word, never glued to the program and never an uninterpolated
/// `${SUDO}` placeholder. Elevation is data on the [`Step`], so the driver
/// decides it; `retired-check-deps:164-168` re-encoded that decision as a string
/// prefix carrying its own trailing space, and a caller could then read a
/// command whose privilege did not match its step.
fn elevated<const COUNT: usize>(
    privilege: PrivilegeRequirement,
    parts: [&str; COUNT],
) -> Vec<OsString> {
    elevated_with(privilege, parts, [])
}

/// Prepend `sudo` when needed, then append dependency-supplied words.
///
/// `trailing` is separate from `parts` so every dependency-supplied string
/// becomes exactly one `OsString`, never split and never quoted. That is
/// what makes a package name or path containing a space representable.
fn elevated_with<const FIXED: usize, const EXTRA: usize>(
    privilege: PrivilegeRequirement,
    parts: [&str; FIXED],
    trailing: [&str; EXTRA],
) -> Vec<OsString> {
    let mut argv = Vec::with_capacity(FIXED + EXTRA + 1);
    if privilege == PrivilegeRequirement::Root {
        argv.push(OsString::from("sudo"));
    }
    argv.extend(parts.into_iter().map(OsString::from));
    argv.extend(trailing.into_iter().map(OsString::from));
    argv
}

/// Render an argv for a human to read.
///
/// Display only. Nothing re-parses this text, and nothing spawns it: the
/// vector is what runs. A word containing a space or a quote is shown in
/// single quotes so a reader can tell one word from two, which the string
/// form could not do at all.
#[must_use]
pub fn render_argv(argv: &[OsString]) -> String {
    argv.iter()
        .map(|word| {
            let text = word.to_string_lossy();
            if text.is_empty() || text.contains([' ', '\t', '"', '\'', '$', '&', '|', ';']) {
                format!("'{}'", text.replace('\'', r"'\''"))
            } else {
                text.into_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Render a whole sequence, one command per ` && `.
///
/// The separator is for a reader copying the line into a shell. This module
/// never runs the rendered text.
#[must_use]
pub fn render_sequence(sequence: &[Vec<OsString>]) -> String {
    sequence.iter().map(|argv| render_argv(argv)).collect::<Vec<_>>().join(" && ")
}

/// Whether a run may spawn, and whether it may ask.
///
/// A seam, not a flag pair. A test constructs [`Spawning::refusing_to_spawn`]
/// so it can exercise `describe` and the approval path without running
/// `apt-get`, which is not a command this repo's suite can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpawnPolicy {
    /// Really spawn the child processes.
    Execute,
    /// Never spawn. `perform` reports the action as not automatable here
    /// rather than claiming an install that never ran.
    Refuse,
}

/// Whether a step needs the user to say yes.
///
/// `--yes` is not a convenience here. `deps-check.yml` pins `--fix --yes` on
/// two CI legs, and a binary that reads stdin there hangs rather than fails,
/// so [`Approval::Assumed`] must never reach a read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Approval {
    /// `--yes` was passed. Never read stdin.
    Assumed,
    /// Ask before each install, and treat anything but `y` as a refusal.
    Ask,
}

/// The [`Installer`] that turns an action into spawned argv.
///
/// Holds the manager because [`InstallAction::Package`] names a package and
/// not the program that installs it: `plan` resolves the manager once per
/// run, and re-deriving it per action would let two steps in one run
/// disagree about which machine they are on.
pub struct Spawning {
    manager: PackageManager,
    approval: Approval,
    policy: SpawnPolicy,
}

impl Spawning {
    /// An installer that really runs commands.
    #[must_use]
    pub fn executing(manager: PackageManager, approval: Approval) -> Self {
        Spawning { manager, approval, policy: SpawnPolicy::Execute }
    }

    /// An installer that never spawns, for tests.
    ///
    /// `perform` reports [`StepOutcome::NotAutomatable`] rather than
    /// [`StepOutcome::Installed`], because claiming an install that never ran
    /// is the false statement this whole design exists to prevent.
    #[must_use]
    pub fn refusing_to_spawn(manager: PackageManager) -> Self {
        Spawning { manager, approval: Approval::Assumed, policy: SpawnPolicy::Refuse }
    }

    /// Ask the user whether to install, unless `--yes` already answered.
    ///
    /// Returns false on anything but `y` or `Y`, matching
    /// `retired-check-deps:576-580`. A closed stdin, or a stdin that is not a
    /// terminal, reads end-of-file and is a refusal rather than a hang.
    fn approved(&self, summary: &str) -> bool {
        if self.approval == Approval::Assumed {
            return true;
        }
        if !std::io::stdin().is_terminal() {
            return false;
        }
        print!("  install {summary}? [y/N] ");
        if std::io::stdout().flush().is_err() {
            return false;
        }
        let mut reply = String::new();
        if std::io::stdin().read_line(&mut reply).is_err() {
            return false;
        }
        matches!(reply.trim(), "y" | "Y")
    }
}

impl Installer for Spawning {
    fn describe(&self, step: &Step) -> ActionDescription {
        let sequence = argv_sequence_for(&step.action, self.manager, step.privilege);
        let preview =
            if sequence.is_empty() { None } else { Some(render_sequence(&sequence)) };
        ActionDescription {
            summary: summarize(&step.action),
            // The step's privilege, never a guess. Reading it from the step
            // is what lets a dry run disclose every privileged step before
            // the first password prompt.
            privilege: step.privilege,
            command_preview: preview,
            changes_trust_root: matches!(step.action, InstallAction::AptSource { .. }),
        }
    }

    fn perform(&self, action: &InstallAction) -> StepOutcome {
        if let InstallAction::NotAutomatable { reason } = action {
            return StepOutcome::NotAutomatable { reason: reason.clone() };
        }

        // `perform` takes the action rather than the step, so the privilege
        // is re-derived from the manager here. That is the same rule `plan`
        // applied and not a second opinion: brew never elevates, and every
        // other manager's own `needs_root` decides.
        let privilege = if self.manager.needs_root() && needs_manager_privilege(action) {
            PrivilegeRequirement::Root
        } else {
            PrivilegeRequirement::None
        };
        let sequence = argv_sequence_for(action, self.manager, privilege);
        if sequence.is_empty() {
            return StepOutcome::NotAutomatable {
                reason: NoInstallReason::ManagerNotNamedInManifest { manager: self.manager },
            };
        }

        if !self.approved(&summarize(action)) {
            return StepOutcome::Declined;
        }

        if self.policy == SpawnPolicy::Refuse {
            return StepOutcome::NotAutomatable {
                reason: NoInstallReason::RequiresInteractiveApproval,
            };
        }

        let environment = child_environment_for(self.manager);
        for argv in &sequence {
            if let Some(cause) = run_one(argv, &environment) {
                return StepOutcome::InstallFailed { action: action.clone(), cause };
            }
        }
        StepOutcome::Installed
    }
}

/// Whether this action installs through the manager, so the manager's own
/// privilege rule applies.
///
/// A pip install, a clone and an nvm install all land in a user-owned
/// directory whatever manager the machine has, so none of them elevates.
fn needs_manager_privilege(action: &InstallAction) -> bool {
    matches!(action, InstallAction::Package { .. } | InstallAction::AptSource { .. })
}

/// Spawn one argv and report why it failed, or `None` on success.
///
/// Builds [`ExecFailure`] from the real output. `Output::status.code()` is
/// `None` for a signal kill, which is not a nonzero exit and must not be
/// reported as one, and `Command::output` yields bytes with no UTF-8
/// guarantee while [`BoundedText::truncating`] takes `&str`.
fn run_one(argv: &[OsString], environment: &BTreeMap<OsString, OsString>) -> Option<ExecFailure> {
    let (program, arguments) = argv.split_first()?;
    let mut command = Command::new(program);
    command.args(arguments);

    // The same widened search path the checks use, so a step that calls a
    // tool an earlier step just installed can find it. `nvm` installs node
    // through a script that expects nvm's own directory to be reachable, and
    // the fixpoint's later waves run after the installs of earlier ones.
    if let Ok(joined) = std::env::join_paths(super::gather::search_path()) {
        command.env("PATH", joined);
    }

    for (key, value) in environment {
        command.env(key, value);
    }

    let output = match command.output() {
        Ok(output) => output,
        Err(error) => return Some(ExecFailure::Spawn(spawn_error(&error))),
    };
    if output.status.success() {
        return None;
    }
    Some(exec_failure(program, &output))
}

/// Classify a spawn failure into the core's closed set.
fn spawn_error(error: &std::io::Error) -> SpawnError {
    match error.kind() {
        std::io::ErrorKind::NotFound => SpawnError::NotFound,
        std::io::ErrorKind::PermissionDenied => SpawnError::PermissionDenied,
        _ => SpawnError::Other,
    }
}

/// Turn a failed [`Output`] into the failure the core reports.
///
/// A `sudo` that exited 1 with nothing on stdout is the authentication
/// refusal `ExecFailure::AuthenticationRefused` exists for: knowing it at
/// step 1 of 8 rather than at step 8 is the whole reason
/// `Elevation::ViaSudo` is documented as a prediction rather than a
/// guarantee.
fn exec_failure(program: &OsStr, output: &Output) -> ExecFailure {
    let stderr = String::from_utf8_lossy(&output.stderr);
    if program == OsStr::new("sudo") && looks_like_refused_authentication(&stderr) {
        return ExecFailure::AuthenticationRefused;
    }
    match output.status.code() {
        Some(code) => {
            ExecFailure::NonZeroExit { code, stderr: BoundedText::truncating(&stderr) }
        }
        // No code means a signal killed the process. Reporting that as
        // `NonZeroExit { code: -1 }` would invent an exit status the process
        // never reported, so it is a spawn-side failure instead.
        None => ExecFailure::Spawn(SpawnError::Other),
    }
}

/// Whether sudo's own diagnostic says the user may not escalate.
///
/// A text match, which is the only signal sudo gives: it exits 1 for both a
/// refused password and a command that failed. Matching on the diagnostic
/// rather than the code keeps a genuine install failure from being reported
/// as an authentication problem.
fn looks_like_refused_authentication(stderr: &str) -> bool {
    let lowered = stderr.to_lowercase();
    lowered.contains("incorrect password")
        || lowered.contains("is not in the sudoers file")
        || lowered.contains("no askpass program")
        || lowered.contains("a terminal is required")
}

/// A one-line rendering of an action, for display.
fn summarize(action: &InstallAction) -> String {
    match action {
        InstallAction::Package { id } => format!("package {id}"),
        InstallAction::Brew { kind, id, tap } => {
            let shape = match kind {
                BrewKind::Formula => "formula",
                BrewKind::Cask => "cask",
            };
            match tap {
                Some(tap_name) => format!("brew {shape} {id} from tap {tap_name}"),
                None => format!("brew {shape} {id}"),
            }
        }
        InstallAction::AptSource { .. } => {
            "gh, after adding a third-party APT trust root".to_string()
        }
        InstallAction::Pip { id, break_system_packages: true } => {
            format!("pip {id}, overriding PEP 668")
        }
        InstallAction::Pip { id, break_system_packages: false } => format!("pip {id}"),
        InstallAction::Script { installer } => format!("script {}", script_url(*installer)),
        InstallAction::GitClone { source, .. } => format!("clone {}", clone_url(*source)),
        InstallAction::NvmInstall => "node through nvm".to_string(),
        InstallAction::NotAutomatable { .. } => "no automated install".to_string(),
    }
}

/// Wire the two installer slots for one run.
///
/// `privileged` is `None` when elevation is unavailable. That `None` is what
/// makes "this machine cannot install with root" a property of the wiring
/// rather than a string test on command text: `retired-check-deps:544` keyed the
/// same decision on the command containing `${SUDO}`, and emitting a command
/// anyway is what produced "sh: 1: sudo: not found" eleven times in a single
/// run.
#[must_use]
pub fn wire(
    manager: PackageManager,
    elevation: Elevation,
    approval: Approval,
) -> Installers<'static> {
    let privileged = match elevation {
        Elevation::AlreadyRoot | Elevation::ViaSudo => {
            Some(Box::new(Spawning::executing(manager, approval)) as Box<dyn Installer>)
        }
        Elevation::Unavailable => None,
    };
    Installers { ordinary: Box::new(Spawning::executing(manager, approval)), privileged }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deps_core::{CheckPath, DependencyName, TapName};
    use dotfiles_path::{CheckRelPath, PackageId};

    fn an_apt_action() -> InstallAction {
        InstallAction::Package { id: PackageId::parse("ripgrep").expect("a valid package id") }
    }

    fn a_pacman_action() -> InstallAction {
        InstallAction::Package { id: PackageId::parse("ripgrep").expect("a valid package id") }
    }

    fn a_clone_into(rest: &str) -> InstallAction {
        InstallAction::GitClone {
            source: CloneSource::Tpm,
            into: CheckPath::new(
                PathRoot::Home,
                CheckRelPath::parse(rest).expect("a valid relative path"),
            ),
        }
    }

    fn an_apt_step_needing_root() -> Step {
        Step {
            dependency: DependencyName::parse("ripgrep").expect("a valid dependency name"),
            action: an_apt_action(),
            privilege: PrivilegeRequirement::Root,
        }
    }

    /// The apt argv carries -y, and the child environment carries
    /// DEBIAN_FRONTEND, with the ambient variable UNSET.
    ///
    /// This is the repo's sharpest incident: an unattended bootstrap halted
    /// at tzdata's debconf prompt while every image and CI leg passed,
    /// because Dockerfile.ubuntu set the variable itself. The environment was
    /// compensating for a gap in the engine, so the engine looked correct
    /// everywhere it was tested and failed on a real machine. A test that
    /// inherits the variable proves nothing, which is the whole lesson.
    #[test]
    fn the_apt_child_environment_carries_debian_frontend_with_the_ambient_unset() {
        assert!(
            std::env::var_os("DEBIAN_FRONTEND").is_none(),
            "this test must run with DEBIAN_FRONTEND unset; something set it"
        );

        let environment = child_environment_for(PackageManager::Apt);

        assert_eq!(
            environment.get(OsStr::new("DEBIAN_FRONTEND")).map(OsString::as_os_str),
            Some(OsStr::new("noninteractive")),
            "the engine sets it, not the image"
        );
    }

    /// Every apt install argv carries -y, and every pacman argv carries
    /// --noconfirm.
    #[test]
    fn non_interactivity_flags_live_in_the_argv() {
        let apt = argv_for(&an_apt_action(), PackageManager::Apt, PrivilegeRequirement::Root);
        let pacman =
            argv_for(&a_pacman_action(), PackageManager::Pacman, PrivilegeRequirement::Root);

        assert!(!apt.is_empty() && !pacman.is_empty(), "the control builds real argv");
        assert!(apt.iter().any(|word| word == "-y"), "apt needs -y: {apt:?}");
        assert!(
            pacman.iter().any(|word| word == "--noconfirm"),
            "pacman needs --noconfirm: {pacman:?}"
        );
    }

    /// The privilege prefix is never interpolated into a command word.
    #[test]
    fn privilege_is_a_separate_word_never_a_string_prefix() {
        let elevated =
            argv_for(&an_apt_action(), PackageManager::Apt, PrivilegeRequirement::Root);

        assert_eq!(
            elevated.first().map(OsString::as_os_str),
            Some(OsStr::new("sudo")),
            "root means sudo is its own argv word"
        );
        assert!(
            !elevated.iter().any(|word| word.to_string_lossy().contains("${SUDO}")),
            "no word may carry an uninterpolated prefix: {elevated:?}"
        );
        assert!(
            !elevated.iter().any(|word| word.to_string_lossy().contains("sudo apt-get")),
            "sudo must not be glued to the program: {elevated:?}"
        );
    }

    /// A path with a space survives as one argv word.
    #[test]
    fn a_name_with_a_space_stays_one_word() {
        let action = a_clone_into("weird dir/tpm");

        let argv = argv_for(&action, PackageManager::Apt, PrivilegeRequirement::None);

        assert!(
            argv.iter().any(|word| word.to_string_lossy().ends_with("weird dir/tpm")),
            "the destination is one word, not two: {argv:?}"
        );
    }

    /// describe and perform read the same vector.
    #[test]
    fn describe_renders_the_argv_perform_would_spawn() {
        let step = an_apt_step_needing_root();
        let installer = Spawning::refusing_to_spawn(PackageManager::Apt);

        let described = installer.describe(&step);

        assert_eq!(
            described.privilege,
            PrivilegeRequirement::Root,
            "describe reports the step's privilege, not a guess"
        );
        // The whole sequence, not just the install: `perform` spawns every
        // command in it, so a preview showing only the last one would hide
        // an `apt-get update` the reader is about to run as root.
        let expected = argv_sequence_for(&step.action, PackageManager::Apt, step.privilege);
        let preview = described.command_preview.expect("a spawnable action previews its argv");
        assert_eq!(preview, render_sequence(&expected), "the preview IS the argv");
        assert_eq!(
            expected.last().map(Vec::as_slice),
            Some(argv_for(&step.action, PackageManager::Apt, step.privilege).as_slice()),
            "argv_for is the install command of the same sequence"
        );
    }

    /// A plain formula is `brew install <id>`, with no tap and no --cask.
    #[test]
    fn a_brew_formula_installs_without_a_tap() {
        let action = InstallAction::Brew {
            kind: BrewKind::Formula,
            id: PackageId::parse("ripgrep").expect("a valid package id"),
            tap: None,
        };

        let sequence = argv_sequence_for(&action, PackageManager::Brew, PrivilegeRequirement::None);

        assert_eq!(sequence.len(), 1, "a core formula is one command: {sequence:?}");
        assert_eq!(sequence[0], words(["brew", "install", "ripgrep"]));
    }

    /// A cask in a third-party tap taps first, then installs with --cask.
    ///
    /// `Named` cannot express either half, so the default
    /// `brew install aerospace` failed twice over: "No available formula with
    /// the name aerospace" because it is a cask, and an untapped third-party
    /// cask is not findable even with --cask
    /// (`retired-check-deps:265-281`). `brew tap` is idempotent, so re-running
    /// costs a no-op.
    #[test]
    fn a_cask_in_a_tap_taps_before_it_installs() {
        let action = InstallAction::Brew {
            kind: BrewKind::Cask,
            id: PackageId::parse("aerospace").expect("a valid package id"),
            tap: Some(TapName::parse("nikitabobko/tap").expect("a valid tap name")),
        };

        let sequence = argv_sequence_for(&action, PackageManager::Brew, PrivilegeRequirement::None);

        assert_eq!(sequence[0], words(["brew", "tap", "nikitabobko/tap"]));
        assert_eq!(sequence[2], words(["brew", "install", "--cask", "aerospace"]));
        assert!(
            sequence.iter().flatten().all(|word| word != "sudo"),
            "brew refuses to run as root, so no word may be sudo: {sequence:?}"
        );
    }

    /// No privilege means no `sudo` word at all.
    ///
    /// The positive control for the absence: the same action at
    /// `PrivilegeRequirement::Root` does carry one, so this assertion is
    /// about the privilege and not about an argv that happens to be empty.
    #[test]
    fn an_unprivileged_step_carries_no_sudo_word() {
        let elevated =
            argv_for(&an_apt_action(), PackageManager::Apt, PrivilegeRequirement::Root);
        assert!(elevated.iter().any(|word| word == "sudo"), "the control elevates");

        let plain = argv_for(&an_apt_action(), PackageManager::Apt, PrivilegeRequirement::None);

        assert!(!plain.is_empty(), "the control builds real argv");
        assert!(!plain.iter().any(|word| word == "sudo"), "no elevation, no word: {plain:?}");
    }

    /// A fetched installer script is written to a file and then run from it.
    ///
    /// The defect this pins: the retired shell wrote `curl ... | sh`, and the
    /// port kept both commands but dropped the pipe, because an effect here
    /// is argv and a pipe is not expressible as argv. `curl` then wrote to a
    /// discarded stdout and `sh -s --` read an empty stdin, so the step exited
    /// 0 having installed nothing and the check failed afterwards.
    ///
    /// Every existing test asserted the planned argv, and each argv was
    /// individually correct, so none of them could see it. This one asserts
    /// the two argvs are CONNECTED: the path curl writes is the path sh runs.
    #[test]
    fn a_fetched_installer_is_written_to_a_file_and_run_from_it() {
        for installer in
            [ScriptInstaller::Rustup, ScriptInstaller::OhMyZsh, ScriptInstaller::Zoxide]
        {
            let sequence = script_argv(installer);
            assert_eq!(sequence.len(), 2, "a fetch and a run: {sequence:?}");

            let fetch = &sequence[0];
            let run = &sequence[1];

            // The fetch names an output file rather than writing to stdout.
            let output_flag = fetch
                .iter()
                .position(|word| word == "-o")
                .unwrap_or_else(|| panic!("the fetch writes to a file: {fetch:?}"));
            let written = fetch
                .get(output_flag + 1)
                .unwrap_or_else(|| panic!("-o takes a path: {fetch:?}"));

            // The run executes that same file. This is the connection the
            // dropped pipe severed.
            assert_eq!(
                run.get(1),
                Some(written),
                "sh runs the file curl wrote: {run:?} against {fetch:?}"
            );

            // And it must not ask for stdin, which is what no longer arrives.
            assert!(
                !run.iter().any(|word| word == "-s"),
                "no -s, which reads the script from stdin: {run:?}"
            );
        }
    }

    /// Every write into `/etc` in the gh apt pipeline carries the privilege.
    ///
    /// Found by the bootstrap container, which is the first environment that
    /// runs as a genuine non-root user with gh absent. The shell wrote the
    /// keyring with an unprivileged redirect after an elevated `mkdir`, so
    /// the directory was created and the write into it was refused.
    ///
    /// The positive control is the same pipeline at
    /// [`PrivilegeRequirement::None`], which must carry no `sudo` at all: it
    /// proves this assertion reads the privilege rather than a constant.
    #[test]
    fn every_etc_write_in_the_gh_pipeline_is_privileged() {
        let action = InstallAction::AptSource {
            keyring: KeyringSource::GithubCli,
            list: SourceListEntry::GithubCli,
        };

        let elevated_argvs =
            argv_sequence_for(&action, PackageManager::Apt, PrivilegeRequirement::Root);
        let touches_etc: Vec<&Vec<OsString>> = elevated_argvs
            .iter()
            .filter(|argv| argv.iter().any(|word| word.to_string_lossy().contains("/etc/")))
            .collect();

        assert!(!touches_etc.is_empty(), "the control finds writes into /etc");
        for argv in &touches_etc {
            assert_eq!(
                argv.first().map(|word| word.to_string_lossy().into_owned()),
                Some(String::from("sudo")),
                "an unprivileged write into /etc: {argv:?}"
            );
        }

        let plain = argv_sequence_for(&action, PackageManager::Apt, PrivilegeRequirement::None);
        assert!(
            !plain.iter().any(|argv| argv.iter().any(|word| word == "sudo")),
            "no elevation, no word: {plain:?}"
        );
    }

    /// The oh-my-zsh installer never replaces the tracked `.zshrc`.
    ///
    /// Its installer overwrites `~/.zshrc` with its own template and moves
    /// the real one aside, so a bootstrap that omits `--keep-zshrc` silently
    /// discards the shell configuration the whole repository exists to ship.
    /// `--unattended` is the other half: without it the installer starts an
    /// interactive shell and a bootstrap with no terminal hangs.
    ///
    /// The positive control is rustup, which takes neither flag: it proves
    /// this reads the installer rather than asserting a constant.
    #[test]
    fn the_oh_my_zsh_installer_keeps_the_zshrc_and_never_prompts() {
        let oh_my_zsh = script_argv(ScriptInstaller::OhMyZsh);
        let words: Vec<String> = oh_my_zsh
            .iter()
            .flatten()
            .map(|word| word.to_string_lossy().into_owned())
            .collect();

        assert!(words.iter().any(|word| word == "--keep-zshrc"), "{words:?}");
        assert!(words.iter().any(|word| word == "--unattended"), "{words:?}");

        let rustup: Vec<String> = script_argv(ScriptInstaller::Rustup)
            .iter()
            .flatten()
            .map(|word| word.to_string_lossy().into_owned())
            .collect();
        assert!(
            !rustup.iter().any(|word| word == "--keep-zshrc"),
            "the control carries neither flag: {rustup:?}"
        );
    }

    /// An action with no automated install produces no argv and no preview.
    ///
    /// The positive control comes first: an action that does have an install
    /// previews one, so an empty preview below is the action's emptiness and
    /// not a `describe` that previews nothing at all.
    #[test]
    fn an_unautomatable_action_previews_nothing() {
        let installer = Spawning::refusing_to_spawn(PackageManager::Apt);
        let installable = installer.describe(&an_apt_step_needing_root());
        assert!(installable.command_preview.is_some(), "the control previews its argv");

        let step = Step {
            dependency: DependencyName::parse("nvm").expect("a valid dependency name"),
            action: InstallAction::NotAutomatable {
                reason: NoInstallReason::UpstreamPublishesNoStableUrl,
            },
            privilege: PrivilegeRequirement::None,
        };

        let described = installer.describe(&step);

        assert_eq!(described.command_preview, None);
        assert!(argv_for(&step.action, PackageManager::Apt, PrivilegeRequirement::None).is_empty());
    }

    /// `perform` never reports an outcome that is `reconcile`'s to produce.
    ///
    /// `AlreadyPresent` and `InstalledButCheckStillFails` come from the
    /// post-loop world. An installer returning either would bypass the
    /// re-check that catches an install reporting success while changing
    /// nothing.
    #[test]
    fn perform_returns_only_the_outcomes_it_owns() {
        let installer = Spawning::refusing_to_spawn(PackageManager::Apt);

        let outcome = installer.perform(&an_apt_action());

        assert!(
            matches!(
                outcome,
                StepOutcome::Installed
                    | StepOutcome::InstallFailed { .. }
                    | StepOutcome::NotAutomatable { .. }
                    | StepOutcome::Declined
            ),
            "perform produced an outcome reconcile owns: {outcome:?}"
        );
    }

    /// A manual-only action reports the reason it carries, unchanged.
    #[test]
    fn a_manual_only_action_reports_its_own_reason() {
        let installer = Spawning::refusing_to_spawn(PackageManager::Apt);
        let action = InstallAction::NotAutomatable {
            reason: NoInstallReason::UpstreamPublishesNoStableUrl,
        };

        assert_eq!(
            installer.perform(&action),
            StepOutcome::NotAutomatable {
                reason: NoInstallReason::UpstreamPublishesNoStableUrl
            }
        );
    }

    /// `--yes` never reads stdin.
    ///
    /// `deps-check.yml` pins `--fix --yes` on two CI legs, and a binary that
    /// reads stdin there hangs rather than fails. Asserted through the
    /// approval decision rather than through a spawn, because the suite
    /// cannot run `apt-get`.
    #[test]
    fn assumed_approval_does_not_consult_stdin() {
        let assumed = Spawning::executing(PackageManager::Apt, Approval::Assumed);
        let asking = Spawning::executing(PackageManager::Apt, Approval::Ask);

        assert!(assumed.approved("package ripgrep"), "--yes approves without asking");
        // The control: with Ask and a non-terminal stdin, which is what
        // `cargo test` gives, the same call refuses rather than blocking.
        assert!(!asking.approved("package ripgrep"), "a non-terminal stdin is a refusal");
    }

    /// Unavailable elevation wires no privileged installer.
    ///
    /// That `None` is what makes "this machine cannot install with root" a
    /// property of the wiring rather than a string test on command text.
    #[test]
    fn no_elevation_wires_no_privileged_installer() {
        // Positive control: a machine that can escalate does get one, so the
        // `is_none` below is about the elevation and not about `wire`
        // returning `None` always.
        let escalating = wire(PackageManager::Apt, Elevation::ViaSudo, Approval::Assumed);
        assert!(escalating.privileged.is_some(), "sudo on PATH wires a privileged slot");

        let stranded = wire(PackageManager::Apt, Elevation::Unavailable, Approval::Assumed);

        assert!(stranded.privileged.is_none());
    }

    /// The rendering quotes a word containing a space, so a reader can tell
    /// one word from two. Nothing re-parses this text.
    #[test]
    fn the_rendering_shows_a_spaced_word_as_one_word() {
        let argv = vec![OsString::from("git"), OsString::from("weird dir/tpm")];

        assert_eq!(render_argv(&argv), "git 'weird dir/tpm'");
    }
}
