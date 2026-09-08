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
    TarballRelease,
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
        // Pinned to a tag, unlike its three neighbours. nvm publishes no
        // moving install URL: its README's own command names a version, and
        // that is what made this dependency manual-only. Bumping this
        // constant is a deliberate, reviewable edit.
        ScriptInstaller::Nvm => {
            "https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.7/install.sh"
        }
    }
}

/// The release tag each pinned tarball names.
///
/// Bumping this is a deliberate, reviewable edit, matching the rule
/// `ScriptInstaller::Nvm` already states: a pin goes stale on purpose
/// rather than by accident.
fn tarball_tag(release: TarballRelease) -> &'static str {
    match release {
        TarballRelease::Neovim => "v0.12.5",
    }
}

/// The asset name for a release on this machine's architecture.
///
/// Two architectures because both are real here: the Linux machines are
/// x86_64 and upstream also publishes arm64, which is what an arm64
/// container or an ARM server needs. An unrecognized architecture is
/// `None`, which the caller turns into "no automated install" rather than
/// guessing an asset that would 404 halfway through a bootstrap.
fn tarball_asset(release: TarballRelease) -> Option<&'static str> {
    match (release, std::env::consts::ARCH) {
        (TarballRelease::Neovim, "x86_64") => Some("nvim-linux-x86_64.tar.gz"),
        (TarballRelease::Neovim, "aarch64") => Some("nvim-linux-arm64.tar.gz"),
        (TarballRelease::Neovim, _) => None,
    }
}

/// The full download URL for a pinned release on this architecture.
fn tarball_url(release: TarballRelease) -> Option<String> {
    let project = match release {
        TarballRelease::Neovim => "neovim/neovim",
    };
    let asset = tarball_asset(release)?;
    Some(format!(
        "https://github.com/{project}/releases/download/{}/{asset}",
        tarball_tag(release)
    ))
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
    elevation: Elevation,
) -> Vec<Vec<OsString>> {
    match action {
        InstallAction::Package { id } => match manager {
            // `-qq` on update and `-y` on install, matching
            // `retired-check-deps:435`. The `-y` is the argv half of the same
            // rule DEBIAN_FRONTEND is the environment half of: a question
            // apt would otherwise ask an unattended run.
            PackageManager::Apt => vec![
                elevated(privilege, elevation, ["apt-get", "update", "-qq"]),
                elevated_with(privilege, elevation, ["apt-get", "install", "-y"], [id.as_str()]),
            ],
            // `-Sy` syncs and installs in one command, so pacman is one
            // argv where apt is two (`retired-check-deps:437`).
            PackageManager::Pacman => {
                vec![elevated_with(
                    privilege,
                    elevation,
                    ["pacman", "-Sy", "--noconfirm"],
                    [id.as_str()],
                )]
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
            elevated(privilege, elevation, ["apt-get", "update", "-qq"]),
            elevated(privilege, elevation, ["apt-get", "install", "-y", "curl"]),
            elevated(privilege, elevation, ["mkdir", "-p", "-m", "755", "/etc/apt/keyrings"]),
            elevated(
                privilege,
                elevation,
                ["curl", "-fsSL", "-o", keyring_path(*keyring), keyring_url(*keyring)],
            ),
            elevated(privilege, elevation, ["chmod", "go+r", keyring_path(*keyring)]),
            elevated_with(privilege, elevation, ["tee", source_list_path(*list)], []),
            elevated(privilege, elevation, ["apt-get", "update", "-qq"]),
            elevated(privilege, elevation, ["apt-get", "install", "-y", "gh"]),
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
            // NVM_DIR is set explicitly, not left to nvm.sh's own default.
            // The script locates itself through $BASH_SOURCE, which dash does
            // not set, so under /bin/sh on a Debian image NVM_DIR resolved to
            // "/" and `nvm install --lts` wrote node to //versions/node while
            // the check read $HOME/.nvm/versions/node. The install printed
            // "Now using node v24.20.0" and the step still reported "the
            // install succeeded and the check still fails".
            //
            // Still one fixed string with no interpolated data: NvmInstall
            // carries no payload, so no manifest text reaches this word
            // (`retired-check-deps:387`).
            vec![words([
                "sh",
                "-c",
                "NVM_DIR=\"$HOME/.nvm\"; export NVM_DIR; \
                 . \"$NVM_DIR/nvm.sh\" && nvm install --lts",
            ])]
        }
        // Fetched, unpacked, and linked as separate spawns rather than one
        // `sh -c` pipeline. The pipeline spelling would put a URL and two
        // paths into a shell word, which is the interpolation spec 3.6
        // closes; these are argv vectors with no shell between them.
        //
        // THE WHOLE TREE IS INSTALLED, not the executable alone. This
        // sequence used to end with `cp <staging>/bin/nvim ~/.local/bin/nvim`
        // and discard everything else the tarball carried. Neovim locates
        // $VIMRUNTIME by walking up from its own executable looking for
        // `share/nvim/runtime`, so the orphaned binary searched
        // `~/.local/share/nvim/runtime`, found nothing, and fell back to the
        // paths compiled in on upstream's build machine. Reproduced in
        // ubuntu:24.04: `VIMRUNTIME=/usr/local/share/nvim` (nonexistent),
        // `require 'nvim.spellfile'` failed, and every runtime lookup raised
        // `E484: Can't open file .../syntax/syntax.vim`.
        //
        // A VERSIONED PREFIX plus a symlink, rather than merging into
        // ~/.local directly. Three properties that shape buys:
        //   - the artifact tree stays separate from the state trees that
        //     live under ~/.local/share/nvim (lazy, mason, shada), so an
        //     uninstall or a version bump cannot delete a plugin directory;
        //   - a version bump is a symlink swap, and the old prefix stays
        //     until it is removed deliberately;
        //   - the executable and its `share/` are siblings by construction,
        //     so there is no second command that could forget one of them.
        //
        // Verified in ubuntu:24.04 that Neovim resolves the symlink BEFORE
        // the walk-up, so `~/.local/bin/nvim -> ~/.local/opt/nvim-<v>/bin/nvim`
        // yields `VIMRUNTIME=~/.local/opt/nvim-<v>/share/nvim/runtime`. The
        // link target is what makes ~/.local/bin work as the search path
        // entry `search_path` already prepends.
        //
        // `mv` of the staging directory rather than `cp -R` of its contents:
        // a rename is atomic on one filesystem, so a killed run cannot leave
        // a half-populated prefix that a later `DirExists` check would accept.
        // The staging directory is therefore created INSIDE the destination
        // root, not in /tmp, because a cross-device /tmp would silently
        // degrade the rename into a copy.
        InstallAction::ReleaseTarball { release } => {
            let Some(url) = tarball_url(*release) else {
                return Vec::new();
            };
            let staging_path = tarball_staging_dir(*release).to_string_lossy().into_owned();
            let prefix = tarball_prefix(*release);
            let binary = tarball_binary(*release);
            vec![
                // A stale prefix from an interrupted run must not be merged
                // with this one. Removing the staging path is safe because it
                // is this action's own scratch directory.
                words(["rm", "-rf", &staging_path]),
                words(["mkdir", "-p", &staging_path]),
                words(["curl", "-fsSL", "-o", &format!("{staging_path}/release.tar.gz"), &url]),
                words([
                    "tar",
                    "-xzf",
                    &format!("{staging_path}/release.tar.gz"),
                    "-C",
                    &staging_path,
                    "--strip-components=1",
                ]),
                words(["rm", "-f", &format!("{staging_path}/release.tar.gz")]),
                // Replacing an existing prefix for the same version, so a
                // re-run converges rather than failing on a non-empty target.
                words(["rm", "-rf", &prefix]),
                words(["mkdir", "-p", &tarball_prefix_parent(*release)]),
                words(["mv", &staging_path, &prefix]),
                words(["mkdir", "-p", &home_local_bin()]),
                // -s symbolic, -f replace an existing link, -n treat an
                // existing link to a directory as a file rather than
                // descending into it, which would nest the link one level
                // deeper on every re-run.
                words([
                    "ln",
                    "-sfn",
                    &format!("{prefix}/bin/{binary}"),
                    &format!("{}/{}", home_local_bin(), binary),
                ]),
            ]
        }
        InstallAction::NotAutomatable { .. } => Vec::new(),
    }
}

/// The binary a release tarball installs.
fn tarball_binary(release: TarballRelease) -> &'static str {
    match release {
        TarballRelease::Neovim => "nvim",
    }
}

/// Where a release tarball is unpacked before it is promoted to its prefix.
///
/// One directory per release, so two tarballs in the same run cannot
/// overwrite each other, matching `script_path`'s rule.
///
/// Deliberately NOT under `std::env::temp_dir()`. The promote step is a
/// `mv`, which is atomic only within one filesystem; on a machine where
/// /tmp is a separate mount (a tmpfs, a container volume) the rename would
/// degrade into a copy and a killed run could leave a partially populated
/// prefix. Staging beside the destination keeps the rename a rename.
fn tarball_staging_dir(release: TarballRelease) -> std::path::PathBuf {
    let name = match release {
        TarballRelease::Neovim => ".nvim-release-staging",
    };
    std::path::PathBuf::from(tarball_prefix_parent(release)).join(name)
}

/// The versioned prefix a release tarball is installed into.
///
/// Versioned so a bump leaves the previous tree in place: the symlink in
/// `~/.local/bin` is the only thing that moves, so a rollback is one
/// `ln -sfn` rather than a re-download.
fn tarball_prefix(release: TarballRelease) -> String {
    let name = match release {
        TarballRelease::Neovim => "nvim",
    };
    format!("{}/{name}-{}", tarball_prefix_parent(release), tarball_tag(release))
}

/// `~/.local/opt`, which holds the versioned prefixes.
///
/// Separate from `~/.local/share`, where Neovim keeps its own state (lazy,
/// mason, shada). Installing the artifact tree into the state tree would
/// mean a version bump or an uninstall could remove a plugin directory.
fn tarball_prefix_parent(_release: TarballRelease) -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/root"));
    format!("{home}/.local/opt")
}

/// `~/.local/bin`, the directory `search_path` prepends.
fn home_local_bin() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/root"));
    format!("{home}/.local/bin")
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
    elevation: Elevation,
) -> Vec<OsString> {
    argv_sequence_for(action, manager, privilege, elevation).pop().unwrap_or_default()
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
        ScriptInstaller::Nvm => "nvm-install.sh",
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
    //
    // The interpreter is per-installer because nvm's refuses to be anything
    // else. Its first lines test `BASH_VERSION` and exit 1 with "the install
    // instructions explicitly say to pipe the install script to `bash`",
    // and the bootstrap images run dash as sh, so a shared `sh` here would
    // fail on every Debian leg. Every base image this repo targets ships
    // bash, verified against debian:bookworm-slim, which is the slimmest.
    let interpreter = match installer {
        ScriptInstaller::Nvm => "bash",
        ScriptInstaller::Rustup | ScriptInstaller::OhMyZsh | ScriptInstaller::Zoxide => "sh",
    };
    let mut run = words([interpreter]);
    run.push(path);
    match installer {
        ScriptInstaller::Rustup => run.extend(words(["-y"])),
        ScriptInstaller::OhMyZsh => run.extend(words(["--unattended", "--keep-zshrc"])),
        // The installer takes no flags. It writes into $NVM_DIR, which
        // defaults to $HOME/.nvm -- the directory deps.toml checks.
        ScriptInstaller::Zoxide | ScriptInstaller::Nvm => {}
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

/// Whether a step's argv gets a literal `sudo` word.
///
/// Two independent facts decide it, and conflating them is what produced
/// "sudo: not found" eleven times in one run. [`PrivilegeRequirement`] is a
/// property of the STEP: this install writes outside the user's own
/// directories. [`Elevation`] is a property of the MACHINE: this is how the
/// process reaches root, if it can at all. `sudo` is the machine's answer,
/// so a step that needs root on a process already running AS root takes no
/// prefix -- there is nothing to escalate to, and on an image without sudo
/// the word is simply a command that does not exist.
fn needs_sudo_word(privilege: PrivilegeRequirement, elevation: Elevation) -> bool {
    privilege == PrivilegeRequirement::Root && elevation == Elevation::ViaSudo
}

/// Prepend `sudo` as its own word when the step needs root and the machine
/// reaches root through `sudo`.
///
/// Its own word, never glued to the program and never an uninterpolated
/// `${SUDO}` placeholder. `retired-check-deps:164-168` re-encoded that
/// decision as a string prefix carrying its own trailing space, and a caller
/// could then read a command whose privilege did not match its step.
fn elevated<const COUNT: usize>(
    privilege: PrivilegeRequirement,
    elevation: Elevation,
    parts: [&str; COUNT],
) -> Vec<OsString> {
    elevated_with(privilege, elevation, parts, [])
}

/// Prepend `sudo` when needed, then append dependency-supplied words.
///
/// `trailing` is separate from `parts` so every dependency-supplied string
/// becomes exactly one `OsString`, never split and never quoted. That is
/// what makes a package name or path containing a space representable.
fn elevated_with<const FIXED: usize, const EXTRA: usize>(
    privilege: PrivilegeRequirement,
    elevation: Elevation,
    parts: [&str; FIXED],
    trailing: [&str; EXTRA],
) -> Vec<OsString> {
    let mut argv = Vec::with_capacity(FIXED + EXTRA + 1);
    if needs_sudo_word(privilege, elevation) {
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
    elevation: Elevation,
    approval: Approval,
    policy: SpawnPolicy,
}

impl Spawning {
    /// An installer that really runs commands.
    #[must_use]
    pub fn executing(
        manager: PackageManager,
        elevation: Elevation,
        approval: Approval,
    ) -> Self {
        Spawning { manager, elevation, approval, policy: SpawnPolicy::Execute }
    }

    /// An installer that never spawns, for tests.
    ///
    /// `perform` reports [`StepOutcome::NotAutomatable`] rather than
    /// [`StepOutcome::Installed`], because claiming an install that never ran
    /// is the false statement this whole design exists to prevent.
    ///
    /// Defaults to [`Elevation::ViaSudo`], which is the shape most existing
    /// tests assert: a non-root machine that reaches root through `sudo`.
    #[must_use]
    pub fn refusing_to_spawn(manager: PackageManager) -> Self {
        Spawning {
            manager,
            elevation: Elevation::ViaSudo,
            approval: Approval::Assumed,
            policy: SpawnPolicy::Refuse,
        }
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
        let sequence =
            argv_sequence_for(&step.action, self.manager, step.privilege, self.elevation);
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
        let sequence = argv_sequence_for(action, self.manager, privilege, self.elevation);
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
///
/// Both streams are captured because a failing script picks either one.
/// oh-my-zsh's installer explains itself on stdout and exits 1 with zero
/// bytes on stderr, so reading stderr alone reported "no output" while the
/// explanation sat in the same `Output`.
fn exec_failure(program: &OsStr, output: &Output) -> ExecFailure {
    let stderr = String::from_utf8_lossy(&output.stderr);
    if program == OsStr::new("sudo") && looks_like_refused_authentication(&stderr) {
        return ExecFailure::AuthenticationRefused;
    }
    match output.status.code() {
        Some(code) => ExecFailure::NonZeroExit {
            code,
            stdout: BoundedText::truncating(&String::from_utf8_lossy(&output.stdout)),
            stderr: BoundedText::truncating(&stderr),
        },
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
        InstallAction::ReleaseTarball { release } => match tarball_url(*release) {
            Some(url) => format!("{} from the pinned release tarball {url}", tarball_binary(*release)),
            None => format!(
                "{}: no release tarball for this architecture",
                tarball_binary(*release)
            ),
        },
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
///
/// The elevation reaches the installer itself, not only this match. Deciding
/// only WHETHER a privileged installer exists leaves HOW it escalates
/// unanswered, and the argv builder then has to guess. It guessed `sudo`
/// unconditionally, which reproduced the same eleven failures on a root
/// image with no sudo -- the exact shape `Dockerfile.bootstrap-bare` exists
/// to catch.
#[must_use]
pub fn wire(
    manager: PackageManager,
    elevation: Elevation,
    approval: Approval,
) -> Installers<'static> {
    let privileged = match elevation {
        Elevation::AlreadyRoot | Elevation::ViaSudo => {
            Some(Box::new(Spawning::executing(manager, elevation, approval))
                as Box<dyn Installer>)
        }
        Elevation::Unavailable => None,
    };
    Installers {
        ordinary: Box::new(Spawning::executing(manager, elevation, approval)),
        privileged,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deps_core::{CheckPath, DependencyName, TapName};
    use dotfiles_path::{CheckRelPath, PackageId};

    /// A failed child's stdout reaches the failure value, not only its stderr.
    ///
    /// The 2026-09-08 incident lived HERE, not in the renderer: oh-my-zsh's
    /// installer wrote its explanation to stdout, `exec_failure` read only
    /// `output.stderr`, and the sentence was dropped at the capture site. A
    /// renderer that handles stdout perfectly reports nothing if this
    /// function never passes it on, which is why this test exists beside
    /// the render-side ones rather than instead of them.
    #[test]
    fn a_failed_child_carries_both_of_its_streams() {
        let output = Output {
            status: exit_status_of(1),
            stdout: b"Zsh is not installed. Please install zsh first.\n".to_vec(),
            stderr: Vec::new(),
        };

        let failure = exec_failure(OsStr::new("sh"), &output);

        match failure {
            ExecFailure::NonZeroExit { code, stdout, stderr } => {
                assert_eq!(code, 1, "the reported code is the child's own");
                assert!(
                    stdout.as_str().contains("Zsh is not installed"),
                    "the capture site must keep stdout: {stdout}"
                );
                assert!(
                    stderr.as_str().trim().is_empty(),
                    "this child wrote nothing to stderr, and inventing text \
                     there would misdirect a reader: {stderr}"
                );
            }
            other => panic!("a nonzero exit must classify as NonZeroExit, got {other:?}"),
        }
    }

    /// Positive control for the test above: stderr still arrives.
    ///
    /// Without this, an `exec_failure` that swapped the two fields would
    /// pass the stdout assertion while breaking every apt failure message.
    #[test]
    fn a_failed_child_that_writes_to_stderr_keeps_it_there() {
        let output = Output {
            status: exit_status_of(100),
            stdout: b"Reading package lists...\n".to_vec(),
            stderr: b"E: Unable to locate package ripgrep\n".to_vec(),
        };

        match exec_failure(OsStr::new("apt-get"), &output) {
            ExecFailure::NonZeroExit { stdout, stderr, .. } => {
                assert!(
                    stderr.as_str().contains("E: Unable to locate package"),
                    "stderr must not be displaced by the stdout field: {stderr}"
                );
                assert!(
                    stdout.as_str().contains("Reading package lists"),
                    "both streams are carried, so progress chatter is kept \
                     for the case where it is all there is: {stdout}"
                );
            }
            other => panic!("a nonzero exit must classify as NonZeroExit, got {other:?}"),
        }
    }

    /// An `ExitStatus` reporting the given code, built without spawning.
    ///
    /// `ExitStatus` has no public constructor, so this goes through the
    /// unix extension trait. The suite runs on macOS and Linux only, both
    /// unix, so there is no second branch to write.
    #[cfg(unix)]
    fn exit_status_of(code: i32) -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt as _;
        std::process::ExitStatus::from_raw(code << 8)
    }

    /// The tarball install must keep the runtime tree with the binary.
    ///
    /// THE BUG THIS PINS, reported 2026-09-08 and reproduced in
    /// ubuntu:24.04. The sequence copied `bin/nvim` out of the staging
    /// directory and left `share/nvim/runtime/` behind, where the next run's
    /// `mkdir -p` or a reboot removed it. Neovim locates $VIMRUNTIME by
    /// walking up from its own executable looking for `share/nvim/runtime`,
    /// so an orphaned `~/.local/bin/nvim` searched
    /// `~/.local/share/nvim/runtime`, found nothing, and fell back to the
    /// paths compiled in on upstream's build machine. Measured on the broken
    /// install: `VIMRUNTIME=/usr/local/share/nvim` (nonexistent),
    /// `require 'nvim.spellfile'` failed, and every runtime file lookup
    /// raised `E484: Can't open file .../syntax/syntax.vim`.
    ///
    /// Asserted as "no command copies the executable ALONE" rather than as a
    /// literal expected sequence. A literal-sequence assertion would need
    /// rewriting for any change to staging paths, and the invariant that
    /// actually matters is relational: the executable and `share/` must
    /// arrive at the same prefix, so nothing may move one without the other.
    #[test]
    fn the_tarball_install_never_relocates_the_binary_alone() {
        let sequence = argv_sequence_for(
            &InstallAction::ReleaseTarball { release: TarballRelease::Neovim },
            PackageManager::Apt,
            PrivilegeRequirement::None,
            Elevation::ViaSudo,
        );

        assert!(!sequence.is_empty(), "the control: this action must plan commands");

        // A command that names the executable as its SOURCE and a directory
        // that is not the prefix as its destination is the defect. The
        // executable may only move as part of its whole tree.
        for command in &sequence {
            let words: Vec<String> = command
                .iter()
                .map(|word| word.to_string_lossy().into_owned())
                .collect();
            let is_copy = words.first().is_some_and(|first| first == "cp" || first == "mv");
            if !is_copy {
                continue;
            }
            let copies_the_bare_executable = words
                .iter()
                .any(|word| word.ends_with("/bin/nvim") || word.ends_with("/nvim"));
            let copies_a_tree = words.iter().any(|word| {
                word == "-R" || word == "-r" || word == "-a" || word.ends_with("/share")
            });
            let relocates_the_executable_alone = copies_the_bare_executable && !copies_a_tree;
            assert!(
                !relocates_the_executable_alone,
                "this command relocates the executable without its runtime tree, \
                 which is the $VIMRUNTIME bug: {words:?}"
            );
        }
    }

    /// The installed prefix keeps `bin` and `share` as siblings.
    ///
    /// The positive half of the assertion above: proving no command copies
    /// the binary alone does not prove the runtime arrives at all. Without
    /// this, a sequence that fetched the tarball and installed nothing would
    /// satisfy the negative test.
    #[test]
    fn the_tarball_install_places_the_runtime_beside_the_binary() {
        let sequence = argv_sequence_for(
            &InstallAction::ReleaseTarball { release: TarballRelease::Neovim },
            PackageManager::Apt,
            PrivilegeRequirement::None,
            Elevation::ViaSudo,
        );

        let flattened: Vec<String> = sequence
            .iter()
            .flat_map(|command| command.iter())
            .map(|word| word.to_string_lossy().into_owned())
            .collect();

        // The final destination has to be a prefix that will hold both, and
        // the executable has to be reachable on PATH from it. `search_path`
        // prepends ~/.local/bin, so a prefix elsewhere needs a link there.
        let names_a_versioned_prefix = flattened
            .iter()
            .any(|word| word.contains("/.local/opt/nvim-"));
        assert!(
            names_a_versioned_prefix,
            "the install must land in a versioned prefix that holds bin/ and \
             share/ together: {flattened:?}"
        );

        let links_onto_the_search_path = flattened
            .iter()
            .any(|word| word.ends_with("/.local/bin/nvim"));
        assert!(
            links_onto_the_search_path,
            "the executable must be reachable at ~/.local/bin/nvim, which is \
             what search_path prepends: {flattened:?}"
        );
    }

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

    /// The engine puts DEBIAN_FRONTEND in the apt child environment itself.
    ///
    /// This is the repo's sharpest incident: an unattended bootstrap halted
    /// at tzdata's debconf prompt while every image and CI leg passed,
    /// because Dockerfile.ubuntu set the variable itself. The environment was
    /// compensating for a gap in the engine, so the engine looked correct
    /// everywhere it was tested and failed on a real machine.
    ///
    /// The lesson is that the engine must not depend on inheriting it. This
    /// asserts that directly, on the map the engine builds, which holds only
    /// what the engine contributes: an inherited variable cannot put an entry
    /// here, so a pass means the engine set it.
    ///
    /// It used to also assert `std::env::var_os` was None, to prove the
    /// ambient variable was not doing the work. That assertion described the
    /// harness rather than the engine, and ubuntu-latest sets the variable
    /// image-wide, so it failed on a runner for a reason that says nothing
    /// about this code. `run_one` applies this map with `Command::env`, which
    /// overrides any inherited value, so the engine's guarantee holds whether
    /// or not the ambient variable is set.
    #[test]
    fn the_apt_child_environment_carries_debian_frontend() {
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
        let apt = argv_for(
            &an_apt_action(),
            PackageManager::Apt,
            PrivilegeRequirement::Root,
            Elevation::ViaSudo,
        );
        let pacman =
            argv_for(
                &a_pacman_action(),
                PackageManager::Pacman,
                PrivilegeRequirement::Root,
                Elevation::ViaSudo,
            );

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
            argv_for(
                &an_apt_action(),
                PackageManager::Apt,
                PrivilegeRequirement::Root,
                Elevation::ViaSudo,
            );

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

        let argv = argv_for(
            &action,
            PackageManager::Apt,
            PrivilegeRequirement::None,
            Elevation::ViaSudo,
        );

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
        let expected = argv_sequence_for(
            &step.action,
            PackageManager::Apt,
            step.privilege,
            Elevation::ViaSudo,
        );
        let preview = described.command_preview.expect("a spawnable action previews its argv");
        assert_eq!(preview, render_sequence(&expected), "the preview IS the argv");
        assert_eq!(
            expected.last().map(Vec::as_slice),
            Some(
                argv_for(
                    &step.action,
                    PackageManager::Apt,
                    step.privilege,
                    Elevation::ViaSudo,
                )
                .as_slice(),
            ),
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

        let sequence = argv_sequence_for(
            &action,
            PackageManager::Brew,
            PrivilegeRequirement::None,
            Elevation::ViaSudo,
        );

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

        let sequence = argv_sequence_for(
            &action,
            PackageManager::Brew,
            PrivilegeRequirement::None,
            Elevation::ViaSudo,
        );

        assert_eq!(sequence[0], words(["brew", "tap", "nikitabobko/tap"]));
        assert_eq!(sequence[2], words(["brew", "install", "--cask", "aerospace"]));
        assert!(
            sequence.iter().flatten().all(|word| word != "sudo"),
            "brew refuses to run as root, so no word may be sudo: {sequence:?}"
        );
    }

    /// A process already running as root emits no `sudo` word, even for a
    /// step that needs root.
    ///
    /// The regression this pins: `elevated_with` keyed the prefix on
    /// `PrivilegeRequirement` alone and never read `Elevation`, so a run on
    /// a root image with no `sudo` emitted `sudo pacman -Sy ...` for every
    /// privileged entry and every one of them died with "sudo: not found".
    /// Measured on the `Full bootstrap / one-liner on bare Arch` leg: 11
    /// privileged installs, 11 failures, 13 dependencies reported FAILED.
    ///
    /// `ViaSudo` is the positive control, so this asserts the elevation is
    /// what decides the word rather than asserting an argv that happens to
    /// be empty.
    #[test]
    fn a_root_process_needs_no_sudo_word_for_a_privileged_step() {
        let via_sudo = argv_for(
            &a_pacman_action(),
            PackageManager::Pacman,
            PrivilegeRequirement::Root,
            Elevation::ViaSudo,
        );
        assert_eq!(
            via_sudo.first().map(|word| word.to_string_lossy().into_owned()),
            Some(String::from("sudo")),
            "the control escalates: {via_sudo:?}"
        );

        let already_root = argv_for(
            &a_pacman_action(),
            PackageManager::Pacman,
            PrivilegeRequirement::Root,
            Elevation::AlreadyRoot,
        );

        assert!(!already_root.is_empty(), "the control builds real argv");
        assert!(
            !already_root.iter().any(|word| word == "sudo"),
            "already root, nothing to escalate to: {already_root:?}"
        );
        assert_eq!(
            already_root.first().map(|word| word.to_string_lossy().into_owned()),
            Some(String::from("pacman")),
            "the manager runs directly: {already_root:?}"
        );
    }

    /// The same rule holds for every argv in a multi-command action.
    ///
    /// The apt source action is eight commands, and the bug dropped `sudo`
    /// onto all eight. A test that only checked the install command would
    /// pass while seven `sudo apt-get`/`sudo tee` calls still failed.
    #[test]
    fn a_root_process_needs_no_sudo_word_in_any_command_of_a_sequence() {
        let action = InstallAction::AptSource {
            keyring: KeyringSource::GithubCli,
            list: SourceListEntry::GithubCli,
        };

        let via_sudo = argv_sequence_for(
            &action,
            PackageManager::Apt,
            PrivilegeRequirement::Root,
            Elevation::ViaSudo,
        );
        assert!(
            via_sudo.iter().all(|argv| argv.first().is_some_and(|word| word == "sudo")),
            "the control escalates every command: {via_sudo:?}"
        );

        let already_root = argv_sequence_for(
            &action,
            PackageManager::Apt,
            PrivilegeRequirement::Root,
            Elevation::AlreadyRoot,
        );

        assert_eq!(
            already_root.len(),
            via_sudo.len(),
            "the same commands run either way, only the prefix differs"
        );
        assert!(
            !already_root.iter().any(|argv| argv.iter().any(|word| word == "sudo")),
            "already root, no command escalates: {already_root:?}"
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
            argv_for(
                &an_apt_action(),
                PackageManager::Apt,
                PrivilegeRequirement::Root,
                Elevation::ViaSudo,
            );
        assert!(elevated.iter().any(|word| word == "sudo"), "the control elevates");

        let plain = argv_for(
            &an_apt_action(),
            PackageManager::Apt,
            PrivilegeRequirement::None,
            Elevation::ViaSudo,
        );

        assert!(!plain.is_empty(), "the control builds real argv");
        assert!(!plain.iter().any(|word| word == "sudo"), "no elevation, no word: {plain:?}");
    }

    /// A fetched installer script is written to a file and then run from it.
    ///
    /// The defect this pins: the retired shell wrote `curl ... | sh`, and the
    /// port kept both commands but dropped the pipe, because an effect here
    /// Every `ScriptInstaller`, for tests that must cover all of them.
    ///
    /// The match below is the guard: adding a variant makes it fail to
    /// compile, which is the only reason this list can be trusted. The
    /// hand-written list it replaced silently excluded `Nvm`, so the test
    /// that proves a fetched installer runs the file it fetched did not
    /// cover the one installer that runs under a different interpreter.
    const EVERY_SCRIPT_INSTALLER: [ScriptInstaller; 4] = [
        ScriptInstaller::Rustup,
        ScriptInstaller::OhMyZsh,
        ScriptInstaller::Zoxide,
        ScriptInstaller::Nvm,
    ];

    #[test]
    fn the_installer_list_covers_every_variant() {
        for installer in EVERY_SCRIPT_INSTALLER {
            match installer {
                ScriptInstaller::Rustup
                | ScriptInstaller::OhMyZsh
                | ScriptInstaller::Zoxide
                | ScriptInstaller::Nvm => {}
            }
        }
        let mut seen = EVERY_SCRIPT_INSTALLER.to_vec();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), EVERY_SCRIPT_INSTALLER.len(), "the list repeats a variant");
    }

    /// nvm's installer is fetched from a pinned tag and run under bash.
    ///
    /// Two facts, both load-bearing, neither visible in the argv assertions
    /// that cover the other three installers.
    ///
    /// The tag is the whole reason nvm is installable at all: upstream
    /// publishes no moving URL, so this dependency was manual-only and every
    /// unattended bootstrap left node uninstallable. A URL that drifted back
    /// to a branch would 404 rather than go stale quietly.
    ///
    /// bash is not interchangeable with sh here. nvm's installer tests
    /// `BASH_VERSION` in its first lines and exits 1 under dash, which is
    /// what `/bin/sh` is on every Debian-based image this repo bootstraps.
    #[test]
    fn the_nvm_installer_is_pinned_and_runs_under_bash() {
        let url = script_url(ScriptInstaller::Nvm);
        assert!(
            url.contains("/v0.40.7/"),
            "the nvm installer URL must name a version tag, got {url}"
        );

        let sequence = script_argv(ScriptInstaller::Nvm);
        let run = sequence.last().expect("the sequence ends with the run");
        assert_eq!(
            run.first().map(OsString::as_os_str),
            Some(OsStr::new("bash")),
            "nvm's installer refuses to run under sh: {run:?}"
        );
    }

    /// is argv and a pipe is not expressible as argv. `curl` then wrote to a
    /// discarded stdout and `sh -s --` read an empty stdin, so the step exited
    /// 0 having installed nothing and the check failed afterwards.
    ///
    /// Every existing test asserted the planned argv, and each argv was
    /// individually correct, so none of them could see it. This one asserts
    /// the two argvs are CONNECTED: the path curl writes is the path sh runs.
    #[test]
    fn a_fetched_installer_is_written_to_a_file_and_run_from_it() {
        for installer in EVERY_SCRIPT_INSTALLER {
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
            argv_sequence_for(
                &action,
                PackageManager::Apt,
                PrivilegeRequirement::Root,
                Elevation::ViaSudo,
            );
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

        let plain = argv_sequence_for(
            &action,
            PackageManager::Apt,
            PrivilegeRequirement::None,
            Elevation::ViaSudo,
        );
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
        assert!(argv_for(
            &step.action,
            PackageManager::Apt,
            PrivilegeRequirement::None,
            Elevation::ViaSudo).is_empty(),
        );
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
        let assumed =
            Spawning::executing(PackageManager::Apt, Elevation::ViaSudo, Approval::Assumed);
        let asking =
            Spawning::executing(PackageManager::Apt, Elevation::ViaSudo, Approval::Ask);

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
