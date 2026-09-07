# `config-cli`: the adapter that runs the pure core

**Date:** 2026-09-07
**Status:** design, not yet planned
**Parent spec:** `docs/superpowers/specs/2026-09-06-pure-core-architecture.md`.
This document specifies the **adapter half of that spec's Step 3**, and
revises its 3.7 and 7.2 where noted.

**On the label "step 3b":** the parent's 7.4 has Step 0 through Step 6 with
no sub-lettering. "3b" is a subdivision this document introduces, because
Step 3 bundles two separable pieces: the `deps-core` crate, which is done,
and the adapter, which is this. Where other documents in this set write
"step 3b" they mean this one.

## 1. Why this exists

`crates/deps-core` decides what to install. Nothing calls it. It has no
`[[bin]]` and no `main.rs`, and `.scripts/deps/check-deps.sh` (619 lines) is
still what runs when a machine needs dependencies.

So the architecture the parent spec describes is built and unplugged. Every
user-facing path is still shell, and the previous three steps bought a
correct core plus gates that actually gate, which is real and invisible.

This step plugs it in. It is the step that makes the previous three worth
having, and it is the last one that changes what runs on a machine.

## 2. Scope

**In:** the `config-cli` binary, its two subcommands (`config deps check` and
`config deps install`), the `Installer` implementations that perform real
effects, the elevation edge, the observation gather, and the retirement of
`check-deps.sh`.

**Out:** the nine remaining `config-*` subcommands (step 4), the tmux scripts
(step 5), the shell test suites (step 6). Each has its own spec.

## 3. What the parent spec already decided

Recorded here so this document does not re-litigate them, with the reasoning
kept because a reader who checks one and finds it unexplained will reopen it.

| Decision | Where | Reasoning |
|---|---|---|
| One binary, subcommand dispatch | 7.1 | "Two binaries is how status 1 comes to mean eight things again." |
| `deps` is a noun-namespace in an otherwise verb surface | 7.3 | It is what lets `deps check` and `deps install` share a manifest parser. Named as a choice rather than left implicit. |
| `config install` survives as a 3-line shell shim | 7.3 | It is in `config help`, the README, and muscle memory. `config-usage.test.sh:94` asserts a wrapper answers `--help` itself rather than handing it to a program that may not be installed. |
| Clap's `name` must render `config deps` | 7.3 | `config-usage.test.sh:69-75` requires help text to contain the literal `config <sub>`. A binary named `config-cli` behind a `config-deps` shim prints the wrong string. |
| The core takes observations as a value; there is no `Probe` trait | 3.3 | A trait the core does not invoke is not injection of the core, and a `BTreeMap` literal has no behavior that can be wrong. |
| Effects are structured, not shell strings | 5.1 | `InstallAction`'s variants (`Package`, `Brew`, `AptSource`, `Pip`, `GitClone`, `NvmInstall`, `ViaScript`) are already data. |
| Elevation resolves once, at the edge, before the loop | 3.5 | The core records what a step requires; the driver compares. The core never asks whether it can elevate. |

## 4. The shape

**Corrected after review. The first draft said "the parent spec's section 4
pipeline is already implemented inside `deps-core`; this document specifies
only the edges that surround it." That is false in two ways, and both change
this step's scope.**

**4.0a `render` and `Rendered` do not exist.** The diagram below labels
`render(&report, verb)` as `deps-core: pure -> Rendered`. Verified:
`grep 'fn render\|struct Rendered' crates/deps-core/src/` returns nothing.
Both exist only in `config-manifest/src/stamp.rs:18`, a different crate and a
different domain. So this step must ADD them to `deps-core`, following
`stamp.rs`'s shape (`Rendered { stdout, stderr, exit_code }`), which is the
precedent the parent spec's section 4 already cites. Nothing currently
constructs a `Verdict` either, and `Verdict::DryRun` versus `Verdict::Check`
is exactly the per-verb distinction section 9's exit-code regression test
depends on.

**4.0b Four `InstallAction` variants are unreachable from `plan`, and this
step needs all four.** `action_for` (`plan.rs:347-380`) constructs actions
solely from `PackageAvailability`, whose three variants map to `Package`,
`Brew`, `Script`, and `NotAutomatable`. Verified by grep:

| Variant | References in `deps-core` |
|---|---|
| `GitClone` | **0** |
| `NvmInstall` | **0** |
| `Pip` | **0** |
| `AptSource` | 2 (declaration and export only) |

So the planner cannot emit them, which means **this step as first scoped
cannot install `zsh-autosuggestions` (GitClone), `node` (NvmInstall),
`pyyaml` (Pip), or `gh` on apt (AptSource)**: four of the 22 dependencies.
Section 9 names zsh-autosuggestions convergence as this step's acceptance
test, and the action it requires cannot be planned.

That is **core work, not adapter work**: `PackageAvailability` or `action_for`
must grow a way to select those four actions. The first draft's file table
listed only adapter modules, so the largest missing piece was invisible.

It also invalidates the first draft's section 10.2 argument that
`PathRoot::OhMyZshCustom` "becomes live here". The only variant carrying a
`CheckPath` is `GitClone`, which `plan` cannot produce, so the variant could
not have become live in this step regardless. Section 10.2 now deletes it for
a different and better reason.

Four pieces, each with one job. Two of them need core changes first.

```
config deps <verb>
  |
  +-- resolve_elevation()          edge: once, before anything
  +-- gather()                     edge: run each Check, build ObservationMap
  +-- run_to_fixpoint(...)         deps-core: pure loop, calls Installer
  |     |
  |     +-- Installer::describe    edge: render an action, perform nothing
  |     +-- Installer::perform     edge: spawn the real command
  |
  +-- render(&report, verb)        deps-core: pure -> Rendered
  +-- write two streams, one exit code
```

### 4.1 `Installer`, the only port

`deps-core` declares one trait with two methods
(`crates/deps-core/src/driver.rs:91`):

```rust
pub trait Installer {
    fn describe(&self, action: &InstallAction) -> ActionDescription;
    fn perform(&self, action: &InstallAction) -> StepOutcome;
}
```

`config-cli` provides two instances, not two traits: `installers.ordinary`
and `installers.privileged`. The driver's dispatch is an exhaustive match on
`Step::privilege`, so `privileged: None` makes "this machine cannot install
with root" a property of the wiring rather than a string test on command
text.

**`--dry-run` is not an `Installer`.** Per 6.2 it is `describe` over the
plan, which is why `describe` exists as a method at all. An installer whose
`perform` secretly does nothing is the shape that lets a dry run drift from
the real run.

### 4.2 Effects are argv, never a shell string

Measured in `check-deps.sh`, **counting code lines only**: 61 effect sites
across 9 command shapes.

| Shape | Sites |
|---|---|
| `${SUDO}apt-get` | 18 |
| `brew` | 16 |
| `pacman` | 10 |
| `${SUDO}pacman` | 8 |
| `curl` | 3 |
| `rustup` | 2 |
| `git clone` | 2 |
| `python3 -m pip` | 1 |
| `apt-get` | 1 |

**Corrected from this document's own first draft**, which said "98 effect
sites across 10 command shapes" with per-shape figures roughly 2x these. That
grep counted comment lines: the file is 619 lines of which 306 are comment or
blank, and it discusses its own commands in prose extensively. 61 is also
exactly the figure the parent spec cites ("313 code lines of which only 61
invoke an external tool"), which should have been noticed as corroboration
rather than contradicted.

The bare `pacman` sites are `command -v` probes rather than unelevated
writes: checked, and every real pacman write carries `${SUDO}`.

Today each is a **string** built by `printf`, including the privilege
prefix: `printf '${SUDO}pacman -Sy --noconfirm github-cli'`. `config-cli`
builds `Vec<OsString>` instead and spawns without a shell.

Three defects that removes, each present today:

1. **`${SUDO}` is a string prefix.** Elevation is data on the step per 3.5,
   so the driver decides it. Interpolating it into a command string means the
   decision is re-encoded in text and a caller can read a command whose
   privilege does not match its step.
2. **No quoting is possible.** A package name or path containing a space or a
   shell metacharacter is not representable safely in the current form.
   Nothing in the manifest triggers it today, which is exactly why it would
   ship unnoticed.
3. **The dry run and the real run are different code.** With argv they are
   the same vector, rendered by `describe` or spawned by `perform`.

### 4.3 The `${DEBIAN_FRONTEND}` hazard, which this step must not inherit

`check-deps.sh:66-80` records the sharpest incident in the repo's history,
and it is the reason this section exists:

> An unattended bootstrap halted at tzdata's debconf prompt, "Please select
> the geographic area in which you live", waiting for a keypress nobody was
> there to supply. Every image and CI leg passed anyway, **because
> `Dockerfile.ubuntu` sets that variable itself.** "The environment was
> quietly compensating for a gap in the engine, so the engine looked correct
> everywhere it was tested and failed on a real machine."

That is this project's dominant bug class. The parent spec's 10.6
tabulates four confirmed instances, and the previous plan's execution found
more that are recorded in its ledger rather than in the spec. So:

**The apt installer sets `DEBIAN_FRONTEND=noninteractive` in the child
environment itself, and a test asserts it does so with the ambient variable
UNSET.** A test that inherits the variable from its own environment proves
nothing, which is the whole lesson of the incident.

The same rule generalizes: every non-interactivity flag the engine depends on
(`--noconfirm` for pacman, `-y` for apt) belongs in the argv the engine
builds, and its test must run with the environment stripped rather than
prepared.

**The compensating fixture is still in place today, and this step deletes
it.** Verified: `check-deps.sh:89-90` exports the variable in the engine, and
`Dockerfile.ubuntu:46` **still** sets `ENV DEBIAN_FRONTEND=noninteractive`.
So the image that hid the original hang would hide a regression of it right
now. Removing that line belongs here rather than as a standalone fix, because
this step rewrites the apt installer and can land the deletion together with
the test that proves the engine no longer needs it.

The image is not dead code, which I initially believed: `test-local.sh:61`
builds it on every local run through a `for image in ubuntu arch` loop, and
`deps-harness.test.sh:130` pins its `ENTRYPOINT` and `CMD`. It stays; only
the `ENV` line goes.

## 5. What changes on disk

**Corrected after review.** The first draft named four modules and left
**four of `plan()`'s six inputs with no owner**: the package catalog, the
manifest, the selection, and the package manager. `PackageCatalog` is the
worst of those: it is the entire 61-site install-command table, and
`grep PackageCatalog crates/` returns only the type alias, the parameter, and
test helpers. **Nothing in production builds one.** The largest piece of
knowledge being ported had no file, no owner, and no test.

Module boundaries inside the crate, drawn so each answers who creates, owns,
consumes and decides:

```
crates/config-cli/src/
  main.rs           clap dispatch, exit code. Owns argv. Owns nothing else.
  deps/
    mod.rs          assembles Planning, calls run_to_fixpoint, renders
    catalog.rs      const: PackageCatalog + Requirements
    selection.rs    edge, once: conf paths, Selection, PackageManager
    elevation.rs    edge, once
    gather.rs       edge, per wave
    installer.rs    edge, per action
```

The `deps/` boundary is load-bearing: everything inside knows `deps-core`,
nothing outside does. Parent spec 7.1's "two binaries is how status 1 comes
to mean eight things again" is an argument about **exit codes and argument
parsing**, and it is right about those. It is not an argument against
internal module boundaries, and the first draft silently promoted it to one.

The data flow, with all six inputs placed:

```
catalog.rs (pure, const)  --+
selection.rs -> Manifest ---+--> Planning --> plan() --> Step --> perform_all
gather.rs -----------------------------------^                      (effects)
elevation.rs --------------------------------^
```

`catalog.rs` and `selection.rs` supply data; `gather.rs` and `elevation.rs`
supply observations. Section 4's diagram showed only the observation half.

**`catalog.rs` holds both the package catalog and the requirement table**,
because they are the same kind of fact and section 10.5 explains why.

**`selection.rs` is not optional detail: section 10.1's argument depends on
it.** That argument is "the manifest selection already carries the platform
condition, so the graph needs no condition." True only if selection is
correct, and the shell resolves it across `check-deps.sh:37` (`DEPS_CONF`
default), `:19` (platform variant), `:459` (concatenation) and `:101-107`
(`--only` parsing), with `deps-ci.conf:3` noting that file is selected only
by an explicit `DEPS_CONF`. All of that needs an owner and a test.

| Path | Change |
|---|---|
| `crates/config-cli/` | New. Module layout above. |
| `crates/Cargo.toml` | New workspace member. |
| `tests/docker/Dockerfile` | Builder stage gains the member. `tests/container.test.sh` already asserts cargo's member list matches this file, so omitting it fails a test rather than a Docker build. |
| `.scripts/config/config-deps` | New shell shim. Sources `usage.sh`, execs `config-cli deps "$@"`. |
| `.scripts/config/config-install` | Becomes a 3-line shim to `config deps install` per 7.3. |
| `.scripts/deps/check-deps.sh` | Deleted, last, in one commit with every consumer. |

## 6. The retirement, and why it is one commit

The parent spec measured the rename at "18 consumers". Re-measured at the
time of writing: **41 files and 128 references**, excluding `docs/`.

```sh
config grep -rl -I 'check-deps.sh' -- . | grep -v '^docs/' | wc -l
config grep -rn -I 'check-deps.sh' -- . | grep -v '^docs/' | wc -l
```

The previous plan measured 34 and 97, so the figure grew by 7 files between
then and now. **Re-run the two commands above rather than trusting any
number in this document**: a remembered count is how a half-finished rename
ships, and this count has already drifted once inside one session.

Atomicity is a correctness requirement here, not tidiness: the `config`
dispatcher falls through to `git` for any unmatched verb, so a window where
`check-deps.sh` is gone before its replacement is installed silently
reinterprets a command as a git verb.

Two gates already exist for this and both were fixed in the previous plan so
they can actually fail:

- `tests/deps-docs.test.sh` harvests every documented flag and probes it.
  Its oracle read exit 127 ("no such program") as "flag accepted", so it
  passed against a nonexistent program. Fixed in `93c3ae20`.
- `tests/container.test.sh` compares cargo's workspace members against the
  paths the test Dockerfile names, so a new crate cannot be forgotten.

## 7. Two corrections to the parent spec

**7.1 `zsh-git-widgets.sh` is not wholly shell-bound.** Sections 3.7 and 7.2
both list it as permanently shell because assigning `LBUFFER` is
"structurally impossible from another process". The assignment is
impossible. The computation is not. The widget currently spawns **six** processes (`git`,
`rg`, `sed`, `sed`, `fzf`, `cut`) and the branch-listing half measures
**35 ms**. Replacing five of them with one binary leaves the `LBUFFER=`
assignment in the widget and is a latency improvement, not a cost. It is also
not a keystroke path: it blocks on an interactive `fzf` picker, so a human is
reading the screen. Owned by step 5's spec.

**7.2 `tmux-split.sh` does not "convert cleanly".** Section 3.7 says the four
sourced tmux scripts' `return` statements "are early-exit guards passing no
value back". `tmux-split.sh:74` is `return 1`, a value, and section 7.4
contradicts 3.7 on exactly this point. Owned by step 5's spec.

## 8. The Docker images: the parent spec's decision is now wrong

Section 7.4 step 3 decided "**add a build stage**" to the deps images, on the
reasoning that "the images exist to exercise a real bootstrap on a clean
machine, and a bootstrap that cannot build the tool is not the bootstrap
being shipped."

That decision predates `deps-core` and predates the 7.5a bootstrap gates. A
consult plus direct verification says it is now the **worst** of the
available options, for a reason the parent spec could not have known when it
was written.

### 8.1 Why a build stage is wrong: `rustup` is a manifest entry

`deps.conf:35`:

```
rustup|command -v rustup|https://www.rust-lang.org/tools/install
```

`rustup` is a dependency **under test**. Installing a Rust toolchain into the
image so `config-cli` can be compiled there **pre-satisfies a manifest entry
the run exists to exercise**.

That is the `DEBIAN_FRONTEND` shape from section 4.3 exactly: the environment
compensating for the engine, the gate green, the real machine broken. Adding
a build stage would not mitigate this project's dominant bug class. It would
be a new instance of it.

It is worse for the bare legs. `Dockerfile.bootstrap-curl` and
`Dockerfile.bootstrap-curl-arch` have no sudo, no git and no dependencies,
running as root, and that bareness **is** their coverage. A build stage needs
a compiler and the network, so adding one destroys the property being tested.

### 8.2 The decision: inject a prebuilt binary, and require it

Build `config-cli` **once**, in a native `ubuntu-latest` job step with
`actions/cache` over `~/.cargo` and `crates/target`, then hand the binary to
each image through the `/seed` mount that already exists.

**The seam is already there and is currently dead code.**
`bootstrap-curl-entrypoint.sh:45-60` reads `BOOTSTRAP_PREBUILT_BIN`, and its
comment reached this conclusion before the port began:

> "After spec step 3 it is a Rust binary that must exist BEFORE the
> dependency install that places rustup, and this image cannot compile it:
> the build context is only `.scripts/deps`. So the caller may hand one in."

No caller sets it. This step sets it.

**One change to that seam: it becomes required, not optional.** Today
`prebuilt=${BOOTSTRAP_PREBUILT_BIN:-}` defaults to empty and an unset
variable skips the copy, so the run proceeds against whatever is on `PATH`.
That is a fail-open of precisely the kind section 4.3 describes, and the
comment's stated reason for optionality ("the shell path is what ships until
step 3 lands") expires the moment this step lands.

### 8.3 Costs, measured

| Item | Figure |
|---|---|
| `cargo fetch` | 0.95 s |
| Cold `cargo build --release --offline` | **4.22 s**, 17 rlibs |
| Pinned toolchain 1.94.0 on disk | ~1.2 GB |

Compilation is free. **Toolchain acquisition is the entire cost**, and a
build stage pays it once per image across six images, including
`bootstrap-curl-arch` which runs under qemu emulation inside a 30-minute
timeout.

Layer caching does not rescue it: `deps-check.yml` uses neither
`docker/build-push-action` nor a registry cache, so every runner starts with
an empty layer store, and the workflow's primary trigger is a monthly cron
where a cache would be cold regardless.

### 8.4 Rejected: a shell fallback

"Keep a thin shell entrypoint that execs the binary when present and falls
back to shell logic otherwise" is rejected outright. A gate that falls back
can pass having tested **the shell path this step deletes**. That is the
`deps-docs.test.sh` exit-127 defect reproduced deliberately.

### 8.5 `Dockerfile.ubuntu` stays, minus its compensating fixture

I believed nothing built this image. That was wrong, and the correction
matters: `.scripts/deps/test-local.sh:61` builds it on every local run via a
`for image in ubuntu arch` loop, and `tests/deps-harness.test.sh:130` pins
its `ENTRYPOINT` and `CMD`. Two live consumers. My `config grep` missed them
because the filename is *constructed* in the loop rather than written out.

So it stays. But **delete `ENV DEBIAN_FRONTEND=noninteractive` from
`Dockerfile.ubuntu:46`.** `check-deps.sh:89-90` now exports that variable in
the engine, and the image line is the exact fixture that hid the tzdata bug.
Verified: both are present today, so the local harness still cannot catch a
regression of it. Removing the image line restores that ability.

`Dockerfile.arch` is built once, not twice: `deps-check.yml:84` is the only
build, and line 296 is prose inside another job.

## 9. Testing

The parent spec's 7.5a bootstrap gates are the acceptance test for this step:
the rewrite is not done until a bare image fully initializes through the
documented one-liner. Both legs must stay green with `config-cli` in place of
`check-deps.sh`, and both must receive the binary through 8.2's seam.

Two defects the 7.5a gates found on real hardware are this step's
regression tests, because both are now the core's responsibility rather than
the script's:

1. **One `--fix` pass does not converge.** `zsh-autosuggestions` clones into
   `~/.oh-my-zsh/custom/plugins/`, and `check-deps.sh:339-341` emits that
   clone only if the directory already exists. Observed in a container run:
   "no automated install for zsh-autosuggestions" at line 312, "installed
   oh-my-zsh" at line 1237, 925 lines apart. `deps-core`'s fixpoint loop is
   the fix and is already implemented and tested. This step must not
   reintroduce a single-pass driver.
2. **Exit 0 on an incomplete machine.** The same run ended "no unresolved
   failures (16 of 18 were already missing)" and exited **0** with three
   dependencies absent. `deps-core`'s `exit_status` is the fix: per-verb
   disjoint codes, exit 2 for every `PlanError`, and `NotSelected` counting
   as not-ready.

## 9a. Two contract defects the review found in `deps-core` itself

Both must be fixed in the core before this step's acceptance tests can mean
what section 9 claims.

### 9a.1 `deps install` still exits 0 on a not-ready machine

Section 9.2 names "exit 0 on an incomplete machine" as this step's
regression test and says `exit_status` is the fix. **It is not, for the
install verb.** Verified:

- `summarize_install` (`outcome.rs:143-152`) returns `AllSucceeded` unless an
  outcome is `InstallFailed` or `InstalledButCheckStillFails`. A
  `NotAutomatable` outcome, which is what a manual-only dependency produces,
  yields `AllSucceeded`.
- `Verdict::Install(InstallStatus)` (`outcome.rs:164`) carries no
  `CheckStatus`, so no cell of the exit table can consult readiness.
- `exit_status` maps `Install(AllSucceeded)` to **0**.

So `config deps install` on a machine with `nvm` and `node` absent exits 0.
That is byte-for-byte the reported incident: "no unresolved failures (16 of
18 were already missing)" and exit 0. The doc comment at `outcome.rs:140-142`
says the not-ready signal "belongs to `summarize_check`, which the driver
also calls", and the driver does call it, but nothing carries the result into
the install verdict.

The consumer harm is concrete: `config-init:142` propagates any nonzero from
`config-install`, so `config init` on a bare machine would report a
successful bootstrap while dependencies are missing.

**Decision: `Verdict::Install(InstallStatus, CheckStatus)`, with a distinct
code for "installs succeeded, machine still not ready".** The alternative
(declare `install` an attempt verb whose 0 means "I broke nothing", and make
`config init` run `deps check` afterward) is defensible but it is a contract
change with a named consumer edit, and leaving it unstated is how the
tested-for regression ships.

### 9a.2 `describe` cannot see the step's privilege

`Installer::describe(&self, action: &InstallAction)` receives only the
action. `describe` over a plan (`driver.rs:187-193`) discards `step.privilege`
at the call site. But privilege is decided by `action_for` from
`(availability, manager, elevation)` (`plan.rs:347-379`), none of which an
`InstallAction::Package { id }` carries, so an installer **cannot** re-derive
it. The crate's own reference implementation hardcodes
`privilege: PrivilegeRequirement::None` (`driver.rs:448`), which is the
divergence shipping in miniature.

This matters because parent spec 3.5 and 6.1 load a correctness property onto
`ActionDescription::privilege`: a dry run must disclose privileged steps
before the first password prompt. A dry run that renders every step as
unprivileged defeats it, and it is the same "a caller can read a command
whose privilege does not match its step" defect section 4.2 claims argv
removes.

**Decision: `fn describe(&self, step: &Step) -> ActionDescription`.** Passing
the step instead of the action makes the same-object property structural
rather than a convention two methods uphold by hand, which is what
`driver.rs:85-90` already says the design requires.

### 9a.3 The `gather` edge needs its own section, and does not have one

The whole specification of the riskiest edge in this design is one diagram
line and one parenthetical in the file table. Grepping all four specs for
`Unresolvable`, `SpawnError` or `ExecFailure` returns **zero hits**, while
parent spec 5.2 spent several paragraphs establishing that `Unresolvable`
exists to fix a live silent-false bug.

What an implementer would have to invent, each with a wrong answer that is
silent:

- **Root resolution failure.** `PathRoot::BrewPrefix` spawns `brew --prefix`.
  Failure is `Unresolvable`, not `Absent`.
- **Check-spawn failure.** `Check::PythonImport` spawns an interpreter.
  "The interpreter is missing" and "the module is missing" are different
  facts with different remedies, and today's shell collapses both
  (`check-deps.sh:523`: `if sh -c "$check" >/dev/null 2>&1`). This port is
  positioned as fixing that collapse. Decide explicitly whether `Absent` is
  right for a failed probe, or whether `Observation` needs a fourth variant.
- **Which checks get recorded.** `ObservationMap::observe`
  (`check.rs:141-145`) returns `Absent` for any key it does not hold. So
  `gather` must enumerate every leaf of every `AnyOf`, recursively. A gather
  that records only top-level checks silently reports the three `AnyOf`
  dependencies absent.
- **`Unresolvable` is not blocking.** `plan.rs:300-303` records
  `Event::CheckUnanswerable` and falls through to plan an install. That may
  be right. It is a policy decision nobody wrote down.
- **Roots must be re-resolved at the instant of use.** Parent spec 5.2 says
  so, because `oh-my-zsh`'s install creates the directory
  `OhMyZshCustom` names, so a root resolved at gather time is stale by
  perform time.

### 9a.4 Process failure to `ExecFailure` is unspecified

Section 4.2 specifies how commands go out in detail and says nothing about
how failure comes back. `BoundedText` exists in `dotfiles-path` for exactly
this call site and is never mentioned. Specifics that need a written answer:
`Output::status.code()` is `Option<i32>` and is `None` for a signal kill;
`Command::output()` yields `Vec<u8>` with no UTF-8 guarantee while
`BoundedText::truncating` takes `&str`; and `ExecFailure::AuthenticationRefused`
carries the claim "every remaining privileged step will also fail" with
nothing in `perform_all` short-circuiting on it.

Also: `Installer::perform` should return only `Installed`, `InstallFailed` or
`NotAutomatable`. `AlreadyPresent` and `InstalledButCheckStillFails` are
`reconcile`'s to produce (`reconcile.rs:74-88`), so an installer returning
one would bypass the post-loop re-check that catches a failed install.

### 9a.5 Interactive approval has no owner and no representation

Parent spec 7.1 lists `config-cli` as owning "Adapters, drivers,
**Approval**, one exit code" and 6.3 says approval is per-step. None of the
four specs mentions approval, `--yes`, or stdin. `check-deps.sh:572-581`
prompts per dependency and reads stdin, and `deps-check.yml:54` and `:66`
both pin `--fix --yes`, so **`config deps install` needs `--yes` or CI
hangs**, and section 6's retirement cannot be atomic without it.

There is also no `StepOutcome` for "the user declined". `Declined` is not
`NotAutomatable` (an automated install exists), not `Blocked` (nothing
unblocks it), and not `InstallFailed` (nothing failed). Per
`outcome.rs:131-135` it must count toward `NotReady`, and per `:143-150` it
must not count as `AttemptFailed`. Either add the variant, or decide approval
lives entirely at the edge so a declined step never reaches `perform_all`,
which changes the `ready` filter and therefore the termination argument.

## 10. Decisions taken after the first draft

### 10.1 The requirement graph: a hardcoded table, and the platform condition dissolves

The first draft left this open. A consult plus direct reading of
`plan.rs` settles it, and the interesting part is that **the question was
posed wrongly**.

I described the edge as platform-conditional: `node` always needs `nvm`, but
`zsh-autosuggestions` needs `oh-my-zsh` only on Linux, because on macOS it
installs through brew. That framing implied a graph format expressive enough
to hold a platform condition, which is what made options 2 and 3 look
necessary.

**The edge is unconditional. The manifest selection already carries the
platform condition.** Verified in `plan.rs:395-401`:
`first_unsatisfied_prerequisite` treats a prerequisite that is absent from
the manifest as **not an error**, and its comment names this exact case:
`oh-my-zsh` lives in `deps-linux.conf:12` and is legitimately absent on
macOS. So on macOS the edge is not blocking, with no condition
anywhere.

Encoding the platform a second time in the graph would be a redundant
condition that can disagree with the manifest.

So the production table is two unconditional, platform-blind pairs:

```
node                 -> [nvm]
zsh-autosuggestions  -> [oh-my-zsh]
```

**Decision: `config-cli` holds that table in Rust.**

Three reasons, in order:

1. **The graph is not manifest data.** It is knowledge about install
   mechanics that already lives in code, beside the install-command table.
   `check-deps.sh:346-352` decides zsh-autosuggestions' install shape per
   manager and `:383-386` decides node's per nvm presence. The prerequisite
   is the same fact those branches already encode. A separate file splits one
   fact across two artifacts that can drift, with nothing to catch it, which
   is the failure `deps-ci.conf:8-12` documents about a duplicated list.
2. **`Requirements` is the contract, and it is already right.** It is
   opaque and constructor-only, so `plan` cannot tell a hardcoded table from
   a parsed file. The graph's source is an implementation detail of the
   caller. The boundary that survives a rewrite is `Requirements`, not the
   format feeding it, so pick the cheapest producer.
3. **"A new edge needs a code change" is a feature at this size.** Every
   existing edge already required a code change to the install-command table.
   Revisit only past roughly ten edges, or if a manifest ever comes from
   outside this repo.

**Inference from check strings is rejected on evidence.** `tpm`'s check is
`[ -d "$HOME/.tmux/plugins/tpm" ]`, and the only thing creating that path is
`tpm`'s own install at `check-deps.sh:345`. So path matching finds a self
edge or nothing, while a human sees that `tpm` requires `tmux`. It also fails
inversely: `alacritty`'s check names `/Applications/Alacritty.app`, which no
entry installs. And it reverses silently, since fixing a typo in a check
string would delete an edge with no error. That makes check strings
load-bearing for ordering, on a field whose documented contract
(`deps.conf:2`) is "a shell command".

**One correction to my own framing.** I called the requirement graph "dead
weight" because nothing populates it. That was wrong: `PrerequisiteNotSelected`
and `RequirementCycle` are contract shape, and `topological_order` is what
removes the 925-line ordering defect. Only the *producer* was missing.

### 10.2 `PathRoot::OhMyZshCustom` is deleted, and the reason is a real bug

**This section's first draft was wrong on the evidence.** It said the variant
is "unconstructed pending its consumer" and contrasted it with the two
variants deleted this session as being unlike them. A review found the
opposite, and I confirmed it by reading the parser and the manifest.

`check.rs:365` registers the prefix `${ZSH_CUSTOM:-$HOME/.oh-my-zsh/custom}/`.
The only manifest entry that could carry it, `deps.conf:26`, writes
`$HOME/.oh-my-zsh/custom/...` instead. **No conf string carries that prefix,
so the variant is unconstructible from the grammar**, exactly like the two
already deleted.

**The sharper finding is a latent divergence in the shell, which the port
would make reachable.** The check and the install use different roots:

```
check   (deps.conf:26)          $HOME/.oh-my-zsh/custom/plugins/...
install (check-deps.sh:339)     ${ZSH_CUSTOM:-$HOME/.oh-my-zsh/custom}/plugins/...
```

Verified by execution with `ZSH_CUSTOM=/opt/omz-custom`:

```
install target: /opt/omz-custom/plugins/zsh-autosuggestions
check subject:  /Users/austin/.oh-my-zsh/custom/plugins/zsh-autosuggestions
```

They agree only when `ZSH_CUSTOM` is unset or set to its default, which is
why the defect is latent on these machines. Ported as-is it becomes a
non-converging fixpoint: the clone succeeds, the re-gather still reports the
dependency absent, and `Attempted` (`driver.rs:30`) has already retired the
step, so the run exits nonzero having installed the thing correctly. The
fixpoint does not rescue this, because `Attempted` records "considered and
resolved" rather than "converged".

**Decision: delete `PathRoot::OhMyZshCustom`, and give `GitClone` a
`Home`-rooted `CheckPath` that matches `deps.conf:26` byte for byte.**

That gives the property that matters: **the install target and the check
subject are the same value**, so convergence is structural rather than
hoped for. `ZSH_CUSTOM` support is not a requirement anywhere in the corpus;
it is an accident of the `${VAR:-default}` idiom. If it is wanted later, it
needs the two-case shape (`ZshCustomOverride` resolving to
`Observation::Unresolvable` when the variable is unset, which forces the
default case to be expressed `Home`-relative) plus a test, because
`PathRoot` is `Copy` and payload-free, so a single variant cannot carry a
resolved value and every consumer would re-read the environment. That is a
hidden effect in a type whose stated purpose (`check.rs:11-15`) is to delete
all shell expansion from the check field.

### 10.2a Two gaps at the `Requirements` boundary

Both found in review, both real, both cheap.

**A bad requirement edge is silently absorbed.** `PlanError::UnknownDependency`
is constructed only from `Selection` (`plan.rs:289`, `:442`); nothing walks
`Requirements`. So a typo in the hardcoded table,
`(zsh-autosuggestions, [oh-my-zhs])`, is a well-typed value that plans
successfully, emits `PrerequisiteNotSelected { on: "oh-my-zhs" }`, orders
nothing, and exits 0. **The typo is indistinguishable from a correct macOS
run.** Section 10.1's "a new edge needs a code change is a feature" assumed
that code change gets reviewed. It gives the type system nothing to check.

Fix: a validating constructor, `Requirements::validated(pairs, known)`,
where `known` is the union of every shipped conf file. `from_pairs` stays for
tests. The test that matters asserts the production table validates against
that union, because it runs on both platforms and catches a typo that
neither platform's live run would.

**`PrerequisiteNotSelected` conflates two different facts.**
`plan.rs:397` is a disjunction:

```rust
if manifest.get(prerequisite).is_none() || !selection.contains(prerequisite) {
```

The first disjunct is "this platform does not have this dependency", which is
the legitimate macOS case section 10.1 relies on. The second is "this
platform has it, but the run narrowed past it", which is **not** a platform
difference. On Linux, `config deps install --only zsh-autosuggestions` takes
that branch, does not block, and plans a real `GitClone` into a
`~/.oh-my-zsh/custom` that does not exist. That is the 925-line ordering
defect reachable through a flag rather than a wave, and
`check-deps.sh:338-341`'s `if [ -d ... ]` guard is what suppresses it today.
This step deletes that guard.

Fix: split the event into `PrerequisiteNotInManifest` and
`PrerequisiteDeselected`, and treat the second as blocking. The sum forces
the two-case decision at every match site instead of letting a `||` decide
it silently.

### 10.3 Documentation debt this step creates

`deps.conf:17-20` says "no ordering between it and zsh-autosuggestions is
guaranteed here." Once the Rust planner holds the table, ordering **is**
guaranteed, and that comment understates the guarantee. It must be updated in
the same commit that adds the table.

## 11. Consequences

After this step, `check-deps.sh` is gone and `config deps check` /
`config deps install` are what a machine runs. That makes this the first step
whose completion is visible to a user, and the point at which the previous
three steps stop being invisible infrastructure.

Steps 4, 5 and 6 remain, and none of them is on the critical path for
dependency installation working.
