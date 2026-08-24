---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Observability Patterns

Reference for OpenTelemetry (OTel) Collector design, sampling strategies, cardinality management, and pipeline reliability. Used by the `otel-pipeline` subagent.

---

## Deployment Topology

### Agent + gateway is the default production shape
A near-host **agent** (sidecar or DaemonSet) collects locally and forwards via OTel Protocol (OTLP) to a centralized **gateway** that does the heavy processing.

**Why:** the agent gives you cheap local buffering, host-level resource attribution (pod, node, container), and isolation from app failures (apps export to localhost). The gateway is the policy choke point: tail sampling, multi-tenant routing, fan-out, redaction.

**Review flag:** apps exporting OTLP directly to a hosted backend. You lose local buffering on backend outages, every app re-implements retry/queue config, and you can't change export topology without redeploying every service.

### Tail sampling forces a gateway and forces sticky routing
Tail sampling needs every span of a trace to land on the same collector instance, because the sampling decision is made once the trace is "complete." That means either (a) a single gateway, (b) a consistent-hash load balancer keyed on `trace_id` in front of the gateway pool, or (c) a two-tier setup where tier 1 routes by `trace_id` to tier 2.

**Review flag:** tail sampling configured with a round-robin or random load balancer in front. Half the trace lands on instance A, half on instance B, neither sees the full trace, the policy can't fire correctly.

---

## Pipeline Structure

### Receivers -> Processors (ordered) -> Exporters
Pipelines are signal-typed (`traces`, `metrics`, `logs`). Multiple pipelines can share components but each pipeline has its own processor chain.

### Processor order is load-bearing, not cosmetic
The ordering rules that consistently bite:

- **`memory_limiter` first.** It must see and reject load before downstream processors allocate.
- **Transformation / enrichment before sampling.** Tail sampling policies often inspect attributes; if you redact or rename after sampling, your policies see the wrong shape. Conversely, if a policy keys on an attribute that gets dropped later, that's fine.
- **Redaction before export.** Obvious in principle, easy to break when adding a new exporter to a pipeline that previously redacted only for one destination.
- **`batch` last.** Batching is for export efficiency. Batching before a processor that drops data wastes work; batching before a processor that adds attributes risks attribute reassignment confusion.

**Review flag:** `batch` placed mid-pipeline, or `memory_limiter` missing entirely. Both are concrete signs the config was assembled by copy-paste without reading the chain.

### Separate pipelines per signal, and per tenant when policy diverges
One pipeline per signal type is standard. The richer pattern: per-tenant pipelines fed by a `routing` processor on `tenant.id` or `deployment.environment` -- each tenant gets its own sampling policy, redaction rules, export destination.

A side **debug pipeline** fanning out to a `file` exporter alongside the production pipeline is invaluable during incidents and config changes. Cheap to add, easy to forget.

---

## Processors That Matter

### `memory_limiter`: the only thing standing between a load spike and OOM
Without it the collector accepts data until the process dies, taking all in-flight telemetry with it. Tune `check_interval`, `limit_mib`, and `spike_limit_mib` to your container limits.

**Review flag:** any production collector config that omits `memory_limiter` from every pipeline.

### `batch`: nearly universal, but tunable
Tune `send_batch_size` and `timeout` against backend ingestion preferences and your latency budget. Smaller batches mean lower export latency but more network overhead; larger batches mean spikier load on the receiver side.

### `transform`: the OTel Transformation Language (OTTL) workhorse
`transform` with OTTL has subsumed a lot of what older processors did: derive attributes, mutate values, drop conditionally, redact via pattern. Prefer `transform` over `attributes` + `span` + `metricstransform` stacks for new configs.

**Review flag:** a chain of three or four single-purpose processors doing what one `transform` block could express.

### `filter`: drop early, drop cheaply
Drop noise (health checks, liveness probes) at the agent, before it ever hits the gateway. Cheapest possible cardinality control.

### `tail_sampling`: powerful, expensive, gateway-only
Buffers spans by `trace_id` until a decision window expires (default 30s), then evaluates a list of policies in order: `always_sample`, `status_code`, `latency`, `string_attribute`, `numeric_attribute`, `rate_limiting`, `composite`, `and`. First policy that matches wins.

**Review flag:** tail-sampling decision window shorter than your p99.9 trace duration -- you'll sample traces incomplete and miss the slow ones, which is exactly the population you wanted to keep. Also flag: no `rate_limiting` policy at the end, so a sudden burst of errors can blow up your sampled volume.

### `probabilistic_sampler`: head-based, simple, blind
Sampling decision at trace start based on a hash of `trace_id`. Cheap. Predictable. Cannot keep "all errors" because it doesn't know what's an error yet.

### `k8sattributes`: free enrichment in Kubernetes
Attaches pod, namespace, node, deployment, container metadata to every signal.

### `redaction`: PII / secret scrubbing
Allowlist mode (only keep listed attributes) is safer than denylist. Pattern-based redaction (credit-card, email regex) is best-effort -- defense in depth, not the primary control.

**Review flag:** denylist redaction with no schema review. Anything not on the denylist leaks.

### `resource`: tag the source
Attaches `service.name`, `deployment.environment`, `service.version` to all signals. Configure at the agent so apps don't carry this responsibility.

---

## Sampling Strategies

### Head-based: cheap, predictable, blind to outcome
Decision at trace start. Hash `trace_id`, keep if hash < threshold. Parent-based variant: if your parent span was sampled, you are too -- this is the OTel default, and it's the right default because it keeps trace coherency across services.

**Tradeoff:** sampling 1% means you keep 1% of errors. Acceptable only when error rate is high enough that 1% is still useful, or when error budget is so loose that you don't care about per-error visibility.

### Tail-based: keep what's interesting, pay for the buffer
Decision after the trace assembles. Can keep all errors, all slow traces, all traces with a specific attribute, plus a probabilistic baseline. The Honeycomb Refinery model. Requires sticky `trace_id` routing as noted above.

**Cost:** memory to buffer in-flight traces. A 30-second decision window at 100k traces/sec and 10 spans/trace means buffering 30M spans. Real money in RAM.

### Adaptive / dynamic sampling: ratchet by key
Per-key (typically `(service, endpoint, status_code)` or `(customer_tier, endpoint)`) sample-rate that adjusts to volume: high-volume boring tuples get heavy sampling, rare tuples get kept at high rates. Honeycomb's dynamic sampling popularized this; the same shape can be implemented in OTel with `tail_sampling` + a composite policy plus rate limiting, though it's coarser than dedicated tools.

**Review flag:** flat probabilistic sampling on a workload with extreme key-skew (one customer is 90% of traffic). You'll oversample the heavy customer and undersample everyone else.

### The universal pattern: always errors, always slow, baseline probabilistic
Three policies, OR'd together:
1. Keep all error traces (`status_code = ERROR`).
2. Keep all slow traces (`latency > SLO threshold`).
3. Keep N% of everything else.

Cheap to implement, hard to regret. Most production sampling configs are this plus a rate-limit cap.

### Importance weighting
Weight by customer tier, request value, internal-vs-external. Encode as an attribute at the app boundary; let `tail_sampling` key on it. Don't try to put business logic in the sampling config itself.

### Sample traces as a unit, never spans within a trace
Sampling individual spans inside one trace breaks the waterfall in your trace viewer and produces meaningless aggregate metrics. The unit of sampling is the trace.

**Review flag:** per-span sampling decisions, or any sampling logic that doesn't key on `trace_id`.

---

## Cardinality Management

### Each unique label combination is a separate time series
For traditional time-series databases (Prometheus, Mimir, Cortex), this is the core cost driver. A metric with labels `{service, endpoint, status_code, user_id}` where `user_id` ranges over a million users creates a million time series per `(service, endpoint, status_code)` triple.

**Rule of thumb:** stay under ~10k unique label combinations per metric. Bound any label that could be free-form with an explicit allowlist (`status_code in {200, 4xx, 5xx}` instead of raw codes; `endpoint` from a registered route table, not from URL path).

**Never label with:** `request_id`, `trace_id`, raw `user_id`, raw URL, raw user-input strings, IP addresses, session IDs, anything customer-controlled. These belong on events or spans, not metrics.

### High cardinality is fine for events/traces, fatal for metrics on TSDB backends
Honeycomb / wide-event systems are built for unbounded label combinations; query-time aggregation lets you slice by anything. Prometheus is built for low-cardinality pre-aggregated counters with bounded labels. Don't confuse the two regimes.

**Review flag:** "let's just add `user_id` as a metric label so we can query by user." That's a trace/event use case, not a metric use case.

### Exemplars link metrics back to traces
Prometheus exemplars (and OTLP equivalent) attach a `trace_id` to a histogram bucket sample. You keep your bounded-label metric and still have a one-click path to a representative trace.

### Histogram bucket boundaries: put a boundary AT your SLO
For a fixed-bucket histogram, the bucket boundary you actually care about is your SLO threshold. If your SLO is "p99 < 300ms", you want a bucket boundary at 300ms -- otherwise `histogram_quantile` interpolates between buckets that straddle your threshold and your SLO query lies.

Exponential histograms (OpenHistogram-style) eliminate this tuning problem entirely. Prefer them when the backend supports them.

---

## Exporters and Backend Choice

### OTLP is the wire format; everything else is legacy
OTLP gRPC (default) and OTLP HTTP both work. Every modern backend accepts OTLP. Vendor-specific exporters (Datadog, New Relic) exist but lock you in -- prefer OTLP when the backend supports it, which most do.

### Multi-export for migrations and dev/prod splits
Same pipeline can fan out to multiple exporters: send production to backend A, send a sampled copy to backend B during a migration, and dump to a `file` exporter for local replay. Cheap insurance.

**Review flag:** multi-export with no `sending_queue` per exporter -- a slow backend backpressures the fast one.

---

## Pipeline Reliability

### The collector is on the critical path
If the collector dies, telemetry goes dark exactly when you need it. Hard requirements:

- `memory_limiter` on every pipeline.
- `sending_queue` per exporter with bounded `queue_size`.
- `retry_on_failure` with sane backoff -- not infinite retries on a poison batch.
- **Persistent queue** via the `file_storage` extension for survival across restarts. Without it, the in-memory queue is gone on restart.

### Telemetry of telemetry
The collector exports its own metrics (`otelcol_process_*`, `otelcol_receiver_*`, `otelcol_exporter_*`). Service-level objective (SLO) the collector itself: ingest rate, queue depth, export success rate, refused-spans count. If you can't SLO your collector you can't trust your observability.

**Review flag:** no scrape / no dashboard on `otelcol_exporter_send_failed_*`. You won't notice silent telemetry loss until an incident.

### The telemetry-tax problem
At scale the observability bill rivals the infrastructure bill it observes. Sampling discipline (tail sampling with rate limits) and cardinality discipline (bounded labels, exemplars instead of high-cardinality metric labels) are the only two real levers. Vendor pricing is rarely the lever; what you send is.

**Review flag:** "we'll fix the bill by negotiating with the vendor." The negotiation is downstream of the pipeline design.
