---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-09
---

# Scholarly publishing (process, venue, and the machine-readable record)

A reference for advising on and reviewing how a document becomes a published record of account, and the obligations that attach. Covers journals, conferences, monographs, preprints, technical reports, and **industry white papers**, which borrow the apparatus without the peer review. Used by the `scholarly-publishing` subagent and the `/expert-review` / `/expert-plan` / `/expert-consult` / `/consult` skills.

**Source research**: `~/.claude/local/research-notes/scholarly-publishing-research.md`

The unifying thesis: **publishing is a set of promises about a document -- that it was scrutinized, that it will persist, that it can be found, that its authorship is accountable, and that its record can be corrected.** Each promise is carried by a specific mechanism, and each mechanism fails in a specific way. The failures are almost never loud: a paper drops out of an index, a link rots, a retraction goes undetected, an artifact promise is unfulfillable.

The agent's operational question: **"which promise is this document making, what mechanism carries it, and how will anyone know when the mechanism fails?"**

**The single organizing frame is Goodhart's Law.** Nearly every integrity failure in this domain -- paper mills, salami slicing, citation cartels, gift authorship, impact-factor gaming, "available on request" data statements -- is a rational response to an assessment metric. Diagnose the metric before diagnosing the behavior.

The empirical priority, in rough order of how often it bites:

1. **Silent index and discovery failure** -- a malformed metadata tag costs citations with no error, no warning, and no dashboard.
2. **Stale policy** -- the funder, publisher, and retraction-infrastructure surface moved substantially in 2025-2026, and confident recollection here is usually wrong.
3. **Retraction detection** -- the mechanism broke in May 2026 in a way that returns HTTP 200 and stale data.
4. **Unfulfillable promises** -- data "available on request", artifact links that rot, versionless documents that cannot be cited.
5. **Deanonymization** -- in double-anonymous review, mostly through channels nobody checks.
6. **Venue and route mismatch** -- an author caught between an OA mandate and a prohibition on paying for it.

---

## Volatile surface

`last-verified` is in the frontmatter -- do not restate it in prose. These rot; the rest of this file is comparatively durable.

| Claim class | Rots | Re-verify at |
|---|---|---|
| **US federal OA policy (Nelson memo repeal)** | **Fastest here; actively in flux** | `aip.org/fyi`, OSTP, agency public-access plans |
| NIH APC caps and public-access policy | **Fast** | `grants.nih.gov/grants/guide/`, `osp.od.nih.gov` |
| Plan S / cOAlition S direction | **Fast** (restructured Nov 2025, new Director May 2026) | `coalition-s.org` |
| Publisher AI policies | **Fast** | Each publisher's current author-policy page |
| Crossref retraction endpoints | **Broke May 2026** | `community.crossref.org`, Crossref docs |
| APC price levels | Medium | Each publisher |
| DOAJ / delisting events, paper-mill screening tools | Medium | DOAJ, STM Integrity Hub |
| Venue preprint and LLM-review policies | **Annually, per venue** | The venue's current CFP |
| Service naming (SHERPA/RoMEO → Jisc open policy finder) | Medium, and high-churn here | The service itself |
| ACM badge definitions | Changed once (terms swapped) | `acm.org` artifact-review policy |
| Google Scholar `citation_*` requirements | **No** -- stable | `scholar.google.com/intl/en/scholar/inclusion.html` |
| JATS, DOI, ORCID, CITATION.cff contracts | **No** -- the stable layer | NISO, Crossref, ORCID |

---

## 1. Venue types and what each obligates

| Venue | Review | "Published" means | Notes |
|---|---|---|---|
| Journal article | Peer review, months | Version of record with a DOI | The default assumption in most fields |
| Conference paper | Peer review, fixed deadline cycle | **In computer science, a terminal venue** -- unlike most fields, where a conference paper is a precursor to a journal article | Getting this wrong misreads a CS CV entirely |
| Monograph | Proposal + manuscript review | A book with an ISBN | Route production to `print-production` |
| Preprint | **None** (moderation only) | Publicly posted, citable, versioned | Not a publication for assessment purposes in most fields |
| Technical report | Institutional, variable | A numbered series entry | The series number is the durable handle |
| Industry white paper | **None** | Whatever the publisher says | Must supply its own credibility apparatus -- §7 |
| Thesis | Examination | Deposited, usually OA | Prior publication rules vary by institution |

**Prior-posting eligibility is the question that catches people.** Most venues now accept preprints, but the set that does not is nonempty, and ML/CV conference policies on *actively promoting* a preprint during review change annually. Check the current call, not last year's.

---

## 2. Peer review

### What actually deanonymizes a submission

A 2026 study analyzed **all 2.7 million arXiv submissions with available source files** and found that **nearly every submission contains some form of hidden information** -- including, in real cases, links to internal coordination documents, **API keys and private keys**, and complete Git histories. Three leakage categories: unnecessary files swept into the submission, metadata embedded in files, and irrelevant content such as source comments. The authors also showed that **existing cleaning tools fail to reliably do what they claim**, so `arxiv-latex-cleaner` having run is not evidence of anything.

**This is a security finding as much as an anonymity one.** Trigger: a `.git` directory or editor backup swept in by a glob. Symptom: full authorship, institutional emails, internal review comments, and sometimes live credentials are publicly downloadable from the source archive **even though the compiled PDF is clean.**

The channels, concretely:

1. **PDF metadata.** `hyperref` writes `\author{}` and `\title{}` into the PDF `Author` and `Title` fields via `pdfauthor`/`pdftitle` **even when the title block is visually suppressed**. Many templates set these from a macro nobody comments out. **Test with `pdfinfo paper.pdf` and `exiftool paper.pdf` before submitting.**
2. **Filenames** -- `smith_neurips2026_final.pdf`, or a supplementary zip whose internal paths contain `/Users/jsmith/`.
3. **Self-citation phrasing.** "In our previous work [17]" leaks; the *phrasing* is the leak, not the citation. The correct form is third-person ("Smith et al. [17] showed") with the reference intact. **Removing self-citations entirely is worse** -- it reads as missing related work and reviewers penalize it.
4. **An arXiv preprint** with the same title and abstract. The largest channel, and the one venue policies actually disagree about.
5. **Repository and project-page links**, including anonymized-looking links whose repo name matches a public one, and Docker Hub or HuggingFace org names.
6. **Acknowledgements** -- **funder grant numbers are effectively author identifiers**, searchable in NIH RePORTER, CORDIS, and the Crossref funder registry.
7. **Small-field topicality** -- a specialized instrument, dataset, or cohort with one owning group in the world.
8. LaTeX source comments and `\todo{}` notes, where source reaches reviewers.

**On the arXiv-and-acceptance correlation**: an analysis of ICLR 2019-2020 submissions (n=5050) found a statistically significant positive correlation between acceptance and arXiv-released papers from high-reputation authors. **State this carefully -- it is correlational, and the confound is real** (better labs write better papers *and* preprint more). It is evidence of co-occurrence, not proof that deanonymization caused it.

### Review models and their tradeoffs

Single-anonymous, double-anonymous, open, post-publication, **registered reports** (review the protocol before results exist, which structurally eliminates publication bias and p-hacking), and **portable/transferable review** (Review Commons, PCI).

**Reviews can have DOIs.** Crossref's `<peer_review>` schema supports `<anonymous/>`, `revision-round`, and `recommendation` values -- so "our review is anonymous" is **not** a reason it cannot be deposited and cited.

**The "reviewer used an LLM" question is live and publisher policies differ.** Check the specific venue; do not generalize.

### Violations, named precisely

**Salami slicing** (splitting one study into minimum publishable units), **duplicate submission** (concurrent submission to two venues), **redundant publication** (republishing substantially the same work), **citation cartels**, and **gift/ghost authorship**. Each is a Goodhart response to a counting metric.

---

## 3. Preprints and versioning

- **arXiv**: moderation and endorsement can decline a submission **with no reasons given**, which matters for industry work that reads as marketing. Category choice and cross-listing affect discovery. **Licence choice is irrevocable per version**, and **the arXiv perpetual licence is not an open licence** -- so a paper can be arXiv-licensed in its metadata and CC BY on its first page, which is exactly the case licence-metadata pipelines misjudge.
- **bioRxiv explicitly refuses white papers, guidelines, and theses.** For grey literature the answer is **Zenodo or OSF Preprints**, not a biology preprint server.
- **medRxiv/bioRxiv added prominent "not peer reviewed" banners** in response to the COVID-era critique -- itself evidence the operators found it partly valid.
- **Version identity matters**: submitted manuscript, **author accepted manuscript (AAM)**, and **version of record (VoR)** are three different documents with different rights and different citation consequences. Green OA delivers the AAM, not the VoR.
- Preprint-to-published linking runs through Crossref relationship metadata (`is-preprint-of`).

---

## 4. Open access, precisely

| Route | Who pays | What you get |
|---|---|---|
| **Gold** | Author or funder (APC) | VoR, immediately open, licensed |
| **Green** | Nobody | AAM in a repository, possibly embargoed |
| **Diamond/platinum** | Institutions, consortia, societies | VoR open, no author fee |
| **Hybrid** | APC inside a subscription journal | VoR open; the double-dipping critique |
| **Bronze** | Publisher discretion | Free to read, **no licence** -- so not reusable and revocable |

### The current policy surface -- all VOLATILE (2026-09-09), and this is where recollection fails

**The Nelson memo is being repealed.** OSTP is reported to be in the process of repealing the 2022 zero-embargo memo, and the House FY2027 CJS report asked NSF to pause implementation of new public-access policies pending it. **Critically, the consequence is undefined** -- restoring 12-month embargoes, dropping only the requirement, or replacing it entirely are all live.

**And there is a collision worth naming**: a government-wide prohibition now bars federal funds for "prohibitively high publishing costs" without prior agency approval. **So an author can be simultaneously required to make work immediately open and forbidden from spending grant money on the APC that would do it.** **Green deposit of the AAM is the only advice robust to every branch of this.**

**Plan S changed shape.** cOAlition S published a 2026-2030 strategy (November 2025) that **names APCs as a problem** ("increasing costs", "issues of inequity"), concedes "no single model can meet all needs", and pivots toward **diamond OA** (the Bengaluru Roadmap, May 2026). New Director and host Secretariat as of May 2026. **The Rights Retention Strategy page is still live and unretracted but is absent from the new strategy** -- de-emphasized rather than revoked, with the durable mechanism now **Secondary Publication Rights in national copyright law.** So "comply with Plan S by paying an APC in a hybrid journal" is stale advice twice over.

**The RRS mechanism, if used**: the author puts a prior-obligation notice in the submitted manuscript applying a CC BY licence to any AAM arising from it. Publishers cannot override it because **the licence grant predates the publication agreement**. Some publishers responded by refusing submissions carrying the notice -- check the Journal Checker Tool before submitting.

**NIH** revised its public-access policy effective 1 July 2025: manuscripts **accepted** on or after that date must be in PubMed Central immediately at publication, with the 2008 embargo option gone. **Note the trigger is the acceptance date**, so a paper submitted in 2024 and accepted in August 2025 is in scope. A proposed APC cap was floated and had not been finalized as of this file's verification -- **check before advising**, because a $2,000 cap sits below most selective OA venues' APCs and would push authors to green or no-fee venues.

### The predatory and integrity apparatus

**DOAJ** (whitelist, with delisting events), **COPE**, **OASPA**, **WAME**, and the joint **Principles of Transparency and Best Practice**; **Think. Check. Submit.** as the author-facing checklist. Beall's list is historical and was taken down; do not cite it as current.

**Hijacked/clone journals**, **paper mills**, and **citation cartels** are the active threats. Mass-retraction events at scale have followed, with **delisting consequences** for the affected titles.

**On detecting AI-generated submissions, one inverted heuristic**: **tortured phrases predate LLMs** -- they are a text-spinner fingerprint. **LLM-generated submissions read cleaner.** So "look for tortured phrases to spot AI papers" is backwards. The Problematic Paper Screener remains useful for the spinner class specifically.

---

## 5. The machine-readable record

### Retraction detection -- the highest-value code-review item here, and it broke

**The Crossref Labs retraction-annotations endpoint was deprecated on 2026-05-29**, with no new updates pushed from either Crossmark or Retraction Watch, and no hard removal date announced.

**The failure mode is worse than a 404.** Trigger: a citation-checking pipeline written before mid-2026 against `api.labs.crossref.org` annotations. **Symptom: the endpoint keeps returning HTTP 200 with plausible-looking data frozen at May 2026 -- so every retraction after that date is silently reported as clean.** The check's entire purpose (catching *new* retractions) is exactly what fails, and nothing errors.

**The correct source**: `api.crossref.org`, using `update-to` / `updated-by`, with the **`source` field** distinguishing `"publisher"` (Crossmark) from `"retraction-watch"` (curated). A pipeline reading only publisher deposits misses the curated flags. Bulk CSV at `gitlab.com/crossref/retraction-watch-data`, git-cloneable, updated every working day.

Three data-modeling traps in that CSV:

1. **`RetractionNature` is not a boolean.** It carries Retraction, Correction, **Expression of concern**, and **Reinstatement** in one field. `is_retracted: bool` over-flags expressions of concern and **never un-flags reinstatements**.
2. **Two DOI columns.** `OriginalPaperDOI` is the thing you cited; `RetractionDOI` is the notice. **A naive join on the wrong column reports the notice as retracted.**
3. **Coverage is incomplete on both sides.** Crossref knows what publishers deposit plus what Retraction Watch curates by hand, and a retraction can be live on a publisher's site and absent from both feeds for weeks. **So a pipeline must report "no retraction found as of &lt;date&gt;", never "not retracted."**

Also: the Crossref/Retraction Watch arrangement is a **five-year time-limited licence with an annual payment**, not a perpetual gift, so a hard dependency carries policy risk, not just availability risk. **PubMed** `PublicationType` provides a third independent signal for biomedical work.

### Indexing -- the highest-value five-line fix in scholarly web publishing

Google Scholar uses the Highwire `citation_*` convention and requires **exactly three tags**: `citation_title`, `citation_author` (repeated per author), and `citation_publication_date`.

**Two details that break pipelines:**

- **The date format is `2010/5/12` -- slash-separated, not zero-padded, NOT ISO 8601.** A pipeline emitting `2010-05-12` is emitting the wrong format for this consumer.
- **"Pages that don't provide any one of these three fields will be processed as if they had no meta tags at all."**

**The failure mode**: a template that emits the date only when an issue is assigned, so continuous-publication and ahead-of-print articles ship without it. **Symptom**: Scholar falls back to scraping the rendered page, then indexes the paper under a wrong title (often the site name or a nav heading), attributes it to wrong authors, splits it from the VoR record, or drops it. **No error, no warning, no dashboard** -- the only signal is that the paper does not appear, or appears wrong, weeks later. **This silently costs citations.**

For grey literature: **`citation_technical_report_institution` and `citation_technical_report_number` are how a technical report or white paper becomes findable in Scholar.** Most corporate white-paper pages emit no citation tags at all, which is the single biggest reason industry technical writing is uncitable in practice.

`citation_pdf_url` must point at a directly downloadable PDF for full-text indexing. Emitting `citation_*` **plus** schema.org JSON-LD covers both consumers; **emitting only schema.org loses Google Scholar**, which explicitly prefers `citation_*` over Dublin Core.

### The substrate

**JATS** (NISO Z39.96) is what publishers actually store articles in; **BITS** for books, **STS** for standards, **MECA** for manuscript exchange. **DOI registration** runs through Crossref (literature) or DataCite (data, software, grey literature), with different schemas and APIs. **ORCID** for identity with auto-update; **ROR** for affiliations; the Crossref funder registry (now converging on ROR) for funders.

The open citation graph: **OpenCitations/COCI**, **OpenAlex** (successor to the shut-down Microsoft Academic Graph), **Semantic Scholar**, against closed **Scopus** and **Web of Science**. **OAI-PMH** remains the repository harvesting interface.

---

## 6. Preservation, reproducibility, and artifacts

### Reference rot -- get the number right

The widely quoted "one in five" is **20% of all STM articles**, but **roughly 70% of articles that cite the web at all**. And **content drift means a 200 OK proves nothing** -- the URL resolves, the content changed. **Status-code link checkers miss this entirely.**

The mitigations: **Memento** and robust links (`data-versionurl`, `data-versiondate`), **Perma.cc** for legal citation, **Software Heritage** for code, and the Wayback Machine. For journals: **CLOCKSS, Portico, LOCKSS**, and the Keepers Registry. **A journal that ceases without preservation takes its content with it.**

### "Available on request" -- the number that ends the argument

Gabelica et al. (2022): **42% of data-availability statements used that phrase; of 1,792 authors contacted, 93% did not respond or refused.** That is a compliance rate **identical to papers with no statement at all.** So the statement carries no information, and treating it as satisfying a data-availability requirement is unsupported.

### Artifacts and badging

**FAIR** -- Findable, Accessible, Interoperable, Reusable.

**ACM swapped the definitions of "reproducibility" and "replication"** on NISO's recommendation and retro-updated all badges. **Pre-swap literature uses the opposite sense**, so a paper's badge means different things depending on when it was awarded. Also: **"Artifacts Available" certifies deposit location only**, and **ACM states badges are not comparable across venues.**

**Software citation**: `CITATION.cff` has only **four required keys**, and **the spec itself warns that `preferred-citation` may violate the FORCE11 "Importance" principle** -- pointing it at a companion paper stops the software itself accruing citations. Zenodo's **concept DOI versus version DOI** distinction determines whether a citation is reproducible.

Reporting guidelines by study type -- **CONSORT** (trials), **PRISMA** (systematic reviews), **ARRIVE** (animal research) -- indexed at the **EQUATOR Network**. Trial registration is a publication precondition in medicine. Preregistration via OSF Registries or AsPredicted.

---

## 7. Industry white papers and technical reports

No peer review, so **the document must supply its own credibility apparatus**. What carries it:

1. **Named authorship with affiliation**, ideally ORCIDs. "By the Acme Research Team" is uncitable and unaccountable.
2. **A date** -- publication and last-revised.
3. **A version identifier on the document face**, monotonic.
4. **A stable URL**, not the marketing site's `/resources/` path, plus ideally a **DOI**.
5. **Methodology disclosure** -- sample, instrument, timeframe, exclusions; for benchmarks, hardware, versions, configuration, and number of runs.
6. **Data provenance** -- and if it is the vendor's own telemetry, which it almost always is, say so, with the denominator.
7. **Conflict-of-interest disclosure.** The vendor sells the thing the paper concludes is good. **Saying so is the highest-credibility move available and costs nothing.**
8. A reference list with resolvable identifiers, a licence, and reproducibility artifacts where applicable.

**The highest-leverage advice in this section**: a **Zenodo deposit with a DOI converts a white paper from ephemeral collateral into a citable, archived, versioned record at zero marginal cost.** arXiv accepts in-scope industry work (subject to moderation that declines marketing); SSRN is the norm in economics, law, and business.

**Technical-report series numbering is the durable citation mechanism** -- an immutable institutional identifier (`UCB/EECS-2024-107`, `NIST SP 800-53r5`). **The number, not the URL, is the citable handle**, which is why an institutional report series beats a corporate blog post even when both are just a PDF on a website.

**The specific failure: the undated, unversioned, unattributed white paper.** Trigger: a PDF at `acme.com/resources/whitepaper.pdf`, **overwritten in place** on each revision, corporate byline, no date. Compounding symptoms: it **cannot be cited** (no year, no author, no version); a quote from it becomes unverifiable the moment the file is overwritten, which is **content drift with the URL held constant** -- the citing work now misquotes the source with no way to detect it; a systematic reviewer must exclude it on methodological grounds; it accrues no Scholar record and therefore no citations; and internally nobody can tell which version a customer or regulator holds.

**Grey-literature discovery is irreducibly fragmented.** OpenGrey closed in 2020-2021 with **no single replacement**. What substitutes: OpenAIRE Explore, CORE, BASE, OpenAlex (`type: report`), Google Scholar (only if `citation_*` tags exist), plus domain repositories -- NTRS, OSTI.GOV, RePEc, SSRN, NBER, ERIC, TRID, WHO IRIS, World Bank OKR. A systematic reviewer must search several by hand.

---

## 8. Authorship, credit, and AI

**ICMJE's four criteria**, all required, and **criterion 4 (accountability) is why AI cannot be an author** -- a non-person cannot be accountable. **CRediT** provides 14 contribution roles. Gift and ghost authorship are Goodhart responses to authorship counting.

**Publisher AI policies are VOLATILE and must be checked, not recalled.** The stable core across COPE, ICMJE, and major publishers: **AI is not an author**, and **use must be disclosed** in methods or acknowledgements. Detection tools are unreliable in both directions.

**The Frontiers rat-figure incident is the case that matters, and it is usually told wrong: the paper disclosed its Midjourney use.** The failure was **review**, not disclosure. **Which means an AI policy that ends at "authors must disclose" prevents nothing.**

*Copyright in AI output, human-authorship requirements, and licence questions route to `copyright-and-permissions`.*

---

## 9. Metrics and assessment

**The Journal Impact Factor** is a mean of a skewed distribution over a two-year window, is not comparable across fields, and describes a journal rather than a paper. **h-index** conflates productivity with impact and cannot decrease. Citation counts are database-dependent (Scopus, WoS, Scholar, and OpenAlex disagree).

**The reform positions**: **DORA** (San Francisco Declaration on Research Assessment) and the **Leiden Manifesto**.

**Why this belongs in a publishing lens**: **venue choice is often an assessment decision rather than a communication decision**, and an author optimizing for a national evaluation system (REF, ERA) or a ranking list (ABS, ABDC, CORE) is making a rational choice that a pure communication analysis will misread as irrational.

---

## 10. Schools of thought (live, unreconciled)

### Is peer review broken?

**The critique.** **Richard Smith** (BMJ editor 1991-2004) calls it "faith-based (not evidence-based), slow, wasteful, ineffective, largely a lottery, easily abused, prone to bias, doesn't detect fraud and irrelevant." His empirical exhibit is devastating because he ran it as an editor: **8 deliberate errors inserted into a 600-word paper, sent to 300 reviewers -- none spotted more than five, a fifth spotted none, the median was two.** **Ivan Oransky and Adam Marcus** (Retraction Watch) contribute the evidentiary record of what review let through. **Elisabeth Bik**'s image-forensics work is the strongest evidence that **review does not detect image manipulation** and that detection depends on a handful of unpaid individuals who are frequently met with legal threats. Structurally: reviewer supply is unpaid while margins are high, reviewer agreement is barely above chance, and nobody reruns anything.

**The counterposition, stated properly.** From "Rumors of the Demise of Peer Review are Premature": (1) **no proposed alternative has been shown to do better on the same measures** -- post-publication review has been available for two decades and attracts almost no participation, and open review changes tone more than accuracy; (2) **the critique measures review against a job it never claimed** -- it was never a fraud detector or a replication engine, and its demonstrable functions (improving manuscripts, catching methodological errors authors accept, recruiting expertise, filtering the obviously unsound) are narrower and real; (3) **the counterfactual is not "no filter" but "an unmanaged filter"** -- attention, prestige, and social media allocate credibility otherwise, with no reason to think they are less biased; (4) **selection effects** -- retractions are the visible failures, and correctly rejected papers are invisible and uncounted.

**Reform-not-abolish positions worth naming as distinct**: eLife's **publish-review-curate**, **Peer Community In**, **SciPost**, **Review Commons**, and registered reports.

**Do not resolve this.** Both sides argue from real evidence about **different variables**: the critics measure error detection and fairness, the defenders measure the counterfactual.

### Preprints: acceleration versus the COVID critique

**Pro**: removes 6-18 months from the record, establishes priority without a gatekeeper, makes null results postable, free to author and reader. COVID is the pro case too -- sequence and epidemiological data circulated in days.

**Critique**: during COVID, preprints were reported by mass media **as findings**, and the "not peer reviewed" framing did not survive the retelling. Withdrawn preprints continued to be cited and shared, and the withdrawal mechanism is weak against screenshots and press coverage. **The sharper version is not "preprints are bad" but "preprints assume a reader who can calibrate, and the media layer removed that assumption."**

**Unresolved core**: whether the harm is intrinsic to preprints or a property of science journalism and platform amplification. Both sides point at the same COVID record.

### Gold versus green versus diamond

**Gold/APC**: immediate open VoR funded by the party benefiting from dissemination; scales; converts subscription spend to publishing spend; auditable per-article cost.

**The exclusion critique** (the strongest single argument against APC-gold): **APCs move the paywall from reader to author**, excluding exactly the authors with no grant -- unfunded and early-career researchers, independent scholars, and researchers in low- and middle-income countries. Waivers are discretionary, must be requested, and largely exclude middle-income countries where most excluded authors are. **A publishing inequity replaces a reading inequity, and the affected population is different and less powerful.** **Björn Brembs** is a leading voice. The second-order effect is structural, not a slur: **APC revenue rewards volume, which is the economic engine behind the special-issue and paper-mill problems.**

**Diamond**: no fee to read or publish, funded by institutions and societies, with an existence proof at scale in Latin America (**SciELO**, **Redalyc**) and much of the humanities -- and now cOAlition S's stated direction. **Counter-critique**: fragile funding, small scale per title, uneven infrastructure (DOIs, JATS, preservation, indexing), and dependence on unpaid academic labour, **which is a hidden subsidy rather than an absence of cost.**

**Green** (**Stevan Harnad**): the fastest and cheapest route is universal self-archiving; no money changes hands and no publisher must consent. **Counter**: green delivers the AAM not the VoR, discovery is worse, version confusion is real, and it leaves the subscription system intact -- **which critics call parasitic and advocates call pragmatic.**

**Peter Suber** is the definitional authority (*Open Access*, MIT Press, free online) and keeps the distinctions honest: **gratis versus libre, and green versus gold as venues rather than licences.** The declarations, in order: **Budapest** (2002, which coined the term), **Bethesda** and **Berlin** (2003); Budapest's 20th-anniversary recommendations explicitly warn against APC dependence.

**The learned-society sustainability argument**, usually underweighted by OA advocates: many societies fund conferences, fellowships, and prizes from journal surplus. Flipping to diamond removes that surplus with no replacement, and the society's non-publishing functions are the casualty. **This is not publisher rent-seeking; it is a genuine transition-financing problem with no agreed answer.**

### Double-anonymous versus open review

**Double-anonymous** reduces demonstrated bias against women, non-elite institutions, and non-Anglophone authors. **Open review** argues that accountability improves review quality and that anonymity shields bad behavior, while conceding that junior reviewers face retaliation risk when signing critical reviews of senior figures. Neither has decisive evidence on accuracy.

---

## 11. Anti-pattern catalog

| Pattern | Trigger | Consequence | Fix |
|---|---|---|---|
| Retraction check on the Labs endpoint | Pipeline written pre-mid-2026 | **HTTP 200 with data frozen at May 2026** -- new retractions silently reported clean | Migrate to `api.crossref.org` `update-to`/`updated-by` |
| `is_retracted: bool` | Modeling the RW feed | Over-flags expressions of concern, never un-flags reinstatements | Enum on `RetractionNature` |
| Join on `RetractionDOI` | Two DOI columns | Reports the notice as retracted | Join on `OriginalPaperDOI` |
| "Not retracted" as an assertion | Feed treated as complete | Both feeds lag publisher sites by weeks | "No retraction found as of &lt;date&gt;" |
| ISO date in `citation_publication_date` | Reasonable-looking default | Wrong format for this consumer | `2010/5/12` |
| Date tag emitted only when an issue exists | Continuous publication | **Scholar processes the page as if it had no tags at all** -- wrong title, wrong authors, or dropped | Always emit all three required tags |
| schema.org JSON-LD only | Modern-metadata instinct | Loses Google Scholar entirely | Emit `citation_*` too |
| No citation tags on a report page | Corporate CMS | The white paper is uncitable in practice | `citation_technical_report_*` |
| Status-code link checking | Standard tooling | **Content drift passes a 200** | Memento/robust links, archived snapshots |
| "Data available on request" | Journal requires a statement | **93% non-response; identical to no statement** | Deposit with a DOI, or state the actual restriction |
| `preferred-citation` to a companion paper | CITATION.cff convenience | The software accrues no citations | Cite the software; list the paper separately |
| Zenodo concept DOI in a methods section | Latest-version convenience | Resolves to a behaviorally different release | Version DOI |
| `\author{}` left in a double-anonymous PDF | `hyperref` default | Authorship in PDF metadata despite a suppressed title block | `pdfinfo` and `exiftool` before submitting |
| Glob-swept submission tarball | `tar czf` over the working dir | `.git` history, internal comments, sometimes **live credentials** publicly downloadable | Build the tarball from an explicit file list |
| "In our previous work [17]" | Natural phrasing | Deanonymizes; but deleting self-citations reads as weak related work | Third-person phrasing, reference retained |
| Advising an APC for Plan S compliance | 2021-era knowledge | Stale twice over -- APCs named as a problem, direction is diamond | Green AAM deposit is robust to every branch |
| Assuming the Nelson memo is in force | 2022-2024 knowledge | Being repealed, consequence undefined | Check; advise green |
| "Tortured phrases mean AI" | Plausible heuristic | **Inverted** -- LLM text reads cleaner; tortured phrases are a spinner fingerprint | Use the screener for the spinner class only |
| Pre-swap ACM badge semantics | Literature written before the change | "Reproducibility" and "replication" mean the opposite | Check the award date |
| Treating a CS conference paper as a precursor | Cross-field habit | Misreads a CS record entirely | Terminal venue in CS |

---

## 12. Authorities

**Normative bodies**: **COPE** (and its specific flowcharts, which are the practical instrument), **ICMJE Recommendations**, **CSE**, **DOAJ**, **OASPA**, **STM** (and the Integrity Hub), **NISO** (JAV for versioning, CRediT, access and licence indicators), **EQUATOR Network** (reporting guidelines), **FORCE11** (software and data citation principles).

**Infrastructure**: **Crossref** and **DataCite** documentation, **ORCID**, **ROR**, **JATS** (`jats.nlm.nih.gov`), **OpenAlex**, **OpenCitations**, **Zenodo**, **Software Heritage**, **CLOCKSS/Portico/LOCKSS**.

**People and positions**: **Peter Suber** (*Open Access*, MIT Press, free -- the correct first reading, and the keeper of the gratis/libre and green/gold distinctions). **Richard Smith** (the peer-review indictment). **Ivan Oransky and Adam Marcus** (Retraction Watch). **Elisabeth Bik** (image forensics). **Björn Brembs** (against APCs and publisher power). **Stevan Harnad** (green self-archiving). **DORA** and the **Leiden Manifesto** (assessment reform). **The Scholarly Kitchen** for industry commentary, read as commentary.

---

## 13. Severity rubric (domain-specific)

- **blocker** -- a mechanism that fails silently while appearing to work: a retraction check against the deprecated Labs endpoint, a missing required `citation_*` tag on a published article page, a submission tarball leaking credentials or a `.git` history, a data-availability statement the project cannot actually honor, an artifact DOI that resolves to a different version than the one used.
- **major** -- a broken promise or a stale-policy exposure: `is_retracted: bool` modeling, a join on the wrong DOI column, "not retracted" asserted as fact, status-code-only link checking, schema.org without `citation_*`, an undated and unversioned public document that cannot be cited, `preferred-citation` suppressing software citations, advising an OA route that current policy has moved away from, deanonymization channels left open in a double-anonymous submission.
- **minor** -- fragile or suboptimal: concept DOI where a version DOI belongs, a preprint cited where a VoR exists, missing ORCIDs on a white paper, missing `citation_pdf_url`, badge semantics cited without an award date.
- **nit** -- venue-name formatting, ordering within a contributorship statement, choice among equivalent repositories.
- **insight** -- a structural reframing: this white paper needs a DOI and a version identifier to be citable at all; this integrity problem is a Goodhart response to a specific metric, so the metric is the intervention point; the author is caught between an OA mandate and a prohibition on paying for it, and green is the only robust route; this venue choice is an assessment decision rather than a communication decision.

**Confidence calibration**: mechanism and format findings (metadata tags, API fields, data-model shape) are checkable and should be high-confidence. **Policy findings must carry a date and a re-check pointer**, because the funder, publisher, and infrastructure surface here moves faster than anything else in the publishing domain. **Never state a current policy from recollection.**

---

## Changelog

- **2026-09-09** -- Initial file. Five current-state facts established that contradict likely recollection: **the Nelson memo is being repealed with the consequence undefined**, alongside a federal-funds prohibition on high publishing costs that can leave an author required to publish OA and forbidden from paying for it; **Plan S restructured** (2026-2030 strategy names APCs as a problem, pivots to diamond, new Director and Secretariat May 2026, RRS de-emphasized in favour of Secondary Publication Rights); **the Crossref Labs retraction endpoint was deprecated 2026-05-29 and still returns HTTP 200 with data frozen at that date**; **tortured phrases predate LLMs** so the common AI-detection heuristic is inverted; and service naming churn (SHERPA/RoMEO → Jisc open policy finder, OpenGrey closed with no successor). Also recorded: **ACM swapped "reproducibility" and "replication"** and retro-updated badges, so pre-swap literature is inverted; **Google Scholar's three required tags with the `2010/5/12` date format**, and that missing one makes the page processed as if it had no tags at all; the **93% non-response rate** behind "available on request"; reference rot is **~70% of articles that cite the web**, not 20%; the 2026 study finding hidden information in **nearly every** arXiv source submission including live credentials; and that **the Frontiers rat paper disclosed its AI use**, so disclosure-only policies prevent nothing. Twenty residual UNVERIFIED items with URLs are listed in the research notes.
