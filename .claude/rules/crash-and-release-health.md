---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Crash Reporting and Client Release Health

A reference for reviewing crash capture, symbolication, and the telemetry that decides whether a release is healthy. Used by the `crash-and-release-health` subagent.

Distinct from:
- **`observability-practice`**: server-side SLOs, burn-rate alerting, golden signals, on-call. **The shapes genuinely differ**: a server emits continuous telemetry from machines you control; a client emits sampled, delayed, consent-gated telemetry from machines you do not control, running versions you cannot recall.
- **`debuggability`**: development-time affordances. We own post-release diagnosis.
- **`platform-release`**: signing, submission, rollout mechanics. **The seam**: they own that a staged rollout is non-decreasing and non-recallable; we own the metric and threshold that says halt.
- **`app-privacy-compliance`**: the regulatory obligation. We own the mechanism by which crash payloads carry personal data.

The core thesis: **the worst failures produce the least data.** A crash-on-launch loop can generate *fewer* reports than a mild bug, because the process dies before the reporter can initialize or flush. **A decline in crash reports during a rollout is not good news until proven otherwise.**

The operational priority: **check that symbolication will actually work for the build being shipped.** An unsymbolicated release does not merely inconvenience triage; it silently degrades grouping, because a tracker with no usable stack falls back to message text that often contains variable data, fragmenting one bug into hundreds of issues exactly when triage matters most.

Verification markers: **[V]** verified against primary source, **[U]** unverified.

---

## Universal principles

### Symbols are keyed to a build, not to source

Apple states it plainly [V]: *"If you build two binaries from the same source code but with different Xcode versions or build settings, the build UUIDs for the two binaries won't match"*, and a binary and its symbol file are *"only compatible with each other when they have identical build UUIDs."*

**Rebuilding from the same tag does not regenerate matching symbols** unless the build is bit-for-bit reproducible. This is why "we can just rebuild it" is a dead end and why the correct policy is archive-forever, keyed by UUID. Apple's own guidance is an Important callout: retain the archive for every build you distribute, or you may not be able to diagnose it.

The Android equivalent is blunter [V]: the mapping file *"is overwritten every time you build, so you must save a copy each time you publish a new release."*

### A crash category is not a diagnosis

On Apple platforms, `EXC_CRASH (SIGKILL)` covers three unrelated events [V]: a **watchdog termination** (main thread blocked past a wall-clock budget), a **jetsam memory kill**, and a **user force-quit**. Only the termination metadata separates them.

Treating all of them as crashes counts user behavior as defects; treating none of them as crashes hides the two worst classes of failure. Note that watchdog budgets are **wall-clock, not CPU time** [V], so a blocked main thread waiting on the network counts fully.

Swift runtime failures -- force-unwrapping nil, array bounds, precondition failures -- surface as `EXC_BREAKPOINT` or `EXC_BAD_INSTRUCTION`, **not** as exceptions [V]. An uncaught-exception handler never sees them.

### First-party data is a biased sample, not a rate

Apple's crash organizer presents reports only *"from customers who share diagnostic and usage information"* [V], while TestFlight users share **automatically regardless of device settings** [V]. So one source is an opt-in sample of production with unknown bias, and the other is a census of a non-representative population. **Neither supports a rate you can alert on.**

Android's `ApplicationExitInfo` is the only reliable first-party source for memory kills and ANRs, because neither delivers an in-process callback -- but it is read on the *next* launch from a bounded history buffer, so a user who never relaunches is invisible and a crash loop can evict earlier records.

### Metrics are not comparable across vendors

The session definitions differ by three orders of magnitude [V]. One major vendor starts a new session after **30 minutes** of backgrounding and counts a device installation as a user; two others use **30 seconds**; one measures **error**-free rather than **crash**-free, so adding a `try`/`catch` can *lower* the reported number. Browser sessions may be created per page load or per single-page-application navigation.

**Switching vendors moves the number without the application changing**, and a target inherited from a previous vendor is meaningless. Crash-free targets are only interpretable alongside a stated session definition and vendor.

---

## Crash capture per platform

### Apple

The crash file is **two JSON objects, not one** [V] -- a metadata line followed by the report body -- so parsing the whole file as a single document fails. **Numeric codes are stored in decimal**, so grepping for the familiar hexadecimal watchdog constant silently matches nothing.

The image list carries the **UUID that keys symbolication**, along with load address and architecture.

**MetricKit is not a crash reporter.** Delivery is *"at most once per day per metric source"* for metrics, with diagnostics immediate on recent versions [V]. It cannot drive a rollout decision on its own timescale. Its real value is the aggregate signal a third-party SDK cannot see -- in particular, the **only first-party count of memory kills and watchdog exits**, as a daily aggregate with no stack.

**The entire Objective-C `MX*` class family is deprecated as of the 27.0 releases** [V], replaced by Swift-native value types delivered through asynchronous sequences. Code written against the old manager is on a deprecation path.

**Unverified** [U]: the minimum hang threshold, hang histogram bucket boundaries, and the numeric thresholds that trigger CPU and disk-write exceptions are not published.

### Android

`ApplicationExitInfo` exposes reason codes covering crashes, ANRs, memory kills, excessive resource usage, watchdog, permission change, and user action, plus process importance -- which is how you compute *user-perceived* rather than raw counts [V].

**ANR timeouts** [V]: input dispatch **5 seconds**; foreground-service promotion **5 seconds**; broadcast receipt **5 seconds** with a foreground activity; service and job callbacks a few seconds.

**Play vitals thresholds are the numbers that gate discoverability** [V]: user-perceived crash rate at or above **1.09%** of daily users overall and **8%** on a single device model; user-perceived ANR rate at or above **0.47%** overall and **8%** per model. "User-perceived" means the app was displaying an activity or running a foreground service, and for ANR the store currently counts **only input-dispatch timeouts**.

Native crashes write tombstones, and the abort *reason* is gathered from the last line of fatal log output rather than from the signal itself [V].

`Thread.setDefaultUncaughtExceptionHandler` is a **single global slot** -- installing one displaces any previous handler silently. Chaining the prior handler is the correct pattern.

### Native and desktop

**A correction worth carrying** [V]: neither Breakpad's own documentation nor Crashpad's status page contains any deprecation notice or successor statement. The accurate claim is that **Chromium's crash handler lineage moved to Crashpad, and Crashpad has the broader modern platform matrix** -- not that Breakpad is deprecated, which is undocumented.

On Windows, the unhandled-exception filter is documented to *"replace the existing top-level exception filter for all existing and all future threads"* -- another single global slot -- and to execute *"in the context of the thread that caused the fault. This can affect the exception handler's ability to recover from certain exceptions, such as an invalid stack"* [V]. **That is the documented reason in-process handlers are unreliable**: a stack overflow leaves no stack to run the handler on, so those crashes are systematically under-reported.

**Installing an in-process handler opts you out of the operating system's own dump collection**, which documents that applications doing custom crash reporting *"are not supported by this feature"* [V]. That is a direct input to the should-you-install-a-native-handler debate.

### Web

**The opaque cross-origin error is a specification behavior, not a bug** [V]. A script loaded cross-origin carries a muted-errors flag, set when the response is cross-origin, with the stated rationale that error information *"can leak private information."*

**The fix is two-sided and both halves are required**: the `crossorigin` attribute on the script tag **and** the CORS header on the script response. Setting only one yields no improvement, and the half-fix is common.

**Asynchronous errors do not reach the global error handler**; unhandled promise rejections need their own listener [V]. `reportError()` is the supported way for a library to route a caught error into the same pipeline, since it emulates an uncaught exception [V].

**React error boundaries do not catch** [V]: event handlers, server-side rendering, errors thrown in the boundary itself, or asynchronous code -- **with one exception**, errors thrown inside a transition function *are* caught. That carve-out is recent and missed in both directions.

### Rust

**A correction to a widespread misconception** [V]: a panic hook *"will run with both the aborting and unwinding runtimes."* **Panic hooks fire under `panic = "abort"`.** The claim that unwinding is required to capture panics is wrong.

The trap is elsewhere: release builds default to **no debug information**, and with default stripping there is no separate symbol artifact to recover line numbers from later [V]. A line-tables-only setting is the documented middle ground.

---

## Symbolication

Apple documents `atos` as the current path, with an inlining flag that matters: **without it, inlined frames vanish and the stack lies** [V]. **Unverified** [U]: whether the older symbolication script still ships -- it is absent from all current documentation, which is suggestive but not proof. Do not assert its removal.

**The `debugSymbolLevel` casing contradiction is settled** [V]. Both documentation sources are right about their own style and neither describes a behavioral constraint: the value passes through a case-normalizing converter that is idempotent on already-uppercase input, so lowercase, uppercase, and mixed all resolve. Recommend lowercase for consistency with the DSL reference a reader will find, and note that existing uppercase code needs no change. Also verified: the debug-symbols upload limit is **800 MB**, symbol-table level is the recommended remedy, and auto-inclusion requires app bundles rather than raw APKs.

**Source-map debug IDs are a Stage 2 proposal, not part of the specification** [V]. The draft's field list does not contain it. The mechanism is sound and has genuine multi-vendor traction across generators and consumers, but **anyone writing "per the source map spec, debugId..." is wrong today**, and the field name and comment syntax can still change. The practical benefit is real: the identifier travels with content, so symbol upload and deploy need not be coordinated.

**Unverified** [U]: Windows symbol-server specifics, the XCFramework debug-symbols flag, and the current Android keep-attributes guidance, whose documentation was restructured.

**The CI failure mode is the one to look for.** The canonical shape is a symbol upload suffixed with a no-op success, or backgrounded, or wrapped in a disabled error check: the build stays green, the release ships, and every crash for that version arrives unsymbolicated **with no signal anything failed**. It surfaces days later mid-incident, by which time the artifact may be gone.

**The rule: symbol upload must be a gating step whose exit code fails the pipeline, and the pipeline should verify the uploaded UUID matches the binary actually submitted** -- not merely that some upload occurred. The store-delivered binary may not be the one built locally, and the UUID in the crash report is authoritative.

---

## Release health metrics

**Crash-free sessions and crash-free users answer different questions.** Sessions measures how often the product fails; users measures how many people were affected at least once. A rare crash in a high-frequency flow tanks session rate while barely moving user rate; a crash-on-launch affecting a small share of devices does the reverse.

**Which to alert on**: sessions for rollout gating, since the event count is higher and stabilizes sooner; users for severity assessment and communication. **Crash-free users during a 1% rollout is the worst of both** -- a small denominator where each affected user is a whole unit, so the metric is dominated by discreteness noise.

**Adoption normalization is mandatory.** Raw crash counts rise monotonically with rollout percentage regardless of quality, so a count-based comparison always says the new release is worse early and better late. Only per-session or per-user rates are comparable across adoption levels, and even those need a minimum denominator before they mean anything.

**iOS hang rate is a vendor-defined metric, not a platform constant.** One major SDK classifies at a two-second default threshold that is configurable, and the platform publishes no minimum. Two teams each reporting "1% hang rate" may not be measuring the same thing.

### The staged-rollout decision

The honest framing: **"wait for statistical significance" usually loses to "halt on any new blocker"**, for two reasons. At low adoption you may have too few sessions to distinguish a small regression from noise, but **a crash-on-launch is visible in the first dozen reports and is categorical rather than statistical**. Worse, the signal you would wait for is **suppressed by the very failure you are watching for**.

The workable policy is two-tier:
1. **An immediate categorical halt** on any new crash signature in a launch or critical path, with no significance test.
2. **A rate-based gate that only arms once the release clears a minimum session count**, comparing against the previous release's rate **at the same adoption stage** rather than its steady-state rate.

`platform-release` owns the fact that rollouts are non-decreasing and non-recallable, which is precisely what makes an error in the permissive direction expensive.

---

## Client problems with no server analogue

1. **You cannot recall a shipped binary.** A server rollback is a deploy; a client rollback is a new submission through review, and users on the bad build stay there until they update. Telemetry informs a **forward fix on review latency**. Halting a rollout stops new exposure only.
2. **Delayed, sampled, consent-gated arrival.** Every rate has an unknown, non-random denominator.
3. **Survivorship bias.** The most severe failures suppress their own reporting. **The signature is a suspiciously low report volume combined with a drop in sessions or active users.**
4. **Version fragmentation.** Many versions live simultaneously and indefinitely. Any aggregate not sliced by version is a weighted average over a population mix that shifts daily for unrelated reasons.
5. **Grouping fails in both directions.** *One issue holding hundreds of unrelated crashes* comes from a group key too generic to discriminate -- a common framework entry point, or an unsymbolicated stack where every key is an address in the same image. *Hundreds of issues that are one bug* comes from a key that varies per event: an interpolated identifier, a timestamp, address randomization, or inconsistent inlining. **An unsymbolicated release silently demotes grouping quality**, and rule changes typically apply only to future events, so the existing mess cannot be merged retroactively.
6. **Personal data in crash payloads.** Breadcrumbs capture navigation and network paths; request bodies may carry tokens; local variables and memory dumps can contain user content; screenshots and session replay capture whatever was on screen. **The pre-send hook is a required boundary, not an optional one.** The regulatory obligation is `app-privacy-compliance`'s; the mechanism is ours.
7. **Third-party SDK crashes count against your store thresholds and your rate.** Your only levers are pinning, defensive wrapping, a remote kill switch, or removal. **The absence of a kill switch for a third-party SDK is a finding in its own right.**
8. **Sampling and quota interact catastrophically with a crash storm.** Quota burns fastest exactly when a release is failing. Note that the common defaults sample *errors* at full rate while tracing is unset, so a team that "turned on sampling" often changed the wrong signal. Session-derived release-health metrics stay accurate under error sampling, which is the mitigation worth knowing.

---

## Anti-pattern catalog

### Symbolication
- Symbol upload with its failure suppressed, so the release ships unsymbolicated with no signal.
- Upload succeeding but shipping the wrong symbol file, producing a UUID mismatch for the shipped build specifically.
- Symbol artifacts not archived per build forever, making them unrecoverable by rebuilding.
- Mapping file not saved per release, since the next build overwrites it.
- Native symbol upload above the size limit, or assumed to happen automatically from a non-bundle build.
- Release builds with no debug information and no separate symbol artifact.
- Hosting production source maps publicly to obtain symbolication.
- Citing debug IDs as specified behavior.

### Metrics and gating
- Alerting on crash-free users during a low-percentage rollout.
- Comparing raw crash counts across rollout stages.
- Comparing crash-free percentages across vendors, or inheriting a target from a previous one.
- Counting a single-page-application route change as a session.
- **Reading a decline in crash reports during rollout as improvement.**
- Aggregating without slicing by version.

### Capture
- Treating every kill signal as a crash, conflating watchdog, memory kill, and user force-quit.
- Grepping crash files for a hexadecimal code stored in decimal.
- Parsing a two-object crash file as a single document.
- Relying on the first-party organizer for a rate, or on daily-aggregate metrics for rollout gating.
- Assuming an uncaught-exception handler catches runtime failures that trap instead.
- Assuming a panic hook does not fire under abort.
- Installing only a global error handler and missing every unhandled rejection.
- Setting the cross-origin attribute or the CORS header but not both.
- Assuming a React error boundary catches asynchronous errors or event handlers.
- Replacing rather than chaining a single-slot global handler.
- Expecting an in-process handler to survive stack exhaustion.
- Installing an in-process handler while still expecting operating-system dump collection.

### Data and cost
- No pre-send scrubbing, shipping breadcrumbs and request bodies to a processor by default.
- Sampling the wrong signal because errors and traces have different defaults.
- No quota headroom, so data disappears at peak need.
- No kill switch for a third-party SDK whose crashes count against your thresholds.

---

## Schools of thought (preserve disagreement)

- **First-party versus third-party.** First-party is free, needs no SDK, cannot crash your process, and **is what the store actually enforces against** -- but it is opt-in-sampled, delayed, and un-alertable. Third-party gives real-time grouping and alerting at the cost of an SDK in-process and a metric that disagrees with the store's. **The mature position is both**, understanding that store vitals gate discoverability while the vendor number gates your rollout.
- **Self-hosted versus hosted.** Self-hosting buys residency and cost control at the price of operating a high-cardinality store during exactly the incidents when it matters. Note that at least one major self-hosted option ships under a non-OSI-approved source-available license with community-only support.
- **Sampling aggressiveness.** One camp samples errors hard for cost, accepting that rare bugs may never appear. The other keeps errors whole and samples traces instead, on the grounds that the crash stream is the cheapest high-value signal and cannot be reconstructed. **The decisive argument is that session-derived release health stays accurate under error sampling** -- so sample errors, not sessions.
- **Whether to install a native handler at all.** Against: async-signal-unsafe by nature, conflicts with debuggers, a single global slot that fights other SDKs, and it disables the operating system's own collection. For: the platform reporters are sampled, delayed, and give no breadcrumbs or alerting. **A defensible middle is out-of-band capture**, so collection does not run inside a corrupted process.
- **Crash-free targets.** "Four nines" is near-meaningless without stating the session definition and vendor, since the definitional spread swamps the difference between 99.9% and 99.99%. Better: a target against your own historical baseline on a fixed definition, with the store's absolute thresholds as a floor you never approach.
- **Halt-on-threshold versus human judgment.** Automation is fast and unbiased but blind to severity, since a crash in a settings screen is not a crash in checkout. Human judgment is severity-aware but slow and optimistic during a launch. **The synthesis is automated halt on categorical signals and human judgment on rate regressions.**
- **Whether to report handled errors.** Counting them surfaces silent degradation but makes the headline sensitive to how defensively the code is written -- **adding a `try`/`catch` can lower reported stability**. Counting only fatals is comparable across teams but blind to swallowed failure. Reporting handled errors as a separate stream gets both.

---

## What is NOT a crash-and-release-health finding

- Server-side SLOs, burn-rate alerting, golden signals, on-call practice. Route to `observability-practice`.
- Development-time debugging affordances. Route to `debuggability`.
- Signing, submission, and rollout mechanics. Route to `platform-release`; we own the metric that gates the rollout.
- The lawfulness of data in a crash payload. Route to `app-privacy-compliance`; we own how it gets there.
- Fixing the underlying crash. We review the ability to detect and diagnose it.
- Generic vendor advocacy without a named gap.

---

## Severity calibration

Per `panel-contract.md`:

- **blocker**: symbol upload whose failure is suppressed on a release path; no archived symbols for shipped builds; alerting or gating logic that cannot detect a crash-on-launch; personal data shipped in crash payloads with no scrubbing on a path handling user content.
- **major**: mapping file not retained per release; wrong-artifact upload with no UUID verification; crash-free users used for low-adoption gating; raw counts compared across adoption stages; a single-slot global handler replaced rather than chained; missing unhandled-rejection listener; half-applied cross-origin fix; no kill switch for a third-party SDK; kill signals treated uniformly as crashes.
- **minor**: release builds with no line tables; sampling applied to the wrong signal; no quota headroom; session definition undocumented alongside a stated target; grouping key containing variable text.
- **nit**: naming of release identifiers; breadcrumb verbosity.
- **insight**: structural -- "crash reports dropped during this rollout while sessions dropped too, which is the survivorship signature rather than an improvement"; "the crash-free target was inherited from a previous vendor with a different session definition and is not comparable"; "grouping will collapse for this release because symbolication is not gated, so triage degrades exactly when it is needed."

Confidence: high when the trigger is a concrete pipeline step, handler installation, or configuration value; medium when reasoned from metric shape. **Mark unverified platform specifics as unverified**, since several were not confirmable.

---

## Process for the crash-and-release-health agent

1. **Identify the platforms and the reporting stack**, first-party, third-party, or both.
2. **Walk symbolication end to end**: generated, archived per build, uploaded, upload gated on success, UUID verified against the submitted artifact.
3. **Walk capture**: which handlers are installed, whether they chain, what each one cannot catch, and whether anything is installed twice.
4. **Walk the metric definitions actually in use**, including the session definition, and whether targets state their vendor.
5. **Walk the gating logic**: is there a categorical halt for launch-path crashes, and does the rate gate require a minimum denominator?
6. **Check for the survivorship blind spot**: would a crash-on-launch be detectable, and is session or active-user volume monitored alongside crash volume?
7. **Walk the payload** for personal data, and confirm a pre-send boundary exists.
8. **Walk cost and sampling**: which signal is sampled, and is there quota headroom for a storm?
9. **Check third-party SDK exposure** and whether a kill switch exists.
10. **Route to other lenses**: rollout mechanics to `platform-release`; regulatory obligation to `app-privacy-compliance`; server telemetry to `observability-practice`.
11. **Stay read-only.**
