---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Client Sync and Local-First Architecture

A reference for evaluating client-side sync: offline-capable applications, local databases, conflict resolution as the user experiences it, and the protocol between a client and its server. Used by the `sync-and-offline` subagent.

The scope is **the client and the client-to-server sync protocol**. The sibling `distsys-data` agent owns server-side replication topology, sharding, isolation levels, consensus, and storage-engine choice.

Distinct from:
- **`distsys-data`**: server replication, sharding, isolation, consensus. Where a server's own replication lag surfaces as a client-visible non-monotonic read, diagnose here and fix there.
- **`distsys-runtime`**: retries, queues, idempotency, and caching at the service layer. We own the client's outbound queue and the sync wire protocol.
- **`mobile-native`** / **`desktop-native`**: platform background-execution APIs. We own what the sync design must assume about them.
- **`security`**: the threat model. We own end-to-end encryption only where it constrains merge architecture.
- **`app-privacy-compliance`**: erasure obligations. We own why a tombstone is not an erasure.

The core thesis: **a client that buffers any local write has already accepted eventual consistency.** The only real decision is whether the resulting divergence is managed (explicit merge semantics, a convergence property you can test) or accidental (whatever the retry loop happens to do).

The operational priority: **identify what carries the ordering metadata.** A retry queue transports requests. Sync transports state or operations plus enough metadata to order and merge them. The metadata (version vectors, hybrid logical clocks, operation IDs, causal parents) is the entire difference, and its absence is the diagnostic.

---

## Universal principles

### Partition is the normal operating condition

Server-side CAP arguments treat partition as exceptional. On a client it is routine: airplane mode, subway, captive portal, backgrounded app, dead radio, suspended process. A mobile client is partitioned some fraction of every day.

The consequence is not subtle. **You do not get to choose CP on the client.** Choosing consistency means the interface blocks or errors whenever the network degrades, which is the spinner failure mode. Every offline-capable client is an AP system whether the team decided that or not.

**Flag**: code that assumes read-your-writes across devices, global ordering, or uniqueness constraints enforced client-side.

### A retry queue is not a sync engine

The most common architecture in the wild is a persisted queue of HTTP requests replayed on reconnect. It fails structurally, not because the retry code is weak:

- **Non-idempotency.** A retried `POST` after a lost response creates two rows.
- **Offline-generated identity.** The client must create an ID before the server sees the row. Server-assigned autoincrement is unusable. Temp IDs require rewriting every foreign key in the queued batch, a remapping pass most implementations never write.
- **Dependent writes and ordering.** Create-parent then create-child. If the parent fails and the child succeeds, you get an orphan. Strict serial ordering fixes it and introduces head-of-line blocking.
- **Partial application.** A batch half-applies. No client-side transaction spans the network, so local and remote disagree in a way neither detects.
- **No convergence definition.** Two devices replaying different queues produce a result that depends on arrival order. There is no property to state or test.
- **Conflict is invisible.** The request returns 200 having silently clobbered a concurrent edit.
- **Nothing on the read path.** A queue handles writes, so teams bolt on polling and now have two unsynchronized mechanisms.

**Flag**: a persisted request queue described as sync, with no version vector, no operation IDs, and no causal metadata.

### Convergence is not correctness

A conflict-free replicated data type guarantees all replicas reach the same state. It does not guarantee that state is meaningful.

For text, the interleaving anomaly is the proof: two users concurrently insert `eggs` and `bread` at one position, and a flawed merge yields `ebgrgesad`. Both replicas converge, on garbage.

For business data it is worse. Two users decrement inventory from 1 and both replicas converge to -1. Two users book the same slot and both bookings survive. **No general-purpose merge function enforces a domain invariant it does not know about.**

**Flag**: a merge strategy chosen for convergence alone, applied to data carrying cross-field or cross-entity invariants.

### Deletion is the hardest operation

A create is self-describing. A delete is the absence of something, and absence does not replicate. A replica that never saw the row cannot distinguish deleted from never-existed; a replica holding an old copy cannot distinguish deleted-remotely from not-yet-created-remotely.

**Flag**: physical row removal with no tombstone, no delete operation, and no `deleted` flag carrying a version.

### Identity is created on the device

IDs must be creatable offline without coordination. Use UUIDv4, or UUIDv7 / ULID when lexicographic time-ordering helps index locality.

**Flag**: server-assigned autoincrement identifiers for entities a client can create offline.

---

## Terminology (these get conflated constantly)

- **Optimistic UI** -- apply to view state immediately, fire the request, roll back on failure. No durable local store. Reload mid-flight and the change is gone. A rendering technique, not sync.
- **Offline-capable** -- tolerates connectivity loss, typically read-only from a cache, may queue writes. The server is authoritative; the local store is a cache.
- **Offline-first** -- writes work offline and reconcile later; local storage is durable. Reconciliation semantics are usually ad hoc.
- **Local-first** -- the local copy is primary, not a cache. The server is a peer, relay, or backup.
- **Sync engine** -- the reusable layer maintaining convergence between a local store and a remote one. Deliberately weaker than local-first: most sync engines are offline-capable, because the server holds authority and the data is not portable past the vendor.

**The reviewer's test**: if the vendor disappears tomorrow, does the user still have working software and readable data? Yes means local-first. Data readable but application dead means offline-first. Nothing means offline-capable at best.

The seven local-first ideals (Ink & Switch, 2019: Kleppmann, Wiggins, van Hardenberg, McGranaghan) are a scorecard, not a binary: no spinners, not trapped on one device, network optional, seamless collaboration, the Long Now, security and privacy by default, ultimate ownership and control. Kleppmann later reframed them as a gradient rather than a checklist.

The essay's empirical findings matter more than its ideals. **Users rarely encounter conflicts in practice**; datatype-aware merge beat line-oriented version control; network communication (not merge) remained the unsolved problem; and "cloud servers still have their place" as archival backup and relay. Local-first is not anti-server. It is anti-server-as-sole-authority.

---

## Conflict resolution

### Last-write-wins

Convergent and trivially cheap. Genuinely fine when the write is a full replacement the user intends as a replacement (a toggle, a status enum, a theme setting), when the field is single-writer in practice, and when losing an update is acceptable because the user can see and redo it.

Not fine when the value is accumulated (counters, sets, text) or when two fields must move together.

**Two failure modes to distinguish:**

- **Whole-object clobber.** Device A edits `title`, device B concurrently edits `body`. Whole-object last-write-wins discards the entire loser, so an edit to an untouched field vanishes. **Per-field last-write-wins fixes this specific case** and is the highest-value cheap upgrade to a naive design. It does not fix cross-field invariants: per-field merge can produce a state neither device ever had, breaking rules like `status == 'shipped' implies shipped_at != null`.
- **Lost update.** Read-modify-write on both devices; one wins and the other's *intent* (increment, append) is lost. Last-write-wins cannot express an increment. Use a PN-Counter.

### Wall-clock time is the wrong tiebreaker

Device clocks drift, users set them manually, and NTP fails. A device whose clock is a year fast **wins every conflict for a year**. This is not a rare race; it is a permanent state, it is silent, and it presents to support as "my edits never save" on the *other* devices.

`Date.now()` is also non-monotonic (an NTP step moves it backward, so two writes on one device can invert) and its millisecond resolution guarantees ties, which need a deterministic tiebreak (compare replica ID lexically) or convergence itself breaks.

**Hybrid logical clocks** (Kulkarni, Demirbas, Madappa, Avva, Leone, OPODIS 2014) pair a physical component `l` with a logical counter `c`. On a local event, `l = max(l_prev, physical_now)`; if `l` did not advance, increment `c`, else reset `c` to 0. On receipt, `l = max(l_prev, l_remote, physical_now)`. Ordering compares `(l, c)` lexicographically with node ID as final tiebreak.

The properties that matter: it captures causality (plain wall clocks do not), stays bounded close to physical time (so timestamps remain human-meaningful for debugging and TTLs), and fits in 64 bits.

**It does not fix a badly-wrong clock by itself.** Without a max-drift rejection bound, one bad device drags every replica's `l` forward permanently, and the system does not self-heal. That bound is the part implementations forget.

### Conflict-free replicated data types

**Three families:**

- **State-based (CvRDT).** Replicas exchange full state; merge is a semilattice join and must be commutative, associative, and **idempotent**. Robust to duplicate and reordered delivery, so it works over lossy transports. Costs full-state transfer.
- **Operation-based (CmRDT).** Replicas broadcast operations, which must commute for concurrent pairs. Much smaller messages, but **requires exactly-once, causally-ordered delivery**. Teams implementing an op-based type over an at-least-once channel get silent divergence from duplicate application.
- **Delta-state** (Almeida, Shoker, Baquero, JPDC 2018). Ship join-irreducible deltas: state-based robustness at op-like bandwidth. The practical middle ground.

**The base family, each with its anomaly:**

| Type | Mechanism | Anomaly or cost |
|---|---|---|
| G-Counter | Per-replica increment map | Cannot decrement |
| PN-Counter | Two G-Counters | State grows with replica count |
| G-Set | Add only | Cannot remove |
| 2P-Set | Add-set plus tombstone-set | **An element removed once can never be re-added** |
| LWW-Register | Timestamped value | Inherits every clock problem above |
| MV-Register | Keeps all concurrent values | Converges but pushes the conflict to the caller, who usually reads index 0 |
| OR-Set | Unique tag per add; remove deletes observed tags | **Add-wins**; tag growth |

The OR-Set add-wins bias is a semantic choice, not a neutral default. Concurrent add and remove resolves to present. For a blocklist or a permission set that may be the wrong direction, and no library will tell you.

**Sequence types** (text, ordered lists) are the hard case: WOOT (2006), Treedoc (2009), Logoot / LSEQ (2009), RGA (2011), Causal Trees, YATA (the algorithm under Yjs), Fugue / FugueMax (Weidner and Kleppmann, 2023).

**Interleaving susceptibility, from the Fugue paper's survey** -- directly usable in review:

- Forward non-interleaving only: **RGA, Yjs**
- Maximal (forward and backward): **Fugue, FugueMax**
- Interleaves both directions: **Logoot, LSEQ, Treedoc**
- Forward interleaving: WOOT

Fugue's benchmarks show non-interleaving is not paid for in performance: on a 182,315-insertion trace it recorded 168 KB saved (Yjs 160 KB), 20 ms save time (Yjs 17 ms), 2.4 MB memory (Yjs 3.3 MB), and 94,000 operations per second (Yjs 39,000).

**Move operations in trees** (Kleppmann, Mulligan, Gomes, Beresford, IEEE TPDS 2022) are genuinely hard. Naive concurrent move breaks two ways: A moves X under Y while B moves Y under X, producing a **cycle** that is no longer a tree; or move modeled as delete-plus-insert **duplicates** the subtree. The solution is undo-do-redo with a cycle check. This is why "move a folder into another parent" is disproportionately hard, and why many products silently degrade move to delete-and-create. **Loro implements a movable tree.**

**Tombstone garbage collection requires causal stability** -- every replica is known to have seen the delete. In an open client population you cannot establish that without a coordination point, which is a strong practical argument for keeping a server in the loop even in a local-first design. Treat "we will garbage-collect tombstones later" as an unresolved design hole.

**Standard types assume non-Byzantine peers** (Kleppmann, PaPoC 2022). In P2P or untrusted-client topologies, a malicious peer can forge increments, fabricate causal parents, or send different states to different peers and **permanently break convergence**. That needs signed operations over hash-linked history.

### Operational transformation

Origin: Ellis and Gibbs, GROVE (SIGMOD 1989). Transmit operations with positional indices and transform an incoming operation against concurrent ones so the index is corrected.

- **TP1**: for concurrent `a`, `b`, applying `a` then `transform(b, a)` equals applying `b` then `transform(a, b)`.
- **TP2**: transformation must be order-independent across three or more concurrent operations. TP2 is where this gets hard, and **multiple published algorithms were later shown incorrect** by counterexample.

**How real systems dodge TP2**: the Jupiter system (Nichols, Curtis, Dixon, Lamping, UIST 1995) uses a **central server in a star topology**, so each pair only ever needs TP1. This is why Google Docs works and Google Wave's federated model did not.

**The honest summary**: operational transformation is simple and fast centralized, and hard-to-impossible decentralized. **If every edit passes through one server, it is a legitimate and battle-tested choice** -- a point the CRDT-enthusiast literature under-weights.

### The genuine third path

**Eg-walker** (Gentle and Kleppmann, EuroSys 2025). The document is an event graph, a DAG of insert/delete events with causal parents. Critical versions partition the graph so **you replay only concurrent regions**, transforming index operations with an internal type in O(n log n) rather than pairwise O(n²).

The crucial property: **in steady-state editing the replicated-type structure is discarded entirely, retaining only the plain text.** The memory win comes from not holding that state most of the time.

Reported figures: load and merge on sequential traces 4-50 ms (Automerge 100-200 ms, Yjs 150-300 ms); on a long-diverged asynchronous trace ~150 ms against operational transformation's ~3,600 seconds; steady-state memory 10-50 MB (Automerge 100-500 MB, Yjs 200-800 MB).

Stated limitations, from the paper: worst case O(n² log n) on adversarially ordered graphs; performance depends heavily on the topological sort heuristic (a poor order cost 8x); assumes non-Byzantine replicas and reliable broadcast; **plain text only** so far.

### Server-authoritative with rebase

The Replicache and Zero model, and the most important non-CRDT architecture:

1. A **mutator** is a named function plus arguments, not a diff.
2. The client runs it optimistically against the local store; the interface updates immediately.
3. The mutation is queued and sent; the server runs **the same named mutator** with real validation, authorization, and business logic.
4. On each pull the client resets to authoritative server state and **replays still-pending mutations on top**.
5. When the server's version lands, the client's speculative version is discarded.

Rollback needs no rollback code: if the server rejected or altered the mutation, the rebase produces a different result and the interface updates. Business invariants live server-side where they belong. Intent is preserved because you transmit the operation's meaning, not its effect.

Costs: it requires a server, the mutator must be **deterministic and runnable on both sides** (no `Math.random()`, no `Date.now()`, no direct I/O inside it), and the interface must tolerate values changing under it after a rebase.

### Manual conflict presentation

Sometimes the honest answer, when the merge is a judgment call software cannot make. Prior art: Dropbox conflicted copies, git merge conflicts, CouchDB retaining losing revisions in `_conflicts`.

Defensible only if it is rare, **no data is destroyed while awaiting resolution**, and the interface actually exists. The anti-pattern is designing for automatic merge, discovering it is wrong, and bolting on a dialog that shows users raw JSON diffs.

---

## Sync protocol design

### Change tracking

In increasing order of capability and cost:

1. **Dirty flags.** Cheapest. Loses ordering and intent; cannot express deletes without a separate tombstone. Adequate only for single-writer-per-row data.
2. **Oplog / changelog.** Preserves order and intent. Costs write amplification and needs a truncation policy: an oplog never pruned is a slow-growing outage.
3. **Version vectors.** Detect concurrency precisely (happened-before, happened-after, or concurrent). Size grows with replica count, which for a consumer app means every device ever used, including retired ones. Needs a pruning strategy.
4. **Merkle trees.** Locate divergence in O(log n) comparisons. Good for repair, not for normal-path incremental sync.
5. **Checkpoints / cursors.** Simplest correct incremental sync and what most production systems use. **Requires a total order on the server**, so this pattern is server-authoritative by nature. The trap: a cursor plus a `WHERE` clause does not tell you about rows that *left* the filter.

### Partial replication and the bootstrap cliff

**"Sync everything" fails at a cliff, not gradually**: the point where the initial dataset exceeds what a client can download and index during launch. Symptoms include a 30-second blank screen, memory exhaustion on low-end devices, and -- the subtle one -- **the application appearing broken only for your most valuable users**, because they have the most data. It will not reproduce on a developer's account.

Strategies in order of preference: partial replication by shape or bucket (a `WHERE` clause defining what this user can see); progressive bootstrap (working set first, backfill after) which requires the interface to represent "not yet available locally"; snapshot plus delta rather than replaying an operation log from zero; time-bounded windows.

**Partial replication's hard problem is the boundary.** When a row leaves the synced set (unshared, archived, filter no longer matches), the client must **drop** it, and drop is not delete, because a local delete must not propagate as user intent. Conflating "no longer synced to you" with "deleted" is a data-loss bug. PowerSync's rule is the correct shape: remove locally only when the row is gone from *all* buckets the client syncs.

### Idempotency

Networks give at-least-once at best. Every sync operation must be idempotent or de-duplicated.

State-based merges are idempotent by construction. Op-based types are not -- applying an increment twice increments twice -- so the transport must dedupe by operation ID. Mutator-style sync dedupes by **per-client monotonic mutation ID**: the server records the last ID processed per client and drops re-sends below the watermark. Idempotency keys on HTTP writes need a retention window, and that window is a correctness parameter: a retry after expiry double-applies.

### Ordering

**Causal consistency is the practical target.** If a user creates a project then a task in it, no device may ever see the task without the project. Violating this produces dangling references and crashes. Op-based types *require* causal delivery; if the transport does not guarantee it, buffer operations whose causal parents have not arrived.

Lamport timestamps give a total order consistent with causality but cannot distinguish concurrent from ordered. Version vectors detect concurrency exactly at O(replicas) size. Hybrid logical clocks give causality plus human-meaningful time in 64 bits. Server sequence numbers are simplest but unavailable for offline operations.

### Schema migration with old clients still syncing

**The most under-designed area in practice**, and where review adds the most value.

You cannot force upgrades. App-store review delays, disabled auto-update, and worst, an offline client holding **unsynced local writes under the old schema** -- blocking it loses that data. The sync payload is a **wire contract with the same compatibility discipline as a public API**.

Rules to enforce:

1. **Additive-only by default.** New optional fields with defaults. Never renumber, retype, or repurpose.
2. **Old clients must ignore unknown fields, not drop them.** The subtle killer: an old client parses a record, drops fields it does not know, and **writes it back**, silently deleting those fields for everyone. Either preserve and round-trip unknown fields, or make old clients read-only for records they do not fully understand.
3. **Version the payload explicitly** and negotiate at connect.
4. **Maintain a minimum-supported-version floor** with a tested upgrade wall that **still allows exporting unsynced local data**.
5. **Expand and contract.** Add the field, dual-write, wait for the population to migrate, stop writing the old field, then remove it. Each phase ships at least one client-update cycle apart.
6. **Changing a field's merge type is not a migration.** LWW-Register to MV-Register, list to text: the semantics differ and old operations cannot be reinterpreted. It is a new field. Event-sourced systems can never change the meaning of a past event, only add types plus an upcasting layer.

### Binary attachments

**Never put blobs in the sync stream.**

Sync metadata separately from bytes: the record holds an attachment ID, content hash, size, and type; bytes move over a channel with its own retry and progress. **Content-addressed storage** gives free deduplication, idempotent upload, and client-side integrity verification. **Resumable upload** is mandatory above a few megabytes on mobile.

The ordering hazard: the record syncs fast, the blob does not, so other devices see a record pointing at bytes that do not exist. The interface needs a first-class pending state, and the record must not be treated as invalid. Blob reference counting is its own tombstone problem, and clients should be able to evict bytes while keeping metadata.

### Authentication and authorization

- **Token refresh mid-sync.** A long-lived connection outlives a short-lived token. The common bug is treating 401 as fatal, clearing local state, and logging out **with unsynced writes still pending**. Refresh in-band; never discard the queue on an auth error.
- **Logout with unsynced writes.** Discarding them silently is data loss. Block until the queue drains, warn explicitly, or retain the queue tied to the user ID. **Multi-account applications must partition local storage per user**, or one user's data leaks to the next.
- **Per-row authorization, re-evaluated over time.** When a user loses access, the server must tell the client to drop the document and the client must actually delete it. Most implementations never implement revocation, so revoked users keep a permanent local copy.
- **Row-level security interacts badly with change-data-capture.** The write-ahead log contains all changes regardless of policy, so the sync server must apply the filter; getting that wrong ships every row to every client. **Ask what evaluates authorization, and whether it is re-evaluated when permissions change.**
- Never trust a client-supplied filter as an authorization boundary.

### Multi-device versus multi-user

Different problems; conflating them causes both over- and under-engineering.

| | Multi-device, one user | Multi-user collaboration |
|---|---|---|
| Concurrency | Rare, usually sequential | Common and simultaneous |
| Trust | Single domain | Multiple; needs per-user authorization |
| Right answer | Per-field LWW plus HLC is often enough | Sequence type for text; explicit merge elsewhere |
| Presence | Not needed | Essential |
| A lost update is | The user's own | A social problem |

The common over-engineering error is a full replicated-type stack for a single-user notes application syncing phone and laptop, where per-field merge would have shipped in a tenth of the time. The under-engineering error is the reverse.

**The intermediate case that gets missed**: same user, two devices, one offline for a week. Concurrency is rare but **divergence is deep** -- exactly where operational transformation performs worst and last-write-wins loses most.

### Presence must not persist

Cursor positions, selections, typing indicators, live pointers.

**This state must never be persisted and must never enter document history.** Yjs gets it right with a separate awareness protocol.

Failure modes when violated: the document grows forever with cursors from years past; presence becomes versioned, merged, and **undoable**, so one user's undo moves another's cursor; a crashed client never sends "I left", so presence needs heartbeat expiry rather than a clean-disconnect assumption, and ghost avatars are the visible symptom; presence is high-frequency and must not share a backpressure budget with edits; and presence leaks data, since cursor position reveals what someone is reading.

### Backpressure and the radio cost

**The mobile radio dominates battery cost and is not proportional to bytes.** A transmission promotes the radio to a high-power state which persists for a **tail time** of seconds. Ten small requests spread over a minute cost far more than one request carrying the same bytes.

Batch and coalesce; debounce outbound changes; align with existing wakeups. **Backpressure must be real**: a bounded queue with a defined policy (coalesce older operations for the same entity, or block the producer) plus visible pending state. **Do not poll** -- use push with exponential backoff **plus jitter**, since without jitter every client reconnects simultaneously after a restart and the herd prevents recovery. Compress; sync payloads are highly repetitive.

### Background sync is best-effort everywhere

- **Web Background Sync is Chromium-only.** Not Safari, not Firefox. Requires HTTPS. Treat it as an enhancement over a sync-on-foreground fallback you must implement anyway.
- **iOS `BGTaskScheduler`**: the system decides if and when, based on usage, battery, and Low Power Mode. Hard time budget; register an expiration handler and checkpoint partial progress.
- **Android `WorkManager`**: subject to Doze and App Standby buckets, and aggressive vendor battery managers kill background work regardless of framework promises.

**The universal consequence: the design must be correct when background sync never runs.** It must sync on foreground, and the interface must never imply data is safely uploaded when it is not.

---

## Local storage substrate

### Web

**IndexedDB** is the only durable, large-capacity, structured, widely-supported browser store, and it is full of hazards:

- **The transaction auto-close footgun.** A transaction stays active only while work is pending in its event-loop turn. **Awaiting a non-IndexedDB promise inside a transaction yields the loop, the transaction auto-closes, and subsequent requests throw `TransactionInactiveError`.** This appears the moment anyone modernizes callback code to `async/await`. **Safari closes transactions more aggressively**, so it reliably manifests as "works in Chrome, broken in Safari." Gather data before opening the transaction; issue every request inside it with no intervening await.
- **Safari evicts script-writable storage after 7 days of no interaction**, IndexedDB included. A user returning from a two-week vacation finds an empty local database. Mitigate with `navigator.storage.persist()`, which Safari grants only for installed sites or strong engagement. **A web application that cannot obtain persistent storage cannot honestly claim local-first.**
- Quota varies by browser and free disk; exceeding it throws `QuotaExceededError` and needs a real strategy, not a crash.
- Multi-tab upgrades block: handle `onversionchange` and `onblocked`.

**OPFS** enables synchronous file access in Workers and makes WASM SQLite viable, but is subject to the same eviction policy.

**`localStorage`** is synchronous main-thread I/O, roughly 5 MB, strings only, no transactions or indexes, and evictable. Acceptable only for tiny non-critical preferences. **Storing a sync queue or user data in it is a defect.**

**Cross-tab coordination is a requirement, not an edge case.** Two tabs both running sync double-apply operations and fight over the local database. Use `BroadcastChannel` plus **Web Locks** to elect a leader tab owning the connection.

**WASM SQLite plus OPFS** now works, with two virtual filesystems and a real tradeoff. The standard OPFS VFS is multi-connection but **requires COOP/COEP headers** (which break third-party embeds); `opfs-sahpool` needs no such headers and is fastest but is **single-connection**. Both are Worker-only. Safari below 17 has a sub-worker bug affecting the standard VFS, while the pool VFS works from 16.4.

### Mobile

SQLite everywhere, via Room, GRDB, or SQLDelight. **Enable write-ahead logging** so sync writes do not block interface reads.

Corruption on force-quit is rare with write-ahead logging but reachable via `synchronous=OFF`, via storing the database where the operating system may reclaim it, and via **iOS Data Protection**: a file with complete protection is **unreadable while the device is locked**, so a background task waking on a locked phone gets I/O errors that are often mishandled as corruption. Set the protection class deliberately.

**The sync database and outbound queue must live in persistent storage, never a cache directory** -- caches can be reclaimed at any time. Large synced caches should be excluded from backup.

### End-to-end encryption conflicts with server-side features

The conflict is structural, not a matter of effort. **If the server cannot read the data, it cannot merge it, validate it, run authoritative business logic on it, index it, or search it.** That forecloses the entire server-authoritative-rebase family. Merge must happen on clients, which means replicated types, which means accepting that convergence is not correctness and that cross-user invariants cannot be enforced.

Key distribution is the hard part, not encryption. **Revocation is close to impossible**: you can prevent future reads, not claw back the past. Key loss is data loss. Metadata leaks regardless -- document sizes, edit timing, the collaboration graph.

**The choice must be made up front.** Retrofitting end-to-end encryption onto a server-authoritative sync engine is a rewrite, not a feature.

---

## Testing and observability

### Convergence is property-testable

The best engineering property of replicated types: convergence is formal, so it is testable.

1. Generate a random operation sequence and a random partition across N replicas.
2. Generate a random delivery schedule: arbitrary order, duplication, delay.
3. Deliver everything.
4. **Assert all replicas are byte-identical** -- serialize and compare, not "equivalent."

Assert the algebraic laws directly, since convergence rests on them: `merge(a, b) == merge(b, a)`, `merge(merge(a, b), c) == merge(a, merge(b, c))`, `merge(a, a) == a`.

Test what convergence does **not** cover, because that is where real bugs live: no interleaving for sequences; intent preservation under concurrent delete-and-edit; **no resurrection** under any schedule; causal integrity (no observable state where a child exists without its parent); idempotent re-application of an operation log.

### Deterministic simulation

Run all replicas plus a simulated network in one process on one thread, with **a seeded generator driving every nondeterministic choice** -- message ordering, delays, drops, partition timing, clock values, crash points. A failing run is then **reproducible from its seed alone**, which is what makes distributed bugs debuggable rather than "we saw it once."

This requires all I/O, time, and randomness behind injectable interfaces. **Flag direct clock, random, or socket access in sync-core code as a testability defect**, independent of correctness. Retrofitting is expensive.

### Fault injection matrix

| Fault | Real cause | Bug it finds |
|---|---|---|
| Partition (including one-way) | Subway, captive portal | Divergence handling; naive ack logic |
| Delay, asymmetric | Slow cellular | Timeout assumptions; head-of-line blocking |
| Reorder | Multipath, retries | Types assuming ordered delivery |
| Duplicate | At-least-once transport | Non-idempotent apply |
| Clock skew, including backward | Bad time sync | Every last-write-wins bug |
| Crash mid-write | Force-quit, memory kill | Partial application; lost queue |
| Storage eviction | Browser or platform cleanup | Unhandled empty-database-on-launch |
| Auth expiry mid-sync | Token TTL | Queue loss on 401 |
| Long offline past the GC horizon | Vacation | Resurrection; missing forced resync |
| Schema version mismatch | Staggered rollout | Field-dropping round-trip loss |

The last three are routinely omitted and produce the worst incidents.

### Sync failures are silent

The client believes it is fine while diverging. Instrument to catch silence.

Track sync lag as a distribution, not an average -- the p99 is the interesting one, because averages hide users whose sync is broken. Track **queue age (the age of the oldest unsynced item), which is higher-signal than depth**: depth looks fine for a client with one permanently-failing item. Track bootstrap duration segmented by account-data-size percentile, which is how you catch the power-user cliff before it becomes support tickets. Track conflict rate by entity type, a product signal as much as a technical one. Track **schema version distribution across the live population** -- you cannot plan a contraction without it.

**Detect divergence with checksums** compared client to server on a cadence, with full re-download as the blunt but correct repair. **A repair path must exist and be tested**; if the only recovery is reinstalling, you have unbounded data loss, and the repair should preserve unsynced local writes.

**Correlate client and server** by propagating a client-generated mutation ID into server logs, so "my edit did not save" is answerable end-to-end. Without it, sync bug reports are unactionable.

---

## Anti-pattern catalog

### Clocks and ordering
- `Date.now()` as the conflict tiebreaker; a fast-clocked device wins permanently.
- No max-drift bound on a hybrid logical clock; one bad client poisons every replica's clock, and it does not self-heal.
- No deterministic tiebreak for equal timestamps; replicas pick different winners and diverge.
- Assuming wall-clock ordering implies causal ordering.

### Deletion
- Physical row removal with no tombstone; deleted rows resurrect from a replica that still holds them.
- Tombstone garbage collection without causal stability; a client offline past the horizon resurrects everything it deleted.
- No enforced "offline longer than N days means full resync" wall.
- Treating "no longer in my sync filter" as "deleted" and propagating it as user intent.
- A tombstone treated as satisfying an erasure request.

### Merge semantics
- Whole-object last-write-wins, discarding edits to untouched fields.
- Per-field merge applied to data with cross-field invariants.
- 2P-Set (or any remove-once semantics) for a re-addable collection.
- A sequence type with known bidirectional interleaving used for collaborative text.
- Naive concurrent tree move, producing a cycle or a duplicated subtree.
- Convergence treated as sufficient for business invariants (inventory, booking, balance).
- Op-based type over an at-least-once transport with no dedupe.
- Assuming non-Byzantine peers in an untrusted or P2P topology.

### Protocol
- A persisted retry queue presented as a sync engine.
- Server-assigned autoincrement IDs for offline-creatable entities.
- Non-deterministic mutators in a rebase model (`Date.now()`, `Math.random()` inside the mutator).
- Blobs in the sync stream; a large attachment stalls the change channel.
- Unbounded outbound queue, or one poison item blocking all others.
- No jitter on reconnect backoff, producing a thundering herd that prevents recovery.
- Polling on an interval instead of push.
- Syncing everything on login.

### Schema
- Old clients that drop unknown fields and write records back, silently deleting them for everyone.
- Changing a field's merge type in place rather than adding a new field.
- No minimum-supported-version floor, or a floor that blocks export of unsynced local data.
- No payload version negotiated at connect.

### Storage
- Awaiting a non-IndexedDB promise inside an IndexedDB transaction.
- No `navigator.storage.persist()` on a durable web application.
- `localStorage` for the sync queue or user data.
- Two tabs each running the sync engine with no leader election.
- Sync database or queue in a cache directory.
- A database protection class that makes the file unreadable during background sync on a locked device.

### Authentication
- Clearing local state on 401, destroying unsynced writes.
- Authorization evaluated only at initial sync, never on revocation.
- Shared local storage across accounts in a multi-account application.
- A client-supplied filter trusted as an authorization boundary.

### Interface
- Rendering optimistic local data as confirmed; ignoring pending-write and from-cache flags.
- Persisting presence or awareness into the document.
- Presence with no heartbeat expiry, leaving ghost users.
- Claiming background upload completed before acknowledgement.

---

## Library landscape (verified 2026-08-26)

Repository activity was confirmed live. **Treat activity as evidence of maintenance, not of feature development.**

**Replicated-type libraries.** Yjs (v13.6.32, 2026-08-04) is mature and stable on a long-settled line; Yjs 14 is in development with changesets and attributions for version history and track changes. Automerge is active; **Automerge 3.0 (2025)** moved the columnar compressed format into runtime memory, reporting roughly 100x memory reduction in the cited case and much faster load, with the same file format as 2.x. **Note that `automerge-repo-rs` is not wire- or disk-compatible with the JavaScript `automerge-repo`** -- a blocking concern for a Rust server against JavaScript clients. Loro is very active and implements both a **movable tree** and Eg-walker, making it the main off-the-shelf option when correct tree moves matter. Diamond Types is Gentle's research vehicle behind the Eg-walker results; treat it as a reference implementation.

**Sync engines.** **Electric pivoted hard in July 2024**, abandoning a CRDT-based bidirectional local-first framework for an HTTP API syncing Shapes (a `WHERE` clause over a table), **read-path only**, with writes going through your existing backend. That makes it easy to adopt and explicitly not local-first. **Zero 1.0 shipped June 2026** with custom mutators and a query language used identically against local cache and server. **Replicache is archived** (last push 2022); do not start new work on it. PowerSync is active and its documented bucket-and-checkpoint protocol is worth studying: ordered per-bucket operations, sequential checkpoints with per-bucket checksums, full bucket re-download on mismatch, and **client-side removal only when a row is gone from every bucket**.

**Stalled -- do not adopt for new work.** Triplit (last push 2026-01-19) and **WatermelonDB (last push 2025-08-11, 302 open issues)**, the latter notable because it was a default React Native recommendation for years.

**Corrections to common belief**: **cr-sqlite is active** (last push 2026-08-10) despite its author joining Rocicorp, and **PouchDB is active** (2026-08-25) despite widespread assumption otherwise.

**Established platforms.** Firestore offline persistence is **enabled by default on mobile but disabled on web**, with a 100 MB default cache and last-write-wins conflict resolution; most Firestore bugs in the wild come from ignoring the pending-writes and from-cache metadata flags. Offline collection queries with no cached matches **return empty while single-document fetches throw** -- different failure shapes that both need handling.

**MongoDB Atlas Device Sync / Realm was deprecated September 2024 and reached end of life 30 September 2025.** Local Realm remains Apache-2.0, but SDK 20.x and later have no cloud sync. **The sync server and conflict logic were proprietary and never self-hostable, which is why no community rescue was possible.** This is the Longevity ideal failing in the real world, and the strongest practical argument for preferring sync layers whose protocol and merge semantics you could reimplement.

The **CouchDB replication protocol** remains the best-documented open sync protocol, and its design of converging automatically while **retaining losing revisions** for application resolution is underrated prior art.

**Apple platform sync** (CKSyncEngine, `NSPersistentCloudKitContainer`, SwiftData) could not be verified against current documentation in research; the long-standing Core Data plus CloudKit constraints (no unique constraints, all attributes optional or defaulted) are stable but should be confirmed before being relied on.

---

## Schools of thought (preserve disagreement)

- **Replicated types vs operational transformation vs server-authoritative.** Not settled; they answer different questions. Replicated types win for P2P, long divergence, and no trusted server. Operational transformation remains defensible and battle-tested **when a central server orders all edits**, and Sun et al. argue the field under-weights that most shipped co-editors use it. Server-authoritative rebase wins when business invariants matter more than offline-forever. Eg-walker beats both on their respective weak cases. **Ask which failure mode this product can least afford**, not which algorithm is best.
- **General-purpose library vs hand-rolled domain merge.** The library camp: convergence is proven and the hard cases are done. The hand-rolled camp: most applications are not collaborative text, and per-field merge plus a hybrid logical clock is a few hundred lines producing merges you can explain to users. General merge converges but may produce semantic garbage; domain merge is correct but must be re-derived per entity type.
- **Buy vs build.** Buy: sync is a multi-year project. Build: **Realm's end-of-life is the argument.** The moderate position is to adopt a layer whose protocol is documented and whose merge semantics you could reimplement, so the vendor is replaceable.
- **Local-first purism vs pragmatic offline-capable.** Purists hold all seven ideals. Pragmatists note the most successful products marketed as local-first are server-authoritative and would not exist under purist constraints. Kleppmann's gradient reframing concedes ground. The marketing hazard: the distinction matters to users only when the vendor dies, which is exactly when it is too late.
- **End-to-end encryption vs server-side features.** Structurally exclusive. There is no middle, and the choice precedes architecture.
- **"Sync is solved, use a library" vs "sync is domain-specific."** **The disagreement most worth surfacing in review**, because it determines whether "we use Yjs" counts as having a sync design. Convergence was never the hard part; partial-replication boundaries, authorization and revocation, schema migration across un-upgradeable clients, bootstrap cost, and deciding what a merge *should mean* are, and none are in any library.

---

## What is NOT a sync-and-offline finding

- Server-side replication topology, sharding, isolation levels, consensus, storage-engine choice. Route to `distsys-data`.
- Service-layer retries, circuit breakers, and caching not part of the client sync path. Route to `distsys-runtime`.
- Platform background-execution API usage as such. Route to `mobile-native` or `desktop-native`; we own what the sync design must assume about them.
- General cryptography review. Route to `security`; we own end-to-end encryption only where it constrains merge architecture.
- Erasure and retention obligations as regulation. Route to `app-privacy-compliance`; we own why a tombstone is not an erasure.
- Generic "use a CRDT" or "rewrite on Zero" advocacy. The value is a concrete defect or a named tradeoff, not paradigm preference.
- Conflict-resolution *policy* the product deliberately chose and documented (last-write-wins on a settings object is fine). Flag undocumented or accidental policy.

---

## Severity calibration

Per `panel-contract.md`:

- **blocker**: silent data loss with a reachable trigger -- wall-clock tiebreaking on multi-device data, physical deletes with no tombstone, clearing the outbound queue on auth failure, old clients round-tripping records and dropping unknown fields, treating a filter exit as a user deletion, a local delete propagating from a partial-replication boundary.
- **major**: a retry queue serving as the sync design on offline-writable data; server-assigned IDs for offline-creatable entities; unbounded queue or tombstone growth with no policy; no minimum-version floor or no export path behind the upgrade wall; authorization never re-evaluated on revocation; blobs in the change stream; two tabs syncing without leader election; awaiting non-IndexedDB work inside a transaction; no repair path for detected divergence.
- **minor**: missing `navigator.storage.persist()`; no jitter on reconnect; polling where push is available; sync lag tracked as an average; presence without heartbeat expiry; missing pending-state in the interface.
- **nit**: naming and structure of sync metadata; log verbosity around retries.
- **insight**: structural reframing -- "this is multi-device single-user, and per-field merge plus a hybrid logical clock would replace the replicated-type dependency"; "the bootstrap has no partial-replication boundary and will cliff at the p99 account"; "convergence is guaranteed here but the invariant that inventory stays non-negative is not, and no merge function can enforce it."

Confidence: high when the trigger is concrete (a specific timestamp comparison, a specific delete path, a specific transaction body); medium when reasoned from architecture without seeing the merge implementation.

---

## Process for the sync-and-offline agent

1. **Classify the product shape.** Multi-device single-user, multi-user collaborative, or both. The correct answer differs sharply, and over-engineering is as common as under-engineering.
2. **Find the ordering metadata.** Version vectors, hybrid logical clocks, operation IDs, causal parents, sequence cursors. Absence means it is a retry queue, and that is the headline finding.
3. **Walk identity.** Who generates IDs, and can that happen offline?
4. **Walk the merge.** What is the conflict policy per entity and per field? Is it documented and deliberate? Does it preserve intent? Does it hold cross-field and cross-entity invariants?
5. **Walk deletion.** Tombstones present? Garbage-collection policy? Offline horizon enforced? Is "dropped from my scope" distinguished from "deleted"?
6. **Walk the bootstrap.** Partial replication boundary? What happens to the p99 account? What happens when a row leaves the boundary?
7. **Walk schema evolution.** Additive-only? Do old clients preserve unknown fields? Is there a version floor, and can a too-old client still export its unsynced data?
8. **Walk authentication.** Token refresh mid-sync, logout with a pending queue, per-user storage partitioning, authorization re-evaluation on revocation.
9. **Walk storage.** Correct substrate, transaction discipline, persistence requested, eviction handled, cross-tab leader election, database not in a cache directory.
10. **Walk the queue.** Bounded? Coalescing? Poison-item handling? Jitter on backoff? Batching against radio cost?
11. **Walk observability.** Queue age, sync lag distribution, divergence detection, a tested repair path, client-to-server mutation correlation.
12. **Check testability.** Is time, randomness, and I/O injectable in the sync core? Are convergence properties asserted?
13. **Route to other lenses**: server replication to `distsys-data`; platform background APIs to `mobile-native` or `desktop-native`; cryptography to `security`; erasure obligations to `app-privacy-compliance`.
14. **Stay read-only.**
