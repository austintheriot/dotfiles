---
paths:
  - "**/*.{tsx,jsx,vue,svelte}"
  - "**/*.{swift,kt,kts}"
  - "**/*.{html,htm}"
---

# Accessibility Review Principles

A reference for evaluating frontend / UI code for accessibility (a11y). Used by the `accessibility` subagent. Grounded in WCAG 2.1 / 2.2 (Web Content Accessibility Guidelines), the WAI-ARIA Authoring Practices Guide (APG), and platform-specific guides (Apple Human Interface Guidelines, Android Accessibility Developer Guide).

The core thesis: accessibility is not "added later" or "for blind users." It is a property of the system that decides whether your code is usable by people with motor impairments, cognitive impairments, visual impairments (low vision, color blindness, blindness), or temporary limitations (broken arm, bright sunlight, noisy environment). 15-20% of users have some disability; many more benefit from accessibility (keyboard shortcuts, captioning, etc.).

WCAG's four principles (POUR): **Perceivable, Operable, Understandable, Robust**. Every finding fits one of these.

---

## When to invoke this lens

Signal-driven, frontend-focused:

- HTML / JSX / Vue / Svelte component code with rendered UI
- Native iOS (SwiftUI / UIKit) or Android (Compose / Views) view code
- Custom widgets / form controls
- Anywhere ARIA attributes appear
- Color / theme / styling that affects contrast or motion
- Anywhere keyboard interaction is implemented (key handlers, focus management)

If the change is server-only / backend / CLI / config, this lens does not fire.

---

## Perceivable

### Text alternatives for non-text content
- **`<img>` requires `alt`**. Decorative images get `alt=""` (not omitted, not "image"). Functional images describe the *function*, not the literal image ("Save" not "floppy disk icon").
- **Icon buttons need accessible names**: `<button aria-label="Close" />`, not bare `<button><svg/></button>`.
- **SVGs that convey meaning need `<title>`** or `aria-label`; decorative SVGs need `aria-hidden="true"`.
- **Video / audio**: captions for spoken content, transcripts for audio-only, audio descriptions for visual-only content.
- **Charts / data visualizations**: alternative text or tabular data fallback.

### Color
- **Don't convey information by color alone**. Red text for errors plus an icon plus text; not red text alone.
- **Contrast ratios** (WCAG 2.1 AA minimums): 4.5:1 for normal text, 3:1 for large text (18pt+ or 14pt bold), 3:1 for UI components and graphical objects. AAA goes to 7:1 / 4.5:1.
- **Avoid fixed colors** that fight the user's preferences. Respect `prefers-color-scheme`; don't override `color-scheme`.
- **Focus indicators must be visible** -- 3:1 contrast against the unfocused state.

### Adaptable content
- **Semantic HTML over divs with roles**: `<button>`, not `<div role="button">`. The native element brings keyboard, focus, screen-reader semantics for free.
- **Heading hierarchy is real**: don't skip levels for styling. `<h1>` -> `<h2>` -> `<h3>`, not `<h1>` -> `<h4>`.
- **Landmark roles**: `<main>`, `<nav>`, `<header>`, `<footer>`, `<aside>`. Screen-reader users navigate by landmark.
- **Lists are `<ul>` / `<ol>`**, not styled divs.
- **Tables for tabular data only**, not layout; include `<th>` and `scope`.

### Sensory cues
- Don't rely on "the green button" / "the icon at top right" / "the box on the left" -- screen-reader users have no positional cue.
- Animations / sounds as the *only* signal fail. Always pair with text / state change.

---

## Operable

### Keyboard accessibility (largest single category)
- **All interactive elements reachable by Tab**. If a button can be clicked, it must be tabbable.
- **Focus order matches visual order**. `tabindex` other than `0` and `-1` is almost always wrong.
- **Custom widgets fully keyboard-operable**: a custom dropdown needs arrow keys, Enter, Escape, type-ahead. Follow the WAI-ARIA APG patterns.
- **No keyboard traps**: a modal must let Escape close; a focus-trap must release on close.
- **Visible focus indicator**: never `outline: none` without a replacement. The default browser ring is acceptable; custom rings need contrast.
- **Click handlers on non-interactive elements** (e.g., `<div onClick>`) miss keyboard. Use `<button>` or add `role`, `tabindex`, `onKeyDown`, focus styles. Avoid this -- prefer semantic element.

### Focus management
- **After a route change** in a SPA, focus should move to the new page's main heading or a known landmark; otherwise the user is stranded.
- **After opening a modal**, focus moves into the modal; Tab is constrained to within it; Escape closes; focus returns to the trigger.
- **After deleting an item**, focus moves to the next item or to a landmark, not into the void.
- **`autofocus`** sparingly -- can disorient screen-reader and keyboard users mid-page.

### Timing
- **No time limits** on user actions, or the limit is adjustable, extendable, or disabled.
- **Auto-updating content** (carousels, live feeds) is pauseable or doesn't move automatically. Animations that auto-play violate.
- **Auto-dismissing notifications** stay long enough to read; toasts that vanish in 2s fail.

### Motion / animations
- **Respect `prefers-reduced-motion`**. CSS:

  ```css
  @media (prefers-reduced-motion: reduce) {
    *, *::before, *::after {
      animation-duration: 0.01ms !important;
      transition-duration: 0.01ms !important;
    }
  }
  ```

- **Parallax / scroll-jacking** trigger vestibular disorders.
- **Flashes** more than 3 per second can trigger seizures. Avoid.

### Input modalities
- **Touch targets at least 24x24 CSS pixels** (WCAG 2.5.8 AA), 44x44 better (Apple HIG).
- **Don't require fine motor precision** -- drag handles, hover-only revelations, exact double-clicks.
- **Pointer events work via keyboard too**: `mouseover` content also `focus`-visible.

---

## Understandable

### Readable text
- **Set `<html lang>`** -- screen readers pick the right voice.
- **Language changes mid-page**: `<span lang="fr">`.
- **Abbreviations**: `<abbr title="Cascading Style Sheets">CSS</abbr>` (first use).
- **Plain language**: prose readable at 8th-grade level. Tools (Hemingway, Flesch-Kincaid) help.

### Predictable
- **Don't auto-submit forms** on input changes without warning.
- **Don't move focus unexpectedly** -- a user typing shouldn't have their cursor jump.
- **Consistent navigation**: same nav in same place on every page.
- **Consistent identification**: same icon means same thing across the app.

### Input assistance
- **Form labels**: `<label for="email">` or wrapping `<label>`. **Placeholder is NOT a label** -- it disappears on focus and has poor contrast.
- **Required fields**: marked with text ("required") plus `aria-required="true"`. Asterisks alone fail.
- **Error identification**: which field, what went wrong, how to fix. `aria-invalid="true"` plus `aria-describedby` pointing to the error text.
- **Autocomplete attributes**: `autocomplete="email"`, `autocomplete="new-password"`, etc. Help users with cognitive impairments and password managers.
- **Don't disable submit until valid** -- the user can't see what's missing. Show errors on submit attempt.

---

## Robust

### Compatibility with assistive tech
- **Valid HTML / parseable markup**. Screen readers stumble on malformed.
- **Use ARIA only when native won't do**. The first rule of ARIA: don't use ARIA. Then: don't change native semantics. Then: keyboard support is your problem now.
- **Status messages** via `aria-live="polite"` / `aria-live="assertive"` / `role="status"` / `role="alert"`. Polite for non-critical updates; assertive interrupts the current speech.
- **Custom widgets follow WAI-ARIA APG patterns** -- combobox, dialog, listbox, menu, tabs, tree have well-defined keyboard interactions and ARIA roles. Match the pattern; don't invent.

### Live regions and dynamic content
- **Newly-inserted content** isn't automatically announced -- needs `aria-live` or to receive focus.
- **`aria-busy`** during loading; remove when done.
- **Modal dialogs** need `role="dialog"` or `role="alertdialog"`, `aria-labelledby`, `aria-modal="true"`.

---

## Common framework-specific anti-patterns

### React
- **Lists keyed by index** mess with assistive-tech tracking when items reorder.
- **`dangerouslySetInnerHTML`** on user content -- XSS risk plus accessibility risk (no semantic check).
- **Conditional rendering swapping focused elements**: focus is lost.
- **Portal-rendered modals** without focus management.
- **Click on non-button**: `<div onClick={...}>`. Use `<button>` instead.
- **`<a>` without `href`** used as a button. Use `<button>` or give the `<a>` `role="button"` plus keyboard handlers (preferably the former).

### Vue / Svelte
- Same issues as React; framework-specific syntax.
- Slot content can lose role semantics -- verify in rendered output.

### Mobile native
- **iOS**: Every `View` that's interactive needs an `accessibilityLabel`. `accessibilityHint` for non-obvious actions. Custom controls need `accessibilityTraits`.
- **Android**: `contentDescription` on `ImageView`, `Button` etc. Use `android:importantForAccessibility` to hide decoratives.
- **Both**: respect Dynamic Type / system font scaling; use semantic sizing not hardcoded points.

---

## What is NOT an accessibility finding

- **Server-only / backend / non-UI code** -- this lens doesn't apply.
- **Generic semantic HTML in templates the framework owns** (e.g., `<div>` for a Next.js layout wrapper).
- **Style preferences** unrelated to accessibility (color palette choices that meet contrast).
- **Native widgets used correctly** -- `<button>` with text and clickable behavior is fine without ARIA.
- **Content the user manages** (CMS-driven; accessibility of that content is the author's responsibility, though tooling that helps them is a feature).

---

## Severity

The `accessibility` subagent uses `panel-contract.md`'s rubric. Specific calibration:

- **blocker**: complete inaccessibility for a class of user -- non-keyboard-operable interactive element, missing labels on form controls, missing `alt` on functional images, contrast below 3:1, missing landmark on a SPA page, keyboard trap with no escape.
- **major**: significant barrier -- non-semantic markup that screen readers will navigate poorly, missing focus management after route change, missing reduced-motion handling on heavy animation, missing live-region for dynamic updates.
- **minor**: friction -- subtle contrast issue at 4.4:1 (just below AA), missing autocomplete attribute, slightly-wrong heading hierarchy.
- **nit**: cosmetic accessibility hygiene -- `alt=""` could be more descriptive on a non-decorative image.
- **insight**: structural -- "this whole modal flow rebuilds focus state on every render; consider adopting a focus-trap library."

Confidence: high when verifiable (a button with no label is a button with no label); medium when reasoned (contrast may be context-dependent; respect of `prefers-reduced-motion` may be tested in non-accessible-mode preview).

---

## Process for the accessibility agent

1. **Read the project's a11y conventions.** Look for `docs/accessibility.md`, a11y sections in CLAUDE.md, any `eslint-plugin-jsx-a11y` config.
2. **Identify the rendered surface** -- is this a web component, a native iOS view, a native Android view? Apply the relevant catalog.
3. **Walk POUR** in priority order: Operable (keyboard) first, then Perceivable (semantics, contrast, alt), then Understandable (labels, errors), then Robust (ARIA, live regions).
4. **For each candidate, name the affected user group**: "keyboard-only users cannot reach this button"; "screen-reader users get no label on this icon button"; "low-vision users at 200% zoom see overlapping content."
5. **Suggest the concrete fix** -- the right semantic element, the right ARIA attribute, the right focus-management hook.
6. **Stay read-only.**
