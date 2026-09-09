---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-09
---

# Citation and bibliography

A reference for advising on and reviewing citation practice, bibliographic data, and the machinery that renders one into the other. Used by the `citation-and-bibliography` subagent and the `/expert-review` / `/expert-plan` / `/expert-consult` / `/consult` skills. Cross-style, cross-toolchain.

**Source research**: `~/.claude/local/research-notes/citation-and-bibliography-research.md`

The unifying thesis: **a citation is a claim about a source, and a bibliography is a rendering of records through a style.** Almost every defect in this domain comes from confusing those three layers -- the record (what the source is), the style (how this venue renders it), and the rendering pass (the program that applies one to the other). Data errors are recoverable. **Style-locked logic and order-dependent rendering are not**, because they corrupt output that looks correct.

The agent's operational question: **"will this citation resolve to the right source, and will the rendering say what the style requires?"**

The empirical priority, in rough order of how often it bites:

1. **Silent degradation** -- a renderer's default behavior on incomplete data is to emit plausible output, not to fail. `Doe (n.d.)` is valid output and a wrong fact.
2. **Identifier corruption** -- a lowercased, URL-encoded, integer-stored, or truncated identifier. The citation looks fine and resolves to nothing.
3. **Order-dependent rendering** -- `a`/`b`/`c` suffixes, `ibid.`, numeric labels, and 3-em dashes are functions of document order, not of the record.
4. **Style-locked logic** -- a hardcoded author threshold or date format that encodes one style's rules in application code.
5. **Build-pipeline defects** -- an unresolved citation is a *warning* in every major toolchain, so the defect ships.
6. **Case transforms** -- automated title-casing corrupts `pH`, `nm`, `NaCl`, `iOS`, `BRCA1`.

**The meta-rule for review**: bibliographic tooling is designed to degrade gracefully, which in an automated build is exactly wrong. Look for the place where a missing field should have stopped the build and didn't.

---

## Volatile surface

`last-verified` is in the frontmatter -- do not restate it in prose. These rot; the rest of this file is comparatively durable.

| Claim class | Rots | Re-verify at |
|---|---|---|
| Style-manual editions | Medium (3-10 yr, asymmetric) | `chicagomanualofstyle.org/help-tools/what-s-new.html`, `apastyle.apa.org`, `style.mla.org`, `legalbluebook.com` |
| Turabian edition (10th reportedly due ~Oct 2026) | **Fast, unresolved** | `press.uchicago.edu` |
| AI-citation guidance | **Fastest class in the domain** | `apastyle.apa.org/blog`, `chicagomanualofstyle.org/qanda`, `style.mla.org` |
| CSL **styles** repo content | **Daily** | `gh api repos/citation-style-language/styles` |
| CSL **spec** version | No -- frozen at 1.0.2 since 2020-09-06 | `gh api repos/citation-style-language/schema/releases` |
| biblatex / biber versions (lockstep) | Medium (~2x/yr) | `curl ctan.org/json/2.0/pkg/biblatex`, `/biber` |
| DataCite schema | Medium (~1/yr) | `schema.datacite.org/versions.html` |
| Reference-manager internals (Zotero/BBT coupling) | Fast | `gh api repos/retorquere/zotero-better-bibtex/releases` |
| Docs-pipeline tool versions | Ordinary release cadence | PyPI JSON API, `gh api` |
| Identifier syntax (DOI, ORCID, arXiv, ISBN) | **No** -- this is the stable layer | `support.orcid.org`, `info.arxiv.org/help/arxiv_identifier.html` |

---

## 1. The style systems (know which one governs, and that it is not universal)

There is no "correct" citation format. There is only *this venue's* format, and a claim about correctness that does not name a style is meaningless. The review reflex: **find the declared style before evaluating any output.**

### Chicago -- 18th edition (September 2024)

**VOLATILE** (2026-09-09) and **the single most likely stale fact in this domain**: Chicago is on its **18th edition**, published September 2024; the 17th was 2017. A model trained before late 2024 will say the 17th is current. The changes matter to code:

- **Publisher place is no longer required for books.** `Pantheon Books, 2024`, not `New York: Pantheon Books, 2024`. Code that requires `publisher-place` or warns on its absence now enforces 17th-edition rules.
- **Author thresholds changed**: up to **six** authors listed in full; **more than six** renders first three + "et al." (17th: up to ten, then first seven). Any `if authors > 10` truncation is a 17th-edition artifact.
- **The 3-em dash for a repeated author is now dispreferred** -- repeat the name. A CSL style still emitting `———.` is rendering 17th-edition output.
- **`ibid.` is de-emphasized.** Shortened notes may be author-title, author-only, or title-only.
- Month and season can usually be omitted for journal articles.
- New formal guidance for citing AI-generated text and images.

Chicago has **two systems**, and conflating them is the most common practitioner error: **notes-bibliography** (superscript number to a footnote, humanities) and **author-date** (`(Smith 2020, 45)`, sciences). There is no single "Chicago style"; a CSL file must pick one.

Paywalled. Free surrogates: the CMOS Citation Quick Guide, **CMOS Shop Talk** (`cmosshoptalk.com`), and the **CMOS Q&A** -- the de facto authority for edge cases, and citable.

### APA -- 7th edition

Author-date only. What code gets wrong:

- **Sentence case for article and book titles, title case for journal names.** The single biggest automatic-transform hazard.
- **Reference list**: up to 20 authors listed; 21+ renders first 19, then an **ellipsis**, then the **final** author. This is *not* "et al." A renderer emitting `et al.` in an APA-7 reference list is wrong. (APA 6 cut at 7 -- a pinned old CSL gives APA-6 behavior.)
- **In-text**: `et al.` from the **first** citation at **three or more** authors. Any "first-use-full, later-use-et-al" logic is APA-6 behavior.
- DOIs as `https://doi.org/...`, no `doi:` label, no "Retrieved from".
- Publisher location dropped (Chicago followed only in its 18th, five years later).

Partly free: `apastyle.apa.org` hosts a large reference-example corpus and the APA Style Blog, where new guidance lands first. **Note for tooling**: the blog path is behind Imperva/Incapsula and blocks both plain fetching and headless browsers.

### MLA -- 9th edition (2021)

MLA's **container model** is architecturally different from every other style: an entry is assembled from nine core elements (Author. Title of Source. Title of Container, Other Contributors, Version, Number, Publisher, Publication Date, Location.) rather than from a per-source-type template. **This is why MLA maps badly onto CSL and BibTeX type enums** -- MLA deliberately does not enumerate source types.

Author-page in text (`(Smith 45)`), **no year**, so an MLA renderer needs no year-based `a`/`b`/`c` disambiguation; it disambiguates by shortened title. Free authority: **MLA Style Center** (`style.mla.org`), including "Ask the MLA".

### IEEE -- numeric, in citation order

`[1]`, `[1], [3]-[5]`. The distinguishing property: **the reference list is ordered by first appearance in the text**, not alphabetically. So the bibliography sort is `citation-number`, and **inserting a paragraph renumbers everything downstream** -- the same order-dependence class as year-suffix disambiguation. Citations are grammatical objects in the sentence (`as shown in [3]`).

`et al.` threshold is aggressive: 7+ authors renders first author + et al. Abbreviated journal titles come from IEEE's own list, not the author's guess. The **IEEE Reference Guide** is a free PDF from the IEEE Author Center.

### Vancouver / ICMJE / NLM -- and the naming trap

The authority chain is **ICMJE Recommendations -> NLM Sample References -> *Citing Medicine*, 2nd ed.** "Vancouver", "ICMJE style", and "NLM style" get used interchangeably, but only *Citing Medicine* is normative -- and it is **freely available** on NCBI Bookshelf (`NBK7256`), unusual in a field of paywalled manuals. It is also book-length, which is why everyone uses the Sample References page instead.

Two ICMJE rules belong in any biomedical pipeline, quoted because they are normative duties:

- Preprints: "When preprints are cited, the citation should clearly indicate that the reference is a preprint."
- Retractions: "**Authors are responsible for checking that none of the references cite retracted articles** except in the context of referring to the retraction."

That second one is the strongest available argument for an automated retraction gate (§6). A biomedical pipeline without one is failing a stated ICMJE requirement.

Mechanics: numeric, citation-order; author lists commonly cut at 6 then "et al." (journals vary); journal titles abbreviated per the **NLM Title Abbreviation** list -- which is what pandoc's `--citation-abbreviations` exists for.

### ACS, AMA

**ACS** accepts **three in-text systems in one style** (superscript numeric, bracketed numeric, author-date) and the journal picks. So "ACS style" underdetermines the citation form. Journal abbreviations in italics with periods (`J. Am. Chem. Soc.`). ACS is the best concrete case for the title-casing argument: titles carry formulae and locants (`H2O`, `cis-`/`trans-`, `pH`, `α`-helix) where a case transform corrupts *chemical meaning*, not just typography. `pH` and `PH` are different quantities.

**AMA** -- 11th edition (2020), numeric superscript, citation-order list, NLM journal abbreviations, up to 6 authors then et al. Used by JAMA journals. Paywalled; free AMA Style Insider blog.

### Bluebook / legal -- why it breaks every bibliographic tool

**VOLATILE** (2026-09-09): the Bluebook is on its **22nd edition**. A model saying "21st (2020)" is stale.

The structural point matters more than the edition. Legal citation is hostile to automated rendering for four reasons:

1. It is **citation-in-text-only** -- full citations in footnotes, no separate bibliography. Tools built around a bibliography artifact have nothing to render.
2. Citations point to **jurisdiction-specific reporters, statutes, and dockets** with their own abbreviation tables (`410 U.S. 113 (1973)`, `17 U.S.C. § 107`). These are not author-title-year records; CSL's `legal_case` / `legislation` / `regulation` / `hearing` types are a thin approximation, and CSL Bluebook styles are notoriously incomplete.
3. **Short forms, `id.`, and `supra` are pervasive and position-dependent** -- far more than Chicago's `ibid.` `supra note 14` embeds a *footnote number*, so the citation text depends on final numbering.
4. Consequently legal publishing still relies on human cite-checkers and Westlaw/Lexis cite-checking rather than citeproc.

Paywalled. The only freely redistributable legal citation standard is **The Indigo Book** (Sprigman et al., public domain); **Cornell LII's "Basic Legal Citation"** (Peter Martin) is the other free reference.

### Nature / Science -- house styles, which is the point

These are **house styles, not published manuals**: they live in each journal's author guide, change without notice, carry no version number, and override any general style. Nature: numeric superscript, author lists truncated at 5, article titles included but journal titles abbreviated, and a **strict cap on reference count** -- a content constraint no bibliography tool can express. Science: numeric, and historically omitted article titles entirely in some formats, which a CSL style that always emits `title` cannot reproduce.

**Review point**: for a target journal, the house style is authoritative over Chicago/APA/CSL defaults, is unversioned, and the only reliable source is the live author guide. A pipeline claiming "Nature style" from a `nature.csl` should be spot-checked, because the CSL file lags silently.

### Turabian

**VOLATILE and unresolved** (2026-09-09): Turabian 9th ed. (2018) is keyed to CMOS 17, so it currently **lags Chicago 18**. A 10th edition is reportedly due around October 2026 -- treat as unconfirmed and re-check at `press.uchicago.edu`. Sources genuinely conflict here; one library guide claiming the 9th corresponds to CMOS 18 is almost certainly an error.

---

## 2. The machine-readable layer

### CSL -- the spec is frozen, the styles are not

**VERIFIED**: the CSL schema repo's **only** release is `v1.0.2`, published **2020-09-06**. Six years static. Meanwhile the **styles** repo is pushed almost daily and runs ~85 MB, ~2,600 independent + ~8,500 dependent styles.

**That split is the reproducibility hazard, and it is the opposite of where people look.** The spec will not move under you. The style file will. See §5.

Confusing artifact to know about: the docs site at `docs.citationstyles.org/en/stable/specification.html` still carries the page title "Citation Style Language 1.0.1-dev documentation". The content is 1.0.2; the label is wrong. Do not conclude 1.0.1 is current.

### CSL JSON -- the structural contract

- `id` is required (the citekey). `type` is a **closed enum of 45 values**:

```
article, article-journal, article-magazine, article-newspaper, bill, book,
broadcast, chapter, classic, collection, dataset, document, entry,
entry-dictionary, entry-encyclopedia, event, figure, graphic, hearing,
interview, legal_case, legislation, manuscript, map, motion_picture,
musical_score, pamphlet, paper-conference, patent, performance, periodical,
personal_communication, post, post-weblog, regulation, report, review,
review-book, software, song, speech, standard, thesis, treaty, webpage
```

- **The naming inconsistency is real and must be matched exactly**: `legal_case` and `motion_picture` use **underscores**; every other multi-word type uses **hyphens**. A "clean up the enum" refactor that hyphenates uniformly produces `legal-case`, fails schema validation, and falls back to a default type. **Trigger**: a normalization refactor. **Symptom**: legal cases rendering as generic documents.
- **There is no `preprint` type.** Preprints are conventionally `article-journal` with the server as `container-title`, or `report`, or `manuscript`. This genuine gap is why preprint citations render inconsistently across styles.
- Under-used types worth knowing: `classic` (ancient works cited by standard divisions rather than page), `document` (a better generic fallback than `misc`), `standard`, `software`, `dataset`, `treaty`, `regulation`, `periodical`, `performance`.
- **Dates are `date-parts`: arrays of arrays.** `{"date-parts": [[1953, 4, 25]]}`; a range is two inner arrays. **Failure mode**: code assuming three elements crashes or emits "January 1" for a year-only citation. Year-only is `[[1953]]` -- length 1 is normal, not an error. Also `raw`, `literal`, `season`, `circa`.
- **Names**: `family`, `given`, `non-dropping-particle`, `dropping-particle`, `suffix`, `literal`. `literal` is the corporate-author escape hatch. **The dropping/non-dropping distinction is load-bearing**: non-dropping particles stay with the family name when inverted and affect sorting; dropping particles drop when only the family name shows. Most importers get this wrong and flatten everything into `family`.
- **Field names are case-sensitive and inconsistent by design**: `DOI`, `ISBN`, `ISSN`, `PMID`, `PMCID`, `URL` are **uppercase**; everything else is lowercase-hyphenated. **Code that camelCases or lowercases keys silently loses the DOI.**

**The three-markup-languages trap**: inline markup inside a field differs by serialization. CSL JSON uses an HTML-like subset (`<i>`, `<b>`, `<sub>`, `<sup>`, `<sc>`, and `<span class="nocase">`); CSL YAML uses pandoc Markdown; BibTeX/BibLaTeX fields are parsed as **LaTeX**. The same logical bibliography has three different inline languages, and round-tripping is where italicized species names and subscripted formulae get mangled.

### citeproc implementations -- current state

**VOLATILE** (2026-09-09), and the highest-value stale fact here: **citeproc-rs is dead.** Zotero's Rust rewrite was **archived 2026-08-13**, last code push 2024-08-06, never published to crates.io, never shipped a processor 1.0. Any advice to "use citeproc-rs" or "citeproc-rs is coming" is stale; recommend against new dependencies on it.

- **citeproc-js** (Frank Bennett / Juris-M) -- the reference implementation and the completeness benchmark. Powers Zotero and Mendeley. Remains the only production-grade full CSL implementation.
- **citeproc (Haskell)** (John MacFarlane) -- pandoc's built-in since 2.11. Pandoc's own docs describe it as fixing many shortcomings of `pandoc-citeproc`.
- **citeproc-py** -- more maintained than its reputation suggests, still incomplete on note styles and disambiguation.
- **citeproc-el** (andras-simonyi) -- powers Org-cite, one of the few to explicitly claim CSL 1.0.2.
- **Typst / Hayagriva** -- Typst consumes `.bib`, `.csl`, and its own Hayagriva YAML. Notable as the only new typesetting system to adopt CSL as first-class rather than reinventing.

### BibTeX vs BibLaTeX/biber

**VOLATILE** (2026-09-09): biblatex **3.22** and biber **2.22** both shipped **2026-08-13**. BibTeX itself is frozen at **0.99e**.

**The lockstep rule is a real CI break.** biblatex and biber must be version-matched; biber refuses a `.bcf` control file whose version it does not know ("Found biblatex control file version X, expected version Y"). **Trigger**: a CI image with distro TeX Live (old biber) plus biblatex from tlmgr (new), or a developer on TeX Live 2026 and CI on 2023. **Symptom**: failure at the biber step -- and worse, if the pipeline ignores biber's exit code, LaTeX proceeds and emits a document with **every citation as a bold question mark**. Pin the whole distribution (a Docker digest or `texlive/texlive:TL2026`), not the package.

**Why `.bst` is effectively dead**: BibTeX is frozen under the Knuth license, which forbids distributing modified versions under the same name, and the program is 8-bit-unaware by design. `.bst` files are written in a reverse-Polish stack language; biblatex styles are ordinary LaTeX macros. "Customize the bibliography" is a research project in BibTeX and an afternoon in biblatex.

Lineage worth citing correctly: **Philipp Lehman** wrote biblatex and left around 2012; **Philip Kime** is the long-running current maintainer and also wrote biber. Citing "the Lehman manual" cites a 2012-era document.

**Field-name divergence that silently drops data** -- BibTeX ignores unknown fields *without warning*:

| BibTeX | biblatex |
|---|---|
| `journal` | `journaltitle` (`journal` aliased) |
| `address` | `location` (`address` aliased) |
| `school` | `institution` |
| `year` + `month` | `date = {2024-03-15}` (ISO 8601, supports ranges `2020/2021`, open `2020/`) |

biblatex-only and worth knowing: `langid` (drives title-case and hyphenation), `eprint`/`eprinttype`/`eprintclass` (**the correct way to record arXiv**), `urldate`, `related`/`relatedtype` (reprints, version-of-record links -- machinery CSL has no equivalent for), `pagination`, `sortkey`/`presort`, `options`.

**`crossref` semantics differ materially, and the BibTeX behavior surprises people.** In BibTeX, `crossref` makes a child inherit parent fields *and* auto-adds the parent to the bibliography once `min_crossrefs` (default **2**) children cite it -- whether or not it was cited. biblatex has two mechanisms: `crossref` with a configurable per-field **inheritance map** (`\DeclareDataInheritance`, so a `@book`'s `title` becomes an `@inbook`'s `booktitle` rather than being copied verbatim), and **`xdata`** -- an entry type that exists only to be inherited from, is never printed, and never triggers auto-inclusion. **When reviewing `.bib` generation: prefer `xdata` for shared boilerplate.**

**Three canonical `.bib` data pitfalls:**

1. **Brace protection for capitalization.** Styles calling for sentence case lowercase everything after the first word except what is braced. `title = {A study of DNA in Paris}` becomes "A study of dna in paris". You must write `{DNA}` and `{Paris}`. Non-obvious: a brace around the whole title protects everything but in some styles also defeats the style's own capitalization, and biblatex treats `{{...}}` around a whole field differently from per-word braces.
2. **`and` is the author separator, so literal names need bracing.** `author = {Ronald Fisher and Sons}` parses as **two authors**. Corporate authors are worse: `author = {National Institutes of Health}` parses "National" as a given name and renders **"Health, N. I. of"**. Correct: `{{National Institutes of Health}}`. `and others` is the magic value producing "et al."
3. **Name particles are detected by lowercase first letter.** `Ludwig van Beethoven` yields a von-part `van`. So `{Vincent Van Gogh}` sorts under V and `{Vincent van Gogh}` sorts under G -- **a data-entry-dependent sort key**, which is why the same bibliography sorts differently across tools. biblatex adds explicit control: `author = {given=Vincent, prefix=van, family=Gogh, useprefix=true}`.

**UTF-8**: classic `bibtex` is 8-bit-unclean -- non-ASCII must be TeX-escaped, and **sorting happens on the escaped bytes**, so `Ä` sorts after `Z`. biber is fully Unicode and uses the Unicode Collation Algorithm with `--sortlocale`. **Trigger**: piping a Zotero-exported UTF-8 `.bib` into a `natbib`+`bibtex` pipeline. **Symptom**: mojibake in names plus non-obviously wrong alphabetical order.

**Key collisions and stability**: BibTeX keys are a flat global namespace with no uniqueness enforcement. Concatenating two files with the same key makes `bibtex` **silently use the first and warn**. Worse, generated citekeys *change* when metadata is corrected -- a year fix moves `smith2020foo` to `smith2021foo` and **rewrites every `\cite` in the document**. **Review rule: generated citekeys must be pinned before they enter prose, or the prose and the database drift.**

### Other interchange formats

- **JATS** (NISO Z39.96) is what the publishing industry actually stores articles in. References are `<ref-list>/<ref>` containing either `<element-citation>` (fully structured) or `<mixed-citation>` (semi-structured, punctuation baked in). **That choice is the most consequential decision in a JATS pipeline**: `mixed-citation` preserves the publisher's rendered string but is nearly unusable for machine matching; `element-citation` is queryable but requires the publisher to have parsed correctly. Most legacy corpora are `mixed-citation`, which is why reference-matching services exist. **BITS** is the book analogue, **STS** the standards analogue.
- **RIS** is the lowest common denominator and therefore the most lossy: flat two-letter tags, no nesting, no name-part structure, and a type enum smaller than everyone's. Use only when nothing else is offered.
- **Dublin Core** is **too lossy for citation in principle** -- 15 elements, no way to distinguish a container title from a title, no structured names. If a system's bibliographic layer is Dublin Core, correct rendering is not achievable.
- **MODS** (Library of Congress) is library-oriented XML, richer than Dublin Core, common in repositories.
- **Crossref's deposit schema** is what publishers POST to register a DOI, so its required fields determine what metadata exists downstream for everyone else.
- **CiTO** (Citation Typing Ontology, Peroni and Shotton) lets a citation carry its *rhetorical function* -- `cito:disagreesWith`, `cito:usesMethodIn`, `cito:citesAsAuthority` -- machine-readable *why* rather than only *that*. Underused; **OpenCitations / COCI** is the open DOI-to-DOI citation index.

**Prefer for transfer**: CSL JSON between CSL-aware tools, BibLaTeX when the destination is LaTeX, RIS never by choice.

---

## 3. Identifiers and their contracts

This is the durable layer -- these syntaxes do not rot -- and it is where the highest-confidence findings live, because the rules are checkable.

### DOI

Form: `10.<registrant-prefix>/<suffix>`, prefix always `10.` plus 4+ digits. **The suffix is opaque** and may contain `/`, `.`, `(`, `)`, `<`, `>`.

**What to check**: matches `^10\.\d{4,9}/\S+$` and *nothing else*. Do not validate the suffix's internal shape, do not assume it is URL-safe, do not assume a single `/`.

**Two opposite-direction failures, and one function handling both contexts is the bug:**

- **Never URL-encode the suffix** before appending to `https://doi.org/`. `doi.org` expects the raw suffix. Percent-encoding `<`/`>` in an old Wiley DOI like `10.1002/(SICI)1099-1085(199908/09)13:12/13<1839::AID-HYP893>3.0.CO;2-M` breaks resolution. **Symptom**: 404s concentrated on exactly the oldest, ugliest, most-cited DOIs.
- **Always encode** a DOI placed in a URL *query parameter*.

**Case**: the DOI is **case-insensitive for resolution**, but **compare case-insensitively and store as received**. Lowercasing for display is permitted; lowercasing for *storage* destroys information you cannot recover, and diverges from the publisher's landing page and from others' reference lists. Never uppercase. **Symptom of a `LOWER()` in the schema**: works for years, then breaks on dedup against an un-lowercased source, or on DOIs used as filenames or map keys.

**Display** (Crossref, verified): always the full URL form `https://doi.org/10.xxxx/xxxxx`, hyperlinked, **never prefixed `doi:`**, HTTPS, never `dx.` in the hostname. Older `dx.doi.org` and `http://` forms keep working indefinitely. **Crossref's guidelines do not address print** -- see the §8 disagreement.

**Registration agencies**: Crossref (literature) and DataCite (data, software, increasingly preprints and theses) are both DOI RAs with **different schemas and APIs**. **The DOI prefix does not reliably tell you which RA**, so a resolver must try both or query the DOI proxy at `https://doi.org/api/handles/<doi>`.

### ORCID

- **16 digits, and all 16 are required** -- "they can not be shortened to remove leading zeros." **Trigger**: storing an ORCID in an integer column, or JSON round-tripping through a numeric type. **Symptom**: `0000-0002-1825-0097` becomes `21825097`, unrecoverably.
- ORCID is a **namespaced subset of ISNI** (ISO 27729), not a parallel scheme.
- **Check digit** is ISO/IEC 7064 MOD 11-2 and may be `0`-`9` **or capital `X`** (value 10). The accumulator is `(total + digit) * 2` *inside* the loop -- not positional weights. Hand-rolled versions commonly get this wrong. Accept `X` uppercase only.
- **Validate the checksum, not the range.** Issued blocks are `0000-0001-5000-0007`-`0000-0003-5000-0001` **and a second block `0009-0000-0000-0000`-`0009-0010-0000-0000`** after ORCID exhausted the ISNI-reserved range. **Trigger**: a regex hardcoded to `0000-000[123]-` (a real pattern in older code). **Symptom**: every ORCID issued after the block switch is rejected.
- **Storage contract**: ORCID's own guidance is to store the **full https URI with hyphens**, not the bare digits. Schemas storing bare digits deviate and need normalization on both sides of any join against ORCID API payloads.
- The `http` -> `https` migration is visible across record schema versions, so **string-comparing URIs harvested at different times double-counts the same author**.
- Test values: `0000-0002-1825-0097` (Josiah Carberry), and **`0000-0002-1694-233X` for the X-checksum case**.

### arXiv

- **New scheme** (since 1 April 2007): `arXiv:YYMM.number{vV}`. **The digit count changed**: April 2007 - December 2014 is **4 digits**; January 2015 onward is **5**. A regex of `\d{4}\.\d{4}` silently fails on everything since 2015; `\d{4}\.\d{4,5}` is correct. **Symptom**: post-2015 IDs unmatched, or matched-and-truncated into a valid-looking but wrong identifier.
- **Old scheme** (1991 - March 2007): `archive[.subject-class]/YYMMNNN`, e.g. `math.GT/0309136`. **Two structurally different grammars coexist permanently**, and the old form contains `/` and `.` in positions that break naive splitting.
- **Versioning**: an unversioned identifier means "most recent version" -- so **an unversioned arXiv citation is a moving target** that silently re-points as the author revises. A citation whose claim depends on the text must carry `vN`.
- The correct biblatex encoding is `eprint = {2301.00001}, eprinttype = {arxiv}, eprintclass = {cs.LG}` -- **not** stuffing `arXiv:2301.00001` into `journal` or `note`, which defeats eprint-aware styles.

### ISBN, ISSN, and the rest

- **ISBN**: ISBN-10 check digit is mod 11 (weights 10..1) and **may be `X`**; ISBN-13 is mod 10 with alternating 1,3 weights (identical to EAN-13). **Converting ISBN-10 to ISBN-13 is not just prefixing `978` -- you must recompute the check digit.** Hyphenation is **registration-group-dependent** and requires the ISBN range table; you cannot hyphenate correctly with a fixed pattern. Store unhyphenated.
- **ISSN / ISSN-L**: 8 digits, mod-11 check digit that **may be `X`**. **ISSN-L is the piece code usually misses**: a journal with print and online editions has *two* ISSNs plus one ISSN-L grouping them. **Trigger**: deduplicating journals by ISSN. **Symptom**: one journal appearing as two venues, splitting citation counts. Dedup on ISSN-L.
- **PMID vs PMCID**: PMID is a bare integer (safe as an integer, unlike ORCID). PMCID is `PMC` + digits and **the `PMC` prefix is part of the identifier**. They are different numbers for the same paper. NIH public-access compliance requires the PMCID specifically.
- **ROR** (affiliations, the open GRID successor): `https://ror.org/` + 9 characters with an ISO 7064 MOD 97-10 checksum in the last two. **Lowercase** -- uppercasing breaks the URL.
- **PURL vs DOI**: a PURL is just a URL with an operator promise -- no syntax to validate, no checksum, no guarantee once the operator lapses. Treat a PURL as a URL requiring an access date; treat a DOI as an identifier that does not.
- **LCCN** is structurally messy (pre-2001 `YY-NNNNNN` with optional alphabetic prefixes, post-2001 `YYYYNNNNNN`) and has a **documented normalization algorithm at LoC**. Do not parse ad hoc.
- **OCLC numbers** appear with historical prefixes `ocm`/`ocn`/`on` and `(OCoLC)` in MARC. String-comparing `ocm12345678` against `12345678` fails dedup against WorldCat.

---

## 4. Rendering failures (order-dependence, thresholds, case)

### Disambiguation is ordered, and hand-rolling it is wrong

CSL applies four disambiguation methods **always in this order**: (1) expand names (add initials or full given names), (2) show more names, (3) render with the `disambiguate` condition, (4) **add a year-suffix**.

Two consequences:

- **A naive implementation that jumps straight to `a`/`b`/`c` produces different -- and differently correct -- output** than CSL, which would first render `J. Doe 2007` and `S. Doe 2007`.
- **The year-suffix is a function of bibliography sort order, not of the data.** The spec: assignment "follows the order of the bibliographies entries, and additional letters are used once 'z' is reached." So changing the sort, the `lang`/collation, or adding one unrelated reference turns `Doe 2007a` into `Doe 2007b` **throughout the document**. **Trigger**: adding a reference, changing `lang`, upgrading the style. **Symptom**: a large mystifying diff in rendered output with no source change -- "the citations changed and nobody edited them" is a real docs-as-code diff.
- Past 'z' it continues `aa`, `ab`. **Code doing `char suffix = 'a' + n` overflows silently at the 27th collision.**

### et al. thresholds are position-dependent and style-specific

- The spec says truncation fires when the name count **"matches or exceeds"** `et-al-min`. **An off-by-one (`>` vs `>=`) here is the single most common style bug.**
- `et-al-subsequent-min` / `et-al-subsequent-use-first` **replace** the base thresholds for *subsequent* cites. So **the same reference renders with a different number of names on first vs. later citation.** A renderer treating formatting as a pure function of the record cannot express this -- it needs document-order state.
- `et-al-use-last` adds the delimiter-ellipsis-last-name pattern. **APA 7's 21-author rule is not a special case in the processor** -- it is `et-al-min=21, et-al-use-first=19, et-al-use-last=true`.
- **The thresholds differ per style and there is no universal rule**: APA 7 in-text 3+, reference list 21+ -> 19 + ellipsis + last; Chicago 18 bibliography 7+ -> first 3 + et al.; Vancouver commonly 7+ -> first 6; IEEE 7+ -> first author only. **Hardcoding any of these is style lock-in.**

### Name particles: whether "van Beethoven" files under B or V is a style attribute

CSL's `demote-non-dropping-particle` has three values: `never` (particle is part of the primary sort key), `sort-only` (demoted to secondary), and `display-and-sort` (**the default**). **So filing position is a per-style decision, not a fact about the name.** Dutch convention files under B; some Anglo-American library practice files under V. **Trigger**: a reviewer says the bibliography is misalphabetized. **Symptom**: it is correctly alphabetized *for a different style*. Check the style's attribute before "fixing" data.

Parallel failure classes CSL cannot fully express:

- **CJK names** -- family-first, no inversion; CSL has no `name-order` field, so processors infer from `language`/script, which fails for a Chinese author publishing in English.
- **Icelandic patronymics** -- "Einarsson" is not a family name and convention files under the *given* name. Workaround is `literal`.
- **Mononyms** -- putting Aristotle in `given` with an empty `family` sorts to the top and renders as ", Aristotle". Correct: `family: Aristotle` with no `given`, or `literal`.
- **Spanish/Portuguese double surnames** -- García Márquez files under G; splitting on the last space files it under M.

### Title case: the transform that corrupts meaning

**The mechanism, and it is the root of the whole bug class**: citeproc **stores every title internally in sentence case** and converts to title case on render. That is why `pH` becomes `Ph`, `nm` becomes `Nm`, `mRNA` becomes `Mrna`, `NaCl` becomes `Nacl`, `iOS` becomes `Ios`.

**The required source convention is opposite depending on serialization** (from pandoc's manual):

- **BibTeX/BibLaTeX input**: English titles in **title case**; non-English in sentence case with `langid` set.
- **CSL JSON/YAML input**: **all** titles in **sentence case**, with `language` set for non-English.

And **even in BibTeX you must protect tokens that should stay lowercase or camelCase**, because citeproc round-trips through sentence case regardless: `title = {Spin Wave Dispersion on the {nm} Scale}`.

Per-serialization protection syntax: `{nm}` in `.bib`, `<span class="nocase">nm</span>` in CSL JSON, `[nm]{.nocase}` in CSL YAML.

**The spec concedes the limit**: sentence-case conversion "capitalizes only the first word and proper nouns" -- which a processor cannot identify -- and for non-English items `text-case` "should not be used on the title variable." That concession is the substance of the §8 argument.

### The rest of the rendering catalog

- **Unescaped LaTeX specials.** **Trigger**: a title containing `%`, `&`, `#`, `_`, `$`, `{`, `}`, `~`, `^`, `\` reaching a `.bib` or `.tex` from a database, a Crossref response, or a form. **The `%` case is the dangerous one because it fails silently** -- it comments out the rest of the line, deleting the remainder of the entry and often producing a *valid but wrong* bibliography. Escape at the serialization boundary, never at data entry, and never with a regex that also mangles already-escaped input (`\&` -> `\textbackslash{}\&` is the second-order bug). Pandoc *parses* LaTeX in `.bib` fields, so escaping is contractual, not optional.
- **Page ranges.** CSL's `page-range-format` (`chicago`, `expanded`, `minimal`, `minimal-two`) produces genuinely different output: `321-325` vs `321-25` vs `321-5`. Chicago's rule is conditional on the hundreds digit. **Trigger**: computing abbreviation in application code instead of letting citeproc do it. Also: styles want an **en dash**, data usually has a hyphen, and an em dash or Unicode minus breaks range parsing entirely. A record with only a start page must not be fabricated into a range.
- **`ibid.` is layout-dependent, and getting it wrong is a misattribution.** CSL position tests (`ibid`, `ibid-with-locator`, `near-note` with `near-note-distance`, default 5) depend on *immediately* preceding notes, which changes when content reflows, a note moves pages, or a chapter is reordered. **Trigger**: caching or reusing rendered citation strings. **Symptom**: "Ibid." pointing at the wrong work -- a factual error invisible to spell-check and CI. Chicago 18 de-emphasizing ibid. is partly a response. `op. cit.` is deprecated essentially everywhere.
- **`subsequent-author-substitute`** is the 3-em-dash mechanism (rules: `complete-all` default, `complete-each`, `partial-each`, `partial-first`). It depends on the *preceding entry*, so re-sorting changes which entries get dashed -- another order-dependent transform. **This is the feature Chicago 18 just deprecated.**
- **Layout belongs to the style and template, not the data.** `cs:bibliography` carries `hanging-indent`, `entry-spacing`, `line-spacing`, `second-field-align` (how numeric styles put `[1]` in the margin). **Trigger**: a CSS or LaTeX theme setting its own list indentation. **Symptom**: numeric labels overlapping text, or hanging indents lost in HTML-to-PDF.
- **Missing records degrade instead of failing -- the meta-failure mode.** A CSL record with no `issued` renders `Doe (n.d.)`: valid output, wrong fact. A missing `container-title` collapses a numeric entry to `[1] J. Doe, "Title," pp. 3-4.` with no venue. An empty `author` array makes many styles fall back to title-as-author, **changing the sort position**. A `date-parts` of `[[]]` renders an empty year rather than erroring. **Nothing in CSL, pandoc, or biblatex validates per-style required fields for you.** A pipeline must do it and fail.

---

## 5. The pipeline (bibliography as a build artifact)

**This is where the highest-severity findings live in automated publishing**, because every toolchain in this section treats an unresolved citation as a *warning*, and the defect ships as visible garbage in a published document.

### The universal rule

**In every major toolchain, an unresolved citation is a warning and the build exits 0.** Promoting it to an error is a deliberate act. No pipeline does it for you. The canonical symptom is a PDF published to a docs site with `[?]` or a bold `?` scattered through it because CI treated warnings as noise.

### LaTeX: the multi-pass requirement

The sequence is **latex -> bibtex/biber -> latex -> latex**. Pass 1 writes `\citation{key}` into the `.aux` (or `.bcf`); bibtex/biber reads it and writes `.bbl`; pass 2 typesets the bibliography; pass 3 resolves labels that moved because the bibliography changed pagination.

**What a skipped pass produces is a successful exit code and a defective document:**

- Skip bibtex/biber entirely: every `\cite` renders `[?]` or bold `?`, the bibliography is **absent**, and LaTeX exits **0** with only `LaTeX Warning: There were undefined references`.
- One latex pass after bibtex: citations render but **numbers can be wrong**, with `Label(s) may have changed. Rerun to get cross-references right.`

**The fix, verified from the latexmk manual (dated 9 March 2026):**

`latexmk -Werror` "causes latexmk to return a non-zero status code if any of the files processed gives a warning about problems with citations or references (i.e., undefined citations or references or about multiply defined references) ... but only when they occur on the last run of *latex and only after processing is complete."

Two properties make this the right gate:

1. **It is scoped, not blanket** -- citation and reference warnings specifically, unlike pandoc's equivalent.
2. **It is fixed-point aware** -- evaluated only on the final pass, after latexmk resolves everything it can. **This is why a naive `grep "Citation .* undefined" build.log` is inferior**: a grep over the whole log catches first-pass warnings that later passes legitimately resolved, producing false failures.

The manual explicitly endorses this use: "latexmk can also be used as part of a build process for some bigger project, e.g., for creating documentation in the build of a software application. Then it is often sensible to treat citation and reference warnings as errors." Settable in `.latexmkrc` as `$warnings_as_errors`, so the gate lives in the repo rather than the CI invocation.

Also: **biber's `--validate-datamodel`** catches malformed and incomplete entries at the data level, which is the better place to catch them.

### LaTeX: the citation-command matrix and a hard conflict

| | natbib | biblatex |
|---|---|---|
| Parenthetical | `\citep` | `\parencite` |
| Textual | `\citet` | `\textcite` |
| Style-delegating | -- | **`\autocite`** |
| Declare source | `\bibliography{file}` (no ext.) | `\addbibresource{file.bib}` (**with** ext.) |
| Backend | `bibtex` | `biber` |

**Prefer `\autocite`** precisely so switching style does not require editing prose.

**natbib and biblatex cannot both be loaded** -- biblatex errors out. `\usepackage[natbib=true]{biblatex}` *emulates* `\citep`/`\citet` as a migration path. **Trigger**: a journal template loading natbib in the class file plus an author-added biblatex. Same conflict class: `cite.sty`, `citeref`, `backref`, `multibib`, `bibtopic`. **`\bibliography` vs `\addbibresource` is a reliable tell** of which system a document actually uses; mixing them yields no bibliography and only a warning.

### Pandoc

- Flag is `--citeproc` / `-C`. **`pandoc-citeproc` was deprecated in pandoc 2.11.**
- **Filter ordering matters.** `--citeproc` behaves like a filter and is positional. A Lua filter sees either raw `Cite` elements or rendered citations depending on argument order. **Trigger**: someone adds `--lua-filter` to an existing command. **Symptom**: the filter silently stops matching.
- **`cite-method` only affects LaTeX output.** Setting `cite-method: natbib` without `citeproc: true` silently produces `\citep{...}` in the `.tex` and **no bibliography** unless a LaTeX pass runs bibtex.
- **`.bib` is interpreted as BibLaTeX by default**; use `.bibtex` to force classic BibTeX parsing. **A silent-wrongness trigger** for classic files named `.bib`.
- **`link-bibliography` (default true) has a surprising clause**: if an entry has a DOI/PMCID/PMID/URL but the style renders none of them, **the title -- or the whole entry -- gets hyperlinked**. That is the source of mysterious blue blobs in a print references list. Set `link-bibliography: false` for print.
- **`link-citations` silently does nothing in note styles** (author-date and numeric only).
- **`lang` drives collation and is the reproducibility landmine.** A BCP 47 tag; Unicode `-u-` extensions are honored (`zh-u-co-pinyin`, `es-u-co-trad`, `en-US-u-kf-upper`). **The bibliography's sort order is a function of document `lang`**, and unset means inheriting a default that differs across pandoc versions and ICU data. Pin it explicitly.
- **The default style is Chicago author-date**, and which Chicago depends on the pandoc version's bundled copy. A build that never sets `--csl` has silently chosen a style.
- Useful and under-known: **`nocite: | @*`** includes every entry in the database, which is how you generate a standalone "further reading" artifact from a `.bib`. **`--citation-abbreviations`** supplies the journal-abbreviation map for legal and medical styles. Bibliography placement is a `::: {#refs} :::` div contract; with `--file-scope` the id becomes `FILE__refs`, which explains a multi-file build growing several bibliographies.
- **The gate**: `--fail-if-warnings` **does** catch unresolved citations (verified by source trace: citeproc warnings are emitted as `CiteprocWarning`, assigned `WARNING` severity, and `--fail-if-warnings` throws on any WARNING). But it is **indiscriminate** -- it fires on ~30 unrelated warning classes (`DuplicateIdentifier`, `CouldNotFetchResource`, `MissingCharacter`, `Deprecated`, ...), so enabling it on an existing project typically fails immediately for unrelated reasons. **The precise gate is parsing `--log=FILE` structured JSON and failing only on `CiteprocWarning`** -- essentially undocumented, and the right answer for a mature pipeline.

### Other generators

- **Quarto** wraps pandoc, so citeproc semantics carry over (`bibliography:`, `csl:`, `citeproc: false` to hand off to biblatex). Adds a document-level `citation:` block making the *document itself* citable.
- **Sphinx** via `sphinxcontrib-bibtex` is **BibTeX-based, not CSL**, so it **cannot use the CSL style repo** -- styles are pybtex plugins. Roles are `:cite:p:` (parenthetical) and `:cite:t:` (textual); the migration from the old `:cite:` was breaking. Known pain: duplicate-label warnings across documents, and `bibliography` directives in included files producing duplicated entries.
- **Docusaurus** has no first-party support; `rehype-citation` is the notable plugin because it wraps citeproc-js and real CSL.
- **Asciidoctor** via `asciidoctor-bibtex` -- BibTeX-based, few built-in styles, not CSL.
- **mdBook** has **no first-party bibliography support**; community preprocessors are low-bus-factor. Practical pattern: preprocess with pandoc.
- **Typst** consumes `.bib`, `.csl`, and Hayagriva YAML natively.

### Reproducibility: five concrete mechanisms

1. **Unpinned CSL style.** The styles repo changes almost daily. Fetching a `.csl` at build time, or letting Zotero auto-update styles, means **the bibliography changes without a source change**. Vendor the `.csl` into the repo and pin it.
2. **Locale-dependent sorting.** `lang` / `--sortlocale` determine collation, and some toolchains inherit `LANG`/`LC_COLLATE` from the environment. **Trigger**: CI with `LANG=C` versus a developer on `en_US.UTF-8`. **Symptom**: bibliography order differs between local and CI -- and because year-suffix disambiguation follows sort order, **the in-text citations differ too**.
3. **Tool version drift**: biber/biblatex lockstep, TeX Live year, pandoc's bundled default CSL, ICU/CLDR data. Pin the container digest.
4. **`.bib` as a merge-conflict surface.** Reference managers **reorder and reformat the whole file on export** (field order, indentation, brace-vs-quote, `month = jan` vs `month = {1}`). **Trigger**: two authors each re-export from Zotero. **Symptom**: a 3000-line diff with 2 semantic changes. Mitigations, best last: split into per-chapter `.bib` files; commit a canonicalized file from a formatter (`bibtool -s -d`, `biber --tool`, `pandoc -f biblatex -t biblatex`) enforced by a pre-commit hook; or **keep the source of truth in the reference manager and treat `.bib` as a generated artifact** that is gitignored or regenerated by CI. The last is the docs-as-code-correct answer and the one most teams miss.
5. **Deduplication across sources.** Pandoc's documented rule: with both an external bibliography and inline YAML `references`, both are used and **inline wins on conflicting `id`s**. For multiple `--bibliography` args, precedence is version-dependent and undocumented -- **do not rely on it**. Dedup on **DOI case-insensitively**, then normalized title + first-author-family + year. **Never dedup on citekey** -- the whole problem is that two sources gave the same work different keys.

---

## 6. Retraction, versioning, and citing non-article things

### Retraction: an implementable gate almost nobody implements

Crossref acquired the Retraction Watch database in September 2023 (from the Center for Scientific Integrity). How to consume it:

- REST API: retraction data surfaces in the **`update-to`** field; filter `https://api.crossref.org/v1/works?filter=update-type:retraction`.
- Each update carries a **`source`** of `"publisher"` or `"retraction-watch"`. **This discriminator matters**: a paper can be flagged by Retraction Watch curators without a publisher-deposited notice, and vice versa. Code reading only publisher-deposited `update-to` misses the curated flags.
- Full CSV at `gitlab.com/crossref/retraction-watch-data`, git-cloneable, **updated once per working day**, **CC BY 4.0**.

**The design trap**: **`RetractionNature` is not a boolean.** Its values include Retraction, Correction, **Expression of concern**, and **Reinstatement** -- in the same field. A pipeline modeling this as `is_retracted: bool` both **over-flags expressions of concern** and **fails to un-flag reinstated papers**. There is no single `is-retracted` field in the API; status is derived from `update-to` entries plus `RetractionNature`.

**Style rule**: you **cite the retracted work and label it** (APA appends `[Retracted]` plus the retraction notice details). You do not silently drop it -- deleting the citation misrepresents what the author relied on.

### Preprints and versions

A preprint and its version-of-record are **different DOIs**, linked by Crossref `is-preprint-of` / `has-preprint` relations. Cite the version of record once it exists; cite the preprint only when no VOR exists or when the preprint specifically is what you used. **A bibliography citing the preprint of a long-published paper looks like the author never checked.**

### Software, data, and standards

- **`CITATION.cff`** (YAML 1.2) is at spec **1.2.0** and has been stable since 2021-08-09 -- unusually low volatility here. **GitHub natively parses it** and renders a "Cite this repository" widget with APA/BibTeX export, which makes a malformed CFF a *visible* defect on the repo page and makes CFF the pragmatic default. Tooling: `cffconvert`, `ruby-cff`. Author: Stephan Druskat.
- **The Zenodo concept-DOI trap**: Zenodo mints a **version DOI** per release *and* a **concept DOI** resolving to the latest. **Citing the concept DOI cites a moving target.** **Symptom**: a paper's software citation resolves to a later, behaviorally different release than the one that produced the results. Cite the version DOI; CFF has `version` and `date-released` to express it.
- CSL type is `software` (added in 1.0.2); biblatex is `@software`. **Neither exists in classic BibTeX** -- `@misc` is the fallback, which is why software citations render badly in older pipelines.
- **Software Citation Principles**: Smith, Katz, Niemeyer (2016), *PeerJ CS* 2:e86, `10.7717/peerj-cs.86`. FORCE11 also produced the Joint Declaration of Data Citation Principles (2014).

---

## 7. Reference managers, and the human workflow that breaks

- **Zotero** -- the de facto standard. **The translator architecture is the important non-obvious piece**: ~700 JavaScript translators (web/import/export/search), each with a `target` regex and a `priority`. Consequences: **site redesigns break translators silently** (you get a "Web Page" item with no metadata, not an error); translator quality varies wildly, so **the same paper imported from the publisher, PubMed, and Google Scholar yields three different records** (Google Scholar notoriously worst -- truncated authors, missing DOIs, conference papers typed as journal articles). The fix for bad metadata is re-importing by DOI/PMID, not hand-editing.
- **Better BibTeX** -- deterministic pattern-driven citekeys, key **pinning**, auto-export on change (the mechanism that makes docs-as-code viable), and far more correct `.bib` export than Zotero's built-in. **It patches Zotero internals**, which is why a Zotero major upgrade routinely breaks it for days. **VOLATILE** (2026-09-09) and worth re-checking: Zotero 8 reportedly moved citation keys into a **native field** and made pinning universal, which would change the standard "pin your keys in `Extra`" advice from necessary to legacy. Unconfirmed.
- **JabRef** is **BibTeX-native** -- its data model *is* `.bib`, so no lossy mapping. The right choice when the `.bib` file is the source of truth rather than a generated artifact. Good at `crossref`/`xdata` and consistency checks.
- **EndNote** uses its own `.ens` style format, **not CSL**, so styles do not transfer either way. EndNote XML is the interchange path.
- **Field-mapping loss between managers** -- where data actually dies: notes and annotations (rarely mapped); attachment paths; tags vs keywords vs groups collapsing; **item type** (anything outside the target enum degrades to `misc`, permanently losing report vs thesis vs preprint); **name particles** (`non-dropping-particle` has no home in most formats and gets concatenated into `family`, silently changing sort order); date granularity; Zotero's free-text `Extra` field conventions that only Zotero-aware tools parse.

**Cite-while-you-write failure modes** (the highest-value section for advising a human author). Zotero/Mendeley/EndNote store the citation payload inside **Word field codes**. Failures: a collaborator without the plugin leaves fields un-updatable; **accepting or rejecting track changes can corrupt field boundaries**; copy-pasting between documents carries a *different* embedded item registry, so a merged document has two conflicting ID sets; round-tripping `.docx` through Google Docs or LibreOffice mangles fields; Word's Bookmarks mode is more fragile than Fields but required for LibreOffice interop. Google Docs integration breaks on "Make a copy".

**The mitigation every collaboration needs**: agree on one manager *and one plugin mode* up front, keep field codes live until the end, then produce a **final flattened copy** ("Unlink Citations") for the publisher -- and never edit the flattened copy back into the live one.

---

## 8. Schools of thought (live, unreconciled)

State these as genuine tensions. Do not synthesize a middle.

### Author-date vs numeric vs note-bibliography

- **Author-date** (APA, Chicago author-date, sciences): `(Kahneman 2011)` is **self-describing**. An expert reader recognizes the work without a lookup and judges recency and provenance in place. Numeric forces a saccade to the back of the document for information the reader often already has.
- **Numeric** (IEEE, Vancouver, Nature, Science): parenthetical names are **visual noise that destroys sentence rhythm**, and the cost compounds badly -- `(Smith, Jones, and Wu 2019; Alvarez et al. 2021; Chen and Park 2023)` is a line and a half of nothing. In fields where citations cluster heavily, numeric is the only readable option, and print space is a real economic argument.
- **Note-bibliography** (Chicago NB, humanities, legal): the note carries **discursive content** -- provenance, hedging, an alternative reading, an archival shelfmark -- which neither other system can express. Humanists cite manuscripts, editions, and translations where "author-date" actively misrepresents the source.

**Unreconciled**: this tracks disciplinary norms, not evidence, and no reading-comprehension data settles it. The one thing everyone agrees on is the tooling consequence: **keep the source format style-agnostic** so the argument can be deferred to the target. That is the design intent of both pandoc's citation syntax and CSL.

### Do URLs and DOIs belong in a print bibliography?

- **Yes**: a DOI is the only stable identifier for a digital object, and omitting it makes the citation unresolvable for a reader retyping it. **Crossref's stated position is that the DOI "must be displayed as a link", with no print exemption.**
- **No**: a full DOI URL is 30+ unreadable characters that cannot be clicked on paper, consumes journal space, and is redundant with author-title-year.
- **The common middle -- printing a bare `10.1038/171737a0` without the resolver wrapper -- is explicitly forbidden by Crossref's guidelines.** So widespread practice directly conflicts with the registration agency's rule, and **Crossref's guidelines simply do not address print**. The conflict is real and unresolved; publishers fill the gap however they like.

### Are access dates useful or noise?

- **Useful**: for content that mutates or vanishes, the access date is the only honest claim you can make about *what you saw*. MLA and Chicago retain them for undated web sources.
- **Noise**: for anything with a DOI or version number the object is immutable and identified, so the date adds nothing -- APA 7 reduced access-date requirements for exactly this reason. Critics add that access dates are **routinely fabricated**, auto-filled to the export date rather than the reading date, which makes them affirmatively misleading.
- **The synthesis nobody has adopted**: an access date is only meaningful alongside an **archival snapshot**. Citing a URL with an access date but no archived copy records a date for something the reader cannot retrieve. Legal citation has largely accepted this (perma.cc grew out of Harvard Law Library's link-rot work); scholarly publishing broadly has not.

### The DOI display form

Crossref, DataCite, and the IDF are aligned on `https://doi.org/10.x/y`, hyperlinked, never `doi:`-prefixed. **Against**: the `doi:` form is shorter and is a genuine URN-style identifier rather than a location, and **baking a specific resolver hostname into every citation in the scholarly record is exactly the centralization DOIs were meant to avoid** -- if `doi.org` ever lapses, the entire literature points at it. **The counter**: the IDF operates `doi.org` precisely as a permanent public resolver, and actionability beats purity. Note that **the "correct" answer changed within living memory** (`dx.doi.org` -> `doi.org`, `http` -> `https`), which is itself evidence for the skeptics.

### Is automated title-casing ever acceptable?

- **Against, strongly**: the CSL spec concedes that sentence-case conversion capitalizes "the first word and proper nouns" -- which a processor cannot identify -- and instructs that `text-case` should not be used on titles for non-English items. Empirically the transform corrupts gene symbols (`p53`, `BRCA1`), formulae (`NaCl`), units (`nm`, `pH`), and product names (`iOS`, `macOS`, `npm`). In a docs pipeline these are **silent content errors**.
- **For, pragmatically**: without the transform you cannot honor a style whose case convention differs from your stored data, and you cannot mix sources (a title-case `.bib` plus sentence-case Crossref records) without normalizing. Hand-protecting every token does not scale to thousands of references and degrades the moment someone adds an entry.
- **The actual state of practice is two incompatible cultures**: store titles in the case the style needs and never transform (JabRef/biblatex, where `.bib` is the source of truth), versus store sentence case and transform with per-token protection (Zotero/CSL). **These cultures produce mutually incompatible `.bib` files**, which is why importing a colleague's bibliography reliably produces casing errors. **Naming which culture a project is in is a real review question.**

### CSL vs BibLaTeX as the substrate

- **Pro CSL**: styles are **data, not code** -- editable without programming, ~2,600 styles maintained collaboratively, reused across Zotero, Mendeley, pandoc, Typst, and the web, and portable across output formats because rendering is separated from typesetting. For anything that is not LaTeX it is the only serious option.
- **Pro BibLaTeX**: CSL is **not expressive enough** for hard cases -- a declarative language without general computation, visibly struggling with legal citation, multi-volume works, and editions/translations/reprints (biblatex's `related`/`relatedtype` machinery has no CSL equivalent). biblatex styles are Turing-complete LaTeX and biber's data model is richer (per-field inheritance maps, `xdata`, name-part control). For a humanities monograph, biblatex produces output CSL cannot.
- **The newest evidence cuts against conventional wisdom**: CSL's spec has not moved since 2020-09-06, and citeproc-rs was **archived in August 2026 without ever shipping**. biblatex/biber shipped 3.22/2.22 on 2026-08-13. On "which substrate is under active development," the evidence currently favors biblatex.
- **The counter-counter**: CSL's *styles* repo is extremely live even though the spec is frozen, and a frozen spec with thousands of maintained styles and five independent implementations may be a **finished** standard rather than an abandoned one. biblatex's activity concentrates in one maintainer, which is a bus-factor argument the other way.
- **Named positions**: Frank Bennett (citeproc-js, Juris-M) has argued at length that legal and multilingual citation needs extensions CSL lacks -- **Juris-M exists as a fork of Zotero precisely because of it**. John MacFarlane implemented CSL for pandoc and treats it as the interchange layer. Philip Kime maintains biblatex/biber. Rintze Zelle and Sebastian Karcher are long-standing CSL maintainers; Bruce D'Arcus originated CSL.

---

## 9. Anti-pattern catalog

| Pattern | Trigger | Consequence | Fix |
|---|---|---|---|
| Hardcoded author threshold | `if authors > 10: et_al()` | Style lock-in; wrong for every style but one, and wrong for Chicago 18 even if it was right for 17 | Let the style engine decide; never encode thresholds in application code |
| Hand-rolled `a`/`b`/`c` disambiguation | Same author, same year | Skips CSL's prior name-expansion steps, so output differs from every conforming processor; overflows at 27 collisions | Use a citeproc implementation |
| `LOWER(doi)` in schema | Normalizing on write | Silent for years, then breaks dedup, display fidelity, and key equality | Compare case-insensitively, store as received |
| `encodeURIComponent(doi)` before `doi.org/` | Building a resolver link | 404s on the oldest, most-cited DOIs | Append the raw suffix; encode only in query parameters |
| ORCID in an integer column | Schema design | Leading zeros lost, identifier unrecoverable | Store the full https URI with hyphens |
| ORCID range regex | `^0000-000[123]-` validation | Rejects every ORCID from the `0009-` block | Validate the MOD 11-2 checksum, accept `X` |
| `\d{4}\.\d{4}` for arXiv | Regex written pre-2015 | Silently misses or truncates every post-2015 ID | `\d{4}\.\d{4,5}`, plus a branch for the pre-2007 grammar |
| Unversioned arXiv or Zenodo concept DOI | Citing a preprint or software | Citation is a moving target; resolves to a different text than the one used | Cite `vN` and the version DOI |
| `is_retracted: bool` | Modeling retraction status | Over-flags expressions of concern, never un-flags reinstatements | Model `RetractionNature` as an enum |
| Corporate author unbraced in `.bib` | `author = {National Institutes of Health}` | Renders "Health, N. I. of" | Double-brace, or use CSL `literal` |
| Unescaped `%` in a `.bib` field | Title from a database or API | **Silently comments out the rest of the entry**; valid but wrong bibliography | Escape at the serialization boundary only |
| Automated title-casing without protection | Any citeproc render | `pH` -> `Ph`, `NaCl` -> `Nacl`, `iOS` -> `Ios` -- silent content errors | Per-token `nocase` protection; decide the project's casing culture |
| Fetching `.csl` at build time | Convenience | Bibliography changes with no source change | Vendor and pin the style |
| Unset `lang` in a reproducible build | Default | Collation differs local vs CI; **year-suffixes and in-text citations differ** | Pin `lang` explicitly |
| Warnings ignored in CI | Default toolchain behavior | `[?]` ships in a published PDF | `latexmk -Werror`; pandoc `--log` parsed for `CiteprocWarning` |
| Dedup on citekey | Merging `.bib` sources | Does not dedup -- differing keys for one work is the problem | Dedup on DOI case-insensitively, then title+author+year |
| `.bib` hand-edited *and* manager-exported | Two sources of truth | Constant merge conflicts; 3000-line diffs | Pick one: `.bib` as source (JabRef) or as generated artifact (Zotero + BBT) |
| Unpinned generated citekeys | BBT pattern change or metadata fix | Every `\cite` in the prose silently breaks | Pin keys before they enter prose |
| Caching rendered citation strings | Performance optimization | `ibid.` and numeric labels point at the wrong work | Render in document order; never cache position-dependent output |
| Claiming a house style from a repo `.csl` | `nature.csl` | House styles are unversioned and the CSL file lags silently | Spot-check against the live author guide |

---

## 10. Authorities

**Style manuals** (access status matters, because a paywalled manual means the free surrogate is what people actually use):

- *Chicago Manual of Style*, 18th ed. (2024) -- paywalled. **CMOS Q&A** and **CMOS Shop Talk** are free and citable for edge cases.
- *APA Publication Manual*, 7th ed. -- paywalled; large free reference corpus and the APA Style Blog at `apastyle.apa.org`.
- *MLA Handbook*, 9th ed. (2021) -- paywalled; **MLA Style Center** free.
- ***Citing Medicine*, 2nd ed.** (NLM) -- **free** on NCBI Bookshelf, and the most thorough treatment of source-type edge cases in any style. **ICMJE Recommendations** free.
- *IEEE Reference Guide* -- **free** PDF, IEEE Author Center.
- *AMA Manual of Style*, 11th ed. -- paywalled. *ACS Guide to Scholarly Communication* -- paywalled, free Quick Guide.
- *The Bluebook*, 22nd ed. -- paywalled. **The Indigo Book** (public domain) and **Cornell LII Basic Legal Citation** are the free alternatives.

**Standards and registries**: CSL (`docs.citationstyles.org`, schema and styles repos; maintainers Rintze Zelle, Sebastian Karcher, Frank Bennett; originator Bruce D'Arcus). Crossref (display guidelines, REST API, retraction data; **Geoffrey Bilder** on identifier design). DataCite. ORCID. arXiv. BibLaTeX/biber on CTAN (**Philip Kime**). Pandoc's manual (**John MacFarlane**) is the authority on its citation pipeline. JATS (`jats.nlm.nih.gov`). FORCE11.

**People and projects**: **Ivan Oransky and Adam Marcus** (Retraction Watch). **Daniel S. Katz and Arfon Smith** (software citation); **Stephan Druskat** (CFF). **Silvio Peroni and David Shotton** (OpenCitations, CiTO). **Kieran Healy**, "The Plain Person's Guide to Plain Text Social Science" -- the canonical argument for the docs-as-code bibliography workflow. **Robert Bringhurst**, *The Elements of Typographic Style*, for the layout half (hanging indents, en dashes in ranges, small caps).

---

## 11. Severity rubric (domain-specific)

- **blocker** -- a citation that resolves to the wrong source or nothing: a corrupted or truncated identifier, a lowercased-and-stored DOI used as a key, an ORCID in an integer column, a retracted reference cited as current in a biomedical pipeline (an explicit ICMJE violation). Also: an unresolved-citation path that ships `[?]` into a published artifact, and redaction-style data loss where `%` silently truncates entries.
- **major** -- output that is systematically wrong for the declared style: hardcoded thresholds, hand-rolled disambiguation, APA-6 behavior in an APA-7 document, 17th-edition Chicago output claimed as 18th, an unpinned CSL style or unset `lang` in a build that claims reproducibility, title-case corruption of technical terms, a conformance claim (a named style) with no validation.
- **minor** -- correct but fragile: an unversioned arXiv or concept DOI, `link-bibliography` left on for print, `.bib` as a merge-conflict surface, dedup on citekey, missing ISSN-L handling.
- **nit** -- en dash vs hyphen in ranges, hyphenation of stored ISBNs, field ordering in a generated `.bib`.
- **insight** -- a structural reframing: the project is in the wrong casing culture; the `.bib` should be a generated artifact rather than a source file; this document's style is a house style and cannot be satisfied from the CSL repo; the substrate choice (CSL vs biblatex) does not match the output targets.

**Confidence calibration**: identifier and threshold findings are checkable and should be high-confidence. Style-conformance findings depend on the declared style -- if no style is declared, say so and make that the finding rather than assuming one.

---

## Changelog

- **2026-09-09** -- Initial file. Notable current-state facts established against primary sources: Chicago is on its 18th ed. (2024); Bluebook 22nd; CSL spec frozen at 1.0.2 (2020-09-06) while the styles repo changes daily; **citeproc-rs archived 2026-08-13 without ever shipping**; biblatex 3.22 / biber 2.22 both 2026-08-13 (lockstep); DataCite 4.7; CFF 1.2.0 stable since 2021. Verified from primary sources during writing: `latexmk -Werror` is scoped to citation/reference warnings and fixed-point aware (latexmk manual, 9 March 2026), making it superior to log-grepping; `pandoc --fail-if-warnings` does catch unresolved citations but is indiscriminate. Open gaps recorded in the research notes: APA's Sept 2025 AI guidance (bot-blocked), CMOS 18 exact thresholds (paywalled), Turabian's current edition (sources conflict), Zotero 8 citation-key mechanics.
