# API Design Principles

A reference for evaluating code from an API-design lens during review. Used by the `api-design` subagent. The scope is *contract design at any surface where consumers depend on shape*: HTTP/REST APIs, gRPC / Protobuf services, GraphQL schemas, library / SDK public surfaces, and command-line interfaces (CLIs). Distinct from `documentation.md` (which evaluates whether the surface is *documented*), `security.md` (which evaluates the threat model of the surface), and `performance.md` (which evaluates how the surface performs).

The core thesis: **backward compatibility is the central concern of API design.** Naming, pagination format, error envelope choice -- all conventions, all important mostly because inconsistency creates friction. Breaking changes are different in kind: they impose work on consumers, erode trust, and once shipped cannot be unshipped. Every breaking-change finding outranks every style finding.

The empirical law underneath: **Hyrum's Law** ("with a sufficient number of users, every observable behavior of your system will be depended on by somebody," -- Hyrum Wright). Documentation is a defense in court, not in production. The reviewer's lens is therefore: *what about this contract will hurt consumers in six months?*

---

## Universal principles (cross-paradigm)

These hold whether you're reviewing a REST endpoint, a gRPC service, a GraphQL schema, an exported function, or a CLI flag.

### "Public APIs are forever -- one chance to get it right" (Bloch)

Joshua Bloch's 2006 talk frames it: once a consumer depends on your surface, you cannot remove or narrow without breaking them. The corollaries:

- **"When in doubt, leave it out."** It's easier to add later than to remove. Every exposed symbol is a future constraint.
- **"Easy to use, hard to misuse."** A signature that admits invalid arguments will be called with invalid arguments. Push invariants into types and parse-don't-validate at the boundary.
- **"Write the client code first."** Sketch how callers will use the unwritten API before implementing it. If the call site reads poorly, redesign before shipping.
- **"Expect to throw the first one away."** First-draft APIs are almost always wrong; treat the first internal users as design feedback, not production consumers.

### Hyrum's Law: observable behavior IS the contract

Every behavior consumers can observe -- field ordering, capitalization of error messages, exact latency under load, the specific 4xx code returned for a specific edge case -- will eventually be depended on by someone. Documenting "this is unspecified" reduces the cost of change but does not eliminate it.

The implications for review:
- **Documented behavior is the contract floor, not the ceiling.** Flag observable-but-undocumented behavior as a Hyrum's-Law trap.
- **Behavioral consistency matters.** If two endpoints sort differently by default, consumers will encode that assumption.
- **Latency, ordering, identity, capitalization** are all part of the surface even when nobody wrote them down.

### Information hiding

Every implementation detail you don't expose is a degree of freedom you keep. Conversely, every detail you do expose -- through a default value, a type that leaks an enum, a serialization format that reveals an internal field -- is a constraint forever.

**Flag**: response objects that include internal database column names; SDK return types that expose ORM entities; error messages that leak stack traces or internal service names; default values that betray implementation choices ("`shard_id: 17`" suggests a fixed shard count).

### Consistency within the surface > correctness of any one choice

Snake_case vs camelCase, plural vs singular nouns, errors-as-200 vs errors-as-4xx -- these are all defensible choices but one of them. **The choice is local; the consistency is global.** A mixed surface (some endpoints camelCase, some snake_case) is harder to use than either pure choice.

The review move is rarely "you chose snake_case; switch." It's "this endpoint uses camelCase while every other endpoint in this service uses snake_case -- pick one."

### Optionality and defaults: defaults are part of the contract

A field with a default value is observably present to clients who don't set it. Changing the default is a breaking change for any client that relied on the old default's behavior.

**Flag**: changing a default value in a new version of a function, endpoint, or flag without versioning the change. "We changed `timeout` default from 30s to 10s" is breaking even though no signature changed.

### Composability

A good API composes with itself and with neighbors. Operations chain (`list` returns IDs you can `get`), pagination composes with filtering (filter applies to the underlying set, then pages), bulk operations have well-defined partial-failure semantics. A surface where every primitive needs special-casing at the call site signals missing abstractions.

---

## HTTP / REST

### Richardson Maturity Model: most "REST" is Level 2

Leonard Richardson's four-level taxonomy, popularized by Martin Fowler:
- **Level 0**: one endpoint, one method (SOAP-style RPC over HTTP).
- **Level 1**: resources (multiple URLs, but one method).
- **Level 2**: HTTP verbs + status codes (the industry "REST").
- **Level 3**: HATEOAS / hypermedia controls (Fielding's actual REST).

Fielding's 2008 rant ("REST APIs must be hypertext-driven") frames Level 2 as RPC-in-disguise. Industry adopted Level 2 as "REST" and largely ignored Level 3. The pragmatic position (Sturgeon, Higginbotham): Level 2 is sufficient for typed SDK consumers; HATEOAS adds overhead without benefit for the common case. The empirical answer: **Level 2 won.** Whether it should have is a separate question.

The reviewer's stance: don't flag the absence of HATEOAS as a defect. Do flag inconsistency *within* the chosen level -- Level 2 APIs that randomly tunnel through POST are violating their own model.

### HTTP semantics: read RFC 9110

`GET` (safe, idempotent, cacheable, no body in request), `POST` (creates / acts), `PUT` (replaces, idempotent), `PATCH` (partial update, not necessarily idempotent), `DELETE` (idempotent), `HEAD` / `OPTIONS` (metadata). The methods are not interchangeable; clients, proxies, caches, and CDNs all behave differently based on which.

**Flag**:
- `GET` that mutates state (caching breaks it, browsers prefetch it).
- `POST` that is genuinely idempotent and replaceable (consider `PUT`).
- `DELETE` with a body (RFC 9110 allows but most middleware ignores).
- Confusion between `PUT` (replace whole resource) and `PATCH` (partial update).

### Status codes: be specific, not creative

The categories: 2xx success, 3xx redirect, 4xx client error, 5xx server error. The specific codes matter:
- `200 OK` (general success), `201 Created` (resource created, include `Location` header), `202 Accepted` (async work started), `204 No Content` (success, no body).
- `400 Bad Request` (malformed), `401 Unauthorized` (not authenticated), `403 Forbidden` (authenticated but not authorized), `404 Not Found`, `409 Conflict` (state precludes), `422 Unprocessable Content` (semantic validation failure), `429 Too Many Requests` (rate-limited).
- `500 Internal Server Error` (unexpected server fault), `502 Bad Gateway`, `503 Service Unavailable` (with `Retry-After`), `504 Gateway Timeout`.

**Flag**:
- `200` with `{"success": false, "error": ...}` in the body -- defeats HTTP-aware tooling.
- `401` returned when the user *is* authenticated but not authorized (use `403`).
- `500` returned for client errors (validation should be 4xx).
- Inconsistent status code for the same logical condition across endpoints.

### PATCH semantics: pick a standard

Two competing patch formats:
- **JSON Merge Patch (RFC 7396)**: partial object; present fields overwrite, absent fields unchanged, `null` deletes. Simple but cannot represent "set list to empty" distinctly from "leave alone."
- **JSON Patch (RFC 6902)**: array of operations (`{"op": "replace", "path": "/name", "value": "Alice"}`). More powerful but verbose.

Many APIs accept partial objects on `POST` or `PATCH` as upserts without naming the standard. That's fine if documented and consistent; flag if the semantics are ambiguous between merge and replace.

### Resource-oriented vs action-oriented

The big divide:
- **Resource-oriented (Google AIP, Stripe)**: URLs are nouns. Standard methods (Get/List/Create/Update/Delete) handle most operations. Custom methods (`POST /users/123:promote` per AIP-136, or `POST /users/123/promote` more pragmatically) for the rest.
- **Action-oriented (some AWS, RPC-flavored)**: URLs are verbs or actions; `?Action=DescribeInstances`.

The pragmatic answer: resource-oriented by default; custom methods when the operation is genuinely not CRUD (`refund`, `merge`, `archive_with_reason`). Forcing every business operation into PUT/PATCH violates the user's mental model.

**Flag**: god endpoints (`/api/data?type=user&action=update`); `POST /server` with `{action: "restart"}` (tunneling RPC through POST when `POST /servers/{id}:restart` would be honest); CRUD over DB tables with no domain meaning (anemic API).

### Pagination: cursor, not offset

Offset pagination (`?page=3&size=20`) is broken under concurrent insert/delete (rows shift; duplicates and gaps appear), and offset N requires scanning N rows in most databases.

**Cursor / keyset pagination** (`?cursor=abc&limit=20`) is the only correct approach for any growing list. The cursor is typically an opaque base64-encoded representation of `(sort_key, id)`; the response includes `next_cursor` and `has_more`.

**Flag**: offset pagination on lists that grow (any time-ordered list, audit log, feed); cursor formats that expose primary keys (couples external contract to internal storage); list responses without explicit ordering; pagination that doesn't compose with filtering (filter applied per-page rather than to the underlying set).

### Error responses: machine + human + remediation

The triad:
- **Machine-readable code**: stable identifier the client branches on (`insufficient_funds`, `rate_limit_exceeded`).
- **Human-readable message**: English; for developer / log consumption, not end-user display.
- **Remediation hint**: param-level details, link to docs, retry guidance.

Three competing structured-error standards:
- **RFC 7807 / RFC 9457 (Problem Details)**: `type`, `title`, `status`, `detail`, `instance`. The IETF default.
- **Google's `google.rpc.Status`**: `code`, `message`, `details` (array of structured info like `BadRequest`, `QuotaFailure`).
- **Stripe-style**: `type`, `code`, `message`, `param`, `doc_url`. Most informative for SDK consumers.

**Flag**: errors that don't include a stable code (clients reduced to string-matching on `message`); errors with different envelope shapes across endpoints; errors leaking stack traces or internal IDs; errors that fail to identify which field caused validation failure; success responses (`200`) that contain errors.

### Versioning: no consensus, but pick one and commit

The major strategies, in rough order of popularity:

- **URL versioning** (`/v1/users`): Twitter, GitHub v3, most public APIs. Visible, easy to route, easy to grep. Downside: ties resource identity to version; encourages big-bang transitions; doubles surface forever.
- **Date-based versioning** (`Stripe-Version: 2024-06-20`): Stripe, Shopify, Twilio. Versions form a total order; per-account pinning; server-side translation layers. Requires real infrastructure; pays off at B2B scale.
- **Header versioning** (`Accept: application/vnd.foo.v2+json`): GitHub preview headers. Aligned with content negotiation; invisible in URLs; curl-hostile.
- **No versioning / continuous evolution** (Mark Nottingham's preferred position, GraphQL by design): only additive changes; deprecation with sunset headers (RFC 8594). Requires discipline; some changes genuinely cannot be made additively.
- **Semver in URLs**: rarely well-defined for HTTP APIs (what's a "patch" in an HTTP API?). AIP-185: major-only in URL.

**Flag**:
- Versions in headers AND URLs simultaneously (which wins?).
- A v1 that's been bumped to v2 with no migration documentation.
- "We don't break things" coexisting with quietly removed fields.
- No documented deprecation policy.
- Inability to articulate which changes would force a new version.

The reviewer's actual move: **ask "what's the deprecation policy, and how do customers learn about it?"** -- not "you must use strategy X."

### Idempotency keys

Mutating endpoints should accept an `Idempotency-Key` header (or similar) for safe retry. The Stripe model:
- Client generates a stable key per logical operation (UUIDv4 typically).
- Server records `(key, response)` in a dedicated table; on retry with the same key, returns stored response.
- Retention window: 24 hours is typical.
- Mismatch handling: same key with different params returns `422`, doesn't execute twice.

**Flag**: `POST` endpoints that create resources (charges, transfers, orders, notifications) without idempotency; idempotency keys that aren't scoped per API-key / account (leak across customers); retention windows shorter than client retry budgets.

(See `~/.claude/rules/distributed-systems.md` principle 17 and `~/.claude/rules/system-design-patterns.md` § Idempotency Keys for the systems-design angle.)

### Long-running operations

Two patterns:
- **Operation resource (AIP-151)**: request returns an `Operation` object with `done: false`; client polls `GET /operations/{name}`. REST-conservative.
- **Webhooks**: server pushes completion. Faster, but requires infrastructure on both sides.

**Flag**: synchronous endpoints that take > 30s (clients time out); polling-only APIs with no webhook alternative (forces clients to choose between staleness and load); webhook endpoints with no signing (anyone who knows the URL can forge events).

### Webhook design contract

When the API publishes webhooks:
1. **Signed payloads** (HMAC over body, `X-Signature: sha256=...`).
2. **At-least-once delivery** (consumers must be idempotent on event ID).
3. **Exponential backoff retry** (typical: 1m, 5m, 30m, 2h, 12h, 24h, dead-letter).
4. **30s timeout** for the receiver's response.
5. **Replay protection** (timestamp in signed payload; reject events older than ~5 minutes).
6. **Out-of-order tolerance** (consumers use sequence numbers or timestamps).
7. **Dead-letter queue** after N failed retries.

**Flag**: webhooks without signing; webhooks with no retry policy documented; webhooks claiming "exactly-once" delivery (impossible; clients still need idempotency).

---

## gRPC / Protobuf

### Wire-format rules (Kenton Varda)

The Protobuf wire format is defined by field numbers, not names. The rules are mechanical and the Buf toolchain enforces them in CI:

- **Never reuse a field number.** Once tag 7 was a `string name`, it's bound to that semantic forever. Use `reserved 7;` to enforce.
- **Field numbers 1-15 use one byte on the wire; 16+ use two.** Reserve 1-15 for hot, common fields.
- **Cardinality changes break the wire.** Singular ↔ repeated is breaking; repeated → map is breaking.
- **No `required` fields in proto3.** Required-ness is application-level; the wire treats every field as optional.
- **Default values are not transmitted.** A field set to its default is indistinguishable from "not set" in proto3 without `optional`.
- **Type changes are mostly unsafe.** `int32 ↔ int64` is wire-compatible but semantically can truncate. `string ↔ bytes` is wire-compatible. Most other changes are not.

**Flag**:
- Any reuse of a previously-assigned field number.
- Cardinality changes on existing fields.
- New `required`-like validation that wasn't there before.
- Type widening / narrowing without versioning.

### Service / method evolution

- **Renaming a service or method** is breaking (RPC routing keys on the name).
- **Removing a method** is breaking.
- **Changing streaming semantics** (unary ↔ streaming) is breaking.
- **Package renames** are breaking.

**Tooling check**: does the project run `buf breaking` in CI against the main branch? If not, this is the highest-leverage CI addition for a Protobuf shop.

### `oneof` and enum care

- **Adding a field to a `oneof`** is wire-compatible but can be semantically breaking (old code never matches the new case).
- **Reordering enum values** that changes numeric assignments is breaking.
- **Removing an enum value** is breaking for senders; clients with the old value will fail.
- **Adding an enum value** is safe wire-wise, but clients without exhaustive handling will land in the default branch unexpectedly.

---

## GraphQL

### Schema evolution: no versioning, only deprecation

The Facebook position (Lee Byron, Dan Schafer, Nick Schrock): clients query for specific fields, so additive changes don't break anyone; deprecation via `@deprecated(reason: ...)`; never remove, only deprecate.

Strength: no version proliferation. Weakness: deprecated fields accumulate forever; schema bloat is real; "deprecation" is advisory because old clients are unkillable.

### Breaking changes in GraphQL

- Removing a field from an output type (clients depending on it).
- Changing a field's type.
- Adding a required field to an input type (clients omitting it now fail).
- Narrowing an enum (removing a value).
- Removing or renaming an enum value.
- Changing an argument's type or removing a default-less argument.
- **Nullability on output: non-null → nullable is breaking** (clients expected non-null).
- **Nullability on input: nullable → non-null is breaking** (clients omitted the field).
- Removing a type from a union.
- Changing an interface's required fields.

**Tooling check**: GraphQL Inspector or Apollo Studio in CI to detect breaking schema changes pre-merge.

### Query complexity / depth limits

A single GraphQL query can fan out to thousands of resolver calls (nested lists). Without limits, a malicious or buggy client can DoS the server.

**Flag**: GraphQL endpoints with no query complexity analysis, no depth limit, no rate limiting per query cost; resolvers that don't batch (the N+1 problem at the resolver layer, solved by DataLoader).

### Persisted queries

For controlled clients, persist queries server-side (clients send an operation ID rather than the raw query). Recovers REST's HTTP-cache benefits and reduces wire size.

---

## Library / SDK APIs

### Public surface vs internal surface

Distinguish *public* (committed, versioned, documented) from merely *visible* (exported but not promised). The "Published Interface" pattern (Fowler): just because a symbol is `pub` / `export` / `public` doesn't mean it's part of the contract.

**Flag**: large public surface with no clear "this is the contract" boundary; absent `@internal` / `@experimental` / `#[doc(hidden)]` markings on symbols that aren't intended as contract; SDK exports that mirror the database schema 1:1.

### Breaking changes in libraries

Per `~/.claude/rules/typescript.md`, `~/.claude/rules/rust.md`, and Bloch's *Effective Java*:
- Removing a public function, method, class, type, or constant.
- Changing a signature (parameters, types, return type).
- Narrowing an input type (was `string | number`, now `string`).
- Widening a return type when callers exhaustively pattern-match.
- Changing thrown exception types (when callers catch specific types).
- Changing default values for parameters.
- **Semantic changes without signature changes**: the function now sometimes does I/O; now caches; now throws on previously-tolerated input.
- Changing `Send`/`Sync` bounds in Rust (or thread-safety guarantees in other languages).
- Tightening or loosening immutability guarantees on returned objects.

**Flag**: any of the above without a semver-major bump or a deprecation cycle.

### Semver discipline

For libraries with semver-versioned releases (most package-manager-published code):
- **MAJOR** = breaking changes (anything in the list above).
- **MINOR** = additive (new symbol, new optional parameter).
- **PATCH** = bug fixes, no surface change.

**Flag**: a "minor" version bump that includes a removed export; a "patch" that changes a default; a `1.x.x` library that's been making breaking changes via patch versions for a year (signal: the team hasn't internalized semver).

### Deprecation lifecycle

A complete deprecation:
1. **Annotate**: `@deprecated since=X.Y removal=A.B reason="..."` (language-appropriate equivalent). Compiler warns.
2. **Document**: changelog entry, migration guide, replacement named.
3. **Maintain through the deprecation window** (typically two majors, or 6-12 months for HTTP APIs).
4. **Remove in the named major.**

**Flag**: `@deprecated` with no `note` / `since` / `replacement`; deprecation messages that don't tell you what to use instead; removed deprecations that didn't go through annotation first.

### Builder, factory, smart-constructor patterns

For APIs with many optional parameters or invariant-requiring construction:
- **Named / keyword arguments** where the language supports them (Python, Kotlin, Swift, C#).
- **Builder pattern** (Bloch's Item 2 in *Effective Java*) for languages without -- Java, older C++.
- **Typestate builders** (Rust): compile-time enforcement of "must call `with_name` before `build`."
- **Smart constructors** (Haskell, Rust `try_new`): private constructor + validated factory function returning `Result<T, E>`.

**Flag**: constructors with 5+ parameters; constructors that accept invalid combinations; functions whose argument order is non-obvious (`copy(src, dst)` vs `copy(dst, src)` -- both exist in the wild).

---

## CLI design

Often forgotten in API discussions but among the most rigid contracts in practice (`kubectl`, `aws`, `git`, `docker` are all maintained with deep care).

### CLI breaking changes

- Removing a flag.
- Renaming a flag.
- Changing a flag's default value.
- Changing positional argument order.
- Changing the format of stdout output (scripts parse it).
- Changing exit codes (scripts check `$?`).
- Removing or renaming a subcommand.
- Changing where the tool reads config from.
- Changing the format of generated files.

**Flag**: stdout output format changes without an opt-in flag (`--output json` is the right pattern for adding a new format); exit-code reuse for different conditions.

### Conventions

- **Long and short flags**: `--verbose` and `-v`. Be consistent; not every flag needs a short form.
- **`--help` is required.** Generated from the parser, ideally; never hand-written and rotted.
- **Subcommand structure**: `tool action target` reads well (`git commit`, `kubectl get pods`). Resource-oriented analog of REST.
- **Output formats**: `--output json` for machine consumption; default human-readable. Document both.
- **Exit codes**: `0` success, `1` general error, `2` misuse, conventional `64-78` for sysexits.h categories. Document.

### CLI vs config file vs env var

Three places config lives. Precedence is part of the contract:
- CLI flag overrides
- environment variable overrides
- config file overrides
- compiled default.

**Flag**: precedence orders that disagree across subcommands; secrets accepted via CLI flag (visible in `ps`); config that silently differs based on which file the tool found first.

---

## Cross-cutting patterns

These transfer across HTTP / gRPC / GraphQL / library / CLI and are higher-leverage findings than paradigm-specific ones.

### Identifiers

- **Opaque** (UUID, ULID, Stripe-style `cus_XYZ123`): consumer infers nothing; safe for public surface.
- **Structured** (sequential integers, paths): debuggable but enables enumeration attacks and exposes counts.
- **Stripe's prefix pattern**: `cus_`, `ch_`, `pi_` -- two-part `prefix_opaque`. Trivial to identify resource type; trivial to grep logs.
- **ULID / UUIDv7**: sortable by creation time; useful for time-series.

**Flag**: sequential integer IDs in public APIs (`/users/1`, `/users/2`); opaque tokens that decode to internal data; missing ID prefix conventions in a large surface.

### Time and timestamps

- **Always RFC 3339** (`2024-06-20T15:30:00Z` or with explicit offset). Never naive.
- **Or Unix epoch integers** (Stripe's choice): no timezone ambiguity; trivial to compare.
- **Document the timezone** even if always UTC.
- **Never accept dates without timezones** in inputs.

**Flag**: date-only fields without timezone (`2024-06-20` -- which midnight?); mixing epoch and ISO across the surface; client-required clock sync (rejecting timestamps outside ±N minutes) on flaky-clock environments.

### Money

- **Never floats.** `0.1 + 0.2 != 0.3` is poison for accounting.
- **Minor units + currency code**: `{amount: 1099, currency: "USD"}` for $10.99.
- **ISO 4217 currency codes** (`USD`, `EUR`, `JPY`).
- **Zero-decimal currencies exist** (JPY). `amount: 1000` for ¥1000.
- **Three-decimal currencies exist** (JOD, KWD). Don't hardcode two.

### Filtering, sorting, searching

- **Few flat fields** (`?status=active&created_after=2024-01-01`): simple, limited.
- **Standard filter language** (AIP-160 CEL-flavored, OData `$filter`): standardized, more complex.
- **Search subresource** (`POST /users/search` with structured body): avoids URL length limits.

The recurring anti-pattern: roll your own DSL, reimplement a subset of SQL badly. **Flag**: undocumented query syntax; no max-depth / max-complexity guards.

### Sparse fields / expansion

JSON:API `?fields[user]=name,email&include=orders`; Stripe `?expand[]=customer`; GraphQL native; OData `$select` / `$expand`. Recovers some of GraphQL's flexibility for REST. Costs: caching and rate limiting become harder.

### Tolerant reader / Postel's Law

The historical principle ("be conservative in what you do, liberal in what you accept") is now contested. The modern critique (Allman, IETF): tolerant readers normalize spec violations, causing long-term interop and security bugs.

Pragmatic 2026 position: **tolerate clearly equivalent variations** (whitespace, field ordering, header case); **be strict about semantically meaningful variations** (unknown enum values, mistyped fields). Document the tolerance boundary.

**Flag**: a parser that silently accepts unknown fields without recording or warning (Hyrum's-Law trap -- senders start depending on the tolerance); strictness that varies between endpoints.

### Expand / contract migrations (parallel change)

The four-step pattern for any breaking-shaped change:
1. **Expand**: add the new schema / field / endpoint alongside the old. Both work.
2. **Migrate readers**: update consumers to use the new shape.
3. **Migrate writers**: update producers.
4. **Contract**: remove the old.

The pattern works for HTTP, gRPC, GraphQL, library APIs. **Flag**: changes that bundle steps 1 and 4 ("renamed `foo` to `bar` in one commit"); changes that skip step 2 (consumers never migrated, removal breaks production).

### Consumer-driven contract testing

Pact / Spring Cloud Contract: consumers declare what they use; providers test against the declarations in CI. Catches breaking changes before deploy.

**Flag**: services with many internal consumers and no contract test layer; contracts that go stale (consumers don't update them).

### AI-agent consumers

A genuinely new constraint (~2023+). LLM-as-API-client implies:
- **Clear, machine-readable schemas** (OpenAPI / GraphQL SDL).
- **Self-describing errors** (the agent recovers without human help).
- **Idempotency on every mutation** (agents retry liberally).
- **Conservative required parameters; generous examples in descriptions.**
- **Side effects clearly named in tool descriptions** (the agent will call destructive endpoints).
- Emerging standards: Anthropic's Model Context Protocol (MCP); OpenAI's function-calling schemas.

**Flag**: APIs designed for human SDKs only, exposed to LLM tool-calling without review for side-effect labeling, idempotency, or error self-description.

---

## The anti-pattern catalog

Signal-to-noise matters; this is the high-yield list:

- **God endpoint**: one endpoint with 30 query params doing everything. Sign of missing resource modeling.
- **Anemic CRUD over DB schema**: leaks storage shape; couples API to migration; violates bounded-context principle.
- **Tunneling through POST**: RPC over POST when a custom-method endpoint would be honest.
- **Implicit ordering**: list endpoint with unspecified order; clients depend on it; you can't change it.
- **String enums**: accepting arbitrary strings where typos should error.
- **Boolean fields that grow**: `is_active`, `is_deleted`, `is_archived`, `is_pending` instead of `status: enum`. The booleans drift out of sync.
- **Mutation via GET**: counter increments via GET; caching breaks them, browsers prefetch them.
- **Reusing 200 for everything**: errors embedded in 200 bodies; defeats HTTP-aware tooling.
- **Inconsistent naming**: camelCase mixed with snake_case in the same surface.
- **Pagination that doesn't compose with filtering**: filter applied per-page, results wrong.
- **Webhooks without signing**: forgeable.
- **API keys in URLs**: logged everywhere; leaked via referrer headers.
- **Polling-only APIs with no webhook alternative**: forces clients to choose between staleness and load.
- **Required client clock sync**: brittle.
- **Date-only with no timezone**: ambiguous.
- **Hyrum's-Law denial**: "we may change this at any time" then surprised when clients depend on it.
- **HTML in JSON fields**: forces clients into HTML rendering; XSS surface.
- **Inconsistent error envelopes**: validation errors one shape, auth errors another, server errors a third.
- **`null` vs missing vs empty string vs zero**: undefined absence semantics.
- **Top-level array responses**: `[...]` instead of `{data: [...]}` -- can't add pagination or metadata later.
- **Versions in headers AND URLs simultaneously**: which wins?
- **HTTP method override** (`X-HTTP-Method-Override: DELETE`): historical workaround now just confusing.
- **Synchronous huge responses**: 10 MB of JSON when SSE / chunked would let clients process incrementally.

---

## What is NOT an api-design finding

Signal-to-noise. Don't flag:

- **Style choices the team has deliberately made and applies consistently.** Snake_case vs camelCase, plural vs singular, status code 200-vs-204 for DELETE: pick one, stick to it; the choice doesn't matter as much as the consistency.
- **Philosophical preferences without a concrete consumer impact.** "You should use HATEOAS" is not a finding unless the absence is causing observable problems.
- **Internal-only APIs with controlled clients and no semver commitment.** A function `foo()` used by three modules in one repo doesn't need a deprecation window.
- **Already-shipped breaking changes you can't undo.** If the breaking change is in production, the finding is "document the migration"; not "revert it."
- **Documentation gaps** -- those are the `documentation` agent's lens.
- **Security / threat-model concerns** -- those are the `security` agent's lens. (Cross-reference with `See also: security`.)
- **Performance characteristics of the API** -- those are the `performance` agent's lens.
- **Schema evolution at the storage layer** rather than the API layer -- that's `distsys-data`.

---

## Severity calibration

Using `panel-contract.md`'s rubric; the API-design-specific calibration:

- **blocker**: a breaking change shipping without versioning or deprecation; an idempotency-key mishandling that double-charges; a webhook with no signing on a public endpoint; a Protobuf field-number reuse.
- **major**: a missing idempotency mechanism on a mutating endpoint; a versioning strategy that doesn't have a deprecation policy; a god endpoint that mixes resources; an inconsistent error envelope; offset pagination on a growing list; a public API surface 1:1 with the DB schema.
- **minor**: inconsistent naming within a service; suboptimal status code choice; missing rate-limit headers; an ambiguous PATCH format.
- **nit**: missing CLI short-flag; a doc string that doesn't name the return shape.
- **insight**: structural -- "this surface has accreted three error envelope shapes; consider standardizing"; "the date-versioning translation layer is reaching its complexity limit; consider major-version cutover."

Confidence: high when the trigger path is concrete (a specific endpoint, a specific field number reuse, a specific signature change); medium when reasoned from convention (an inconsistency where the team's intent isn't documented).

---

## Process for the api-design agent

1. **Identify the surface(s) under review.** HTTP? gRPC? GraphQL? Library? CLI? Multiple?
2. **Read the project's API conventions.** `CLAUDE.md`, `docs/api.md`, `docs/conventions.md`, `openapi.yaml`, `.proto` files, any style-guide rules in `CONTRIBUTING.md`. Project conventions override generic principles.
3. **Walk the per-paradigm rules** for each surface (REST / gRPC / GraphQL / library / CLI sections above).
4. **Walk the cross-cutting checks**: identifiers, timestamps, money, pagination, errors, idempotency, versioning, deprecation.
5. **Pattern-match against the anti-pattern catalog.**
6. **For breaking-change findings, name the consumer impact concretely.** "This field rename will break any client that reads `user.fullName`; the v3 SDK and the mobile app both reference it" beats "this is breaking."
7. **For style / consistency findings, cite the inconsistency.** "This endpoint uses camelCase while `/orders` and `/users` use snake_case" beats "naming inconsistent."
8. **Stay read-only.**
