---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Concurrency Principles

A reference for evaluating code from a concurrency-correctness lens during review. Used by the `concurrency` subagent. The scope is **in-process, intra-machine, shared-memory concurrency**: threads, locks, atomics, memory models, lock-free / wait-free structures, actors, CSP-style channels, async/await in any language, structured concurrency, web workers, SharedArrayBuffer.

The scope is **NOT**:
- Rust-specific async (covered by `rust-async`).
- Cross-process / cross-machine concurrency: retries, idempotency, queues, sagas, fencing tokens, replication (covered by `distsys-runtime` and `distsys-data`).
- Single-threaded line-level bug patterns (covered by `bug-hunter`'s race section as patterns; this agent reviews concurrency as a *property* of the system, not patterns in a single line).
- Performance under contention as a goal in itself (covered by `performance`); we flag correctness defects that *also* hurt performance, but raw speedup is the performance agent's lens.

The core thesis: **concurrency bugs are a category of category.** A race manifests as a null dereference, a logic error, a security hole, an availability failure -- whatever the racing operations happened to be doing. Reviewers must learn to see the concurrency dimension *overlaid* on every other failure mode. Therac-25, Mars Pathfinder, Knight Capital, the 2003 Northeast blackout, Cloudbleed all started as concurrency oversights.

The operational thesis (Hans Boehm, 2005-2011, "SC for DRF"): **write data-race-free programs, get sequential consistency for free.** Programs with even one data race have *undefined behavior* in C/C++/Rust; partially-defined behavior in Java/Go. The reviewer's question therefore reduces to: "is this access ordered with respect to that one, and by what?"

---

## Universal principles

### Sequential Consistency for Data-Race-Free programs (SC for DRF)

The most important fact in modern concurrent programming. Adve & Hill, 1990; ratified by Java (JSR-133), C++11, C11, C# 2.0, Go (2014), Rust. The deal:

- The programmer promises no data races (every conflicting pair of accesses is ordered by synchronization).
- The language promises sequential consistency: the program behaves as if all operations executed in some interleaved total order that respects each thread's program order.
- Programs with races have UB (C/C++/Rust) or weakly-defined behavior (Java/Go) -- the compiler can and does exploit "no races" to optimize aggressively.

**Flag**: any unsynchronized access to mutable shared state. Even reads. Even on x86. Even if "it works in testing."

### Hardware memory models exist; the language model hides them only if you obey the rules

x86 is strong (Total Store Order): single-variable atomics mostly just work. ARM/Power/RISC-V are weak: loads reorder with loads, stores with stores, write atomicity is not guaranteed in all cases. Apple Silicon and AWS Graviton routinely expose memory-ordering bugs that never appear on x86.

**Flag**: code that assumes x86 semantics implicitly -- `volatile` without atomic guarantees, plain stores used as flags, `if (!stopped) {}` loops with non-atomic `stopped`, double-checked locking without proper publication.

### Concurrency is composed, not just spawned

The structured-concurrency principle (Nathaniel Smith, 2018, "Notes on structured concurrency, or: Go statement considered harmful"): unrestricted `go` / `spawn` / `Thread.start()` / `Promise` is to concurrent code what `goto` was to sequential code -- it destroys local reasoning. Every concurrent task should live inside a scope that:
- Waits for every child task before exiting.
- Propagates cancellation downward.
- Propagates errors upward.

Kotlin's `coroutineScope` / `supervisorScope`, Swift's `TaskGroup`, Java 21+ Loom's `StructuredTaskScope`, Tokio's `JoinSet`, Trio's `nursery` -- all implementations of the same idea.

**Flag**: fire-and-forget `tokio::spawn` / `Task.run` / `setTimeout` / `new Thread().start()` with no documented exit condition and no handle held; coroutines launched on `GlobalScope` or equivalent; "background" work with no supervisor.

### Cancellation is part of the contract

Every long-running concurrent operation should:
- Accept cancellation cooperatively (poll for it, `yield()`, check `isActive`, honor `interrupt()` / `CancellationToken`).
- Clean up resources on cancellation (locks held, files open, partial state).
- Propagate cancellation to children.

The Rust async ecosystem documents a stronger property: **cancellation safety** -- a future is cancel-safe if dropping it at any await point leaves the system in a valid state. `tokio::select!` cancels all unselected branches at every iteration; any branch that mutates state across awaits without restoring it is a bug.

**Flag**: `Thread.interrupt()` ignored or swallowed; cancellation tokens never checked; cancellation that doesn't release locks or close resources; `select!` branches that mutate state without considering cancellation; sleep / wait loops that don't honor cancellation.

### Locks beat lock-free in the 95% case

The conventional wisdom (McKenney, Lea, Williams): prefer well-designed locks. Modern locks (MCS, CLH, futex-backed) are very efficient under low contention. Lock-free is hard to write correctly (ABA, memory reclamation, memory ordering) and often slower than locks under low contention. Reach for lock-free only when contention is *measured* as the bottleneck and the data structure is small enough to verify.

**Flag**: hand-rolled lock-free code without measurement justifying it; lock-free reinventions of `ConcurrentHashMap` / `java.util.concurrent.atomic.*` / `crossbeam::queue` / etc.; lock-free without a clear memory-reclamation strategy.

---

## The canonical bug shapes

### Races

**Data race**: two unsynchronized concurrent accesses to the same memory, at least one a write. UB in C/C++/Rust. Detection: ThreadSanitizer (TSan), Go's `-race` flag (every Go project should run tests with `-race`), Helgrind.

**Race condition** (the broader category, even when individual ops are atomic):

- **Lost update**: read-modify-write without atomicity. Two writers each read old value, both write `old + 1`, one increment vanishes. Fix: atomic RMW (`fetch_add`, CAS loop), explicit lock around the whole sequence, transactional update.
- **Check-then-act (TOCTOU)**: `if (map.contains(k)) map.get(k)` -- between the two, another thread removes `k`. Fix: atomic check-and-act primitives (`computeIfAbsent`, `putIfAbsent`, `entry().or_insert_with()`). Application-level equivalents apply at every "if-then-do" pattern over shared state.
- **Publication race**: object reference stored in shared location before construction completes. Reader sees the reference, dereferences, sees partial object. Fix: `volatile` reference in Java, release store / acquire load in C++/Rust, `final` fields in Java (special JMM guarantee).
- **Iterator invalidation**: collection mutated during iteration. `ConcurrentModificationException` if lucky; silent skipped/duplicated elements if not. Fix: concurrent collections, snapshot iteration, locking.
- **Initialization race**: lazy singleton without proper synchronization. The classic double-checked locking bug.

### Deadlocks

The four Coffman conditions: mutual exclusion, hold-and-wait, no preemption, circular wait. Break any one.

- **Classic deadlock**: two threads each hold one lock, each want the other's.
- **Lock-order inversion**: path 1 takes A then B; path 2 takes B then A. Fix: total ordering on locks; never acquire a lower-ordered lock while holding a higher-ordered one. Static analyzers (Helgrind) detect dynamically.
- **Self-deadlock**: re-entering a non-reentrant lock. Fix: reentrant locks (Java's `ReentrantLock`); be aware Rust's `std::sync::Mutex` is non-reentrant by design.
- **Resource deadlock**: connection pool exhaustion where every connection waits for another.
- **Async deadlock**: blocking the executor by calling sync code from async; awaiting a task that needs the current task's thread; `runtime.block_on(future)` from inside async context.

### Livelock and starvation

- **Livelock**: threads make no progress but don't block (two retry-on-conflict transactions retrying in lockstep). Fix: randomized backoff (see `~/.claude/rules/distributed-systems.md` principle 19), bounded retry, priority inheritance.
- **Starvation**: some threads never get the resource (unfair locks; writer never gets through reader stream with non-fair RWLock; unbounded retry without backoff). Fix: fair locks, queue-based wait, bounded retry.
- **Priority inversion**: low-priority holds lock, high waits, middle preempts low -- high effectively waits for middle. **The Mars Pathfinder bug.** Fix: priority inheritance on the mutex.

### Lock-free hazards

- **ABA**: value goes A → B → A; CAS thinks unchanged. Classic in lock-free stacks. Fix: versioned pointers (double-width CAS), hazard pointers (Maged Michael 2004), RCU (McKenney), epoch-based reclamation, GC.
- **Memory reclamation**: when can you `free` a node a lock-free reader might still hold? Hazard pointers, RCU, epoch reclamation. GC sidesteps the problem at GC's cost.
- **Reordering visible to other threads**: relaxed memory ordering exposing partial state. Producer stored data with `relaxed`; consumer reads with `relaxed`; consumer sees "ready" flag before data.

### Visibility / staleness

- **Stale read**: reading a value another thread wrote without synchronization. May see arbitrarily old value. Compiler may hoist non-atomic reads out of loops. The canonical bug: `while (!stopped) { ... }` where `stopped` is non-atomic -- the compiler hoists the read; the loop never terminates. Fix: `volatile` (Java), atomic (C++/Rust), `sync/atomic` (Go).
- **Word tearing**: non-atomic write to a value spanning multiple memory accesses. Rare on modern hardware for aligned naturally-sized values; real for 64-bit values on 32-bit platforms, misaligned values, non-power-of-2 sizes.
- **`volatile` in C/C++ is not for concurrency.** `volatile` prevents compiler reordering but does not provide atomic operations or memory ordering between threads. Use `std::atomic` / `_Atomic` for concurrent shared variables. (Java's `volatile` is different; it provides happens-before and is correct for concurrency.)

### Async-specific

- **Holding a lock across `await` / `.await` / `await`**: the future is suspended while the lock is held. Another task wakes, needs the lock, blocks. If the original task can only resume on a thread the blocking task holds, deadlock. Fix: drop the lock before await; use an async-aware mutex (`tokio::sync::Mutex`, `kotlinx.coroutines.sync.Mutex`).
- **Cancellation safety**: a future cancelled mid-operation must leave invariants intact. `tokio::select!` is the canonical prompt for this question because every iteration cancels all unselected branches.
- **Fire-and-forget**: `tokio::spawn(f())` without holding the `JoinHandle`. The work runs; if it errors, the error is dropped; panics are logged but not propagated.
- **Sync-over-async**: `runtime.block_on(future)` from inside async context. Often deadlocks. Worse: it works in tests with one runtime worker, hangs in production with N workers.
- **Sync-on-async-thread**: blocking I/O (file read, sync DB driver, sync HTTP) on an event-loop thread blocks every other task on that thread. Fix: `spawn_blocking` (Tokio), `Dispatchers.IO` (Kotlin), `worker_threads` (Node), `asyncio.to_thread` (Python).
- **The "function color" problem** (Bob Nystrom 2015): async and sync functions can't be called from each other transparently; the async/sync boundary infects every caller. Loom and Erlang sidestep by making all code "sync" but cheap. Rust embraces coloring and gets cancellation safety as a result.

### Actor-specific

- **Mailbox overflow**: unbounded queue eats memory; bounded queue produces head-of-line blocking or dropped messages. Akka default is unbounded; bounded mailboxes need explicit configuration. Erlang's per-process mailbox is unbounded; runaway producer eats heap.
- **Selective receive** (Erlang): pattern-match-only `receive` on a specific message shape. Non-matching messages pile up waiting for the selective receive to clear. `gen_server` handles arrive in order; raw `receive` is dangerous.
- **Supervision strategy mistakes**: putting too many siblings under one supervisor with `one_for_all` (mass restarts); not enough nesting (no isolation); no max-restart-frequency (restart storms).

### Channel / CSP-specific

- **Unbuffered channel send blocks**: deadlock if no receiver. Classic Go bug.
- **Closed channel**: in Go, send panics, receive returns zero value with a flag. Receiver ignoring the flag treats closed channel as endless stream of zeros.
- **`select` default case**: turns blocking `select` into busy-wait. Flag `select { ... default: }` in a loop without sleep.
- **Goroutine leak**: every `go` needs a documented exit. Send-only goroutine with no receiver blocks forever; receive-only goroutine on a channel never closed blocks forever. Use `context.Context` for cancellation.

### Initialization races

- **Double-checked locking** (pre-Java 5): broken without `volatile`. The construction can reorder past the publication; another thread sees the reference but partially-constructed object. Fix: `volatile` reference in Java; release-store / acquire-load in C++.
- **Static initialization order** in C++: across translation units, undefined. Fix: function-local statics (thread-safe since C++11).
- **Lazy singleton race**: two threads both see `null`, both construct; one wins, other's instance escapes (worst case).

### Cache and false sharing

- **False sharing**: two unrelated values on the same 64-byte cache line. Every write to one invalidates the other's cache copy on remote cores. Classic symptom: parallel loop that scales poorly with cores; profiler shows high MESI traffic. Fix: pad to cache-line boundary (`alignas(64)`, `@Contended` in Java, `#[repr(align(64))]` in Rust, `hardware_destructive_interference_size` in C++17).
- **True sharing pathology**: hot atomic counter on N cores -- every increment bounces the line. Fix: per-core sharded counters, aggregate on read.

### Thread-pool pathologies

- **Pool exhaustion + recursive submit**: task submitted to the same pool that hosts the current task. Bounded full pool deadlocks. Fix: separate pools, or ForkJoinPool with work-stealing.
- **Pool sized wrong**: too small starves; too large oversubscribes. Goetz: CPU-bound `N_threads = N_cpu + 1`; I/O-bound `N_threads = N_cpu * (1 + wait_time / compute_time)`. Loom virtual threads sidestep entirely.
- **Blocking task on compute pool**: long blocking call on `ForkJoinPool.commonPool()` blocks one of N_cpu threads. Fix: separate I/O pool, `ManagedBlocker`, `spawn_blocking`, `Dispatchers.IO`.

---

## Memory ordering (the four you need)

In any language with explicit memory ordering (C++11, C11, Rust, modern Java, modern Go atomics):

- **`relaxed`**: atomicity only, no ordering. Use for counters and statistics where order doesn't matter.
- **`acquire`** (load) and **`release`** (store): the publication pair. Release synchronizes-with a subsequent acquire of the same atomic that observes the released value. Builds locks, message-passing, lazy publication.
- **`acq_rel`**: for read-modify-write operations (compare-exchange, fetch-add); both acquire and release semantics.
- **`seq_cst`** (sequential consistency): total order across all threads. The default; the slowest. Required for some patterns (Dekker without flag fix-ups, totally-ordered events).
- **`consume`**: never properly implemented; treated as acquire. **Do not use.**

The reviewer's question for any explicit-ordering atomic: "Is this ordering sufficient for what depends on it? Is it more than necessary?" Both are findings -- under-ordered is a correctness bug; over-ordered is performance overhead.

---

## Per-language specifics

### Java + JVM
**Primitives**: `synchronized`, `volatile`, `final` (special JMM guarantee), `java.util.concurrent.locks.*`, `j.u.c.atomic.*`, `ConcurrentHashMap`, executors, `CompletableFuture`. JDK 21+: virtual threads, `StructuredTaskScope`.

**Pitfalls**: missing `volatile` on flag fields; `synchronized(this)` (callers can lock you); exposing internal collections without defensive copy; `Executors.newFixedThreadPool` uses unbounded `LinkedBlockingQueue`; `CompletableFuture.thenApply` swallows exceptions (use `whenComplete`); `Thread.interrupt()` discipline.

**Tooling**: JFR, async-profiler, jstack (deadlock detection), `-XX:+UnlockDiagnosticVMOptions`.

### C# / .NET
**Primitives**: `Task`, `async`/`await`, `Channel<T>`, `SemaphoreSlim`, `Interlocked.*`, `lock`, `ConcurrentDictionary`, TPL Dataflow, PLINQ.

**Pitfalls**: `async void` (only for event handlers); sync-over-async (`.Result`/`.Wait()`) deadlocking on `SynchronizationContext`-using contexts (classic ASP.NET Framework bug); `ConfigureAwait(false)` discipline in libraries; capturing `this` in long-lived tasks.

**Tooling**: PerfView, Concurrency Visualizer, Coyote for systematic testing.

### C++
**Primitives**: `std::thread`, `std::jthread` (C++20, joinable on destruction, supports `stop_token`), `std::mutex`, `std::shared_mutex`, `std::atomic<T>`, `std::condition_variable`, `std::latch`/`std::barrier` (C++20).

**Pitfalls**: data races as UB (compilers exploit); `volatile bool` is useless for concurrency (use `std::atomic<bool>`); `std::shared_ptr` reference count is atomic but the pointee is not; concurrent `std::vector` access (any non-const method requires exclusive access); double-checked locking with non-atomic pointer.

**Tooling**: ThreadSanitizer, Helgrind, Relacy.

### Go
**Primitives**: goroutines, channels, `sync.Mutex`, `sync.RWMutex`, `sync.WaitGroup`, `sync.Once`, `sync.Map`, `sync/atomic` (typed atomics since 1.19), `context.Context`.

**Pitfalls**: goroutine leaks (no exit condition); unbuffered channel deadlock; `select` with default in a tight loop; closing a channel from the receiver; closing while another goroutine still sends; copying a `sync.Mutex` by value (`go vet` catches); capturing the loop variable in a goroutine (fixed for `for` loops in 1.22 but still a footgun in older code and other shapes).

**Tooling**: `go test -race` should run on every Go project; `go vet`, `pprof`, `go tool trace`.

### Python
**Primitives**: `threading.*`, `multiprocessing.*`, `asyncio.*`, `queue.Queue`.

**Pitfalls**: GIL doesn't make `+= 1` atomic on shared ints; mixing threading and asyncio carelessly; `asyncio.run` from inside a running loop; blocking I/O in async coroutines (`time.sleep` instead of `await asyncio.sleep`); Python 3.13+ free-threaded mode -- C extensions may not be thread-safe.

**Tooling**: `py-spy`, `PYTHONASYNCIODEBUG=1`, `aiomonitor`.

### JavaScript / Node
**Primitives**: single-threaded event loop, `Promise`, `async`/`await`, `worker_threads`, `MessageChannel`, `BroadcastChannel`, `AsyncLocalStorage`, `AbortController`.

**Pitfalls**: unhandled promise rejections; fire-and-forget promises; blocking the event loop with synchronous CPU work; `process.nextTick` starvation; missing `await` on side-effecting promises; `Promise.all` short-circuits (use `Promise.allSettled` when you want all results).

### JavaScript browser
**Primitives**: Web Workers, SharedArrayBuffer (cross-origin isolation required), `Atomics.*`, MessageChannel.

**Pitfalls**: SAB unavailable without COI headers; `Atomics.wait` throws on the main thread (forbidden); structured-clone overhead on large objects (use Transferable for zero-copy); main-thread sync blocking.

### Kotlin
**Primitives**: coroutines, `suspend` functions, `CoroutineScope`, `Job`, `Channel`, `Flow`, `Mutex` (kotlinx), structured concurrency via `coroutineScope { }` / `supervisorScope { }`, `Dispatchers.{Default, IO, Main}`.

**Pitfalls**: launching on `GlobalScope` (no structured ancestor; leaks); `runBlocking` inside a coroutine context (blocks the carrier thread); cooperative cancellation (check `isActive`, `yield()`); `Flow` operators on shared mutable state.

### Swift
**Primitives**: GCD (`DispatchQueue`), async/await (5.5+, 2021), `actor`, `Sendable` protocol, `Task`, `TaskGroup`, `MainActor`. Swift 6 (2024): data-race checking by default.

**Pitfalls**: capturing non-`Sendable` in `Task`; actor reentrancy (when an actor awaits, other tasks can interleave on the actor -- mid-await invariants can break); `MainActor` isolation violations; mixing GCD and async/await.

### Erlang / Elixir
**Primitives**: processes, `spawn`, `!` (send), `receive`, `link`/`monitor`, OTP behaviors (`gen_server`, `gen_statem`, `supervisor`), ETS.

**Pitfalls**: mailbox overflow under load; selective receive on an unbounded queue; ETS access without read/write-concurrency tuning; assuming ordering of messages from different senders (per-sender FIFO only); large message payloads (copied to receiver's heap).

### Clojure
**Primitives**: atoms (CAS), refs (STM), agents (async), `core.async` channels, virtual threads (JDK 21).

**Pitfalls**: side effects inside `swap!`/`alter` (retried; must be idempotent or moved outside); `core.async` go-blocks with blocking calls (block the underlying pool).

### Haskell
**Primitives**: `forkIO`, `MVar`, `TVar` + `STM`, `Chan`, `Async` library, `ThreadKilled` exception, `bracket` for cleanup.

**Pitfalls**: `MVar` deadlocks; lazy evaluation interacting with concurrent thunks (`unsafePerformIO` is a trap); STM transactions with `IORef` outside `TVar`.

### Rust (non-async)
**Primitives**: `std::thread`, `std::sync::Mutex` / `RwLock`, `std::sync::Arc`, `std::sync::atomic::*`, `std::sync::mpsc`, `crossbeam`, `parking_lot`, `rayon` (data parallel).

**Pitfalls**: `std::sync::Mutex` is non-reentrant by design (lock twice = deadlock); `Mutex::lock()` returns `LockResult` (poisoning) -- unwrap considered acceptable in most code but the poisoning behavior is real; `RwLock` writer starvation under read pressure; `Send` / `Sync` bounds (compile-time but easy to misunderstand).

**Note**: Rust *async* (Tokio, Future, Pin, async/await) is **out of scope for this agent** -- route to `rust-async`.

---

## What is NOT a concurrency finding

- **Patterns within a single function that bug-hunter's race section already catches.** If the issue is a clear single-line TOCTOU pattern, that's `bug-hunter`'s lens; mention "see also: bug-hunter" rather than duplicating.
- **Distributed-systems concerns** (retries, idempotency keys, fencing tokens, sagas, queue backpressure). Route to `distsys-runtime`.
- **Performance under contention** as a primary concern (raw speedup, scaling). Route to `performance`. We flag correctness defects that *also* hurt performance, but not lock-shape choices made for speed without correctness impact.
- **Async Rust specifically.** Route to `rust-async`.
- **Code that's deliberately racy** with documented justification (statistics counters where slight drift is acceptable; lossy-by-design event sampling). Flag only if the documentation is missing.
- **GC concerns disguised as concurrency** (memory pressure, GC pause variance). Route to `performance`.

---

## Severity calibration

Using `panel-contract.md`'s rubric; specific calibration:

- **blocker**: a reachable data race (UB in C/C++/Rust; partially-defined elsewhere); a deadlock with a documented trigger; a fire-and-forget task in a critical path; a memory-ordering bug that allows safety-critical state to escape uninitialized; an actor with unbounded mailbox under realistic load; cancellation that leaves a resource leaked.
- **major**: a race condition (not technically a data race but timing-dependent) on important state; a lock held across an `await` in user-facing paths; a goroutine without a documented exit; a thread pool sized inappropriately for the workload; missing cancellation propagation in structured concurrency.
- **minor**: false sharing on a non-hot path; redundant `seq_cst` where `acquire`/`release` would do; missing `Send`/`Sync` rationale on a public Rust type.
- **nit**: style-level (e.g., `std::sync::Mutex` where `parking_lot::Mutex` is project convention).
- **insight**: structural -- "this module's concurrency primitives are bespoke; consider adopting structured concurrency"; "the thread-pool topology has accreted three pools; consider unifying".

Confidence: high when the trigger path is concrete and the synchronization is visibly missing or wrong; medium when the access pattern is ambiguous from the snippet alone (the agent must reason about callers).

---

## Process for the concurrency agent

1. **Identify the runtime model.** Single-threaded? Threads + locks? Async (which runtime)? Actors? Coroutines? Multiple? The pitfalls differ per model.
2. **Read project conventions.** `CLAUDE.md`, any `docs/concurrency.md`, any `STYLE.md`, lint rules that encode concurrency policy.
3. **Walk the bug-shape catalog**: races, deadlocks, livelock/starvation, lock-free hazards, visibility, async-specific, actor-specific, channel-specific, initialization races, cache effects, thread-pool pathologies, cancellation.
4. **For each shared-mutable-state site, ask "what synchronizes access here?"** If no answer, that's a finding.
5. **For each `await` / `yield` / `select!`**, ask "what state crosses this suspension point, and is it safe to be cancelled here?"
6. **For each spawned task / thread / goroutine**, ask "what's the exit condition? Who handles errors? Is it joined?"
7. **For each memory-ordering choice**, ask "is this ordering sufficient for what depends on it? Is it more than necessary?"
8. **Route specifics to neighbors.** Single-line patterns to `bug-hunter`; async Rust to `rust-async`; cross-process to `distsys-runtime`.
9. **Stay read-only.**
