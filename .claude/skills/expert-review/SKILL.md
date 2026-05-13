---
name: expert-review
description: Deep multi-expert code review. Detects what to review, classifies code regions by language and domain, spawns the relevant specialist subagents in parallel (typescript-types, rust-async/backend/unsafe/wasm/ffi, distsys-data/distsys-runtime, fp-*, oo-*, otel-*, observability-practice, bug-hunter, code-simplifier, test-coverage, and any future review-capable subagents), then synthesizes their findings into one severity-and-confidence-ranked report tagged by which expert raised each issue. Always invokes `bug-hunter` (canonical bug-pattern catalog) and at least one FP agent. Supports two modes -- diff review (current branch vs. main, a PR, a git range) and survey review (a file, directory, or subsystem reviewed in full). Burns more tokens than the per-language review skills -- use when you genuinely want a panel pass, not for routine review. Does NOT post comments to GitHub. Does NOT apply fixes -- read-only.
---

# Expert Review

You are running a **panel-style review**: classify what to review, spawn the relevant specialist subagents in parallel, then synthesize their findings into one unified report. The user invoked this skill specifically because they want deep, expensive review -- do not optimize for token frugality at the cost of depth.

This skill is **read-only**. It reports findings; it does not apply fixes. The user reviews the report and decides what to act on.

## Modes

The skill operates in one of two modes, selected by argument shape:

- **Diff mode** (default for arg-less, `<PR#>`, `<range>`): review only what changed. The unit of analysis is a *changed region*. Findings on lines the author did not modify are out of scope unless they're cross-cutting concerns the diff exposes. Pre-existing issues are filtered out.

- **Survey mode** (default for `<path>`; explicit via `--survey <path>` or `--survey <description>`): review the named code in full, as a snapshot. The unit of analysis is a *code region* (file, function, module). Pre-existing issues ARE the point. The "don't flag lines the author didn't modify" rule does not apply.

Mode is decided in Stage 1. The subagents themselves are mode-agnostic -- the dispatch prompt tells them what scope to review and what kind of finding is in-scope, and the synthesis layer handles mode-specific grouping. **Do not push diff/git/hunk concepts down into subagent prompts.** Hand the agent code regions to review and a scope description; let it produce findings.

## The expert panel

The full roster of review-capable subagents on this machine, ordered by domain:

**Languages**:
- `typescript-types` -- TypeScript type-design, inference, generics, advanced patterns. `~/.claude/agents/typescript-types.md`.
- `rust-async` -- async Rust, tokio, pinning, Send/Sync, cancellation safety. `~/.claude/agents/rust-async.md`.
- `rust-backend` -- axum/tower/hyper/sqlx, project layout, telemetry. `~/.claude/agents/rust-backend.md`.
- `rust-unsafe` -- unsafe Rust soundness, UB, MaybeUninit, pin projection. `~/.claude/agents/rust-unsafe.md`.
- `rust-wasm` -- Rust/WebAssembly, wasm-bindgen, boundary cost. `~/.claude/agents/rust-wasm.md`.
- `rust-ffi` -- Rust/C/multi-language ABIs, repr, ownership across the boundary. `~/.claude/agents/rust-ffi.md`.

**Distributed systems**:
- `distsys-data` -- storage, replication, sharding, transactions, consistency, schema evolution.
- `distsys-runtime` -- messaging, retries, idempotency, timeouts, caches, failure modes.

**Observability**:
- `otel-instrumentation` -- OTel SDK, span lifecycle, attribute hygiene, semantic conventions, context propagation, exemplars, log/trace correlation. `~/.claude/agents/otel-instrumentation.md`.
- `otel-pipeline` -- OTel Collector config, processors, sampling strategies, cardinality management, exporters, pipeline reliability. `~/.claude/agents/otel-pipeline.md`.
- `observability-practice` -- SLO design, burn-rate alerting, four-golden-signals / RED / USE, postmortem culture, on-call ergonomics, debugging workflow. `~/.claude/agents/observability-practice.md`.

**Functional programming** (ALWAYS include at least one of these in every panel -- see note below):
- `fp-types` -- ADT design, parametricity, refinement types, GADTs, phantom types, typestate, "make illegal states unrepresentable." `~/.claude/agents/fp-types.md`.
- `fp-effects` -- effect tracking, monads, pure-core/imperative-shell, structured concurrency as effect, async-as-monad, ZIO/Cats Effect/Effect-TS patterns. `~/.claude/agents/fp-effects.md`.
- `fp-verification` -- Curry-Howard in practice, dependent types, Lean/Agda/Coq/Idris/F* dipping. Use sparingly. `~/.claude/agents/fp-verification.md`.

**Object-oriented programming** (signal-driven, NOT mandatory -- include only when the code under review has OO-shaped code):
- `oo-patterns` -- Gang of Four + modern design patterns. `~/.claude/agents/oo-patterns.md`.
- `oo-architecture` -- inheritance/composition, SOLID/CUPID, hexagonal/clean/onion. `~/.claude/agents/oo-architecture.md`.
- `oo-domain-modeling` -- DDD specialist (aggregates, entities, value objects, bounded contexts). `~/.claude/agents/oo-domain-modeling.md`.

The OO panel has a pedagogical bias -- when the user encounters unfamiliar OO patterns, the agents explain rather than just critique. Use the OO agents whenever the code has classes-with-methods, inheritance, factories, builders, visitor/observer/strategy/decorator patterns, DDD-shaped code (aggregates, repositories, value objects), or virtual dispatch.

**Cross-cutting** (domain-general lenses):
- `bug-hunter` -- canonical bug-prone patterns: TOCTOU, races, async/await footguns, caching, null/optionality, integer arithmetic, resource leaks, mutability/aliasing, error handling, time/timezone, encoding, boundary conditions, API leaks, security-shaped bugs. **MANDATORY in every panel** (see below). `~/.claude/agents/bug-hunter.md`.
- `code-simplifier` -- surplus complexity: single-implementation abstractions, pass-through layers, premature configuration, dead code, excessive nesting, duplication ripe for extraction, unnecessary state, cleverness, misplaced abstraction levels. Read-only -- suggests, never applies. `~/.claude/agents/code-simplifier.md`.
- `test-coverage` -- coverage gaps prioritized by failure-cost; discovers and runs repo tooling (coverage reports, mutation testing, flamegraphs) to ground findings in data. Also flags bad existing tests. `~/.claude/agents/test-coverage.md`.

### Mandatory lenses

Two lenses are invoked on **every** `/expert-review` invocation, regardless of what signals the code shows:

**`bug-hunter` (always invoked).** The canonical bug-pattern catalog (TOCTOU, races, async, caching, null, integer arithmetic, resource leaks, mutability, error handling, time, encoding, boundaries, API leaks, security) is domain-general; almost any code touches several categories. The bug-hunter is the "what compiles cleanly and still pages someone at 3am" lens.

**At least one FP agent (always invoked).** Even when the code has no obvious FP-flavored signals. Rationale: the FP lens frequently surfaces outside-the-box suggestions other reviewers miss -- ADT opportunities masquerading as boolean flags, mutation-across-async-boundaries, smart-constructor candidates, pure-core/impure-shell separations.

FP routing:
- Default: `fp-types`. It applies to almost every codebase and produces the most universally-useful insights (ADTs, refinement, parametricity).
- If the code has substantial async/concurrent/effectful code, ALSO include `fp-effects`.
- Include `fp-verification` only if the code is in a safety-critical context (crypto, kernel, financial settlement) OR the user explicitly invokes `/expert-review --verify` (treat any "verify" / "lean" / "formal" hint in args this way).

In the synthesis, FP findings often carry an "expert insight" severity rather than blocker/major -- treat that as additive rather than competing with the language and domain reviewers.

Two other cross-cutting lenses (`code-simplifier`, `test-coverage`) are signal-driven but apply broadly enough that they fire on most reviews -- see the classification table below.

**Discoverability**: at the start of a session, also run `ls ~/.claude/agents/*.md` to pick up any subagents added since this skill was last updated. Read each agent's frontmatter `description` line to decide if it's review-capable. (The user noted they'll add more agents over time.) Any agent whose description includes "review" or "expert" or "audit" is fair game; agents that are clearly action-only (writing code, running commands, brainstorming) are not.

## Process

### Stage 1 -- Scope resolution

Decide mode and scope from the invocation:

- **No arg** -- **Diff mode**. Diff between current branch and the merge base with the main branch (check the repo's CLAUDE.md; may be `main`, `master`, `staging`, `develop`). Include uncommitted changes; flag dirty tree in the report.
- **`<PR#>`** (numeric) -- **Diff mode**. Use `gh pr diff <PR#>` and `gh pr view <PR#> --json title,body,headRefName,baseRefName,additions,deletions,files,url`.
- **`<range>`** (contains `..` or `...`) -- **Diff mode**. Review that git range.
- **`<path>`** (file or directory) -- **Survey mode**. Review the file/dir in full as a snapshot.
- **`--survey <path>`** -- **Survey mode**, explicit. Same as bare `<path>`.
- **`--survey <description>`** -- **Survey mode** with feature discovery. The argument is a feature/subsystem description (e.g., "the auth flow", "the payment processing pipeline"). Discover the relevant files via grep, follow imports, then survey them. If discovery is uncertain, list the files you're about to review and ask the user to confirm or refine.
- **`--diff <path>`** -- **Diff mode** restricted to changes touching that path.

Always exclude `node_modules/`, `target/`, `dist/`, `build/`, `.next/`, `coverage/`, generated bindings, `Cargo.lock`, lock files.

Open a one-line status:
- Diff mode: `Reviewing diff: N files across {languages}, M changed regions. Classifying...`
- Survey mode: `Surveying N files across {languages} (P total lines). Classifying...`

### Stage 2 -- Region classification

Read each in-scope file. For survey mode, read every file in full. For diff mode, read each changed file -- for small files (<400 lines), read the whole thing for context; review findings usually depend on surrounding code.

For each region (a changed region in diff mode; a file, function, or coherent module in survey mode), assign one or more expert lenses. **A region can match multiple lenses** -- that's the whole point of this skill.

Classification rules (tune to additional agents as they appear):

| Signal | Lens |
|---|---|
| `*.ts` or `*.tsx` file | typescript-types |
| Async patterns: `async fn`, `.await`, `tokio::`, `select!`, `JoinSet`, `Stream`, `Pin`, `Future` | rust-async |
| `axum::`, `tower::`, `hyper::`, `sqlx::`, `tracing::span`, `IntoResponse`, handler fns | rust-backend |
| `unsafe {`, `unsafe fn`, raw pointers (`*const`/`*mut`), `MaybeUninit`, `transmute`, manual `Send`/`Sync` impl, `#[may_dangle]` | rust-unsafe |
| `wasm_bindgen`, `JsValue`, `web_sys::`, `js_sys::`, wasm-pack config, `wit-bindgen` | rust-wasm |
| `extern "C"`, `#[repr(C/transparent/packed)]`, `bindgen`, `cbindgen`, `cxx::bridge`, `pyo3`, `napi`, `uniffi`, `Box::into_raw`, `CString`/`CStr`, `#[no_mangle]` | rust-ffi |
| Storage/replication/sharding/schema concerns: migrations, replication-aware reads, secondary indexes, isolation level config, CRDT code, conflict-resolution logic, DB connection setup, sharding logic | distsys-data |
| Messaging/retries/idempotency/caching/failure-mode patterns: retry loops, queue handlers, message consumers, cache reads/writes, circuit breakers, saga / outbox code, idempotency-key handling, timeout/deadline propagation, fencing tokens, distributed locks | distsys-runtime |
| Instrumentation code: `tracing::`, `opentelemetry::`, `@opentelemetry/api`, span creation, `tracer.start_span` / `startSpan`, attribute/event/exception recording, metric instruments (counters, histograms, gauges), structured logger setup with trace correlation, propagator config | otel-instrumentation |
| Pipeline/Collector config: `otel-collector.yaml`, `refinery_rules.toml`, processors (batch, memory_limiter, transform, tail_sampling), exporters (otlp, prometheus), sampling configuration, cardinality-affecting code | otel-pipeline |
| SLO/alert definitions, runbook files, dashboard config, error-budget policies, on-call documentation, postmortem templates | observability-practice |
| Type definitions, ADTs, enums, sealed classes, discriminated unions, pattern matching, smart constructors, branded primitives, validation logic | fp-types |
| Effect-shaped code: monads, Result/Option chains, async/await pipelines, IO interleaved with logic, error handling strategy, structured concurrency, Effect-TS / ZIO / Cats Effect / fp-ts usage | fp-effects |
| Safety-critical code (crypto, kernel, financial settlement) or regions the user explicitly flags for verification | fp-verification |
| **All code (mandatory FP lens)**: every panel review includes at least `fp-types` regardless of the table above, per the "always include at least one FP agent" policy | fp-types |
| Classes with methods, inheritance hierarchies, abstract classes, virtual dispatch, factory patterns, builder patterns, design-pattern-shaped code (Visitor, Observer, Strategy, Decorator, Template Method, etc.) | oo-patterns |
| Larger OO architecture: deep class hierarchies, SOLID-flavored design, ports-and-adapters/hexagonal/clean/onion structures, module boundaries, dependency direction concerns | oo-architecture |
| DDD-shaped code: aggregate roots, entities + value objects, repositories, domain events, bounded-context integration, anti-corruption layers, ubiquitous-language naming | oo-domain-modeling |
| **All code (mandatory bug-hunter lens)**: every panel review includes `bug-hunter` regardless of the table above, per the "always invoked" mandatory-lens policy | bug-hunter |
| Code that looks more complex than the problem demands: deep nesting, interfaces with one implementation, layers that just forward calls, "configurable" params never varied, duplicated blocks ripe for extraction, state that mirrors a computation, clever one-liners | code-simplifier |
| Any production code (i.e., not pure config / docs / generated): test-coverage runs on most reviews because gaps in error-paths and edge-cases are common. Skip only for diffs / surveys that are entirely test code, config, docs, or generated bindings | test-coverage |

If a region matches no expert lens (pure plumbing, config, docs), it gets a `[generic]` tag and is reviewed inline by the main agent using the cross-cutting principles in `~/.claude/rules/coding-style.md` and `~/.claude/rules/testing.md`. Note: `bug-hunter` still runs on every panel regardless.

### Stage 3 -- Soft warning on panel size

Count the total number of (lens, region-cluster) pairs that will dispatch. If the count is **> 15**, surface a one-line warning before proceeding:

> Note: this review will invoke N specialist subagents across M region clusters. This will burn substantial tokens. Proceeding (`/expert-review` was invoked intentionally). To narrow, re-run with a tighter scope.

Do NOT block. The user invoked the skill knowing the cost.

### Stage 4 -- Parallel dispatch

For each expert lens with matched regions, build a **single self-contained prompt** for that lens and spawn the agent in parallel. The prompt should include:

1. **Scope description.** "Review the following code from a <lens> perspective. Mode: <diff | survey>. Diff mode = review only the changed regions; treat surrounding code as context, not as review targets. Survey mode = review the code as a snapshot in full; pre-existing issues are in scope."
2. **The code itself**, with file:line context. In diff mode, include the changed regions plus enough surrounding context that the lens can reason about them. In survey mode, include the full files (or representative sections for large ones). Don't make the agent re-derive what to look at.
3. **Project conventions.** The contents of the repo's root `CLAUDE.md` (if present) and any `CLAUDE.md` files that share a path prefix with the code under review, plus relevant `.claude/rules/*.md`. These override generic principles -- enforce project rules.
4. **Repo context.** Location, branch, the relevant `tsconfig.json` / `Cargo.toml` / `package.json` flags or other config that affects the lens.
5. **Output format**, required for every finding:
   - **severity**: one of `blocker`, `major`, `minor`, `nit`, or `insight` (for FP/OO outside-the-box reframings that aren't bugs).
   - **confidence**: 0-100. **Only report findings with confidence >= 50.** Rubric below.
   - **file:line** anchoring the finding.
   - **Headline** -- one sentence.
   - **Body** -- 1-3 sentences explaining the issue (and the fix if obvious).
6. **Confidence rubric** (give verbatim to the agent):
   - **90-100**: Verified real issue. Will break in production, violates an explicit project rule, or is a definitively miscalibrated design.
   - **70-89**: Very likely real. Strong evidence but not fully verified.
   - **50-69**: Probably real but may be a nitpick, edge case, or matter of taste.
   - **Below 50**: Do not report.
7. **Do NOT flag** (give verbatim to the agent):
   - Pre-existing issues outside your lens's primary concern *(diff mode only -- in survey mode, pre-existing issues ARE the point)*.
   - Issues that linters, typecheckers, or compilers would catch (imports, types, formatting). Assume CI runs these separately.
   - Issues on lines outside the scope you were given.
   - Code with explicit lint-ignore or suppression comments.
   - Intentional functionality changes consistent with the code's purpose.
   - **Pedantic nitpicks a senior engineer wouldn't bother calling out.** When in doubt, omit.
   - Findings outside your lens. If you spot something for a different lens, mention it briefly in a "See also" line; don't duplicate other experts' work.
8. **Scope reminder.** "Report only findings within your lens. The synthesis pass will merge overlapping findings across experts."

Send all agent calls in a single message with parallel tool invocations. The skill blocks until all return.

If one or more agents fail, surface that in the report and proceed with what completed.

### Stage 5 -- Synthesis

Now collect every finding from every agent. The synthesis pass is the differentiator -- a clean panel review without synthesis is just N parallel reports. Do these steps:

1. **Bucket by location.**
   - Diff mode: group findings by `file:line ± 5 lines`. Findings within that window probably address the same underlying issue.
   - Survey mode: group findings by file and by function/section within the file. Cross-file architectural findings get their own cross-cutting bucket.

2. **Semantic dedup within each bucket.** If two agents flag the same underlying problem (e.g., `rust-async` flags "MutexGuard across .await" and `distsys-runtime` flags "synchronous lock held across async boundary in handler"), merge them into one finding with both lens tags: `[rust-async, distsys-runtime]`. The merged finding's body should preserve both perspectives -- often the language agent identified the mechanism and the domain agent identified the consequence; both belong.

3. **Note agreements.** When two or more agents flag the same thing, mark it: `(flagged by N experts)`. This is a strong signal the finding is real, and it should boost the synthesized confidence.

4. **Synthesize confidence.** Each agent gave its own confidence. The merged finding's confidence is the **max** of the inputs, bumped by +10 (capped at 100) when two or more experts independently flag the same underlying issue. Disagreements (one expert at 90, another at 30 saying "this is fine") get surfaced as a range with both views, not collapsed.

5. **Note disagreements explicitly.** If two agents disagree (one calls something a blocker, another minor or harmless), surface both perspectives in one entry rather than picking sides. Example: "rust-async: blocker, confidence 85 (futures::Mutex held across yield can cause cross-task deadlock under load). distsys-runtime: minor, confidence 40 (this codepath only runs once at startup, contention isn't possible)." Let the user decide.

6. **Re-rank by max severity.** A finding's severity is the highest severity any expert assigned, unless one expert's reasoning specifically overrides another's (rare; usually only when one agent had context the other didn't).

7. **Cross-cutting findings get their own section.** Some findings (e.g., "no tests for any of this," "every handler has a different error type," "schema evolution breaks rolling deploys," "this module mixes pure logic with I/O throughout") aren't tied to a single region. Synthesize these from patterns across agent reports + your own pass over the code.

8. **Apply the report threshold.** Findings with confidence >= 70 go in the **main report**. Findings with confidence 50-69 go in a **collapsible appendix** ("Lower-confidence findings -- worth a skim"). Anything below 50 was filtered by the agents and never reaches synthesis.

### Stage 6 -- Report

Open with:

```
Mode: <diff | survey>
Scope: <files/range/path description>
Reviewed N files (X TS, Y Rust, Z other), M regions.
Spawned K expert agents in parallel: {agent1, agent2, ...}.
Synthesized P findings (A blockers, B major, C minor, D nits, E insights).
{Q findings were flagged by multiple experts -- strong signals.}
{R additional lower-confidence findings in appendix.}
```

Then a **panel verdict** -- one line, chosen from:

- **Ship it** -- no blockers or majors, only minor/nit/insight findings.
- **Discussion-worthy** -- no blockers, but findings (especially insights or architectural questions) deserve a conversation before moving on.
- **Address before merge** (diff) / **Address before relying on this** (survey) -- one or more blockers, or majors that compound.
- **Substantial rework recommended** -- multiple blockers, or cross-cutting structural concerns.

Then findings in severity order. Format:

```
**[blocker | confidence 92]** [lens1, lens2] `path/to/file.rs:42` -- short headline (flagged by 2 experts)

<one or two sentences explaining the issue, integrating both experts' perspectives where they merged>

<optional: suggested fix, or "see lens1 detailed report below for the full reasoning">
```

For findings only one agent raised, omit the "flagged by N experts" suffix.

Close with three sections:

1. **Cross-cutting concerns** -- findings spanning multiple regions or files (testing gaps, architecture-level issues, schema/API-design problems).
2. **Areas the panel did not flag** -- if a region was reviewed and got a clean bill of health from its lens, say so. Useful signal that "the rust-unsafe expert reviewed src/lib.rs:127 and had no concerns."
3. **Lower-confidence findings (appendix)** -- collapsible / clearly demarcated. Findings with confidence 50-69, same format. The user can skim and surface anything that resonates.

If two experts strongly disagreed on a single finding, surface that in a small "Disagreements worth your judgment" subsection rather than burying it.

## Synthesis quality matters more than agent count

The user is paying for the panel; the value is in the synthesis. A clean synthesis pass:

- **Compresses agreement.** If three experts flagged the same line, one merged finding is better than three separate ones.
- **Preserves disagreement.** When experts disagree, the user wants to see both views, not your averaged opinion.
- **Surfaces patterns.** "Three different regions all have unbounded retries" is a more useful framing than three separate findings.
- **Ranks honestly.** A finding flagged by every relevant expert and at high confidence is almost certainly real; one flagged by a single expert at confidence 55 deserves the user's skepticism check -- but it earns its place in the appendix because the panel cost was already paid.

## Discoverability and future experts

The user mentioned more agents are coming. When the panel composition is in question:

1. Run `ls ~/.claude/agents/` at session start.
2. For each agent, read its frontmatter `description`. Agents whose descriptions mention "review," "audit," "expert," "soundness," "design critique" are eligible for the panel.
3. Match agents to regions based on the description's domain keywords (e.g., a hypothetical future `security-review` agent's description would mention "auth," "crypto," "injection," "secrets" -- match regions accordingly).

When a new agent is added, this skill should pick it up automatically without code changes here. If an agent's domain doesn't fit the keyword classification table above, just note it in the report: "Reviewed by new lens `<name>` based on agent description."

## What NOT to do

- **Do not** post comments to GitHub. Reports go to chat only.
- **Do not** rewrite or apply code fixes. Findings only. This skill is read-only by design.
- **Do not** call `/ts-review`, `/rust-review`, or `/distsys-review` -- those skills duplicate this skill's first stage. Invoke the underlying subagents directly via the Agent tool.
- **Do not** dispatch agents serially. Always parallel.
- **Do not** invoke an agent for a lens that has zero matched regions.
- **Do not** drop a finding because another agent disagreed -- surface the disagreement.
- **Do not** push diff/hunk/git concepts into subagent prompts. Subagents are mode-agnostic; the dispatch and synthesis layers handle mode-specific framing.
- **Do not** flag every cosmetic issue. The per-language skills can do that; this skill's value is the panel + synthesis.
- **Do not** put backlinks, citations, or source URLs in the report.
- **Do not** apply rules dogmatically; the user's judgment and the local codebase conventions win.

## Decision references

When you need to break a tie or apply a principle in the synthesis pass:

- TypeScript principles: `~/.claude/rules/typescript.md`
- Rust principles: `~/.claude/rules/rust.md`
- Distributed-systems principles: `~/.claude/rules/distributed-systems.md` and `~/.claude/rules/system-design-patterns.md`
- Cross-cutting: `~/.claude/rules/coding-style.md` and `~/.claude/rules/testing.md`
