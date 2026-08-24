---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Observability Principles

The load-bearing principles for instrumenting, operating, and debugging production systems. Companion to `observability-patterns.md` (which covers Collector, sampling, cardinality, exporters). Used by `/observability-review`, `/observability-design`, and the `otel-instrumentation` / `otel-pipeline` / `observability-practice` subagents.

Honeycomb-aware (the user works there) but the principles aim to be backend-agnostic. Where schools of thought genuinely disagree, both are preserved -- observability has the strongest cultural component of any domain in these rules, and different questions need different answers.

## The cultural argument

### Observability is not monitoring with extra steps

Monitoring asks predicted questions ("alert when CPU > 90%"); observability lets you ask new ones ("why is this customer's deploy 10x slower than the prior one"). Monitoring works for known failure modes; observability works for the combinatorial space of customer × deploy × region × feature-flag × build that nobody can pre-declare.

If your debugging workflow is "I had a hunch, then I checked the dashboard that confirmed my hunch," you're in monitoring. If it's "I had no hunch, so I sliced the data along dimensions I didn't pre-declare and found the answer," you're in observability.

**Flag**: a team adding a new alert after every incident. Alert count grows monotonically. Nobody can explain what "normal" looks like anymore. Monitoring metastasizing in place of observability.

**When monitoring is enough**: static systems with a small, well-understood failure surface (single-service CRUD apps, batch jobs with deterministic inputs). Don't over-engineer.

### The "three pillars" framing is partly misleading

The Charity Majors critique: metrics, logs, and traces are not three independent things; they are three projections of the same substrate (structured events). A trace is a connected graph of events; a metric is an aggregate over events; a log is an event with a message. Treating them as separate pillars leads to three tools, three bills, and timestamp-correlating across three data stores at 3am.

The OTel position is more pragmatic: support all three as first-class signals, let the team decide the balance. OTel doesn't commit to a philosophy -- which means it doesn't push back on architecturally-poor choices.

**Where the pillars-critique holds**: application debugging in complex distributed systems where the interesting failures are combinatorial across user / deploy / flag / region.

**Where the pillars are pragmatic**: at extreme scale (hyperscaler infra metrics, billions of cardinality points per minute), storing every raw event is infeasible. Pre-aggregation is forced engineering.

### High cardinality is the resolution of observability

If you can't slice by user_id, request_id, build_sha, customer_id, plan_tier, feature_flag_state, you don't have observability -- you have aggregates that hide the signal. The interesting questions in production are almost always cohort-shaped: "what happened to customer X after deploy Y on region Z with feature flag W enabled?"

Traditional metrics systems (Prometheus, StatsD, Mimir, Loki labels) charge exponentially for cardinality because each unique label combination becomes a separate time series. Event-based systems (Honeycomb, Lightstep) make cardinality free at the storage layer and pay at query time.

**Where high cardinality is the point**: span attributes, structured log fields, Honeycomb-style event stores. Add everything you know.

**Where high cardinality is fatal**: Prometheus metric labels, Loki stream labels, classic TSDB tag dimensions. Keep label cardinality bounded (≤ 10k combinations per metric); never put `user_id`, `request_id`, `trace_id`, or unbounded user input in a metric label.

**The bridge between regimes**: exemplars. Metrics with low-cardinality labels, exemplars on data points linking back to one trace with full high-cardinality detail. Severely under-used.

### Sampling preserves shape; aggregation at write time destroys it

Sampling (keep 1 in 100 success events, 100% of errors) lets you still answer questions about the population with known statistical error bars. Aggregation at write time (compute p99 per minute, store only that) is a one-way function -- you can't recover the underlying distribution; you can't ask "what was the p99 for customer X" after the fact.

This is the load-bearing technical decision behind Honeycomb's architecture: store raw events, compute aggregates at query time.

**Flag**: pre-computing percentiles in the agent "to save storage." Optimized for storage at the cost of every future question you didn't know to ask.

**When aggregation is right**: long-term retention (months to years) for capacity planning and trend analysis. The pattern: hot raw events for 30-60 days, then downsampled rollups for the long tail.

## OpenTelemetry essentials

### The three signals are independent but correlatable

Traces (causal chains, what one request did), metrics (aggregate behavior over time), logs (discrete events with context). OTel treats them as separable -- you can emit one without the others -- but their value compounds when correlated via trace context.

**Use traces** for "what did this request do across services" (the causal model).
**Use metrics** for "how much / how many / how slow" (aggregate trends, SLO/alert input).
**Use logs** for "what happened here with all the surrounding state" (discrete events).

**Overlap is real**: a span event resembles a log; a counter could be derived from spans. The spec leaves the choice to the user.

**Flag**: a debugging session that starts in metrics, hops to logs, hops to traces, manually correlating timestamps. Each tool has a different ID space. The team has accepted high coordination cost as the price of "best-of-breed."

### Span lifecycle: create, activate, attribute, end exactly once

A span is created from a `Tracer`, made current (activated in context), populated with attributes/events/status, then ended exactly once. Ending twice is undefined; mutating after end is undefined. Parent/child relationships are determined at creation from the active context -- create a span before activating its intended parent and you get an orphan.

**Flag**: missing `defer span.End()` / `try-finally` / `using` patterns. Attributes set after `End()`. Spans created without context activation, then children that miss the parent link. Async work where context doesn't propagate.

### Span kinds matter for waterfall correctness

Five kinds: `INTERNAL` (default), `SERVER`, `CLIENT`, `PRODUCER`, `CONSUMER`. Backends use kind to render waterfalls and compute RED metrics. A `SERVER` span's parent is typically a remote `CLIENT` span; the network gap is real elapsed time.

**Flag**: HTTP/gRPC/DB client calls left as `INTERNAL` -- flattens the waterfall. Server handlers not labeled `SERVER` -- breaks root-of-service detection. Async work labeled `INTERNAL` when it crosses a queue boundary.

### Attributes vs events vs span status

- **Attributes**: "what was this span about" -- request method, user ID, query shape. Span-level metadata.
- **Events**: timestamped points within the span's duration with their own attributes -- "something notable happened mid-span" (cache miss, retry attempt, GC pause). Events cost more than attributes.
- **Exceptions** are a specific event type (`exception` event with `exception.type`, `exception.message`, `exception.stacktrace`). Recording an exception does **not** automatically set span status to `Error` -- you must do both.

**Status**: `Unset` (default, the spec's preferred non-decision), `Ok` (explicit success override), `Error`. Don't always set `Ok` at the end of a function -- defeats the spec's design.

**Flag**: recording an exception by setting attributes instead of using `recordException` / `RecordError`. Always setting `Ok` at function end. Using events for low-cardinality categorical data that should be an attribute.

### Resource attributes describe the producer

`service.name` is the only **required** resource attribute. Strongly recommended: `service.version`, `service.namespace`, `service.instance.id`, `deployment.environment.name` (renamed from `deployment.environment`). These attach to every span, metric, and log from the process -- not span attributes; duplicating them onto spans wastes payload.

**Flag**: setting `service.name` per-span. Missing `deployment.environment.name` -- can't separate prod from staging. Copying resource attributes onto spans "to make them searchable."

### Context propagation: W3C Trace Context is the default

`traceparent` carries trace-id, span-id, and the sampled flag. `tracestate` carries vendor-specific data. Without propagation, every remote call starts a new trace.

Async contexts (goroutines, promises, threads, queue handlers) require explicit context handoff. Many bugs come from assuming thread-local context survives a background job.

**Flag**: manual header construction instead of using a propagator. Missing instrumentation on outbound HTTP clients (breaks the chain even if inbound works). Background jobs that don't extract context from the message. Using only B3 in a greenfield system -- W3C is the default; B3 is legacy compatibility.

### Baggage is propagated but not auto-attributed

Baggage is for cross-cutting context (tenant ID, feature flag) propagated across hops. It is **not** automatically added to spans as attributes; that's a deliberate choice (some SDKs have a span processor that does it). Baggage is visible to every hop including third parties -- never put secrets, auth tokens, or PII in baggage.

### Metric instruments: pick by semantics

- **Counter** -- monotonically increasing (request counts, bytes sent).
- **UpDownCounter** -- allows decrement (active connections, queue depth).
- **Histogram** -- distributions (latency, payload size). Prefer **exponential histograms** over explicit-bucket; auto-scaled, accurate percentiles across orders of magnitude, compress better.
- **Gauge** -- instantaneous values (CPU temp, memory in use).
- **Observable** versions of Counter / UpDownCounter / Gauge are callback-driven; the SDK invokes at collection time.

**Flag**: `Counter` for a value that can decrease. `Gauge` for a cumulative quantity. Custom explicit buckets like `[0.1, 0.5, 1, 5]` on a latency histogram. Polling in a loop to record a gauge when an observable callback would work.

### Sampling: ParentBased(TraceIdRatioBased) is the standard

Pure `TraceIdRatioBased` without `ParentBased` wrapper breaks distributed sampling -- some services in a trace sample, others don't, you get partial traces. ParentBased respects the upstream sampled flag. Tail-based sampling ("keep all errors, 1% of success") requires collector-side logic; the SDK can't do it because the decision must propagate at span start. See `observability-patterns.md` § Sampling Strategies for the full treatment.

## SLOs and error budgets

### 100% reliability is the wrong target

Above some threshold, users cannot perceive additional nines because their own clients, networks, and devices are less reliable. Driving toward 100% costs disproportionately and leaves no slack for change -- and change is the primary source of outages.

Every service needs an explicit SLO **below 100%**. If a team cannot name theirs, the unstated target is 100% and any work to improve velocity will be perceived as recklessness.

### SLI, SLO, SLA are three different things

- **SLI** is the *measurement* (a ratio of good events to total events).
- **SLO** is the *target value* of that ratio over a window.
- **SLA** is the *contract* with external consequences if missed.

**Flag**: "our SLA is 99.9%" without a contractual penalty clause -- they almost certainly mean SLO. Push for clarity; the math and cultural weight differ.

### Good SLIs are user-visible, ratio-shaped, simple

Canonical form: `good_events / valid_events`, ranging 0-100%. Composes cleanly with error budgets, dashboards, burn-rate alerts. Bespoke shapes (means, raw counts, weird percentiles) break tooling and arguments.

**SLI patterns by component**:
- **Request-driven**: availability (success / total), latency (requests fast enough / total -- ratio, not percentile target)
- **Pipeline**: freshness (`reads where data was less than N minutes old`), throughput, correctness
- **Storage**: durability (separate, longer windows), availability, throughput
- **Background workers**: lag (oldest unprocessed age) as a fraction-of-minutes-with-acceptable-lag

**Flag**: SLIs aggregated as means. Latency SLOs expressed as "p99 < 300ms" rather than "requests under 300ms / total" -- the percentile target tells you nothing about *how often* the tail fails.

### Separate SLI specification from SLI implementation

The specification is *what you'd measure if instrumentation were free* ("home page loads in under 1s from the user's perspective"). The implementation is *the actual telemetry source* ("ratio of server-side request durations under 1s, measured at the load balancer"). Document both. When reviewing, ask "what's the specification this implementation is a proxy for, and where does the proxy lie?"

### Error budgets are for change, not just breakage

`Budget = (1 - SLO) * window`. The budget is the currency teams spend to ship features, run experiments, do migrations, and absorb genuine incidents. Treating it as a breakage-only allowance leads to either freezes (when there's spendable budget) or recklessness (when teams ship through an exhausted budget because "this isn't really breakage").

**The error budget policy must specify both boundaries**: what happens when exhausted (freeze launches, redirect to reliability work), and when healthy and unspent (ship more aggressively, fund experiments).

### SLA buffer: SLA < SLO

If you sign a 99.9% SLA, set internal SLO to 99.95%. Buffer absorbs measurement skew, the fact that internal measurements are usually rosier than the customer's, and the cost of being wrong. SLA = SLO is a planning bug.

### One service, a small number of SLOs

Typically one availability, one latency, occasionally correctness or freshness. More than four or five per service means none drives behavior.

**Flag**: a service with ten SLOs -- ask which two would change next week's prioritization. The rest are vanity.

## Alerting

### Page on symptoms, not causes

A symptom is user-visible ("users can't check out"). A cause is diagnostic ("DB connection pool at 90%"). Pages on causes train the on-call to ignore pages, which is the real failure mode. Pages on symptoms wake someone up when something is actually wrong, and the cause is what they investigate.

**Page-worthy criteria, all three required**: human must act now, problem is real (not transient), action exists. If any is missing, it's a ticket or a dashboard, not a page.

**Flag**: "we leave this page in place so we know about it" -- belongs as a ticket. Pages whose standard response is "ack and go back to sleep, it'll recover" -- trained out of relevance.

### Burn-rate alerting beats threshold alerting

Burn rate is dimensionless: rate 1 means you'll exhaust the window's budget exactly at window end; rate 14.4 means you exhaust 2% of a 30-day budget in 1 hour. The math: `time_to_exhaustion = (1 - SLO) / current_error_rate * window`. Alert threshold encodes *how much budget you're willing to spend before paging*.

A "5xx rate > 1%" threshold has no relationship to SLO and produces noise + late detection simultaneously. Burn-rate alerts directly express "we're consuming the budget faster than the window sustains."

### Multi-window, multi-burn-rate (MWMBR) is the canonical approach

Use at least two pairs of windows. Google's starting point for a 99.9% SLO:
- **Page**: 2% budget in 1h (burn rate 14.4) AND sustained in 5m short window
- **Page**: 5% budget in 6h (burn rate 6) AND sustained in 30m short window
- **Ticket**: 10% budget in 3d (burn rate 1) AND sustained in 6h short window

The short window (1/12 of long is a good heuristic) is an "is it *still* happening" filter that gives fast reset times. Without it, a 100% outage that ends in 2 minutes keeps firing for 6 hours.

**Flag**: burn-rate alerts with only a long window -- terrible reset behavior, ghost pages after recovery.

### Low-traffic services need different math

With 100 requests/hour, a single failure is 1% error rate; ratio-based alerts become useless. Options: aggregate to a higher level (route or tenant), synthesize traffic (probers), widen the window dramatically, or accept that this service doesn't have an SLO supporting paging -- it has a ticket SLO.

### Alert hygiene

Alerts decay; review them on a schedule. New alerts get added after incidents; old alerts rarely get retired. A monthly or quarterly alert review meeting where every alert that fired in the last period is classified as kept-actionable, retired, or rewritten.

## Structured logging

### Logs are events, not strings

A log entry is a typed record with named fields. Free-text logs are a category mistake. Once logs are structured, every line is queryable, aggregatable, joinable.

**Flag**: `logger.info(f"User {user_id} ordered {item_id}")` -- f-strings or printf-style interpolation passed to a logger. Mix of structured and unstructured on the same stream. Custom `print()` next to a structured logger.

### Pick one format per service: JSON or logfmt

JSON nests, handles arbitrary types, universal. Logfmt (`key=value`) reads naturally in a tail and stays parseable. The actual cost is inconsistency. Pick per service, ideally per org, enforce at the logger layer.

### Stdout is the transport

Per 12-factor: write event streams to stdout/stderr, let the platform collect, ship, rotate. Apps that own their log files own their disk, rotation, backpressure. Apps that write to stdout get all of that from the runtime for free, and the same code works locally, in CI, in staging, and in prod.

**Exception**: high-volume audit logs with compliance requirements often need a dedicated, durable, separately-routed sink.

### Every entry needs the same skeleton

Timestamp (ISO 8601 with offset, or Unix epoch ms -- never mix), level, message, service. **trace_id and span_id when in a request context** (critical). request_id / correlation ID for cross-service tracking. Business context (user_id, customer_id, etc.) when in user context.

### Levels: keep three, distrust the rest

ERROR / WARN / INFO are routing decisions. DEBUG and TRACE belong in dynamic sampling or feature flags. ERROR fires alerts; WARN feeds investigation; INFO is the operational record.

**Flag**: `logger.error` on a handled 404 or validation failure. DEBUG-level stack traces. `logger.debug` for anything you'd actually want during an incident (it'll be off in prod). FATAL used when a single request failed.

### trace_id in every log line is the load-bearing correlation

A log without a trace_id in a request context is observability debt. The single highest-leverage telemetry pattern is: jump from a slow trace to its logs at the affected span, and back. That pivot requires both signals to carry the same identifier.

OTel's logs SDK can emit logs natively OR bridge an existing logger (Pino, Bunyan, Winston, tracing crate, zerolog, structlog). **Bridging is almost always the right call** -- leaves your formatting and ergonomics intact while injecting trace context.

**Flag**: logging from inside a queue worker without re-extracting trace context. Async boundaries (`setTimeout`, `tokio::spawn`, `asyncio.create_task`) where context doesn't propagate. Cross-service calls where context isn't forwarded in headers.

### Don't echo-log

Log a request entering and leaving a service, not every layer in between. Spans give you the layered breakdown for free with a parent-child relationship logs don't have. Log on entry/exit at the service boundary and on the decisions that matter (state transitions, fallbacks, surprising branches).

**Flag**: the same request_id in 8+ log lines per request, all carrying the same payload. "Entered method X" / "Exited method X" patterns. Logger calls at the top of every function.

### Don't log from hot loops

A logger call inside a tight loop is silently O(n) on egress, storage, and index cost. At 10k iterations per request × 1k req/sec, that's 10M log lines per second. Aggregate to a span attribute (count, sum, max) or a metric; emit one summary line at the end.

### Redact at the logger, not the call site

Centralized redaction is the only kind that survives the next developer adding a new logger call. Field-name allowlists (only emit known-safe fields) beat denylists. Tokens, passwords, API keys, raw emails, full names, free-form user content all need redaction, hashing, or omission.

Audit logs are a separate stream with their own retention and access controls -- not just a higher level on the same pipeline.

## Methodologies

### RED for services: Rate, Errors, Duration

For every service: requests per second, errors per second, latency distribution. Simpler than USE for request/response shapes. Aligns with three of the four golden signals.

### USE for resources: Utilization, Saturation, Errors

For every resource (CPU, memory, network, disk, controllers, interconnects): how full is it, how queued/back-pressured, what errors. Saturation is queue depth / run queue length / dropped packets / swap. USE answers "is this resource the bottleneck?" -- not a replacement for SLOs.

### Four golden signals: Latency, Traffic, Errors, Saturation

Google's union of RED and USE for service-level views. Latency split by success/error (failure latency lies). Traffic = request rate. Errors = explicit and silent failures. Saturation = how close to limits.

The minimum dashboard for any user-facing service. If you can't show all four at a glance, that's the next dashboard you build before any bespoke one.

## Conflicting schools of thought (preserve, don't collapse)

The user works at Honeycomb, so the traces-first / wide-events school is the default lens here. But these schools genuinely disagree, and the right choice depends on the question:

- **Metrics-first** (Prometheus / Grafana / Mimir lineage). Position: metrics are cheap, queryable, aggregable, scale to extreme rates; cardinality is a cost to manage; traces are for deep dives. **Wins**: massive infrastructure scale where storing raw events is infeasible. Pull-based scraping gives you a known-good ingestion model with no agent gymnastics. **Fails**: the moment a developer wants to answer "what happened to customer X on this deploy."

- **Traces-first / wide-events** (Honeycomb / Lightstep lineage). Position: structured events with rich attributes are the foundation; metrics derive from events; cardinality is the point. **Wins**: application-level debugging in complex distributed systems where failures are combinatorial. **Fails**: pure infrastructure observation (host CPU, network throughput) where the cardinality is bounded and a metrics stack is simpler and cheaper.

- **Logs-first / structured-logging** (Datadog / Splunk / CloudWatch-era lineage). Position: structured logs are the universal substrate; traces and metrics derive. **Wins**: heterogeneous environments where you can't get everyone on the same trace propagation library. Logs are the lowest common denominator and always work. **Fails**: without trace context propagation, you lose the causal chain. Debugging becomes timestamp correlation.

- **OpenTelemetry pragmatic**. Position: support all signals as first-class, let the team choose the balance. **Wins**: vendor-neutral instrumentation, future-proof. **Fails**: doesn't push back on poor architectural choices; "MELT" (Metrics, Events, Logs, Traces) papers over the structured-events-vs-aggregates question rather than answering it.

Apply whichever school fits the question. Don't pretend there's one right answer; the failure mode is dogma in either direction.

## Debugging workflow

The intended workflow telemetry enables:

1. **Start at the SLO dashboard**: is something user-visible degraded?
2. **Pivot to the affected request shape**: what dimensions are correlated with the badness? (BubbleUp-style analysis in Honeycomb; PromQL `topk` in metrics-first stacks; log faceting in logs-first.)
3. **Drill into example traces**: pick a slow / failed exemplar and follow the waterfall.
4. **Jump from trace to logs at the affected span** via trace_id.
5. **Use logs for in-process detail; traces for cross-service flow.**

Starting from logs -- grepping for an error -- works but is slower, scales worse, and misses the cross-service shape entirely.

**Flag**: runbooks that start with "grep the logs for...". Dashboards built on log counts where a metric exists. Incident retros where root cause was found in logs after hours, and the trace existed but nobody pivoted to it.
