# Browser Spec / Browser Internals

A reference for the `browser-spec` subagent. The lens: **"this code needs the eyes of someone who's worked in a browser implementation. What does the platform already give you that you've reinvented in JavaScript?"**

The core thesis: the modern web platform (2020 onward) has absorbed an enormous amount of what frameworks used to provide. Custom focus traps, intersection detection, debouncing, pub/sub, modal management, URL parsing, query strings, internationalization, drag-and-drop, history, lazy loading -- the platform now does all of these natively, faster than userland code, and with semantics the browser's accessibility tree and developer tools already understand. **Code written before 2020 didn't have these options; code written in 2025 that ignores them is a liability.**

The agent's value: recognize the shape of the reinvention -- "you wrote a polyfill for X; the platform has Y" -- cite the specific spec, and point to the migration.

Distinct from:
- **`accessibility`** (a11y for screen-reader / keyboard users). We touch platform primitives that also solve a11y (`<dialog>`, Popover), with `See also: accessibility`.
- **`performance`** (measured hot paths). We flag platform misuse that's likely a perf bug (layout thrash, blocking scroll handlers) with `See also: performance`.
- **`webassembly`** (WASM specifically). We touch the WASM↔DOM boundary.
- **`graphics-programming`** (GPU specifics). We touch the canvas / WebGL / WebGPU API surface from the JS side.

---

## Spec mechanics: where authority lives

The single highest-leverage shift since ~2012: **WhatWG Living Standards are the truth**, W3C TR snapshots are usually outdated copies. When in doubt, check WhatWG.

**WhatWG specifications** (continuously updated):
- **HTML Living Standard** (html.spec.whatwg.org) -- HTML parsing, elements, event loop, scripting model, navigation, sessions, storage, drag-and-drop. The most load-bearing spec.
- **DOM Living Standard** (dom.spec.whatwg.org) -- Node / Element / EventTarget / Event / MutationObserver / AbortController. The events spec.
- **Fetch Standard** (fetch.spec.whatwg.org) -- HTTP fetching, CORS, request/response.
- **URL Living Standard** (url.spec.whatwg.org) -- URL parsing.
- **Streams Standard** (streams.spec.whatwg.org) -- ReadableStream / WritableStream / TransformStream.
- **Encoding Standard** (encoding.spec.whatwg.org) -- TextEncoder / TextDecoder.

**W3C specs** still authoritative for: CSS modules (drafts.csswg.org), ARIA, Web Components (Custom Elements, Shadow DOM), WebRTC, WebAuthn.

**MDN** (developer.mozilla.org): practitioner's reference; accurate for the common case; not authoritative on spec edge cases (check WhatWG for those).

**Reviewer's instinct**: when the agent makes a claim about platform behavior, the citation should be a specific spec section or MDN page, not a vague "the browser does this."

---

## Event handling depth

### The three phases

Events propagate in three phases: **capture** (top-down from window → target), **target** (at the target), **bubble** (target → window, bottom-up). Most events bubble; some don't (`focus`, `blur`, `mouseenter`, `mouseleave`, `load`, `unload`, `scroll` on element).

`addEventListener(type, listener, true)` or `{ capture: true }` registers for capture phase; default is bubble. Useful when you want to intercept before children handle it.

### `addEventListener` options

```js
element.addEventListener(type, listener, {
  capture: false,  // capture phase
  once: false,     // remove after first fire
  passive: false,  // can't preventDefault (scroll perf)
  signal: abortController.signal  // remove on abort
});
```

**Passive listeners**: scroll perf. `touchstart`, `touchmove`, `wheel` listeners default to passive in modern browsers if not explicitly set. A non-passive listener that doesn't call `preventDefault()` is a performance bug; the browser must wait for the listener to finish before scrolling.

**`signal`**: AbortController unifies cleanup. `controller.abort()` removes all listeners registered with that signal at once.

### Propagation control

- **`stopPropagation()`**: stops at the current phase boundary; sibling listeners on this element still fire.
- **`stopImmediatePropagation()`**: also stops sibling listeners on the same element.
- **`preventDefault()`**: prevents the default action (form submit, link navigation, etc.). Different from stopping propagation.

`event.defaultPrevented` is true after `preventDefault()`. Note: passive listeners can't call `preventDefault()`.

### Event delegation

One listener on a common ancestor; check `event.target` to identify the actual element. Eliminates N listeners on N children. The canonical pattern for dynamic lists.

```js
list.addEventListener('click', e => {
  const item = e.target.closest('.list-item');
  if (item) handleClick(item);
});
```

### `target` vs `currentTarget`

`event.target`: the element the event originated on (deepest in the tree).
`event.currentTarget`: the element the listener is registered on.

Inside a delegated handler, `target` is the clicked child; `currentTarget` is the parent with the listener.

### Custom events

```js
element.dispatchEvent(new CustomEvent('my-event', {
  detail: { data },
  bubbles: true,
  composed: true,  // crosses shadow boundary
  cancelable: true  // allows preventDefault()
}));
```

`composed: true` is critical for custom elements: events that should escape the Shadow DOM need it.

### `composedPath()`

Returns the full event path including elements inside shadow trees. `event.target` retargets at shadow boundaries (the listener sees the host, not the inner element); `composedPath()` reveals the real path. Use when you need to know what's actually inside a shadow tree.

### Pointer events

`pointerdown`, `pointermove`, `pointerup`, `pointercancel`. Unified mouse / touch / pen. `pointerType` distinguishes. **`element.setPointerCapture(event.pointerId)`** locks subsequent pointer events to the element during a drag, even if the pointer leaves the element. This eliminates the "drag-tracking on document" pattern.

### React's synthetic events critique

React's `SyntheticEvent` wraps native events; the abstraction loses some browser semantics (event pooling was a known footgun in React 16; mostly resolved in React 17+). The modern position (acknowledged by React team): use real events when the synthetic abstraction adds nothing. `event.nativeEvent` accesses the underlying native event when needed.

---

## Built-in observers: stop polling

### IntersectionObserver

Async notification when an element enters / leaves the viewport (or a specified root). Replaces scroll-listener-based visibility checks.

```js
const observer = new IntersectionObserver((entries) => {
  entries.forEach(entry => {
    if (entry.isIntersecting) load(entry.target);
  });
}, { rootMargin: '50px', threshold: [0, 0.5, 1] });
observer.observe(element);
```

**Flag**: scroll listeners doing visibility math; `getBoundingClientRect()` in scroll handlers; custom "load when near viewport" code.

### ResizeObserver

Async notification when an element's size changes. Replaces `window.resize` polling for element-size-dependent behavior.

**Flag**: `window.addEventListener('resize', ...)` followed by `getBoundingClientRect()` to measure a specific element; element-size polling.

### MutationObserver

Async notification when DOM mutations occur in a subtree. Mostly library / framework use.

### PerformanceObserver

Buffered access to performance entries (LCP, CLS, INP, FCP, navigation, resource timing). Replaces `performance.getEntries()` polling.

### ReportingObserver

Errors, deprecations, browser intervention reports. Useful for production error reporting.

---

## Scheduling: stop using `setTimeout` for everything

- **`requestAnimationFrame`**: frame-aligned work (animation, layout-affecting code).
- **`requestIdleCallback`**: low-priority work in idle time.
- **`queueMicrotask`**: microtask (runs before next paint; before `setTimeout`).
- **`scheduler.postTask(fn, { priority })`** (Chrome): explicit priorities (`'user-blocking'`, `'user-visible'`, `'background'`).
- **`scheduler.yield()`** (Chrome): yield to the event loop.
- **`setTimeout(fn, 0)`**: macrotask; runs after microtasks and rendering.

**Flag**: `setTimeout(fn, 0)` used as "next tick" without considering microtask vs macrotask semantics; long synchronous main-thread work that could be chunked with `scheduler.yield`; custom priority scheduling that `scheduler.postTask` provides.

### AbortController as universal cancellation

`AbortController` + `AbortSignal` cancel async operations and unify cleanup.

```js
const controller = new AbortController();

element.addEventListener('click', handler, { signal: controller.signal });
fetch(url, { signal: controller.signal });

controller.abort();  // removes listener, aborts fetch
```

`AbortSignal.any([signal1, signal2])` (recent) composes signals.

**Flag**: arrays of cleanup functions in `useEffect`-style code where AbortController would unify; cancel-checking flags as a manual pattern when `signal.aborted` is the standard.

---

## Communication primitives

- **MessageChannel** / **MessagePort**: cross-context comms; supports transfer of ports.
- **BroadcastChannel**: same-origin pub/sub across tabs / workers. Simpler than MessageChannel for fan-out.
- **postMessage**: cross-window / iframe / worker; structured clone (most types) plus transferables (zero-copy for ArrayBuffer, MessagePort, ImageBitmap, OffscreenCanvas, ReadableStream).
- **Web Locks API** (`navigator.locks.request`): named locks across same-origin contexts; replaces ad-hoc localStorage-mutex patterns.
- **Comlink** (Surma's library): Promise-based RPC over postMessage.

**Flag**: custom EventEmitter class with `on` / `off` / `emit` when `extends EventTarget` would do; `localStorage` + `storage` event for cross-tab when BroadcastChannel is the right primitive; bespoke RPC over postMessage when Comlink would simplify.

---

## Storage hierarchy

- **Cookies**: small; sent with every request; mostly auth.
- **localStorage / sessionStorage**: synchronous; 5-10MB; strings only.
- **IndexedDB**: async; large; structured data; the right choice for non-trivial client state.
- **Cache API** (Service Worker context): for offline / response caching.
- **Origin Private File System (OPFS)**: browser-private file system; persistent; large; modern alternative to IndexedDB for some uses.
- **File System Access API**: user-granted real filesystem access (limited to user-selected directories / files).
- **Storage Buckets API** (recent): scoped storage with per-bucket quotas and eviction policies.

**Flag**: localStorage abuse for large structured data (use IndexedDB); custom serialization for IndexedDB when the structured clone algorithm already handles it; synchronous storage on the hot path.

---

## Modern UI features that replace JS

### `<dialog>` element

Native modal with focus management, ESC-to-close, top-layer rendering, focus trap, accessibility (`aria-modal`).

```html
<dialog id="d">
  <form method="dialog">
    <button>Close</button>
  </form>
</dialog>
<script>
  document.getElementById('d').showModal();
</script>
```

`showModal()` opens modal (focus trapped, ESC closes, backdrop). `show()` opens non-modal. `close()` closes; `cancel` event fires on ESC.

**Flag**: custom Modal with focus-trap implementation, backdrop, z-index management when `<dialog>` does it.

### Popover API

Non-modal popovers (tooltips, menus, dropdowns) with click-outside dismissal, top-layer rendering. Declarative with `popover` attribute.

```html
<button popovertarget="my-popover">Open</button>
<div id="my-popover" popover>...</div>
```

Combined with **CSS Anchor Positioning** (`anchor()`, `position-anchor`) for placement.

**Flag**: custom Tooltip / Menu with positioning math, scroll/resize repositioning, click-outside, ESC handling when Popover API does it.

### View Transitions API

`document.startViewTransition(updateDom)` animates between two DOM states.

**Flag**: route-transition animation libraries that could use View Transitions.

### CSS that obsoletes JS

- **`position: sticky`** -- sticky headers without JS.
- **`@container`** -- container queries for element-scoped responsiveness.
- **`:has()`** -- parent/sibling selection.
- **`@layer`** -- explicit cascade precedence (replaces `!important` ladders).
- **`@scope`** (recent) -- scoped styles without Shadow DOM.
- **`field-sizing: content`** (recent) -- textarea auto-grow.
- **`scroll-snap`** -- snap-points for carousels.
- **`aspect-ratio`** -- box aspect without padding-hack.

**Flag**: JS toggling classes on scroll for sticky headers (`position: sticky`); ResizeObserver applying classes by size when `@container` does it; measure-and-set for textarea height when `field-sizing: content` does it; specificity-hack `!important` chains where `@layer` would express the precedence cleanly.

---

## Forms and input

### Modern `<input>` types

`date`, `time`, `datetime-local`, `month`, `week`, `color`, `range`, `email`, `tel`, `url`, `number`, `search`. Each comes with native UI (date picker, color picker, number stepper) and validation.

**Flag**: custom date-picker components when `<input type="date">` is appropriate; custom number-stepper when `<input type="number">` works.

### HTML5 form validation

`required`, `pattern`, `minlength`, `maxlength`, `min`, `max`, `step`. Combined with `:invalid` / `:valid` CSS pseudo-classes and `setCustomValidity()` for custom validation.

**Flag**: custom validation library that doesn't compose with `:invalid` / `:valid`; validation logic running on every keystroke when HTML5 validation + `:user-invalid` would handle the UX.

### Contenteditable depth

`contenteditable="plaintext-only"` (modern, restricted). The `BeforeInputEvent` (`inputType`: `insertText`, `deleteContentBackward`, etc.) is the modern hook; `execCommand` is deprecated.

**Selection API** + **Range API** for cursor / selection manipulation.

**Composition events** (`compositionstart`, `compositionupdate`, `compositionend`) for IME (East Asian input). The `event.isComposing` flag on `input` / `keydown` lets you ignore IME-in-progress events.

**Flag**: handling `input` events in text fields without checking `event.isComposing` (breaks Japanese / Chinese / Korean input); using deprecated `execCommand`; reinventing selection / range manipulation when the spec APIs exist.

---

## Internationalization

The `Intl` namespace:
- `Intl.DateTimeFormat` -- date / time formatting.
- `Intl.RelativeTimeFormat` -- "3 days ago", "in 2 hours".
- `Intl.NumberFormat` -- numbers, currency, units, percentages.
- `Intl.PluralRules` -- plural categories per locale.
- `Intl.ListFormat` -- "A, B, and C" per locale.
- `Intl.Segmenter` -- grapheme / word / sentence boundaries.
- `Intl.Collator` -- locale-aware sort comparison.
- `Intl.Locale` -- locale parsing / negotiation.
- `Intl.DurationFormat` (recent) -- duration formatting.

**Flag**: custom date/time/number/currency/plural formatters when `Intl.*` exists; `string.length` for "visible character count" when `Intl.Segmenter` with `granularity: 'grapheme'` is correct; `string.localeCompare` for sort when `Intl.Collator` is more controllable. (Cross-reference `i18n` rules for the broader internationalization concerns.)

---

## URL and encoding

- **`new URL(input)`**: parses URLs; reads `protocol`, `hostname`, `pathname`, `search`, `searchParams`, `hash`.
- **`URLSearchParams`**: query string parsing / construction; iterable.
- **`TextEncoder` / `TextDecoder`**: UTF-8 (and legacy decoding).
- **`btoa` / `atob`**: base64 (operates on binary strings; for arbitrary bytes need `Uint8Array` round-trip).
- **`Uint8Array.fromBase64()` / `toBase64()`** (Stage 3 TC39): modern binary-base64.

**Flag**: regex URL parsing; manual query-string splitting; `encodeURIComponent` calls scattered in string templates instead of `URLSearchParams`; `btoa(String.fromCharCode(...bytes))` instead of `Uint8Array.toBase64()`.

---

## Workers and off-main-thread

- **Dedicated Worker**: one-to-one; postMessage for comms.
- **Shared Worker**: one-to-many; same-origin shared across tabs.
- **Service Worker**: network proxy; install/activate/fetch lifecycle; Cache API; the offline-first foundation.
- **AudioWorklet** (covered in `audio-programming`).
- **`OffscreenCanvas`**: canvas usable inside a Worker for off-main-thread rendering.

**Flag**: heavy synchronous work (large JSON parse, image decode, encoding) on the main thread; INP-breaking long tasks that a Worker would offload; service worker missing for offline-capable apps.

---

## Modern platform APIs (selected)

- **Page Visibility API** (`document.visibilityState`, `visibilitychange`): pause animations / polling when hidden.
- **Page Lifecycle API** (`freeze`, `resume`, `pageshow`, `pagehide`): save state for tab freeze.
- **Wake Lock API**: keep screen on (video, navigation).
- **Idle Detection API**: user idle/active (requires permission).
- **Permissions API**: query permission state without prompting.
- **Web Share API**: native share sheet.
- **WebAuthn**: passkeys, biometrics, phishing-resistant auth.
- **Compute Pressure API**: thermal / load signals.
- **Speculation Rules API** (`<script type="speculationrules">`): prerender / prefetch hints.

### Loading hints

- `<img loading="lazy">` / `<iframe loading="lazy">` -- defer load until near viewport. Browser does this better than userland IntersectionObserver-based lazy loading.
- `<link rel="preload" as="...">` -- early fetch for critical resources.
- `<link rel="modulepreload">` -- preload ES module + transitively.
- `<link rel="preconnect">` -- warm DNS+TCP+TLS.
- `fetchpriority="high|low"` on `<img>` / `<link>` / `fetch()`.

**Flag**: userland lazy-loading library for images when `loading="lazy"` works; manual prefetch in JS when `<link rel="prefetch">` or Speculation Rules suffices.

---

## Web Components

Custom Elements v1 + Shadow DOM + `<template>` give framework-free encapsulated components.

```js
class MyButton extends HTMLElement {
  static observedAttributes = ['variant'];
  constructor() {
    super();
    this.attachShadow({ mode: 'open' }).innerHTML = `
      <style>:host { display: inline-block; }</style>
      <button><slot></slot></button>
    `;
  }
  attributeChangedCallback(name, old, neu) { /* react */ }
  connectedCallback() { /* on insert */ }
  disconnectedCallback() { /* on remove */ }
}
customElements.define('my-button', MyButton);
```

**Shadow DOM**: style + event encapsulation; slots for light-to-shadow projection; `:host`, `:host-context`, `::slotted` for shadow-aware CSS.

**Declarative Shadow DOM** (`<template shadowrootmode="open">`): server-rendered shadow without JS.

**Form-associated custom elements** (`static formAssociated = true` + `attachInternals()`): participate in form submission, validation, `:invalid` -- enabling real custom form inputs.

**Lit** (Justin Fagnani's library): thin reactive layer over Custom Elements. The community position (Lea Verou, Alex Russell, others): production UI without React/Vue is feasible with custom elements + small templating. The framework rebuttal: ecosystem (state, routing, hydration) is where the real cost lives.

---

## Anti-pattern catalog (consolidated)

**Observation / scheduling**:
- Custom IntersectionObserver polyfill or scroll-based visibility detection → `IntersectionObserver`.
- `window.resize` polling for element-size-dependent code → `ResizeObserver`.
- `setInterval` for periodic visibility → `IntersectionObserver` + Page Visibility.
- `setTimeout(fn, 0)` for "next tick" without microtask/macrotask understanding → `queueMicrotask` or `scheduler.postTask`.
- Custom debounce / throttle when `scheduler.postTask` + AbortController would express priority.

**Cleanup**:
- Arrays of `removeEventListener` calls → AbortController + `signal`.
- Long `useEffect` cleanup blocks → single `controller.abort()`.

**Communication**:
- Custom EventEmitter class → `extends EventTarget`.
- `localStorage` + `storage` event for cross-tab → BroadcastChannel.
- Bespoke RPC over postMessage → Comlink or MessageChannel.

**Modal / popover / focus**:
- Custom Modal with focus trap, ESC, backdrop → `<dialog>` with `showModal()`.
- Custom Tooltip / Menu with positioning, click-outside, ESC → Popover API + CSS Anchor Positioning.
- Custom focus trap → `<dialog>`, `popover="auto"`, or `inert` attribute.

**URL / encoding**:
- Regex URL parsing → `new URL(input)`.
- Manual `&` / `=` splitting → `URLSearchParams`.
- `btoa(String.fromCharCode(...bytes))` → `Uint8Array.toBase64()`.

**Internationalization**:
- Custom date / time / relative-time / currency / plural / list / segment → `Intl.*`.
- `string.length` for grapheme count → `Intl.Segmenter`.

**Network / loading**:
- `XMLHttpRequest` in new code → `fetch`.
- Userland lazy-loading library → `loading="lazy"`.
- Manual prefetch → `<link rel="prefetch">` or Speculation Rules.

**Events**:
- `stopPropagation()` instead of `preventDefault()` (or vice versa).
- Confusing `event.target` and `event.currentTarget` in delegated handlers.
- N listeners on N children when one delegated listener works.
- Missing `passive: true` on `wheel` / `touchstart` / `touchmove` that don't `preventDefault()`.
- Ignoring `event.isComposing` in input handlers (breaks IME).
- Custom DnD when Pointer Events + pointer capture is the answer.

**CSS-able problems in JS**:
- Sticky-header classes on scroll → `position: sticky`.
- ResizeObserver for size-by-class → `@container`.
- Textarea height measure-and-set → `field-sizing: content`.
- Parent state from child state → `:has()`.
- `!important` ladders → `@layer`.

**Layout thrash**:
- Read-write-read in a frame (forced reflow) → batch reads, then writes, one per `requestAnimationFrame`.
- `getBoundingClientRect()` in scroll handlers → IntersectionObserver, or RAF.

**Forms**:
- Custom date picker → `<input type="date">`.
- Custom validation that doesn't compose with `:invalid` → HTML5 validation + `setCustomValidity()`.
- Missing `autocomplete` attribute (a11y + UX bug).

**History**:
- Hash-based routing in SPAs when `history.pushState` + `popstate` works.

---

## What is NOT a browser-spec finding

- **Accessibility correctness** (ARIA, screen-reader semantics, color contrast, alt text): route to `accessibility`.
- **Measured performance**: route to `performance`.
- **Framework opinions** (Redux vs Zustand, React vs Vue): out of scope. We flag when framework code reinvents platform primitives.
- **CSS visual design**: out of scope. We touch CSS that replaces JS, not visual styling.
- **WebGL / WebGPU specifics**: route to `graphics-programming`.
- **WASM specifics**: route to `webassembly`.
- **i18n details beyond `Intl` reach** (HarfBuzz shaping, complex script handling): route to `i18n`.

---

## Severity calibration

Per `panel-contract.md`:

- **blocker**: platform misuse causing defect with reachable trigger. Sync main-thread work blocking input above INP threshold. Non-passive `touchstart` listener calling `preventDefault()` causing scroll jank. Modal without focus trap, no ESC, no `aria-modal` (also an a11y blocker).
- **major**: significant reinvention costing maintenance and likely correctness. Custom modal / popover / focus-trap reimplementing `<dialog>` / Popover. Scroll listeners doing visibility math an IntersectionObserver replaces. Manual cleanup arrays where AbortController would unify. IME-breaking input handlers (missing `isComposing`).
- **minor**: noticeable opportunity not blocking. `setTimeout(0)` where `queueMicrotask` semantically matches better. Custom URL parsing in cold path.
- **nit**: cosmetic. `XMLHttpRequest` in a script run once at boot.
- **insight**: structural -- "this component layer would benefit from Custom Elements + Lit"; "form validation should compose with `:invalid` rather than maintaining a parallel state."

Confidence: high when the reinvention is concrete and verifiable in the code; medium when reasoned from one file about a pattern that spans more.

---

## Process for the browser-spec agent

1. **Read the project's framework choices and target browsers.** Evergreen browsers? Older Safari? IE? Modern platform primitives are available in evergreen; older targets need polyfill awareness.
2. **For each region of code**, ask:
   - Is there a platform **observer** that replaces this listener?
   - Is there a platform **primitive** (`<dialog>`, Popover, AbortController, `Intl.*`, `URL`) that replaces this hand-rolled code?
   - Is the event-handling code using **phases, options, propagation, delegation** correctly?
   - Does the code understand **attribute reflection, shadow boundaries, microtask/macrotask order, layout/paint** correctly?
3. **For each finding**, cite the spec or MDN page. "WhatWG DOM § Observers" beats vague "there's an API for this." Concrete migration ("replace with IntersectionObserver, here's the 10-line shape") beats "consider using a better API."
4. **Defer overlapping findings**. If a11y-shaped, append `See also: accessibility`. If measured perf, `See also: performance`.
5. **Stay read-only.**
