# `config-cli`: the adapter that runs the pure core

**Date:** 2026-09-07
**Status:** design, not yet planned
**Parent spec:** `docs/superpowers/specs/2026-09-06-pure-core-architecture.md`
(this document is step 3b of that spec's section 7.4, and revises 3.7 and 7.2
where noted)

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

Four pieces, each with one job. The parent spec's section 4 pipeline is
already implemented inside `deps-core`; this document specifies only the edges
that surround it.

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

Measured in `check-deps.sh`: 98 effect sites across 10 command shapes (36
`brew`, 20 `${SUDO}apt-get`, 15 `pacman`, 8 `${SUDO}pacman`, 4 each of
`rustup`, `git clone` and `curl`, 3 `pip`, 1 `python3 -m pip`). The bare
`pacman` count is prose and a `command -v` probe, not unelevated writes:
checked, and every real pacman write carries `${SUDO}`.

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

That is this project's dominant bug class, found eleven times during the
previous plan's execution. So:

**The apt installer sets `DEBIAN_FRONTEND=noninteractive` in the child
environment itself, and a test asserts it does so with the ambient variable
UNSET.** A test that inherits the variable from its own environment proves
nothing, which is the whole lesson of the incident.

The same rule generalises: every non-interactivity flag the engine depends on
(`--noconfirm` for pacman, `-y` for apt) belongs in the argv the engine
builds, and its test must run with the environment stripped rather than
prepared.

## 5. What changes on disk

| Path | Change |
|---|---|
| `crates/config-cli/` | New. `main.rs`, `installer.rs` (the two `Installer` impls), `gather.rs` (run each `Check`), `elevation.rs`. |
| `crates/Cargo.toml` | New workspace member. |
| `tests/docker/Dockerfile` | Builder stage gains the member. `tests/container.test.sh` already asserts cargo's member list matches this file, so omitting it fails a test rather than a Docker build. |
| `.scripts/config/config-deps` | New shell shim. Sources `usage.sh`, execs `config-cli deps "$@"`. |
| `.scripts/config/config-install` | Becomes a 3-line shim to `config deps install` per 7.3. |
| `.scripts/deps/check-deps.sh` | Deleted, last, in one commit with every consumer. |

## 6. The retirement, and why it is one commit

The parent spec measured the rename at "18 consumers"; the real figure,
measured during the previous plan, is **34 files and 97 references**. That
count is in this document because a remembered count is how a half-finished
rename ships.

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
"structurally impossible from another process". The assignment is; the
computation is not. The widget currently spawns **six** processes (`git`,
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
a build stage would not mitigate this project's dominant bug class; it would
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
   the fix and is already implemented and tested; this step must not
   reintroduce a single-pass driver.
2. **Exit 0 on an incomplete machine.** The same run ended "no unresolved
   failures (16 of 18 were already missing)" and exited **0** with three
   dependencies absent. `deps-core`'s `exit_status` is the fix: per-verb
   disjoint codes, exit 2 for every `PlanError`, and `NotSelected` counting
   as not-ready.

## 10. Open questions

**Where does the requirement graph come from?** `Requirements` has no
production populator; every real caller passes `none()`. The fixpoint's
ordering evidence *is* a requirement relationship (`oh-my-zsh` before
`zsh-autosuggestions`), so something must state it. The manifest format is
`name|check_command|docs_url` with no requires column, and `deps.conf:18-20`
says in the file itself that no ordering is guaranteed. Three options, none
chosen here: a fourth manifest column, a separate graph file, or a hardcoded
table in `config-cli` for the two known pairs. **This is the first question
the implementation plan must answer.**

**What owns `PathRoot::OhMyZshCustom`?** Unused across the whole conf corpus.
`check-deps.sh:338-339` is where it would come from, which is this step's
territory.

## 11. Consequences

After this step, `check-deps.sh` is gone and `config deps check` /
`config deps install` are what a machine runs. That makes this the first step
whose completion is visible to a user, and the point at which the previous
three steps stop being invisible infrastructure.

Steps 4, 5 and 6 remain, and none of them is on the critical path for
dependency installation working.
