---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# PDF (Portable Document Format)

A reference for reviewing and advising on code that produces, mutates, renders, extracts from, signs, or validates PDF documents. Used by the `pdf` subagent and the `/expert-review` / `/expert-plan` / `/expert-consult` skills. Cross-language, cross-library; the format is the constant, the libraries vary.

The unifying thesis: **a PDF is a structured object graph serialized into a byte stream with a fragile, append-friendly physical layout, wrapped in a 1000-page ISO specification that almost no library implements completely.** Most PDF bugs are not algorithmic. They are (1) **structural** -- a broken cross-reference table, a dangling object reference, an incremental update that corrupts the file; (2) **conformance** -- a file that opens fine in Acrobat but fails PDF/A archival validation, fails PDF/UA accessibility, or won't pass a print RIP; (3) **fidelity** -- fonts not embedded, color spaces wrong, text that can't be extracted or searched; or (4) **security** -- JavaScript / launch actions / signature-wrapping attacks. The agent's operational question: **"is this PDF structurally valid, conformant to the standard it claims, faithful to its content, and safe to open?"**

The empirical priority, in rough order of how often it bites: structural integrity > font/text fidelity > conformance to the claimed subset (PDF/A, PDF/UA, PDF/X) > rendering correctness > security. Pattern-match these aggressively before reaching for cleverness.

---

## 1. The specification landscape (know which document governs)

The single most common source of confusion: **"the PDF spec" is not one document.** There is a core spec and a family of ISO conformance subsets, each governing a different concern. A finding is only meaningful relative to the standard the code claims to target.

### Core specification: ISO 32000

- **ISO 32000-1:2008** -- the first ISO PDF standard, derived verbatim from **Adobe PDF Reference 1.7**. This is the moment Adobe handed stewardship of the core format to ISO. Most "PDF" knowledge in the wild describes this.
- **ISO 32000-2:2020** -- **PDF 2.0**. The current core standard. (A 2017 first edition was superseded by the 2020 dated revision with errata folded in.) Adds: explicit Unicode handling improvements, modernized digital-signature support, page-level output intents, richer annotations, document-part metadata, deprecation of proprietary Adobe-isms.
- **PDF 2.0 is freely available** from the PDF Association (sponsored by Adobe, Apryse, Foxit since April 2023): https://pdfa.org/resource/iso-32000-2/ . The companion technical specifications ISO/TS 32001-32004 ride along. **There is no excuse for guessing at PDF 2.0 syntax** -- the authoritative document is free. Cite it.
- The historical **Adobe "PDF Reference"** editions (up to 1.7) are still floating around and still largely accurate for the structural core, but for anything version-sensitive, ISO 32000-2 is the truth.

**Review reflex**: when code emits a `/Version` or sets the header (`%PDF-1.7`, `%PDF-2.0`), check that the features it uses match the declared version. Object streams and xref streams require 1.5+. Many PDF 2.0 features fail silently in 1.x-targeting validators.

### PDF/A -- archival (ISO 19005)

The "this file must still open and look identical in 50 years" subset. The defining constraint: **self-containment and determinism.** No external dependencies, no JavaScript, no encryption (in older parts), no unembedded fonts, no device-dependent color, mandatory XMP metadata.

- **PDF/A-1** (ISO 19005-1:2005, based on PDF 1.4): levels **1a** (accessible / tagged) and **1b** (basic visual reproduction). Most restrictive; no transparency, no layers.
- **PDF/A-2** (ISO 19005-2:2011, based on ISO 32000-1): levels **2a / 2b / 2u** (the **u** level = 2b plus Unicode mappings for all text). Adds transparency, layers, JPEG 2000, PDF/A file embedding.
- **PDF/A-3** (ISO 19005-3:2012): identical to PDF/A-2 **except it permits embedding arbitrary (non-PDF/A) file types** as attachments. This is the basis of hybrid invoice formats like **ZUGFeRD / Factur-X** (a human-readable PDF/A-3 with a machine-readable XML invoice embedded).
- **PDF/A-4** (ISO 19005-4:2020, based on **PDF 2.0**): the modern part. **Drops the a/b/u distinction** -- every PDF/A-4 file requires Unicode mappings and full transparency support is allowed. Two optional conformance levels replace the old letters: **PDF/A-4f** (permits non-PDF/A embedded files, the -3 analogue) and **PDF/A-4e** (engineering: 3D / U3D / PRC / rich media, the successor to PDF/E).

**Review reflexes for PDF/A:**
- All fonts **must** be embedded (including the "standard 14" -- PDF/A does not exempt them).
- An **OutputIntent** with an ICC color profile is mandatory; device-dependent color (raw `DeviceRGB`/`DeviceCMYK` without an output intent) is a violation in the strict reading.
- **XMP metadata** is mandatory and must declare the PDF/A part and conformance level (`pdfaid:part` / `pdfaid:conformance`).
- No JavaScript, no `/Launch` actions, no embedded multimedia (except where a part allows), no reliance on external content.
- Encryption is prohibited in PDF/A-1/2/3; PDF/A-4 relaxes some rules but the no-JS / self-containment ethos holds.
- For the **a** levels (1a/2a/3a), the file must also be **tagged** (logical structure tree) -- effectively PDF/UA-adjacent.

### PDF/UA -- universal accessibility (ISO 14289)

The "a screen reader and assistive tech can faithfully convey this document" subset. Built entirely on **tagged PDF**: a logical structure tree (`/StructTreeRoot`) of semantic elements (`/Document`, `/H1`...`/H6`, `/P`, `/Table`, `/Figure`, `/Link`, ...) mapped to the visual content via marked-content sequences.

- **PDF/UA-1** (ISO 14289-1, based on ISO 32000-1): the original. Defines requirements for tagging, reading order, alternative text on figures, language specification, no reliance on color alone, proper table structure, artifact marking of decorative content.
- **PDF/UA-2** (ISO 14289-2:2024, based on **PDF 2.0**): published March 2024. Modernized for PDF 2.0's structure model. **Does not replace UA-1** -- they target different base specs.
- **WTPDF (Well-Tagged PDF) 1.0** (PDF Association, Feb 2024, **free**): practical tagging guidance that underpins PDF/UA-2 and content reuse. The PDF Association's framing: "if you support WTPDF, you also support PDF/UA-2." When advising on *how* to tag, WTPDF is the readable, free reference; ISO 14289-2 is the formal (paid) standard.

**Review reflexes for PDF/UA:**
- A tagged structure tree must exist and be **complete** -- every meaningful content item is tagged; all decorative content is marked as an `/Artifact`.
- **Reading order** in the structure tree must match the logical reading order, independent of the visual layout order on the page.
- Every `/Figure` needs alternative text (`/Alt`). Every link needs a meaningful description.
- Document and per-element **language** must be set (`/Lang`).
- Headings form a proper hierarchy (no skipping levels in the strict reading).
- Tables use real table structure (`/Table` / `/TR` / `/TH` / `/TD`), not visually-faked grids.
- This is **not** the same as WCAG, but the two are deeply related. **A clean PDF/UA file is not automatically WCAG-conformant and vice versa.** For the general a11y catalog (contrast, focus, ARIA-equivalents), defer to the `accessibility` lens; the PDF lens owns *tagged-PDF structural correctness specifically.*

### PDF/X -- print production (ISO 15930)

The "a commercial printer's RIP will reproduce this exactly" subset. Constraints center on color and font determinism for print.

- **PDF/X-1a** (ISO 15930): CMYK / spot only, all fonts embedded, no transparency. Still the most-submitted format at many print shops.
- **PDF/X-4** (ISO 15930-7, based on PDF 1.6): permits **live transparency, layers, JPEG 2000**. The modern workhorse; what most "print-ready PDF" guidance means today.
- **PDF/X-6** (ISO 15930-9:2020, based on **PDF 2.0**): the PDF 2.0 successor. Adoption is **limited** because it requires PDF 2.0-aware RIPs, which are not yet ubiquitous. Don't push X-6 on a team whose print partner runs older equipment.

**Review reflexes for PDF/X:** all fonts embedded, color spaces are device-independent or have output intents, no RGB where CMYK is required, no transparency for X-1a, bleed / trim / media boxes set correctly.

### How to use the subset map in review

When code generates a PDF, **ask which subset (if any) it claims.** If it claims none, the only bar is "valid PDF that opens." If it claims PDF/A for archival, PDF/UA for accessibility (often legally required -- Section 508, EU EN 301 549, the European Accessibility Act), or PDF/X for print, the bar is dramatically higher and validator-checkable. **A claim of conformance with no validation step in the pipeline is itself a finding** -- see veraPDF below.

---

## 2. Physical and logical structure (where structural bugs live)

A PDF file has four conceptual sections, in file order: **header**, **body** (the objects), **cross-reference table**, **trailer**. Understanding the physical layout is what separates "the library produced bytes" from "the library produced a *valid* file."

### The object model

Eight basic object types: booleans, numbers, strings (literal `(...)` or hex `<...>`), names (`/Foo`), arrays (`[...]`), dictionaries (`<< /Key value >>`), streams (a dictionary + raw bytes between `stream`/`endstream`), and the null object. **Indirect objects** (`12 0 obj ... endobj`) are numbered and referenced by `12 0 R`. The whole document is a graph of these, rooted at the catalog.

### The cross-reference table (xref)

Maps each object number to its **byte offset** in the file. The classic form is a text table; **xref streams** (PDF 1.5+) are a compressed binary replacement that can itself live in an object stream. The xref is what lets a reader jump directly to any object without scanning the file.

**This is the single most fragile part of a PDF.** If a byte offset is wrong by even one byte, a strict reader rejects the file (lenient readers like Acrobat "repair" it by rescanning, which masks the bug -- a file that "works in Acrobat" can still have a broken xref). **Any code that writes raw PDF bytes and computes its own offsets is a prime suspect.** Off-by-one in offset math, forgetting that the offset is from the start of the file (not the start of the body), miscounting newline bytes (`\r\n` vs `\n`), or failing to update offsets after inserting an object -- these are classic, high-severity structural bugs.

### The trailer

Points to the catalog (`/Root`), the document `/ID`, the total object count (`/Size`), and -- critically for incremental updates -- `/Prev`, the byte offset of the previous xref section. A reader starts at the end of the file, reads the trailer, finds the xref, and walks back through `/Prev` chains.

### Object streams and compressed xref streams (PDF 1.5+)

**Object streams** pack many non-stream objects into a single compressed stream, dramatically shrinking files with lots of small objects (typical of tagged PDFs). They **require** an xref stream to index them (the classic text xref can't point inside an object stream). Mixing a classic xref with object streams is a structural error.

### Incremental update (incremental saving)

PDF is designed to be **appended to without rewriting.** To modify a file, you append new/changed objects, a new xref section listing them, and a new trailer whose `/Prev` points to the old xref. The original bytes are untouched. This is how signing works (the signature covers the original bytes; later changes append after it), how annotations and form fills are saved, and how many "edit a PDF" libraries operate.

**The double-edged sword:** incremental update preserves the *entire history* of the document in the file. Sensitive content "deleted" via an incremental update is **still physically present** in the earlier bytes and trivially recoverable. **Redaction by drawing a black box and saving incrementally is a data-leak bug, not redaction.** True redaction must *remove* the content and rewrite the file (a "sanitize" or "flatten" operation). This is a recurring, high-severity finding. It is also the mechanism abused by signature **shadow attacks** (see Security).

### Linearization ("Fast Web View")

Reorganizes the file so a reader can render the **first page before the whole file has downloaded** -- objects for page 1 come first, with a linearization dictionary and a special first-page xref. Useful for serving large PDFs over HTTP. **Incremental updates break linearization** (appended content lands after the linearized structure); a file that must stay linearized has to be fully rewritten, not appended. Code that linearizes and then incrementally updates has silently un-linearized the file.

### Content streams and the graphics model

A page's visible content is a **content stream**: a sequence of operators in a PostScript-derived imaging model. Text is drawn with `BT`/`ET` (begin/end text), font selection (`Tf`), positioning (`Td`/`Tm`), and showing (`Tj`/`TJ`). Graphics use path construction (`m`/`l`/`c`/`re`), painting (`S`/`f`/`B`), and graphics state (`q`/`Q` save/restore, `cm` transform, `gs` extended state). **The text in a content stream is glyph codes, not Unicode** -- which is why text extraction is a separate, hard problem (see Fonts).

---

## 3. Fonts and text fidelity (the second-most-common bug class)

**The deepest, most underestimated source of PDF bugs.** A PDF can render perfectly on the author's machine and fall apart everywhere else, or render fine and be completely un-extractable, all because of font handling.

### Embedding

- **Embed the font, or the file is not portable.** An unembedded font relies on the reader having an identical font installed; absent that, the reader substitutes, and metrics / glyphs shift. PDF/A and PDF/X **require** embedding (including the "standard 14" base fonts -- do not assume Helvetica/Times/Courier are safe to leave unembedded for archival/print).
- **Subsetting** embeds only the glyphs actually used, with a `ABCDEF+` tag prefix on the font name. Standard practice; shrinks files. The bug is incremental editing across subsetted fonts: adding a glyph not in the original subset requires re-subsetting or it renders as `.notdef` (tofu / blank).

### ToUnicode -- the extraction and accessibility lifeline

A content stream shows **glyph codes** mapped through the font's encoding, **not** Unicode code points. To make text **searchable, copyable, and extractable**, the font dictionary needs a **`/ToUnicode` CMap** mapping glyph codes back to Unicode. **Missing or wrong `/ToUnicode` is why "the PDF has text but I can't search/copy it, or copying gives garbage."** PDF/A-2u/3u and PDF/A-4 *require* ToUnicode for exactly this reason. Any generation code that embeds a subsetted font without a ToUnicode CMap has produced a visually-fine but text-broken file -- a major finding for any document meant to be searched, indexed, or read by assistive tech.

### Encoding and CID fonts

Simple fonts (1 byte per glyph, 256-glyph limit) suffice for Latin. Anything beyond -- CJK, complex scripts, large glyph repertoires -- needs **composite (Type 0 / CID) fonts** with a CIDFont descendant and a CMap. Code that assumes single-byte encoding silently breaks on non-Latin text. For shaping correctness (ligatures, combining marks, bidi, complex-script reordering) the layout happens *before* the glyphs reach the content stream -- defer the shaping engine itself (HarfBuzz / FreeType) to the `graphics-programming` lens, but the **PDF-side contract** (right CID font, right CMap, right ToUnicode) is this lens's.

**Review reflexes for text:**
- Generation: are all fonts embedded? Subsetted with ToUnicode? Type 0 / CID for non-Latin?
- Extraction: does the code handle missing ToUnicode (fall back gracefully, or at least surface that extracted text may be unreliable)? Does it reconstruct reading order, or just dump glyphs in content-stream order (which is *not* reading order)?
- Never assume extracted text positions imply reading order; columns, tables, and rotated text wreck naive top-to-bottom-left-to-right extraction.

---

## 4. Color, images, and rendering

- **Color spaces**: `DeviceGray` / `DeviceRGB` / `DeviceCMYK` (device-dependent), `CalRGB` / `Lab` / **ICCBased** (device-independent), plus `Indexed`, `Separation` (spot color), `DeviceN`, `Pattern`. PDF/A and PDF/X care intensely: device-dependent color without an **output intent** is a conformance violation. Print work that emits RGB where CMYK is required will color-shift on press.
- **Images**: embedded as image XObjects with a filter (`DCTDecode` = JPEG, `JPXDecode` = JPEG 2000, `CCITTFaxDecode` = fax/bilevel, `FlateDecode` = zlib, `JBIG2Decode` = bilevel). Wrong filter / colorspace / bit-depth combinations produce corrupt or color-wrong images. Oversized images (full-resolution photos at screen DPI) bloat files -- a common performance finding.
- **Transparency / blend modes / soft masks**: rich but version- and subset-gated. PDF/A-1 and PDF/X-1a forbid transparency; flattening is required for those targets. Live transparency in an X-1a-claiming file is a conformance bug.
- **Rendering engines** (when reviewing code that *displays* PDFs): the dominant engines are **pdfium** (Google's C++ engine, BSD, in Chrome and most native wrappers), **pdf.js** (Mozilla's JS engine, in Firefox, Apache-2.0, the standard browser-side renderer), **MuPDF** (Artifex, lightweight C), **Ghostscript** (Artifex, the PostScript/PDF interpreter, also used for conversion/rasterization), and the commercial **Apryse** (formerly PDFTron) / **Foxit** SDKs. **GPU-level rendering internals, glyph rasterization, and shader concerns are the `graphics-programming` lens, not this one** -- this lens owns "are you feeding the renderer a valid, faithful PDF and handling its output and failure modes correctly."

---

## 5. Digital signatures

PDF signatures are a frequent source of subtle, high-severity bugs because the security model is non-obvious and the failure modes are silent.

### The model

- A signature is an object in a signature field; the signed byte range is recorded in `/ByteRange`, and the signature itself (a PKCS#7 / CMS blob) lives in `/Contents`. **The signature covers the document bytes as they existed when signed; subsequent changes are appended via incremental update.**
- **Approval signatures**: ordinary signatures via a signature field. Multiple allowed; each covers the bytes up to its point.
- **Certification signatures (DocMDP -- Modification Detection and Prevention)**: exactly **one** per document, set by the author, declaring what later changes are permitted via the DocMDP transform `/P` value: `1` = no changes allowed, `2` = form fill-in and signing allowed, `3` = also annotations allowed. (Note: the parameter is **DocMDP**, not "DocMTK" -- a common misremember.)
- **Document timestamps (DTS)**: an RFC 3161 timestamp token embedded as a signature, attesting the document existed at a time; allowed even under DocMDP `/P = 1`.

### Standards

- **PAdES (PDF Advanced Electronic Signatures)** -- ETSI **EN 319 142** (Part 1 building blocks + baseline profiles; Part 2 extended). The European framework for legally-recognized e-signatures (eIDAS). PAdES layers CAdES-style signed/unsigned attributes onto PDF's native signature mechanism.
- ISO 32000-2 carries modernized native signature handling.

**Review reflexes for signatures:**
- Does verification check that the `/ByteRange` actually covers the *whole* file up to the signature, with no gap an attacker could fill? A `/ByteRange` that leaves a window is the core of several attacks.
- Does the code distinguish "signature is cryptographically valid" from "the signed content is what the user sees"? They are different questions (see shadow attacks).
- Is signing done as a *true incremental update* (preserving prior signatures) rather than a rewrite (which invalidates them)?
- Does it validate the certificate chain and trust anchor, not just the math?
- Long-term validation (LTV): are revocation data (CRL/OCSP) and timestamps embedded so the signature stays verifiable after the cert expires?

---

## 6. Security

PDF is a code-execution and data-exfiltration vector, not just a document format. The canonical risk categories:

1. **Embedded JavaScript** (`/JavaScript`, `/JS`, auto-run via `/OpenAction`): Acrobat/Reader's JS engine has a long, ugly CVE history. Generation code should not emit JS into documents meant for untrusted distribution; parsing/processing code should treat JS-bearing PDFs as suspect. PDF/A forbids JS entirely.
2. **Embedded files and launch / auto actions** (`/EmbeddedFile`, `/Launch`, `/OpenAction`, `/AA` additional-actions): used to drop or trigger payloads on open. A processing pipeline that opens untrusted PDFs should strip or refuse these.
3. **Parser memory-safety bugs**: native renderers (pdfium, MuPDF, Ghostscript) have had many memory-corruption CVEs; pdf.js has a smaller but real attack surface in JS. **Never parse untrusted PDFs in-process in a privileged context without a sandbox** -- this is a `security`-lens concern this lens flags and routes.
4. **Signature attacks**: notably the **"Shadow Attacks" (Hide / Replace / Hide-and-Replace)** by Mainka, Mladenov, Rohlmann et al. (Ruhr University Bochum, NDSS 2021). These use **well-formed, spec-compliant incremental updates** to change what a signed document displays *after* signing while the signature still validates -- they exploit the format, not a viewer bug. Also the earlier "Breaking the Signature" / incremental-saving-attack line of work from the same group. Reference: https://www.ndss-symposium.org/ndss-paper/shadow-attacks-hiding-and-replacing-content-in-signed-pdfs/
5. **PDF as malware delivery**: phishing attachments, often combining the above.
6. **Redaction failures**: see the incremental-update warning in §2 -- "deleted" content physically remaining in the file. A confidentiality breach.

The PDF Association publishes signature-validation guidance: https://pdfa.org/wp-content/uploads/2020/07/2020-10-07_PDF-Signature-Validation_comp.pdf . For the general threat model (sandboxing, trust boundaries, supply chain), defer to the `security` lens; this lens owns the *PDF-specific* attack surface and names which `security` concern applies.

---

## 7. The library and tool ecosystem (vendor peculiarities)

The format is the constant; libraries differ wildly in completeness, fidelity, license, and idiom. Reviewing PDF code means knowing what the chosen tool can and can't do.

### Generation / manipulation libraries

- **iText** (Java + .NET; **iText Core 9**, 2024; Apryse-owned since 2021): the heavyweight commercial-grade generator/manipulator. **AGPLv3 + commercial dual license** -- this is a frequent licensing trap: AGPL means using iText in a network service can obligate you to release your source unless you hold a commercial license. **Flag AGPL-licensed PDF libraries in proprietary/SaaS contexts as a license finding.** Bruno Lowagie's *iText in Action* (Manning) is the canonical book.
- **Apache PDFBox** (Java; **3.x**, Apache-2.0): the open-source workhorse for create/manipulate/extract. Permissive license, no AGPL trap. Lower-level API than iText.
- **pdf-lib** (JS/TS; create **and** modify, runs anywhere -- browser/Node/Deno/RN, no native deps): popular but **upstream is lightly maintained** (single maintainer, intermittent). A more active community fork (`cantoo/pdf-lib`, adds encryption + SVG) exists. Flag heavy reliance on a feature that upstream pdf-lib lacks (e.g. encryption) when the team is on the upstream package.
- **pdfkit** (JS, **foliojs/pdfkit**): generation-only (no modify), active. **Disambiguate from the unrelated Python `pdfkit`**, which is a wkhtmltopdf wrapper -- and **wkhtmltopdf was archived in January 2023**, so new HTML-to-PDF work built on it is building on a dead dependency (route HTML-to-PDF to a headless-Chromium / Playwright / Puppeteer `page.pdf()` approach or a maintained engine instead).
- **reportlab** (Python): mature, actively maintained programmatic generation with full layout control. The default serious Python PDF generator.
- **QPDF** (C++ library + CLI; Apache-2.0; Jay Berkenbilt): **content-preserving structural transformation** -- linearization, encryption/decryption, splitting/merging, inspecting internals, repairing xref, JSON round-tripping of structure. The right tool for "manipulate the structure without touching page content." Excellent for diagnosing structural bugs.

### Rendering / conversion

- **pdfium** (C++, BSD, Google/Chromium): the dominant native open-source render engine; the basis of many language wrappers.
- **pdf.js** (JS, Apache-2.0, Mozilla): the standard browser-side renderer; in Firefox; `pdfjs-dist` on npm.
- **MuPDF / mutool** (C, Artifex, **AGPL + commercial**): lightweight renderer + toolkit; `mutool` is the CLI.
- **Ghostscript** (Artifex, **AGPL + commercial**): the PostScript/PDF interpreter; conversion, rasterization, PDF/A conversion (`-dPDFA`). Same AGPL license consideration as MuPDF.
- **Apryse** (formerly **PDFTron**, rebranded Feb 2023) and **Foxit**: the major commercial SDKs -- broadest fidelity and feature coverage, priced accordingly.

### Validation

- **veraPDF** (open-source, jointly led by the **Open Preservation Foundation** and the **PDF Association**): the **industry-reference conformance validator.** Covers **all PDF/A parts and levels** (1a/1b, 2a/2b/2u, 3a/3b/3u, 4/4e/4f), **PDF/UA-1, PDF/UA-2, and WTPDF 1.0**. https://verapdf.org/ . **If code claims PDF/A or PDF/UA conformance and the pipeline has no veraPDF (or equivalent) gate, that is a finding** -- conformance is machine-checkable and a claim without a check rots.

**Licensing as a first-class review concern.** The PDF tooling world is full of **AGPL + commercial** dual licenses (iText, Ghostscript, MuPDF). AGPL's network-use clause is a genuine obligation for SaaS. The permissive alternatives (PDFBox / Apache-2.0, pdfium / BSD, QPDF / Apache-2.0, pdf.js / Apache-2.0) exist for exactly this reason. When reviewing a dependency choice, **name the license and its implication for the deployment context** -- this is often the highest-leverage finding in a PDF PR and one general reviewers miss.

---

## 8. Anti-pattern catalog (pattern-match these)

- **Redaction by overlay.** Drawing a black rectangle (or white-on-white text) over content and saving. The content is still in the byte stream. True redaction removes content and rewrites. **Confidentiality breach -- blocker.**
- **Incremental update where a rewrite was needed** (and vice versa). Appending after linearization (un-linearizes). Appending to "remove" sensitive data (leaves it recoverable). Rewriting a signed document (invalidates signatures).
- **Unembedded fonts** in any portable/archival/print context. Renders differently elsewhere; fails PDF/A and PDF/X.
- **Missing `/ToUnicode`** on embedded subsetted fonts. Text is visible but un-searchable / un-copyable / inaccessible. Breaks PDF/A-2u+ and assistive tech.
- **Claiming a conformance subset with no validator in the pipeline.** PDF/A / PDF/UA / PDF/X claims are checkable; an unchecked claim is a future failed audit.
- **Hand-computed xref offsets.** Off-by-one, wrong newline accounting, offset-from-wrong-origin. Produces files that "work in Acrobat" (which silently repairs) but fail strict readers and validators.
- **Mixing classic xref with object streams.** Object streams require xref streams.
- **Assuming single-byte encoding / Latin-only text.** Breaks on CJK and complex scripts; needs Type 0 / CID fonts.
- **Device-dependent color without an output intent** in PDF/A or PDF/X. Conformance violation; color shifts on press.
- **Treating content-stream order as reading order** in extraction. Columns, tables, rotated and absolutely-positioned text defeat naive extraction.
- **Parsing untrusted PDFs in-process, unsandboxed.** Memory-safety CVE surface (pdfium / MuPDF / Ghostscript). Route to `security`.
- **Emitting `/JavaScript` / `/Launch` / `/OpenAction` into distributed documents.** Attack surface; forbidden in PDF/A.
- **AGPL PDF library in a proprietary SaaS without a commercial license.** License obligation. (iText, Ghostscript, MuPDF.)
- **Building new HTML-to-PDF on wkhtmltopdf.** Archived January 2023; dead dependency.
- **Verifying a signature's math but not its `/ByteRange` coverage.** Shadow-attack / wrapping surface. The crypto can be valid while the displayed content is not what was signed.
- **Tagging that fakes structure visually** (a "table" that's just positioned text, headings that are just big bold `/P`s). Fails PDF/UA; useless to screen readers.
- **Loading an entire large PDF into memory to extract one page.** Use a streaming / random-access API (QPDF, PDFBox's random-access, pdfium page-by-page). Performance finding.

---

## 9. Authorities and canonical sources

- **The PDF Association** (pdfa.org) -- vendor-neutral non-profit; **administers ISO TC 171 SC 2** (the committee that owns the PDF standards) on behalf of ANSI as an ISO Category A liaison. The single best free resource: hosts the free PDF 2.0, WTPDF, and extensive practitioner guidance.
- **Duff Johnson** -- CEO of the PDF Association; **Project co-Leader for ISO 32000**; Project Leader for ISO 14289 (PDF/UA). The authority on the standards process and accessibility.
- **Leonard Rosenthol** -- Adobe Senior Principal Architect for PDF; ISO Project Editor for PDF/A, PDF/X, PDF/E; author of the PAdES signature standard at ETSI; author of *Developing with PDF* (O'Reilly, 2013). (He is a key Adobe editor/contributor on 32000; the 32000 *co-leader* title is Johnson's -- attribute carefully.)
- **Bruno Lowagie** -- founder of iText; author of *iText in Action* (Manning); long-time ISO PDF committee member.
- **Jay Berkenbilt** -- author of **QPDF**; deep authority on PDF structure and content-preserving transformation.
- **Adobe "PDF Reference"** -- the pre-ISO spec; version **1.7 became ISO 32000-1:2008**, after which Adobe ceded core stewardship to ISO.
- **Ruhr University Bochum / CASA group** (Mainka, Mladenov, Rohlmann, et al.) -- the academic source for PDF signature and encryption attacks (Shadow Attacks NDSS 2021, "Breaking the Signature," PDF encryption attacks).
- **ISO 32000-2:2020** (free at pdfa.org), **ISO 19005** (PDF/A), **ISO 14289** (PDF/UA), **ISO 15930** (PDF/X), **ETSI EN 319 142** (PAdES) -- the formal standards.

---

## 10. Severity and confidence (for the subagent's findings)

Following the panel contract (`~/.claude/rules/panel-contract.md`):

- **blocker**: redaction-by-overlay leaving sensitive content recoverable; signature verification that doesn't validate `/ByteRange` coverage; a file that claims PDF/A but is structurally invalid in a way that fails ingestion; emitting auto-run JavaScript into untrusted distribution; xref corruption that fails strict readers; AGPL library in a shipping proprietary SaaS with no commercial license.
- **major**: unembedded fonts in an archival/print/portable context; missing `/ToUnicode` on a document meant to be searched or read by assistive tech; claiming a conformance subset with no validation gate; faked tag structure failing PDF/UA; device-dependent color without output intent in a PDF/X/A target; parsing untrusted PDFs unsandboxed.
- **minor**: oversized embedded images; un-subsetted fonts bloating files; incremental update where a rewrite would be cleaner (no correctness impact); extraction that ignores reading-order reconstruction where it matters.
- **nit**: declared `/Version` higher than features used; cosmetic metadata gaps; non-canonical (but valid) object layout.
- **insight**: "this generation pipeline would benefit from a veraPDF gate in CI"; "consider PDF/A-3 + embedded XML (Factur-X) for this invoice use case"; "QPDF would diagnose this structural corruption faster than reading bytes by hand"; "the choice of an AGPL engine here will constrain the deployment model -- worth deciding deliberately."

Confidence: **high** when verifiable from the code or output (a missing ToUnicode dict, an absent OutputIntent, an AGPL import, a black-rectangle redaction); **medium** when it depends on the document's intended use (whether searchability/accessibility/archival matters here), which the agent should name as the conditioning assumption.

---

## 11. What is NOT a PDF-lens finding

- **GPU rendering internals, glyph rasterization, shader/tessellation concerns** -- `graphics-programming`.
- **The general accessibility catalog** (contrast ratios, focus order in apps, ARIA) -- `accessibility`. (Tagged-PDF *structural* correctness is this lens.)
- **Text encoding as a locale/i18n concern** (collation, normalization, bidi rendering policy) -- `i18n`. (The PDF-side CID-font/CMap/ToUnicode contract is this lens.)
- **The general threat model** (sandboxing strategy, secret handling, dependency supply chain) -- `security`. (The PDF-specific attack surface and which security concern applies is this lens.)
- **General performance unrelated to PDF structure** -- `performance`. (PDF-specific perf -- streaming vs full-load, image bloat, object-stream compression -- is this lens.)
- **Doc-comment / README / changelog quality of the surrounding code** -- `documentation`.
- **Stylistic choices in the host language** that don't affect the PDF output -- the language lens.
