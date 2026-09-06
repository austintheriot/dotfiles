# Pure core architecture for the dotfiles tooling (2026-09-06)

**Supersedes `2026-09-06-rust-migration-design.md`**, written earlier the same
day. That spec kept the tmux scripts and the whole bash test harness in shell
and treated the migration as a per-script judgment. This one replaces the
judgment with an architecture: a pure Rust core, effects behind injected
ports, and shell reduced to what a separate process cannot do.

The earlier spec is left in place rather than edited, because its reasoning
about the bootstrap phase boundary is still correct and section 6 of this
document builds on it. Where the two disagree, this one wins.

Follows `2026-09-04-config-command-and-manifest-crate-design.md`, which
established `crates/config-manifest`.

Every measurement in this document was taken on the owner's machine (darwin,
Apple Silicon) and reproduced at least twice. Claims that were not measured
say so.

**Revision, later the same day.** A six-lens `/expert-review` found nine
blockers in the first draft, and four decisions were taken after it was
written. Both are folded in. The revision log is section 10, which lists
every claim that was corrected and every claim that was withdrawn, because
the corrections are more instructive than the original text.

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
- Shell reduced to what a separate process structurally cannot do, plus the
  bootstrap phase where no toolchain exists yet.

## 2. Non-goals

- Performance. The per-prompt path gets slower and that is accepted; see 3.2.
  No part of this work is justified by speed.
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
IO references live in `git.rs` (27) and `main.rs` (10). Verified by grep.

So pure-core-with-effects-at-the-edges is not aspirational here. It is the
established pattern, achieved by passing values. The one place that crate
failed is `run_sync` in `main.rs:153-293`: 140 lines of policy fused to
`Command`, untestable without a real repository. That failure is a missing
pure function, not a missing trait.

`run_sync` also demonstrates the exit-code defect 5.4 exists to prevent. It
returns a bare `u8` from eleven sites: `Ok(1)` at seven of them, meaning
detached HEAD, wrong source branch, target checked out, origin ahead,
unreadable manifest, and incomplete manifest coverage. Six meanings, one
code.

### 3.2 The prompt path gets FASTER, which reverses the first draft

**This section previously said the opposite, and the first draft's number was
wrong.** It claimed the script's single-window path cost 17.8 ms against a
~21 ms floor for any binary, and accepted a ~3 ms regression as the price of
readability. Re-measured, the comparison inverts.

Measured on this machine, n=60 per row, warmup discarded, reproduced twice:

| | p50 | min | sd |
|---|---|---|---|
| `tmux-update-window-names.sh -w <pane>`, real git pane | **47.7 ms** | 34.9 | 7.2 |
| bare `tmux display-message` | 13.1 ms | 7.6 | 2.8 |
| bare `git rev-parse --abbrev-ref HEAD` in a linked worktree | 15.0 ms | 9.3 | 3.8 |
| `git branch --show-current` in the same worktree | 11.3 ms | 9.7 | 4.3 |
| `/usr/bin/true`, process spawn floor | 6.6 ms | 5.7 | 1.9 |

So the shell path is **47.7 ms**, and a Rust binary's floor on the same work
is one spawn plus tmux plus git: 6.6 + 13.1 + 15.0 ≈ **35 ms**. A port is
faster, not slower, because the script pays a shell interpreter spawn that a
compiled binary does not.

**Why the first draft got 17.8 ms.** The measurement was taken in a context
where `TMUX_PANE` was unset. Line 239 of the script guards the single-window
path on `[ -n "${TMUX_PANE:-}" ]`, so with it unset the script exits early and
the number measures an early exit rather than the prompt path. Reproduced
this exact error during the revision: measuring without a pane gave 15.2 ms,
and passing `-w <a real git pane>` gave 47.7 ms. **Any future measurement of
this script must pin the pane explicitly**, or it measures nothing.

Two consequences for the rest of the document:

- Section 2's non-goal stands unchanged: performance still justifies no part
  of this work. But the spec must stop claiming a regression it does not
  have, because "we accepted 3 ms" is a claim a reader will check.
- The dominant term is the **git spawn**, at 11-15 ms of the 47.7 ms. The
  `--all` fix in 7.4 step 5 replaces it with a direct read of `.git`,
  `commondir` and `HEAD`, so it is worth more than the language change and it
  is what should be measured after the port.

The `--all` path measures **1446-1465 ms p50** (n=15, two passes), which
reproduces the first draft's 1473 ms within 2%. The first draft's "roughly
80x the per-prompt path" is **wrong**: against the corrected 47.7 ms the
ratio is **31x**. The 80x figure came from dividing by the bad 17.8 ms.

One caveat on the absolute number, recorded because it will not reproduce
later: this tmux server now holds 59 windows against the 21 of the original
measurement, yet the total held while per-window cost fell, because 40 of the
59 are now shallow non-repo directories that pay two wasted git spawns each
instead of a full status. Treat 1473 ms as valid-for-that-session, not as a
constant.

### 3.3 `Probe` is not a port

The first draft of this design injected a `Probe` trait. All four consulted
lenses rejected it, including the one asked to argue the opposite.

The core takes gathered observations as a value, so it never calls `Probe`.
A trait the core does not invoke is not dependency injection of the core.

A value is also a better test double than a mock: a mock has behavior that can
be wrong (what does it return for an unasked name, in what call order), and a
`BTreeMap` literal has none.

**Corrected from the first draft.** That draft justified this with "no check
depends on another," which is false. Gather-then-decide is correct here for
two of the three properties it needs, and the third fails:

- Observations are finite and enumerable ahead of time. **Holds**: the
  manifest lists every dependency.
- Each check is cheap. **Holds**: microseconds, no network.
- Checks are independent of the decisions. **Fails.** Two dependency pairs
  cross-satisfy, and in both the install command *emitted for the second* is
  conditional on the first's filesystem state:

  | Prerequisite | Dependent | Where the conditional lives |
  |---|---|---|
  | `nvm` (`deps.conf:36`) | `node` (`deps.conf:45`) | `check-deps.sh:387`, guarding `nvm install --lts` |
  | `oh-my-zsh` (`deps-linux.conf:12`) | `zsh-autosuggestions` (`deps.conf:26`) | `check-deps.sh:339`, guarding the `git clone` |

  Both guards emit **nothing** when the prerequisite is absent, and an empty
  install command renders as manual-only. The source says so in both places:
  "Empty when nvm is absent" and "Report it as manual-only instead."

So gathering once and deciding once is the wrong shape. The fix preserves
purity completely; see section 4.

### 3.4 Effects are ports, and there is exactly one

`Installer` is a trait because it has several real implementations: the
adapter that spawns processes, and a recording implementation for tests.

**Corrected from the first draft**, which declared two traits, `Installer`
and `PrivilegedInstaller`, with byte-identical method signatures. Identical
signatures mean no operation is expressible through one and not the other, so
the pair is one contract with two names. It also forced every
dual-capability adapter, the recorder, and the dry-run implementation to be
written twice, which is the drift risk the section 6 dissent warns about.

Privilege is expressed as data on the plan step instead; see 3.5.

The first draft also claimed `--dry-run` should be a `DescribeOnly`
implementation of the port. Withdrawn; see 6.2. `perform` returns
`StepOutcome`, and no variant of `StepOutcome` is a true statement about a
run that deliberately did nothing.

### 3.5 Privilege is data on the plan step, resolved once at the edge

Whether an install needs root is a property of the package manager, not of
the dependency: apt needs it, brew never does. So it is not a manifest field,
because a manifest could claim otherwise and be wrong.

The elevation state is resolved **once**, at the edge, before `gather`, and
is not part of `observations`. `check-deps.sh:173-188` already computes
exactly this three-state value, and `DEPS_FORCE_ROOT` exists only so both
branches are testable, so naming it as a type deletes that environment seam.

```rust
pub enum Elevation { AlreadyRoot, ViaSudo, Unavailable }
```

**Corrected from the first draft on where the requirement lives.** That draft
rejected a field on the action (right, for the reason above), rejected
adapter-only (right: `--dry-run` must disclose privileged steps before the
first password prompt), and rejected an `Executor::needs_elevation` query
(right: it opens a check-then-act gap). Having rejected all three it left the
driver with no way to route a step to the privileged installer, and then
claimed that absence of that installer made privileged actions
"structurally unreachable." It did not. Nothing in the types performed the
routing.

The requirement belongs on the **plan step**, derived purely from the
`(action, manager)` pair by `plan`, which is exactly where the fact lives:

```rust
pub enum PrivilegeRequirement { None, Root }
pub struct Step { pub action: InstallAction, pub privilege: PrivilegeRequirement }
```

The driver's dispatch is then an exhaustive match the compiler checks, and
`plan` receiving `Elevation::Unavailable` never emits a `Root` step at all.
It emits `NoInstallReason::PrivilegeUnavailable` instead, so the condition
flows through the report and the exit code rather than being discovered at
perform time. `check-deps.sh:546-548` already prints exactly this message.

This is **not** a WebAssembly-grade capability, and the spec should not claim
it is. `sudo` is ambient authority: any code in the process can invoke it, so
the capability is not unforgeable. Two narrower properties are real:

- Today the "no root available" case is enforced by **sniffing the command
  text** for a literal `${SUDO}` (`check-deps.sh:545`). That is a runtime
  string test standing in for a type, and it is one refactor from silently
  passing.
- `depcheck-hook.sh` runs on shell startup. Wiring it with no privileged
  installer makes "cannot install" a property of the wiring rather than of a
  missing flag.

**`ViaSudo` is a prediction, not a guarantee**, and the first draft's claim
that resolving once "closes a check-then-act gap" was too strong. It closes
one gap (two queries about the same fact disagreeing within a run) and widens
another: `command -v sudo` proves a binary exists on PATH. It does not prove
the user is in sudoers, that the 5-minute credential cache is still valid, or
that `NOPASSWD` applies. So resolve-once is still correct, for a different
reason: it makes the plan a deterministic function of one observation, which
is what keeps `--dry-run` truthful. `ExecFailure` needs a variant for
authentication refusal, so "you are not in sudoers and every remaining
privileged step will also fail" is knowable at step 1 of 8.

### 3.6 The manifest becomes pure data

Today `deps.conf` field 2 is an arbitrary shell string evaluated with
`sh -c` (`check-deps.sh:524`), through `depcheck-hook.sh` on interactive
shell startup, before any `--dry-run` gate. The file legitimately contains
shell constructs, so a hostile or mistaken entry does not look anomalous.

**Corrected from the first draft**, which said this happens
"unconditionally, on every interactive shell startup." It does not.
`depcheck-hook.sh` throttles to once per 24 hours through
`~/.cache/depcheck-last-run`, and its own header says "Nags at most once
every 24h." The frequency is roughly once a day per machine, not once per
terminal.

The security argument does not depend on the frequency, and rests on the
stronger half:

- The path evaluates manifest-supplied text with `sh -c` and no gate.
- `config sync` writes across branches with `commit-tree`, which runs no
  hooks, so a synced conf file reaches the other branch **without passing
  pre-commit**. Section 7.4 step 3 shows why this is the primary path rather
  than an edge case.
- `depcheck-hook.sh:7` aliases `depcheck` to `check-deps.sh --fix`. The
  automatic invocation passes no `--fix` and so cannot install, but the
  manual alias is the same binary and does.

All 22 checks across the four conf files were enumerated by reading them. A
closed enum expresses every one, with **no shell escape hatch**; see 5.2.

Install targets stay out of the manifest. Today the conf holds only
documentation URLs, and every install URL is hardcoded in
`install_cmd_for`. A prior review called that deliberate and correct. A
manifest-supplied URL would make one edited line an arbitrary-code-execution
vector, amplified by the `commit-tree` bypass above.

### 3.7 Shell keeps what a process cannot do, and what runs before a toolchain

`zsh-git-widgets.sh` registers a zsh line-editor widget and assigns to
`LBUFFER`, the calling shell's command-line buffer. No separate process can
do that.

`setup.sh`, `config-init` and `config-install-hooks` run before a toolchain
exists. `rustup` is itself a dependency entry, so `config-install` is what
places cargo.

**Added in revision: the deps Docker images are in that second category and
the first draft missed them.** `docker/Dockerfile.ubuntu` and
`Dockerfile.arch` set `ENTRYPOINT` to `check-deps.sh` and copy **only**
`.scripts/deps` into the image. Its own comment says why: "Only the
dependency-checking tree, not the whole dotfiles repo." There is no
`crates/`, no `Cargo.toml`, and no Rust toolchain, and the image installs
only `sudo curl git wget ca-certificates`. `check-deps.sh` runs there
*because it is shell*. See 7.4 step 3 for what this forces.

Everything else converts. Of the four tmux scripts currently sourced through
aliases, `tmux-close.sh`, `tmux-setup.sh` and `tmux-split.sh` convert
cleanly: their `return` statements are early-exit guards passing no value
back. `tmux-start.sh` converts except its final `tmux attach`, which moves
into the alias.

## 4. The architecture

The pipeline is a **fixpoint**, not a single pass, because 3.3 established
that installing one dependency can change another's observation and its
plan.

```
elevation    = resolve_elevation()             // edge: once, before anything
observations = gather(&selection)              // edge: Probe impl, private

loop {
    (plan, events) = plan(&manifest, manager, &selection, &observations, elevation)
                                               // pure: no IO of any kind
    ready          = plan.steps where privilege is satisfiable and not Blocked
    if ready.is_empty() { break }

    let (step_outcomes, attempted) = perform_all(&installers, &ready)
                                               // edge: sequential, effectful
    outcomes.extend(step_outcomes)
    observations = gather(&selection)           // edge: full re-gather
}

(report, events) = reconcile(&plan, &outcomes, &observations)   // pure
rendered         = render(&report, verb)                        // pure -> Rendered
main             = drain events, write two streams, one exit code
```

Data flows one direction. The core produces inert values; the driver performs
effects; the core classifies the results. Nothing in the core can trigger IO,
which is a security property as well as a testing one: a core holding no
capability cannot be induced to use one.

Four properties of this shape are load-bearing.

**Each iteration's `plan` is individually a pure value.** So section 6's rule
("if `Plan` grows a conditional that depends on a runtime result it has
stopped being a value") is preserved, restated as: `plan` may order steps;
`plan` may not condition an action on another step's outcome; the driver may
skip a step whose prerequisite failed. That makes the first draft's defect
unrepresentable rather than merely documented.

**The loop body is more testable than a single pass, not less.** It is a pure
step function driven by a scripted sequence of observation maps. No mock, no
call-order semantics.

**Termination**: every iteration either performs at least one step or breaks.
Steps are removed from consideration once attempted, so the loop runs at most
once per dependency.

**The re-gather is full, not scoped to what was attempted.** The first draft
had `gather(&plan.attempted)`, which under-reports: installing `oh-my-zsh`
makes `zsh-autosuggestions` installable, and a scoped re-gather would still
call it missing. 3.3 establishes each check costs microseconds, so scoping
was a false economy. Full re-gather is also *required* if apt installs are
ever batched, because one `apt-get install a b c` yields one exit status for
three dependencies and per-step outcomes stop being derivable from it.

**`attempted` comes back from the loop, not from `plan`.** In the first draft
`after = gather(&plan.attempted)` took an argument available the instant
`plan` returned, so writing the re-gather *before* the perform loop compiled,
type-checked, and reconciled every outcome against a pre-install world.
`perform_all` returning `attempted`, with a constructor private to the driver
module, makes that reorder a compile error. This is the technique 5.4 already
applies to `ExitStatus`.

`Rendered { stdout, stderr, exit_code }` follows `check.rs:31-36`, which
already returns rendered output as a value in this crate.

### 4.1 Observability: the core returns events, it does not hold a logger

Decided after the first draft. The core returns structured events alongside
its values; the driver drains them into a `Services` handle that owns the
`Log`. `Services` is a parameter to the **driver**, not to the core.

```rust
pub trait Log {
    fn debug(&self, event: &Event);
    fn info(&self, event: &Event);
    fn warn(&self, event: &Event);
    fn error(&self, event: &Event);
}
pub trait Services { fn log(&self) -> &dyn Log; }
```

Passing `Services` into `plan` would falsify section 4's claim that nothing
in the core can trigger IO, and would contradict 3.3: a trait the core *does*
invoke is exactly what `Probe` was rejected for being. Returning events keeps
both claims true and is strictly more testable, because a test asserts on an
event vector as a value.

Under the fixpoint the event vectors concatenate across iterations, which is
the append that makes this work.

The tradeoff, recorded so it is not rediscovered: the core cannot log
*during* a computation, so a hang inside `plan` produces no output. `plan` is
a fold over 22 entries, so this does not matter now. If `plan` ever becomes
slow, revisit.

## 5. The core's types

### 5.1 Install intent

The core emits intent, never a command string. This is the owner's stated
boundary: the core knows what it means to do, not how the command is written.

```rust
pub enum InstallAction {
    Package    { id: PackageId },
    Brew       { kind: BrewKind, id: PackageId, tap: Option<TapName> },
    AptSource  { keyring: KeyringSource, list: SourceListEntry },
    Pip        { id: PackageId, break_system_packages: bool },
    Script     { installer: ScriptInstaller },
    GitClone   { source: CloneSource, into: CheckPath },
    NvmInstall,
    NotAutomatable { reason: NoInstallReason },
}

pub enum ScriptInstaller { Rustup, OhMyZsh, Zoxide }
pub enum CloneSource     { Tpm, ZshAutosuggestions }
```

`ScriptInstaller` and `CloneSource` are closed sets of *identities*. The
adapter maps each to a hardcoded URL. Adding one is a code change that
appears in a diff and passes pre-commit, which is the control 3.6 requires.

**Three variants were added in revision, because the first draft's enum could
not represent installs that exist today.**

`Pip` exists because `pyyaml` on brew and on an unknown manager is
`python3 -m pip install --break-system-packages pyyaml`
(`check-deps.sh:411`). No first-draft variant fitted: it is not the detected
manager, brew has no formula (which is *why* that line exists), and
`ScriptInstaller` is a closed set of three that does not include pip.
`break_system_packages` is a named field rather than a hidden default because
it overrides PEP 668, and 3.6's own argument is that blast radius belongs in
the type.

`AptSource` exists because `gh` on apt (`check-deps.sh:236`) is not a package
install. One command installs `wget`, creates `/etc/apt/keyrings` mode 755,
fetches `githubcli-archive-keyring.gpg` from `cli.github.com`, tees it into
that directory under `sudo`, appends a `deb` line to
`/etc/apt/sources.list.d/github-cli.list`, runs `apt-get update`, and only
then installs. Collapsing that to `Package { id: "gh" }` means `describe`
prints "install package gh" for an action that **permanently adds a
third-party APT trust root**, which defeats 3.4's truthfulness requirement
and 3.5's disclosure requirement. `Brew` already carries `tap` for exactly
this reason ("brew has real structure"); apt has the same structure here and
the first draft gave it no field. `plan` emits `AptSource` as a separate step
ordered before the `Package` step, which composes with the topological
ordering section 9 already needs.

`NvmInstall` lost its `NodeSpec` payload. There is one node entry, no conf
file pins a version, and the only value is `--lts`. A `String` payload there
would reopen 3.6 for one dependency by admitting shell-adjacent text as data.

`NotAutomatable` also lost its `docs` field, which moves to the manifest
entry where the URL already lives. The first draft typed it `HttpsUrl`, and
**that type cannot parse the manifest this repo ships**: 21 of 22 docs URLs
are `https://`, and `deps-ci.conf:23` is
`http://gondor.apana.org.au/~herbert/dash/`. The URLs are printed for a human
and never fetched, so the field is a scheme-agnostic `DocsUrl`.

`NoInstallReason` is a sum, not a comment:

```rust
pub enum NoInstallReason {
    UpstreamPublishesNoStableUrl,                       // nvm
    RequiresInteractiveApproval,                        // cc on macOS
    NotPackagedForThisManager,                          // dash on brew
    ManagerNotNamedInManifest { manager: PackageManager },
    PrivilegeUnavailable,                               // needs root, none available
    PrerequisiteNotYetInstalled { dependency: DependencyName },
}
```

The last three are new. `PrivilegeUnavailable` is what 3.5 requires so an
unsatisfiable privileged step is never planned.
`ManagerNotNamedInManifest` is what 5.3 requires so `resolve` can be total
without fabricating a reason. `PrerequisiteNotYetInstalled` replaces the
first draft's `PrerequisiteMissing`, whose name read as permanent: under the
fixpoint in section 4 the correct meaning is "not in this wave, retry after
one," which is a different claim from "this can never be automated."

This is the lesson from the orphan-`!` finding in this crate, where
`Rule::Excluded` carries no evidence of what it excludes from and an orphan
silently disables the guard. `NotAutomatable` with no reason cannot
distinguish a permanent upstream fact from a regression nobody noticed.

**`NotAutomatable` appears in both `InstallAction` and `StepOutcome`, and
that is correct rather than duplication.** They are different propositions.
As an *action* it is the terminal element of the algebra: `plan` returns an
`InstallAction` for every missing dependency, never `Option<InstallAction>`,
and that totality is what fixes today's bug where "no install" is signalled
by an **empty string** (`check-deps.sh:374,389`) which `:568` cannot
distinguish from a missing prerequisite. As an *outcome* it records that the
driver performed the step and correctly did nothing. Written down so nobody
later "simplifies" the pair away.

### 5.2 Presence checks

All 22 checks across `deps.conf`, `deps-mac.conf`, `deps-linux.conf` and
`deps-ci.conf` were enumerated, and re-counted during revision.

The **top-level** distribution, one `Check` per manifest entry:

| Variant | Count | Entries |
|---|---|---|
| `Command` | 15 | git, gh, zsh, neovim, fzf, ripgrep, zoxide, tmux, shellcheck, cc, rustup, aerospace, xclip, python3, dash |
| `DirExists` | 2 | tpm, oh-my-zsh |
| `FileNonEmpty` | 1 | nvm |
| `PythonImport` | 1 | pyyaml |
| `AnyOf` | 3 | alacritty, zsh-autosuggestions, node |
| **total** | **22** | |

The three `AnyOf` entries decompose to six leaves: `AnyOf(DirExists,
Command)` for alacritty, `AnyOf(FileExists, FileExists)` for
zsh-autosuggestions, `AnyOf(Command, GlobExists)` for node.

**Corrected from the first draft**, whose distribution summed to 23 across 22
entries because it mixed the two levels: it counted the three `AnyOf` entries
*and* separately counted `GlobExists`, which occurs only as a leaf. It also
listed no `FileExists` while the enum declares the variant and two leaves use
it.

```rust
pub enum Check {
    Command(CommandName),
    DirExists(CheckPath),
    FileExists(CheckPath),
    FileNonEmpty(CheckPath),                  // nvm uses -s, not -f
    GlobExists { dir: CheckPath, pattern: GlobPattern },
    PythonImport(ModuleName),
    AnyOf { first: Box<Check>, rest: Vec<Check> },
}

pub enum PathRoot { Home, MacApplications, BrewPrefix, OhMyZshCustom }
pub struct CheckPath { root: PathRoot, rest: CheckRelPath }

pub enum Observation { Present, Absent, Unresolvable { root: PathRoot } }
```

Five details are load-bearing.

**`AnyOf` cannot be empty.** The first draft had `AnyOf(Vec<Check>)`, and
`AnyOf(vec![])` is a well-typed value that evaluates false under any
reasonable rule, so a manifest entry parsing to it reports a dependency
**permanently missing with no diagnostic** and nags forever with no way to
satisfy it. That is the same shape as the two bugs this section is proudest
of catching. `{ first, rest }` makes non-emptiness structural with no new
dependency, and all three real uses have exactly two branches.

**`FileNonEmpty` is distinct from `FileExists`.** The nvm check is
`[ -s "$HOME/.nvm/nvm.sh" ]`, verified byte-exact. A truncated `nvm.sh`
passes `-f` and sources to nothing, so collapsing the two would introduce a
bug during the port.

**`PathRoot` is a closed sum, which deletes all shell expansion.** A root
that fails to resolve is `Unresolvable`, not false.

This fixes a live bug. The current `zsh-autosuggestions` check is a single
`test` with two `-f` operands joined by `-o`, whose second operand is
`"$(brew --prefix 2>/dev/null)/share/..."`. On a machine without brew the
substitution is empty, so it tests `/share/...` at the filesystem root.
Verified by `sh -x` trace: the command becomes
`test -f /share/zsh-autosuggestions/zsh-autosuggestions.zsh`. The inverse is
reachable too: an empty substitution in a `-d` test against `/tmp` returns
**true**. Same shape as the orphan-`!` finding, in both directions.

*Two corrections to the first draft here.* It quoted the check as if the brew
operand were the whole test, eliding the oh-my-zsh fallback and the `-o`;
`grep` for the quoted form returns zero hits. And the bug is **dormant on
this machine**, which has brew at `/opt/homebrew`. It fires on a brew-less
machine, which is exactly the fresh-macOS case `setup.sh` exists for.

**`PathRoot::Absolute` is replaced by `MacApplications`.** The first draft had
an open `Absolute` variant with **zero** instances in any conf file. The only
absolute path in the corpus is `/Applications/Alacritty.app`, so a named
variant covers it, by the same rule this section applies to `Check::Shell`: a
named variant per need beats an escape hatch, because each addition names its
own blast radius. An open `Absolute` also reopens precisely what
`RelPath::parse` closes.

**`CheckRelPath` is a new type, not a reuse of `RelPath`.** The first draft
said "reuse it rather than adding a second path type." Two reasons that is
wrong here. First, `RelPath` lives in `config-manifest`, and `deps-core`
depending on it would make the dependency-manifest domain depend on the
git-sync domain for a string newtype, dragging in a `git` module it never
calls; see 7.1. Second, `RelPath::parse` rejects exactly three things
(`Empty`, `Absolute`, and any `..` segment) and its own tests assert that
`a/./b` and `dir/sub dir/fïle.txt` are accepted. It does not reject
backslashes, NUL or control bytes, a leading `~`, or a Windows drive prefix.
"Already rejects escapes" oversells a three-branch check for a type that will
be composed with a variable root. `CheckRelPath::parse` rejects all of the
above, and the two types stay in their own domains.

**Names are validated at parse time, not at use.** `CommandName::parse`
rejects an empty name, any path separator, a leading `-`, non-printable
bytes, and anything over 64 bytes. A name containing `/` would bypass PATH
lookup entirely if the value ever reached `Command::new`.

`PythonImport` is the one variant that spawns an interpreter. Its argument is
a validated module name, so the surface is
`python3 -c "import <validated identifier>"` rather than arbitrary code.

**One accidental property, now made deliberate.** `PythonImport`'s only
instance is in `deps-ci.conf`, which is selected only by an explicit
`DEPS_CONF` and never by platform detection, so the sole interpreter-spawning
check never runs on the shell-startup path. Nothing enforced that. Parse
rejects `PythonImport` in a platform-selected conf file, so the property is a
rule rather than a coincidence.

**There is no `Check::Shell`.** Every real check fits. A new named variant per
need is strictly better than an escape hatch, because each addition is a
reviewable decision naming its own blast radius while an escape hatch grants
all future blast radius at once.

**Observations are three-state, not boolean.** `Unresolvable` is invented in
this section to fix a silent-false bug, and the first draft then discarded it
by keying observations to a per-dependency boolean. `reconcile` must
distinguish "brew is absent so this check is unanswerable" from "the file is
missing," because they have different remedies. Roots are resolved by
`gather` at the edge and recorded in `observations` **for deciding only**;
adapters resolve roots again at the instant of use, because `oh-my-zsh`'s own
install creates the directory `OhMyZshCustom` names, so a root resolved at
gather time is stale by perform time on the exact path this section fixes.

### 5.3 Per-manager package availability

A map with absent keys cannot distinguish "same name here" from "not
installable here", and the conf files contain both: `git` on brew is `git`,
while `cc` on brew is genuinely unavailable because it needs an interactive
Xcode prompt.

```rust
pub enum PackageAvailability {
    Named(PackageId),
    ViaScript(ScriptInstaller),
    Unavailable(NoInstallReason),
}
pub struct PackageMap {
    per_manager: BTreeMap<PackageManager, PackageAvailability>,
    fallback: PackageAvailability,          // mandatory, states its own reason
}
```

`PackageManager` is a closed enum **including `Unknown`**, so a misspelled
`[dependency.package.aptt]` key is a parse error rather than a silently
ignored line that falls back to the default and installs the wrong package.

**Two corrections to the first draft.**

Its `default: Option<PackageId>` made `resolve` total only in the trivial
sense. With `default: None` and no key for the queried manager there is no
correct answer: none of the four `NoInstallReason` variants meant "the
manifest author omitted this manager," and fabricating
`NotPackagedForThisManager` would be a false positive claim about upstream,
which is the failure 5.1 objects to. A **mandatory** `fallback` that must
state its own reason makes `resolve` genuinely total, and
`ManagerNotNamedInManifest` gives it something true to say.

Its `PackageAvailability` had no way to express "no package here, but a script
works," which `zoxide` needs: apt, brew and pacman all have packages, and any
other manager gets `curl | sh` (`check-deps.sh:309`).
`ScriptInstaller::Zoxide` existed in 5.1 but the map could not select it, so
5.1 and 5.3 did not compose. `ViaScript` closes that.

`PackageManager::Unknown` is a **variant, not an error.** The first draft had
`PlanError::UnknownPackageManager`, which would abort the run. Today
`detect_pm` returns the literal `unknown` and the script **proceeds**: every
dependency reports manual-only with its docs URL. On a fresh macOS box with
no brew, which is the machine `setup.sh` exists for, the useful output is that
list. Aborting would replace it with one line.

### 5.4 Outcomes and the single exit code

Three of the four listed outcomes are results of a plan that ran correctly,
not errors. Only one is an `Err`.

```rust
pub enum PlanError {                      // the run never started
    UnknownDependency { name: DependencyName, did_you_mean: Option<DependencyName> },
    MalformedSelector { raw: RawSelector },
    ManifestParse { path: CheckRelPath, detail: ParseError },
    ManifestVersion { found: u32, supported: u32 },
    RequirementCycle { chain: Vec<DependencyName> },
}

pub enum StepOutcome {
    AlreadyPresent,
    Installed,
    InstalledButCheckStillFails { check: Check },
    InstallFailed { action: InstallAction, cause: ExecFailure },
    NotAutomatable { reason: NoInstallReason },
    Blocked { on: DependencyName },
}

pub enum ExecFailure {
    NonZeroExit { code: i32, stderr: BoundedText },
    AuthenticationRefused,                // sudo said no; later steps will too
    Spawn(SpawnError),
}
```

`InstalledButCheckStillFails` carries the `Check` so the report names which
predicate failed. `NotAutomatable` as a variant forces the exit-code function
to decide about it, which is the direct fix for today's behavior where a
manual-only dependency is invisible to the exit code and `config-init` can
report success on a machine that is not ready.

**Corrections to the first draft.** `UnknownDependency { name: String }` beside
a validated `did_you_mean: DependencyName` was a parse-don't-validate
inconsistency, and it was the only pre-parse value in the core's whole type
surface. `--only` values are parsed at the CLI boundary, so a malformed
selector is `MalformedSelector` (carrying `RawSelector`, a newtype whose only
guarantee is "bounded and safe to render") and `UnknownDependency` is
symmetric. This matches the script, which validates every selector before the
check loop. `RequirementCycle` and `ManifestVersion` are new; see 5.5 and 9.
`ExecFailure` is specified rather than left undefined, and its `stderr` is
`BoundedText` because an unbounded subprocess string in an error type is how
a terminal gets a control sequence written to it.

The mapping to a process exit code exists in exactly one place, and the verbs
do not share a status type:

```rust
pub enum CheckStatus   { Ready, NotReady }
pub enum InstallStatus { AllSucceeded, AttemptFailed }

pub fn summarize_check(outcomes: &[StepOutcome]) -> CheckStatus;      // pure
pub fn summarize_install(outcomes: &[StepOutcome]) -> InstallStatus;  // pure

pub struct ExitStatus(u8);   // private field; constructed only here
pub fn exit_status(result: Result<Verdict, PlanError>) -> ExitStatus;
```

**Corrected from the first draft**, which had one
`summarize(outcomes, verb) -> ExitStatus` and a table with two "(unused)"
cells. Those cells were a convention the function upheld by hand, not an
impossibility the types enforced: nothing stopped `summarize(_, Verb::Check)`
returning `AttemptFailed`. Worse, `ExitStatus::CallerError` corresponded to a
`PlanError`, meaning **the plan never ran and there are no outcomes**, so it
was a variant of the return type that `summarize` could never legitimately
produce. Splitting by verb makes "(unused)" an absent variant instead of a
table cell, and leaves `exit_status` as the only constructor of `ExitStatus`.

`ExitStatus` is a **newtype with a private field, not a `pub enum`.** The
first draft declared `pub enum ExitStatus` and claimed "its constructors are
private to the render module." That is not implementable: a `pub enum` has
public constructors, and `#[non_exhaustive]` restrains only other crates
while `config-cli` is in the same workspace. The nine-site regression this
prevents is real, so the mechanism has to actually work.

| Status | `deps check` | `deps install` | `--dry-run` |
|---|---|---|---|
| 0 | ready | every actionable install succeeded | nothing missing |
| 1 | something missing | -- | something missing |
| 2 | **any caller error** | **any caller error** | any caller error |
| 3 | -- | an install failed, or a check still fails | -- |

**Exit 2 keeps its repo-wide meaning: the caller made a usage error.** The
first draft narrowed it to "unknown dependency named," which collides with a
tested, uniform convention. Today three distinct misuse conditions exit 2
(`check-deps.sh:109,116,506`: unknown argument, `--only` with no value,
`--only` naming a nonexistent dependency), and clap exits 2 for its own usage
errors at `main.rs:103,114` **deliberately, with a comment saying so**. So
the first draft's claim that one function owns the mapping was false: clap is
a second authority. It is reconciled by making 2 mean all caller errors in
both, which is what the tests already pin.

That pinning is load-bearing and the first draft did not know about it.
`check-deps.test.sh` has **15** numeric status assertions; `config.test.sh`
pins exit 2 for five `config test` misuse cases; and `deps-docs.test.sh:115`
uses `[ "$?" -ne 2 ]` as its **oracle** for "the parser rejected this flag."
Redefining 2 breaks that oracle's semantics.

**`--dry-run` gets its own column, and its exit code changes.** Today it
exits **0 unconditionally** (`check-deps.sh:600-602`), pinned by
`check-deps.test.sh:130` (`'dry-run always exits 0'`). That is a latent hole:
a CI gate on `--dry-run` passes on a machine with everything missing. Dry-run
should exit as `deps check` does, because "would install three things" means
"three things are missing." **This is a deliberate behavior change; see 7.5.**

`AttemptFailed` stays 3, but **not for the reason the first draft gave.** That
reason was that reusing 1 would reintroduce the nine-site problem. It would
not: the nine-site problem was one of *provenance*, fixed completely by the
single funnel plus the private constructor, and a fourth number does not
improve the funnel. The real argument is that the codes are **per-verb
disjoint**, so a consumer learning "nonzero and not 2 means the environment is
not ready" is correct for both verbs permanently, and the reserved cells allow
growth without renumbering.

**One collision to resolve before implementing.** `main.rs:275` already
returns 3, and it means `"BUG: the branches still differ after syncing"` -- an
internal invariant violation, semantically a panic. Under 7.1's one-binary
decision both reach the user through `config`, where 3 would mean "the tool is
broken" for `sync` and "apt failed" for `deps install`. No consumer can learn
a correct rule from that. The branch collapse (7.6) deletes `run_sync`, which
removes the collision, but that must be stated rather than left to accident.

### 5.5 Prerequisites and the requirement graph

A manifest entry may name prerequisites:

```toml
[[dependency]]
name     = "node"
requires = ["nvm"]
```

`plan` sorts topologically and emits `Blocked { on }` for a step whose
prerequisite has not yet succeeded. Three details the first draft left open:

**The distinction between `Blocked` and `PrerequisiteNotYetInstalled`.** Under
the fixpoint they mean different things: `Blocked` is "not in this wave, a
later wave can unblock it," and `NoInstallReason::PrerequisiteNotYetInstalled`
is what a dry run reports because it cannot know what a later wave would do.
Both were reachable in the first draft with no stated difference.

**A prerequisite may be absent from the selected manifest.** `oh-my-zsh` is
in `deps-linux.conf`; `zsh-autosuggestions` is in shared `deps.conf` and its
own comment says "no ordering between it and zsh-autosuggestions is
guaranteed here." On macOS `oh-my-zsh` is legitimately not in the manifest and
`zsh-autosuggestions` installs fine through brew. So "requires a dependency
that does not exist" and "requires a dependency not selected on this
platform" are different, and `PlanError::UnknownDependency` is wrong for the
second.

**Cycles are a distinct error.** `RequirementCycle { chain }` rather than
`ManifestParse`, because every line parses fine.

The phantom-typed `Step<Ready>` / `Step<Blocked>` alternative is rejected, and
the decisive reason is stronger than the first draft's "there is one
transition": `plan` returns an **ordered heterogeneous collection**. With
phantom types, `Vec<Step<Ready>>` cannot hold a blocked step, so the options
are `Vec<Box<dyn StepLike>>` (erases the parameter at the point the driver
consumes it), two vectors (destroys the topological order that is the whole
point), or `Vec<Either<..>>` (a closed sum written verbosely). The third
collapsing to the closed sum is the proof. Typestate earns its cost when
values are consumed one at a time through a transition chain, not when they
are collected into a monomorphic container.

## 6. The ports

### 6.1 One trait

```rust
pub trait Installer {
    fn describe(&self, action: &InstallAction) -> ActionDescription;
    fn perform(&self, action: &InstallAction) -> StepOutcome;
}

pub struct Installers {
    ordinary:   Box<dyn Installer>,
    privileged: Option<Box<dyn Installer>>,
}
```

One trait, two slots. `plan` marks each step's `PrivilegeRequirement` (3.5),
so the driver's dispatch is an exhaustive match rather than a predicate it
could get wrong, and `Elevation::Unavailable` means no `Root` step is ever
planned.

`ActionDescription` is **structured, not a string**:

```rust
pub struct ActionDescription {
    summary: String,                     // adapter-owned; it knows the command
    privilege: PrivilegeRequirement,
    command_preview: Option<String>,
    changes_trust_root: bool,            // AptSource, and any future equivalent
}
```

The first draft referenced this type three times and never defined it, while
loading a correctness property onto it. A bare `String` cannot support 3.5's
requirement that `--dry-run` disclose privileged steps **before** the first
password prompt, because the driver must aggregate that across steps
beforehand, and aggregating over strings means grepping for `sudo` -- which
resurrects `check-deps.sh:545`'s string sniff inside the new design.
`changes_trust_root` exists so `AptSource` (5.1) cannot be disclosed as an
ordinary package install.

Note what the same-object property actually requires: `describe` must be a
pure function of exactly the inputs `perform` consumes. Today's script gets
this right by a stronger mechanism than two methods -- it substitutes `${SUDO}`
**once** into a single string used for both display and execution, with a
comment saying so. Two methods can diverge; one string cannot. The adapter
must therefore build the command once and have both methods read it.

**Recorded dissent, and the revision partly vindicates it.** The
`oo-architecture` lens argued the trait declaration belongs beside the
adapters, since the core never calls it, so a trait in the core's public
surface misstates the contract. Its evidence is 3.1: this repo's existing
crate reached a pure core with zero traits. The first draft accepted the cost
on the grounds that "nothing moves when the core does eventually need to call
an effect" -- but that clause contradicts 3.3 (which rejected `Probe` because
the core does not call out) and section 6.3 (which treats the core calling an
effect as a design *failure*). The prediction is dropped. The decision to
declare `Installer` in `deps-core` now rests only on the owner's stated
default of traits for enforced dependency injection, which needs no
prediction, and collapsing two traits to one makes the objection much smaller.

### 6.2 `--dry-run` is not an `Installer`

```rust
let described: Vec<ActionDescription> =
    plan.steps.iter().map(|step| installer.describe(&step.action)).collect();
render_dry_run(&described)
```

The first draft made `--dry-run` a `DescribeOnly` implementation whose
`perform` "writes the description and does nothing." But `perform` returns
`StepOutcome`, and **every variant is a false statement** about a run that did
nothing: `Installed` and `AlreadyPresent` make `summarize` report success,
`InstallFailed` exits 3 for a successful dry run, `NotAutomatable` destroys the
distinction `NoInstallReason` exists to preserve, and `Blocked` means
something else. A port whose return type cannot express one of its own
implementations' outcomes is leaking.

`plan` is pure and complete before any effect, so a dry run is `describe` over
the plan with the effectful segment simply not executed. This also makes an
honest limitation visible instead of hidden: under the fixpoint a dry run
**cannot** simulate later waves, because it cannot know what installing `nvm`
does to the observations. It reports the first wave and says so.

### 6.3 The rule for the future

A port gets a new method only when every implementation has a meaningful one.
Batching apt installs belongs in `plan` as a pure grouping transformation, not
as an `install_all` method only one adapter implements.

`plan` may order steps. `plan` may **not** condition an action on another
step's outcome. The driver may skip a step whose prerequisite failed. Those
three sentences make the first draft's blocker unrepresentable rather than
merely documented.

If batching lands, note it **requires** the full re-gather from section 4: one
`apt-get install a b c` yields one exit status for three dependencies, so
per-step outcomes stop being derivable from per-step exit codes. Also, the
interactive prompt becomes per-*step* rather than per-*dependency*, so
`Approval` is per-step.

## 7. Layout and scope

### 7.1 Crates

```
crates/
  Cargo.toml            workspace root, per-crate stamps (see 8.1)
  rust-toolchain.toml   exact pin
  dotfiles-path/        no deps, no IO: CheckRelPath, CommandName, ModuleName,
                        GlobPattern, PackageId, DocsUrl, BoundedText
  config-manifest/      -> dotfiles-path.  The .sync-manifest domain.
                        RelPath, BlobId, CommitId stay here: git concepts.
  deps-core/            -> dotfiles-path.  No edge to config-manifest.
                        manifest, check, plan, action, outcome, ports
  config-cli/           -> all three.  Adapters, drivers, Approval, one exit code
```

**`dotfiles-path` is added in revision, because the first draft's crate tree
had no dependency arrows and both available arrows were wrong.** Section 5.2
said to reuse `RelPath`, which lives in `config-manifest`. So either
`deps-core` depends on `config-manifest` (making the dependency-manifest
domain depend on the git-sync domain, and transitively carrying a 595-line
`git` module it never calls, along with its whole test surface), or
`config-manifest` depends on `deps-core` (inverted: the existing crate
depending on the new one for a type it already owns). Neither is acceptable,
and duplicating the validator in both is the drift shape this repo has
already been burned by.

`RelPath` is not a `config-manifest` concept; its home is an accident of being
written first. But `BlobId` and `CommitId` genuinely are git object
identities and stay. 5.2 explains why `CheckRelPath` is a separate type from
`RelPath` rather than the same one with a variable root.

The first draft's own rule -- "a crate split is a compile-time cost paid on
every build, so YAGNI applies harder to crate boundaries than to types" -- is
what justifies `dotfiles-path` rather than opposing it: there are two
concrete consumers today, not a speculative second.

That rule does, however, argue against `deps-core` as a separate crate, since
its only consumer is `config-cli` and 7.1's stated long-term shape is one
binary. The counter-argument is that a crate boundary makes "the core cannot
reach IO" compiler-enforced rather than a discipline, and 3.1 shows the
existing crate achieved that discipline with modules alone. **Decision: keep
`deps-core` separate**, because the compile-time enforcement of the central
architectural claim is worth one crate boundary, and record that module
discipline plus a lint would be the cheaper alternative if build time becomes
a problem.

Adapters start inside `config-cli` and move only when a second binary needs
them. The long-term shape is one binary with subcommand dispatch rather than
several binaries with several argument parsers and several exit-code
conventions. Two binaries is how status 1 comes to mean eight things again.

### 7.2 What stays shell

| File | Reason |
|---|---|
| `setup.sh` | Entry point on a machine with nothing installed. |
| `config-init`, `config-install-hooks` | Run before a toolchain exists. |
| `.scripts/config/config` | Must locate and exec binaries including when absent, and is therefore the **sole owner of binary resolution**. 34 lines. |
| `config-install` | A 3-line shim to `config deps install`; see 7.3. |
| `platform.sh` | Exports `DOTFILES_PLATFORM` into the calling shell. See below. |
| `zsh-git-widgets.sh` | Registers a ZLE widget and assigns `LBUFFER`. Structurally impossible from another process. |
| `.zshrc` | The shell's own configuration. |
| `docker/Dockerfile.{ubuntu,arch}` entrypoint | Pre-toolchain container; see 7.4 step 3. |

**`platform.sh` fuses two concerns at two lifetimes, and the first draft's
justification for keeping it was factually wrong.** It claimed the file is
"sourced by `.zshrc`, `setup.sh` and tmux config." tmux does **not** source
it: `tmux.conf:137-139` runs its own `uname -s` and hardcodes the
`-mac`/`-linux` convention that `platform_variant()` exists to own in one
place. `setup.sh:337-347` duplicates the mapping a third time, with a comment
admitting it.

The two concerns:

1. **`DOTFILES_PLATFORM` as an exported variable.** Lifetime: the calling
   shell and its children. Only shell can do this, by the same mechanism
   argument as `zsh-git-widgets.sh`. Airtight.
2. **`platform_variant()`, the `base.ext -> base-<platform>.ext`
   convention.** Pure string manipulation, zero IO, already unit-tested. The
   most `deps-core`-shaped function in the repo, and the one duplicated three
   times.

Only concern 1 has a mechanism reason to stay. Concern 2 would be a natural
port -- except that delegating it to a binary puts a ~7 ms process spawn on
`.zshrc` startup, once per interactive shell, and `zshrc-startup-budget.test.sh`
times startup. **Decision: `platform_variant` stays shell as a stated
exception**, and a test asserts that `tmux.conf` and `platform.sh` agree on
the convention. That closes the real defect (silent three-way drift) without
paying the spawn. Compile-time `cfg!(target_os)` is rejected outright: it
would make `DOTFILES_PLATFORM=linux` unrepresentable, and that override is
documented as load-bearing so one machine can exercise both variant files.

### 7.3 The CLI surface

`config deps check` and `config deps install` replace `check-deps.sh` and
`check-deps.sh --fix`. Three contract points the first draft did not address.

**`config install` survives as a shell shim.** It already exists and already
execs `check-deps.sh --fix "$@"`, and it is in the `config help` listing, the
README, and muscle memory. Keeping it as a 3-line shim that execs
`config deps install` preserves all three. It must stay **shell** rather than
becoming a binary subcommand, because `config-usage.test.sh:94` asserts
`config install --help` does not exec, on the stated grounds that "a wrapper
must answer `--help` itself rather than handing it to a program that may not
be installed" -- the incident behind that assertion is `config install-hooks
--help` having once linked the hooks and rewritten `~/.local/bin/config`
before printing help. A shim sourcing `usage.sh` satisfies it for free and
still answers `--help` when the binary is not built.

**`deps` is the first noun-namespace in a `tool verb` surface.** Every
existing subcommand is a verb (`check`, `sync`, `build`, `test`, `push-all`,
`reload`, `init`, `stamp`, `install`). Introducing a namespace level is a fine
choice -- it is what lets `deps check` and `deps install` share a manifest
parser -- but it is a choice and the spec should name it.

**Clap's `name` must render `config deps`.** `config-usage.test.sh:69-75`
requires every subcommand to exit 0 on `--help` **and** for its help text to
contain the literal `config <sub>`, and `:77-82` requires `-h` to be
byte-identical to `--help`. Clap prints `#[command(name = ...)]`, so a binary
named `config-cli` reached through a `config-deps` shim prints the wrong
string and that test fails for the first ported subcommand.

**`--describe` on the dispatcher** replaces `config-help`'s
`sed -n 's/^# help: //p'` over source text. Its contract: **one line to
stdout, exit 0**, because `config-help:40` formats it with
`printf '  %-14s %s\n'`. Note the current failure mode is worse than
"(undocumented)": pointing that `sed` at a binary also emits
`sed: RE error: illegal byte sequence` to stderr, non-fatally because the
pipeline ends in `head -1`. `usage.sh:32-35` extracts the `# usage:` block
the same way and breaks at the same moment; it needs the same treatment.

### 7.4 Order of work

**Step 0. The four verified blockers**, per
`docs/superpowers/plans/2026-09-06-blockers-and-build-infrastructure.md`
Tasks 1 to 4. Three leak-guard fail-opens on a public repo and a test suite
that reports PASS on zero assertions. Independent of this architecture and
higher priority than all of it.

**Step 1. `--describe` on the dispatcher**, plus `usage.sh`. Per 7.3.

**Step 2. Workspace, toolchain pin, per-crate stamps, `dotfiles-path`.** Plan
Tasks 5 to 7, plus the new crate from 7.1. Doing the crate split here is
cheap; doing it after `deps-core` exists is not.

**Step 3. `deps-core` plus the adapters.** The first and largest application
of this architecture. `check-deps.sh` is 313 code lines of which only 61
invoke an external tool, the best logic-to-orchestration ratio in the repo.

This step has a **rename blast radius of 18 consumers**, and the first draft
enumerated none of them. Five pin literal command strings or program names:

| Consumer | What it pins |
|---|---|
| `deps-harness.test.sh:136` | Docker `ENTRYPOINT` contains `check-deps.sh` |
| `deps-harness.test.sh:319-323` | ubuntu and macos CI jobs run `.scripts/deps/check-deps.sh --fix --yes` |
| `shellcheck.test.sh:85` | greps `check-deps.sh --fix --yes`, then asserts shellcheck is in the matched step |
| `scripts-dir-name.test.sh:56,220` | `deps/check-deps.sh` in `EXECUTED_SCRIPTS` plus an execute-bit assertion |
| `README.md:111` | public-facing docs, **and** an input to `deps-docs.test.sh` |

Plus `depcheck-hook.sh:7,28`, the `depcheck` alias, `config-install:16`,
`test-suite.yml:89,102`, `deps-check.yml` (three legs), both Dockerfiles,
`test-local.sh:87`, and `check-deps.test.sh`. For one maintainer this is one
commit; the finding is that 18 items is past what anyone holds in their head
six months later, so the step needs the checklist above.

**Two blockers inside this step.**

*The Docker images cannot run the replacement.* They are pre-toolchain
consumers (3.7): `ENTRYPOINT` is the script, the build context is only
`.scripts/deps`, and the image installs just
`sudo curl git wget ca-certificates`. `deps-harness.test.sh:136` asserts the
literal program name. Three options: keep a thin shell entrypoint that execs
the binary when present; add a build stage to the images; or accept that the
containers test the shell path only until step 3 lands and retire them with
it. **Decision: add a build stage.** The images exist to exercise a real
bootstrap on a clean machine, and a bootstrap that cannot build the tool is
not the bootstrap being shipped. This is more work than the first draft
implied and it belongs in the plan as its own task.

*`deps-docs.test.sh` will pass vacuously.* It harvests every `--flag` from
README lines mentioning `check-deps.sh` or `depcheck`, probes each against
`$CHECK_SCRIPT`, and uses exit 2 as its oracle:
`[ "$?" -ne 2 ] || rejected_flags="..."`. **Verified by execution:** a missing
program exits **127**, `[ 127 -ne 2 ]` is true, so nothing is added to
`rejected_flags` and the assertion passes. Delete the script, leave the
harvest grep matching, and the test reports PASS while probing a program that
does not exist. That is the same failure mode 7.5 cites as this repo's
established bug class. This test must be updated or deleted **in the same
commit**, not deferred to step 6.

**Step 4. The remaining `config-*` subcommands.** Ports must be atomic: the
dispatcher falls through to `git` for any unmatched verb, so a window where
`config-<sub>` is deleted before its replacement is installed silently
reinterprets the verb as a git command.

**Step 5. The tmux scripts.** Convert three of the four sourced ones to
binaries; move `tmux-start.sh`'s `attach` into its alias; port
`tmux-update-window-names.sh` with the `--all` fix folded in.

That fix is: guard on `.git` existing before any git call, then read `.git`,
`commondir` and `HEAD` directly, with `git rev-parse` retained as the
fallback for shapes a file read does not model. **Re-verified during revision:**
21 unique window directories on the live server, 21 of 21 agree with
`git branch --show-current`, zero fallbacks needed. 17 of the 21 are linked
worktrees whose `.git` is a file, and branch names with slashes round-trip
exactly. One honest caveat: `/Users/austin` is itself a bare-repo worktree
with no `.git`, so both implementations skip it identically -- that is
agreement, not evidence the file read handles bare-repo shapes.

**`tmux-split.sh` does not convert cleanly, and the first draft said it did.**
Two reasons. Its `show_usage` ends `return 1`, a value, with an adjacent
comment recording that `exit` was deliberately rejected. More importantly it
has a **sourcing dependency for argument passing**: `tmux-start.sh:34` does
`source ~/.scripts/tmux-split.sh` with **no arguments**, relying on
positional-parameter inheritance so `tmux-split.sh`'s `LAYOUT_TYPE=${1:-}`
reads the *caller's* `$1`. Verified: sourced, the inner script sees the outer
`$1`; exec'd, it sees empty. Converting it silently changes behavior at that
call site unless the argument is passed explicitly. This is a second
structural sourcing dependency beyond `LBUFFER`, and 3.7 missed it.

Also understated: `tmux-setup.sh` has **two** `tmux attach`-shaped calls, and
`tmux-start.sh` has two as well (`new-session -A` and `attach`), not the one
the first draft implied.

**Step 6. The test port.** Last, because it is the safety net for everything
above it -- with the exception of `deps-docs.test.sh`, which step 3 must
handle, and the five literal-string tests, which move with their subject.

### 7.5 Tests

All 43 suites become `cargo test` (43 verified exactly: `tests/*.test.sh`).
`tests/lib.sh` and `tests/run-all.sh` are deleted **last**, and only once the
Rust suite has run green alongside them for a while; converting a reversible
migration into an irreversible one at the moment of the swap buys nothing.

**The first draft's three-way split covered 27 of 43 suites and one of its
three counts was wrong.** 10 + 12 + 5 = 27, leaving **16 suites
unclassified**, and step 6 is sized directly from that split. The corrected
shape:

- **10 suites get better.** They currently `grep` and `sed` over tracked
  files. In Rust they get real parsers: `serde_yaml` for the workflow
  assertions, a Markdown parser for link checking. This tranche has a
  verified bug class behind it, so it pays and it can go early and
  independently, since it tests tracked files and needs no architecture.
- **7 suites keep shell as their subject**, not 5: the six `zshrc-*` suites
  plus `zsh-git-widgets.test.sh`. Rust drives them; the subject stays shell.
  This is honest rather than a gap. Note `zshrc-platform-split.test.sh` needs
  re-derivation rather than a port; see 7.6.
- **26 suites are equivalent** with better fixtures, which is the number the
  first draft undercounted as 12. `assert_cmd` plus `tempfile` replaces the
  harness. The payoff here is thin per suite, so this tranche goes last and
  can be done incrementally.

The bug class the first tranche prevents is real and verified.
`config-docs.test.sh:47-53` extracts subcommand names with
`sed -n 's/^- `config \([a-z-]*\)`.*/\1/p'` and asserts the result names no
removed subcommand. Changing only the bullet marker from `- ` to `* ` -- the
edit a Markdown linter makes -- yields **zero** extracted names, **zero** loop
iterations, and `assert_equals '' ''` **passes**. Reproduced by execution. The
pattern hard-codes the marker, the backticks, and `[a-z-]*`, so a subcommand
with a digit also silently drops out. The forward loop in the same file is
safe by contrast, because it iterates `ls config-*` rather than parsed prose.

**The fixture-ownership claim is withdrawn.** The first draft said `tempfile`
"fixes the fixture-ownership defect where cleanup kills tmux sessions by name
pattern on a shared server." The pattern-kill exists (`lib.sh:77-79`) but is
**PID-scoped**: names are `TEST_NAME-$$-suffix` and the grep is
`^${TEST_NAME}-$$-`, with a comment stating the PID is there precisely so
concurrent runs cannot collide. Cross-kill would require the same test file
*and* the same PID on the same server. No `TEST_NAME` contains a regex
metacharacter. So the defect as described is unreachable. The `tempfile`
half stands on its own merits (`mktemp -d` plus a four-signal trap is more
fragile than a Drop guard), and there is a real but *opposite* gap worth
noting: a session created outside `session_name()` would be caught by neither
cleanup pass, which is under-cleanup rather than over-reach.

**Two currently-green assertions become red, deliberately.** 5.4 changes
behavior that tests pin:

| Test | Pins | After |
|---|---|---|
| `check-deps.test.sh:130` | `'dry-run always exits 0'` | dry-run exits as `deps check` |
| `check-deps.test.sh:141` | `'a manual-only dependency does not fail --fix'` | manual-only counts toward not-ready |

The second is the exact behavior 5.4 calls the defect that lets `config-init`
report success on a machine that is not ready. So these are intended changes,
not regressions -- but the first draft did not say so, and an executor hitting
two red assertions would have to guess. Whoever implements 5.4 updates both
assertions in the same commit, with the new expectation stated.

The pre-push container gains a Rust toolchain in its runtime stage, keeping
the multi-stage build for its dependency-caching layer. `cargo test` becomes
the gate.

**`deps-check.yml` stays a shell-level integration gate, not a `cargo test`.**
Its header states its purpose: it runs on a schedule against real runners "so
a stale install command in `.scripts/deps/check-deps.sh` is caught on a
schedule rather than by a bootstrap that fails." That is the **only** detector
of a dead upstream URL, because a unit test over a hardcoded URL table
asserts a constant equals itself. Six install sites carry URLs, not the three
the first draft listed: the zoxide, oh-my-zsh and rustup `curl | sh` pipes,
the tpm and zsh-autosuggestions clones, and the gh apt keyring fetch. Folding
this workflow into `cargo test` would delete the gate.

### 7.6 The branch collapse

Decided after the first draft, which does not mention it. The `mac` and
`linux` branches collapse to one.

**What it deletes.** `config check` exists **only** to compare two refs; its
help text is "Report drift between the mac and linux branches on shared
paths," it defaults to `origin/mac` against `origin/linux`, and it is
`exec config-manifest check "$@"`. `pre-push` gates the push on it. With one
branch there is no second ref, so the drift subsystem has no work -- and that
subsystem is `config-manifest`, the crate 3.1 cites as this design's
precedent.

**What survives.** The `.sync-manifest` domain still describes which paths are
shared versus per-platform, which is what the symlinked-per-platform-file
solution needs. `RelPath`, `BlobId`, `CommitId`, and the tree/blob machinery
stay. `run_sync` goes, which incidentally removes the exit-3 collision noted
in 5.4.

**What must be re-derived, not ported.** `check-branch-drift.test.sh`, and
contracts 3 and 4 of `zshrc-platform-split.test.sh`, which assert
cross-branch properties ("both variants ship on both branches so neither can
drift unseen"). One branch cannot drift from itself, so the guarantee holds
trivially -- but the *tests* assert a mechanism that no longer exists.

**What the collapse does not reintroduce.** The historical `.zshrc` drift (a
380 ms pyenv init on mac only, an fzf fallback on linux only) came from
per-branch **near-copies**. The shared-file-plus-variants fix already solved
that independently of branch count. Recorded because the test file documents
the incident and a future reader will reasonably ask.

**Compile-time gating is bounded.** Platform gating happens at the
**boundary** only. Configuration values are plain data passed into the core,
never `cfg!`-selected inside it, so the core stays testable for both
platforms from either machine. `cfg!(target_os)` is not used for
`DOTFILES_PLATFORM` at all; see 7.2.

## 8. Consequences for existing decisions

### 8.1 The stamp: one computation owner, one policy owner

`config-stamp` remains the owner of the stamp **computation**, in shell,
because `config-build` calls it to decide what to compile and a Rust owner
would be circular.

**Corrected from the first draft**, which claimed `config-stamp` is "the sole
owner of the stamp rule." It is not; there are two writers, and only one of
them is circular:

- **Computation** (`config-stamp:33-41`): temp index, `read-tree --empty`,
  `add -- "$CRATE"`, `write-tree`, `rev-parse "$root_tree:$CRATE"`.
  Parameterized by `$CRATE`. Consumer: `config-build`. Circularity argument
  **sound**.
- **Comparison policy** (`pre-push:130-144`):
  `git rev-parse --verify --quiet "$ref:crates/config-manifest"` against
  `config-manifest --stamp`, then refuse-or-allow. `pre-push` does not build,
  so circularity **does not apply**. And it **hardcodes** the crate path that
  `config-stamp` takes as a parameter.

The two spellings agree only through an invariant documented in prose in one
file and depended on silently in the other. Under 7.1's per-crate stamps
across four crates the stamp becomes a *set*, `pre-push` must compare a set,
and both writers must agree on the crate list -- a third place the same fact
lives.

Reassignment:

- `config-stamp` gains `--ref`, so `pre-push` stops spelling `rev-parse`, and
  prints one id per crate.
- A pure Rust function owns the verdict:
  `verify(built, pushed) -> StampVerdict = Fresh | Stale { crate, .. } | NotBuilt { crate }`.
  This is `plan`-shaped by 8.3's own criterion.
- `pre-push` is demoted to a consumer that renders the verdict and exits.

`config-build` keeps calling `config-stamp`, so circularity stays broken in
the right place.

**Binary resolution has four owners today and needs one.** `config-build`
decides where the binary is *written* (`CONFIG_BIN_DIR`); the caller's PATH
decides where it is *found*; `config-init:84-93` and `check-deps.sh:80` each
independently repair PATH; `pre-push:133` decides that not-on-PATH is fatal.
`config-build:8-12` documents that a *foreign* `config-manifest` earlier on
PATH was a real observed problem, mitigated by embedding the stamp -- a
mitigation for wrong-binary-resolution, not a fix. 7.1's one-binary decision
routes every subcommand through one PATH lookup, so the dispatcher becomes
the sole owner of resolution (7.2) and owns the not-found message, which
`config-check` currently lacks entirely: with the binary absent it produces
the shell's bare "command not found," never mentioning `config build`.

### 8.2 Task 9 of the committed plan is dropped

That task rewrote `tmux-update-window-names.sh` in shell. Under this
architecture the script becomes Rust, so the shell rewrite would be
discarded. The measured logic and the 21-of-21 equivalence evidence carry
into step 5.

Task 8 **stays**: it fixes `parse_git_dirty` in `.zshrc`, which remains shell
permanently, so that work is never wasted. **Re-verified during revision:**
`parse_git_dirty` is still unfixed at `.zshrc:208-213`, still running a bare
`git status`, and still called from `PS1` at `:224` under `prompt_subst`, so
it costs its full runtime on every prompt draw. The replacement
(`git status --porcelain -uno`) measures **44.0-44.7 ms p50**, reproducing the
plan's 44.6 ms to within 0.1 ms. The "before" figure of 299 ms is
cache-dependent: measured 250 ms on a warm-ish worktree, 386-394 ms on a cold
first touch, 92 ms fully hot. The ~6x improvement holds; the headline number
needs a "cold cache" qualifier. No `core.fsmonitor` or `core.untrackedCache`
is set on those repos, which is why the spread is wide.

### 8.3 The `GitRepo` trait recommendation is withdrawn

An earlier review recommended a `GitRepo` trait so `run_sync`'s policy became
testable. Withdrawn. `run_sync`'s defect is that policy was never extracted
into a pure function, and every one of its decisions consumes a small value
and produces a refusal or a next step, which is `plan`-shaped. A 16-method
trait mirroring git subcommands would violate interface segregation by
construction and would exist only to satisfy mocks.

The fix is the same shape as `deps-core`: gather preconditions into a struct
at the edge, a pure function returning `Result<SyncIntent, SyncRefusal>`, and
`SyncRefusal` as an ADT that also removes the bare-`u8` exit codes.

Note 7.6 supersedes most of this: the branch collapse deletes `run_sync`. The
reasoning is kept because the *shape* is the repo's one architecture (section
4) and `sync`, `deps`, and the tmux scripts are three instances of it.

## 9. Open questions

- **Whether `pyyaml` should be `PythonImport` or a `FileExists` on a
  site-packages path.** The `PythonImport` variant spawns an interpreter. A
  path check would not, but no stable site-packages path across the three
  managers was verified.
- **Pinning the third-party installer URLs.** Six sites carry URLs, and
  **three** are `curl | sh` from three different origins: zoxide (`main`),
  oh-my-zsh (`master`), and rustup. The first draft named two. `CloneSource`'s
  two clones have identical exposure (no `--depth`, no pinned ref) and were
  not mentioned. Pinning changes what a variant *means* --
  `ScriptInstaller::Zoxide` goes from "install by the current upstream
  method" to "install at SHA abc123" -- and moves the failure from "silently
  ran new upstream code" to "install fails loudly when upstream force-pushes."
  That is the right trade, and it has a maintenance cost (someone bumps the
  SHA) that should be budgeted rather than discovered.
- **`setup.sh` duplicates the three-state elevation decision** and 7.2 keeps
  it shell permanently while 3.5 types it in Rust. After the port the same
  policy exists in two languages with no shared definition and no test
  asserting they agree; today they at least agree by copied code with a
  comment pointing at the original. This spec creates the divergence and does
  not resolve it.
- **A `--output json` renderer.** `render` is already pure over `Report`, so a
  second renderer is one more pure function. The argument for it: the design
  spent its budget on the 8-bit exit-code channel, while every consumer reads
  the code as a boolean and reads the *text* for detail -- and this repo has
  been bitten twice by tests parsing prose (`config-docs.test.sh`, and 5.4's
  "consumers discriminating by grepping message text"). A JSON mode makes the
  human-readable text explicitly *not* a contract, which is the only way it
  stays free to change.
- **The `commit-tree` hook bypass.** `config sync` writes commits with
  plumbing, so a synced file reaches the other branch without passing
  pre-commit. 7.6 removes `run_sync` and therefore most of this, but the
  manifest-format transition in 7.4 step 3 depends on the answer while both
  branches still exist.

## 10. Revision log

A six-lens `/expert-review` (type design, effect architecture, data-flow
topology, contract surfaces, complexity, and adversarial fact-checking) ran
against the first draft. It found nine blockers. Every numeric and
file-referencing claim in the draft was independently re-verified.

This section exists because the corrections are more instructive than the
original text, and because a reader who checks a claim and finds it wrong has
no way to tell which other claims to trust.

### 10.1 Claims that were false, and are now corrected

| Draft claim | Reality | Where |
|---|---|---|
| "no check depends on another. All three hold." | Two pairs cross-satisfy, and the *emitted install command* is conditional on another dependency's state | 3.3 |
| single-window prompt path is 17.8 ms, so Rust is ~3 ms slower | **47.7 ms p50**; Rust floor ≈35 ms, so Rust is **faster**. The 17.8 ms was measured with `TMUX_PANE` unset, which exits early | 3.2 |
| `--all` is "roughly 80x the per-prompt path" | **31x**. The 80x came from dividing by the bad 17.8 ms | 3.2 |
| checks run `sh -c` "unconditionally, on every interactive shell startup" | 24-hour throttle; the hook's own header says "at most once every 24h" | 3.6 |
| distribution 15/2/1/1/1/3 across 22 checks | Sums to 23 because it mixes top-level entries with `AnyOf` leaves. Top level is 15/2/1/1/3 with no `GlobExists`, and two unlisted `FileExists` leaves | 5.2 |
| "every install URL is hardcoded at `:309,325,340`" | **Six** sites, not three: +236 (gh apt keyring), +346 (tpm clone), +368 (rustup) | 9 |
| `platform.sh` is "sourced by ... tmux config" | tmux runs its own `uname -s` at `tmux.conf:137` and duplicates the convention | 7.2 |
| `config-stamp` is "the sole owner of the stamp rule" | Two writers; `pre-push:131` computes it independently with a hardcoded crate path | 8.1 |
| `check-deps.sh:173-188` "computes exactly this three-state value" | Two booleans plus a string; the three states are emergent from a 2x2 with one unreachable corner. Strengthens the argument for the enum, but the code described is not there | 3.5 |
| `RelPath::parse` "already rejects escapes" | Rejects empty, absolute, and `..` segments only. Its own tests accept `a/./b`. No rejection of backslashes, NUL, `~`, or drive prefixes | 5.2 |
| `tmux-split.sh` converts cleanly, `return`s pass no value | `return 1` at `:74`, and it depends on **sourced `$1` inheritance** from `tmux-start.sh:34`. Verified: sourced sees the caller's `$1`, exec'd sees empty | 7.4 |
| the three-way test split covers the suites | 10+12+5 = 27 of 43; **16 unclassified**, and "5 shell-subject" is **7** | 7.5 |
| the zsh-autosuggestions check is `test -f "$(brew --prefix ...)/share/..."` | It is one `test` with two `-f` operands joined by `-o`; the quote elides the oh-my-zsh fallback. `grep` for the quoted form returns zero hits. The bug is also **dormant here** (brew present), firing on a brew-less machine | 5.2 |

Two numbers in the draft were confirmed exactly and are worth recording as
such: the 37 IO references (27 in `git.rs`, 10 in `main.rs`, zero in the five
pure modules, zero trait declarations anywhere) and the 43 test suites. The
`--all` figure of 1473 ms reproduced within 2%, and `parse_git_dirty`'s
44.6 ms reproduced within 0.1 ms.

### 10.2 Design defects, with the input that breaks each

| # | Defect | Breaking input | Fix |
|---|---|---|---|
| 1 | Pipeline was single-pass | Install `oh-my-zsh`; `zsh-autosuggestions` becomes installable and a scoped re-gather still calls it missing | Fixpoint loop, pure function inside (4) |
| 2 | `InstallAction` could not represent two real installs | `pyyaml` on brew (a pip command); `gh` on apt (a keyring + APT source) | `Pip`, `AptSource` (5.1) |
| 3 | `None` privileged slot did not make privileged actions unreachable | A plan with a `Root` step and no privileged installer: behavior undefined | Privilege on the plan step; `plan` never emits an unsatisfiable one (3.5) |
| 4 | `DescribeOnly::perform` had no truthful return | Any dry run: every `StepOutcome` variant is a false statement | `--dry-run` is not an `Installer` (6.2) |
| 5 | `AnyOf(Vec<Check>)` admitted the empty vector | `AnyOf(vec![])` reports a dependency permanently missing, no diagnostic | `{ first, rest }` (5.2) |
| 6 | `HttpsUrl` could not parse the shipped manifest | `deps-ci.conf:23` is `http://` | `DocsUrl`, scheme-agnostic (5.1) |
| 7 | `PackageMap::resolve` was not really total | `default: None` + absent manager key: no `NoInstallReason` fits | Mandatory `fallback` (5.3) |
| 8 | `summarize` permitted every "(unused)" cell plus an unreachable variant | `summarize(_, Verb::Check)` returning `AttemptFailed`; `CallerError` with no outcomes | Split by verb (5.4) |
| 9 | `pub enum ExitStatus` with "private constructors" | Not implementable in one workspace | `pub struct ExitStatus(u8)` (5.4) |
| 10 | Exit 2 redefined against a tested convention | `deps-docs.test.sh:115` uses `[ $? -ne 2 ]` as its oracle | 2 stays all caller errors (5.4) |
| 11 | Two traits, identical signatures | No operation expressible through one and not the other | One trait, two slots (3.4, 6.1) |
| 12 | `deps-core` needed `RelPath` from `config-manifest` | Both available dependency edges are wrong | `dotfiles-path` crate (7.1) |
| 13 | `after = gather(&plan.attempted)` was orderable-by-convention | Writing the re-gather before the perform loop compiles and reconciles against a pre-install world | `perform_all` returns `attempted` with a private constructor (4) |
| 14 | Docker images cannot build the replacement | `docker run <image>` after step 3: `ENTRYPOINT` points at a deleted script | Build stage (7.4 step 3) |
| 15 | `deps-docs.test.sh` passes vacuously | Delete `check-deps.sh`: missing program exits 127, `[ 127 -ne 2 ]` is true, assertion passes | Update or delete in the same commit (7.4 step 3) |

### 10.3 Claims withdrawn

**The fixture-ownership defect.** The draft credited `tempfile` with fixing
"cleanup kills tmux sessions by name pattern on a shared server." The
pattern-kill is real but PID-scoped, so the defect is unreachable. See 7.5.

**`PathRoot::Absolute`.** Zero instances in any conf file, and it reopens what
path validation closes. Replaced by `MacApplications`.

**`NodeSpec`.** One dependency, no version pinned anywhere, one possible
value.

**The forward-looking justification for declaring traits in `deps-core`**
("nothing moves when the core does eventually need to call an effect"). It
contradicts 3.3 and 6.3. The decision stands on the owner's stated default
instead.

**`PlanError::UnknownPackageManager`.** Regresses the fresh-macOS case, where
today's script proceeds and lists all 22 missing dependencies.

### 10.4 Decisions taken after the first draft

Four, none of which appear in it: the branch collapse (7.6), boundary-only
compile-time gating (7.6), the core returning structured events (4.1), and
`Services` passed as a parameter to the driver rather than the core (4.1).

Plus one from `docs/research/rust-error-handling-at-the-edge.md`: `thiserror`
for library error types, `anyhow` at the binary edge, and `fn main() ->
ExitCode` rather than `-> Result`, because `fn main() -> Result` exits **1**
for every error regardless of type, which would collapse the exit-code design
in 5.4. `main.rs:63` already returns `ExitCode`. `color-eyre` was evaluated
and rejected: it writes ANSI escapes into a pipe, and grepping 0.6.5 for
`is_terminal`, `supports_color`, and `Stream` returns zero matches, so no TTY
detection exists in the crate.

### 10.5 What the review did not change

Recorded so the revision is not read as a rewrite. The following were
examined adversarially and survived intact: the pure-core-with-effects-at-the-
edges shape; gather-then-decide over injecting `Probe` (3.3); `NoInstallReason`
as a sum rather than a comment; `PackageAvailability` distinguishing
"same name here" from "not installable here"; `FileNonEmpty` distinct from
`FileExists`; the absence of `Check::Shell`; keeping install URLs out of the
manifest; `CommandName::parse` rejecting path separators; the rejection of
phantom-typed `Step` (with a stronger reason supplied in 5.5); the withdrawal
of the `GitRepo` trait (8.3); and section 6.3's rule about `Plan` remaining a
value, which is the sentence the whole design's integrity rests on.
