---
name: expert-consult
description: Multi-expert panel consultation on an open-ended question. Classifies which expert lenses pertain, dispatches 2-4 consult-capable agents in parallel, then synthesizes their answers while preserving genuine disagreement. No code, no spec -- just a question that benefits from multiple expert perspectives. Sibling to `/expert-review` (panel on code), `/expert-plan` (panel on a spec), and `/consult` (single agent on a question). Soft cap of 4 agents; confirms before exceeding. Read-only; the user reviews and decides.
---

# Expert Consult

Multi-expert consultation. Pick the lenses that matter for the question, run them in parallel, synthesize. Read-only.

Sibling to:
- `/expert-review` -- panel on code (diff or survey).
- `/expert-plan` -- panel on a spec.
- `/consult` -- one expert on a question.

`/expert-consult` is the question-shaped equivalent of `/expert-review`. Use when one lens isn't enough but the question doesn't have code or a spec attached.

## When to use

- Cross-cutting design question that genuinely spans multiple expert lenses ("how should I think about idempotency vs eventual consistency for this workflow" → distsys-data + distsys-runtime).
- Trade-off question where different schools of thought disagree productively ("should I use a saga or a distributed transaction" → distsys-runtime + distsys-data + maybe `oo-domain-modeling`).
- Strategy question that benefits from disagreement ("should we adopt event sourcing" → distsys-data + fp-effects + oo-domain-modeling + performance).
- Architecture question where multiple specialists would catch different things.

**Don't use when**:
- One lens is clearly sufficient → use `/consult <agent>`.
- You have code to review → use `/expert-review`.
- You have a spec to critique → use `/expert-plan`.
- The question is a quick factual lookup → answer in the main conversation.

## The roster

Eligible agents are the 35 consult-capable ones from `/consult`. Run `ls ~/.claude/agents/` at session start to pick up new ones.

### Already advisor-shaped

- **`distsys-data`** -- storage, replication, sharding, consistency, isolation, schema evolution.
- **`distsys-runtime`** -- retries, idempotency, queues, sagas, caching, circuit breakers, metastable failures.
- **`fp-types`** -- ADT design, parametricity, totality, refinement types.
- **`fp-effects`** -- monads, transformers, free monads, tagless final, algebraic effects, pure-core/imperative-shell.
- **`fp-verification`** -- formal verification, Lean / Agda / Coq / Idris / F*. Use sparingly.
- **`oo-patterns`** -- Gang of Four, modern patterns (DI, Repository, Saga, Hexagonal, Active Record).
- **`oo-architecture`** -- inheritance vs composition, polymorphism, SOLID / CUPID / GRASP, hexagonal / clean / onion.
- **`oo-domain-modeling`** -- DDD, aggregates, entities, value objects, bounded contexts, anti-corruption layers.
- **`observability-practice`** -- SLO design, burn-rate alerting, golden signals, postmortem culture, on-call ergonomics.
- **`otel-instrumentation`** -- span lifecycle, attribute hygiene, semantic conventions, context propagation, exemplars.
- **`otel-pipeline`** -- OTel Collector topology, processors, exporters, sampling, cardinality, pipeline reliability.
- **`people-and-org`** -- management, 1:1s, feedback, hiring, performance, team org, culture, hard conversations.
- **`product-leadership`** -- product strategy, discovery, prioritization, OKRs, MVP, pricing, growth, roadmap.
- **`rust-async`** -- `Future`, `Send` / `Sync` bounds, cancellation safety, Tokio, channels, streams.
- **`rust-backend`** -- axum / tower / hyper, error handling, telemetry, sqlx, integration testing.
- **`rust-ffi`** -- FFI boundaries, ABI, repr, panic safety, bindgen / cbindgen / cxx / uniffi / pyo3.
- **`rust-unsafe`** -- soundness, raw pointers, custom allocators, Pin projections, manual Send / Sync.
- **`rust-wasm`** -- Rust-to-WASM, wasm-bindgen vs WASI, build profiles, cross-boundary serialization.
- **`typescript-types`** -- complex type design, conditional / mapped / template-literal types, generics, branded types.

### Multi-mode (preload `agent-modes`)

- **`security`**, **`performance`**, **`accessibility`**, **`api-design`**, **`concurrency`**, **`i18n`**, **`ci-pipeline`**, **`devops-infrastructure`**, **`graphics-programming`**, **`audio-programming`**, **`pdf`**, **`webassembly`**, **`mobile-native`**, **`sync-and-offline`**, **`platform-payments`**, **`platform-release`**, **`desktop-native`**, **`app-privacy-compliance`**, **`llm-app`**, **`browser-spec`**, **`web-analytics`**, **`data-flow`**.

See their frontmatter `description` fields for the exact lens of each.

### NOT in the roster

`bug-hunter`, `code-simplifier`, `first-principles`, `test-coverage`, `readability`, `debuggability`, `documentation`. These need code or a spec to be useful. If the question implicitly calls for one of these, surface it back to the user ("this is really a code-review question; consider `/expert-review`").

## Process

### Step 1 -- Classify the question

Read the question. Identify which lenses genuinely pertain. The bar for "pertains": this expert would have a *substantive*, *non-obvious* answer that another expert wouldn't already cover.

Reject:
- Lenses that would restate what another included lens already covers.
- Lenses that the question only tangentially touches.
- "Why not, might as well" picks.

Aim for 2-4 agents by default. **Soft cap is 4.** If the question genuinely demands more, confirm with the user first: "This question touches 6 lenses (...). I'll dispatch the top 4 unless you'd rather see all 6 -- the cost is roughly N×."

If only 1 lens applies, suggest `/consult <agent>` instead. If 0 apply, the question is out of scope for this skill (either too broad, too narrow, or in a lens this skill doesn't cover -- name the closest match).

### Step 2 -- Confirm the roster (optional, fast)

For a clear-cut question, dispatch directly. For an ambiguous one, name the chosen lenses in one line and invite a correction: "Consulting `distsys-data`, `distsys-runtime`, and `oo-domain-modeling` on this. Sound right?"

Don't burn three turns deciding the roster. The user can redirect after seeing answers.

### Step 3 -- Dispatch in parallel

Use the Agent tool with one call per chosen subagent, all in the same message so they run concurrently. Each dispatch prompt should:

1. **Name the mode**: "Mode: consult."
2. **Quote the user's question verbatim.**
3. **State the panel context**: "You are one of N experts being consulted in parallel on this question. Other lenses being consulted: [list]. Answer in your own lens; don't try to cover theirs."
4. **Request a structured-enough answer for synthesis**: "Open with your one-sentence position. Then expand. End with where you'd push back on the other lenses if you can predict the disagreement."
5. **Cap length per agent**: "Under 400 words unless the question genuinely needs more."

Example dispatch:

> Mode: consult.
>
> The user is asking: "Should we adopt event sourcing for our order pipeline?"
>
> You are one of 4 experts being consulted in parallel. Other lenses: `distsys-runtime`, `oo-domain-modeling`, `performance`. Answer in your `distsys-data` lens.
>
> Open with your one-sentence position. Expand. End with where you'd push back on the other lenses if you can predict the disagreement. Under 400 words.

### Step 4 -- Synthesize, preserving disagreement

After all agents return, write a synthesis. **The synthesis's job is to surface where the experts agree, where they disagree, and why -- not to flatten the answers into one consensus.**

Synthesis structure:

```
## Where the panel agrees
- [point 1] -- cited by `agent-a`, `agent-b`
- [point 2] -- cited by `agent-c`, `agent-d`

## Where the panel disagrees
- **[Tension 1]**: `agent-a` says X (because Y); `agent-c` says not-X (because Z). The tension is real, not a misunderstanding -- they're optimizing for different things.
- **[Tension 2]**: ...

## What the user should decide
- The question that resolves Tension 1 in one direction or the other: [e.g., "is this system more write-heavy or read-heavy?"]
- The question for Tension 2: ...

## My read (optional)
One paragraph. Where I'd land given the user's known context (FP-leaning, Anthropic, prior decisions). One assumption I'd challenge in the panel's collective framing.
```

The "My read" section is optional but encouraged for genuinely contested questions. **Do not simply affirm the panel's consensus.** Per the user's global directive, find at least one assumption to challenge.

### Step 5 -- Surface the full per-agent answers

After the synthesis, include the per-agent answers in collapsible sections (use Markdown details/summary if your renderer supports it; otherwise just headers). The synthesis is the executive summary; the per-agent answers are the source material the user can verify.

```
## Full per-agent answers

### `distsys-data`
[full answer]

### `distsys-runtime`
[full answer]

### `oo-domain-modeling`
[full answer]

### `performance`
[full answer]
```

## Cost and scope

Each agent invocation has real token cost. 4 agents × ~400 words each + a synthesis is roughly 3-4x a single consult. Worth it when the question genuinely benefits from disagreement; wasteful when one lens would suffice. The skill should err on the side of fewer agents and let the user request more.

## Routing heuristics

Same vocabulary as `/consult`. The difference: `/consult` picks one; `/expert-consult` picks 2-4. When the question shape suggests multiple natural lenses:

| Question shape | Likely panel |
|---|---|
| "Should we adopt event sourcing / CQRS / event-driven architecture" | `distsys-data` + `distsys-runtime` + `oo-domain-modeling` + (maybe `fp-effects`) |
| "How should I model multi-tenant data" | `distsys-data` + `security` + `oo-domain-modeling` |
| "How should I handle async work that must complete" | `distsys-runtime` + `rust-async` or `concurrency` (language-appropriate) + `observability-practice` |
| "Should this be a microservice or monolith" | `distsys-runtime` + `oo-architecture` + `data-flow` + `people-and-org` (Conway's Law) + `performance` |
| "Should this state live in Redux / the store / a singleton / per-request scope" | `data-flow` + `oo-architecture` + (maybe `fp-effects` if effects are involved) |
| "Where should the boundary between X and Y live" | `data-flow` + `oo-architecture` + `api-design` (if X or Y has a consumer-facing surface) |
| "Who should own this long-lived resource (cache / pool / scheduler)" | `data-flow` + `oo-architecture` + (maybe `distsys-runtime` if the resource is a cache that affects fault behavior) |
| "How should I roll out this big migration" | `ci-pipeline` + `distsys-data` (if schema) + `observability-practice` + `product-leadership` |
| "How do I structure my agent / RAG / eval system" | `llm-app` + `distsys-runtime` (orchestration) + `observability-practice` + (maybe `security` for prompt injection) |
| "Should I rewrite this in Rust" | `rust-async` or `rust-backend` (situation-appropriate) + `performance` + `people-and-org` (hiring / ramp-up cost) |
| "How should I think about offering this as a public API" | `api-design` + `security` + `product-leadership` + (maybe `distsys-runtime` for rate-limiting / idempotency) |
| "How do I plan this team reorg" | `people-and-org` + `oo-architecture` (Conway's Law lens) + `product-leadership` |
| "How should I structure my CI / build pipeline for X" | `ci-pipeline` + `devops-infrastructure` + `security` |

When the panel is non-obvious, lean toward 2-3 lenses with strong fit rather than 4 with weak fit. A two-expert disagreement is more legible than a four-expert overlap.

## What this skill does NOT do

- Does not review code (use `/expert-review`).
- Does not critique a spec (use `/expert-plan`).
- Does not consult a single agent (use `/consult`).
- Does not write code (panel produces text; the user decides what to act on).
- Does not chain agents sequentially -- this is a parallel panel.
- Does not invoke review-only agents (`bug-hunter`, `code-simplifier`, etc.); their value comes from code in hand.
- Does not run more than 4 agents without confirmation.
- Does not flatten disagreement; the synthesis preserves it explicitly.
