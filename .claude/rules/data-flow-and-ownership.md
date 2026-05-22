# Data Flow, Ownership, and Lifetime

A reference for evaluating code from the **data-flow / state-ownership / lifetime** lens during planning, writing, and review. Used by the `data-flow` subagent. Companion to `coding-style.md` (push effects to edges), `object-oriented-programming.md` (entities vs value objects, encapsulation, DDD aggregates), `oo-patterns.md` (hexagonal / ports-and-adapters, repository, DI), `api-design.md` (Bloch / Hyrum's Law -- contracts are forever), `simplification.md` (premature abstraction, hidden state), and `system-design-patterns.md` (systems of record vs derived data, identity vs value).

The core thesis: **at every altitude -- function, module, class, service, system -- the same four questions determine whether the code is correct, evolvable, and debuggable: who creates this, who owns it, who consumes it, who decides about it.** Mismatches between conceptual scope and instantiation scope are bugs. Misaligned ownership produces compounding pain. The implementation can be replaced; the *direction of data flow*, the *lifetime story*, and the *ownership topology* are what survive replacements and what callers couple to.

This lens is especially load-bearing in a world where the implementation body is increasingly AI-written and routinely regenerated. If the interface boundary is paramount (per the user's CLAUDE.md), then *what's on each side of the boundary, and which direction the dependency points* is the meat of interface design.

The operational thesis: **draw the data-flow / ownership / lifetime diagram before reviewing the body.** Most "this code is hard to follow" complaints decompose into one of: (a) the wrong object owns this state, (b) the wrong object creates this dependency, (c) the lifetime of object X doesn't match its conceptual scope, (d) the data flows in the wrong direction (consumer reaches up into producer to fetch instead of producer pushing to consumer), (e) the boundary is in the wrong place. The body is rarely the problem; the topology is.

---

## The four questions

For every non-trivial piece of state, dependency, or boundary in the code under review, name:

1. **Who creates it?** (Construction site. Whose constructor / factory / initializer runs?)
2. **Who owns it?** (Who holds the canonical reference; whose lifetime governs its lifetime; who is responsible for its disposal / teardown / reset.)
3. **Who consumes it?** (Who reads from it, calls methods on it, depends on its behavior.)
4. **Who decides about it?** (Who sets policy: when to act, what to update, what the next state is. Distinct from "who consumes" -- a UI consumes state but a reducer / store / domain object decides about it.)

A clean design has these four answers crisp, and the answers *match the concept*:

- The user's scroll position is created by the UI, owned by Redux (because it lives across renders), consumed by the priority-queue manager (to decide what to render next), and decisions about it are made by Redux reducers from user input.
- A note's render state is created at note-open, owned by the per-note `Session` (because its lifetime is the note's lifetime), consumed by `Renderer`, decisions by user input + scroll position.
- A `PageTaskManager` priority queue: created and owned by ??? -- this is exactly the question the user's review on Notability PR #52822 asked. If it's app-singleton (lifetime = app lifetime), it lives in `InstanceManager`. If it's per-note (lifetime = note's lifetime), it lives in `Session`. The bug is when it's a singleton conceptually but created inside the constructor of a per-note `Renderer` -- the lifetime claim and the construction site disagree.

**Flag**: any code under review whose answers to the four questions disagree with each other, or whose construction site reveals a different scope than the conceptual lifetime would imply.

---

## Anti-patterns (the high-yield catalog)

### 1. Constructor reaches out into the world

The constructor of class A creates a singleton, opens a connection, registers a global listener, hits the network, reads a file, calls into another module's factory. Result: A's tests cannot run without the world; A's lifecycle is tangled with that singleton's; replacing A requires replacing what it pulled in.

**Right shape**: constructors are nearly pure. They accept their dependencies (passed in by the caller), assign fields, validate invariants. They do not *create* their collaborators; they *receive* them. ("Write the call site before the body" applies at construction too.)

**The user's review comment, verbatim** (Notability PR #52822): "I typically think of constructors as being fairly pure and/or scoped mostly to just the initialization they need to do inside themselves." Exact statement of this anti-pattern.

**Specific shapes to flag**:
- Constructor body that calls `SomeSingleton.getInstance()` and assigns the result to a field.
- Constructor that calls `new SomeOtherService()` instead of accepting it as a parameter.
- Constructor that subscribes to a global event bus, registers a callback, kicks off async work.
- React component / Composable / SwiftUI view that creates a ViewModel / store inline at the first render and survives subsequent renders only by coincidence (memo, key tricks).

**Fix shape**: pass the dependency in. `class Renderer(canvas, session, pageTaskManager)` instead of `class Renderer(canvas, session)` where the constructor reaches out for the manager.

### 2. Lifetime / scope mismatch

The object's conceptual scope (when it should exist) does not match its instantiation scope (when it actually exists). Three sub-shapes:

- **Singleton with note-scoped state**: a long-lived process-global object accumulates state that's only meaningful for one note / session / request. Stale state leaks across sessions; the developer fights it forever with `clear()` / `reset()` calls that get forgotten.
- **Per-note object with app-scoped responsibility**: a per-note class holds work that should outlive the note. Switching notes destroys queued work; the system rebuilds expensive state on every note change.
- **Per-request object with cross-request invariants**: a request-scoped object holds rate-limit counters / dedup keys / idempotency tokens; the limits and dedup don't work because each request gets fresh state.

**The user's review comment**: "This class feels like something that either would make sense to be part of the `InstanceManager` if really is intended to be a singleton, or moved to per-note setup, like the session & Renderer if it's intended as a per-note thing. Not having to track if stale note values/calls are hanging on is a really freeing thing if it would make sense in this context."

**Fix shape**: name the conceptual lifetime explicitly (process / session / note / request / transaction / job). Place the object's creation, ownership, and disposal at that exact altitude. If a single class needs *both* lifetimes' worth of state, decompose it -- pull the long-lived part up, push the short-lived part down.

### 3. Data flows the wrong direction

The clean shape: state flows downward from the source of truth; events flow upward from the user; the source of truth is the single decider. The broken shapes:

- **Consumer reaches up into producer.** `Renderer.getVisibleViewport()` instead of "the priority queue manager tells the Renderer what to render." The Renderer becomes responsible for *both* rendering and tracking what to render -- two responsibilities, no separation, hard to test either independently.
- **Two-way binding without a clear authoritative side.** State sometimes propagates parent-to-child, sometimes child-to-parent; whichever changed last wins. Reproduces the classic React "lift state up" lesson at every altitude.
- **Pub/sub everywhere.** Every component subscribes to every event; the data-flow diagram is a full mesh; nobody can answer "who owns this" because the answer is "everybody and nobody."

**The user's diagram, verbatim**:
```
User scroll --> Redux ---> Priority queue manager class --> Renderer
                                                       |
                                                       --> PDF systems
```

The lesson: **inputs feed a decider; deciders feed consumers; consumers consume.** Renderer is a consumer of priority decisions, not the owner of the priority logic. The PDF systems are also consumers. The priority-queue manager is the decider. The user's scroll is the input.

**Fix shape**: draw the diagram. Identify input → decider → consumer. If consumers are reaching back up the chain, invert: make the decider push to consumers, not the other way around. If a class has both decider responsibilities and consumer responsibilities, split it.

### 4. Hidden state via closure / global / module mutable

State lives somewhere that doesn't appear in any class field or function parameter. The developer can't find "where is this set," and tools can't help (no `Find All References` on a closure-captured variable).

**Specific shapes**:
- Module-level `let x = ...; export function setX(...) { x = ... }`. The module is now a singleton holding mutable state with no lifecycle.
- A factory function returning closures over a captured variable. The captured variable is hidden state shared across all returned closures.
- A long-lived event handler that captures `this` from a transient object. The transient object can't be GC'd until the handler is removed -- which it usually isn't.
- Redux middleware that stores state outside the Redux store ("I'll just keep a `Map<string, X>` here").

**Fix shape**: name the state, give it an owner, expose it through the owner's interface, document its lifetime. If it's truly process-global and immutable after init, mark it so. If it's mutable, it has an owner; find or create one.

### 5. Identity vs value confusion (Hickey)

Two questions get conflated:
- **Identity**: "is this the same user / session / connection across these two points in time?" Identity is about *which*.
- **Value**: "is this the same content?" Value is about *what*.

When code treats values as identities, you get "two separate objects representing the same user" bugs. When code treats identities as values, you get "I modified the user but my cache still has the old one" bugs.

**Fix shape**: be explicit at every boundary which you mean. Use immutable value types (records, dataclasses, structs) for values; use references with stable IDs for identities. Don't bolt mutation onto value types and don't compare identity by content.

(See `~/.claude/rules/system-design-patterns.md` § Values vs Places / DDD entities vs value objects for the deeper treatment.)

### 6. Wrong boundary placement

The system's modules / classes / services don't match the conceptual boundaries of the problem. Two sub-shapes:

- **Boundary too high**: one giant module owns multiple concerns. Internal cohesion is low. Changes ripple. Nobody can describe "what this module is for" in one sentence.
- **Boundary too low**: a coherent concept is split across many tiny modules that only make sense together. Trace the data flow and you find yourself jumping across 6 files to follow one operation.

**Conway's Law applied locally**: if three different teams (or three different mental models within your head) work on three different files to make one feature work, the boundary is in the wrong place. Either the feature should be one module (boundary too low) or the teams should be one team (boundary correct, org wrong).

**Fix shape**: redraw boundaries along the natural seams of the data flow. The shapes that survive are the ones where each module owns a coherent slice of the input → decide → output chain.

### 7. Implicit ordering / temporal coupling

The code works only if A is called before B before C, but nothing in the types or the API expresses this. New developers (and AI-generated code) hit the trap regularly.

**Fix shape**: encode the ordering in the types (typestate -- `BuilderUninitialized` → `BuilderConfigured` → `BuilderBuilt`), in the API (require the parent step's result as input to the child step), or document explicitly. "Make illegal states unrepresentable" applies to *temporal* states too.

### 8. Shared mutable state across what should be isolated

Two requests / sessions / users / tenants share a mutable cache, a mutable counter, a mutable map. Race conditions appear under load; "works in dev, breaks in prod" is the signature.

**Fix shape**: isolate. Per-tenant scoped storage; per-request scoped contexts; lock-free per-shard structures; explicit synchronization at the boundary if cross-boundary state is genuinely required.

(See `~/.claude/rules/concurrency.md` for the deep treatment; this lens flags the *design choice* of allowing the sharing.)

### 9. Ownership-without-tearDown

An owner creates a child, holds a reference, and never releases it. The child's lifetime should be ≤ the owner's, but the owner doesn't run cleanup. Most React `useEffect` cleanup bugs are this. Most "we leaked an event listener" bugs are this. Most "the connection pool is full" bugs are this.

**Fix shape**: every `acquire` / `subscribe` / `connect` / `addEventListener` / `spawn` deserves a release pair, on every exit path of the owner. Prefer language-level scoped cleanup (`using` / `defer` / `Drop` / `with` / `try-with-resources`) over manual.

### 10. Hidden dependency direction

The dependency points the wrong way. Domain code imports from infrastructure code. The "high-level" module knows about the "low-level" module's storage format. The HTTP handler shapes the domain model.

**Fix shape** (hexagonal / clean / onion architectures): domain owns the interfaces it needs; infrastructure implements them. Domain knows nothing about HTTP / DB / queues. Code-level: `import` graph points inward; never outward. (See `~/.claude/rules/oo-patterns.md` § Hexagonal Architecture.)

---

## The review heuristic

**Before reading the body of any non-trivial change or class, ask:**

1. What pieces of state / dependencies / boundaries are in scope?
2. For each, what's the answer to the four questions (create, own, consume, decide)?
3. Does the construction site match the conceptual scope?
4. Which direction does the data flow? Are consumers consuming, or are they reaching back up the chain?
5. Where are the boundaries? Do they match the natural seams of the data flow?

If you can't answer these crisply from the code, **that's the finding** -- the topology isn't legible. Either the code is hard to read, or the topology genuinely is wrong.

**For each candidate finding, state the fix as a diagram or as a reassignment**, not as a vague principle:
- Bad: "Renderer has too many responsibilities."
- Good: "Renderer owns rendering decisions AND priority decisions. Split: priority decisions go to a PageTaskManager that lives at the InstanceManager (singleton) or Session (per-note) altitude. Renderer becomes a consumer of priority output. Data flow: user scroll → Redux → PageTaskManager → Renderer / PDF systems."

The user's PR review comments are the model. Specific, diagrammed, alternative proposed, room left for pushback.

---

## When to apply this lens

**Always at planning time**: every spec, ADR, RFC, design doc. Before any code is written, the topology should be crisp. The cheapest place to fix a wrong boundary is in the doc.

**Always at non-trivial review time**: any change that adds a class, adds a module, changes a module's responsibilities, introduces a new long-lived object, adds a new boundary, crosses an existing boundary. Bug fixes inside a single function are usually out of scope; anything that touches who-owns-what is in scope.

**At implementation time**: when writing new code, draw the topology on a notepad / in a comment / in the spec before starting. Identify the natural construction site, owner, lifetime. The body is easier to write when the topology is right.

---

## What is NOT a data-flow finding

The dual lens matters; without it, this becomes "every class is wrong."

- **Bodies that work and have correct topology**: the implementation may be ugly, slow, or unidiomatic, but if the construction site, owner, lifetime, and direction are right, this lens doesn't fire. Route to `readability` / `performance` / language-specific.
- **Style preferences disguised as topology critiques**: "I'd structure this with classes instead of functions" without a concrete topology benefit is preference, not finding.
- **Premature topology**: a 50-line script does not need hexagonal architecture, ports, adapters, or four layers. Match the complexity to the system.
- **Topology choices the team has made deliberately**: if the project's docs say "we use a global event bus," the topology critique is to argue the meta-decision, not flag every consumer.
- **Refactors that ignore stated constraints**: if the spec says "must be a singleton because of the GPU resource," reorganizing it into a per-note thing ignores the constraint.
- **Speculation past the immediate concern**: "if you ever needed three of these..." -- premature. Reach for it only when the second instance is concretely on the way.

---

## Severity calibration

Using `~/.claude/rules/panel-contract.md`'s rubric:

- **blocker**: ownership / lifetime defects with reachable trigger -- stale state across sessions on a path the user actually hits; data race from shared mutable state; resource leak from missing teardown on owned object; constructor reaching into the world in a way that makes the class untestable in CI.
- **major**: structural mismatch with concrete cost -- singleton inside per-note constructor (the Notability PR #52822 shape); two classes splitting one responsibility, or one class holding two; wrong dependency direction (domain depending on infrastructure); consumer reaching up into producer where the data flow should be inverted; hidden state where module-mutable should be class-owned.
- **minor**: topology that works but could be cleaner -- a class that's grown a second responsibility but isn't broken yet; a boundary that's the wrong level of abstraction but the cost is small.
- **nit**: name-of-class vs class's actual responsibility mismatch; doc-comment that describes a different ownership than the code expresses.
- **insight**: structural reframes -- "this whole pipeline would read more clearly as input → decider → consumer with a single source of truth at the decider"; "consider DDD aggregates here"; "the boundary between Renderer and PageTaskManager would land more naturally if pulled to <X>."

Confidence is high when the trigger is concrete (the constructor at file:line creates a singleton with note-scoped state that survives note-close); medium when the topology is inferred from a class API (the agent reads the public methods and the dependency direction).

---

## Process for the data-flow agent

1. **Identify the scope of the change**: which classes / modules / boundaries are touched.
2. **For each affected entity, name the four questions' answers** (create / own / consume / decide). If you cannot, the topology is illegible -- that itself is a finding.
3. **Cross-check construction site vs conceptual lifetime**. Singleton in per-note constructor? Per-note in app-singleton holder? Flag the mismatch.
4. **Trace the data flow**. Draw the diagram (text-shaped: A → B → C). Where do consumers reach back up? Where does state propagate in two directions without a single source of truth?
5. **Check dependency direction**. Domain importing from infrastructure? UI importing from internal storage details? High-level module depending on a low-level module's shape?
6. **Identify boundary candidates**: where would natural seams in the data flow place the module boundary, and does the code respect them?
7. **For findings, propose the fix as a reassignment** (move X to Y, pass Z in as a parameter, invert the data flow at the W boundary), not as a principle.
8. **Defer to other lenses** when the finding has a closer home: API contract shape → `api-design`; concrete bug pattern → `bug-hunter`; abstract complexity → `code-simplifier`; DDD-aggregate-flavored questions → `oo-domain-modeling`; hexagonal-architecture-flavored questions → `oo-architecture`. This agent's value is the *topology* lens; the others overlap but are not duplicates.
9. **Stay read-only.** Suggest the topology; the user decides whether to apply it.
