---
paths:
  - "**/*.{wat,wasm}"
  - "**/*wasm-bindgen*"
  - "**/Cargo.toml"
---

# WebAssembly

A reference for evaluating WebAssembly code from a correctness, performance, security, and modern-best-practice lens. Used by the `webassembly` subagent.

The scope: **WebAssembly at the spec level** (linear memory, modules, imports/exports, validation), the proposal landscape and shipping status (SIMD, threads, GC, exceptions, Component Model, WASI Preview 2, tail calls, stack switching), runtime environments (V8 / SpiderMonkey / JSC in browsers; Wasmtime, Wasmer, WAMR, wazero standalone), the JS↔WASM boundary, the Component Model and WIT, WASI's capability model, toolchain choices (Emscripten, WASI-SDK, wasm-bindgen, cargo-component, wit-bindgen, jco, TinyGo, AssemblyScript), security model, current shortcomings.

Distinct from:
- **`rust-wasm`**: Rust-to-WASM specifically -- the Rust subagent. We discuss Rust WASM but the deep Rust angle is theirs.
- **`rust-ffi`**: FFI generally -- Rust-to-C / Rust-to-Python / etc. We touch the JS↔WASM and host↔WASM boundary specifically.
- **`security`**: general security. We flag WASM-specific capability concerns (WASI grants, supply chain of components) that the general agent doesn't have the depth for.
- **`performance`**: general performance. We flag WASM-specific perf (boundary cost, JIT tier transitions, allocation patterns in WASM-targeted code).

The core thesis: **WebAssembly is a target, not a language**, and reviewing WASM code requires recognizing the specific patterns where the target's properties (sandbox, capability model, boundary cost, JIT compilation tiers, memory growth semantics) reshape what's safe and fast.

The empirical priority: **most WASM bugs are at the boundary** -- between the WASM module and its host (JS in browsers, the runtime in standalone, the embedding in plugins). Marshalling strings per call, holding `Uint8Array` views across `memory.grow`, allocating WASM memory per call, calling back to JS for each data element in a loop -- these compound and dominate the perf profile.

---

## Universal principles

### The spec is the contract; the engine is the implementation

WebAssembly has a formal spec. Engines (V8, SpiderMonkey, JSC, Wasmtime, Wasmer, WAMR, wazero) implement subsets at varying paces. Code targeting a feature must verify the feature's CG (Community Group) phase status and the runtime's implementation status.

CG phase model: Phase 1 (Feature Proposal) → Phase 2 (Proposed Spec Text) → Phase 3 (Implementation Phase) → Phase 4 (Standardize) → Phase 5 (Featured in W3C Standard).

**Flag**: code using a Phase 1 or Phase 2 feature in production without explicit feature detection; code assuming a Phase 4 feature is universally available without checking the target runtimes.

### The boundary dominates

The JS↔WASM boundary (or host↔WASM in standalone) costs ~100ns per call in modern engines, plus the marshalling cost of arguments (strings copied, objects translated to handles). For numeric data, the boundary is fast (typed arrays passed as `(ptr, len)`); for strings and complex objects, it's slow.

**Flag**: per-call string marshalling in hot paths; allocation of WASM memory per call; JS-to-WASM-to-JS callbacks per element in a loop; `console.log` from inside a hot WASM loop (the JS call dominates).

### Linear memory grows in 64KB pages; views detach on grow

`memory.grow` extends the WASM heap. JS-side `Uint8Array` / `Float32Array` / etc. views into WASM memory **detach** when the memory grows -- the view becomes a zero-length array. Reacquire the view after every grow.

**Flag**: code holding `Uint8Array` views across `memory.grow`; no `memory.grow` failure handling (grow can fail; OOM crashes the module); excessively large initial memory "to avoid grows" wasting resources.

### Capabilities are the security model

WASI Preview 1/2 uses capability-based security. A module can do nothing by default; capabilities are granted via imports. A module with only `wasi:clocks/wall-clock` can read the clock; it can't open files, connect to the network, or read env vars.

**Flag**: WASI granting filesystem at root when the module needs only `/tmp` (narrow the preopen); granting full env when the module needs specific vars; granting `wasi:sockets` or `wasi:http` outbound without an allowlist (SSRF risk); modules from registries executed with broad capabilities without provenance verification.

---

## WebAssembly at the spec level

### MVP (1.0, 2017)

The original spec:
- 32-bit linear memory (4GB max).
- i32, i64, f32, f64 only.
- Modules, imports, exports, start function, tables (for function pointers).
- Stack-based VM semantics; type-checked at validation time.
- **Structured control flow**: block / loop / if / br / br_if / br_table. No arbitrary `goto`.

### What MVP deliberately omitted

- **Threads, atomics**: deferred (Phase 4 now, shipped).
- **SIMD**: deferred (Phase 4 now, shipped).
- **GC**: deferred (Phase 4 now, shipping 2024 in V8 / SpiderMonkey).
- **Exceptions**: deferred (Phase 4 now, shipped).
- **64-bit memory addresses**: deferred (Memory64, Phase 3).
- **Multiple memories**: deferred (Phase 3).

### Post-MVP, shipped in MVP-relevant engines

- **Bulk memory operations (2020)**: `memory.copy`, `memory.fill`, `table.copy`, `table.init`. Critical for compact code emission.
- **Reference types (2020)**: `externref`, `funcref`. Hold opaque host references without copying.
- **Multi-value (2020)**: functions return multiple values.
- **Mutable globals (2017)**: across-instance mutation.
- **Sign extension operators**, **non-trapping float-to-int**.
- **SIMD / fixed-width 128-bit (2021)**: `v128`, lane ops.
- **Threads / atomics (Phase 4, shipped)**: shared linear memory, atomic ops, futex-like wait/notify. Requires SharedArrayBuffer + COOP/COEP in browsers.
- **Tail calls (2024)**: `return_call`, `return_call_indirect`. Enables functional-language compilers (Scheme, OCaml).
- **WASM-GC (2024)**: structs, arrays, references with GC. Enables managed-language compilation (Kotlin/WASM, Dart-WASM, Java via TeaVM).
- **Exception handling (Phase 4)**: try / catch / throw / rethrow. Important for C++ / Java / Python.

### In flight (Phase 2-3)

- **Component Model (Phase 2)**: composable WASM components with typed interfaces (WIT).
- **Stack switching (Phase 2)**: fibers / coroutines / async. Important for emulating green threads without compiler transforms.
- **Memory64 (Phase 3)**: 64-bit linear memory addresses.
- **Multiple memories (Phase 3)**.

---

## The JS↔WASM boundary

### What it costs

- ~100ns per call in modern engines (V8, SpiderMonkey).
- Argument marshalling: numbers fast (i32/f32/f64 pass directly); strings expensive (UTF-8 / UTF-16 conversion, copy into WASM memory); objects expensive (handles for externref or full serialization).
- Return values: same cost structure.
- `instantiateStreaming` is faster than `instantiate` (streaming compile during fetch).

### Rules of thumb

- **Bridge once, not per-call**: pass batches of data, not per-element calls.
- **Numbers cross cheaply; strings cross expensively**: design APIs around numeric primitives + buffer slices.
- **Hot paths stay inside WASM**: a hot loop should not call out to JS per iteration.
- **`postMessage` of WASM modules**: structured-clone cost is real for large modules.

### Tools that generate the boundary

- **wasm-bindgen** (Rust): generates JS glue + Rust-side bindings. Handles strings, objects, closures, async.
- **Emscripten** (C/C++): generates JS glue; supports POSIX-ish via WASI-libc or its own libc; large generated runtime.
- **Embind** (Emscripten): C++ class binding to JS.
- **jco**: WASM-Component to JS adapter; runs components in Node / browsers.
- **AssemblyScript**: TypeScript-like syntax compiling to WASM; minimal runtime.

### What goes wrong at the boundary

- **String marshalling per call** in a hot path.
- **Allocating WASM memory per call** (wasm-bindgen-generated code may default to this).
- **`console.log` from a hot loop** in WASM.
- **Holding `Uint8Array` views across `memory.grow`** (detached).
- **JS↔WASM↔JS callbacks per element** in a loop.
- **Synchronous `instantiate`** instead of `instantiateStreaming`.

---

## The Component Model and WIT

The most consequential current development.

### What it solves

The pre-Component Model state: every language compiled to WASM has its own memory layout, ABI, calling conventions. Cross-language interop requires per-pair glue.

The Component Model defines:
- **Components**: higher-level packages wrapping modules with typed interfaces.
- **WIT (WebAssembly Interface Types)**: IDL for declaring component interfaces. Records, variants, lists, options, results, resources.
- **Canonical ABI**: how WIT types are passed across the boundary -- in-memory representation, ownership, lifting / lowering.
- **Composition**: components can be composed (one's exports → another's imports), producing a new component.
- **Resources**: linear types with constructors and destructors for handles (file descriptors, etc.).

### Why it matters

- Cross-language WASM: a Rust component and a JS-compiled-to-WASM component can talk via the Component Model without knowing each other's internals.
- WASI as components: WASI Preview 2 is defined as components.
- Plugin ecosystems: the polyglot plugin layer.
- "WASM replaces Docker" (Solomon Hykes, 2019): components are the technical substrate for this thesis. Partially vindicated for edge / serverless / plugins; general workloads still use containers.

### Current state (2025)

- WIT toolchain mature for Rust (cargo-component, wit-bindgen).
- JS toolchain emerging (jco).
- Python support via py-wit-bindgen.
- Browsers don't natively support components; jco creates JS shims.
- Standalone runtimes (Wasmtime, jco-via-Node) support components.

### Review patterns

**Flag**: components paired with non-component runtimes (older Wasmtime, Wasmer without component support, browsers without jco shim); composing untrusted components without provenance verification; mismatched WIT versions between host and component (breaking changes in `wasi:http@0.2.0` vs `@0.2.1`); overly broad worlds (a component declaring imports it doesn't need increases trust footprint).

**Skepticism for small projects**: components add toolchain complexity and ABI overhead. For a small project where one language compiles to WASM and talks to one host (browser JS or one runtime), components may be overkill. Flag adoption decisions that don't justify the complexity.

---

## WASI and the capability model

### WASI Preview 1 (2019-2023)

- POSIX-flavored: file descriptors, paths, env vars, args, time, random, sockets.
- Capability-based: handles passed in; modules can't open files they weren't given.
- Single-process / single-thread / blocking model.
- Stable enough for production (Fastly Compute@Edge, others).

### WASI Preview 2 (2024)

- Component-Model-native.
- **Worlds**: collections of imported and exported interfaces.
- Modular: a module imports `wasi:io`, `wasi:filesystem`, `wasi:clocks`, etc. Each interface is a separate component.
- Async-capable (Preview 3 will fully embrace it).
- Network sockets, HTTP server (`wasi:http`), CLI.

### The capability model in review

A WASM module can do nothing by default. Capabilities are granted via imports.

**Flag**:
- Granting `wasi:filesystem/preopens` at root when the module needs `/tmp`.
- Granting full env when the module needs specific vars (filter on host side).
- Granting `wasi:sockets` or `wasi:http` outbound without an allowlist of destinations (SSRF risk).
- WASI Preview 1 syscalls in new code (`wasi_snapshot_preview1::*`) when P2 is available.

### P1-to-P2 migration

Not source compatible. A project committed to P1 in 2022-2023 faces migration cost; one starting fresh in 2024-2025 should go to P2.

---

## Toolchain matrix

### C / C++
- **Emscripten** (Alon Zakai): the original; LLVM backend; large generated JS glue; mature. Best for porting browser-targeted C/C++ codebases.
- **WASI-SDK**: clang + WASI-libc; simpler for non-browser use; smaller output.

### Rust
- `wasm32-unknown-unknown`: pure WASM, no system calls.
- `wasm32-wasip1` (formerly `wasm32-wasi`): WASI Preview 1.
- `wasm32-wasip2`: WASI Preview 2.
- **wasm-bindgen** (Alex Crichton): JS↔Rust glue.
- **cargo-component**: build Component Model components from Rust.
- **wit-bindgen**: generate bindings from WIT IDL.

### Go
- **TinyGo** is the primary path: smaller, supports WASI / Component Model.
- **Mainline Go's WASM support**: exists but heavier; not recommended for size-sensitive deployments.

### Python
- **Pyodide**: CPython compiled to WASM; very large runtime; for in-browser Python.
- **MicroPython for embedded WASM**: smaller; less complete stdlib.

### JavaScript / TypeScript
- **AssemblyScript**: TypeScript-like syntax → WASM; minimal runtime; not full TypeScript.
- **Javy**: bundles QuickJS + JS into WASM.
- **jco**: not a compiler; an adapter for running WASM Components in Node / browsers.

### Other
- **Swift**: SwiftWasm.
- **C# / Mono**: Blazor (WASM Mono runtime); Uno Platform.
- **Kotlin**: Kotlin/WASM (uses WASM-GC; new in 2024+).
- **Java**: TeaVM, J2CL → WASM.
- **OCaml**: js_of_ocaml → WASM via wasm_of_ocaml.

### Reviewer mental model

- **Browser, small kernel, no POSIX needed**: WASI-SDK or Rust `wasm32-unknown-unknown` + wasm-bindgen.
- **Server-side, needs filesystem / sockets**: Rust `wasm32-wasip2` + cargo-component, or Go via TinyGo.
- **Plugin in a host runtime**: depends on runtime; Wasmtime / Wasmer / WAMR have different ABIs.
- **Browser, ported large C/C++ codebase**: Emscripten.

---

## Performance

### WASM vs native

- Most workloads: 50-80% of native.
- SIMD workloads: 70-90%.
- Numeric loops: near-native.
- Pointer-chasing / cache-bound: closer to native.
- JIT tiers: Liftoff (fast startup, V8) → TurboFan (steady-state); BaldrMonkey baseline → IonMonkey optimized. Tier transitions cost.

### WASM vs JS

- WASM startup: fast (no parse / compile of source like JS).
- WASM steady-state: comparable to optimized JS for numeric; better for SIMD / parallel.
- JS faster for GC'd workloads (until WASM-GC) and DOM-heavy code.

### Performance ceiling

For workloads where the last 10-50% matters (real-time audio at 1ms latency, ML inference at p99 deadlines, AAA games), WASM may not suffice.

### Memory

- Linear memory grows in 64KB pages.
- Browser memory limited (~2-4GB without Memory64).
- `memory.grow` can fail; modules must handle.
- Allocation strategy: bring your own (Rust's allocator, Emscripten's, custom).

---

## Shortcomings

### Toolchain
- Multiple incompatible toolchains (Emscripten vs WASI-SDK; `wasm32-unknown` vs `wasm32-wasi`).
- Component Model toolchain still maturing; sharp edges.
- Cross-language component composition limited in practice.

### Boundary
- Call overhead non-zero (improving).
- DOM / Web APIs require JS glue.
- String marshalling expensive without typed-array optimization.

### Debugging
- Source maps historically incomplete; improving.
- Chrome DevTools: best WASM debugging in 2024 (DWARF support).
- Firefox: improved DWARF support 2023-2024.
- Logging from WASM: typically via JS console call (boundary cost) or wasi:io/stdio.

### GC for managed languages
- WASM-GC helps; Kotlin/WASM, Dart-WASM use it.
- Without GC: managed-language WASM bundles a GC runtime (~100KB+ overhead).

### Threads
- Browser threads via Web Workers + SharedArrayBuffer (after COOP/COEP).
- Hard for some apps to deploy due to COEP requirements (third-party content).
- Standalone runtimes: native threading available.

### Security
- Sandboxing robust (no syscalls without imports; linear memory bounded).
- Side-channel attacks (Spectre): mitigated by COOP/COEP for SharedArrayBuffer.
- Capability leakage: granting broad caps to untrusted modules is the failure mode.
- Supply chain: components are arbitrary code; verify provenance.

### Performance ceiling for niche use cases
- Real-time audio at 1ms latency: marginal.
- AAA games at native parity: not yet.
- ML inference at p99 deadlines: marginal but improving.

### Browser API surface
- WASM cannot call DOM, fetch, IndexedDB, WebGL, WebGPU, AudioWorklet APIs directly. Every call goes through JS.

---

## Anti-pattern catalog

### Boundary
- String marshalling per call in hot paths.
- Allocating WASM memory per call.
- `console.log` from hot WASM loops.
- Holding `Uint8Array` views across `memory.grow`.
- JS-to-WASM-to-JS callbacks per data element in a loop.
- Synchronous `instantiate` instead of `instantiateStreaming`.

### Toolchain
- Emscripten for a small compute kernel with no POSIX needs (use WASI-SDK or Rust).
- `wasm32-unknown-unknown` for a server-side daemon needing filesystem / sockets (use `wasm32-wasip2`).
- Mainline Go for a plugin when TinyGo would fit better.
- Component Model toolchain pinned to old version (`cargo-component`, `wit-bindgen` move fast).
- `wee_alloc` in 2024+ Rust WASM (deprecated; use default or `lol_alloc`).

### Performance
- No `wasm-opt -O3` post-step for size-sensitive builds (20-40% shrink typical).
- No compression on production WASM (`Content-Encoding: identity`).
- No SIMD in numeric kernels where applicable.
- Hardcoded linear memory limits ignoring platform variance.
- No `console_error_panic_hook` in Rust WASM (panics print uninformative "RuntimeError: unreachable").
- Production builds with debug info stripped and no source-map / symbol archive saved.

### Memory and grow
- No `memory.grow` failure handling.
- `memory.grow` inside hot loops.
- Excessively large initial memory "to avoid grows."
- No resource limits on WASM in standalone runtimes (use Wasmtime's fuel / memory caps).

### WASI / capabilities
- Granting filesystem at root when `/tmp` would suffice.
- Granting full env when specific vars would suffice.
- Granting `wasi:sockets` / `wasi:http` outbound without allowlist.
- P1 syscalls in greenfield code.

### Component Model
- Components paired with non-component runtimes.
- Composing untrusted components without provenance verification.
- Mismatched WIT versions between host and component.
- Overly broad worlds (declaring imports the component doesn't need).

### Debugging
- Source-map-less production builds.
- No symbol archive of debug binaries.
- AudioWorklet + WASM without fallback for non-supporting browsers.
- WASM thread features without COOP/COEP headers.

### Browser-specific
- `postMessage` of WASM modules (structured-clone cost for large modules).
- Loading WASM from `eval`-style data URLs (slow; CSP issues).
- No CSP for WASM source (`script-src 'wasm-unsafe-eval'` or Trusted Types).
- WASM in service workers without explicit instantiation lifecycle.

### Security
- Unverified WASM binaries from registries with broad capabilities.
- Capability exceeding need (confused-deputy).
- Trusting module to enforce its own bounds (trust the sandbox, not the module).
- Logging sensitive data via WASI stdout to shared logs.

### Cross-cutting
- Performance assumptions from native ported wholesale (profile, don't assume).
- Trust in compiler heuristics (WASM JITs optimize different patterns than native).
- Vendor-lock-in via runtime APIs (Wasmtime-specific host functions, etc.).

---

## Modern shifts (2024-2025)

- **Component Model toolchain maturation**: cargo-component, wit-bindgen, jco production-ready.
- **WASI Preview 2 adoption**: P2 the current target; P1 maintenance.
- **WASM-GC shipped** in V8, SpiderMonkey 2024: managed-language WASM (Kotlin/WASM, Dart-WASM) practical.
- **Tail calls shipped** 2024: functional-language WASM (Scheme, OCaml) practical.
- **Stack switching proposal** advancing: proper async without compiler transforms expected 2025-2026.
- **Edge platforms maturing**: Fastly Compute (Wasmtime, production), Fermyon Spin (Component Model native), Cosmonic, Cloudflare Workers (V8 isolates + WASM).
- **WASI HTTP** stable in P2 (`wasi:http@0.2.0`): HTTP servers in WASM first-class.
- **WIT package registries**: `wa.dev`, `bytecode-alliance/components-registry`.
- **OCI artifact distribution**: WASM components in OCI registries (Docker Hub, GitHub Container Registry).

---

## Schools of thought (preserve disagreement)

- **"WASM replaces Docker"** (Hykes 2019): partially vindicated for edge / serverless / plugins; general workloads still use containers. The marketing is ahead of the practice.
- **Component Model vs raw modules**: components add toolchain complexity and ABI overhead. For small projects with one language and one host, components may be overkill. The Bytecode Alliance position is "components are the future"; the pragmatic position is "components when polyglot composition matters."
- **WASI vs Emscripten POSIX-like**: WASI is the standard direction; Emscripten's POSIX shim was the pre-WASI path. New code targeting servers / edge / plugins should use WASI; new browser-only code can use either.
- **Browser WASM vs standalone WASM**: very different review priorities. Browser focuses on boundary cost, size, CSP. Standalone focuses on capability grants, resource limits, runtime portability.
- **WASM for real-time audio at 1ms**: contested. Doable with care (AudioWorklet + WASM, pre-allocated buffers) but not native-equivalent.

---

## What is NOT a webassembly finding

- **Rust-to-WASM language-specific concerns**: route to `rust-wasm`.
- **FFI generally (Rust to non-WASM)**: route to `rust-ffi`.
- **General JavaScript performance unrelated to WASM**: route to `performance`.
- **General security unrelated to WASM capability model**: route to `security`.
- **Native code that's not compiled to WASM**: out of scope.
- **WebGPU / Web Audio API design without WASM in the picture**: route to `graphics-programming` / `audio-programming`.

---

## Severity calibration

Per `panel-contract.md`:

- **blocker**: capability grant exceeding need on a network-exposed module (SSRF, filesystem access escape); spec-level UB (use of a Phase 1/2 feature without feature detection in production); detached `Uint8Array` views after `memory.grow` producing data corruption; unverified WASM binaries from registries executed with broad capabilities.
- **major**: per-call string marshalling in hot paths; allocating WASM memory per call; no `memory.grow` failure handling; missing `console_error_panic_hook` in Rust WASM; P1 syscalls in greenfield code; components paired with non-component runtime; production builds without source maps or symbol archive.
- **minor**: no `wasm-opt -O3` post-step; no SIMD in numeric kernels where applicable; `wee_alloc` in Rust WASM; toolchain choice mismatch (Emscripten for a small kernel).
- **nit**: minor toolchain version mismatch; missing optional optimization step.
- **insight**: structural -- "this codebase has both Emscripten and WASI-SDK toolchains; consider unifying"; "Component Model would simplify the polyglot story"; "WASM-GC would eliminate the bundled GC runtime overhead."

Confidence: high on spec-level findings (instructions required by the spec); moderate on toolchain findings (well-known patterns); lower on performance speculation without profiling data.

---

## Process for the webassembly agent

1. **Identify the target.** Browser (which engines)? Standalone runtime (which)? Plugin in a host (which)?
2. **Identify the toolchain.** Emscripten / WASI-SDK / wasm-bindgen / cargo-component / TinyGo / AssemblyScript / other?
3. **Identify the proposals used.** SIMD, threads, GC, exceptions, Component Model, WASI version? Check CG phase status and target-runtime support.
4. **Walk the boundary.** Where does the WASM module talk to its host? Is the boundary used efficiently?
5. **Walk the memory model.** Are `Uint8Array` views across `memory.grow` handled? Does `memory.grow` failure handle gracefully? Are linear memory limits sensible?
6. **Walk capability grants.** What capabilities are granted to the module? Are they minimal? Are runtime caps (memory, CPU, fuel) bounded?
7. **Walk debugging.** Source maps? Symbol archives? `console_error_panic_hook`?
8. **Walk browser-specific concerns** (if applicable): CSP, COOP/COEP for threads, structured-clone cost, service worker semantics.
9. **Route to other lenses** where the angle is theirs:
   - Rust-specific WASM patterns → `rust-wasm`.
   - General JS performance → `performance`.
   - General threading/race patterns → `concurrency`.
   - General supply-chain / threat model → `security`.
10. **Stay read-only.**
