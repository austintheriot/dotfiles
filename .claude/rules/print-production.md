---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-09
---

# Print production and the publication artifact

A reference for advising on and reviewing the path from "the manuscript is final" to "the object exists and is distributable". Used by the `print-production` subagent and the `/expert-review` / `/expert-plan` / `/expert-consult` / `/consult` skills.

**Source research**: `~/.claude/local/research-notes/print-production-research.md`

The unifying thesis: **a print-ready file is a manufacturing input, not a document.** It is consumed by a machine you do not own, operated by people you will never speak to, and the feedback loop is a physical object that arrives weeks later. Almost nothing in this domain fails loudly. The file builds, the preflight passes, the proof looks fine on a 300 dpi laser print, and the defect is discovered in a box of 500 copies.

The agent's operational question: **"will the printer accept this file, and will the physical object be right when it comes off the machine?"**

The empirical priority, in rough order of how often it bites:

1. **Geometry** -- bleed, boxes, gutter, and the asymmetry of an interior page. Wrong geometry shifts every page and no tool complains.
2. **Cross-artifact staleness** -- the cover encodes a spine width derived from an interior page count that has since changed. Both artifacts pass their own checks; the defect is physical.
3. **Vendor-specific requirements** -- the same PDF is accepted by one printer and rejected by another, on documented and mutually contradictory rules.
4. **Notices and the verso page** -- claims about the physical object (paper permanence, country of origin, certification, impression number) that are false for the object actually produced.
5. **Colour and ink** -- registration black in text, exceeded ink limits, double conversion.
6. **Fonts** -- silent RIP substitution, which repaginates against the approved proof.

**The meta-rule**: in this domain the build succeeding is not evidence of anything. Look for the claim the file makes about a physical object that nobody verified.

---

## Boundary with the `pdf` agent

`pdf` owns the file **as a data structure**: object model, xref, incremental update, linearization, `/ToUnicode`, Type 0 / CID mechanics, signature `/ByteRange`, the tag tree, PDF/A conformance mechanics, veraPDF.

`print-production` owns the file **as a manufacturing input**: will the printer accept it, and what does the object look like off the machine.

The shared surface, explicitly:

| Surface | `pdf` owns | `print-production` owns |
|---|---|---|
| PDF/X | what the standard requires structurally (which ISO part, which keys) | which X flavour a given printer demands, and what gets rejected in practice |
| Page boxes | `/MediaBox` inheritance and defaults, units | which box the RIP reads, and what happens when TrimBox is absent |
| Font embedding | subsetting mechanics, `/FontFile` semantics, the `fsType` bits | the licence-forbids-embedding problem and RIP substitution symptoms |
| Tagged PDF | the structure tree itself | a print-first pipeline yielding an untagged artifact with an unmet accessibility obligation |

**The decision rule**: "is this key legal per ISO 15930?" routes to `pdf`. "Will IngramSpark bounce this?" is ours.

---

## Volatile surface

`last-verified` is in the frontmatter -- do not restate it in prose. These rot; the rest of this file is comparatively durable.

| Claim class | Rots | Re-verify at |
|---|---|---|
| POD vendor requirements, trim lists, page floors | **Fast (months)** | Each vendor's own spec document -- IngramSpark file-creation-guide.pdf, KDP help, Lulu, Blurb |
| Vendor numeric constants (bleed, TAC, spine, margins) | Fast | **The vendor's generated template, not its prose** |
| Typst / Paged.js / WeasyPrint capability | **Fast** | `gh api` on each repo; each project's own docs |
| EAA enforcement and harmonised-standard state | **Fast, and legally consequential** | Official Journal of the EU; W3C Publishing Maintenance WG |
| KDP AI-disclosure policy | Fast | `kdp.amazon.com/en_US/help/topic/G200672390` |
| Pantone / Adobe licensing | Medium | Pantone and Adobe announcements |
| ICC standard profile versions (GRACoL, FOGRA) | Slow | Idealliance, Fogra, ECI |
| PDF/X part numbering, ISO 12647, Z39.48 | **No** -- the standards layer is stable | ISO TC 130, NISO |
| Physical conventions (signatures, recto/verso, front-matter order) | **No** | Chicago Manual, Williamson |

---

## 1. Page geometry (where the silent failures live)

### The five boxes, and which one the printer reads

- **`/MediaBox`** -- required, the physical medium. Everything else defaults to it.
- **`/CropBox`** -- default view/clip region. **A viewer concern, not a print concern.** A common author error is setting CropBox to trim size and assuming the printer honors it; prepress reads TrimBox/BleedBox and ignores CropBox.
- **`/BleedBox`** -- the region content may bleed into, plus printer slack.
- **`/TrimBox`** -- the intended finished size after trimming. **This is the one the printer's imposition software reads.** PDF/X requires TrimBox or ArtBox on every page and forbids both.
- **`/ArtBox`** -- meaningful-content extent. Rare in book work.

Invariant a preflight will flag: **TrimBox ⊆ BleedBox ⊆ MediaBox.** Violations (TrimBox larger than MediaBox, BleedBox equal to TrimBox on a file claiming bleed) are classic generated-PDF bugs.

**Two non-obvious facts that make this the highest-yield check in a pipeline review:**

1. **Most source-to-PDF engines do not set TrimBox by default.** pdfTeX writes only MediaBox unless you use `pdfx` or `crop`. WeasyPrint and headless Chrome historically wrote MediaBox only. **So the default artifact from a text pipeline is structurally non-PDF/X-conformant even when its geometry is right.**
2. **POD services do not want bleed boxes or crop marks at all** -- they want the page *sized* to trim+bleed with no marks. **"Correct PDF/X handoff" and "correct KDP/Ingram handoff" are different targets**, and a pipeline built for one fails the other.

### Bleed, and the asymmetry nobody expresses

Standard bleed is **0.125 in / 3 mm**. Cover bleeds on all four sides.

**The interior bleeds on three edges only -- top, bottom, outside. Never the bind/gutter edge.** IngramSpark states it plainly: adding bleed to the bind edge "will cause incorrect positioning."

**The consequence is the single most common generated-interior geometry bug**: a CSS or LaTeX pipeline setting symmetric 3 mm bleed produces a 6x9 page 6.25 in wide instead of 6.125 in, and **every page shifts toward the gutter.** Worse, it means **the interior page box is asymmetric and mirrors recto/verso** -- a recto's extra 0.125 in is on the right, a verso's on the left. A pipeline emitting one page size for all pages is wrong. This is genuinely hard to express in CSS Paged Media, and in LaTeX needs `twoside` with explicit odd/even setup.

Worked: a 6x9 interior with bleed is **6.125 in wide x 9.25 in tall**.

**All-or-nothing rule** (KDP): if even one interior page needs bleed, the entire interior must be prepared with bleed dimensions.

### Safety margins, and the variance that eats them

**IngramSpark allows a 1/16 in (0.0625 in / 2 mm) variance on all books printed.** This is why a 0.25 in safety margin is really 0.1875 in worst case: a folio set 0.25 in from trim can land 0.1875 in from trim, and a hairline rule 0.125 in inside trim **can be cut off**.

Verified Ingram figures worth knowing:

- Interior margin: minimum **0.5 in (13 mm)** all sides, with headers, folios, body text, and non-bleed images inside it.
- Cover type safety: **0.25 in** recommended, templates allow down to 0.125 in.
- Spine type safety: **0.0625 in (2 mm)** each side for spines ≥ 0.35 in; **0.03125 in (1 mm)** below.
- **No spine text at all for perfect bound below 48 pages.** A cover generator that always draws spine text produces an unprintable cover for a short book -- **and the failure is a human rejection, not a preflight error.**
- An additional **0.125 in (3 mm) white strip inside the trim on the bind side** for perfect-bound and hardcover colour interiors, *on top of* the 0.5 in margin. This is not a margin; it is an anti-glue and anti-gutter-shadow allowance, and it is easy to miss.

### Gutter grows with page count, which creates a fixpoint problem

KDP's gutter table:

| Page count | Gutter (inside) |
|---|---|
| 24-150 | 0.375 in |
| 151-300 | 0.5 in |
| 301-500 | 0.625 in |
| 501-700 | 0.75 in |
| 701-828 | 0.875 in |

A thicker perfect-bound block loses more inner margin to spine curl and glue, so required gutter grows monotonically. Coil and wire-o need the most because the punch physically removes paper.

**The pipeline landmine**: gutter is a function of page count, and page count is a function of typeset output. Set gutter, typeset, cross a band boundary at 150 to 151, gutter grows, re-typeset, page count changes again. **This is a fixpoint iteration and it can oscillate.** Practical resolution: pick the gutter for the worst-case band you might land in, typeset once, accept a slightly generous gutter. **A build that recomputes gutter from the previous run's page count is non-deterministic across a clean checkout.**

### Spine width, and the three incompatible caliper units

`spine = (page_count / 2) x caliper_per_sheet`, plus board allowance for hardcover. Caliper is expressed three ways, and this is where arithmetic goes wrong:

1. **PPI (pages per inch)**, US book-paper convention: `spine_in = page_count / PPI`. **PPI already counts pages, not sheets -- there is no /2.** 300 pages on 400 PPI paper is a 0.75 in spine.
2. **Caliper in thousandths of an inch per sheet**: `spine_in = (page_count / 2) x mils / 1000`. **"Point" here means 0.001 in, not 1/72 in.** Same word, two meanings, off by roughly 14x.
3. **gsm + bulk factor**, European: `spine_mm = (page_count / 2) x gsm x bulk / 1000`. **gsm alone does not determine thickness** -- two 80 gsm papers can differ 20% in caliper. **Deriving spine width from gsm without a vendor bulk figure is the most common spine miscalculation.**

**The load-bearing rule: do not compute spine width yourself for a POD service.** Use the vendor's template generator, which encodes the vendor's actual stock. Two further wrinkles: Ingram requires templates and files built on a **mod-2 spine calculation**, and Ingram **reserves the right to change your paper stock** (and therefore your spine) for titles with heavy one-sided ink coverage. **So even a correctly computed spine can be superseded by the printer.**

### Cover-wrap geometry

**Perfect bound** (one flat sheet):
`bleed_width = 0.125 + trim_w + spine + trim_w + 0.125`; `bleed_height = 0.125 + trim_h + 0.125`.

**Casebound** (verified Ingram formulas):
- Board width = **trim_width − 0.185 in**; board height = **trim_height + 0.25 in**.
- `bleed_width = 0.625 + board_w + 0.5 + spine + 0.5 + board_w + 0.625`
- `bleed_height = 0.625 + board_h + 0.625`
- The 0.625 in is the turn-in that folds inside. The 0.5 in each side of the spine is the **hinge**, which is pliable and takes an indentation -- keep artwork out of it.
- **Note the sign: the board is narrower than trim but taller than trim.** Getting this backwards is a classic case-cover bug and shows as wrong squares at head and foot.

**Dust jacket**: cover width = trim_w + 0.4375 in, height = trim_h + 0.25 in; flaps 3.25 in with a 0.25 in fold allowance.

### Recto, verso, and the blank page

**Recto = odd = right-hand. Page 1 is always a recto.** A pipeline starting numbering on an even page has an off-by-one that mirrors the whole book into the wrong gutter.

Chapters conventionally open on a recto in trade work, which means **inserting a blank verso** when the previous chapter ends on a recto. Either convention (recto-only or any-page) is defensible, must be *consistent*, and **changes page count, which changes spine width.** LaTeX expresses this as `openright` vs `openany`.

**Inserted blanks must be truly blank** -- no running head, no folio. LaTeX's `\cleardoublepage` still gets a running head under some page styles; the `emptypage` package fixes it. This is a real defect that reaches print.

**The POD last-page rule** (Ingram): all text files are stored with a page count divisible by two, **the last page is left blank for the printer's manufacturing mark**, and Ingram will add pages if needed. So **the last page of an Ingram book carries a mark you did not put there**, and ending on a content page gets you silently padded -- changing page count and therefore spine width. Design in a deliberate blank last leaf.

---

## 2. Imposition and binding

### Signature arithmetic, and why POD is different

Offset folds press sheets into **signatures** of 32, 16, or 8 pages, so page counts want to be a signature multiple and at minimum a multiple of 4. Unused pages in the last signature are paper you paid for.

**POD is mod 2, not mod 16.** Ingram processes text files to a count divisible by two, printing in four- or six-page single-sheet signatures depending on trim. KDP is likewise not signature-constrained. **"Always pad to a multiple of 16" is offset advice misapplied to POD**, where it adds blank leaves and spine thickness for nothing. Which rule applies is a **procurement fact, not a typesetting fact**, and a pipeline must be told it.

### Binding constraints

- **Saddle stitch**: page count divisible by **4**, bindery minimum 8. The practical ceiling is set by creep and the stapler; published limits vary (48 to 92) so **ask the printer rather than asserting a number**.
- **Perfect binding**: Ingram's floor is **18 pages** (and no spine text below 48); KDP's is **24**. Maxima run 900-1200 pages depending on stock. Perfect-bound books do not lie flat, which is the actual reason for the growing gutter table.
- **Case binding** gets **squares** (the case overhangs the block), hence board height = trim + 0.25 in.
- **Coil / wire-o** lies flat, needs the largest bind-edge margin because the punch removes paper, and has no spine to print.
- **Layflat / Otabind** frees the spine from the cover so spreads open flat -- relevant when the design has cross-gutter images.

### Creep, and why you must not pre-compensate

In saddle stitch, nested signatures make inner leaves project further, so after trimming the inner pages lose more outer margin. Compensation ("shingling") shifts each page toward the spine by a per-position amount, applied **at imposition time**.

**Creep compensation is the printer's job.** A submitted single-page sequential PDF should **not** have creep baked in -- **if you pre-compensate and the printer also compensates, you double-shift.**

### Why you must usually not impose

Imposition depends on the press, sheet size, folder, and bindery -- facts the author does not have. **Submit single-page, sequential, reader-order PDF.** Ingram: files in spread format "will be rejected for a corrected submission."

**Do not include crop, printer, or registration marks.** Ingram: "Marks included in a file could show up in printed copies." **The symptom is that the marks print inside the trim**, because the vendor sizes the page to trim+bleed and your marks sit inside that.

Three distinct things people confuse:

- **Single pages, sequential** -- what you submit.
- **Reader spreads** (2|3, 4|5) -- what a designer exports for review. Submitting these makes every page double-width; the printer rejects it or trims your book in half.
- **Printer spreads** (last|first, ...) -- what the printer produces internally. Submitting these means the printer imposes an already-imposed file and the order becomes gibberish.

**The generated-pipeline version of this bug**: InDesign's "Spreads" checkbox, a two-page-view CSS rule leaking into the print stylesheet, or `pdfjam --nup 2x1` left in a Makefile from a proofing target.

---

## 3. Colour and ink

### Ink limits, and the retroactive enforcement

**Total Area Coverage (TAC)** is the sum of the four channels at a point. Offset runs 300-320%; **Ingram's ceiling is 240%**.

**The non-obvious part is that Ingram enforces it retroactively**: "Files with densities greater than 240% may process and print without rejection. If these files ... encounter print issues in future orders, LS will require a corrected file." **A file that built and printed can become non-compliant later.**

Exceeding TAC produces: ink that does not dry, set-off onto the facing page, blocked or picked sheets, and crushed shadow detail.

### Rich black, registration black, 100K

- **100K** (`K=100` alone) is correct for body text. Solid, single-plate, no registration risk.
- **Rich black** (e.g. 60/40/40/100) is for large areas that would otherwise look washed. **Do not use it under small text** -- any misregistration produces a coloured halo on the letterforms.
- **Registration black** (100/100/100/100) is a *press mark* colour, never a content colour. In text it produces coloured fringes, filled counters, muddy unreadable type, and set-off. **Trigger**: a designer picks the `[Registration]` swatch, or a converted logo brings it in.
- **A 100K box over a rich-black field** shows as a visible lighter rectangle. Mixing DeviceGray text with CMYK backgrounds is the trigger.

Vendor rich-black recipes **conflict**: Ingram specifies 60/40/40/100 (240%, exactly its own ceiling) while Blurb specifies 60/50/50/100 (260%, which **exceeds** Ingram's ceiling). **A single hardcoded `RICH_BLACK` constant is wrong somewhere.**

### The Ingram ICC contradiction, worth knowing because it looks like a mistake

Ingram says **"do not include Spot colors or ICC profiles"** in a black-and-white interior, because a profile applied to 100% K text converts it to a *percentage of gray* -- which prints washed out rather than solid. **But Ingram also mandates PDF/X-1a, which requires an output intent, which is an ICC profile.** The vendor's own requirements are in tension. Follow the specific instruction for the specific product and do not try to satisfy both abstractly.

### Conversion, and where it should happen

**Double conversion** (the application converts on export *and* the RIP converts again) produces muddy blacks, lost density, and saturated colours drifting grey. Convert once, deliberately, and know where.

Standard profiles a printer may ask for: **GRACoL 2013 / CRPC6** and **US Web Coated SWOP** (US), **Coated FOGRA39 / FOGRA51** (Europe), **Japan Color**. Which one is a fact about the press, not a preference.

---

## 4. Images, fonts, and the silent-substitution class

### Resolution

300 ppi **effective at final size** is the working floor; line art wants 600-1200; a 1-bit pre-screened bitmap is a guaranteed moiré source.

**Effective versus native is the trap**: a 300 ppi image scaled to 200% at placement is 150 ppi effective, and the source file still reports "300 dpi". Export presets are the other trigger -- a screen-targeted preset silently downsamples 600 ppi line art to 150.

**Barcodes**: never rasterize one (bar-width ratios are lost and no preflight catches it), and **never resize the vendor's supplied barcode**. Ingram will drop a generic barcode onto your artwork if you reserve no space for one, and **"the publisher may not be notified."**

### Fonts, and why substitution is worse than it sounds

- **The standard 14 must be embedded** for print. Acrobat's "Standard" preset, reportlab's default, and wkhtmltopdf's fallback all fail this.
- **Silent RIP substitution is the dangerous failure**: an `fsType`-restricted, corrupt, or unembedded font gets replaced with a metrically different face, so **line and page breaks differ from the approved proof** -- which changes page count, which invalidates the spine width, which feeds the stale-cover bug. Or everything becomes Courier.
- **A variable font embedded with a live axis** renders at the RIP's default instance: Light becomes Regular, Condensed becomes Regular.
- **Missing glyphs are silent** unless you ask. LaTeX needs `\tracinglostchars=3`; a browser pipeline silently font-falls-back, giving you one em dash from a different family at a different weight.
- **The `fsType` bits are not the licence.** They express the foundry's technical intent; the EULA governs. And **vendoring a commercial font into a repo for reproducibility is redistribution** -- usually prohibited. Route the licence question to `copyright-and-permissions`.
- **Hairlines**: CSS `0.5px`, SVG `stroke-width="0"`, or Illustrator's "hairline" produce rules that break up or vanish in print while looking fine on a 300 dpi laser proof.

---

## 5. The publication apparatus (the "print notices")

### Front matter, in order

| Page | Element | Note |
|---|---|---|
| i (recto) | Half title | Title only -- no author, no subtitle |
| ii (verso) | Frontispiece, series title, or "Also by" | |
| iii (recto) | **Title page** | Title, subtitle, author, translator, imprint |
| iv (verso) | **Copyright page** | The notices page -- see below |
| v | Dedication | |
| vi | Epigraph | |
| vii (recto) | Table of contents | |
| -- | Lists of illustrations / tables | |
| -- | Foreword | **By someone other than the author** |
| -- | Preface | **By the author**, about the making of the book |
| -- | Acknowledgements | Or in back matter |
| -- | Introduction | **Part of the work itself** if substantive |

The foreword/preface/introduction distinction is the one people get wrong. Mnemonic: **fore**word comes *fore* of your voice (someone else); **pre**face is you, before the book; **intro**duction is part of the book.

Roman numerals for front matter, arabic restarting at 1 for the body, with blind folios on the first few leaves. **LaTeX's `\frontmatter` / `\mainmatter` / `\backmatter` encodes this convention exactly, including the numeral switch and recto opening -- CSS Paged Media does not.**

### Back matter, in order

Appendices → Notes → Glossary → Bibliography → **Index** → Contributors → Colophon.

**The index must come last of the reference apparatus because it points at final page numbers**, so it can only be built after final pagination. In a pipeline this is a hard ordering constraint.

The **colophon** traditionally notes typefaces and their designers, paper, printer, binder, and edition size. Typeface credit is a courtesy, though **some font EULAs request or require it** -- check the licence.

### The copyright page, item by item

1. **Copyright notice** -- `Copyright © 2026 by <name>`. Not required for protection in the US post-1989, but cheap, and it forecloses an innocent-infringement mitigation. (Mechanism and statute route to `copyright-and-permissions`.)
2. **"All rights reserved"** -- a Buenos Aires Convention artifact, no longer legally required, universally retained.
3. **Publisher imprint and address** -- for a self-publisher, the invented imprint name plus a real mailing address. Required in some jurisdictions.
4. **Reproduction-reservation boilerplate**, increasingly carrying a **text-and-data-mining / AI-training reservation**. Whether such a reservation is effective is unsettled; in the EU, DSM Article 4(3) makes a machine-readable opt-out meaningful, which gives it some weight for the digital edition. **The presence of the clause is now standard; the legal effect is counsel's question.**
5. **Edition and printing statement.**
6. **The printing number line** (`10 9 8 7 6 5 4 3 2 1`). **The lowest surviving digit is the impression number** -- the printer deletes digits from the plate on each new printing. Variants interleave (`1 3 5 7 9 10 8 6 4 2`) or append year digits. **Random House historically indicated a first printing with a line starting at 2**, so "lowest digit = impression" is wrong for that imprint. **Pipeline consequence: the number line is a field, not decoration.** Hardcode it and the second printing is mislabelled forever. **On a POD title there are no meaningful impressions, so printing the line is a small lie.**
7. **ISBN(s)** -- one per format.
8. **Library of Congress data**, and the eligibility rule that catches people:
   - A **CIP data block** is a full pre-publication catalogue record printed on the title-page verso.
   - **Eligibility is narrow.** A publisher must have already published **three titles by three different authors, each acquired by at least 1,000 US libraries**. Book vendors, distributors, printers, production houses, and **fee-for-service publishers are ineligible**, and **self-publishers are explicitly ineligible**.
   - **So a self-published book cannot have a CIP block.** A self-published title printing a CIP-looking block has either a fake or a commercial "publisher's CIP" from a cataloguer -- legitimate, but it must not be presented as LoC data.
   - **PCN / LCCN** is the self-publisher's route: a bare control number, requiring a **US place of publication printed in the book**.
   - **Both must be applied for before publication**, and CIP requires uploading a galley.
   - **The sharp pipeline consequence**: the CIP/PCN block is text you receive from an external agency *after* the galley is final and *before* you print. **It cannot be generated from source.** The build needs a slot for externally-supplied verso content, and **if you re-typeset after submitting the galley, the CIP record's collation statement is wrong.**
9. **British Library CIP** -- the one-line "A CIP catalogue record for this book is available from the British Library" is the normal UK form. A convention, not a statute.
10. **"Printed in ..."** -- both a trade convention and, for imported goods, a **country-of-origin marking obligation under 19 U.S.C. 1304**. **The POD nuance is sharp**: an Ingram title prints at whichever global plant is nearest the customer, so "Printed in the United States of America" is *false* for the copy shipped from the Australian plant. Omit it or use distribution-neutral language.
11. **Paper permanence** -- ANSI/NISO **Z39.48-1992 (R2009)**, marked with the **infinity symbol in a circle**; ISO 9706 is the international equivalent. **You may only make the claim if the paper actually meets it -- and for POD you do not choose the paper and generally cannot know.** Printing a Z39.48 claim on a POD title is a false statement about the physical object.
12. **FSC / PEFC / SFI, and the POD trap.** Only a holder of a valid **Chain of Custody certificate** may apply an on-product label; a promotional licence explicitly does not permit product marks. **And Ingram prohibits these marks outright**: placement by a publisher "is prohibited", a mark carried over from a prior printing "must be removed by the publisher before the book is submitted", and if Ingram finds one it "will remove the certification claim **at the publisher's expense**." **Converting an offset title to Ingram POD requires stripping the FSC mark from the verso** -- a real migration task, easy to miss when the verso is a static asset.
13. **Disclaimers** -- the fiction disclaimer (defamation and false-light risk reduction; limited legal effect but its absence is worse), no-professional-advice clauses, warranty and liability disclaimers (near-universal in technical titles), and third-party-URL disclaimers.
14. **AI disclosure -- state it precisely.** **Amazon KDP requires disclosure to Amazon, not on the page**: publishers must inform KDP of **AI-generated** content (text, images, translations) at publish time, while **AI-assisted** content -- where you created it and used AI to edit, refine, or brainstorm -- **requires no disclosure at all.** This is a **metadata obligation, not a print notice.** **No major English-language retailer currently mandates an on-page AI notice.** Some publishers add one voluntarily. Treat any claim of a mandatory on-page notice as unverified and check the specific retailer.
15. **Permissions acknowledgements** -- on the copyright page if short, otherwise a separate Credits page with the copyright page carrying "Permissions acknowledgements appear on page NNN, which constitutes an extension of this copyright page." **That sentence is a real construction and it matters** -- it makes the later page part of the notice.
16. **Translation credit** -- the translation is a **separate copyrightable work with its own term and its own notice**.

### Verso requirements specific to self-published and POD titles

- No CIP block. PCN/LCCN instead, and only with a US place of publication printed.
- **Avoid the printing number line** -- no meaningful impressions.
- **Avoid Z39.48 paper claims** -- you do not control the paper.
- **Must strip FSC/PEFC/SFI** for Ingram.
- **Do not print "Printed in the United States of America"** on a globally-distributed POD title.
- Print a real imprint name and address if you want trade treatment. **A POD service's free ISBN puts the service in the publisher field of the distribution metadata**, which shows up in library catalogues, not just on your page.

### Legal deposit

- **US, 17 U.S.C. 407**: **two copies of the best edition** to the Copyright Office **within three months of publication**. Distinct from registration (§408) -- deposit is mandatory, registration optional. Failure is not loss of copyright; the sanction arrives only after a written demand from the Register, then a fine of **not more than $250 per work** plus the copies' retail cost.
- **UK, Legal Deposit Libraries Act 2003**: one copy to the **British Library within one month**, automatically. The other five deposit libraries receive copies **only on written request** via the ALDL, up to 12 months later. Applies to anything published in the UK or Ireland, and **self-publishers are publishers for this purpose** -- practitioners call it the rule almost everyone breaks.
- **France** is the notable non-obvious one: *dépôt légal* imposes a distinct obligation on the **printer** as well as the publisher, and France requires an **"achevé d'imprimer"** statement (month and year of printing completion plus printer name) in the book.

---

## 6. ISBN and distribution metadata

- **ISBN is per format and per edition.** A format change requires a new ISBN. Paperback, hardcover, EPUB, and audio are four ISBNs.
- **The free-ISBN trap**: a POD service's free ISBN names **the service** as publisher in the distribution metadata. That is a durable catalogue fact, not a cosmetic one.
- Allocation: Bowker (US), Nielsen (UK), national agencies elsewhere.
- **Barcode**: GTIN-13/EAN with the 5-digit price add-on. Quiet zones, minimum size, and placement matter; see the barcode notes in §4.
- **ONIX for Books** (EDItEUR) is the distribution metadata standard; **BISAC** (BISG) and **Thema** are the subject-code schemes. Metadata quality drives discoverability, and it is the half of publishing that source-based pipelines usually ignore entirely.

*ISBN syntax and check-digit validation route to `citation-and-bibliography`; allocation, placement, and distribution metadata are ours.*

---

## 7. The pipeline (print-ready output as a build artifact)

### The stale-cover failure class -- the highest-value pipeline finding

**The mechanism**: the interior repaginates (a font substituted, a toolchain bumped, content edited). The cover was generated earlier and encodes a spine width derived from the *old* page count. **Both artifacts pass their own checks.** The defect is physical: spine art offset by a couple of millimetres, the front fold landing mid-title, the barcode too near the fold. **It is found only on a physical proof, after money is spent.**

**The fix is a build-graph fix**: the cover must take the interior PDF as a dependency and read its page count, not a constant. **With one wrinkle** -- Ingram silently pads to mod 2, so **the number feeding the spine calculation is the vendor's post-padding count**, not your raw page count.

### Making print defects fail the build

Every failure in this domain defaults to a warning or to silence. Concrete gates that work:

- `pdffonts` -- assert every font is embedded and subsetted. Catches the standard-14 and substitution class.
- `pdfimages -list` -- assert effective resolution at final size. Catches downsampling and the scaled-image trap.
- `gs -o - -sDEVICE=inkcov` -- measure ink coverage. Catches TAC violations before the printer does.
- Assert TrimBox exists and that TrimBox ⊆ BleedBox ⊆ MediaBox, with the expected asymmetric interior geometry.
- Assert page count parity and the mod-2/mod-4 rule for the target vendor, and **feed that count into the cover build**.
- `\tracinglostchars=3` (LaTeX) -- promote missing glyphs from silence to a message.
- Grep for `Label(s) may have changed` and fail -- a single-pass build ships a stale index and stale cross-references.
- Fail on overfull hbox. With `-interaction=nonstopmode` LaTeX exits 0 while ink extends past the text block **in print**.

### Per-engine reality

- **LaTeX** -- `geometry` for the page, `pdfx` for PDF/X output intents, `crop` for marks, `microtype` for protrusion. Widow and orphan control via `\clubpenalty`/`\widowpenalty`; `\raggedbottom` vs `\flushbottom`. **Overfull hbox is a real print defect that is only a warning.** The book class encodes front/main/back matter convention properly.
- **Typst** -- **VOLATILE** (2026-09-09): `page(bleed:)` landed in **v0.15.0** with `inside`/`outside` keys, which **directly solves the asymmetric-bleed problem**, and it writes a TrimBox. **But PDF/X remains an open feature request.** So: correct geometry, no PDF/X handoff.
- **WeasyPrint** -- **its PDF/X-1a mode is a metadata claim, not conformance.** Verified from source: it sets `output_intent='device-cmyk'` but only ships `sRGB2014.icc`, so with no CMYK profile supplied **`/OutputIntents` is never written while `GTS_PDFXVersion` is.** The file lies about itself, with no warning. It also performs no RGB-to-CMYK conversion, which X-1a forbids. **This is worse than not supporting PDF/X.**
- **Paged.js** -- the free CSS-Paged-Media option, and it lacks footnotes, leaders, top/bottom floats, and bookmarks, and gets `counter-reset` wrong. It delegates PDF writing to Chrome, **which writes no TrimBox and no output intent**, so it cannot produce a prepress artifact without a post-pass. Classic bug: `@page { size }` set in CSS but Puppeteer's `pdf()` not given matching width and height, yielding a 6x9 content block in the corner of a Letter MediaBox.
- **Prince** -- the mature commercial engine; the CSS camp's best results run on it.
- **InDesign** -- **has no supported headless mode**, so it cannot be part of an automated pipeline in the sense this agent cares about.

### Reproducibility

- **Font availability differing between local and CI** is the root cause that feeds the stale-cover bug: substitution → reflow → repagination → wrong page count → wrong spine. Reference fonts by repo-local path, never by system font name. (But see the licence constraint in §4 -- vendoring a commercial font is redistribution.)
- **An unpinned toolchain repaginates.** TeX Live, Chrome, WeasyPrint, and Typst all change hyphenation and protrusion tables between versions. Pin the container digest.
- **Non-deterministic PDF output** (timestamps, `/ID`) breaks artifact comparison. Make it deterministic if you want to diff builds.
- Index and cross-reference generation **requires multiple passes**, and the pass requirement interacts with the ordering constraint in §5.

---

## 8. Accessibility in the same pipeline

**The EAA obligation took effect 2025-06-28** for ebooks sold in the EU. Two points stated carefully, both **VOLATILE** (2026-09-09):

- **No presumption of conformity currently exists under the EAA.** Article 15(1) grants it only to harmonised standards cited in the Official Journal, and **none has been published under Directive 2019/882.** EN 301 549 v3.2.1 is harmonised under the *Web Accessibility Directive*, not the EAA. **So "EN 301 549 conformant, therefore EAA compliant" is an invalid inference.** (Single secondary source -- verify, but the mechanism is checkable.)
- **EPUB Accessibility 1.1's own floor is WCAG 2.0 Level A**, weaker than the AA the EAA route implies. These are commonly conflated.

**Why a print-first pipeline produces an inaccessible EPUB**: no `<h1>` hierarchy (visual styling stood in for structure), captions pressed into service as alt text, folios and running heads read aloud, and hard line breaks inside paragraphs. The print artifact's structure is visual, and the conversion has nothing semantic to work from.

**Two false-claim failures worth flagging as findings**: `accessModeSufficient: textual` written by a template rather than derived from content is a **machine-readable false statement about accessibility**; and a Ghostscript re-distill to hit PDF/X-1a **destroys the tag tree**, making a PDF/UA claim false.

*Tag-tree structure routes to `pdf`; the general a11y catalogue routes to `accessibility`. The workflow failure -- a print pipeline yielding an artifact with an unmet obligation -- is ours.*

---

## 9. Schools of thought (live, unreconciled)

### PDF/X-1a versus PDF/X-4

**For X-1a**: it is *blind exchange* -- everything device-CMYK, transparency flattened, nothing left to interpretation. **Failure happens on your machine where you can see it**, rather than in a RIP you do not control. If you flatten yourself, you can look at the result. Every RIP since 2001 handles it, and most POD and many mid-size shops run non-APPE RIPs. **IngramSpark still requires PDF/X-1a:2001 or PDF/X-3:2002 as of its 2026-08-24 guide revision** -- a large share of the world's book production, in 2026, on a 2001 standard, and not by accident.

**For X-4**: flattening is lossy and irreversible, and it is **the cause** of the seam, rasterised-text, and spot-conversion defects catalogued above -- so mandating X-1a mandates the failure mode it claims to avoid. Live transparency lets a modern RIP render it once, correctly, at device resolution. X-4's ICC colour permits late binding, so one file retargets to press, digital, and proof. **Ghent Workgroup's current specifications are X-4-based.**

**The unresolved middle, and it is genuinely unresolved**: **GWG's own X-4-based specs restrict colour to DeviceCMYK, greyscale, and spot** -- taking X-4's live transparency and **rejecting its colour management.** So even the authority does not endorse the full X-4 proposition. That is not a compromise; it is an admission that the colour-managed half is the contested half.

**Practical stance**: ask the printer and encode their answer as a pipeline target. **Do not tell anyone X-1a is obsolete -- the largest POD vendor in the world requires it today.**

### Convert to CMYK yourself, or submit RGB

**Submit CMYK**: you are the only person who can see whether the conversion damaged your images -- whether the sky banded, the brand red went brick, the shadows crushed. Converting means you **approved** the result, and it removes the printer's unknown default conversion.

**Submit RGB**: the RGB gamut is larger, so converting early throws away information permanently and pins the file to one printing condition. The printer knows their press, ink, and paper; you do not. **Photo-book printers overwhelmingly want RGB** because their originals are wide-gamut.

**And the vendors disagree with each other, which is the point.** Ingram requires CMYK, converts RGB anyway, and disclaims liability either way ("any dissatisfaction with color shift will be the publisher's responsibility to correct"). Photo-book printers want sRGB. **There is a right answer per vendor per product, and the pipeline must carry it as configuration, not as a house style.**

### Source-based typesetting versus direct manipulation

**For source (LaTeX, Typst)**: the book is code -- diffable, reviewable, buildable in CI, reproducible from a tag five years later. A page-count change is a diff, not a discovery. The apparatus (TOC, index, cross-references, bibliography) is generated and consistent by construction. A hundred-chapter multi-author technical book is only practical this way. **And crucially, only a source-based pipeline can be gated by CI -- you cannot unit-test an InDesign document.**

**For direct manipulation (InDesign)**: typography at the level Bringhurst and Hochuli describe is a sequence of **judgements about specific pages** -- this widow needs the paragraph re-run, this figure fights the heading opposite, this spread needs a line stolen, the optical margin is wrong even though the metrics are right. A designer needs to see the page and change it, and the edit-compile-look round trip is the difference between fluency and drudgery. **Hochuli's *Detail in Typography* is essentially a catalogue of decisions that cannot be expressed as a global parameter.**

**Where each side concedes.** The source camp's real weakness is not typographic capability (LaTeX with a good class sets excellent pages) but **the last 5%** -- and expressing per-page fixes means littering the source with `\looseness` and `\enlargethispage`, **which is exactly the unreproducible manual intervention the approach was meant to avoid.** The InDesign camp's weakness is that the artifact is opaque, the build is a human, and **there is no supported headless mode.** Typst shifts the terms (sub-second incremental builds approach interactive, and v0.15 has real `bleed`) but lacks PDF/X and has a thin book-class ecosystem. **Nobody should claim this is settled.**

### Is POD adequate for trade?

**Yes**: text reproduction on a modern digital press is indistinguishable from offset to a normal reader; the same distributors sell both side by side; major publishers use POD for backlist; and the alternative for most titles is not existing.

**No**, with specific checkable defects: perfect-bound POD spines crack and release pages with use; stock choice is narrow and often brighter and thinner than trade; **colour is markedly worse and the TAC ceiling is 240% versus 300-320% offset**, which flattens shadows; **there is no press check and no contract proof**, so colour is whatever it is; registration is looser; and **you cannot use spot colour, foil, emboss, deckle edge, coloured endpapers, or head-and-tail bands** -- the whole vocabulary of book-as-object design.

**The honest split**: adequate for text-driven trade, inadequate for colour-critical and object-critical work, and **the boundary sits at "does the reader look at the paper."**

### Are CSS-Paged-Media pipelines production-ready?

**Yes**: Prince has shipped production books for two decades; WeasyPrint is mature; the source is HTML/CSS so the whole web toolchain applies; and **the EPUB comes out right way round** rather than as a lossy derivative of a print artifact. For documents-at-scale -- reports, catalogues, generated documentation -- it is the correct architecture.

**No**: the spec is fragmented across CSS Paged Media 3, CSS Fragmentation, and GCPM (a Working Draft for over a decade, partly abandoned), so no two engines implement the same thing. **The good engine is commercial. Paged.js -- the free one everyone reaches for -- lacks footnotes, leaders, and top/bottom floats, and Chrome writes no TrimBox.** And WeasyPrint's PDF/X mode can write the claim without the output intent, **which is worse than not supporting it.**

**The unreconciled core**: **HTML/CSS has no native concept of the spread, and book typography is a spread-level art.** Vertical rhythm across facing pages, cross-gutter images, balanced facing columns, spread-aware image placement -- the model does not express any of it. **That is a design-language limitation, not an implementation gap**, and it is why the CSS camp's best results are documents rather than books.

---

## 10. Authorities

**Typography and book design**: **Bringhurst**, *The Elements of Typographic Style* (measure, scale, the page as proportional system, the historical margin canons). **Hochuli**, *Detail in Typography* and (with Kinross) *Designing Books* -- the micro scale, and the best counterweight to "set global parameters and let the engine run". **Tschichold**, *The Form of the Book* and the Penguin Composition Rules -- the canonical example of house standards as an enforceable spec. **Williamson**, *Methods of Book Design*. **Knuth**, *Digital Typography* -- the global-optimum line-breaking algorithm is the technical reason TeX pages beat greedy-engine pages.

*Note: "The Bookmaker's Dozen" could not be verified as a real title -- do not cite it.*

**Editorial convention**: *Chicago Manual of Style*, 18th ed. (2024) for front-matter order and proof stages; *New Oxford Style Manual* / *Hart's Rules* for UK convention.

**Colour and prepress**: ISO 12647-2 (offset process control), ISO 15930-x (PDF/X), ISO 3664 (viewing conditions). **Idealliance** (GRACoL, SWOP, G7), **Fogra** (FOGRA39/51, the media wedge), **ECI** (free profile distribution), **BVDM** (the Altona Test Suite -- the actual tool for settling "does your RIP handle transparency"), **Ghent Workgroup** (the exchange specifications), **the PDF Association**, **ISO TC 130**.

**Distribution**: **BISG** (BISAC, the *Barcoding Guidelines for the US Book Industry*), **EDItEUR** (ONIX, Thema), the **International ISBN Agency**, Bowker, Nielsen.

**W3C**: the Publishing Maintenance Working Group (EPUB 3.3, EPUB Accessibility 1.1), the CSS Working Group, the Print and Page Layout Community Group.

**Vendor documents, and the first is the single most useful document in the domain**: the **IngramSpark / Lightning Source File Creation Guide** (`ingramspark.com/hubfs/downloads/file-creation-guide.pdf`) -- specific, numeric, and it states its own contradictions. Then KDP's print help pages, Lulu's distribution requirements, and Blurb's PDF-to-book specification.

---

## 11. Severity rubric (domain-specific)

- **blocker** -- the object will be physically wrong or rejected: symmetric interior bleed, a cover built from a stale page count, reader spreads or printer marks in a submitted file, registration black in body text, exceeded TAC, missing TrimBox on a PDF/X handoff, a font that will substitute at the RIP, WeasyPrint's PDF/X claim without an output intent. Also **false claims about the physical object**: a Z39.48 paper claim or "Printed in the USA" on a globally-distributed POD title, an FSC mark on an Ingram title (removed *at the publisher's expense*), a CIP block on a self-published book.
- **major** -- a real defect that survives to print or blocks distribution: gutter too small for the page count, spine text below the vendor floor, effective resolution below the floor, a rasterized or resized barcode, a single-pass build shipping a stale index, an overfull hbox reaching print, an unpinned toolchain in a pipeline that claims reproducibility, hardcoding a printing number line, a false `accessModeSufficient` claim.
- **minor** -- fragile but not yet wrong: a hardcoded vendor constant that happens to match today's requirement, a computed spine width where the vendor template should be used, non-deterministic PDF output, a missing deliberate blank last leaf.
- **nit** -- typeface credit omitted from the colophon, front-matter ordering deviations that are house choices, metric annotations in prose that disagree with the inch values.
- **insight** -- a structural reframing: the cover should be a build-graph dependent of the interior; this pipeline targets PDF/X but the vendor wants sized-to-bleed with no marks, which are different targets; the verso page needs a slot for externally-supplied CIP text because it cannot be generated from source; this is an object-critical title and POD is the wrong manufacturing route.

**Confidence calibration**: vendor requirements are checkable and citable, so findings against a named vendor's published spec should be high-confidence -- **but state the revision you checked**, because these change and vendors contradict themselves. Physical-outcome predictions (how a defect will look in print) are mechanism-based; state the mechanism.

---

## Changelog

- **2026-09-09** -- Initial file. Established against primary sources: interior bleed on three edges only, never the bind edge, making the interior page box asymmetric and recto/verso-mirrored (Ingram); Ingram's 240% TAC ceiling **enforced retroactively**; Ingram prohibits FSC/PEFC/SFI marks and removes them at the publisher's expense; self-publishers are ineligible for LoC CIP (three titles by three authors, each in 1,000+ libraries); Ingram requires PDF/X-1a:2001 or X-3:2002 as of its 2026-08-24 guide; Ingram pads to mod 2 and marks the last page; KDP's AI obligation is metadata-only and AI-*assisted* work needs no disclosure. Verified from source: **WeasyPrint's PDF/X-1a writes the conformance claim without necessarily writing `/OutputIntents`** -- a file that lies about itself. Verified via GitHub: **Typst gained `page(bleed:)` with `inside`/`outside` in v0.15.0 but PDF/X remains an open request**. Noted: **no presumption of conformity exists under the EAA** because no harmonised standard has been cited in the Official Journal, so the common "EN 301 549 therefore EAA" inference is invalid (single secondary source -- re-verify). Vendor requirements conflict across Ingram/KDP/Lulu/Blurb on PDF/X, page floors, spine-text floors, rich-black recipes, and colour space, so a single hardcoded constant is wrong somewhere. "The Bookmaker's Dozen" could not be verified and must not be cited. 19 further unverified items are listed in the research notes.
