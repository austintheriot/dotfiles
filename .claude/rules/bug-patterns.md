# Bug-Prone Patterns

A reference catalog of the bug categories most worth scrutinizing during review. Used by the `bug-hunter` subagent. Cross-cutting; complements language-specific (`typescript.md`, `rust.md`) and domain-specific (`distributed-systems.md`, `observability.md`) rules.

The principle: most production bugs are not novel. They cluster into a small number of canonical shapes, and recognizing the shape is faster than rediscovering each one from first principles. This file is the pattern-recognition vocabulary.

---

## Time-of-check to time-of-use (TOCTOU)

A check and the action that depends on the check are separated in time, and the state can change in between. Classic examples: `if (file.exists()) read(file)` (file deleted between the two), `if (user.balance >= amount) charge(user, amount)` (concurrent withdrawal drains the balance first), `if (key not in cache) cache[key] = compute()` (two workers compute the same value).

**Where it lives**: any precondition check followed by an action that assumes the precondition still holds. The gap can be microseconds (interrupt, context switch) or arbitrarily large (the user changed something on another tab). Anywhere a thread, process, request, or external actor can intervene between check and use.

**Defenses**: atomic compare-and-swap, transactional `SELECT ... FOR UPDATE`, idempotent operations (the action validates its own preconditions), passing the checked value into the action rather than re-fetching, holding a lock across both operations. In application code: `try { do(); } catch (NotFound) { recover; }` is often safer than `if (exists) do();`.

**Review heuristic**: any `if (x.exists()) ... x` or `if (status == X) ... operate(x)` pattern. Ask "what happens if x changes between line A and line B?"

---

## Race conditions and data races

Two execution contexts (threads, async tasks, processes) access shared mutable state and at least one writes; the result depends on scheduling. **Data race** is the language-level subset (unsynchronized concurrent access to the same memory) and is undefined behavior in C/C++/Rust. **Race condition** is the broader concept including higher-level ordering bugs even when individual memory accesses are atomic.

**Common shapes**:
- *Lost update*: read-modify-write (counter, balance, JSON field) without atomicity. Two writers each read the old value, increment, and write back; one increment is lost.
- *Check-then-act*: TOCTOU above is the special case.
- *Publication race*: object reference published before construction completes; reader observes partially-constructed state.
- *Iterator invalidation*: collection mutated during iteration.
- *Initialization race*: lazy-init `if (instance == null) instance = new()` without locking creates two instances under contention.
- *Reordering*: weak memory models permit writes to be observed out of order; visible without synchronization fences.

**Defenses**: locks (mutex, rwlock, semaphore), atomics with the right ordering, message-passing instead of shared state, immutable data, fork/join structured concurrency. Confine mutable state to one thread when possible.

**Review heuristic**: any `pub` / `static mut` / shared `Arc<Mutex<...>>` deserves a "who else can access this concurrently?" pass. Any `async` function that reads then writes the same datum without holding state across the await is suspect.

---

## Async / await footguns

Async code looks sequential and is not. The await point is a scheduling boundary -- the task can be cancelled, the runtime can yield to another task, the caller can drop the future. Several distinct bug shapes:

- *Holding a lock across `await`*: ties up the lock for the duration of the await, including arbitrary I/O. Blocks every other task waiting on it. In Rust, `MutexGuard` across `.await` is a `Send` error or a deadlock; in C#/Kotlin/JS the runtime doesn't stop you.
- *Cancellation safety*: when the future is dropped, in-flight work disappears. A partial network write, a half-emitted log line, a transaction left open. Most async APIs do **not** specify their cancellation behavior; assume cancellation can happen at any await point.
- *Fire-and-forget*: `task::spawn(f())` without holding the handle drops errors and panics silently. The work runs but failures don't propagate.
- *Sync-over-async*: `runtime.block_on(future)` from inside async context deadlocks; calling sync I/O from an async runtime blocks the executor and stalls every other task on that worker thread.
- *Error context loss*: `?`-propagated errors across awaits often strip the surrounding context (which call, with what arguments, on behalf of which user).

**Defenses**: structured concurrency (every spawned task is owned by a scope), explicit cancellation handling (clean up on drop or document the unsafety), keep awaits out of critical sections, never call sync I/O from async without `spawn_blocking` or equivalent.

**Review heuristic**: every `await` is a yield point; every yield point is a potential cancellation point and reordering point. Ask "what happens if this future is dropped here?"

---

## Caching bugs

Caches turn correctness questions into freshness, invalidation, and stampede questions. The canonical Phil Karlton quote: "There are only two hard things in computer science: cache invalidation and naming things."

**Common shapes**:
- *Stale read*: cache hit returns data older than the application's consistency promise. Underlying store updated; cache not invalidated; downstream sees the old value indefinitely.
- *Inconsistent caches*: two caches (or a cache and the source of truth) updated in different orders; window where they disagree. The bug surfaces only when a request reads from one cache for one field and another for a related field.
- *Cache stampede / thundering herd*: high-traffic key expires; many concurrent requests miss, all recompute, all write back. Backend is hammered, latency spikes, sometimes cascading. Fix: single-flight (one in-flight compute, others wait) or probabilistic early refresh.
- *Negative cache absence*: a 404 / not-found result is not cached; every miss hits the slow backend. Or the opposite: a transient error is cached as the result; subsequent good requests get the cached failure.
- *Unbounded cache*: no eviction policy, no TTL, no size cap. Memory grows until OOM.
- *Cache-key collisions*: two different requests hash to the same cache key (e.g., key omits a relevant parameter). Cross-tenant or cross-user data leakage.

**Defenses**: write-through or write-around with explicit invalidation events; TTL with jitter; single-flight or coalescing; bounded LRU/LFU; cache-key audits ("does this key include every parameter that affects the result?").

**Review heuristic**: any `cache.get / compute / cache.set` triad needs answers for invalidation, stampede, bounds, negative caching, and key completeness.

---

## Null and optionality bugs

The billion-dollar mistake. `null` / `nil` / `None` / `undefined` represents "absence," and code that doesn't distinguish "this field is meaningfully absent" from "I forgot to set this" produces NullPointerException, segfault, or silent wrong behavior.

**Common shapes**:
- *Unchecked dereference*: `user.address.city` where `address` is sometimes null.
- *Default-coerced null*: `count ?? 0` papering over a real "no data" signal; downstream treats absence as a valid zero.
- *Optional chain hides errors*: `user?.payment?.method` silently returns undefined when any step is null; the caller thinks "no payment method" but the real cause was "user lookup failed."
- *Null in collections*: `List<User>` containing nulls; iteration crashes; equality / hashing breaks.
- *Implicit null on initialization*: a field declared but not yet set; later code reads it; especially common in objects with multi-step initialization.
- *Type-system lies*: `T!!` in Kotlin, `as T` in TypeScript, `unwrap()` in Rust used to silence a real possibility.

**Defenses**: total functions (`Option<T>` / `Maybe<T>` / `Result<T, E>` as return types), pattern matching that forces the absent case, smart constructors that make `T` itself the proof of presence, separate types for "validated" vs "raw" inputs (parse, don't validate), nullable-by-default off in the language config.

**Review heuristic**: every `?.`, `!!`, `as`, `unwrap`, `assert not None`, or "should never be null" comment is a finding to scrutinize. Often there's a real "absent" case the code is ignoring.

---

## Integer overflow / underflow / precision

The number type is finite; arithmetic operations can produce results outside the type's range or with precision loss.

**Common shapes**:
- *Signed overflow*: `i32::MAX + 1` -- undefined behavior in C/C++, panic in Rust debug / wrap in release, defined wrap in Java/Go/.NET. Different language, different bug.
- *Multiplication overflow*: `length * sizeof(item)` for allocation sizing -- classic vulnerability shape (allocate small, write large).
- *Truncation on cast*: `(int)longValue`, `as u8`, `Number(bigInt)` silently dropping bits.
- *Floating-point precision*: `0.1 + 0.2 != 0.3`, accumulated drift in summation, NaN propagation, infinity from divide-by-zero.
- *Currency in floats*: rounding errors compound; use fixed-point or integer cents.
- *Off-by-one*: `<` vs `<=`, fencepost (N items needs N+1 fence posts), inclusive/exclusive boundary confusion.
- *Mixed-signedness comparison*: `size_t < int` promotes int to unsigned; a negative becomes huge positive.

**Defenses**: checked arithmetic (`checked_add`, `Math.addExact`), saturating arithmetic when wrap is wrong, bigint for unbounded ranges, fixed-point or integer types for money, explicit boundary tests, never compare signed and unsigned without an explicit cast.

**Review heuristic**: any arithmetic on user-controlled input, any `*` for allocation sizing, any loop bound, any boundary value (0, MAX, -1, INT_MIN) is worth a glance.

---

## Resource leaks

Files, sockets, database connections, cursors, mutexes, threads, GPU contexts, subscription handles, event listeners -- anything acquired and supposed to be released. Leaks accumulate silently and manifest as "the system runs fine for a week then falls over."

**Common shapes**:
- *Missing release on error path*: `acquire(); risky(); release()` -- if `risky()` throws, release never runs.
- *Early return between acquire and release*: code added later returns before the release call.
- *Lifetime mismatch*: caller holds a handle longer than expected; references leak through closures or fields.
- *Event listener leaks*: subscribed on mount, never unsubscribed; the object can't be GC'd because the listener list retains it.
- *Connection-pool exhaustion*: borrowed connection not returned to pool on every code path.
- *Goroutine / task leaks*: spawned task whose stop signal is never sent; runs forever, holds memory.

**Defenses**: language-level scoped cleanup (`defer`, `try-with-resources`, `using`, RAII, `with` statement, `Drop`), structured concurrency for tasks, weak references for event subscriptions, connection-pool metrics with alerts.

**Review heuristic**: every `acquire`/`open`/`connect`/`spawn`/`subscribe` deserves a release pair, on every exit path (success, error, panic, cancellation).

---

## Mutability and aliasing

Shared mutable state is the source-of-truth of bugs. Whenever two references can mutate the same datum, the result is observation-order dependent; whenever a reference outlives the assumed lifetime of the data, you get use-after-free, dangling pointer, or reading a value that was supposed to be invalidated.

**Common shapes**:
- *Aliased mutable references*: pass a reference into a function that stores it; caller continues to mutate; callee observes changes; behavior depends on call order.
- *Defensive copy missing*: function returns an internal collection by reference; caller mutates it; class invariants break.
- *Defensive copy unnecessary*: function clones a 10MB structure on every read because "safer."
- *Shared default*: Python `def f(x=[])` -- the default list is shared across all calls.
- *Frozen-looking-but-mutable*: a "readonly" view backed by mutable storage; underlying mutation aliases through.

**Defenses**: immutable data structures, value semantics (clone on share), Rust's borrow checker, Java/Kotlin `val`/`final`, structural sharing for cheap immutable updates.

**Review heuristic**: any reference passed across an API boundary deserves the question "who else holds this? who can mutate it? when?"

---

## Error handling failures

Errors mishandled cause silent data loss, wrong results, or stuck operations. The patterns are language-independent.

**Common shapes**:
- *Empty catch*: `try { ... } catch (e) {}` swallows everything including bugs.
- *Catch and log, then continue*: error logged at the wrong level, ignored downstream; the operation "succeeded" from the caller's perspective.
- *Catch the wrong exception type*: catching `Exception` (base class) when only `IOException` was expected; masks unrelated bugs.
- *Re-thrown without context*: error propagates upward but loses the call site and arguments; debugging takes hours.
- *Error treated as success*: HTTP 200 with `{"error": "..."}` in the body; client treats as success because status was OK.
- *Partial success not signaled*: batch operation processes 100 items, 3 fail, returns success; failures are silent.
- *Retry on non-retryable error*: 4xx errors retried as if transient; infinite loops or repeated bad work.
- *Retry without backoff*: hammers the failing service; turns partial failure into full outage.
- *No idempotency on retry*: retried operation duplicates the side effect (charge a card twice, send the email twice).

**Defenses**: errors as values (`Result`/`Either`), explicit `?` propagation that preserves type, structured error types with context, classify errors as retryable/non-retryable in the type, idempotency keys on every non-GET API.

**Review heuristic**: every `catch`, `try?`, `if err != nil { return err }`, `.unwrap_or(default)` is a finding to scrutinize. "What if this fails?" should have a deliberate answer.

---

## Time and timezone bugs

Time is one of the most bug-rich primitives. Most code that touches time has at least one latent bug.

**Common shapes**:
- *Naive timestamps*: storing local times without timezone; ambiguous around DST transitions; some local times are skipped (spring forward), some occur twice (fall back).
- *UTC/local confusion*: a "midnight" stored in one zone displayed in another; subscriptions renew on the wrong day.
- *DST transitions*: scheduled job runs twice or zero times on transition days; "every day at 2am" hits "2am that doesn't exist."
- *Calendar arithmetic*: `now + 30 days` is not `next month`; "first Tuesday of the month" needs care; February exists; leap years exist; leap seconds exist.
- *Monotonic vs wall clock*: latency measurements with wall clock break when NTP adjusts the clock backward.
- *Clock skew*: two machines disagree on `now()` by milliseconds to minutes; signature expiration / nonce validation across machines fails.
- *Time-zone in test data*: tests pass in CI's timezone (UTC), fail in the user's local timezone.
- *`Date.now()` in pure functions*: function appears pure but depends on wall clock; non-reproducible, hard to test.

**Defenses**: store UTC timestamps with timezone metadata; use proper date/time libraries (not strings, not manual math); monotonic clock for durations; inject `Clock` for testability; explicit timezone in every external API.

**Review heuristic**: any `new Date()`, `Date.now()`, `System.currentTimeMillis()`, `LocalDate` (without zone), timezone string, or scheduled-cron pattern is worth scrutiny. Ask "what timezone is this in, and what happens during DST?"

---

## Encoding, escaping, locale

Text and bytes have many representations; converting between them is bug-rich.

**Common shapes**:
- *Mojibake*: bytes decoded as the wrong encoding (UTF-8 read as Latin-1 etc.) -- visible as garbled characters but only for non-ASCII input.
- *SQL injection / command injection / log injection*: user input concatenated into a query / shell / log without escaping; control characters break out of the intended context.
- *HTML / XSS*: user input rendered without escaping; script tags execute.
- *Length in chars vs bytes vs grapheme clusters*: `"é".length` differs across languages and across precomposed vs decomposed forms; counts wrong, truncations slice through multi-byte chars.
- *Case folding by locale*: Turkish `i`/`İ`/`ı` famously breaks ASCII-only `toLowerCase`. Comparing case-insensitive requires Unicode-aware fold + locale awareness.
- *Sorting by string compare*: collation differs by locale; "ABC" vs "abc" depends on locale.
- *Locale-formatted numbers*: `1,000.50` (US) vs `1.000,50` (de-DE); parsing user input or formatting machine output to humans.

**Defenses**: parameterized queries, explicit escaping at every output boundary, Unicode-aware string libraries, never concat user input into a different syntactic context, locale boundary at the UI layer (machine-readable formats internal, locale-formatted only at display).

**Review heuristic**: any `String.format`, `+ user_input +`, `exec(cmd)`, `innerHTML =`, `eval`, `.toLowerCase()`, `.length` on text deserves a glance.

---

## Boundary conditions

The corners of the input space. Where most edge-case bugs hide.

**Common shapes**:
- *Empty collection*: code that assumes "at least one item" -- `list[0]`, `list.first()`, `list.reduce(f)` without identity.
- *Single item*: pairwise comparisons (`zip(list, list[1:])`) that need at least two.
- *Maximum size*: integer overflow on length, memory exhaustion, hashtable degradation at high load factor.
- *Zero / negative / NaN / infinity*: division, log/sqrt, percentage calculations.
- *Min/max value*: `Integer.MIN_VALUE` -- `abs(MIN_VALUE)` overflows in two's complement.
- *First / last / only / none*: pagination boundaries, week-of-year boundaries (week 53), end-of-month.
- *Special characters in input*: null byte, newline, NUL, BOM, RTL override, zero-width joiner.

**Defenses**: explicit boundary tests, property-based testing, fuzzing for input parsers, exhaustive pattern match instead of `if-else` chains.

**Review heuristic**: for every function, ask "what does this do on []? on [single]? on max-size? on NaN?"

---

## API and abstraction leaks

The interface promises X; the implementation actually requires Y. Callers eventually discover Y the hard way.

**Common shapes**:
- *Performance leak*: `List.contains` is O(n) but the type system says nothing; caller writes `for x in xs: if other.contains(x)` -- O(n²).
- *Order leak*: `dict.keys()` happens to iterate insertion order in your runtime; code depends on it; behavior breaks on a different runtime or runtime version.
- *Identity leak*: `==` defaults to reference equality; tests pass because reuse keeps identity, fail in production with fresh objects.
- *Thread-safety leak*: class is "thread-safe for reads" but a method documented as read-only actually mutates internal cache.
- *Resource-cost leak*: function makes a network call without saying so; caller invokes it in a tight loop.
- *Lifetime leak*: `getView()` returns a view that aliases internal storage; mutating the returned object mutates the parent.
- *Sentinel-value leak*: `findIndex` returns -1 for not-found; caller treats it as a valid index.

**Defenses**: types that encode the invariant (`Result<T, E>` not magic sentinel, `Set<T>` not `List<T>` for membership, `Cow<'_, T>` for borrowed-or-owned), documented complexity, immutable return types, suffix conventions (`getOrNull`, `getOrThrow`).

**Review heuristic**: "what does this API promise, what does it actually require?" Especially for collections (order, duplicates, immutability), I/O (cost, blocking, retries), and mutability (aliased state, defensive copies).

---

## Security-shaped bugs

Many security bugs are also correctness bugs viewed through an adversarial lens. The bug-hunter should flag them whether or not a dedicated security lens is also active.

- *Trust-boundary confusion*: input from outside the trust boundary used as if it were internal. User-supplied URL passed to `fetch()` (server-side request forgery), user-supplied path passed to `open()` (path traversal), user-supplied template passed to `render()` (SSTI).
- *Insufficient authentication / authorization*: endpoint that should require auth doesn't; auth checked once at session start but not on the specific operation; horizontal privilege escalation (`/users/123` accessible to user 456).
- *Information leak via error*: stack trace, query, internal IDs returned in error responses.
- *Timing side channels*: `==` on secret strings short-circuits; attacker times the response to extract bytes. Use constant-time comparison for tokens/passwords/HMACs.
- *Logging secrets*: tokens, passwords, full auth headers in logs.
- *Hardcoded credentials*: API keys, DB passwords in source.

**Defenses**: defense in depth, fail closed, parameterized inputs at every trust boundary, constant-time crypto comparisons, structured logging with redaction, secrets in env / secret stores.

**Review heuristic**: every external input is hostile. Every output crosses a trust boundary -- redact before logging, sanitize before rendering.

---

## How to use this catalog in review

Go through the changed code (or surveyed module) and ask, for each category: *does this code touch this pattern?* If yes, scrutinize. If no, move on. The catalog is a checklist of *where to look*, not a list of things that must exist.

When flagging a bug, anchor it to the category and (where possible) the specific shape within the category. "TOCTOU between line 42 and 47 -- user.balance is rechecked after a yield, allowing concurrent withdrawal" is more actionable than "possible race condition."

The bug-hunter should be willing to flag findings even when uncertain, but should mark confidence accordingly. A confirmed bug pattern with a clear trigger is 90+ confidence. A pattern that's present but might be unreachable, or might be defended-against elsewhere, is 60-70 -- still worth surfacing.
