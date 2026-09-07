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

## 0. Where this sits

Five specs cover the parent spec's remaining steps. Read in this order:

| # | Spec | Blocks on |
|---|---|---|
| 1 | `2026-09-07-deps-core-completion-design.md` | nothing |
| 2 | `2026-09-07-config-cli-adapter-design.md` | 1 |
| 3 | `2026-09-07-config-subcommand-ports-design.md` | 2 |
| 4 | `2026-09-07-tmux-and-zsh-scripts-design.md` | nothing |
| 5 | `2026-09-07-shell-test-port-design.md` | nothing for its first tranche |

Specs 4 and 5 are independent of the 1-2-3 chain and of each other. Spec 5's
first tranche is the only piece with a reproduced defect behind it and no
prerequisite, so it can run first, alongside spec 1.

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

## 3a. Why a new crate rather than adding `deps` to `config-manifest`

A review lens argued the whole `config-cli` crate is redundant, because
`config-manifest` already ships clap dispatch, `--describe`, `--stamp`, an
exit-code convention, and argv subprocess spawning. It also observed that
parent 7.1's "two binaries is how status 1 comes to mean eight things again"
appears to argue against creating a second binary at all. The objection is
serious and this section answers it with measurements rather than preference.

### 3a.1 What the parent spec actually says

Parent 7.1's crate tree is explicit:

```
config-manifest/      -> dotfiles-path.  The .sync-manifest domain.
deps-core/            -> dotfiles-path.  No edge to config-manifest.
config-cli/           -> all three.  Adapters, drivers, Approval, one exit code
```

`config-cli` depends on **all three**, `config-manifest` included. So the
parent's intended end state is not two peer binaries: it is one binary
(`config-cli`) that consumes `config-manifest` as a **library**. The "two
binaries" warning is about the end state, and the tree it sits beside already
describes how to avoid it.

### 3a.2 The topology compiles today, verified

`config-manifest` is **already both a library and a binary**: `src/lib.rs`
exports `doctor`, `git`, `path` and `stamp`. So the arrow parent 7.1 draws
needs no restructuring.

Proved rather than assumed. I built a throwaway fourth member depending on
all three crates and calling into two of them:

```rust
let _pure = deps_core::ConfKind::PlatformSelected;
let _git_domain = config_manifest::stamp::Rendered { .. };
```

Result: compiles clean, binary produced, prints "all three arrows compile".

### 3a.3 The two objections, measured

**"A second binary means two exit-code conventions."** They already agree.
`config-manifest/src/main.rs` returns 2 for usage errors and 1 for failures;
`deps-core`'s `exit_status` returns 2 for every `PlanError`, 0/1 for check,
0/3 for install. Both were written to the same repo-wide convention, so the
divergence the parent warns about is not materializing. What a second binary
does cost is that the convention is maintained by hand in two places rather
than funnelled through one `ExitStatus`.

**"The build stamp will churn."** Weaker than stated. The stamp is
`<crate-tree>:<lock-blob>:<workspace-blob>`, and `config-stamp` shows
distinct first fields per crate against shared second and third fields. So
editing `config-cli` does not restamp `config-manifest`. The real defect is
smaller and is a rename: `config-build:59` sets `CONFIG_MANIFEST_STAMP` for
every member inside its per-member loop, so a second binary crate would read
a correctly-valued variable under a misleading name.

### 3a.4 Decision

**Create `config-cli`, and make `config-manifest` library-only in the same
plan that moves its subcommands.**

Three reasons, in order:

1. **`deps-core` must not gain an edge to `config-manifest`** (parent 7.1
   forbids it, because it would drag a 248-line git module into the
   dependency domain). So the adapter that consumes both cannot live inside
   either. It has to be a third place, and `config-cli` is that place.
2. **The alternative is a rename with a wider blast radius than it looks.**
   Renaming `config-manifest` to `config-cli` touches the installed binary
   name, `config-doctor`'s `exec` target, `pre-push`'s `verify-stamps` call,
   `CONFIG_MANIFEST_STAMP`, `tests/config-manifest-lifecycle.test.sh` (which
   asserts `config-manifest 0.1.0` by name), the test Dockerfile's builder
   stage, and the golden help fixture. Doing that **before** the adapter
   exists means renaming a working binary to make room for code not yet
   written.
3. **The binary half of `config-manifest` has exactly two consumers**:
   `config-doctor`'s `exec` and `pre-push`'s `verify-stamps`. Both move to
   `config-cli` subcommands in step 4, at which point `config-manifest`'s
   `main.rs` is deleted and the crate is library-only. **That is when the
   parent's one-binary end state is reached**, and it is reached without ever
   renaming a binary that something depends on.

**So the two-binary window is real and bounded**, lasting from this step
until step 4 deletes `config-manifest/src/main.rs`. The first draft of this
document did not name that window, which is what made the objection land.
Recorded now with its exit condition: **the window closes when
`config-manifest` has no `main.rs`.**

**Rename `CONFIG_MANIFEST_STAMP` to `CONFIG_CRATE_STAMP` in this step**,
since it is per-member already and the name is the only thing wrong with it.

## 4. The shape

**Corrected after review. The first draft said "the parent spec's section 4
pipeline is already implemented inside `deps-core`; this document specifies
only the edges that surround it." That is false in two ways, and both change
this step's scope.**

**This step depends on `2026-09-07-deps-core-completion-design.md`.**
That spec fixes seven defects a review found inside `deps-core`, two of
which block this one: `plan` cannot emit the `GitClone`, `NvmInstall`,
`Pip` or `AptSource` actions that four of the 22 dependencies need, and
`render`/`Rendered` do not exist. Read it first; this document assumes
both are done.

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
blank, and it discusses its own commands in prose extensively.

**These are 61 occurrences, not 61 lines, and the apparent agreement with the
parent spec is a coincidence of unit.** A first revision of this section
claimed 61 "is exactly the figure the parent spec cites" and treated that as
corroboration. The parent's 61 counts code LINES that invoke an external
tool; measured, that is **34**:

```sh
grep -vE '^[[:space:]]*(#|$)' check-deps.sh \
  | grep -cE 'brew|apt-get|pacman|rustup|git clone|curl|pip'
```

Two different metrics landing on the same number is not independent
confirmation, and presenting it as such was the strongest possible claim
resting on nothing. Either figure is fine for sizing the work. Neither
corroborates the other.

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
| `tests/docker/Dockerfile` | Builder stage gains the member. `tests/container.test.sh` already asserts cargo's member list matches this file, so omitting it fails a test rather than a Docker build. The runtime stage stays Rust-free, and stays that way until a ported Rust suite needs to run in it (shell-test-port spec, section 4a). |
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

**The seam exists for ONE of the six images, not all of them. Corrected
after review.** A first draft of this section said "hand the binary to each
image through the `/seed` mount that already exists", which is false for
two legs and needs new work for two more:

| Image | `/seed` mount | Entrypoint reads the variable |
|---|---|---|
| `bootstrap-curl` | yes | **yes** (`bootstrap-curl-entrypoint.sh:50`) |
| `bootstrap-curl-arch` | yes | yes (same entrypoint) |
| `bootstrap` | yes (`deps-check.yml:151`) | **no** (0 hits for `PREBUILT`) |
| `bootstrap-bare` | yes | **no** (0 hits) |
| `arch` | **no** (`deps-check.yml:89` is a bare `docker run --rm depcheck-arch`) | n/a, `ENTRYPOINT` is the script itself |
| `ubuntu` | **no** (`test-local.sh:87`, local only) | n/a, same |

So section 9's "both must receive the binary through 8.2's seam" is
buildable for the two curl legs and requires per-image work for the rest.
**The `arch` and `ubuntu` images have no shell wrapper at all**: their
`ENTRYPOINT` is `check-deps.sh` directly (`Dockerfile.ubuntu:47`), so there
is nothing to read a variable.

**This strengthens the parent spec's third option, which section 8 omitted.**
Parent 7.4 step 3 offered three, and this document argued against two of
them without noticing the third: "accept that the containers test the shell
path only until step 3 lands and **retire them with it**." Given that the
`arch` and `ubuntu` legs would each need a new wrapper entrypoint plus a
mount to receive a binary, and that the two curl legs already cover apt and
pacman on genuinely bare images, retirement is the option the evidence most
supports. Section 8.5's argument that `Dockerfile.ubuntu` has two live
consumers is an argument against **silent deletion**, not against deliberate
retirement.

**Decision: add a wrapper entrypoint plus `-v /seed` to `arch`, and leave
`ubuntu` alone. Retirement is rejected on evidence.**

Retirement looked attractive because the curl gates already cover both apt
(`debian:bookworm-slim`) and pacman (`archlinux:base`) from bare images. It
is wrong, and `Dockerfile.bootstrap-curl-arch:17-19` says why in the repo's
own words:

> "Dockerfile.arch also targets pacman, but it PREINSTALLS sudo and git...
> or the escalation logic on a machine with no sudo, which is the pair of
> [axes this covers]"

So the two arch images cover **different privilege models**: `arch` has sudo
present, `bootstrap-curl-arch` has none and runs as root. Section 4.1's
`privileged: None` wiring and the three-way `Elevation` mapping are exactly
what those two legs discriminate. Retiring `arch` would delete the only
sudo-present pacman coverage, which is the path most machines actually take.

`ubuntu` needs no work at all. The **CI Ubuntu leg runs on a native
`ubuntu-latest` runner** (`deps-check.yml:46-53`), not in the image, so after
this step it runs `config-cli` natively with no seam required.
`Dockerfile.ubuntu` is built only by `test-local.sh`, where the maintainer
can build the binary first. It stays, per 8.5, minus its compensating
`ENV` line.

**So the per-image work is: one wrapper entrypoint for `arch`, one `-v /seed`
in `deps-check.yml:89`, and `BOOTSTRAP_PREBUILT_BIN` handling added to
`bootstrap-entrypoint.sh` and `bootstrap-bare-entrypoint.sh`** (currently 0
hits each). The two curl entrypoints already have it.

**The one seam that does exist is currently dead code.**
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

**`bootstrap-harness.test.sh:357` pins the optional spelling** with
`grep -qE 'BOOTSTRAP_PREBUILT_BIN:-'`, so making the variable required breaks
a passing gate. Name it in the commit and replace the assertion with its
inverse: assert the variable is required, and that an unset variable **fails**
the run. Without that, whoever hits the failure is likely to "fix" it by
restoring the default.

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

**Two obligations this step inherits, both from work that landed before it.**

First, `Requirements::validated` exists but nothing production calls it. This
step introduces the catalog that holds the requirement table, so this step
owns the test the `deps-core` completion spec's section 5 named and could not
write: **the production table must validate against the union of all four
shipped conf files** (`deps.conf`, `deps-ci.conf`, `deps-linux.conf`,
`deps-mac.conf`). That test catches a typo neither platform's live run would,
because a misspelled prerequisite is absent everywhere and therefore looks
exactly like the legitimate macOS-absent case the design depends on. The
obligation is also written on `validated`'s own doc comment, so it does not
depend on anyone reading this spec.

Second, the catalog's `PackageAvailability::Clone { into }` must be set to the
same `CheckPath` the dependency's check reads, byte for byte. Nothing
enforces it: `PackageCatalog` is a bare map, and neither `plan` nor
`action_for` compares the clone target against the manifest's check. A
catalog that points the clone at a directory while the check reads a file
inside it produces the non-converging fixpoint described in item 1 below.
`deps-core`'s `Clone` doc comment states this requirement; this step is where
it becomes possible to get wrong.

**One gate note.** This step adds the workspace's first binary crate beyond
`config-manifest`, and as of 2026-09-07 no gate anywhere in this repo runs
`cargo clippy` (verified: zero hits outside prose across `.github/`,
`.scripts/` and `tests/`). The seven-task `deps-core` completion held that
invariant by hand across every task, and two implementers tripped it in
tasks whose briefs did not predict it.

The shell-test-port spec's section 4a owns the fix, declares the lint policy
in `crates/Cargo.toml` so it binds every invocation rather than one command
line, and blocks on nothing, so it should land **before** this step. A new
binary crate arriving under an unenforced invariant is how the invariant
stops being true. If 4a has not landed when this step starts, hold the
invariant by hand and say so in the plan's Global Constraints, as the
`deps-core` plan did.

Note also that this crate is the first workspace member whose tests may
genuinely need IO: `config-cli` is the adapter, so its tests drive real
installers. 4a.1's argument that the Rust checks are safe to run on the host
rests on today's members being hermetic (pure crates, plus a
`config-manifest` whose every test builds its own `tempfile::tempdir()`).
**Keep that property**: an adapter test that mutates the real `$HOME` or the
real repository breaks the host-side gate for everyone. Drive IO against a
fixture directory the test owns, the way `config-manifest` already does.

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

## 9a. Core defects, moved out

Five contract defects the review found inside `deps-core` moved to
`2026-09-07-deps-core-completion-design.md` items 3 to 7: `deps install`
exiting 0 on a not-ready machine, `describe` being unable to see the step's
privilege, the unspecified `gather` construction rules, the unspecified
process-failure mapping, and interactive approval having no owner.

They were here because this document absorbed them as the review arrived.
That was wrong: section 2 says core work is out of scope, and two of the
five change public types. They are prerequisites of this step, not part of
it.

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
`oh-my-zsh` lives in `deps-linux.conf:11` and is legitimately absent on
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
   `check-deps.sh:335-342` decides zsh-autosuggestions' install shape per
   manager and `:386` decides node's per nvm presence. The prerequisite
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

### 10.2 `PathRoot::OhMyZshCustom` and the `Requirements` gaps: moved

Both moved to `2026-09-07-deps-core-completion-design.md`, items 6 and 7.
The variant is unconstructible from the manifest grammar and its deletion
carries a latent check-versus-install root divergence; the `Requirements`
boundary absorbs a bad edge silently and conflates two facts in one event.
All four are type-level changes inside `deps-core`.

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
