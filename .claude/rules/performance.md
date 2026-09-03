---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Performance Review Principles

A reference for evaluating code from a performance lens during review. Used by the `performance` subagent. Distinct from `simplification.md` (surplus complexity), `bug-patterns.md` (correctness), and `distsys-runtime.md` (distributed-system tail latency and queue dynamics).

The core thesis: most performance problems are not micro-optimization opportunities; they are **wrong algorithm**, **wrong data structure**, **wrong work cadence**, or **wrong place to do the work**. The reviewer's value is recognizing those shapes, anchored to specific call sites.

Casey Muratori's framing applies: every line of code has a cost, but the cost that matters is the cost *on the critical path*. Hot code is cheap to optimize, cold code is expensive to even read. Spend depth where it matters.

Mitchell Hashimoto's "What's in a Production Web App" perspective also applies: the dominant performance concerns in real applications are I/O patterns (N+1, sync-in-async, blocking the event loop) and rendering patterns (re-renders, work in render, bundle size), not raw CPU.

---

## Algorithmic complexity

### Common shapes
- **Quadratic on a list that grows**: `for x in xs: for y in xs: ...` -- fine at N=10, breaks at N=10000. Often hidden behind `.includes()` / `.contains()` / `Array.indexOf()` inside a loop, which makes a quadratic algorithm look linear.
- **Repeated `O(n)` lookups**: `arr.find(matcher)` in a loop instead of building a `Map` once outside.
- **Repeated regex compilation**: regex created inside the hot loop rather than module top-level.
- **String concatenation in a loop in languages where strings are immutable**: O(n²) growth. JavaScript, Python (sort of -- CPython interns), Java pre-StringBuilder. Use buffers.
- **Sorting to compare**: `sort(a) == sort(b)` is O(n log n) when a multiset comparison is O(n) with a hashmap.
- **Recomputation that could be memoized**: especially when the same key is requested many times in a render or request.
- **Unnecessary deep clones**: `JSON.parse(JSON.stringify(obj))` on a hot path; `structuredClone()` of a large tree per call.

### Defenses
- Hashmap / set membership over linear search.
- Pre-process once outside the loop.
- Memoize, but only when the cost of the cache lookup is less than the recomputation.
- For real-time-shaped work, consider data-oriented layouts (struct of arrays) and contiguous storage.

### Review heuristic
For every loop, ask: what does the inner operation cost? Is it O(1) amortized, or is it secretly O(n)? Count the nesting depth -- two nested O(n) operations on the same data is O(n²).

---

## I/O patterns

### N+1 queries
The single most common performance defect in web apps. A list of N entities each triggers one more query.

- ORM lazy-loading: `for order in orders: print(order.customer.name)` triggers N queries.
- GraphQL resolvers without DataLoader: per-field resolution issues per-field DB queries.
- REST endpoints that fetch a collection then fetch each item's details separately.
- Sequential `await` in a loop: `for id in ids: await fetchOne(id)` -- serializable, should be batched or parallelized.

**Defenses**: eager loading (`select_related`, `joinedload`, `include`), explicit JOINs, `WHERE id IN (...)` batched queries, DataLoader pattern, `Promise.all(ids.map(fetchOne))` when independent.

### Missing or wrong indexes
- Query filters on a column with no index.
- Query that uses a function on the column (`WHERE LOWER(email) = ...`) preventing index use; need a functional index.
- Sort orders that don't match any index.
- The classic ORM `findOne` with a join that triggers `SELECT DISTINCT` and disables the index plan. This one is worth knowing by name: it looks like a single-row lookup at the call site, so it reads as cheap, and the `DISTINCT` only appears in the generated SQL.

**Defenses**: `EXPLAIN ANALYZE` on representative production data, especially before merging any new query shape. Index existence verified for every new `WHERE` / `ORDER BY` / `JOIN` predicate.

### Sync I/O on the wrong thread
- Synchronous file read inside an async handler: blocks the event loop in Node, blocks the executor in Rust async, blocks the Goroutine.
- Synchronous network call in a UI thread: app hangs.
- Sync DB driver inside an async runtime: blocks all sibling tasks on the same worker.

**Defenses**: explicit `spawn_blocking` / `tokio::task::spawn_blocking` in Rust; native async DB drivers in Node; offload sync work to a worker thread / pool.

### Sequential when parallel works
`await a(); await b()` when `await Promise.all([a(), b()])` would also work. Roundtrip latency added unnecessarily.

### Chatty interfaces
Many small requests where one batched request would do. Especially over network where the overhead per call (TCP, TLS, auth, parsing) dominates.

### Write amplification
- `for each entity: update_db(entity)` doing N writes instead of one bulk.
- Pub/sub fanout that explodes one event into hundreds of unnecessary writes.

---

## Memory and allocation

### Hot-path allocations
Allocations have costs beyond the bytes: GC pressure (JVM, V8, Go, .NET), heap fragmentation, cache pollution, allocator lock contention. A hot loop that allocates per iteration is suspicious in any language.

- **Closures created per call**: passing an inline lambda in a hot path allocates the closure each time.
- **Per-iteration string formatting / templating**.
- **Per-iteration boxing** in languages with value/reference dichotomy (Java auto-boxing of `int`, .NET boxing).
- **Buffer allocation per call** instead of buffer reuse / pooling.
- **Per-render React component creation in render bodies**.

### Defenses
- Hoist allocations out of loops.
- Pool reusable buffers (object pool, slab allocator, `Pool<T>`).
- In Rust: prefer `&str` over owned `String` on internal boundaries; `Cow<'_, str>` for borrow-or-own; reuse `Vec` with `clear()` rather than reallocating.
- In V8: avoid hidden-class churn (don't mutate object shape after creation); preallocate arrays with `new Array(size)` when size is known.

### Memory leaks vs growth
- Listeners / subscriptions never unregistered -- holds the entire object graph alive.
- Caches with no eviction -- grow until OOM.
- Closures capturing the wrong scope -- the React closure-staleness bug; the JS leak via DOM-node closure capture.
- Bounded queues becoming unbounded under degraded conditions.

### Large structures by-value
Passing or returning a 10MB struct by value forces a copy. Pass by reference, use `Arc<T>` / `Rc<T>` for shared ownership, design for borrow.

---

## Frontend / browser performance

The dominant concerns in real React / Vue / Svelte applications.

### React-specific
- **Component re-renders triggered by parent re-render**: a parent state change re-renders all children unless they're memoized. Wrap in `React.memo` when the cost of comparing props is less than the cost of rendering.
- **Inline object / array / function props**: `<Child config={{a: 1}} onClick={() => ...}>` creates a fresh reference each render, defeating `React.memo`. Hoist or `useMemo` / `useCallback`.
- **`useEffect` with wrong dependency array**: missing deps cause stale closures; spurious deps cause infinite re-runs.
- **Work in render**: heavy computation in the render body instead of `useMemo`. `JSON.parse`, sorting, filtering large arrays.
- **Unmemoized context values**: `<Context.Provider value={{...}}>` (inline object) re-renders every consumer on every parent render.
- **Lists without keys, or keyed by index**: forces React to remount on every reorder.
- **Suspense boundaries too high or too low**: too high → entire page flickers; too low → waterfall of sequential loads.
- **Hydration mismatch**: server-rendered vs client-rendered HTML differ; React tears down and re-renders the entire subtree.

### Bundle size
- **Top-level imports of large libraries**: `import _ from 'lodash'` instead of `import debounce from 'lodash/debounce'`. Tree-shaking depends on module-form imports and side-effect-free packages.
- **Moment.js / large date libs** when `date-fns` / `dayjs` / `Intl` suffices.
- **Polyfills for browsers you don't support**.
- **Dynamic imports for code-split routes**: code that's only needed on `/admin` shouldn't load on `/`.
- **Unused CSS shipped**: lack of PurgeCSS / Tailwind JIT.

### Main thread / event loop
- **Long tasks (>50ms)**: block input, animation, hit Interaction-to-Next-Paint (INP) hard. Break up via `requestIdleCallback`, `scheduler.postTask`, time-slicing, or worker threads.
- **Layout thrash**: reading and writing layout properties (`offsetHeight` then `style.left`) in a loop forces synchronous layout per iteration. Batch reads then writes.
- **`forced reflow`** in DevTools Performance panel = the canonical sign.

### Network / loading
- **Render-blocking resources**: synchronous scripts in `<head>`, non-deferred third-party scripts.
- **Waterfalls**: critical resource is loaded only after a non-critical one finishes.
- **Missing preload / preconnect hints** for known critical resources.
- **Images without `width` / `height`**: causes layout shift; bad for CLS (Cumulative Layout Shift).
- **Unoptimized images**: no `srcset`, no modern format (WebP / AVIF), no lazy loading on below-fold.

---

## Backend / server performance

### Database
Beyond N+1 and missing indexes:
- **Locks held too long**: long transactions blocking other writers; advisory locks held across slow operations.
- **Lock contention on hot rows**: counters incremented on a single row by 100 workers serialize.
- **`SELECT *`** when only a few columns are needed: wastes bandwidth, prevents index-only scans, breaks when schema evolves.
- **Connection-pool exhaustion**: workers blocked on connection acquire; the pool became the bottleneck.
- **No prepared statements**: parser overhead, plan-cache miss.
- **OR queries** that the planner can't optimize; sometimes rewriting as `UNION` is faster.

### Caching
- **Cache stampede**: hot key expires, many requests miss simultaneously, all recompute. Single-flight, lock the recompute, or use probabilistic early refresh.
- **No negative cache**: every missing-key lookup hits the backend.
- **Cache invalidation by TTL only**: stale data acceptable? Or do you need event-driven invalidation?
- **Wrong cache layer**: caching a serialized response when caching the parsed object would save serialization too.

### CPU / encoding
- **Per-request JSON parse of static config**: parse once at boot.
- **Repeated cryptographic work**: validating a signature on every request when the result could be cached briefly.
- **Compression overhead**: `gzip` everything, including responses smaller than the gzip header; cache fully-compressed static bodies.

### Concurrency
- **Lock granularity too coarse**: one big mutex when finer-grained locking or lock-free structures would scale.
- **Lock contention**: `dashmap` / `ConcurrentHashMap` shards or sync.Map for read-heavy workloads.
- **False sharing**: two threads writing to adjacent atomics on the same cache line.
- **Unbounded fan-out**: 10000 parallel `Promise.all` without a concurrency limiter exhausts file descriptors, sockets, or CPU.

---

## Mobile / desktop performance

### iOS / macOS
- **Main thread blocked by Core Data fetches**: use background contexts.
- **Heavy work in `viewDidLoad`**: delays first paint.
- **Auto Layout in a UICollectionView cell**: per-cell layout cost adds up; `flowLayout` or manual layout for hot lists.
- **Unbatched `tableView.reloadData()`** when only some rows changed.

### Android
- **Main thread network or DB**: `StrictMode` should catch but doesn't always; review handlers and lifecycle callbacks.
- **Heavy work in `onCreate`**: blocks app startup.
- **Recyclerview without `setHasFixedSize` / `DiffUtil`**: re-layout per data change.
- **Bitmaps loaded full-resolution into thumbnails**: OOM under low memory.

### Native generally
- **Allocator pressure** in tight loops -- use arenas or stack allocation.
- **Cache miss patterns**: hot loops over array-of-struct vs struct-of-array layouts.
- **Branch prediction**: tight loops with unpredictable branches are slow; data-oriented design eliminates them.

---

## Async-runtime-specific concerns

### Tokio (Rust)
- **Blocking call inside `async fn`**: blocks the executor thread. Use `spawn_blocking`.
- **Held `std::sync::Mutex` across `.await`**: deadlocks under contention. Use `tokio::sync::Mutex`.
- **Fire-and-forget `tokio::spawn` without naming the task**: leaked work, lost panics.
- **`block_on` inside async context**: deadlocks.
- See `~/.claude/rules/rust.md` and the `rust-async` agent for the deep dive.

### Node.js
- **CPU-bound work on the event loop**: use worker_threads for crypto, parsing, image processing.
- **Unbounded promise concurrency**: `Promise.all(items.map(fetchOne))` with 100k items exhausts sockets. Use `p-limit` or `Promise.allSettled` with batching.
- **Streams not piped properly**: backpressure ignored, memory grows.

### Go
- **Unbounded goroutine spawn**: 1M goroutines hits stack memory limits. Use a worker pool with bounded channel.
- **Channel buffer too small**: forces synchronous handoff under load.
- **`sync.Map` vs `map + sync.Mutex`**: sync.Map only wins for caches with append-mostly write patterns.

---

## Measurement discipline

### Profile before optimizing
Most performance "obvious" optimizations are wrong. Modern compilers and runtimes are smart; humans are bad at predicting hot paths. Trust the profiler.

- **Use the right profiler**: CPU vs heap vs flamegraph vs trace.
- **Profile with realistic data**: 10-element test array vs 100k production array.
- **Profile under load**: a function fast in isolation may be slow under contention.
- **Production profiling**: continuous profilers (Pyroscope, Parca, py-spy) catch what bench can't.

### Benchmarks lie when done wrong
- **Microbenchmarks** are routinely defeated by the optimizer; `criterion` (Rust), `JMH` (Java), `Benchmark.js` exist for a reason.
- **Cold cache vs warm cache** dramatically affects results.
- **GC effects**: a benchmark that doesn't trigger GC tells you nothing about production.
- **Single-shot timing in a script** is essentially noise.

### Premature optimization is still real
Knuth's full quote: "premature optimization is the root of all evil. *Yet we should not pass up our opportunities in that critical 3%.*" Identifying the 3% is the skill. Profile first, optimize the hottest path, leave the rest readable.

---

## What is NOT a performance finding

Signal-to-noise matters:

- **Code that's slow in unmeasured paths**. "This could be faster" without evidence that it's hot is a hypothesis, not a finding.
- **Cosmetic optimizations**: `var i = 0` vs `let i = 0`; ++ vs +=; ternary vs if. The JIT eats these.
- **Reasonable verbosity at trust boundaries**: explicit validation adds nanoseconds you don't care about.
- **Test code**: tests should be readable, not fast.
- **Generated code, bindings, framework glue**: only flag if you have evidence it's hot.
- **Single-call overhead** if the function isn't called frequently.

---

## Severity

The `performance` subagent uses `panel-contract.md`'s rubric. Specific calibration:

- **blocker**: a measured or extremely-likely production-scale performance defect (N+1 on a hot endpoint, blocking the event loop in an async server, O(n²) on data that grows unbounded).
- **major**: a likely defect under expected load (missing index on a new `WHERE` clause, hot-path allocation that could be pooled, render thrash in a frequent component, sequential awaits where parallel works).
- **minor**: optimization opportunity that may pay off (memoization, batching, bundle-size cleanup).
- **nit**: micro-optimization that probably doesn't matter.
- **insight**: structural -- "this whole pipeline materializes 1GB before the next stage processes it; consider streaming."

Confidence: high when the trigger is concrete and the cost path is named ("at 10k items, `for x in xs: arr.find(y => y.id == x.id)` is 100M comparisons"); medium when reasoned without measurement.

---

## Process for the performance agent

1. **Read the project conventions.** Performance-relevant docs: profiling commands, known hot paths, query performance rules. A backend `CLAUDE.md` that names the specific ORM patterns which have hurt that project before is worth more than this whole catalog, because it is grounded in that codebase's real incidents. Look for one and prefer it where it conflicts with the generic advice here.
2. **Identify the runtime context.** Frontend, backend, mobile, native? Each has its own catalog above.
3. **Walk the catalog** in priority order: algorithmic complexity, I/O patterns (N+1, sync-on-async, parallel-vs-sequential), memory / allocation, runtime-specific concerns.
4. **For each candidate**, ask: is this on a hot path? Can the cost be named at production scale? If the answer is "I don't know," flag at lower confidence as a question rather than a finding.
5. **Use profiling tooling when available**. If `cargo flamegraph` artifacts exist, read them. If the project has its own benchmark suite (`bench/`), check whether the change is covered.
6. **Suggest measurement** when the right answer depends on data the agent doesn't have. "Run `EXPLAIN ANALYZE` on this query on production-scale data" is a valid finding.
7. **Stay read-only.**
