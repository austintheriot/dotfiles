---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Text engineering

A deep reference for the text stack, encoding through rendering. Used by the `text-engineering` subagent. Cross-language, cross-platform.

The unifying thesis: **text is a pipeline, not a string.** Bytes become code units become code points become grapheme clusters become shaped glyphs become pixels. Each stage has a different unit, a different length, a different notion of "a character," and a different correctness contract. Almost every text bug is the same mistake: code that assumes two adjacent stages are the same stage. `name.length` (code units) used as a display width (grapheme clusters or cells). A `substring` (code-unit offset) used to truncate user-perceived characters. A `cmap` lookup (code point → default glyph) mistaken for shaping (code points → positioned glyphs). A byte-sort mistaken for a collation.

The agent's operational question at every text site: **which stage of the pipeline is this, what unit does it actually operate in, and does the code respect that unit?**

Version pin: facts here are current as of **Unicode 17.0 (released 2025-09-09)**. Pin a specific version in any finding rather than saying "latest" — the standard ships annually and counts/blocks change.

---

## 0. Boundary with adjacent lenses

This lens shares territory with three others. The split is altitude and pipeline-stage, not topic.

- **`i18n`** owns text as a **locale-correctness** concern: is this the right locale, is text going through translation, are CLDR plural categories used, is `Accept-Language` plumbed, is the Turkish-I login bug present, are dates/numbers/currencies locale-formatted. **`text-engineering` owns the machinery underneath**: is the normalization implementation correct (Hangul algorithmic composition), does segmentation follow UAX #29 extended grapheme clusters, is `wcwidth` right for East Asian Width, is the code storing Arabic presentation forms instead of base characters. Same topic (normalization, bidi, collation), different question (policy vs. implementation depth). When a finding is "you should normalize here" → that's shared; when it's "your NFC is wrong because it doesn't reorder by canonical combining class" → that's this lens.
- **`graphics-programming`** owns **glyphs → pixels on the GPU**: SDF/MSDF atlases, batched instanced quads, shader rasterization, WebGPU text, atlas eviction. **`text-engineering` owns everything before the rasterizer**: cmap/GSUB/GPOS, shaping engines, font fallback, the shaping-vs-rendering distinction itself. Handoff point: "you have positioned glyphs; now draw them" → `graphics-programming`.
- **`pdf`** owns the `/ToUnicode` CMap and CID-font contract **as a PDF-structural concern** (searchable/extractable text, PDF/A-2u). **`text-engineering` owns OpenType internals generally** (the tables, shaping, variable fonts) independent of PDF.

When a finding lands mostly in another lens, name it and route: `See also: i18n` / `See also: graphics-programming` / `See also: security` (Trojan Source, Shift-JIS injection) / `See also: pdf`.

---

## 1. Encoding and the four lengths

### The current standard
**Unicode 17.0**, released 2025-09-09; total **159,801 encoded characters**, 172 scripts. The code space is **1,114,112 code points** (17 planes × 65,536), of which 2,048 are surrogate code points (U+D800–U+DFFF) and 66 are permanent noncharacters. Roughly 14% assigned. A **Unicode scalar value** is any code point except a surrogate; that is the set Rust's `char` and most "valid character" definitions use.

### Encodings and their quirks
- **UTF-8**: 1–4 bytes/code point, ASCII-compatible, self-synchronizing. Designed by Ken Thompson (Sept 1992, to Rob Pike's criteria) for Plan 9. **RFC 3629** (2003) caps it at U+10FFFF / 4 bytes and forbids surrogates. **W3Techs reports ~99% of websites use UTF-8.** The "UTF-8 Everywhere" manifesto (Radzivilovsky, Galka, Novgorodov; utf8everywhere.org) is the canonical argument for UTF-8 as the single internal encoding, UTF-16 only at legacy API boundaries.
- **UTF-16**: 2 or 4 bytes; code points above U+FFFF use **surrogate pairs** (high U+D800–U+DBFF + low U+DC00–U+DFFF). This is why JS/Java/C#/Windows string lengths overcount astral characters.
- **CESU-8** (UTR #26): encodes supplementary chars as two 3-byte surrogate sequences (six bytes). Shows up in some Oracle/MySQL and Java-serialization contexts. **WTF-8** (Simon Sapin): a UTF-8 superset that tolerates lone surrogates; Rust's std uses it internally for Windows OS strings.
- **BOM** (U+FEFF): meaningful for UTF-16/32 endianness; **for UTF-8 it is neither required nor recommended** and breaks shebang lines, concatenation, and leading-whitespace-sensitive parsers. A UTF-8 BOM in a source file or config is usually a bug.
- **Legacy CJK encodings**: Shift-JIS / CP932 (Japanese), EUC-JP, EUC-KR, GB2312/GBK (Simplified Chinese), Big5 (Traditional Chinese). **GB18030** is the PRC national standard, **mandatory since 2023-08-01 (GB 18030-2022)**; it is a full Unicode transformation format (encodes all 17 planes) and round-trips with UTF-8. "Chinese = UTF-8 only" can be a compliance gap for PRC-market products.
- **Mojibake**: text decoded with the wrong charset (Japanese 文字化け). The signature of a missing or wrong charset declaration at a boundary.

### The four lengths
Bytes ⊆ code units ⊆ code points ⊆ grapheme clusters, conceptually. Same string, four different counts:

| Language | `.length` / `len` / `count` returns | Correct "user-perceived char" path |
|---|---|---|
| JavaScript | UTF-16 **code units** (`"😄".length === 2`) | `Intl.Segmenter(locale,{granularity:'grapheme'})` |
| Swift | **grapheme clusters** (`Character`) — correct default | `String.count` is already right |
| Python 3 | **code points** | grapheme libs (e.g. `grapheme`, `regex \X`) |
| Go | **bytes** (`len("日本語") == 9`) | `uniseg` package |
| Rust | `str::len()` = **bytes**; `.chars().count()` = code points | `unicode-segmentation` crate |

😄 is 1 grapheme / 1 code point / 2 UTF-16 units / 4 UTF-8 bytes. A ZWJ family emoji or a flag is 1 grapheme but many code points. **The bug class**: any `name.length`, `name[0]`, `name.slice(0,N)`, `name.substring(...)`, max-length input validation, or "truncate to N characters" that runs in the wrong unit. Slicing on a non-character boundary produces mojibake (most languages) or a panic (Rust on a non-char-boundary byte index).

**Flag**: code-unit/byte length used as a count of characters or a display width; indexing into user text by integer offset; truncation/ellipsis without grapheme awareness; `charCodeAt` where `codePointAt` is needed; regex `.` or `[0-9]` assumed to match one user-perceived character or only ASCII digits.

---

## 2. The Unicode algorithms

These are the specs the agent is expected to know by number and to recognize hand-rolled (usually wrong) reimplementations of.

### Normalization — UAX #15
Four forms: **NFD** (canonical decomposition), **NFC** (decompose then canonically compose), **NFKD** / **NFKC** (compatibility decomposition, the "K" forms, which collapse formatting distinctions — ligature ﬁ → fi, fullwidth Ａ → A, ¼ → 1⁄4 — and are **lossy**). Canonical equivalence = same character, same appearance (é as U+00E9 vs U+0065 U+0301). Combining marks are reordered by **Canonical_Combining_Class**; a correct implementation must do that reordering, not just substitute.

- **Hangul** composition/decomposition is **algorithmic, not table-driven**: constants `SBase=0xAC00, LBase=0x1100, VBase=0x1161, TBase=0x11A7, LCount=19, VCount=21, TCount=28, NCount=588, SCount=11172`. A normalizer that table-lookups Latin diacritics but misses the Hangul arithmetic is incomplete.
- **Policy**: NFC for interchange and storage (W3C recommendation); normalize at boundaries (input, storage, pre-comparison). The canonical bug is comparing/hashing/deduping/DB-looking-up strings in mixed forms: NFC input fails to match NFD-stored data though they look identical. **macOS HFS+** stored filenames in an Apple-specific variant of NFD (per Apple TN1150 — a *variant*, not strict UAX #15 NFD), which broke Git/Dropbox/rsync cross-platform with phantom duplicates; APFS (2017) switched to normalization-insensitive "bag of bytes." Git's `core.precomposeUnicode` is the mitigation.

**Flag**: equality/dedup/index/hash on user text without a documented normalization step; NFKC/NFKD used where information must be preserved (it's lossy); a hand-rolled normalizer that ignores combining class or Hangul.

### Segmentation — UAX #29
Defines **grapheme cluster**, **word**, and **sentence** boundaries. **Extended grapheme clusters** are the recommended "user-perceived character" unit (base + marks + ZWJ sequences); legacy clusters are the older narrower form. Rules GB11/GB12/GB13 keep emoji ZWJ sequences, modifier sequences, and regional-indicator (flag) pairs together. Emoji data is **UTS #51** (ZWJ = U+200D; VS16 U+FE0F = emoji presentation, VS15 U+FE0E = text; skin-tone modifiers U+1F3FB–U+1F3FF; regional indicators U+1F1E6–U+1F1FF). UTS #51 guarantees no grapheme boundary falls inside an emoji sequence.

Implementations to recognize as correct (don't flag as reinvention): `Intl.Segmenter` (JS), ICU `BreakIterator`, the `unicode-segmentation` crate (Rust), `uniseg` (Go), `grapheme` (Python). **Flag**: a hand-rolled grapheme counter/splitter (especially one that treats one code point as one character); truncation that severs a ZWJ sequence, surrogate pair, or base+combining-mark, producing stray ZWJs, lone surrogates, or orphaned skin-tone swatches.

### Line breaking — UAX #14
Distinguishes **mandatory** breaks (must break) from **break opportunities** (may break; higher-level software picks actual breaks by width). Dozens of property classes (BK, CR, LF, SP, CM, GL, ID, CL/CP, OP, NS, CJ, …). Why `split(' ')` / `split(/\s+/)` is wrong for wrapping:
- **CJK** has no inter-word spaces; breaks fall *between ideographs* (class ID), constrained by punctuation rules (don't break before closing brackets/sentence punctuation, don't break after opening ones).
- **Thai, Lao, Khmer, Burmese** have no spaces at all and need **dictionary/ML word segmentation** — UAX #14 explicitly defers these to "morphological analysis." ICU ships dictionary break iterators; ICU4X uses ML models. A whitespace-only wrapper finds no break points in a long Thai run and overflows or breaks arbitrarily (orphaning a vowel/tone mark).

**Flag**: line-wrapping or word-count by splitting on whitespace for text that may be CJK/Thai/Lao/Khmer/Burmese; `word-break: break-all` applied globally (mangles CJK punctuation rules — see kinsoku below).

### Bidirectional text — UAX #9 (the UBA)
Maps **logical (storage) order** to **visual (display) order** for mixed LTR/RTL text. Embedding levels (even = LTR, odd = RTL; max_depth 125). Explicit formatting characters: legacy embeddings/overrides **LRE/RLE/PDF/LRO/RLO** (U+202A–U+202E), and the **isolates** introduced in Unicode 6.3 — **LRI U+2066, RLI U+2067, FSI U+2068, PDI U+2069** — which the spec recommends over embeddings in new content because embeddings "have too strong an effect on their surroundings." Marks: LRM U+200E, RLM U+200F, ALM U+061C.

- **The classic production bug**: interpolating a user-supplied unknown-direction value into a template (`"Order " + name + " #" + id`) without isolation; the RTL run reorders the surrounding neutrals and numbers, scrambling the visual order. Data-dependent and intermittent. **Fix**: wrap interpolated fields in FSI…PDI (U+2068…U+2069), or use HTML `<bdi>` / `dir="auto"`. Never "reverse the string to make it RTL."
- **Security crossover — Trojan Source** (CVE-2021-42574; Boucher & Anderson, Cambridge, 2021): bidi controls in source comments/literals make the rendered order differ from the compiler's byte order, hiding logic from reviewers. PoCs across C/C++/C#/JS/Java/Rust/Go/Python/SQL/Bash. → `See also: security` (don't duplicate at full depth; name the text-side mechanism, route the threat model).

**Flag**: user-controlled text concatenated into a directional template without isolation; unexpected bidi control characters in source or logs.

### Case folding and collation
- **Case mapping** (toUpper/toLower/toTitle) is for **display** and is **locale- and context-sensitive**. **Case folding** (CaseFolding.txt) is for **caseless matching** and is mostly locale-independent — fold both sides, then binary-compare; never store or display folded text. The famous edge cases: **Turkish dotless-i** (`i`↔`İ`, `I`↔`ı` differ by locale — applying English rules corrupts Turkish identifiers; the canonical login bug), **German ß → "SS"** (1:2 length-changing; capital ẞ U+1E9E added 2008), **Greek final sigma** (ς word-final vs σ elsewhere; both uppercase to Σ, so lowercasing Σ is position-dependent).
- **Collation — UTS #10 (UCA)**: linguistic sort via **DUCET** plus multi-level keys (primary = base letters, secondary = accents, tertiary = case, quaternary = punctuation). **Byte/code-point sort is wrong**: it sorts all uppercase before all lowercase, scatters accented letters, ignores language order. **CLDR tailoring** (current CLDR 48, 2025) overrides DUCET per language: Swedish sorts å/ä/ö after z, German has phonebook vs dictionary orders, etc.

**Flag**: `toLowerCase()`/`toUpperCase()` without a locale for security-sensitive comparison (the Turkish-I bug); case-insensitive matching via case mapping rather than case folding; byte/`ORDER BY` sort presented to users as alphabetical for non-ASCII; storing folded or case-mapped text as canonical.

---

## 3. Fonts and OpenType

### Formats and the sfnt wrapper
**OpenType** (Microsoft + Adobe, 1996; published as ISO/IEC 14496-22 "Open Font Format") wraps **both** TrueType (`glyf`, quadratic Béziers) and PostScript/CFF (cubic Béziers) outlines in one **sfnt** container. `sfntVersion` 0x00010000 = TrueType outlines, 'OTTO' = CFF. **WOFF/WOFF2** are W3C web-font packagings of sfnt — WOFF2 uses Brotli plus a `glyf` transform and is the modern web default. PostScript **Type 1** is end-of-authoring-support in Adobe apps (2023) but still displays.

**Required tables** (8): `cmap`, `head`, `hhea`, `hmtx`, `maxp`, `name`, `OS/2`, `post`. **Outlines**: `glyf`+`loca` *or* `CFF`/`CFF2`, never both. **Layout**: `GSUB`, `GPOS`, `GDEF`, `BASE`, `JSTF`, `MATH`; legacy `kern`. **Vertical**: `vhea`/`vmtx`/`VORG`. **Variations**: `fvar` (axes + named instances), `gvar` (TrueType outline deltas), `STAT` (required in variable fonts), `HVAR`/`MVAR`/`avar`.

### The cmap table and the character-vs-glyph distinction
`cmap` maps a **character code to a single default glyph ID**; unmapped → glyph 0 (`.notdef`). Standard subtables: format 4 (BMP), format 12 (supplementary planes), format 14 (variation sequences). **The load-bearing fact** (straight from the OpenType GSUB spec): cmap maps *one character to one default glyph*. It **cannot** express many-characters-to-one-glyph (ligatures f+i → ﬁ) or one-character-to-many-glyphs (Arabic positional forms, stylistic alternates). All of that lives in **GSUB**. So "look up each code point in cmap and place the glyphs left to right" is structurally incapable of rendering real text.

### Variable fonts and color fonts
- **Variable fonts** (OpenType 1.8, 2016): `fvar` defines an N-dimensional design space; registered axes are **wght** (1–1000, default 400), **wdth** (%, 100), **slnt** (degrees, right-lean is *negative*), **ital** (0–1), **opsz** (points). Custom axis tags are uppercase; registered tags lowercase. Named instances are labeled coordinate tuples.
- **Color fonts** — four competing formats: **COLR/CPAL** (layered outlines + palettes; v1 adds gradients/transforms/compositing — Microsoft Segoe UI Emoji, modern Noto Color Emoji), **sbix** (Apple raster strikes — Apple Color Emoji), **CBDT/CBLC** (Google bitmap), **SVG** (Adobe/Mozilla, y-down coordinates). A renderer/pipeline that supports only one will tofu the others' emoji.

**Flag**: code that maps code points to glyphs via cmap and renders left-to-right with no shaping step (see §4); assuming a font covers all of Unicode (a single OpenType font holds ≤ 65,535 glyphs); hardcoding stylistic/positional glyph selection instead of OpenType features.

---

## 4. Shaping and rendering

### Shaping vs. rendering — the central distinction
**Shaping** = (code points + font + script/language/feature selectors) → a buffer of **positioned glyphs** (each with glyph ID, x/y advance, x/y offset, and a `cluster` index back to source). **Rendering/rasterization** = glyphs → pixels. These are different stages owned by different libraries; HarfBuzz shapes and does *not* rasterize, FreeType rasterizes.

Why you cannot skip shaping: glyph IDs don't correlate to code points (consult cmap); ligatures collapse many code points to one glyph; contextual substitution picks Arabic positional forms; mark positions depend on the preceding glyph's shape; complex scripts reorder. **GSUB** does substitution (ligatures `liga`/`dlig`, contextual alternates `calt`, Arabic forms `init`/`medi`/`fina`/`isol`, required ligatures `rlig`, small caps); **GPOS** does positioning (pair kerning type 2, cursive attachment, mark-to-base/mark/ligature).

### The shaping engines (recognize these as correct; flag their absence)
- **HarfBuzz** (Behdad Esfahbod) — the dominant open-source shaper; shapes most text on modern screens (Chrome, Firefox, Android, GNOME/KDE/Qt, LibreOffice, Flutter, XeTeX, Adobe apps). API `hb_buffer_*` / `hb_shape`. Implements Arabic/Indic/Mongolian joining, reordering, stacking, and the Universal Shaping Engine. Esfahbod is the single most important name here (he is HarfBuzz's author/maintainer; he was a contributor to fontconfig/Pango/FriBidi, not their creator).
- **Apple Core Text** (+ AAT tables `morx`/`kerx`), **Microsoft DirectWrite** (current; Uniscribe is legacy) — the platform shapers.
- **Rust ecosystem**: **rustybuzz** and the newer **harfrust** (HarfBuzz ports, under the harfbuzz org), **swash** (shaping+scaling+rendering), **allsorts** (from the Prince engine), **cosmic-text** (layout). `fontTools` (Python; Just van Rossum, Cosimo Lupo) for font manipulation/subsetting.

### Kerning, fallback, the tofu
- **Kerning**: modern fonts use **GPOS** (class-based) kerning; the legacy `kern` table is read **only if there is no `kern` feature in GPOS**. Applying both yields exact **double kerning** (the canonical stb_truetype bug). Naive advance-width-only placement leaves proportional pairs (AV, To) visually loose.
- **Font fallback**: no single font covers Unicode, so a fallback **cascade** (CoreText cascade lists, DirectWrite `IDWriteFontFallback`, fontconfig on Linux) tries successive fonts before showing the `.notdef` box — the **"tofu."** Google's **Noto** ("No Tofu") project aims to cover all scripts. **Flag**: a hardcoded single font for multilingual text with no fallback chain; tofu in CJK/emoji/RTL because the fallback stack is missing.
- GPU rasterization (SDF/MSDF/Slug/atlases/batching) is the **`graphics-programming`** lens — name the boundary and route.

**Flag**: hand-rolled shaping ("split on spaces, look up code points, place left-to-right"); RTL or complex-script text rendered without a shaping engine; double-kerning; missing font fallback.

---

## 5. Writing systems — what breaks and why

The agent must connect "this code does X" to "X breaks for script Y because Z." Concrete failures per system:

### CJK (Chinese, Japanese, Korean)
- **Han unification**: Chinese/Japanese/Korean Han ideographs share code points where they differ only in typeface. **A unified code point renders region-specific shapes**, so the correct glyph is a *font + locale* decision — supply `lang`/`zh-Hans`/`zh-Hant`/`ja`/`ko` and a matching font (or OpenType `locl`). The failure is not garbage; it's subtly *wrong-looking* glyphs a native reader perceives as foreign, with no character-level error to catch. CJK Unified Ideographs total **101,996** across the main block (BMP) and Extensions A–J (B–J are supplementary-plane, surrogate pairs in UTF-16). Ideographic Variation Sequences (base + selector from U+E0100–U+E01EF, governed by the IVD under UTS #37) pin specific name/legal glyph variants — stripping combining marks or truncating between base and selector silently changes the variant.
- **East Asian Width — UAX #11 — the terminal/console killer**: wide characters (CJK ideographs, kana, fullwidth forms) occupy **2 cells**, not 1. Using `str.length`/byte length/code-point count as a column count is off by ~2× per wide char: broken table/TUI alignment, wrong wrap/truncate math, and the classic **cursor desync** (emulator advances 2 cells, app assumes 1, the line corrupts). Use `wcwidth`/`wcswidth` (Markus Kuhn's reference) or an East-Asian-Width-aware width function. The **ambiguous-width class (A)** has *no* context-free answer (1 cell in Western context, 2 in legacy East-Asian) — two correct implementations legitimately disagree; terminals expose a config.
- **Halfwidth/Fullwidth Forms** (U+FF00–U+FFEF): `ＡＢＣ` ≠ `ABC` as code points but identical to a human. Username/coupon/search/dedup matching needs **NFKC** (compatibility) normalization — NFC is insufficient.
- **Chinese**: Simplified vs Traditional is a many-to-many mapping (一发 → 髮/發), so S↔T conversion is genuinely ambiguous and context-dependent, not a lookup; `zh` alone doesn't give the script (need `zh-Hans`/`zh-Hant`). No inter-word spaces → dictionary segmentation; W3C **CLReq** specifies layout. **GB18030-2022** compliance for PRC market.
- **Japanese**: four scripts mixed (kanji/hiragana/katakana/romaji) — can't detect language by script. **Kinsoku shori** line-break rules (JIS X 4051, W3C **JLReq**): closing brackets `）」` and sentence punctuation `、。`, small kana, long-vowel ー cannot *start* a line; opening brackets cannot *end* one — so naive char-count or `word-break: break-all` wrapping is instantly wrong. **Shift-JIS / CP932 trail-byte hazard**: a double-byte kanji's *second* byte can be 0x5C `\`, 0x7C `|`, 0x5B/5D `[`/`]` — byte-level scanning for escapes/path-separators/shell-metachars mistakes the trail byte for ASCII, historically causing **SQL/path injection and XSS**. Decode to a proper string type *before* any byte-level scan. (`See also: security`.) The 0x5C **yen-vs-backslash** display residue: JIS X 0201 put ¥ at 0x5C, so many Japanese fonts still display `\` as ¥. Ruby/furigana via `<ruby>`/`<rt>`/`<rp>` (the old `<rb>` is deprecated); naive tag-stripping interleaves base and gloss text.
- **Korean (Hangul)**: featural alphabet; syllable blocks compose from jamo (19 leading × 21 medial × 28 trailing). Unicode has **11,172 precomposed syllables** (U+AC00–U+D7A3, exactly 19×21×28) *and* conjoining Hangul Jamo (U+1100–U+11FF). **Normalization matters intensely**: "한글" is 2 code points (NFC) or 6 (NFD) — identical on screen, different bytes; equality/dedup/DB-lookup fail across forms (the HFS+ cross-platform filename disaster was largely a Korean/CJK NFD-vs-NFC problem). NFD truncation can sever a jamo from its block. Beware pasting **Compatibility Jamo** (U+3130–U+318F, non-conjoining) where conjoining jamo is expected — it won't compose.

### Arabic
Cursive script; letters encode **abstract forms, not shapes**. A shaper selects the **four positional forms** (isolated/initial/medial/final) from each letter's joining type. You **cannot** render Arabic by emitting code points LTR (wrong direction) or RTL one-glyph-per-code-point (disjoint isolated forms — looks broken). Correct = bidi (UAX #9) for order + shaping for glyph selection. **Lam-alef** is a *mandatory* ligature (`rlig`). Harakat (vowel marks) are stacking combining marks; text is often stored unvocalized. **Three digit families** with *different bidi classes*: ASCII 0–9 (EN), Arabic-Indic U+0660–U+0669 (AN), Extended/Persian U+06F0–U+06F9 (EN) — `parseInt`/`\d`/`[0-9]` won't handle the non-ASCII families, and the bidi-class difference makes them reorder differently. **Do not store Presentation Forms** (U+FB50–U+FDFF, U+FE70–U+FEFF) — they exist for legacy-codepage round-trip only; storing them breaks search/compare and is rewritten by NFKC. Persian/Urdu add letters and lean on ZWNJ; Urdu Nastaliq is among the hardest scripts to render.

### Hebrew
RTL. **Five final letter forms are SEPARATE code points** (kaf/mem/nun/pe/tsadi: e.g. U+05DB → U+05DA final) chosen at *input time* — unlike Arabic, **there is no contextual final-form shaping**. So expecting the renderer to auto-substitute produces a medial mem at word-end; and programmatic word-building / `endsWith` / search must treat מ and ם as related (normalize final↔non-final for indexing). Niqqud (vowel points) and cantillation are stacking combining marks. The **bidi interpolation bug** (RTL value in an LTR template scrambling numbers/neutrals) is the most common real Hebrew defect — fix with isolates (§2).

### Indic / Brahmic (Devanagari, Bengali, Tamil, Telugu, Kannada, Malayalam, Gujarati, Gurmukhi, Odia) — the hardest to render
Abugidas: consonants carry an inherent vowel; matras (vowel signs) change it; the **virama/halant** removes it. **The reordering problem**: a vowel sign stored *after* a consonant can render *before* (to the left of) it — KA (U+0915) + I-sign (U+093F) renders the i-matra to the LEFT of ka though it follows in memory. **Conjuncts**: consonant + virama + consonant → a single ligature via GSUB (`half`, `rphf`, `pref`/`blwf`/`pstf`, `cjct`). All four hard problems (cluster detection, reordering, ligature substitution, mark positioning) land in one cluster and are interdependent, which is why a generic LTR pipeline cannot render Indic and why these scripts need either the script-specific OpenType Indic model or the **Universal Shaping Engine (USE)** (Microsoft's property-driven generic model; HarfBuzz adopted it ~2015). Naive failures: caret/selection/backspace splitting a cluster; `indexOf` matching mid-cluster; truncation dropping a virama/matra (which *changes meaning*); per-code-point width math. Rule: operate on grapheme clusters; let HarfBuzz own glyph order.

### Thai / Lao / Khmer / Myanmar
No inter-word spaces → **dictionary/ML word segmentation** for line breaking and word ops (UAX #14 and #29 cannot segment spaceless text; ICU ships dictionary break iterators). Stacking marks: Thai stacks base + above-vowel + tone mark (Unicode warns canonical combining class *alone* is insufficient for correct Thai stacking); Khmer subscripts via the invisible **coeng** U+17D2 (grapheme splitters unaware of it wrongly split the cluster). **Myanmar — the Zawgyi disaster**: the non-conformant "Zawgyi" font/encoding became dominant, misusing the Myanmar block (U+1000–U+109F) and colliding with minority-language code points, so one Burmese word has multiple encodings and a Zawgyi stream read as Unicode is garbage. There is **no in-band marker** — you must statistically detect Zawgyi-vs-Unicode before rendering/storing (Google's `myanmar-tools` does this; Unicode tags Zawgyi as script `Qaag`).

### Other notable
- **Mongolian**: vertical top-to-bottom, columns left-to-right (opposite of CJK), *plus* Arabic-style cursive joining and Free Variation Selectors (FVS1–FVS4) — two hard problems at once; one of the worst-supported scripts (UTR #54 baseline; Unicode 13.0 even removed the contextual-form mapping pending model work).
- **CJK vertical text**: not "rotate the line 90°" — needs vertical glyph variants (OpenType `vert`/`vrt2`, with `vrt2` last and never combined with `vert`), `vmtx`/`VORG` metrics, distinct vertical punctuation forms, and **tate-chu-yoko** (`text-combine-upright`) for short horizontal runs.
- **Emoji as a writing system** (UTS #51): ZWJ sequences (family 👨‍👩‍👧‍👦 = 7 code points, 1 grapheme), skin-tone modifiers, two flag mechanisms (regional-indicator pairs; black-flag + tag sequence + CANCEL TAG for subdivisions like England), VS16 needed to force emoji presentation on default-text code points (❤ → ❤️). `String.length` lies (`"👨‍👩‍👧‍👦".length === 11` in JS); the correct unit is the extended grapheme cluster. Fixed-offset truncation is the classic corruption bug.

---

## 6. Anti-pattern catalog (fast pattern-match)

1. **Length/width in the wrong unit** — `.length` (code units/bytes) used as character count or display width.
2. **Integer-offset indexing/slicing/truncation** of user text without grapheme awareness (mojibake, lone surrogates, severed ZWJ sequences, Rust panic).
3. **Whitespace-split line breaking / word counting** for text that may be CJK/Thai/Lao/Khmer/Burmese.
4. **East Asian Width ignored** in terminal/TUI/column code (cursor desync, broken alignment).
5. **Equality/dedup/hash/DB-lookup without normalization** (NFC vs NFD mismatch, esp. Korean/CJK/accented Latin).
6. **NFKC/NFKD used where information must be preserved** (lossy) — or NFC used where NFKC is needed (fullwidth/halfwidth matching).
7. **`toLowerCase()`/`toUpperCase()` without locale** for security comparison (Turkish-I); case mapping used where case folding is required.
8. **Byte/code-point sort presented as alphabetical** for non-ASCII (should be UCA/CLDR collation).
9. **cmap-lookup-and-place-LTR** mistaken for shaping; RTL/complex script rendered without a shaping engine.
10. **Double kerning** (`kern` table applied alongside a GPOS kern feature).
11. **Missing font fallback** (tofu on CJK/emoji/RTL); single hardcoded font for multilingual text.
12. **Storing Arabic presentation forms** instead of base characters; expecting Hebrew final-form auto-substitution.
13. **Bidi interpolation without isolation** (RTL value in LTR template); unexpected bidi controls in source/logs (Trojan Source → `security`).
14. **Byte-level scanning of undecoded Shift-JIS/CP932** for escapes/separators (0x5C trail-byte injection → `security`).
15. **Non-ASCII digit families** unhandled (`\d`/`[0-9]`/`parseInt` on Arabic-Indic/Persian/fullwidth digits).
16. **BOM in UTF-8** source/config; assuming UTF-8 without a declared/sniffed charset at a boundary (mojibake); Zawgyi assumed to be Unicode.
17. **Han unification ignored** — no `lang`/locale + font for CJK, yielding wrong-region glyph shapes.

---

## 7. Severity and confidence

Matching the panel scale (`~/.claude/rules/panel-contract.md`):

- **blocker**: a reachable correctness/security defect — Shift-JIS trail-byte injection on a byte-scanned input; Turkish-I case bug in an auth comparison; truncation that corrupts stored user data (severed surrogate/cluster written back); a normalization mismatch that makes accounts unfindable/un-deduplicatable.
- **major**: a real, likely-hit defect — whitespace line-breaking that overflows for CJK/Thai users; East Asian Width cursor desync in a shipped TUI; missing font fallback producing tofu for a supported locale; RTL interpolation scrambling order; hand-rolled shaping for a complex script.
- **minor**: narrower or lower-frequency — grapheme-naive truncation on a field unlikely to hit emoji; NFKC vs NFC chosen suboptimally with low blast radius.
- **nit**: cosmetic — a BOM in a file that happens to tolerate it; a kerning subtlety with negligible visual impact.
- **insight**: structural reframe — "this whole module hand-rolls Unicode operations that `Intl.Segmenter`/ICU/HarfBuzz provide correctly; adopting the platform primitive removes a class of bugs."

Confidence: **high** when the unit mismatch is concrete and verifiable from the code (`name.length` used as a max-character limit at a specific line). **medium** when it depends on whether the field ever receives non-ASCII/complex-script/emoji input, which the agent often can't fully verify — say so. Don't assert a script-specific failure without naming the script and the mechanism.

---

## 8. Scan process

1. **Identify the pipeline stage(s)** each text site occupies: encoding boundary, length/measurement, indexing/slicing/truncation, comparison/sort/dedup, normalization, segmentation, line-breaking, shaping, font handling, rendering setup.
2. **For each site, name the unit it operates in and the unit it should** (bytes vs code units vs code points vs grapheme clusters vs display cells). Mismatch = finding.
3. **At every storage/comparison boundary**, check for a documented normalization step and the right form.
4. **At every "measure/wrap/truncate/align" site**, check grapheme awareness and (for terminals) East Asian Width.
5. **At every shaping/font site**, check that a real shaping engine is used for non-trivial scripts and that fallback exists; confirm shaping vs. rendering aren't conflated.
6. **Walk the script axis** for any code that handles arbitrary user text: would it break for CJK / Arabic / Hebrew / Indic / Thai / emoji? Name the script and the mechanism.
7. **Recognize correct platform primitives** (`Intl.Segmenter`, ICU, HarfBuzz, `unicode-segmentation`, etc.) and do *not* flag them as reinvention.
8. **Route** security crossovers (Trojan Source, Shift-JIS injection), locale-policy concerns (`i18n`), GPU rasterization (`graphics-programming`), and PDF text contracts (`pdf`).
9. **Anchor findings to file:line**; pin the Unicode version when citing counts/blocks; respect that some specifics shift annually.
10. **Read-only.** Suggest the fix and the primitive; don't write it.

---

## 9. Authorities and references

- **Unicode Consortium** — the standard, the UAX/UTS/UTR annexes, the UCD. **Mark Davis** (co-founder; president 1991–2022; primary author of the bidi, collation, and normalization algorithms; CLDR/ICU). **Ken Whistler** (longtime core-spec/UCD editor). **Ken Lunde** (*CJKV Information Processing*, O'Reilly — THE CJK reference; the Adobe Source Han / Google Noto CJK fonts).
- **Algorithms by number**: UAX #9 (bidi), #11 (East Asian Width), #14 (line breaking), #15 (normalization), #24 (script property), #29 (segmentation); UTS #10 (collation), #37 (IVD), #51 (emoji); UTR #26 (CESU-8), #54 (Mongolian baseline). RFC 3629 (UTF-8).
- **Encoding history**: Ken Thompson & Rob Pike (UTF-8, Plan 9); Markus Kuhn (the UTF-8/Unicode FAQ, `wcwidth`, the decoder stress test); "UTF-8 Everywhere" manifesto. **Manish Goregaokar** ("Let's Stop Ascribing Meaning to Unicode Code Points," "Breaking Our Latin-1 Assumptions"; ICU4X).
- **Fonts/shaping**: **Behdad Esfahbod** (HarfBuzz — the central figure). **Raph Levien** (font rendering, Vello/Linebender, spline work). **Cosimo Lupo** & **Just van Rossum** (fontTools). **Simon Cozens** (*Fonts and Layout for Global Scripts*, the modern complex-script reference; SILE). **Viktor Chlumský** (MSDF), **Eric Lengyel** (Slug), **Chris Green / Valve** (SDF) — though GPU rasterization routes to `graphics-programming`. **FreeType** (David Turner, Werner Lemberg). Google **Noto** project. Microsoft OpenType / Universal Shaping Engine docs; Apple AAT / Core Text.
- **Layout requirements**: W3C i18n (**Richard Ishida / r12a**); the per-script layout-requirement docs **JLReq** (Japanese, the flagship, from JIS X 4051), **CLReq** (Chinese), **KLReq** (Korean), **ALReq** (Arabic), **MLReq** (Mongolian), **TLReq** (Tibetan), **ELReq** (Ethiopic).
- **Implementations**: **ICU** (the C/C++/Java reference; consumes **CLDR**), **HarfBuzz** (shaping), **fontTools** (manipulation), and the Rust stack (rustybuzz/harfrust/swash/cosmic-text/unicode-segmentation).

Some specifics (per-version character counts, exact dates of older milestones, vendor attributions of color-font formats) shift or are secondary-sourced — pin the Unicode version, and when unsure of a precise figure, say so rather than asserting it. The standard is free at unicode.org; verify against the annex rather than guessing.
