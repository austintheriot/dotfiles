use std::collections::{BTreeMap, BTreeSet};

use dotfiles_path::{CheckRelPath, NameError};

use crate::action::{
    BrewKind, InstallAction, NoInstallReason, PackageAvailability, PackageManager, PackageMap,
};
use crate::check::{Observation, Observations, PathRoot, evaluate};
use crate::manifest::{DependencyName, Manifest, ParseError};

/// The maximum byte length of a raw selector.
const MAX_SELECTOR_LEN: usize = 256;

/// The minimum shared prefix a name needs before it is offered as a
/// suggestion.
///
/// One shared character makes every name that starts with the same letter a
/// candidate, which is noise rather than a suggestion.
const MIN_SUGGESTION_PREFIX: usize = 2;

/// What each dependency can be installed as, per manager.
///
/// A named alias because the map appears in `plan`'s signature and in every
/// caller's local, and the bare nested generic is both unreadable and a
/// `clippy::type_complexity` finding at the call site.
pub type PackageCatalog = BTreeMap<DependencyName, PackageMap>;

/// The elevation state, resolved once at the edge before `gather`.
///
/// `check-deps.sh:168-187` already computes exactly this three-state value,
/// including a `DEPS_FORCE_ROOT` override that exists only so both branches
/// are testable, so naming it a type deletes that environment seam.
///
/// `ViaSudo` is a prediction, not a guarantee: `command -v sudo` proves a
/// binary is on `PATH`, not that the user is in sudoers, that the credential
/// cache is valid, or that NOPASSWD applies. Resolving once is still correct
/// for a different reason: it makes the plan a deterministic function of one
/// observation, which is what keeps `--dry-run` truthful.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elevation {
    /// The process is already root, so a privileged step needs no prefix.
    AlreadyRoot,
    /// `sudo` is on `PATH`, so a privileged step can be attempted.
    ViaSudo,
    /// The machine has neither root nor sudo.
    Unavailable,
}

/// Whether a step must run through the privileged installer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivilegeRequirement {
    /// The step runs as the invoking user.
    None,
    /// The step must go to the privileged slot.
    Root,
}

/// One planned install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The dependency this step is for.
    pub dependency: DependencyName,
    /// What installing it means.
    pub action: InstallAction,
    /// What privilege the action needs, decided here rather than at perform
    /// time.
    pub privilege: PrivilegeRequirement,
}

/// An ordered, heterogeneous collection of steps.
///
/// One `Vec<Step>` rather than phantom-typed `Step<Ready>` and
/// `Step<Blocked>`. With phantom types the options are `Vec<Box<dyn
/// StepLike>>` (which erases the parameter exactly where the driver consumes
/// it), two vectors (which destroys the topological order that is the whole
/// point), or `Vec<Either<..>>`, which is this closed sum written verbosely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The steps, prerequisites first.
    pub steps: Vec<Step>,
}

/// Which dependencies this run considers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection(BTreeSet<DependencyName>);

impl Selection {
    /// Every entry the manifest holds.
    pub fn all(manifest: &Manifest) -> Self {
        Selection(manifest.entries().iter().map(|entry| entry.name.clone()).collect())
    }

    /// Only the named entries, which is what `--only` produces.
    pub fn named(names: Vec<DependencyName>) -> Self {
        Selection(names.into_iter().collect())
    }

    /// Whether `name` is in the selection.
    pub fn contains(&self, name: &DependencyName) -> bool {
        self.0.contains(name)
    }
}

/// The requirement graph.
///
/// A separate argument rather than a manifest field, because no conf file has
/// a `requires` column: the real format is `name|check_command|docs_url`
/// (`deps.conf:2`), and `deps.conf:18-20` states in the file itself that "no
/// ordering between it and zsh-autosuggestions is guaranteed here". Adding
/// the column is a later decision; the planner does not need to invent it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Requirements(BTreeMap<DependencyName, Vec<DependencyName>>);

impl Requirements {
    /// The empty graph, which is what the conf files describe today.
    pub fn none() -> Self {
        Requirements(BTreeMap::new())
    }

    /// Build the graph from dependent-to-prerequisites pairs.
    pub fn from_pairs(pairs: Vec<(DependencyName, Vec<DependencyName>)>) -> Self {
        Requirements(pairs.into_iter().collect())
    }

    /// What `of` requires, in the order the caller listed it.
    pub fn prerequisites(&self, of: &DependencyName) -> &[DependencyName] {
        self.0.get(of).map_or(&[], Vec::as_slice)
    }
}

/// A `--only` value as the caller typed it.
///
/// Its only guarantee is "bounded and safe to render". It exists so
/// `PlanError::MalformedSelector` can quote the input without an unbounded or
/// terminal-active string entering an error type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSelector(String);

impl RawSelector {
    /// Parse a raw selector.
    ///
    /// # Errors
    ///
    /// Returns `NameError::ControlByte` for terminal-active input and
    /// `NameError::TooLong` past 256 bytes.
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        if raw.len() > MAX_SELECTOR_LEN {
            return Err(NameError::TooLong { len: raw.len(), max: MAX_SELECTOR_LEN });
        }
        if raw.chars().any(|character| character.is_control()) {
            return Err(NameError::ControlByte);
        }
        Ok(RawSelector(raw.to_string()))
    }

    /// The validated selector.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The run never started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// The selection named an entry the manifest does not hold.
    UnknownDependency {
        /// The name as selected.
        name: DependencyName,
        /// The closest manifest name, when one is close enough to offer.
        did_you_mean: Option<DependencyName>,
    },
    /// A `--only` value that is not renderable.
    MalformedSelector {
        /// The value as the caller typed it.
        raw: RawSelector,
    },
    /// A conf file did not parse.
    ManifestParse {
        /// The file the caller read.
        path: CheckRelPath,
        /// Which rule the file broke, and on which line.
        detail: ParseError,
    },
    /// Reserved. Nothing constructs it yet: the pipe format carries no
    /// version line, and inventing one would be a manifest change disguised
    /// as a port. The variant exists so a future format change has a place
    /// to report rather than folding into `ManifestParse`.
    ManifestVersion {
        /// The version the file declared.
        found: u32,
        /// The version this build understands.
        supported: u32,
    },
    /// A distinct error from `ManifestParse`, because every line parses fine.
    RequirementCycle {
        /// The cycle, in visit order, with the repeated name last.
        chain: Vec<DependencyName>,
    },
}

/// What the core observed while planning.
///
/// Returned rather than logged. The driver drains these into a `Services`
/// handle that owns the log; `Services` is a parameter to the driver, not to
/// the core. Passing it here would falsify the no-IO claim and contradict
/// spec 3.3, which rejected `Probe` for being exactly a trait the core
/// invokes. Under the fixpoint the vectors concatenate across iterations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The dependency is already present, so it gets no step.
    CheckSatisfied {
        /// The dependency whose check passed.
        dependency: DependencyName,
    },
    /// The check could not be answered, because a root did not resolve.
    CheckUnanswerable {
        /// The dependency whose check was unanswerable.
        dependency: DependencyName,
        /// The root that failed to resolve.
        root: PathRoot,
    },
    /// A step was planned, and this is the privilege it needs.
    StepPlanned {
        /// The dependency the step installs.
        dependency: DependencyName,
        /// The privilege the step needs.
        privilege: PrivilegeRequirement,
    },
    /// A prerequisite is selected but not yet present, so this wave skips
    /// the dependent.
    StepBlocked {
        /// The dependent that is not attempted in this wave.
        dependency: DependencyName,
        /// The prerequisite that is not yet present.
        on: DependencyName,
    },
    /// The prerequisite is not in the selected manifest, which differs from
    /// "requires a dependency that does not exist". `oh-my-zsh` is in
    /// `deps-linux.conf:12` and legitimately absent on macOS, where
    /// `zsh-autosuggestions` installs fine through brew, so
    /// `PlanError::UnknownDependency` would be the wrong error.
    PrerequisiteNotSelected {
        /// The dependent whose requirement is out of scope.
        dependency: DependencyName,
        /// The prerequisite that is not selected.
        on: DependencyName,
    },
}

/// Build the plan for one wave.
///
/// Pure: every input is an argument and the observation arrives as an
/// injected [`Observations`], so this function opens no file and spawns no
/// process. It may order steps. It may not condition an action on another
/// step's outcome, which is why blocking is decided from `observations`
/// rather than from a callback (spec 6.3).
///
/// # Errors
///
/// Returns `PlanError::UnknownDependency` when the selection names an entry
/// the manifest does not hold, and `PlanError::RequirementCycle` naming the
/// chain when the requirement graph does not sort.
pub fn plan(
    manifest: &Manifest,
    manager: PackageManager,
    selection: &Selection,
    requirements: &Requirements,
    observations: &impl Observations,
    elevation: Elevation,
    packages: &PackageCatalog,
) -> Result<(Plan, Vec<Event>), PlanError> {
    let ordered = topological_order(manifest, selection, requirements)?;
    let mut satisfied: BTreeSet<DependencyName> = BTreeSet::new();
    let mut steps = Vec::new();
    let mut events = Vec::new();

    for name in &ordered {
        let entry = manifest.get(name).ok_or_else(|| PlanError::UnknownDependency {
            name: name.clone(),
            did_you_mean: nearest_name(manifest, name),
        })?;

        match evaluate(&entry.check, observations) {
            Observation::Present => {
                satisfied.insert(name.clone());
                events.push(Event::CheckSatisfied { dependency: name.clone() });
                continue;
            }
            Observation::Unresolvable { root } => {
                events.push(Event::CheckUnanswerable { dependency: name.clone(), root });
            }
            Observation::Absent => {}
        }

        if let Some(blocker) = first_unsatisfied_prerequisite(
            name,
            requirements,
            selection,
            manifest,
            &satisfied,
            &mut events,
        ) {
            events.push(Event::StepBlocked { dependency: name.clone(), on: blocker.clone() });
            steps.push(Step {
                dependency: name.clone(),
                action: InstallAction::NotAutomatable {
                    reason: NoInstallReason::PrerequisiteNotYetInstalled { dependency: blocker },
                },
                privilege: PrivilegeRequirement::None,
            });
            continue;
        }

        let unnamed = PackageAvailability::Unavailable(
            NoInstallReason::ManagerNotNamedInManifest { manager },
        );
        let availability = packages
            .get(name)
            .map_or(&unnamed, |per_dependency| per_dependency.resolve(manager));
        let (action, privilege) = action_for(availability, manager, elevation);
        events.push(Event::StepPlanned { dependency: name.clone(), privilege });
        steps.push(Step { dependency: name.clone(), action, privilege });
    }

    Ok((Plan { steps }, events))
}

/// Derive the action and its privilege from availability and elevation.
///
/// The privilege is derived from the `(action, manager)` pair here, once,
/// which is what makes the driver's dispatch an exhaustive match rather than
/// a predicate it could get wrong. `Elevation::Unavailable` never yields a
/// `Root` step: it yields `PrivilegeUnavailable`, so the condition flows
/// through the report and the exit code instead of surfacing at perform time
/// (`check-deps.sh:544-546` already prints this message).
fn action_for(
    availability: &PackageAvailability,
    manager: PackageManager,
    elevation: Elevation,
) -> (InstallAction, PrivilegeRequirement) {
    let (action, wants_root) = match availability {
        PackageAvailability::Named(id) => match manager {
            PackageManager::Brew => (
                InstallAction::Brew { kind: BrewKind::Formula, id: id.clone(), tap: None },
                false,
            ),
            other => (InstallAction::Package { id: id.clone() }, other.needs_root()),
        },
        PackageAvailability::ViaScript(installer) => {
            (InstallAction::Script { installer: *installer }, false)
        }
        PackageAvailability::Unavailable(reason) => {
            (InstallAction::NotAutomatable { reason: reason.clone() }, false)
        }
    };

    if wants_root && elevation == Elevation::Unavailable {
        return (
            InstallAction::NotAutomatable { reason: NoInstallReason::PrivilegeUnavailable },
            PrivilegeRequirement::None,
        );
    }
    let privilege = if wants_root {
        PrivilegeRequirement::Root
    } else {
        PrivilegeRequirement::None
    };
    (action, privilege)
}

/// The first prerequisite that is selected, in the manifest, and not yet
/// present.
///
/// A prerequisite outside the selected manifest records an event and does not
/// block, because it is a legitimate platform difference rather than a
/// missing install.
fn first_unsatisfied_prerequisite(
    name: &DependencyName,
    requirements: &Requirements,
    selection: &Selection,
    manifest: &Manifest,
    satisfied: &BTreeSet<DependencyName>,
    events: &mut Vec<Event>,
) -> Option<DependencyName> {
    for prerequisite in requirements.prerequisites(name) {
        if manifest.get(prerequisite).is_none() || !selection.contains(prerequisite) {
            // Not an error: oh-my-zsh is in deps-linux.conf:12 and
            // legitimately absent on macOS, where zsh-autosuggestions
            // installs through brew.
            events.push(Event::PrerequisiteNotSelected {
                dependency: name.clone(),
                on: prerequisite.clone(),
            });
            continue;
        }
        if !satisfied.contains(prerequisite) {
            return Some(prerequisite.clone());
        }
    }
    None
}

/// How far along a name is in the depth-first walk.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    InProgress,
    Done,
}

/// Order the selection so every prerequisite precedes its dependent.
///
/// # Errors
///
/// Returns `PlanError::RequirementCycle` carrying the chain, because every
/// line parses fine and `ManifestParse` would misreport the cause.
fn topological_order(
    manifest: &Manifest,
    selection: &Selection,
    requirements: &Requirements,
) -> Result<Vec<DependencyName>, PlanError> {
    let mut marks: BTreeMap<DependencyName, Mark> = BTreeMap::new();
    let mut ordered = Vec::new();

    for entry in manifest.entries() {
        if !selection.contains(&entry.name) {
            continue;
        }
        visit(
            &entry.name,
            manifest,
            selection,
            requirements,
            &mut marks,
            &mut Vec::new(),
            &mut ordered,
        )?;
    }
    Ok(ordered)
}

/// Visit one name and everything it requires, prerequisites first.
fn visit(
    name: &DependencyName,
    manifest: &Manifest,
    selection: &Selection,
    requirements: &Requirements,
    marks: &mut BTreeMap<DependencyName, Mark>,
    chain: &mut Vec<DependencyName>,
    ordered: &mut Vec<DependencyName>,
) -> Result<(), PlanError> {
    match marks.get(name) {
        Some(Mark::Done) => return Ok(()),
        Some(Mark::InProgress) => {
            let mut reported = chain.clone();
            reported.push(name.clone());
            return Err(PlanError::RequirementCycle { chain: reported });
        }
        None => {}
    }

    marks.insert(name.clone(), Mark::InProgress);
    chain.push(name.clone());
    for prerequisite in requirements.prerequisites(name) {
        if manifest.get(prerequisite).is_some() && selection.contains(prerequisite) {
            visit(prerequisite, manifest, selection, requirements, marks, chain, ordered)?;
        }
    }
    chain.pop();
    marks.insert(name.clone(), Mark::Done);
    ordered.push(name.clone());
    Ok(())
}

/// The closest manifest name by common prefix length.
///
/// Prefix length rather than an edit distance, because a dependency inside a
/// no-dependency crate cannot pull in a Levenshtein implementation and a
/// hand-rolled one is more code than the suggestion is worth.
fn nearest_name(manifest: &Manifest, wanted: &DependencyName) -> Option<DependencyName> {
    manifest
        .entries()
        .iter()
        .map(|entry| {
            let shared = entry
                .name
                .as_str()
                .chars()
                .zip(wanted.as_str().chars())
                .take_while(|(left, right)| left == right)
                .count();
            (shared, entry.name.clone())
        })
        .filter(|(shared, _)| *shared >= MIN_SUGGESTION_PREFIX)
        .max_by_key(|(shared, _)| *shared)
        .map(|(_, name)| name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Check, ObservationMap};
    use dotfiles_path::{CommandName, PackageId};

    fn dependency(name: &str) -> DependencyName {
        DependencyName::parse(name).expect("a test dependency name parses")
    }

    fn package(name: &str) -> PackageId {
        PackageId::parse(name).expect("a test package id parses")
    }

    fn manifest_of(names: &[&str]) -> Manifest {
        let text: String = names
            .iter()
            .map(|name| format!("{name}|command -v {name}|https://example.invalid/{name}\n"))
            .collect();
        crate::parse_manifest(&text, crate::ConfKind::PlatformSelected)
            .expect("a synthesized manifest parses")
    }

    fn packages_named(names: &[&str]) -> PackageCatalog {
        names
            .iter()
            .map(|name| {
                let mut per_manager = BTreeMap::new();
                per_manager.insert(
                    PackageManager::Apt,
                    PackageAvailability::Named(package(name)),
                );
                per_manager.insert(
                    PackageManager::Brew,
                    PackageAvailability::Named(package(name)),
                );
                (
                    dependency(name),
                    PackageMap::new(
                        per_manager,
                        PackageAvailability::Unavailable(
                            NoInstallReason::ManagerNotNamedInManifest {
                                manager: PackageManager::Unknown,
                            },
                        ),
                    ),
                )
            })
            .collect()
    }

    // Privilege is a property of the manager, not of the dependency: apt
    // needs root and brew never does (spec 3.5). It is derived here rather
    // than sniffed out of command text, which is what check-deps.sh:545
    // does today.
    #[test]
    fn an_apt_package_step_is_marked_root_and_a_brew_step_is_not() {
        let manifest = manifest_of(&["ripgrep"]);
        let selection = Selection::all(&manifest);
        let packages = packages_named(&["ripgrep"]);
        let observations = ObservationMap::from_pairs(vec![]);

        let (apt_plan, _) = plan(
            &manifest,
            PackageManager::Apt,
            &selection,
            &Requirements::none(),
            &observations,
            Elevation::ViaSudo,
            &packages,
        )
        .expect("a one-entry plan succeeds");
        assert_eq!(apt_plan.steps.len(), 1);
        assert_eq!(apt_plan.steps[0].privilege, PrivilegeRequirement::Root);

        let (brew_plan, _) = plan(
            &manifest,
            PackageManager::Brew,
            &selection,
            &Requirements::none(),
            &observations,
            Elevation::ViaSudo,
            &packages,
        )
        .expect("a one-entry plan succeeds");
        assert_eq!(brew_plan.steps[0].privilege, PrivilegeRequirement::None);
    }

    // Elevation::Unavailable never emits a Root step at all. The condition
    // flows through the report and the exit code instead of being
    // discovered at perform time, and check-deps.sh:544-546 already prints
    // this message from its string sniff.
    #[test]
    fn elevation_unavailable_emits_privilege_unavailable_instead_of_a_root_step() {
        let manifest = manifest_of(&["ripgrep"]);
        let selection = Selection::all(&manifest);
        let packages = packages_named(&["ripgrep"]);
        let observations = ObservationMap::from_pairs(vec![]);

        let (built, _) = plan(
            &manifest,
            PackageManager::Apt,
            &selection,
            &Requirements::none(),
            &observations,
            Elevation::Unavailable,
            &packages,
        )
        .expect("a one-entry plan succeeds");
        assert_eq!(built.steps[0].privilege, PrivilegeRequirement::None);
        assert!(matches!(
            built.steps[0].action,
            InstallAction::NotAutomatable {
                reason: NoInstallReason::PrivilegeUnavailable
            }
        ));
        assert!(
            built
                .steps
                .iter()
                .all(|step| step.privilege == PrivilegeRequirement::None),
            "no Root step may be planned when elevation is unavailable"
        );
    }

    // AlreadyRoot still needs the requirement on the step, because the
    // driver's dispatch is an exhaustive match on it rather than a
    // predicate. Root here means "this must go to the privileged slot",
    // and when the process is already root that slot is the ordinary one.
    #[test]
    fn already_root_still_marks_an_apt_step_root() {
        let manifest = manifest_of(&["ripgrep"]);
        let packages = packages_named(&["ripgrep"]);
        let (built, _) = plan(
            &manifest,
            PackageManager::Apt,
            &Selection::all(&manifest),
            &Requirements::none(),
            &ObservationMap::from_pairs(vec![]),
            Elevation::AlreadyRoot,
            &packages,
        )
        .expect("a one-entry plan succeeds");
        assert_eq!(built.steps[0].privilege, PrivilegeRequirement::Root);
    }

    // A satisfied check is not a step. plan is a function of the injected
    // observation, so this is the whole no-IO surface of the planner.
    #[test]
    fn a_present_dependency_gets_no_step_and_one_event() {
        let manifest = manifest_of(&["ripgrep"]);
        let check = Check::Command(CommandName::parse("ripgrep").expect("a name parses"));
        let observations = ObservationMap::from_pairs(vec![(check, Observation::Present)]);
        let (built, events) = plan(
            &manifest,
            PackageManager::Brew,
            &Selection::all(&manifest),
            &Requirements::none(),
            &observations,
            Elevation::ViaSudo,
            &packages_named(&["ripgrep"]),
        )
        .expect("a one-entry plan succeeds");
        assert!(built.steps.is_empty());
        assert_eq!(
            events,
            vec![Event::CheckSatisfied { dependency: dependency("ripgrep") }]
        );
    }

    // plan returns events; it holds no logger. Passing Services in would
    // falsify the no-IO claim and contradict spec 3.3, which rejected Probe
    // for being a trait the core invokes.
    #[test]
    fn plan_is_deterministic_over_the_same_inputs() {
        let manifest = manifest_of(&["ripgrep", "fzf"]);
        let packages = packages_named(&["ripgrep", "fzf"]);
        let selection = Selection::all(&manifest);
        let observations = ObservationMap::from_pairs(vec![]);
        let run = || {
            plan(
                &manifest,
                PackageManager::Apt,
                &selection,
                &Requirements::none(),
                &observations,
                Elevation::ViaSudo,
                &packages,
            )
            .expect("a two-entry plan succeeds")
        };
        assert_eq!(run(), run());
    }

    // deps.conf:36 and :45. node requires nvm, and nvm's own install is
    // manual-only (check-deps.sh:369-372), so on a machine with neither,
    // node is blocked rather than attempted.
    #[test]
    fn a_dependent_is_blocked_when_its_prerequisite_is_absent() {
        let manifest = manifest_of(&["nvm", "node"]);
        let requirements =
            Requirements::from_pairs(vec![(dependency("node"), vec![dependency("nvm")])]);
        let (built, events) = plan(
            &manifest,
            PackageManager::Apt,
            &Selection::all(&manifest),
            &requirements,
            &ObservationMap::from_pairs(vec![]),
            Elevation::ViaSudo,
            &packages_named(&["nvm", "node"]),
        )
        .expect("a two-entry plan succeeds");

        // The ordering claim is deliberately NOT made here. manifest_of
        // writes nvm first, so a planner that ignored the graph entirely
        // would still put nvm at index 0 and this assertion would pass
        // vacuously. Confirmed: with topological_order replaced by the
        // manifest order, this test stays green. The ordering guarantee is
        // pinned by the_graph_reorders_a_dependent_written_before_its_prerequisite,
        // which writes the dependent first.
        let node_step = built
            .steps
            .iter()
            .find(|step| step.dependency == dependency("node"))
            .expect("node has a step");
        assert!(matches!(
            node_step.action,
            InstallAction::NotAutomatable {
                reason: NoInstallReason::PrerequisiteNotYetInstalled { .. }
            }
        ));
        assert!(events.contains(&Event::StepBlocked {
            dependency: dependency("node"),
            on: dependency("nvm"),
        }));
    }

    // The ordering claim above, made non-vacuous. manifest_of writes nvm
    // first, so a planner that ignored the graph entirely would still put
    // nvm at index 0. Here the manifest writes node first, so index 0 is
    // nvm only because topological_order moved it.
    #[test]
    fn the_graph_reorders_a_dependent_written_before_its_prerequisite() {
        let manifest = manifest_of(&["node", "nvm"]);
        let requirements =
            Requirements::from_pairs(vec![(dependency("node"), vec![dependency("nvm")])]);
        let (built, _) = plan(
            &manifest,
            PackageManager::Apt,
            &Selection::all(&manifest),
            &requirements,
            &ObservationMap::from_pairs(vec![]),
            Elevation::ViaSudo,
            &packages_named(&["nvm", "node"]),
        )
        .expect("a two-entry plan succeeds");
        let order: Vec<&str> = built
            .steps
            .iter()
            .map(|step| step.dependency.as_str())
            .collect();
        assert_eq!(order, vec!["nvm", "node"], "the graph outranks the file order");
    }

    // The fixpoint evidence, as a value. check-deps.sh:339-341 emits the
    // zsh-autosuggestions clone only when the oh-my-zsh custom directory
    // exists, so installing oh-my-zsh in wave 1 is what makes
    // zsh-autosuggestions installable in wave 2. One pass does not
    // converge, and this pins that plan() itself does not pretend it does:
    // the same inputs with the prerequisite now Present yield a real step
    // where the earlier wave yielded a block.
    #[test]
    fn a_later_wave_unblocks_a_dependent_once_the_prerequisite_is_present() {
        let manifest = manifest_of(&["oh-my-zsh", "zsh-autosuggestions"]);
        let requirements = Requirements::from_pairs(vec![(
            dependency("zsh-autosuggestions"),
            vec![dependency("oh-my-zsh")],
        )]);
        let packages = packages_named(&["oh-my-zsh", "zsh-autosuggestions"]);
        let run = |observations: &ObservationMap| {
            plan(
                &manifest,
                PackageManager::Apt,
                &Selection::all(&manifest),
                &requirements,
                observations,
                Elevation::ViaSudo,
                &packages,
            )
            .expect("a two-entry plan succeeds")
        };

        let (first_wave, _) = run(&ObservationMap::from_pairs(vec![]));
        let blocked = first_wave
            .steps
            .iter()
            .find(|step| step.dependency == dependency("zsh-autosuggestions"))
            .expect("the dependent has a step in wave 1");
        assert!(matches!(
            blocked.action,
            InstallAction::NotAutomatable {
                reason: NoInstallReason::PrerequisiteNotYetInstalled { .. }
            }
        ));

        let installed = Check::Command(
            CommandName::parse("oh-my-zsh").expect("a name parses"),
        );
        let (second_wave, _) =
            run(&ObservationMap::from_pairs(vec![(installed, Observation::Present)]));
        let unblocked = second_wave
            .steps
            .iter()
            .find(|step| step.dependency == dependency("zsh-autosuggestions"))
            .expect("the dependent has a step in wave 2");
        assert!(
            matches!(unblocked.action, InstallAction::Package { .. }),
            "wave 2 plans the real install: {:?}",
            unblocked.action
        );
    }

    // A prerequisite absent from the selected manifest is not an error.
    // oh-my-zsh is in deps-linux.conf:12 and legitimately not in the macOS
    // manifest, where zsh-autosuggestions installs through brew, so
    // PlanError::UnknownDependency would be the wrong answer.
    #[test]
    fn a_prerequisite_outside_the_selected_manifest_is_an_event_not_an_error() {
        let manifest = manifest_of(&["zsh-autosuggestions"]);
        let requirements = Requirements::from_pairs(vec![(
            dependency("zsh-autosuggestions"),
            vec![dependency("oh-my-zsh")],
        )]);
        let (built, events) = plan(
            &manifest,
            PackageManager::Brew,
            &Selection::all(&manifest),
            &requirements,
            &ObservationMap::from_pairs(vec![]),
            Elevation::ViaSudo,
            &packages_named(&["zsh-autosuggestions"]),
        )
        .expect("an absent prerequisite is not a plan error");
        assert!(events.contains(&Event::PrerequisiteNotSelected {
            dependency: dependency("zsh-autosuggestions"),
            on: dependency("oh-my-zsh"),
        }));
        assert!(matches!(built.steps[0].action, InstallAction::Brew { .. }));
    }

    // A cycle is RequirementCycle, not ManifestParse: every line parses.
    #[test]
    fn a_requirement_cycle_names_its_chain() {
        let manifest = manifest_of(&["fzf", "ripgrep"]);
        let requirements = Requirements::from_pairs(vec![
            (dependency("fzf"), vec![dependency("ripgrep")]),
            (dependency("ripgrep"), vec![dependency("fzf")]),
        ]);
        let failure = plan(
            &manifest,
            PackageManager::Apt,
            &Selection::all(&manifest),
            &requirements,
            &ObservationMap::from_pairs(vec![]),
            Elevation::ViaSudo,
            &packages_named(&["fzf", "ripgrep"]),
        )
        .expect_err("a cycle does not plan");
        let PlanError::RequirementCycle { chain } = failure else {
            panic!("a cycle must be RequirementCycle, not {failure:?}");
        };
        assert!(chain.len() >= 2, "the chain names the cycle: {chain:?}");
    }

    // A manager with no entry resolves through the mandatory fallback, which
    // states its own reason. This is what makes resolve total without
    // fabricating a claim about upstream.
    #[test]
    fn an_unnamed_manager_resolves_through_the_fallback() {
        let manifest = manifest_of(&["ripgrep"]);
        let (built, _) = plan(
            &manifest,
            PackageManager::Unknown,
            &Selection::all(&manifest),
            &Requirements::none(),
            &ObservationMap::from_pairs(vec![]),
            Elevation::ViaSudo,
            &packages_named(&["ripgrep"]),
        )
        .expect("Unknown is a variant, not an error");
        assert!(matches!(
            built.steps[0].action,
            InstallAction::NotAutomatable {
                reason: NoInstallReason::ManagerNotNamedInManifest {
                    manager: PackageManager::Unknown
                }
            }
        ));
    }

    // A dependency the catalog holds no entry for at all, which is a
    // different path from a manager the entry does not name: the first
    // resolves through PackageMap's fallback, the second never reaches a
    // PackageMap. Both must state the same reason rather than one of them
    // panicking or silently producing a Package step.
    #[test]
    fn a_dependency_outside_the_catalog_reports_the_manager_it_asked_for() {
        let manifest = manifest_of(&["ripgrep"]);
        let (built, _) = plan(
            &manifest,
            PackageManager::Apt,
            &Selection::all(&manifest),
            &Requirements::none(),
            &ObservationMap::from_pairs(vec![]),
            Elevation::ViaSudo,
            &PackageCatalog::new(),
        )
        .expect("an uncatalogued dependency is not a plan error");
        assert_eq!(
            built.steps[0].action,
            InstallAction::NotAutomatable {
                reason: NoInstallReason::ManagerNotNamedInManifest {
                    manager: PackageManager::Apt
                }
            }
        );
        assert_eq!(built.steps[0].privilege, PrivilegeRequirement::None);
    }
}
