---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# System Design Patterns and Decisions

A pattern catalog and decision reference for system design. Use this alongside `distributed-systems.md` (which contains the *principles*). This file contains the *moves* -- what each pattern does, when to reach for it, what reviewers flag. Distilled from *Designing Data-Intensive Applications* 2nd ed. (Kleppmann + Riccomini, 2026) plus the established microservices/architecture canon.

## Foundations

### Operational vs Analytical Systems (OLTP vs OLAP)
OLTP serves user-facing transactions: small, indexed point lookups; latency-sensitive; freshness-critical. OLAP serves business analysts: aggregate scans over historical data; throughput-sensitive; freshness can lag. Star/snowflake schemas, columnar storage, ETL/ELT into a warehouse / lake are OLAP answers; row-oriented B-trees and short transactions are OLTP answers.

**HTAP (hybrid)**: claims to serve both from one engine (SingleStore, TiDB, AlloyDB). Treat HTAP marketing skeptically. Ask which workload is second-class and whether the row/columnar split is per-table, per-replica, or via dual storage.

**Flag**: running analytical queries against the OLTP database; running OLTP-shape queries against a warehouse. Both are anti-patterns.

### Systems of Record vs Derived Data
A **system of record** holds the authoritative, normalized, source-of-truth representation. **Derived data** is anything reconstructable from a system of record: caches, search indexes, denormalized views, materialized aggregates, ML feature stores, ML models themselves.

Label every dataset as one or the other. Bidirectional sync between two "systems of record" is almost always a bug. Update propagation from system-of-record to derived stores is the integration pattern (CDC, event logs, batch refresh).

**Flag**: two systems both claiming authority over the same fact; "we sync these two databases" without a clear unidirectional flow.

### Cloud-Native: Disaggregated Storage and Compute
The defining cloud-native pattern: storage and compute scale independently. S3/Blob/R2 hold durable bytes; ephemeral compute attaches and detaches against shared storage (Snowflake, BigQuery, Aurora, Neon). Consequences: local disks become caches; instances are cattle; multi-tenancy is the norm.

Watch for "cloud databases" that emulate block devices over the network -- every I/O is a network call, with tail latency to match. Genuine cloud-native systems talk to object-store APIs directly with batching.

### Distributed vs Single-Node
Default to single-node until a concrete bottleneck forces distribution. Single machines are bigger and faster than ever -- DuckDB, SQLite, single-node Postgres handle workloads that needed a cluster a decade ago. Reasons to go distributed (in priority order): durability, fault tolerance, geographic latency, scale beyond a single machine, regulatory residency, specialized hardware mix. Pre-distributing for "future scale" usually underperforms.

### Microservices: an Organizational Choice
Microservices are primarily an *organizational* technology. Each service owns its data; APIs are the only contract. **Pay the complexity tax for team autonomy, not for technical reasons.** For a small org with few teams, microservices add overhead (distributed-system complexity, schema-evolution discipline, deployment matrix, distributed tracing) without the payoff. Shared databases between services defeat the purpose -- the schema becomes the API.

**Flag**: "we need microservices to scale" without naming the team-coordination bottleneck. Decomposition by technical layer ("the auth service," "the validation service") rather than business capability. Distributed transactions across service boundaries -- if you need ACID, you have one service.

### Two-Pizza Teams and Conway's Law
System structure mirrors team structure. Service boundaries should be where the *language* changes (an "Order" in fulfillment means something different than in billing). Two-pizza teams (~6-10 engineers) is the autonomy unit. If three teams own one service, you have three services pretending to be one.

### Choose Boring Technology
Three innovation tokens per team. Spend them on what differentiates the business, not the database + queue + runtime simultaneously. "Boring" means *failure modes are well-understood*, not "bad." MongoDB was once new; Postgres is now boring and excellent. Reviewer's question: who on this team has paged on this tech at 3am?

### The 12-Factor App (condensed)
1. One codebase, many deploys
2. Dependencies declared and isolated
3. Config in env (secrets out of artifact)
4. Backing services as URLs
5. Build / release / run separated
6. Stateless processes
7. Port binding (app exports itself)
8. Concurrency via processes
9. Disposable processes (fast boot, graceful SIGTERM)
10. Dev/prod parity in time, personnel, tools
11. Logs as event streams to stdout
12. Admin tasks as one-off processes in the same release

**Flag**: secrets baked into Docker images; "the staging fork of the prod repo."

## Capacity and Latency

### Napkin Numbers to Memorize
- L1 cache: ~1 ns; main memory: ~100 ns; SSD random read: ~100 µs
- Same-DC round trip: ~500 µs; cross-region: ~50-150 ms
- 1 Gbps NIC: ~125 MB/s; modern server: 10-25 Gbps
- Single Postgres: 10-50k simple QPS; Redis: 100k+ QPS per shard
- One year of seconds: ~31.5M; daily QPS = events / 86,400
- Typical OLTP row: 100-500 bytes
- Consumer apps often have 100:1 read:write or worse -- size the read replicas and cache, not the primary

**Flag**: estimating from average. Peak is typically 2-10x average; design for peak.

### Percentiles, not Averages
Use p50/p95/p99/p999. Tail percentiles correspond to users with the most data, most history, most value -- exactly whom you can't afford to lose.

- **Never average percentiles across machines or windows.** Aggregate the underlying histograms (HdrHistogram, t-digest, DDSketch) and recompute the percentile from the merged histogram.
- **Tail-latency amplification under fan-out**: when one request issues N parallel backend calls, the user sees the *max*. At N=100 with backend p99 of 1%, user hits a slow path ~63% of the time. Aggregate p99 demands per-backend p999 or better.
- **Diminishing returns past p99.99** -- driven by GC, packet loss, fail-slow hardware, outside your control.

### Little's Law and Utilization
`L = λ × W` (concurrency = arrival rate × time-in-system). Pick any two, the third follows. The corollary: as utilization (`ρ`) approaches 1, queue length goes as `ρ / (1 - ρ)`. At 80% utilization the system has 4 in the queue; at 95% it has 19. The cliff is real. Don't run hot.

## Data Modeling

### Relational vs Document
Document wins on locality and one-shot tree-shaped reads (résumé, order with line items, JSON the UI renders directly). Relational wins on many-to-many relationships and consistent updates of shared entities. Schema-on-read is dynamic typing of data; schema-on-write is static typing. Neither is universally better; heterogeneous data favors schema-on-read, homogeneous long-lived data favors schema-on-write.

**Flag**: hand-rolled application-side joins replacing a missing `$lookup`; denormalized fields with no documented refresh story; "schemaless" code that secretly assumes a fixed schema.

### Graph Models
Reach for graph when (a) connections are heterogeneous, (b) traversal depth is variable, or (c) the schema needs to evolve to add new edge types without migration. SQL recursive CTEs work for shallow recursion; Cypher/Datalog/SPARQL beat them once traversal logic grows. GraphQL is not a graph database -- it's a deliberately restricted client-driven query language; watch for N+1 resolvers and unbounded recursion from untrusted clients.

### Event Sourcing and CQRS
**Event sourcing**: store every state change as an immutable, past-tense event in an append-only log; derive read-optimized projections. Captures *intent*, not just resulting mutation. Views are reproducible.

**CQRS** (Command Query Responsibility Segregation): separate write model (commands → events) from read model (projections). Related but not identical to event sourcing -- you can do CQRS without ES, though they pair naturally.

**Reach for** when audit history is the product, when read and write shapes diverge sharply, or when read QPS dwarfs write QPS. **Skip** for CRUD apps.

**Pitfalls**:
- Events must be past-tense (`seat_was_booked`), not commands (`book_seat`)
- Non-determinism in projections (`now()`, RNG, external lookup) breaks rebuilds
- GDPR conflicts with immutability; mitigate with crypto-shredding (per-user encryption keys you destroy)
- External side effects on replay (resending emails) need an idempotency layer or replay-mode guard
- Snapshotting is required if you can't tolerate full replay
- Multi-year deployments accumulate event versions; have a versioning strategy from the start

### Storage Engines: B-tree vs LSM-tree
B-tree updates in place on fixed-size pages; LSM-tree appends to a write-ahead log, buffers in memtable, flushes to immutable sorted SSTables, compacts in background.

| | B-tree | LSM-tree |
|---|---|---|
| Write amp | Page+WAL+full-page writes | Sequential, batched; compaction rewrites |
| Read amp | One page per level | May consult several SSTables; Bloom filters help |
| Space amp | Fragments (vacuum needed) | Tombstones linger until compaction |
| Range scans | Cheap | Merge across SSTables |
| Latency | Predictable | Compaction spikes |

LSM wins on write-heavy workloads; B-tree wins on read-latency predictability. **Flag**: benchmarks that don't run long enough to expose compaction's steady-state write amp; tombstone retention concerns for compliance.

### Column-Oriented Storage
Store column-by-column for analytics queries that touch few of many columns. Wins from: read only required columns, extreme compression (RLE, dictionary, bitmap), vectorized execution on compressed data, bitmap operations on filter predicates. **Sort order matters** -- compression effect is strongest on the leading column. Don't confuse with wide-column / column-family stores (Bigtable, HBase: row-oriented with sparse rows).

### Vector Embeddings
Embeddings map text/images/audio into high-dimensional points where similarity = closeness. Indexing approaches:
- **Flat**: scan every vector. Accurate, slow. Fine for small corpora.
- **IVF (inverted file)**: cluster into centroids; query nearest probes. Approximate.
- **HNSW**: hierarchical proximity graph. Generally fastest at scale, higher memory.

All approximate -- trade recall for latency. Embeddings are fragile under model version changes; reindexing the corpus is the norm, not the exception. Query embeddings must use the same model version as the index. **Flag**: treating HNSW as exact; no plan for embedding-model rotation.

## Encoding and Evolution

### Schema Evolution: Forward/Backward Compatibility
During any rolling upgrade -- or any system where data outlives code -- old and new code coexist. Encodings must provide:
- **Backward compatibility**: new code reads old data
- **Forward compatibility**: old code reads new data

For APIs: backward on request, forward on response (older client → newer service); vice versa for newer client → older service. The "unknown fields" problem: an older reader that deserializes into a typed model missing the new field, then writes back, drops the new field's data unless the framework preserves unknown fields.

### Protobuf vs Avro
**Protobuf**: each field gets a numeric tag. Encoded record is a sequence of (tag, type, value). Unknown tags skipped (forward compat). Missing fields take defaults (backward compat). Rename a field freely (tag is what matters); never reuse a deprecated tag. Datatype widening (32→64 bit) can silently truncate.

**Avro**: no tag numbers; binary stream is concatenated values, decoded by walking the *writer's schema*. Reader has its own schema; resolves differences by field name. Compatibility rule: only add or remove fields *with default values*. Natural choice for dynamically-generated schemas (DB column adds/removes).

Where each shines: Protobuf for gRPC, statically-generated code, slightly less subtle. Avro for bulk data files, Kafka with a schema registry, schemas derived from a moving source.

**Flag**: anyone treating Protobuf or Avro as schema-free at runtime; missing CI checks against a schema registry; JSON-only APIs that "version" by adding fields with no compatibility tests.

### Durable Execution and Workflows
A workflow engine (Temporal, Restate, Cadence) runs your code so crashes/restarts don't lose progress: every external call and state change is logged; on replay, completed steps return their logged results and execution resumes from the failure point.

Gives **exactly-once semantics** (not exactly-once delivery) for workflows by combining replay-based determinism with idempotent external APIs.

**Replay-based determinism is the load-bearing constraint**:
- No `now()`, RNG, system clocks, network calls outside the framework's `execute_activity`. Frameworks provide deterministic shims.
- Reordering or modifying existing workflow code is dangerous; deploy new versions side-by-side and route new invocations to the new version.
- External services must still expose idempotency keys; the framework can't make a third-party gateway exactly-once on its own.

**Flag**: `time.now()` or random inside a workflow body; in-place edits to a workflow with active executions; assuming durable execution gives transactional rollback (it doesn't -- it gives replay).

### Event-Driven Architecture
Producers publish to a broker (Kafka, RabbitMQ, Pub/Sub); consumers subscribe. Two patterns: queues (one consumer wins each message) and topics (all subscribers receive).

**Failure modes**:
- **Poison messages**: a malformed message that crashes the consumer on every redelivery. Without DLQ + max-retry, the whole stream stalls.
- **Ordering**: most brokers guarantee ordering only within a partition / queue, not globally.
- **Exactly-once myths**: brokers offer at-least-once in the general case. "Exactly-once" features refer to specific configurations (Kafka transactions within Kafka, idempotent producers). Cross-service: still the consumer's job to be idempotent.
- **Unknown-field preservation**: republishing decoded messages drops unknown fields unless the codec preserves them.

**Flag**: missing DLQs; no idempotency keys on consumers; ordering assumptions that don't survive partition scaling; "exactly-once" claims unaccompanied by the configuration that justifies them.

## Replication

### Single-Leader Replication
One node is source of truth; followers tail its change log. Simple mental model, easiest debugging, easy to add read capacity. Pick this unless cross-region writes or offline operation force otherwise.

Sync vs async: fully sync is fragile (any slow follower halts writes), fully async risks ack'ing a write that's lost on failover. Realistic shape is "semi-synchronous": one sync follower, rest async.

Replication log implementations:
- **Statement-based**: breaks on `NOW()`, `RAND()`, autoincrement, triggers. Avoid.
- **WAL-shipping** (Postgres, Oracle): tightly coupled to storage format; no version-mismatch upgrades.
- **Logical/row-based** (MySQL binlog, Postgres logical): decoupled, allows version mismatches, feeds CDC.
- **Trigger-based**: flexible but slow and bug-prone.

**Flag**: a "logical" replication setup that silently falls back to statement-based for nondeterministic statements; a system claiming "synchronous replication" while ack'ing before any follower applies the write.

### Replication Lag is Application-Visible
Asynchronous followers fall behind (ms to minutes). Three named anomalies, each with its own guarantee:

- **Read-your-writes**: user submits a write, then reads stale and "loses" their data. Fix: route reads of "things this user might have just modified" to the leader or a sync follower; track per-user last-write timestamp (logical preferred).
- **Monotonic reads**: refresh shows older state. Fix: pin each user's reads to one replica (hash of user ID).
- **Consistent-prefix reads**: causally related writes observed out of order. Fix: keep causally related writes on the same shard, or track causal dependencies.

**Flag**: any read path that says "just read from a replica" without specifying which one or how fresh.

### Multi-Leader Replication
Multiple leaders accept writes; replicate asynchronously to each other. Worth the complexity only for cross-region latency, regional fault tolerance, or offline-capable clients. **Flag**: multi-leader with autoincrement IDs, application-level uniqueness checks, or "balance can't go negative" -- these constraints are incompatible with multi-leader.

### Sync Engines and Local-First (new DDIA 2nd ed)
Each user's device is a leader; sync is async multi-leader at the extreme. Full local replica, UI reads/writes locally (16ms target), background sync reconciles. **Local-first** = works even if the original vendor shuts down (open protocol, swappable backends). Constraint: working set must fit on the device.

### Conflict Resolution: LWW is Dangerous
**Last-write-wins silently loses data**. Two concurrent writes look identical to two sequential ones, so one is dropped. Worse, "highest timestamp" depends on clock sync. A fast-clock node permanently wins over a slower correct node -- no error signal. Use LWW only when (a) you never update existing values, or (b) the app genuinely doesn't care which concurrent write wins.

Alternatives:
- **Conflict avoidance**: route writes for a record to one home leader.
- **Manual resolution (siblings)**: store all concurrent values, surface to app on next read.
- **CRDTs / OT**: automatic merging via Conflict-free Replicated Data Types or Operational Transformation.

CRDTs apply to: counters, grow-only or two-phase sets, key-value maps (resolve per-key), ordered sequences/text. They **cannot enforce global invariants** ("at most 5 items") -- if the constraint matters, drop items or use single-leader.

**Flag**: "merge by union" for a delete-capable collection -- the Amazon-cart bug (deleted items resurrect). Tombstones, not set union.

### Leaderless (Dynamo-Style)
Clients write to several replicas in parallel; reads probe several too. Examples: original Dynamo, Cassandra, ScyllaDB, Riak. **Modern DynamoDB is not this** -- it's single-leader on Multi-Paxos. Easy to confuse.

Quorum math: W+R>N. In practice it breaks via:
- **Sloppy quorum**: writes accepted by any reachable nodes when usual replicas unreachable; subsequent reads on usual replicas miss them.
- **Slow nodes counted as success**: client waits for fastest W; slow correct nodes' values aren't counted.
- **Real-time clock LWW** for conflict resolution inherits all LWW pathologies.

**Flag**: leaderless systems used for workloads requiring strong consistency (locks, uniqueness, ordered counters); monitoring that doesn't track staleness explicitly.

## Sharding

### Key-Range vs Hash Sharding
Key-range: each shard owns a contiguous key range. Range scans cheap; hot spots for free on sequential access. Hash: even load distribution; range scans scattered. **Hash modulo N is wrong** (mod N rebalancing moves nearly all keys). Use fixed large number of shards, hash-range with random boundaries per node, or consistent hashing (rendezvous, jump consistent hash).

Compound keys: partition by first column, sort within shard by the rest -- standard pattern for "by user, ordered by time."

### Hot Spots: Three Canonical Patterns
- **Celebrity user / viral post**: one key gets disproportionate traffic. Mitigation: pull hot key into its own shard; salt writes (prepend 2 random digits to spread one logical key across 100 physical). Reads then scatter-gather across the splits.
- **Sequential / monotonic keys**: autoincrement IDs, timestamps. Every new write hits the highest shard. Mitigation: prefix with non-sequential element; use hash-range.
- **Hash sharding alone doesn't fix skew**: distributes keys uniformly, not load. A hot key is hot regardless of hash.

**Flag**: "we'll just hash the user ID" with no plan for celebrities or spiking events.

### Rebalancing: An Operations Question
The mechanism is easy; the hard parts are:
- Don't trigger rebalancing automatically from transient slowness (slow node "looks dead" → cluster drains it → load lands on strained peers → cascading failure)
- Keep serving reads/writes during the move (old assignment for in-flight; cutover overlap)
- Cost is proportional to dataset size
- Human-in-the-loop rebalancing is slower but prevents surprises (Couchbase, Riak generate plan automatically, require admin commit)

### Secondary Indexes: the Fundamental Tradeoff
**Local (document-partitioned)**: each shard indexes its own records. Write cheap; read by secondary index scatter-gathers across all shards. Tail-latency amplification on the read path. (MongoDB, Riak, Cassandra, Elasticsearch.)

**Global (term-partitioned)**: the index itself is sharded, partitioned by indexed term. Read cheap; write expensive (a single record touches multiple index shards). Atomic consistency requires distributed transactions; otherwise async and stale. (CockroachDB, TiDB, YugabyteDB; DynamoDB supports both.)

Pick the side whose cost you can afford to pay every time.

## Transactions and Isolation

### Weak Isolation: What Each Level Actually Prevents
- **Read committed**: prevents dirty reads/writes. Postgres/Oracle/SQL Server default. Allows non-repeatable reads, lost updates, write skew, phantoms.
- **Snapshot isolation / repeatable read**: reads from consistent snapshot at transaction start; MVCC implementation. Readers don't block writers, writers don't block readers. **First-committer-wins** rule on concurrent writes to the same object. PostgreSQL's "repeatable read" is snapshot isolation; MySQL's "repeatable read" is weaker; Oracle's "serializable" is snapshot. Names lie across vendors.
- **Serializable**: three implementations -- actual serial execution (VoltDB, Redis, Datomic), two-phase locking (Postgres before 9.1, MySQL), serializable snapshot isolation / SSI (Postgres, CockroachDB, FoundationDB).

### Lost Updates
The classic read-modify-write race. Prevention, in order of preference:
1. Atomic operations (`UPDATE counters SET value = value + 1`)
2. Explicit locks (`SELECT ... FOR UPDATE`)
3. Automatic detection (Postgres repeatable read, Oracle serializable, SQL Server snapshot do this; **MySQL InnoDB repeatable read does NOT**)
4. Compare-and-set (`UPDATE ... WHERE value = old_value`)
5. App-layer conflict resolution (CRDTs, siblings)

**Flag**: any read-modify-write outside a transaction or atomic op -- counters, JSON document edits, wiki overwrites.

### Write Skew and Phantoms
Two transactions read an overlapping set, each makes a decision, each writes to *different* objects. No row collides; first-committer-wins doesn't fire. Classic examples: two on-call doctors both go off-call leaving zero, two users book the same room, two users register the same username, two users both spend from a balance.

Defenses:
- Unique constraint (if conflict is on a single column)
- Materializing the conflict (pre-create lockable rows so `SELECT FOR UPDATE` has something to grab -- ugly, last resort)
- Serializable isolation -- the right answer

**Flag**: any "check then act" pattern (count, exists, sum, range search) followed by an insert/update that would change the check's result.

### Two-Phase Commit and Its Discontents
2PC: coordinator asks each participant to prepare (durably write, promise no abort); after all yes, broadcasts commit. **In-doubt transactions** (coordinator crashes between prepare and commit) hold locks indefinitely. Coordinator log loss requires manual intervention. XA (cross-vendor 2PC) is lowest-common-denominator and cannot integrate with SSI or do cross-system deadlock detection.

**Database-internal distributed transactions** (Spanner, CockroachDB, TiDB, FoundationDB) work better because they control both ends. Cross-system 2PC is mostly to be avoided.

### Exactly-Once Messaging
**Exactly-once is at-least-once + idempotent processing.** Pattern: receiving worker starts a DB transaction, checks message ID in a processed-messages table; if exists, ack and drop. Otherwise insert the ID and do the writes in the same transaction; commit; then ack the broker. Unique constraint on the message-ID table makes concurrent retries safe. **You do not need distributed transactions for exactly-once -- you need local transactions plus idempotency keys.**

## Failure-Tolerant Patterns

### Idempotency Keys (Stripe model)
Client provides a unique key per logical operation; server stores `(key, result)` and replays the stored result on retry. Stripe's model: client-generated key, scoped per API key, 24-hour retention, stored result includes status code and body. Reach for it on every non-GET mutation that handles money, sends notifications, or kicks off external work.

**Flag**: storing only "I saw this key" without the response. Retried request runs validation differently and double-charges. The key must guard the *result*, not the *receipt*.

### Saga Pattern
Sequence of local transactions plus *compensating actions*, replacing distributed ACID across services. Two variants:
- **Choreography**: each service listens for events, decides next step. Simpler at small scale but the flow lives nowhere; can't read the order.
- **Orchestration**: central coordinator drives the steps. Easier to reason about, test, and observe. Costs you a coordinator service.

Reach for sagas over 2PC whenever services own their own data. Skip when one transactional database holds the whole flow.

**Pitfall**: compensations are not "undo." `cancelShipment()` after the package is on a truck is not free, may fail, may be visible to the customer. Design the business semantics, not the technical rollback.

### Outbox Pattern
Update DB AND publish event atomically without 2PC: write the event to an `outbox` table in the same DB transaction as the business change; a separate publisher reads the outbox and ships to the broker. Alternative: CDC tailing the DB's write-ahead log (Debezium). At-least-once delivery becomes the contract; downstream consumers MUST be idempotent.

**Flag**: "publish then commit" or "commit then publish" -- both lose messages.

### Circuit Breakers vs Load Shedding
Circuit breakers stop calling a dependency you've established is down. The half-open state causes thundering herds. Modern practice often prefers **continuous load shedding** (drop new work when latency rises, regardless of "open/closed" state) over breakers, plus **hedging** (fire a backup request if the first is slow).

**Flag**: a circuit breaker per host when retries route around it; no alert on breaker open; multiplicative retries at every mesh hop amplifying a hiccup into an outage.

### Bulkhead Pattern
Partition resources (thread pools, connections, queues) so one bad tenant cannot drain shared capacity. Reach for it in multi-tenant systems. Skip for single-tenant internal services.

**Flag**: statically-sized bulkheads. Set so the sum exceeds capacity (over-subscription fine if average usage low) but no single pool takes more than its share.

### Rate Limiting
Three algorithms:
- **Token bucket**: allows bursts up to bucket size, refills at steady rate. Default choice.
- **Leaky bucket**: smooth output, queue input. Good for downstream protection.
- **Sliding window**: more accurate than fixed windows, no second-boundary cliff.

Distributed rate limiting almost always lands on Redis with `INCR` + TTL or a Lua script. Limit at multiple layers (gateway, service) -- gateway alone won't stop a retry storm from one internal service.

## API Surface

### REST vs RPC vs Events
- **REST**: HTTP, resource-oriented, content negotiation, generally JSON. Widely interoperable. Use OpenAPI/Swagger to define schemas.
- **RPC (gRPC, Protobuf)**: looks like a local call. **Location transparency is a flawed abstraction** -- network calls can be lost, slow, retried (idempotency required), partially completed. Never let RPC client code pretend a remote call is a local one.
- **Message passing (events)**: async, decoupled, broker-buffered. Producer doesn't wait for or know recipients.

### API Gateways and BFFs
Gateway centralizes auth, rate limiting, request shaping, observability. BFF (Backend For Frontend) is a gateway *per client type* (iOS, web, Android) doing client-specific composition. Reach for gateway as soon as >3 public services. Reach for BFFs when clients diverge sharply (mobile bandwidth, different feature surfaces).

**Flag**: gateway with business logic in it. Keep gateways thin: auth, routing, shaping, limits.

### Service Mesh
mTLS, retries, timeouts, traffic shifting, L7 observability without app changes. Does NOT give you good defaults -- those are still yours. Reach for a mesh when you have dozens of services and inconsistent libraries across languages. Skip for ~5 services in one language; the operational cost (control plane, sidecar resources, debugging) outweighs the win.

**Flag**: mesh retries enabled at every hop -- multiplicative amplification.

### Caching Strategies
- **Cache-aside (lazy load)**: app checks cache, falls back to DB, writes to cache. Default. Risk: thundering herd on miss; mitigate with single-flight (request coalescing).
- **Read-through**: cache library handles DB fallback. Cleaner code, less control.
- **Write-through**: write to cache and DB synchronously. Strong consistency, write latency penalty.
- **Write-back**: write to cache, async flush to DB. Fast writes, data loss risk.

Cache invalidation: TTL is the last resort; event-driven invalidation is better. Add **TTL jitter** to prevent synchronized expiration causing a stampede. Always have a plan for cache failover (cold cache = downstream gets full traffic).

**Flag**: no plan for cache invalidation when underlying row changes via a different code path.

### Pagination
**Offset pagination** breaks at scale (`LIMIT 20 OFFSET 10000` scans+discards 10k rows; shifts under inserts → duplicates or missing items). **Cursor / keyset pagination** (`WHERE (created_at, id) < (?, ?) ORDER BY ... LIMIT 20`) is stable and has constant cost. Use cursors for any list that grows or is time-sorted.

**Flag**: exposing cursor as raw primary key. Wrap in an opaque token so you can change sort order without breaking clients.

### API Versioning
URI versioning (`/v1/...`) is obvious and ugly -- that's why it works. Header versioning and content negotiation are cleaner but harder to debug from curl. **Design for forward compatibility from the start** (unknown fields ignored, enums extensible) so you rarely need a v2.

**Flag**: tightening validation, repurposing a field, removing a field without version bump.

## Batch and Stream Processing

### Batch Processing
A batch job is a stream job over a finite window. Two storage models: distributed filesystems (HDFS, replicates blocks, schedules computation near data) vs object stores (S3, separates storage/compute at the cost of bandwidth). Object stores generally lack hard links, file locking, atomic renames.

Fault tolerance: retries are cheap because outputs are derived data. MapReduce materializes intermediate state to DFS; Spark keeps in memory and tracks lineage (RDDs); Flink uses periodic checkpoints.

**Serving derived data**: do NOT write directly from many parallel batch tasks to a production database. Push to Kafka or build a new database inside the batch job and bulk-import (TiDB Lightning, RocksDB SST imports). Atomic dataset swaps decouple write traffic from read traffic.

### Stream Processing
**Change Data Capture (CDC)**: tap the database's replication log and republish changes as a stream. Makes one database the leader and turns all derived stores (search index, cache, warehouse) into followers. Solves the dual-write race condition.

**State, Streams, Immutability**: mutable state and an append-only log of immutable events are two sides of the same coin. The database is a cached projection of the log.

**Event time vs processing time**: event time is correct but hard (events delayed by network, offline mobile, restarts). Processing time produces nonsense the moment there's any lag. Use watermarks to declare "no further events with timestamps below t will arrive"; late events either drop or get retraction-and-replace.

**Window types**: tumbling (fixed length, disjoint), hopping (overlapping), sliding (events within distance t), session (gap-defined).

**Stream joins**:
- Stream-stream (window join): match events from two streams within a time window
- Stream-table (enrichment): per-event lookup against a slowly-changing reference table, kept fresh via CDC
- Table-table: both inputs are changelogs; output is the changelog of the join

**Fault tolerance**: microbatching (Spark Streaming) or checkpointing (Flink). Exactly-once via idempotent writes (include source offset in written value) or transactional output at the sink.

### The End-to-End Argument
Low-level guarantees (TCP checksums, DB transactions) are not sufficient for application correctness. **Correctness must be enforced at the application level by an end-to-end mechanism**. Canonical implementation: client-generated request ID included in every retry of the same logical request; deduplication happens at the level where the duplication originated. Idempotent writes built on these end-to-end identifiers replace expensive distributed transactions.

### Timeliness vs Integrity
Two things often lumped as "consistency":
- **Timeliness**: users observe up-to-date state. Violations are temporary; wait and retry fixes them.
- **Integrity**: no data loss, contradictions, or corruption. Violations are permanent; waiting doesn't fix them.

Integrity is non-negotiable. Timeliness is a tunable tradeoff. Many real applications run optimistically and apologize for timeliness violations (overbook flights, oversell stock, apply compensating credits).

## Reviewer's recurring failure modes

Three failure modes show up over and over in system designs:

1. **Pattern named, failure mode unowned.** "We'll use circuit breakers." Who alerts when they open? What's the half-open strategy? Who tunes the threshold?
2. **Pattern chosen for technical, not organizational, reasons.** "Microservices for scale" -- almost always wrong; team autonomy is the real reason.
3. **At-least-once without idempotency.** Outbox, retries, queues, sagas all imply duplicate delivery. Every consumer needs a dedup key and you should be able to name it.

When in doubt: start with a modular monolith, a managed Postgres, a single cache, boring deployment. Add a pattern only when you can name the specific problem it solves for your team this quarter.
