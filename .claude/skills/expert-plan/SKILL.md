---
name: expert-plan
description: Iterative spec-development skill. Combines a relentless Socratic interview (grill-me style) with optional expert-agent consultation during questioning and full expert-panel critique of the resulting spec. Always runs `first-principles` (the wildcard "is the answer already in scope?" lens) so we never spec a thing the codebase already has. Loops: grill -> draft/update spec file -> expert critique + synthesis -> convergence check -> repeat. Re-surfaces previously-decided questions only on high-confidence major-or-blocker findings; otherwise grills new/unresolved ambiguities. Soft cap of 3 cycles, warns at cycle 4+, no hard shutoff. Use to firm up a feature spec before any code is written. Read-only on the codebase; writes a spec markdown file the user owns.
---

# Expert Plan

Spec-development panel. Interview the human relentlessly, draft a spec, critique it with experts, surface what's still uncertain, repeat until the spec is firm. Read-only on the codebase. Writes one spec file the user owns.

Sibling to `/expert-review`. That skill critiques code that exists. This skill develops the spec for code that doesn't exist yet.

## When to use

- A feature has a ticket, a Slack thread, a half-baked proposal doc, and no shared understanding of what's actually being built.
- The user wants to nail down design intent before any code is written.
- The user explicitly invokes this skill. Do not auto-route to it.

For "critique my existing spec doc," `/expert-plan` still applies (cycle 1 grills on the gaps; cycle 2 critiques the result). For "is this code well-designed," use `/expert-review`. For free-form design brainstorming without a spec artifact, use `/system-design`.

## The loop

```
cycle 1: grill -> draft spec file -> expert critique + synthesis -> convergence check
cycle 2: grill (new/unresolved + high-confidence re-opens only) -> update spec -> critique -> check
cycle 3: same
cycle 4+: same, with warning to the user that we're past the soft cap
```

The human calls the stop. The skill reports progress each cycle; convergence is a recommendation, not an automatic exit.

## Roster (curated for spec-development, not code review)

These are the experts consulted during step 1 (model-discretion, tactical) and step 3 (full critique). Curated subset of `/expert-review`'s roster -- the review-only agents (`bug-hunter`, `code-simplifier`, `test-coverage`, `readability`, `debuggability`, `documentation`) have nothing useful to say about a spec.

- **`distsys-data`** -- storage, replication, sharding, consistency, schema evolution, isolation levels.
- **`distsys-runtime`** -- retries, queues, idempotency, caching, sagas, timeouts, circuit breakers, metastable failures.
- **`security`** -- threat model, trust boundaries, AuthN / AuthZ design, OWASP / CWE concerns at the spec level, secrets handling, supply chain.
- **`observability-practice`** -- SLO design, alert plans, on-call ergonomics, error-budget policy, debugging workflows the spec needs to support.
- **`performance`** -- algorithmic and I/O shape concerns visible at the spec level (N+1 risks, hot-path cost, expected scale).
- **`accessibility`** -- POUR concerns when the spec describes UI surfaces. Skip when the feature is backend / CLI / config.
- **`fp-types`** -- ADT / sum-type design, making illegal states unrepresentable, type-driven domain modeling. Almost always applicable.
- **`oo-domain-modeling`** -- aggregates, entities + value objects, bounded contexts, ubiquitous language. Applicable whenever the spec has a domain model.
- **`data-flow`** -- topology lens: who creates / owns / consumes / decides about each long-lived object the spec introduces; what lifetime each object lives at; in which direction does the data flow; where are the boundaries; does the dependency direction match the conceptual altitude. Almost always applicable -- specs that don't crisp these answers produce code that requires expensive refactoring after the fact. The user's `~/.claude/CLAUDE.md` flags "interface boundaries are paramount"; this is the spec-time enforcer.
- **`first-principles`** -- mandatory wildcard reviewer. Asks "is the answer already in scope?" before any spec commitment is made. Q1 (does the codebase already have a service / module / utility solving this problem; does an installed dependency provide it) is the highest-yield check at spec time -- catching "we're about to build a thing that already exists" costs much less here than after code is written. Q2-Q4 (constraint relaxation, problem reframe, cross-domain precedent) cap at `insight`. Especially valuable in cycle 1 of the loop; the user should consider explicitly re-asking it to scrutinize for prior-art on every cycle if the spec scope is expanding.

Other agents (`otel-instrumentation`, `otel-pipeline`, `fp-effects`, `fp-verification`, `oo-patterns`, `oo-architecture`, language specialists like `rust-async` / `typescript-types`) can be consulted opportunistically during the grill when a tactical question lands in their lens. Do not include them in the default critique fan-out.

## Process

### Stage 0: Gather context

The user typically opens with: ticket numbers, links, a Slack thread to search, a proposal doc, a sketch. Collect everything they offer. Ask whether there's anything else (other docs, prior incidents, related features, stakeholders) before starting the grill. This is the only stage where the skill is not driving; the user is loading context.

If the user gave a path to a proposal doc, read it. If they referenced a ticket, ask for the contents (do not fabricate). If they referenced Slack, ask for the relevant excerpts or a search query.

Then propose the spec file path (see "Spec file location" below) and confirm before writing.

### Stage 1: Grill

Adapted from `/grill-me`. Three load-bearing instructions, in priority order:

1. **Interview the user relentlessly about every aspect of this plan until you reach a shared understanding. Walk down each branch of the design tree resolving dependencies between decisions one by one.** Cluster related questions; do not jump randomly between topics.
2. **If a question can be answered by exploring the codebase, explore the codebase instead of asking.** Read files, run targeted `grep`, check existing patterns. The user shouldn't be asked about things the code already says.
3. **For each question, provide your recommended answer with reasoning.** Make it easy to accept or redirect. "I'd default to X because Y, unless Z" beats "what should we do here?"

Additional rule specific to `/expert-plan`:

4. **When a question would meaningfully sharpen with a specialist's perspective, consult the relevant subagent before asking.** Model discretion. Examples: "should this be one aggregate or two?" -> consult `distsys-data` or `oo-domain-modeling` first, then ask the user with the specialist's framing. "What's the right failure mode here?" -> consult `distsys-runtime`. The user can also explicitly request consultation: "ask security about this."

Subsequent cycles narrow the grill (see "Re-grill rule" below).

End of grill: produce a structured **decisions-and-deferrals summary**:

- **Decided this cycle**: list each decision with the chosen answer and one-sentence rationale.
- **Deferred**: questions the user could not answer yet, with an explicit "needs input from <stakeholder | document | spike>" note.
- **TBDs for expert consultation**: questions the user explicitly punted to a specialist ("we'll see what security says about this").

### Stage 2: Draft or update the spec file

Write or update `docs/specs/<feature>.md` (or fallback path -- see below). Structure:

```markdown
# <Feature name>

**Status**: draft (cycle N)
**Last updated**: <ISO date>

## Summary
One paragraph: what is this, why does it exist, who is it for.

## Goals
Bulleted, user-visible outcomes.

## Non-goals
Bulleted, explicit exclusions. Often the most valuable section.

## Background and context
Links to tickets, proposals, prior incidents, related features. The "why now" and "what already exists."

## Design

### <Subsystem 1>
Decisions reached so far, with rationale where non-obvious. Cite the cycle when useful ("decided cycle 2: ...").

### <Subsystem 2>
...

## Open questions

- TBD: <question> -- needs <stakeholder | spike | doc>
- TBD: consulting `<agent>` on <question>

## Risks
What could go wrong. Concrete, not generic.

## Out of scope for spec
Implementation details deliberately not specified here.

## Decision log
- Cycle 1: chose X over Y because Z.
- Cycle 2: revised X to W after security flagged ABC.
```

Subsequent cycles update in place; the decision log accumulates so the history of the spec is auditable.

### Stage 3: Expert critique + synthesis

Fan out the spec file to the curated roster. Signal-driven: only dispatch agents whose lens has substance in the current spec. `accessibility` runs only if the spec describes UI; `distsys-data` runs only if the spec touches storage or replication; etc. Use the same parallel-dispatch pattern as `/expert-review`.

#### Dispatch prompt template

```
Lens: <agent name>
Mode: spec-critique
Cycle: <N>
Spec file: <path>

Read `~/.claude/rules/panel-contract.md` for severity / confidence rubrics and output format. Follow your agent definition for what to look for.

You are critiquing a *spec* (a design document), not code. Findings should focus on:
- Design decisions that look wrong or risky from your lens.
- Missing decisions: what should the spec specify that it doesn't?
- Ambiguities: what reads two ways?
- Assumptions the spec is making that may not hold.
- Alternatives worth considering.

Severity calibration for spec critique:
- blocker: the spec, as written, would lead to a system that fails on your lens's core concerns (security bypass, data-loss-shaped design, inaccessible UI for a class of user).
- major: significant design gap worth resolving before implementation.
- minor: friction or hardening opportunity.
- nit: cosmetic / wording.
- insight: structural reframe (e.g., "this should be an aggregate, not two entities").

Spec contents:
<full contents of the spec file>

Project conventions (override generic principles):
<conventions bundle: root CLAUDE.md, path-prefix CLAUDE.mds, relevant docs/*.md, lint configs, local .claude/rules/*.md>
```

Send all dispatches in a single message (parallel tool calls). Block until all return.

**Synthesize**, applying the same rules as `/expert-review`:

1. Bucket findings by section of the spec.
2. Dedup overlapping findings; merge with both lens tags.
3. Bump confidence when multiple experts agree.
4. Preserve disagreements explicitly. **Mark contradictions** for the next grill cycle ("`security` wants allowlist; `performance` wants fast path; user must choose").
5. Threshold: confidence ≥ 70 main report, 50-69 appendix, < 50 filtered.
6. Re-rank by max severity.

Produce a **cycle critique report** with:
- New findings (severity, confidence, lens, section).
- **Re-open candidates**: findings at major-or-blocker severity with confidence ≥ 70 that contradict a previously-decided question.
- Contradictions between experts (for human resolution).
- Suggestions the spec could absorb directly (low-risk wording fixes, missing non-goals, etc.).

### Stage 4: Convergence check

Report to the user, end of every cycle:

```
Cycle N complete.

Decisions resolved this cycle: A
New questions surfaced by experts: B
Re-open candidates (high-confidence majors against prior decisions): C
Remaining open questions: D
Blocker findings outstanding: E
Contradictions between experts: F

Recommendation: <continue | converged | user-call>
```

**Convergence heuristic**: when B is small (≤ 2), C is zero, E is zero, and the user is comfortable, the loop has converged. Otherwise recommend another cycle.

Soft-cap: at the start of cycle 4 and every subsequent cycle, print:

```
Note: this is cycle <N>. Most specs converge by cycle 3. If we're still surfacing new blockers, consider whether the feature is too big and should be split, or whether there's a stakeholder we haven't pulled in. Continuing.
```

Never stop the loop on the soft cap. The user calls the stop.

## Re-grill rule (cycles 2+)

Default: grill only on **new / unresolved / open** questions surfaced in stage 3.

**Re-surface a previously-decided question** only when a stage-3 finding lands at:
- severity ≥ major, AND
- confidence ≥ 70, AND
- the finding contradicts the prior decision.

When re-surfacing, present the user with:
- The original decision (cycle, rationale).
- The new evidence (which expert, what severity / confidence, the specific concern).
- A recommended revision.

Example: "In cycle 1 you chose JWT with HS256 for service-to-service auth. `security` flagged this at blocker / 90: HS256 means every service holds the signing secret, so any compromise compromises all. Recommend mTLS or RS256. Keep, change to mTLS, or change to RS256?"

**Batch related re-opens.** If one expert finding invalidates a cluster of decisions, present them as one synthesizing question, not separate re-asks.

## Spec file location

1. If `docs/` exists in the repo, use `docs/specs/<kebab-case-feature-name>.md`. Create `docs/specs/` if absent.
2. Otherwise, use `.claude/plans/<kebab-case-feature-name>.md`.
3. **Confirm the path with the user on first write.** Do not silently pick.
4. **Check for collisions**: if a file already exists at the chosen path, ask whether to update it in place or pick a new name. Never silently overwrite.
5. Once confirmed in cycle 1, reuse for subsequent cycles without re-asking.

## Spec writing principles

- **Sections, not stream-of-consciousness.** A reader six months from now needs the structure.
- **Decisions cite their reason.** "Chose X over Y because Z" is much more durable than "Chose X."
- **TBDs are explicit and owned.** "TBD: ask design about copy" beats "needs copy."
- **Decision log accumulates.** Don't rewrite history; append.
- **Out-of-scope is a feature.** Non-goals and "out of scope for spec" sections prevent scope creep.
- **The spec is the artifact; the chat is the working memory.** Anything load-bearing should be in the spec file before cycle end, or it's lost.

## What NOT to do

- Do not write code. This skill produces a spec. Implementation comes after.
- Do not auto-converge. The human calls the stop.
- Do not grill the user on things the codebase can answer. Read the code first.
- Do not consult every expert on every question during the grill. Model discretion. Most questions don't need a specialist.
- Do not re-grill on previous decisions without high-confidence justification. The re-grill rule is the convergence guarantee.
- Do not collapse expert disagreements to a single answer. Surface them for human resolution.
- Do not loop silently. Every cycle ends with a convergence-check report to the user.
- Do not write to a spec path without confirming first.
- Do not include filler like "TODO: figure out" -- TBDs name what input is needed and from whom.

## Decision references

- Panel contract: `~/.claude/rules/panel-contract.md`
- `/expert-review` skill: `~/.claude/skills/expert-review/SKILL.md` (sibling; same synthesis principles)
- `/grill-me` lineage: relentless interview, codebase exploration, recommended answer for each question.
- `/system-design` skill: brainstorm + critique modes for design questions without a spec artifact.
