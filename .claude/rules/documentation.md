# Documentation

A reference for evaluating documentation quality during review. Used by the `documentation` subagent. Cross-language, cross-surface; complements `~/.claude/rules/readability.md` (which covers in-code prose / naming / function shape).

The unifying thesis: **documentation is a product with users**. Different users need different things at different times. The two most expensive failure modes are (1) documentation that lies (worse than no documentation, because it costs time and erodes trust) and (2) documentation in the wrong shape for the question being asked (a tutorial when the reader needs a reference, or vice versa).

The agent's job: surface the gaps, flag the lies, call out the structural mismatches. Read-only; the user decides what to act on.

---

## 1. Universal principles

### Diátaxis: the four-quadrant framework

Daniele Procida's framework (now used by Canonical, Cloudflare, NumPy, the Python core docs, and many others) identifies four distinct documentation types, each serving a distinct user need:

- **Tutorials** (studying + practical). Learning-oriented. A lesson that takes a beginner through a guided foundational experience. Goal: build confidence and foundational skill.
- **How-to guides** (working + practical). Goal-oriented. Practical recipes for a competent user with a specific objective. "How to configure HTTPS." "How to migrate from v3 to v4."
- **Reference** (working + theoretical). Information-oriented. Neutral, accurate, complete description of the system. A map, not a journey. API references, configuration listings, CLI flags.
- **Explanation** (studying + theoretical). Understanding-oriented. Background, context, design rationale, alternatives, history. The reader is learning *about* the thing.

Failure mode: mixing the types on a single page. A tutorial with reference interjections derails the learner; a how-to larded with explanation frustrates the user who wants to ship; a reference page with tutorial-style "first, let's..." prose is unscannable.

**Review heuristic**: for any documentation page, ask "which quadrant is this serving?" If the answer is "all four," that page needs to be four pages.

### Docs-as-code

Documentation lives in the source repository, uses the same Git workflow, gets reviewed in pull requests, builds in CI, and deploys on merge. Same versioning, same review discipline, same automation. Wiki-as-truth is the failure pattern: wikis have no relationship to the code's version, no review process, no CI to catch broken links, no enforced ownership. They become orphan documents whose drift from reality compounds.

**Review flag**: documentation in a wiki / Confluence / Notion describing code that lives in a git repo, with no link between the two and no expectation that a PR updates both.

### Write the Docs principles (the operational set)

- **ARID** (Accept Repetition In Documentation). Unlike code, some duplication in docs is necessary for readability. Aim for DRY where it pays; accept restatement where the reader needs it.
- **Skimmable**. Most readers scan. Descriptive headings, paragraphs that lead with their key concept, well-anchored links.
- **Exemplary**. Examples for common cases, not every case. Too many examples make docs less skimmable.
- **Consistent**. Same terminology, same formatting, same voice across the corpus.
- **Current**. "Incorrect documentation is worse than missing documentation." The strongest principle in the set.
- **Nearby**. Store docs close to the code: inline doc-comments, Markdown files in the same repo. Distance kills sync.
- **Unique**. Avoid duplicated authority -- two documents both claiming to be canonical for the same fact is a sync hazard. (Unique forbids duplicated *truth*; ARID accepts duplicated *exposition*. Different axes.)
- **Cumulative**. Order content so prerequisites precede dependents -- especially in tutorials.

### Google developer documentation style guide (the writing-rules baseline)

- **Second person** ("you"), not "the user" or "we."
- **Active voice** ("Send a query"), not passive ("A query is sent").
- **Present tense** ("returns"), not future ("will return").
- **Sentence case** in headings.
- **Serial (Oxford) commas**.
- **Conditions before instructions**: "If authenticated, return the profile" not "Return the profile if authenticated."
- **Descriptive link text**, not "click here" or bare URLs.
- **Write for a global audience**: avoid idioms, regional slang, ambiguous date formats.

### Document *why*, not *what* (Hunt & Thomas, *The Pragmatic Programmer*)

Comments should discuss *why* -- purpose, intent, trade-off, business reason -- not *what* the code does. The "what" should be evident from the code; if it isn't, refactor the code rather than annotating it.

`// x is incremented` above `x++` is dead weight. `// x++ because the protocol counts ACKs starting from 1` is irreducible information that cannot be inferred from the code.

**Review heuristic**: for every comment, ask "would deleting this lose information?" If no, delete. If yes, the information is usually the *why*.

### README-driven development (Tom Preston-Werner)

Write the README *before* the code. The act of writing the README is the act of designing the API. Surfaces design's confusing parts before any code locks them in. Best fit: libraries, tools, and APIs with a public surface; less appropriate for exploratory work where you don't yet know what the thing is.

### What top-tier docs do (Stripe, Twilio, GitLab, Sentry as reference points)

- **Code samples as first-class content**, often in multiple languages with one-click switching.
- **Interactive / runnable examples** in the browser rather than copy-paste-only.
- **Code-first navigation**: developers scroll past prose to find a working sample. Lead with the sample.
- **Comprehensive error documentation**: every error code has its own entry with cause and remediation.
- **Versioning + changelog** visible from the docs UI; old versions preserved at stable URLs.
- **Strong search** -- often the primary navigation mode in practice.
- **AI-readable** structure: clean Markdown / OpenAPI / structured data so LLM-based assistants ingest cleanly.

---

## 2. Rust documentation conventions

Rust has the strongest documentation culture of any mainstream language: rustdoc is first-class, doctests compile and run as tests, and docs.rs auto-publishes every released crate. The ecosystem expectation: every non-trivial crate has rendered docs at `docs.rs/<crate>` and they work.

### Anatomy
- `///` denotes an outer doc comment, attached to the *next* item.
- `//!` denotes an inner doc comment, attached to the *enclosing* item. Used at the top of `lib.rs` for crate-level docs, and at the top of a module file for module docs.
- Markdown with CommonMark extensions (tables, footnotes, strikethrough).

### The summary line rule
The first line before any blank line is the **summary**. It appears in search results, module overviews, and "see also" lists.
- One sentence.
- Third-person singular present indicative: "Returns the length of the slice."
- Don't restate the item's name; the reader sees the signature.
- After a blank line, write detailed explanation.

### Standard sections, in order
1. **`# Examples`** -- runnable code. Required by C-EXAMPLE for every public item. Use `?` for error handling rather than `unwrap()` (C-QUESTION-MARK), since users copy-paste verbatim.
2. **`# Panics`** -- conditions under which the function panics. Required for any function that can panic, even rarely.
3. **`# Errors`** -- for `Result`-returning functions, what `Err` variants mean and when.
4. **`# Safety`** -- **required** for every `unsafe fn`. Documents the invariants the caller must uphold to avoid undefined behavior. A missing `# Safety` section is itself an API bug.
5. **`# Performance`** -- when complexity or allocation behavior matters.

### Doctests as a feature
Code in `# Examples` is compiled and run by `cargo test` by default. This makes examples self-checking: if the signature changes, the example breaks the build.
- Code fences without a language default to Rust.
- Lines starting with `#` are hidden in rendered output but still compiled. Use to hide boilerplate (`# use crate::Foo;`) while keeping the example self-contained.
- Fence attributes: `ignore`, `no_run`, `should_panic`, `compile_fail` control behavior.

### Documentation attributes
- `#[must_use]` warns when the return value is discarded. Apply to builders, `Result`, lazy iterators. Optional message: `#[must_use = "this iterator is lazy and does nothing unless consumed"]`.
- `#[deprecated(since = "1.2.0", note = "use `bar` instead")]` issues a deprecation warning. **Always include `note`** -- without it, callers don't know what to use instead.
- `#[doc(hidden)]` removes an item from rendered docs while keeping it public (for items that must be `pub` for macro expansion but aren't part of the API surface).
- `#[doc(alias = "foo")]` adds a search alias. If users might search "uppercase" but the method is `to_ascii_uppercase`, an alias surfaces it.

### Intra-doc links
Refer to other items by path: `` [`Vec`] ``, `` [`Vec::push`] ``, `[crate::module::Item]`. Backticks render in code font. Rustdoc warns on broken links at build time; bare backtick `` `foo` `` text rots silently when the item is renamed.

**Review heuristic**: cross-references in docs should always be intra-doc links, never plain code-formatted text.

### Where the documentation lives: traits vs impls
Document the trait once, with the contract every implementer must satisfy. Implementation-specific notes go on the impl. The pattern: `Iterator::next` has the canonical contract; `std::vec::IntoIter::next` has any iterator-specific notes plus the default-rendered trait doc.

### The `missing_docs` lint
`#![deny(missing_docs)]` at the crate root makes a missing doc on a public item a compile error. `#![warn(missing_docs)]` is the gentler "ratchet" option. Libraries with a public API should treat one or the other as table stakes.

### Rust API Guidelines documentation checklist
- **C-CRATE-DOC**: Crate-level docs are thorough, include examples and ecosystem positioning on the front page.
- **C-EXAMPLE**: Every public module, trait, struct, enum, function, method, macro, and type has an example.
- **C-QUESTION-MARK**: Examples use `?` not `unwrap()`.
- **C-FAILURE**: `# Errors`, `# Panics`, `# Safety` as appropriate.
- **C-LINK**: Prose hyperlinks to relevant items (intra-doc links).
- **C-METADATA**: `Cargo.toml` has authors, description, license, repository, keywords, categories.
- **C-RELNOTES**: A changelog identifies breaking changes per semver.
- **C-HIDDEN**: Use `#[doc(hidden)]` for items that must be public but aren't part of the API surface.

---

## 3. TypeScript / JavaScript / JSDoc / TSDoc

### JSDoc tags
`/** ... */` block comment immediately above the item. Common tags:
- `@param {Type} name - description` -- per-parameter doc. With TypeScript the `{Type}` is redundant.
- `@returns {Type} description` -- return value.
- `@throws {Error} description` -- exceptions.
- `@example` -- code block.
- `@deprecated <since-version> - use Foo instead` -- mark and link the replacement.
- `@see` -- cross-reference.
- `@since <version>` -- introduction version.

### TSDoc: Microsoft's TypeScript-flavored standardization
TSDoc (`tsdoc.org`, Microsoft) is a rigorous spec for doc comments in TypeScript. Motivation: JSDoc's grammar is informally inferred from one tool's behavior; most JSDoc tags are about *type annotations* (which TypeScript handles natively); custom tags from one tool break parsing in another.

Differences from JSDoc:
- Drops type annotations (TypeScript already has types).
- Requires `{@link Foo}` for cross-references rather than auto-hyperlinking after `@see`.
- Specifies a parseable grammar so different tools (TypeDoc, API Extractor, IDEs) agree.
- Custom tags must be declared in a `tsdoc.json`.

### "Comments describe why, types describe what"

In TypeScript / Rust / Kotlin / Swift, the type signature carries most of the "what." `function fetchUser(id: UserId): Promise<User | NotFound>` already tells you the input, output, and failure mode. A doc comment that restates this is overhead.

What's still worth documenting:
- *Why* this function exists (the business reason).
- *When* to call it vs alternatives (does it cache? trigger side effects? involve I/O?).
- Edge cases the types don't express (rate limits, idempotency, ordering guarantees).
- Examples of typical use.
- Deprecation, replacements, migration notes.

**Review flag**: a doc comment that is a verbatim restatement of the type signature in English ("Takes a userId and returns a Promise of User or NotFound") is dead weight. Either delete or rewrite to explain the contract beyond the types.

### Tooling
- **TypeDoc** -- de facto static-site generator for rendered TypeScript reference docs. Small projects' default.
- **API Extractor** (Microsoft) -- more rigorous: analyzes a project, produces a `.d.ts` rollup with release-type trimming plus a machine-readable API surface description. Pair with **API Documenter**.
- **`.d.ts` files** -- the source of truth for IDE intellisense. Doc comments in `.d.ts` (or carried from `.ts` source by the build) are what most consumers actually see. Document there or in the generating source; never separately.

### npm package README conventional sections
1. Title and one-sentence description -- what is this, why does it exist.
2. Badges -- build status, coverage, version, license, npm downloads. Useful signal; overdone if there are more badges than content.
3. Installation -- `npm install foo` / `pnpm add foo` / `yarn add foo`.
4. Quick start -- 5-10 lines of code that does something useful. The reader's decision to keep reading rides on this.
5. API or link to full API docs.
6. Examples -- larger code samples for common patterns.
7. Configuration / options.
8. Browser / Node compatibility.
9. Contributing (or link to CONTRIBUTING.md).
10. License.

---

## 4. Other languages (briefly)

### Python
Three docstring conventions coexist; consistency within a project matters more than the choice.
- **Google style**: indentation-delimited sections (`Args:`, `Returns:`, `Raises:`). Compact; most common in general application code.
- **NumPy style**: underline-delimited sections, more vertical. Standard in scientific Python.
- **reST / Sphinx-native**: `:param x:`, `:returns:`, `:raises:`. Closer to underlying restructured-text.

`sphinx.ext.napoleon` converts Google and NumPy to reST so any can be the source. PEP 484 type hints carry types; docstrings carry intent and examples.

### Go
Minimalist by design.
- Every exported identifier has a doc comment immediately above the declaration (no blank line).
- The comment is a complete sentence beginning with the identifier's name: `// User represents an authenticated user.`
- The package comment, on a single file at the top of the package, describes the package as a whole.
- `// Deprecated: use Foo instead.` (the literal prefix `Deprecated:` followed by description) marks deprecation. `go vet` recognizes this exact form.
- Example functions in `_test.go` files become rendered examples *and* run as tests. Naming: `ExampleFoo`, `ExampleFoo_bar`, `ExampleType_Method`.

### Java
The `@param` / `@return` / `@throws` triad, in that order. Null-tolerance documented in `@param` / `@return` ("not null" / "may be null") -- in pre-JSpecify Java, the type doesn't tell you, so the doc must. `{@link Foo}` and `{@link Foo#bar()}` for cross-refs. `@deprecated since=... description` plus the `@Deprecated` annotation.

### C / C++
Doxygen. `@brief` (one-sentence summary), then detailed prose, then `@param`, `@tparam` (templates), `@return`, `@throws`, `@note`, `@warning`, `@see`. Either `/** ... */` block style or `///` line style. Markdown inside doc blocks is supported.

### Swift
DocC (Apple, since Xcode 13). Markdown plus DocC-specific extensions (symbol links via double backticks, asides, term lists). First paragraph is the summary; subsequent paragraphs are discussion. Parameter/return/throws blocks use `- Parameter foo:`, `- Returns:`, `- Throws:`. Asides via `> Important:`, `> Warning:`, `> Note:`.

---

## 5. Documentation surfaces beyond doc comments

### README
The single highest-leverage document. The user's first contact, the search-result hit, the entry-point for every other doc. Should answer: what is this, why use it, how do I install and try it, where do I learn more. Quick-start within the first scroll. README is **not** the place for full reference -- it's the on-ramp.

### CHANGELOG
[Keep a Changelog](https://keepachangelog.com/) is the de facto standard:
- One file (`CHANGELOG.md`) at the repo root.
- Newest version on top, with version + ISO 8601 date (`YYYY-MM-DD`).
- `[Unreleased]` section at the top accumulates changes between releases.
- Each version groups changes under: **Added**, **Changed**, **Deprecated**, **Removed**, **Fixed**, **Security**.
- Written for users, not developers: "what changed from the user's perspective."
- Don't skip versions. Don't dump a git log. Don't bury breaking changes.

Pairs with [Semantic Versioning](https://semver.org/): MAJOR.MINOR.PATCH. MAJOR for breaking changes, MINOR for backward-compatible features, PATCH for backward-compatible fixes. Breaking changes in a MINOR release is a contract violation.

### Architecture Decision Records (ADRs)
Michael Nygard's 2011 format. One short doc per decision, in-repo (commonly `doc/adr/`), Markdown, numbered (`0001-use-postgres.md`).

Five sections:
1. **Title** (numbered, action-oriented: "Use Postgres for the primary store").
2. **Status**: proposed / accepted / deprecated / superseded by ADR-NNN.
3. **Context**: what forces are at play, what problem are we solving.
4. **Decision**: what we will do (active voice, present tense).
5. **Consequences**: positive, negative, and neutral results.

ADRs work because they're small (1-2 pages), in-repo (version-controlled), immutable (you supersede, you don't edit), and decision-focused (one ADR per decision).

### Runbooks
Operational documentation: "when alert X fires, do Y." Read at 3am by a sleepy on-call who didn't write the system.
- **One runbook per alert.** A 12-scenario runbook is unfindable mid-incident.
- **Lead with diagnostics**, not actions. Symptom-checking first, then remediation.
- **Owned, dated, reviewed**. Named owner; reviewed quarterly; tested in game days.
- **Linked from the alert** itself, not search-only.
- **Plain language, no jargon shortcuts.** The reader may not be a domain expert.

### API reference docs
Generated from source (rustdoc, TypeDoc, godoc, Sphinx autodoc, Javadoc). Hand-written only for overview prose, conceptual chapters, migration guides. The generated form is authoritative for *what exists*; hand-written prose is authoritative for *why and how*.

### Tutorials vs how-to guides
Tutorials are for learners building skill; how-to guides are for competent users solving problems. Tutorials don't assume context, walk through every step, prioritize confidence over breadth, have a definite endpoint. How-to guides assume the reader knows what they want, focus on the goal not the journey, and can branch ("if X, do Y; if Z, do W").

A "tutorial" that says "configure your environment as appropriate for your platform" has failed at being a tutorial -- it's a how-to wearing tutorial clothes.

### CONTRIBUTING.md and meta-docs
A good `CONTRIBUTING.md` welcomes the reader; names the channels; sets expectations; documents the dev environment with version pins; documents test commands; describes the PR process; links to the code of conduct, license, security policy.

Companion files: `CODE_OF_CONDUCT.md`, `SECURITY.md` (responsible disclosure), `SUPPORT.md` (where to ask questions), `GOVERNANCE.md` (decision-making for larger projects).

### Inline comments vs doc comments
- **Doc comments** (`///`, `/** */`, `"""..."""`) are part of the API surface, render in tooling, travel with the item.
- **Inline comments** (`//`, `#`) are for implementation notes -- the *why* of a specific block, the link to the bug report, the historical reason a constant has its value.

Don't put implementation details in doc comments (callers don't care how, only what). Don't put API contract in inline comments (callers don't read source). A function with both a doc comment (the contract) and inline comments (the why of the implementation) is well-documented.

### Migration guides
A specific form of how-to: "if you're on v3, do these steps to get to v4." Required for any major version with breaking changes.
- One guide per major version jump.
- Lead with a **breaking changes list** -- the scan target.
- Each breaking change has: what changed, why, before/after code samples, migration steps.
- Document **deprecations introduced in the previous version** -- those are the breaking changes of the next version's user.
- If the upgrade is multi-step (3 -> 3.8 -> 4), say so up front.

### Examples directories
A repo-level `examples/` directory with standalone runnable programs is the most credible documentation -- it's executable, it can't lie, consumers copy it directly.
- One subdirectory per example, each with its own README.
- Each example builds and runs in CI; broken examples fail the build.
- Cover canonical use cases, not exhaustive variants.
- Mentioned in the main README ("see `examples/` for full programs").

The Rust ecosystem standardizes on `examples/foo.rs` runnable via `cargo run --example foo`. npm packages often use `examples/` with each subfolder being its own runnable project.

---

## 6. Anti-patterns

### Stale docs that lie
The strongest principle: incorrect documentation is worse than missing documentation. Missing docs cost time (read the code). Incorrect docs cost time *and* trust *and* introduce bugs (follow wrong instructions). Mitigations: docs-as-code, generated docs where possible, ownership per doc, scheduled review, automated link checks, CI doctests.

### Auto-generated docs with no human-written content
Auto-generated reference is necessary but not sufficient. A page that says only "function `foo(x: number): number` -- takes x, returns a number" is noise. Generated docs need human-written summaries, examples, and explanation to be useful.

### Tutorials masquerading as references (or vice versa)
The Diátaxis violation. A "reference" full of "first, you'll want to..." prose is unscannable. A "tutorial" full of `--flag` enumerations is unfollowable. Per page, name the type and stick to it.

### Over-documentation
Every getter with `/// Gets the X` adds noise without information. Doc comments should provide information the type signature doesn't. The "rarely too much documentation" principle applies to *meaningful* documentation, not mechanical restatement.

### Under-documentation at the API boundary
The other direction: public items with no doc, sparse examples, missing `# Safety` / `# Errors` / `# Panics`. The library author knows the contract; consumers don't. Anything `pub` or exported is a contract; document it.

### "Hello world" examples that don't show real usage
`console.log('hi')` works. It doesn't tell anyone how to use the library for real work. Examples should solve a believable problem -- close enough to real use that copy-and-modify yields something useful.

### Inline comments restating the code in English
`// increment x` above `x++` is dead weight. If the code is unclear, refactor; if the code is clear, the comment is noise. Exception: when the *why* isn't obvious (non-obvious algorithm choice, workaround for an external bug, measured performance optimization), comment the why.

### Missing "why," abundant "what"
Code documents the "what" intrinsically. The "why" -- design rationale, alternatives considered, business constraint, historical accident -- is the irreducible content of comments. Flag comments that recite the "what" and absent comments where the "why" is non-obvious.

### Rotted TODO / FIXME / HACK comments
A TODO from 2019 with no ticket, no owner, no context is documentation of abandonment, not a task. Conventions: every TODO has an issue link or an owner; no TODOs ship to production without one. HACK is the strongest form ("I knew this was wrong and shipped anyway") and demands a ticket.

### Docs describing internal implementation instead of contract
Public docs should describe the *contract* -- what callers can rely on. Implementation details ("uses a B-tree internally," "caches for 5 minutes") leak abstraction. Document them if they're load-bearing (cache TTL affects observed behavior); omit if incidental and subject to change.

### Doc rot from no review process
If docs are reviewed only when someone notices they're wrong, they rot continuously between noticings. Review on every code change that affects them; review periodically regardless. The maintenance is the cost of having docs.

---

## 7. Severity and confidence

The `documentation` subagent produces findings at these severity levels (matching `/expert-review`'s scale):

- **blocker**: documentation that actively misleads. A doc comment that says the function returns `User` when it returns `User | null`. A README install command that doesn't work. A safety doc on an `unsafe fn` that omits a required invariant. Reviewers and consumers will be wrong because of this doc.
- **major**: significant gap or structural problem. Public API with no doc; `unsafe fn` with no `# Safety` section; panicking function with no `# Panics`; a tutorial that's actually a how-to; a CHANGELOG that buries breaking changes; broken intra-doc links; missing migration guide for a breaking change.
- **minor**: noticeable but not blocking. Doc comment restates type signature; example uses `unwrap()` instead of `?`; inline comments paraphrase the code; `@deprecated` without `note`; missing `# Errors` section on a `Result`-returning function in a non-public crate.
- **nit**: cosmetic. Style-guide deviation (passive voice, "the user" instead of "you"); inconsistent doc-comment style within a file; ungrouped `Cargo.toml` keywords.
- **insight**: structural observation worth a conversation. "This module's docs would benefit from a Diátaxis split: the README currently mixes tutorial, how-to, and reference content." "Consider adopting Keep a Changelog format for the existing CHANGELOG." "The `examples/` directory would be the most credible documentation for this library; currently there is none."

Confidence: high when the gap is concrete and verifiable from the code or repo configuration ("this public function `foo` in `src/lib.rs:42` has no doc comment"); medium when reasoned ("the prose here mixes tutorial and reference styles, though I cannot fully verify the team's documentation conventions without more context").

---

## 8. Process for the documentation agent

1. **Discover the project's docs surface.** Look for `README.md`, `CHANGELOG.md`, `docs/`, `doc/adr/`, `runbooks/`, `examples/`, `CONTRIBUTING.md`, `SECURITY.md`. Check for generated-doc tooling: `rustdoc`, TypeDoc / API Extractor configuration, Sphinx config, Docusaurus / Mintlify / MkDocs site, godoc URL in README.
2. **Identify the language(s).** Apply the per-language conventions (Rust rustdoc, TypeScript JSDoc/TSDoc, Python docstring style, Go godoc, etc.). Project conventions in `CLAUDE.md` / style guides override generic principles.
3. **Walk the in-code documentation lens.** For each public API item: is there a doc comment? Does it lead with a one-sentence summary? Are the required sections present (`# Safety`, `# Panics`, `# Errors`, `@throws`)? Are examples runnable and meaningful? Are deprecations annotated with replacements?
4. **Walk the documentation-surfaces lens.** Does the README have the conventional sections for the ecosystem? Is the CHANGELOG (if present) following Keep a Changelog? Are ADRs used for non-trivial decisions? Are runbooks present where alerts exist? Is the `examples/` directory present, populated, and CI-verified?
5. **Look for anti-patterns.** Stale docs (compare doc claim to code reality), over-documentation (mechanical restatement), under-documentation (no doc at API boundaries), tutorial-as-reference mixing, rotted TODO/FIXME/HACK, comments that restate code.
6. **For diff mode**, check that the PR updates the docs the code change affects. A signature change without a doc comment update is an incomplete PR. A new public item with no doc is a missing finding.
7. **For survey mode**, the full documentation surface is in scope. Pre-existing gaps and rotted comments are the point.
8. **Anchor findings to file:line.** Cite the specific gap, the specific anti-pattern instance, the specific stale claim.
9. **Stay read-only.** Suggest the better doc; do not write it.

---

## 9. What is NOT a documentation finding

- **Style preferences the team has deliberately chosen.** If the repo's `CONTRIBUTING.md` says "we use NumPy-style docstrings," the agent does not flag NumPy-style docstrings as inconsistent with Google style.
- **Comments documenting deliberate weirdness.** A comment that says "this looks unusual because we measured the obvious form and it was 10x slower" is the documentation that justifies the form.
- **Internal-only code without `pub` / `export`.** Apply lower coverage expectations to private helpers; the agent's primary scrutiny is at API boundaries.
- **Auto-generated bindings, lock files, fixture data**. Documentation expectations there are minimal.
- **Code in performance-critical hot paths where excess prose adds maintenance cost.** A small kernel of well-documented contract is more useful than a wall of docs on every iteration.

---

## Summary checklist (for the subagent's review pass)

1. Public API items have doc comments. The compiler / linter can enforce (`#[deny(missing_docs)]`, `eslint-plugin-jsdoc`, `pydocstyle`).
2. Doc comments lead with a one-sentence summary, per rustdoc / godoc / DocC convention.
3. Required sections present where applicable: `# Safety` on every `unsafe fn`, `# Panics` on every panicking function, `# Errors` on every `Result`-returning function, `@throws` on every throwing function.
4. Examples are runnable and meaningful. Compiled (doctests, examples directory) and showing real usage, not `console.log('hi')`.
5. Comments explain why, not what. Restating-the-code comments flagged.
6. TODO / FIXME / HACK comments have owners or tickets. Rotted ones flagged.
7. Cross-references use intra-doc links (rustdoc), `{@link}` (TSDoc), or godoc identifier names. Bare backticks rot.
8. Deprecation includes a `note` / replacement path. Not just `@deprecated` with no guidance.
9. README has the conventional sections for the ecosystem (npm / cargo / pypi / etc.).
10. CHANGELOG follows Keep a Changelog (when one exists).
11. No mixing of Diátaxis quadrants in a single page.
12. Docs match the code at the diff being reviewed. A signature change without a doc update is incomplete.
13. No stale "see also"-style cross-refs pointing to renamed or removed items.
14. No documentation in a wiki for code in a repo with no sync mechanism.
