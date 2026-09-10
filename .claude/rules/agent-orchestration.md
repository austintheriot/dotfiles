---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-10
---

# Agent orchestration

A reference for reviewing and advising on running more than one coding agent at a time on one machine or one account: concurrency caps, queueing and backpressure, supervision, what a stalled agent looks like from outside, shared-resource hazards on one checkout, and the cost model that decides whether a second agent is worth its tokens. Used by the `agent-orchestration` subagent. The scope is the pool and the resources it contends for. It is not what one agent's subprocess may reach (`agent-sandboxing`), not the model server's memory budget (`local-inference`), not the application code that calls a model (`llm-app`), and not cross-process distributed-systems design in production (`distsys-runtime`), though it borrows that lens.

The unifying model: **an agent pool is a queueing system, and the bottleneck is never the one the pool was sized for.** People size for CPU and hit the API's token bucket. They size for the token bucket and hit one `Cargo.lock`. They size for the build and hit their own review bandwidth. Little's law does the rest: when throughput is capped by any one of these, adding agents raises waiting time and not completed work.

The operational question, for every pool: **where is the queue right now, how deep is it, and what signal separates an agent that is waiting from an agent that is stuck?** A pool that cannot answer the third question will discover its stalls by wall clock, which is how one agent sat in a wait loop for over an hour on 2026-09-07 with nobody noticing.

Empirical priority order, by how often each bites:

1. **Shared-resource contention on one checkout.** Two agents in one working tree: `cargo build --locked` fails because a sibling rewrote `Cargo.lock`; a shared `target/` serializes both on `.cargo-build-lock` so the parallelism is fake; `git` dies on `index.lock` with no retry. The vendor's own teams doc: "Two teammates editing the same file leads to overwrites."
2. **Stalls nobody can see.** Liveness is not progress. A reviewer agent that keeps producing findings is alive and not progressing; an agent in a wait loop is alive and not progressing; an agent waiting on a permission prompt in another pane is alive and not progressing. Without a state taxonomy and a signal per state, all three look identical from the outside.
3. **A cap picked by feel.** Three to four "feels right." The API enforces a per-organization token bucket, so N agents on one seat behave like N small-team users, and past the bucket each new agent only adds `retry-after` waits.
4. **Cost without a ledger.** Multi-agent research runs at about 15x the tokens of a chat; agent teams at about 7x a standard session in plan mode; token cost scales linearly with teammates; and Cognition's term nobody budgets for is rework from conflicting decisions made in parallel.
5. **No supervision policy.** Agents "may stop after encountering errors instead of recovering"; the documented remedy is "Spawn a replacement teammate", by hand. There is no restart-intensity cap, so a pool thrashing on a persistent 429 or a broken toolchain restarts forever. Orphans (locked worktrees, tmux sessions) outlive the run.

## Volatile surface

`last-verified` (see frontmatter; do not restate the date here). Vendor machinery rots weekly; the theory does not rot at all.

| Claim class | Rots | Re-verify at |
|---|---|---|
| Claude Code teams, worktrees, agent view, goals, hooks, cost figures | **Weekly** | code.claude.com/docs/en/agent-teams, /worktrees, /agent-view, /agents, /costs, /monitoring-usage, /best-practices |
| Anthropic rate-limit tiers, headers, acceleration limits, cache exemptions | Quarterly | platform.claude.com/docs/en/api/rate-limits |
| Per-user TPM sizing table | Quarterly | code.claude.com/docs/en/costs |
| Cursor parallel-agent features | Monthly | cursor.com/blog, cursor.com/docs |
| Vendor cost and adoption percentages (15x, 7x, 90.2%, 40%, $13/day) | Half-yearly, vendor-reported | Inline citations |
| METR productivity estimates | Per study release | metr.org/research |
| Cargo and git lock semantics | Per major release (years) | doc.rust-lang.org/cargo, `git help config`, git-worktree(1) |
| Little's law, OTP supervision, structured concurrency, SRE overload chapter, USE method | Stable | Cited sources |

## Limits, and how to measure a cap instead of guessing one

### The API side

Anthropic's rate limits (verified) are per organization and per model class, expressed as requests per minute (RPM), input tokens per minute (ITPM), and output tokens per minute (OTPM), enforced by a token bucket that is "continuously replenished". Sub-minute enforcement is real: "60 requests per minute (RPM) might be enforced as 1 request per second." A 429 carries `retry-after`. There are also **acceleration limits** on sharp ramps, so a pool that goes from zero to eight agents in one second trips a limit a steady eight would not. Response headers report `anthropic-ratelimit-{requests,tokens,input-tokens,output-tokens}-{limit,remaining,reset}`; the `tokens-*` set shows the most restrictive limit in effect. Two facts change the arithmetic:

- **Cache-read tokens do not count toward ITPM** (except Haiku 3.5). The doc's own example: a 2,000,000 ITPM limit with an 80% cache hit rate processes 10,000,000 input tokens per minute. Prompt caching is the single largest lever on how many agents one organization can run.
- **OTPM counts actual output, not `max_tokens`**, so a generous `max_tokens` is free until the model uses it.

Workspace limits can cap an agent pool's share of the organization's bucket to protect production traffic (verified). Subscription plans add a five-hour rolling window and a weekly window on top (verified).

Anthropic's own sizing table (verified, **VOLATILE** (2026-09-10)): 1 to 5 users need 200k to 300k TPM and 5 to 7 RPM each; at 500-plus users the per-user figure falls to 10k to 15k TPM and 0.25 to 0.35 RPM, "because fewer users tend to use Claude Code concurrently in larger organizations." **INFERRED**: N concurrent agents on one seat behave like N users at the small-team rate, so a first-order cap for Opus-class agents doing real work is organization TPM divided by about 250k.

### The host side

Google's overload chapter (verified): "simply using CPU consumption as the signal for provisioning works well" in most cases, with executor load average (active threads) as the utilization signal. Brendan Gregg's USE method (verified): for every resource, check utilization, saturation, and errors; measure saturation "as a queue length"; and "any degree of saturation can be a problem (non-zero)". cgroup v2 supplies the per-agent enforcement knobs: `cpu.max`, `memory.high` (throttle) before `memory.max` (OOM kill), `pids.max`, `io.max`.

### The procedure (INFERRED from the above)

Run 1, 2, 4, 8 agents on representative tasks. Record per-agent wall time, host run-queue length, `memory.high` throttling events, I/O wait, and 429 count. The cap is the largest N before any saturation signal is non-zero or per-agent latency degrades faster than linearly. Little's law (L = lambda * W, Little 1961; standard theory, cited from memory) is the interpretation: with throughput lambda capped by a token bucket, adding agents raises W. When observed W per agent grows linearly with N, the pool is saturated on the API dimension, and the eighth agent is buying nothing but `retry-after`.

The 2026-09-07 session picked three to four by feel and never measured. That is the finding this file exists to prevent.

## Backpressure, retries, supervision, structured concurrency

### Retry budgets and admission control

The SRE overload chapter (verified): at most three attempts per request; a per-client retry budget of 10%; adaptive throttling that rejects locally when requests exceed K times accepted, with K = 2; criticality tiers (CRITICAL_PLUS, CRITICAL, SHEDDABLE_PLUS, SHEDDABLE); prefer a degraded response to a crash. Anthropic's client guidance is the same in miniature: honor `retry-after`, ramp gradually (verified). One documented mechanism cuts against this: in agent teams, "A message from the lead or another teammate wakes an in-process teammate that is waiting to retry a failed API request, so it retries immediately instead of waiting for the full retry delay" (verified). **INFERRED**: a chatty lead is a retry-budget bypass, and a pool under 429 pressure with a lead that keeps messaging will hammer the bucket.

### Supervision, from OTP

Erlang/OTP's supervisor principles (verified): restart strategies `one_for_one`, `one_for_all`, `rest_for_one`, `simple_one_for_one`; child restart types `permanent`, `transient`, `temporary`; shutdown by `brutal_kill` or timeout-then-kill; and a **maximum restart intensity**, default one restart per five seconds, past which "the supervisor terminates all the child processes and then itself." Armstrong's thesis (2003) is the source (**FOUND-UNVERIFIED**, cited from memory).

Mapping to agents (**INFERRED**): an agent that errors out on its task is `transient` (restart only on abnormal exit); a reviewer whose implementer died is `rest_for_one` (restart the implementer, then the reviewer, in order); and the restart-intensity cap is the missing guard. Claude Code's teams doc describes the failure OTP prevents, without preventing it: "Teammates may stop after encountering errors instead of recovering", remedy "Spawn a replacement teammate" (verified). That is a human acting as a supervisor with no intensity limit.

### Structured concurrency

Nathaniel Smith (2018, verified): "the nursery block doesn't exit until all the tasks inside it have exited"; a child's exception cancels its siblings, waits for them, and re-raises in the parent; unstructured spawning means "any function might be a goto in disguise." JEP 505 (verified): `StructuredTaskScope`, fifth preview in JDK 25, goals to "eliminate common risks arising from cancellation and shutdown, such as thread leaks" and to make the hierarchy visible in thread dumps. Kotlin (verified): "A parent coroutine waits for its children to complete before it finishes. If the parent coroutine fails or gets canceled, all its child coroutines are recursively canceled too."

Where Claude Code enforces the shape (verified): "an in-process teammate's own subagents run in the foreground, because a teammate's background work can't outlive the lead's process"; no nested teams; cleanup on session exit. Where it violates the shape (verified): `-p` worktree sessions leave locked worktrees behind until a later sweep; orphaned tmux sessions are documented with `tmux ls` and `tmux kill-session` as the remedy. The test for any orchestration design: **can a child outlive its parent?** If yes, the design has a leak and needs a sweep, and the sweep is the design's admission that it leaks.

## What a stalled agent looks like from outside

### Liveness is not progress

The SRE monitoring chapter (verified): four golden signals (latency, traffic, errors, saturation); symptoms versus causes; and the tail example, "average latency of 100 ms at 1,000 requests per second, 1% of requests might easily take 5 seconds." Charity Majors (verified): observability is "the power to ask new questions of your system, without having to ship new code or gather new data"; "By pre-aggregating you are forever-destroying your ability to answer any questions you didn't predict in advance"; high-cardinality fields such as a request ID are the first-order group-by. **INFERRED application**: per-agent wide events keyed by session ID, prompt ID, and tool name are what let you distinguish "waiting on a tool" from "looping". A dashboard of averages cannot.

The vendor's own anti-signal (verified, best practices): "A reviewer prompted to find gaps will usually report some, even when the work is sound." Output volume is liveness. It is not progress.

### The signals Claude Code actually exposes

Telemetry is off by default; `CLAUDE_CODE_ENABLE_TELEMETRY=1` (verified, monitoring-usage). Metrics: `claude_code.session.count`, `lines_of_code.count`, `pull_request.count`, `commit.count`, `cost.usage`, `token.usage`, `code_edit_tool.decision`, `active_time.total`. Events: `user_prompt`, `assistant_response`, `tool_result`, `api_request`, `api_error`, `api_refusal`, `tool_decision`, `permission_mode_changed`, `mcp_server_connection`. `prompt.id` is "a UUID v4 identifier linking all events produced while processing a single user prompt", alongside `message.uuid`, `client_request_id`, `workflow.run_id`.

**A stall taxonomy built from those signals** (**INFERRED**; the signals are verified, the mapping is not):

| State | What the telemetry shows | What to do |
|---|---|---|
| Waiting on a human | agent view `Needs input`; a `tool_decision` with no following `tool_result` | Answer the prompt; the agent is fine |
| Waiting on a tool | `tool_result` absent while a subprocess is alive (a long build, a hung network call) | Inspect the subprocess; set a tool timeout |
| Looping | repeated `tool_result` with identical inputs; `token.usage` rising; `lines_of_code.count` and `commit.count` flat | Stop it; the loop will not end on its own |
| Throttled | `api_error` bursts with 429 and `retry-after` | Reduce N or wait; do not add agents |
| Dead | process gone with no `Completed` or `Failed` transition | Restart under a supervision policy |

### The agent view and its watchdog

Agent view states (verified): Working, Needs input ("a permission decision, or another prompt only you can answer, such as a sandbox prompt to allow a network host"), Idle, Completed, Failed, Stopped; glyphs distinguish "process is alive" from "process has exited but you can still peek and reply". A supervisor stops a session "Finished or waiting for your next message, and unattached for about an hour." A session that was mid-response when the machine slept "can come back unresponsive"; the supervisor restarts it. That one-hour watchdog is the only automatic stall handling in the product, and it acts on idleness, not on looping.

### Progress gates

`/goal` re-evaluates after every turn and "If Claude stalls, Claude Code eventually stops the run with the goal still set"; a Stop hook can block a turn until a check passes, but "Claude Code overrides the hook and ends the turn after 8 consecutive blocks"; at most three idle goal check-ins per goal (2.1.246 and later; verified). Agent teams expose `TeammateIdle`, `TaskCreated`, and `TaskCompleted` hooks, where exit code 2 pushes back (verified). Two documented weaknesses: "Task status can lag: teammates sometimes fail to mark tasks as completed, which blocks dependent tasks", and "The lead can stop early too." Task claiming uses file locking; mailbox JSON files live under `~/.claude/teams/<name>/inboxes/`, and a malformed entry blocked delivery every second before 2.1.207 (verified).

## Shared-resource hazards on one checkout

This is the section the 2026-09-07 session needed.

### Cargo

`cargo build --locked` errors when "Cargo attempted to change the lock file due to a different dependency resolution" (verified). **INFERRED mechanism of the observed failure**: agent A runs `cargo add` or a resolution-changing build and rewrites `Cargo.lock`; agent B's concurrent `--locked` build sees a lock file that no longer matches its resolution and fails; B's own `Cargo.toml` edits meanwhile make A's lock stale. The failure is transient because it depends on interleaving, which is why it looked like flakiness. Separately, two agents sharing one `target/` serialize on "[BLOCKING] waiting for file lock on build directory (.../target/debug/.cargo-build-lock)" (verified in cargo's own test suite), so the parallelism they appear to have is not there. Mitigations: per-agent `CARGO_TARGET_DIR` (verified flag), `--frozen` (equals `--locked` plus `--offline`), a worktree per agent so each has its own `Cargo.lock`, and sccache to share compiled artifacts across target directories (**FOUND-UNVERIFIED** for this use).

### git

Reproduced locally on git 2.50.0: "fatal: Unable to create '.../.git/index.lock': File exists. Another git process seems to be running in this repository..." There is **no retry for the index lock**. Only refs retry: `core.filesRefLockTimeout` (default 100 ms; -1 waits forever) and `core.packedRefsTimeout` (default 1000 ms) (verified, `git help config`). Two agents running `git add` in one working tree will collide, and the loser exits non-zero with no backoff.

Worktrees (verified, git-worktree): they share the object database, refs, and config, and have private `HEAD`, `index`, `refs/bisect`, `refs/worktree`, `refs/rewritten`. A branch can be checked out in one worktree only. Submodules: "Multiple checkout in general is still experimental, and the support for submodules is incomplete." **INFERRED**: two agents in two worktrees still contend on `packed-refs` and `.git/config`, which have the retry timeouts above, and no longer contend on `index`, which does not.

### Claude Code's worktree machinery

Verified: `claude --worktree <name>` creates `.claude/worktrees/<name>/` on branch `worktree-<name>`; subagents take `isolation: worktree` in frontmatter; four enforced checks block edits, `cwd` changes, `git -C` / `GIT_DIR` / `GIT_WORK_TREE` redirects, and unparseable command shapes that could touch the main checkout; `git worktree lock` is held while an agent runs; a periodic sweep runs after `cleanupPeriodDays`; `.worktreeinclude` copies gitignored files such as `.env` into each worktree; a fresh worktree has no `node_modules` or `target`, so dependencies reinstall; base branch `fresh` (origin default) versus `head`; filter drivers such as LFS `--local` are skipped at creation because "a filter driver is a shell command"; agent view moves background sessions into worktrees before their first edit. Permission approvals granted in a worktree save to the main checkout's `.claude/settings.local.json` and apply everywhere (2.1.211 and later).

And the fact that decides the design: **agent teams do not isolate teammates in worktrees.** The doc's instruction is "partition the work so each teammate owns a different set of files", and its warning is "Two teammates editing the same file leads to overwrites" (verified). Subagents can be isolated; teammates share the tree.

Cursor 2.0 (2025-10-29, verified): parallel agents "powered by git worktrees or remote machines", and "having multiple models attempt the same problem and picking the best result significantly improves the final output, especially for harder tasks."

### The rest

- **Shared package caches** (cargo registry, `~/.npm/_cacache`, pip): advisory locks that serialize (**FOUND-UNVERIFIED** beyond the cargo build-dir lock).
- **Ports**: no vendor document addresses collision (**INFERRED**). Bind port 0 and read back the assignment, or give each agent a port range through the environment.
- **tmux**: split-pane teams need tmux or iTerm2's `it2`; VS Code's terminal, Windows Terminal, and Ghostty are unsupported for split panes; orphaned sessions are documented (verified).

## The cost model

Every figure here is vendor-reported or from one research group, and **VOLATILE** (2026-09-10).

- **Anthropic's multi-agent research system** (2025-06-13, verified): agents use about 4x the tokens of a chat; multi-agent systems about 15x; token usage explains 80% of the variance on BrowseComp; an Opus 4 lead with Sonnet 4 subagents beat a single Opus 4 by 90.2% on the internal research eval and "cut research time by up to 90% for complex queries." The caveats in the same post: not for "domains that require all agents to share the same context or involve many dependencies", and the design must "require tasks where the value of the task is high enough to pay for the increased performance."
- **Claude Code costs** (verified): enterprise average "around $13 per developer per active day and $150-250 per developer per month", "below $30 per active day for 90% of users"; agent teams use "approximately 7x more tokens than standard sessions when teammates run in plan mode"; "Token costs scale linearly" with teammates; "Three focused teammates often outperform five scattered ones"; start with 3 to 5 teammates and 5 to 6 tasks each. Background burn: scheduled tasks, cross-session messages, and goal check-ins each resend full context. The in-process teammate cache TTL defaults to five minutes (`subagentPromptCacheTtl: 1h` to extend), which interacts with the ITPM cache exemption above: a teammate idle past the TTL pays full input price on its next turn.
- **Cognition, Walden Yan** (2025-06-12, verified): "Share context, and share full agent traces"; "Actions carry implicit decisions, and conflicting decisions carry bad results"; parallel subagents "make conflicting assumptions not prescribed upfront"; "running multiple agents in collaboration only results in fragile systems." The cost term this names is **rework from conflict**, and no vendor cost table includes it.
- **METR** (verified, 2025-07-10 and 2026-02-24): the early-2025 randomized controlled trial (RCT) found 16 experienced developers on 246 issues were 19% slower with AI while expecting to be 24% faster and believing afterwards they had been 20% faster. The 2026 update: the original cohort at -18% (confidence interval -38% to +9%), new recruits at -4% (-15% to +9%), across 57 developers, 143 repositories, 800-plus tasks; "30% to 50% of developers told us that they were choosing not to submit some tasks because they did not want to do them without AI"; and the sentence that matters for this file: "Time measurements are unreliable for developers using multiple AI agents concurrently." The only RCT group in the space says the N-agent workflow cannot yet be measured by wall clock.
- **Cursor** (verified): sandboxing cut agent stops by 40%, and the stated motivation is approval fatigue. **INFERRED**: the human's attention is the bottleneck the vendors are engineering around.
- **`/batch`** (verified): 5 to 30 worktree-isolated subagents, each opening a PR. **INFERRED**: review bandwidth binds long before compute does. Thirty PRs is thirty reviews.

## Schools of thought

Each position in its own strongest form. Do not average them.

### Many small agents, or few capable ones

**Many.** Anthropic's research system: parallel Sonnet subagents under an Opus lead, plus 90.2%, at 15x tokens, and the tokens explain the gain. Cursor's best-of-N: several models attempt one problem and a human picks. `/batch` and dynamic workflows for "a codebase-wide audit, a 500-file migration". Independence is cheap to buy when the task decomposes.

**Few.** Cognition: "decision-making ends up being too dispersed", and parallel agents make conflicting assumptions that cost rework. Anthropic's own caveat about shared-context domains. The vendor's own doc: "Three focused teammates often outperform five scattered ones", and "For sequential tasks, same-file edits, or work with many dependencies, a single session or subagents are more effective." METR: with concurrent agents, even the measurement is unreliable, so nobody knows.

Unreconciled. The shared observation both camps make: **the win condition is task independence, and coding tasks are less independent than research tasks.**

### Parallel-then-merge, or serial

**Parallel.** A worktree per agent (Anthropic, Cursor); best-of-N; adversarial multi-hypothesis debugging, because "Sequential investigation suffers from anchoring" (verified, teams doc). Different agents starting from different hypotheses is a real defense against a single agent's fixation.

**Serial.** Cognition's single-threaded agent with the full trace in one context. The Writer/Reviewer pattern is sequential by design. And the concrete hazard: teams do not isolate teammates, so parallel edits in one tree overwrite each other.

Unreconciled. When each is right: parallel when the branches are hypotheses or disjoint file sets; serial when the second step needs the first step's reasoning and not just its output.

### Human in the loop per agent, or per batch

**Per agent, per action.** Manual mode. "Teammate permission prompts appear in the lead session, so approve them there yourself." Becker's conclusion after the credential leak: disable the sandbox and approve every command. The security page: "You're responsible for reviewing proposed code and commands for safety before approval."

**Per batch.** Auto mode's classifier "blocks scope escalation, unknown infrastructure, and hostile-content-driven actions". Cursor Auto-review. `/batch` produces thirty PRs and the human reviews PRs, which is the artifact humans are good at reviewing. Agent view: "step in only when one needs you." The same vendor supplies the counterweight in the same doc set: "Letting a team run unattended for too long increases the risk of wasted effort."

Unreconciled. Both vendors ship both.

## Anti-pattern catalog

Each: the pattern, the trigger, the consequence, the fix.

- **Two agents, one working tree.** Trigger: teammates or ad hoc parallel sessions in one checkout. Consequence: `index.lock` failures with no retry, `Cargo.lock` races, overwritten files. Fix: a worktree per agent (`isolation: worktree`), or partition by file set and mean it.
- **`--locked` as a correctness gate while a sibling can resolve.** Trigger: one agent runs `cargo add`. Consequence: the other's build fails transiently and gets called flaky. Fix: per-agent worktrees, or a single agent owns dependency changes.
- **Shared `target/` called parallel.** Trigger: agents in one tree or worktrees sharing `CARGO_TARGET_DIR`. Consequence: serialized on `.cargo-build-lock`; wall time equals serial. Fix: per-agent target directories, or accept serial and save the tokens.
- **A cap picked by feel.** Trigger: "three or four seems fine." Consequence: 429 waits that look like slow agents, or a host in swap. Fix: the 1-2-4-8 procedure; write the number and the saturation signal that set it.
- **Ramping to N in one second.** Trigger: spawning the whole pool at once. Consequence: the acceleration limit fires before the steady-state limit would. Fix: stagger spawns.
- **Ignoring cache exemption.** Trigger: agents with divergent system prompts. Consequence: every input token counts against ITPM. Fix: a shared cached prefix; the exemption is worth 5x at an 80% hit rate.
- **No stall taxonomy.** Trigger: "it has been running a while." Consequence: an hour in a wait loop. Fix: the five-state table above, with the telemetry field for each, and a timeout per tool call.
- **Reading findings volume as progress.** Trigger: a reviewer agent that keeps reporting. Consequence: a run that never converges. Fix: "A reviewer prompted to find gaps will usually report some, even when the work is sound." Bound the review by rounds, not by silence.
- **Manual supervision with no intensity cap.** Trigger: "spawn a replacement." Consequence: a pool that restarts forever on a persistent 429 or a broken toolchain. Fix: an explicit restart budget (OTP's one-per-five-seconds is the reference) and a policy per agent kind: `transient`, `temporary`, `rest_for_one`.
- **Children that outlive the parent.** Trigger: `-p` worktree sessions, tmux panes, background tasks. Consequence: locked worktrees and orphaned sessions until a sweep. Fix: structured concurrency: the run does not end until its children have, and anything else is a leak with a janitor.
- **A chatty lead under 429.** Trigger: the lead messages teammates that are waiting to retry. Consequence: they retry immediately, bypassing the backoff. Fix: quiet the lead when `api_error` is bursting.
- **Cost without the rework term.** Trigger: a 15x token budget approved for a coding task. Consequence: conflicting decisions and a merge nobody budgeted. Fix: price the merge, or pick serial for dependent work.
- **Thirty PRs and one reviewer.** Trigger: `/batch` at its upper bound. Consequence: review bandwidth binds, and PRs merge unread or rot. Fix: size the batch to the reviewer, not to the compute.

## Authorities

- **Google SRE Book**: "Handling Overload" (retry budgets, adaptive throttling, criticality) and "Monitoring Distributed Systems" (golden signals, symptoms versus causes, tail latency).
- **Brendan Gregg, the USE method**, for "any degree of saturation can be a problem (non-zero)".
- **Little (1961)** for L = lambda * W, the one equation every pool obeys.
- **Erlang/OTP supervisor principles**, and Armstrong's 2003 thesis, for restart strategies and the restart-intensity cap.
- **Nathaniel J. Smith, "Notes on structured concurrency, or: Go statement considered harmful"** (2018); **JEP 505**; **Kotlin coroutine docs**, for the rule that children do not outlive parents.
- **Charity Majors, "Observability: A Manifesto"**, for why pre-aggregation destroys the question you did not predict.
- **Anthropic**: the rate-limits reference, the Claude Code costs and monitoring pages, the agent-teams, worktrees, agent-view, and best-practices docs, and "How we built our multi-agent research system" (2025-06-13).
- **Walden Yan, Cognition, "Don't build multi-agents"** (2025-06-12), the strongest case for shared context and serial execution.
- **METR** (2025-07-10; 2026-02-24 update), the only randomized trial data, and its statement that concurrent-agent time cannot yet be measured.
- **Cursor**: the 2.0 announcement (2025-10-29) and the sandboxing post (2026-02-18) for best-of-N and approval fatigue.
- **Cargo documentation** (`--locked`, `--frozen`, `CARGO_TARGET_DIR`) and cargo's `tests/testsuite/concurrent.rs`; **git-worktree(1)** and `git help config` for lock timeouts.

## Severity rubric

What the levels mean in this domain specifically.

- **blocker**: two or more agents editing one working tree with no partition and no worktree; `--locked` builds running while another agent can change resolution; a pool with no stall detection running unattended; a restart loop with no intensity cap; a child that can outlive its parent with no sweep.
- **major**: a concurrency cap with no measurement behind it; agents spawned without stagger against a known acceleration limit; no per-tool timeout; findings volume used as a convergence signal; a cost plan that omits rework from conflict; a `/batch` sized past the reviewer.
- **minor**: shared `target/` presented as parallel; teammate cache TTL left at five minutes for a pool that idles longer; system prompts that defeat the cache exemption; ports chosen by convention rather than by `bind 0`.
- **nit**: a cost figure cited without its date; a vendor percentage repeated without "vendor-reported".
- **insight**: structural observations. "This pool's tasks are dependent; Cognition's argument applies and serial is the cheaper design." "This pool's binding constraint is the human reviewer, and every engineering effort here is on the compute side."

## Changelog

**Source research**: `~/.claude/local/research-notes/sandboxing-orchestration.md` (shared with `agent-sandboxing`; Part 2 is this file's source; claims tagged VERIFIED / FOUND-UNVERIFIED / INFERRED, with a gaps list). Read it before a refresh.

- **2026-09-10** -- Initial version. Rate limits and sizing table verified at platform.claude.com and code.claude.com. Telemetry surface, agent view states, goal and hook behavior, worktree and teams machinery verified at code.claude.com. Cargo `--locked` semantics verified at doc.rust-lang.org and the build-dir lock in cargo's test suite; git `index.lock` reproduced on git 2.50.0 and ref timeouts verified in `git help config`. OTP, structured concurrency, SRE, USE, and Honeycomb sources verified. Cost figures from Anthropic, Cognition, Cursor, and METR verified at source. The stall taxonomy and the 1-2-4-8 procedure are inferred and marked. Known gaps: Little's law and Armstrong's thesis cited from memory; port collision and shared package-cache locks inferred; Devin, Factory, and OpenAI cloud parallel-agent numbers not fetched.
