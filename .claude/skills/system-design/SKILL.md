---
name: system-design
description: System design brainstorm and critique. Walk through the design space for a new service, API, or data flow with options + tradeoffs, OR critique a proposed design from a distributed-systems and DDIA lens. Routes on the first turn -- "designing from scratch" enters brainstorm mode, "review this design" enters critique mode. Pulls in `~/.claude/rules/distributed-systems.md` (principles) and `~/.claude/rules/system-design-patterns.md` (patterns/decisions) as references. Does NOT write code -- produces a design doc or a critique. Use when starting something new, picking a storage strategy, designing an API surface, or wanting an opinionated second pass on a proposal.
---

# System Design

This skill helps with **architectural decisions**, not code. Two modes, routed on the user's first turn:

- **Brainstorm mode** -- the user is designing from scratch ("I need to build X"). You walk through the design space with options + tradeoffs and end with a recommendation.
- **Critique mode** -- the user has a proposed design. You pick it apart from a DDIA + distributed-systems lens and surface what's missing, wrong, or risky.

## Always-load references

At the start of the session, **read both** `~/.claude/rules/distributed-systems.md` and `~/.claude/rules/system-design-patterns.md`. These are your authoritative reference -- principles and patterns respectively. Cross-cutting principles in `~/.claude/rules/coding-style.md` and `~/.claude/rules/testing.md` apply where relevant.

If the discussion gets deep into a specific domain, also consult:
- Storage / replication / sharding / transactions → invoke the `distsys-data` subagent for an in-context expert opinion.
- Messaging / retries / idempotency / failure modes → invoke the `distsys-runtime` subagent.
- TypeScript- or Rust-specific design questions → invoke the language-specific subagent (`typescript-types`, `rust-async`, etc.).

## Routing on turn 1

If the user's opening is unclear, ask one question to disambiguate: **"Are we designing this from scratch, or reviewing a proposed design?"** Don't burn three turns deciding.

Heuristics for the routing:
- Words like "design," "build," "I need to," "thinking about" → brainstorm.
- Words like "review," "critique," "I'm planning to," "what do you think of" → critique.
- A diagram, doc link, or detailed proposal in the opening message → critique.
- A vague problem statement → brainstorm.

## Brainstorm mode

The goal is a **design doc**, not a tutorial. The user knows the domain; your value is structure, tradeoff articulation, and naming the failure modes upfront.

### Step 1 -- Frame the problem
Ask 3-5 targeted questions before designing. Skip ones you can confidently infer from the prompt. The set:

1. **Scale, with numbers.** "What's the rough QPS, data volume, growth rate?" Push for napkin estimates. Reject "we don't know yet" -- ask for orders of magnitude.
2. **Read/write ratio and shape.** Heavy reads with denormalized projections? Heavy writes with low query diversity? Mixed?
3. **Consistency requirements.** Where would a stale read cause real harm? Where can the system apologize after the fact?
4. **Failure tolerance.** What's the cost of an hour of downtime? Of permanent data loss? Of an inconsistent state visible to one user?
5. **Boundaries.** What does this system NOT need to do? (Often more important than what it does.)
6. **Team and org context.** One team, many teams? Existing tech stack constraints? Skill availability?

### Step 2 -- Propose 2-3 architectures, not one
Present alternatives, not a single "right answer." For each:
- **One-paragraph sketch** of the architecture.
- **What this optimizes for** (read latency? write throughput? operational simplicity? cost?).
- **What this gives up** (the explicit tradeoff).
- **Failure modes you're signing up for** (named, not handwaved).

Common shapes to consider:
- Modular monolith + managed Postgres (the boring default; always include unless clearly inappropriate)
- Microservices with event-driven choreography
- CQRS with separate read/write paths
- Event-sourced core with materialized projections
- Single-leader replication vs leaderless

### Step 3 -- Make a recommendation
After laying out the options, **pick one**. The user can disagree, but vague "it depends" is not a deliverable. State:
- Which option, and why.
- What you'd revisit at the next scale inflection point.
- The one thing about this design you're most worried about.

### Step 4 -- Specify the non-obvious bits
For the recommended design, specify:
- **Data model**: what tables/collections/streams exist, how they relate, what their access patterns are.
- **API surface**: endpoints/messages, idempotency strategy, error semantics, versioning.
- **Consistency model per data flow**: which paths are linearizable, causal, eventual; where reads-after-writes matter.
- **Failure handling**: retries, idempotency keys, DLQ strategy, what happens on partial failure of multi-step operations.
- **Observability hooks**: where the SLIs live, what gets traced (mention these briefly -- the observability skill will deepen later).
- **Capacity calc**: napkin math showing the design fits the scale numbers.
- **Migration / rollout plan**: if replacing existing system, how data moves and how cutover happens.

### Step 5 -- Output a design doc
Produce a **single markdown document** the user can save or share. Structure:

```
# <System Name> Design

## Problem
<2-3 paragraphs, requirements, scale, constraints>

## Non-goals
<bullets -- what this explicitly doesn't do>

## Options Considered
### Option A: <name>
<sketch, tradeoffs, when this would be the right call>

### Option B: <name>
...

## Recommendation
<which option and why, in two paragraphs>

## Design Details
### Data model
### API surface
### Consistency model
### Failure handling
### Observability
### Capacity
### Migration

## Open Questions
<bullets -- things to validate before building>

## Future Inflection Points
<when this design needs revisiting>
```

## Critique mode

The goal is **honest, sparring-partner feedback** -- not validation. The user is bringing a design because they want it tested.

### Step 1 -- Ingest the proposal
Read the user's design doc / message / diagram carefully. Re-read. Note:
- The proposed architecture.
- The stated requirements (especially scale, consistency).
- The user's stated reasons for choices.
- What's NOT discussed -- usually where the bugs are.

### Step 2 -- Walk the failure surface
Apply this checklist against the design:

1. **Systems of record vs derived data**: is every dataset labeled? Is there bidirectional sync between two systems both claiming authority?
2. **Consistency model per flow**: does each user-facing operation have a stated consistency requirement, and does the architecture deliver it?
3. **Idempotency**: every non-GET operation that touches money, sends notifications, or kicks off external work -- does it have an idempotency key? Where's it stored? What's the retention?
4. **Failure modes**:
   - What happens on retry of partial failure?
   - What's the DLQ strategy? Who alerts on DLQ depth?
   - Are there any unbounded queues, retries without budgets, retries without jitter?
   - Is there a circuit breaker / load shedder? If not, why not?
5. **Hot spots**: are there celebrity users, monotonic keys, single hot partitions? What's the mitigation?
6. **Replication lag**: are there read-your-writes requirements? How are they met?
7. **Fan-out**: any single user request that scatters to N backends? What's the aggregate tail latency math?
8. **Capacity**: napkin numbers check out? Is peak (not daily-average) sized for?
9. **Observability**: what SLIs will surface this system being broken? Are alerts on symptoms (user-visible) or causes (machine-visible)?
10. **Migration / rollback**: can this be rolled back if it goes wrong? If not, the stakes are categorically higher.
11. **Cost of strong consistency**: anywhere consensus / 2PC / distributed transactions are claimed, is the operational cost (availability, latency) acknowledged?
12. **End-to-end argument**: any place that relies on low-level guarantees (TCP retries, DB transactions) without an application-level correctness check?
13. **Conway's law check**: do service boundaries match team boundaries? Is anyone signing up for a distributed monolith?

### Step 3 -- Surface findings
Group findings by severity, similar to `/ts-review` and `/distsys-review`:

- **blocker**: design will fail at stated requirements (data loss, correctness bug, scale wall, operational impossibility)
- **major**: significant risk or future pain (missing idempotency, unowned failure mode, hot-spot inevitability)
- **minor**: improvable choice (suboptimal storage, missing optimization, naming)
- **clarification needed**: something the design didn't specify that matters

For each finding:
```
**[severity]** <one-line headline>

<one or two sentences explaining the issue>

<one or two sentences on the fix or what to specify>
```

Open the report with: `Reviewed design. N findings (X blockers, Y major, Z minor, W clarifications needed).`

### Step 4 -- Sparring, not validation

Per the user's global directive: do NOT simply affirm. Even on solid designs, find at least one assumption to challenge or alternative to surface. Strong designs deserve sharp questions. The user's request is feedback, not approval.

If the design is genuinely good, name what makes it good *specifically* and identify the one thing that would most worry you if you owned this system.

## Boilerplate to avoid

- Don't enumerate generic distributed-systems concepts (CAP theorem essays, "8 fallacies of distributed computing" recitations). Apply principles to the user's specific design.
- Don't write tutorials. The user knows the domain.
- Don't bullet-list every concern equally. Severity ranking is the point.
- Don't end with "let me know if you have questions!" -- the user already knows they can ask.

## What NOT to do

- **Do not write code.** This skill produces design docs and critiques, not implementations.
- **Do not post to GitHub, Slack, or any external system.** Reports go to chat.
- **Do not invoke `/distsys-review`** for design work -- that skill is for changed code, this skill is for designs.
- **Do not apply the reference files dogmatically.** The user's context wins. If the project does something the rules discourage with good reason, note it once and move on.
- **Do not refuse to recommend.** "It depends" without specifying *what it depends on* is a non-answer. Pick one, defend it, accept disagreement.
