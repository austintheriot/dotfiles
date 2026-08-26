---
name: consult
description: Low-overhead consultation with a single expert subagent on a specific question. Routes the user's question to the named agent in consult mode -- no code review, no spec critique, just expert advice in that lens. Use when the user types `/consult <agent> <question>` or asks for a specialist's opinion on a topic without wanting a full panel review. Sibling to `/expert-review` (multi-agent panel on code) and `/expert-plan` (multi-agent critique of a spec). Read-only; the agent answers and the conversation continues.
---

# Consult

Single-agent consultation. The user has a question for one expert; this skill loads that expert, passes the question, and returns the answer. No panel, no synthesis, no findings format.

Sibling to `/expert-review` and `/expert-plan`. Use this when the question is genuinely about one lens and a full panel would be wasted tokens.

## Usage shapes

The user invokes this with:

- `/consult <agent> <question>` -- explicit agent and question.
- `/consult <agent>` -- explicit agent, conversational question follows.
- `/consult <question>` -- agent inferred from the question's topic.
- `/consult` -- ask which agent and what about.

When the agent is ambiguous, suggest 2-3 candidates and ask the user to pick.

## The consult-capable roster

Every agent listed here knows how to operate in consult mode (either because it was designed as an advisor, or because it preloads the `agent-modes` skill).

### Already advisor-shaped (designed for consultation from the start)

- **`distsys-data`** -- storage engines, replication, sharding, isolation levels, conflict resolution, schema evolution, CDC.
- **`distsys-runtime`** -- retries, idempotency, queues, sagas, caching, circuit breakers, timeouts, metastable failures.
- **`fp-types`** -- ADT design, parametricity, totality, refinement types, "make illegal states unrepresentable."
- **`fp-effects`** -- monads, monad transformers, free monads, tagless final, algebraic effects, pure-core/imperative-shell architecture.
- **`fp-verification`** -- formal verification, dependently-typed programming, Lean / Agda / Coq / Idris / F*, Curry-Howard in practice. Use sparingly.
- **`oo-patterns`** -- Gang of Four patterns, modern patterns (DI, Repository, Saga, Specification, Hexagonal, Active Record vs Data Mapper).
- **`oo-architecture`** -- inheritance vs composition, polymorphism dispatch, encapsulation, SOLID / CUPID / GRASP, hexagonal / clean / onion.
- **`oo-domain-modeling`** -- DDD, aggregates, entities, value objects, bounded contexts, ubiquitous language, anti-corruption layers.
- **`observability-practice`** -- SLO design, burn-rate alerting, golden signals, postmortem culture, on-call ergonomics, debugging workflows.
- **`otel-instrumentation`** -- span lifecycle, attribute hygiene, semantic conventions, context propagation, exemplars, metric instrument choice.
- **`otel-pipeline`** -- OTel Collector topology, processors, exporters, sampling strategies, cardinality management, pipeline reliability.
- **`people-and-org`** -- management, 1:1s, feedback, hiring, performance, team organization, culture, hard conversations, comms.
- **`product-leadership`** -- product strategy, discovery, prioritization, OKRs, MVP design, pricing, growth, roadmap, launches.
- **`rust-async`** -- the `Future` trait, `Send` / `Sync` bounds, cancellation safety, structured concurrency, Tokio, channels, streams.
- **`rust-backend`** -- axum / tower / hyper service design, error handling, telemetry, sqlx, integration testing, middleware composition.
- **`rust-ffi`** -- FFI boundaries, ABI choice, repr layouts, ownership conventions, panic / unwind safety, bindgen / cbindgen / cxx / uniffi / pyo3.
- **`rust-unsafe`** -- soundness audits, unsafe blocks, raw-pointer data structures, custom allocators, Pin projections, manual Send / Sync impls.
- **`rust-wasm`** -- Rust-to-WASM design, wasm-bindgen vs WASI, build profiles, serialization strategies, cross-boundary footguns.
- **`typescript-types`** -- complex type design, conditional / mapped / template-literal types, generic inference, branded types, discriminated unions.

### Newly multi-mode (preload `agent-modes` skill)

- **`security`** -- threat modeling, AuthN/AuthZ design, OWASP Top 10, CWE Top 25, secrets handling, supply-chain integrity.
- **`performance`** -- algorithmic complexity, I/O patterns (N+1, sync-on-async), hot-path allocations, frontend / backend / runtime-specific perf.
- **`accessibility`** -- WCAG POUR, ARIA, keyboard operability, focus management, contrast, text alternatives, cross-platform (web / iOS / Android).
- **`api-design`** -- HTTP / REST, gRPC / Protobuf, GraphQL, library / SDK, CLI; backward compat, versioning, pagination, error envelopes.
- **`concurrency`** -- threads, locks, atomics, memory models, lock-free, actors, channels, async/await (any language except Rust async).
- **`i18n`** -- text encoding, normalization, pluralization, dates / times / timezones, currency, sorting, names, RTL / BiDi, IDN.
- **`text-engineering`** -- the text stack encoding-through-rendering: Unicode algorithms (normalization / segmentation / line-breaking / bidi / collation by spec number), fonts and OpenType (cmap / GSUB / GPOS, variable / color fonts), shaping (HarfBuzz / Core Text / DirectWrite), and the international writing systems (CJK Han unification + East Asian Width, Arabic cursive joining, Hebrew final forms, Indic reordering, Thai segmentation, Zawgyi, emoji). The implementation depth beneath `i18n`'s locale altitude.
- **`ci-pipeline`** -- GitHub Actions / GitLab CI / Buildkite / Jenkins workflows, Dockerfiles, branch protection, OIDC, signing, SLSA.
- **`devops-infrastructure`** -- Terraform / Pulumi / CloudFormation / Helm; state management, IAM, networking, secrets, K8s manifests, GitOps, FinOps, DR.
- **`graphics-programming`** -- WebGL / WebGPU, 2D vector graphics, text shaping (HarfBuzz / SDF / MSDF), shaders, GPU best practices.
- **`audio-programming`** -- real-time audio thread discipline, DSP fundamentals, JUCE / Web Audio / mobile audio, plugin formats, MIDI.
- **`webassembly`** -- WASM spec / proposals, runtimes, JS↔WASM boundary, Component Model + WIT, WASI capability model, toolchain.
- **`mobile-native`** -- iOS (Swift / SwiftUI / UIKit / Swift Concurrency / SwiftData), Android (Kotlin / Compose / Coroutines / Room), platform lifecycle, store policy.
- **`sync-and-offline`** -- local-first / offline-first architecture, CRDTs vs operational transformation vs server-authoritative rebase, conflict resolution, tombstones, partial replication, schema migration across un-upgradeable clients, local storage substrate, sync observability.
- **`platform-payments`** -- StoreKit, Play Billing, Stripe and merchant-of-record, cross-platform entitlement architecture, webhook idempotency and reconciliation, store payment policy, subscription metrics.
- **`platform-release`** -- code signing and notarization, store submission and review, phased / staged rollout and the absence of rollback, versioning rules, desktop packaging and auto-update.
- **`desktop-native`** -- Windows / macOS / Linux desktop runtime: windowing and DPI, lifecycle and single-instance, OS integration, sandbox and filesystem, the Wayland transition, desktop UX conventions.
- **`app-privacy-compliance`** -- GDPR / US state patchwork / COPPA, consent ordering, data minimization, deletion propagation, privacy manifests and store declarations. Engineering guidance, not legal advice.
- **`native-bridge`** -- Electron / Tauri / React Native / Flutter / KMP / Capacitor bridges, IPC contract design, the renderer trust boundary, WebView-in-native, serialization and version skew.
- **`input-and-peripherals`** -- pointer / stylus / touch / keyboard / gamepad input fidelity, and camera / mic / Bluetooth / location / file / USB access with their permission models.
- **`llm-app`** -- prompt engineering, tool use, RAG, evals, context management (caching / compaction), prompt-injection defense, agentic patterns.
- **`browser-spec`** -- WhatWG specs, DOM event model, modern platform primitives (`<dialog>`, Popover, AbortController, `Intl.*`), Web Components.
- **`web-analytics`** -- Mixpanel / Amplitude / Segment / PostHog; event taxonomy, identity correctness, funnels / retention, A/B test instrumentation, privacy.

### NOT exposed (review-only)

These agents are excluded from `/consult` because they need code to be useful:

`bug-hunter`, `code-simplifier`, `first-principles`, `test-coverage`, `readability`, `debuggability`, `documentation`.

If the user asks for one of these in consult mode, suggest the closest advisor-shaped alternative (e.g., for `bug-hunter` → the relevant domain expert; for `documentation` → no advisor exists; for `first-principles` → fold into the consult question itself).

## Dispatch

Use the Agent tool with the chosen `subagent_type`. The dispatch prompt should:

1. **Name the mode explicitly**: start with "Mode: consult."
2. **Quote the user's question verbatim**, or a tight paraphrase if the original was conversational.
3. **State the expected output shape**: a direct answer in the agent's lens, no severity / confidence / file:line scaffolding, no findings format.
4. **Pass any relevant context** the user provided (code snippets, design constraints, prior decisions). Don't fabricate context the user didn't give.
5. **Cap length when the question is narrow**: "Under 250 words" for quick lookups, no cap for genuinely broad questions.

Example dispatch prompt:

> Mode: consult.
>
> The user is asking: "Should I use Tokio's `select!` macro or `tokio::join!` to wait on these three independent HTTP requests?"
>
> Answer directly in your async-Rust lens. Cite the cancellation-safety implications, the error-propagation difference, and pick a recommendation. No severity / confidence / file:line scaffolding. Under 250 words.

## When to NOT use /consult

- **Code review on a real diff** → use `/expert-review` instead (panel of all relevant lenses).
- **Spec critique** → use `/expert-plan`.
- **Domain-specific design brainstorm with a multi-step process** → use the matching design skill (`/system-design`, `/observability-design`, `/fp-design`, `/oo-design`, `/product-design`, `/analytics-design`, `/comms-and-team`).
- **A question that genuinely spans multiple lenses** → suggest `/expert-review` if there's code, or pick the single most relevant lens and add `See also: <other-agent>` to the answer.
- **A trivial factual lookup** → answer in the main conversation; `/consult` is for genuine expert depth, not for restating MDN.

## Routing heuristics

When the agent isn't named, infer from question keywords. The strongest signals:

| Question shape | Agent |
|---|---|
| "Should I use postgres / SQL replication / sharding / consistency level / isolation level" | `distsys-data` |
| "How do I design retries / idempotency / queues / circuit breakers" | `distsys-runtime` |
| "How do I model X as types / ADTs / sum types" | `fp-types` |
| "How do I structure effects / IO / Result chains" | `fp-effects` |
| "Should I use Visitor / Strategy / Repository pattern" | `oo-patterns` |
| "How should I draw bounded contexts / aggregates" | `oo-domain-modeling` |
| "What SLOs should I set / how should I alert" | `observability-practice` |
| "How should I design my spans / what attributes" | `otel-instrumentation` |
| "How should I configure my Collector / sampling" | `otel-pipeline` |
| "How do I run my 1:1s / give difficult feedback / handle this report" | `people-and-org` |
| "What's the MVP / how do I prioritize / what OKRs" | `product-leadership` |
| "Is this Future Send / how do I structure this async / cancel-safe" | `rust-async` |
| "How should I design this axum service / handler" | `rust-backend` |
| "Is this unsafe block sound" | `rust-unsafe` |
| "Should I use wasm-bindgen or WASI" | `rust-wasm` |
| "How do I expose this Rust function to Python / C / Swift" | `rust-ffi` |
| "How do I type this generic / conditional type / branded type" | `typescript-types` |
| "Is this design vulnerable to X / how do I protect against Y" | `security` |
| "Why is this slow / how do I optimize" | `performance` |
| "How do I make this accessible / WCAG / focus management" | `accessibility` |
| "Should this be REST / gRPC / GraphQL / how do I version" | `api-design` |
| "How do I synchronize threads / avoid deadlock / use channels" (non-Rust) | `concurrency` |
| "How do I handle Unicode / timezones / currencies / RTL / IDN" | `i18n` |
| "How do I normalize / segment graphemes / shape text / use HarfBuzz / handle OpenType / render CJK-Arabic-Indic / get terminal width right" | `text-engineering` |
| "How do I structure my GitHub Actions / sign artifacts / OIDC" | `ci-pipeline` |
| "How do I structure my Terraform / Helm / IAM / K8s manifests" | `devops-infrastructure` |
| "How do I render this on WebGPU / shape text / use SDF" | `graphics-programming` |
| "How do I structure my AudioWorklet / DSP code / plugin" | `audio-programming` |
| "Should I use Component Model / WASI Preview 2 / wasm-bindgen" | `webassembly` |
| "How should I structure this SwiftUI / Compose / lifecycle" | `mobile-native` |
| "How do I sync offline edits / resolve conflicts / pick a CRDT vs a sync engine" | `sync-and-offline` |
| "How should I model subscriptions / validate receipts / handle entitlements across platforms" | `platform-payments` |
| "How do I sign / notarize / ship this / set up auto-update / stage a rollout" | `platform-release` |
| "How should this behave on the desktop / handle windows / DPI / tray / Wayland" | `desktop-native` |
| "Do we need consent for this / how do I handle deletion requests / what goes in the privacy label" | `app-privacy-compliance` |
| "How should I design this IPC / expose native code to JS / secure my Electron or Tauri app" | `native-bridge` |
| "How do I handle stylus input / request this permission / access this device" | `input-and-peripherals` |
| "How should I prompt Claude / structure tool use / design my RAG" | `llm-app` |
| "Is there a platform API for this / should I use `<dialog>` / `Intl.*`" | `browser-spec` |
| "How should I name this event / structure my funnel / track this A/B test" | `web-analytics` |

When two lenses overlap, pick the one whose center-of-mass best matches the question. If genuinely 50/50, ask the user.

## After the answer

The dispatched agent returns its answer. Pass it through to the user as-is or with a brief framing line if context helps. Do not synthesize / re-summarize / add commentary -- the agent's answer is the product.

If the user has follow-up questions in the same lens, you may dispatch the same agent again (a fresh context each time). For sustained back-and-forth, consider whether the question has grown into a design exercise that should switch to the matching design skill.

## What this skill does NOT do

- Does not run multiple agents (use `/expert-review` or `/expert-plan`).
- Does not write code (the agent might illustrate with code in its answer; you don't apply it).
- Does not maintain state across invocations (each `/consult` is fresh).
- Does not chain agents (if the answer points to a second lens, mention it; don't auto-dispatch).
- Does not post to anywhere (Slack / GitHub / Linear); the answer stays in conversation.
