//! The production package catalog and requirement table.
//!
//! This is the install knowledge `retired-check-deps:203-441` held as shell
//! `case` arms, restated as data. The shell decided what to run by string
//! matching a dependency name against a manager name; here the same 22
//! dependencies each carry a [`PackageMap`], and `deps_core::plan` turns a
//! resolved [`PackageAvailability`] into an action.
//!
//! Nothing in this module performs IO. The four manifests reach it through
//! `include_str!`, which reads at compile time, so the tests below need no
//! filesystem and the module holds no capability.

use std::collections::{BTreeMap, BTreeSet};

use deps_core::{
    CloneSource, ConfKind, DependencyName, KeyringSource, NoInstallReason, PackageAvailability,
    PackageCatalog, PackageManager, PackageMap, PathRoot, Requirements, ScriptInstaller,
    SourceListEntry, TarballRelease, parse_manifest_toml,
};
use dotfiles_path::{CheckRelPath, PackageId};

/// The four manifests this repository ships, paired with how each is chosen.
///
/// `deps-ci.toml` is [`ConfKind::ExplicitOnly`] because its own header says
/// it is selected only by an explicit `DEPS_CONF`. That kind is what permits
/// its `python_import` entry, which a platform-selected file may not carry.
const SHIPPED_MANIFESTS: [(&str, ConfKind); 4] = [
    (include_str!("../../../../deps/deps.toml"), ConfKind::PlatformSelected),
    (include_str!("../../../../deps/deps-linux.toml"), ConfKind::PlatformSelected),
    (include_str!("../../../../deps/deps-mac.toml"), ConfKind::PlatformSelected),
    (include_str!("../../../../deps/deps-ci.toml"), ConfKind::ExplicitOnly),
];

/// Where the `zsh-autosuggestions` clone lands, relative to `$HOME`.
///
/// The plugin DIRECTORY, matching `retired-check-deps:339`. `deps.toml`
/// checks the `.zsh` FILE one level inside it, and the clone creates that
/// file by creating the directory that contains it.
///
/// This used to name the file, on the reasoning that a clone target must
/// equal its check subject byte for byte. That reasoning holds for `tpm`,
/// whose check is `-d`, and inverts here: `git clone <url> <path>` makes
/// `<path>` a DIRECTORY, so cloning to the file path produced a directory
/// named `zsh-autosuggestions.zsh` and the `-f` check could never pass. The
/// Arch bootstrap leg reported "the install succeeded and the check still
/// fails" on every run, which is exactly the non-converging fixpoint the old
/// comment was trying to avoid, reached by the other road.
///
/// The invariant is not equality. It is that the clone must CREATE the check
/// subject, which for a `-d` check means the target is the subject and for a
/// `-f` check means the target is its parent.
/// `every_clone_target_creates_its_check_subject` holds that.
const ZSH_AUTOSUGGESTIONS_CLONE_TARGET: &str =
    ".oh-my-zsh/custom/plugins/zsh-autosuggestions";

/// Where the `tpm` clone lands, relative to `$HOME`.
///
/// `deps.toml` checks the directory and `retired-check-deps:345` clones the
/// directory, so this one already agreed in the shell. It is a constant here
/// for the same reason as the plugin path above: the equality is asserted
/// against the parsed conf file rather than against a second copy of the
/// string.
const TPM_CLONE_TARGET: &str = ".tmux/plugins/tpm";

/// The install table for every dependency the shipped conf files name.
///
/// Total over the 22 shipped dependencies. A dependency the four files name
/// but this function omits is not a compile error and `PackageMap::resolve`
/// is total, so the omission would surface as a plausible-looking "no
/// automated install" for something that has one. That is what
/// `every_shipped_dependency_has_a_catalog_entry` exists to refuse.
pub fn packages() -> PackageCatalog {
    let mut catalog = PackageCatalog::new();

    // Rows whose package name is the dependency name on every manager, which
    // is the shell's `*)` default case at `retired-check-deps:434-438`.
    for name in ["git", "zsh", "fzf", "ripgrep", "tmux", "shellcheck", "xclip"] {
        insert_same_name_everywhere(&mut catalog, name);
    }

    // ripgrep's binary is `rg`, but the package is named for the project on
    // all three managers, so it needs no override.

    // neovim is NOT in the row above, though its package is spelled the same
    // everywhere, because on apt the correctly-spelled package is too old to
    // satisfy the manifest's `>=0.10` floor: Pop!_OS 22.04 ships 0.6.1 and
    // 24.04 ships 0.9.5. Installing it would leave the fixpoint running the
    // same install every wave against a check that can never pass.
    //
    // brew and pacman both ship current Neovim, so only the apt arm differs.
    insert(
        &mut catalog,
        "neovim",
        per_manager(vec![
            (PackageManager::Apt, PackageAvailability::ViaTarball(TarballRelease::Neovim)),
            // brew and pacman use the pinned tarball too, though both package
            // a recent enough Neovim to satisfy the floor. The floor is not
            // the point: treesitter parsers are compiled against the running
            // Neovim's ABI, so a `brew upgrade` moved the ABI under a parser
            // set the repo believes is pinned. Homebrew cannot express the
            // pin -- there is no neovim@0.12 formula, and brew stable had
            // already moved to 0.12.5 while this machine sat at 0.12.4 -- so
            // the tarball is the only mechanism that fixes the version on
            // every platform.
            (PackageManager::Brew, PackageAvailability::ViaTarball(TarballRelease::Neovim)),
            (PackageManager::Pacman, PackageAvailability::ViaTarball(TarballRelease::Neovim)),
        ]),
        PackageAvailability::Unavailable(NoInstallReason::ManagerNotNamedInManifest {
            manager: PackageManager::Unknown,
        }),
    );

    // The same gzipped-binary release on every platform, unlike neovim which
    // comes from brew on a mac. Upstream publishes linux-x64, linux-arm64 and
    // macos-arm64 assets; the package managers either lack it or lag it, and
    // the version has to match what compiled the parsers.
    insert(
        &mut catalog,
        "tree-sitter-cli",
        per_manager(vec![
            (PackageManager::Apt, PackageAvailability::ViaTarball(TarballRelease::TreeSitterCli)),
            (PackageManager::Brew, PackageAvailability::ViaTarball(TarballRelease::TreeSitterCli)),
            (
                PackageManager::Pacman,
                PackageAvailability::ViaTarball(TarballRelease::TreeSitterCli),
            ),
        ]),
        PackageAvailability::Unavailable(NoInstallReason::ManagerNotNamedInManifest {
            manager: PackageManager::Unknown,
        }),
    );

    insert(
        &mut catalog,
        "gh",
        per_manager(vec![
            // Not a package install. `retired-check-deps:235` adds a third-party
            // APT trust root and a source-list entry before installing, and
            // collapsing that to a named package would let a dry run print
            // "install package gh" for a permanent change to what the
            // machine trusts.
            (
                PackageManager::Apt,
                PackageAvailability::AptWithSource {
                    keyring: KeyringSource::GithubCli,
                    list: SourceListEntry::GithubCli,
                },
            ),
            (PackageManager::Brew, named("gh")),
            (PackageManager::Pacman, named("github-cli")),
        ]),
        PackageAvailability::Unavailable(NoInstallReason::ManagerNotNamedInManifest {
            manager: PackageManager::Unknown,
        }),
    );

    insert(
        &mut catalog,
        "alacritty",
        per_manager(vec![
            (PackageManager::Apt, named("alacritty")),
            (PackageManager::Pacman, named("alacritty")),
            // `retired-check-deps:261-263` drops the brew case deliberately:
            // Homebrew disabled the cask on 2026-09-01 for failing the
            // Gatekeeper check, and the release .dmg is adhoc-signed with no
            // Team ID, so `spctl -a` rejects it too.
            (
                PackageManager::Brew,
                PackageAvailability::Unavailable(NoInstallReason::NotPackagedForThisManager),
            ),
        ]),
        PackageAvailability::Unavailable(NoInstallReason::ManagerNotNamedInManifest {
            manager: PackageManager::Unknown,
        }),
    );

    insert(
        &mut catalog,
        "zsh-autosuggestions",
        per_manager(vec![
            (PackageManager::Brew, named("zsh-autosuggestions")),
        ]),
        clone_availability(CloneSource::ZshAutosuggestions, ZSH_AUTOSUGGESTIONS_CLONE_TARGET),
    );

    insert(
        &mut catalog,
        "zoxide",
        per_manager(vec![
            (PackageManager::Apt, named("zoxide")),
            (PackageManager::Brew, named("zoxide")),
            (PackageManager::Pacman, named("zoxide")),
        ]),
        // `retired-check-deps:308`. The installer resolves the latest release
        // through an unauthenticated api.github.com call, whose 60-per-hour
        // per-IP quota every Actions runner shares, so it is the fallback
        // rather than the first choice.
        PackageAvailability::ViaScript(ScriptInstaller::Zoxide),
    );

    insert(
        &mut catalog,
        "tpm",
        per_manager(Vec::new()),
        clone_availability(CloneSource::Tpm, TPM_CLONE_TARGET),
    );

    insert(
        &mut catalog,
        "cc",
        per_manager(vec![
            (PackageManager::Apt, named("build-essential")),
            (PackageManager::Pacman, named("base-devel")),
            // macOS ships clang through the Xcode command line tools, and
            // `xcode-select --install` opens a GUI prompt, so there is no
            // unattended path.
            (
                PackageManager::Brew,
                PackageAvailability::Unavailable(NoInstallReason::RequiresInteractiveApproval),
            ),
        ]),
        PackageAvailability::Unavailable(NoInstallReason::ManagerNotNamedInManifest {
            manager: PackageManager::Unknown,
        }),
    );

    insert(
        &mut catalog,
        "rustup",
        per_manager(Vec::new()),
        PackageAvailability::ViaScript(ScriptInstaller::Rustup),
    );

    insert(
        &mut catalog,
        "nvm",
        per_manager(Vec::new()),
        // Was UpstreamPublishesNoStableUrl, which was true of the URL shape
        // rather than of nvm: upstream publishes no MOVING url, and every
        // install command in its README names a version. Pinning one tag is
        // what the docs actually instruct, so the reason no longer holds.
        //
        // The cost of the pin is that it goes stale, which is the tradeoff
        // taken deliberately: a stale pin is a visible constant somebody
        // bumps, while manual-only meant node never installed on any
        // unattended run and the bootstrap reported it every time.
        PackageAvailability::ViaScript(ScriptInstaller::Nvm),
    );

    // node installs through nvm on every manager, so that a brew or apt node
    // does not sit on PATH beside nvm's and shadow it in whichever order the
    // shell resolves them.
    insert(&mut catalog, "node", per_manager(Vec::new()), PackageAvailability::ViaNvm);

    insert(
        &mut catalog,
        "oh-my-zsh",
        per_manager(Vec::new()),
        PackageAvailability::ViaScript(ScriptInstaller::OhMyZsh),
    );

    // A cask in a third-party tap. `PackageAvailability::Named` resolves to
    // `InstallAction::Brew { kind: Formula, tap: None }` in
    // `deps_core::plan`, so the tap-and-cask shape of
    // `retired-check-deps:280` is not expressible through this catalog today. The
    // entry is here so the dependency is not reported as unautomatable; the
    // gap is in `deps_core`, not in this table.
    insert(
        &mut catalog,
        "aerospace",
        per_manager(vec![(PackageManager::Brew, named("aerospace"))]),
        // No Linux build exists, so every other manager is honestly absent
        // rather than merely unnamed.
        PackageAvailability::Unavailable(NoInstallReason::NotPackagedForThisManager),
    );

    insert(
        &mut catalog,
        "python3",
        per_manager(vec![
            (PackageManager::Apt, named("python3")),
            (PackageManager::Brew, named("python")),
            (PackageManager::Pacman, named("python")),
        ]),
        PackageAvailability::Unavailable(NoInstallReason::ManagerNotNamedInManifest {
            manager: PackageManager::Unknown,
        }),
    );

    insert(
        &mut catalog,
        "pyyaml",
        per_manager(vec![
            // apt and pacman prefer the distribution package: on a
            // Debian-derived system PEP 668 refuses a pip install into the
            // system interpreter, and the distro package is what the
            // runner's python3 imports.
            (PackageManager::Apt, named("python3-yaml")),
            (PackageManager::Pacman, named("python-yaml")),
        ]),
        // Homebrew has no pyyaml formula. `break_system_packages` overrides
        // PEP 668, which is correct on a CI runner and a throwaway container
        // and is what `test-suite.yml` already did by hand.
        pip_distribution("pyyaml"),
    );

    insert(
        &mut catalog,
        "dash",
        per_manager(vec![
            (PackageManager::Apt, named("dash")),
            (PackageManager::Pacman, named("dash")),
            // Homebrew does not package dash under this name and macOS ships
            // none, so the suite's one dash assertion skips rather than
            // installing something else and calling it dash.
            (
                PackageManager::Brew,
                PackageAvailability::Unavailable(NoInstallReason::NotPackagedForThisManager),
            ),
        ]),
        PackageAvailability::Unavailable(NoInstallReason::ManagerNotNamedInManifest {
            manager: PackageManager::Unknown,
        }),
    );

    catalog
}

/// The prerequisite edges between dependencies, rejected if any edge names a
/// dependency no shipped conf file holds.
///
/// One edge today. `deps.toml` states in the file that no ordering
/// between `oh-my-zsh` and `zsh-autosuggestions` is guaranteed there, and
/// that is the ordering this table supplies: on Linux the plugin installs by
/// cloning into `$HOME/.oh-my-zsh/custom`, which does not exist until
/// oh-my-zsh does.
///
/// `known` must be the union of every shipped conf file rather than one
/// platform's manifest, because an edge is correct or not independently of
/// which machine runs it. `oh-my-zsh` is in `deps-linux.toml` and absent on
/// macOS, and validating against the macOS manifest alone would reject a
/// correct edge.
///
/// # Errors
///
/// Returns every edge that names a dependency absent from `known`, rather
/// than the first, so one run names every typo.
/// Every prerequisite edge, as dependent-to-prerequisites pairs.
///
/// A pair here says "the dependent's install reads something the
/// prerequisite creates", which is stronger than "these are related". Both
/// edges below are that: each dependent's install command names a path the
/// prerequisite writes.
const EDGES: &[(&str, &[&str])] = &[
    // The plugin clones into $HOME/.oh-my-zsh/custom, which does not exist
    // until oh-my-zsh does. deps.toml states in the file that no
    // ordering between the two is guaranteed there, and this supplies it.
    ("zsh-autosuggestions", &["oh-my-zsh"]),
    // node installs by sourcing $HOME/.nvm/nvm.sh. Without the edge the
    // planner attempted node in the same wave nvm was still missing and the
    // step failed with `sh: 1: .: cannot open /home/tester/.nvm/nvm.sh`,
    // reported as an install failure (exit 3) on every bootstrap leg.
    //
    // The edge still carries weight now that nvm installs from a pinned
    // script: it is what orders the two, so nvm's install runs in an earlier
    // wave and node's source of nvm.sh finds a file that exists.
    ("node", &["nvm"]),
    // oh-my-zsh's installer refuses to run without zsh: its first check
    // prints "Zsh is not installed. Please install zsh first." and exits 1.
    // The message goes to STDOUT, so the engine reported "exited 1 with no
    // output on stderr" -- a failure naming neither the cause nor the fix.
    //
    // This edge was load-bearing and UNDECLARED for as long as the manifest
    // parser preserved file order, which put zsh (deps.toml) ahead of
    // oh-my-zsh (deps-linux.toml) by luck rather than by rule. The TOML
    // parser sorts by name, oh-my-zsh sorts first, and all four full-
    // bootstrap legs failed while the four package-manager legs passed --
    // because only a bootstrap installs zsh in the same run.
    //
    // Reproduced before fixing: upstream's install.sh in a container with
    // curl, git and NO zsh exits 1 with zero bytes on stderr, matching CI's
    // report exactly.
    //
    // Same shape as the two edges above: the dependent's install reads
    // something the prerequisite provides. Here it is the zsh binary rather
    // than a directory.
    ("oh-my-zsh", &["zsh"]),
];

pub fn requirements(
    known: &BTreeSet<DependencyName>,
) -> Result<Requirements, Vec<UnknownRequirementEdge>> {
    // A name that will not parse is a typo in this file, not a fact about the
    // machine, and `validated` below reports an unknown name anyway. Skipping
    // the edge keeps this function total without inventing a second error
    // path for a case the constructor already owns.
    let mut pairs = Vec::new();
    for (dependent, prerequisites) in EDGES {
        let Some(dependent) = dependency_name(dependent) else {
            continue;
        };
        let named: Vec<_> = prerequisites.iter().filter_map(|name| dependency_name(name)).collect();
        if named.len() == prerequisites.len() {
            pairs.push((dependent, named));
        }
    }

    Requirements::validated(pairs, known).map_err(|edges| {
        edges
            .into_iter()
            .map(|edge| UnknownRequirementEdge {
                dependent: edge.dependent,
                unknown: edge.unknown,
            })
            .collect()
    })
}

/// An edge naming a dependency no shipped conf file holds.
///
/// A restatement of `deps_core`'s own `RequirementEdgeError`, which that
/// crate defines as `pub` but does not re-export from its `lib.rs`. The type
/// is therefore unnameable outside `deps_core` and cannot appear in this
/// module's signature. Delete this struct and return the original once
/// `deps-core`'s `pub use plan::{...}` list names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownRequirementEdge {
    /// The dependent the edge belongs to.
    pub dependent: DependencyName,
    /// The name that matches no entry in any shipped conf file.
    pub unknown: DependencyName,
}

/// Every dependency named by any of the four shipped conf files.
///
/// The union rather than one platform's manifest, for the reason
/// [`requirements`] documents. A file that fails to parse contributes
/// nothing rather than panicking, because `packages()` and `requirements()`
/// are called from a path that must not abort; the tests below assert the
/// union is the full 22, which is what turns a silent parse failure into a
/// red suite.
pub fn every_shipped_dependency() -> BTreeSet<DependencyName> {
    let mut names = BTreeSet::new();
    for (text, kind) in SHIPPED_MANIFESTS {
        let Ok(manifest) = parse_manifest_toml(text, kind) else {
            continue;
        };
        for entry in manifest.entries() {
            names.insert(entry.name.clone());
        }
    }
    names
}

/// Build the per-manager half of a [`PackageMap`].
fn per_manager(
    pairs: Vec<(PackageManager, PackageAvailability)>,
) -> BTreeMap<PackageManager, PackageAvailability> {
    pairs.into_iter().collect()
}

/// Record one dependency's install table, skipping a name this crate's own
/// rules refuse.
///
/// A skipped entry is not silently tolerated: the dependency then has no
/// catalog entry, and `every_shipped_dependency_has_a_catalog_entry` fails.
/// That is the trade this function makes deliberately, because `packages()`
/// runs on a path where `deny(clippy::unwrap_used)` applies and a panic
/// would replace a legible report with a backtrace.
fn insert(
    catalog: &mut PackageCatalog,
    name: &str,
    availabilities: BTreeMap<PackageManager, PackageAvailability>,
    fallback: PackageAvailability,
) {
    if let Some(parsed) = dependency_name(name) {
        catalog.insert(parsed, PackageMap::new(availabilities, fallback));
    }
}

/// Record a dependency whose package name matches its dependency name on
/// apt, brew and pacman, which is `retired-check-deps:434-438`'s default case.
fn insert_same_name_everywhere(catalog: &mut PackageCatalog, name: &str) {
    let availability = named(name);
    insert(
        catalog,
        name,
        per_manager(vec![
            (PackageManager::Apt, availability.clone()),
            (PackageManager::Brew, availability.clone()),
            (PackageManager::Pacman, availability),
        ]),
        // An undetected manager has no install command in the shell either:
        // the default case has no `*)` arm.
        PackageAvailability::Unavailable(NoInstallReason::ManagerNotNamedInManifest {
            manager: PackageManager::Unknown,
        }),
    );
}

/// One manager's package name, or a stated absence if the literal above is
/// not a usable package name.
///
/// The degraded value names the manager rather than claiming anything about
/// upstream, so a catalog typo cannot masquerade as "upstream does not
/// package this".
fn named(raw: &str) -> PackageAvailability {
    match PackageId::parse(raw) {
        Ok(id) => PackageAvailability::Named(id),
        Err(_) => PackageAvailability::Unavailable(
            NoInstallReason::ManagerNotNamedInManifest { manager: PackageManager::Unknown },
        ),
    }
}

/// A pip install with the PEP 668 override, or a stated absence.
fn pip_distribution(raw: &str) -> PackageAvailability {
    match PackageId::parse(raw) {
        Ok(id) => PackageAvailability::PipDistribution { id, break_system_packages: true },
        Err(_) => PackageAvailability::Unavailable(
            NoInstallReason::ManagerNotNamedInManifest { manager: PackageManager::Unknown },
        ),
    }
}

/// A clone landing at `rest` under `$HOME`, or a stated absence.
///
/// `PathRoot::Home` is the only root a clone target can have. A previous plan
/// deleted `PathRoot::OhMyZshCustom` for exactly this: the shell's install
/// wrote `${ZSH_CUSTOM:-$HOME/.oh-my-zsh/custom}` while its check read
/// `$HOME/...`, and the two diverge whenever the variable is set.
fn clone_availability(source: CloneSource, rest: &str) -> PackageAvailability {
    match CheckRelPath::parse(rest) {
        Ok(parsed) => PackageAvailability::Clone {
            source,
            into: deps_core::CheckPath::new(PathRoot::Home, parsed),
        },
        Err(_) => PackageAvailability::Unavailable(
            NoInstallReason::ManagerNotNamedInManifest { manager: PackageManager::Unknown },
        ),
    }
}

/// Parse a dependency name, discarding one this crate's rules refuse.
fn dependency_name(raw: &str) -> Option<DependencyName> {
    DependencyName::parse(raw).ok()
}

#[cfg(test)]
mod tests {
    use deps_core::{Check, CheckPath, Manifest};

    use super::*;

    /// The production requirement table validates against the union of every
    /// shipped conf file.
    ///
    /// This is the test `Requirements::validated`'s own doc comment names and
    /// `deps-core` could not write, because the table it validates did not
    /// exist there. It catches a typo neither platform's live run would: a
    /// misspelled prerequisite is absent on every platform, so it looks
    /// exactly like the legitimate macOS-absent case the design relies on,
    /// plans successfully, orders nothing, and exits 0.
    #[test]
    fn the_production_requirement_table_validates_against_every_conf_file() {
        let known = every_shipped_dependency();

        // Positive control. Validation below passes vacuously against an
        // empty or partial union, so assert the union really spans all four
        // files first, naming one dependency exclusive to each.
        assert_eq!(known.len(), 23, "the union must cover every conf file");
        assert!(known.contains(&name("git")), "deps.toml entries are present");
        assert!(known.contains(&name("oh-my-zsh")), "deps-linux.toml entries are present");
        assert!(known.contains(&name("aerospace")), "deps-mac.toml entries are present");
        assert!(known.contains(&name("pyyaml")), "deps-ci.toml entries are present");

        requirements(&known).expect("the production requirement table must validate");
    }

    /// Every edge the table declares, by content.
    ///
    /// `validated` proves the names exist; it cannot prove an edge is
    /// present. `requirements` skips a pair whose name will not parse, so a
    /// dropped edge leaves a table that still validates and still plans, and
    /// the only symptom is a dependent installed before its prerequisite --
    /// which is what the node/nvm edge was added to stop.
    #[test]
    fn the_table_declares_the_edges_the_manifests_need() {
        let known = every_shipped_dependency();
        let table = requirements(&known).expect("the production table validates");

        assert_eq!(
            table.prerequisites(&name("zsh-autosuggestions")),
            [name("oh-my-zsh")],
            "the plugin clones into a directory oh-my-zsh creates"
        );
        assert_eq!(
            table.prerequisites(&name("node")),
            [name("nvm")],
            "node installs by sourcing nvm.sh"
        );
        // oh-my-zsh's installer refuses to run without zsh. Its first check
        // prints "Zsh is not installed. Please install zsh first." to STDOUT
        // and exits 1, so the engine reported "exited 1 with no output on
        // stderr" -- a diagnostic that names neither the cause nor the fix.
        //
        // This edge was load-bearing and undeclared for as long as the
        // manifest parser preserved file order, which happened to put zsh
        // before oh-my-zsh. The TOML parser sorts by name, oh-my-zsh sorts
        // first, and four bootstrap legs failed. The ordering was doing the
        // work an edge should do.
        assert_eq!(
            table.prerequisites(&name("oh-my-zsh")),
            [name("zsh")],
            "oh-my-zsh's installer exits 1 when zsh is absent"
        );
    }

    /// The union test's own control: a misspelled edge is rejected.
    ///
    /// Without this, `validated` returning `Ok` for everything would make the
    /// test above pass no matter what the table said.
    #[test]
    fn a_misspelled_prerequisite_is_rejected_by_the_same_union() {
        let known = every_shipped_dependency();

        let misspelled = Requirements::validated(
            vec![(name("zsh-autosuggestions"), vec![name("oh-my-zhs")])],
            &known,
        );

        let errors = misspelled.expect_err("a misspelled prerequisite must not validate");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].unknown, name("oh-my-zhs"));
    }

    /// Every clone target equals the check subject of its dependency, byte
    /// for byte.
    ///
    /// Nothing in `deps_core` enforces this: `PackageCatalog` is a bare map
    /// and neither `plan` nor `action_for` compares them. A clone pointed at
    /// a directory whose check reads a file inside it gives a non-converging
    /// fixpoint, where the clone succeeds, the re-gather still reports
    /// absent, and the step has already been retired as attempted.
    #[test]
    fn every_clone_target_creates_its_check_subject() {
        let catalog = packages();

        let mut checked = 0;
        for entry in shipped_entries() {
            let Some(package_map) = catalog.get(&entry.name) else {
                panic!("{:?} is in a shipped conf file but has no catalog entry", entry.name);
            };
            for manager in every_manager() {
                let PackageAvailability::Clone { into, .. } = package_map.resolve(manager) else {
                    continue;
                };
                // The clone must CREATE the subject, which is not the same
                // as equalling it. `git clone <url> <path>` makes <path> a
                // DIRECTORY, so it satisfies a directory subject by being it
                // and a file subject by being that file's parent -- and it
                // can never satisfy a file subject by equalling it.
                //
                // The old rule was plain equality, which admitted exactly
                // that impossible case: the zsh-autosuggestions target named
                // the .zsh file, the clone made a directory by that name,
                // and the -f check failed forever while the install reported
                // success. Pairing each subject with the kind of check it
                // came from is what makes the rule able to say no.
                let satisfiable = subjects_by_kind(&entry.check).into_iter().any(
                    |(subject, is_file)| {
                        if is_file {
                            subject.root == into.root
                                && subject
                                    .rest
                                    .as_str()
                                    .rsplit_once('/')
                                    .is_some_and(|(parent, _)| parent == into.rest.as_str())
                        } else {
                            &subject == into
                        }
                    },
                );
                assert!(
                    satisfiable,
                    "the clone target {into:?} for {:?} on {manager:?} cannot create any subject \
                     of its own check. A clone makes a directory, so it must equal a directory \
                     subject or be the parent of a file subject. Subjects: {:?}",
                    entry.name,
                    subjects_by_kind(&entry.check)
                );
                checked += 1;
            }
        }

        // Positive control: at least one clone must exist, or this test
        // passes by iterating nothing.
        assert!(checked > 0, "the catalog must contain at least one Clone availability");
    }

    /// Neovim's version is pinned on every platform, not only on apt.
    ///
    /// THE GAP THIS CLOSES. apt got `ViaTarball` because Pop!_OS ships 0.6.1
    /// and 24.04 ships 0.9.5, both below the manifest's floor. brew and
    /// pacman got `named("neovim")`, which is whatever the distribution
    /// currently offers, so the version was an exact fact on Linux and an
    /// accident on macOS.
    ///
    /// That asymmetry lands on the one value every treesitter parser is
    /// keyed to. Parsers are compiled against the running Neovim's ABI, so a
    /// `brew upgrade` moves the ABI under a parser set the repo believes is
    /// pinned -- and pinning parser sources (which the plugin pin already
    /// does) buys nothing if the thing that loads them floats.
    ///
    /// Homebrew cannot express the pin: there is no `neovim@0.12` formula,
    /// only third-party 0.11.x taps, and brew stable had already moved to
    /// 0.12.5 while this machine sat at 0.12.4. Upstream does publish
    /// `nvim-macos-arm64.tar.gz` and `nvim-macos-x86_64.tar.gz` with the
    /// same bin/lib/share layout the Linux path already installs, so the
    /// tarball is the only mechanism that pins it.
    #[test]
    fn every_manager_installs_the_pinned_neovim_release() {
        let catalog = packages();
        let entry = catalog.get(&name("neovim")).expect("neovim is a catalog entry");

        for manager in every_manager() {
            // Unknown is the no-manager case and legitimately has no answer.
            if manager == PackageManager::Unknown {
                continue;
            }
            let availability = entry.resolve(manager);
            assert!(
                matches!(
                    availability,
                    PackageAvailability::ViaTarball(TarballRelease::Neovim)
                ),
                "{manager:?} must install the pinned release rather than a \
                 distribution package, or the treesitter ABI floats on that \
                 platform: {availability:?}"
            );
        }
    }

    /// Every dependency in every shipped conf file has a catalog entry.
    ///
    /// A missing entry is not a compile error, and `PackageMap::resolve` is
    /// total, so a dependency absent from the catalog would plan as
    /// `NotAutomatable` and report "no automated install" for something that
    /// has one.
    #[test]
    fn every_shipped_dependency_has_a_catalog_entry() {
        let catalog = packages();
        let known = every_shipped_dependency();

        assert!(!known.is_empty(), "the control must find dependencies");
        for dependency in &known {
            assert!(
                catalog.contains_key(dependency),
                "{dependency:?} is in a shipped conf file but has no catalog entry"
            );
        }
        assert_eq!(
            catalog.len(),
            known.len(),
            "the catalog must name exactly the shipped dependencies, no more"
        );
    }

    /// No catalog entry degraded to the sentinel the fallible constructors
    /// return when a literal in this file is not a usable name.
    ///
    /// `named`, `pip_distribution` and `clone_availability` cannot panic,
    /// because `packages()` runs under `deny(clippy::unwrap_used)`. They
    /// return `ManagerNotNamedInManifest { manager: Unknown }` instead, which
    /// is also a legitimate fallback for a genuinely unnamed manager. This
    /// test separates the two: it asserts no PER-MANAGER entry carries the
    /// sentinel, where a real omission would appear only in the fallback.
    #[test]
    fn no_catalog_literal_failed_to_parse() {
        let catalog = packages();
        let sentinel = PackageAvailability::Unavailable(
            NoInstallReason::ManagerNotNamedInManifest { manager: PackageManager::Unknown },
        );

        // Positive control: the sentinel is the value the constructors
        // actually return, so a rename there fails here rather than silently
        // making the loop below unfalsifiable.
        assert_eq!(named("not a package id"), sentinel);

        for (dependency, package_map) in &catalog {
            for manager in [PackageManager::Apt, PackageManager::Brew, PackageManager::Pacman] {
                assert_ne!(
                    *package_map.resolve(manager),
                    sentinel,
                    "{dependency:?} on {manager:?} degraded to the parse-failure sentinel"
                );
            }
        }
    }

    /// The four managers a catalog entry can be resolved against.
    fn every_manager() -> [PackageManager; 4] {
        [
            PackageManager::Apt,
            PackageManager::Brew,
            PackageManager::Pacman,
            PackageManager::Unknown,
        ]
    }

    /// Every entry of every shipped conf file, parsed.
    ///
    /// Panics on a parse failure, unlike [`every_shipped_dependency`]: a conf
    /// file this repository ships that does not parse is a red suite, not a
    /// quietly smaller union.
    fn shipped_entries() -> Vec<deps_core::ManifestEntry> {
        SHIPPED_MANIFESTS
            .into_iter()
            .flat_map(|(text, kind)| {
                let manifest: Manifest =
                    parse_manifest_toml(text, kind).expect("every shipped manifest parses");
                manifest.entries().to_vec()
            })
            .collect()
    }

    /// Every path a check reads, recursing into `AnyOf`.
    ///
    /// A clone target must equal one of these. `zsh-autosuggestions` has two
    /// branches, the oh-my-zsh plugin file and the Homebrew share directory,
    /// and only the first is a clone destination.
    /// Every subject of `check`, paired with whether it names a FILE.
    ///
    /// The kind is what decides whether a clone can satisfy the subject at
    /// all: a clone creates a directory, so it can BE a directory subject
    /// but only ever CONTAIN a file subject.
    fn subjects_by_kind(check: &Check) -> Vec<(CheckPath, bool)> {
        match check {
            Check::DirExists(path) => vec![(path.clone(), false)],
            Check::FileExists(path) | Check::FileNonEmpty(path) => vec![(path.clone(), true)],
            // A glob names the directory it searches, which a clone creates.
            Check::GlobExists { dir, .. } => vec![(dir.clone(), false)],
            // Neither names a path, so neither can be a clone target: a
            // version check reads a binary already on PATH.
            Check::Command(_) | Check::CommandVersion { .. } | Check::PythonImport(_) => {
                Vec::new()
            }
            Check::AnyOf { first, rest } => {
                let mut subjects = subjects_by_kind(first);
                for branch in rest {
                    subjects.extend(subjects_by_kind(branch));
                }
                subjects
            }
        }
    }


    /// Parse a dependency name a test wrote as a literal.
    fn name(raw: &str) -> DependencyName {
        DependencyName::parse(raw).expect("a test dependency name parses")
    }

}
