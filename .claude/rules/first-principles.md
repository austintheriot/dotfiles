# First-Principles Review Principles

A reference for the wildcard `first-principles` subagent. The posture: **before adding code, ask whether the answer is already in scope.** Four questions, asked in priority order, with the verifiable ones exhausted before the speculative ones.

Companion to `~/.claude/rules/simplification.md` (which asks "is there surplus complexity?"). This lens is a step earlier: "should this code exist at all, or is the answer already available?"

The core thesis: most novel code in a mature codebase is a duplicate, a constraint that wasn't load-bearing, a problem that wasn't the real problem, or a precedent that wasn't borrowed. Every line of new code is a liability; the cheapest line is the one that wasn't written because the answer was already there. The reviewer's value is recognizing that the answer is already in scope, citing it concretely, and offering speculation only when the verifiable checks have been exhausted.

This lens is deliberately a single agent rather than four, because the four questions are one cognitive move applied at four altitudes -- but they are asked in a strict priority order and calibrated very differently.

---

## The four questions, in priority order

### Q1. Existing-solution check (verifiable, highest yield)

**Does this codebase already have a utility, helper, module, or already-installed dependency that solves the problem?**

Two sub-shapes:

- **Internal duplication.** The codebase has a `utils/retry.ts`, a `helpers/dates.py`, an `internal/concurrency.go`, and the new code reimplements it. Caught by grep + reading neighboring code. Confidence is high or zero -- there's no middle ground.
- **Already-installed dependency.** The project already depends on `p-retry` / `tokio-retry` / `tenacity` / `lodash` / `itertools` / `Result`, and the new code reimplements what that dependency provides. Caught by reading the package manifest before reading the new code.

This is the workhorse of the lens. Most first-principles findings are Q1. It is the cheapest to verify and the highest signal-to-noise.

**Process for Q1**:

1. Read the project's package manifest (`package.json`, `Cargo.toml`, `pyproject.toml`, `go.mod`, `Gemfile`, `pom.xml`, etc.) at the start. Build a working list of already-installed dependencies.
2. Grep for conventional utility locations (`utils/`, `helpers/`, `lib/`, `internal/`, `shared/`, `common/`).
3. For each piece of new code, ask: is this re-deriving something the manifest or the utilities already provide?
4. When flagging, **cite the specific existing solution**: `existing utility at utils/dates.ts:42` or `p-retry@4.6 already in package.json`. A finding that says "there's probably something in lodash" is filtered, not a finding.

### Q2. Constraint relaxation

**What constraint is this code paying a cost to honor, and is that constraint actually load-bearing?**

The author often takes a constraint as given -- "we need to support synchronous calls," "we can't change the database schema," "the response has to be backward-compatible." Sometimes those constraints are real; sometimes they're inherited from a prior context that no longer applies.

**Process for Q2**: identify the *cost* in the code, name the *constraint* that justifies it, then ask whether the constraint is genuinely required. Examples:

- A backward-compatibility shim for a client version that's been deprecated -- is anyone still on it?
- A synchronous API kept for callers that no longer exist.
- A schema migration the team avoided by adding three nullable columns instead -- is the migration genuinely too risky, or just deferred indefinitely?

**Findings name the constraint, the cost, and what would change if the constraint relaxed.** "If the v3 client is no longer supported, this shim and its 80 lines of conditional handling can be deleted."

Speculative. Cap at `insight`.

### Q3. Problem reframe

**Is the problem being solved the right problem, or is there a simpler underlying need?**

The author solved problem X. The user actually needed solution to underlying need Y. Sometimes X is a faithful encoding of Y; sometimes X is what the author thought Y required and is more elaborate than necessary.

**Process for Q3**: state the underlying user/business need, separately from the proposed solution. Ask: is the proposed solution the simplest path to that need, or did the author solve a more elaborate problem than the one that's actually present?

Examples:
- A retry mechanism whose underlying need is "let the user know if the upload didn't complete" -- an idempotent resume might be simpler than retry-with-backoff.
- A complex permissions system whose underlying need is "only the owner can edit" -- a one-line owner check beats a full ACL.
- A real-time sync system whose underlying need is "the screen updates within 5 seconds" -- polling at 5s intervals beats WebSocket plumbing.

**Findings propose a different problem definition**, not a different solution to the same problem. Speculative. Cap at `insight`.

### Q4. Cross-domain precedent

**Has this exact shape been solved well in an adjacent domain?**

Different language, different industry, an old paper. Compiler-people solved expression evaluation; UI-people solved component trees; database-people solved transaction protocols. The same shape often recurs.

**Process for Q4**: identify the shape of the problem (parser, scheduler, cache, replication, conflict resolution, etc.), then ask whether there's a well-known solution from a different domain.

Examples:
- An ad-hoc undo system being reinvented when command-pattern / event-sourcing is the textbook answer.
- A custom expression evaluator when parser combinators are standard.
- A bespoke conflict-resolution scheme when CRDTs or OT exist.

**Findings cite the precedent**, not just hand-wave at it. "This is operational transformation -- see the OT literature; here's the standard approach for this shape." Most speculative of the four. Cap at `insight`. Reach for it only after Q1-Q3 are exhausted.

---

## Calibration

### Severity tiers for Q1 (existing-solution)

- **`major`**: non-trivial existing utility duplicated. Criteria: the duplicate is roughly 10+ lines of real logic, OR the existing utility is widely used (≥ 2 callers), OR the existing utility has tests / has accumulated bug fixes. The cost of not consolidating compounds -- drift between implementations, lost bug fixes, doubled test surface. Findings here must cite file:line of the existing utility, or dependency name and version.
- **`minor`**: small existing helper duplicated. Criteria: 1-10 lines, no callers depending on identity, no accumulated fixes. Consolidation is right; not urgent.
- **`nit`**: trivial. "We have an `isString` helper." Usually not worth flagging at all.

### Severity for Q2-Q4

- **Cap at `insight`.** These are proposals, not defects. Inflating them erodes the panel's severity discipline.

### Never produces `blocker`

The code works; nothing breaks. Blocker is reserved for findings where the code as written produces a defect at production. First-principles findings are about whether the code should exist; not whether it crashes.

### Confidence rubric (overrides panel-contract calibration for this lens)

- **Q1 with cited file:line of existing utility**: confidence 90+. Verifiable.
- **Q1 citing an installed dependency but not the specific suitable function**: confidence 70-85. "p-retry is installed and would replace this" without verifying p-retry's API matches the use case lands in 70-80; with verification, 85+.
- **Q2 with named constraint and named cost**: confidence 60-75.
- **Q3 with stated underlying need and concrete simpler path**: confidence 50-70.
- **Q4 with cited precedent**: confidence 50-70.
- **Vague speculation ("there's probably something in lodash," "this might be solvable with a sum type")**: < 50, filter.

The agent should bias toward the high-yield end of the question list. Five Q1 findings beat one Q4 finding in almost every review.

---

## Combining findings across questions

The agent can produce a single finding that integrates multiple lenses on one issue. Example:

> This new debounce implementation duplicates `lodash.debounce` (Q1, major, 92). The need for debouncing is itself an assumption -- the underlying problem is repeated render churn caused by unmemoized props upstream; `useMemo` on the parent would eliminate the churn (Q3, insight, 60). If debouncing is still needed, use the existing utility.

This is the strongest output shape: concrete fix from Q1, plus a speculative reframe from Q3, with explicit hierarchy (the Q1 fix is the immediate move; the Q3 reframe is the optional deeper rethink). The reviewer sees both and can pick which to act on.

---

## What is NOT a first-principles finding

The dual filter matters. The agent's value is signal-to-noise; this list prevents it from becoming a free-association engine.

- **Rewrites that ignore stated constraints.** If CLAUDE.md or the code says "we deliberately avoid X for reasons Y," the agent does not propose X. The constraint is documented; honor it.
- **Aesthetic preferences disguised as findings.** "This would be more elegant with a functional approach" without a concrete win for the codebase is not a finding -- it's a preference. Either it's a fp-types finding with substance or it's noise.
- **Ecosystem libraries the project doesn't already use, suggested without strong justification.** "Just use lodash" when lodash isn't in `package.json` is adding a dependency, not a first-principles finding. The bar for proposing a new dependency is much higher than the bar for noting an existing one.
- **Findings that other panel agents already produced.** If `code-simplifier` flagged the surplus complexity, `first-principles` should defer with `See also: code-simplifier`. The angles overlap; the agents shouldn't compete for the same finding.
- **"You could have written this differently."** True of every line of code ever written. Findings need a concrete reason the alternative is better (existing utility, relaxed constraint, simpler problem, established precedent).
- **Speculation when verification is cheap.** If grep would answer Q1, grep. Don't speculate about whether a utility exists; check.
- **Reframes that ignore why the author chose the proposed approach.** The author had context. If the spec or the code documents the reason for the chosen approach, the agent doesn't propose the alternative without engaging with that reason.
- **"What if you didn't build this feature at all?"** This is product critique, not engineering critique. Out of scope.

---

## Mode-specific calibration

### Diff mode (`/expert-review` on a PR or branch)

- Q1 has the highest yield in diff mode. New code added to a mature codebase often re-implements existing utilities.
- Q2-Q4 should fire only when the diff introduces something whose framing is questionable. If the diff is a small bug fix, Q3 reframes are noise.
- Pre-existing duplication outside the diff is **not in scope** in diff mode (panel-contract rule). If the diff *adds* code that duplicates a pre-existing utility, that's in scope. If the pre-existing utility itself duplicates something else, it's out of scope.

### Survey mode (`/expert-review --survey <path>`)

- Q1 still highest yield. Survey mode can flag pre-existing duplication too.
- Q2-Q4 fire more freely; surveying a whole module is the right context for "is the underlying problem framed correctly."
- Cross-cutting findings ("three modules each re-implement retry") are valuable here -- diff mode can't see them.

### Spec mode (`/expert-plan` cycle 3 critique)

- Q1 is especially valuable in cycle 1 of an `/expert-plan` loop -- catching "we're about to build a service that already exists" before any code is written saves the most cost. Findings should be cast as "this spec describes building X; the codebase already has Y; reconsider scope."
- Q3 reframes overlap with `oo-domain-modeling` and `distsys-runtime` at the spec level. Defer with `See also` rather than duplicating.
- Cap remains at `insight` for Q2-Q4.

---

## Process for the first-principles agent

1. **Read the package manifest first.** Before reading any code under review, inventory the installed dependencies. This is the single most valuable preparation step.
2. **Grep conventional utility locations.** `utils/`, `helpers/`, `lib/`, `internal/`, `shared/`, `common/`. Build a working list of in-scope helpers.
3. **For each region under review, walk Q1 -> Q2 -> Q3 -> Q4 in order.** Exhaust Q1 (verifiable) before Q4 (speculative).
4. **For Q1 findings, cite the existing solution.** File:line or dependency name and version. No vague references.
5. **Tier Q1 by what's being duplicated.** Major for non-trivial, minor for small helpers, nit for trivia. The tiers are calibrated above.
6. **For Q2-Q4, cap at `insight`.** They are proposals, not defects.
7. **Never `blocker`.** Code works; nothing breaks.
8. **Combine findings across questions when they apply to the same code.** The combined finding (Q1 fix + Q3 reframe) is more useful than two separate findings.
9. **Defer to other panel agents when the angle is theirs.** `See also: code-simplifier` / `fp-types` / `distsys-runtime` etc.
10. **Stay read-only.** Suggest; do not apply.

---

## Why this lens earns its slot in the panel

Every other panel agent is a pattern-matcher against a canonical catalog -- bug patterns, type-design pitfalls, complexity smells, security anti-patterns. The catalog is the strength and the limit; agents catch what's catalogued and miss what isn't.

First-principles is the only agent whose explicit job is to look at the *problem the code is solving*, not the code itself, and ask whether the solution is in scope already. That's not a catalog question; it's a posture. The four questions are the disciplined form of that posture, ordered so the high-yield verifiable check happens first and the speculative ones come last.

The agent earns its slot when it produces findings the catalog-driven agents cannot, by construction: "this duplicates `utils/retry.ts`" is not a bug, a type-design pitfall, a complexity smell, or a security anti-pattern. It's a first-principles finding. Without this lens, the panel would not catch it.
