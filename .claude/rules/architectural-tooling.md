---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-06
---

# Architectural tooling

A reference for advising on the software and digital deliverables of architectural practice: CAD and BIM authoring, openBIM and interoperability, model coordination, parametric and computational design, visualization and rendering, reality capture, drawing standards, asset libraries and their licensing, and AI image tools in concept work. Used by the `architectural-tooling` subagent and the `/expert-review` / `/expert-plan` / `/expert-consult` skills.

**Built environment, not software architecture.**

The unifying thesis: **a building information model is a database that produces drawings, and almost every tooling failure in practice comes from treating it as drawings that happen to be three-dimensional.** The consequences follow from that inversion: models are coordinated rather than drawn, data is exchanged rather than redrawn, level of information matters more than level of detail, and the deliverable's value lies in what downstream parties can extract rather than in what a sheet looks like.

The operational question: **"what does this model have to tell someone else, and will it survive the handoff?"**

Empirical priority, in rough order of how much cost it causes: **interoperability and handoff loss > model discipline and naming > coordination and clash workflow > licensing and cost structure > visualization quality.** Visualization consumes the most attention and causes the least downstream harm.

## Volatile surface

`last-verified` (see frontmatter). **Software pricing, licensing models, and version-gated features are the fastest-rotting material here and the most consequential**, especially given the industry's ongoing subscription disputes.

| Claim class | Rots | Re-verify at |
|---|---|---|
| Software pricing and licence models | **Very fast** | Each vendor's pricing page |
| Version-gated features and file-format compatibility | Fast | Release notes |
| Standard versions (IFC, ISO 19650) | Medium | buildingSMART, ISO |
| Asset-library licence terms | Fast | The library's own terms |
| Renderer capability and integration status | Fast | The vendor |
| Modelling discipline and the interoperability argument | Slow | Durable |

**Two verified anchors worth carrying**: **ISO 19650 Parts 1 and 2 were published in 2018 and launched in the UK in January 2019** -- sources that appear to disagree are describing different events, so state both. And **SketchUp's @Last Software was founded in 1999 with the product shipping in 2000**.

**Pricing, verified where possible and marked where not** (as of 2026-09-06, all needing re-verification before use):

- **BricsCAD BIM: EUR 1,060/year**, first-party, with "Lifetime Perpetual" still offered as a term option. Against Revit at a secondary-sourced figure around $2,910/year, that is a little over a third of the price with a perpetual route as well. **The cost is ecosystem, not capability**: consultant compatibility, library depth, and the plain fact that a client's BIM Execution Plan may simply name Revit.
- **Rhino: $995 perpetual**, first-party.
- **Archicad, Allplan, Chaos, Solibri**: first-party figures obtainable.
- **Autodesk: no first-party price could be obtained at all** -- the domain blocks automated access at the root. Five independent secondary sources cluster around $2,910-3,005/year, which is the best available and is **not** a verified figure. **Do not present it as one.**
- **Vectorworks: no price published.** The pricing page shows no figures and routes to a sales call. That is deliberate opacity rather than a fetch failure, and it is worth naming as such.

One pricing mechanism worth carrying because it makes "subscription flexibility" mostly notional: on at least one product the monthly rate annualises to a **71% premium** over the committed annual price.

## BIM is a database

The single most consequential idea in this domain, and the one most often nodded at and then ignored.

**A BIM model's value is the information attached to its geometry**, and its purpose is to be queried by parties other than its author: the estimator, the contractor, the fabricator, the facilities manager, the energy analyst. A model that produces beautiful sheets and carries no reliable data is a drawing set with extra steps and extra cost.

**Level of Development and Level of Information Need are the vocabulary for how much can be relied upon.** The persistent failure is geometric detail outrunning information reliability -- a model that *looks* like construction documentation while its parameters are placeholder. Downstream users read the geometry as a promise. LOD exists precisely to make that promise explicit, and skipping it is how a contractor prices from a model that was never meant to be priced from.

**ISO 19650** provides the information-management framework -- the common data environment, the information requirements, the naming and status conventions. Its adoption is uneven and its value is entirely in whether the CDE discipline is actually followed rather than declared.

## openBIM and the interoperability question

**IFC is the vendor-neutral exchange format**, governed by buildingSMART, and it is the practical hedge against a model that only one company's software can read. Alongside it: **COBie** for handover data, **BCF** (current version 3.0, released 2021) for exchanging issues and viewpoints without exchanging models -- an underused format that solves a real coordination problem cheaply -- and **IDS** (Information Delivery Specification), which is newer and increasingly the right way to state requirements machine-readably.

**Current state, verified**: the formal release is **IFC 4.3.2.0** (which *is* ADD2 under the Major.Minor.Addendum.Corrigendum notation), standardised as **ISO 16739-1:2024**. Two traps here. **Anyone citing ISO 16739-1:2023 has read the draft cover page** -- buildingSMART's own submission cover in the spec repository still says 2023, while the published standard and three national adoptions all say 2024. And buildingSMART's public IFC Release Notes page is **stale**, with its newest entry at IFC4.3 RC1, so it is the wrong source for a current-version question. IFC 4.4 exists but is not submitted to ISO; IFC5 is in alpha.

**The single most consequential openBIM finding for a practitioner: IFC 4.3 is the ISO-standardised schema, and no software is certified against it.** Certification lags the standard by two full schema generations. **Writing "certified IFC4.3 export" into a BIM Execution Plan specifies something that does not exist** -- and that phrasing appears in real documents. Note also that buildingSMART's self-reported implementations database uses view-level vocabulary ("IFC 4.3 reference view") but is explicitly **not** certification; the two are easy to confuse and only one is an assurance.

**The round-trip is where the theory meets reality.** IFC export and re-import is lossy in ways that vary by authoring tool and by IFC version; parametric relationships, native object behaviour, and non-standard parameters degrade. **Exporting to IFC is not the same as working in IFC**, and treating a one-way export as an interoperability strategy is the common overstatement.

**The genuine disagreement**: openBIM advocates argue that vendor-neutral exchange is the only defence against lock-in on a deliverable with a decades-long life, and that public clients are right to mandate it. The closed-ecosystem position is that a single-vendor stack preserves parametric intelligence that IFC discards, that round-trip loss makes openBIM workflows slower and more error-prone in practice, and that the mandate produces compliance exports nobody uses. **Both are describing real experience**, and the resolution usually depends on whether the model has a genuine downstream consumer or only a contractual one.

## Coordination

**Clash detection is a workflow, not a button.** Running clash detection produces thousands of results, most of which do not matter; the discipline is in rule setup, tolerance, grouping, assignment, and tracking to resolution. A clash report with no assignment and no closure loop is generated evidence of coordination rather than coordination.

**Hard clashes (geometry intersecting) are the easy half.** Clearance clashes -- maintenance access, pull space, insulation zones -- and workflow clashes -- construction sequence -- are where the expensive discoveries live, and they require rules somebody has to write.

**BCF is the right vehicle for issues** precisely because it moves the issue and the viewpoint without moving the model.

## Parametric and computational design

**Rhino and Grasshopper are the practical standard** for computational geometry in practice, with **Rhino.Inside.Revit** as the bridge into BIM authoring and **Dynamo** as the Revit-native alternative.

**The characteristic failure is definition rot.** A Grasshopper definition is a program written by someone who is not a programmer, usually without version control, naming discipline, or documentation, and it becomes unreadable to its own author within months and unmaintainable by anyone else immediately. The practices that help are the ones software engineering already knows -- name things, group and comment, keep it in version control, keep inputs explicit -- and they are rarely applied.

**The second failure is bakeless dependency**: geometry that exists only as live definition output, so the project depends on one person's machine and one file's health.

**Parametricism as a style is a separate argument from parametric tools as a method**, and conflating them muddies both. Schumacher's stylistic programme is contested on its own terms; the tooling is just tooling.

## Visualization

**The archviz stack splits between offline and real-time**, and the split is now genuinely contested rather than a quality hierarchy. Offline path tracing still wins on final-frame fidelity for hero images; real-time engines win decisively on iteration, on client interactivity, and on walkthrough and VR deliverables. **The real-time case has strengthened with WebGPU shipping across all major browsers**, since a browser-deliverable walkthrough no longer requires a download.

**The failure mode specific to this discipline is that the image outruns the design.** A photoreal render of an unresolved design communicates certainty the project has not earned, and clients approve images rather than drawings. This is an ethical and practical problem, not merely an aesthetic one.

**Asset and material library licensing is routinely violated** because it is invisible in the deliverable, and the answer to the practical question is now verified rather than assumed: **handing a client a model file containing licensed assets is normally a prohibited distribution.**

**The test the licences converge on is extractability, not visibility.** Poliigon's terms prohibit sharing assets "even if they've been modified" and say so explicitly for this case -- the prohibition "includes providing scene files to clients/others with Poliigon assets in a whole or easily extractable state." CGTrader permits redistribution only of an "Incorporated Product," defined as one that "cannot be extracted from an application or other product... and used as a stand-alone object without the use of reverse engineering tools."

| Deliverable | Typical status |
|---|---|
| Rendered still or animation | **Permitted** -- the asset is not extractable from pixels |
| `.blend`, `.max`, `.3dm`, `.skp`, Revit model, glTF with textures | **Prohibited** -- present and trivially extractable |
| Packaged real-time build | **Contested** -- cooked builds closer to permitted, editable projects not |

**Why practices get this wrong**: the render is obviously fine, so the mental model becomes "I bought it, I can use it." But handing over the model file is a different act, and it is exactly the one clients increasingly demand through BIM deliverables, coordination models, and working-files clauses in appointments. The mitigations are to strip or substitute proprietary textures before handover, to standardise on **CC0 sources** (ambientCG is a public-domain dedication with no distribution restriction, which makes it the structural fix for a practice that routinely delivers model files), or to buy a tier that permits transfer -- **checking that such a tier exists, because for many vendors it does not.**

**Epic's Fab Standard License** matters most because that library is the archviz default, and it contains a clause forbidding assets in tools "that allow works to be exported" -- which catches BIM and real-time deliverables directly -- plus a collaborator carve-out carrying a deletion obligation. **Textures.com terms remain unverified**; the site renders as an empty JavaScript shell at every path, and its historical numeric cap on textures per distributed product should not be quoted from memory.

## Reality capture

**Laser scanning and photogrammetry both produce point clouds; scan-to-BIM is the interpretive step**, and it is manual, expensive, and where the actual value is added. Automation exists and is partial.

The practical constraints: registration error accumulates across setups, occlusion means the scan records what was visible on the day, and **the model derived from a scan is only as current as the scan**. Existing-condition models are treated as ground truth long after the building has changed.

**Photogrammetry needs texture and light** and fails on dark, shiny, or transparent surfaces, which describes a great deal of a building interior. Gaussian splatting is the newer capture branch with a serious licensing caveat: **the original Inria reference implementation is a non-commercial research licence that propagates to derivative tools**, so commercial reality-capture pipelines need to check the lineage of what they build on.

## Drawing standards and documentation

Drawings remain the contract document in most jurisdictions, and **model-derived drawings fail differently from drawn ones**: they are consistent by construction and wrong in ways that propagate everywhere at once. A parameter error appears on every sheet simultaneously.

The durable disciplines are naming and classification (which is what makes a model queryable), sheet setup and annotation standards, and view templates -- all unglamorous, all determining whether a second person can work in the model.

## AI image tools in concept work

**The professional critique is specific and largely correct: image models produce plausible-looking buildings with no structural, constructional, or code logic**, because they optimize image plausibility. The output is a mood, not a proposal, and its danger is that it reads as a proposal to a client.

The defensible uses are early, disposable, and internal -- mood, atmosphere, massing variation to react against. The failure is presenting generated imagery as design intent, and the compounding failure is anchoring, where the team converges on the first striking image instead of exploring.

**There is also an ownership problem practices under-appreciate**: a practice's imagery is normally an owned business asset, and **AI-generated material may carry no copyright at all**. That is a business-model question, not only an ethics question.

## The 1:5:200 ratio is an urban myth, and the correction is usable

The claim that construction, maintenance, and staffing costs run 1:5:200 over a building's life is quoted constantly to justify spending on the building because staff cost dwarfs it. **It does not survive scrutiny, and the demolition is well-sourced enough to cite.**

Hughes, Ancell, Gruneberg and Hirst (ARCOM, 2004) traced it to a 1998 Royal Academy of Engineering paper in which "no data is given and no derivation or defence of the ratio appears." **They wrote to the original authors, who said they no longer had the data**, recalled it had come from a contractor using mainly US sources, and supplied a definition that is internally inconsistent with the paper itself.

The reductio arguments are what make the critique stick: 1:5:200 implies roughly **£91,000 to £120,000 of business cost per office worker per year**; it implies the UK spends five times more on maintenance than on new build, when national figures show it spends **less**; and it puts property at about **3%** of the cost of running a business when the real-estate literature says 10-30%.

**Their replacement, computed from published cost and wage data across three office buildings, is roughly 1:0.4:12**, with building cost at 11-12% of total business cost. That is still an argument for caring about whole-life cost and about the occupants -- just a defensible one. **Use 1:0.4:12 and cite Hughes et al.; do not repeat 1:5:200.**

## Schools of thought

- **openBIM versus a closed ecosystem** -- vendor-neutral longevity against preserved parametric intelligence and round-trip friction.
- **Subscription versus perpetual licensing** -- continuous development and support against loss of ownership and unbounded price escalation on a tool the practice cannot leave. The anger behind this is real and not irrational.

  **But the strongest form of the grievance needs a correction, and it is the useful part.** Two distinct fears get conflated: *"my licence stops working and I lose access to archived projects"* and *"my software stops being updated and eventually will not run on a current OS."* For Archicad the first does **not** materialise -- Graphisoft's own terms confirm perpetual holders may continue using the software indefinitely once maintenance lapses, losing only updates. What decays is operating-system and hardware compatibility, which is a slower and different failure mode. **Distinguish them; conflating them is the most common error in this argument**, and it weakens an otherwise sound case. Archicad's perpetual sunset is the best-documented instance: available to new customers through the end of 2024, to existing customers through the end of 2025, subscription-only from 2026.

  What remains genuinely **unverified** is the equivalent question for Autodesk -- what happens to a lapsed subscriber's access to archived models. That is the crux of the whole dispute and no first-party answer was obtainable.
- **Real-time versus offline rendering** -- iteration and interactivity against final-frame fidelity. Newly genuinely contested rather than settled.
- **AI in design** -- ideation aid against unbuildable plausibility, anchoring, labour displacement, and the copyright question.
- **BIM as deliverable versus BIM as process** -- whether the model is the product or the coordination is.

## Anti-pattern catalog

| Pattern | Trigger | Consequence | Fix |
|---|---|---|---|
| Modelling detail ahead of information reliability | Geometry is satisfying to make | Downstream parties price and build from placeholder data | State LOD/LOIN per element and hold to it |
| IFC export treated as interoperability | Contractual requirement met | Lossy one-way export nobody consumes | Test the round-trip with the actual receiving party |
| Clash detection without assignment or closure | Report generated | Evidence of coordination, not coordination | Rules, tolerance, grouping, owner, closure |
| Only hard clashes checked | Default rule sets | Clearance and sequence clashes found on site | Write clearance and access rules |
| Grasshopper definition with no naming or version control | Solo authorship, fast iteration | Unreadable within months; unmaintainable by anyone else | Name, group, comment, version-control |
| Geometry living only in a live definition | Never baked | Project depends on one machine and one file | Bake and archive milestone geometry |
| Photoreal render of an unresolved design | Wanting a good image early | Client approves an image the design has not earned | Match render fidelity to design certainty |
| Licensed assets shipped in a client model | Invisible in the deliverable | Transfer is a distribution event the licence may prohibit | Check terms; purge or license for handover |
| Scan treated as current | Existing-conditions model exists | Ground truth ages silently | Date the scan; re-verify before relying |
| Inria-lineage splat code in commercial capture | Reaching for the reference implementation | Non-commercial research licence violation | Check the lineage; use permissive alternatives |
| AI concept image presented as design intent | Impressive output | Unbuildable proposal anchors the project | Keep generated imagery internal and disposable |
| Vendor pricing quoted from memory | Reasonable recall | Wrong on the fastest-moving and most consequential axis | Check the vendor's page |

## Severity rubric for this lens

- **blocker** -- the deliverable fails its purpose or breaches a licence: a model that cannot be exchanged with a party contractually entitled to it, licensed assets distributed in breach, an existing-conditions model known to be stale and used as ground truth.
- **major** -- systematic downstream cost: LOD overstatement, clash workflow with no closure, definition rot on load-bearing geometry, a coordination process that produces reports rather than resolutions.
- **minor** -- friction and rework: naming inconsistency, view template drift, avoidable export loss.
- **nit** -- presentation and sheet convention.
- **insight** -- a reframe: this model has no downstream consumer and is being built as if it did; this parametric definition wants to be a simple family; this render is answering a question the design has not asked yet.

## Authorities

- **buildingSMART** -- IFC, BCF, COBie, and the openBIM position. The authority on the exchange formats.
- **ISO 19650** -- the information-management framework; read the parts rather than summaries.
- **The vendors' own current documentation and pricing** -- the only authority on capability and cost, both of which move.
- **The asset library's own licence terms** -- per-library and frequently misread.
- **National annexes and local drawing standards** -- documentation convention is jurisdictional.

## Changelog

- **2026-09-06** -- Gap-closing pass on tooling and pitfalls. **The asset-handoff question is now answered from licence text rather than inferred**: the test is extractability, not visibility, and handing a client a model file containing licensed assets is normally prohibited (Poliigon says so explicitly; CGTrader's 'Incorporated Product' definition turns on the same test). **IFC 4.3 is ISO 16739-1:2024 and no software is certified against it** -- certification lags the ISO schema by two generations, so 'certified IFC4.3 export' in a BIM Execution Plan specifies something that does not exist. Anyone citing ISO 16739-1:**2023** has read buildingSMART's draft cover page. Added the 1:5:200 demolition with its replacement ratio (roughly 1:0.4:12, Hughes et al. 2004), verified pricing where first-party figures exist, and a correction to the subscription grievance -- Archicad perpetual licences keep working indefinitely, so licence-death and OS-compatibility decay are different failure modes and conflating them weakens the case.

**Source research**: `~/.claude/local/research-notes/architecture-research.md` (the tooling material is in the summary layer rather than the written parts). Read it before a refresh -- it records what was verified against a primary source, what was not, and which sites blocked automated fetching, so a refresh pass need not re-derive any of that.

- **2026-09-05** -- Initial version. ISO 19650's 2018 publication versus January 2019 UK launch verified (the apparent contradiction in circulating sources describes two different events). SketchUp's @Last Software founding date corrected to 1999 with the product shipping in 2000. RealityCapture's acquisition by Epic verified as **March 2021**, with the frequently-cited 2024 date being the pricing change rather than the acquisition.

  **Known gaps requiring verification before use**: **Autodesk pricing remains unverified from any first-party source** -- the domain blocks automated access at the root, and the secondary-source cluster around $2,910-3,005/yr must not be presented as verified. **What happens to a lapsed Autodesk subscriber's access to archived models is likewise unresolved, and it is the crux of the whole subscription dispute.** Vectorworks publishes no figures at all. Textures.com terms could not be read (empty JavaScript shell at every path). No year-over-year price series was obtainable for any vendor, so **do not assert an escalation percentage**. Defect-claim frequency by category -- the central quantitative ask for the envelope section -- was not obtained from any insurer or warranty source. Why ASTM E2807 is marked Historical is unknown; resolve before citing E57 as current in a specification. Software prices and licence models are the fastest-rotting material in this file and none should be quoted without checking. The openBIM round-trip loss characterization is a directional summary of practitioner experience rather than a measured study.
