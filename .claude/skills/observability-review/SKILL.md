---
name: observability-review
description: Expert review pass for observability quality in changed code -- span design (kinds, attributes, lifecycle), context propagation, cardinality discipline, structured logging patterns, trace/log correlation, sampling configuration, metric instrument choice, SLI/SLO coverage, and the OpenTelemetry semantic conventions. Reviews the current branch diff against main by default, a specific file/PR with `/observability-review <path>` or `/observability-review <PR#>`, or a git range. Auto-routes deep questions to `otel-instrumentation`, `otel-pipeline`, or `observability-practice` subagents. Produces severity-labeled findings with file:line references. Does NOT post comments. Use when reviewing changes that add or modify telemetry: spans, metrics, logs, collector config, alert definitions, SLO config.
---

# Observability Review

You are doing an **expert-level review** for observability quality. The goal: catch telemetry that's spec-incorrect, gap-prone, or operationally fragile -- the issues that make a system harder to debug at 3am.

The reference files `~/.claude/rules/observability.md` (principles) and `~/.claude/rules/observability-patterns.md` (Collector, sampling, cardinality, exporters) are your authoritative checklist. Cross-cutting principles in `~/.claude/rules/coding-style.md` and `~/.claude/rules/testing.md` apply.

The user works at Honeycomb. Honeycomb-flavored patterns (wide events, trace-id correlation, BubbleUp-friendly cardinality, tail-sampling via Refinery) are reasonable defaults. But preserve school-of-thought neutrality where the question is genuinely contested -- the reference files cover the metrics-first / traces-first / logs-first debate; don't dogmatize.

## Scope resolution

- **No arg** -- diff between current branch and the merge base with the main branch (check the repo's CLAUDE.md; may be `main`, `master`, `staging`, `develop`). Include uncommitted changes; flag dirty tree.
- **`<PR#>`** (numeric) -- a GitHub PR. Use `gh pr diff <PR#>` and `gh pr view <PR#>`.
- **`<path>`** -- review that file or directory in full.
- **`<range>`** (contains `..` or `...`) -- review that git range.

Exclude `node_modules/`, `target/`, `dist/`, `build/`, `.next/`, `coverage/`, generated bindings, lockfiles.

## What to flag

Categories, roughly ordered by how often they actually matter:

### 1. Cardinality nightmares

- High-cardinality values (user_id, request_id, trace_id, build_sha, raw URLs with IDs/queries) used as **metric labels** or **Loki stream labels** -- explodes the time-series count, breaks the backend, gets dropped, removes the per-user view.
- Free-form user input (search queries, document names) as labels.
- Combinatorial label explosions: `(route × user × deploy × region)` -- one new label can multiply the existing count.

**Rule of thumb**: keep label cardinality bounded (≤10k combinations per metric). High-cardinality data belongs in event attributes / span attributes / log fields, not in metric labels.

### 2. Context propagation gaps

- Outbound HTTP/gRPC clients without instrumentation (breaks the chain even if inbound works).
- Background jobs / queue handlers that don't extract trace context from the message.
- Async boundaries (`setTimeout`, `tokio::spawn`, `asyncio.create_task`, goroutines, threads) that drop context.
- Manual header construction instead of using a propagator.
- B3 / Jaeger format used in greenfield code -- W3C Trace Context is the default; legacy formats are compatibility-only.

### 3. Span lifecycle bugs

- Missing `defer span.End()` / `try-finally` / `using` patterns -- span never closes, never exports.
- Attributes set after `End()` -- silently dropped.
- Spans created before context activation -- parent linkage broken.
- Re-using span objects across goroutines/threads without explicit context propagation.
- Manual span construction where instrumentation libraries would suffice.

### 4. Span kinds and semantic conventions

- HTTP/gRPC/DB client calls left as `INTERNAL` -- flattens waterfall, breaks RED calculations.
- Server handlers not labeled `SERVER` -- breaks root-of-service detection.
- Async work crossing a queue boundary labeled `INTERNAL` -- should be `PRODUCER`/`CONSUMER`.
- Legacy attribute names: `http.method` (should be `http.request.method`), `http.status_code` (should be `http.response.status_code`), `http.url` (should be `url.full`), `db.system` (should be `db.system.name`).
- `http.route` (matched template) vs `url.path` (literal) confusion -- literals explode cardinality.
- Missing `service.name` / `deployment.environment.name` on resource.
- Custom attributes in reserved namespaces (`http.custom_field`).
- Inconsistent casing (`userId` vs `user.id`); convention is `snake_case` dotted.

### 5. Attribute hygiene

- `db.query.text` with literal values instead of parameterized form (cardinality + PII).
- `url.full` logged with query strings containing API keys, session tokens, or PII.
- Nested JSON-as-string in a single attribute when values should be flattened keys.
- Recording exceptions by setting attributes instead of using `recordException` / `RecordError` (produces non-spec-conformant events).
- Recording an exception without also setting span status to `Error` -- the spec doesn't auto-set.
- Always setting `Ok` at function end -- defeats the spec's `Unset` default.
- Missing business-context attributes (user_id, customer_id, plan_tier) on user-facing spans -- you can't slice by cohort later.

### 6. Metric instrument choice

- `Counter` for a value that can decrease.
- `Gauge` for a cumulative quantity (request total).
- Custom explicit bucket boundaries on a histogram, especially when they don't include the SLO threshold as a boundary.
- Polling in a tight loop to record an observable gauge instead of using an observable callback.
- Pre-computing percentiles in the agent -- one-way function, kills future questions.

### 7. Sampling configuration

- `TraceIdRatioBased` without `ParentBased` wrapper -- breaks distributed sampling, partial traces.
- Custom samplers that don't respect the parent sampled flag.
- Tail-sampling logic written in application code instead of in the collector.
- Head-based sampling that drops all errors at low rates (1% sampling = 1% error retention).
- Missing exemplar attachment on metrics that could pivot to traces.

### 8. Structured logging gaps

- Free-text logs: `logger.info(f"User {user_id} did X")`. F-string/printf-style interpolation passed to loggers.
- Mix of structured and unstructured on the same stream.
- Custom `print()` / `console.log` next to a structured logger.
- Missing trace_id/span_id in log entries emitted during a request.
- Async-boundary logging that drops trace context.
- Inconsistent log levels: `logger.error` on a handled 404 or validation failure; `logger.debug` for things you'd want during an incident.
- Echo-logging (same request_id in 8+ log lines per request).
- Logger calls inside tight loops.
- Stack traces emitted on every handled exception (expensive, trains operators to ignore).
- PII/secrets logged without redaction.

### 9. Backend-specific footguns

For Honeycomb users (the user's context):
- Narrow spans with 3 attributes (method, path, status) -- structured logging masquerading as observability. Wide events with business context are the point.
- Manual sampling in the SDK when Refinery is in the path -- double sampling.
- Datasets sprawl (one per microservice is fine; one per *operation* is anti-pattern).

For Prometheus/Mimir:
- High-cardinality labels (as above; this stack is most sensitive to it).
- Histogram bucket choices that don't include SLO thresholds.

For Loki:
- Stream labels with high-cardinality fields (`user_id`, `request_id`).
- Indexed labels confused with structured log fields.

For Grafana Tempo / Jaeger:
- Trace context propagation gaps (same as universal); these backends don't auto-correlate without trace_id.

### 10. SLO and alert design

- Services with no SLO -- the unstated target is 100%, no error budget exists.
- SLOs expressed as instantaneous percentile thresholds (`p99 < 300ms`) rather than ratios (`requests_under_300ms / total`) -- breaks burn-rate math.
- Alerts on cause (CPU, disk, connection pool) where symptom alerts (user-visible errors, latency SLO burn) would work.
- Burn-rate alerts with only a long window -- terrible reset behavior.
- Threshold alerts (`error_rate > 1%`) instead of burn-rate alerts -- no relationship to SLO.
- Pages whose standard response is "ack and go back to sleep" -- trained out of relevance.

### 11. Collector / pipeline issues

- Collector pipelines without `memory_limiter` -- OOM under load.
- Processor order: `batch` not last, sampling before transform, redaction after export.
- Custom OTLP encoders/parsers instead of generated bindings.
- Vendor-specific exporters where OTLP would work (couples code to one backend).
- Tail-sampling configured without trace-routing that ensures all spans of a trace land on the same collector instance.
- Persistent queue not configured for collector restarts.
- No SLO on the collector itself (telemetry of telemetry).

### 12. Test gaps

- Code that adds telemetry without verifying span shape in tests (spec-conformant attribute presence, status code, span kind).
- No test that propagation crosses an async boundary in code that needs it.
- No test that confirms PII isn't leaking into attributes or logs.

## Routing to subagents

When depth is needed, delegate. Pass a self-contained prompt: snippet + question + surrounding context.

- **`otel-instrumentation`** -- SDK usage, span lifecycle, attribute conventions, instrumentation libraries (auto vs manual), context propagation, exemplars, log/trace correlation, semantic conventions. Use when the question is "is this instrumentation spec-correct and complete?"
- **`otel-pipeline`** -- Collector config, processors, exporters, sampling strategies (head/tail/adaptive), pipeline reliability, cardinality strategy. Use when the question is "is this pipeline well-designed?"
- **`observability-practice`** -- SLO design, alert hygiene, burn-rate math, postmortem culture, dashboard design, debugging workflow, the cultural arguments. Use when the question is "is this how we operate?"

## Process

Run in parallel where possible:

1. Resolve scope. Capture file list and diff.
2. Read changed files. For small files, read the whole thing -- telemetry bugs often live in surrounding code (the logger config a few lines up, the sampling config in a sibling file).
3. Check the repo's CLAUDE.md, any `otel-collector.yaml` / `prometheus.yaml` / `tracing.toml` / similar telemetry config files, and the closest CLAUDE.md for project-specific conventions.
4. Walk the categories above against the diff.
5. Route subagent-worthy questions in parallel where independent.

## Reporting

Group findings by severity:

- **blocker** -- correctness bug that will lose telemetry, leak PII/secrets, or fail the SLO (sampling that drops all errors, PII in attributes/logs, missing context propagation in a critical async boundary).
- **major** -- significant gap (high-cardinality label that will break the backend, missing business-context attributes on user-facing spans, alert on cause rather than symptom, no SLO).
- **minor** -- improvable pattern (legacy attribute name, missing exemplar, log level wrong, echo-log).
- **nit** -- naming, doc, micro-style.

Format:

```
**[severity]** `path/to/file.ts:LINE` -- short headline

<one or two sentences explaining the issue>

<optional: suggested fix or what to specify>
```

For subagent-delegated findings, prefix with the subagent name: `**[major]** [otel-pipeline] config/collector.yaml:42 -- tail_sampling configured without sticky trace routing`.

Open with: `Reviewed N files, M findings (X blockers, Y major, Z minor, W nits). Routed K hunks to specialist subagents.`

If the change is clean, "No findings worth flagging" is an honest answer.

## Quick decision references

- **Span attribute vs metric label**: span attribute always wins for high-cardinality data. Metric labels need bounded cardinality.
- **Native OTel log SDK vs bridge**: bridge an existing logger almost always. Native API for new code only.
- **Symptom vs cause alert**: page on symptoms, alert (ticket) on causes.
- **Burn-rate vs threshold**: burn-rate beats threshold for ratio-shaped SLIs; threshold for things genuinely binary.
- **Head vs tail sampling**: head for cheap predictable sampling, blind to outcomes. Tail when you need to keep all errors / slow traces.

## What NOT to do

- **Do not** re-report linter / type-checker output.
- **Do not** post comments to GitHub. Reports go to chat only.
- **Do not** rewrite code. Suggest fixes inline.
- **Do not** invoke a subagent for trivial issues -- only when delegation actually saves context or buys expertise.
- **Do not** apply rules dogmatically. The user's context wins. If the project consistently does something the rules discourage with stated reasoning, mention once.
- **Do not** flag things explicitly required by a CLAUDE.md or project standards.
- **Do not** dogmatize the schools of thought. Honeycomb-flavored is a useful default but not the only valid lens.
- **Do not** invoke `/observability-design` -- that's brainstorm/critique for design work, not code review.
