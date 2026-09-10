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

**Section 4a is smaller than a tranche and blocks on nothing at all**, not
even Tranche A, and it needs no change to the test image. It adds the Rust
checks to `tests/pre-push` on the host and to `test-suite.yml`, and declares
the lint policy in `crates/Cargo.toml` so it binds every invocation rather
than one command line. Worth landing before spec 2, which adds the first new
binary crate and would otherwise inherit an unenforced invariant.

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

## 2a. Runtime, measured 2026-09-10 (added after the fact)

This spec carried no timings. Grepping it for "second", "slow" or "fast" as a
runtime measure returns nothing, so "the suite is slow" was never a premise
here and must not become one retroactively. The numbers below exist so a
later reader does not assume converting shell to Rust makes the suite faster.

Measured on an M1 Max, host leg, `tests/run-all.sh -q`:

| What | Wall time |
|---|---|
| The whole suite | **374s** |
| `config.test.sh` alone | **105s** |
| `config-usage.test.sh` | 15s |
| `leak-check.test.sh` | 12s |
| `config-manifest-lifecycle.test.sh` | 11s |
| every other suite | under 8s each |

Two consequences.

**`config.test.sh` is 28% of the suite, and the cost is cargo, not shell.**
Line 454 calls `config-build`, which compiles all six workspace crates. A
conversion to Rust cannot remove that cost, and adds compile time of its own.
Whatever is done about the 105s is a separate change from this spec, and this
spec should not be credited with it.

**`.claude/rules/dotfiles-tests.md` said "about 25 seconds".** That is a 15x
understatement and it is corrected in the same commit as this amendment. The
claim was written when it was true; nothing updated it as suites accumulated.

**`config.test.sh` appears in no tranche.** It is absent from section 3
entirely, which is an omission rather than a decision: it is the single most
expensive suite in the set. Placing it needs the `config-build` question
answered first, so it stays unplaced here deliberately, with the reason
recorded.

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
  Section 4a owns that change. It does NOT own the local gate fix: the Rust
  checks are hermetic and run on the host before the container leg, so
  closing today's gate hole does not wait for this image to grow a
  toolchain.
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

### 4a.1 The Rust checks run on the HOST, before the container

The first draft of this section said the local fix waits for a toolchain in
the test image. **That was wrong, and the reason it was wrong is the reason
the container exists in the first place.**

`tests/run-in-docker.sh` exists to contain **mutation**. Its own header and
`tests/docker/Dockerfile`'s say so: the shell suites write fixture git repos
into `$HOME`, spawn tmux sessions on the real server, and once created a real
`~/.oh-my-zsh/custom/plugins/zsh-autosuggestions` on a machine that does not
use oh-my-zsh. `tests/pre-push` refuses a host fallback for that reason, and
that refusal is correct **for those suites**.

`cargo test` is not one of those suites. It mutates nothing:

- `deps-core` and `dotfiles-path` are pure by construction, and `deps-core`
  enforces it with a purity test that scans its own sources for the IO
  capability paths and fails the build if one appears.
- `config-manifest` does shell out to `git` and read `$HOME` in production,
  but **every one of its tests builds its own `tempfile::tempdir()` and
  passes explicit `--git-dir` and `--work-tree`**, so no test reads or writes
  the real repository. The `$HOME` reads are in `main.rs` runtime paths, not
  tests.

**Verified rather than reasoned.** The whole workspace was run with `HOME`
pointed at an empty throwaway directory (`RUSTUP_HOME` and `CARGO_HOME` kept
real, since those are the toolchain's, not the tests'). Result: all suites
green, the real home directory unchanged at 111 entries, tmux session count
unchanged, and **the fake home completely empty afterwards**. Zero mutation.

So the local hole needs no image change at all:

1. **Run the Rust checks on the host in `tests/pre-push`, before it hands off
   to Docker.** They are fast, hermetic, and they fail fast on exactly the
   class of breakage the container leg cannot see. This is not the rejected
   "host fallback": a fallback silently substitutes a weaker check when the
   daemon is down, whereas this is an additional gate that always runs and
   never replaces the container leg. If cargo is absent it must SKIP loudly,
   the same way `run-all.sh` does, because a gate that says nothing when it
   skips is indistinguishable from one that is not installed.
2. **Leave `tests/docker/Dockerfile`'s runtime stage Rust-free until a ported
   Rust suite actually needs to run there.** Section 4's container-leg
   requirement still stands for Tranche A onward, but it is no longer a
   prerequisite for closing the gate, and it stops being a reason to carry
   toolchain weight in an image that today runs only shell.
3. **Add the same checks to `test-suite.yml`.** `cargo test` already runs
   there through `run-all.sh` on the runner; the strict checks below do not.

All Rust invocations run from inside `crates/`, never with
`--manifest-path`, for the reason already documented at `run-all.sh:196-199`
and in `test-suite.yml`: rustup honours `crates/rust-toolchain.toml` only
when the working directory is under `crates/`, so a run from the repo root
declares the 1.94.0 pin without applying it. That exact mistake has failed
CI once already.

### 4a.2 Strict checks, and where strictness belongs

The invariant the `deps-core` work held by hand was one command line. A
command line is the wrong home for it: it binds one invocation, so an IDE, a
bare `cargo clippy`, and a teammate's terminal all disagree with the gate.

**Decision: `[workspace.lints]` in `crates/Cargo.toml`, with each member
opting in via `[lints] workspace = true`.** Lint configuration then travels
with the code and applies to every invocation, and the gate's `-D warnings`
becomes the enforcement of a policy declared in the manifest rather than the
policy itself.

**Measured before specifying, because a strict set adopted blind is a
strict set that gets switched off.** A trial of `clippy::all` +
`clippy::pedantic` + `rust_2018_idioms` + `missing_docs` + the
panic-family lints over the current workspace produced **315 warnings with
`--all-targets`**. The breakdown is what decides the design:

| Warning | Count | Disposition |
|---|---|---|
| `expect()`/`unwrap()`/`panic` in tests | ~123 of 126 | **Test-only. Must not be denied.** CLAUDE.md permits them in test code, and only **3** occur outside tests. |
| `missing_docs` on public variants and struct fields | 68 | **Adopt.** CLAUDE.md already requires doc comments on public items, so these are real gaps. |
| `must_use_candidate` | 41 | **Do not adopt.** Pure `pedantic` noise on a codebase whose builders are already used positionally. |
| `missing_errors_doc` | 5 | **Adopt.** CLAUDE.md already requires `# Errors` on fallible public functions. |
| `format_push_string`, `doc_markdown`, `redundant_closure`, `match_same_arms`, `items_after_statements` | ~30 | **Adopt case by case**, all mechanical. |

The three non-test `expect()` calls are all `writeln!` into a `String` at
`config-manifest/src/doctor.rs:153,157,167`, which cannot fail. Those are
CLAUDE.md's "proven invariant" exception, so `expect_used` at deny would
force an `#[allow]` on correct code. `clippy::format_push_string` already
flags that same pattern more precisely and is the better lint to adopt.

**So the strict set is deny-by-default with two scoped carve-outs, not a
blanket `pedantic`:**

```toml
[workspace.lints.rust]
unsafe_code = "forbid"          # deps-core and dotfiles-path have none; keep it that way
missing_docs = "warn"
unused_qualifications = "warn"
rust_2018_idioms = { level = "warn", priority = -1 }

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
missing_errors_doc = "warn"
format_push_string = "warn"
# NOT pedantic wholesale: 41 must_use_candidate warnings are noise here.
# NOT unwrap_used/expect_used/panic at workspace level: 98% of the hits are
# test code where CLAUDE.md permits them, and the 3 production sites are
# writeln! into a String, which cannot fail.
```

`unsafe_code = "forbid"` is the one at `forbid` rather than `warn`: forbid
cannot be lifted by a local `#[allow]`, which is the point. Nothing in these
crates needs unsafe, and `deps-core`'s whole design argument is that it holds
no capabilities.

**The panic-family lints are still worth having, scoped to non-test code.**
Cargo cannot express "deny in `src`, allow in `#[cfg(test)]`" through
`[workspace.lints]`, so the options are a crate-level
`#![cfg_attr(not(test), deny(clippy::unwrap_used))]` per crate, or leaving
the rule to review as today. **Decision: the `cfg_attr` form, one line per
crate**, because it makes the rule mechanical exactly where CLAUDE.md makes
it absolute and silent exactly where CLAUDE.md permits the construct. Verify
after adding it that the three `doctor.rs` sites still compile, since they
are the only production hits and they are legitimate.

**Adoption is one commit per lint family, not one big commit.** 68 missing-doc
warnings is real work on public API surface, and mixing it with the gate
wiring means a reviewer cannot tell a policy change from a docs change. Land
the gate first with the lint set at `warn`, then flip to the gate's
`-D warnings` once the count is zero, so the gate never lands red.

**Sequencing.** All of 4a.1 and 4a.2 blocks on nothing, not even Tranche A,
and none of it needs the test image to change. It is worth landing before
the adapter step, because that step adds the workspace's first binary crate
past `config-manifest` and would otherwise inherit an invariant that no gate
enforces.

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
