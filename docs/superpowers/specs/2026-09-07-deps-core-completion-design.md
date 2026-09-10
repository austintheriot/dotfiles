# `deps-core` completion: the seven defects the adapter needs fixed

**Date:** 2026-09-07
**Status:** design, not yet planned
**Parent spec:** `docs/superpowers/specs/2026-09-06-pure-core-architecture.md`
(the core half of that spec's Step 3)
**Depends on:** nothing. Every item is inside `crates/deps-core` or
`crates/dotfiles-path`, both of which exist with 130 passing tests.
**Blocks:** `2026-09-07-config-cli-adapter-design.md`. That step cannot be
planned honestly until items 1 and 2 below are done, because the adapter
cannot install four of the 22 dependencies and has nothing to render with.

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

## 1. Why this spec exists

`crates/deps-core` was declared complete: 130 tests, zero IO, purity enforced
by a test with a positive control. A seven-lens expert review found seven
defects in it, six of them by reading the code rather than the specs.

The adapter spec absorbed all seven, which was wrong. That document's own
section 2 says core work is out of scope, and two of the seven change
**public types other code already depends on** (`Verdict` and
`Installer::describe`). Landing type surgery and a new binary in one step
means a failure cannot be attributed to either half.

So this is the core half of the parent spec's Step 3, and it goes first.

## 2. Scope

**In:** everything under `crates/deps-core/src/` and
`crates/dotfiles-path/src/` that the review found wrong or missing.

**Out:** the `config-cli` binary, the `Installer` implementations, the
observation gather's *implementation*, the Docker seam, and the retirement of
`check-deps.sh`. All of those are the adapter spec's.

The distinction is mechanical: if it compiles with no new IO and no new
crate, it is here.

## 3. The seven items, in dependency order

Item 1 and item 2 are what unblock the adapter. Items 3 to 7 are correctness
defects that would otherwise be discovered by the adapter's acceptance tests,
which is the expensive place to find them.

### Item 1: four `InstallAction` variants are unreachable from `plan`

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

### Item 2: `render` and `Rendered` do not exist

Covered by item 1's text above. Stated separately because it is a distinct
deliverable: `deps-core` gains `render(&Report, Verb) -> Rendered` and
`Rendered { stdout, stderr, exit_code }`, following
`config-manifest/src/stamp.rs:18`, which the parent spec's section 4 already
cites as the precedent. Nothing constructs a `Verdict` today either, and
`Verdict::DryRun` versus `Verdict::Check` is the per-verb distinction the
exit-code regression test depends on.

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

## 3a. The lint-tool acquisition path is unwritten (added 2026-09-10)

Not part of this spec's seven items, recorded here because this is the spec
that owns the dependency engine and the question has no other home.

`stylua` and `selene` gate two test suites (`nvim-lua-format.test.sh` and the
selene suite) and are acquired through **mason**, on nvim's first launch.
Neither appears anywhere in this corpus: grepping every spec and plan for
either name returns nothing. Nor does `mason`. So the boundary between
"the deps engine installs it" and "nvim's package manager installs it" is
unwritten, not decided.

The precedent for moving them is already in the manifest, in
`deps/deps.toml`'s own comment on `tree-sitter-cli`: it was a mason package
first, and it moved to the engine because "it must exist BEFORE nvim first
runs", after which only 3 of 19 parsers compiled on a fresh machine. A test
gate that needs a linter before nvim has ever launched is the same shape.

Verified 2026-09-10 in an `ubuntu:24.04` container: **neither tool is in
apt.** `apt-cache policy` reports no candidate for `stylua` or `selene`, while
`shellcheck` has 0.9.0-1. Both are in brew. So the Linux path cannot be a
`Named` availability and needs a `ViaTarball` arm, which means two new
`TarballRelease` variants. `stylua` ships a `.zip` release asset and `selene`
a `.tar.gz`, and `installer.rs` currently handles the gzipped-binary shape;
whether it handles a `.zip` is unverified.

What this buys, stated honestly so it is not oversold: it removes about 4 of
the roughly 19 runtime skips that survive a Rust conversion. It does not
touch the four platform-variant skips, which need the *other* platform's zsh
and no dependency can supply that. The justification is the fresh-machine
bug, not the skip count.

Unresolved and deliberately left so: whether two linters used by two suites
justify a heavier bootstrap on every machine and every CI leg.

## 4. What this changes on disk

| Path | Change |
|---|---|
| `crates/deps-core/src/action.rs` | `PackageAvailability` or a sibling gains a way to select `GitClone`, `NvmInstall`, `Pip`, `AptSource`. |
| `crates/deps-core/src/plan.rs` | `action_for` reaches the four new variants. `Requirements::validated`. `PrerequisiteNotSelected` splits in two. |
| `crates/deps-core/src/outcome.rs` | `Verdict::Install` carries a `CheckStatus`. `StepOutcome::Declined`. `render` and `Rendered`. |
| `crates/deps-core/src/driver.rs` | `Installer::describe` takes `&Step`. |
| `crates/deps-core/src/check.rs` | `PathRoot::OhMyZshCustom` deleted; `GitClone` gets a `Home`-rooted `CheckPath`. |
| `crates/dotfiles-path/src/bounded.rs` | Unchanged. Named because item 6 makes it load-bearing for the adapter. |

## 5. How this is verified

Every item is a defect the review demonstrated, so every item has a test
that fails before it and passes after. That is the acceptance criterion, and
it is stronger than usual here because the code already has 130 tests that
pass while these defects are present.

Three that need naming, because a weak version would pass vacuously:

- **Item 1** needs a test that `plan` emits `GitClone` for
  `zsh-autosuggestions` under a non-brew manager. A test asserting only that
  the variant exists proves nothing.
- **Item 3** needs a test that `deps install` on a manifest containing a
  manual-only dependency yields a **nonzero** exit. The existing test at
  `outcome.rs:309-315` asserts the opposite and must be updated
  deliberately, not deleted.
- **Item 5** needs the union-of-all-conf-files test: it runs on both
  platforms and catches a typo in the requirement table that neither
  platform's live run would.

The purity test must be extended to any new module in the same commit that
creates it, and the needles stay assembled from runtime segments, because a
literal in a scanned file makes the crate fail its own check.

## 6. Consequences

After this, `deps-core` can express every install the manifest requires, and
the adapter spec's acceptance tests can mean what they claim. Before it, they
cannot: `plan` cannot emit the action `zsh-autosuggestions` needs, and that
dependency's convergence is the adapter step's own named regression test.
