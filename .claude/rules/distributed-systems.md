---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Distributed Systems Principles

Load-bearing mental models for systems where more than one machine, process, or service must agree on something. Frames to think *in*, not tactical patterns.

## 1. Entities and activities are different things

**Headline.** Inside one entity, serial consistency. Across entities, messages and uncertainty -- no third option.

**Origin.** Pat Helland's "Life Beyond Distributed Transactions." An *entity* is the unit at which strong consistency is achievable (a row, an aggregate, a single-node document). An *activity* is coordination *between* entities, fundamentally asynchronous.

**Why it matters.** Most architecture confusion is asking for strong consistency *across* entities without realizing it. Accept the partition and cross-service questions collapse into questions about messages, idempotency, reconciliation. DDIA's "system of record / derived" descends from this.

**Apply when.** Drawing service or aggregate boundaries. What is the entity? Which activities cross? Cross-boundary operations are messages with stable IDs and idempotent handlers, not joins.

## 2. Uniqueness lives within a scope

**Headline.** "Unique" is always relative to a scope -- globally unique requires coordination.

**Origin.** Helland. UUIDs work because their scope is "all v4 generators, statistically." Auto-increment keys are a trap across shards.

**Apply when.** Pick the smallest scope that gives uniqueness for free; escalate to coordination (sequences, allocators, consensus) only when the scope forces it.

## 3. Fate sharing is the boundary of a transaction

**Headline.** A transaction is meaningful only across things that fail together.

**Origin.** Helland. Same process, same disk, same node: transactions are cheap. The moment two pieces of state can fail independently, a "transaction" across them is a distributed transaction -- and you almost certainly don't want one.

**Apply when.** Reaching for two-phase commit or a cross-service transaction. If they don't share fate, design for "one succeeded, the other didn't": compensations, sagas, idempotent retry to convergence.

## 4. Data on the outside is immutable

**Headline.** Once data has left your service, you cannot change it -- only publish a new version.

**Origin.** Helland's "Data on the Outside vs. Data on the Inside." Inside data is mutable under your transactional control; outside data -- handed to another service, written to a log, returned in an API response -- is frozen. Outbound data is a publication, not a pointer.

**Apply when.** Designing message schemas, event payloads, API responses. Make them versioned, self-describing, rich enough to interpret without calling back.

## 5. Append-only logs are the foundational primitive

**Headline.** A log -- ordered, append-only, durable -- is the substrate from which databases, queues, replication, and caches are derivations.

**Origin.** Jay Kreps, "The Log." Helland's "Immutability Changes Everything" makes the same case from the data-modeling side.

**Why it matters.** Once you see the log, you stop arguing about message bus vs. database vs. cache and start arguing about *which interpretation* of the log you want. State is a cached projection; replication ships the log; change-data-capture exposes it.

**Apply when.** Multiple services need the same change. Publish to a log; each maintains its own projection. Log is truth; projections are disposable.

## 6. Kappa over Lambda

**Headline.** One log, one processing path, reprocess when needed -- don't run batch and stream side by side.

**Origin.** Kreps, "Questioning the Lambda Architecture." Lambda parallels batch and stream and reconciles; Kappa keeps the stream and replays the log when logic changes.

**Apply when.** Tempted to add batch reconciliation alongside a streaming view. Ask whether a durable log plus replay lets you delete the batch path. Usually yes.

## 7. CALM: monotonic programs need no coordination

**Headline.** A program is consistent without coordination iff it is monotonic in its input.

**Origin.** Joe Hellerstein's CALM theorem (Consistency As Logical Monotonicity). Monotonic operations (set union, max, "ever seen") only add information -- replicas cannot disagree given the same eventual input. Non-monotonic ones (delete, exact count, "currently active") can flip on later input and require coordination.

**Apply when.** Replicated state or multi-master logic. Push as much as possible into monotonic shape (grow-only sets, version vectors, CRDT-style). Non-monotonic operations become the explicit coordination points; nothing else has to be.

## 8. The consistency hierarchy is a lattice, not a binary

**Headline.** "Strong" and "eventual" are endpoints; the design space is between them.

**Origin.** Aphyr (Kyle Kingsbury), "Strong consistency models." Linearizable > sequential > causal > eventual, plus session guarantees: read-your-writes, monotonic reads, monotonic writes, writes-follow-reads. Users don't notice eventual consistency abstractly, but they *do* notice losing their own post or a timeline jumping backward -- the session layer is where user-facing promises live.

**Apply when.** Specifying guarantees. Don't say "strongly consistent" -- say "linearizable per key," "causally consistent across a session," "read-your-writes within a region."

## 9. PACELC: you trade off all the time, not just during partitions

**Headline.** CAP describes partition behavior. PACELC adds: *even without a partition*, you trade latency for consistency.

**Origin.** Daniel Abadi's PACELC. CAP is the emergency-mode theorem; PACELC is steady-state. Every multi-region read has a latency-vs-consistency dial whether or not the network is broken.

**Apply when.** Choosing a database or replication topology. Ask both: partition behavior, and steady-state latency cost of the consistency level.

## 10. Eventual consistency has a shape

**Headline.** "Eventually consistent" is not one thing -- it has an inconsistency window, client-centric guarantees, and conflict-resolution semantics.

**Origin.** Werner Vogels, "Eventually Consistent." Writes converge if they stop, but window size and per-client guarantees during the window are independent choices.

**Apply when.** Promising eventual consistency. Specify the window (bounded by what?), per-session guarantees, and the conflict-resolution rule. "Eventual" without those is a non-promise.

## 11. Most transactions don't need serializability; lin and ser are different axes

**Headline.** Serializability is overkill for most workloads. Linearizability (single-object real-time order) and serializability (multi-object equivalence to *some* serial order) are orthogonal -- either, both, or neither.

**Origin.** Peter Bailis, "Highly Available Transactions" and RAMP: atomic visibility (a multi-object write visible all-or-nothing) is achievable without coordination. His lin/ser clarification has settled many architecture arguments.

**Apply when.** Someone asks for "a transaction." Pin down what they need: atomic visibility, no lost updates, monotonic reads, isolation from a specific op. Usually a weaker, coordination-free protocol suffices. Strict serializability is both axes; snapshot isolation is neither.

## 12. Coordination is the cost; design to avoid it

**Headline.** Coordination -- consensus, locks, distributed transactions -- costs latency, availability, complexity. Default to coordination-free.

**Origin.** Bailis and Hellerstein's coordination-avoidance work; I-Confluence is the formal lens: an invariant is coordination-free maintainable iff preserved under all interleavings.

**Apply when.** Mark every place requiring coordination. Ask: is the invariant actually required, or incidental? Drop the incidental ones.

## 13. Memories, guesses, and apologies

**Headline.** Async systems decide on memories (past state) and guesses (predictions); when reality disagrees, the system apologizes. Build apologies in from the start.

**Origin.** Helland, "Memories, Guesses, and Apologies." Any system at human time over partitioned infrastructure acts on stale information and must have a story for being wrong.

**Apply when.** Async workflows (booking, charging, inventory, notifications). The happy path is easy. Name the apology: refund, retraction, correction event, escalation. If you can't name it, you don't have a design.

## 14. Formal methods for the load-bearing parts

**Headline.** TLA+ for protocol, deterministic simulation testing for implementation, property-based testing in between.

**Origin.** Lamport's TLA+; Hillel Wayne's writing on it; the FoundationDB / TigerBeetle deterministic-simulation lineage; Antithesis productizing it. Example-based tests can't cover a concurrent protocol's state space. Model checking proves properties; deterministic simulation runs millions of pseudo-random schedules against a real implementation under controllable clock and network.

**Apply when.** Protocol matters more than throughput: consensus, replication, allocation, leases, state machines money rides on. Sketch in TLA+ or PlusCal first. Make I/O, clock, and randomness injectable so the implementation runs deterministically under a chaotic schedule.

## 15. Replicate for durability, shard for capacity, don't conflate them

**Headline.** Most correctness-critical systems should be one logical machine, replicated. Sharding is a capacity tool, not a correctness tool, and it makes everything harder.

**Origin.** The TigerBeetle thesis (Joran Greef): financial-grade systems usually aren't capacity-bound on modern hardware -- one core clears millions of transfers per second. Sharding's tax (cross-shard transactions, rebalancing, hot partitions) buys nothing if you weren't capacity-bound.

**Apply when.** About to shard. Ask: am I actually capacity-bound on one machine (Non-Volatile Memory Express (NVMe) storage, tens of cores, hundreds of GB of RAM)? If not, replicate for durability and high availability, keep the model single-node, revisit only when measurement forces it.

## 16. Time is a lie; make causality explicit

**Headline.** Wall-clock time across machines is not ordered. For ordering, use logical clocks, version vectors, or a consensus-issued sequence -- never `now()`.

**Origin.** Lamport's logical clocks, plus Aphyr's evidence that "last-write-wins by timestamp" silently drops data.

**Apply when.** `timestamp` is being used to order events across machines, resolve conflicts, or as a unique key. Replace with a Lamport clock, Hybrid Logical Clock (HLC), or consensus-issued sequence per need.

## 17. Idempotency is non-negotiable

**Headline.** Every cross-process operation will be retried. Make every handler produce the same result on the second delivery as the first.

**Origin.** Helland: at-least-once is the realistic default; "exactly-once" is achievable only via idempotent receivers (end-to-end dedup on a stable request ID). Exactly-once is a property of the *application*, not the transport.

**Apply when.** Every Remote Procedure Call (RPC), message handler, webhook receiver. Require a stable request ID at the boundary; record processed IDs; return the prior result on replay.

---

# Part II: Operational Patterns

Principles 1 to 17 are about *what the system promises*. Principles 18 onward are about *how it behaves under load and failure*. Distilled from the AWS Builder's Library (Brooker, Yanacek, MacCarthaigh, Gabrielson, Weiss, Furr, Featonby, Wires, Brinkley, Chhabra) and Marc Brooker's blog.

## 18. Retries are selfish; budget them globally

**Headline.** Every retry is the client spending the server's time to raise the client's success rate. Unbudgeted retries turn partial failures into total ones.

**Why.** When a downstream is already overloaded, retries multiply offered load by `1 + retries` per layer; three retries at three layers is a 243x amplifier. The retry storm is the canonical metastable trigger.

**Pattern.** Use a retry-token bucket (success deposits a fraction of a token, retry costs one whole token) so retries auto-throttle as failure rate climbs. Retry at exactly one layer of the call stack, not at every layer. Reviewer flag: fixed retry counts repeated at multiple layers, retries on non-idempotent operations, or a retry policy that does not adapt to observed failure rate.

## 19. Exponential backoff without jitter is a synchronizer

**Headline.** Plain exponential backoff lines clients up to retry at the same moment -- the exact failure mode backoff was meant to prevent.

**Why.** Backoff spreads retries across time only if the spread is randomized. Identical timers fire identically.

**Pattern.** Full jitter as the default: `sleep = uniform(0, min(cap, base * 2**attempt))`. Decorrelated jitter is the runner-up: `sleep = min(cap, uniform(base, previous_sleep * 3))`. Reviewer flag: retry code that does `sleep(base * 2**attempt)` with no randomization, or a "tiny" jitter window (plus-or-minus 10 percent) too small to actually break correlation.

## 20. Backoff alone does not reduce work; combine with budgets

**Headline.** Backoff defers work to later; with an unbounded client population, "later" still arrives in a synchronized wave.

**Why.** Brooker: backoff reduces total work only with a bounded number of serial clients. With many independent clients (web users, autoscaled fleets), it shifts the spike without shrinking it. Total work landing on the server is unchanged unless something also caps retry count or rate.

**Pattern.** Backoff plus jitter plus a retry budget plus an idempotent server -- four things, not one. Reviewer flag: "we added jitter, we are protected" with no cap on aggregate retry rate.

## 21. Circuit breakers are mode switches; prefer continuous load shedding

**Headline.** Circuit breakers convert a sharded, partial failure into a global, total one, and create untested code paths.

**Why.** A breaker tripped on aggregate error rate cannot tell that only one shard is sick; it pulls the plug on healthy shards too. Modal behavior (open / closed / half-open) is fragile during recovery and rarely exercised in production.

**Pattern.** Prefer fine-grained load shedding (per-shard, per-tenant, per-priority) and adaptive retry budgets over a global breaker. If you do use a breaker, scope it to the specific dependency that can independently fail, and exercise the tripped path in production regularly. Reviewer flag: a single global circuit breaker on a sharded backend.

## 22. Fallback paths are untested code paths

**Headline.** Code that only runs during outages is, by definition, the least-tested code in your system. It will surprise you.

**Why.** Gabrielson catalogs the failure modes: fallbacks are hard to test, cascade their own failures, place unpredictable load on shared dependencies, and harbor latent bugs for years. The 2001 Amazon retail outage went from partial to total when the cache-miss fallback locked up the database.

**Pattern.** Eliminate fallback in favor of either (a) making the primary path reliable enough not to need one, (b) pushing the failure decision up to the caller and letting it retry with backoff, or (c) converting the fallback into an always-on failover that is continuously exercised. Reviewer flag: a `try { primary } catch { fallback }` block where the fallback talks to a different system or runs a different algorithm.

## 23. Static stability: steady state requires no control-plane action

**Headline.** A statically stable system continues to do the right thing while its control plane is unavailable.

**Why.** Outages are when you most need to launch instances, refresh credentials, recompute routes, push config -- and they are when those operations are most likely to fail. Any plan that says "during the failure, we will do X" depends on X working during the failure.

**Pattern.** Pre-provision across failure domains (run three Availability Zones (AZs) at 66 percent so any one can absorb the others); keep zonal services zonal; data plane independent of control plane; static config baked in rather than fetched at boot. Reviewer flag: a recovery plan requiring autoscaling, Domain Name System (DNS) changes, credential refresh, or fresh config fetches on the critical path during an outage.

## 24. Constant-work patterns: same work in good times and bad

**Headline.** A system that does the same amount of work regardless of input rate has no positive feedback loop to amplify.

**Why.** The Route 53 health-check propagator sends a fixed-size table every interval, padded with dummies, whether one or millions of checks changed. Massive simultaneous failures cause no additional work, so no retry storm, no metastable mode.

**Pattern.** Push, do not pull; full-state, not deltas; precompute at constant cadence; cap output size and pad to the cap. Reviewer flag: a "push only what changed" optimization on a critical control path -- the optimization makes the system cheaper in the common case and unbounded in the failure case.

## 25. Shuffle sharding: probabilistic isolation between tenants

**Headline.** Assign each tenant a random small subset of workers; the probability that two tenants share *all* their workers shrinks combinatorially with pool size.

**Why.** A bad tenant (poison request, hot key, traffic flood) only damages workers in its shuffle shard. Other tenants overlap partially at worst, so most have at least one untouched worker. With 8 workers and shard size 2 there are 28 shards; with 100 workers and shard size 5 there are 75 million.

**Pattern.** Pick shard size so the probability of full overlap between any two tenants is acceptably small. Reviewer flag: a multi-tenant system where one tenant's load can saturate a resource that all tenants share (one queue, one thread pool, one connection pool, one database session).

## 26. Queues are bimodal; defend against the slow mode

**Headline.** A queue runs in one of two regimes: empty-ish and fast, or backlogged and slow. The transition is sharp and recovery is asymmetric.

**Why.** Yanacek: once processing latency exceeds the visibility timeout, redelivery doubles offered load and the system "fork-bombs itself." Once a backlog forms, draining it takes roughly double capacity for the same duration the backlog took to form.

**Pattern.** (a) Cap queue depth; reject at the edge rather than accept-and-fail. (b) Time out work whose deadline has already passed before processing it. (c) Sideline old messages to a separate queue so fresh work does not wait behind stale work (Last-In-First-Out, LIFO-ish on the live queue). (d) Heartbeat long jobs to prevent spurious redelivery. Reviewer flag: an unbounded queue, or a worker that processes a message without checking whether the original requester has already given up.

## 27. Metastable failures: the trigger is not the disease

**Headline.** A metastable failure is a system that stays broken after the trigger is removed, because a positive feedback loop sustains it.

**Why.** Brooker: a load spike pushes the system into a regime where retries, garbage-collection (GC) pressure, lock contention, or cache misses each generate more of themselves. Removing the original trigger does nothing; the loop sustains itself. Retry-induced load is over half the sustaining effects in real-world incidents studied.

**Pattern.** Identify the sustaining loop, then break it with a one-shot intervention: drop traffic (not just throttle), restart workers to clear GC heaps and connection state, flush caches, drain queues. Design preventively: limit concurrency (not just arrival rate), bound retries with budgets, prefer constant-work patterns. Reviewer flag: a runbook whose recovery step is "wait for traffic to subside" with no active intervention to break the loop.

## 28. Tail latency is what users feel; chains amplify it

**Headline.** With many parallel or serial sub-calls per user request, the p99 of any component becomes the p50 of the user experience.

**Why.** Brooker: 10 parallel sub-calls turn a 1-percent-rare slow component into roughly 10-percent-of-user-requests slow. For serial chains, variance compounds (he cites a 25x factor in one example). Averages hide this entirely.

**Pattern.** Set Service Level Objectives (SLOs) and alarms on p99 / p99.9 of *user-visible* latency, not service-internal averages. Use hedged requests, request reissue, or scheduling tweaks (Brooker's "nudge": swap a small job ahead of a large one, once) to flatten the tail. Reviewer flag: an SLO stated in averages, or a dashboard with no high-percentile latency panel.

## 29. Little's Law: concurrency, throughput, and latency are one quantity

**Headline.** `concurrency = arrival_rate * time_in_system`. You cannot move two of the three independently; the third follows.

**Why.** Concurrency is what consumes real resources (threads, file descriptors, memory, connections). When latency rises with steady arrival rate, concurrency rises silently until a hard limit fails.

**Pattern.** Size thread pools, connection pools, and admission limits from Little's Law, not intuition. Watch concurrency as a leading indicator: steady arrival rate plus rising concurrency means rising latency, even if your latency metric has not caught up yet. Reviewer flag: a thread pool sized to "feels about right" with no measurement of in-flight concurrency.

## 30. Utilization above ~80 percent is a cliff, not a slope

**Headline.** Mean queue length grows as `rho / (1 - rho)`. At 50 percent utilization mean queue is 1; at 99 percent it is 99.

**Why.** Pure queueing theory. Headroom is not waste; it is the only thing between you and a latency cliff. Real systems are worse than the formula because of Universal Scalability Law contention.

**Pattern.** Run critical-path services at 50 to 70 percent utilization. Reserve the rest for variance and recovery. Use mean latency as the efficiency metric and p99 as the impending-overload indicator -- they tell you different things. Reviewer flag: a capacity model targeting 90-plus percent utilization or treating mean latency as the saturation signal.

## 31. Caches are addictive and turn failure into worse failure

**Headline.** A system that depends on cache hit rate has a hidden second steady state: cold cache, high latency, unrecoverable.

**Why.** Brinkley / Chhabra and Brooker: when a cache empties (deploy, failover, restart, eviction storm), the surge of misses lands on a backend scaled assuming the cache absorbs it. The miss traffic is also less cacheable than the hit traffic (popular keys were already cached), so the cold state self-sustains.

**Pattern.** (a) Negative caching: cache the not-found and the error, with their own Time-To-Live (TTL). (b) Request coalescing: one miss for the same key in flight, the rest wait on it. (c) Two-tier TTL: a soft TTL that triggers async refresh, a hard TTL that fails open. (d) TTL jitter so caches do not expire together. (e) Load-test with the cache disabled and confirm the backend survives. (f) Treat the cache as best-effort optimization, not a capacity multiplier you are entitled to. Reviewer flag: a capacity plan assuming a particular hit rate, or a deploy that flushes cache with no warmup.

## 32. Idempotency tokens have a lifecycle, not just a value

**Headline.** Real idempotency requires the server to atomically record the token alongside the side effect, return semantically equivalent responses on replay, reject token reuse with different parameters, and remember the token long enough to outlive in-flight retries.

**Why.** Principle 17 says "be idempotent." This is *how* you do it. A dedup table that forgets too fast lets a late retry create a duplicate; one that forgets too slow accumulates state forever. DynamoDB's `ClientRequestToken` and S3's request IDs handle this through bounded retention (lifetime of the resource plus a safety margin).

**Pattern.** Stable token at the API boundary; atomic write of `(token, response)` inside the same transaction as the side effect; reject mismatched-parameter reuse with a validation error rather than executing again; document retention window explicitly. Reviewer flag: an idempotency check that reads the token *before* the write and persists it *after* (race window between the two), or one with no documented retention.

## 33. Continuous delivery requires graduated blast radius

**Headline.** Deploys are the largest single source of outages. Treat each rollout as a series of bets, each roughly ten times larger than the last, each gated on the previous one not bleeding.

**Why.** A canary at 0.1 percent of traffic catches most bad changes at one-thousandth the blast radius of a full deploy. Automatic rollback on health-metric regression closes the loop without paging a human.

**Pattern.** One-box canary, then a single cell, then a region, then global; bake time between stages proportional to change risk; rollback triggers wired to the same metrics the on-call would look at, including real customer traffic, not just synthetic checks. Reviewer flag: a deploy pipeline with no bake time, no canary stage, or with rollback gated only on synthetic probes.

## 34. Asymmetric failure modes: know which way your system breaks

**Headline.** Some systems get better as load drops (queues drain, retries succeed). Some get worse (caches go cold, autoscalers scale in, leader leases expire). Know which one you are in before you shed load.

**Why.** The intervention is opposite. For a load-sensitive failure, shed traffic and the system recovers on its own. For an idle-sensitive failure (cold cache, scaled-in fleet, lost lease, stale connection pool), shedding traffic deepens the hole; you need to warm, pre-scale, or pin the resource first.

**Pattern.** For each critical service, write down which failure mode it has and what the first-response action is. Test recovery from a cold start under load, not only from a warm fleet at steady state. Reviewer flag: an incident runbook that always says "shed load" without distinguishing the failure mode, or a service that has never been started under load.

---

# Part III: SRE practice and Jepsen empiricism

Parts I and II cover system shape and operational tactics. Part III is the organizational and verification layer: how Google's Site Reliability Engineering (SRE) practice frames reliability as a budgeted resource, and what Kyle Kingsbury's Jepsen analyses have taught the industry about the gap between vendor claims and observed behavior under partition.

## 35. 100 percent is the wrong reliability target

**Headline.** Pick a reliability target below the achievable ceiling; the gap between that target and 100 percent is the budget you spend on velocity.

**Why.** Past a threshold, users cannot perceive added reliability, but engineering cost grows fast. Worse, dependents of a perfectly-reliable service start *assuming* it cannot fail, so when it eventually does, every dependent breaks at once (the Chubby precedent). The target is a negotiated business choice, not an engineering ideal.

**Pattern.** Set a Service Level Objective (SLO) below the observed ceiling. Compute the error budget as `1 - SLO`. Spend it on releases, experiments, and risk; when it is gone, freeze feature work until reliability recovers. The budget is not punishment, it is the explicit contract between product and reliability concerns. Reviewer flag: a stated target of "as reliable as possible," or an SLO chosen by adopting current performance without slack.

## 36. SLIs measure user-visible behavior; SLOs are the contract

**Headline.** Express Service Level Indicators (SLIs) as ratios of good events to total events at the user-visible boundary, not internal resource counters.

**Why.** Central Processing Unit (CPU) and queue depth tell you a machine is sad, not that a user is sad. SLIs at the user boundary are robust against implementation churn and align directly with what the error budget protects.

**Pattern.** Minimum-viable SLI shape: `good_events / total_events` at the request boundary, sliced by user-meaningful tiers if needed (premium vs free, interactive vs batch). Keep the SLO count small enough that exceeding any one of them changes behavior. Set internal SLOs tighter than externally-promised Service Level Agreements (SLAs) so chronic issues surface before customers see them. Reviewer flag: SLOs reported as averages instead of percentiles, or so many SLOs that no single one has consequences when missed.

## 37. Toil has a quantitative ceiling

**Headline.** Cap manual operational work at 50 percent per engineer; the other 50 percent must reduce future toil.

**Why.** Toil (manual, repetitive, automatable, tactical, no enduring value, scales linearly with the service) expands to fill all available time. Once it does, no improvements happen, on-call quality drops, and people leave. The 50 percent rule is the organizational tripwire.

**Pattern.** A runbook step executed by a human more than twice with the same outcome is the threshold to automate. "Requires judgment" is sometimes real and sometimes a place no one encoded the judgment; be honest about which. Overhead (meetings, hiring) is not toil; one-time grungy engineering with lasting value is not toil. Reviewer flag: a service whose on-call workload is rising linearly with usage, or a team consistently over the 50 percent ceiling with no plan to bring it back down.

## 38. Cascading failures: positive feedback breaks with load shedding

**Headline.** Overload concentrates load on survivors, which overloads them. Retries, reflexive automation, and lateral request forwarding all amplify the loop. Only load shedding breaks it.

**Why.** Overload is the most common trigger of large outages; cascading is what turns local overload into a full one. SRE's framing makes the dynamic explicit: every component that responds to overload by doing more work is a sustaining link in the feedback loop.

**Pattern.** Shed load before queueing it; reject early, reject at the client if possible, fail fast. Propagate deadlines down the call chain so backends drop work that is already too late. Tag request criticality (critical-plus, critical, sheddable-plus, sheddable); provision for the critical tiers, shed the sheddable ones first. Offer degraded responses (stale cache, fewer results, lower fidelity) before offering errors. Reviewer flag: a service that responds to overload by adding workers or retrying, with no admission control at the edge. (See also principles 22, 27, 28 on metastability, queues, and tail latency.)

## 39. Use consensus where you need it; avoid it where you do not

**Headline.** Leader election, distributed locks, group membership, and shared critical state require a proven consensus algorithm. Anywhere else, do not pay the cost.

**Why.** Ad-hoc coordination (heartbeats, gossip, "ping the other node") produces split-brain under partition. Jepsen's catalogue is largely a record of this mistake being repeated across vendors. But consensus is also expensive: a network round-trip per write, leader as bandwidth bottleneck, geographic distribution multiplies latency.

**Pattern.** Prefer consensus-as-a-service (ZooKeeper, etcd, Consul) over an embedded library. Use `2f + 1` replicas to survive `f` failures; five replicas is the practical default (survives two, lets you take one down for maintenance and still tolerate one unplanned failure). Do not run six: even-numbered quorums reduce availability without buying fault tolerance. Reviewer flag: a hand-rolled leader-election heartbeat, or a system that puts consensus in a high-throughput, low-latency hot path.

## 40. Backups are theater; restores are the product

**Headline.** No one wants backups; people want restores. The only proof a backup works is a restore that completes within the Service Level Objective.

**Why.** Data is lost via roughly 24 distinct combinations: {user, operator, application bug, infrastructure defect, hardware failure, site disaster} crossed with {wide, narrow} crossed with {big bang, creeping}. A defense addressing one cell is useless against the others. Replication is *not* recovery: replicas faithfully copy your corruption.

**Pattern.** Defense in depth, three layers: (a) soft delete with a 30-to-60 day grace period to defeat user error and account hijack, (b) tiered backups (local snapshots, incremental copies, offsite/archival) sized to restore-time requirements, (c) out-of-band validation pipelines that detect creeping corruption before it ages out of backups. Test restores continuously and automatically, not once a quarter. Reviewer flag: a backup system whose last successful end-to-end restore was a tabletop exercise, or a "high availability" story that conflates replication with recovery.

## 41. Distributed cron: prefer a missed launch over a double launch

**Headline.** Most cron-driven actions (payroll, billing, emails) are not idempotent, so duplicates are worse than misses.

**Why.** Distributing cron inherits leader election, partial-failure recovery, and "did the previous leader already start launch T before it crashed?" Without explicit handling, a failover produces either silent miss or silent duplicate.

**Pattern.** Run the scheduler under consensus, with synchronous confirmation before and after each launch so a new leader knows what the previous one did. Embed scheduled timestamps in launched job names so a recovering leader can ask "did launch T already happen?" Spread schedules with randomized hashing (the crontab `?` extension) instead of clustering at midnight; midnight is a thundering herd. If a job is genuinely idempotent (principle 17, principle 32), the at-least-once stance is safer. Reviewer flag: a non-idempotent job scheduled with at-least-once semantics, or a clustered scheduler with no consensus-backed leader.

## 42. Vendors lie about consistency; verify, do not trust

**Headline.** Documented guarantees diverge from observed behavior under partition, clock skew, and partial failure. Treat consistency claims as hypotheses to falsify.

**Why.** Jepsen's recurring findings across vendors are not exotic: replica divergence, lost updates under partition, stale reads claimed to be strongly consistent, snapshot isolation marketed as serializability, split-brain in systems advertised as partition-tolerant. The Knossos and Elle checkers find these by running real binaries under randomized network and clock faults, then checking observed histories against the claimed model.

**Pattern.** Where the algorithm requires linearizability (locks, compare-and-set, leader election), use a system specifically built for it and verify behavior under partition. Where eventual consistency is acceptable, prefer it: the operational margin is enormous. Randomized fault injection (network drops, partitions, clock skew, process pauses) and deterministic simulation testing (principle 14) are the only credible verifications; example-based testing cannot reach the relevant state space. Reviewer flag: marketing-grade consistency language ("strongly consistent" with no qualifier), single-node tests treated as evidence about cluster behavior, or a configuration that silently downgrades consistency for performance by default.

## 43. Know the consistency lattice well enough to pick a level

**Headline.** Stronger consistency is not universally better, it is universally more expensive. Use the weakest model that still makes the algorithm correct.

**Why.** Real systems mix levels. Distributed locks need linearizability. A "last write wins" social feed often does not even need causal consistency. The cost of getting this wrong is paid in latency and availability that you could have kept.

**Availability under partition.** Models at or stronger than sequential consistency (single-object) or snapshot isolation (transactions) cannot be totally available during a partition -- this is the Consistency, Availability, Partition tolerance (CAP) theorem made precise. Models at or stronger than read-your-writes are at most sticky-available (clients must pin to one replica). Weaker models can be totally available.

**Pattern.** State guarantees precisely (principle 8): "linearizable per key," "causally consistent across a session," "read-your-writes within a region." Mix coordination-needing operations into a small set of linearizable primitives (ZooKeeper-style) and run the bulk of the system on eventual consistency. Reviewer flag: a design that demands serializability everywhere "to be safe," or one that promises "strong consistency" without naming which axis (linearizability is single-object real-time order; serializability is multi-object equivalence to some serial order -- they are orthogonal, principle 11).

## 44. Monitor on symptoms, alert on what users feel

**Headline.** Four golden signals for user-facing systems: latency, traffic, errors, saturation. Pages must be urgent, actionable, and tied to user-visible failure; everything else is a ticket or a dashboard.

**Why.** One layer's symptom is another layer's cause; the cause set is unbounded but the symptom set is finite. Alerting on causes guarantees alert fatigue, which is itself a reliability bug. (Deeper observability is a separate skill set; this is the load-bearing minimum.)

**Pattern.** Distinguish pages (urgent, user-visible right now), tickets (non-urgent, needs eventual attention), and dashboards (trending and subcritical signals). Pages must be rare enough that every one is treated as urgent. Alert on SLO burn rate, not on every cause metric. Reviewer flag: a page on CPU above 80 percent, an SLO measured in averages, or a dashboard with no high-percentile latency panel (principle 28).

## Meta-rules

- Partition tolerance is not optional. Networks partition. Plan for it.
- Every retry needs exponential backoff, jitter, and a budget (principles 18, 19, 20).
- Every deadline must propagate down the call chain.
- Every automated remediation is also a way to amplify an outage. Gate it on a circuit breaker scope and a human escape hatch.
- The thing you did not test under load will surprise you under load.
- The thing you did not restore from backup is not backed up.
