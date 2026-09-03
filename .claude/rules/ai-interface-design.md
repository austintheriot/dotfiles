---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# AI Interface Design

A reference for evaluating the human-facing surface of a system that includes a model. Used by the `ai-interface-design` subagent. The scope is *what the person sees, decides, and can undo*: how capability and fallibility are communicated, how a suggestion is presented, how uncertainty is shown, what evidence accompanies a claim, how a wrong answer gets corrected, what an agent is allowed to do before someone approves, and whether the person's reliance ends up matching the system's actual reliability.

Distinct from `llm-app.md`, which owns everything behind the surface: prompt construction, tool and schema design, retrieval-augmented generation architecture, evaluation harnesses, caching, cost, and prompt-injection defence. That agent asks whether the model call is well built. This one asks whether the person on the other side of it ends up better off. They meet at exactly two places, named in section 9.

Also distinct from `interaction-design.md` (general state completeness and feedback, which all applies here and is not repeated), `content-design.md` (wording, except where the wording *is* the uncertainty signal), and `app-privacy-compliance.md` (whether sending the data was lawful).

The core thesis: **the goal is calibrated trust, not maximum trust.** Reliance should match reliability, including the failure modes. An interface that makes a person trust a 70%-accurate system 95% of the time has failed, and so has one that makes them trust a 99%-accurate system 40% of the time. The measurable target in the literature is *appropriate reliance*: the person accepts correct output and catches incorrect output. Every design decision here either improves that ratio or degrades it.

The second thesis: **a suggestion is not a neutral offer.** Presenting an answer changes the answer the person would otherwise have given. That effect is measurable, it is not eliminable by telling people to be careful, and designs that ignore it will systematically over-report agreement between human and machine.

---

## 1. The Microsoft HAX guidelines, which are the closest thing to a settled checklist

Amershi et al., "Guidelines for Human-AI Interaction," CHI 2019. Synthesized from more than 150 design recommendations across two decades, then filtered through several validation rounds including a study where 49 design practitioners applied them against 20 popular AI-infused products. Grouped by when in the interaction they apply.

**Initially**

- **G1. Make clear what the system can do.** Help the person understand what the system is capable of.
- **G2. Make clear how well the system can do what it can do.** Help the person understand how often it may make mistakes.

**During interaction**

- **G3. Time services based on context.** Act when the person's task and environment make it useful.
- **G4. Show contextually relevant information.**
- **G5. Match relevant social norms.** Deliver the experience the way the person would expect given their social and cultural context.
- **G6. Mitigate social biases.** Do not reinforce undesirable and unfair stereotypes.

**When wrong**

- **G7. Support efficient invocation.** Make it easy to request the system's services when needed.
- **G8. Support efficient dismissal.** Make it easy to dismiss or ignore undesired services.
- **G9. Support efficient correction.** Make it easy to edit, refine, or recover when the system is wrong.
- **G10. Scope services when in doubt.** Disambiguate or degrade gracefully when uncertain about the person's goals.
- **G11. Make clear why the system did what it did.**

**Over time**

- **G12. Remember recent interactions.**
- **G13. Learn from user behavior.**
- **G14. Update and adapt cautiously.** Limit disruptive changes when updating behaviour.
- **G15. Encourage granular feedback.** Let the person express preferences during ordinary interaction.
- **G16. Convey the consequences of user actions.** Show how an action will affect future behaviour.
- **G17. Provide global controls.** Let the person customize what is monitored and how it behaves.
- **G18. Notify users about changes.**

**What the validation study found, which is the useful part:**

- **G11 ("make clear why the system did what it did") had one of the highest violation counts**, despite explainability being a heavily researched area. The gap between the research literature and shipped products is widest exactly here.
- **G3 and G4 were confused with each other**, as were **G15 and G17** (granular versus global feedback), and **G13** was frequently confused with others. The authors treated systematic confusion (four or more instances) as a signal the guideline needed clearer wording, and revised accordingly. For review purposes, this means: when applying G15 versus G17, be explicit about *local, instance-level* feedback versus *global, standing* controls, because the distinction is genuinely slippery.
- **G5 and G6 drew the most "does not apply" responses**, sometimes from participants who elsewhere reported violations of the same guidelines in the same product category. Treat "not applicable" claims about bias and social norms with suspicion.
- Some guidelines had no observed instances in some categories (G10 in social networks, G14 in activity trackers), which the authors read as those guidelines being hard to observe rather than universally satisfied.

Google's **People + AI Guidebook** (PAIR, 2019, revised) covers overlapping ground under six chapters, and its chapter names are a usable review outline in their own right: User Needs and Defining Success, Data Collection and Evaluation, **Mental Models**, **Explainability and Trust**, **Feedback and Control**, and **Errors and Graceful Failure**. Its central claim, which is worth holding onto: explainability and trust are the same problem, because a person can only calibrate trust once they have a working model of what the system can and cannot do.

---

## 2. Automation bias, complacency, and over-reliance

The failure this whole lens exists to prevent. The distinction matters because the remedies differ:

- **Automation complacency is passive.** The person stops checking. Monitoring degrades because the system is usually right, so vigilance has no recent reward.
- **Automation bias is active.** The person checks, notices a discrepancy, and defers to the machine anyway. Documented in clinical settings: clinicians who received model-generated diagnostic suggestions have followed them against their own assessment. The system's confidence overrode the person's judgment.

A third and subtler form: the person encounters output that conflicts with their intuition and **revises their own reasoning rather than overriding the system**. This is automation bias operating as self-doubt, and it produces agreement statistics that look excellent and mean nothing.

**Amplifiers:** time pressure, high workload, high volume, and long streaks of correct output. Every one of those describes a review queue. A design that pays a reviewer by throughput is a design that will manufacture agreement.

**What actually helps** (and what does not):

- Making the person commit before revealing the suggestion is the strongest intervention, and the most expensive: it doubles the interaction cost and slows the queue. It is worth naming as an option and worth being honest that it is often rejected on those grounds.
- Explanations help only when they support *verification*. An explanation that merely sounds plausible increases trust without increasing accuracy, which makes it worse than none.
- Showing *evidence* (the source passage, the crop, the underlying data) supports verification. Showing *reasoning* mostly supports persuasion.
- Surfacing disagreement between independent sources is a strong signal because it is not a self-report. Where several sources are available, "these three disagree" is more useful than any one confidence number.
- Deliberate friction on the highest-stakes cases, targeted by risk rather than applied uniformly, because uniform friction gets habituated exactly like uniform confirmation dialogs.

**Anchoring specifically.** Presenting a suggestion before the person forms their own judgment shifts that judgment toward it. This is well enough evidenced in human-in-the-loop annotation to design against. It does not mean suggestions must be hidden; hiding them costs speed and is often the wrong trade. It means a design that shows a suggestion first should not then claim its human-machine agreement rate measures independent verification, because it does not.

---

## 3. Communicating uncertainty

### What not to show

- **A raw model self-reported confidence number is the weakest available signal.** Language models are consistently overconfident in their self-assessments across multiple independent studies. A displayed "94% confident" gives a precision that the number does not have, and precision is itself persuasive.
- **A confidence number that is compared across different systems or different tasks is worse than none.** Two models' scores are not on the same scale, and a shared sortable column silently asks the person to compare them anyway. If several sources produce scores, attach each score to its own source's row and never present them as one ranking. (This is the same argument `confidence-and-calibration` reasoning makes in a project that has met this problem.)

### What to show instead, in rough order of strength

1. **Disagreement between independent sources.** Objective, uncomputed, and directly actionable.
2. **The evidence itself.** The retrieved passage, the source image, the matched record.
3. **Coverage and provenance facts.** "Based on 3 of 7 witnesses," "no source produced a candidate," "this claim has no citation."
4. **Coarse bands rather than numbers**, where a band is genuinely calibrated against measured outcomes.
5. **Hedging language**, which is weak but free.

### Framing

The wording of an uncertainty signal changes how it is received. "The AI is unsure" reads as a system failure and invites dismissal. "Limited data available for this recommendation" reads as useful context about the world and invites checking. Prefer statements about *the evidence* over statements about *the model's feelings*.

### Confidence-based escalation

The pattern that has converged across practice: **route by confidence rather than displaying it.** High-confidence output surfaces inline with a lightweight accept; output below the threshold presents as a verification card that requires an explicit decision and shows its evidence. This spends the person's attention where it changes outcomes.

Two caveats that must accompany it in any review: the threshold is a load-bearing number that needs justification and re-measurement, and the design must state what the low-confidence path *does* if nobody attends to it. A verification card that silently defaults to accepted after some period has reintroduced the automation it was meant to prevent.

---

## 4. Provenance, citation, and explanation

For any product where accuracy matters, citation is not decoration:

- **Attach citations to claims, not to whole responses.** A paragraph with three assertions and one source at the end tells the person nothing about which assertion the source supports.
- **Deep-link into the passage**, not to the document. A citation the person cannot check in one action will not be checked.
- **Preview on hover or expansion**, so verification does not require leaving the task.
- **Degrade visibly when sources are weak or absent.** An answer with no supporting source should look different from one with three. Silence about missing evidence is the failure mode that makes citations counterproductive, because their presence elsewhere trains the person to assume grounding.
- **Distinguish retrieved fact from model inference in the interface**, where the architecture makes the distinction available. If it does not, that is a finding for `llm-app` about the architecture.

On explanation (G11): a post-hoc rationale generated by the same model that produced the answer is not evidence about why the answer was produced. It may still be useful as a *hypothesis the person can check*, and it should be framed as one. Presenting a generated rationale as the system's actual reasoning is a truthfulness problem in the interface, independent of whether it is accurate.

---

## 5. Streaming, cancellation, and irreversibility

Streaming output creates interface problems that non-streaming output does not:

- **Once the response has started streaming, an error cannot arrive as a status code.** It has to be a stream event the client parses and renders differently. A design that assumes errors arrive before the first token will render an error as content.
- **The stop button's meaning must be defined.** Stop generating text is one thing. Stop an agent that has already dispatched side-effecting tool calls is another, and if the dispatcher fires calls eagerly as they are emitted, "stop" cannot undo what already ran. **If the system can take an irreversible action, it must not take it without a preview or an approval gate**, because cancellation cannot be relied on to arrive first.
- **Partial output is a state** (see `interaction-design.md` section 4). A stream that ends early must be visibly distinguishable from one that completed.
- **Live regions and accessibility.** Streaming text into the DOM without a considered `aria-live` strategy either says nothing to a screen reader or says everything, character by character. Route the conformance detail to `accessibility`; this lens owns the fact that the decision must be made.

**Approval gates.** For agent actions, an approval request carrying the accumulated context (what has been done, what this step does, what happens next) produces faster and more accurate decisions than a terse approve/deny. The corollary anti-pattern is the gate that has been shown so often it is approved reflexively, which is confirmation fatigue with higher stakes.

---

## 6. Correction, feedback, and control

- **Every output needs a visible way to edit, reject, or flag it** (G9, G15). This is not only a data-collection mechanism: people who know they can correct a system are measurably more willing to engage with it, so the affordance improves adoption even when nobody uses it.
- **Correction must be cheaper than redoing the work.** If fixing the machine's answer takes longer than writing your own, the machine is a tax and the person will start ignoring it, which is the worst equilibrium: the cost is paid and the benefit is not.
- **Distinguish local feedback from global control** (G15 versus G17, the pair the validation study found people confuse). "This one is wrong" and "stop doing this kind of thing" are different affordances and a system usually needs both.
- **Convey consequences** (G16). If rejecting a suggestion changes future suggestions, say so at the moment of rejection. If it does not, do not imply it does; a thumbs-down that goes nowhere is a dark pattern in a small way.
- **Record provenance in the data model, not only in the interface.** Whether a value came from a person, a model, or a bulk operation determines what any later measurement means. An interface that cannot show "who decided this" is usually sitting on a schema that did not record it, which is a finding worth raising even though the fix is not in this lens.

---

## 7. Failure modes catalog

- **Confident wrongness with no signal.** The interface renders a fabrication identically to a grounded fact.
- **Confidence theatre.** A precise-looking number with no calibration behind it.
- **Cross-source confidence comparison** presented as a ranking.
- **Anchored agreement.** Suggestion shown first, agreement rate then reported as verification.
- **Correction more expensive than creation.**
- **The dead thumbs-down.** Feedback collected, nothing changes, no acknowledgement.
- **Irreversible action inside a stream**, with a stop button that cannot stop it.
- **Approval fatigue.** A gate shown so often it is reflexive.
- **Citation as decoration.** Sources attached to responses rather than claims, or links that do not reach the passage.
- **Silent degradation.** The system answers with less evidence than usual and looks the same.
- **Capability opacity** (G1). The person cannot tell what the feature is for or what it will not do.
- **Fallibility opacity** (G2). Nothing anywhere indicates it can be wrong.
- **The unbounded agent.** An agent whose scope of action is not stated before it acts.
- **Sycophantic revision.** The system changes its answer when pushed back on, in a way that reads as responsiveness and is actually the removal of a signal.
- **Anthropomorphic hedging that hides mechanism.** "I think" and "I feel" where "no matching record was found" is the truth.

---

## 8. Schools of thought that genuinely disagree

### Legible AI versus ambient AI

- **Legible**: the person should always know a model is involved, what it did, and why. Favours explicit surfaces, visible citations, approval gates, disclosure.
- **Ambient**: the best AI is invisible, and constant disclosure is friction that degrades the experience; autocomplete does not announce itself.
- The distinction that resolves most cases is **reversibility and stakes**. Ambient is defensible where the action is cheap to reverse and the cost of a wrong one is low. Legibility is mandatory where an action is irreversible, where a record is created, or where the person will be held responsible for the outcome. A reviewer should locate the surface on that axis rather than applying one doctrine.

### Chat versus embedded

- **Chat** is general, discoverable, and requires the person to know what to ask. It externalizes the interface problem onto the user's ability to phrase things.
- **Embedded** (inline suggestion, in-context action) is specific, requires no phrasing, and is discoverable at the point of need, but only supports what was anticipated.
- The strong critique of chat-first design: a text box is the interface you ship when you have not decided what the feature is. The strong counter: embedded surfaces cannot cover the long tail, and pretending they can produces a feature that works for the demo path only.

### Human-in-the-loop, and when it is theatre

A review step that the person cannot realistically perform is not oversight. If a reviewer is given 900 items and a throughput expectation that allows eight seconds each, the human-in-the-loop claim is false regardless of the interface. Three tests worth applying: does the person have the information to judge, do they have the time to judge, and is disagreeing with the machine as cheap as agreeing with it? A design where accepting is one click and rejecting is five is not neutral.

### Anthropomorphism

One camp holds that conversational, first-person framing builds rapport and lowers the barrier to use. The other holds it systematically miscalibrates trust, because the cues that make a system feel like a colleague are the same cues people use to judge competence in colleagues, and those cues are uncorrelated with the system's accuracy. No settled answer. The reviewable middle: first-person framing is cheap and harmless in low-stakes surfaces, and it is a liability wherever the person must judge whether the output is right, because it substitutes social confidence for evidence.

---

## 9. Where this lens meets `llm-app`

Two places, and both are worth an explicit `See also`:

1. **The interface can only show what the architecture produced.** If the surface should distinguish retrieved fact from inference, or should cite at claim granularity, and the pipeline does not return that structure, the finding starts here and the fix is there.
2. **Prompt injection has an interface half.** `llm-app` owns the trust boundary and the privilege separation. This lens owns whether the person can *see* that content originating from an untrusted document is being treated as instructions, and whether a destructive action reaching an approval gate shows enough context for the approval to be meaningful.

Everything else divides cleanly. Model choice, token cost, eval design, retrieval quality, and caching are not this lens.

---

## 10. What to flag, and what not to

**Flag:**
- No indication anywhere that output is model-generated or can be wrong (G1, G2).
- A confidence number displayed without calibration, or compared across sources.
- A suggestion surface with no edit, reject, or flag affordance (G9).
- An irreversible action reachable without a preview or approval, especially inside a stream.
- Citations attached at the wrong granularity, or unreachable.
- A design that will silently manufacture agreement: anchored suggestion, cheap accept, expensive reject, throughput pressure.
- A feedback control whose effect is unstated or nonexistent (G15, G16).
- A stop or cancel affordance whose semantics do not match what the backend does.
- Missing distinction between local feedback and global control where both are needed.
- An approval gate whose payload is too thin to approve responsibly.

**Do not flag:**
- Prompt content, tool schemas, retrieval strategy, evaluation design, model choice, or cost. `llm-app`.
- General loading, empty, and error states. `interaction-design`, unless the state is specifically about uncertainty or partial generation.
- Wording quality on its own. `content-design`, unless the wording is the uncertainty signal.
- Whether sending the data to a vendor is lawful. `app-privacy-compliance`.
- The absence of AI features. Not a finding.
- Speculation about model behaviour not evidenced in the code or the interface.

Every finding names the decision the person is being asked to make and what they are missing to make it: not "add more transparency" but "the candidate panel shows the model's answer selected by default with no indication that a second source disagreed, so a reviewer clearing the queue at speed will record agreement they never checked."
