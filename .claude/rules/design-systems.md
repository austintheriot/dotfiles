---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Design Systems, Tokens, and Drift

A reference for evaluating a design system from an architecture-and-durability lens. Used by the `design-systems` subagent. The scope is *the contract between design intent and shipped pixels*: how values are named and layered, what a component promises its consumers, how variants are modelled, how themes are expressed, who is allowed to change what, and whether what ships still matches what the system says.

Distinct from `visual-hierarchy.md` (whether the values are good), `api-design.md` (which owns the same backward-compatibility discipline at network and library surfaces, and whose rules apply here almost verbatim to component props), `accessibility.md` (whether the components conform), `interaction-design.md` (whether the states are complete), and `build-systems.md` (how the token build runs).

The core thesis: **the token graph is the interface, and everything above it is replaceable.** Component implementations get rewritten, framework choices change, whole codebases get regenerated. What survives is the set of named decisions and the shape of the components' public surfaces. That makes token naming and prop design worth disproportionate care, exactly as `~/.claude/CLAUDE.md`'s "interface boundaries are paramount" argues for code generally.

The second thesis, and the reason this lens matters more every year: **drift is the default state, not an incident.** zeroheight's 2026 Design Systems Report found only about 8% of teams describe their system as "very stable" while roughly 44% call it unstable or very unstable. A system is not a thing you build; it is a thing that decays unless something detects the decay.

---

## 1. Token architecture

### The three tiers

The settled industry structure, and each layer must reference **only the layer below it**:

1. **Primitive** (also: option, global, core, base). Raw values with no meaning. `color-blue-500`, `space-4`, `font-size-3`. These name a *value*.
2. **Semantic** (also: alias, decision, system). Named by role. `color-action`, `color-surface-primary`, `color-text-on-surface`, `space-inset-md`. These name an *intent* and point at a primitive.
3. **Component**. Scoped to one component's part. `button-background-primary`, `input-border-error`. These point at a semantic.

Why the strictness pays: changing a raw value cascades everywhere automatically, and renaming or re-pointing a role leaves every component untouched. Dark mode is then a re-pointing of the semantic layer at different primitives, not a second set of components.

### The semantic layer is the one that matters

It is the layer that carries meaning, and it is the one teams skip. A codebase where components reference primitives directly (`color: var(--blue-500)` inside a Button) has two tiers and no theming story: you cannot introduce a second brand, a dark mode, or a "destructive" meaning without touching every component.

**Keep alias names brand-agnostic and mode-agnostic.** `color-surface-primary` and `color-text-on-surface` work regardless of which brand or theme is active. `color-brand-blue-bg` bakes both a brand and a value into a role and will be a lie the first time either changes.

### Naming rules

- **Name the job, not the value.** `color-danger`, not `color-red`. The commonest single defect: a semantic token whose name contains a colour word.
- **Name the job, not the place.** `color-text-secondary`, not `color-sidebar-text`. Place-named tokens fragment the moment the same treatment appears elsewhere, and you get `sidebar-text` used in a modal.
- **One consistent term order**, applied everywhere. A common shape is `category-property-variant-state`: `color-background-danger-hover`. Which order matters far less than that there is one.
- **Do not encode the value in the name at the semantic tier**, and do not encode the tier in the name at the primitive tier beyond a scale position.
- **Resist over-tokenization.** A component token for every property of every part produces a graph nobody can hold, and it makes each component impossible to restyle without adding tokens. Component tokens earn their place when a consumer genuinely needs to re-point that specific part.

### The interchange format

The **W3C Design Tokens Community Group** format reached its **first stable version in October 2025** (Format Module 2025.10). Shape: a JSON tree where a token is an object with `$value` and `$type`, aliases are written as `{path.to.token}` references, and groups carry shared `$type`. Tooling support is broad (Figma, Penpot, Sketch, Tokens Studio, Style Dictionary, Terrazzo). Style Dictionary has first-class DTCG support from v4; support for the 2025.10 revision is still landing in v5.

For review purposes: a token file in a bespoke JSON shape is not a defect on its own, but a new system choosing a bespoke shape over DTCG in 2026 is choosing to write its own converters and to opt out of every tool that speaks the standard. Say that, once, and move on.

### Modes and themes

Express a theme as an alternative binding of the semantic layer, never as a parallel component set or an override stylesheet. The two smells:

- A `.dark` stylesheet that redeclares component rules rather than re-pointing tokens. It will drift the first time a component changes.
- Semantic tokens whose names presuppose one mode (`color-text-black`), which cannot be re-pointed honestly.

---

## 2. Component API design

Component props are a public API and every rule in `api-design.md` applies: public surfaces are effectively forever, Hyrum's Law means consumers depend on observable behaviour rather than documented behaviour, and it is easier to add than to remove. The system-specific parts:

### Variants, not boolean piles

The failure progresses predictably. It starts as `<Button primary>`, then `<Button primary secondary>` becomes representable and meaningless, then `isPrimary`, `isDanger`, `isGhost`, `isLarge`, `isSmall`, `isLoading`, `isDisabled` multiply into a state space where most points are invalid.

Model mutually exclusive choices as a **single enumerated prop** (`variant="primary" | "secondary" | "ghost" | "danger"`, `size="sm" | "md" | "lg"`), which makes illegal states unrepresentable, and reserve booleans for genuinely orthogonal, genuinely binary states (`disabled`, `loading`). This is the same finding `fp-types` would make about the underlying type, and the same one `api-design` makes about growing boolean fields. Cite whichever is nearer.

### Composition over configuration

A component that accumulates props for every possible child arrangement (`leftIcon`, `rightIcon`, `badge`, `subtitle`, `trailingAction`) is asking for a compound API instead:

```tsx
<Card>
  <Card.Header>...</Card.Header>
  <Card.Body>...</Card.Body>
</Card>
```

The test: if a prop's value is markup, or if adding one more arrangement means adding one more prop, the component wants slots or children, not configuration.

### Escape hatches

**A system with no escape hatch gets forked.** Somebody will need one padding value the system does not have, at 5pm, and the only available move will be to copy the component. Provide a bounded exit: `className` pass-through, a `style` prop, `asChild` or polymorphic `as`, and ref forwarding. Then treat heavy use of the escape hatch as **evidence for a gap in the system**, and mine it: a prop everyone overrides the same way is a missing variant.

Ref forwarding specifically is not optional. A component that swallows its ref cannot be focused, measured, or positioned by a consumer, which breaks tooltips, popovers, focus management, and virtualization.

### Controlled and uncontrolled

Pick a stance per component and document it. The defect is the half-controlled component that accepts `value` but also keeps internal state, so the two disagree under fast input or external reset. Where both are supported, the convention (`value` + `onChange` for controlled, `defaultValue` for uncontrolled) is worth following exactly, because it is what Jakob's Law predicts consumers expect.

---

## 3. Atomic design, and the honest state of it

Brad Frost's *Atomic Design* (2013, book 2016) proposed atoms, molecules, organisms, templates, pages. Its lasting contributions are real: build systems rather than pages, and think in a hierarchy of composition.

The critiques are also real and are not fringe:

- **The category boundaries are not decidable.** Whether a card is a molecule or an organism has no answer, and teams spend meeting time on it. The taxonomy generates argument without generating decisions.
- **It does not scale cleanly to enterprise complexity.** The 2013 web is not the 2026 application, and five tiers do not describe a component set with layout primitives, compound components, providers, and hooks.
- **The chemistry metaphor is not load-bearing.** Nothing follows from atoms combining into molecules except the composition idea, which does not need the metaphor.

Frost's own position is that atomic design is not dogma and that whatever taxonomy helps the organization communicate is the right one. That is the position to take in review: **a system with a clear, followed, documented composition taxonomy is healthy regardless of what the tiers are called; a system arguing about tier membership is spending on the wrong thing.** Do not flag a system for not being atomic. Do flag a system with no composition story at all.

---

## 4. Governance

Nathan Curtis's three team models (2015) remain the standard vocabulary:

- **Solitary**: one team builds a system primarily for its own needs; others may adopt it.
- **Centralized**: a dedicated team owns and distributes the system and does not ship product.
- **Federated**: representatives from several product teams share ownership, typically at partial allocation (Curtis's often-quoted figure is roughly 25% of their time).

Curtis's later and more useful correction, in "The Fallacy of Federated Design Systems," is that pure federation does not work on its own: **successful systems have a central team *and* seek federated participation.** Federation without a centre produces a system nobody is accountable for; a centre without federation produces a system that does not fit the products.

For review, the governance findings that matter are concrete rather than organizational:

- **Is there a documented way to propose a change?** A system with no contribution path gets forked instead of extended.
- **Is there a documented way to say no?** A system that accepts everything becomes a catalogue.
- **Who breaks ties on naming?** Naming is where systems stall, and an unowned naming decision is an unmade one.
- **Is adoption measured?** "We have a design system" and "our products use it" are different claims, and only the second one matters.

At one developer, all three models collapse and none of this applies. The finding that survives at any size is whether the decisions are *written down*, because the alternative is that they live in one person's head and are lost on contact with a regenerated codebase.

---

## 5. Versioning and breaking changes

Components version like libraries and the same discipline applies:

- **Semver, honestly.** A visual change that shifts layout is breaking for consumers whose pages depend on the old size, whatever the type signature says. This is Hyrum's Law arriving in CSS.
- **Deprecate before removing**, with the replacement named in the deprecation. A `@deprecated` tag with no migration path is a complaint, not a deprecation.
- **Codemods for mechanical migrations.** A rename across 400 call sites is a codemod, not a request.
- **The breaking-change taxonomy for components**: removing a prop, narrowing a prop's accepted type, changing a default, changing the rendered DOM structure (breaks consumer CSS and tests), changing an ARIA role or the focus order (breaks assistive technology and tests), and changing a token's *meaning* while keeping its name (the worst, because it is invisible in every diff).
- **Renaming a token is a breaking change; re-pointing one is usually not.** That asymmetry is the payoff of the semantic tier and it is worth stating explicitly in a review that finds components referencing primitives.

---

## 6. Drift

### Why it happens

Drift is the gap between what the system says and what ships. It accumulates from ordinary pressure: a deadline where copying a component is faster than extending it, a one-off value that never gets tokenized, a design file updated without the code, a component forked in a product repo.

**AI-assisted development accelerates every one of these.** Generated implementations do not reach for a project's tokens unless told to; they emit plausible literal values. Each generation is locally reasonable and globally divergent, and the divergence does not show up as an error anywhere. This is the specific reason the token file, and not the component implementation, is the artifact worth defending: implementations are now cheap and regenerable, so the durable thing has to be the named decisions the implementations reference.

### What to look for in code

- **Hard-coded values that duplicate a token's value.** `padding: 16px` where `--space-md` is 16px. This is the primary drift signal and it is greppable.
- **Near-miss values.** `#f4f1eb` where the token is `#f4f1ea`. Worse than an exact duplicate, because it will never be caught by a search for the token's value and it is visible to nobody except in aggregate.
- **A value that exists in no scale.** `padding: 14px` in an 8/4 system.
- **One-off components** in a product directory that shadow a system component by name or by shape.
- **Overrides on system components**, especially repeated ones. Three consumers overriding the same property the same way is a missing variant, and the third occurrence is the finding.
- **Token defined and never referenced.** Dead decisions, which make the graph harder to read and suggest the design and code sides have diverged.
- **Component rendered with an inline `style` for something the system covers.**

### What to look for in process

- Is there any automated check that new code uses tokens? A lint rule against raw hex in component files, or against numeric values outside the spacing scale, converts drift from invisible to blocking.
- Is the token file generated from one source, or maintained twice? Two hand-maintained copies is not a source of truth, it is a diff waiting to happen.
- Are visual regression snapshots run against the system's own components? Without them, a token re-point that breaks one component ships silently.

---

## 7. Documentation as part of the system

A component nobody can find is a component nobody uses, and an undocumented system is a system that will be reimplemented next to itself. The minimum that changes behaviour:

- **Usage guidance, not only an API table.** When to use this component and when to use a different one. The "when not to" section is the one that prevents misuse and the one always missing.
- **Live examples of every variant and state**, including the states from `interaction-design.md` section 4. A component library whose docs show only the ideal state has undocumented half its surface.
- **Accessibility notes per component**: what it handles, what the consumer must supply (a label, a description, an `aria-live` region).
- **The token reference**, generated from the token source rather than transcribed.

Storybook or an equivalent catalogue is the usual vehicle, and its real value in review is that it is the only place where every state of every component can be seen at once.

---

## 8. Schools of thought that genuinely disagree

This is a live architectural argument with no consensus, and a review should locate a project within it rather than assume one answer.

### The component-library school

Ship a versioned package of styled components. **Strengths**: one upgrade path, consistency by default, fixes propagate, accessibility handled once. **Weaknesses**: every customization is a negotiation with the maintainer or an escape hatch; consumers wait for releases; the library's opinions are load-bearing forever; bundle cost is paid for components you do not use unless tree-shaking actually works.

### The utility-first school (Tailwind and relatives)

Compose from constrained utility classes; the design system *is* the configured scale. **Strengths**: the constraint is enforced at the point of authoring, there is no naming problem because there are no component names, and it is very hard to write an off-scale value by accident. **Weaknesses**: no semantic layer unless you build one, so intent lives in class strings; repeated markup rather than a named abstraction; and multi-brand theming needs a token layer bolted back on. The pointed critique from the systems side is that utility-first optimizes the author's convenience over the reader's comprehension. The pointed critique from the utility side is that component libraries generate more abstraction than most teams need and calcify decisions early.

### The headless school (Radix, Base UI, Ark, Headless UI)

The library supplies behaviour, state, and accessibility; you supply every pixel. **Strengths**: the hardest and most-often-botched parts (focus management, keyboard interaction, ARIA, collision-aware positioning) are solved by people who specialize in it, while the design stays entirely yours. **Weaknesses**: you own all the styling, so consistency is back on you; and you have taken a behavioural dependency whose DOM structure your CSS now depends on.

### The copy-paste ownership school (shadcn/ui and relatives)

The component source is copied into your repo rather than installed. **Strengths**: total ownership, no upstream negotiation, no version pinning, and you can add a variant without filing an issue. Bundle cost is only what you added. **Weaknesses**: **there is no upgrade path.** Upstream fixes, including accessibility fixes, do not reach you unless you go and get them, and after six months of local edits you cannot take them cleanly. It is a fork by design, and forks are cheap to create and expensive to maintain.

### How to review against this

Do not flag a project for choosing one of these. Flag the **mismatch between the chosen model and the practices around it**: a copy-paste model with no record of which upstream version each component came from; a headless model with no token layer, so every consumer invents spacing; a component-library model with no escape hatch, so products fork; a utility-first model with no configured scale, so the constraint does nothing.

---

## 9. Anti-pattern catalog

- **Two-tier system.** Primitives referenced directly by components. No theming is possible.
- **Semantic token named for its value.** `color-blue-action`.
- **Semantic token named for its location.** `sidebar-bg`.
- **Mode baked into a name.** `color-text-black`.
- **Token graph with cycles or upward references.** A primitive that points at a semantic.
- **Boolean prop pile** where an enum belongs.
- **Prop drilling markup.** Props whose values are elements.
- **No escape hatch**, followed inevitably by forks.
- **Swallowed ref.**
- **Half-controlled component.**
- **Dark mode as an override stylesheet.**
- **Docs showing only the ideal state.**
- **A "v2" component living beside "v1" indefinitely**, with no deprecation date and no codemod.
- **Design file and code as two sources of truth**, reconciled by meetings.
- **Silent token re-meaning**: same name, different intent, invisible in review.

---

## 10. What to flag, and what not to

**Flag:**
- A hard-coded value that duplicates or near-misses an existing token, with both values named.
- A component referencing a primitive where a semantic token exists or should.
- A semantic token named for a value, a place, or a mode.
- A boolean prop set that admits invalid combinations, with the invalid combination named.
- A missing escape hatch, or a repeated identical override that indicates a missing variant.
- A breaking component change shipped without a version bump, deprecation, or codemod, including a DOM-structure or ARIA-role change.
- A token re-pointed in a way that changes its meaning without changing its name.
- Absence of any automated drift check in a system large enough to drift.
- A model mismatch from section 8.

**Do not flag:**
- The choice of methodology, framework, or library on its own.
- Whether the values are aesthetically right. `visual-hierarchy`.
- Whether the component is accessible. `accessibility`. This lens owns whether the accessibility contract is *documented and stable across versions*.
- Organizational governance at solo scale.
- Missing tokens in genuinely one-off surfaces (a marketing page, a one-time export) where the system was never claimed to apply.
- Bespoke token format on its own, once noted.

Every finding names the contract and who breaks: not "this should use tokens" but "`Button.tsx:42` hard-codes `#241f18`, which is `--color-ink`; a theme re-point will change every other use and silently skip this one."
