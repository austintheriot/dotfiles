---
name: expert-review
description: Deep multi-expert code review. Classifies code regions, spawns the relevant specialist subagents in parallel (typescript-types, rust-async/backend/unsafe/wasm/ffi, distsys-data/distsys-runtime, fp-*, oo-*, otel-*, observability-practice, bug-hunter, code-simplifier, test-coverage, readability, debuggability, documentation), then synthesizes into one severity-and-confidence-ranked report. Always invokes `bug-hunter` and at least one FP agent. Two modes: diff (current branch / PR / range) and survey (a path or feature). Burns more tokens than per-language review skills -- use for genuine panel passes. Read-only; does NOT apply fixes or post to GitHub.
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
- **Cross-cutting**: `bug-hunter`, `code-simplifier`, `test-coverage`, `readability`, `debuggability`, `documentation`

### Mandatory lenses (every invocation)

- **`bug-hunter`** -- the canonical bug-pattern catalog. Domain-general.
- **At least one FP agent.** Default `fp-types`. Add `fp-effects` if the code is substantially async / effectful. Add `fp-verification` only for safety-critical contexts or explicit `--verify`.

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

A region matching no specialist gets a `[generic]` tag and is reviewed inline using `~/.claude/rules/coding-style.md` and `~/.claude/rules/testing.md`. `bug-hunter` still runs.

## Process

### Stage 1: Scope resolution

Parse the arg to determine mode + scope (above). Always exclude `node_modules/`, `target/`, `dist/`, `build/`, `.next/`, `coverage/`, generated bindings, lock files.

Open one status line:
- Diff: `Reviewing diff: N files across {langs}, M changed regions. Classifying...`
- Survey: `Surveying N files across {langs} (P total lines). Classifying...`

### Stage 2: Region classification

Read each in-scope file. For diff mode, read small (<400 lines) files in full for context; for larger ones, read changed regions plus enough surrounding code. For survey mode, read every file in full.

For each region, assign one or more lenses via the table + the mandatory / broad rules. A region can (and should) match multiple lenses -- that's the point.

### Stage 3: Soft warning

Count (lens, region-cluster) pairs. If > 15, print:

> Note: this review will invoke N specialist subagents across M region clusters. This will burn substantial tokens. Proceeding. To narrow, re-run with a tighter scope.

Do not block.

### Stage 4: Parallel dispatch

For each lens with matched regions, spawn the agent. **Use the dispatch template below** -- do not repeat the panel contract per call; the agent reads `~/.claude/rules/panel-contract.md` itself.

#### Dispatch prompt template

```
Lens: <lens-name>
Mode: <diff | survey>
Scope: <one-sentence description of what to review>

Read `~/.claude/rules/panel-contract.md` for the output format, severity / confidence rubrics, and "do NOT flag" list. Follow your agent definition for what to look for.

Project conventions (these override generic principles):
<contents of repo root CLAUDE.md, plus any CLAUDE.md sharing a path prefix with the code, plus relevant `.claude/rules/*.md`>

Repo context:
<branch, base branch, relevant config flags (tsconfig.json, Cargo.toml, package.json)>

Code:
<the code regions with file:line context. Diff mode: changed regions plus enough surrounding lines to reason. Survey mode: the files in full, or the representative sections for very large ones>
```

Send all dispatches in a single message (parallel tool calls). Block until all return. If an agent fails, surface that in the report and proceed.

### Stage 5: Synthesis

Synthesis is the differentiator. Steps:

1. **Bucket findings by location.** Diff: `file:line ± 5 lines`. Survey: by file + function/section. Cross-file findings: separate bucket.

2. **Semantic dedup.** If two agents flag the same issue, merge with both lens tags (e.g., `[rust-async, distsys-runtime]`). Preserve both perspectives -- the language agent often spots the mechanism, the domain agent the consequence.

3. **Note agreements.** `(flagged by N experts)`. Strong signal; bump confidence.

4. **Synthesize confidence.** Merged confidence = max of inputs, +10 (capped 100) when 2+ experts independently agree.

5. **Preserve disagreements.** If experts disagree (one blocker, one minor), show both views with their lenses and confidences. Do not collapse to an average.

6. **Re-rank by max severity.** Highest severity any expert assigned, unless one expert had context the other didn't.

7. **Surface cross-cutting findings.** Patterns spanning regions ("no tests anywhere," "every handler has a different error type," "schema evolution breaks rolling deploys"). Synthesize from agent reports + your own scan.

8. **Threshold the report.** Confidence >= 70: main report. 50-69: collapsible appendix. <50: filtered by agents, never reaches synthesis.

### Stage 6: Report

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
- Cross-cutting: `~/.claude/rules/coding-style.md`, `~/.claude/rules/testing.md`
