---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-09
---

# Copyright, notices, and permissions

A reference for advising on and reviewing rights, notices, and clearance for books, publications, and white papers -- including pipelines that **generate** notices and attribution lists. Used by the `copyright-and-permissions` subagent and the `/expert-review` / `/expert-plan` / `/expert-consult` / `/consult` skills.

**Source research**: `~/.claude/local/research-notes/copyright-and-permissions-research.md`

**This file reports mechanism, never verdict.** It is engineering and practice guidance, not legal advice. Section 10 states the counsel boundary explicitly, and it binds every other section.

Weighting: **US-primary** (17 U.S.C., US Copyright Office) as the default frame, with Berne, EU (InfoSoc 2001/29, DSM 2019/790), and UK (CDPA 1988) as named contrasts. **Advice framed for US fair use is wrong for a UK or EU publication**, and saying so is often the single most useful correction available.

The unifying thesis: **a copyright page, a credit list, and a third-party-notice file are legal assertions with the ergonomics of generated code.** Every generated-code failure mode applies -- staleness, incompleteness, unfalsifiable correctness, transformation damage -- but the blast radius is a rights claim rather than a stack trace. The contract question is the same one you would ask of any generated artifact: **what does this assert, who relies on it, and what breaks if the generator is replaced?**

The agent's operational question: **"what does this artifact claim about rights, on what mechanism does that claim rest, and who is exposed if it is wrong?"**

The empirical priority, in rough order of how often it bites:

1. **Confident myths** -- the 300-word rule, "transformative therefore fair use", "government document therefore public domain", "we paid the freelancer therefore we own it". These are the highest-frequency errors and they are *correctable facts*, not judgment calls.
2. **Incomplete generated attribution** -- scanners see package manifests, and a book is not made of packages.
3. **Notices that assert the wrong thing** -- a build-time year, a stale year, an unsupported range.
4. **Licence incompatibility inside one document** -- a CC BY-NC figure in a CC BY article.
5. **Clearance gaps discovered at print time** -- editorial-use-only images, unlicensed fonts, uncleared lyrics.
6. **Jurisdiction mismatch** -- fair-use reasoning applied to a UK or EU publication.

---

## Volatile surface

`last-verified` is in the frontmatter -- do not restate it in prose. These rot; the rest of this file is comparatively durable.

| Claim class | Rots | Re-verify at |
|---|---|---|
| **AI-training litigation** | **Fastest class here; weekly** | Court dockets; do not rely on secondary reporting |
| Publisher AI policies | **Fast** | Each publisher's current author-policy page |
| Funder OA mandates (Plan S, NIH, OSTP) | **Fast and politically contingent** | Each funder; route to `scholarly-publishing` |
| Stock-agency terms, especially AI clauses | Fast | Getty / Shutterstock / Adobe Stock ToS |
| Annual public-domain boundary | Yearly, predictably | `copyright.cornell.edu` public-domain chart |
| Statutory damages figures | Slow | 17 U.S.C. 504(c) |
| Copyright Office circular numbering | Slow | `copyright.gov/circs/` |
| CC licence version status | Slow (4.0 since 2013) | `creativecommons.org` |
| EU DSM national implementation | Medium, per member state | Per-state transposition |
| Statute and treaty text (17 U.S.C., Berne, CDPA) | **No** -- the stable layer | `uscode.house.gov`, WIPO Lex, `legislation.gov.uk` |

---

## 1. Copyright notice: form, and what it still buys

### The statutory form

**17 U.S.C. 401(b)** for visually perceptible copies requires three elements: the symbol **©** (or the word "Copyright", or the abbreviation "Copr."), **the year of first publication of the work**, and **the name of the owner**.

**17 U.S.C. 402(b)** for phonorecords specifies **℗ alone** -- there is no "Copyright"/"Copr." alternative for a sound recording. An audiobook layer therefore carries its own ℗ notice with **the year of first publication of the recording**, which is not necessarily the book's year.

### What notice buys, post-1989

**Notice became optional in the US on 1989-03-01**, when the Berne Convention Implementation Act took effect. It is not a condition of protection. What it still does:

- **17 U.S.C. 401(d) / 402(d)**: where a proper notice appears on the copy to which a defendant had access, **no weight is given to a defendant's innocent-infringement claim in mitigation of damages**. That is the concrete benefit -- it forecloses a damages-mitigation argument, not liability.
- Signalling and ordinary trade convention.

**"All rights reserved"** is a Buenos Aires Convention artifact with no current US legal function. Universally retained anyway; harmless.

**The three-era rule** matters when the *status* of an older work depends on notice: pre-1978, 1978-1989, and post-1989 works are governed differently, and for 1923-1977 US works you must also clear **notice and renewal** before concluding anything about public-domain status.

### Multi-year notices -- resolved

The Copyright Office **Compendium § 2205.1(A)** expressly accepts "year of first publication followed by multiple year dates" and gives the form **`1981, 1982, 1983`** -- a **comma-separated list of actual publication years, not a hyphenated range.** There is no statutory basis for `2019-2026`. And § 2205.1(F) shows the printed years interact with a registration's Limitation of Claim, so the printed list is not cosmetic.

---

## 2. Term and the public domain

- **Current US term**: life + 70 for a natural author. For **works made for hire**, anonymous, and pseudonymous works: **95 years from publication or 120 years from creation, whichever expires FIRST.** The common error states it as "the longer of" -- it is the **shorter**.
- **The annual boundary**: the rule is publication year + 96 for the 1 January boundary, so **works published in 1930 entered the US public domain on 2026-01-01**. State the rule, not just the year, because the year changes annually. Sound recordings run on a separate Music Modernization Act timeline.
- **For 1923-1977 US works, term is not sufficient** -- published-without-notice and non-renewal both matter, and both require research. Do not quote a non-renewal percentage; the commonly cited 85-93% is unverified.
- **The Cornell "Copyright Term and the Public Domain" chart is the canonical working tool.** Use it rather than reasoning from the statute.

### Government works -- the most common single error

**17 U.S.C. 105 covers US FEDERAL government works only**, and it does not reach **contractor-produced work** (a federal agency can hold copyright in commissioned work by assignment). It says nothing about:

- **State and local** government works, which are frequently copyrighted.
- **Foreign** governments. **UK Crown copyright** subsists and is licensed, not free.

**Crown copyright is one rule with two branches** (CDPA s.163(3)), not two rules: **125 years from the end of the year of making**, cut to **50 years from the end of the year of first commercial publication if commercially published within 75 years of creation.** Plus the **2039 rule** -- unpublished Crown works are protected for 125 years from creation *or until 2039-12-31, whichever is shorter*, a known cliff-edge that releases a large body of UK archival material on 2040-01-01. Works merely *assigned* to the Crown get the ordinary life+70 term.

Crown and Parliamentary material is licensed under the **Open Government Licence v3.0** / Open Parliament Licence: **commercial reuse permitted, attribution required in a specified form.** So it is usable in a commercial book and **generates a credit-line obligation** -- another manifest entry, not a free-form credit.

---

## 3. Fair use, fair dealing, and the closed lists

### The four factors, and the Warhol correction

**17 U.S.C. 107** factors: (1) purpose and character, including commerciality; (2) nature of the work; (3) amount and substantiality; (4) effect on the potential market. The preamble purposes (criticism, comment, news reporting, teaching, scholarship, research) are **illustrative, not a closed list** -- which is the structural contrast with the UK and EU. The final sentence, added in 1992, provides that unpublished status does not itself bar fair use.

**Campbell v. Acuff-Rose (1994)** made "transformative" the factor-one lodestar (from Judge Leval's 1990 formulation), held parody can be fair use, and established that commerciality is not dispositive.

**Andy Warhol Foundation v. Goldsmith, 598 U.S. 508 (2023) is the case to get right, and the one most often misdescribed.** What it actually did:

- The factor-one inquiry attaches to **the specific use challenged**, not the work in the abstract. The challenged use was AWF's **licensing of "Orange Prince" to a magazine for a cover** -- the same purpose Goldsmith licenses her photographs for. It was **not** a holding that the Prince Series is infringing as art.
- **New expression, new meaning, or new aesthetic is not sufficient on its own.**
- Factor one now folds in substitution and commerciality, so **factors one and four converge** and market substitution does more work than it did between 1994 and 2023.
- **Correct summary**: transformativeness survives, but it is now a question of **differing purpose measured against substitution**, not of added creativity.
- **The error to flag**: describing Warhol as narrowing fair use for derivative art generally. It narrowed the transformativeness prong **for competing-purpose commercial licensing**. Criticism, comment, parody, and genuinely different-purpose uses are untouched.

### The 300-word myth, and its two real origins

**There is no statutory word count.** The number persists because of two separate things:

1. **A publisher's own licence terms.** The University of Chicago Press grants gratis reuse of *its own* catalogue up to 5,000 words aggregate, 5% of the source, **300 consecutive words**, and 5% of the new work, excluding poems and images. That is a licence, not a statement of law.
2. **Harper & Row v. Nation Enterprises**, where roughly **300 words** quoted from an unpublished memoir was held **not** fair use.

So the number that circulates as a safe harbour is, in its most famous appearance, the amount that lost. **Publisher word limits are risk policy, and following them is sensible -- but they are not the legal standard, and they do not travel between publishers.**

High-risk quotation categories regardless of length: **poetry and song lyrics** (a few words can be a substantial portion), **epigraphs** (decorative, so factor one is weak), and **unpublished works**.

### The STM Permissions Guidelines -- countable, and CI-checkable

The **STM Permission Guidelines (2024)** are a voluntary reciprocal scheme among signatory STM publishers, and they are unusually useful because the limits are **numeric**: 3 figures per signatory per chapter, 30 per book, and 400/800-word thresholds.

Two non-obvious gates:

- **The 70% rule**: the guidelines apply only where **at least 70% of the primary work is newly commissioned, original, previously unpublished material** (no more than 30% previously published). **So an anthology, reader, or compiled volume falls outside the guidelines entirely.**
- **"Editing of figures for style purposes is permitted"** provided meaning and accuracy are preserved -- an explicit licence to restyle to house design, which is exactly what an automated production pipeline needs. **Contrast CC ND, which prohibits it.**
- Section 7 makes the **per-figure third-party rights note a contractual requirement**, which is the mechanism behind the CC BY incompatibility problem in §5.

### UK and EU -- and one correction worth carrying

**The UK quotation exception, CDPA s.30(1ZA), is purpose-agnostic** -- it does not require criticism, review, or any particular purpose. The real deltas from US fair use are not purpose closure but:

- **Mandatory sufficient acknowledgement.**
- **An explicit proportionality cap** ("no more than is required by the specific purpose").
- **No residual equitable doctrine.** Where US courts weigh factors openly, the UK has enumerated exceptions (ss.29, 29A, 30) and the EU has the **closed list of InfoSoc Article 5**, implemented differently across 27 member states.

**The DSM TDM exceptions, and the asymmetry nobody mentions**: Article 3 (scientific research by research organisations and cultural heritage institutions) and Article 4 (general, with a machine-readable opt-out). **Article 7(1) makes contract override unenforceable against Articles 3, 5, and 6 -- but omits Article 4.** So **the commercial TDM exception can be contracted away by terms of service, while the research one cannot.** That is load-bearing for anyone drafting or relying on a website TDM reservation.

Also **DSM Article 14**: a faithful reproduction of a public-domain visual work does not attract a new copyright -- the EU's statutory answer to the question *Bridgeman v. Corel* answered by case law in the US, and the opposite of what many European museums assert.

### AI training -- report the mechanism, never a verdict

**No appellate court has decided whether training a generative model on copyrighted works is fair use.** Everything below is district-court or foreign, and one key case settled before appeal. **Never state this as settled in either direction.**

- **Bartz v. Anthropic** (N.D. Cal.): the June 2025 summary judgment held that training on **lawfully acquired** books is fair use ("transformative -- spectacularly so"), that buying, scanning, and destroying print copies is a fair-use format shift, **but that downloading and retaining ~7M pirated books to build a permanent library is not.** **The acquisition/use split is the most stable thing on the board and the most commonly collapsed.** A $1.5B settlement followed; the fairness hearing was 2026-05-14 (92.77% claims rate, 447,576 works, ~$3,000/work) and was taken under submission. **Whether final approval has been entered is unverified -- check the docket.** Authors Alliance notes the settlement "does not create any precedent binding on future courts."
- **Kadrey v. Meta** (N.D. Cal.): summary judgment for Meta **explicitly because these plaintiffs failed to prove market harm**, not because training is lawful. Judge Chhabria wrote that in many circumstances training without permission would be unlawful, and floated **"market dilution"** -- now the leading plaintiff-side theory. **Quoting Kadrey's outcome without its reasoning is the classic misreport: it is a loss for those plaintiffs, not a win for developers.**
- **Thomson Reuters v. ROSS** (D. Del.): fair use **rejected** for training a legal-research tool on Westlaw headnotes. **Distinguishers matter**: ROSS's system was **not generative** and was a direct market substitute. Certified for interlocutory appeal; **current Third Circuit posture unverified.**
- **NYT v. OpenAI** (S.D.N.Y.): **undecided.** Summary judgment argued early September 2026. A DOJ brief reportedly supports OpenAI's position -- news-reported only, and politically contingent.
- **Getty v. Stability (UK)**, [2025] EWHC 2863: Getty **abandoned its primary copyright claims mid-trial** on jurisdictional and evidential grounds, and the court held **model weights are not an "infringing copy" because the model stores no copies of the training works.** That is the most consequential holding available on the "are weights a copy" question, and it went the developers' way -- **on a narrower question than commentary claims**, in the UK, and it creates **no UK fair-dealing safe harbour for training.** The watermark finding was a **trade mark** claim, not copyright.

**One thing that IS near-settled, and is routinely conflated with the above**: **human authorship is required for US copyright** (Thaler, D.C. Cir. 2025, cert denied 2026-03-02). A wholly AI-generated work is not copyrightable and must be disclaimed on registration. That is a different question from whether training is infringement, and commentary mixes them constantly.

**A litigation-shaped engineering lesson**, from the NYT discovery fight over ~20M conversation logs and the 2026 sanctions motion: **output logs and training-corpus manifests are discoverable artifacts and become subject to litigation holds.** Design for that.

---

## 4. Permissions clearance in practice

### What actually needs clearance in a book

Images and figures, tables, long quotations, **poetry and lyrics** (the notorious high-risk category), screenshots (with a trade-dress overlay), maps, previously published chapters, and third-party code.

The workflow: identify the rightsholder, send a request specifying **territory, format, print run, edition, language, duration, and media**, and absorb the response-time reality -- **clearance is a schedule risk, not a formality.** Fees range from free-for-scholarly-use to substantial, unpredictably.

### The image failure that recurs most

**"Editorial use only" fails in a commercial book.** A sold book is a commercial use, and an editorial-only asset has **no model release and no property release**. This is the most common image-rights failure in publishing, and it is mechanically checkable in a manifest.

Related: **rights-managed vs royalty-free vs editorial-only** are three different products; museum and archive reproduction fees are **contract terms, not necessarily copyright** in the underlying work (compare *Bridgeman v. Corel* on "slavish copies" with the opposite rule in **DSM Art. 14**); and **PLUS** provides the standard vocabulary for expressing all of it in metadata.

### Fonts -- the most-missed obligation in print production

**Desktop, web, app, and ebook/embedding are separately licensed tiers.** A desktop licence does not authorize embedding in an ebook or a PDF for distribution. Additional traps:

- **Subsetting** can violate a no-subsetting clause.
- **OFL Reserved Font Names**: modifying an OFL font requires renaming it.
- **Vendoring a commercial font into a repo** for build reproducibility is **redistribution** and usually prohibited -- which collides directly with the pipeline requirement that fonts be available identically in CI. Name the collision; it has no clean technical answer.
- The `fsType` embedding bits express foundry intent; **the EULA governs.**

### Orphan works

No US legislative solution exists (repeated attempts failed). The **EU Orphan Works Directive 2012/28** and the **UK orphan works licensing scheme** provide routes in those jurisdictions. A **diligent-search record** is the artifact that matters -- it does not confer permission, but it evidences good faith. Keep it in the manifest.

---

## 5. Open licensing and attribution mechanics

### Creative Commons

The six licences, plus CC0. **What BY actually requires is TASL**: **T**itle, **A**uthor, **S**ource, **L**icence. CC **4.0** added a **30-day cure provision** that 3.0 lacked, and handles database rights; **4.0 is the current suite** and has been since 2013.

Points that matter in practice:

- **CC licences are irrevocable.** A rightsholder who changes their mind cannot un-license already-distributed copies.
- **ND prohibits derivatives**, which includes restyling a figure to house design -- **the exact operation STM's guidelines permit.** A production pipeline that restyles figures must know which regime each asset is under.
- **NC is genuinely ambiguous** and a sold book is the easy case: it is commercial.
- **The Public Domain Mark is a third party's assertion about a work, not a grant.** **CC0 is the grant you can rely on.** Wikimedia Commons files are individually licensed and their PD rationales can be wrong for your territory.

### The licence-conflict-inside-one-document failure

**A CC BY-NC-SA figure embedded in a CC BY article is a licence conflict**: the article's blanket CC BY grant is broader than the rights actually held in the figure, so the article asserts a permission it cannot give. This is common under funder mandates (Plan S CC BY requirements) and it is why **STM section 7 makes a per-figure third-party rights note contractual.**

**The check is mechanical**: for every included asset, is the asset's licence at least as permissive as the licence the containing document grants? A pipeline can verify that. Almost none do.

### Software and content in publications

- **GPL/AGPL code listings** in a book, and **Apache-2.0 NOTICE obligations** for a book's companion repo or sample app -- route dependency-level analysis to `licensing-and-oss`, but note that the obligation **follows each distributed artifact separately** (see §6).
- **Stack Overflow snippets are CC BY-SA** (4.0 since May 2018, 3.0 before). Pasting one into a book's prose carries **attribution and share-alike** obligations on a text snippet. Almost universally violated.
- **GFDL** and Wikipedia/Wikimedia reuse carry their own obligations.

---

## 6. Generated notices and attribution (the pipeline angle)

### The year: the canonical bug, in three increasingly subtle forms

1. **Hardcoded and stale** -- `© 2019` still in the template in 2026. **Ironically often the least wrong**, because the year of first publication does not change.
2. **`date +%Y` at build time** -- three separate failures: the artifact is not reproducible (the PDF hash changes with no source change); **a reprint of the 2019 edition run in 2026 prints "© 2026", asserting a false year of first publication**; and CI shows a diff on 1 January that nobody authored.
3. **`© 2019-{{build_year}}`** -- combines both failures and adds an unsupported form.

**The correct rule, stated as an invariant**: the notice year is **the year of first publication of this edition** (17 U.S.C. 401(b)(2)). It is **a data field on the edition, checked into source, changed by a human when a new edition publishes. `copyright_year` is an input, never a computed value.**

- A revised edition gets its own year, represented as a **comma-separated list of actual years**.
- **A reprint with no new copyrightable material is not a new edition and does not get a new year.**
- An audiobook layer needs its own ℗ year.
- **The test**: a build of a two-year-old tag must reproduce the same copyright page byte-for-byte. That golden-file test catches all three bugs.

### Attribution lists from dependency scanners are structurally incomplete

**Scanners see package manifests, and a book is not made of packages.** What `license-checker`, `cargo-about`, `pip-licenses`, or an SBOM tool **will miss**:

- **Fonts** (the most-missed obligation in print).
- **Images, photographs, figures, icon sets** -- Font Awesome, Material Icons, the Noun Project, each with its own terms.
- **Sample data and datasets** -- a CSV under ODbL or CC BY-SA carries attribution and share-alike.
- **Code snippets pasted into prose** -- Stack Overflow's CC BY-SA, or a gist with no licence at all.
- **Vendored code** with no manifest entry, and anything in an untraversed **git submodule**.
- **CDN-referenced assets** rather than installed ones.
- **Apache NOTICE content** (see below).

**The correct architecture**: the scanner is **one input**, and the authoritative source is a **hand-maintained, machine-verified manifest**. A notice file whose only input is a package scanner will always be incomplete for a publication, and **its completeness is unfalsifiable from inside the build** -- which is precisely why it needs an external manifest to check against.

### SPDX identifiers do not satisfy an attribution obligation

**This is the most useful single correction in the pipeline domain.** An SPDX identifier (`MIT`, `Apache-2.0`) **names a licence. It does not reproduce the licence text or the copyright notice** -- and it is the notice plus the text that the licence requires.

MIT and BSD both require that "the above copyright notice and this permission notice shall be included in all copies or substantial portions." A line reading `left-pad - MIT` satisfies **neither** clause: it omits the holder's copyright line and omits the permission text.

**So an attribution page needs, per dependency: the copyright notice line(s) and the licence text.** Where many dependencies share a licence, the compliant pattern is to list each component with its copyright line, then reproduce each distinct licence text once with components mapped to it.

**SPDX remains valuable as an index and compatibility key -- it is a lookup key, not the payload.** Treating the key as the payload is the failure.

**Corollary: licence text must be reproduced verbatim, whitespace included.** Do not let a markdown pipeline re-wrap it, smart-quote it, convert `(c)` to `©`, or lowercase the ALL-CAPS warranty disclaimer -- **that capitalisation is a conspicuousness convention drawn from UCC 2-316 practice**, and reflowing it departs from the required text. **Store licence texts as opaque blobs, render preformatted, and golden-file-test them byte-for-byte.**

### Apache-2.0 NOTICE, and per-artifact surfaces

**Section 4(d) is a distinct obligation from 4(a)-(c), and generated pipelines drop it.** If the distributed work includes a NOTICE file from upstream, you must include a readable copy of **the attribution notices in that NOTICE file**. Mechanics that break:

- **NOTICE content is additive and transitive** -- yours must aggregate upstream NOTICE content from every Apache-2.0 dependency that ships one. **A scanner reading `LICENSE` and ignoring `NOTICE` produces a silently non-compliant artifact.**
- NOTICE is **not** the licence text; both are required when a NOTICE exists.
- 4(b) separately requires a **statement of modification** in changed files.
- **The obligation follows each distributed artifact separately.** The print book, the ebook, the sample repo, and the companion app each need their own compliant surface. **A pipeline that generates one notice file and puts it in one artifact leaves the others bare.** For a publication the surfaces are the copyright page, a back-matter credits section, an appendix, or a navigable ebook section -- **a companion URL alone is risky**, since the obligation is a readable copy *in the distribution*, and URLs rot.

### Image credits from metadata the pipeline already destroyed

Rights data genuinely lives in IPTC/XMP: `dc:rights`, `photoshop:Credit`, `xmpRights:UsageTerms`, `xmpRights:WebStatement`, `plus:Licensor`, `plus:CopyrightOwner`, `plus:ModelReleaseStatus`, `plus:PropertyReleaseStatus`, `plus:DataMining`, and `Iptc4xmpExt:DigitalSourceType` (which is how AI provenance is declared).

**On which tools strip it -- the folklore is half wrong, and this was tested empirically rather than repeated:**

| Tool | XMP after a resize |
|---|---|
| **ImageMagick `magick -resize`** | **PRESERVED** (a 540-byte XMP packet survived); `-strip` destroys it |
| `cwebp` | **STRIPPED** by default |
| `jpegtran` | **STRIPPED** by default |
| `sharp` | **STRIPPED** by default (per its docs) |

So "ImageMagick strips metadata by default" is **false**. But the operative rule is unchanged and stronger: **never make the image file the system of record for rights.** Metadata survives some tools and not others, and a single pipeline change silently converts a compliant build into a non-compliant one. **The manifest is the record; image metadata is a convenience copy.** (`squoosh`, `oxipng`, and `optipng` defaults were untested.)

**C2PA / Content Credentials** is the emerging provenance layer; treat it as additive, not as a rights record.

### The clearance manifest, and what CI can actually check

A permissions manifest is the source of truth. Per asset it should carry: the asset identifier and hash, the rightsholder, the licence or permission reference, the **scope granted** (territory, format, print run, edition, language, media, duration), the credit-line text **as an opaque string to be reproduced verbatim**, an expiry or renewal date if any, and the diligent-search record for an orphan.

**Checks that are mechanically possible, and rare in practice:**

- Every asset in the build appears in the manifest, and every manifest entry resolves to an asset in the build. **Both directions** -- an orphaned manifest entry means an asset was removed and the credit still prints.
- No asset marked editorial-use-only appears in a commercial build.
- **Licence compatibility across the document**: every included asset's licence is at least as permissive as the licence the document grants.
- Every asset requiring a credit has its credit string rendered somewhere in the artifact.
- Licence texts match their canonical form byte-for-byte.
- The copyright year matches the edition's declared publication year and is not derived from the clock.
- Scope covers the actual print run and territory of this build.

---

## 7. Author agreements, and who carries the risk

- **Assignment vs exclusive licence vs licence-to-publish** determine what the author retains. Read which one it is before anything else.
- **The warranty and indemnity clause is the one that matters most to an author**, and it is why permissions discipline is not optional: it shifts the financial consequence of a clearance failure onto the author personally. **This is the mechanism that makes a clearance manifest an author's self-interest, not a publisher's paperwork.**
- Grant scope: territory, language, format, subsidiary, electronic, audio -- and increasingly **AI/TDM clauses**, which are new enough that their drafting is unsettled.
- **Work for hire (17 U.S.C. 101) requires one of nine enumerated categories PLUS a signed writing.** **Photographs and software are in none of the nine.** Absent that, **CCNV v. Reid** governs and **the contractor owns their work** -- so "we paid the freelancer, so we own it" is wrong, and it is one of the highest-frequency errors in this domain.
- **Moral rights** diverge sharply: Berne Art. 6bis; **VARA's US scope is narrow (visual art only, 17 U.S.C. 106A)**; and moral rights are **waivable in the UK but not waivable in advance in France or Germany.** **A translation or a heavy edit is the classic integrity claim**, which makes this directly relevant to editorial pipelines.

---

## 8. Authorship, credit, and integrity

- **ORCID** for identity; the **CRediT** taxonomy's 14 roles for contribution; **ICMJE's four criteria** for authorship.
- **AI cannot be an author.** ICMJE criterion 4 requires accountability, which a non-person cannot provide; and in the US a wholly AI-generated work is not copyrightable and must be disclaimed on registration (Thaler). Disclosure of AI assistance is increasingly required in methods or acknowledgements -- **publisher policies here are high-volatility and must be checked, not recalled.**
- **Plagiarism and copyright infringement are distinct failures with distinct remedies.** **Attribution cures plagiarism, not infringement.** And **text recycling may be neither** -- if the author holds the rights, reusing their own text is not infringement at all; it may still be an integrity question, and the Text Recycling Research Project's four categories are the right frame. Methods sections are the legitimate case.

*Retraction and correction as process, venue policy, and research-integrity workflow route to `scholarly-publishing`.*

---

## 9. Schools of thought (live, unreconciled)

### Fair-use assertion versus permissions culture

**The assertion camp** (Peter Jaszi and Patricia Aufderheide's Codes of Best Practices, the Association of Research Libraries, Authors Alliance): fair use is a **right, not a loophole**, and it atrophies when unused. Decades of reflexive permission-seeking have produced a "permissions culture" in which rightsholders are asked for -- and paid for -- uses the law already allows, which chills scholarship, makes books about visual culture unaffordable to write, and effectively privatizes the public domain. Community best-practice codes exist precisely to give practitioners defensible norms without a lawyer per quotation.

**The risk-aversion camp** (trade and academic publishers' permissions departments): the counter is not ignorance of fair use, it is **who bears the cost of being right**. A publisher's exposure is aggregated across a list, litigation costs money even when you win, the author's indemnity makes the author's own assets the backstop, and a librarian's fair-use confidence does not indemnify anyone. Conservatism is **rational given the loss function**, not a misunderstanding of doctrine.

**Unreconciled, and the disagreement is about risk allocation rather than about law.** Both camps agree on what §107 says.

### AI training

**For**: training is non-expressive statistical learning, the outputs are not copies, and the UK Getty holding that weights contain no copies supports the mechanism. Requiring licences for training would make the technology available only to those who can afford a corpus.

**Against**: the corpus was acquired without permission and often from piracy (Bartz's split matters here), market dilution harms authors even absent substitutable outputs (Chhabria's theory), and a licensing market exists and is being bypassed (the ROSS reasoning).

**No appellate authority. This is the counsel boundary.**

### Is CC BY-NC meaningfully open?

**No** (Creative Commons' own Open Definition alignment, Open Knowledge Foundation): NC blocks commercial reuse, which excludes translation businesses, textbook incorporation, and much downstream infrastructure -- and "commercial" is so ambiguous that risk-averse reusers treat NC as "do not touch."

**Yes, and it is the author's call**: many authors will share only if excluded from someone else's profit, and NC gets material released that would otherwise stay closed. Purism about the open definition costs real availability.

### Is registration worth it for a trade author?

**Yes**: timely registration is the gate to **statutory damages and attorney's fees** (17 U.S.C. 504(c) -- $750 to $30,000 ordinary, up to $150,000 wilful, $200 innocent floor). Without it you are limited to actual damages, which for a book are usually too small to fund a suit at all. Registration is what makes the right practically enforceable.

**Not necessarily**: for most trade titles the realistic infringement scenario is piracy by judgment-proof actors, and the enforcement path is DMCA notices, not litigation. The fee and the administrative overhead per work buy an option most authors will never exercise.

---

## 10. The counsel boundary (this section binds all others)

**Report mechanism. Never render a verdict.** Specifically, do **not** conclude on:

- whether a specific use is fair use, or falls within a specific UK/EU exception
- whether a specific work is in the public domain (especially 1923-1977 US works, or anything outside the US)
- whether a contract clause is acceptable, or what an author's indemnity exposure amounts to
- whether a specific asset needs clearance
- whether a redrawn figure is a derivative work
- whether an AI use is permitted by a specific licence or contract
- anything requiring the application of law to specific facts, in a specific jurisdiction, for a specific party

**What to do confidently instead:**

- Name the statute, case, or licence clause, and the mechanism each supplies.
- Name the factors the decision turns on, and what evidence would move each.
- Identify what is unresolved and say so plainly.
- **Correct factual errors about what a rule says** -- the 300-word myth, §105's scope, "whichever is shorter", Warhol's actual holding, SPDX-as-attribution, editorial-only in commercial books, contractor ownership, ImageMagick's real default. **This is squarely in scope and should be assertive.**
- **Design the pipeline, the manifest, and the CI checks. This is engineering work, and the agent should be assertive here.**
- **Tell the user precisely what to ask counsel** -- often the highest-value output. A well-framed question with the facts assembled and the mechanism identified costs far less billable time than "is this OK?"

**Standard phrasing, rather than vague hedging**: "The mechanism is X. Whether X applies to your facts is a call for counsel. Here is what counsel will need from you."

---

## 11. Anti-pattern catalog

| Pattern | Trigger | Consequence | Fix |
|---|---|---|---|
| `date +%Y` in the notice | Template convenience | Non-reproducible build; a reprint asserts a false first-publication year | `copyright_year` is a checked-in data field |
| `© 2019-2026` range | Seems tidier | Unsupported form; the Compendium specifies a comma list | `© 2019, 2026` |
| SPDX id as the attribution | Scanner output pasted in | Omits the required copyright line and permission text | Per-dependency copyright line + verbatim licence text |
| Licence text re-wrapped or smart-quoted | Markdown pipeline | Departs from required text; weakens the conspicuous disclaimer | Opaque blob, preformatted, golden-file tested |
| `LICENSE` read, `NOTICE` ignored | Scanner default | Silently non-compliant Apache-2.0 distribution | Aggregate upstream NOTICE content transitively |
| One notice file, many artifacts | Single build target | Ebook, print, and sample repo ship bare | The obligation follows each artifact |
| Scanner as the sole attribution input | Automation instinct | Fonts, images, icons, datasets, snippets all missing, unfalsifiably | Scanner + hand-maintained manifest, cross-checked |
| Image metadata as the rights record | It is already there | One pipeline change strips it and compliance vanishes | Manifest is the record |
| Editorial-only asset in a commercial book | Stock search convenience | No model or property release; the most common image failure | Manifest flag + CI check |
| CC BY-NC figure in a CC BY article | Funder mandate plus reuse | The document grants rights it does not hold | Compatibility check across included assets |
| Restyling an ND-licensed figure | House design pass | Derivative, which ND prohibits | Check the regime per asset; STM permits, ND does not |
| Desktop font licence for an ebook | "We licensed the font" | Embedding is a separate tier | Per-tier licence check |
| Vendoring a commercial font for CI | Reproducibility | Redistribution, usually prohibited | Name the collision; no clean technical answer |
| Quoting the 300-word rule | Received wisdom | No statutory basis; and ~300 words lost in Harper & Row | Publisher policy, not law |
| "Transformative therefore fair use" | Pre-2023 framing | Warhol: new expression alone is insufficient | Purpose measured against substitution |
| "Government work therefore public domain" | §105 half-remembered | Federal only, excludes contractors; state, local, and Crown are copyrighted | Check the actual source |
| "We paid them so we own it" | Contractor invoice | CCNV v. Reid: the contractor owns it | Signed assignment |
| Fair use reasoning for a UK/EU book | US-default habit | Enumerated exceptions, mandatory attribution, proportionality cap | Name the jurisdiction first |
| Orphaned manifest entry | Asset removed, manifest not | A credit prints for an absent asset | Check both directions |

---

## 12. Authorities

**Primary**: **17 U.S.C.** at `uscode.house.gov` (§§ 101, 105, 106A, 107, 401, 402, 407, 408, 504(c)). **Berne** and **CDPA 1988** via WIPO Lex and `legislation.gov.uk`. **DSM 2019/790** and **InfoSoc 2001/29** via EUR-Lex.

**US Copyright Office**: the **Compendium of Practices** (§2205 on notice) is the most useful and least-read document here. Circulars -- **Circular 1** (basics), **Circular 3** (notice), **Circular 30** (works made for hire; **not** Circular 9, a common miscitation). **There is no fair-use circular** -- the **Fair Use Index** replaces it.

**Working tools**: the **Cornell public-domain chart** (the canonical term calculator). The **STM Permissions Guidelines (2024)** for countable gratis limits. **Creative Commons** for licence text and the TASL requirement. **IPTC Photo Metadata Standard** and **PLUS** for rights metadata vocabulary. **Authors Alliance** guides for author-side practice.

**Commentary, with positions**: **Peter Jaszi** and **Patricia Aufderheide** (Codes of Best Practices; the permissions-culture critique). **Kenneth Crews** (library and educational practice). **William Patry** (treatise). **Jane Ginsburg** (comparative and international). **Pamela Samuelson** (software and AI). **Lawrence Lessig** and **James Boyle** (the public domain and enclosure).

---

## 13. Severity rubric (domain-specific)

- **blocker** -- an artifact that asserts rights it does not have or omits a required notice: an editorial-use-only asset in a commercial build, a CC BY grant over an incompatible included figure, a missing Apache NOTICE aggregation, an SPDX-only attribution list where notice text is required, an embedded font with no embedding licence, licence text materially altered. Also **a notice asserting a false fact**: a build-time copyright year on a reprint.
- **major** -- a real exposure or a systematic gap: a scanner-only attribution list for a publication, image metadata as the rights record, no clearance manifest where third-party assets are used, a manifest with no CI check in either direction, fair-use reasoning applied to a UK/EU publication, a contractor-authored asset with no signed assignment, missing per-figure third-party rights notes under STM terms.
- **minor** -- fragile or unconventional: a hyphenated year range, a stale hardcoded year on a title with no new edition, a credit line reworded rather than reproduced, a companion-URL-only notice surface.
- **nit** -- "All rights reserved" present or absent, notice placement conventions, ordering within a credits section.
- **insight** -- a structural reframing: the notice page should be data-plus-template rather than generated prose; the manifest should be the source of truth with the scanner as one input; this pipeline needs a slot for externally-supplied text; the indemnity clause means the author personally carries the clearance risk, which changes the cost-benefit of the whole permissions process.

**Confidence calibration**: statutory and licence-text mechanisms are checkable, so findings that a required element is *absent* should be high-confidence. **Findings that depend on applying law to facts are not findings** -- they are questions for counsel, and should be framed that way. **Never state an unresolved litigation question as settled**, and mark AI-training claims as unresolved every time.

---

## Changelog

- **2026-09-09** -- Initial file. Verified verbatim against statute: 17 U.S.C. 401(b), 402(b) (**℗ alone** for phonorecords), 401(d)/402(d), 504(c), 105 (all three subsections); CDPA ss.29, 29A, 30, 163; DSM Arts. 3, 4, 7, 14; Berne 6bis, 9(2), 10. Six corrections to received framing established this pass: **CDPA s.30(1ZA) quotation is purpose-agnostic** (the UK/US delta is mandatory attribution plus a proportionality cap, not purpose closure); **DSM Art. 7(1) omits Art. 4**, so the commercial TDM exception can be contracted away while the research one cannot; **ImageMagick preserves XMP by default** (tested empirically -- `cwebp`, `jpegtran`, `sharp` strip); multi-year notices take a **comma list, not a range** (Compendium §2205.1(A)); **Crown copyright is one rule with two branches** plus the 2039 cliff; work-for-hire is **Circular 30**, and there is no fair-use circular. Recorded as unresolved and requiring docket checks: Bartz final approval, ROSS Third Circuit posture, NYT summary judgment, Getty US posture. Noted that **human authorship is near-settled (Thaler, cert denied 2026-03-02)** and is routinely conflated with the unsettled training question. The 300-word myth traced to both of its origins. Residual gaps (unread opinions, COPE current guidance, publisher AI policies, stock ToS) listed in the research notes.
