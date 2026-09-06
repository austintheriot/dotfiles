---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-06
---

# Architectural design (built environment)

A reference for advising on building architecture: design theory and proportion, historical movements and their arguments, urbanism and its critiques, practice structure, codes and accessibility, sustainability and carbon, art and acoustics in architecture, and environmental graphics. Used by the `architectural-design` subagent and the `/expert-review` / `/expert-plan` / `/expert-consult` skills.

**This is buildings, not software.** Nothing here is about software architecture.

The unifying thesis: **architecture is the discipline where an aesthetic argument has to survive contact with gravity, money, regulation, climate, and a hundred-year service life.** Most of the field's live disagreements are not about taste. They are about which of those constraints a design is entitled to subordinate, and for how long. A proposal that reads beautifully and fails on moisture, egress, or maintenance has not made a tradeoff; it has made an error.

The operational question: **"what is this design's actual argument, what does it cost to hold, and who pays that cost over the building's life?"**

Empirical priority, in rough order of how often it produces real harm: **envelope and moisture performance > life safety and egress > accessibility > durability and maintenance over the life cycle > energy and carbon performance in use > spatial and formal quality.** That ordering will annoy designers, and it is what the litigation and post-occupancy record shows.

## Volatile surface

`last-verified: 2026-09-05`. Design theory and history are durable. **Standards editions, regulatory dates, and rating-system versions are not, and they are exactly what people cite with false confidence.**

| Claim class | Rots | Re-verify at |
|---|---|---|
| Code editions and section numbers | Fast | The code body's own publication |
| Accessibility standard editions and dimensions | Medium | The Access Board / relevant authority |
| Rating-system versions and thresholds | Fast | USGBC, BRE, PHI |
| Carbon-limit values by jurisdiction | Fast | The national regulator |
| Effective dates for new requirements | Fast | The regulator |
| Design theory, history, the named debates | Very slow | Durable |

**Verified anchors as of 2026-09-05, each with a caution attached:**

- **ICC A117.1**: the sequence is 2009, then **2017 (current, with Supplement 1)**, then **2026 releasing in the autumn**, referenced by the 2027 I-Codes. **There is no 2023 edition** despite frequent citation of one.
- **IBC dead-end corridors are section 1020.5** in the 2021 IBC (Chapter 10 was renumbered; the 20 ft / 50 ft sprinklered values are unchanged).
- **LEED v5** launched in final form **April 2025**, reorganized around decarbonization (50%), quality of life (25%), and ecological conservation (25%). Certification thresholds are unchanged at 40 / 50 / 60 / 80.
- **BREEAM is on Version 7 (2025)**, with V7.1 circulating.
- **MUTCD is on the 11th edition (December 2023)**, and US federal law now requires an update every four years, so a 12th edition is due around 2027.
- **Denmark's carbon limit changed on 2025-07-01**: the old 12 kgCO2e/m2/yr expired, replaced by typology-differentiated limits (6.7 single-family and row houses, 7.5 apartments and offices, 8.0 institutions) plus a separate 1.5 cap on the construction process. Calculation is required above 50 m2; limits bind above 1,000 m2.

## Proportion and the golden-ratio question

This is the field's best-documented case of a claim that propagates by citation of citation, and it is worth knowing precisely because it teaches how to read architectural evidence.

**The sceptical case is strong and well-sourced.** Markowsky's "Misconceptions about the Golden Ratio" (1992) shows that published Parthenon dimensions vary by source because authors measure between different points, so "with so many numbers available a golden ratio enthusiast could choose whatever number gave the best result" -- and that in the standard golden-rectangle overlay, parts of the building including the stylobate edges fall **outside** the drawn rectangle, a detail routinely cropped from the illustration. Neveux (1995) went to the actual paintings with radiography rather than working from reproductions, and found that golden-section analyses rely on convenient endpoints and tolerances wide enough to admit phi, root-2, and 5:3 equally. She traces the modern myth to Zeising (1854), with the term *goldener Schnitt* itself dating only to Ohm (1835) -- **so there is no ancient or Renaissance name for it as an aesthetic principle at all.** Pacioli's *De divina proportione* (1509), always cited as the Renaissance authority, does not recommend phi for architectural proportion; it endorses the Vitruvian body-based system. Livio adds that the Egyptian seked construction method fully explains the Great Pyramid's slope, and that **phi appears in no surviving Egyptian mathematical text.**

**The sympathetic case, not strawmanned.** Le Corbusier, Zeising, Ghyka, and Hambidge's dynamic symmetry genuinely did use it, so the myth became true for the twentieth century because people believed it. The phi-derived rectangle also has real mathematical properties -- self-similarity under removal of a square -- that make it a defensible design tool regardless of whether the Greeks used it.

**The honest position, and the one to hold: phi is a real twentieth-century design method and a false historical claim about antiquity. Those are separable propositions, and sceptics sometimes conflate rejecting the second with rejecting the first.**

**Le Corbusier's Modulor** (1948, with Modulor 2 in 1955) is the same story in miniature. The red series runs 113, 70, 43, 27, 16.5, 10.2 cm from the navel height; the blue series runs 226, 140, 86, 53, 33, 20 cm from the raised hand. **The tell is the revision of the base figure from 1.75 m to 1.83 m -- six English feet -- reportedly because "in English detective novels, the good-looking men, such as policemen, are always six feet tall."** Anthropometry was adjusted to make the arithmetic work, so the system is not derived from the body; the body was fitted to the system. It is also over-determined: forcing the body, phi, and an additive Fibonacci rule to agree simultaneously guarantees that something was fudged. **The counter-argument worth keeping is that it worked as dimensional coordination** -- a small vocabulary of related sizes for a mass-production era, which is what a module is for, with a genuinely clever metric-imperial dual rounding.

**Palladio's seven room shapes** (I quattro libri, 1570, Book I ch. XXI) and the **ken/tatami module** are the better-documented proportional systems, because both were actually used as construction modules rather than reconstructed after the fact.

## What the record says about failure

The priority ordering above is not a preference. It reflects where buildings actually hurt people and cost money.

**Moisture and envelope failure is the dominant construction-litigation category.** Thermal bridging, interstitial condensation, and rain-screen detailing errors produce mold, structural decay, and claims. They are also invisible in the drawings that get published.

**Life safety is where regulation is least negotiable**, and egress, occupancy, and fire ratings shape plans before any aesthetic decision does. **Grenfell** is the reference case and the dates matter: the Phase 2 report was published **2024-09-04** and the inquiry closed 2025-02-10 (Phase 1 was 2019-10-30). Its regulatory consequence in the UK is the Building Safety Act 2022 and the "golden thread" information requirement. One detail that transfers directly to other disciplines: **aluminium composite panel with a polyethylene core is both the Grenfell cladding and the default signage substrate**, so any specification touching ACM must ask the core-type question.

**The performance gap** between predicted and measured energy use is systematic rather than incidental, which is why post-occupancy evaluation exists. The **PROBE** studies are the canonical evidence base -- five papers in *Building Research & Information* in 2001, covering 20-odd buildings; note the programme's own acronym changed mid-stream from "Engineering" to "Environment." **Building Use Studies** began in 1985 as the Office Environment Survey by **Sheena Wilson and Alan Hedge** (4,300 workers, 50 UK buildings); Adrian Leaman consolidated and productised it and is properly credited with the BUS *methodology* rather than its origin. **Soft Landings** is credited to Mark Way of Darwin Consultancy, with Bordass, Leaman, and Bunn.

**Value engineering as quality erosion** and **maintenance cost over the life cycle** are the two commercial forces that convert a good design into a poor building, and neither appears in the design phase's own documents.

## Movements as arguments

History is only useful here as a record of positions people still hold. The through-line worth carrying:

**Vitruvius** gives the triad -- firmitas, utilitas, venustas -- and was **rediscovered in 1416** by Poggio Bracciolini at St Gall. **Gothic structural rationalism** (Viollet-le-Duc) argues form follows structural logic; note that **Pol Abraham's 1934 attack on the rib-vault rationalist account** is a serious counter that English-language sources largely omit. **Modernism and CIAM** produced the Athens Charter, and a fact that reframes much of the postwar critique: **the Charter as published is Le Corbusier's 1943 rewrite of Sert's 1942 evidence-based document, with the plans deleted.** A great deal of what is criticized as "modernist planning" is one man's editorial decision.

**Postmodernism** (Venturi's *Complexity and Contradiction*, and *Learning from Las Vegas*) introduced the **duck versus decorated shed** distinction, which is literally an argument about signage and remains unanswered: Venturi's claim that structural expressionism is itself ornamental has never been well rebutted. **Critical Regionalism** (Frampton) and **Metabolism** are the two most-cited alternatives to universal modernism -- and Metabolism's central promise has a documented failure worth knowing: **Nakagin's replaceable capsules were never replaced once**, defeated by condominium law requiring 80% owner approval, per-capsule cost, and the absence of a supply chain. **Not by technique.** **Lacaton & Vassal's "never demolish"** is the current position with the strongest carbon argument behind it.

## Urbanism and its critiques

**Jane Jacobs** against **Robert Moses** is the founding conflict, and *The Death and Life of Great American Cities* remains the best statement of the case for fine-grained, mixed, incrementally-evolved urban fabric.

**Kevin Lynch's** *The Image of the City* gives the legibility vocabulary -- paths, edges, districts, nodes, landmarks -- which is the single most transferable idea in the field and is used almost unchanged in level design.

**Christopher Alexander versus Peter Eisenman** (the 1982 Harvard debate) is the field's genuinely unresolved split and should never be reconciled. Alexander holds that buildings should produce a felt quality of life and that pattern languages recover what vernacular building knew; Eisenman holds that architecture is a critical and intellectual discipline whose job includes disruption, and that Alexander's harmony is nostalgia. Both are describing something real about what architecture is for.

**Space Syntax** (Hillier) and **Defensible Space** (Newman, with its subsequent critiques) are the two attempts to make spatial-social claims measurable. **Gehl's** *Life Between Buildings* is the human-scale case. **Salingaros** and the traditionalist critique of modernism deserve to be stated at strength rather than dismissed, as does **Tom Wolfe's** *From Bauhaus to Our House* as polemic.

**The housing-supply debate is live and the evidence has moved.** Mast's migration-chain work (published **2023**, not 2021) finds **45 to 70 people -- not units -- per 100 new market-rate units** move out of below-median-income neighbourhoods over six rounds. Rosenthal (2014) is the filtering estimate. Been, Glaeser and Gedal covers historic districts. Damiano and Frenier's Minneapolis work is now peer-reviewed (2026), and Greenaway-McGrevy and So's Auckland rent study is in *Economic Inquiry* (2026). **Cite these precisely; the titles and dates are routinely swapped.**

## Sustainability, honestly

**LEED's value is genuinely contested and the strongest critique is empirical rather than rhetorical.** Scofield's reanalysis, done **floor-area-weighted**, finds the site-energy advantage falls to about 4% and loses statistical significance; his New York study found Gold buildings using roughly 20% less source energy while Silver and Certified used 11 to 15% **more**. The defence is that LEED covers more than energy and has moved the market. Both are true.

**The operational-to-embodied shift is the substantive change in the field.** As operational energy falls, embodied carbon becomes the majority of whole-life impact, which is the argument behind retrofit-first and behind mass timber.

**Two citation traps in the timber debate.** First, **Sterman et al. (2018) is about bioenergy for electricity, not construction timber** -- its headline mechanism is combustion efficiency, which does not transfer to timber that is never burned, so citing it against mass timber drops half the paper silently. Second, **the Netherlands' MPG change from 0.8 to 1.6 is a method change** (11 impact categories to 19), not a doubling of the permitted impact. And **RIBA and LETI targets are conflated constantly** -- "under 300 domestic" is LETI's *upfront A1-A5* figure, not a RIBA whole-life target.

**Mass timber's constraint is fire regulation**, and **ANSI/APA PRG 320 is on its 2025 edition**.

## Accessibility

**Accessibility law is a floor; universal design is a broader and separate idea**, and conflating them produces buildings that comply and exclude.

Verified dimensional facts that are commonly misstated, all from the ADA Standards: **latch-side pull clearance is 24 in parallel** (18 in is the front approach, pull side); **counters have three different heights** depending on the section -- 904.4.1 is 36 in max, 902.3 is a 28-34 in range, and 904.3.2 check-out counters are 38 in; **tactile character stroke thickness is 10% to 30% of character height** (703.2.6); **character spacing is 10% of character height with a 1/16 in minimum** (703.2.7), and line spacing 135-170%.

**The "70% contrast rule" is not in the ADA Standards, and citing it as a requirement is an error.** Verified by extracting the full text of the 2010 ADA Standards and searching it: the only numeric percentage in this area is the 135-170% line-spacing figure. **§703.5.1 requires only a non-glare finish and *directional* contrast** -- "either light characters on a dark background or dark characters on a light background" -- with no numeric value anywhere in the enforceable text. The accompanying advisory (explicitly non-enforceable) says only that signs "are more legible for persons with low vision when characters contrast as much as possible with their background." The 70% figure descends from 1991 ADAAG **appendix** material specifying a Light Reflectance Value differential of `(B1 - B2) / B1 x 100`. **It remains a defensible design target and an industry rule of thumb; it is not a requirement**, and putting it in a specification or compliance memo as one is a mistake.

Two further §703.5 details worth carrying because they are load-bearing and routinely missed: **visual character stroke thickness is 10% to 30% and spacing 10% to 35%** of character height (both differ from the tactile values), and **§703.5.5 defines viewing distance as the horizontal distance between the character and an obstruction preventing further approach** -- not the distance from which someone might happen to read it, which is how it is usually applied.

**The UK second-staircase requirement takes effect 2026-09-30**, verified from the Approved Document B 2026 amendment booklet itself (first published March 2024, updated September 2024, "Taking effect on 30 September 2026", England only). The provision: **a new recommendation for more than one common stair in blocks of flats with a storey 18 m or more in height**, with the corollary stated in the tables that "a single common stair arrangement is only suitable for use in buildings with a top storey less than 18 m in height." The same amendment adds design provisions supporting evacuation lifts in blocks of flats.

**The transitional arrangement is the part that decides live projects**, and it is generous. The previous edition continues to apply where a building notice, initial notice, or building control approval application with full plans was given before 30 September 2026 **and** the work either has started and is sufficiently progressed before that day, **or** starts and is sufficiently progressed within **18 months beginning on that day**. "Sufficiently progressed" is defined: for new construction, when pouring of concrete for the permanent placement of trench, pad, or raft foundations has started, or permanent placement of piling has started; for work to an existing building, when that work has started; for a material change of use, when work to effect the change has started. So a scheme with full plans in before the date and foundations poured within eighteen months after it stays on the old edition.

## Environmental graphics, art, and sound

These are three specialisms that sit inside architecture and are usually the first things cut.

**Environmental graphic design.** The canonical systems -- Vignelli's NYC Subway, Kinneir and Calvert's UK road signs, Wyman's Mexico 68, Aicher's Munich 72 -- are worth knowing as case studies with contested outcomes. **Two corrections: the Vignelli story is garbled nearly everywhere** (the 1970 manual specified Standard, the US trade name for Akzidenz-Grotesk; the MTA did not formally adopt Helvetica until **1989**; and the 1972 diagram returned as an official MTA variant in 2025). The **Kindersley story is usually overstated in the other direction.** The romantic version says his alphabet won the legibility test and was rejected on taste. The evidence is weaker: it tested legible at *marginally* greater distances, and the source itself doubts the difference was statistically significant. The Road Research Laboratory deemed it legible; the committees simply never seriously considered displacing the Kinneir and Calvert work already under way. **Marginally better within noise, and not seriously considered, is a different and duller story than demonstrably superior and rejected for aesthetics** -- and the duller one is what the record supports. The later vindication of mixed case was itself conditional on larger signs with adequate spacing. **Trajan's letters decrease downward** -- the top line is largest, designed to be read from below. **Catich developed but did not originate** the brush-then-chisel theory of serif origin, and his absolutist claim that serifs owed their form "wholly" to the flat brush is what the field disputes.

**Art in architecture.** The **GSA Art in Architecture** programme dates to **1963** (as "Fine Arts in New Federal Buildings", relaunched 1972) and allocates **0.5%** of construction cost -- not one percent, and not 1962, which conflates it with that year's *Guiding Principles*.

**Richard Serra's *Tilted Arc*** is the canonical site-specificity conflict: Serra's argument was that relocation *is* destruction, because location was constitutive of the work. **The usual "if only VARA had existed" framing is wrong**, and inverting it is the useful part. The Visual Artists Rights Act was enacted in **1990, after the removal**, so it did not apply -- but more importantly, a **2006 decision established that VARA does not protect location as a component of a site-specific work.** Relocation is permitted where the move is not itself destruction or mutilation. Serra would have lost the site-specificity argument under VARA too, on different reasoning. The law never adopted the premise his case rested on.

The artwashing critique -- art as developer amenity and as a displacement instrument -- deserves statement at strength.

**Architectural acoustics.** **Sabine** founded the field and the reverberation equation carries his name.

**The echo and fusion thresholds are signal-dependent, and three distinct claims get conflated.** Stated separately:

- **Fusion window**: roughly 1 to 5 ms for clicks, up to about 40 ms for speech and complex sounds. Below this a reflection is not heard as a separate event at all.
- **The Haas effect is a different claim** and is constantly merged with the above: a single reflection arriving 5 to 30 ms late can be **up to 10 dB louder** than the direct sound without detaching into a second auditory event. That is about *level tolerance*, not about the delay at which fusion breaks. Cite them separately.
- **Echo threshold**: approximately **50 ms for impulsive signals and speech**, up to about **100 ms for music**, and as long as **1 to 2 seconds** for sustained, nearly constant-amplitude signals. **20 ms is wrong** -- it sits inside the fusion window. **100 ms is the music end of the range, not the general figure**, and quoting it as the general value overstates it.

The 50 and 80 ms figures are not arbitrary: they are exactly where the standard **C50 (speech) and C80 (music) clarity measures split early from late energy**, which is the same perceptual boundary expressed as a metric.

A caution on the path-length arithmetic: published sources compute the equivalent reflecting distance from *different* delays and some use path length where others use round trip, so a stated metre figure needs its delay and its convention named before it means anything. The **shoebox versus vineyard** debate (Musikverein against Berlin's Philharmonie, 1963, the vineyard prototype) is a real unresolved split. Musikverein's **19 m width** is the substance of the shoebox argument -- narrow halls deliver strong lateral reflections, which is what produces envelopment -- and the Concertgebouw's **2.2 s occupied against 2.8 s unoccupied** shows how large the occupancy delta is.

**The most useful thing to understand about this argument is that both camps accept the same measurements.** Shoebox halls are louder, more enveloping, and warmer; vineyard halls are clearer and less reverberant. Nobody disputes that. **They disagree about which property is the defect.** The shoebox camp says the vineyard trades away what makes orchestral music sound like orchestral music in order to buy seats and sightlines. The vineyard camp says the shoebox buys its sound by excluding the audience the hall needs to survive, and that clarity and proximity are real goods rather than consolation prizes. The Elbphilharmonie's split reception is that same argument as lived experience -- the praise for its clarity and precision and the complaint that it "does not help" are describing one acoustic property from opposite sides. Do not reconcile it.

**Beranek's bibliography, corrected**: the sequence is *Music, Acoustics, and Architecture* (1962, 55 halls), ***Concert and Opera Halls: How They Sound* (1996)** -- which does exist, contrary to an earlier failure to find it -- and *Concert Halls and Opera Houses* (2004, 100 halls). The strongest critique of his hall rankings is his own admission that no person he interviewed was well acquainted with more than about a third of the halls being ranked.

**The Royal Festival Hall's assisted resonance system** resolves an apparent date conflict: experimental work began in **1962**, the full 172-channel system was installed in **1964**, and it ran until it was switched off in **December 1998**. Both commonly-cited start dates are right about different things. **Schafer's** soundscape work and **Blesser and Salter's** *Spaces Speak, Are You Listening?* are the two texts that treat sound as a spatial medium rather than a nuisance to control.

## Schools of thought

Recorded unreconciled, because each side diagnoses something the other misses.

- **Alexander versus Eisenman** -- felt quality of life against architecture as critical discipline. The field's deepest split.
- **Structural honesty versus dressing** -- whether expressed structure is truth or is itself ornament. Venturi's duck-and-decorated-shed reframing has never been well answered.
- **Weathering as patina versus weathering as defect** -- "the building finishing itself" against "uncontrolled differential soiling the client will call a defect." Both parties are describing the same streak.
- **LEED's value** -- market transformation against Scofield's floor-area-weighted null result.
- **Modernism versus traditionalism** -- Salingaros and the traditionalists against the modernist mainstream, on whether the twentieth century's formal break was a discovery or a mistake.
- **Parametricism** (Schumacher) and its critics -- whether computational form-finding is a style, a method, or a mannerism.
- **Housing supply** -- YIMBY supply economics against preservation and displacement concerns. Note this one has actual empirical movement, cited above.
- **Biophilia's evidence base** -- real effect sizes against publication bias and weak instruments.
- **Open plan** -- collaboration claims against the measured finding that open plans **reduce** face-to-face interaction.

## Anti-pattern catalog

| Pattern | Trigger | Consequence | Fix |
|---|---|---|---|
| Citing a standard edition that does not exist | Recalling a version number | Advice keyed to a nonexistent document (A117.1 "2023") | Verify the edition before citing |
| Envelope treated as a finish decision | Focus on appearance | Moisture failure -- the dominant litigation category | Detail the assembly, including thermal bridges |
| Accessibility retrofitted | Compliance handled late | Awkward, stigmatizing, expensive; often still non-compliant | Design in from the plan stage |
| Predicted energy presented as expected performance | Model output taken as measurement | The performance gap, discovered by the occupant | Commit to post-occupancy evaluation |
| Life-cycle maintenance unpriced | Capital-cost framing | Owner inherits a building they cannot afford to keep | Price maintenance and replacement over the life |
| Acoustics as an afterthought | No acoustician until late | Unfixable geometry; treatment applied to a room that needed a shape | Involve acoustics at plan stage |
| Signage substrate unquestioned | Spec by appearance | ACM with a PE core is a fire pathway | Ask the core-type question every time |
| Golden-ratio justification | Post-hoc rationalization | Historical claim that does not survive scrutiny | Use it as a design tool if you like; do not claim antiquity |
| Sterman cited against mass timber | Reasonable-looking citation | The paper is about bioenergy combustion, not construction | Read what the paper measured |
| RIBA and LETI targets conflated | Both are carbon targets | Wrong scope compared against wrong benchmark | Name which target and which stages |
| Rating certification treated as performance | Plaque on the wall | Certification and measured performance are weakly correlated | Measure |

## Severity rubric for this lens

- **blocker** -- life safety, code non-compliance, or a defect that will cause building failure: egress that does not work, a cladding or envelope assembly that will fail, an accessibility violation.
- **major** -- a performance or durability problem that will surface in use: thermal bridging, unmanaged moisture risk, unpriced maintenance burden, acoustics that cannot be fixed after construction.
- **minor** -- quality and cost issues that are recoverable: detail resolution, specification inconsistency, coordination gaps.
- **nit** -- presentation and drawing convention.
- **insight** -- a reframe: this programme wants a different parti; this is a retrofit question presented as a new-build question; this proportional justification is doing no work.

## Authorities

- **Vitruvius**, *Ten Books on Architecture* -- the origin of the triad, and worth reading for how much is about practice.
- **Jane Jacobs**, *The Death and Life of Great American Cities* -- the founding critique of top-down planning.
- **Kevin Lynch**, *The Image of the City* -- legibility; the most transferable idea in the field.
- **Christopher Alexander**, *A Pattern Language* / *The Timeless Way of Building* / *The Nature of Order* -- and the Eisenman debate as its necessary counterweight.
- **Robert Venturi**, *Complexity and Contradiction* and *Learning from Las Vegas* -- the duck and the decorated shed.
- **Kenneth Frampton**, *Studies in Tectonic Culture* and the Critical Regionalism essays.
- **Mostafavi and Leatherbarrow**, *On Weathering* -- time as a material.
- **Markowsky, Neveux, Livio** -- the golden-ratio debunking literature, and a model for reading evidence in this field.
- **Beranek**, *Music, Acoustics, and Architecture* (1962) and *Concert Halls and Opera Houses* (2004) -- note these are the verified titles.
- **Blesser and Salter**, *Spaces Speak, Are You Listening?* -- aural architecture.
- **The PROBE studies** (*Building Research & Information*, 2001) -- the post-occupancy evidence base.
- **The code and standard bodies themselves** -- ICC, the Access Board, RIBA, USGBC, BRE, PHI. Nothing secondary is authoritative on a requirement.

## Changelog

- **2026-09-06** -- Gap-closing pass on environmental graphics, art, and acoustics. **Four claims published on 2026-09-05 were wrong and are corrected in the body.** (1) **The ADA 70% contrast rule does not exist.** Verified by extracting the full 2010 ADA Standards text and searching it: SS703.5.1 requires a non-glare finish and directional contrast only, with no numeric value in the enforceable text. The 70% figure is non-enforceable 1991 ADAAG appendix material. The previous version asserted the opposite, explicitly. (2) **The echo threshold is ~50 ms for speech and impulsive signals, not ~100 ms** -- 100 ms is the music end of a signal-dependent range, and the Haas effect is a separate claim about level tolerance that the previous version merged with fusion. (3) **The Kindersley legibility story was overstated**: marginally better within noise and never seriously considered, not demonstrably superior and rejected on taste. (4) **The 'if only VARA had existed' framing of Tilted Arc is wrong** -- a 2006 decision established that VARA does not protect location as a component of site-specific work, so Serra would have lost that argument under it too. Also added: GSA Art in Architecture is 1963 and 0.5%; Beranek's 1996 title does exist; the RFH assisted-resonance dates are 1962 experimental / 1964 full system / 1998 switch-off, which resolves the apparent conflict.

**Source research**: `~/.claude/local/research-notes/architecture-research.md` (Parts 0-4 written in full (corrections, theory, movements, critiques, practice); Parts 5-9 exist only as the summary this file was written from). Read it before a refresh -- it records what was verified against a primary source, what was not, and which sites blocked automated fetching, so a refresh pass need not re-derive any of that.

- **2026-09-05** -- Initial version. Built from research that audited and corrected roughly fifty premises, of which the load-bearing ones are recorded in the body: A117.1 has no 2023 edition; IBC dead-end corridors moved to 1020.5; the ADA latch-side, counter-height, tactile-stroke and character-spacing figures were all misstated in the brief and are corrected here; Haas fusion is signal-dependent; Trajan's letters decrease downward; the Grenfell Phase 2 report is 2024-09-04; Denmark's carbon limits changed 2025-07-01; LEED v5 is final; Vitruvius was rediscovered in 1416; Mast's migration-chain finding is people not units and published 2023; and BUS originates with Wilson and Hedge rather than Leaman.

  **Known gaps requiring verification before use**: BREEAM's percentage thresholds, most Passivhaus PER tier values and all EnerPHit figures, ISO 3382-1 acoustic parameters, NC curve targets by room type, SEGD's founding and rename dates, Frampton's six points enumerated, and the Preservation Green Lab retrofit payback range -- which is the headline claim of the retrofit case and is currently unsourced. **Resolved 2026-09-06**: the UK second-staircase requirement was verified from the Approved Document B 2026 amendment booklet and is now recorded in the accessibility section with its transitional arrangement.
