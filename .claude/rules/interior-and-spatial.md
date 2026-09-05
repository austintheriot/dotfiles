---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-05
---

# Interiors and inhabited space

A reference for advising on interior design and interior architecture: spatial planning at room scale, anthropometrics and ergonomics, lighting design, room acoustics, colour and materials, furniture and FF&E, biophilic design, and interior wayfinding. Used by the `interior-and-spatial` subagent and the `/expert-review` / `/expert-plan` / `/expert-consult` skills.

**Built environment, not software.**

The unifying thesis: **the interior is the only part of a building most people ever experience, and it is judged by the body rather than by the eye.** A room is assessed through comfort, acoustics, light quality, reach, clearance, and temperature -- none of which photograph, and all of which are set by decisions made early and cheaply. Interiors are also where a building's quality is most often destroyed, because interior scope is the first thing value-engineered and the last thing measured.

The operational question: **"what does this room ask of the body that uses it, for the hours it is used?"**

Empirical priority, in rough order of how often it makes a space unusable: **acoustics > lighting quality > clearance, reach, and circulation > thermal and air quality > material durability and maintenance > colour and finish selection.** Acoustics ranks first because it is the least reversible and the most complained about, and finish selection ranks last despite consuming most of the design conversation.

## Volatile surface

`last-verified: 2026-09-05`. Human factors are durable. Standards, certification schemes, and professional-credential structures are not.

| Claim class | Rots | Re-verify at |
|---|---|---|
| Accessibility dimensions and standard editions | Medium | The Access Board / relevant authority |
| Material-restriction lists and certification schemes | Medium | ILFI, GREENGUARD, the scheme itself |
| Professional credential structures | Medium | The credentialing body |
| Illuminance and acoustic target values by standard | Medium | The standard itself, by number |
| Anthropometric and ergonomic principles | Very slow | Durable |
| The evidence disputes (biophilia, open plan, circadian) | Slow | Durable, though the literature grows |

**Verified anchors as of 2026-09-05:** **NCIDQ no longer has a PRAC section** -- it was replaced by **IDIX (Implementation)**, with new blueprints effective 2026 organized so each exam covers two design phases. **The ILFI Red List is now 19 class-based categories** rather than a per-chemical list, with cadmium, chromium VI, lead, mercury, and arsenic consolidated into "Toxic Heavy Metals" and chloroprene and CPVC folded into "Chlorinated Polymers."

## The discipline question

**Interior decoration, interior design, and interior architecture are different scopes, and the distinction is legally live rather than a matter of pride.** Decoration is finishes and furnishing. Design covers spatial planning, code compliance, lighting, acoustics, and specification. Interior architecture extends into non-structural construction and building systems.

The title question has a sharp illustration: **the UK's Architects Act 1997 protects the title "architect" with a closed exception list -- naval, landscape, and golf-course.** Interior architects are not on it. **Golf-course architects received a carve-out and interior architects did not**, which tells you the boundary is historical accident rather than a considered scope judgment. Jurisdictions vary; check locally before advising on titles.

## Acoustics: first, because it is least reversible

Room acoustics is set by volume, geometry, and surface, all of which are decided early and are expensive to change afterward. Treatment applied late compensates; it does not fix.

**Absorption and isolation are different problems and are constantly confused.** Absorption (measured by NRC and related coefficients) controls reverberation *within* a room. Isolation (STC, and NIC as measured in place) controls transmission *between* rooms. **Adding absorptive panels to a room does almost nothing for sound coming through its wall**, and this is the single most common interior-acoustics mistake -- money spent on the wrong problem, with the complaint unchanged.

**Diffusion** is a third distinct tool: scattering rather than absorbing, which preserves liveliness while removing flutter echo. Flutter echo between parallel hard surfaces is a geometry problem with a geometry solution.

**Reverberation time targets vary by programme** and are set by standards rather than taste -- a classroom, an open office, a restaurant, and a recital room want very different values, and the specific figures should be read from the governing standard rather than recalled.

**Speech intelligibility** (STI and related measures) is the metric that matters in most working interiors, and it degrades from both excess reverberation and excess background noise. **HVAC noise is the usual hidden contributor**, and noise-criteria curves exist to specify it; a mechanical selection made without an acoustic target routinely lands a system that makes the room fail.

**Open-plan acoustics is where the evidence and the marketing diverge most sharply.** The claim is collaboration; **the measured finding (Bernstein and Turban) is that open-plan conversion reduced face-to-face interaction substantially while electronic messaging rose.** People respond to a loss of acoustic privacy by withdrawing. That result does not settle the question -- cost per head, flexibility, and sightline arguments remain -- but any open-plan proposal that asserts increased collaboration is asserting something the best-known study contradicts.

## Lighting

**Layered lighting** -- ambient, task, accent -- is the working structure, and the failure mode is a single uniform layer that serves no task well.

The measurable properties worth specifying: **illuminance** targets by task (read from the governing standard, not recalled), **correlated colour temperature**, and **colour rendering**, where **TM-30 is the more informative modern metric and CRI remains the one most specifications use**. Glare is specified through UGR in most interior contexts and is the property most often ignored until occupancy.

**Daylight is the highest-quality source and the hardest to control.** Daylight factor and the newer climate-based metrics quantify availability; the design problem is almost always glare and solar gain rather than quantity.

**Circadian lighting is genuinely contested.** The mechanism is real -- melanopic response is well established -- but the leap from mechanism to specified product benefit is where the evidence thins, and the metrics are still moving. State the mechanism confidently, the product claims sceptically.

**Lighting cannot be specified from a plan.** Reflectance, geometry, and surface finish determine the result, so a lighting scheme selected from a schedule without a model or a mock-up is a guess. This is a routine and expensive failure.

## The body: anthropometrics and ergonomics

**Design for the range, not the mean.** The average user does not exist, and a dimension set for the fiftieth percentile fails roughly half the population in one direction. The convention is to accommodate reach with a small percentile and clearance with a large one -- a doorway sized for a large person and a shelf placed for a small one.

The Panero and Zelnik *Human Dimension and Interior Space* tradition is the standard reference, alongside Neufert in the European context. **Accessibility dimensions are legal minimums, not comfort targets**, and designing to the minimum produces spaces that comply and remain awkward.

**Seated work, standing work, and circulation have different envelopes**, and clearance failures show up as spaces that are visibly fine and practically unusable -- a corridor that two people cannot pass in, a table whose chairs cannot be pushed back, a counter that cannot be reached across.

## Materials, colour, and air

**Durability and maintenance dominate.** An interior's realized quality over ten years is mostly a function of how its surfaces wear and how they are cleaned, which is decided at specification and never revisited. Cleaning protocol is a design input: a finish that requires a treatment the facilities team will not perform will fail.

**Off-gassing and material health** are specification concerns with real schemes behind them -- the ILFI Red List (now 19 class-based categories) and the various low-emission certifications. **The specification is where this is controlled**, because substitution during construction is where compliance is lost.

**Colour in space behaves differently from colour on a sample.** Area effect, adjacent-surface reflection, and the specified lighting all shift perceived colour, so sample-board selection under showroom light is systematically unreliable. The colour-psychology literature should be treated with more scepticism than it usually receives; the reliable claims are about contrast, legibility, and light reflectance value rather than about mood.

## Biophilia, honestly

**The pro case** is a real and reasonably well-replicated literature on restorative effects of natural views, daylight, and greenery, with plausible mechanisms.

**The sceptical case** is that effect sizes in the applied design literature are small, studies are often short-term with weak controls, publication bias is likely, and the commercial framework layer built on top of the research claims far more specificity than the underlying studies support.

**Both are recorded because the honest position uses the mechanism and distrusts the product.** Daylight and views are worth designing for on evidence; a certified biophilic scheme priced as a wellness intervention is claiming more than the literature carries.

## FF&E and the specification process

Furniture, fixtures, and equipment is where interior intent survives or dies. The pattern to watch: **specification, then value engineering, then substitution** -- each step defensible, the cumulative result a different building. The design decisions worth defending explicitly are the ones with performance consequences (acoustic absorption in furnishings, task-chair adjustability range, surface durability class), because those are indistinguishable from aesthetic preferences on a cut sheet and are cut as if they were.

**Furniture standards exist** for durability and safety testing, and specifying to a standard rather than a product is what survives substitution.

## Interior wayfinding

At room and floor scale the Lynch vocabulary still applies -- landmarks, nodes, districts -- but the dominant tools are sightline, threshold, and sequence rather than signage. **A signage system compensating for an illegible plan is a permanent tax**, and it is the usual outcome when wayfinding is engaged after the plan is fixed.

Signage carries accessibility requirements with specific dimensional rules: **tactile character stroke thickness is 10% to 30% of character height, character spacing is 10% of character height with a 1/16 inch minimum, line spacing is 135% to 170%, and there is a 70% contrast minimum in the standard** -- not merely advisory. Verify against the current edition.

## Schools of thought

- **Open plan** -- cost, flexibility, and sightlines against the measured reduction in face-to-face interaction. Both real; the evidence favours the sceptics on the collaboration claim specifically.
- **Biophilia's evidence base** -- mechanism against effect size and publication bias.
- **Circadian lighting** -- established photobiology against thin applied evidence and moving metrics.
- **Interior design's scope and title** -- a professional discipline with code and life-safety responsibility against a decoration tradition, argued through licensure. The golf-course carve-out is the detail that shows how arbitrary the settlement is.
- **Minimum versus comfort** -- accessibility compliance as a floor against universal design as a broader goal that the floor does not deliver.

## Anti-pattern catalog

| Pattern | Trigger | Consequence | Fix |
|---|---|---|---|
| Absorption specified for an isolation problem | Complaint about noise from next door | Panels installed, complaint unchanged, budget spent | Diagnose transmission versus reverberation first |
| Acoustics engaged after the plan is fixed | Late consultant appointment | Geometry cannot be changed; treatment compensates at best | Set volume and geometry with acoustic targets |
| Lighting specified from a schedule | No model, no mock-up | Glare, uneven distribution, wrong colour rendering discovered at occupancy | Model it, or mock it up |
| HVAC selected without an acoustic target | Mechanical and acoustic scopes separated | Background noise defeats speech intelligibility in a finished room | Specify a noise criterion up front |
| Designing to accessibility minimums | Compliance framing | Spaces that comply and remain awkward | Treat the minimum as a floor |
| Fiftieth-percentile dimensions | "Average user" reasoning | Fails roughly half the population | Reach small, clearance large |
| Colour chosen from a sample board | Showroom lighting | Area effect and reflection shift the result | Sample at size, under the specified light |
| Finish requiring unavailable maintenance | Specified for appearance | Degrades quickly; blamed on the material | Confirm the cleaning protocol is real |
| Performance FF&E cut as aesthetic | Value engineering by cut sheet | Acoustic and ergonomic performance quietly removed | Name the performance requirement in the spec |
| Signage compensating for an illegible plan | Wayfinding engaged late | A permanent tax on every visitor | Fix the plan; sign what remains |
| Open plan justified by collaboration | Received wisdom | The best-known study found the opposite | Argue cost and flexibility, which are defensible |

## Severity rubric for this lens

- **blocker** -- makes the space unusable or non-compliant: accessibility violation, egress obstruction, an acoustic condition that defeats the room's purpose, a specified material prohibited in the occupancy.
- **major** -- systematically degrades use: speech intelligibility failure, glare, clearance that blocks normal use, a finish that will not survive its maintenance regime.
- **minor** -- recoverable quality issues: colour under the wrong light, suboptimal fixture selection, inconsistent detailing.
- **nit** -- finish and styling preference.
- **insight** -- a reframe: this is an isolation problem being treated as absorption; this wayfinding problem is a plan problem; this wellness claim is doing no work.

## Authorities

- **Panero and Zelnik**, *Human Dimension and Interior Space* -- the anthropometric reference.
- **Neufert**, *Architects' Data* -- the European dimensional standard.
- **The Access Board and the governing accessibility standard** -- for any dimensional requirement; verify the edition.
- **The governing lighting and acoustic standards by number** -- illuminance, reverberation, and noise-criteria targets belong to standards, not to recollection.
- **Bernstein and Turban** on open-plan interaction -- the study any open-plan proposal has to answer.
- **ILFI's Red List** -- the material-restriction reference; now class-based.
- **The credentialing bodies** (NCIDQ and equivalents) for scope and title questions, which change.

## Changelog

- **2026-09-05** -- Initial version. NCIDQ's replacement of PRAC with IDIX and the Red List's move to 19 class-based categories verified. The UK Architects Act title exceptions, including the golf-course carve-out, verified. ADA signage dimensions (tactile stroke 10-30% of character height, character spacing 10% with a 1/16 in minimum, line spacing 135-170%, and the 70% contrast minimum being in the standard rather than advisory) verified and correct several commonly-misstated figures.

  **Known gaps requiring verification before use**: ISO 3382-1 acoustic parameter definitions, noise-criteria targets by room type, and specific illuminance targets by task were not verified in this pass and are deliberately stated as "read the standard" rather than given as numbers. Passivhaus comfort and overheating criteria are likewise unverified. The biophilia and circadian evidence characterizations are directional summaries of a contested literature rather than a systematic review.
