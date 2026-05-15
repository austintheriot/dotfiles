---
name: expert-review
description: Deep multi-expert code review. Classifies code regions, spawns the relevant specialist subagents in parallel (typescript-types, rust-async/backend/unsafe/wasm/ffi, distsys-data/distsys-runtime, fp-*, oo-*, otel-*, observability-practice, bug-hunter, code-simplifier, test-coverage, readability, debuggability, documentation, security, performance, accessibility, api-design, concurrency, i18n, ci-pipeline, web-analytics, graphics-programming, audio-programming, webassembly, browser-spec, first-principles), then synthesizes into one severity-and-confidence-ranked report. Always invokes `bug-hunter`, at least one FP agent, and `first-principles` (the wildcard reviewer that asks "is the answer already in scope?"). Two modes: diff (current branch / PR / range) and survey (a path or feature). Burns more tokens than per-language review skills -- use for genuine panel passes. Read-only; does NOT apply fixes or post to GitHub.
---

# Expert Review

Panel-style review. Classify regions, dispatch specialists in parallel, synthesize. Read-only.

## Modes

- **Diff mode** (no arg / `<PR#>` / `<range>` / `--diff <path>`): review the changed regions; pre-existing issues out of scope unless the change exposes them.
- **Survey mode** (`<path>` / `--survey <path>` / `--survey <description>`): review the named code in full; pre-existing issues are the point.

Subagents are mode-agnostic. The dispatch prompt names the mode and scope; subagents handle it via the panel contract.

## The expert panel

The roster is the set of review-capable agents under `~/.claude/agents/`. Each agent's frontmatter `description` says what it does and when it applies. **Read those descriptions for routing** -- do not duplicate them here.

Current review-capable agents (run `ls ~/.claude/agents/` at session start to pick up new ones):

- **Languages**: `typescript-types`, `rust-async`, `rust-backend`, `rust-unsafe`, `rust-wasm`, `rust-ffi`
- **Distributed systems**: `distsys-data`, `distsys-runtime`
- **Observability**: `otel-instrumentation`, `otel-pipeline`, `observability-practice`
- **Functional programming**: `fp-types`, `fp-effects`, `fp-verification`
- **Object-oriented programming**: `oo-patterns`, `oo-architecture`, `oo-domain-modeling`
- **Cross-cutting**: `bug-hunter`, `code-simplifier`, `test-coverage`, `readability`, `debuggability`, `documentation`, `security`, `performance`, `accessibility`, `api-design`, `concurrency`, `i18n`, `ci-pipeline`, `web-analytics`, `graphics-programming`, `audio-programming`, `webassembly`, `browser-spec`, `first-principles`

### Mandatory lenses (every invocation)

- **`bug-hunter`** -- the canonical bug-pattern catalog. Domain-general.
- **At least one FP agent.** Default `fp-types`. Add `fp-effects` if the code is substantially async / effectful. Add `fp-verification` only for safety-critical contexts or explicit `--verify`.
- **`first-principles`** -- wildcard reviewer that asks "is the answer already in scope?" before any code is added. Runs Q1 (existing-utility / already-installed-dependency check) on every invocation; Q2-Q4 (constraint relaxation, problem reframe, cross-domain precedent) when the diff / survey has substance to reframe. Never produces blocker; Q1 caps at major, Q2-Q4 cap at insight.

### Broadly-applicable lenses (fire on most reviews)

- **`code-simplifier`** -- almost any production code can have surplus complexity.
- **`test-coverage`** -- almost any production code has coverage gaps. Skip only when diff / survey is entirely test code, config, docs, or generated bindings.
- **`readability`** -- skip only for generated / lock / fixture files.
- **`debuggability`** -- skip only for purely pure / static / generated content.
- **`documentation`** -- always when the change touches a public API or doc surface; otherwise skip only for purely-internal helpers.

### Signal-driven lenses

Match a region to a specialist via signals. Keep the table tight; the agent's own description disambiguates if needed.

| Lens | Trigger signals |
|---|---|
| `typescript-types` | `*.ts` / `*.tsx` |
| `rust-async` | `async fn`, `.await`, `tokio::`, `select!`, `JoinSet`, `Stream`, `Pin`, `Future` |
| `rust-backend` | `axum::`, `tower::`, `hyper::`, `sqlx::`, handler fns, `IntoResponse` |
| `rust-unsafe` | `unsafe {`, `unsafe fn`, raw pointers, `MaybeUninit`, `transmute`, manual `Send`/`Sync` |
| `rust-wasm` | `wasm_bindgen`, `JsValue`, `web_sys::`, `js_sys::`, wasm-pack, `wit-bindgen` |
| `rust-ffi` | `extern "C"`, `#[repr(C/...)]`, `bindgen`, `cbindgen`, `cxx::bridge`, `pyo3`, `napi`, `uniffi`, `CString`/`CStr`, `#[no_mangle]` |
| `distsys-data` | storage / replication / sharding / schema / isolation / CRDT / migrations |
| `distsys-runtime` | retries, queues, idempotency, caching, circuit breakers, sagas / outbox, timeouts, locks |
| `otel-instrumentation` | `tracing::`, `opentelemetry::`, `@opentelemetry/api`, span / attribute / metric / propagator code |
| `otel-pipeline` | `otel-collector.yaml`, `refinery_rules.toml`, sampling / processor / exporter config |
| `observability-practice` | SLO / alert / runbook / dashboard / error-budget / postmortem files |
| `fp-types` | type definitions, ADTs, enums, sealed classes, discriminated unions, pattern matching, smart constructors, branded primitives (also: mandatory FP lens by default) |
| `fp-effects` | monads, `Result`/`Option` chains, async/await pipelines, IO interleaved with logic, Effect-TS / ZIO / Cats Effect / fp-ts |
| `fp-verification` | safety-critical (crypto, kernel, financial settlement) OR explicit `--verify` |
| `oo-patterns` | classes with methods, inheritance, factory/builder/visitor/observer/strategy/decorator patterns |
| `oo-architecture` | deep class hierarchies, SOLID-flavored design, hexagonal/clean/onion, module boundaries |
| `oo-domain-modeling` | aggregates, entities + value objects, repositories, domain events, bounded contexts |
| `security` | trust boundaries (any handler accepting external input), auth code (login, session, token, JWT, OAuth, MFA), AuthZ checks, crypto / hashing / signing, secrets handling, dependency / lockfile / supply-chain changes, file upload, URL fetching / SSRF surfaces, deserialization, SQL / NoSQL / template / command construction, CORS / CSP / cookie config, redirect logic, admin / debug / internal endpoints |
| `performance` | hot-path code (request handlers on busy endpoints, render functions, tight loops, batch processors), DB query construction (especially with `JOIN` / `WHERE` / `ORDER BY` on new columns), loops over collections that grow, async / parallel patterns (`Promise.all`, `tokio::spawn`, goroutines), React / Vue / Svelte component bodies and effects, bundle / import changes, caching code, allocation-heavy paths (string building, buffer construction), state-management updates that trigger re-renders |
| `accessibility` | rendered UI code: `*.html` / `*.jsx` / `*.tsx` / `*.vue` / `*.svelte` with rendered markup, SwiftUI / UIKit views (`*.swift` with `View` / `UIView` / `UIViewController`), Jetpack Compose / Android Views (`*.kt` / `*.java` with `Composable` / `View` / `Activity` / `Fragment`), ARIA attribute usage, role / `tabindex` / `aria-*`, focus management code, color / theme / contrast changes, animation / transition with no `prefers-reduced-motion`, custom form controls or modal / dialog code. Skip for backend / CLI / config / non-UI |
| `api-design` | consumer-facing contract surfaces: HTTP route handlers, OpenAPI / Swagger definitions, `.proto` / gRPC service definitions, GraphQL schemas and resolvers exposing new fields / types, public library / SDK exports, CLI flag / subcommand / output-format changes, webhook payload schemas, SDK signatures. Skip for purely internal helpers, generated bindings, tests, fixtures |
| `concurrency` | in-process shared-memory concurrency in any language EXCEPT Rust async (route Rust async to `rust-async`): threads (`std::thread`, `Thread`, goroutines, JVM virtual threads), locks / mutexes / RWLocks / atomics, async/await in JS / C# / Python / Kotlin / Swift / Haskell, actors (Erlang processes, Akka, Swift `actor`), CSP / channels (Go, core.async), STM (Clojure refs, Haskell STM), web workers, SharedArrayBuffer, thread pools / executors / dispatchers, cancellation tokens, structured-concurrency scopes |
| `i18n` | user-facing text rendering (web / native / CLI shipping to users), date / time / calendar code, number / currency / unit formatting, text manipulation on user-relevant data (names, addresses, sort, search), file / path handling crossing OS boundaries or accepting user names, URL / domain handling for user-facing URLs, translation files / language-tag handling / locale negotiation, storage code writing user text. Skip for backend / CLI / config without user-facing text |
| `ci-pipeline` | CI / CD workflow files (`.github/workflows/*.yml`, `.gitlab-ci.yml`, `Jenkinsfile`, `azure-pipelines.yml`, `.circleci/config.yml`, `buildkite/*.yml`, `.tekton/*.yaml`), Dockerfiles, `compose.yml`, release-engineering scripts (`release-please-config.json`, `goreleaser.yaml`), branch protection / `CODEOWNERS` / `.github/settings.yml`, deploy configs (Helm, Kustomize, ArgoCD / Flux manifests, Terraform for deploy infra), runner / agent configuration, OIDC trust policies |
| `web-analytics` | analytics instrumentation: Mixpanel / Amplitude / Segment / PostHog / Heap / Snowplow SDK calls (`track`, `identify`, `alias`, `people.set`, `register`, `reset`, `capture`), tracking-plan / schema files (Avo, Iteratively, Segment Protocols, Snowplow JSON Schemas), Mixpanel Lexicon edits, A/B testing SDK code (Mixpanel Experiments, Amplitude Experiment, Statsig, LaunchDarkly, Split.io, Optimizely, GrowthBook), cookie-consent / privacy-gate code that interacts with analytics init. Skip for backend without analytics surface or for purely operational telemetry (route to `observability-practice`) |
| `graphics-programming` | graphics / GPU code: WebGL (`gl.*`), WebGL2, WebGPU (`device.*`, `passEncoder.*`, `GPUBuffer`, `GPUTexture`, `GPUBindGroup`), WGSL / GLSL / HLSL / MSL shader files, 2D vector graphics (Bézier paths, SVG rendering, Loop-Blinn, SDF / MSDF, Vello / pathfinder / lyon), text shaping / rendering (HarfBuzz / FreeType / atlas / SDF / Slug), game / engine code with explicit GPU calls, wgpu / bgfx / Diligent / Sokol cross-platform layers, render graphs / frame graphs, GPU compute kernels (image processing, ML inference, particle systems on GPU) |
| `audio-programming` | audio code: audio callbacks / `processBlock` / `AudioWorkletProcessor.process` / `AURenderCallback`, DSP (filters, oscillators, convolution, FFT, oversampling, distortion / saturation), plugin entry points (VST3 `IComponent::process`, AudioUnit, AAX, LV2, CLAP), plugin lifecycle (`prepareToPlay`, `releaseResources`, parameter listeners), Web Audio (`AudioWorkletProcessor`, `AudioContext` graph, `decodeAudioData`), mobile native audio (Oboe / AAudio / AVAudioEngine / AVAudioSession), cross-platform audio I/O (PortAudio, RtAudio, miniaudio), JUCE-specific code (`juce::AudioProcessor`, `juce::dsp::*`, `juce::SmoothedValue`), MIDI / OSC handling |
| `webassembly` | WebAssembly: compilation targets (`wasm32-unknown-unknown`, `wasm32-wasip1`, `wasm32-wasip2`, Emscripten, AssemblyScript, TinyGo), JS↔WASM glue (`wasm-bindgen`, Embind, jco), WIT files (`*.wit`), `cargo-component` projects, WASI imports (`wasi_snapshot_preview1::*`, `wasi:io/streams`), Wasmtime / Wasmer / WAMR embedding code, browser WASM loading (`WebAssembly.instantiate`, `instantiateStreaming`), WASM threads / atomics / SIMD, WASM-GC / exceptions / tail-calls usage, AudioWorklet + WASM, edge / serverless WASM (Fastly Compute, Fermyon Spin, Cloudflare Workers WASM). Skip Rust-specific WASM patterns inside Rust code (route to `rust-wasm`) |
| `browser-spec` | frontend / browser code that may reinvent platform primitives: event handlers, listener cleanup, DOM manipulation, modal / popover / menu / tooltip / dropdown components, form code (validation, input, contenteditable, selection / range), URL parsing / query strings / encoding, date / number / currency / locale-aware formatting in browser code, Web Components (Custom Elements, Shadow DOM, slots), Service Worker code, Worker / OffscreenCanvas / SharedArrayBuffer, scheduling / animation / async patterns, cross-tab / cross-window communication, storage code (localStorage, IndexedDB, Cache API, OPFS), history / routing in SPAs. Use proactively to catch reinventions where `IntersectionObserver` / `ResizeObserver` / `<dialog>` / Popover / `AbortController` / `Intl.*` / `URL` / `URLSearchParams` / BroadcastChannel / etc. would do the job natively |
| `first-principles` | always fires (mandatory lens). Reads the package manifest and conventional utility locations (`utils/`, `helpers/`, `lib/`, `internal/`, `shared/`, `common/`) before the rest of the panel. Q1 (existing-utility / installed-dependency check) runs on every invocation; Q2-Q4 (constraint relaxation, problem reframe, cross-domain precedent) fire when the diff / survey has substance to reframe |

A region matching no specialist gets a `[generic]` tag and is reviewed inline using `~/.claude/rules/coding-style.md` and `~/.claude/rules/testing.md` (plus `~/.claude/rules/testing-typescript.md` for TS/JS test files). `bug-hunter` still runs.

## Process

### Stage 1: Scope resolution

Parse the arg to determine mode + scope (above). Always exclude `node_modules/`, `target/`, `dist/`, `build/`, `.next/`, `coverage/`, generated bindings, lock files.

Open one status line:
- Diff: `Reviewing diff: N files across {langs}, M changed regions. Classifying...`
- Survey: `Surveying N files across {langs} (P total lines). Classifying...`

### Stage 2: Project-conventions discovery

**Critical step. This is where the panel learns the project's local rules.** Most production-flagged review concerns are documented in the repo itself; agents only catch them if those docs reach the dispatch prompt.

Discover and load:

1. **Every `CLAUDE.md` in the repo** -- root, plus every subdirectory CLAUDE.md whose path is a prefix of any file under review. For Notability-shaped repos this includes `Backend/CLAUDE.md`, `ios/CLAUDE.md`, `Android/CLAUDE.md`, etc. Use `find <repo-root> -name CLAUDE.md -not -path '*/node_modules/*' -not -path '*/target/*'`.

2. **PR / contribution conventions** -- `pull_request_template.md` (and `.github/pull_request_template.md`), `CONTRIBUTING.md`, `STYLE.md`, `CODE_OF_CONDUCT.md`. The PR template often encodes a review checklist.

3. **Project docs that look like guidance** -- `docs/*.md` files whose names match conventions / standards / engineering / best-practices / analytics / observability / architecture / decisions / runbooks. Read them. They commonly contain the project-specific rules (canonical log key names, analytics naming, SOC2 prohibitions, framework conventions) generic agents will not know.

4. **Lint and config that encodes convention** -- `.eslintrc*`, `tsconfig.json`, `biome.json`, `.rubocop.yml`, `.pre-commit-config.yaml`, `clippy.toml`, `rustfmt.toml`, `.editorconfig`, package-specific configs (`jest.config`, `vitest.config`, `playwright.config`). Skim for rules; load the strict ones into dispatch.

5. **Local `.claude/rules/*.md`** in the repo (separate from global `~/.claude/rules/`).

Build a **conventions bundle** per region: for each file under review, the conventions are the union of (root CLAUDE.md, all path-prefix CLAUDE.mds, the relevant `docs/` files, and the relevant lint configs). When dispatching, include this bundle in the agent's prompt -- not just root CLAUDE.md.

If a `docs/*.md` file references a specific concern in the changed code (e.g., a `docs/analytics.md` describing event-naming and the diff touches analytics calls), prioritize it. The main agent should grep the loaded docs for keywords matching the regions being reviewed.

### Stage 3: Region classification

Read each in-scope file. For diff mode, read small (<400 lines) files in full for context; for larger ones, read changed regions plus enough surrounding code. For survey mode, read every file in full.

For each region, assign one or more lenses via the table + the mandatory / broad rules. A region can (and should) match multiple lenses -- that's the point.

### Stage 4: Soft warning

Count (lens, region-cluster) pairs. If > 15, print:

> Note: this review will invoke N specialist subagents across M region clusters. This will burn substantial tokens. Proceeding. To narrow, re-run with a tighter scope.

Do not block.

### Stage 5: Parallel dispatch

For each lens with matched regions, spawn the agent. **Use the dispatch template below** -- do not repeat the panel contract per call; the agent reads `~/.claude/rules/panel-contract.md` itself.

#### Dispatch prompt template

```
Lens: <lens-name>
Mode: <diff | survey>
Scope: <one-sentence description of what to review>

Read `~/.claude/rules/panel-contract.md` for the output format, severity / confidence rubrics, and "do NOT flag" list. Follow your agent definition for what to look for.

Project conventions (these override generic principles -- read them BEFORE applying your generic catalog):
<full contents of the conventions bundle for the regions under review:
  - root CLAUDE.md
  - every CLAUDE.md whose path is a prefix of any file under review
  - relevant docs/*.md guidance files (analytics, observability, engineering values, architecture decisions, runbooks)
  - PR template if it encodes review rules
  - relevant lint / config rules that encode convention
  - local .claude/rules/*.md if present>

When project conventions contradict your generic catalog, the project wins. Flag deviations from documented project conventions at higher severity than deviations from generic principles -- the project authors wrote those rules for a reason.

Repo context:
<branch, base branch, relevant config flags (tsconfig.json, Cargo.toml, package.json), framework versions if affecting review>

Code:
<the code regions with file:line context. Diff mode: changed regions plus enough surrounding lines to reason. Survey mode: the files in full, or the representative sections for very large ones>
```

Send all dispatches in a single message (parallel tool calls). Block until all return. If an agent fails, surface that in the report and proceed.

### Stage 6: Synthesis

Synthesis is the differentiator. Steps:

1. **Bucket findings by location.** Diff: `file:line ± 5 lines`. Survey: by file + function/section. Cross-file findings: separate bucket.

2. **Semantic dedup.** If two agents flag the same issue, merge with both lens tags (e.g., `[rust-async, distsys-runtime]`). Preserve both perspectives -- the language agent often spots the mechanism, the domain agent the consequence.

3. **Note agreements.** `(flagged by N experts)`. Strong signal; bump confidence.

4. **Synthesize confidence.** Merged confidence = max of inputs, +10 (capped 100) when 2+ experts independently agree.

5. **Preserve disagreements.** If experts disagree (one blocker, one minor), show both views with their lenses and confidences. Do not collapse to an average.

6. **Re-rank by max severity.** Highest severity any expert assigned, unless one expert had context the other didn't.

7. **Surface cross-cutting findings.** Patterns spanning regions ("no tests anywhere," "every handler has a different error type," "schema evolution breaks rolling deploys"). Synthesize from agent reports + your own scan.

8. **Threshold the report.** Confidence >= 70: main report. 50-69: collapsible appendix. <50: filtered by agents, never reaches synthesis.

### Stage 7: Report

Header:

```
Mode: <diff | survey>
Scope: <files/range/path>
Reviewed N files (X TS, Y Rust, Z other), M regions.
Spawned K expert agents: {agent1, agent2, ...}.
Synthesized P findings (A blockers, B major, C minor, D nits, E insights).
{Q findings flagged by multiple experts -- strong signals.}
{R additional lower-confidence findings in appendix.}
```

Panel verdict, one of:

- **Ship it** -- no blockers or majors.
- **Discussion-worthy** -- no blockers, but findings worth a conversation.
- **Address before merge** (diff) / **Address before relying on this** (survey) -- one or more blockers, or compounding majors.
- **Substantial rework recommended** -- multiple blockers, or cross-cutting structural concerns.

Findings in severity order:

```
**[blocker | confidence 92]** [lens1, lens2] `path/to/file.rs:42` -- short headline (flagged by 2 experts)

<1-2 sentences integrating both perspectives>

<optional: suggested fix>
```

Close with:

1. **Cross-cutting concerns** -- findings spanning regions / files.
2. **Areas the panel did not flag** -- regions reviewed and judged clean. Useful negative signal.
3. **Lower-confidence findings (appendix)** -- collapsible, same format, confidence 50-69.
4. **Disagreements worth your judgment** -- only if experts strongly disagreed on a finding.

## Synthesis quality > agent count

The user pays for the panel; the value is the synthesis:

- **Compress agreement.** Three experts on one line -> one merged finding.
- **Preserve disagreement.** Show both views; don't average.
- **Surface patterns.** "Three regions all have unbounded retries" > three separate findings.
- **Rank honestly.** Multi-expert + high confidence = almost certainly real. Single-expert + 55 = appendix.

## What NOT to do

- Do not post to GitHub.
- Do not rewrite or apply fixes. Read-only.
- Do not call `/ts-review`, `/rust-review`, `/distsys-review` -- those duplicate this skill's classification stage. Invoke subagents directly.
- Do not dispatch agents serially -- always parallel.
- Do not invoke an agent for a lens with zero matched regions.
- Do not drop a finding because another agent disagreed -- surface the disagreement.
- Do not push diff / hunk / git concepts into subagent prompts -- the mode is in the dispatch template, the contract is in `panel-contract.md`.
- Do not flag every cosmetic issue. The per-language skills can do that.
- Do not put backlinks / citations / source URLs in the report.

## Decision references

- Panel contract: `~/.claude/rules/panel-contract.md`
- TypeScript: `~/.claude/rules/typescript.md`
- Rust: `~/.claude/rules/rust.md`
- Distributed systems: `~/.claude/rules/distributed-systems.md`, `~/.claude/rules/system-design-patterns.md`
- Cross-cutting: `~/.claude/rules/coding-style.md`, `~/.claude/rules/testing.md`, `~/.claude/rules/testing-typescript.md` (TS/JS test runner specifics)
