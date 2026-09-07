# The remaining `config-*` subcommands

**Date:** 2026-09-07
**Status:** design, not yet planned
**Parent spec:** `docs/superpowers/specs/2026-09-06-pure-core-architecture.md`
(step 4 of section 7.4)
**Depends on:** `2026-09-07-config-cli-adapter-design.md`. This step adds
subcommands to the binary that step creates, so it cannot start first.

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

## 1. What is actually in scope

Nine `config-*` scripts exist, 742 lines total. **Four of them are already
decided as permanent shell by the parent spec**, which leaves five, and one
of those is a shim.

| Script | Lines | Disposition | Authority |
|---|---|---|---|
| `config-init` | 212 | **Stays shell.** Runs before a toolchain exists. | 7.2 |
| `config-stamp` | 168 | **Stays shell.** `config-build` calls it to decide what to compile; a Rust owner would be circular. | 8.1 |
| `config-install-hooks` | 61 | **Stays shell.** Runs before a toolchain exists. | 7.2 |
| `config-install` | 16 | **Stays shell.** A 3-line shim to `config deps install`. | 7.3 |
| `config-test` | 91 | Ports. | |
| `config-build` | 78 | Ports, with a caveat; see 3.2. | |
| `config-help` | 60 | Ports, with a caveat; see 3.1. | |
| `config-reload` | 30 | Ports. | |
| `config-doctor` | 26 | Already a shim to `config-manifest doctor`. Delete it; see 3.3. | |

**So this step is 259 lines across four scripts, not 742 across nine** before
section 3.2's decision, and **113 lines across three scripts plus a 26-line
deletion** after it, because 3.2 keeps `config-build` shell. The second
figure is the one to size the work by. The first is what the parent spec's
"the remaining `config-*` subcommands" phrasing implies, which is 6.5x the
real surface once the parent's own decisions are applied.

## 2. The one hard rule

Parent spec 7.4 step 4, and it is a correctness requirement:

> "Ports must be atomic: the dispatcher falls through to `git` for any
> unmatched verb, so a window where `config-<sub>` is deleted before its
> replacement is installed silently reinterprets the verb as a git command."

`config reload` with no `config-reload` present becomes `git reload`, which
is not a git verb and errors confusingly. Worse shapes exist: a subcommand
name that *is* a git verb would silently do something else.

Each port is therefore one commit that removes the script and installs the
subcommand together.

## 3. The three caveats

### 3.1 `config-help` must keep asking, not embed

`config-help` builds its listing by running `config-<sub> --describe` on each
sibling. That was the point of step 1, and it is why a binary subcommand can
be listed at all.

If `config-help` becomes a binary subcommand, it must **still enumerate
`config-*` siblings and ask each one**, not embed a compiled-in list. An
embedded list is a second copy of the same facts, and per the file's own
comment "the one that nobody edits is the one that goes stale."

The binding constraint: `tests/config-usage.test.sh` pins `config help`
output byte-for-byte against
`tests/fixtures/config-help-before-describe.txt`, captured before any
`--describe` work existed. That fixture is the acceptance test for this port.

### 3.2 `config-build` bootstraps the thing it would become

`config-build` compiles the workspace and installs each stamped binary. If it
becomes a subcommand of the binary it builds, then a machine with no binary
cannot build one.

Three ways out, decided below:

- **Keep `config-build` shell**, on the same pre-toolchain reasoning that
  keeps `config-init`. Simplest, and consistent with 7.2's existing entries.
- **Port it and keep a shell fallback path** for the no-binary case. This
  reintroduces two code paths for one job, which section 8.4 of the adapter
  spec rejects for the deps images on the grounds that a fallback can pass
  having tested the path being deleted.

**Decision: keep `config-build` shell** and add it to 7.2's table with this
reasoning. The circularity is the same shape as `config-stamp`'s, which 8.1
already resolved the same way, and a fallback path would let the gate pass
having exercised the path being deleted.

**Confirmed, not merely recommended.** The circularity is the same one
parent 8.1 already resolved for `config-stamp`, and the adapter spec's 8.4
rejects fallback paths on the grounds that a gate can pass having exercised
the path being deleted. Both arguments apply here unchanged, so this is a
decision rather than a preference.

**Recorded as a future direction, not part of this step:** the third option
(port it and have a fresh machine get its first binary from a release
artifact rather than a local compile) is the more interesting long-term
answer, and it interacts with the adapter spec's section 8.2, which already
injects a prebuilt binary into the bootstrap images for exactly this reason.
If prebuilt-binary bootstrapping is adopted there, `config-build` stops being
load-bearing for a fresh machine and this decision is worth revisiting.
Tracked in `TODO-AGENTS.md`.

### 3.2a This step closes the two-binary window

The adapter spec's 3a records that steps 3b and 4 run with both
`config-manifest` and `config-cli` on PATH, and names the exit condition:
**the window closes when `config-manifest` has no `main.rs`.** This step is
where that happens.

The binary half of `config-manifest` has exactly two consumers, verified:

| Consumer | Call |
|---|---|
| `.scripts/config/config-doctor:26` | `exec config-manifest doctor "$@"` |
| `tests/pre-push:145` | `config-manifest verify-stamps` |

Both become `config-cli` subcommands here. Then `config-manifest/src/main.rs`
is **deleted** and the crate is library-only, which is the end state parent
7.1's crate tree describes.

Two consequences to land in the same commit:

- **`config-stamp` and `config-build` must stop expecting a
  `config-manifest` binary.** `config-build:64` skips installation for a
  member with no `src/main.rs`, so deleting the file is sufficient there,
  and it already prints "is a library, nothing to install".
- **`tests/config-manifest-lifecycle.test.sh:67` asserts
  `config-manifest 0.1.0` from `config-manifest --version`.** That assertion
  is about a binary that will not exist. It moves to `config-cli --version`
  or it goes.

### 3.3 `config-doctor` is already a shim and should go

`config-doctor` is 26 lines whose body is `exec config-manifest doctor "$@"`.
Once `config-manifest`'s subcommands move into `config-cli`, the shim's
target moves and the shim itself has no remaining purpose: the dispatcher can
reach `config cli doctor` directly.

Deleting it removes the string duplication that
`tests/config-usage.test.sh` currently asserts away: the shim's `# help:`
line and `config-manifest --describe` are two copies of one description, and
the assertion tying them together exists only because both exist. That
assertion goes with the shim, which the test's own comment already
anticipates.

## 4. Order

1. `config-reload` (30 lines). Smallest, no caveat. Proves the atomic
   swap pattern on the lowest-risk subject.
2. `config-test` (91 lines). Self-contained.
3. `config-help` (60 lines) per 3.1, with the golden fixture as its gate.
4. `config-doctor` deleted per 3.3, in the same commit that moves
   `config-manifest`'s subcommands into `config-cli`.

`config-build` is removed from scope per 3.2's recommendation, pending the
plan's confirmation.

## 5. What this step must not break

- **`config help` byte-for-byte.** The golden fixture is the whole gate.
- **`--help` and `-h` byte-identical**, and help text containing the literal
  `config <sub>`. `config-usage.test.sh:69-82`. Clap prints
  `#[command(name = ...)]`, so each subcommand's name must render
  `config <sub>` and not `config-cli <sub>`.
- **`--help` must not execute anything.** `config-usage.test.sh:94`. The
  incident: `config install-hooks --help` once linked the hooks and rewrote
  `~/.local/bin/config` before printing help.
- **`tests/scripts-dir-name.test.sh`** counts scripts in `.scripts/` exactly.
  Every deletion changes that count and must update it in the same commit.
