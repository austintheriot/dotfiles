---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-05
---

# Architectural tooling

A reference for advising on the software and digital deliverables of architectural practice: CAD and BIM authoring, openBIM and interoperability, model coordination, parametric and computational design, visualization and rendering, reality capture, drawing standards, asset libraries and their licensing, and AI image tools in concept work. Used by the `architectural-tooling` subagent and the `/expert-review` / `/expert-plan` / `/expert-consult` skills.

**Built environment, not software architecture.**

The unifying thesis: **a building information model is a database that produces drawings, and almost every tooling failure in practice comes from treating it as drawings that happen to be three-dimensional.** The consequences follow from that inversion: models are coordinated rather than drawn, data is exchanged rather than redrawn, level of information matters more than level of detail, and the deliverable's value lies in what downstream parties can extract rather than in what a sheet looks like.

The operational question: **"what does this model have to tell someone else, and will it survive the handoff?"**

Empirical priority, in rough order of how much cost it causes: **interoperability and handoff loss > model discipline and naming > coordination and clash workflow > licensing and cost structure > visualization quality.** Visualization consumes the most attention and causes the least downstream harm.

## Volatile surface

`last-verified: 2026-09-05`. **Software pricing, licensing models, and version-gated features are the fastest-rotting material here and the most consequential**, especially given the industry's ongoing subscription disputes.

| Claim class | Rots | Re-verify at |
|---|---|---|
| Software pricing and licence models | **Very fast** | Each vendor's pricing page |
| Version-gated features and file-format compatibility | Fast | Release notes |
| Standard versions (IFC, ISO 19650) | Medium | buildingSMART, ISO |
| Asset-library licence terms | Fast | The library's own terms |
| Renderer capability and integration status | Fast | The vendor |
| Modelling discipline and the interoperability argument | Slow | Durable |

**Two verified anchors worth carrying**: **ISO 19650 Parts 1 and 2 were published in 2018 and launched in the UK in January 2019** -- sources that appear to disagree are describing different events, so state both. And **SketchUp's @Last Software was founded in 1999 with the product shipping in 2000**.

**Vendor pricing frequently cannot be fetched automatically** (Autodesk's site blocks it), so treat any price in circulation, including any recalled here, as unverified until checked directly.

## BIM is a database

The single most consequential idea in this domain, and the one most often nodded at and then ignored.

**A BIM model's value is the information attached to its geometry**, and its purpose is to be queried by parties other than its author: the estimator, the contractor, the fabricator, the facilities manager, the energy analyst. A model that produces beautiful sheets and carries no reliable data is a drawing set with extra steps and extra cost.

**Level of Development and Level of Information Need are the vocabulary for how much can be relied upon.** The persistent failure is geometric detail outrunning information reliability -- a model that *looks* like construction documentation while its parameters are placeholder. Downstream users read the geometry as a promise. LOD exists precisely to make that promise explicit, and skipping it is how a contractor prices from a model that was never meant to be priced from.

**ISO 19650** provides the information-management framework -- the common data environment, the information requirements, the naming and status conventions. Its adoption is uneven and its value is entirely in whether the CDE discipline is actually followed rather than declared.

## openBIM and the interoperability question

**IFC is the vendor-neutral exchange format**, governed by buildingSMART, and it is the practical hedge against a model that only one company's software can read. Alongside it: **COBie** for handover data, and **BCF** for exchanging issues and viewpoints without exchanging models -- an underused format that solves a real coordination problem cheaply.

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

**Asset and material library licensing is routinely violated** because it is invisible in the deliverable. Terms vary sharply -- CC0 libraries impose nothing, others restrict redistribution, embedding, or commercial use, and some prohibit inclusion in a model handed to a client. **A model containing licensed third-party assets that is transferred to a client is a distribution event**, and the terms usually address that.

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

## Schools of thought

- **openBIM versus a closed ecosystem** -- vendor-neutral longevity against preserved parametric intelligence and round-trip friction.
- **Subscription versus perpetual licensing** -- continuous development and support against loss of ownership, the inability to open old projects without a live subscription, and unbounded price escalation on a tool the practice cannot leave. This one has real professional anger behind it and the anger is not irrational.
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

- **2026-09-05** -- Initial version. ISO 19650's 2018 publication versus January 2019 UK launch verified (the apparent contradiction in circulating sources describes two different events). SketchUp's @Last Software founding date corrected to 1999 with the product shipping in 2000. RealityCapture's acquisition by Epic verified as **March 2021**, with the frequently-cited 2024 date being the pricing change rather than the acquisition.

  **Known gaps requiring verification before use**: current Revit pricing (Autodesk's site blocks automated fetching), Fab and Megascans licensing terms, and Poliigon and Textures.com terms entirely. Software prices and licence models are the fastest-rotting material in this file and none should be quoted without checking. The openBIM round-trip loss characterization is a directional summary of practitioner experience rather than a measured study.
