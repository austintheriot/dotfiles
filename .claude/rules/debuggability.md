# Debuggability

A reference for evaluating whether code provides the means to observe, inspect, and reproduce its runtime behavior during development and incident response. Used by the `debuggability` subagent. Distinct from `observability.md` (which is production telemetry: SLOs, alerts, sampling, span hygiene for the operator) -- this file is the *developer-facing* lens: can I set a breakpoint here, will the stack trace help me, can I attach a debugger to a worker, do panics carry source location, can I reproduce locally?

The two domains overlap (structured logs are useful in both) but the priorities differ. Production observability optimizes for low-cardinality bounded telemetry that survives sampling; dev-time debuggability optimizes for breakpoint-able code, useful errors, reproducible execution, and tooling that the team actually uses.

The core thesis: code that's hard to debug compounds every incident. Five minutes added to the median "how do I see what's happening here" lookup turns into hours when paired with a deadline. Debuggability is the property that says "future-you, or your teammate at 2am, can answer 'what is this doing right now' in seconds, not minutes."

---

## Universal principles

Cross-language, cross-runtime. The shape of "debuggable code" is roughly the same everywhere:

### Errors carry context
- **Errors should preserve the call site, the inputs, and the cause.** An error that says "ParseError" is worth less than one that says "ParseError at field `users[3].email`: expected RFC 5322 address, got 'NULL'". The reader needs to know *what*, *where*, and *why*, not just *what*.
- **Error wrapping preserves the chain.** `?`-propagation in Rust, `try { ... } catch (e) { throw new Error("during user import", { cause: e }); }` in JS/TS, exception chaining in Python. Stripping the cause hides the root.
- **Stack traces should reach the user's code.** A 40-frame stack ending in framework internals is less useful than a 5-frame stack ending in the user's call site. Library boundaries should mark themselves (blackboxing in DevTools, `#[track_caller]` in Rust) so user frames stay visible.

### State is observable
- **Important state is reachable from the debugger, the REPL, or the logs.** Hidden state inside a closure that's invoked once and discarded is debugging-hostile. State held on a named object or module is inspectable.
- **Identifiers travel with the work.** Request IDs, trace IDs, job IDs, user IDs propagated through every step so that "find everything related to *this* failure" is a query rather than a guess.
- **Distinct things have distinct names in logs and errors.** "Worker started" / "Worker started" / "Worker started" tells you nothing. "Worker[id=abc123, queue=mail] started" lets you correlate three log lines with one job.

### Code is reproducible
- **The behavior depends only on inputs the developer can supply locally.** A function that depends on `Date.now()`, `Math.random()`, a network call, or a feature-flag service is harder to reproduce than one that takes a clock, an RNG, a client, and a flag value as parameters. (See `coding-style.md`: push effects to the edges.)
- **State is restorable.** A bug in a stateful pipeline is reproducible if the developer can replay the input sequence; not, if they can only "wait for it to happen again."
- **Test fixtures match the failing shape.** If production breaks on a 12MB JSON payload with a specific Unicode sequence, the developer should be able to load that payload into a test, not generate a synthetic minimum.

### Breakpoints are reachable
- **Source maps work.** TypeScript/JavaScript without working source maps debugs at the wrong line, in the wrong file, in transpiled output the reader doesn't recognize. The debug-tooling pre-flight is "does the breakpoint hit where I set it?"
- **Async stacks are visible.** Promises, futures, callbacks, channels all have a "what called this" answer; whether the debugger surfaces it depends on the runtime. Code that obscures the async chain (manual `setTimeout`/`setImmediate`, raw thread spawns, untraced workers) makes the chain unrecoverable.
- **Hot paths are interruptible without performance dependence.** Code that only triggers the interesting branch under specific load is a poor target for `debugger`; design for the interesting branch to be testable in isolation.

### Reasonable verbosity is available
- **Logs at different levels.** TRACE/DEBUG/INFO/WARN/ERROR with the right messages at the right levels. The developer enables DEBUG temporarily to debug; production runs at INFO. Code that puts the useful information at DEBUG but never logs at INFO is harder to inspect by attachment.
- **Dynamic verbosity is possible.** Re-routing log output, increasing a specific module's verbosity, attaching a debugger to a running process -- the right move depends on what's broken. Hard-coded `console.log` everywhere is the worst of both worlds: noisy in prod, can't be turned off, can't be turned up.

### Tooling presence implies tooling use
- **The repo's debug tooling should be discoverable and configured.** If the project has `--inspect` flags, source map config, `tracing-subscriber` setup, browser-side debug builds -- they should work out of the box for a new developer. The agent should check: does the README mention how to attach a debugger? Is there a `launch.json` or `.vscode` config that just works? Are debug builds documented?

---

## Per-environment tooling

The agent should match the code's runtime and check whether the project uses the environment's debug toolkit. Where the toolkit is absent, the absence itself can be a finding (often `insight` severity).

### Node.js

- **`node --inspect` / `node --inspect-brk` / `node --inspect=0.0.0.0:9229`** -- attaches the V8 inspector for Chrome DevTools or VS Code. `--inspect-brk` breaks on first line; `--inspect` runs and waits for attach. `chrome://inspect` lists running Node targets. Use this over `console.log` for stepping through anything non-trivial.
- **`process.report.getReport()` / `process.report.writeReport()`** -- snapshot the entire process state (heap, stack, libuv state, env, resource usage) as JSON. Available since Node 11.8. Critical for "why did it hang."
- **Diagnostic channels (`node:diagnostics_channel`)** -- structured event emission with subscribers; preferred over scattered emit/log calls for cross-cutting diagnostics. Integrates cleanly with `tracing`-style libraries.
- **`async_hooks`** -- low-level tracking of async resource lifecycle. Powerful but slow; the right tool for diagnosing leaked timers, hanging promises, async context loss. Most production code shouldn't use it directly; `cls-hooked` or `AsyncLocalStorage` are the user-facing wrappers.
- **`AsyncLocalStorage`** -- propagates context (request ID, user ID) across async boundaries without explicit threading. The Node equivalent of Rust's `tokio::task_local!` or Python's `contextvars`. Code without it in a multi-request server has a debuggability gap: you can't correlate logs to requests.
- **`--prof` + `node --prof-process`** -- V8 sampling profiler; produces a tick-by-tick flamegraph. Use for "why is this slow" when DevTools' Performance tab is unavailable (server-side, headless).
- **clinic.js (`clinic doctor`, `clinic flame`, `clinic bubbleprof`)** -- higher-level performance and event-loop diagnosis. `clinic doctor` will tell you "I/O bound," "event-loop blocked," "memory churn." Useful when you don't yet know what the problem is.
- **`--enable-source-maps`** -- enables source map support in stack traces. Without it, TS-compiled code shows compiled positions, not source. Default in Node 16+.
- **`util.inspect.defaultOptions`** -- control the depth/format of `console.log(obj)` output. Default depth (2) hides important state; bump to 5 or `null` for debugging.
- **`v8.getHeapSnapshot()` / `v8.writeHeapSnapshot()`** -- programmatic heap snapshots for memory-leak investigation. Open in Chrome DevTools' Memory tab.

### Web / JavaScript / Browser

- **`debugger;` statement** -- a hard-coded breakpoint. Triggers when DevTools is open. Useful when you can't reach the code from DevTools' UI (event handlers fired by remote code, dynamically-loaded scripts).
- **DevTools logpoints** (right-click a line gutter, "Add logpoint"): logs an expression every time the line is hit, *without modifying the source*. Strictly better than scattered `console.log` -- no edit/save/refresh cycle, no leftover logs to clean up.
- **Conditional breakpoints** -- only break when an expression is true. The right tool for "this runs 10000 times and fails on the 7842nd."
- **`console.table(arr)` / `console.group()` / `console.trace()` / `console.assert()`** -- richer than `console.log`. Underused. `console.trace()` prints a stack trace; `console.assert(cond, msg)` only logs when the condition is false (useful for invariants).
- **`monitor(fn)` / `monitorEvents(el, eventName)`** -- DevTools-only console helpers. `monitor(myFn)` logs every call to `myFn` with arguments; `monitorEvents($0, 'click')` logs every event on the inspected element.
- **`$0` / `$_`** -- the currently inspected element / the last expression result. Trivial; saves typing.
- **Source maps** -- non-negotiable for any non-trivial frontend. The check: open DevTools sources, find a `.ts` or `.jsx` file in the tree, set a breakpoint, verify it hits. If the build pipeline strips or breaks source maps in production-mode builds, breakpoints land in transpiled gibberish. `eval-source-map` vs `source-map` vs `inline-source-map` vs `cheap-module-source-map` -- pick deliberately, document the choice.
- **Blackboxing** -- DevTools setting to skip framework code (React internals, Webpack runtime, polyfills) when stepping. Without it, every step-into lands in `react-dom.development.js`. Setting > Frameworks > Blackbox; or `// webpack-blackboxed` source comments.
- **Workspace folders (DevTools)** -- map a DevTools page to a local folder so edits in DevTools save to disk. Underused; powerful for tweaking layout/style in DevTools and committing the result.
- **`window.__REDUX_DEVTOOLS_EXTENSION__` / `window.__REACT_DEVTOOLS_GLOBAL_HOOK__`** -- framework-specific hooks. Verify they're attached in dev builds. Missing them = degraded debuggability for state-driven UI.
- **Network panel + replay** -- right-click a request, "Copy as cURL" reproduces it locally. The browser is a debugger for the network layer.
- **`Performance.measureUserAgentSpecificMemory()`** (Chrome 89+) -- isolated process memory measurement. Useful for memory leak hunts.
- **Mutation Observers / Performance Observers** -- programmatic visibility into DOM changes and performance entries. The right tool when "the UI flickers and I can't see why."

### Chrome (browser-specific)

- **`chrome://inspect`** -- one page lists every inspectable target: tabs, workers, Node processes connected via `--inspect`, WebViews, devices via USB debugging.
- **`chrome://tracing`** (deprecated, but the descendant `chrome://tracing` and Perfetto-based UI live on) -- low-level timeline of Chrome's internal events. Almost always overkill for app code; necessary for rendering-pipeline pathologies.
- **Live Expressions** in the DevTools console -- pin an expression and watch it update in real time as the page changes. Underused; eliminates "log and refresh" loops.
- **Coverage panel** -- shows which CSS/JS bytes ran. Useful for "what code isn't even loaded for this page."
- **Lighthouse + Performance Insights** -- diagnostic that maps performance issues to underlying causes ("layout shift caused by font loading," "main-thread blocked for 240ms by script X"). The bridge from "site feels slow" to "specific finding."

### Rust (native, server, desktop)

- **`dbg!(expr)`** -- print expression with file/line and the value, return the value. `let x = dbg!(some_computation());` is the canonical "what is this right now" tool. Strictly better than `println!` for ad-hoc inspection.
- **`RUST_BACKTRACE=1`** (or `=full`) at runtime -- produces a backtrace on panic. `=full` includes hidden frames; `=1` is the user-readable subset. Without it, panics print a one-line message and the developer has to guess. **In any Rust binary the developer might run, the agent should check whether the README/setup mentions setting this.**
- **`RUST_LIB_BACKTRACE=1`** -- enables backtraces in `Result::Err` values (when using `anyhow` or `eyre`). Independent of the panic backtrace.
- **`#[track_caller]`** -- marks a function such that panics inside it report the *caller's* location, not the function's. Apply to helpers like `unwrap_or_panic`, custom assertions, library-internal "this can't happen" paths. Without it, the panic blames your assertion helper, not the caller that triggered it.
- **`tracing` + `tracing-subscriber`** -- structured logging with spans. `tracing` is the de facto Rust answer for both dev and prod observation: instrument once, decide at deploy-time how verbose. Combine with `tracing-tree` for human-readable hierarchical output during development.
- **`tokio-console`** -- live introspection of a Tokio runtime: which tasks exist, which are stuck on what, how long each held the runtime. The right tool for "my async server hangs and I don't know why." Requires `console-subscriber` in the binary; off in production builds.
- **`cargo-flamegraph`** -- `cargo flamegraph --bin myapp` produces a flamegraph SVG. The fastest path from "this is slow" to "this function on this stack is slow."
- **`rust-gdb` / `rust-lldb`** -- shipped with rustup; pretty-print `Option`, `Vec`, `HashMap`, etc. Vanilla `gdb`/`lldb` work too but the output is C-shaped.
- **`cargo-expand`** -- expands macros so you can read what `#[derive]` and proc-macros actually generated. Critical when a derive misbehaves.
- **`cargo-nextest`** -- faster test runner with better failure reporting (per-test output, parallelism control, JUnit XML for CI). Strictly better than `cargo test` for any project with non-trivial test count.
- **`miri`** -- interpreter that detects undefined behavior. Run `cargo miri test` on unsafe code; it catches use-after-free, uninitialized reads, ABA, and pointer-provenance bugs that `cargo test` will pass. Slow; targeted use.
- **`cargo-asm`** -- inspect the generated assembly for a function. Niche but the right tool for "why is this hot loop slow."
- **`panic = "abort"` vs `panic = "unwind"`** -- the choice affects whether `catch_unwind` works and whether destructors run on panic. Document the choice; debugging difference can be invisible until it bites.
- **`std::backtrace::Backtrace`** (stable since 1.65) -- programmatic backtrace capture. The basis for error types that carry their origin.
- **`thiserror` / `anyhow` / `color-eyre`** -- error libraries that preserve context, location, and chains. `thiserror` for typed errors at API boundaries; `anyhow`/`eyre` for application code. The pattern: structured errors at library boundaries, contextful messages at application boundaries.

### Rust web servers (axum, actix, tower)

- **`tower-http`'s `TraceLayer`** -- structured request/response tracing across the middleware stack. Set up once; gets you request-id propagation, latency, status, span hierarchy.
- **Request-id propagation** -- a single ID that travels from the inbound request through every span, every downstream call, every log line. `tower-http::request_id::SetRequestIdLayer` + `MakeRequestUuid` is the default pattern.
- **Structured error responses with internal correlation IDs** -- the response shows the user "request 12345 failed"; the logs show 12345 with the full trace. The user can report the ID and the team can find everything related.

### Rust + WebAssembly

- **`console_error_panic_hook`** -- on panic, prints a useful error to the browser console *with* the Rust stack. Without it, a Rust panic in WASM prints "RuntimeError: unreachable executed" -- almost useless. Add it to every WASM crate's `main` or `start`.
- **`wasm-bindgen` debug builds** -- `wasm-pack build --dev` keeps debug info and human-readable names; release builds strip everything. The agent should check the build script: is `--dev` used for local development?
- **DWARF in Chrome DevTools** -- Chrome 88+ supports stepping through Rust source in the WASM debugger, with breakpoints in `.rs` files. Requires the WASM binary to be built with DWARF debug info enabled. Install the "C/C++ DevTools Support (DWARF)" extension; enable "WebAssembly Debugging: Enable DWARF support" in DevTools settings.
- **`wee_alloc` vs the default allocator** -- `wee_alloc` is smaller but has worse debugging tooling; the default `dlmalloc` integrates better with browser memory profilers. Default for dev; `wee_alloc` only if size matters and you've measured.
- **Source maps for the JS glue** -- `wasm-bindgen` generates JS bindings; their source maps need to be configured in the bundler. Without them, errors in the glue layer show up at compiled positions.
- **`wasm-objdump` / `wasm2wat`** (from WABT, the WebAssembly Binary Toolkit) -- inspect the WASM binary or convert it to text format. Niche but the right tool when "the binary itself is the suspect."
- **Profiling WASM**: Chrome's Performance tab shows WASM frames in flamegraphs (with DWARF for source-level attribution). Server-side WASM (wasmtime, wasmer) has its own profilers but the tooling is less mature.

---

## What to flag and how

The `debuggability` subagent looks for **gaps** in observability-during-development, not for absence of every possible tool. The right framing per finding:

### Concrete gap patterns

- **Errors thrown/returned without context.** `throw new Error("invalid input")` -- no input value, no call site, no cause. Replace with `throw new Error(\`invalid input: expected X, got \${JSON.stringify(input)}\`, { cause: original })`.
- **Panics / unwraps in code paths reachable from external input.** Every `unwrap()` / `!` / `expect()` / `as!` is a potential mystery crash. The fix isn't always to eliminate them -- some unwraps are correct because the invariant holds upstream -- but if the path is reachable, the panic should at least carry context (`expect("user_id was None even though authenticated")`).
- **Async work without identifiers.** A `tokio::spawn` / `Promise.then` / `setTimeout` with no name and no span makes failures untraceable. Where the runtime supports it, name the span or task.
- **Logging at the wrong level or with no useful information.** `console.log("entering function")` -- noise. `tracing::debug!(user_id = %user.id, action = "subscription_change", from = %old, to = %new)` -- useful at the right level.
- **Hidden state.** A module-level mutable cache with no accessor for inspection; a closure-trapped variable that holds the only reference to a bug-relevant value.
- **Reproducibility blockers.** Hardcoded `new Date()`, `Math.random()`, network calls without test doubles, file system reads with no test fixture. Each one is a "can't reproduce locally" failure waiting to happen.
- **Stack traces that don't reach user code.** Long framework stacks with no boundary marker. In TypeScript, missing `Error.captureStackTrace`. In Rust, missing `#[track_caller]` on helper functions. In Node, missing `--enable-source-maps` in the runtime config.
- **Debug builds that don't preserve symbols.** Production-mode Webpack builds checked in to dev environments; release-mode Rust binaries with `strip = true` used by developers; WASM binaries built without DWARF.

### Tooling absence as a finding

The agent should also flag *missing standard tooling* for the runtime, especially when the cost of adding it is small:

- Node project without `--inspect`-friendly setup (no documented debug flow).
- TypeScript project without working source maps (verify by inspecting the build output).
- Rust binary without `RUST_BACKTRACE` mentioned in the README or set in `.env` / `.cargo/config.toml`.
- Rust WASM crate without `console_error_panic_hook`.
- Async Rust project without `tracing` (instead of `log` or `println!`).
- Browser app without React/Redux/Vue devtools hooks in development builds.
- Server-side service without request-id propagation through logs.
- Project with no documented "how to attach a debugger" path.

### Severity

- **major**: a debug-time blocker. Panics with no context in a reachable path; async work that can't be traced; broken source maps; production-mode builds in development.
- **minor**: friction. Logs at the wrong level; missing debug helpers; reproduction requires effort.
- **nit**: cosmetic. Inconsistent log formats, missing structured fields on otherwise-fine log lines.
- **insight**: structural debuggability observation. "This module's state machine is impossible to inspect mid-transition; consider exposing the current state as a read-only field." "The project would benefit from adopting `tracing` over the current ad-hoc `println!` calls."

Confidence: high when the gap is verifiable from the code ("this `unwrap()` on line 42 has no expect-message and is reachable from any HTTP handler"); medium when the gap depends on the runtime/build configuration the agent cannot fully observe.

---

## What is NOT a debuggability finding

- **Production-grade observability concerns.** Span design, sampling, cardinality, SLOs are owned by `observability-practice` / `otel-instrumentation` / `otel-pipeline`. The debuggability agent flags dev-facing gaps; if a span attribute is wrong for *Honeycomb cardinality*, that's the observability agent's lens.
- **Test coverage.** Owned by `test-coverage`.
- **Code that's deliberately opaque for security reasons.** Authentication paths that don't log secrets, crypto operations that don't print intermediate state -- those are correct, not debug-hostile.
- **Code in performance-critical hot paths where debug builds are documented to be slow.** A SIMD loop with no per-iteration logging is correct; the debuggability path is elsewhere (representative test inputs, breakpoint at the boundary).
- **Verbose dev-time output that the project has deliberately turned off in prod.** That's the design, not a bug.

---

## Process for the debuggability agent

1. **Read the project's setup docs.** README, CONTRIBUTING.md, `.vscode/launch.json`, `.cargo/config.toml`, `package.json` scripts, `Makefile`, `justfile`. Identify the documented debug workflow.
2. **Identify the runtime(s).** Node? Browser? Rust native? Rust WASM? A mix? Each has its own tooling baseline.
3. **Walk the code given** for the gap patterns above. Anchor findings to file:line.
4. **Check for the runtime's standard tooling.** For Node: source maps, `--inspect` support, `AsyncLocalStorage` for request context. For Rust: `tracing`, `RUST_BACKTRACE`, `#[track_caller]` on helpers. For Rust WASM: `console_error_panic_hook`. For browser: source maps, framework devtools.
5. **Flag tooling absence as findings** when the cost is small and the value is high.
6. **State concrete fixes.** "Add `console_error_panic_hook::set_once()` to the WASM start function" beats "improve panic debugging."
7. **Stay read-only.** Suggest; do not apply.
