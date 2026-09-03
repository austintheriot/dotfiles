---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Information Architecture

A reference for evaluating how a product organizes, names, and exposes its content and functions. Used by the `information-architecture` subagent. The scope is *whether a person can find the thing and understand where they are*: the grouping scheme, the labels on it, the navigation that expresses it, the search that backstops it, and the addresses that make any of it linkable.

Distinct from `interaction-design.md` (what happens once you are on the right screen), `visual-hierarchy.md` (whether the grouping is visually legible, as opposed to conceptually right), `content-design.md` (the wording of body copy and messages, where this lens owns *labels* specifically), `api-design.md` (which owns the same URL-as-contract discipline for machine consumers), and `accessibility.md` (landmarks, heading outline, skip links as conformance).

The core thesis, from Abby Covert: **you cannot organize what you have not defined.** Most information-architecture defects are vocabulary defects wearing a structural costume. Two labels that mean the same thing, one label that means two things, or a category whose members share nothing except that nobody knew where else to put them. Fix the language and the structure usually follows.

The second thesis: **the structure is a claim about the domain, and the person will believe it.** If two things sit in one category, the person infers they are the same kind of thing. If a function sits under Settings, they infer it is configuration rather than an action. The architecture teaches a model whether or not anyone intended to teach one.

---

## 1. The four systems

Rosenfeld, Morville and Arango's framing (*Information Architecture*, the "polar bear book," now in its fourth edition) divides the work into four interacting systems. A review that treats navigation as the whole subject will miss most of the available findings.

1. **Organization systems**: how content is grouped and structured.
2. **Labeling systems**: what those groups and items are called.
3. **Navigation systems**: how a person moves through the structure.
4. **Search systems**: how a person queries it directly.

They fail in each other's territory. A search system exists partly to rescue people from an organization system that did not match their model. Heavy search usage on a small, well-organized product is a signal about the organization, not a success of the search.

---

## 2. Organization schemes

### Exact schemes

Alphabetical, chronological, geographical, numerical. **Unambiguous, mutually exclusive, and require the person to already know what they are looking for.** Excellent for known-item lookup (a lexicon, a directory, an archive), useless for exploration. Their great virtue is that they need no agreement: nobody argues about where "M" goes.

### Ambiguous schemes

Topical, task-oriented, audience-oriented, metaphor-driven. **These are where all the value and all the arguments are**, because assigning an item to a topic is a judgment and reasonable people differ.

- **Topical** requires domain agreement and is the default for content.
- **Task-oriented** ("Import," "Review," "Export") works when the product is a set of verbs, which is most tools.
- **Audience-oriented** ("For admins," "For reviewers") is tempting and usually a mistake: people do not reliably self-identify, roles overlap, and it duplicates content across audiences that then drifts apart. Reserve it for cases where the audiences genuinely never overlap.
- **Metaphor-driven** (desktop, folders, trash) buys instant comprehension and then constrains you forever to the metaphor's limits. Tognazzini's rule applies: abandon a metaphor the moment it holds the design back.

### The hybrid failure

The commonest structural defect: a single navigation level that mixes schemes. "Books, Lexicon, Grammar, Settings, Admin, Help" mixes objects, functions, and audiences. It is often unavoidable at the top level of a real product, and it is survivable when each item is individually unambiguous. It becomes a defect when the mixing makes membership unpredictable, so a person cannot guess which of two plausible parents holds the thing they want.

### Polyhierarchy

One item genuinely belonging in several places is normal, and forcing a strict tree produces arbitrary choices. Faceted classification is the principled answer: describe items by independent attributes (type, status, date, owner) and let the person compose the view. It is more work up front and it removes the "where does this go" argument permanently. Where a strict hierarchy is retained, cross-links from the other plausible parents are the cheap mitigation.

---

## 3. Labeling, and information scent

### The rule

**Use the person's words, not the system's.** The most common leak is the schema: a table name, a column name, an internal service name, or an enum value appearing in the interface. `corpus_reference_conflict` is a table; "conflict" is a word. `bulk_verified` is a status value; "checked in bulk" is a phrase. This is Nielsen's second heuristic, and it is the highest-frequency labeling finding in any codebase where the interface was built from the schema outward.

### Information scent

Pirolli and Card's **information foraging theory** (PARC) is the mechanism underneath every navigation judgment, and it is underused. People forage for information the way animals forage for food: they follow **scent**, meaning proximal cues that suggest a distal source, and they abandon a patch when the scent weakens. The design consequences are direct and testable:

- A link's label is a scent signal. A weak label ("More," "Details," "Click here," a bare chevron) has no scent, so the person cannot estimate whether the path pays.
- **Scent must strengthen as you descend.** If a category's children do not obviously belong to it, the person will back out even when the target is one level down.
- **Pogo-sticking** (down into an item, back out, down into the next) is the observable signature of weak scent, and it is diagnosable from analytics without a lab.
- Generic category labels ("Resources," "Tools," "Other," "Misc") are scentless by construction. "Other" is not a category; it is an unfinished decision.

### Consistency

One concept, one term, everywhere: interface, documentation, error messages, and support. Synonym rotation is not variety, it is a defect, and it is exactly the rule `~/.claude/rules/simplified-technical-english.md` states for prose. A product that calls the same object a "book," a "text," a "work," and a "corpus" in four screens has four objects as far as the person is concerned.

Two directions of failure, both worth flagging:

- **One label, two meanings.** "Export" meaning both "produce a PDF" and "serialize for transfer."
- **Two labels, one meaning.** "Delete" and "Remove" used interchangeably for the same operation, which implies a distinction that does not exist.

---

## 4. Navigation

### Kinds

- **Global**: present everywhere, expresses the top-level structure.
- **Local**: within a section.
- **Contextual**: inline links between related items. Underrated, and where polyhierarchy actually gets served.
- **Supplemental**: search, sitemap, index, help. The safety net.

### Wayfinding

Three questions, all of which need answers on every screen: **where am I, where can I go, where have I been.** Concretely: a current-location indicator in the navigation, visible affordances for the available moves, and visited-state or history where it matters. The commonest omission is the first, especially in single-page applications where the highlighted nav item is not updated on programmatic navigation.

### Breadth versus depth

The genuine trade-off: **depth increases the number of decisions at which a person can go wrong; breadth increases the cognitive cost of each decision.** Broad-and-shallow generally beats narrow-and-deep for findability, because a wrong turn is cheap to recover from at the top and expensive four levels down. But there is no magic number, and see the folklore correction below.

The right question is not "how many items" but **"is each choice at this level unambiguous."** Nine well-differentiated items beat five where two overlap.

### Folklore corrections

- **The three-click rule is not supported.** The claim that users abandon after three clicks was tested (notably by Joshua Porter, 2003) and no relationship was found between the number of clicks and either task success or satisfaction. What predicts success is whether each click feels like progress, which is information scent again. **Do not flatten a hierarchy to satisfy a click count.** A flattening that produces ambiguous top-level categories makes findability worse while satisfying a rule that was never real.
- **Miller's 7±2 does not bound menu length.** See `interaction-design.md` section 3. A visible menu is recognition, not recall.
- **"Above the fold" as an absolute is long dead**, but the underlying point survives in a narrower form: content below the fold is seen by fewer people and requires an incentive to reach. Scent, again.

### First-click research

Whether the first click is correct is a strong predictor of eventual task success. This makes the top-level structure disproportionately important relative to the depth below it, and it makes first-click testing the cheapest useful evaluation available.

---

## 5. Search

- **Search and browse are complementary**, and which one a person uses depends on whether they know what they are looking for. Both must exist for any non-trivial content set.
- **Search is not a fix for bad organization.** It rescues individual sessions and hides the defect from you.
- **The zero-results state is part of the search system**, and it is almost always undesigned. It should say what was searched, suggest a correction, offer a broader query, and provide a browse entry point. See `interaction-design.md` section 4.
- **Fuzzy matching where the person may not know the spelling.** In domains with unfamiliar orthography (historical text, chemical names, transliterated proper nouns), exact-match search is a wall. Trigram similarity or equivalent is the baseline expectation.
- **Faceted filtering after search** is what converts a large result set into a usable one, and it is the same faceted classification that solves the polyhierarchy problem.
- **Scoped search must say its scope.** A search box that silently searches only the current section produces confident wrong conclusions ("it is not in the system").

---

## 6. URLs are information architecture

A URL is a public contract, a bookmark, a shared link, and a piece of navigation the person can read. Every rule from `api-design.md` about identifiers applies, plus:

- **The URL should express the structure**, so a person can read where they are and can often edit it to go somewhere sensible.
- **Every meaningful view state belongs in the URL**: the tab, the selected item, the filters, the sort, the page. If a person cannot send a colleague a link to what they are looking at, the state is trapped.
- **Distinguish state that belongs in the path from state that belongs in the query.** Identity in the path, view configuration in the query, ephemera in neither.
- **URLs are forever.** Changing one breaks every bookmark and every link anyone has shared. Redirect, do not rename silently.
- **Slugs should be stable and human-meaningful**; a slug that is derived from a mutable title will change when the title is edited, which breaks the previous point.

---

## 7. Evaluation methods, and what each actually tells you

- **Open card sort**: participants group items and name the groups. Generative. Tells you how people naturally categorize and what vocabulary they use. Around 15 to 30 participants for stable patterns.
- **Closed card sort**: participants sort into your categories. Evaluative of the categories, not of the labels.
- **Tree testing** (reverse card sort): given only the structure as text, participants are asked to find things. Isolates the structure from visual design entirely, which is its whole value. Tells you where people go wrong and on which label. Commonly run with 15 to 50 participants.
- **First-click testing**: cheapest of the set, strong predictive value per section 4.
- **Analytics as a signal, not an answer**: pogo-sticking, internal search queries (a direct list of what people could not find, and in their own words), and dead-end pages. Internal search logs are the highest-value and least-used information-architecture artifact in most products.

The honest caveat that applies to all of these: **at solo or pre-launch scale, none of them is available**, and this lens then has to argue from structure and vocabulary alone. Say so rather than recommending a study nobody will run.

---

## 8. Schools of thought that genuinely disagree

### Taxonomy-first versus search-first

- **Taxonomy-first**: a well-designed structure teaches the domain, supports exploration, and serves people who do not yet know what to ask for.
- **Search-first**: at sufficient scale and with good enough ranking, browsing is a fallback and effort belongs in retrieval. The historical evidence for the strong version is real (large content sites where search dominates traffic).
- The resolution is content volume and task type. Known-item retrieval favours search; exploration, comparison, and learning the shape of a domain favour structure. A product whose users must *understand* its object model, which is most professional tools, cannot substitute search for architecture, because search answers questions the person already knows how to ask.

### Structure versus emergence

The tagging-and-folksonomy position holds that imposed taxonomies are always wrong at the edges and that user-generated tags plus search adapt better. The counter is that folksonomies have no synonym control, no hierarchy, and a long tail of near-duplicate tags, so they degrade as they grow. Practical middle: a controlled vocabulary for the spine, free tags for the edges, and a periodic reconciliation, which is exactly what a well-run lexicon or grammar list is.

### Audience-based organization

Persistently popular with stakeholders and persistently criticized by practitioners, for the reasons in section 2. Worth naming as contested when a review encounters it, rather than flagging it flatly.

---

## 9. Anti-pattern catalog

- **Schema vocabulary in the interface.** Table names, column names, enum values.
- **"Other," "Misc," "Resources," "Tools"** as categories.
- **Scentless links**: "More," "Details," "Click here," bare chevrons.
- **Two labels for one thing**, or one label for two.
- **Mystery hierarchy**: a category whose members share nothing predictable.
- **No current-location indicator**, especially after programmatic navigation.
- **Untitled or duplicate page titles**, which break history, tabs, and bookmarks.
- **Undesigned zero-results state.**
- **Silently scoped search.**
- **View state absent from the URL**, so nothing is shareable.
- **URL derived from a mutable title.**
- **Flattening to satisfy the three-click rule.**
- **A navigation item that is a dead end**: a section header that is also a link to a page containing only its own children.
- **Deep hierarchy with no breadcrumb.**
- **Audience-split content that has drifted** between the audiences.

---

## 10. What to flag, and what not to

**Flag:**
- A label taken from the schema rather than the domain, with the domain word proposed.
- Inconsistent terminology across surfaces, with all the observed variants listed.
- A category whose membership rule cannot be stated in one sentence.
- Weak or absent information scent on a path the person is expected to take.
- Missing wayfinding: no location indicator, no back path, no breadcrumb where depth warrants.
- Meaningful state absent from the URL, named as the specific view that cannot be shared.
- A search system with no zero-results design, no fuzzy matching in a domain that needs it, or an unstated scope.
- Mixed organization schemes at one level where membership becomes unpredictable.
- An audience-oriented split that will duplicate and drift.

**Do not flag:**
- Visual treatment of navigation. `visual-hierarchy`.
- Whether the nav is keyboard-reachable, whether landmarks exist, heading-outline conformance. `accessibility`.
- Body copy, error wording, empty-state prose. `content-design`, except labels, which are this lens.
- Route implementation, framework choice, router mechanics. Not a lens finding.
- Structure in a product too small to have a structure problem. Three screens do not need an architecture review.
- A recommendation to run a card sort or tree test at a scale where no participants exist. Argue from vocabulary and structure instead.

Every finding names the person's question and where the structure fails to answer it: not "the navigation is confusing" but "`Conflicts` and `Grammar List` both hold corpus-wide disputes, and nothing in either label predicts which one holds a disputed construction, so a person will check both."
