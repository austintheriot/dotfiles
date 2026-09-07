# Converting the shell test suites to Rust

**A note on vocabulary.** This document says **convert** rather than
**port** for moving a suite to Rust, because `port` is load-bearing
architectural vocabulary in the parent spec: an effect boundary, as in
"`Installer` is the only port". Two of this set's filenames use the migration
sense, which is why the distinction is worth stating once. The tmux spec's
own table column already says "Converts?".

**Date:** 2026-09-07
**Status:** design, not yet planned
**Parent spec:** `docs/superpowers/specs/2026-09-06-pure-core-architecture.md`
(step 6 of section 7.4, and section 7.5)
**Depends on:** nothing for tranche A. Tranches B and C should follow the
subjects they test.

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

**Section 4a's clippy leg is smaller than a tranche and blocks on nothing at
all**, not even Tranche A. It is a few lines in `run-all.sh` and
`test-suite.yml`, it closes an invariant that no gate enforces today, and it
is a no-op in the current Rust-free container, so it can land before spec 1's
successor rather than waiting for 34 suite conversions. Sequenced in 4a.

## 1. Why this is last, and why one tranche is not

Parent spec 7.4: "Step 6. The test port. **Last**, because it is the safety
net for everything above it."

That is right for the suites that test the code being ported: converting the
net before the thing it catches is backwards. It is **not** right for the
suites that test tracked *files* rather than ported code. Those depend on
nothing, and one of them has a verified defect behind it today.

So this document splits step 6 by dependency rather than treating it as one
block.

## 2. Current state, measured

41 shell suites (43 in the parent spec, minus two the branch collapse
deleted). 130 Rust tests exist, all covering new `deps-core` and
`dotfiles-path` code. **Zero suites have been converted.**

`tests/lib.sh` and `tests/run-all.sh` are deleted **last**, and only once the
Rust suite has run green alongside them for a while. Converting a reversible
migration into an irreversible one at the moment of the swap buys nothing.

## 3. The three tranches

### Tranche A: real parsers instead of regex (goes first, independently)

These suites parse **structured formats** with `sed`, `grep -oE` and `awk`.
In Rust they get `serde_yaml`, a TOML parser, and a Markdown parser. Measured
by which suites read which format:

| Format | Suites that parse it |
|---|---|
| YAML (`.github/workflows/`) | `bootstrap-harness`, `check-deps`, `container`, `deps-harness`, `readme-badges`, `scripts-dir-name`, `shellcheck`, `workflow-action-versions`, `workflow-labels` (**9**) |
| Markdown | `config-docs`, `doc-links`, `deps-docs`, `readme-badges`, `setup`, `container`, `deps-harness`, `leak-check`, `pre-push-multi-ref`, `scripts-dir-name`, `check-deps`, `config-usage` (**12**) |
| TOML | `alacritty-platform-split`, `config-manifest-lifecycle`, `container`, `doc-links`, `platform`, `pre-push-multi-ref`, `run-all-filter` |

**The three tranches OVERLAP. They are not a partition, and an earlier
draft implied they were.** The table above names 17 distinct suites, and
17 + 7 + 26 = 50 against a total of 41. The excess is real overlap rather
than an error in the totals: a suite can both parse a structured format
(tranche A) and need `assert_cmd` fixtures (tranche C), and
`container.test.sh` parses all three formats by itself.

So read the tranches as **work streams, not buckets**. The partition that
does sum is the one in section 6: 7 suites keep shell as their subject, 34
become Rust. Tranche A names where a real parser is the payoff. Tranche C
names where the harness is the only change. A suite in both gets its parser
work in A and its fixture work in C.

**The bug class this closes is verified, not theoretical.** Parent spec 7.5's
example, reproduced by execution: `config-docs.test.sh:47-53` extracts
subcommand names with

```sh
sed -n 's/^- `config \([a-z-]*\)`.*/\1/p'
```

Change the bullet marker from `- ` to `* `, which is the edit a Markdown
linter makes, and it yields **zero** extracted names, **zero** loop
iterations, and `assert_equals '' ''` **passes**. The pattern also hardcodes
the backticks and `[a-z-]*`, so a subcommand name containing a digit silently
drops out.

Four more instances of that same shape were found and fixed during the
previous plan, three of them in `deps-docs.test.sh` alone: an exit-127 oracle
read as acceptance, a parser harvest over an absent file, and a `grep` handed
a file's *contents* where a path belongs. A real parser makes all four
unrepresentable rather than merely fixed.

**Tranche A is nearly twice the size the parent spec states, and this is an
unflagged parent correction until now.** Parent 7.5 says "**10 suites get
better.** They currently `grep` and `sed` over tracked files." Measured
against the real files, tranche A names **19 distinct suites**: the parent's
count predates two workflow suites and omits two Markdown parsers.

`workflow-labels.test.sh:50` is the sharpest omission. It already shells out
to an inline Python parser to filter `.yml` and `.yaml`, which is the
strongest single argument for this tranche, and the parent spec did not have
it.

Doubling the surface strengthens the ordering argument rather than weakening
it: more suites gain a real parser, and none blocks on another step.

**This tranche pays for itself and blocks on nothing. It goes before steps
3b, 4 and 5, not after them.**

**Two exclusions, because steps 3b and 4 use them as gates.**
`container.test.sh` is what the adapter spec relies on to assert cargo's
workspace members against the test Dockerfile, and `scripts-dir-name.test.sh`
is what the subcommand spec relies on to count scripts exactly. Converting
either while another step depends on it puts two documents in one file for
different reasons. **Those two convert after step 4.**

### Tranche B: shell stays the subject (7 suites)

The six `zshrc-*` suites plus `zsh-git-widgets.test.sh`. Rust *drives* them;
the thing under test stays shell, because `.zshrc` is the shell's own
configuration and a ZLE widget must run in-process.

This is honest rather than a gap, and it means "migrate the majority of shell
tests to Rust" resolves to: the harness becomes Rust, the subject does not.

`zshrc-platform-split.test.sh` needs **re-derivation, not a port**: two of
its contracts assert cross-branch properties ("both variants ship on both
branches so neither can drift unseen") that the branch collapse made
vacuous. One branch cannot drift from itself, so the guarantee holds
trivially and the test asserts a mechanism that no longer exists.

### Tranche C: equivalent, with better fixtures (the remainder)

`assert_cmd` plus `tempfile` replaces the `tests/lib.sh` harness. The payoff
is thin per suite, so this goes last and incrementally.

**One claim withdrawn from the parent spec's first draft, recorded so it is
not used as justification again.** It said `tempfile` "fixes the
fixture-ownership defect where cleanup kills tmux sessions by name pattern on
a shared server." The pattern-kill at `lib.sh:87-89` exists but is
**PID-scoped**: names are `TEST_NAME-$$-suffix` and the grep is
`^${TEST_NAME}-$$-`, with a comment stating the PID is there precisely so
concurrent runs cannot collide. Cross-kill would need the same test file
*and* the same PID on the same server. So that defect is unreachable, and
tranche C rests on consistency alone.

## 4. What the port must preserve

- **The `skip` mechanism.** `tests/lib.sh` distinguishes a skipped assertion
  from a passing one, and `run-all.sh` reports the count (currently 49
  skipped). A port that turns skips into passes hides platform-gated
  coverage. `#[ignore]` is not equivalent: it hides the count.
- **Positive controls.** The previous plan's Global Constraints require every
  empty-expected assertion to assert first that its pipeline produced
  something. That rule survives the port and is easier to hold in Rust,
  where an empty `Vec` and a failed command are different types.
- **The container leg.** `tests/run-in-docker.sh` runs the suite inside
  `debian:bookworm-slim`, which is where three Docker-only defects were
  caught that the host missed, including two shellcheck findings the host's
  newer version does not report. A Rust suite must still run there, which
  means the test image needs the toolchain the deps images deliberately lack.
  Section 4a states what that costs and what it fixes, because the same
  change closes a gate hole that exists today.
- **`cargo test` already runs the whole workspace.** Fixed in `fc33e5ac`:
  `run-all.sh` previously pointed `--manifest-path` at one crate, so
  `dotfiles-path`'s tests were outside the suite from the day it landed.
  Measured 4 test binaries before, 6 after.

## 4a. The gate hole this port closes, and the one it does not

Observed on the `deps-core` completion push (2026-09-07): the pre-push gate
printed `SKIP  cargo test (cargo not found)` and passed. Verified afterwards
that the crate was in fact green, so nothing bad shipped, but the gate did
not establish that.

**The mechanism, measured rather than assumed.** `run-all.sh:200` gates the
Rust leg on `command -v cargo`, which is correct logic. The skip is not a
PATH bug on the developer's machine: cargo is on the host PATH at
`~/.cargo/bin/cargo`. It is that pre-push runs the suite in the container
(`tests/pre-push` calls `tests/run-in-docker.sh`, deliberately with no host
fallback), and `tests/docker/Dockerfile`'s runtime stage is Rust-free by
design, carrying only the `config-manifest` binary out of the builder stage.
So `command -v cargo` is correctly false there.

**What is actually covered today, stated precisely, because the first
diagnosis of this was wrong in a way worth recording:**

| Check | Local pre-push | CI (`test-suite.yml`) |
|---|---|---|
| `cargo build` of all three crates | **yes**, builder stage | yes |
| `cargo test` | **no**, runtime stage is Rust-free | yes, `run-all.sh` runs on the runner with cargo preinstalled |
| `cargo clippy -D warnings` | **no** | **no, and nowhere else either** |

So `cargo test` is not unguarded, it is guarded one step later than it
appears: a push cannot break the Rust build locally, and CI catches a broken
test before merge. The narrow local hole is that a red Rust test can leave
the machine, which the seven-task `deps-core` work relied on a human running
`cargo test` by hand to catch.

**The wider hole is clippy, and it has no gate at all.** `grep -rn clippy`
over `.github/`, `.scripts/` and `tests/` returns zero hits outside prose.
The `deps-core` plan's Global Constraints treated
`cargo clippy --locked --all-targets -- -D warnings` at 0 errors as a
standing invariant and spent seven tasks holding it, entirely by hand. An
invariant that no gate enforces is a convention, and this one is load-bearing
enough that two implementers hit it (a `clone_on_copy` on a `Copy` type, and
an unused import that only fires on the non-test target) in tasks whose
briefs did not predict either.

**Decision: this port owns both fixes, because it already owns the change
they need.** Section 4's container-leg bullet commits to putting a Rust
toolchain in `tests/docker/Dockerfile`'s runtime stage, since a ported suite
cannot run without one. Once that toolchain is there:

1. `command -v cargo` becomes true in the container, so `run-all.sh`'s
   existing Rust leg starts running under pre-push with no change to
   `run-all.sh` at all. The hole closes as a side effect of the port rather
   than as separate work.
2. Add a clippy leg to `run-all.sh` beside the `cargo test` leg, gated on the
   same `command -v cargo` probe and reported through `run_suite` so it
   counts like every other suite. It must run
   `cargo clippy --locked --all-targets -- -D warnings` from inside
   `crates/`, not with `--manifest-path`, for the reason already documented
   at `run-all.sh:196-199` and in `test-suite.yml`: rustup honours
   `crates/rust-toolchain.toml` only when the working directory is under
   `crates/`, so a run from the repo root declares the 1.94.0 pin without
   applying it.
3. Add the same clippy step to `test-suite.yml`, so the invariant is enforced
   before merge and not only before push. Cheap: Rust is preinstalled on both
   runners and the workflow already builds from `crates/`.

**The cost, stated rather than waved at.** A toolchain in the runtime stage
makes the test image substantially larger and its build slower, and
`tests/docker/Dockerfile`'s own header gives "stays Rust-free" as a
deliberate property. That property was chosen when the image ran shell
suites against a prebuilt binary. This port changes the premise: the suite
being run IS Rust, so the toolchain stops being overhead and becomes the
thing under test. Reusing the existing builder stage's cached
`~/.cargo`/`target` layers is what keeps the added time bounded, and the
builder already compiles all three crates on every push, so the compile cost
is largely paid twice rather than newly.

**Do NOT fix this by adding a host fallback to `tests/pre-push`.** Its own
comment rejects that explicitly: falling back when the daemon is down
reintroduces the tmux and fixture-repo flake the container exists to contain,
and "a gate that quietly changes what it tests is worse than one that tells
you to start Docker." A host-side `cargo test` in the hook would be that
same defect in a new place. The toolchain goes in the image.

**Sequencing.** Step 2 and step 3 do not depend on the port and are worth
landing first: the clippy leg in CI is a few lines and closes the wider hole
immediately, and it can gate on `command -v cargo` locally so it is a no-op
in today's Rust-free container. Step 1 arrives with Tranche A, since that is
when a Rust test first needs to run in the container. Ordering it this way
means the unenforced invariant gets a gate in the next step rather than after
34 suite conversions.

## 5. Order

1. **Tranche A**, starting with `config-docs.test.sh` because its defect is
   the reproduced one. Then the YAML suites, since `serde_yaml` covers seven
   at once.
2. **Tranche B** after step 5, so the widget port and its test move together.
3. **Tranche C** incrementally, after the subject of each suite has settled.
4. **Delete `lib.sh` and `run-all.sh`** only after both suites have run green
   side by side across several pushes.

## 6. Honest scope statement

Of 41 suites: **7 keep shell as their subject permanently**, and the
remaining 34 become Rust. So the answer to "are we migrating the majority to
Rust" is yes, 34 of 41, but the 7 that stay are staying for a mechanism
reason and not as unfinished work.
