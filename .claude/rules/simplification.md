---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Simplification Principles

A reference for surfacing simplification opportunities during review. Used by the `code-simplifier` subagent. This file is **review-oriented** -- it describes what to flag, not how to rewrite. `/expert-review` is read-only by design; the user decides which suggestions to apply.

The core thesis: every line of code is a liability. The best code is the code that doesn't exist. But this principle has a sharp limit -- *over-simplification* (cleverness, density, abstraction collapse) is itself a bug source. The reviewer's job is to find the surplus complexity that earns no value, while recognizing when complexity is load-bearing.

Companion to `~/.claude/rules/coding-style.md` (function shape, parse-don't-validate, architecture principles) and `~/.claude/rules/object-oriented-programming.md` (when ceremony is justified). This file is the **delete more code** lens.

---

## What surplus complexity looks like

### Single-implementation abstractions

An interface, trait, abstract class, or strategy pattern with exactly one implementation. Premature generalization. The abstraction's cost (extra file, extra type, extra indirection in the IDE jump) buys flexibility that has not been needed. Flag with a "concrete-first, abstract on second use" framing.

**Common shapes**: `IUserService` with one `UserService`; a strategy pattern where only one strategy has ever shipped; a factory whose only call site uses one concrete type; a sealed hierarchy with one variant; generic types parameterized at one call site.

**When not to flag**: the abstraction is at a system boundary (port-and-adapter, public API surface, test-double seam) where the second implementation is the test or the mock. Or the project explicitly preserves abstractions for known-imminent multi-impl needs.

### Pass-through layers

A function, class, or module whose only job is to forward calls to another layer with no transformation, no validation, no behavior. Indirection without value. Each pass-through adds a hop in the call graph, a name to remember, a place where a stale comment can lie.

**Common shapes**: `class UserController { constructor(service) {} getUser(id) { return service.getUser(id); } }`; a "facade" that exposes every underlying method 1:1; a wrapper function that just `return inner(...args)`s.

**When not to flag**: the layer adds a real concern -- request validation, response shaping, authorization, logging, tracing, telemetry, type adaptation across an API boundary, dependency-direction enforcement (hexagonal architecture). The bar: name the concern. If you cannot name it, the layer is surplus.

### Premature configuration

Function parameters, class fields, environment variables, feature flags, or settings that are read but never varied. Designed-for-flexibility that is not flexed. Configuration points are an unmaintained surface: each one is a possible value the test matrix should cover, and each is a possible production misconfiguration.

**Common shapes**: a function with 5 parameters where 3 are always passed the same constant; a class with `enableX`, `enableY`, `enableZ` flags always set to true at every call site; an env var read by code but never set by deploy config.

**When not to flag**: the configurability is at a tested seam (test injects a different value), or there is a near-term plan to vary it documented in the surrounding code or ticket.

### Dead code

Unused variables, parameters, imports, methods, branches, types. The compiler / linter catches most of this; what the simplifier catches is the **semantically** dead code: methods that exist but are never called outside their own tests, types referenced only by `Unknown` casts, `if` branches with conditions that cannot fire in practice, log statements at levels that are never enabled.

**Common shapes**: `default:` branch in a switch on a closed sum that exhaustively covers cases; `if (false)` guarding "preserved for later" code; functions kept around because "we might need them"; commented-out blocks.

**When not to flag**: the code is reachable from a public API contract (even if not currently called from this repo). Or the code is genuinely a placeholder for an imminent feature.

### Excessive nesting

Code structured as if-pyramids, nested ternaries, deeply chained method calls without intermediate naming, callback hell. Each level of nesting roughly doubles the reader's working-memory load. Refactor with early return (guard clauses), extracted helpers, sum-type pattern matching, or pipeline-style transformation.

**Common shapes**: `if (...) { if (...) { if (...) { return doIt(); } else { ... } } else { ... } }`; nested ternaries `a ? b ? c : d : e ? f : g`; `.then(.then(.then(.then(...))))` Promise pyramids.

**When not to flag**: the nesting reflects the inherent structure of the problem (tree-shaped data, recursive algorithm, decision tree of irreducible cases).

### Code duplication that has earned an abstraction

The DRY-versus-WET pragmatic line: two near-identical blocks are tolerable; three is suspicious; four is almost always abstractable. **But only when the pattern is genuinely the same.** Rule of three, not rule of two: extract on the third instance, not the second, because the second is when you're still discovering whether the resemblance is accidental.

**Common shapes**: three places computing the same `(value - min) / (max - min)` normalization; four places parsing a date in the same idiosyncratic format; five places mapping `User` to `UserDTO` with the same field projections.

**When not to flag**: the duplicates are at different abstraction levels or in different domains; extracting them would couple unrelated modules. Or the duplicates serve different evolution paths (the team plans to change one but not the others).

### Unnecessary state

State that could be derived, state with duplicated sources of truth, state that exists only for caching when the underlying compute is cheap. State is the most expensive thing in a program -- every state slot is a degree of freedom the system can wander into.

**Common shapes**: a field that mirrors a computation over other fields (`isEmpty` alongside `items` -- just use `items.length === 0`); a cached `totalCount` updated alongside every `add`/`remove`; a `lastModified` field updated alongside every mutation; "two copies of the same fact" in different parts of the data model.

**When not to flag**: the cached field is genuinely hot-path (measured), or it serves an external API that needs O(1) access without invariant fragility.

### Cleverness

Code that's correct, terse, and confusing. Bitfield encodings of state, point-free composition chains, regex puzzles, deeply chained type-level computations. The bar: if reading the line takes more time than reading three equivalent obvious lines, prefer the obvious version.

**Common shapes**: `(n & (n - 1)) === 0` (power-of-two check) without a comment; `arr.reduce((acc, x) => ({...acc, [x.key]: x}), {})` instead of an explicit loop or `Object.fromEntries(arr.map(...))`; regex doing what a parser should do.

**When not to flag**: the clever form is a recognized idiom in the language's ecosystem (Haskell point-free, Rust iterator chains, Pandas vectorization). Or the obvious form is genuinely slower in a measured hot path.

### Misplaced abstraction level

A function or class operates at the wrong altitude -- mixing low-level details with high-level orchestration, or splitting a single concept across many tiny functions that only make sense together.

**Common shapes**: a request handler that opens database connections inline; a "manager" class that delegates every method 1:1 to other managers; a function decomposed into six helpers that each call exactly one of the others.

**When not to flag**: the layering exists for testability or for genuine reuse from multiple call sites.

---

## What is NOT a simplification opportunity

The dual lens matters as much as the primary one. Reviewers who flag every "this could be shorter" become noise.

- **Verbosity that aids clarity.** Explicit return types on top-level functions, named intermediate variables, type annotations on tricky inference points, exhaustive `match`/`switch` cases even when redundant. These cost lines but pay in readability and refactor safety.
- **Code that is duplicated but evolving differently.** Two `User.toDTO()` and `User.toAPIResponse()` methods may look identical today but represent different output contracts; merging them couples the contracts and forces every API change to also be a DTO change.
- **Defensive programming at trust boundaries.** Validation, escaping, redaction at the edge of the system is not surplus. The "extra" check between trusted internal code is.
- **Documented complexity.** A comment explaining *why* the code is the way it is justifies the form. The comment is itself the documentation that the simpler-looking alternative was considered and rejected.
- **Test code.** Tests have different optimization criteria -- explicit setup, repeated arrangement, named cases beat clever helpers. See `~/.claude/rules/testing.md`.

---

## Severity calibration

The `code-simplifier` produces findings at these severity levels (matching `/expert-review`'s scale):

- **major**: substantial complexity that costs real understanding time. Single-implementation abstractions, pass-through layers, premature config, excessive nesting, unused state. Removable without behavior change.
- **minor**: noticeable opportunity but not load-bearing. Modest duplication ripe for extraction, intermediate variables that would aid readability, dead branches in cold paths.
- **nit**: cosmetic. Names that could be shorter (or longer), comment-vs-code redundancy, formatting if not handled by the formatter.
- **insight**: a deeper structural simplification -- "this whole module could be a single function," "this hierarchy is a sum type in disguise," "this state machine reduces to two states." Not a blocker; a refactor worth a conversation.

Confidence is independent of severity: a high-confidence nit is "this duplication is real and removable;" a low-confidence major is "this looks like a pass-through but I cannot verify there's no validation step in the inheritance chain."

---

## Process for the simplifier agent

1. Read each code region. For each region, classify any complexity smells using the categories above.
2. For each candidate finding, apply the "what is NOT a simplification opportunity" filter. When in doubt, omit -- the agent's value is signal-to-noise, not coverage.
3. State the simpler form concretely. "Could be simpler" is not a finding; "this 30-line method is three calls to library X" is.
4. Acknowledge tradeoffs honestly. "Removing this abstraction couples A to B" lets the user decide.
5. Stay read-only. Suggest; never apply.
