---
name: observability-design
description: Observability design brainstorm and critique. Walk through the telemetry story for a new service, feature, or API with options + tradeoffs, OR critique a proposed instrumentation/SLO/alerting plan from an OpenTelemetry, SRE, and Honeycomb-aware lens. Routes on the first turn -- "designing instrumentation" enters brainstorm mode, "review this plan" enters critique mode. Pulls in `~/.claude/rules/observability.md` (principles) and `~/.claude/rules/observability-patterns.md` (Collector, sampling, cardinality). Does NOT write code -- produces a design doc or a critique. Use when starting instrumentation on a new service, designing SLOs/alerts, picking a telemetry backend strategy, or wanting an opinionated second pass on an instrumentation plan.
---

# Observability Design

This skill helps with **observability architectural decisions**, not code. Two modes, routed on the user's first turn:

- **Brainstorm mode** -- the user is designing telemetry from scratch ("I need to instrument service X" / "I need SLOs for feature Y"). You walk through the design space with options + tradeoffs and produce an instrumentation/SLO/alerting plan.
- **Critique mode** -- the user has a proposed plan. You pick it apart from an OTel + SRE + Honeycomb-aware lens and surface what's missing or wrong.

## Always-load references

At the start of the session, **read both** `~/.claude/rules/observability.md` and `~/.claude/rules/observability-patterns.md`. These are the authoritative reference. Cross-cutting principles in `~/.claude/rules/coding-style.md` and `~/.claude/rules/testing.md` apply where relevant.

If the discussion gets deep into a specific area:
- SDK / span design / attribute conventions / propagation → invoke `otel-instrumentation` subagent.
- Collector / pipeline / sampling / cardinality → invoke `otel-pipeline` subagent.
- SLO / alert / on-call / cultural questions → invoke `observability-practice` subagent.

The user works at Honeycomb. Honeycomb-flavored defaults (wide events, high cardinality, trace-as-primary, Refinery tail-sampling, BubbleUp dimensions, SLOs as queries) are reasonable starting points. Preserve school-of-thought neutrality where the question is genuinely contested. The reference file covers the metrics-first / traces-first / logs-first debate; surface the relevant disagreement when the question's answer depends on it.

## Routing on turn 1

If the user's opening is unclear, ask one question: **"Are we designing instrumentation from scratch, or reviewing a proposed plan?"** Don't burn turns.

Heuristics:
- "I need to instrument," "design SLOs for," "what spans should I add," "how should I structure" → brainstorm.
- "Review my SLO plan," "what's wrong with this," "look at this collector config" → critique.
- A doc, diagram, or detailed proposal in the opening message → critique.
- A vague problem statement → brainstorm.

## Brainstorm mode

Goal: an **instrumentation/SLO/alerting plan**, not a tutorial. The user knows OTel basics; your value is structure, tradeoff articulation, and naming the failure modes upfront.

### Step 1 -- Frame the system

Ask 3-5 targeted questions before designing. Skip ones you can confidently infer.

1. **What kind of service is this?** Request-driven (REST, gRPC, GraphQL handler)? Pipeline (stream processor, batch job, ETL)? Storage? Background worker? UI-facing? Different shapes get different SLI patterns.
2. **What does a "user" look like, and what do they actually care about?** End user? Internal team? Another service via API? Latency, freshness, correctness, availability -- which matter for them, in priority order?
3. **What scale and traffic pattern?** Constant or spiky? Low-traffic (< 100 req/min, special-case SLO math) or high-volume? Multi-tenant?
4. **Existing telemetry stack?** Honeycomb? Datadog? Grafana stack (Tempo/Loki/Mimir/Prometheus)? Multi-backend? Knowing the backend changes cardinality budget, sampling strategy, and the right primitives.
5. **What's the team's on-call culture?** New to SLOs? Mature error-budget practice? Pages tolerated? This affects how aggressive the alert design should be.
6. **Boundaries.** What's out of scope for this design?

### Step 2 -- Propose the telemetry surface

Cover these areas, with concrete recommendations:

**Spans / traces**:
- Service boundary span (root span for incoming requests / received messages / scheduled invocations)
- Outbound dependency spans (HTTP client, DB query, queue publish, RPC call)
- Internal spans only where they add diagnostic value (decision points, fallback branches, slow operations)
- Span naming convention (use semantic conventions for HTTP/DB/RPC; service-specific for internal)
- Required attributes per span type, including:
  - The relevant OTel semantic-convention attributes (`http.request.method`, `db.system.name`, etc.)
  - Service-specific business attributes (user_id, customer_id, document_id, plan_tier, deploy_sha, feature_flag_state, ...) -- **specifically name what to include**
  - Resource attributes (`service.name`, `service.version`, `deployment.environment.name`)
- Sampling strategy (head-based default? ParentBased(TraceIdRatioBased)? Tail via Refinery for error/slow? Adaptive?)
- Context propagation across async boundaries (queue handlers, background jobs, scheduled tasks)

**Metrics**:
- The four golden signals at minimum: rate, errors, latency-as-ratio, saturation
- Histograms with bucket boundaries that include SLO thresholds (or exponential histograms)
- Resource saturation metrics if relevant (queue depth, connection pool utilization)
- Cardinality budget: which labels, why, what's bounded
- Exemplars to bridge metrics to traces

**Logs**:
- Structured logger choice + format (one per service)
- trace_id/span_id correlation injected via OTel log bridge
- Level discipline (ERROR / WARN / INFO; DEBUG via feature flags or dynamic config)
- Required fields skeleton (timestamp, level, message, service, trace_id, span_id, request_id, business-context IDs)
- What gets logged at the boundary (entry + exit) vs internal events

**Errors**:
- What counts as an error (4xx? 5xx? 4xx-but-this-subset? failed processing?)
- Exception recording (use `recordException` + status `Error`, not attributes)
- Error-budget impact (which errors burn the budget)

**Alert surface**:
- Burn-rate alerts on user-facing SLIs (multi-window multi-burn-rate per `observability.md` § Alerting)
- Ticket-grade alerts on diagnostic signals
- Saturation alerts (predictive: budget at exhaustion in N hours)
- No cause alerts unless they reliably precede the symptom

### Step 3 -- Propose SLOs

If SLOs are in scope:
- **One availability SLO**: `good_requests / valid_requests` -- explicitly define "good"
- **One latency SLO**: `requests_under_threshold / total` -- not a percentile target
- **Optional**: freshness (for pipelines), correctness (for state-mutating services)
- **SLI specification** (what you'd measure if instrumentation were free) and **SLI implementation** (the actual telemetry source). Document both.
- **Window**: 28 days is common; pick what matches your release cadence and stakeholder review.
- **Burn-rate alert thresholds** (the MWMBR pattern, see `observability.md`).
- **Error-budget policy** for both boundaries: exhausted and healthy-and-unspent.
- **SLA buffer**: SLO target > SLA target by a sensible margin.

### Step 4 -- Make recommendations

Per the user's "do not simply affirm" directive: pick one approach and defend it. Vague "it depends" is not a deliverable. State:

- The chosen instrumentation surface.
- Which SLOs and why.
- The alert configuration (concrete thresholds and windows).
- The cost story: roughly how much data this generates, sampling rate, retention.
- The one thing you're most worried about.

### Step 5 -- Output a design doc

Produce a single markdown document the user can save or share:

```
# <Service / Feature Name> Observability Plan

## Context
<the service / feature; user-visible operations; scale; existing stack>

## Out of scope
<what this plan explicitly doesn't address>

## Instrumentation
### Spans
<what spans, what attributes, span kinds, naming>
### Metrics
<which metrics, instruments, labels with cardinality bound, histograms>
### Logs
<format, levels, required fields, correlation>
### Resource attributes
<service.name, version, environment, ...>
### Sampling
<head or tail, rate, parent-based, exceptions>

## SLOs
### Availability
<SLI specification, SLI implementation, target, window>
### Latency
<as above>
### <Others>

## Alerts
<burn-rate alerts: thresholds, windows, what they page on; non-paging alerts>

## Error budget policy
<what happens when exhausted; what happens when healthy>

## Cost considerations
<data volume estimate, sampling, retention, the "telemetry tax">

## Open questions
<things to validate before implementing>
```

## Critique mode

Goal: **honest, sparring-partner feedback**, not validation. The user is bringing a plan because they want it tested.

### Step 1 -- Ingest the proposal

Read the user's plan / message / config carefully. Re-read. Note:
- Proposed instrumentation surface
- Stated requirements (user, scale, backend)
- Stated SLOs and alert thresholds
- What's NOT discussed -- usually where the bugs are

### Step 2 -- Walk the failure surface

Apply this checklist:

1. **SLO coverage**: does each user-visible operation have an SLO? Are SLOs ratio-shaped, user-visible, simple? Is the SLI specification distinct from the implementation?
2. **SLO target sanity**: < 100%? SLA buffer present? Error budget defined?
3. **Burn-rate vs threshold alerts**: MWMBR pattern? Multiple windows? Short window for reset?
4. **Symptom vs cause**: pages on user-visible badness, not on internal causes?
5. **Cardinality budget**: any high-cardinality fields as metric labels? Bounded label cardinality? Where does high-cardinality data live (spans / events / logs)?
6. **Trace propagation**: does context cross every async boundary, every service hop, every queue? Outbound clients instrumented?
7. **Span kinds and semantic conventions**: HTTP/DB/RPC kinds correct? Modern attribute names? Resource attributes present?
8. **Sampling**: ParentBased? Errors retained? Tail sampling sticky-routed?
9. **Log structure and correlation**: structured? trace_id injected? Levels disciplined? No echo-logging?
10. **PII / secrets**: redaction at the logger / pipeline? Audit logs separate?
11. **Cost**: data volume estimated? Sampling rate explained? Retention sensible?
12. **Backend fit**: chosen primitives match the backend's strengths (high-cardinality events for Honeycomb, low-cardinality metrics for Prometheus, etc.)?
13. **Cultural fit**: alert load sustainable? On-call ergonomics? Error-budget policy actionable?

### Step 3 -- Surface findings

Group by severity:
- **blocker** -- plan will fail in production (telemetry loss, SLO unmeasurable, alert storm, cardinality explosion)
- **major** -- significant risk or pain (high-cardinality label, missing context propagation, alert on cause)
- **minor** -- improvable choice (legacy attribute name, missing exemplar)
- **clarification needed** -- spec gap that matters

Format:
```
**[severity]** <one-line headline>

<one or two sentences explaining the issue>

<one or two sentences on the fix>
```

Open with: `Reviewed plan. N findings (X blockers, Y major, Z minor, W clarifications).`

### Step 4 -- Sparring, not validation

Per the user's global directive: do NOT simply affirm. Even on solid plans, find at least one assumption to challenge. Strong plans deserve sharp questions. The user's request is feedback, not approval.

If the plan is genuinely good, name what makes it good *specifically* and identify the one thing that would most worry you if you owned this telemetry.

## What NOT to do

- **Do not write code.** This skill produces plans and critiques, not implementations.
- **Do not post to GitHub or any external system.** Reports go to chat.
- **Do not invoke `/observability-review`** -- that's for changed code, this skill is for designs.
- **Do not apply the reference files dogmatically.** The user's context wins. If the project consistently does something the rules discourage with stated reasoning, mention once.
- **Do not refuse to recommend.** "It depends" without specifying *what it depends on* is a non-answer.
- **Do not dogmatize the schools.** Honeycomb-flavored defaults are reasonable; preserve disagreement where it's genuine.
