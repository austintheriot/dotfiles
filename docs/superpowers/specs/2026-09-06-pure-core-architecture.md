# Pure core architecture for the dotfiles tooling (2026-09-06)

**Supersedes `2026-09-06-rust-migration-design.md`**, written earlier the same
day. That spec kept the tmux scripts and the whole bash test harness in shell
and treated the migration as a per-script judgment. This one replaces the
judgment with an architecture: a pure Rust core, effects behind injected
ports, and shell reduced to the two things a separate process cannot do.

The earlier spec is left in place rather than edited, because its reasoning
about the bootstrap phase boundary is still correct and section 6 of this
document builds on it. Where the two disagree, this one wins.

Follows `2026-09-04-config-command-and-manifest-crate-design.md`, which
established `crates/config-manifest`.

Every measurement in this document was taken on the owner's machine (darwin,
Apple Silicon) and reproduced at least twice. Claims that were not measured
say so.

## 1. Goals

- One architecture for every piece of tooling in the repo, rather than a
  per-script decision about language.
- A core that can be tested without touching a filesystem, spawning a
  process, or reading an environment variable.
- Effects isolated behind ports with more than one real implementation, so
  `--dry-run` is a different implementation rather than a branch inside a
  loop.
- The dependency manifest becomes pure data. It currently carries executable
  shell.
- Shell reduced to what a separate process structurally cannot do.

## 2. Non-goals

- Performance. The per-prompt path gets roughly 3ms slower and that is
  accepted; see 3.2. No part of this work is justified by speed.
- Replacing `git` or `tmux` with libraries. Settled by measurement in
  `docs/research/rust-external-tool-boundaries.md`: tmux control mode loses to
  a plain spawn for a short-lived process, and gitoxide cannot express
  `config-stamp` at all.
- Retrofitting a `GitRepo` trait onto the existing crate. Withdrawn; see 8.3.

## 3. Decisions, with the evidence

### 3.1 The core is pure, and the existing crate already proves it

`crates/config-manifest` contains **zero traits**, and its five modules
`manifest`, `check`, `plan`, `path`, `tree` contain **zero** references to
`std::fs`, `std::process`, `std::env`, `std::io`, or `Command::new`. All 37
IO references live in `git.rs` and `main.rs`. Verified by grep.

So pure-core-with-effects-at-the-edges is not aspirational here. It is the
established pattern, achieved by passing values. The one place that crate
failed is `run_sync` in `main.rs:153-293`: 140 lines of policy fused to
`Command`, untestable without a real repository. That failure is a missing
pure function, not a missing trait.

### 3.2 The prompt path gets slower, deliberately

Measured, three ways:

| | Cost |
|---|---|
| `tmux-update-window-names.sh`, single-window path | **17.8 ms** |
| bare `tmux display-message` | 12.4 ms |
| empty compiled binary (spawn floor) | 8.7 ms |

Any binary on that path pays its own spawn plus tmux's, so the floor is about
21 ms against the shell's 17.8 ms. A Rust port is measurably slower and no
implementation can avoid asking tmux.

This is accepted because the goal is a maintainer who can read the code, not
a faster prompt. Recorded so it is not rediscovered as a surprise.

The cost that actually matters in that script is the `--all` path at
**1473 ms**, roughly 80x the per-prompt path, and its fix carries into the
Rust port unchanged; see 7.4.

### 3.3 `Probe` is not a port

The first draft of this design injected a `Probe` trait. All four consulted
lenses rejected it, including the one asked to argue the opposite.

The core takes gathered observations as a value, so it never calls `Probe`.
A trait the core does not invoke is not dependency injection of the core.

Gather-then-decide is correct precisely when observations are finite,
enumerable ahead of time, and independent of the decisions. The dependency
manifest lists every dependency, each check costs microseconds, and no check
depends on another. All three hold.

A value is also a better test double than a mock: a mock has behavior that can
be wrong (what does it return for an unasked name, in what call order), and a
`BTreeMap` literal has none.

### 3.4 Effects are ports, and there are exactly two

`Installer` and `PrivilegedInstaller` are traits because each has several real
implementations: the package-manager adapters, a dry-run implementation, and a
recording implementation for tests. Several implementations is the bar a trait
has to clear.

The dry-run argument is the strongest one. Today `dry_run` is a boolean tested
inside the install loop, tangled with the interactive prompt. As a separate
implementation, `--dry-run` selects an object and the loop has no branch at
all. The dry-run output is then produced by the same object that would have
done the work, which is the only way it stays truthful.

### 3.5 Privilege is a capability the adapter is constructed with

Not a boolean on the action: whether an install needs root is a property of
apt, not of the dependency, so a manifest could claim otherwise and be wrong.
Not adapter-only: `--dry-run` must disclose privileged steps before the first
password prompt. Not an `Executor::needs_elevation` query: that asks the
executor a question it should answer itself, and it opens a check-then-act
gap.

Instead the elevation state is resolved **once**, at the edge, and the adapter
is built with it. `check-deps.sh:173-188` already computes exactly this
three-state value, and `DEPS_FORCE_ROOT` exists only so both branches are
testable, so naming it as a type deletes that environment seam.

This is **not** a WebAssembly-grade capability, and the spec should not claim
it is. `sudo` is ambient authority: any code in the process can invoke it, so
the capability is not unforgeable. Two narrower properties are real:

- Today the "no root available" case is enforced by **sniffing the command
  text** for a literal `${SUDO}` (`check-deps.sh:545`). That is a runtime
  string test standing in for a type, and it is one refactor from silently
  passing.
- `depcheck-hook.sh` runs on every interactive shell startup. That path
  currently cannot install only because `--fix` is not passed. Wiring it with
  no `PrivilegedInstaller` makes that a property of the wiring.

### 3.6 The manifest becomes pure data

Today `deps.conf` field 2 is an arbitrary shell string evaluated with
`sh -c`, unconditionally, on every interactive shell startup through
`depcheck-hook.sh`, before any `--dry-run` gate. The file legitimately
contains shell constructs, so a hostile or mistaken entry does not look
anomalous.

All 22 checks across the four conf files were enumerated by reading them. A
closed enum expresses every one, with **no shell escape hatch**; see 5.2.

Install targets stay out of the manifest. Today the conf holds only
documentation URLs, and every install URL is hardcoded at
`check-deps.sh:309,325,340`. A prior review called that deliberate and
correct. A manifest-supplied URL would make one edited line an
arbitrary-code-execution vector, amplified because `config sync` writes across
branches with `commit-tree`, which runs no hooks, so a synced conf file
reaches the other branch without passing pre-commit.

### 3.7 Shell keeps two things, on mechanism rather than taste

`zsh-git-widgets.sh` registers a zsh line-editor widget and assigns to
`LBUFFER`, the calling shell's command-line buffer. No separate process can
do that.

`setup.sh`, `config-init` and `config-install-hooks` run before a toolchain
exists. `rustup` is itself a dependency entry, so `config-install` is what
places cargo.

Everything else converts. Of the four tmux scripts currently sourced through
aliases, `tmux-close.sh`, `tmux-setup.sh` and `tmux-split.sh` convert cleanly:
their `return` statements are early-exit guards passing no value back.
`tmux-start.sh` converts except its final `tmux attach`, which moves into the
alias.

## 4. The architecture

```
observations = gather(&selection)              // edge: Probe impl, private
plan         = plan(&manifest, manager, &selection, &observations, elevation)
                                               // pure: no IO of any kind
outcomes     = plan.steps.map(installer.perform)
                                               // edge: sequential, effectful
after        = gather(&plan.attempted)         // edge: only what was tried
report       = reconcile(&plan, &outcomes, &after)     // pure
rendered     = render(&report, verb)           // pure -> Rendered value
main         = write two streams, return one exit code // the only exit code
```

Data flows one direction. The core produces inert values; the driver performs
effects; the core classifies the results. Nothing in the core can trigger IO,
which is a security property as well as a testing one: a core holding no
capability cannot be induced to use one.

`Rendered { stdout, stderr, exit_code }` follows `check.rs:31-36`, which
already returns rendered output as a value in this crate.

## 5. The core's types

### 5.1 Install intent

The core emits intent, never a command string. This is the owner's stated
boundary: the core knows what it means to do, not how the command is written.

```rust
pub enum InstallAction {
    Package  { id: PackageId },
    Brew     { kind: BrewKind, id: PackageId, tap: Option<TapName> },
    Script   { installer: ScriptInstaller },
    GitClone { source: CloneSource, into: ClonePath },
    NvmInstall { spec: NodeSpec },
    NotAutomatable { reason: NoInstallReason, docs: HttpsUrl },
}

pub enum ScriptInstaller { Rustup, OhMyZsh, Zoxide }
pub enum CloneSource     { Tpm, ZshAutosuggestions }
```

`ScriptInstaller` and `CloneSource` are closed sets of *identities*. The
adapter maps each to a hardcoded URL. Adding one is a code change that appears
in a diff and passes pre-commit, which is the control 3.6 requires.

`Brew` carries structure rather than a command, because brew has real
structure. `aerospace` needs a tap, a trust, and a cask install; the core says
"cask `aerospace` from tap `nikitabobko/tap`" and tapping and trusting are
adapter policy.

`NoInstallReason` is a sum, not a comment:

```rust
pub enum NoInstallReason {
    UpstreamPublishesNoStableUrl,                       // nvm
    RequiresInteractiveApproval,                        // cc on macOS
    NotPackagedForThisManager,                          // dash on brew
    PrerequisiteMissing { dependency: DependencyName }, // node without nvm
}
```

This is the lesson from the orphan-`!` finding in this crate, where
`Rule::Excluded` carries no evidence of what it excludes from and an orphan
silently disables the guard. `NotAutomatable { docs }` alone cannot
distinguish a permanent upstream fact from a regression nobody noticed.

### 5.2 Presence checks

All 22 checks across `deps.conf`, `deps-mac.conf`, `deps-linux.conf` and
`deps-ci.conf` were enumerated. The distribution: 15 `Command`, 2
`DirExists`, 1 `FileNonEmpty`, 1 `GlobExists`, 1 `PythonImport`, 3 `AnyOf`
composing the rest.

```rust
pub enum Check {
    Command(CommandName),
    DirExists(CheckPath),
    FileExists(CheckPath),
    FileNonEmpty(CheckPath),                  // nvm uses -s, not -f
    GlobExists { dir: CheckPath, pattern: GlobPattern },
    PythonImport(ModuleName),
    AnyOf(Vec<Check>),
}

pub enum PathRoot { Home, Absolute, BrewPrefix, OhMyZshCustom }
pub struct CheckPath { root: PathRoot, rest: RelPath }
```

Three details are load-bearing:

**`FileNonEmpty` is distinct from `FileExists`.** The nvm check is
`[ -s "$HOME/.nvm/nvm.sh" ]`. A truncated `nvm.sh` passes `-f` and sources to
nothing, so collapsing the two would introduce a bug during the port.

**`PathRoot` is a closed sum, which deletes all shell expansion.**
`brew --prefix` and `${ZSH_CUSTOM:-$HOME/.oh-my-zsh/custom}` are the only two
environment consultations in all four files, and both are named roots rather
than arbitrary expansion. A root that fails to resolve is `Unresolvable`, not
false.

This also fixes a live bug. The current `zsh-autosuggestions` check is
`test -f "$(brew --prefix 2>/dev/null)/share/..."`. On a machine without
brew the substitution is empty, so it tests `/share/...` at the filesystem
root and silently returns false. Verified. Same shape as the orphan-`!`
finding: absence of evidence rendered as evidence of absence.

**Names are validated at parse time, not at use.** `CommandName::parse`
rejects an empty name, any path separator, a leading `-`, non-printable bytes,
and anything over 64 bytes. A name containing `/` would bypass PATH lookup
entirely if the value ever reached `Command::new`. `RelPath::parse` already
exists in `path.rs` and already rejects escapes; reuse it rather than adding a
second path type.

`PythonImport` is the one variant that spawns an interpreter. Its argument is
a validated module name, not a command line, so the surface is
`python3 -c "import <validated identifier>"` rather than arbitrary code.

**There is no `Check::Shell`.** Every real check fits. A new named variant per
need is strictly better than an escape hatch, because each addition is a
reviewable decision naming its own blast radius while an escape hatch grants
all future blast radius at once.

### 5.3 Per-manager package availability

A map with absent keys cannot distinguish "same name here" from "not
installable here", and the conf files contain both: `git` on brew is `git`,
while `cc` on brew is genuinely unavailable because it needs an interactive
Xcode prompt.

```rust
pub enum PackageAvailability {
    Named(PackageId),
    Unavailable(NoInstallReason),
}
pub struct PackageMap {
    default: Option<PackageId>,
    per_manager: BTreeMap<PackageManager, PackageAvailability>,
}
```

`PackageMap::resolve(manager)` is total. `PackageManager` is a closed enum, so
a misspelled `[dependency.package.aptt]` key is a parse error rather than a
silently ignored line that falls back to the default and installs the wrong
package.

### 5.4 Outcomes and the single exit code

Three of the four listed outcomes are results of a plan that ran correctly,
not errors. Only one is an `Err`.

```rust
pub enum PlanError {                      // the run never started
    UnknownDependency { name: String, did_you_mean: Option<DependencyName> },
    ManifestParse { path: RelPath, detail: ParseError },
    UnknownPackageManager,
}

pub enum StepOutcome {
    AlreadyPresent,
    Installed,
    InstalledButCheckStillFails { check: Check },
    InstallFailed { action: InstallAction, cause: ExecFailure },
    NotAutomatable { reason: NoInstallReason, docs: HttpsUrl },
    Blocked { on: DependencyName },
}
```

`InstalledButCheckStillFails` carries the `Check` so the report names which
predicate failed. `NotAutomatable` as a field forces the exit-code function to
decide about it, which is the direct fix for today's behavior where a
manual-only dependency is invisible to the exit code and `config-init` can
report success on a machine that is not ready.

The mapping to a process exit code exists in exactly one function:

```rust
pub enum ExitStatus { Ready, NotReady, AttemptFailed, CallerError }
pub fn summarize(outcomes: &[StepOutcome], verb: Verb) -> ExitStatus;  // pure
```

| Status | `deps check` | `deps install` |
|---|---|---|
| 0 | ready | every actionable install succeeded |
| 1 | something missing | (unused) |
| 2 | unknown dependency named | unknown dependency named |
| 3 | (unused) | an install failed, or a check still fails |

`AttemptFailed` is 3 rather than 1 deliberately. A prior review found exit
codes returned as bare `u8` from nine sites with status 1 meaning eight
different things, and consumers discriminating by grepping message text.
Reusing 1 here would reintroduce that.

`ExitStatus`'s constructors are private to the render module, so the nine-site
pattern cannot return.

## 6. The ports

```rust
pub trait Installer {
    fn describe(&self, action: &InstallAction) -> ActionDescription;
    fn perform(&self, action: &InstallAction) -> StepOutcome;
}

pub trait PrivilegedInstaller {
    fn describe(&self, action: &InstallAction) -> ActionDescription;
    fn perform(&self, action: &InstallAction) -> StepOutcome;
}

pub enum Elevation { AlreadyRoot, ViaSudo, Unavailable }
```

Both traits are declared in `deps-core`. That is the owner's stated default:
traits for enforced dependency injection and separation of interface from
implementation.

**Recorded dissent**, because it is well argued and empirically supported.
The `oo-architecture` lens argued the declarations belong beside the adapters,
because the core never *calls* either trait (the driver does), so a trait in
the core's public surface misstates the contract, and a crate with no traits
has no test doubles that can drift from the real adapters. Its evidence is
3.1: this repo's existing crate reached a pure core with zero traits. The
accepted cost of choosing otherwise is one trait declaration in a crate that
does not invoke it; the accepted benefit is that nothing moves when the core
does eventually need to call an effect.

The driver holds `Option<Box<dyn PrivilegedInstaller>>`. Absence makes
privileged actions structurally unreachable, which is what 3.5 requires for
the shell-startup path.

`describe` exists so `--dry-run` is a `DescribeOnly` implementation whose
`perform` writes the description and does nothing. Dry-run output is then
generated by the same type that would have performed the work.

**A rule for the future, because this is the failure mode of this design.**
A port gets a new method only when every implementation has a meaningful one.
Batching apt installs belongs in `plan` as a pure grouping transformation, not
as an `install_all` method that only one adapter implements. If `Plan` ever
grows a conditional or an ordering constraint that depends on a runtime
result, it has stopped being a value and `--dry-run` will begin to lie.

## 7. Layout and scope

### 7.1 Crates

```
crates/
  Cargo.toml            workspace root, per-crate stamps (see 8.1)
  rust-toolchain.toml   exact pin
  config-manifest/      existing: the .sync-manifest domain
  deps-core/            pure: manifest, check, plan, action, outcome, ports
  config-cli/           the binary: adapters, drivers, Approval, one exit code
```

Adapters start inside `config-cli` and move to their own crate only when a
second binary needs them. A crate split is a compile-time cost paid on every
build, so YAGNI applies harder to crate boundaries than to types.

The long-term shape is one binary with subcommand dispatch rather than several
binaries with several argument parsers and several exit-code conventions. Two
binaries is how status 1 comes to mean eight things again.

### 7.2 What stays shell

| File | Reason |
|---|---|
| `setup.sh` | Entry point on a machine with nothing installed. |
| `config-init`, `config-install-hooks` | Run before a toolchain exists. |
| `.scripts/config/config` | Must locate and exec binaries including when absent. 34 lines. |
| `platform.sh` | Sourced by `.zshrc`, `setup.sh` and tmux config; exports `DOTFILES_PLATFORM`. |
| `zsh-git-widgets.sh` | Registers a ZLE widget and assigns `LBUFFER`. Structurally impossible from another process. |
| `.zshrc` | The shell's own configuration. |

### 7.3 Tests

All 43 suites become `cargo test`. `tests/lib.sh` and `tests/run-all.sh` are
deleted. The suites divide three ways, and the value differs:

- **10 suites get better.** They currently `grep` and `sed` over tracked
  files. In Rust they get real parsers: `serde_yaml` for the workflow
  assertions, a Markdown parser for link checking. That eliminates a bug class
  this repo has already hit, where `config-docs.test.sh` became a
  zero-iteration loop when a README bullet format changed and passed silently.
- **12 suites are equivalent** with better fixtures. `assert_cmd` plus
  `tempfile` replaces the harness; `tempfile`'s cleanup is more reliable than
  the `mktemp -d` and trap pairing, and it fixes the fixture-ownership defect
  where cleanup kills tmux sessions by name pattern on a shared server.
- **5 suites keep shell as their subject.** The `zshrc-*` suites test zsh
  behavior, including one that times shell startup. Rust drives them; the
  subject stays shell. This is honest rather than a gap.

The pre-push container gains a Rust toolchain in its runtime stage, keeping
the multi-stage build for its dependency-caching layer. `cargo test` becomes
the gate.

### 7.4 Order of work

**Step 0. The four verified blockers**, per
`docs/superpowers/plans/2026-09-06-blockers-and-build-infrastructure.md`
Tasks 1 to 4. Three leak-guard fail-opens on a public repo and a test suite
that reports PASS on zero assertions. Independent of this architecture and
higher priority than all of it.

**Step 1. `--describe` on the dispatcher.** `config-help` reads descriptions
with `sed` over source text, so the first ported subcommand would list as
`(undocumented)`. Verified against the installed binary.

**Step 2. Workspace, toolchain pin, per-crate stamps.** Plan Tasks 5 to 7,
unchanged by this spec.

**Step 3. `deps-core` plus the adapters.** The first and largest application
of this architecture. `check-deps.sh` is 313 code lines of which only 61
invoke an external tool, the best logic-to-orchestration ratio in the repo.

**Step 4. The remaining `config-*` subcommands.**

**Step 5. The tmux scripts.** Convert three of the four sourced ones to
binaries; move `tmux-start.sh`'s `attach` into its alias; port
`tmux-update-window-names.sh` with the `--all` fix folded in rather than
applied separately in shell first. That fix is: guard on `.git` existing
before any git call, then read `.git`, `commondir` and `HEAD` directly, with
`git rev-parse` retained as the fallback for shapes a file read does not
model. Verified to agree with `git branch --show-current` on 21 of 21 live
window directories, including linked worktrees whose `.git` is a file.

**Step 6. The test port.** Last, because it is the safety net for everything
above it.

## 8. Consequences for existing decisions

### 8.1 The per-crate stamp is unchanged

`config-stamp` remains the sole owner of the stamp rule, in shell, because
`config-build` calls it to decide what to compile. A Rust owner would be
circular. This is unchanged from the superseded spec and from the committed
plan.

### 8.2 Task 9 of the committed plan is dropped

That task rewrote `tmux-update-window-names.sh` in shell. Under this
architecture the script becomes Rust, so the shell rewrite would be discarded.
The measured logic and the 21-of-21 equivalence evidence carry into step 5
above.

Task 8 of that plan **stays**: it fixes `parse_git_dirty` in `.zshrc`, which
remains shell permanently, so that work is never wasted. Measured 299 ms to
44.6 ms in a 24,453-file worktree.

### 8.3 The `GitRepo` trait recommendation is withdrawn

An earlier review recommended a `GitRepo` trait so `run_sync`'s policy became
testable. Withdrawn. `run_sync`'s defect is that policy was never extracted
into a pure function, and every one of its decisions consumes a small value
and produces a refusal or a next step, which is `plan`-shaped. A 16-method
trait mirroring git subcommands would violate interface segregation by
construction and would exist only to satisfy mocks.

The fix is the same shape as `deps-core`: gather preconditions into a struct
at the edge, a pure function returning `Result<SyncIntent, SyncRefusal>`, and
`SyncRefusal` as an ADT that also removes the bare-`u8` exit codes. Do this
after step 3 has proven the shape on a greenfield case.

## 9. Open questions

- **Node's prerequisite ordering.** `node` installs through nvm and requires
  `nvm.sh` to exist. The current script signals this by emitting an empty
  install command, which is indistinguishable from manual-only. The design
  above adds `StepOutcome::Blocked { on }` and a `requires` field on the
  manifest entry, with `plan` ordering steps topologically. A phantom-typed
  `Step<Ready>` versus `Step<Blocked>` was considered and rejected: there is
  one transition, so a closed sum plus an exhaustive match at the render site
  gets the same guarantee without the ceremony.
- **Whether `pyyaml` should be `PythonImport` or a `FileExists` on a
  site-packages path.** The `PythonImport` variant spawns an interpreter. A
  path check would not, but no stable site-packages path across the three
  managers was verified.
- **Pinning the third-party installer URLs.** `oh-my-zsh` and `zoxide` are
  fetched from `master` and `main`, so two third-party repositories have
  execute access on this machine at install time. This is true today and is
  not created by this work, but the adapter table in 5.1 is the natural place
  to pin a commit SHA and record an expected hash.
- **The `commit-tree` hook bypass.** `config-manifest sync` writes commits
  with plumbing, so a synced file reaches the other branch without passing
  pre-commit. This design reduces its impact, because a synced manifest can no
  longer carry shell, but it does not eliminate it.
