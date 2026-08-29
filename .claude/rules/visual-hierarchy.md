---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Visual Hierarchy and Visual Craft

A reference for evaluating a user interface from a visual-design lens. Used by the `visual-hierarchy` subagent. The scope is *what the eye does*: what it lands on first, how it groups things, whether the levels of emphasis are distinguishable, whether the type is readable at its real size and distance, whether the spacing encodes the actual relationships, and whether the colour system means anything.

Distinct from `accessibility.md` (which owns conformance: contrast thresholds, colour-as-only-signal as a Web Content Accessibility Guidelines violation, focus indicators), `interaction-design.md` (what happens on touch), `information-architecture.md` (whether the grouping is conceptually right, as opposed to visually expressed), `design-systems.md` (whether the values are tokenized and survive), and `text-engineering.md` (font machinery, shaping, rasterization).

The core thesis: **hierarchy is a claim about what matters, and it is made by contrast between levels, not by the absolute value of any level.** A page where the heading is 18px and the body is 16px has no heading. A page where six things are bold has nothing bold. Every finding in this lens should be expressible as "these two levels are not distinguishable" or "the emphasis is on the wrong thing."

The second thesis: **space does the grouping.** Before reaching for a border, a card, or a background fill, check whether the relationship is already sayable in whitespace. Most boxes in most interfaces are compensating for spacing that does not encode the grouping.

---

## 1. The two tests that start any review

**The squint test.** Blur the screen until type is illegible. What remains visible is the actual hierarchy. If the thing that remains is not the thing that matters, the hierarchy is inverted regardless of what the labels say. This is doable from source by reading only size, weight, and colour value, ignoring content.

**The two-second test.** On first paint, what does the screen say this page is for? A screen that needs reading before it can be identified has failed a job that layout is supposed to do.

Both tests come before any specific finding. A review that starts with "the padding here is 14px and elsewhere is 16px" and never asks what the screen is for has inverted its own priorities.

---

## 2. Gestalt grouping, which is the mechanism underneath "just space it better"

These are perceptual defaults; they operate whether or not the designer intended them. Each one is a way an interface can lie about relationships.

- **Proximity.** Things near each other are read as related. The strongest of the group, and it beats every other channel. If a label sits equidistant between two fields, it belongs to neither. The corollary rule: **space belongs to the element it introduces.** Margin above a heading should exceed margin below it, or the heading visually joins the section above.
- **Similarity.** Things that look alike are read as the same kind of thing. This is why one-off styling on a single row or button is expensive: it asserts a difference in kind. It is also why a disabled control styled like a static label is a bug.
- **Common region.** A shared bounded area groups its contents, and it *overrides proximity*. This is the legitimate use of a card or a panel: expressing a grouping that proximity alone cannot, usually because the items are not adjacent. A card drawn around already-adjacent items adds a boundary and no information.
- **Continuity.** The eye follows lines and smooth paths. Aligned edges create implied lines; a broken alignment reads as a broken relationship.
- **Closure.** Incomplete shapes are perceived as complete. Relevant to truncation: a row cut mid-height reads as continuing, which is why a partially visible row is an effective scroll signal and a fully-visible last row is not.
- **Common fate.** Things that move together are read as related. Animate a group together or the grouping breaks.
- **Uniform connectedness.** Explicit connection (a line, a shared background) is the strongest grouping signal of all, stronger than proximity or similarity. Use sparingly, because it is loud.
- **Prägnanz.** Ambiguous arrangements are read in their simplest form. If a layout admits a simpler reading than the intended one, the simpler reading wins.

---

## 3. Typography

### Measure (line length)

45 to 75 characters per line for continuous text; 66 is the traditional ideal (Bringhurst). The mechanism is the return sweep: under about 45 characters the eye sweeps too often and rhythm breaks; over about 80 the sweep loses accuracy and readers reoccasionally lose their line. In CSS the honest expression is `max-width: 66ch` on the text container, not a pixel width tuned at one font size.

This applies to *continuous reading text*. It does not apply to table cells, labels, or single-line values, and applying it there is a review defect.

### Scale

A type scale is useful when consecutive steps are **visibly different**. A modular scale (a base multiplied by a fixed ratio: 1.125 major second, 1.2 minor third, 1.25 major third, 1.333 perfect fourth, 1.5 perfect fifth) produces that by construction. Larger ratios suit display-heavy marketing pages; smaller ratios suit dense interfaces where six distinguishable steps must fit between 11px and 24px.

The practical failure is a scale with steps too close to read as steps: 14, 15, 16, 17 is four values and one level. If a design has many sizes and little hierarchy, the fix is fewer sizes further apart, not more sizes.

### Line height

- Body text: 1.4 to 1.6. Longer measures want more leading; shorter measures want less.
- Display and headings: tighter, 1.1 to 1.25. Large type at 1.5 falls apart into separate lines.
- Dense table rows: tighter still, and the row height rather than the line height is the thing being tuned.
- Line height is proportional to size, so a single ratio applied across a scale will over-lead the large sizes. This is why type scales usually pair each size with its own line height rather than one global ratio.

### Weight and the emphasis budget

Emphasis channels available: size, weight, colour, case, spacing, position, and enclosure. **Use the fewest that work.** A heading that is larger, bolder, uppercase, letter-spaced, and coloured is spending five channels on one job, which leaves nothing for the next level down. Two channels is usually enough; three is a lot.

Note that many typefaces have poor mid weights, so "semibold" may be visually indistinguishable from "medium" at small sizes even though the numbers differ. Verify at real size.

### Numerals, the detail that separates data interfaces from the rest

- **Tabular figures** (`font-variant-numeric: tabular-nums`) make every digit the same width so numbers in a column align on their digits. Without it, proportional figures make a column of numbers ragged and uncomparable. **Any column of numbers that is meant to be compared should be tabular.** This is the single most common typographic defect in data-dense interfaces and it is one CSS declaration.
- **Lining versus old-style figures.** Old-style (text) figures have ascenders and descenders and sit better in running prose; lining figures are cap height and sit better in tables and interfaces. Most interface faces default to lining, which is correct here.
- **Right-align integers and decimals**, so magnitude is comparable by length. Left-align identifiers and codes, which are read as strings, not quantities. Align a decimal column on the decimal separator where the face and layout allow.

### Pairing and face selection

- One to three families. Beyond three, the page reads as an accident.
- A pairing works by *contrast of structure*, not by similarity: a serif with a grotesque, a humanist with a geometric. Two similar sans faces read as a mistake rather than a pairing.
- Monospace earns its place for machine-produced values (identifiers, hashes, codes, timestamps, morphology tags), because it signals "this is a literal" and it aligns.
- Check the actual glyph coverage against the actual content. A face without proper macrons, diacritics, or the specific characters in the domain fails on the content that matters most.
- Every web font needs a fallback stack with **similar metrics**, because the fallback is what renders during load, in print or export where the font cannot embed, and on any client that fails to fetch. A fallback with different metrics causes a visible reflow.

---

## 4. Spacing and the grid

### The spacing scale

Use a small, fixed set of spacing values, not arbitrary numbers. The common base is 8, with 4 available for fine adjustment (Material uses an 8pt component grid with a 4pt baseline grid, specifically because line heights in pure 8pt steps give too few options). A geometric-ish scale (4, 8, 12, 16, 24, 32, 48, 64) gives distinguishable steps; a linear one (4, 8, 12, 16, 20, 24, 28) gives many values that look alike, which reproduces the type-scale failure in spacing.

The point of the scale is not the number 8. It is that **spacing values are a closed set, so a difference in spacing always means something.** Three near-identical paddings across three components communicate nothing except that nobody was in charge.

### Where the grid stops

The honest position, which the purists lose: a grid is a guide, not a law. Optical balance beats mathematical alignment when they conflict, and they do conflict:

- **Round and pointed shapes overshoot.** A circle or a triangle must extend slightly past a square's bounds to look the same size. Icon sets handle this internally; hand-drawn glyphs usually do not.
- **Optical centering is above mathematical centering.** Text centered in a button by exact box math looks low, because descender space is counted and not seen. The same applies to a glyph in a circular avatar.
- **Punctuation and quotes hang.** Aligning a pull quote's opening quotation mark to the text edge makes the text look indented.
- **Uppercase and small caps want letter-spacing**; lowercase at text sizes does not, and adding it hurts.

A finding that says "this is 1px off the grid" without a visual consequence is a nit at best and usually noise. A finding that says "this is optically low because the box math ignores the descender" is real.

---

## 5. Colour

### Use a perceptually uniform space

HSL is not perceptually uniform. Two colours at the same HSL lightness have wildly different perceived brightness: `hsl(60, 70%, 50%)` (yellow) reads far brighter than `hsl(240, 70%, 50%)` (blue). The consequences in a real palette are that a "same lightness" ramp across hues is not the same lightness, contrast behaviour changes unpredictably per hue, and every scale needs manual correction.

**OKLCH fixes this and is available in CSS.** Equal numeric steps in OKLCH produce approximately equal perceived steps. Practical consequences worth stating in a review:

- Build ramps by varying L in OKLCH and holding C and H, and the steps look even without hand-tuning.
- Accent colours that share L and C and vary only H read as siblings, and they carry similar contrast against the same background.
- If contrast passes for one hue at a given L, a different hue at the same L behaves similarly. That is not true in HSL and it is the reason HSL palettes need per-hue exceptions.
- Watch gamut: high chroma at some hues falls outside sRGB. `oklch()` clamps, so a value that looks fine in a wide-gamut preview may flatten on an sRGB display.

### Contrast, and which standard applies

- **WCAG 2.x** is the compliance standard and is what `accessibility.md` enforces: 4.5:1 for normal text, 3:1 for large text (18.66px bold or 24px regular) and for user interface components and meaningful graphics. The European Accessibility Act's presumed-compliance route (EN 301 549) incorporates WCAG 2.1 AA, so this is a legal floor in the EU, not a preference.
- **APCA** (Advanced Perceptual Contrast Algorithm) is the candidate method for WCAG 3 and models perception better, accounting for font size, weight, and polarity (light-on-dark behaves differently from dark-on-light, which WCAG 2's symmetric ratio cannot express). Radix Colors targets APCA. **WCAG 3 remains in development and is not a compliance target**, so APCA is a better design tool and not yet a substitute for the 2.x check.
- The practical split: use APCA-aware tooling (Radix, Leonardo, Atmos) to *build* a palette, then verify against WCAG 2.x because that is what anyone will audit against.
- Contrast is a **hierarchy tool** as well as an accessibility floor. Two text levels that both pass 4.5:1 can still be indistinguishable from each other. Passing conformance says nothing about whether the emphasis reads.

### Semantic colour

Colour in an interface should mean something and mean one thing. The failure mode is a palette whose names are values (`blue-500`) used directly in components, so that "the destructive colour" and "the brand colour" cannot diverge and a rebrand rewrites every component. That is `design-systems`' territory; this lens owns whether the *distinctions* the colours draw are the distinctions the interface actually needs.

Two rules that stay here:

- **Colour must not be the only channel for a meaning.** This is WCAG 1.4.1 and `accessibility` owns the conformance finding, but this lens owns the design consequence: a status encoded only by hue collapses for roughly 8% of men with red-green deficiency and for anyone printing in greyscale. Pair with shape, icon, position, or text.
- **Red and green as the two poles of a scale is the specific worst case**, because it is the commonest deficiency and because it is used for the highest-stakes distinctions (pass/fail, up/down, safe/destructive). A design that must distinguish two states by hue alone should not choose that pair. Differentiating by *fill versus outline* or by *chroma versus neutral* survives every deficiency and greyscale.

### Dark mode

Not an inversion. Specific things break:

- Pure black backgrounds with pure white text cause halation and are fatiguing; use a near-black with a slight hue and a near-white.
- Saturated colours vibrate against dark backgrounds. Reduce chroma as lightness drops.
- Shadows do not read on dark. Elevation must be expressed by lighter surfaces rather than by darker shadows.
- Contrast requirements are directional, which WCAG 2.x cannot express and APCA can. A pair that passes in light mode may be perceptually worse when inverted.

---

## 6. Density, and the argument against reflexive whitespace

This section exists because the default advice in the field is wrong for a large class of software.

**Tufte's position.** *The Visual Display of Quantitative Information* defines data-ink as "the non-erasable core of a graphic," the data-ink ratio as data-ink over total ink, and chartjunk as "ink that does not tell the viewer anything new." The prescription is to maximize data density and erase non-data ink. It is the intellectual foundation for high-density professional interfaces.

**The empirical challenge.** Bateman et al. (2010) found embellished charts were remembered better than minimal ones, with no loss of description accuracy. Stephen Few and others have contested the methodology. The honest summary: the empirical literature challenging data-ink is real but small, and as Few himself noted, little of the debate was dispassionate. Tufte's position remains dominant by citation, not by trial. Do not present either side as settled.

**Where this actually bites.** Enterprise and professional tools have a body of practice that contradicts consumer-web minimalism directly:

- Expert users of complex tools **prefer density**, because density enables spatial memory: after a week, the practiced user finds a value by position rather than by reading, in a fraction of the time. Whitespace that pushes content below the fold destroys that, permanently, for every session.
- The cost of density is paid once, during learning. The cost of sparseness is paid on every use. For a tool used eight hours a day the arithmetic is not close.
- A trader who needs twenty prices visible at once loses real money to a layout that shows twelve beautifully.

**The reviewer's obligation** is therefore to establish which kind of surface this is before critiquing its density, and to say so in the finding. "Add whitespace" applied to a professional tool is a recommendation to make the tool worse. See `expert-user-efficiency.md`, which argues this side deliberately and in more depth.

**What density is not a licence for.** Density means information per unit area, not clutter. Tufte's own prescription is to *remove non-data ink* while raising data density: fewer borders, fewer background fills, fewer rules, lighter separators, no decorative chrome. A dense screen with heavy chrome is the worst of both. The correct dense interface is quiet: hairlines rather than boxes, tight rhythm, and the ink spent on content.

---

## 7. Tables, because data interfaces live or die here

- **Alignment**: numbers right, text left, and never centre a column of anything you want compared. Centre only single short values in narrow columns.
- **Tabular figures**, as above. Non-negotiable in any comparable numeric column.
- **Rules versus zebra versus neither.** Horizontal hairlines are the quietest and work at any width. Zebra striping is more scannable at wide widths where the eye must track across a long row, and it costs a background channel that then cannot mean anything else (selection, error, staleness). Neither is right where rows are tall enough to be individually legible. Vertical rules are almost never needed; if columns are hard to separate, the fix is spacing.
- **Column order encodes importance.** Left-most is read first. Identifier, then the thing being judged, then supporting detail, then metadata. A table whose first three columns are metadata has buried its subject.
- **Sticky headers** are required past roughly one screen of rows; a header scrolled out of view makes every column below it unlabeled.
- **Truncation must be visible and recoverable.** An ellipsis with no way to see the full value is a dead end. Prefer wrapping in a column whose content genuinely varies.
- **Row height is the density dial.** Offering a compact/comfortable toggle is a legitimate answer to the density argument, and it is cheap.
- **Do not centre a table in a wide viewport** at a fixed narrow width; let it use the space, since the reason for a table is comparison.

---

## 8. Schools of thought that genuinely disagree

### Swiss modernism versus expressive design

The grid tradition (Müller-Brockmann's *Grid Systems in Graphic Design*, Tschichold's later work, the International Typographic Style) treats the grid as the source of order and clarity, favours the objective and the neutral, and distrusts ornament. The counter-tradition (postmodern typography, contemporary editorial and brutalist web design) treats the grid as a starting point to break deliberately, and argues that neutrality is itself a style and often a bland one.

Neither is wrong. What is wrong is **grid-breaking without a system to break**, which reads as an accident rather than a decision. In review: if a layout breaks alignment, ask whether the break is consistent and repeated (a decision) or singular and unexplained (a mistake).

### Flat design and the signifier cost

The flat turn (roughly 2013 onward) removed gradients, bevels, and shadows. Norman and Nielsen both published on the consequence: removing visual depth removed the *signifiers* that told people what was clickable, and measured discoverability suffered. The industry answer was "flat 2.0," which restores a minimum of depth (subtle shadow, subtle border, hover state) while keeping the flat aesthetic.

The live question in any review of a flat interface: **can a person tell what is interactive without hovering?** If the only difference between a button and a label is colour, and colour is also used for emphasis elsewhere, the answer is no.

### Aesthetic-usability, and why it makes this lens dangerous

Because beautiful interfaces are rated as more usable than they are (section 3 of `interaction-design.md`), a reviewer of a polished design will under-report interaction problems, and a reviewer of an ugly one will over-report them. This lens is the one most exposed to that bias. The defence is to state findings in terms of a mechanism ("these two levels differ by 1px and no weight change") rather than an impression ("this feels cluttered").

---

## 9. Anti-pattern catalog

- **No first-level.** Nothing on the screen is clearly the most important thing.
- **Too many levels.** More than about four distinguishable text levels on one screen and none of them mean anything.
- **The uniform bold.** Every label bold, so bold has no meaning left.
- **Compensating boxes.** A card, border, or fill doing work that spacing should do. Symptom: nested cards.
- **Equal spacing everywhere.** Uniform gaps between everything, so nothing is grouped.
- **Spacing attached to the wrong side.** Heading closer to the paragraph above it than the one it introduces.
- **Ragged numeric column.** Proportional figures in a comparable column.
- **Centred body text** beyond two lines. Every line starts in a different place, so the eye has to hunt for the start of each.
- **Colour-only status.** See section 5.
- **The unreadable mid-grey.** Secondary text at a value that fails contrast, usually chosen because it "looked calmer."
- **Font size below the floor for the medium.** Under about 12px on screen for anything meant to be read, under 12pt in print.
- **Icon without label** where the icon is not one of the small set that is genuinely universal.
- **Full-width text in a wide viewport**, producing 150-character measures.
- **Decoration that reads as meaning**: a coloured left border, a badge, or an accent that turns out to be styling rather than status.
- **Mixed radii, mixed shadows, mixed border colours** within one screen, which is the visual signature of three people or three sessions and no system.

---

## 10. What to flag, and what not to

**Flag:**
- Two hierarchy levels that are not visually distinguishable, with the specific values.
- Emphasis on the wrong element, named against the screen's actual job.
- Spacing that contradicts the grouping (proximity says one thing, the data model says another).
- A comparable numeric column without tabular figures.
- Measure well outside 45-75 for continuous reading text.
- A colour distinction carrying meaning that survives neither greyscale nor red-green deficiency. Note `See also: accessibility` for the conformance finding and keep the design finding here.
- A palette built in HSL whose ramp steps are perceptually uneven, where the code shows it.
- Interactive elements with no signifier distinguishing them from static ones.
- Density that is wrong *for the identified surface type*, in either direction, with the task named.

**Do not flag:**
- Contrast-ratio conformance. `accessibility` owns it. Flag the *hierarchy* consequence if there is one.
- Focus indicators, ARIA, keyboard. `accessibility`.
- Whether a value is tokenized or hard-coded. `design-systems`.
- Copy length or wording. `content-design`.
- Sub-pixel grid deviations with no visual consequence.
- Taste. "I would have used a different typeface" is not a finding. "This face has no tabular figures and the interface has six numeric columns" is.
- Density in an expert tool on minimalist grounds alone. Section 6.

Every finding names the mechanism and the values: not "the hierarchy is weak" but "the section heading is 15px/600 and the row label is 14px/600, so the heading reads as another row."
