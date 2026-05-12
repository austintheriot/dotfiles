---
name: distsys-review
description: Expert review pass for distributed-systems footguns in changed code -- retries without idempotency, unbounded queues, fire-and-forget calls without dedup, missing fencing tokens, cache stampedes, replication-lag races, and other patterns that pass `tsc`/`rustc`/`pytest` but produce 3am pages. Reviews the current branch diff against main by default, a specific file/PR with `/distsys-review <path>` or `/distsys-review <PR#>`, or a git range. Auto-routes deep questions to `distsys-data` or `distsys-runtime` subagents. Produces severity-labeled findings with file:line references. Does NOT post comments. Use when reviewing changes that touch persistence, messaging, retries, caching, or anything that crosses a process boundary.
---

# Distributed Systems Review

You are doing an **expert-level review** for distributed-systems correctness and operability. Many of these issues compile, pass tests, and look fine in code review -- they only surface under partial failure, retry storms, or scale.

The reference files `~/.claude/rules/distributed-systems.md` (principles) and `~/.claude/rules/system-design-patterns.md` (patterns) are your authoritative checklist. Cross-cutting principles in `~/.claude/rules/coding-style.md` and `~/.claude/rules/testing.md` apply.

## Scope resolution

- **No arg** -- diff between current branch and the merge base with the main branch (check repo's CLAUDE.md for the branch name; may be `main`, `master`, `staging`, `develop`). Include uncommitted changes; flag dirty tree.
- **`<PR#>`** (numeric) -- a GitHub PR. Use `gh pr diff <PR#>` and `gh pr view <PR#>`.
- **`<path>`** -- review that file or directory in full.
- **`<range>`** (contains `..` or `...`) -- review that git range.

Exclude `node_modules/`, `target/`, `dist/`, `build/`, `.next/`, `coverage/`, generated code.

## What to flag

You're looking for issues a strong human reviewer would catch but the compiler/linter wouldn't. Categories, ordered by frequency of real production impact:

### 1. Retries without idempotency

The single most common distributed-systems bug.

- HTTP POST / PUT / PATCH retried at the framework or client level without an idempotency key in the request.
- Background jobs / message handlers that perform side effects (DB writes, payments, notifications, third-party calls) without a dedup key check.
- Code that catches an exception and retries without verifying whether the operation completed.
- "Publish then commit" or "commit then publish" patterns -- both lose messages on failure.

Look for: `retry`, `Retry`, `tenacity`, `backoff`, `tower::retry`, `axios-retry`, `p-retry`, and any custom loop with `for attempt in 0..N`.

### 2. Unbounded queues, retries, fanouts

- `mpsc::unbounded`, `tokio::sync::mpsc::UnboundedSender`, `bounded(usize::MAX)`, `new Queue()` without a limit.
- Retry loops with no max-attempts or with `while true` semantics.
- `Promise.all` / `join_all` / fanouts whose size is user-input-driven, with no chunking or concurrency cap.
- Worker pools with no backpressure between stages.

These look innocent until something downstream slows down.

### 3. Missing fencing tokens on distributed locks

- Any code that acquires a distributed lock (Redis SETNX, ZooKeeper, etcd, a DB row lock) and then operates on a shared resource without including a fencing token (a monotonically-increasing version) in subsequent writes.
- TTL-based locks alone are unsafe under GC pauses, VM migration, network delays. The classic bug: lock holder pauses past TTL, second holder acquires, original wakes up and writes "with permission."
- For object stores (S3, Azure Blob, GCS), conditional writes (If-Match / If-None-Match) serve the same purpose.

### 4. Cache stampedes and invalidation

- Cache-aside reads with no single-flight / request coalescing -- when a hot key expires, all in-flight requests hit the origin.
- Synchronized TTLs across all keys -- one cold-cache moment hits every key at once. Add **TTL jitter**.
- Cache-aside reads with no plan for cache invalidation when the underlying row mutates via a different code path.
- Negative caching (caching "not found") with no separate TTL -- a transient miss becomes a long-lived hole.
- Caches that turn failure into worse failure -- cache failover landing 100% of traffic on the origin.

### 5. Replication-lag races

- Code that writes to a primary then immediately reads from a replica without specifying read-from-primary or wait-for-replication.
- Read-after-write user flows that don't account for replication lag (user submits form, redirect to listing page reads stale, "my edit disappeared" ticket).
- Cross-shard transactions that assume linearizable cross-shard reads.
- Single-leader replication that ACKs before any follower applies the write while called "synchronous."

### 6. Timestamps used for ordering / conflict resolution

- Wall-clock timestamps used to order events from multiple machines (`Date.now()`, `time.time()`, `SystemTime::now()`).
- Last-write-wins conflict resolution using wall-clock timestamps -- silently loses data under clock skew.
- Cross-node "happens-before" decisions based on machine clocks rather than logical clocks or sequencers.

Look for: `last_modified`, `updated_at`, `Date.now()`, comparisons of two timestamps from different nodes.

### 7. Event handlers without DLQ / poison-message handling

- Message consumers that crash on a malformed message and infinitely redeliver.
- No max-retry / max-attempt before sending to DLQ.
- No alert / monitoring on DLQ depth.
- "Exactly-once" claims without the configuration that justifies them (Kafka transactions, idempotent producers + idempotent consumers, transactional outbox at the sink).

### 8. Hot-spot inevitability

- Sharding by `user_id` with no celebrity / viral mitigation.
- Sharding by autoincrement / timestamp -- every write hits the highest shard.
- Single-row "counter" that all updates serialize on.
- Top-N queries that hit one partition every time.

### 9. End-to-end argument violations

- Reliance on TCP / DB transactions for correctness across multiple layers, when the client retry on connection loss bypasses both.
- Multi-step business operations relying on framework-level "at-least-once" without an application-level dedup key.
- HTTP POSTs without idempotency keys that perform actions visible to the user.

### 10. Distributed transactions where they shouldn't be

- Two-phase commit across heterogeneous systems (DB + message broker).
- XA transactions in any new code.
- Saga compensations that aren't actually inverses of the forward steps ("cancelShipment" after package is on a truck).

### 11. Cross-channel timing dependencies

- Writing to a system A, then putting a "process this" message on queue B, where B's message references data in A that may not be replicated yet.
- "Send notification" that races with "save record" -- notification fires before the record is committed.

### 12. Schema / encoding evolution

- New protobuf field with a reused tag number.
- Removing a required field without a backward-compatibility shim.
- Tightening enum validation in a way that rejects messages from older publishers.
- Deserializing into a typed model that drops unknown fields, then writing back.

### 13. Capacity and load assumptions

- Capacity estimates based on average rather than peak.
- Connection pool sizes that don't account for slow downstreams holding connections.
- Timeouts longer than the upstream client's timeout (downstream finishes work the caller already gave up on).
- No deadline propagation -- downstream work continues after the user gave up.

### 14. Observability / alerting gaps in the change

- New code path that introduces a new failure mode with no log / metric / trace.
- New external dependency with no timeout, no error metric, no SLO.
- Alerts on internal causes ("CPU high") instead of user-visible symptoms ("p99 latency > 1s").
- Missing structured-logging context (request ID, user ID, trace ID).

Note: deep observability review is out of scope for this skill -- a dedicated observability skill is planned. Flag obvious gaps but don't go deep.

### 15. Cost-of-consensus surprises

- Consensus-backed primitives (etcd leases, Spanner transactions, Kafka committed-offset queries) used in a hot path where their latency cost wasn't budgeted.
- Linearizable reads where causal would do.
- Custom Raft/Paxos implementation (almost always wrong -- use the building blocks).

## Routing to subagents

When a finding requires depth you don't have in-context, delegate. Pass a self-contained prompt with the snippet, the question, and the surrounding context the subagent needs.

- **`distsys-data` subagent**: storage choice, replication, sharding, transactions, schema evolution, indexes, consistency models, hot-spot mitigation, conflict resolution. Use when the question is "is this data model / replication / consistency strategy sound?"
- **`distsys-runtime` subagent**: messaging, retries, idempotency, timeouts, queues, caches, circuit breakers, sagas, outbox patterns, failure-mode analysis. Use when the question is "is this runtime / cross-process behavior correct?"

If the change is language-specific (TS/Rust) AND distsys, prefer the language `*-review` skill for the language issues and route the distsys-specific bits to the appropriate subagent.

## Process

Run in parallel where possible:

1. Resolve scope. Capture file list and diff.
2. Read changed files. For small files, read the whole thing for context; distsys bugs usually live in the surrounding code.
3. Check the repo's CLAUDE.md and any project conventions (`docs/`, `ARCHITECTURE.md`).
4. Look at related infrastructure files (Dockerfile, k8s manifests, CI workflows, infra-as-code) to understand the deployment context.
5. Walk the categories above against the diff.
6. Route subagent-worthy questions in parallel where independent.

## Reporting

Group findings by severity:

- **blocker** -- data loss risk, correctness bug, outage waiting to happen, security implication (secrets in logs, missing input validation at a trust boundary).
- **major** -- significant operational risk (missing idempotency on a non-GET mutation, unbounded retry, hot-spot inevitability, race condition under failure).
- **minor** -- improvable pattern (offset pagination on a growing list, missing TTL jitter, single TTL across the cache).
- **nit** -- naming, doc, micro-style.

Format:

```
**[severity]** `path/to/file.ts:LINE` -- short headline

<one or two sentences explaining the issue>

<optional: suggested fix or what to specify>
```

For subagent-delegated findings, prefix with the subagent name in brackets: `**[major]** [distsys-runtime] src/handlers/payment.rs:42 -- retry without idempotency key`.

Open with: `Reviewed N files, M findings (X blockers, Y major, Z minor, W nits). Routed K hunks to specialist subagents.`

If the change is clean, "No findings worth flagging" is an honest answer.

## Decision quick-references

Pulled from the rule files. Keep these in mind during review:

- **`Mutex` across `.await` (Rust)**: see `rust-async` for Rust specifics, but the universal principle applies -- never hold a sync lock across an async suspension point.
- **At-least-once + idempotency = exactly-once**: there is no other path to exactly-once. Every consumer of an at-least-once stream needs a dedup mechanism.
- **TTL alone is unsafe for distributed locks**: fencing tokens or conditional writes are required.
- **LWW silently loses data**: if conflict resolution matters, use CRDTs, application-level merge, or single-leader.
- **Strong consistency has a permanent latency cost**: it's not free even in the absence of partitions (PACELC).

## What NOT to do

- **Do not** re-report compiler / linter / type-checker output.
- **Do not** post comments to GitHub. Reports go to chat only.
- **Do not** rewrite code. Suggest fixes inline; let the user decide.
- **Do not** invoke a subagent for trivial issues -- only when delegation actually saves context or buys expertise.
- **Do not** apply rules dogmatically. The user's judgment and project conventions win. If the project does something the rules discourage, mention it once.
- **Do not** flag things explicitly required by a CLAUDE.md or project standards.
- **Do not** invoke `/system-design` -- that's the brainstorm/critique skill for design work, not code review.
- **Do not** go deep on observability gaps -- a dedicated observability skill will exist soon. Flag obvious ones once.
