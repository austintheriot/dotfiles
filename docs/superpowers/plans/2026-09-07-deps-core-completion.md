# `deps-core` Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the seven defects a seven-lens expert review found inside
`crates/deps-core`, so the `config-cli` adapter can express every install the
manifest requires and report the truth about what it did.

**Architecture:** All work is inside two existing crates that already hold
130 passing tests and a purity test with a positive control. Nothing gains
IO, no crate is added, and no `Cargo.toml` dependency changes. Four tasks
extend types the planner can reach; three fix contracts that currently
report success where they should not.

**Tech Stack:** Rust 2024 edition, toolchain pinned to 1.94.0 by
`crates/rust-toolchain.toml` (rustup honours it only when the working
directory is under `crates/`). No new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-07-deps-core-completion-design.md`

## Global Constraints

Copied verbatim from the spec and from `~/.claude/CLAUDE.md`, because a
paraphrase is how a constraint gets lost.

- **`deps-core` must contain ZERO references to `std::fs`, `std::process`,
  `std::env`, `std::io`, or `Command::new`.** The purity test at
  `lib.rs:44-121` enforces it by reading its own sources with `include_str!`.
  **No forbidden path may appear as a literal in any scanned file, INCLUDING
  inside a doc comment**, because the test scans the file that holds its own
  needles. Needles are assembled from runtime segments for that reason.
  Add every new module to the array at `lib.rs:58-65` in the same commit
  that creates it.
- **`deps-core` depends on `dotfiles-path` ONLY.** No edge to
  `config-manifest`, which would drag a 248-line git module into the
  dependency domain.
- `cargo clippy --locked --all-targets -- -D warnings` currently reports **0
  errors**. Keep it at 0. The `-D warnings` gate became usable only in
  `c8287694`; do not spend it.
- **Always `cd ~/crates` before any cargo command.** The toolchain pin is
  directory-scoped.
- **Any change under `crates/` moves the workspace build stamp**, which makes
  the installed `~/.local/bin/config-manifest` stale, which the live pre-push
  gate BLOCKS. After each task run `config build`, then confirm
  `config doctor` exits 0 and prints nothing.
- **Use `config commit -F <file>` with a heredoc, NEVER `config commit -m`.**
  A message containing double quotes or `$` gets shredded by the shell into
  pathspec errors. This has happened twice.
- **Do NOT run `config status -uall` and do NOT run `config stash`.** Both
  walk all of `$HOME` in this bare repo and can leave `.cfg/index.lock` held,
  which blocks `config add`.
- NO em dashes anywhere. Use a comma, a colon, parentheses, or two hyphens.
- NO emoji.
- NO single-letter variable names except numeric loop indices `i`, `j`, `k`.
  **Closure parameters included:** write `.map(|step| step.dependency)`,
  never `|s|`. Prefer descriptive generic names over `T`/`K`/`V`.
- Comment WHY not WHAT. No comment restating the code or the type signature.
- Rust: no `unwrap()` or `expect()` in non-test code without a proven
  invariant. Doc comments on public items, with `# Errors` where they apply.
- Every empty-expected assertion needs a positive control FIRST.
- **An assertion is not a test until you have observed it fail against the
  unfixed code.** After a test passes, revert the implementation it covers,
  re-run, confirm red, restore. Report both observations. The previous plan's
  tasks did six, eight, twelve and twenty such checks; match that standard.
- NEVER `--no-verify`. NEVER disable a test instead of fixing it. NEVER
  commit a red suite.

## File Structure

Every file already exists except `render.rs`. Sizes measured before work:

| File | Lines | Responsibility after this plan |
|---|---|---|
| `action.rs` | 281 | `InstallAction`, `PackageAvailability`, `PackageMap`. Gains the four availability variants that reach the unreachable actions. |
| `plan.rs` | 1010 | `plan`, `action_for`, `Requirements`, `Selection`. Gains a validating constructor and the prerequisite-event split. |
| `outcome.rs` | 450 | `StepOutcome`, the summaries, `Verdict`, `ExitStatus`. Gains `Declined` and a `CheckStatus` on `Verdict::Install`. |
| `driver.rs` | 833 | `Installer`, `Attempted`, the fixpoint loop. `describe` changes signature. |
| `check.rs` | 642 | `Check`, `PathRoot`, `Observation`. Loses `OhMyZshCustom`. |
| `render.rs` | **new** | `Rendered`, `render`. |
| `reconcile.rs` | 246 | Unchanged. |

**`render` gets its own file rather than growing `outcome.rs`.** `plan.rs` at
1010 lines is already the largest module in the crate and `outcome.rs` at 450
holds four distinct concerns; adding a renderer to either makes the file
harder to hold in context for no gain. `render.rs` has one responsibility,
consumes `Report` from `reconcile.rs`, and produces a value `main` drains.

---

## Task 1: Reach the four unreachable `InstallAction` variants

`action_for` (`plan.rs:347-380`) builds actions solely from
`PackageAvailability`, which has three variants (`Named`, `ViaScript`,
`Unavailable`) reaching four actions (`Package`, `Brew`, `Script`,
`NotAutomatable`). Verified by grep: `InstallAction::GitClone`,
`::NvmInstall` and `::Pip` have **zero** references in the crate, and
`::AptSource` has two, both declaration.

So `plan` cannot emit the action four of the 22 dependencies need:
`zsh-autosuggestions` (GitClone), `node` (NvmInstall), `pyyaml` (Pip), and
`gh` on apt (AptSource). The adapter step names zsh-autosuggestions
convergence as its own acceptance test, and the action it needs cannot be
planned.

**Files:**
- Modify: `crates/deps-core/src/action.rs:245-253` (`PackageAvailability`)
- Modify: `crates/deps-core/src/plan.rs:347-380` (`action_for`)

**Interfaces:**
- Consumes: nothing from earlier tasks; this is first.
- Produces: `PackageAvailability` gains four variants:
  `AptWithSource { keyring: KeyringSource, list: SourceListEntry }`,
  `PipDistribution { id: PackageId, break_system_packages: bool }`,
  `Clone { source: CloneSource, into: CheckPath }`, and `ViaNvm`.
  `action_for` maps each to its `InstallAction` counterpart. Task 5's
  requirement table and the adapter's catalog both construct these.

- [ ] **Step 1: Write the failing test**

Add to `plan.rs`'s test module. `packages_named` builds a catalog of
`Named` entries only, so this test builds its own catalog with the new
variant.

```rust
    /// `plan` must be able to emit a `GitClone`, which is the action
    /// zsh-autosuggestions needs under any manager but brew.
    ///
    /// Before this task, `PackageAvailability` had no variant that reached
    /// `InstallAction::GitClone`, so the action had zero construction sites
    /// in the crate and four of the 22 dependencies could not be installed.
    #[test]
    fn a_clone_availability_plans_a_git_clone() {
        let manifest = manifest_of(&["zsh-autosuggestions"]);
        let into = CheckPath::new(
            PathRoot::Home,
            CheckRelPath::parse(".oh-my-zsh/custom/plugins/zsh-autosuggestions")
                .expect("a valid relative path"),
        );
        let availability = PackageAvailability::Clone {
            source: CloneSource::ZshAutosuggestions,
            into: into.clone(),
        };
        let mut packages = PackageCatalog::new();
        packages.insert(
            dependency("zsh-autosuggestions"),
            PackageMap::new(BTreeMap::new(), availability),
        );

        // Positive control: a catalog whose only entry is the new variant
        // must still plan exactly one step, or an assertion about that
        // step's action would be reasoning about an empty plan.
        let (built, _events) = plan(
            &manifest,
            PackageManager::Pacman,
            &Selection::all(&manifest),
            &Requirements::none(),
            &ObservationMap::default(),
            Elevation::AlreadyRoot,
            &packages,
        )
        .expect("a clone availability plans");
        assert_eq!(built.steps.len(), 1, "the control must plan one step");

        assert_eq!(
            built.steps[0].action,
            InstallAction::GitClone { source: CloneSource::ZshAutosuggestions, into },
        );
        assert_eq!(
            built.steps[0].privilege,
            PrivilegeRequirement::None,
            "a clone into $HOME needs no root"
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd ~/crates && cargo test --locked -p deps-core a_clone_availability`
Expected: FAIL to compile, with
`no variant or associated item named 'Clone' found for enum 'PackageAvailability'`.
That is the right failure: the variant genuinely does not exist. A failure
naming `CheckPath::new`, `CheckRelPath::parse` or `PackageMap::new` instead
means a helper signature differs from this plan; read the real one before
continuing. Verified at the time of writing: `CheckPath::new(root,
CheckRelPath)` and `PackageMap::new(BTreeMap, PackageAvailability)`. There is
no `CheckPath::parse`.

- [ ] **Step 3: Add the four variants**

In `action.rs`, extend `PackageAvailability`. Keep the existing three
variants first and unchanged, because `Named` and `ViaScript` are matched by
`action_for`'s brew special case and reordering costs nothing but reads as a
behavior change in review.

```rust
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
```

- [ ] **Step 4: Map them in `action_for`**

In `plan.rs:352`, add four arms to the `match availability`. Each pairs the
action with whether it wants root, which the existing code then converts to
a `PrivilegeRequirement` and downgrades to `NotAutomatable` when elevation
is unavailable.

```rust
        // Adds an APT trust root, so it needs root on every manager that
        // has apt at all. `needs_root` is asked of the manager rather than
        // hardcoded, so a brew machine with an apt-shaped availability is
        // not silently escalated.
        PackageAvailability::AptWithSource { keyring, list } => (
            InstallAction::AptSource { keyring: keyring.clone(), list: list.clone() },
            manager.needs_root(),
        ),
        // pip installs into the user site directory, so no root.
        PackageAvailability::PipDistribution { id, break_system_packages } => (
            InstallAction::Pip {
                id: id.clone(),
                break_system_packages: *break_system_packages,
            },
            false,
        ),
        // A clone lands under $HOME, so no root.
        PackageAvailability::Clone { source, into } => (
            InstallAction::GitClone { source: *source, into: into.clone() },
            false,
        ),
        PackageAvailability::ViaNvm => (InstallAction::NvmInstall, false),
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cd ~/crates && cargo test --locked -p deps-core a_clone_availability`
Expected: PASS.

Then the whole crate, because `action_for`'s match is exhaustive and four new
variants may have broken another arm:

Run: `cd ~/crates && cargo test --locked -p deps-core`
Expected: PASS, 61 or more tests.

- [ ] **Step 6: Add the other three tests**

One per variant, each asserting the action AND the privilege, because
privilege is what `action_for` computes and the review found a case where it
was silently wrong.

```rust
    #[test]
    fn an_apt_source_availability_needs_root_on_apt() {
        let manifest = manifest_of(&["gh"]);
        let availability = PackageAvailability::AptWithSource {
            keyring: KeyringSource::GithubCli,
            list: SourceListEntry::GithubCli,
        };
        let mut packages = PackageCatalog::new();
        packages.insert(dependency("gh"), PackageMap::new(BTreeMap::new(), availability));

        let (built, _events) = plan(
            &manifest,
            PackageManager::Apt,
            &Selection::all(&manifest),
            &Requirements::none(),
            &ObservationMap::default(),
            Elevation::AlreadyRoot,
            &packages,
        )
        .expect("an apt-source availability plans");
        assert_eq!(built.steps.len(), 1, "the control must plan one step");
        assert!(matches!(built.steps[0].action, InstallAction::AptSource { .. }));
        assert_eq!(
            built.steps[0].privilege,
            PrivilegeRequirement::Root,
            "adding a trust root needs root"
        );
    }

    #[test]
    fn a_pip_availability_needs_no_root() {
        let manifest = manifest_of(&["pyyaml"]);
        let availability = PackageAvailability::PipDistribution {
            id: PackageId::parse("pyyaml").expect("a valid package id"),
            break_system_packages: true,
        };
        let mut packages = PackageCatalog::new();
        packages.insert(dependency("pyyaml"), PackageMap::new(BTreeMap::new(), availability));

        let (built, _events) = plan(
            &manifest,
            PackageManager::Brew,
            &Selection::all(&manifest),
            &Requirements::none(),
            &ObservationMap::default(),
            Elevation::Unavailable,
            &packages,
        )
        .expect("a pip availability plans");
        assert_eq!(built.steps.len(), 1, "the control must plan one step");
        assert!(matches!(
            built.steps[0].action,
            InstallAction::Pip { break_system_packages: true, .. }
        ));
        assert_eq!(built.steps[0].privilege, PrivilegeRequirement::None);
    }

    #[test]
    fn an_nvm_availability_plans_an_nvm_install() {
        let manifest = manifest_of(&["node"]);
        let mut packages = PackageCatalog::new();
        packages.insert(
            dependency("node"),
            PackageMap::new(BTreeMap::new(), PackageAvailability::ViaNvm),
        );

        let (built, _events) = plan(
            &manifest,
            PackageManager::Apt,
            &Selection::all(&manifest),
            &Requirements::none(),
            &ObservationMap::default(),
            Elevation::Unavailable,
            &packages,
        )
        .expect("an nvm availability plans");
        assert_eq!(built.steps.len(), 1, "the control must plan one step");
        assert_eq!(built.steps[0].action, InstallAction::NvmInstall);
        assert_eq!(
            built.steps[0].privilege,
            PrivilegeRequirement::None,
            "nvm installs into $HOME, so this must plan even with no elevation"
        );
    }
```

Run: `cd ~/crates && cargo test --locked -p deps-core`
Expected: PASS.

- [ ] **Step 7: Verify the tests bind to the fix**

Remove only the `PackageAvailability::Clone` arm from `action_for` and
confirm the crate fails to compile with a non-exhaustive-match error, then
restore. Removing the arm rather than the variant is what isolates "the
mapping is missing" from "the type is missing".

```sh
cd ~/crates
cp deps-core/src/plan.rs /tmp/plan.rs.good
python3 - <<'PY'
import pathlib, re
p = pathlib.Path("deps-core/src/plan.rs")
b = p.read_text()
start = b.index("        PackageAvailability::Clone { source, into } => (")
end = b.index("        PackageAvailability::ViaNvm", start)
p.write_text(b[:start] + b[end:])
PY
cargo test --locked -p deps-core 2>&1 | grep -E 'non-exhaustive|error\[E0004\]' | head -2
cp /tmp/plan.rs.good deps-core/src/plan.rs && rm /tmp/plan.rs.good
cargo test --locked -p deps-core 2>&1 | grep -c 'test result: ok'
```

Expected: the sabotaged run names `error[E0004]` (non-exhaustive patterns);
the restored run reports every test binary ok. Record both.

- [ ] **Step 8: Rebuild and check the stamp**

```sh
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
```

Expected: `doctor=0` with no output. Any change under `crates/` moves the
stamp, and the live pre-push gate refuses a push whose installed binary is
stale.

- [ ] **Step 9: Commit**

```bash
cd ~
cat > /tmp/msg.txt <<'MSG'
Let the planner reach the four unreachable install actions

action_for built actions only from PackageAvailability's three variants,
which reach Package, Brew, Script and NotAutomatable. GitClone, NvmInstall
and Pip had zero references in the crate and AptSource had two, both
declaration. So plan could not emit the action four of the 22 dependencies
need: zsh-autosuggestions, node, pyyaml, and gh on apt.

The adapter step names zsh-autosuggestions convergence as its own acceptance
test, and the action it needs could not be planned, so the step was
unbuildable as scoped.

Each new test asserts the privilege as well as the action, because privilege
is what action_for computes and a review found a case where describe
reported it wrongly.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add crates/deps-core/src/action.rs crates/deps-core/src/plan.rs
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 2: `render` and `Rendered`

The adapter spec's pipeline diagram labelled `render(&report, verb)` as
"`deps-core`: pure to `Rendered`". Verified:
`grep 'fn render\|struct Rendered' crates/deps-core/src/` returns nothing.
Both exist only in `crates/config-manifest/src/stamp.rs:18`, a different
crate and a different domain, which the parent spec's section 4 cites as the
precedent to follow rather than the place to reuse.

Nothing constructs a `Verdict` either. `Verdict::DryRun` versus
`Verdict::Check` is the per-verb distinction the exit-code regression test
depends on, so the renderer is what makes that distinction observable.

**Files:**
- Create: `crates/deps-core/src/render.rs`
- Modify: `crates/deps-core/src/lib.rs` (declare the module, re-export, and
  add the file to the purity array at `lib.rs:58-65`)

**Interfaces:**
- Consumes: `Report { rows: Vec<ReportRow>, check: CheckStatus, install: InstallStatus }`
  from `reconcile.rs:26-33`, and `Verb` which this task introduces.
- Produces: `pub struct Rendered { pub stdout: String, pub stderr: String, pub exit_code: u8 }`,
  `pub enum Verb { Check, Install, DryRun }`, and
  `pub fn render(report: &Report, verb: Verb) -> Rendered`. The adapter's
  `main` drains `Rendered` and returns its `exit_code`.

- [ ] **Step 1: Write the failing test**

Create `crates/deps-core/src/render.rs` with only its test module, so the
red run fails on the missing implementation rather than on a missing file.

```rust
//! Rendering a report to two streams and one exit code.
//!
//! Pure. Follows `config-manifest`'s `stamp::Rendered`, which the parent
//! spec cites as precedent: returning output as a value rather than writing
//! it makes the whole reporting path testable without capturing streams.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CheckStatus, InstallStatus, Report, ReportRow, StepOutcome};

    fn a_ready_report() -> Report {
        Report {
            rows: vec![ReportRow {
                dependency: crate::DependencyName::parse("git").expect("a valid name"),
                outcome: StepOutcome::AlreadyPresent,
                after: crate::Observation::Present,
            }],
            check: CheckStatus::Ready,
            install: InstallStatus::AllSucceeded,
        }
    }

    /// A ready check reports success on stdout and exits 0.
    #[test]
    fn a_ready_check_renders_to_stdout_and_exits_zero() {
        let rendered = render(&a_ready_report(), Verb::Check);

        // Positive control: stdout must carry something, or the assertions
        // below would hold for a renderer that writes nothing at all.
        assert!(!rendered.stdout.is_empty(), "a check must say something");
        assert_eq!(rendered.exit_code, 0);
        assert!(rendered.stderr.is_empty(), "nothing failed, so stderr stays empty");
    }

    /// The verb changes the wording, which is what makes the per-verb exit
    /// codes legible: the same NotReady report is a failure for `check` and
    /// a preview for `--dry-run`.
    #[test]
    fn the_verb_changes_the_wording() {
        let report = a_ready_report();
        let checked = render(&report, Verb::Check);
        let previewed = render(&report, Verb::DryRun);

        assert!(!checked.stdout.is_empty(), "the control must produce output");
        assert_ne!(
            checked.stdout, previewed.stdout,
            "a check and a dry run must not read identically"
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd ~/crates && cargo test --locked -p deps-core render`
Expected: FAIL. The module is not declared in `lib.rs`, so the test does not
even build. Add `mod render;` to `lib.rs` first, then the failure becomes
`cannot find function 'render' in this scope`, which is the right red.

- [ ] **Step 3: Implement**

Prepend to `render.rs`, above the test module:

```rust
use crate::{CheckStatus, InstallStatus, Report, StepOutcome};

/// Which verb produced a report.
///
/// The wording and the exit code both depend on it: a `NotReady` report is
/// a failure for `check` and an accurate preview for `--dry-run`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    /// `config deps check`.
    Check,
    /// `config deps install`.
    Install,
    /// Either verb with `--dry-run`.
    DryRun,
}

/// Two output streams and an exit code, as a value.
///
/// Follows `config-manifest`'s `stamp::Rendered`. Returning the streams
/// rather than writing them keeps every reporting decision testable without
/// capturing a process's output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// What belongs on stdout.
    pub stdout: String,
    /// What belongs on stderr. Empty when nothing failed.
    pub stderr: String,
    /// The code to hand the process exit.
    pub exit_code: u8,
}

/// Render one report for one verb.
///
/// Pure. The exit code comes from `exit_status`, so this function has no
/// opinion about which number means what: that mapping has exactly one
/// owner, per spec 5.4.
pub fn render(report: &Report, verb: Verb) -> Rendered {
    let mut stdout = String::new();
    let mut stderr = String::new();

    for row in &report.rows {
        let line = match &row.outcome {
            StepOutcome::AlreadyPresent => format!("  present   {}\n", row.dependency.as_str()),
            StepOutcome::Installed => format!("  installed {}\n", row.dependency.as_str()),
            StepOutcome::NotSelected => format!("  missing   {}\n", row.dependency.as_str()),
            StepOutcome::Blocked { on } => {
                format!("  waiting   {} (needs {})\n", row.dependency.as_str(), on.as_str())
            }
            StepOutcome::NotAutomatable { reason } => {
                format!("  manual    {} ({reason:?})\n", row.dependency.as_str())
            }
            StepOutcome::InstallFailed { .. } | StepOutcome::InstalledButCheckStillFails { .. } => {
                stderr.push_str(&format!("  FAILED    {}\n", row.dependency.as_str()));
                String::new()
            }
        };
        stdout.push_str(&line);
    }

    let heading = match verb {
        Verb::Check => "checked dependencies",
        Verb::Install => "installed dependencies",
        Verb::DryRun => "would install",
    };
    let summary = format!("deps {heading}: {} entries\n", report.rows.len());

    let verdict = match verb {
        Verb::Check => crate::Verdict::Check(report.check),
        Verb::DryRun => crate::Verdict::DryRun(report.check),
        Verb::Install => crate::Verdict::Install(report.install, report.check),
    };

    Rendered {
        stdout: format!("{summary}{stdout}"),
        stderr,
        exit_code: crate::exit_status(Ok(verdict)).code(),
    }
}
```

Note the `Verdict::Install` arm takes two arguments. That is Task 3's
signature; until Task 3 lands, write it as `Verdict::Install(report.install)`
and change it in Task 3's commit. The plan sequences it this way because
Task 2 unblocks the adapter and Task 3 is a separate correctness fix.

- [ ] **Step 4: Declare the module and extend the purity array**

In `lib.rs`, add `mod render;` beside the other module declarations, and add
the re-export:

```rust
pub use render::{Rendered, Verb, render};
```

Then add the file to the purity test's array, in this same commit:

```rust
            ("render.rs", include_str!("render.rs")),
```

**The purity test scans the file holding its own needles**, so no forbidden
path may appear as a literal in `render.rs`, doc comments included.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd ~/crates && cargo test --locked -p deps-core`
Expected: PASS, including the `purity` test with `render.rs` in its array.

- [ ] **Step 6: Verify the purity array actually covers the new file**

The array is the one part of the purity test that fails silently when
incomplete: a file missing from it is simply not scanned.

```sh
cd ~/crates
printf 'const _: &str = "std::proc" ; // deliberate\n' >> deps-core/src/render.rs
cargo test --locked -p deps-core purity 2>&1 | grep -cE 'FAILED|panicked'
git checkout deps-core/src/render.rs 2>/dev/null || true
```

Expected: a nonzero count, proving `render.rs` is scanned. Note the needle
above is split so this plan document does not itself carry the literal.
Restore the file afterwards; if `git checkout` does not apply in this bare
repo, delete the appended line by hand.

- [ ] **Step 7: Rebuild and check the stamp**

```sh
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
```

Expected: `doctor=0`, no output.

- [ ] **Step 8: Commit**

```bash
cd ~
cat > /tmp/msg.txt <<'MSG'
Add render and Rendered to deps-core

The adapter spec's pipeline diagram labelled render as "deps-core: pure to
Rendered" while neither existed in the crate. Both lived only in
config-manifest's stamp module, which the parent spec cites as the precedent
to follow rather than the place to reuse.

Returning the two streams and the exit code as a value keeps every reporting
decision testable without capturing a process's output, and it means main
drains a value rather than deciding anything.

The exit code comes from exit_status, so this module has no opinion about
which number means what. That mapping keeps exactly one owner.

render.rs is added to the purity test's file array in this commit, and the
array's coverage is verified rather than assumed: a file missing from it is
silently unscanned.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add crates/deps-core/src/render.rs crates/deps-core/src/lib.rs
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 3: `deps install` must not exit 0 on a not-ready machine

The spec's item 3, and the defect the parent spec's own bootstrap gate
observed on real hardware: a run ended "no unresolved failures (16 of 18 were
already missing)" and exited **0** with three dependencies absent.

Verified as still present:

- `summarize_install` (`outcome.rs:143-152`) returns `AllSucceeded` unless an
  outcome is `InstallFailed` or `InstalledButCheckStillFails`. A
  `NotAutomatable` outcome, which is what a manual-only dependency produces,
  yields `AllSucceeded`.
- `Verdict::Install(InstallStatus)` (`outcome.rs:164`) carries no
  `CheckStatus`, so no cell of the exit table can consult readiness.
- `exit_status` maps `Install(AllSucceeded)` to **0**.

The doc comment at `outcome.rs:140-142` says the not-ready signal "belongs to
`summarize_check`, which the driver also calls". The driver does call it. The
result is then thrown away.

Consumer harm, concrete: `config-init:142` propagates any nonzero from
`config-install`, so `config init` on a bare machine reports a successful
bootstrap while dependencies are missing.

**Files:**
- Modify: `crates/deps-core/src/outcome.rs:160-167` (`Verdict`), `:207-219`
  (`exit_status`), and the test at `:309-315`
- Modify: `crates/deps-core/src/render.rs` (the `Verdict::Install` arm Task 2
  left one-argument)

**Interfaces:**
- Consumes: `Verdict`, `exit_status`, `Rendered` from Tasks 2.
- Produces: `Verdict::Install(InstallStatus, CheckStatus)`, and a fourth exit
  code: **4** for "installs succeeded, machine still not ready". The adapter
  and `config-init` both read it.

- [ ] **Step 1: Write the failing test**

Add to `outcome.rs`'s test module.

```rust
    /// An install that attempted nothing and left the machine incomplete
    /// must not exit 0.
    ///
    /// The reported incident: a bootstrap ended "no unresolved failures (16
    /// of 18 were already missing)" and exited 0 with three dependencies
    /// absent. summarize_install said AllSucceeded, because NotAutomatable
    /// is not an attempt failure, and Verdict::Install carried no readiness,
    /// so the exit table had no cell that could disagree.
    #[test]
    fn an_install_that_leaves_the_machine_not_ready_does_not_exit_zero() {
        let manual_only = [StepOutcome::NotAutomatable {
            reason: NoInstallReason::UpstreamPublishesNoStableUrl,
        }];

        // Positive controls. Nothing was attempted, so nothing failed, and
        // the machine is not ready. Both must hold or the assertion below
        // is about the wrong inputs.
        assert_eq!(summarize_install(&manual_only), InstallStatus::AllSucceeded);
        assert_eq!(summarize_check(&manual_only), CheckStatus::NotReady);

        let verdict = Verdict::Install(
            summarize_install(&manual_only),
            summarize_check(&manual_only),
        );
        assert_ne!(
            exit_status(Ok(verdict)).code(),
            0,
            "an incomplete machine must not report success"
        );
    }

    /// The success path still exits 0, so the fix does not make every
    /// install look like a failure.
    #[test]
    fn an_install_that_leaves_the_machine_ready_exits_zero() {
        let all_good = [StepOutcome::Installed, StepOutcome::AlreadyPresent];
        assert_eq!(summarize_install(&all_good), InstallStatus::AllSucceeded);
        assert_eq!(summarize_check(&all_good), CheckStatus::Ready);
        assert_eq!(
            exit_status(Ok(Verdict::Install(InstallStatus::AllSucceeded, CheckStatus::Ready)))
                .code(),
            0
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd ~/crates && cargo test --locked -p deps-core an_install_that_leaves`
Expected: FAIL to compile, with
`this enum variant takes 1 argument but 2 arguments were supplied`. That is
the right red: the variant genuinely cannot carry readiness yet.

- [ ] **Step 3: Widen the variant and the table**

In `outcome.rs`, change the variant:

```rust
    /// A `deps install` run.
    ///
    /// Carries both summaries, because they answer different questions and
    /// a run can succeed at every attempt while leaving the machine
    /// incomplete. Reporting only the first is how an install exited 0 with
    /// three dependencies missing.
    Install(InstallStatus, CheckStatus),
```

Then in `exit_status`, replace the single `Install` arm with three, keeping
per-verb disjointness so a consumer learning "nonzero and not 2 means the
environment is not ready" stays correct:

```rust
        Verdict::Install(InstallStatus::AttemptFailed, _) => ExitStatus(3),
        // Every attempt succeeded and the machine is still incomplete,
        // which is the manual-only case. Distinct from 3, because nothing
        // failed, and distinct from 0, because the caller cannot proceed.
        Verdict::Install(InstallStatus::AllSucceeded, CheckStatus::NotReady) => ExitStatus(4),
        Verdict::Install(InstallStatus::AllSucceeded, CheckStatus::Ready) => ExitStatus(0),
```

Update the `exit_status` doc comment's table to name 4.

- [ ] **Step 4: Update the test that asserts the old behavior**

`outcome.rs:309-315` asserts `summarize_install` returns `AllSucceeded` for
`NotAutomatable`. **That assertion is still correct and stays**: nothing was
attempted, so no attempt failed. What changes is only the verdict built from
it. Read the test, confirm it does not construct a `Verdict`, and leave it
alone if so. If it does, give it the second argument.

- [ ] **Step 5: Fix the `render.rs` arm Task 2 deferred**

```rust
        Verb::Install => crate::Verdict::Install(report.install, report.check),
```

- [ ] **Step 6: Run everything**

Run: `cd ~/crates && cargo test --locked -p deps-core`
Expected: PASS.

Run: `cd ~/crates && cargo clippy --locked --all-targets -- -D warnings`
Expected: 0 errors. A widened variant makes every match on it a candidate
for a non-exhaustive error, so this is the step that catches a missed site.

- [ ] **Step 7: Verify the test binds**

```sh
cd ~/crates
cp deps-core/src/outcome.rs /tmp/outcome.rs.good
python3 - <<'PY'
import pathlib
p = pathlib.Path("deps-core/src/outcome.rs")
b = p.read_text()
old = "        Verdict::Install(InstallStatus::AllSucceeded, CheckStatus::NotReady) => ExitStatus(4),"
new = "        Verdict::Install(InstallStatus::AllSucceeded, CheckStatus::NotReady) => ExitStatus(0),"
assert b.count(old) == 1
p.write_text(b.replace(old, new))
PY
cargo test --locked -p deps-core an_install_that_leaves 2>&1 | grep -cE 'FAILED'
cp /tmp/outcome.rs.good deps-core/src/outcome.rs && rm /tmp/outcome.rs.good
cargo test --locked -p deps-core an_install_that_leaves 2>&1 | tail -1
```

Expected: the sabotaged run reports a failure; the restored run passes.
Sabotaging the code to 0 rather than deleting the arm is what proves the
test is about the exit code and not about compilation.

- [ ] **Step 8: Rebuild and commit**

```bash
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
cat > /tmp/msg.txt <<'MSG'
Stop deps install exiting 0 on a not-ready machine

summarize_install returns AllSucceeded for NotAutomatable, correctly:
nothing was attempted, so no attempt failed. But Verdict::Install carried
only that summary, so the exit table had no cell that could consult
readiness, and exit_status mapped it to 0.

The observed incident: a bootstrap ended "no unresolved failures (16 of 18
were already missing)" and exited 0 with three dependencies absent.
config-init:142 propagates any nonzero from config-install, so config init
reported a successful bootstrap on an incomplete machine.

Verdict::Install now carries both summaries and exit 4 means "every attempt
succeeded, the machine is still incomplete". Distinct from 3, because
nothing failed, and from 0, because the caller cannot proceed. Per-verb
disjointness is preserved, so "nonzero and not 2 means the environment is
not ready" stays true.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add crates/deps-core/src/outcome.rs crates/deps-core/src/render.rs
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 4: `describe` takes the step, not the action

`Installer::describe(&self, action: &InstallAction)` receives only the
action, and `describe` over a plan (`driver.rs:187-193`) discards
`step.privilege` at the call site. But privilege is decided by `action_for`
from `(availability, manager, elevation)`, and an
`InstallAction::Package { id }` carries none of the three, so an installer
**cannot** re-derive it. The crate's own reference implementation hardcodes
`privilege: PrivilegeRequirement::None` (`driver.rs:448`), which is the
divergence shipping in miniature.

This matters because parent spec 3.5 and 6.1 load a correctness property onto
`ActionDescription::privilege`: a dry run must disclose privileged steps
before the first password prompt. A dry run that renders every step as
unprivileged defeats it.

**Files:**
- Modify: `crates/deps-core/src/driver.rs:91-97` (the trait), `:187-193`
  (`describe`), `:445-452` (the test installer)

**Interfaces:**
- Consumes: nothing new.
- Produces: `fn describe(&self, step: &Step) -> ActionDescription`. Every
  `Installer` implementation changes signature, which is exactly two sites
  today: the reference installer in `driver.rs`'s tests, and the adapter's
  future one.

- [ ] **Step 1: Write the failing test**

Add to `driver.rs`'s test module. The existing test installer must first
grow a way to report what it saw, so give it a privilege field it echoes.

```rust
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
        let installer = RecordingInstaller::default();

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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd ~/crates && cargo test --locked -p deps-core describe_reports`
Expected: FAIL with `assertion left == right failed`, showing
`left: None, right: Root`. That is the right red: the code compiles and gives
the wrong answer, which is the defect. A compile error naming
`RecordingInstaller` means the test installer has a different name; read
`driver.rs`'s test module and use the real one.

- [ ] **Step 3: Change the trait**

In `driver.rs`, change the method and say why in the doc comment:

```rust
    /// Describe `step` without performing it.
    ///
    /// Takes the step rather than the action, because privilege is decided
    /// by the planner from the availability, the manager and the elevation,
    /// and an `InstallAction` carries none of the three. An installer handed
    /// only the action has to guess, and the guess is what a dry run shows a
    /// reader before the first password prompt.
    fn describe(&self, step: &Step) -> ActionDescription;
```

- [ ] **Step 4: Change the call site and every implementation**

`driver.rs:187-193`:

```rust
pub fn describe(installer: &dyn Installer, built: &Plan) -> Vec<ActionDescription> {
    built.steps.iter().map(|step| installer.describe(step)).collect()
}
```

Then the reference installer in the test module: replace the hardcoded
`privilege: PrivilegeRequirement::None` with `privilege: step.privilege`.

- [ ] **Step 5: Run everything**

Run: `cd ~/crates && cargo test --locked -p deps-core`
Expected: PASS.

Run: `cd ~/crates && cargo clippy --locked --all-targets -- -D warnings`
Expected: 0 errors.

- [ ] **Step 6: Verify the test binds**

```sh
cd ~/crates
cp deps-core/src/driver.rs /tmp/driver.rs.good
sed -i '' 's/privilege: step\.privilege,/privilege: PrivilegeRequirement::None,/' deps-core/src/driver.rs
cargo test --locked -p deps-core describe_reports 2>&1 | grep -cE 'FAILED'
cp /tmp/driver.rs.good deps-core/src/driver.rs && rm /tmp/driver.rs.good
cargo test --locked -p deps-core describe_reports 2>&1 | tail -1
```

Expected: the sabotaged run fails, the restored run passes. Restoring the
hardcoded value rather than reverting the signature is what proves the test
catches the wrong answer rather than the wrong shape.

- [ ] **Step 7: Rebuild and commit**

```bash
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
cat > /tmp/msg.txt <<'MSG'
Hand describe the step, so a dry run can disclose privilege

Installer::describe received only the action, and describe over a plan
discarded step.privilege at the call site. Privilege is decided by action_for
from the availability, the manager and the elevation, and an InstallAction
carries none of the three, so an installer could not re-derive it. The
crate's own reference implementation hardcoded None, which is the divergence
shipping in miniature.

Parent 3.5 and 6.1 put a correctness property on this field: a dry run
discloses privileged steps before the first password prompt. A dry run that
renders every step as unprivileged defeats it.

Passing the step makes describe and the planner read one value rather than
two methods agreeing by convention.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add crates/deps-core/src/driver.rs
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 5: `Requirements::validated`, and split the prerequisite event

Two gaps at one boundary, from the spec's item 7.

**A bad requirement edge is absorbed silently.**
`PlanError::UnknownDependency` is constructed only from `Selection`
(`plan.rs:289`, `:442`); nothing walks `Requirements`. So a typo in the
hardcoded table, `(zsh-autosuggestions, [oh-my-zhs])`, is a well-typed value
that plans successfully, emits `PrerequisiteNotSelected { on: "oh-my-zhs" }`,
orders nothing, and exits 0. **The typo is indistinguishable from a correct
macOS run.**

**`PrerequisiteNotSelected` conflates two facts.** `plan.rs:397` is a
disjunction: `manifest.get(prerequisite).is_none() || !selection.contains(prerequisite)`.
The first disjunct is the legitimate macOS case. The second is "this platform
has it, but the run narrowed past it", which is not a platform difference: on
Linux, `--only zsh-autosuggestions` takes that branch and plans a real clone
into a directory that does not exist.

**Files:**
- Modify: `crates/deps-core/src/plan.rs:123-140` (`Requirements`), `:391-406`
  (`first_unsatisfied_prerequisite`), and the `Event` enum

**Interfaces:**
- Consumes: nothing new.
- Produces: `Requirements::validated(pairs, known) -> Result<Self, Vec<RequirementEdgeError>>`,
  `Event::PrerequisiteNotInManifest { dependency, on }`, and
  `Event::PrerequisiteDeselected { dependency, on }`. The adapter's
  `catalog.rs` calls `validated` and a test asserts the production table
  passes against the union of every shipped conf file.

- [ ] **Step 1: Write the failing tests**

```rust
    /// A requirement edge naming a dependency no manifest holds must be
    /// rejected at construction.
    ///
    /// Nothing walked Requirements, so a typo planned successfully, emitted
    /// PrerequisiteNotSelected for a name that exists nowhere, ordered
    /// nothing, and exited 0. That is indistinguishable from a correct macOS
    /// run, where the prerequisite is legitimately absent.
    #[test]
    fn a_requirement_edge_naming_an_unknown_dependency_is_rejected() {
        let known: BTreeSet<DependencyName> =
            [dependency("zsh-autosuggestions"), dependency("oh-my-zsh")]
                .into_iter()
                .collect();

        // Positive control: the correct table must validate, or the
        // rejection below proves only that the constructor rejects
        // everything.
        Requirements::validated(
            vec![(dependency("zsh-autosuggestions"), vec![dependency("oh-my-zsh")])],
            &known,
        )
        .expect("the correct table validates");

        let errors = Requirements::validated(
            vec![(dependency("zsh-autosuggestions"), vec![dependency("oh-my-zhs")])],
            &known,
        )
        .expect_err("a typo must be rejected");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].unknown, dependency("oh-my-zhs"));
    }

    /// A deselected prerequisite is not the same fact as an absent one.
    ///
    /// plan.rs:397 was a disjunction, so "this platform lacks it" and "the
    /// run narrowed past it" produced one event. The first is the macOS case
    /// the design relies on; the second plans a real install whose
    /// prerequisite nobody is going to satisfy.
    #[test]
    fn a_deselected_prerequisite_is_distinguished_from_an_absent_one() {
        let manifest = manifest_of(&["oh-my-zsh", "zsh-autosuggestions"]);
        let requirements = Requirements::from_pairs(vec![(
            dependency("zsh-autosuggestions"),
            vec![dependency("oh-my-zsh")],
        )]);
        let narrowed = Selection::named(vec![dependency("zsh-autosuggestions")]);

        let (_built, events) = plan(
            &manifest,
            PackageManager::Pacman,
            &narrowed,
            &requirements,
            &ObservationMap::default(),
            Elevation::AlreadyRoot,
            &packages_named(&["oh-my-zsh", "zsh-autosuggestions"]),
        )
        .expect("a narrowed selection plans");

        assert!(!events.is_empty(), "the control must emit events");
        assert!(
            events.contains(&Event::PrerequisiteDeselected {
                dependency: dependency("zsh-autosuggestions"),
                on: dependency("oh-my-zsh"),
            }),
            "a prerequisite the manifest has but the selection excludes is deselected, not absent"
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd ~/crates && cargo test --locked -p deps-core prerequisite`
Expected: FAIL to compile on `Requirements::validated`,
`RequirementEdgeError` and `Event::PrerequisiteDeselected`, none of which
exist.

- [ ] **Step 3: Add the validating constructor**

```rust
/// An edge naming a dependency no shipped manifest holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementEdgeError {
    /// The dependent the edge belongs to.
    pub dependent: DependencyName,
    /// The name that matches no manifest entry.
    pub unknown: DependencyName,
}

impl Requirements {
    /// Build a graph, rejecting any edge that names an unknown dependency.
    ///
    /// `known` is the union of every conf file this repo ships, not one
    /// platform's manifest, because an edge is correct or not independently
    /// of which machine is running. Checking against one platform's manifest
    /// would reject the macOS-absent prerequisite this design depends on.
    ///
    /// # Errors
    ///
    /// Returns every offending edge rather than the first, so one run names
    /// every typo.
    pub fn validated(
        pairs: Vec<(DependencyName, Vec<DependencyName>)>,
        known: &BTreeSet<DependencyName>,
    ) -> Result<Self, Vec<RequirementEdgeError>> {
        let mut errors = Vec::new();
        for (dependent, prerequisites) in &pairs {
            for prerequisite in prerequisites {
                if !known.contains(prerequisite) {
                    errors.push(RequirementEdgeError {
                        dependent: dependent.clone(),
                        unknown: prerequisite.clone(),
                    });
                }
            }
        }
        if errors.is_empty() { Ok(Requirements::from_pairs(pairs)) } else { Err(errors) }
    }
}
```

- [ ] **Step 4: Split the event**

Replace `Event::PrerequisiteNotSelected` with two variants, and split the
disjunction at `plan.rs:397` into two arms:

```rust
    /// The prerequisite is not in this platform's manifest. A legitimate
    /// platform difference: oh-my-zsh is in `deps-linux.conf:11` and absent
    /// on macOS, where zsh-autosuggestions installs through brew.
    PrerequisiteNotInManifest {
        /// The dependent whose prerequisite is absent.
        dependency: DependencyName,
        /// The absent prerequisite.
        on: DependencyName,
    },
    /// The prerequisite exists on this platform and the selection excludes
    /// it. Not a platform difference: the run narrowed past a requirement it
    /// still has.
    PrerequisiteDeselected {
        /// The dependent whose prerequisite was excluded.
        dependency: DependencyName,
        /// The excluded prerequisite.
        on: DependencyName,
    },
```

In `first_unsatisfied_prerequisite`, replace the single `if` with:

```rust
        if manifest.get(prerequisite).is_none() {
            events.push(Event::PrerequisiteNotInManifest {
                dependency: name.clone(),
                on: prerequisite.clone(),
            });
            continue;
        }
        if !selection.contains(prerequisite) {
            // Blocking, unlike the absent case: the machine has this
            // prerequisite in its manifest and the run chose not to satisfy
            // it, so planning the dependent's install would run it against a
            // world nobody is going to prepare.
            events.push(Event::PrerequisiteDeselected {
                dependency: name.clone(),
                on: prerequisite.clone(),
            });
            return Some(prerequisite.clone());
        }
```

- [ ] **Step 5: Run everything and fix the fallout**

Run: `cd ~/crates && cargo test --locked -p deps-core`
Expected: several existing tests fail on the removed
`Event::PrerequisiteNotSelected`. Update each to the correct new variant,
choosing by which fact the test is about. Do not collapse them back to one.

Run: `cd ~/crates && cargo clippy --locked --all-targets -- -D warnings`
Expected: 0 errors.

- [ ] **Step 6: Verify both tests bind**

```sh
cd ~/crates
cp deps-core/src/plan.rs /tmp/plan.rs.good
# Make validated accept everything.
sed -i '' 's/if errors.is_empty() { Ok(Requirements::from_pairs(pairs)) } else { Err(errors) }/Ok(Requirements::from_pairs(pairs))/' deps-core/src/plan.rs
cargo test --locked -p deps-core a_requirement_edge 2>&1 | grep -cE 'FAILED'
cp /tmp/plan.rs.good deps-core/src/plan.rs && rm /tmp/plan.rs.good
cargo test --locked -p deps-core prerequisite 2>&1 | tail -1
```

Expected: the sabotaged run fails; the restored run passes.

- [ ] **Step 7: Rebuild and commit**

```bash
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
cat > /tmp/msg.txt <<'MSG'
Reject unknown requirement edges and split the prerequisite event

Two gaps at one boundary.

Nothing walked Requirements, so a typo in the hardcoded table was a
well-typed value that planned successfully, emitted an event for a name
existing nowhere, ordered nothing, and exited 0. That is indistinguishable
from a correct macOS run, where the prerequisite is legitimately absent.
validated takes the union of every shipped conf file, because an edge is
correct or not independently of which machine runs it.

PrerequisiteNotSelected was a disjunction of two facts with different truth
conditions. "This platform lacks the prerequisite" is the macOS case the
design relies on and is not blocking. "This platform has it and the run
narrowed past it" is a different claim, and on Linux --only
zsh-autosuggestions took that branch and planned a clone into a directory
nobody was going to create. Splitting the event forces the two-case decision
at every match site instead of letting a || decide it.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add crates/deps-core/src/plan.rs
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 6: Delete `PathRoot::OhMyZshCustom`

The spec's item 6. `check.rs:365` registers the prefix
`${ZSH_CUSTOM:-$HOME/.oh-my-zsh/custom}/`, and no conf entry contains
`ZSH_CUSTOM` at all, so the variant is **unconstructible from the grammar**,
exactly like the two variants deleted in `22be72be` and `628a7f83`.

The sharper finding is a latent divergence in the shell, which the port would
make reachable. The check and the install use different roots:

```
check   (deps.conf:26)          $HOME/.oh-my-zsh/custom/plugins/...
install (check-deps.sh:339)     ${ZSH_CUSTOM:-$HOME/.oh-my-zsh/custom}/plugins/...
```

Verified by execution with `ZSH_CUSTOM=/opt/omz-custom`: the install target
is `/opt/omz-custom/plugins/zsh-autosuggestions` and the check subject is
`/Users/austin/.oh-my-zsh/custom/plugins/zsh-autosuggestions`. They agree
only when the variable is unset or set to its default, which is why the
defect is latent on these machines.

Ported as-is it becomes a non-converging fixpoint: the clone succeeds, the
re-gather still reports the dependency absent, and `Attempted`
(`driver.rs:30`) has already retired the step, so the run exits nonzero
having installed the thing correctly. The fixpoint does not rescue this,
because `Attempted` records "considered and resolved" rather than
"converged".

**Files:**
- Modify: `crates/deps-core/src/check.rs` (the `PathRoot` variant and its
  `roots` table entry at `:365`)

**Interfaces:**
- Consumes: Task 1's `PackageAvailability::Clone { into: CheckPath }`.
- Produces: `PathRoot` with three variants. The adapter's catalog builds
  `Clone { into }` with a `Home`-rooted `CheckPath` matching `deps.conf:26`
  byte for byte, so the install target and the check subject are the same
  value and convergence is structural.

- [ ] **Step 1: Write the failing test**

```rust
    /// The clone target and the check subject must be the same value.
    ///
    /// The shell used different roots: the check reads
    /// $HOME/.oh-my-zsh/custom and the install writes ${ZSH_CUSTOM:-...}.
    /// They agree only when the variable is unset, so with ZSH_CUSTOM set
    /// the clone succeeds, the re-gather still reports absent, and Attempted
    /// has retired the step. Making the two one value is what makes
    /// convergence structural rather than hoped for.
    #[test]
    fn the_clone_target_equals_the_parsed_check_subject() {
        let check = parse_check_expression(
            "test -f \"$HOME/.oh-my-zsh/custom/plugins/zsh-autosuggestions/zsh-autosuggestions.zsh\"",
            ConfKind::PlatformSelected,
        )
        .expect("the real deps.conf shape parses");

        let subject = match &check {
            Check::FileExists(path) => path.clone(),
            other => panic!("expected a file test, got {other:?}"),
        };

        // Positive control: the parsed subject must be Home-rooted, or the
        // comparison below is against a root the manifest never produces.
        assert_eq!(subject.root(), PathRoot::Home);

        let clone_target = CheckPath::new(
            PathRoot::Home,
            CheckRelPath::parse(
                ".oh-my-zsh/custom/plugins/zsh-autosuggestions/zsh-autosuggestions.zsh",
            )
            .expect("a valid relative path"),
        );
        assert_eq!(subject, clone_target);
    }
```

Read `check.rs` for the real `Check` variant name and the accessor before
running: this plan assumes `Check::FileExists(CheckPath)` and
`CheckPath::root()`. If either differs, use the real one; the assertion is
about equality, not about the spelling.

- [ ] **Step 2: Run the test to verify it fails or passes for the right reason**

Run: `cd ~/crates && cargo test --locked -p deps-core the_clone_target`
Expected: PASS, if `CheckPath` and its accessors already have this shape.
**A pass here is not a vacuous test**: it pins the property that Task 1's
`Clone` variant must satisfy, and it fails the moment someone gives
`GitClone` a different root. Record it as a characterization test rather
than a red-green cycle, and say so in the report.

- [ ] **Step 3: Delete the variant and its table entry**

In `check.rs`, remove `OhMyZshCustom` from `PathRoot` and remove the
`("${ZSH_CUSTOM:-$HOME/.oh-my-zsh/custom}/", PathRoot::OhMyZshCustom)` line
from the `roots` table. Add a comment where the entry was:

```rust
        // No ZSH_CUSTOM entry. No conf file contains that variable, so the
        // prefix matched nothing, and supporting it would let a check and an
        // install disagree about which directory they mean: the shell's
        // install wrote ${ZSH_CUSTOM:-...} while its check read $HOME/...,
        // which diverge whenever the variable is set.
```

- [ ] **Step 4: Run everything**

Run: `cd ~/crates && cargo test --locked -p deps-core`
Expected: PASS. `PathRoot` is `Copy` and payload-free, so removing a variant
breaks only exhaustive matches, which the compiler names.

Run: `cd ~/crates && cargo clippy --locked --all-targets -- -D warnings`
Expected: 0 errors.

- [ ] **Step 5: Verify the grammar still rejects the prefix**

```sh
cd ~/crates && cargo test --locked -p deps-core grammar_stays_closed 2>&1 | tail -2
```

Expected: PASS. `every_real_check_shape_parses_and_the_grammar_stays_closed`
asserts both directions, so it is what proves the deletion did not widen the
grammar.

- [ ] **Step 6: Rebuild and commit**

```bash
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
cat > /tmp/msg.txt <<'MSG'
Delete PathRoot::OhMyZshCustom, which no manifest can construct

check.rs registered the prefix ${ZSH_CUSTOM:-$HOME/.oh-my-zsh/custom}/ and
no conf file contains ZSH_CUSTOM, so the variant was unconstructible from
the grammar, exactly like the two deleted in 22be72be and 628a7f83.

The sharper reason is a latent divergence the port would have made
reachable. The check reads $HOME/.oh-my-zsh/custom and the install writes
${ZSH_CUSTOM:-...}; verified by execution that with the variable set they
name different directories. Ported as-is the clone succeeds, the re-gather
still reports absent, and Attempted has already retired the step, so the run
exits nonzero having installed correctly. The fixpoint does not rescue it,
because Attempted records "considered and resolved" rather than "converged".

GitClone now takes a Home-rooted CheckPath matching deps.conf byte for byte,
so the install target and the check subject are one value and convergence is
structural.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add crates/deps-core/src/check.rs
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Task 7: `StepOutcome::Declined`, and the gather contract written down

The spec's items 4, 5 and 7's remainder. Three things a reader of
`deps-core` cannot currently learn, all of which the adapter would otherwise
invent.

**Approval has no representation.** Parent 7.1 lists `config-cli` as owning
"Adapters, drivers, **Approval**, one exit code" and 6.3 says approval is
per-step. `check-deps.sh:572-581` prompts per dependency and reads stdin, and
`deps-check.yml:54` and `:66` both pin `--fix --yes`. There is no
`StepOutcome` for "the user declined": `NotAutomatable` is false (an
automated install exists), `Blocked` is false (nothing unblocks it), and
`InstallFailed` is false (nothing failed).

**The gather construction rules are unwritten.** Grepping all five specs for
`Unresolvable` returns almost nothing, while parent 5.2 established that the
variant exists to fix a live silent-false bug.

**`Installer::perform`'s return type is wider than any correct
implementation uses.** `AlreadyPresent` and `InstalledButCheckStillFails` are
`reconcile`'s to produce (`reconcile.rs:74-88`), so an installer returning
one bypasses the post-loop re-check.

**Files:**
- Modify: `crates/deps-core/src/outcome.rs` (`StepOutcome`, both summaries)
- Modify: `crates/deps-core/src/check.rs` (doc comment on `Observations`)
- Modify: `crates/deps-core/src/driver.rs` (doc comment on `perform`)

**Interfaces:**
- Consumes: Task 3's two-argument `Verdict::Install`.
- Produces: `StepOutcome::Declined`, counted as not-ready by
  `summarize_check` and as no-failure by `summarize_install`. The adapter's
  approval edge returns it.

- [ ] **Step 1: Write the failing test**

```rust
    /// A declined step leaves the machine not ready, and is not a failure.
    ///
    /// There was no outcome for "the user said no". NotAutomatable is false,
    /// because an automated install exists. Blocked is false, because
    /// nothing unblocks it. InstallFailed is false, because nothing failed.
    /// Reporting a decline as any of the three is a false statement, which
    /// is the argument parent 6.2 used to reject DescribeOnly.
    #[test]
    fn a_declined_step_is_not_ready_and_not_a_failure() {
        let declined = [StepOutcome::Declined];

        // Positive control: the same summaries must call an installed step
        // ready and unfailed, or the assertions below hold for any input.
        assert_eq!(summarize_check(&[StepOutcome::Installed]), CheckStatus::Ready);
        assert_eq!(
            summarize_install(&[StepOutcome::Installed]),
            InstallStatus::AllSucceeded
        );

        assert_eq!(
            summarize_check(&declined),
            CheckStatus::NotReady,
            "the machine is missing a dependency the user chose not to install"
        );
        assert_eq!(
            summarize_install(&declined),
            InstallStatus::AllSucceeded,
            "nothing was attempted, so no attempt failed"
        );

        // And the two together must not exit 0, which is Task 3's table.
        assert_ne!(
            exit_status(Ok(Verdict::Install(
                summarize_install(&declined),
                summarize_check(&declined),
            )))
            .code(),
            0
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd ~/crates && cargo test --locked -p deps-core a_declined_step`
Expected: FAIL to compile with
`no variant or associated item named 'Declined' found for enum 'StepOutcome'`.

- [ ] **Step 3: Add the variant**

In `outcome.rs`:

```rust
    /// The step had an automated install and the user declined it.
    ///
    /// Distinct from every neighbour: `NotAutomatable` claims no automated
    /// install exists, `Blocked` claims a later wave can unblock it, and
    /// `InstallFailed` claims an attempt failed. All three are false here,
    /// and reporting a decline as any of them is the kind of false statement
    /// parent 6.2 rejected `DescribeOnly` for.
    Declined,
```

`summarize_check`'s fold already treats anything but `AlreadyPresent` and
`Installed` as not-ready, so it needs no change. `summarize_install`'s fold
already treats anything but the two failure variants as no-failure, so it
needs no change either. **Verify both rather than assuming**: run the test
and read the two functions.

- [ ] **Step 4: Write down the gather contract**

In `check.rs`, extend the `Observations` trait's doc comment. This is
documentation, not code, and it is the deliverable the adapter reads:

```rust
/// A gathered world.
///
/// The core reads observations as a value and never probes, per parent 3.3.
/// Four rules the gather at the edge must satisfy, none of which this trait
/// can enforce:
///
/// 1. **Enumerate every leaf.** `ObservationMap::observe` returns `Absent`
///    for a key it does not hold, so a gather that records only top-level
///    checks silently reports every `AnyOf` dependency missing. Recurse into
///    `AnyOf`'s `first` and `rest`.
/// 2. **A failed probe is `Unresolvable`, not `Absent`.** A root that cannot
///    be resolved, or a check whose interpreter is missing, is a different
///    fact from "the dependency is not installed" and has a different
///    remedy. The shell collapsed both and this crate exists partly to stop
///    that.
/// 3. **`Unresolvable` does not block.** `plan` records
///    `Event::CheckUnanswerable` and plans the install anyway, because an
///    unresolvable root is usually a missing manager rather than a satisfied
///    dependency.
/// 4. **Resolve roots at the instant of use, not once at gather time.**
///    Installing `oh-my-zsh` creates the directory a later check reads, so a
///    root resolved before the first wave is stale by the second.
```

In `driver.rs`, extend `perform`'s doc comment:

```rust
    /// Perform `action` and report what happened.
    ///
    /// Returns only `Installed`, `InstallFailed`, `NotAutomatable` or
    /// `Declined`. `AlreadyPresent` and `InstalledButCheckStillFails` are
    /// `reconcile`'s to produce from the post-loop world, so an installer
    /// returning either would bypass the re-check that catches an install
    /// which reported success and changed nothing.
```

- [ ] **Step 5: Run everything**

Run: `cd ~/crates && cargo test --locked -p deps-core`
Expected: PASS.

Run: `cd ~/crates && cargo clippy --locked --all-targets -- -D warnings`
Expected: 0 errors.

Run: `cd ~/crates && cargo test --locked`
Expected: PASS across all three members. This is the last core task, so the
whole workspace matters.

- [ ] **Step 6: Verify the test binds**

```sh
cd ~/crates
cp deps-core/src/outcome.rs /tmp/outcome.rs.good
python3 - <<'PY'
import pathlib
p = pathlib.Path("deps-core/src/outcome.rs")
b = p.read_text()
old = "        matches!(outcome, StepOutcome::AlreadyPresent | StepOutcome::Installed)"
new = "        matches!(outcome, StepOutcome::AlreadyPresent | StepOutcome::Installed | StepOutcome::Declined)"
assert b.count(old) == 1
p.write_text(b.replace(old, new))
PY
cargo test --locked -p deps-core a_declined_step 2>&1 | grep -cE 'FAILED'
cp /tmp/outcome.rs.good deps-core/src/outcome.rs && rm /tmp/outcome.rs.good
cargo test --locked -p deps-core a_declined_step 2>&1 | tail -1
```

Expected: treating a decline as ready makes the test fail; restoring passes.

- [ ] **Step 7: Rebuild, run the shell suites, and commit**

```bash
cd ~ && config build >/dev/null 2>&1 && config doctor; echo "doctor=$?"
bash tests/config-manifest-lifecycle.test.sh 2>&1 | tail -1
bash tests/container.test.sh 2>&1 | tail -1
cat > /tmp/msg.txt <<'MSG'
Represent a declined step, and write down the gather contract

There was no StepOutcome for "the user said no". NotAutomatable claims no
automated install exists, Blocked claims a later wave can unblock it, and
InstallFailed claims an attempt failed. All three are false for a decline,
and reporting it as any of them is the false statement parent 6.2 rejected
DescribeOnly for. Approval is per-step per parent 6.3, and two CI legs pin
--fix --yes, so the adapter needs this variant and a --yes flag.

The gather contract was unwritten, and it is the riskiest edge in the design
because a wrong answer there is silent. Four rules now live on the
Observations trait: enumerate every AnyOf leaf, because an absent key reads
as Absent; a failed probe is Unresolvable rather than Absent; Unresolvable
does not block; and roots resolve at the instant of use, because installing
oh-my-zsh creates the directory a later check reads.

perform's return type was wider than any correct implementation uses.
AlreadyPresent and InstalledButCheckStillFails are reconcile's to produce, so
an installer returning either bypasses the re-check that catches an install
reporting success while changing nothing.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_015V9aqvoTCJYbbVKbXXgqot
MSG
config add crates/deps-core/src/outcome.rs crates/deps-core/src/check.rs crates/deps-core/src/driver.rs
config commit -F /tmp/msg.txt && rm /tmp/msg.txt
```

---

## Self-review

**Spec coverage.** The spec's seven items map to tasks: item 1 to Task 1,
item 2 to Task 2, item 3 to Task 3, item 4 (`describe`) to Task 4, item 5
(`Requirements` gaps) to Task 5, item 6 (`OhMyZshCustom`) to Task 6, item 7
(gather rules, approval, `perform`'s width) to Task 7. The spec's section 5
names three tests that must not pass vacuously; they are Task 1 Step 1,
Task 3 Step 1, and Task 5 Step 1.

**Placeholder scan.** No TBD, no "similar to Task N", no "add error
handling". Every code step carries real code. Two places name a verification
rather than an assertion, both deliberately: Task 6 Step 2 records a
characterization test rather than a red-green cycle and says so, and Task 7
Step 3 says to verify the two folds rather than assuming they need no change.

**Type consistency.** `PackageAvailability::Clone { source, into }` in Task 1
is what Task 6's `CheckPath` equality test constrains. Task 2's
`Verdict::Install(report.install)` is deliberately one-argument and Task 3
Step 5 changes it, which is stated in Task 2 Step 3. `CheckPath::new(root,
CheckRelPath)` and `PackageMap::new(BTreeMap, PackageAvailability)` are the
real signatures, verified; there is no `CheckPath::parse`.

**One dependency between tasks worth restating.** Task 3 widens
`Verdict::Install`, and Task 7's test constructs one. Run them in order.
