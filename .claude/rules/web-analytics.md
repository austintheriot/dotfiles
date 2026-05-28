---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Web Analytics

A reference for evaluating product-analytics instrumentation (event taxonomy, identity, properties, funnels, retention, A/B testing, attribution, privacy) and for designing analytics for a new feature. Used by the `web-analytics` subagent and the `/analytics-design` skill.

The primary target is **Mixpanel** (the user's company uses it heavily). Coverage extends to Amplitude, Heap, Segment, PostHog, Snowplow, Pendo, FullStory / LogRocket / Microsoft Clarity / Hotjar for session replay.

Distinct from:
- **`observability-practice`** (operational SLOs / SLIs / alerts; Honeycomb-aware). This is the product-analytics layer, not service health.
- **`security`** (code-level PII handling, supply chain). We flag PII *in analytics events* as a category; the code-level threat model is theirs.
- **`distsys-data`** (warehouse / replication / schema). We sit at the event-emission and tool-side schema; warehouse modeling is theirs.

The core thesis: **analytics is the product's nervous system. Every bug in instrumentation corrupts the dataset; every taxonomy decision lives forever; every identity mistake invalidates the funnel.** A reviewer's job is to keep the dataset trustworthy enough that decisions made from it are defensible.

The empirical observation: **most analytics problems are not "we don't have the data," they are "we have data we can't trust."** Identity bugs and taxonomy drift are the silent killers. Both compound: bad events ship, get baked into dashboards, get cited in decisions, and unwinding them is expensive.

---

## Universal principles

### Identity correctness is the highest-leverage area

A single identity bug corrupts the entire dataset. The canonical failure modes:

- **Anonymous events not tracked before signup.** Marketing fires no events until the user signs up; the acquisition funnel is invisible.
- **`identify` called after the post-signup redirect, not before.** User clicks "Sign Up," server creates account, redirects to `/welcome`; by the time `identify` fires, the signup event has the anonymous distinct_id and never merges. Fix: `identify` immediately on `/welcome`, before any post-signup events.
- **`alias` called on login** (legacy Mixpanel API): each login merges the current device's anonymous history into the user. Shared computers leak strangers' history into the user's. Mixpanel's original API requires `alias` to be called **only once per user, at signup.**
- **Server-side events with a different distinct_id** than the client SDK uses. The application database must store the analytics distinct_id (`users.mixpanel_distinct_id`) and the server fetches it before firing.
- **No `reset()` on logout.** Shared-computer anonymous behavior leaks into the next user who identifies.
- **Inconsistent identity across SDKs.** Web SDK uses UUID; mobile uses IDFV / AAID; server uses something else. Without a consistent strategy, events from the same user split across three identifiers.

**Flag**: any of the above with concrete file:line references; identity logic that calls `alias` on every login; missing `identify` before the first post-signup event; missing `reset` in the logout handler; server-side events that don't carry the user's analytics distinct_id.

### Taxonomy entropy compounds

New events ship without review; the dataset becomes harder to use; eventually it's unusable. Schema-first tools (Iteratively / Amplitude Data, Segment Protocols, Avo, Snowplow Schema Registry) exist because this entropy is inevitable without enforcement.

**The convention to enforce** (Segment / Iteratively recommendation, widely adopted): **Object Action**, past tense, sentence case. `Account Created`, `Lesson Started`, `Subscription Cancelled`. The choice doesn't matter as much as the consistency.

**Flag**: inconsistent naming within one project (`Signup Submitted` next to `signup_submitted` next to `User Signed Up`); no tracking plan; ad-hoc events added without review; events whose name betrays the implementation (`onClickButtonRed`).

### The Boolean property cliff

One event with seven nullable properties usually represents seven different things. Symptoms: properties only set for some events; some property combinations are nonsensical; downstream queries are filled with `WHERE property_x IS NOT NULL`.

**Flag**: events with many properties that are mutually exclusive ("only set when action=foo, otherwise null"); properties that read like sub-events; "the event is the same but the meaning depends on which properties are present."

### Server-side for critical events

Client-side tracking is privacy-blocker-vulnerable, ad-blocker-stripped, and network-flaky. Critical events (revenue, signup, subscription state changes, churn) must have a server-side counterpart.

**Flag**: revenue events fired only client-side; signup events fired only on the frontend; subscription state changes tracked from the UI rather than from the billing webhook; conversion events that ad blockers can drop.

### PII discipline

Emails, full names, phone numbers, raw addresses are compliance risk. The patterns:

- **Omit** from event properties; if absolutely needed, use the application database, not the analytics dataset.
- **Hash** at the boundary (SHA-256 lowercased email) if matching across systems is required.
- **Separate compliant system** for PII-tagged events (e.g., a separately-permissioned warehouse).

**Flag**: `email` / `phone` / `full_name` / `street_address` as event properties; user-input search queries / form fields as properties (also a cardinality concern); user-controlled URLs with query strings as properties (often contain tokens / PII).

### Property cardinality boundaries

Free-form user input as a property value blows up cardinality. Mixpanel and Amplitude handle high cardinality better than TSDBs (Prometheus / DataDog metrics), but vendor pricing scales with event volume, not property cardinality -- so it's mainly a queryability and dashboard-aggregation issue.

**Flag**: search queries, free-text user input, full URLs, raw user agent, raw IP, raw email all as properties; "include the user's typed message as a property" patterns.

### A/B test exposure, not assignment

A common bug: the test variant is recorded on **assignment** (when the SDK decides which variant the user gets), not on **exposure** (when the user actually sees the variant). Users assigned to variant B who never reach the page where the variant matters dilute the test.

**Flag**: A/B test variant fired in a generic "experiment_assigned" event at page load instead of when the variant feature is rendered / interacted with; missing variant property on downstream events (can't filter funnel by variant post-hoc); no Sample Ratio Mismatch (SRM) check (assignment is 50/50 but observed traffic is 55/45 → variant bug).

---

## Event taxonomy

### The Object-Action convention

`Object Action`, past tense, sentence case:

- `Account Created`
- `Lesson Started`
- `Subscription Cancelled`
- `Payment Succeeded`
- `Item Added To Cart`

Alternative conventions exist (`object.action`, `verbed_object`, `action_object`). The choice is local; the consistency is global. **The agent should flag mixed conventions within one project**, not advocate for one specific style.

### Granularity

Too granular (every click): dataset is noise, dashboards are unintelligible, every UI change breaks instrumentation.

Too coarse (one event per session): can't answer specific behavioral questions.

The "thoughtful granularity" middle: track **milestones** (the actions that map to product or business value), not every interaction. `Editor Opened`, `Document Saved`, `Document Shared` -- not `Toolbar Hovered`, `Menu Item Highlighted`.

**Flag**: events for every click / hover / scroll; one mega-event per session covering many actions; "we track everything by default" (Heap-style autocapture without curated event extraction).

### Marketing vs engineering events

Marketing / growth events (signup, activation, retention milestones) belong in product analytics. Engineering events (errors, performance, internal state changes) belong in observability tools. Mixing them in one project produces dashboard chaos.

**Flag**: error logs sent to Mixpanel; perf timings as Mixpanel events; signups in DataDog; the product-analytics and observability worlds blurred.

### Tracking plans

A tracking plan is the documented schema: every event, its properties, their types, their valid values, the event owner. Tools: Avo, Iteratively (Amplitude Data), Segment Protocols, Snowplow Schema Registry.

**Flag**: no tracking plan; tracking plan that hasn't been updated in months while events have been added; tracking plan that doesn't reach engineers (so events ship without conforming).

---

## Properties

### Event properties vs user properties vs super properties

- **Event properties**: attached to a single event. `{tab: "files"}` on `Tab Switched`.
- **User properties**: set once per user, persist across events. `{plan: "pro", signup_date: "2024-01-15"}`. Settable via `people.set` (Mixpanel) / `setUserProperties` (Amplitude) / `identify(traits)` (Segment).
- **Super properties / global properties**: attached to every subsequent event from this session. `{environment: "production"}`. Settable via `register` (Mixpanel) / per-call defaults.

**Flag**: user properties that should be event properties (or vice versa); same property recorded both as user and event property (drift); super properties set in dev that should be per-environment.

### Property naming

Naming matches the event convention. Camel? Snake? Object-prefixed? Document it.

**Flag**: property names that read like events (`button_clicked` as a property -- should be the event); mixed naming in the same project; properties whose name describes *implementation* (`onClickHandler_target`).

### Property types

- **Enums** (strict, low cardinality): preferred. `{plan: "free" | "pro" | "enterprise"}`.
- **Booleans**: often hide enums. `{is_active: true}` -- but if there's also `is_archived`, `is_pending`, that's a state machine; use one `status: "active" | "archived" | "pending"`.
- **Strings (free-form)**: cardinality risk. Use enums where possible.
- **Numbers**: avoid floats for currency (use minor units integer).
- **Dates**: ISO 8601 with timezone, or Unix epoch. Document.
- **Nested objects**: vendor-dependent (Mixpanel supports; some don't).

**Flag**: boolean field clusters that should be a state enum; floats for money; date fields as locally-formatted strings; nested objects in vendors that flatten them.

---

## Identity (per-vendor)

### Mixpanel (original API, pre-2024)

```
track(event, props)             // fires with current distinct_id
identify(user_id)               // switch to known user_id
alias(user_id, anon_distinct_id) // ONCE at signup to merge histories
```

**Footguns**:
- `alias` on every login → merges shared-computer strangers' history.
- `identify` before `alias` at signup → misses the alias linkage.
- `alias` without `identify` first → half-identified state.

### Mixpanel (simplified API, 2024+)

```
track(event, props)
identify(user_id)               // Mixpanel handles merge automatically
// no alias call
```

**Project-level decision**: original or simplified. Don't mix within one project.

### Amplitude

`user_id` (known) + `device_id` (anonymous). `identify` sets user_id; merge happens server-side. No explicit `alias`. User properties via `setUserProperties` / `Identify().set()`; `setOnce` for fields that should never change (`signup_date`).

### Segment

CDP role: routes `identify(userId, traits)` and `track(event, properties)` to destinations. Maintains `anonymousId` + `userId` linkage. `alias` exists for Mixpanel-old-API destinations; no-op for others.

### Cross-platform identity

Web (UUID), mobile (IDFV / AAID), server (lookup from DB) must converge on the same distinct_id per user. The application DB stores the analytics distinct_id; server-side events look it up; mobile and web SDKs set the user's distinct_id explicitly on login.

**Flag**: SDKs initialized without a consistent distinct_id strategy; server events without distinct_id lookup; mobile + web SDKs identifying the same user with different IDs.

---

## Funnels and retention

### Funnel definition

- **Ordered or unordered?** Most funnels are ordered (must hit step 1 before step 2). Mixpanel allows both.
- **Strict or non-strict?** Strict: no events between funnel steps. Non-strict: other events allowed.
- **Time window**: 1-day, 7-day, 30-day -- typically the most-disputed analytics parameter. Pick by the natural cycle of the action.

**Flag**: funnels with no documented time window; the same funnel measured with different windows in different dashboards; "drop-off" reports that don't show what users did instead.

### Step definitions

A step is an event, optionally with property filters. `Signed Up` with property `plan: premium`.

**Flag**: funnel steps that are property-filtered but the property name has changed (so the filter is now empty); steps that use deprecated event names.

### Retention

- **N-day retention**: did the user return exactly N days later? (Strict.)
- **Rolling retention** (Amplitude default): did the user return on N+ days?
- **Bounded vs unbounded retention**: defined retention window vs forever.
- **Day-0 retention**: did the user return *on the same day as signup*? (Always 100% by some definitions, since they were active to sign up.)

**Flag**: retention type not specified; mixing N-day and rolling in the same report; day-0 retention misinterpreted.

### Retention curves

- **Smile curve**: drops, then rises as power users emerge. Good.
- **Flat curve**: levels off at a sustainable rate. Great.
- **Down curve** to zero: no retention. Bad.

Sean Ellis's framing: a flat curve is the goal; a smile is also healthy.

---

## North Star Metric (NSM)

The NSM (Amplitude playbook): a single metric that captures the core value the product delivers. Should be:
- **A leading indicator** of revenue.
- **Measurable** without complex aggregation.
- **Hard to game** with tactical tricks.
- **Tied to a customer outcome**, not internal activity.

**Examples**:
- Spotify: "Time Spent Listening."
- Airbnb: "Nights Booked."
- Notion: "Weekly Active Editors."
- Slack: "Messages Sent in Active Channels."

**NSM inputs**: 3-5 metrics that drive the NSM. The "tree": NSM at the top, inputs below.

**Flag**: NSMs that are vanity metrics (MAU without engagement filter); multiple NSMs (no NSM); NSMs gameable by tactical tricks (loginscount); no input metrics defined.

---

## A/B testing (Kohavi-shaped)

### Exposure, not assignment

Variant property on every relevant event. The variant must be set when the user **sees** the variant (the rendered component, the route entry), not when assignment happens (in the SDK at page load).

```
// Wrong: tracked at assignment
trackEvent("Experiment Assigned", {experiment: "X", variant: "B"})

// Right: tracked on exposure to the variant
trackEvent("Pricing Page Viewed", {experiment_X_variant: "B", ...})
```

**Flag**: assignment events without subsequent exposure events; downstream events not carrying the variant property (can't filter the funnel by variant); no variant property at all on the events the test measures.

### Sample Ratio Mismatch (SRM)

Assignment was 50/50, observed traffic is 55/45 -- the test is broken. Check before reading results.

**Flag**: A/B test results reported without SRM check; SRM detected but ignored ("close enough"); variant-specific filtering that produces SRM.

### Peeking and sequential testing

Watching p-values daily and stopping when significant inflates false positives. Kohavi: pre-register sample size and significance threshold; use sequential-testing methods if you must stop early.

**Flag**: tests stopped because "it's clearly winning"; no pre-registered sample size; results that include the day-by-day breakdown as the primary view.

### Multiple testing

Testing 20 metrics, one will be "significant" by chance. Bonferroni / Benjamini-Hochberg corrections; or pre-register the primary metric and treat others as secondary.

**Flag**: A/B tests where the reported "winner" is the one metric out of 15 that hit p<0.05; no pre-registered primary metric.

### Novelty and primacy effects

Short-term lifts from "the new thing" don't persist; long-term effects often differ. Run tests long enough.

**Flag**: 1-week test results treated as conclusive; tests not running through a full natural cycle (week, billing cycle).

---

## Implementation patterns

### Client-side tracking

JS SDK, mobile SDK. Privacy-blocker-vulnerable. Use for UX-context events (the user clicked the tab); don't rely on for revenue / signup.

### Server-side tracking

Events fired from backend. More reliable; needs identity passed through. The pattern: application DB stores user's analytics distinct_id; server-side code looks it up before firing.

### Hybrid (CAPI / dual-fire)

Client fires for instant feedback; server fires for reliability. Deduplicate by event ID.

### CDP (Segment, RudderStack, Hightouch, Census)

One ingestion pipeline routes to many destinations. Reverse-ETL: warehouse → tools.

**Flag**: critical events client-only; multiple analytics tools without a CDP (drift between them); revenue events not server-confirmed; no event deduplication for dual-fire.

---

## Mixpanel-specific

### Core API

- `track(event, properties)` — fire an event.
- `identify(distinct_id)` — set the user's distinct_id.
- `alias(new_id, old_id)` — **legacy API only; once per user at signup**.
- `register(properties)` / `register_once(properties)` — set super properties.
- `people.set(properties)` / `people.set_once(properties)` — set user properties.
- `reset()` — clear identity (call on logout).
- `time_event(event)` — start a duration timer, ended on the next `track(event)`.

### Report types

- **Insights**: arbitrary aggregations / charts.
- **Funnels**: ordered conversion analysis.
- **Retention**: N-day or rolling.
- **Flows**: most common paths through events.
- **Cohorts**: user segments by behavior.
- **A/B test analysis**: variant comparison.
- **Impact**: causal-flavored before/after analysis.

### Project structure

- **Lexicon**: the event/property schema documentation; integrates with tracking plans.
- **Tags** on events for organization.
- **Service accounts** for server-side tracking.
- **Group analytics**: aggregate by company / org / team.

### Common Mixpanel anti-patterns

- One Mixpanel project mixing dev and prod data.
- `alias` called on every login (old API).
- Migration to simplified API attempted in-place on an existing project (don't; create a new project).
- `reset()` not called on logout.
- Super properties leaking across users on shared devices.
- `people.set` called with email / PII without compliance review.
- High-frequency event firing without `debounce` (every keystroke).

---

## Privacy and compliance

### GDPR (EU, 2018)

- **Lawful basis** for tracking: consent for analytics is the most common; legitimate interest may apply for first-party analytics with privacy-preserving design.
- **Cookie banners that actually block** until consent. Pre-consent tracking is the violation.
- **Right to access / portability**: users can request their data.
- **Right to deletion**: users can request deletion. Mixpanel's `delete_user` API.
- **Data Processing Agreement (DPA)** with each analytics vendor.
- **Cross-border transfer**: Schrems II (2020) invalidated US-EU Privacy Shield; Standard Contractual Clauses + supplementary measures (encryption, pseudonymization) required.

### iOS App Tracking Transparency (2021)

- IDFA available only with explicit user opt-in.
- SKAdNetwork (now SKAN 4) for privacy-preserving attribution.
- Most users opt out (~75%); attribution accuracy permanently degraded for non-consenting traffic.

### PII boundary in analytics

The list to keep out of event/user properties:
- Email
- Phone number
- Full name
- Street address
- Government ID (SSN, etc.)
- Payment card details
- Health information
- Precise location (city is OK; coordinates are not)

**The hashed-PII pattern**: SHA-256 lowercased email for cross-system matching where required (Facebook CAPI, Google Enhanced Match). The hash is still personal data under GDPR, but lower-risk.

### Retention policy

Document how long analytics data lives. Mixpanel default: forever. Most teams want 2-5 years. Compliance teams want shorter; product teams want longer.

**Flag**: no retention policy; no DPA on file; no deletion-on-request workflow; PII in analytics; cookie banner that fires tracking before consent; cross-border transfer without SCCs.

---

## Anti-pattern catalog

The high-yield findings:

### Taxonomy
- Inconsistent naming within one project.
- Boolean property cliff (one event for what should be seven).
- Properties as catch-all for sub-events.
- Free-form text in properties.
- No tracking plan.
- Marketing and engineering events mixed.

### Identity
- No anonymous tracking before signup.
- `identify` after the post-signup redirect.
- `alias` on every login (legacy Mixpanel).
- `alias` called multiple times per user.
- Server distinct_id differs from client.
- No `reset()` on logout.
- Inconsistent SDK identity across platforms.

### Properties
- PII in event / user properties.
- High-cardinality free-form text.
- Property names that read like events.
- User vs event property confusion.
- Float currency.

### Events
- Pageviews as the primary unit in a non-content product.
- No "first value" event.
- Revenue events client-only.
- Critical events client-only.
- Tracking after a redirect that drops the event.

### Funnels and retention
- Time window not documented.
- Retention type not specified.
- Day-0 retention misinterpreted.
- Funnel measured without segmentation.

### A/B tests
- Exposure tracked at assignment instead of exposure.
- No exposure event at all.
- No SRM check.
- Stopped early because "it's winning."
- Optimizing for clicks while retention silently tanks.
- Multiple metrics tested without correction.
- Variant property not propagated downstream.
- Single A/B test without segment cuts.

### NSM
- NSM that's a vanity metric.
- Multiple NSMs (no NSM).
- NSM gameable by tactical tricks.
- No input metrics.

### Implementation
- Single project mixing dev and prod.
- No data quality monitoring.
- Frontend-only tracking for money / signup events.
- Multiple tools without a CDP (drift).
- No documented event ownership.

### Privacy
- PII in properties.
- No retention policy.
- No deletion-on-request workflow.
- EU user data without SCCs.
- Cookie banner that fires tracking before consent.
- No DPA on file.

---

## What is NOT a web-analytics finding

Signal-to-noise:

- **Operational telemetry** (logs, metrics, traces for service health). Route to `observability-practice`.
- **Code-level PII handling outside analytics** (database PII storage, log redaction). Route to `security`.
- **Warehouse / dbt / schema modeling at the data layer.** Route to `distsys-data`.
- **Session replay tools as a category** without analytics integration. Note but don't flag.
- **Business strategy / metric choice as a product decision** (whether MAU is the right NSM for this business). The agent flags vanity metrics; doesn't second-guess strategic choices the team has made deliberately.
- **A/B test statistical methodology beyond review.** Flag obvious bugs (peeking, no SRM); refer deeper questions to a statistician or experimentation platform docs.

---

## Severity calibration

Using `panel-contract.md`'s rubric; specific calibration:

- **blocker**: identity bugs that corrupt the dataset systematically (alias-on-login in legacy Mixpanel; identify-after-redirect that loses every signup's anonymous history); PII shipping to analytics in violation of GDPR / CCPA; revenue events client-only with no server fallback; A/B tests with no exposure tracking at all.
- **major**: inconsistent taxonomy that's documented as a known problem but not fixed; missing tracking plan on a project shipping new events monthly; missing super properties on environment / app version; A/B tests measured without variant propagation; funnels with no documented time window in a critical dashboard.
- **minor**: property naming inconsistency in non-critical events; missing day-0 retention documentation; unused events accumulating; non-fatal property cardinality.
- **nit**: style-level (event name capitalization on one event vs others); minor lexicon gaps.
- **insight**: structural -- "this project has accreted three identity strategies; consider migrating to the simplified Mixpanel API in a new project"; "consider a tracking plan in Avo / Iteratively to enforce schema."

Confidence: high when the trigger is concrete (a specific call site, a specific naming inconsistency); medium when reasoned about pattern (the agent infers from one file that the convention isn't enforced).

---

## Process for the web-analytics agent

1. **Identify the surface.** Which analytics tool(s)? Mixpanel? Amplitude? Segment? Multiple? Self-hosted PostHog?
2. **Read project analytics conventions.** `docs/analytics.md`, `docs/tracking-plan.md`, the Lexicon export, any Avo / Iteratively config, `CLAUDE.md` sections.
3. **Walk identity.** Every `identify` / `alias` / `track` call: is the timing correct? Does the server know the distinct_id? Is `reset()` called on logout?
4. **Walk taxonomy.** Are event names consistent? Object-Action past-tense? Sentence case?
5. **Walk properties.** PII check. Cardinality check. Property type discipline. Event vs user property correctness.
6. **Walk critical events.** Revenue server-side? Signup server-side? Subscription state changes from billing webhooks?
7. **Walk A/B tests.** Exposure vs assignment. Variant property propagation. SRM check. Multiple-testing discipline.
8. **Walk privacy.** PII boundary, consent boundary, deletion workflow, DPA, retention policy.
9. **Route to other lenses** where the angle is theirs (PII storage / handling code-level → `security`; warehouse modeling → `distsys-data`; operational telemetry → `observability-practice`).
10. **Stay read-only.**
