---
name: data-flow-review
description: Expert review pass for data-flow / state-ownership / lifetime / boundary topology -- the lens that asks "who creates this, who owns it, who consumes it, who decides about it" at every non-trivial entity in scope, and flags mismatches between conceptual scope and instantiation scope (singleton inside a per-note constructor), wrong direction of data flow (consumer reaches up into producer), constructors that reach out into the world, hidden state via module-mutable / closure capture, identity-vs-value confusion (Hickey), wrong dependency direction (domain depending on infrastructure), missing teardown on owned objects, and boundary placement that doesn't match the natural seams. The "draw the data-flow diagram before reading the body" discipline. Especially load-bearing in a world where the implementation body is increasingly AI-written and routinely regenerated -- the topology survives replacements; the body doesn't. Reviews the current branch diff against main by default, a specific file/PR with `/data-flow-review <path>` or `/data-flow-review <PR#>`, or a git range. Auto-delegates depth to the `data-flow` subagent. Produces severity-labeled findings with file:line references plus a data-flow diagram per finding when the alternative is concrete. Does NOT post comments.
---

# Data-Flow / Ownership / Lifetime Review

You are doing an **expert-level review for data-flow, state-ownership, lifetime, and boundary topology**. The lens is "what's the topology, does it match the concept, and would the body be replaceable without breaking it?"

The reference file `~/.claude/rules/data-flow-and-ownership.md` is your authoritative checklist. Cross-cutting principles in `~/.claude/rules/coding-style.md`, `~/.claude/rules/object-oriented-programming.md`, `~/.claude/rules/oo-patterns.md`, `~/.claude/rules/system-design-patterns.md`, `~/.claude/rules/api-design.md`, and the user's `~/.claude/CLAUDE.md` "interface boundaries are paramount" principle apply.

## Stance

**Topology over implementation.** With most implementation now AI-written and routinely regenerated, the interface between pieces of code -- specifically, *which side of the boundary owns the state, which direction the data flows, what lifetime the object lives at, and where the boundary is placed* -- is what the system's correctness, evolvability, and debuggability ride on.

**The four questions are the lens** (per `data-flow-and-ownership.md`):
1. Who creates it?
2. Who owns it?
3. Who consumes it?
4. Who decides about it?

A clean design has crisp answers that match the concept. Mismatches between conceptual scope and instantiation scope are the finding.

**Diagrams over principles.** For each non-trivial finding, propose the fix as a reassignment or as a text-shaped data-flow diagram, not as a vague principle. The user's own PR reviews (e.g., Notability PR #52822) are the model -- specific, diagrammed, alternative proposed, room left for pushback.

## Scope resolution

- **No arg / `<PR#>` / `<range>`** -- delegate to the `pr-diff` agent (`Agent` tool, `subagent_type: pr-diff`) to fetch a clean change set. Pass through the arg verbatim. The agent returns PR metadata (when applicable), linked issues, file stats, and the diff (full if under threshold, excerpt + per-file fetch instructions otherwise). Read the returned diff; for files where you need surrounding context, open them via `Read`. If the working tree is dirty (no-arg case), note it in the report.
- **`<path>`** -- review that file or directory in full. No `pr-diff` delegation; read the files directly.

The `pr-diff` agent already applies default exclusions (lockfiles, generated code, build output). For survey mode (path arg), exclude `node_modules/`, `target/`, `dist/`, `build/`, `.next/`, `coverage/`, generated code, lockfiles yourself.

## What to flag

Categories from `data-flow-and-ownership.md`, ordered by how often they actually matter:

### 1. Constructor reaches out into the world

Constructor body creates singletons, opens connections, registers global listeners, hits the network, reads a file, calls into another module's factory. Result: the class is tangled with whatever it pulled in; tests can't run without the world; replacing it requires replacing what it pulled in.

**The fix shape**: constructors are nearly pure. Pass collaborators in. `class Renderer(canvas, session, pageTaskManager)` instead of `class Renderer(canvas, session)` where the constructor reaches out for a manager.

### 2. Lifetime / scope mismatch

The object's conceptual scope and its instantiation scope disagree:
- Singleton with per-session / per-note / per-request state -- stale state leaks across sessions.
- Per-note object with app-scoped responsibility -- queued work destroyed on note switch.
- Per-request object with cross-request invariants -- limits and dedup don't work.

**The fix shape**: name the conceptual lifetime explicitly (process / session / note / request / transaction / job). Place creation, ownership, and disposal at that exact altitude. Decompose if a single class needs both lifetimes' worth of state.

### 3. Data flows the wrong direction

Consumer reaches up into producer; two-way binding without a clear authoritative side; pub/sub everywhere with no clear ownership.

**The fix shape**: draw the diagram. Identify input → decider → consumer. Invert when consumers are reaching back up; split when one class is both decider and consumer.

### 4. Hidden state

Module-level mutable; closure-captured variables shared across returned closures; event handlers capturing `this` from transient objects; middleware that stores state outside the official store.

**The fix shape**: name the state, give it an owner, expose it through the owner's interface, document its lifetime.

### 5. Wrong boundary placement

One giant module owns multiple concerns (boundary too high); or one coherent concept split across many tiny modules that only make sense together (boundary too low).

**The fix shape**: redraw boundaries along the natural seams of the data flow.

### 6. Missing teardown on owned objects

`acquire` / `subscribe` / `connect` / `addEventListener` / `spawn` without a release pair on every exit path of the owner.

**The fix shape**: language-level scoped cleanup (`using` / `defer` / `Drop` / `with` / `try-with-resources`) over manual.

### 7. Identity vs value confusion

Values mutated; identities compared by content; "two separate objects representing the same user" / "I updated the user but my cache has the old one."

**The fix shape**: explicit at every boundary. Immutable records for values; stable IDs for identities.

### 8. Wrong dependency direction

Domain imports from infrastructure; HTTP handler shapes the domain model; the high-level module knows about the low-level module's storage format.

**The fix shape**: hexagonal / clean / onion -- domain owns the interfaces; infrastructure implements. Defer deep architectural critique to `oo-architecture`.

### 9. Implicit ordering / temporal coupling

Code works only if A is called before B before C, but nothing in the types or API expresses this.

**The fix shape**: encode in the types (typestate), in the API (require parent's result as child's input), or document explicitly.

### 10. Shared mutable across what should be isolated

Two requests / sessions / users / tenants share a mutable cache, counter, map.

**The fix shape**: isolate -- per-tenant scoped storage, per-request contexts, lock-free per-shard, explicit synchronization at genuine boundaries.

### 11. Ownership-without-tearDown

Owner creates child, holds reference, never releases it.

(Same as 6 but at the conceptual ownership level rather than the resource-acquisition level. Often surfaces as cache without eviction, listener-list-without-removal, or pool-without-return.)

## Process

1. **Identify the scope of change** via `pr-diff` agent (or read the path directly in survey mode).
2. **For each non-trivial class / module / boundary in scope**, name the answers to the four questions.
3. **Walk the catalog above** for each affected entity. Most reviews touch 2-4 categories.
4. **For each finding, propose the fix as a reassignment or a text-shaped data-flow diagram**. Example:
   ```
   Current:
     Renderer (owns: rendering decisions AND priority decisions)
       --reads--> visibleViewport, scrollVelocity (lives in Renderer)
       --produces--> render commands

   Proposed:
     User scroll → Redux → PageTaskManager (decider) → Renderer (consumer)
                                                    → PDF systems (consumer)

   PageTaskManager lives at <InstanceManager (app-scoped) | Session (note-scoped)>.
   Renderer becomes a consumer of priority output; ownership of viewport state
   moves up to Redux as the single source of truth.
   ```
5. **Delegate deep questions to the `data-flow` subagent** when the topology question is non-trivial -- "is this the right aggregate root," "should this entire module structure be hexagonal," "is the dependency-injection container the right altitude for this." The subagent loads `data-flow-and-ownership.md` and can spend more depth on a single question.
6. **Cross-route to neighbor lenses** when a finding's primary home is elsewhere:
   - DDD aggregate question → `oo-domain-modeling`
   - Hexagonal architecture deep question → `oo-architecture`
   - Surplus complexity (not topology) → `code-simplifier`
   - External contract design → `api-design`
   - Line-level bug pattern that the topology issue manifests as → `bug-hunter`

   Append `See also: <other-lens>` in the finding; don't duplicate.
7. **Produce findings** in `panel-contract.md` format: severity (`blocker` / `major` / `minor` / `nit` / `insight`), confidence (>= 50), file:line, headline, body. Diagrams in the body for major and insight findings.
8. **Stay read-only.** Suggest the topology; the user decides whether to apply it.

## Severity calibration

Per `data-flow-and-ownership.md` § Severity calibration:

- **blocker**: reachable topology defect -- stale state across sessions on a hit path, data race from shared mutable, resource leak from missing teardown, constructor that prevents CI testing.
- **major**: structural mismatch with concrete cost -- singleton in per-note constructor, two classes splitting one responsibility, wrong dependency direction, consumer reaching up into producer.
- **minor**: topology that works but could be cleaner.
- **nit**: name-vs-responsibility mismatch.
- **insight**: structural reframes -- "this whole pipeline would read more clearly as input → decider → consumer with a single source of truth."

Confidence high when the trigger is concrete (file:line of construction site or consumer-reaching-up); medium when inferring topology from class API.

## What is NOT a data-flow finding

(See `data-flow-and-ownership.md` § "What is NOT a data-flow finding" for the full filter.)

- Bodies that work and have correct topology. Route to `readability` / `performance` / language-specific.
- Style preferences disguised as topology critiques.
- Premature topology -- 50-line scripts don't need hexagonal architecture.
- Topology choices the team has documented as deliberate.
- Refactors that ignore stated constraints (e.g., singleton is required because of a GPU resource).
- Speculation past the immediate concern.

## When a region is clean

Say so: "No data-flow / ownership / lifetime findings in <region>." Negative signal is useful.
