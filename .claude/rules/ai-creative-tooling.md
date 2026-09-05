---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-05
---

# AI in creative production

A reference for advising on generative tooling as a working practice: where it helps and fails in a content workflow, ideation discipline, AI dialogue and runtime generation, provenance and disclosure obligations, generated-asset copyright posture, and the labor and consent debate. Used by the `ai-creative-tooling` subagent and the `/expert-review` / `/expert-plan` / `/expert-consult` skills.

The unifying thesis, and the single most useful test in the domain: **generative tooling helps where errors are cheap and visible, and fails where errors are expensive or invisible.** That one distinction predicts the successes and failures better than any split between art, code, and design. Concept boards work because a bad one costs nothing and you can see it instantly. Art direction across an asset set fails because the error is a property that exists *between* assets, and nothing in the generation loop can see it.

The operational question: **"is the error this produces cheap and visible, or expensive and invisible?"**

Empirical priority, in rough order of consequence: **provenance and disclosure obligations > licensing and copyright posture > matching the tool to error-visibility > the last-10% cost > ideation discipline > policy and team-consent questions.** Provenance ranks first because it is the one that becomes a compliance problem rather than a quality problem.

## Volatile surface

`last-verified: 2026-09-05`. **This file's legal and platform-policy material rots fastest and matters most.** It is jurisdiction-dependent, changes monthly, and none of it is legal advice.

| Claim class | Rots | Re-verify at |
|---|---|---|
| Platform AI-disclosure policies | Fast | The platform's own announcement feed |
| Copyright office guidance and litigation status | Fast | copyright.gov and primary dockets |
| Regulatory obligations (EU AI Act timelines) | Fast | The regulator's own publications |
| Tool and vendor capability claims | Very fast | The vendor, treated as a claim |
| Industry sentiment survey figures | Fast | The actual report; widely misquoted |
| The error-visibility principle and structural failure modes | Slow | Durable; follows from the architecture |

## Disclosure and provenance

**Steam's disclosure requirement is the concrete case, and it is mandatory rather than optional metadata.** Valve's "AI Content on Steam" policy (posted 2024-01-09; **verify current text before relying on it**) splits the obligation in two:

- **Pre-Generated** -- any content created with AI tools during development. The developer's existing promise that the game contains no illegal or infringing content applies to AI output identically. Valve evaluates it the same way as non-AI content.
- **Live-Generated** -- content created by AI while the game runs. Same rules, **plus an affirmative obligation to describe the guardrails preventing illegal generation.**

Three operational consequences follow, and they are what make this an engineering concern rather than a paperwork one:

1. **The disclosure is published on the store page.** It is a marketing surface whether or not the team wants it to be.
2. **Valve is not indemnifying anyone.** The policy restates that the non-infringement promise applies; risk stays with the developer.
3. **A studio that used AI tooling without tracking where cannot complete the survey accurately.** This is the concrete, non-hypothetical reason provenance capture is an engineering requirement rather than good hygiene.

For live generation there is an additional cost consequence: **shipping content moderation becomes a launch requirement, not a nice-to-have**, which materially changes the cost model for AI dialogue. Valve also runs a player-reporting channel through the in-game overlay for live-generated content, and prohibits Adult Only sexual content generated live.

**What provenance capture means in practice**: prompt, seed, model name and version, tool versions, and which asset resulted. Without it, the asset cannot be regenerated, varied, defended, or disclosed accurately.

## Copyright posture

**Not legal advice.** The engineering-relevant shape, as of the verification date:

The US Copyright Office's report series treats **copyrightability** (Part 2, final, 2025-01-29) separately from **training** (Part 3, still pre-publication as of 2026-09-05 -- **do not cite Part 3 as settled Office policy**). Part 2's core conclusion is that copyright protects original human expression in a work **even if the work also includes AI-generated material**. Registration guidance sits at 88 Fed. Reg. 16,190 (2023-03-16).

The practical translation: **AI-generated material may carry no copyright of its own.** For a studio, imagery and assets are normally owned business assets, so this is a business-model question and not only an ethics one -- an asset nobody can own is an asset nobody can stop a competitor from reusing.

Training-data litigation is in varying postures across multiple suits and moves monthly. The EU AI Act imposes transparency and training-data-summary obligations on a timeline. Vendor indemnification offers exist and generally cover far less than they appear to. **Every one of these needs primary-source verification and counsel before it gates a shipping decision.**

## Where it helps and where it fails

The pattern across every successful use is the same: **the output is disposable, or it is a draft a human will replace, or the error is immediately visible.**

**Genuinely useful today:**

- **Concept ideation and moodboards.** The strongest current use. Output is explicitly disposable, a bad one costs nothing, and volume has real value during divergence. See the anchoring counter-argument below.
- **Greybox, blockout, and placeholder assets.** The single best fit between the technology's actual capability and a production need: generated 3D produces structurally plausible geometry with bad topology, and topology does not matter for a blockout.
- **Dialogue and bark variation drafts.** Generating forty variants for a writer to cut to six. The human still selects, which is where the quality comes from.
- **Localization first passes** into a human loop. The industry's objection is not that drafts are useless but that "draft plus review" gets priced and scheduled as review, when **fixing a fluent-but-wrong translation is often slower than translating**.
- **Code assistance** -- the most mature use and the least game-specific.
- **QA, crash triage, log clustering** -- pattern-matching with a checkable answer.
- **Asset variants and upscaling** where consistency requirements are low.

**Structural failures, which are not temporary:**

- **Coherent art direction across an asset set.** Each generation is independent. Art direction is precisely the property that holds *between* assets -- shared palette, material language, silhouette logic, level of stylization. A model producing one excellent asset at a time has no mechanism for the thing that makes a set read as one game. Style-consistency techniques mitigate this; they do not solve it.
- **Game feel.** Feel lives in frame-level timing: input buffering, coyote time, hit stop, cancel windows, camera curves. It is tuned iteratively by a human holding a controller. There is no text description of it, so there is nothing to have learned from.
- **Level layout that plays well.** A model can produce a floor plan that looks like a level. Whether it *plays* depends on sightlines, pacing, encounter spacing, and critical-path readability -- none visible in the layout, all requiring play. This is the perception-loop problem at a higher altitude: the artifact is judged by a property the generator cannot observe.
- **Cross-asset consistency** -- same root cause as art direction.
- **The last 10%.** Generated output plateaus at "structurally plausible." Shipping requires *correct*: topology, UVs, scale, naming conventions, LODs, collision. That stretch is most of the labor, and it is exactly where generation contributes least.

## AI dialogue and runtime generation

The pitch is infinite dialogue. The critique is that **infinite dialogue is not writing**: writing is selection and compression, and a system that generates unboundedly has removed the operation that produces quality. What players remember is a line someone chose to include.

The engineering costs are concrete: per-interaction latency in a real-time loop, per-interaction cost at scale, and the guardrail obligation above, which turns moderation into launch-blocking work. The gap between vendor demos and shipped products in this space remains wide, and vendors in the category have visibly broadened beyond games -- a signal worth reading.

The honest positioning: it works where dialogue is ambient and low-stakes (barks, flavour, systemic reaction) and struggles where dialogue carries plot, character, or a promise the game must keep.

## Procedural generation and the oatmeal problem

Kate Compton's "10,000 bowls of oatmeal" formulation applies directly and is the sharpest available tool for evaluating generated content: **you can generate ten thousand mathematically unique bowls of oatmeal, and to a human they are all just oatmeal. Perceptual uniqueness, not mathematical uniqueness, is what matters.**

Generated asset sets fail this test constantly. The variation is real in parameter space and invisible in perception space, because the model varies detail while holding structure, and structure is what people read. The fix is structural variety at the level people perceive, which is much harder than more sampling.

## Ideation discipline

**The anchoring failure is the most important one.** Early AI output narrows the design space rather than widening it: the team converges on the first striking image instead of exploring, so a tool bought for divergence delivers premature convergence. The mitigation is procedural -- generate widely before looking at anything, review in batches rather than one at a time, and hold the brief before generation rather than letting output define it.

**The buildability critique.** AI concept art optimizes image plausibility, not physical, topological, or riggable coherence. The result is designs that look excellent and cannot be modeled, rigged, or built -- and the cost lands on whoever has to make it, usually after it has been approved. A concept that cannot be executed is not a concept, it is a mood board with a false promise attached.

**Model as critic beats model as generator.** Asking for a critique of an existing design uses the model where its errors are cheap and visible, and keeps authorship with the person. It is the most underused pattern in the space.

**Keep a human art director in the loop** for the same reason art direction fails structurally above: consistency across a set is a human judgment call, and nothing in the generation loop makes it.

## Evaluating creative output

There is no unit test for "looks good." The available proxies, in order of reliability: structural assertions (does it import, does it meet the budget, does it satisfy the pipeline's validator), regression against a golden output at fixed seed and settings, downstream validation, and human spot-check at a defined rate. "The model said it looks good" is unmeasured, and treating it as measured is the field's most common evaluation error.

## Schools of thought

### Adoption versus refusal

**The pro-adoption position.** These are tools, and artists have always adopted tools that automate the tedious parts. Ideation, variation, and drudgery are exactly where they help. A small team gains capability it could not otherwise afford, and refusing on principle costs the artist rather than the vendor.

**The critical position.** The models were trained on working artists' output without consent, licence, or compensation, and are sold as substitutes for the labour they were trained on. The harm is to identifiable people whose portfolios are in the training set, not to a craft abstraction. Adoption normalizes the appropriation regardless of individual intent, and "it is just a tool" is a category error about where the value came from.

**Both are recorded because both identify something real, and the reconciliation people reach for -- "use it ethically" -- resolves nothing about the training data that already exists.** A team's decision is a policy question with legal, ethical, and practical dimensions. Name the dimensions; do not settle them.

**One empirical note worth carrying**: developer sentiment moved *against* generative tooling through 2025 even as adoption rose. In the Game Developer Collective / Omdia survey, concern that AI would negatively affect game quality rose from 34% to 47% year over year while optimism fell from 17% to 11%. Optimism did not track availability. **Do not quote GDC State of the Game Industry figures from memory** -- they are widely misquoted and the editions ask different questions; retrieve the report.

### Productivity gain versus quality floor

**The gain position.** Measured throughput on drafts, variants, and boilerplate is real and large.

**The floor position.** The gain is concentrated in the first 90%, and the last 10% is most of the work. A pipeline that produces more plausible-but-wrong output faster has moved labour downstream to whoever fixes it, and the throughput metric does not capture that transfer.

## Anti-pattern catalog

| Pattern | Trigger | Consequence | Fix |
|---|---|---|---|
| No provenance record | One-shot chat generation | Cannot regenerate, defend, or disclose accurately; the platform survey cannot be completed | Capture prompt, seed, model version, tool versions, and the resulting asset |
| Generated asset shipped as final | Impressive preview quality | Topology, UVs, and naming fail the pipeline; the last 10% lands on someone else | Scope generation to blockout and reference; budget the retopo |
| Live generation without moderation | Shipping an AI NPC feature | Guardrails are a platform obligation; launch blocks on it | Budget moderation as launch-critical, not post-launch |
| First striking image adopted | Reviewing output one at a time | Anchoring; premature convergence disguised as divergence | Generate widely before looking; review in batches |
| Concept approved without buildability check | Image plausibility optimized, not physical coherence | Unmodelable or unriggable design approved; cost lands downstream | Have the person who must build it review before approval |
| Parameter variation called variety | Generator ships uniform-feeling output | Ten thousand bowls of oatmeal | Vary structure at the level people perceive |
| Localization draft scheduled as review | Draft-plus-review framing | Fixing fluent-but-wrong text is often slower than translating | Schedule and price it as translation |
| Model output as its own evaluator | No test for "looks good" | Unmeasured quality presented as measured | Structural assertions, golden regression, defined human sampling |
| Citing Part 3 or survey figures from memory | Reasonable recall | Both are widely misquoted and one is not final policy | Retrieve the primary source |
| Assuming vendor indemnification covers you | Reading the marketing | Coverage is narrower than it appears | Read the terms; involve counsel |

## Severity rubric for this lens

- **blocker** -- a compliance or licensing exposure that ships: undisclosed AI content where disclosure is mandatory, live generation with no guardrails, an asset whose licence forbids its use, no provenance where the platform requires an accurate survey.
- **major** -- a structural quality or cost problem: generated assets scoped as final, moderation deferred past launch, art direction delegated to a process that cannot produce it, evaluation resting on model self-report.
- **minor** -- process friction: anchoring risk in the ideation workflow, missing batch review, avoidable cost.
- **nit** -- workflow preference.
- **insight** -- a reframe: this wants the model as critic rather than generator; this variation problem is structural, not a sampling problem; this productivity gain has moved labour downstream rather than removing it.

## Authorities

- **The platform's own policy pages** -- Valve's Steamworks announcements and equivalents. The only authority on disclosure obligations, and they change.
- **copyright.gov's AI report series** -- read the actual reports; Part 2 is final and Part 3 is not, and the distinction matters.
- **Primary court dockets** for litigation status. Never a secondary summary.
- **Kate Compton** -- the "10,000 bowls of oatmeal" problem. The essential evaluative tool for generated content.
- **The Game Developer Collective / Omdia and GDC State of the Game Industry surveys** -- the sentiment data, read from the reports rather than from coverage.
- **Working artists' own writing on the labor and consent question** -- the critical position is best stated by the people affected, and a summary of it is not a substitute.

## Changelog

- **2026-09-05** -- Initial version. Steam's disclosure policy quoted from Valve's own 2024-01-09 announcement, retrieved directly; **whether Valve amended it between then and the verification date was not confirmed** and should be checked. Copyright Office report status verified at copyright.gov, including that Part 2 is final and Part 3 remains pre-publication. Omdia / Game Developer Collective sentiment figures verified from the reported survey; **GDC State of the Game Industry figures were not retrievable and are deliberately absent rather than quoted from memory.** The where-it-helps-and-fails analysis is reasoned from verified technical constraints rather than from practitioner surveys, which were unreachable; treat the specific claims as well-founded reasoning rather than as surveyed fact.
