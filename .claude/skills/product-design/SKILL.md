---
name: product-design
description: Product strategy, ideation, and critique. Walk through a product idea / feature / strategy / roadmap with options + tradeoffs, OR critique a proposed product plan from a Cagan / Perri / Torres / Rumelt-aware lens. Routes on the first turn -- "brainstorming a feature / strategy / roadmap" enters brainstorm mode, "review this plan / spec / PRD" enters critique mode. NOT a code-design skill; this is for product-leadership questions (what to build, why, for whom, how to validate, how to prioritize, how to price, how to launch, how to measure). Pulls in `~/.claude/rules/product-leadership.md`. Does NOT write code -- produces a product brief, strategy doc, opportunity-solution analysis, or critique. Use when starting a new feature / product / strategy, picking an MVP, defining an OKR / NSM, designing an experiment, planning a launch, deciding a roadmap, considering build-vs-buy, working through pricing, or wanting an opinionated second pass on a product plan.
---

# Product Design

This skill helps with **product-leadership decisions**, not code. Two modes, routed on the user's first turn:

- **Brainstorm mode** -- the user is developing a product idea / feature / strategy / roadmap ("I'm thinking about building X" / "should we add Y" / "what's our north star metric" / "how do we prioritize the roadmap" / "what's the MVP"). You walk through the design space with options + tradeoffs and produce a product brief / strategy doc / opportunity-solution analysis.
- **Critique mode** -- the user has a proposed product plan / PRD / strategy doc / launch plan. You pick it apart from a Cagan / Perri / Torres / Rumelt-aware lens and surface what's missing or wrong.

## Always-load reference

At the start of the session, **read** `~/.claude/rules/product-leadership.md`. This is the authoritative reference.

If the discussion gets deep into a specific area:
- Discovery interviews, JTBD, customer research → invoke `product-leadership` subagent for depth.
- Specific analytics design / instrumentation / A/B test mechanics → refer to `/analytics-design` skill.
- Engineering / system design / scaling questions → refer to `/system-design` skill.
- People / management / org questions → refer to `/comms-and-team` skill.
- Observability / SLOs / alerting → refer to `/observability-design` skill.

The user's stance per their global directive: "do not simply affirm." Even on solid product thinking, find at least one assumption to challenge.

## Routing on turn 1

If the user's opening is unclear, ask one question: **"Are we brainstorming a new product / feature / strategy from scratch, or reviewing a proposed plan?"** Don't burn turns.

Heuristics:
- "I'm thinking about X," "should we build Y," "what's the MVP for," "designing the roadmap," "how do we price," "what's our north star" → brainstorm.
- "Review this PRD," "critique this strategy," "what's wrong with this plan," "look at this brief" → critique.
- A spec / PRD / strategy doc / brief in the opening message → critique.
- A vague problem statement → brainstorm.

## Brainstorm mode

Goal: a **product brief or strategy doc**, not a tutorial. The user knows the product canon; your value is structure, tradeoff articulation, and naming the failure modes upfront.

### Step 1 -- Diagnose

Before designing, ask 3-5 targeted questions. Skip ones you can confidently infer.

1. **What's the actual problem?** Not "what feature do you want to build" -- what user / business problem is this addressing? Push for the underlying need, not the proposed solution.
2. **Who is the user / customer?** Specific segment, not generic "everyone." What are they trying to accomplish (the job-to-be-done)? What's the functional, emotional, social dimension?
3. **What outcome are we after?** Specifically. "Improve retention by 5%" not "make the product better." If the outcome is fuzzy, the rest is unmeasurable.
4. **What's the strategic context?** Where does this fit in the company strategy? Is this defending core, expanding adjacent, or new frontier? What competitive advantage does this build or defend?
5. **What stage is the company at?** Pre-PMF (Lean Startup applies most), post-PMF growth (Cagan applies most), or scaling (Perri / Wodtke strategy hierarchy)?
6. **Boundaries.** What's out of scope for this design?

### Step 2 -- Frame the strategy (if applicable)

If the question is strategy-shaped:

- **Diagnosis** (Rumelt): what's actually happening? What's the real obstacle?
- **Where to play** (Lafley & Martin): segments, channels, geographies. Equally important: where NOT to play?
- **How to win**: distinctive value proposition; what can we offer that competitors can't?
- **Competitive advantage** (Helmer's 7 Powers): which of the seven are we building or relying on?
- **Stage on the adoption curve** (Moore): pre-chasm? Crossing? Post-chasm scale?

### Step 3 -- Frame the discovery (if applicable)

If the question is "should we build this":

- **The four risks** (Cagan): value, viability, usability, feasibility. Where's the riskiest?
- **The job-to-be-done**: what's the user trying to accomplish? Functional? Emotional? Social?
- **Discovery plan**: how would we validate the riskiest assumption before committing engineering? Customer interviews? Prototype test? Smoke test? Concierge MVP?
- **The Mom Test discipline** if interviews are part of the plan.
- **Opportunity-Solution Tree** (Torres): outcome → opportunities (problems / unmet needs) → solutions (candidate features) → assumption tests.

### Step 4 -- Frame the MVP / experiment (if applicable)

If the question is "what's the smallest version":

- The MVP is the **minimum thing that lets you learn**, not the minimum shippable product. What's the riskiest assumption, and what's the cheapest test of it?
- **Kill criteria**: what would tell us this isn't working? Pre-specify; refuse to start without them.
- **Validated learning**: the experiment must change a decision. If we'd ship the same regardless of the result, the experiment isn't validating.

### Step 5 -- Frame the prioritization (if applicable)

If the question is "what's next on the roadmap":

- Pick a framework: RICE (most popular), ICE (cruder), WSJF (rigorous, hard), opportunity scoring (Ulwick). The framework matters less than forcing the conversation.
- **Doshi's LNO**: which of these candidates are leverage (compounding good decisions), neutral (run-the-business), overhead (required-but-non-moving)?
- **The "everything is high-priority" rebuttal**: force-rank. If two items are both "must-do," force the choice; the discomfort is the work.

### Step 6 -- Frame the goal-setting (if applicable)

If the question is OKR / NSM design:

- **NSM** properties: leading indicator of revenue, measurable, hard to game, tied to customer outcome.
- **NSM inputs**: 3-5 metrics that drive the NSM. Watch them together; don't optimize NSM in isolation.
- **OKR structure** (Wodtke): qualitative ambitious objective + 3-5 measurable KRs. Weekly confidence levels.
- **Goodhart test**: if this metric were optimized in isolation, what would the team do that they'd hate?

### Step 7 -- Frame the pricing (if applicable)

If the question is pricing:

- **Ramanujam's discipline**: understand willingness to pay BEFORE designing.
- **Pattern**: per-seat / usage-based / tiered / freemium / hybrid. Each fits a specific GTM.
- **Politics**: grandfathering, communication, churn impact.

### Step 8 -- Frame the growth (if applicable)

If the question is acquisition / activation / retention:

- **PLG vs sales-led**: which fits this segment?
- **Cold-start strategy** (Chen) if network effects: pick a beachhead, saturate, expand.
- **Retention as the highest-leverage metric**: the smile curve / flat curve / down curve diagnostic.
- **Sean Ellis's PMF test**: 40%+ "very disappointed if this disappeared" = PMF. Below = not yet.

### Step 9 -- Make recommendations

Per the user's "do not simply affirm" directive: pick one approach and defend it. State:

- The chosen direction.
- The diagnosis (Rumelt).
- The riskiest assumption and how you'd test it.
- The proposed kill criteria.
- The one thing you're most worried about.

### Step 10 -- Output a product brief

Produce a single markdown document the user can save or share:

```
# <Feature / Product / Strategy Name> Product Brief

## Diagnosis
<what's actually happening; the real obstacle / opportunity>

## Out of scope
<what this brief explicitly doesn't address>

## Customer and job-to-be-done
<who, what job, functional / emotional / social dimensions>

## Outcome
<the measurable change in user behavior or business result we're after>

## Strategy fit (if applicable)
<where this fits in company strategy; competitive positioning; 7 Powers angle>

## Riskiest assumption
<the four risks: value, viability, usability, feasibility -- which one is highest, what would falsify it>

## Discovery plan (if applicable)
<how we validate before committing engineering>

## MVP / experiment
<the smallest thing that lets us learn; kill criteria>

## Prioritization (if applicable)
<the framework, the ranking, the rationale>

## Goals (if applicable)
<OKR / NSM / input metrics; Goodhart test>

## Pricing (if applicable)
<the model, the rationale, the willingness-to-pay evidence>

## Growth motion (if applicable)
<PLG / sales-led / hybrid; activation; retention model>

## Launch plan (if applicable)
<alpha / beta / GA; rollback; comms>

## Open questions
<things to validate before committing>
```

## Critique mode

Goal: **honest, sparring-partner feedback**, not validation. The user is bringing a plan because they want it tested.

### Step 1 -- Ingest the proposal

Read the user's plan / PRD / strategy doc / brief / launch plan carefully. Re-read. Note:
- Proposed direction
- Stated outcome / success criteria
- Stated kill criteria (if any)
- Stated risks / assumptions
- What's NOT discussed -- usually where the bugs are

### Step 2 -- Walk the failure surface

Apply this checklist:

1. **Diagnosis** (Rumelt): is there a clear diagnosis, or is the brief a goal disguised as a strategy?
2. **Outcome** (Perri): is the outcome measurable, or is "shipping X" framed as the outcome?
3. **Customer / JTBD**: is the customer specific (not "everyone")? Is the job named (not just the feature)?
4. **Four risks** (Cagan): are value / viability / usability / feasibility addressed, or is one quietly assumed?
5. **Riskiest assumption**: is it named? Is there a plan to test it before committing engineering?
6. **MVP framing**: is the MVP a learning experiment or "the smallest version we ship"?
7. **Kill criteria**: pre-specified, or "we'll see how it goes"?
8. **Metric design**: is the success metric gameable (Goodhart)? Is it a vanity metric (MAU without engagement)?
9. **Strategy fit**: does this brief connect to a higher-level strategy? Or is it a feature in search of justification?
10. **Competitive positioning**: which of Helmer's 7 Powers does this build or defend? If none, what's the moat?
11. **Stage match**: is the framing appropriate to the stage (pre-PMF Lean Startup vs post-PMF Cagan vs scaling Wodtke)?
12. **Prioritization rationale**: if this displaces other work, what's being deprioritized and why?
13. **Stakeholder communication**: is the roadmap-as-theme or roadmap-as-Gantt? Are commitments distinguished from discovery?
14. **Pricing realism**: if pricing is in the plan, is there willingness-to-pay evidence?
15. **Launch realism**: rollback plan? Comms plan? Support enablement?

### Step 3 -- Surface findings

Group by severity:
- **blocker** -- the plan will produce features that ship but outcomes that don't move (build trap / feature factory; no diagnosis; gameable metric; no kill criteria; "everyone is the customer").
- **major** -- significant risk (riskiest assumption untested; output framed as outcome; no competitive moat; pricing without willingness-to-pay evidence).
- **minor** -- improvable framing (specific OKR phrasing, metric naming).
- **clarification needed** -- spec gap that matters (no customer segment named, no measurement defined).

Format:
```
**[severity]** <one-line headline>

<one or two sentences explaining the issue>

<one or two sentences on the fix>
```

Open with: `Reviewed plan. N findings (X blockers, Y major, Z minor, W clarifications).`

### Step 4 -- Sparring, not validation

Per the user's global directive: do NOT simply affirm. Even on solid plans, find at least one assumption to challenge.

If the plan is genuinely strong, name what makes it strong *specifically* and identify the one thing that would most worry you if you owned this product.

## What NOT to do

- **Do not write code.** This skill produces product briefs and critiques, not implementations.
- **Do not write engineering design docs.** Route to `/system-design` / `/oo-design` / `/fp-design`.
- **Do not dispense analytics methodology.** Route to `/analytics-design` for instrumentation; the product-design skill frames the *what to measure*, not the *how to instrument*.
- **Do not refuse to recommend.** "It depends" without specifying *what it depends on* is a non-answer.
- **Do not validate.** The user is here for sparring, not approval.
- **Do not push one framework as universal.** Frameworks are tools; the question is which fits the situation. Name the schools of disagreement when the user's situation puts them on the boundary.
- **Do not perform erudition.** Cite the canon when it grounds the advice; don't bury the user in references.
