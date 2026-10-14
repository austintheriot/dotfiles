---
last-verified: 2026-10-14
sources:
  - bug-patterns.md
  - data-flow-and-ownership.md
  - api-design.md
  - naming-conventions.md
  - readability.md
  - simplification.md
  - security.md
  - performance.md
  - concurrency.md
  - debuggability.md
  - i18n.md
  - browser-spec.md
---

# Core Practices Digest

This file is a recognition list, not a reference. Each line names a defect shape and the fix in one sentence. The full rules files stay agent-only. When code you write or read matches a line here, read the source file named in that section, or dispatch the agent named there. Do not treat this list as complete.

When a source file changes, update the matching section here and the `last-verified` date. `create-expert-agent` must do this for every new or refreshed cross-cutting rules file.

## Bug shapes

Source: `bug-patterns.md`. Agent: `bug-hunter`.

- **TOCTOU**: a check and the action that depends on it are separate steps. Ask what changes between them. Use compare-and-swap, a transaction, or let the action validate its own precondition.
- **Lost update**: read-modify-write on shared state without atomicity. One writer's change disappears.
- **Await is a yield point**: every `await` can reorder, cancel, or interleave. Do not hold a lock across it. Ask what happens if the future is dropped there.
- **Fire-and-forget**: a spawned task with no held handle loses its errors. Give every task an owner scope.
- **Sync inside async**: blocking IO or `block_on` inside an async runtime stalls every task on that worker.
- **Cache triad**: every `get / compute / set` needs an answer for invalidation, stampede, size bound, negative caching, and key completeness. A key that omits a parameter leaks data across users.
- **Sticky marker**: an effect that must run once per event reads a flag that stays set. The effect fires again on the next mount. Key the effect on the event id, not on flag presence. A regression test must set the marker before mount.
- **Optionality**: every `?.`, `!`, `as`, `unwrap`, `?? 0`, and "never null" comment can hide a real absent case. `?? 0` turns "no data" into a valid zero.
- **Numbers**: allocation sizes multiplied without overflow checks, truncating casts, money in floats, signed and unsigned compared, off-by-one bounds.
- **Leaks**: every acquire, open, connect, subscribe, and spawn needs a release on every exit path, including error, panic, and cancellation.
- **Aliasing**: a returned internal collection lets callers break invariants. A Python mutable default argument is shared across calls.
- **Swallowed errors**: empty catch, log-and-continue, catch of the base exception type, rethrow without context. Errors inside a 200 body. Partial batch failure reported as success.
- **Bad retries**: retry on a 4xx, retry without backoff, retry without an idempotency key.
- **Time**: naive local timestamps, DST gaps and repeats, `now + 30 days` treated as "next month", wall clock used for durations, `Date.now()` inside a function that claims purity. Inject the clock.
- **Boundaries**: test `[]`, one item, max size, zero, negative, NaN, `MIN_VALUE`, and special characters (NUL, BOM, RTL override, zero-width joiner).
- **Abstraction leaks**: `-1` as a not-found sentinel, iteration order the API does not promise, reference equality where value equality is meant, a hidden network call inside a cheap-looking getter.

## Data flow and ownership

Source: `data-flow-and-ownership.md`. Agent: `data-flow`.

For each non-trivial entity, answer four questions: who creates it, who owns it, who consumes it, and who decides about it.

- **Constructor reaches into the world**: a constructor that calls a singleton, opens a connection, subscribes to a bus, or starts async work. Pass the dependency in.
- **Lifetime mismatch**: a process singleton holds per-note or per-request state, or a per-request object holds cross-request counters. Name the lifetime and create the object at that altitude.
- **Wrong flow direction**: a consumer reaches up into its producer for decisions. Inputs feed a decider, and the decider pushes to consumers.
- **Hidden state**: module-level `let` with a setter, or state captured in a closure. Give the state a named owner and a lifetime.
- **Identity versus value**: comparing identity by content, or mutating a value type. Use immutable values and stable ids.
- **Temporal coupling**: A must run before B, and nothing in the types says so. Encode the order in the types or require the earlier step's result as input.
- **Owner without teardown**: the owner creates a child and never releases it. Most effect-cleanup and listener leaks have this shape.
- **Dependency points outward**: domain code imports infrastructure, or an HTTP handler shapes the domain model. Domain owns the interfaces, and infrastructure implements them.

## Contracts at a boundary

Source: `api-design.md`, `naming-conventions.md`. Agents: `api-design`, `naming-conventions`.

- **Hyrum's Law**: all observable behavior becomes the contract, including order, timing, and error text.
- **Boolean pile**: `is_active`, `is_deleted`, `is_archived` drift out of sync. Use one status enum.
- **Absence semantics**: `null`, missing, empty string, and zero must each have one defined meaning.
- **Top-level array response**: `[...]` cannot gain pagination or metadata later. Return `{ data: [...] }`.
- **Inconsistent error envelopes**: validation, auth, and server errors must share one shape with a machine code, a human message, and a remediation.
- **Offset pagination** on data that changes. Use a cursor. Apply filters before pagination.
- **Mutation through GET**: caches and prefetchers will trigger it.
- **Unitless number at a boundary**: `timeout: 30` in JSON, YAML, env, or a column. Put the unit in the name (`timeoutMs`) or use a typed duration.
- **Case mismatch at a JSON boundary**: `userID` versus `userId` decodes to a silent `undefined`. One spelling per boundary, enforced by a schema.
- **Renamed field, metric, or export**: old consumers drop data or dashboards go empty with no error. Treat a rename at a boundary as a breaking change.
- **Unsafe absent default**: a boolean such as `enableFraudCheck` that old clients omit. Name and default it so that absent means safe.

## Names and shape

Source: `readability.md`, `naming-conventions.md`. Agents: `readability`, `naming-conventions`.

- **One term per concept**: `user`, `account`, and `member` for one entity, or `fetch`, `retrieve`, and `get` for one action. Pick one per bounded context.
- **Lying names**: `get` that does IO or mutates, `is` that returns a non-boolean, `validate` that returns nothing, a singular name on a collection, `userList` that holds a Set.
- **Verb lexicon**: `get` is cheap and pure, `fetch` does IO, `load` can be slow or warm a cache.
- **Flag parameter**: `render(item, isPreview)` is often two functions. Three or more booleans at a call site need an options object or an enum.
- **Altitude**: one function mixes orchestration with byte-level work. Extract the low-level part to its own altitude.
- **Length**: a function is as long as the one thing it does. A one-call helper extracted only because the function got long makes reading worse.

## Surplus complexity

Source: `simplification.md`. Agent: `code-simplifier`.

- **Single-implementation abstraction**: an interface with one implementation, outside a real boundary or test seam. Go concrete first, and abstract on the second use.
- **Pass-through layer**: a layer that forwards with no added concern. If you cannot name the concern, delete the layer.
- **Premature configuration**: parameters, flags, or env vars that every caller sets to the same value.
- **Unnecessary state**: a field that a computation over other fields can derive.
- **Rule of three**: tolerate two similar blocks. Extract on the third, and only when the blocks have the same reason to change.
- **Structure the change made obsolete**: after a diff, look at the code around the change. A helper can now have one caller. A parameter can have one value, a branch can be unreachable, or a name can no longer fit.

## Security

Source: `security.md`. Agent: `security`.

- **Every external input is hostile**: user-supplied URL to `fetch` (SSRF), path to `open` (path traversal), string into SQL, shell, HTML, or a template (injection). Parameterize at every trust boundary.
- **Authorization per operation**: check access on the specific resource, not once per session. `/users/123` must reject user 456.
- **Fail closed**: when an auth check is uncertain, deny.
- **Secrets**: never in source, logs, URLs, argv, error pages, bundles, or sourcemaps. Compare tokens in constant time.
- **Supply chain**: a lockfile diff that is only patch bumps, outside a dependency PR, is a delivery channel. A new `build.rs`, proc-macro, or `postinstall` is code that runs on every contributor's machine.

## Performance

Source: `performance.md`. Agent: `performance`.

- **Hidden quadratic**: `.includes`, `.find`, or `.indexOf` inside a loop over the same data. Build a `Map` or `Set` once outside the loop.
- **N+1 queries** and sequential awaits where the calls are independent.
- **Hot-loop waste**: a regex compiled inside the loop, string concatenation in a loop, `JSON.parse(JSON.stringify(...))` as a clone.
- **Measure first**: a finding without evidence that the path is hot is a hypothesis. Record absolute values and the baseline, not only deltas.

## Concurrency

Source: `concurrency.md`. Agent: `concurrency` (Rust: `rust-async`).

- **Data-race-free or nothing**: any unsynchronized access to shared mutable state is a defect, including reads, and including on x86.
- **Structured concurrency**: every task lives in a scope that waits for children, propagates cancellation down, and propagates errors up.
- **Cancellation is contract**: release locks and resources on cancel. A `select!` branch that mutates state across awaits is not cancel-safe.
- **Locks before lock-free**: lock-free code needs a measured contention problem and a memory-reclamation plan.

## Debuggability

Source: `debuggability.md`. Agent: `debuggability`.

- **Errors carry context**: the input value, the operation, and the cause. `throw new Error("invalid input")` gives none of the three.
- **Reachable panics**: an `unwrap` or `!` on a path from external input needs a message that names the broken invariant.
- **Reproducibility**: hardcoded `new Date()`, `Math.random()`, and unmocked IO block local reproduction. Inject them.

## Text and locale

Source: `i18n.md`, `text-engineering.md`. Agents: `i18n`, `text-engineering`.

- **Four lengths**: bytes, code units, code points, and grapheme clusters differ. `.length` and byte truncation split characters.
- **Locale-aware operations**: case folding (Turkish `i`), collation, plurals (Polish has four forms), and number and date formats need locale APIs, not string methods.
- **No concatenated UI strings**: word order changes between languages. Use a message format with placeholders.
- **Future events** need an IANA zone, not only a UTC instant, because zone rules change.

## Browser platform

Source: `browser-spec.md`. Agent: `browser-spec`.

- **Platform first**: before you hand-roll a modal, focus trap, observer, URL parser, query-string parser, formatter, or event emitter, use `<dialog>`, Popover, `IntersectionObserver`, `ResizeObserver`, `URL`, `URLSearchParams`, `Intl`, or `EventTarget`.
- **One `AbortController`** for listener and fetch cleanup instead of arrays of teardown callbacks.
- **Scroll and touch listeners** that do not call `preventDefault` must be `passive`.
- **Text input**: check `isComposing` before acting on Enter, or IME users submit half-typed text.
