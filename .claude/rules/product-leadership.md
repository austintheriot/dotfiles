# Product Leadership and Product Management Craft

A reference for product strategy, discovery, prioritization, goal-setting, MVP design, pricing, growth, roadmap, launch, and the PM craft generally. Used by the `product-leadership` subagent and the `/product-design` skill.

This is an **advisor** lens, not a code-review lens. The assistant invokes this when the user asks product questions: "should we ship this," "how do I prioritize this roadmap," "what's the MVP for X," "is this a good idea," "what's our pricing," "how do I run this launch."

**Not auto-included in `/expert-review`.** Product questions are not code-shape questions.

The core thesis (the through-line of Cagan / Perri / Torres / Cutler / Wodtke): **most product work in most companies produces features that ship but do not move the business.** The PM craft, properly practiced, is the discipline of closing that gap. It does so by (a) tying every initiative to a measurable outcome rather than a deliverable, (b) discovering whether the proposed solution will actually produce the outcome before committing engineering capacity, and (c) empowering the team closest to the work to choose how to achieve it.

Cagan calls the bad pattern the "feature factory." Perri calls it the "build trap." Cutler catalogs its symptoms. Torres designs a weekly habit to prevent it. Wodtke prescribes OKRs to combat it. Doerr brought OKRs from Grove at Intel to Google to combat it. The disagreements that follow are mostly about how, not whether, to escape this pattern.

The earlier canon (Christensen, Moore, Porter, Rumelt, Reinertsen) sits one layer up: it's concerned with strategy, market structure, competitive dynamics, the question of which products to build at all. The synthesis: **strategy answers "which game are we playing"; product management answers "are we winning the game we're in."**

---

## Strategy: picking the game

### Rumelt's strategy kernel (the most-cited modern strategy text)

Richard Rumelt, *Good Strategy / Bad Strategy* (2011). A strategy has three parts:

1. **Diagnosis**: a clear-eyed statement of what's actually happening. "Most strategies fail because the diagnosis is wrong or vague."
2. **Guiding policy**: an overall approach that responds to the diagnosis. Not a goal; a policy.
3. **Coherent actions**: the specific steps, mutually reinforcing.

**Bad strategy** is recognizable by: fluffy vision statements with no diagnosis ("be the leader in..."), goals confused with strategy ("grow 30%"), failure to confront the actual obstacle, inability to choose what NOT to do.

**The reviewer's question**: ask the user for their diagnosis. If they can't articulate it crisply, they don't yet have a strategy; they have an aspiration.

### Lafley and Martin's choice cascade

*Playing to Win* (2013). Five questions, answered in order:

1. **What is our winning aspiration?** Not "be #1"; what does winning look like for this product / company?
2. **Where will we play?** Which markets, customers, channels, geographies, segments? Equally important: where will we NOT play?
3. **How will we win?** What's our distinctive value proposition? What can we offer that competitors can't or won't?
4. **What capabilities must we have?** Skills, assets, partnerships.
5. **What management systems do we need?** Org structures, metrics, incentives.

Most product strategy conversations skip questions 1 and 2 and jump straight to 3. The result is incoherent strategy: trying to win everywhere, against everyone.

### Helmer's 7 Powers

Hamilton Helmer, *7 Powers* (2016). The seven persistent competitive advantages:

1. **Scale Economies** — unit costs decline with scale (AWS, Amazon).
2. **Network Economies** — value increases with users (Facebook, eBay).
3. **Counter-Positioning** — incumbent can't match without cannibalizing their own business (Vanguard vs Fidelity).
4. **Switching Costs** — high cost for customer to leave (Salesforce, SAP).
5. **Branding** — premium for trusted brand (Tiffany, Patek Philippe).
6. **Cornered Resource** — preferential access to a critical input (Pixar's talent in late-90s).
7. **Process Power** — embedded operational capabilities (Toyota Production System).

If your strategy doesn't articulate which of these you have or are building, you're competing on execution, which is fragile.

### Porter's Five Forces

Older (*Competitive Strategy*, 1980), still essential. Industry profitability is determined by:
- Threat of new entrants
- Bargaining power of suppliers
- Bargaining power of customers
- Threat of substitutes
- Rivalry among existing competitors

### Moore's Crossing the Chasm

*Crossing the Chasm* (3rd ed 2014). The technology adoption lifecycle: innovators → early adopters → **chasm** → early majority → late majority → laggards. Most products die in the chasm: early adopters love them; the early majority doesn't. The fix: pick a beachhead segment in the early majority, dominate it, expand.

Crucial for B2B / enterprise; less directly applicable to viral B2C.

### Christensen's disruption theory

*The Innovator's Dilemma* (1997). Sustaining innovation improves a product along its current performance axis; disruption introduces a new axis that customers eventually value more. Established companies can't pursue disruption because their best customers don't want it (yet). Most "disruption" claims are misuses of the term; real disruption is rare and specific.

---

## Discovery: understanding the players

### The Cagan / Torres synthesis

Marty Cagan (*Inspired* 2017, *Empowered* 2020, *Transformed* 2024) and Teresa Torres (*Continuous Discovery Habits* 2021):

- **Empowered product teams**: PM + designer + engineer, owning outcomes, with the authority to choose how.
- **The four risks**: every product idea is risky on at least one of:
  1. **Value risk** — will users buy / use this?
  2. **Viability risk** — does this work for our business?
  3. **Usability risk** — can users figure out how to use it?
  4. **Feasibility risk** — can we build this in a reasonable time?
- **Continuous discovery**: weekly customer interviews (Torres: a minimum of one per week as a baseline habit). Discovery is not a phase before delivery; it's a parallel ongoing practice.
- **Opportunity-Solution Tree (OST)**: outcome → opportunities (customer problems / unmet needs) → solutions (candidate features) → assumption tests. Make the tree visible; let the team see the reasoning.

### Jobs-to-be-Done (JTBD)

Clayton Christensen (*Competing Against Luck* 2016) and Tony Ulwick (*Jobs to Be Done* 2016) and Alan Klement (*When Coffee and Kale Compete* 2016).

"People don't buy products; they hire them to do a job."

The job has three dimensions:
- **Functional**: what task is the user trying to accomplish?
- **Emotional**: how does the user want to feel?
- **Social**: how does the user want to be perceived?

Most product failures result from understanding only the functional job and missing the emotional or social one.

The personas-vs-JTBD argument is real: JTBD advocates argue personas are noise (people don't have characteristics, situations do); persona advocates argue the human-centered grounding matters. The pragmatic move: use both. Personas to ground discovery in real humans; JTBD to frame the actual purchase / use decision.

### The Mom Test (Fitzpatrick, 2013)

How to do customer interviews without biased answers.

**The three rules**:
1. **Talk about their life, not your idea.** Don't pitch; ask about their actual problems.
2. **Ask about specifics in the past, not generics about the future.** "Tell me about the last time you..." not "Would you use a tool that...?"
3. **Talk less, listen more.**

If you can't ask questions that would survive your own mom (who would tell you your idea is great regardless), the interview is biased.

### Discovery anti-patterns

- **Solution-first thinking**: jumping to features before understanding the problem.
- **The "we asked customers what they want" trap** (Ford's "faster horse"; Jobs's "show them what they want"). Customers know their problems, not your solutions.
- **One-time research**: discovery as a phase, not a practice.
- **Internal proxies for customers**: sales tickets, support escalations, engineering opinions standing in for actual interviews.
- **Confirming-not-falsifying**: looking for evidence that supports your idea, not evidence that breaks it.

---

## Prioritization: what to move next

### The framework zoo

- **RICE** (Sean McBride, Intercom): `Reach × Impact × Confidence / Effort`. The most-popular framework. Reach = users affected. Impact = how much per user. Confidence = how sure are you (decimal 0-1). Effort = person-weeks.
- **ICE** (Sean Ellis): `Impact × Confidence × Ease`. Predecessor to RICE; cruder.
- **Kano model** (Noriaki Kano): features classified as Must-have, Performance, Delighter, Indifferent, Reverse. Different feature classes should be invested in differently.
- **MoSCoW**: Must / Should / Could / Won't. Older but widespread.
- **WSJF** (Don Reinertsen): `Cost of Delay / Job Size`. From lean / queue theory. The most rigorous; the hardest to use in practice (cost of delay is hard to estimate).
- **Opportunity scoring** (Tony Ulwick): `Importance + (Importance - Satisfaction)`. Where the customer says "this is important AND I'm not satisfied" → biggest opportunity.

**What the frameworks share**: forcing a numeric score makes implicit tradeoffs explicit. **What they don't share**: how to estimate the numbers. The numbers are mostly fiction; the value is the conversation forced by trying to assign them.

### Shreyas Doshi's LNO framework

Every PM task is one of:
- **Leverage** (L): one good decision that compounds. Strategy, hiring, pricing, key partnerships.
- **Neutral** (N): keeps the business running. Most operational work.
- **Overhead** (O): doesn't move anything but is required (status updates, some meetings).

The discipline: spend disproportionate time on L tasks; minimize O. Most PMs are inverted -- drowning in O, neglecting L.

### Cost of Delay (Reinertsen)

The economic cost of *not* having a feature now. A 6-month delay on a $1M/month feature = $6M cost of delay. WSJF asks: what's the cost-of-delay-per-week / job-size? Highest ratio first.

In practice, cost of delay is rarely estimated quantitatively; the framework's value is forcing the question.

### Kill criteria

Pre-specified conditions that, if hit, cancel the project. The antidote to sunk-cost bias.

Example: "If we don't see a 5% conversion lift by week 4 of the test, we kill this." Set before starting; honor when hit.

**Flag** (when advising the user): proposed initiatives without kill criteria; "we'll see how it goes" framings; refusal to specify what failure looks like.

### The "everything is high-priority" problem

If everything is high-priority, nothing is. Force-rank. Force the choice between two items that are both "important." The discomfort is the work.

---

## Goal-setting: OKRs and the North Star

### OKR mechanics (Doerr / Wodtke)

- **Objective**: qualitative, ambitious, inspiring. "Become the de facto note-taking app for students."
- **Key Results**: 3-5 measurable outcomes. Not tasks; outcomes. "Reach 50% of US college students," "achieve 40% week-2 retention," "hit 4.8 star rating with 10k reviews."

Christina Wodtke (*Radical Focus*) is the best modern reference. Her improvements on Doerr:
- **Confidence levels** (5/10, 7/10) updated weekly. Forces honesty about whether the KR is on track.
- **Friday wins / Monday commitments**. Lightweight cadence.
- **One objective at a time per team**.

### The North Star Metric debate

Single-metric advocates (Sean Ellis, Amplitude): one metric that captures the value the product delivers. Spotify: time spent listening. Airbnb: nights booked. Notion: weekly active editors.

Balanced-scorecard advocates: one metric is gameable; multiple metrics constrain each other.

The synthesis: NSM as the headline, with 3-5 input metrics (the "NSM tree") that prevent gaming. Don't optimize NSM in isolation; watch the inputs.

### OKR theater and Goodhart's Law

**Goodhart's Law** (Charles Goodhart, 1975, restated by Marilyn Strathern): "When a measure becomes a target, it ceases to be a good measure."

OKRs that became gameable rituals:
- Velocity-as-KPI: engineers inflate story points.
- MAU-as-KPI: marketing buys low-quality users.
- NPS-as-KPI: surveys gamed.

**The reviewer's question**: if this metric were optimized in isolation, what would the product do that the team would hate? If you can name something bad, the metric is gameable.

---

## The MVP and the validated-learning loop

### Ries and the Lean Startup canon

Eric Ries, *The Lean Startup* (2011). Build-Measure-Learn. The MVP is **the minimum product that lets you learn**, not the minimum shippable product. The two are routinely confused.

The "validated learning" claim: the experiment must change a decision. If you'd ship the same regardless of the result, it's not a validating experiment.

### When the Cagan / Lean Startup tension matters

Cagan was originally skeptical of Lean Startup as too consumer-startup. *Inspired* targets the empowered-team model in larger companies that have product/market fit and need to scale. Lean Startup targets pre-PMF discovery.

**Diagnosis question for the user**: are you pre-PMF (Lean Startup applies most) or post-PMF (Cagan applies most)? Most PM advice treats them as the same problem; they aren't.

---

## The outcomes-vs-outputs movement

**Output**: things shipped. Features, releases, bug fixes.
**Outcome**: changes in behavior, business results. Users using a feature, revenue, retention.

The Cagan / Perri / Cutler critique: most product orgs measure outputs and call them outcomes. Roadmaps full of features; OKRs full of "ship X, ship Y."

**The reviewer's question** for any proposed initiative: what's the outcome we're after, and how would we know if we got it? If the answer is "we'll have shipped X," the framing is output-driven.

### The Build Trap (Perri) / Feature Factory (Cutler)

Same pathology, two names. The org rewards shipping; the metric is throughput; outcomes go un-measured because the work is already credited.

Symptoms:
- Roadmaps with no outcome columns.
- Quarterly reviews that count features, not impact.
- "Done = launched" instead of "done = working for users."
- No mechanism to kill features that shipped but didn't move anything.

---

## Pricing and monetization

### Ramanujam's price-led product development

*Monetizing Innovation* (Ramanujam & Tacke, 2016). The thesis: most products are built then priced. The result: 72% of new products fail to hit their financial goals. The fix: **understand willingness to pay BEFORE designing the product.**

Four pricing mistakes (every chapter is one):
1. **Feature shocks**: building features customers don't value.
2. **Minivations**: under-pricing breakthrough products.
3. **Hidden gems**: products that could be a business in their own right, undersold as features.
4. **Undeads**: products that should have been killed but are kept alive.

The discipline: customer-development interviews ask about willingness to pay early.

### SaaS pricing patterns

- **Per-seat**: classic SaaS, simple, scales with org size.
- **Usage-based**: aligned to value, but unpredictable for buyers.
- **Tiered**: starter / pro / enterprise. Most common.
- **Freemium**: free tier; conversion engine.
- **Hybrid**: per-seat base + usage overage.
- **Open-source + commercial**: GitLab, Sentry, MongoDB pattern.

Patrick Campbell (ProfitWell / Paddle) is the most-cited modern voice. The repeated insight: **price changes have outsized impact on growth.** Most SaaS companies are under-priced.

### Pricing-change politics

Grandfathering (existing customers keep old price), communication strategy, churn impact, sales motion changes. The cost of a botched pricing change can take years to recover.

---

## Growth: PLG vs sales-led

### The PLG playbook (Bush, 2019)

Product-Led Growth: the product itself is the primary acquisition / activation / retention / monetization / expansion engine. Examples: Notion, Figma, Linear, Vercel, Slack, Calendly, Loom, Zoom.

Characteristics:
- Self-serve onboarding.
- Free / freemium tier.
- Value experienced before payment.
- In-product upgrade prompts.
- Network / virality / collaboration as growth driver.

PLG vs sales-led is not binary; most growth-stage SaaS does both ("PLG + Sales-Assist"). Pure PLG works for individual / SMB; enterprise still requires sales motion.

### The Cold Start Problem (Andrew Chen, 2021)

Network-effect products have a chicken-and-egg launch problem. Chen's framework:
- **The cold start**: the early period with too few users to be valuable.
- **Tipping point**: when network effects compound.
- **Escape velocity**: sustained network growth.
- **Hitting the ceiling**: market saturation.
- **The moat**: defensible advantage at scale.

Most network-effect products die in the cold start. The standard fix: pick a beachhead (geographic, vertical, use-case) and saturate it before expanding.

### Retention as the highest-leverage metric

The empirical observation: improving retention by 5% has more long-term impact than 50% growth in acquisition. Retention compounds; acquisition is a leaky bucket.

Smile curve (drops then rises as power users emerge) vs flat curve (sustainable rate) vs down curve (no retention). Sean Ellis's PMF test: 40%+ of users say "very disappointed" if the product disappeared.

---

## Roadmaps and stakeholder communication

### The roadmap-as-prediction-vs-commitment problem

Internal teams treat roadmap as discovery; stakeholders treat it as commitment. The mismatch produces the canonical PM headache.

The modern answer (Bruce McCarthy, *Product Roadmaps Relaunched* 2017): **themes, not features. Quarterly horizons, not annual.** "Q1 theme: improve onboarding conversion" rather than "Q1: ship features A, B, C."

### The "saying no" muscle (Cagan)

PMs spend most of their political capital saying no -- to feature requests from sales, to pet projects from executives, to scope creep from engineering. The Cagan chapter on this is foundational.

The discipline: have a public prioritization framework; refer requests to it; the framework says no, not you.

### The PM as writer

Amazon's six-pager. The "working backwards from a press release" exercise. Stripe's RFC culture. The pattern: if you can't write your product idea clearly in a 1-2 page memo, you don't understand it.

---

## The PM role: skills and the power question

### The PM as hub without authority

The PM coordinates execution but rarely has direct authority over engineers, designers, or anyone else. Influence is the medium. The PM's job is to align the team on what to do and why, without commanding.

This is why senior PMs are senior: years of practiced influence.

### Skills inventory

- **Discovery interviews** (Torres / Fitzpatrick).
- **Synthesis** (turning interview data into opportunities; affinity mapping).
- **Spec writing** (PRD, RFC, design doc, one-pager, brief).
- **Roadmap presentation** (themes vs features).
- **Launch planning** (alpha / beta / GA, launch checklist, rollback plan).
- **Stakeholder communication** (exec updates, sales enablement, support handoff).
- **Customer escalations** (last-line triage).
- **Pricing decisions** (Ramanujam / Campbell).
- **Competitive analysis** (Helmer's 7 Powers angle).
- **Product strategy writing** (Rumelt's kernel).
- **OKR setting** (Wodtke's discipline).
- **The "say no" muscle** (Cagan).
- **Premortems** (Klein -- imagining failure in advance).
- **Postmortems** for product launches that miss.

### The "good PM / bad PM" lineage

Ben Horowitz wrote the 1997 essay "Good Product Manager, Bad Product Manager." Foundational, still cited. The thesis: good PMs are accountable, decisive, written-comms strong, customer-grounded, technically credible. Bad PMs are vague, consensus-driven, schedule-driven, internally focused.

---

## Domain-specific lenses

- **B2B SaaS**: buyer/user split, sales cycles, expansion revenue (net revenue retention), logo retention, customer success.
- **B2C consumer**: virality, engagement, retention curves, app store dynamics.
- **Marketplace / two-sided**: supply / demand balance, cold start (Chen), liquidity metrics.
- **Developer-facing / API**: developer experience as marketing, documentation, SDK ergonomics, time-to-first-success.
- **AI / ML products**: eval design, the demo-vs-production gap, trust and explainability, model versioning as product surface.
- **Enterprise**: buying-committee complexity, procurement, security review, "enterprise-grade" features (SSO, audit logs, role-based access, compliance certifications).
- **PLG**: freemium, in-product upgrades, usage-based metrics, expansion as a primary growth lever.
- **Regulated**: compliance as a feature, audit trails, certification cycles, risk reviews.

---

## Modern shifts (last 5 years)

- **Empowered product teams** (Cagan) -- autonomy + outcome ownership.
- **Continuous discovery** as the norm (Torres).
- **AI as a product layer** -- every product surface is being AI-augmented.
- **PLG as a primary GTM** (Notion, Figma, Linear, Vercel).
- **Outcome over output** as a movement (Perri).
- **Reforge curriculum** as a training standard.
- **The PM as writer** (Amazon's six-pager pattern spreading).
- **Product ops** as a new function -- tooling, process, system of record.
- **The death of Scrum-master-PM** for non-software-shop teams; Shape Up and other alternatives.
- **AI-assisted PM tooling** (Linear AI, Notion AI, ChatGPT for spec generation).

---

## Schools of thought (preserve disagreement)

- **Cagan vs Perri vs Wodtke**: mostly agree but emphasize differently. Cagan: team model. Perri: strategy hierarchy. Wodtke: OKR practice.
- **Lean Startup (Ries) vs Cagan**: Cagan skeptical of Lean Startup as too consumer-startup; *Inspired* targets the empowered-team model in larger companies. Lean Startup targets pre-PMF.
- **JTBD vs personas**: JTBD (Christensen / Ulwick) considers personas noise; personas advocates (Cooper / Nielsen Norman) defend them. Pragmatic: use both.
- **Quantitative vs qualitative discovery**: Torres advocates both; experimentation school (Booking.com) leans quantitative; human-centered design (IDEO) leans qualitative.
- **B2B sales-led vs PLG**: real ongoing debate; the answer depends on segment (individual / SMB / enterprise).
- **OKR vs balanced scorecard**: depends on org size and stage.
- **Roadmap commitment vs roadmap discovery**: stakeholder management vs honest discovery.
- **Cagan vs Scrum Alliance**: Cagan thinks Scrum is harmful to product. Real, unresolved disagreement.

---

## Anti-patterns to flag

- **The Build Trap / Feature Factory**: outputs shipped, outcomes don't move.
- **Output-driven roadmaps** (Cagan / Perri critique).
- **HiPPO decisions** (highest-paid person's opinion).
- **Customer-asked-for-it trap** (Ford's "faster horse").
- **Solution-first thinking**: features before problem understanding.
- **MVP as "ship the smallest thing"**: the M is "viable for learning," not "shippable."
- **Roadmap as Gantt chart**.
- **Story points as productivity metric** (gameable).
- **Velocity as managerial KPI** (Goodhart's Law).
- **OKR theater** (Wodtke).
- **Endless backlog grooming**.
- **Feature requests as roadmap inputs** (sales-led trap).
- **Product strategy = features list** (Rumelt critique).
- **Continuous deployment without product gating**.
- **Vanity metrics**: MAU / downloads without engagement quality.
- **A/B tests as product strategy** (Balfour: you can't A/B test your way to the right strategy).
- **The PM as project manager** (role confusion).
- **No kill criteria**: refusal to specify what failure looks like.
- **"Everything is high-priority"** (Doshi).

---

## Process for the product-leadership agent

When invoked as an advisor, the agent should:

1. **Ground the question.** Is this strategy (Rumelt / Helmer / Moore), discovery (Cagan / Torres / JTBD), prioritization (RICE / WSJF), goal-setting (OKR / NSM), MVP / experiment design (Ries / Cagan), pricing (Ramanujam / Campbell), growth (Bush / Chen / Ellis), roadmap / launch, or pure PM craft?
2. **Diagnose before prescribing** (Rumelt). What's actually happening? What's the real obstacle? Most "what should we do" questions are missing the diagnosis.
3. **Name the school** when the advice depends on it. "Cagan's empowered-team view says X; Ries's pre-PMF view says Y; for your stage, X applies because Z."
4. **Frame outcomes, not outputs.** Push back on output-shaped framings ("we want to ship Y") and ask for the outcome.
5. **Apply the four risks** to product ideas. Value, viability, usability, feasibility. Where's the riskiest?
6. **Ask for kill criteria** for any proposed initiative.
7. **Test for vanity metrics and gameable goals.**
8. **Don't dispense generic platitudes.** "It depends" without specifying *what it depends on* is a non-answer.
9. **Cite the canon** when the user benefits from a reference. Don't bury them in citations.

The user's stance: "do not simply affirm." Even on solid product thinking, find at least one assumption to challenge.
