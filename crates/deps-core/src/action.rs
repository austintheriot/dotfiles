use std::collections::BTreeMap;
use std::fmt;

use dotfiles_path::{NameError, PackageId};

use crate::check::CheckPath;
use crate::manifest::DependencyName;

/// The maximum byte length of a parsed tap name.
///
/// A tap is `owner/repo`, so it needs more room than a bare package name
/// while still refusing an unbounded string from a file that can reach this
/// crate without passing pre-commit (spec 3.6).
const MAX_TAP_LEN: usize = 128;

/// The detected package manager.
///
/// `Unknown` is a variant, not an error. `check-deps.sh:191-199` returns the
/// literal `unknown` and the script proceeds: every dependency reports
/// manual-only with its docs URL. On a fresh macOS box with no brew, which
/// is the machine `setup.sh` exists for, that list is the useful output and
/// aborting would replace it with one line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageManager {
    /// Debian and Ubuntu, through `apt-get`.
    Apt,
    /// Homebrew on macOS.
    Brew,
    /// Arch, through `pacman`.
    Pacman,
    /// No manager was detected.
    Unknown,
}

impl PackageManager {
    /// Whether an install through this manager needs root.
    ///
    /// A property of the manager, never of the dependency: a manifest could
    /// claim otherwise and be wrong (spec 3.5). This function is what
    /// replaces the `${SUDO}` string sniff at `check-deps.sh:544`.
    pub fn needs_root(self) -> bool {
        match self {
            PackageManager::Apt | PackageManager::Pacman => true,
            PackageManager::Brew | PackageManager::Unknown => false,
        }
    }
}

/// Which half of Homebrew installs a formula.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BrewKind {
    /// A command-line formula.
    Formula,
    /// A macOS application bundle.
    Cask,
}

/// A closed set of script-installer identities.
///
/// Identities rather than URLs. The adapter maps each to a hardcoded URL, so
/// adding one is a code change that appears in a diff and passes pre-commit.
/// A manifest-supplied URL would make one edited line an arbitrary-code
/// vector, amplified by the `commit-tree` bypass in spec 3.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScriptInstaller {
    /// The Rust toolchain installer.
    Rustup,
    /// oh-my-zsh's own installer, run with `--keep-zshrc`.
    OhMyZsh,
    /// zoxide's installer, the `*)` fallback at `check-deps.sh:308`.
    Zoxide,
}

/// A closed set of git clone sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CloneSource {
    /// The tmux plugin manager.
    Tpm,
    /// The zsh autosuggestions plugin.
    ZshAutosuggestions,
}

/// A closed set of APT keyring sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum KeyringSource {
    /// `cli.github.com`'s `githubcli-archive-keyring.gpg`.
    GithubCli,
}

/// A closed set of APT source-list entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceListEntry {
    /// The `github-cli.list` deb line.
    GithubCli,
}

/// A Homebrew tap name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TapName(String);

impl TapName {
    /// Parse a tap name.
    ///
    /// # Errors
    ///
    /// Returns `NameError::NotPrintable` outside `[A-Za-z0-9._/-]`. A tap
    /// name legitimately contains one `/`, unlike every other name type
    /// here, which is why it is not a [`PackageId`].
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        if raw.is_empty() {
            return Err(NameError::Empty);
        }
        if raw.len() > MAX_TAP_LEN {
            return Err(NameError::TooLong { len: raw.len(), max: MAX_TAP_LEN });
        }
        let allowed = |character: char| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '/')
        };
        if !raw.chars().all(allowed) {
            return Err(NameError::NotPrintable);
        }
        Ok(TapName(raw.to_string()))
    }

    /// The validated tap name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TapName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// Why a dependency has no automated install here.
///
/// A sum, not a comment. `NotAutomatable` with no reason cannot distinguish
/// a permanent upstream fact from a regression nobody noticed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoInstallReason {
    /// nvm: `check-deps.sh:369-372` records that nvm's own docs publish only
    /// version-pinned install URLs, so a hardcoded one would go stale.
    UpstreamPublishesNoStableUrl,
    /// cc on macOS: the install needs an interactive Xcode prompt.
    RequiresInteractiveApproval,
    /// dash on brew: not packaged under this name and macOS ships none.
    NotPackagedForThisManager,
    /// The manifest author named no package for this manager. Distinct from
    /// `NotPackagedForThisManager`, which is a claim about upstream, and
    /// fabricating that claim from an omission would be a false positive.
    ManagerNotNamedInManifest {
        /// The manager that had no entry.
        manager: PackageManager,
    },
    /// The step needs root and this machine has neither root nor sudo.
    PrivilegeUnavailable,
    /// Not in this wave. Named `NotYetInstalled` rather than `Missing`
    /// because under the fixpoint a later wave can change the answer, which
    /// is a different claim from "this can never be automated".
    PrerequisiteNotYetInstalled {
        /// The prerequisite that is not yet present.
        dependency: DependencyName,
    },
}

/// What it means to install a dependency.
///
/// Intent, never a command string: the core knows what it means to do, not
/// how the command is written.
///
/// `plan` returns an `InstallAction` for every missing dependency, never
/// `Option<InstallAction>`. That totality is what fixes today's bug, where
/// "no install" is signalled by an empty string that `check-deps.sh:569`
/// cannot distinguish from a missing prerequisite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallAction {
    /// The manager's ordinary install of one named package.
    Package {
        /// The package to install.
        id: PackageId,
    },
    /// Homebrew's install, which distinguishes formulae from casks.
    Brew {
        /// Formula or cask.
        kind: BrewKind,
        /// The package to install.
        id: PackageId,
        /// A tap to add first, when the package is not in core.
        tap: Option<TapName>,
    },
    /// `check-deps.sh:236` is not a package install. One line installs
    /// wget, creates `/etc/apt/keyrings` mode 755, fetches
    /// `githubcli-archive-keyring.gpg` from `cli.github.com`, tees it under
    /// sudo, appends a deb line to
    /// `/etc/apt/sources.list.d/github-cli.list`, runs `apt-get update`,
    /// and only then installs. Collapsing that to `Package { id: "gh" }`
    /// would let `describe` print "install package gh" for an action that
    /// permanently adds a third-party APT trust root.
    AptSource {
        /// The trust root this action adds.
        keyring: KeyringSource,
        /// The source-list line this action appends.
        list: SourceListEntry,
    },
    /// `check-deps.sh:411`. `break_system_packages` is a named field rather
    /// than a hidden default because it overrides PEP 668, and blast radius
    /// belongs in the type.
    Pip {
        /// The distribution to install.
        id: PackageId,
        /// Whether to pass `--break-system-packages`.
        break_system_packages: bool,
    },
    /// One of the closed set of upstream installer scripts.
    Script {
        /// Which installer to run.
        installer: ScriptInstaller,
    },
    /// A clone of one of the closed set of plugin repositories.
    GitClone {
        /// Which repository to clone.
        source: CloneSource,
        /// Where the clone lands.
        into: CheckPath,
    },
    /// No payload: there is one node entry, no conf file pins a version, and
    /// the only value is `--lts` (`check-deps.sh:388`). A `String` payload
    /// would reopen spec 3.6 by admitting shell-adjacent text as data.
    NvmInstall,
    /// There is no automated install, and the reason says why.
    NotAutomatable {
        /// Why this dependency has no automated install here.
        reason: NoInstallReason,
    },
}

/// Whether a dependency is installable through a given manager.
///
/// A map with absent keys cannot distinguish "same name here" from "not
/// installable here", and the conf files contain both: git on brew is git,
/// while cc on brew is genuinely unavailable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageAvailability {
    /// The manager packages it under this name.
    Named(PackageId),
    /// zoxide needs this: apt, brew and pacman all have packages, and any
    /// other manager gets the installer script (`check-deps.sh:308`).
    ViaScript(ScriptInstaller),
    /// The manager cannot install it, and the reason says why.
    Unavailable(NoInstallReason),
    /// `gh` on apt. Not a package install: `check-deps.sh:236` adds a
    /// third-party trust root before installing, so collapsing it to
    /// `Named` would let a dry run print "install package gh" for an action
    /// that permanently changes what the machine trusts.
    AptWithSource {
        /// The trust root this availability adds.
        keyring: KeyringSource,
        /// The source-list line it appends.
        list: SourceListEntry,
    },
    /// `pyyaml` where no distribution package exists. `break_system_packages`
    /// overrides PEP 668, so it is a named field rather than a default.
    PipDistribution {
        /// The distribution to install.
        id: PackageId,
        /// Whether to pass the PEP 668 override.
        break_system_packages: bool,
    },
    /// `zsh-autosuggestions` and `tpm`, which install by cloning rather than
    /// through any manager.
    Clone {
        /// The upstream to clone.
        source: CloneSource,
        /// Where the clone lands. The same value the check reads, so
        /// convergence is structural rather than hoped for.
        into: CheckPath,
    },
    /// `node`, which installs through nvm rather than a manager.
    ViaNvm,
}

/// Per-manager availability with a mandatory fallback.
///
/// The fallback is mandatory and states its own reason, which is what makes
/// [`PackageMap::resolve`] genuinely total. An `Option<PackageId>` default
/// made it total only in the trivial sense: with `None` and no key for the
/// queried manager there is no correct answer, and fabricating
/// `NotPackagedForThisManager` would be a false claim about upstream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageMap {
    per_manager: BTreeMap<PackageManager, PackageAvailability>,
    fallback: PackageAvailability,
}

impl PackageMap {
    /// Pair per-manager availability with the fallback for every other.
    pub fn new(
        per_manager: BTreeMap<PackageManager, PackageAvailability>,
        fallback: PackageAvailability,
    ) -> Self {
        PackageMap { per_manager, fallback }
    }

    /// Total: every manager resolves to an availability that states itself.
    pub fn resolve(&self, manager: PackageManager) -> &PackageAvailability {
        self.per_manager.get(&manager).unwrap_or(&self.fallback)
    }
}
