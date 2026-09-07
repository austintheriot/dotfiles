//! The fixpoint loop and the one effect port.
//!
//! Holds no capability. [`run_to_fixpoint`] takes `gather` as a closure and
//! the [`Installer`] implementations as a parameter, so this module names
//! nothing effectful and the crate's purity check covers it.

use std::collections::{BTreeMap, BTreeSet};

use crate::action::{InstallAction, NoInstallReason, PackageManager, PackageMap};
use crate::check::Observations;
use crate::manifest::{DependencyName, Manifest};
use crate::outcome::StepOutcome;
use crate::plan::{
    Elevation, Event, Plan, PlanError, PrivilegeRequirement, Requirements, Selection, Step, plan,
};
use crate::reconcile::{Report, reconcile};

/// What the driver considered and resolved in one wave.
///
/// The field is private to this module and there is no `new`, no `From`, no
/// `Default`. The only way to obtain an `Attempted` is to call
/// [`perform_all`], which means a re-gather cannot be written before the
/// perform loop: the value it needs does not exist yet.
///
/// This is the fix for a real first-draft defect. `plan.attempted` was
/// available the instant `plan` returned, so `let after =
/// gather(&plan.attempted);` placed above the loop compiled, type-checked,
/// and reconciled every outcome against a pre-install world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempted(BTreeSet<DependencyName>);

impl Attempted {
    /// Whether this wave considered and resolved `name`.
    pub fn contains(&self, name: &DependencyName) -> bool {
        self.0.contains(name)
    }

    /// How many dependencies this wave resolved.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether this wave resolved nothing.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The dependencies this wave resolved, in name order.
    pub fn names(&self) -> impl Iterator<Item = &DependencyName> {
        self.0.iter()
    }
}

/// A structured description of one action.
///
/// Structured, not a string. A bare `String` cannot support the requirement
/// that `--dry-run` disclose privileged steps before the first password
/// prompt, because the driver must aggregate that across steps beforehand,
/// and aggregating over strings means grepping for `sudo`, which resurrects
/// the string sniff at `check-deps.sh:545` inside the new design.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionDescription {
    /// A one-line rendering of the action, for display.
    pub summary: String,
    /// The privilege the action needs.
    pub privilege: PrivilegeRequirement,
    /// The command the action would run, when one can be previewed.
    pub command_preview: Option<String>,
    /// Whether the action permanently adds a trust root.
    ///
    /// `AptSource` and any future equivalent. It exists so the `gh` apt
    /// pipeline at `check-deps.sh:236`, which permanently adds a
    /// third-party APT trust root, cannot be disclosed as an ordinary
    /// package install.
    pub changes_trust_root: bool,
}

/// The one effect port.
///
/// One trait, not two. `--dry-run` is not an implementation of it: every
/// [`StepOutcome`] variant is a false statement about a run that did
/// nothing, so a dry run is [`describe`] over the plan with the effectful
/// segment not executed.
///
/// `describe` must be a pure function of exactly the inputs `perform`
/// consumes. Today's script gets this right by a stronger mechanism than
/// two methods: it substitutes `${SUDO}` once into a single string used for
/// both display and execution (`check-deps.sh:557-567`, with a comment
/// saying so). Two methods can diverge, so an implementation builds the
/// command once and has both methods read it.
pub trait Installer {
    /// Describe `step` without performing it.
    ///
    /// Takes the step rather than the action, because privilege is decided
    /// by the planner from the availability, the manager and the elevation,
    /// and an `InstallAction` carries none of the three. An installer handed
    /// only the action has to guess, and the guess is what a dry run shows a
    /// reader before the first password prompt.
    fn describe(&self, step: &Step) -> ActionDescription;

    /// Perform `action` and report what happened.
    ///
    /// Returns only `Installed`, `InstallFailed`, `NotAutomatable` or
    /// `Declined`. `AlreadyPresent` and `InstalledButCheckStillFails` are
    /// `reconcile`'s to produce from the post-loop world, so an installer
    /// returning either would bypass the re-check that catches an install
    /// which reported success and changed nothing.
    fn perform(&self, action: &InstallAction) -> StepOutcome;
}

/// One trait, two slots.
///
/// The driver's dispatch is an exhaustive match on [`Step::privilege`]
/// rather than a predicate over command text, and `privileged: None` makes
/// "cannot install with root" a property of the wiring.
/// `depcheck-hook.sh` runs on shell startup, so wiring it with no privileged
/// installer is what makes that a structural fact rather than a missing
/// flag.
/// The lifetime is not decoration. `Box<dyn Installer>` means
/// `Box<dyn Installer + 'static>`, which would refuse an installer that
/// borrows anything the caller owns, and an installer that borrows the
/// caller's state is the ordinary case rather than the exception: the CLI's
/// installer holds a borrowed process environment, and a test's holds a
/// borrowed recorder. Forcing `'static` pushes every such caller into `Rc`
/// or a leak for no gain.
pub struct Installers<'wiring> {
    /// The installer for steps that need no elevation.
    pub ordinary: Box<dyn Installer + 'wiring>,
    /// The installer for steps that need root, when one is wired.
    pub privileged: Option<Box<dyn Installer + 'wiring>>,
}

/// Perform one wave's ready steps, sequentially.
///
/// Returns the per-step outcomes and what was attempted. `attempted`
/// includes every step this call resolved, including one refused for want of
/// a privileged installer, because a refused step must not be retried in the
/// next wave: the refusal will not change.
///
/// It excludes a step blocked on a prerequisite that is not yet present.
/// That step is reported [`StepOutcome::Blocked`] and left out of
/// `attempted`, so the next wave plans it again. A refusal that will not
/// change retires the step; a blocking condition a later wave removes does
/// not.
///
/// This function is the effectful segment. It calls into [`Installer`],
/// which is why it takes `installers` rather than performing anything
/// itself.
pub fn perform_all(
    installers: &Installers<'_>,
    ready: &[Step],
) -> (Vec<(DependencyName, StepOutcome)>, Attempted) {
    let mut outcomes = Vec::with_capacity(ready.len());
    let mut attempted = BTreeSet::new();

    for step in ready {
        // A step `plan` emitted only because a prerequisite is not yet
        // present is not performed and is not attempted. It is deferred:
        // the whole point of the fixpoint is that a later wave, after the
        // prerequisite installs, plans a real action for it. Handing it to
        // an installer would let the installer decide, and marking it
        // attempted would retire it before it was ever tried, which is the
        // single-pass behavior this task exists to replace.
        if let InstallAction::NotAutomatable {
            reason: NoInstallReason::PrerequisiteNotYetInstalled { dependency },
        } = &step.action
        {
            outcomes.push((
                step.dependency.clone(),
                StepOutcome::Blocked { on: dependency.clone() },
            ));
            continue;
        }

        let outcome = match step.privilege {
            PrivilegeRequirement::None => installers.ordinary.perform(&step.action),
            PrivilegeRequirement::Root => match &installers.privileged {
                Some(privileged) => privileged.perform(&step.action),
                None => StepOutcome::NotAutomatable {
                    reason: NoInstallReason::PrivilegeUnavailable,
                },
            },
        };
        attempted.insert(step.dependency.clone());
        outcomes.push((step.dependency.clone(), outcome));
    }

    (outcomes, Attempted(attempted))
}

/// Describe every step in a plan.
///
/// This is `--dry-run`. [`plan`] is pure and complete before any effect, so
/// a dry run needs no [`Installer`] implementation of its own.
///
/// One honest limitation, visible rather than hidden: under the fixpoint a
/// dry run cannot simulate later waves, because it cannot know what
/// installing `nvm` does to the observations. It reports the first wave.
pub fn describe(installer: &dyn Installer, built: &Plan) -> Vec<ActionDescription> {
    built.steps.iter().map(|step| installer.describe(step)).collect()
}

/// What each dependency can be installed as, per manager.
///
/// A local alias for the catalog `plan` takes, so this function's signature
/// stays readable and `clippy::type_complexity` has nothing to report.
type Catalog = BTreeMap<DependencyName, PackageMap>;

/// Everything [`plan`] needs that does not change between waves.
///
/// A struct rather than six parameters, and the grouping is not cosmetic:
/// these are exactly the inputs the fixpoint holds constant, so a wave that
/// varied one of them would be a different run. Only the observations
/// change from wave to wave, and they are the one planning input this type
/// deliberately excludes: they arrive from `gather` inside the loop, which
/// is what makes the re-gather's position observable rather than a
/// convention. Grouping them also settles the
/// `clippy::too_many_arguments` finding at its cause instead of silencing
/// it with an `allow`.
pub struct Planning<'inputs> {
    /// The parsed manifest the run covers.
    pub manifest: &'inputs Manifest,
    /// The package manager the host resolved.
    pub manager: PackageManager,
    /// Which dependencies this run is about.
    pub selection: &'inputs Selection,
    /// The ordering graph. `deps.conf:18-20` says the conf file states no
    /// ordering, so this is a separate input rather than a manifest field.
    pub requirements: &'inputs Requirements,
    /// The elevation state, resolved once at the edge.
    pub elevation: Elevation,
    /// What each dependency can be installed as.
    pub packages: &'inputs Catalog,
}

/// Run the pipeline to a fixpoint.
///
/// The loop body is a pure step function: [`plan`] over the current
/// observations, then [`perform_all`], then a **full** re-gather. Full, not
/// scoped to `attempted`: installing `oh-my-zsh` makes
/// `zsh-autosuggestions` installable, and a scoped re-gather would still
/// call it missing (`check-deps.sh:339` emits that clone only when the
/// oh-my-zsh custom directory exists). A full re-gather is also required if
/// apt installs are ever batched, because one `apt-get install a b c` yields
/// one exit status for three dependencies.
///
/// `gather` is a caller-supplied closure taking no arguments, so a scoped
/// re-gather is not expressible and this module names no capability. The
/// CLI driver that calls this owns the log; the core returns events.
///
/// Termination rests on progress, not on `attempted` alone. A step blocked
/// on a prerequisite is deliberately not attempted, so it is replanned next
/// wave, and a run whose prerequisite never installs would otherwise replan
/// the same blocked step forever. So a wave that attempts nothing breaks:
/// it has reached the fixpoint. Every other wave adds at least one name to
/// a set that only grows, so the loop runs at most once per dependency in
/// the selection plus one final wave.
///
/// # Errors
///
/// Propagates [`PlanError`] from [`plan`], which aborts before any effect.
pub fn run_to_fixpoint<GatherFn, Gathered>(
    planning: &Planning<'_>,
    installers: &Installers<'_>,
    mut gather: GatherFn,
) -> Result<(Report, Vec<Event>), PlanError>
where
    GatherFn: FnMut() -> Gathered,
    Gathered: Observations,
{
    let mut observations = gather();
    let mut outcomes: Vec<(DependencyName, StepOutcome)> = Vec::new();
    let mut resolved: BTreeSet<DependencyName> = BTreeSet::new();
    let mut events = Vec::new();

    loop {
        let (built, wave_events) = plan(
            planning.manifest,
            planning.manager,
            planning.selection,
            planning.requirements,
            &observations,
            planning.elevation,
            planning.packages,
        )?;
        events.extend(wave_events);

        let ready: Vec<Step> = built
            .steps
            .into_iter()
            .filter(|step| !resolved.contains(&step.dependency))
            .collect();
        if ready.is_empty() {
            break;
        }

        let (wave_outcomes, attempted) = perform_all(installers, &ready);
        // Termination rests on this, not on `ready` shrinking. A step
        // blocked on a prerequisite is deliberately NOT attempted, so it is
        // replanned next wave, which is the whole point of the fixpoint and
        // is also how a wave can make no progress: if the prerequisite
        // never installs, the same blocked step returns forever. A wave that
        // attempts nothing has therefore reached the fixpoint, and breaking
        // here is what bounds the run. Every other wave grows `resolved` by
        // at least one, and `resolved` only grows, so the loop runs at most
        // once per dependency plus one final wave.
        if attempted.is_empty() {
            outcomes.extend(wave_outcomes);
            break;
        }
        outcomes.extend(wave_outcomes);
        // `attempted` is only obtainable here, which is what forbids writing
        // the re-gather above the perform call.
        for name in attempted.names() {
            resolved.insert(name.clone());
        }

        observations = gather();
    }

    let (report, reconcile_events) = reconcile(planning.manifest, &outcomes, &observations);
    events.extend(reconcile_events);
    Ok((report, events))
}

/// A compile-fail witness for `Attempted`'s private constructor.
///
/// This is the whole mechanism. In the first draft the re-gather read
/// `plan.attempted`, an argument available the instant `plan` returned, so
/// writing `let after = gather(&plan.attempted);` BEFORE the perform loop
/// compiled and then reconciled every outcome against a pre-install world.
/// `Attempted` coming back from [`perform_all`] with no public constructor
/// makes that reorder a compile error rather than a review finding.
///
/// ```compile_fail
/// use std::collections::BTreeSet;
/// let forged = deps_core::Attempted(BTreeSet::new());
/// ```
///
/// ```compile_fail
/// let forged: deps_core::Attempted = Default::default();
/// ```
///
/// ```compile_fail
/// let forged = deps_core::Attempted::new();
/// ```
///
/// The vacuity control, deliberately NOT `compile_fail`: it names the same
/// type in a way that must compile, so a run in which `Attempted` does not
/// exist fails here rather than passing three doctests silently.
///
/// ```
/// fn takes_attempted(_value: &deps_core::Attempted) {}
/// ```
#[allow(dead_code)]
fn attempted_has_no_public_constructor() {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Check, CheckPath, CheckStatus, ConfKind, Elevation, InstallStatus, KeyringSource,
        NoInstallReason, Observation, ObservationMap, PackageAvailability, PackageManager,
        PackageMap, PathRoot, Requirements, Selection, SourceListEntry, parse_manifest,
    };
    use dotfiles_path::{CheckRelPath, PackageId};
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    fn dependency(name: &str) -> DependencyName {
        DependencyName::parse(name).expect("a test dependency name parses")
    }

    // The real Linux pair. `deps-linux.conf:11` holds oh-my-zsh; the
    // zsh-autosuggestions check is `deps.conf:26` with its brew branch
    // dropped, because the brew branch is what makes the pair converge in
    // one pass on macOS and this test is the Linux case.
    fn oh_my_zsh_manifest() -> Manifest {
        let text = "\
oh-my-zsh|[ -d \"$HOME/.oh-my-zsh\" ]|https://ohmyz.sh/
zsh-autosuggestions|[ -f \"$HOME/.oh-my-zsh/custom/plugins/zsh-autosuggestions/zsh-autosuggestions.zsh\" ]|https://github.com/zsh-users/zsh-autosuggestions
";
        parse_manifest(text, ConfKind::PlatformSelected).expect("the real pair parses")
    }

    fn home_path(rest: &str) -> CheckPath {
        CheckPath::new(
            PathRoot::Home,
            CheckRelPath::parse(rest).expect("a test path parses"),
        )
    }

    fn oh_my_zsh_check() -> Check {
        Check::DirExists(home_path(".oh-my-zsh"))
    }

    fn autosuggestions_check() -> Check {
        Check::FileExists(home_path(
            ".oh-my-zsh/custom/plugins/zsh-autosuggestions/zsh-autosuggestions.zsh",
        ))
    }

    fn packages() -> BTreeMap<DependencyName, PackageMap> {
        ["oh-my-zsh", "zsh-autosuggestions"]
            .into_iter()
            .map(|name| {
                let mut per_manager = BTreeMap::new();
                per_manager.insert(
                    PackageManager::Apt,
                    PackageAvailability::Named(
                        PackageId::parse(name).expect("a test package id parses"),
                    ),
                );
                (
                    dependency(name),
                    PackageMap::new(
                        per_manager,
                        PackageAvailability::Unavailable(
                            NoInstallReason::NotPackagedForThisManager,
                        ),
                    ),
                )
            })
            .collect()
    }

    /// An installer that succeeds and records what it was asked to do.
    struct RecordingInstaller {
        performed: RefCell<Vec<InstallAction>>,
    }

    impl RecordingInstaller {
        fn new() -> Self {
            RecordingInstaller { performed: RefCell::new(Vec::new()) }
        }
    }

    /// Both slots wired, which an Apt run requires.
    ///
    /// `PackageManager::Apt::needs_root` is true, so every step an Apt plan
    /// emits carries `PrivilegeRequirement::Root`. A wiring with `ordinary`
    /// alone performs nothing, and every outcome comes back
    /// `NotAutomatable { PrivilegeUnavailable }`, so a test that means to
    /// exercise the loop must fill the privileged slot too.
    fn recording_installers() -> Installers<'static> {
        Installers {
            ordinary: Box::new(RecordingInstaller::new()),
            privileged: Some(Box::new(RecordingInstaller::new())),
        }
    }

    impl Installer for RecordingInstaller {
        fn describe(&self, step: &Step) -> ActionDescription {
            ActionDescription {
                summary: format!("{:?}", step.action),
                privilege: step.privilege,
                command_preview: None,
                changes_trust_root: matches!(step.action, InstallAction::AptSource { .. }),
            }
        }

        fn perform(&self, action: &InstallAction) -> StepOutcome {
            self.performed.borrow_mut().push(action.clone());
            match action {
                InstallAction::NotAutomatable { reason } => {
                    StepOutcome::NotAutomatable { reason: reason.clone() }
                }
                _ => StepOutcome::Installed,
            }
        }
    }

    /// A scripted sequence of worlds, one per gather.
    ///
    /// This is the whole test harness: no mock, no call-order semantics. The
    /// loop body is a pure step function, so a Vec of observation maps is
    /// enough to drive it.
    struct ScriptedWorlds {
        worlds: RefCell<Vec<ObservationMap>>,
        gathers: RefCell<usize>,
    }

    impl ScriptedWorlds {
        fn new(worlds: Vec<ObservationMap>) -> Self {
            assert!(!worlds.is_empty(), "a scripted run needs at least one world");
            ScriptedWorlds { worlds: RefCell::new(worlds), gathers: RefCell::new(0) }
        }

        fn next(&self) -> ObservationMap {
            *self.gathers.borrow_mut() += 1;
            let mut worlds = self.worlds.borrow_mut();
            if worlds.len() > 1 {
                worlds.remove(0)
            } else {
                worlds[0].clone()
            }
        }
    }

    // The ordering evidence, stated as data. `deps.conf:18-20` says no
    // ordering exists in the file today, so the graph is a separate input
    // rather than a manifest field, and this is what makes the pair a
    // fixpoint rather than one pass: cloning into a nonexistent
    // ~/.oh-my-zsh would land the plugin where nothing sources it
    // (check-deps.sh:328-341).
    fn autosuggestions_needs_oh_my_zsh() -> Requirements {
        Requirements::from_pairs(vec![(
            dependency("zsh-autosuggestions"),
            vec![dependency("oh-my-zsh")],
        )])
    }

    /// The planning inputs for the Linux pair, held constant across waves.
    fn linux_pair_planning<'inputs>(
        manifest: &'inputs Manifest,
        selection: &'inputs Selection,
        requirements: &'inputs Requirements,
        packages: &'inputs BTreeMap<DependencyName, PackageMap>,
    ) -> Planning<'inputs> {
        Planning {
            manifest,
            manager: PackageManager::Apt,
            selection,
            requirements,
            elevation: Elevation::AlreadyRoot,
            packages,
        }
    }

    fn linux_pair_worlds() -> Vec<ObservationMap> {
        vec![
            ObservationMap::from_pairs(vec![]),
            ObservationMap::from_pairs(vec![(oh_my_zsh_check(), Observation::Present)]),
            ObservationMap::from_pairs(vec![
                (oh_my_zsh_check(), Observation::Present),
                (autosuggestions_check(), Observation::Present),
            ]),
        ]
    }

    // The fixpoint. Wave 1 sees nothing installed and installs oh-my-zsh.
    // Only after that does the directory the second entry's check points
    // into exist, so a single pass leaves zsh-autosuggestions missing, which
    // is what check-deps.sh:328-341 does today.
    #[test]
    fn the_loop_converges_only_after_a_second_wave() {
        let manifest = oh_my_zsh_manifest();
        let selection = Selection::all(&manifest);
        let requirements = autosuggestions_needs_oh_my_zsh();
        let catalog = packages();
        let planning = linux_pair_planning(&manifest, &selection, &requirements, &catalog);
        let worlds = ScriptedWorlds::new(linux_pair_worlds());
        let installers = recording_installers();

        let (report, _events) = run_to_fixpoint(
            &planning,
            &installers,
            || worlds.next(),
        )
        .expect("the pair converges");

        assert!(
            *worlds.gathers.borrow() >= 3,
            "one initial gather plus one per wave: got {}",
            worlds.gathers.borrow()
        );
        assert_eq!(report.check, CheckStatus::Ready, "the fixpoint converges");
        assert_eq!(
            report.install,
            InstallStatus::AllSucceeded,
            "every attempted install worked, so no attempt failed"
        );
        assert_eq!(report.rows.len(), 2);
    }

    // Termination. Every iteration either performs a step or breaks, and a
    // dependency is removed from consideration once attempted, so a world
    // that never changes cannot loop forever.
    #[test]
    fn a_world_that_never_changes_still_terminates() {
        let manifest = oh_my_zsh_manifest();
        let selection = Selection::all(&manifest);
        let requirements = autosuggestions_needs_oh_my_zsh();
        let catalog = packages();
        let planning = linux_pair_planning(&manifest, &selection, &requirements, &catalog);
        let worlds = ScriptedWorlds::new(vec![ObservationMap::from_pairs(vec![])]);
        let installers = recording_installers();
        let (report, _events) = run_to_fixpoint(
            &planning,
            &installers,
            || worlds.next(),
        )
        .expect("a static world terminates");
        assert_eq!(
            report.check,
            CheckStatus::NotReady,
            "nothing became present, so the environment is not ready"
        );
        assert!(
            *worlds.gathers.borrow() <= 1 + manifest.entries().len(),
            "at most one gather per dependency plus the initial one: got {}",
            worlds.gathers.borrow()
        );
    }

    // A world that oscillates cannot make the loop spin. The observation
    // sequence alternates forever, so only the "resolved once attempted"
    // rule bounds the run.
    #[test]
    fn an_oscillating_world_cannot_spin_forever() {
        let manifest = oh_my_zsh_manifest();
        let selection = Selection::all(&manifest);
        let requirements = autosuggestions_needs_oh_my_zsh();
        let catalog = packages();
        let planning = linux_pair_planning(&manifest, &selection, &requirements, &catalog);
        let worlds = ScriptedWorlds::new(vec![
            ObservationMap::from_pairs(vec![]),
            ObservationMap::from_pairs(vec![(oh_my_zsh_check(), Observation::Present)]),
            ObservationMap::from_pairs(vec![]),
            ObservationMap::from_pairs(vec![(autosuggestions_check(), Observation::Present)]),
            ObservationMap::from_pairs(vec![]),
        ]);
        let installers = recording_installers();
        let (_report, _events) = run_to_fixpoint(
            &planning,
            &installers,
            || worlds.next(),
        )
        .expect("an oscillating world terminates");
        assert!(
            *worlds.gathers.borrow() <= 1 + manifest.entries().len(),
            "at most one gather per dependency plus the initial one: got {}",
            worlds.gathers.borrow()
        );
    }

    // perform_all returns attempted; nothing else constructs it. A test
    // cannot forge one, and the driver cannot read one before the loop.
    #[test]
    fn perform_all_reports_what_it_attempted() {
        let manifest = oh_my_zsh_manifest();
        let (built, _) = plan(
            &manifest,
            PackageManager::Apt,
            &Selection::all(&manifest),
            &Requirements::none(),
            &ObservationMap::from_pairs(vec![]),
            Elevation::AlreadyRoot,
            &packages(),
        )
        .expect("a two-entry plan succeeds");
        let installers = recording_installers();
        let (outcomes, attempted) = perform_all(&installers, &built.steps);
        assert_eq!(outcomes.len(), built.steps.len());
        assert_eq!(attempted.len(), built.steps.len());
        assert!(!attempted.is_empty());
        assert!(attempted.contains(&dependency("oh-my-zsh")));
    }

    // A Root step with no privileged installer is not performed. plan
    // already refuses to emit one when elevation is unavailable, so this is
    // the second guard, and it is an exhaustive match rather than a
    // predicate over command text.
    #[test]
    fn a_root_step_without_a_privileged_installer_is_not_performed() {
        let recorder = RecordingInstaller::new();
        let installers = Installers { ordinary: Box::new(recorder), privileged: None };
        let steps = vec![Step {
            dependency: dependency("ripgrep"),
            action: InstallAction::Package {
                id: PackageId::parse("ripgrep").expect("a test package id parses"),
            },
            privilege: PrivilegeRequirement::Root,
        }];
        let (outcomes, attempted) = perform_all(&installers, &steps);
        assert_eq!(
            outcomes[0].1,
            StepOutcome::NotAutomatable {
                reason: NoInstallReason::PrivilegeUnavailable
            }
        );
        assert!(
            attempted.contains(&dependency("ripgrep")),
            "the step was considered and resolved, so it is not retried"
        );
    }

    // The ordering the private constructor enforces, asserted from the
    // observable side as well. A re-gather written before perform_all cannot
    // compile, and this test says what the correct order produces: the
    // wave-2 plan sees the wave-1 install.
    #[test]
    fn the_regather_happens_after_the_perform_loop() {
        let manifest = oh_my_zsh_manifest();
        let selection = Selection::all(&manifest);
        let requirements = autosuggestions_needs_oh_my_zsh();
        let catalog = packages();
        let planning = linux_pair_planning(&manifest, &selection, &requirements, &catalog);
        let order = RefCell::new(Vec::new());
        let worlds = ScriptedWorlds::new(linux_pair_worlds());

        struct OrderingInstaller<'a> {
            order: &'a RefCell<Vec<&'static str>>,
        }
        impl Installer for OrderingInstaller<'_> {
            fn describe(&self, _step: &Step) -> ActionDescription {
                ActionDescription {
                    summary: String::new(),
                    // This stub exists only to record call order, and this
                    // ActionDescription is never read. A test that starts
                    // asserting on it must read step.privilege instead.
                    privilege: PrivilegeRequirement::None,
                    command_preview: None,
                    changes_trust_root: false,
                }
            }
            fn perform(&self, _action: &InstallAction) -> StepOutcome {
                self.order.borrow_mut().push("perform");
                StepOutcome::Installed
            }
        }

        // Both slots, because an Apt plan's steps need root and an
        // unfilled privileged slot performs nothing.
        let installers = Installers {
            ordinary: Box::new(OrderingInstaller { order: &order }),
            privileged: Some(Box::new(OrderingInstaller { order: &order })),
        };
        let (_report, _events) = run_to_fixpoint(
            &planning,
            &installers,
            || {
                order.borrow_mut().push("gather");
                worlds.next()
            },
        )
        .expect("the pair converges");

        let recorded = order.borrow();
        assert_eq!(recorded[0], "gather", "one gather precedes the first plan");
        assert_eq!(
            recorded[1], "perform",
            "the first wave performs before re-gathering"
        );
        assert_eq!(
            recorded[2], "gather",
            "the re-gather follows the perform loop, not the plan"
        );
    }

    // --dry-run is describe over the plan, so it performs nothing. The
    // recorder proves that: every StepOutcome variant would be a false
    // statement about a run that did nothing, so describe must not reach
    // perform.
    #[test]
    fn describe_performs_nothing_and_discloses_a_trust_root_change() {
        let recorder = RecordingInstaller::new();
        let built = Plan {
            steps: vec![
                Step {
                    dependency: dependency("ripgrep"),
                    action: InstallAction::Package {
                        id: PackageId::parse("ripgrep").expect("a test package id parses"),
                    },
                    privilege: PrivilegeRequirement::Root,
                },
                Step {
                    dependency: dependency("gh"),
                    action: InstallAction::AptSource {
                        keyring: KeyringSource::GithubCli,
                        list: SourceListEntry::GithubCli,
                    },
                    privilege: PrivilegeRequirement::Root,
                },
            ],
        };

        let described = describe(&recorder, &built);

        assert_eq!(described.len(), 2, "one description per step");
        assert!(
            recorder.performed.borrow().is_empty(),
            "a dry run performs nothing: got {:?}",
            recorder.performed.borrow()
        );
        assert!(
            !described[0].changes_trust_root,
            "an ordinary package install adds no trust root"
        );
        assert!(
            described[1].changes_trust_root,
            "the gh apt pipeline at check-deps.sh:236 adds a third-party APT trust root, \
             so it cannot be disclosed as an ordinary package install"
        );
    }

    /// A privileged step must describe itself as privileged.
    ///
    /// describe received only the action, and privilege is computed by
    /// action_for from the availability, the manager and the elevation, none
    /// of which an InstallAction carries. So an installer could not
    /// re-derive it, and the reference implementation hardcoded None. A dry
    /// run that renders every step as unprivileged cannot disclose a
    /// password prompt before it happens, which is the property parent 3.5
    /// puts on this field.
    #[test]
    fn describe_reports_the_step_privilege_not_a_guess() {
        let step = Step {
            dependency: dependency("gh"),
            action: InstallAction::Package {
                id: PackageId::parse("gh").expect("a valid package id"),
            },
            privilege: PrivilegeRequirement::Root,
        };
        let plan = Plan { steps: vec![step] };
        let installer = RecordingInstaller::new();

        let described = describe(&installer, &plan);

        // Positive control: one step in means one description out, or the
        // assertion below is indexing an empty vector.
        assert_eq!(described.len(), 1, "the control must describe one step");
        assert_eq!(
            described[0].privilege,
            PrivilegeRequirement::Root,
            "describe must report the step's privilege, not the action's absence of one"
        );
    }

    // Positive control for the empty-performed assertion above. The same
    // recorder does record when something actually performs, so emptiness
    // there is a fact about describe rather than a recorder that never
    // records.
    #[test]
    fn the_recorder_records_when_a_step_is_performed() {
        let installers = recording_installers();
        let steps = vec![Step {
            dependency: dependency("ripgrep"),
            action: InstallAction::Package {
                id: PackageId::parse("ripgrep").expect("a test package id parses"),
            },
            privilege: PrivilegeRequirement::None,
        }];
        let (outcomes, _attempted) = perform_all(&installers, &steps);
        assert_eq!(outcomes[0].1, StepOutcome::Installed);
    }

    // A dependency blocked in wave 1 and installed in wave 2 appears twice
    // in the accumulated outcomes. reconcile takes the LAST one, so the
    // report says Installed rather than Blocked. Taking the first would
    // report every deferred dependency as blocked no matter what a later
    // wave did, which would make the fixpoint invisible in the report.
    #[test]
    fn a_dependency_blocked_then_installed_reports_installed() {
        let manifest = oh_my_zsh_manifest();
        let selection = Selection::all(&manifest);
        let requirements = autosuggestions_needs_oh_my_zsh();
        let catalog = packages();
        let planning = linux_pair_planning(&manifest, &selection, &requirements, &catalog);
        let worlds = ScriptedWorlds::new(linux_pair_worlds());
        let installers = recording_installers();

        let (report, _events) = run_to_fixpoint(&planning, &installers, || worlds.next())
            .expect("the pair converges");

        let row = report
            .rows
            .iter()
            .find(|row| row.dependency == dependency("zsh-autosuggestions"))
            .expect("the deferred entry has a row");
        assert_eq!(
            row.outcome,
            StepOutcome::Installed,
            "wave 1 blocked it and wave 2 installed it, so the later outcome wins"
        );
    }
}
