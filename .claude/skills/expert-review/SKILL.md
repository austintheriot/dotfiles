---
name: expert-review
description: Deep multi-expert code review. Detects what changed, classifies hunks by language and domain, spawns the relevant specialist subagents in parallel (typescript-types, rust-async/backend/unsafe/wasm/ffi, distsys-data/distsys-runtime, and any future review-capable subagents), then synthesizes their findings into one severity-ranked report tagged by which expert raised each issue. Reviews the current branch diff against main by default; supports `/expert-review <path>`, `/expert-review <PR#>`, or `/expert-review <range>`. Burns more tokens than the per-language review skills -- use when you genuinely want a panel pass, not for routine review. Does NOT post comments to GitHub.
---

# Expert Review

You are running a **panel-style review**: classify what changed, spawn the relevant specialist subagents in parallel, then synthesize their findings into one unified report. The user invoked this skill specifically because they want deep, expensive review -- do not optimize for token frugality at the cost of depth.

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

**Discoverability**: at the start of a session, also run `ls ~/.claude/agents/*.md` to pick up any subagents added since this skill was last updated. Read each agent's frontmatter `description` line to decide if it's review-capable. (The user noted they'll add more agents over time.) Any agent whose description includes "review" or "expert" or "audit" is fair game; agents that are clearly action-only (writing code, running commands, brainstorming) are not.

## Process

### Stage 1 -- Scope resolution

Same conventions as the other review skills:

- **No arg** -- diff between current branch and the merge base with the main branch (check the repo's CLAUDE.md; may be `main`, `master`, `staging`, `develop`). Include uncommitted changes; flag dirty tree in the report.
- **`<PR#>`** (numeric) -- a GitHub PR. Use `gh pr diff <PR#>` and `gh pr view <PR#> --json title,body,headRefName,baseRefName,additions,deletions,files,url`.
- **`<path>`** -- review that file or directory in full.
- **`<range>`** (contains `..` or `...`) -- review that git range.

Always exclude `node_modules/`, `target/`, `dist/`, `build/`, `.next/`, `coverage/`, generated bindings, `Cargo.lock`, lock files.

Open a one-line status: `Reviewing N files across {languages}, M hunks. Classifying...`

### Stage 2 -- Hunk classification

Read each changed file. For small files (<400 lines), read the whole thing for context; review findings usually depend on surrounding code. For each hunk, assign one or more expert lenses. **A hunk can match multiple lenses** -- that's the whole point of this skill.

Classification rules (tune to additional agents as they appear):

| Signal | Lens |
|---|---|
| `*.ts` or `*.tsx` file | typescript-types |
| Async patterns: `async fn`, `.await`, `tokio::`, `select!`, `JoinSet`, `Stream`, `Pin`, `Future` | rust-async |
| `axum::`, `tower::`, `hyper::`, `sqlx::`, `tracing::span`, `IntoResponse`, handler fns | rust-backend |
| `unsafe {`, `unsafe fn`, raw pointers (`*const`/`*mut`), `MaybeUninit`, `transmute`, manual `Send`/`Sync` impl, `#[may_dangle]` | rust-unsafe |
| `wasm_bindgen`, `JsValue`, `web_sys::`, `js_sys::`, wasm-pack config, `wit-bindgen` | rust-wasm |
| `extern "C"`, `#[repr(C/transparent/packed)]`, `bindgen`, `cbindgen`, `cxx::bridge`, `pyo3`, `napi`, `uniffi`, `Box::into_raw`, `CString`/`CStr`, `#[no_mangle]` | rust-ffi |
| Storage/replication/sharding/schema changes: migrations, replication-aware reads, secondary indexes, isolation level config, CRDT code, conflict-resolution logic, DB connection setup, sharding logic | distsys-data |
| Messaging/retries/idempotency/caching/failure-mode patterns: retry loops, queue handlers, message consumers, cache reads/writes, circuit breakers, saga / outbox code, idempotency-key handling, timeout/deadline propagation, fencing tokens, distributed locks | distsys-runtime |

If a hunk matches no expert lens (pure plumbing, config, docs), it gets a `[generic]` tag and is reviewed inline by the main agent using the cross-cutting principles in `~/.claude/rules/coding-style.md` and `~/.claude/rules/testing.md`.

### Stage 3 -- Soft warning on panel size

Count the total number of (lens, hunk-cluster) pairs that will dispatch. If the count is **> 15**, surface a one-line warning before proceeding:

> Note: this review will invoke N specialist subagents across M hunk clusters. This will burn substantial tokens. Proceeding (`/expert-review` was invoked intentionally). To narrow, re-run as `/expert-review <path>` or `/expert-review <PR#>`.

Do NOT block. The user invoked the skill knowing the cost.

### Stage 4 -- Parallel dispatch

For each expert lens with matched hunks, build a **single self-contained prompt** for that lens and spawn the agent in parallel. The prompt should include:

1. The agent's scope: "Review the following hunks from a <lens> perspective."
2. The hunks themselves, with file:line context. Don't make the agent re-derive what changed.
3. The repo's context: location, branch, related project conventions from any nearby CLAUDE.md, the relevant `tsconfig.json` / `Cargo.toml` / `package.json` flags.
4. The expected output format: severity-labeled findings (`blocker` / `major` / `minor` / `nit`) with `file:line` and a short headline + 1-2 sentence explanation. Match the format from the per-language review skills so the synthesis pass can ingest them uniformly.
5. The instruction: "Report only findings within your lens. If you would flag something outside your lens, mention it briefly in a 'See also' line but do not duplicate other experts' work."

Send all agent calls in a single message with parallel tool invocations. The skill blocks until all return.

If one or more agents fail, surface that in the report and proceed with what completed.

### Stage 5 -- Synthesis

Now collect every finding from every agent. The synthesis pass is the differentiator -- a clean panel review without synthesis is just N parallel reports. Do these steps:

1. **Bucket by location.** Group findings by `file:line ± 5 lines`. Findings within that window probably address the same underlying issue.

2. **Semantic dedup within each bucket.** If two agents flag the same underlying problem (e.g., `rust-async` flags "MutexGuard across .await" and `distsys-runtime` flags "synchronous lock held across async boundary in handler"), merge them into one finding with both lens tags: `[rust-async, distsys-runtime]`. The merged finding's body should preserve both perspectives -- often the language agent identified the mechanism and the domain agent identified the consequence; both belong.

3. **Note agreements.** When two or more agents flag the same thing, mark it: `(flagged by N experts)`. This is a strong signal the finding is real.

4. **Note disagreements explicitly.** If two agents disagree (one calls something a blocker, another minor), surface both perspectives in one entry rather than picking sides. Example: "rust-async: blocker (futures::Mutex held across yield can cause cross-task deadlock under load). distsys-runtime: minor (this codepath only runs once at startup, contention isn't possible)." Let the user decide.

5. **Re-rank by max severity.** A finding's severity is the highest severity any expert assigned, unless one expert's reasoning specifically overrides another's (rare; usually only when one agent had context the other didn't).

6. **Cross-cutting findings get their own section.** Some findings (e.g., "no tests for any of this change," "every handler has a different error type," "schema evolution breaks rolling deploys") aren't tied to a single hunk. Synthesize these from patterns across agent reports + your own pass over the diff.

### Stage 6 -- Report

Open with:

```
Reviewed N files (X TS, Y Rust, Z other), M hunks.
Spawned K expert agents in parallel: {agent1, agent2, ...}.
Synthesized P findings (A blockers, B major, C minor, D nits).
{Q findings were flagged by multiple experts -- strong signals.}
```

Then findings in severity order. Format:

```
**[blocker]** [lens1, lens2] `path/to/file.rs:42` -- short headline (flagged by 2 experts)

<one or two sentences explaining the issue, integrating both experts' perspectives where they merged>

<optional: suggested fix, or "see lens1 detailed report below for the full reasoning">
```

For findings only one agent raised, omit the "flagged by N experts" suffix.

Close with two sections:

1. **Cross-cutting concerns** -- findings spanning multiple hunks or files (testing gaps, architecture-level issues, schema/API-design problems).
2. **Areas the panel did not flag** -- if a hunk was reviewed and got a clean bill of health from its lens, say so. Useful signal that "the rust-unsafe expert reviewed src/lib.rs:127 and had no concerns."

If two experts strongly disagreed on a single finding, surface that in a small "Disagreements worth your judgment" subsection rather than burying it.

## Synthesis quality matters more than agent count

The user is paying for the panel; the value is in the synthesis. A clean synthesis pass:

- **Compresses agreement.** If three experts flagged the same line, one merged finding is better than three separate ones.
- **Preserves disagreement.** When experts disagree, the user wants to see both views, not your averaged opinion.
- **Surfaces patterns.** "Three different hunks all have unbounded retries" is a more useful framing than three separate findings.
- **Ranks honestly.** A finding flagged by every relevant expert is almost certainly real; one flagged by a single expert deserves the user's skepticism check.

## Discoverability and future experts

The user mentioned more agents are coming. When the panel composition is in question:

1. Run `ls ~/.claude/agents/` at session start.
2. For each agent, read its frontmatter `description`. Agents whose descriptions mention "review," "audit," "expert," "soundness," "design critique" are eligible for the panel.
3. Match agents to hunks based on the description's domain keywords (e.g., a hypothetical future `observability-review` agent's description would mention "tracing," "metrics," "SLO," "instrumentation" -- match hunks accordingly).

When a new agent is added, this skill should pick it up automatically without code changes here. If an agent's domain doesn't fit the keyword classification table above, just note it in the report: "Reviewed by new lens `<name>` based on agent description."

## What NOT to do

- **Do not** post comments to GitHub. Reports go to chat only.
- **Do not** rewrite code. Findings only.
- **Do not** call `/ts-review`, `/rust-review`, or `/distsys-review` -- those skills duplicate this skill's first stage. Invoke the underlying subagents directly via the Agent tool.
- **Do not** dispatch agents serially. Always parallel.
- **Do not** invoke an agent for a lens that has zero matched hunks.
- **Do not** drop a finding because another agent disagreed -- surface the disagreement.
- **Do not** flag every cosmetic issue. The per-language skills can do that; this skill's value is the panel + synthesis.
- **Do not** put backlinks, citations, or source URLs in the report.
- **Do not** apply rules dogmatically; the user's judgment and the local codebase conventions win.

## Decision references

When you need to break a tie or apply a principle in the synthesis pass:

- TypeScript principles: `~/.claude/rules/typescript.md`
- Rust principles: `~/.claude/rules/rust.md`
- Distributed-systems principles: `~/.claude/rules/distributed-systems.md` and `~/.claude/rules/system-design-patterns.md`
- Cross-cutting: `~/.claude/rules/coding-style.md` and `~/.claude/rules/testing.md`
