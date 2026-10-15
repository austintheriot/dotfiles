---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-10-14
---

# React Components: the Client Model

A reference for the `react-components` subagent. The scope is client-side React: render purity, effects, state shape, identity, refs, external stores, concurrent features, the React 19 Actions hooks, React Compiler, error boundaries, the server-state versus client-state split, context, and React tests.

Out of scope, with the owner:

- Server Components, the `'use client'` / `'use server'` boundaries, Server Functions, framework caching, SSR, streaming, and hydration mismatches: `react-server` (`~/.claude/rules/react-server.md`).
- Re-render cost as a measured performance problem, bundle size, and Interaction to Next Paint (INP): `performance`. This file owns *why* a component re-renders and whether that is a correctness smell. `performance` owns whether the cost matters.
- Who owns a store, a context, or a long-lived object: `data-flow`.
- Prop types, discriminated-union props, and generic components: `typescript-types`.
- Component library API shape, variants, and tokens: `design-systems`.
- Platform built-ins that React code reimplements (observers, focus traps, `AbortController`): `browser-spec`.
- Accessible markup: `accessibility`.

The short always-on editing rule `~/.claude/rules/react.md` (refs instead of document queries) auto-loads on `*.tsx` / `*.jsx`. This file is the deep reference behind it.

## Thesis

**A component is a pure function from props, state, and context to a UI description, and React is free to call it any number of times, in any order, and to throw the result away.** Almost every client React bug is a place where the code assumes React calls it exactly once, in order, and keeps the result. Effects that run "on mount", refs read during render, state that copies a prop, mutation before `setState`, a promise created in render: each one is a bet on a call schedule that React does not promise. StrictMode, concurrent rendering, `<Activity>`, Fast Refresh, and React Compiler each change the call schedule, and each one exposes a different set of those bets.

The operational question: **what does this code assume about when and how often React calls it, and which React feature breaks that assumption?**

### What bites most often

Empirical priority order for triage. The first four produce most real findings.

1. **Effects that should not exist.** Derived state, state reset, parent notification, and event logic written as `useEffect`. The react.dev "You Might Not Need an Effect" catalogue covers nearly all of them, and the 7.x lint rule `set-state-in-effect` now flags the commonest case.
2. **Redundant or contradictory state.** A prop mirrored into state, a copy of an object instead of its ID, two booleans that encode one status, server data copied into `useState`.
3. **Identity mistakes.** Index or random keys, a component defined inside a component, the same component type at the same tree position sharing state across a ternary.
4. **Unstable references into a subscription.** A `useSyncExternalStore` snapshot, a Redux / Zustand selector, a context value, or an effect dependency that is a new object every render.
5. **Fetch races and missing cleanup.** Out-of-order responses and leaked subscriptions.
6. **React 19 Actions misuse.** Signature mix-ups, setters outside a transition, `useFormStatus` in the wrong component.
7. **Compiler-era mistakes.** Code that depends on memoization for correctness, interior-mutability libraries, and blanket removal of manual memoization.
8. **Error boundaries expected to catch what they cannot.**

Pure re-render counting is near the bottom. A re-render is not a bug. Mark Erikson: "rendering is not a bad thing".

---

## Volatile surface

`last-verified` (see frontmatter, do not restate the date here). These rot. The rest of this file is comparatively durable.

| Claim class | Rots | Re-verify at |
|---|---|---|
| Current React version and what each minor added | Fast (minor every 6-11 months, patches monthly) | https://github.com/facebook/react/releases , https://react.dev/blog , `CHANGELOG.md` |
| `eslint-plugin-react-hooks` presets and rule list | Fast | `packages/eslint-plugin-react-hooks/CHANGELOG.md` , https://react.dev/reference/eslint-plugin-react-hooks |
| React Compiler version, guidance, and incompatible-library list | Medium | https://react.dev/learn/react-compiler , https://github.com/reactwg/react-compiler |
| Deprecation status of `forwardRef` and `<Context.Provider>` | Medium | React release notes (grep for "deprecat") |
| Error boundaries being class-only | Slow, but a function API would change advice at once | https://react.dev/reference/react/Component |
| Library major versions and their selector semantics (Redux, Zustand, Jotai, TanStack Query) | Medium | each project's migration guide and npm |
| styled-components maintenance status | Fast and contested | https://opencollective.com/styled-components |
| Docs that lag a release (the transitions-batching note) | Fast | compare the reference page with the changelog |

---

## 1. Version map

**VOLATILE** (2026-10-14). Current stable is React 19.3.1. There is no React 20 release and no announced React 20 plan. Governance moved to the React Foundation under the Linux Foundation (launched 2026-02-24).

| Version | Date | Client changes a reviewer must know |
|---|---|---|
| 18.0 | 2022-03 | Automatic batching everywhere (timeouts, promises, native handlers), `createRoot`, `useTransition`, `useDeferredValue`, `useSyncExternalStore`, `useInsertionEffect` |
| 18.3 | 2024-04 | Warnings for every API that 19 removes |
| 19.0 | 2024-12-05 | Actions, `useActionState`, `useOptimistic`, `useFormStatus`, form `action` prop, `use`, `ref` as a prop, ref-callback cleanup, `<Context>` as provider, metadata hoisting, `preload` / `preinit` / `preconnect` / `prefetchDNS`, root error options |
| 19.1 | 2025-03-28 | Owner Stacks (`captureOwnerStack`, dev only), `useId` format change, `React.act` absent from production builds |
| 19.2 | 2025-10-01 | `<Activity>` stable, `useEffectEvent` stable, Performance Tracks, `useId` uses `_` |
| Compiler 1.0 | 2025-10-07 | React Compiler stable |
| 19.3 | 2026-09-09 | `<ViewTransition>` and `addTransitionType` stable, Fragment refs stable, `react-dom` `browser()`, transitions render independently, StrictMode double-invokes effects during hydration |

### React 19 removals (still found in upgraded code)

- `propTypes` are silently ignored. `defaultProps` on function components is removed (class components keep it). Use default parameters.
- String refs, legacy context (`contextTypes`), `createFactory`, module-pattern factories.
- `ReactDOM.render`, `hydrate`, `unmountComponentAtNode`, `findDOMNode`.
- `act` from `react-dom/test-utils`. Import `act` from `react`.
- UMD builds. `react-test-renderer` is deprecated. `element.ref` is deprecated (use `element.props.ref`).

### React 19 behavior changes

- Render errors are no longer re-thrown. Uncaught errors go to `window.reportError`, caught errors to `console.error`. Configure with `createRoot(el, { onUncaughtError, onCaughtError, onRecoverableError })`. An error tracker that hooked the old re-throw now misses errors.
- In StrictMode the second render reuses the first render's `useMemo` / `useCallback` results.
- TypeScript (`@types/react` 19): an arrow ref callback with an implicit return (`ref={el => (this.el = el)}`) is a type error. `useRef()` requires an argument. The global `JSX` namespace is now `React.JSX`.

### React 19.3 changes that alter existing code

- **Transitions render independently** (facebook/react#37290). A slow transition no longer holds up an unrelated one. Code that relied on two transitions committing together can now commit them separately. The `useTransition` and `useActionState` reference pages still say "multiple ongoing Transitions are batched" as of 2026-10-14. The changelog is the source of truth here. **VOLATILE.**
- **StrictMode double-invokes effects during hydration** (#35961) and after Fast Refresh. An effect that "worked" in a server-rendered root can now show double-mount bugs in development.
- **`use(browser())`** from `react-dom` suspends on the server and resolves on the client. It replaces the `useEffect(() => setMounted(true), [])` pattern and `typeof window` checks.
- Bug fixes that matter in 19.2 codebases: `useEffectEvent` read stale values inside `memo` / `forwardRef` components (#34831), `useSyncExternalStore` missed mutations inside a hidden `<Activity>` (#36947), `useDeferredValue` could get stuck (#36134). In a project pinned to 19.2.x, these are live bugs.

### Not yet deprecated at runtime

`forwardRef` and `<Context.Provider>` still work in 19.3 with no runtime warning. The 19.0 blog says only "in future versions we will deprecate". New code uses `ref` as a prop and `<Context value>`. Do not flag existing `forwardRef` as a defect. Flag it only as a nit in new code.

---

## 2. The render model

### Rendering is unconditional by default

When a component renders, React renders all of its children, with no props comparison. Bailouts exist only where something opts in:

1. `React.memo` (shallow props comparison).
2. `shouldComponentUpdate` / `PureComponent`.
3. **The same element reference.** If the element object is identical to last render, React skips it. This is why the children-as-props pattern works: a state owner that renders `{children}` re-renders, but the `children` elements were created by the grandparent and keep their identity.
4. React Compiler output, which caches JSX elements so that unchanged subtrees keep their identity.

A new context value re-renders every consumer of that context, and `memo` between the provider and the consumer does not block it.

Source: Mark Erikson, "A (Mostly) Complete Guide to React Rendering Behavior", https://blog.isquaredsoftware.com/2020/05/blogged-answers-a-mostly-complete-guide-to-react-rendering-behavior/ (updated for React 18).

### State updates

- `setState` with an `Object.is`-equal value bails out. Mutating an array and passing the same reference (`items.push(x); setItems(items)`) therefore produces no re-render. The UI stays stale until something else renders. The `immutability` lint rule flags it.
- An equal-value `setState` can still render the component once before the bailout. Children do not re-render and effects do not fire. Do not claim that setting the same value "never renders". (Inferred from React internals. Not confirmed against current source.)
- React 18+ batches every update in one tick, not only updates inside React event handlers. `flushSync` opts out and is almost always wrong outside measurement code.
- Updater functions (`setCount(c => c + 1)`) are required when the next value depends on the previous one and the update runs from an async callback, an interval, or more than once per event.

### StrictMode is a call-schedule test

In development, StrictMode:

- Calls component bodies, `useState` / `useMemo` / `useReducer` initializers, and updater functions twice.
- Runs one extra effect setup and cleanup cycle on mount.
- Runs one extra ref-callback setup and cleanup cycle (React 19).
- Runs effects twice during hydration and after Fast Refresh (19.3).

Symptom map, so a reviewer can work back from a bug report:

| Symptom in development | Cause |
|---|---|
| Request fires twice | Effect fetch with no abort or ignore cleanup |
| Duplicated list items | Updater function mutates `prev` |
| Listener or observer attached twice | Ref callback or effect with no cleanup |
| "Connected twice" logs | Subscription with no unsubscribe |
| App init runs twice | Top-level `useEffect` used for one-time init |

"Turn off StrictMode" is never the fix. The double call simulates what `<Activity>`, Fast Refresh, and future offscreen features do in production.

---

## 3. Effects

### The test

react.dev: "Effects are an escape hatch from the React paradigm." **Use an Effect only for code that must run because the component was displayed**, which in practice means synchronizing with a system outside React: a subscription, a non-React widget, a network connection, the document title.

### The "You Might Not Need an Effect" catalogue

Source: https://react.dev/learn/you-might-not-need-an-effect.

| Effect written to... | Use instead |
|---|---|
| Compute a value from props or state | Compute it during render |
| Cache an expensive computation | `useMemo`, after measuring (`console.time`, CPU throttling, production build). `useMemo` does not help the first render. With the Compiler, nothing |
| Reset all state when a prop changes | `key={id}` on the component |
| Adjust part of the state when a prop changes | Store the ID and derive the rest. Second choice: set state during render behind a `prev !== current` guard, in the same component only |
| Share logic between two event handlers | A plain function the handlers call |
| Send a POST when the user submits | Do it in the submit handler |
| Chain state updates (effect A sets x, effect B on x sets y) | Compute all next state in the one event handler |
| Run app init once | A module-level `didInit` flag, or module scope |
| Notify the parent of a state change | Call `onChange` in the same handler (one batched render), or lift state up |
| Pass data a child fetched up to the parent | Let the parent fetch |
| Subscribe to an external store | `useSyncExternalStore` |
| Fetch data | A framework loader or a query library. Minimum: an `ignore` flag or `AbortController` in cleanup |

### Why each one is a defect, not a style issue

- **Derived state in an Effect** renders once with the stale value, then again with the right one. The user can see the stale frame, and any code that reads both values in the first render sees them disagree.
- **Reset via Effect** shows the previous entity's state for one render. In a chat app that means one frame of the wrong user's draft.
- **Event logic in an Effect** fires again on remount, under StrictMode, and when an `<Activity>` becomes visible. A purchase POST in an effect that watches `submitted` can run twice.
- **Notifying the parent from an Effect** leaves parent and child out of sync for a render, and creates a two-pass render on every change.

### `useEffectEvent` (stable since 19.2.0)

Splits a non-reactive read out of an effect, so that the effect does not re-run when that value changes.

```tsx
const onConnected = useEffectEvent(() => {
  showNotification("Connected", theme);
});
useEffect(() => {
  const connection = createConnection(roomId);
  connection.on("connected", () => onConnected());
  connection.connect();
  return () => connection.disconnect();
}, [roomId]);
```

Rules (https://react.dev/reference/react/useEffectEvent):

- Call it only from inside an Effect or another Effect Event. A call during render throws "A function wrapped in useEffectEvent can't be called during rendering".
- Never pass it to a child or to another hook. Lint 7.0.1 flags it as an inline JSX prop.
- Its identity changes every render on purpose. Never list it in a dependency array.
- **It is not a way to skip dependencies.** The docs' pitfall: a `logVisit` event that reads `url`, called from an effect with `[]` deps, misses every navigation. If the effect must re-run when the value changes, the value is reactive and belongs in the deps.
- In React 19.2.x, it reads stale values inside `memo` and `forwardRef` components (#34831, fixed in 19.3.0).

Before 19.2 the workaround was a "latest ref" (`const latest = useRef(fn); latest.current = fn;`). That pattern writes a ref during render, which the `refs` rule flags and the Compiler considers unsafe. In 19.2+ codebases, replace it.

### Dependency arrays

- **Never suppress `react-hooks/exhaustive-deps`.** A suppression hides a stale closure, and it also marks the component as one the Compiler cannot safely optimize.
- **An infinite effect loop has two causes**: a dependency whose identity changes every render (an object or function created in render), or an effect that sets state that it also depends on. An extra *stable* dependency is harmless. The old advice "spurious deps cause infinite re-runs" names the wrong cause.
- The fix for "I do not want X to re-trigger this" is `useEffectEvent`, not omitting X, and not a ref hack.
- Prefer primitive dependencies (`user.id`) over object dependencies (`user`).

### Fetching in an effect

When code does fetch in an effect, the minimum is a race guard:

```tsx
useEffect(() => {
  const controller = new AbortController();
  fetchResults(query, { signal: controller.signal }).then(setResults, ignoreAbort);
  return () => controller.abort();
}, [query]);
```

Without it, a slow response for an old query arrives after a fast response for the new query and overwrites it. The symptom is search results that do not match the search box, and only on slow networks.

Effect-based fetching also has no cache, no deduplication, and produces waterfalls (parent fetches, renders child, child fetches). These are the arguments for a query library or a framework loader (see section 12, school B).

### `useLayoutEffect` and `useInsertionEffect`

- `useLayoutEffect` runs after DOM mutation, before paint. Use it to measure layout and synchronously correct position (tooltips, popovers). It blocks paint, so heavy work in it delays every frame it runs in. Reading a measurement from a ref and setting state in a layout effect is the one case the `set-state-in-effect` rule allows.
- `useInsertionEffect` is for CSS-in-JS library authors to inject styles before layout effects read them. Application code has no reason to use it.

---

## 4. State design

### The five principles

Source: https://react.dev/learn/choosing-the-state-structure.

1. **Group related state** that always changes together (`{x, y}`, not `x` and `y`).
2. **Avoid contradictions.** `isSending` and `isSent` can both be true. Use `status: "typing" | "sending" | "sent"`. This is "make illegal states unrepresentable" applied to `useState`. See also `fp-types`.
3. **Avoid redundant state.** If it can be computed from props or other state, compute it.
4. **Avoid duplication.** Store `selectedId`, not a copy of the selected object. A copy goes stale when the list item is edited.
5. **Avoid deep nesting.** Normalize to a flat map by ID.

### Mirroring props

`const [color, setColor] = useState(props.color)` reads the prop once. Later prop changes are ignored. The symptom: the parent changes the value and the child keeps showing the old one. If the intent really is "initial value", name the prop `initialColor` or `defaultColor`. If the child must reset when the parent's value changes, use `key`.

### State lives at a tree position

React keeps state by component type and position in the tree.

- `isFancy ? <Counter isFancy /> : <Counter />` keeps one `Counter` instance, and its state carries across the branch change. That is a bug when the two branches mean different entities. Give each branch a different `key`.
- A different `key` or a different position resets state, including all child state and DOM.
- Changing a wrapper element type (`<div>` to `<section>`) remounts the whole subtree under it.

### Lazy initializer

`useState(parseLargeJson(raw))` runs the parse on every render and discards the result. Use `useState(() => parseLargeJson(raw))`. The initializer runs twice under StrictMode, so it must be pure.

### Reducers

`useReducer` is the right tool when several state fields change together under named events, or when the next state depends on the previous state in non-trivial ways. A reducer is also the easiest place to test state logic without rendering, which suits the functional-core rule in `~/.claude/CLAUDE.md`. A reducer must be pure: StrictMode calls it twice, and a reducer that pushes to an array duplicates items.

### Controlled versus uncontrolled

A component is controlled when its parent owns the value through props, and uncontrolled when it owns local state with a `default*` prop for the initial value. **A component that is both** (reads `value` but also keeps internal state that diverges) is the defect. Pick one stance per prop and keep it across the component library. See section 12, school E, for the forms debate.

---

## 5. Identity and keys

| Pattern | Trigger | Consequence |
|---|---|---|
| Index key on a list that can reorder, filter, or insert | `items.map((item, i) => <Row key={i} />)` | Local state, uncontrolled input values, focus, and animations stay with the position, so they move to the wrong row. A typed draft appears in the wrong row after a delete |
| Random key in render | `key={Math.random()}` or `crypto.randomUUID()` in render | Every row remounts every render: lost state, lost focus, effect churn. `purity` flags `Math.random` in render |
| Component defined inside a component | `function Row() {}` declared inside `List` | A new component type every render. Full remount, so an input loses focus on every keystroke. The `static-components` rule flags it |
| Key generated on fetch, not on create | `data.map(item => ({...item, key: uuid()}))` per fetch | Every refetch remounts the list |
| Same type, same position, two meanings | Ternary between two `<Form>` of different entities | State leaks across entities |

An index key is fine for a static list that never reorders and whose rows have no state. Say so rather than flagging every index key.

`key` is also a tool: `key={userId}` on a subtree is the correct way to reset all of its state when the entity changes.

---

## 6. Refs

- **Reading or writing `ref.current` during render is a purity violation.** A render that reads a ref shows a value that does not trigger a re-render when it changes, so the UI goes stale. The `refs` lint rule flags it. The one accepted pattern is lazy initialization: `if (ref.current === null) ref.current = createThing();`.
- **Rendered values belong in state. Values for handlers and effects belong in refs.**
- **Ref callback cleanup (React 19).** A ref callback can return a cleanup function. If it returns nothing, React calls it with `null` on detach, and react.dev says that null call "will be removed in a future version". New ref callbacks return a cleanup.
- **An inline ref callback has a new identity every render.** React runs the old cleanup (or the null call) and then the new callback on every render. A ref callback that creates a `ResizeObserver` therefore creates and destroys one per render, and leaks one per render if it has no cleanup. Stabilize the callback, or rely on the Compiler to do it.
- **`ref` is a prop in React 19.** Function components receive `ref` directly. `forwardRef` still works.
- **Fragment refs (19.3)** give a `FragmentInstance` for a group of children without a wrapper `div`: `addEventListener`, `focus`, `focusLast`, `blur`, `observeUsing` (Intersection / Resize observers), `getClientRects`, `scrollIntoView`. A wrapper `div` added only to hold a ref is now avoidable.
- **`useImperativeHandle`** exposes a narrow imperative API (`focus()`, `scrollToRow(n)`) instead of the DOM node. Exposing the raw node to a parent couples the parent to the child's markup.

---

## 7. External stores and concurrency

### Tearing

In concurrent rendering, React can pause a render partway. If an external store changes during that pause, components rendered before the change and after it show different versions of the store. That is tearing. `useSyncExternalStore` prevents it: when the snapshot changes during a transition, React re-checks before commit and restarts the render as a blocking one.

A store read through `useState` + `useEffect` subscription, or by reading a mutable global in render, can tear under transitions.

### `useSyncExternalStore` rules

Source: https://react.dev/reference/react/useSyncExternalStore.

- **`getSnapshot` must return a cached, immutable value.** Returning `{ todos: store.todos }` produces the error "The result of getSnapshot should be cached" and an infinite render loop.
- A `subscribe` function defined inline in the component resubscribes every render. Hoist it to module scope or stabilize it.
- Omitting `getServerSnapshot` makes server rendering throw.
- Do not suspend on a value from `useSyncExternalStore`. Store mutations cannot be transitions, so the nearest Suspense fallback replaces content that is already on screen.
- Prefer `useState` / `useReducer` for state React owns. `useSyncExternalStore` is for non-React stores and browser APIs (`navigator.onLine`, `matchMedia`).

### `use()`

Source: https://react.dev/reference/react/use.

- **The promise must be cached across renders.** `use(fetch(url))` in render creates a new promise every render. React logs "A component was suspended by an uncached promise" and shows the fallback on every render. Create the promise outside render, in a parent, in a cache, or in a library.
- `use()` cannot be inside `try` / `catch`. React throws "Suspense Exception: This is not a real error!". Lint 6.1 enforces this. Use an Error Boundary for rejections.
- `use(Context)` can be called conditionally and in loops, unlike `useContext`.
- 19.3 adds a development warning when a component appears to unblock by calling `use()` conditionally (#37104).

### Transitions

Source: https://react.dev/reference/react/useTransition.

- **State updates after an `await` inside `startTransition` are not transitions.** Wrap each one in a nested `startTransition`. Without it, the update is urgent, and it can replace visible content with a Suspense fallback. This is a current limitation until AsyncContext exists.
- `setTimeout` inside `startTransition` is not marked as a transition.
- A transition cannot drive a controlled text input. The input lags. Keep the input state urgent and defer the expensive consumer.
- Errors thrown inside a `useTransition` `startTransition` reach the nearest Error Boundary. Errors in the standalone `startTransition` function do not.
- In 19.3+, independent transitions commit independently (see section 1).

### `useDeferredValue`

- Pass a primitive or an object created outside render. A new object every render causes a background re-render every time.
- It is not a debounce. It has no fixed delay. A network request keyed on the deferred value still fires per keystroke unless something caches it.
- It helps only if the slow child is memoized (`memo` or the Compiler). Otherwise the child renders with the urgent value anyway.
- `initialValue` (second argument) was added in 19.

### `<Activity>` (stable since 19.2.0)

Source: https://react.dev/reference/react/Activity.

`<Activity mode="hidden">` sets `display: none` on the children, destroys their effects (cleanup runs), keeps their state, and re-renders them at low priority. react.dev: "Think of hidden Activities as being unmounted." On show, effects mount again.

- Replacing `{open && <Panel />}` with `<Activity mode={open ? "visible" : "hidden"}>` changes semantics. Panel keeps state and DOM, and its effects remount on every show. An effect that assumed one mount per lifetime now runs per show.
- `<video>`, `<audio>`, and `<iframe>` keep playing while hidden. Pause them in a `useLayoutEffect` cleanup (layout, so the cleanup is tied to the visual hide).
- Pre-rendering a hidden Activity prefetches only Suspense-enabled data (`use(promise)`, `lazy`, a library in suspense mode). Data fetched in an effect is not prefetched, because effects do not run while hidden.
- In 19.2.x, `useSyncExternalStore` missed store mutations while hidden (#36947, fixed in 19.3.0).

### `<ViewTransition>` (stable since 19.3.0)

- Animates only updates inside a transition: `startTransition`, a Suspense reveal, or `useDeferredValue`. An urgent `setState` does not animate. "The animation does not run" is almost always this.
- Enter and exit animations fire only when the `<ViewTransition>` is the first thing rendered in its subtree, before any DOM node.
- A shared `name` must be unique across the whole app at any one time.
- For Suspense, react.dev recommends `<ViewTransition update="auto" default="none">` so the fallback appears without animation and only the fallback-to-content swap animates.

---

## 8. React 19 Actions hooks

### `useActionState`

Source: https://react.dev/reference/react/useActionState.

- **Signature is `(previousState, payload)`.** Passing an existing `action(formData)` function makes `formData` the previous state, and `formData.get` is undefined. This is the most common migration bug.
- Calls queue and run **sequentially**, each receiving the previous result. A slow action blocks later ones. Rapid "add to cart" clicks serialize. They are not aborted by default.
- A throw cancels every queued action and rethrows to the nearest Error Boundary. Return expected errors (validation, conflict) as state. Throw only for unexpected failures.
- A dispatch outside a transition logs "An async function with useActionState was called outside of a transition" and `isPending` is wrong. Dispatch through `<form action>`, `formAction`, or `startTransition`.
- A dispatch during render throws "Cannot update action state while rendering".
- There is no reset. Use `key`, or rely on `<form action>`, which auto-resets uncontrolled fields after the action.

### `useOptimistic`

Source: https://react.dev/reference/react/useOptimistic.

- The setter must run inside an action or transition. Outside one, React warns "An optimistic state update occurred outside a Transition or Action" and the value flashes and reverts.
- The optimistic value shows only while the action is pending. When the action ends, the hook converges to the real `value`. **If the parent does not update `value` on success, the UI reverts.** `useOptimistic(false)` with a hard-coded base always goes back to `false`.
- On a throw, the transition ends and the UI reverts. Catch the error and show it, or the user sees the change silently undo.
- Use the reducer form when the base can change mid-action. A setter that closes over the base reads a stale base.

### `useFormStatus`

Reports only the **parent** `<form>`. In the component that renders the `<form>`, `pending` is never true. Put it in a child (`<SubmitButton />`).

### Form `action` and auto-reset

After a successful action, React resets uncontrolled form fields (19.3 also fires `onReset`). Controlled fields keep React state. A form that returns validation errors and uses uncontrolled fields can wipe the user's input. Return the submitted values in the action state and render them as `defaultValue`, or reset manually with `requestFormReset`. (`requestFormReset` details: found, not verified against the reference page.)

---

## 9. React Compiler

### Status

**VOLATILE** (2026-10-14). React Compiler 1.0 has been stable since 2025-10-07 (`babel-plugin-react-compiler` 1.0.0 is current). It works with React 17+ through the `target` option and `react-compiler-runtime`. Expo SDK 54+ enables it by default, and `create-vite` and `create-next-app` offer it as a template. Meta reports up to 12% faster loads and navigations on the Quest Store, and some interactions over 2.5x faster.

### What it does

- Memoizes components and hooks granularly, including conditionally, which manual memoization cannot do. It supports optional chains and array indices as dependencies.
- Memoizes inline arrows and objects in JSX, including `() => handleClick(item)`.
- Memoizes components and hooks only, not arbitrary functions, and does not share caches across components.
- On code it detects might break the Rules of React, it "safely skips optimization". The docs do not describe a build-time notice. Treat a skip as silent at build time. The lint rules surface the cause.

### How it fails

react.dev's debugging page says the common failure is at runtime: the compiler compiled something it should have skipped, because of a rule violation it could not detect. The listed causes:

- Effects that rely on referential equality to decide whether to run.
- Dependency arrays that need stable references.
- Conditional logic based on reference checks.

In short: **code whose correctness depends on memoization, or on its absence.** Adding the compiler changes when such code runs, and removing manual memoization changes it again.

Debugging: add `"use no memo"` at the top of the suspect function to isolate it, then remove the directive once fixed. Compiled components show a sparkle badge in React DevTools.

### Guidance from the React team

- New code: rely on the compiler, and use `useMemo` / `useCallback` as an escape hatch for precise control (for example, to keep an effect dependency stable).
- Existing code: leave manual memoization in place, because "removing it can change compilation output", or test carefully before removing it.
- Without good test coverage, pin the compiler to an exact version.

### Incompatible libraries

Libraries with interior mutability break the compiler's assumption that the same input gives the same output. The `incompatible-library` rule flags them, and the compiler skips components that call them.

- react-hook-form `watch()`: `useMemo(() => watch("name"), [watch])` freezes the UI. Use `useWatch`.
- TanStack Table `useReactTable`.
- MobX `observer` is not detected. If a MobX component breaks, add `"use no memo"`.

### `eslint-plugin-react-hooks`

**VOLATILE** (2026-10-14). Current 7.1.1. Since 7.0.0, the `recommended` preset enables every compiler-powered rule. Only two presets exist: `recommended` and `recommended-latest`. The rules do not need the compiler installed. `eslint-plugin-react-compiler` is superseded and must be removed.

Rules a reviewer will see cited: `rules-of-hooks`, `exhaustive-deps`, `purity` (`Date.now`, `Math.random` in render), `refs`, `immutability`, `globals`, `set-state-in-render`, `set-state-in-effect`, `static-components`, `use-memo`, `preserve-manual-memoization`, `incompatible-library`, `error-boundaries` (try/catch around child rendering), `unsupported-syntax`, `config`, `gating`. `component-hook-factories` is a deprecated no-op since 7.1.1.

A codebase on `eslint-plugin-react-hooks` 4.x or 5.x is missing most of these. Recommend the upgrade once, as a single finding.

### What to flag in a compiled codebase

Flag:

- Any violation the rules above report, and any suppression of them.
- Code that depends on memoization for correctness.
- Interior-mutability libraries used without the compatible API.
- A bulk removal of `useMemo` / `useCallback` / `React.memo` with no test coverage.
- `"use no memo"` left in place with no comment or ticket.

Do not flag:

- A missing `useMemo`, `useCallback`, or `React.memo`.
- Inline object, array, or function props.
- Existing manual memoization (it is neither required nor harmful).

**Check whether the compiler is on before flagging any memoization issue.** Look for `babel-plugin-react-compiler` in `package.json` and for `reactCompiler` in the Next.js, Vite, or Babel config. A file under a `compilationMode: "annotation"` setup is compiled only with `"use memo"`.

---

## 10. Error boundaries

- **Still class-only** as of 19.3. react.dev: "There is currently no way to write an Error Boundary as a function component." It recommends `react-error-boundary` (Brian Vaughn). **VOLATILE.**
- **They do not catch** errors in event handlers, in async code (`setTimeout`, promise callbacks), during server rendering, or in the boundary itself. They do catch errors thrown in a `useTransition` `startTransition` and rejections read through `use()`.
- An error in an event handler that the UI must reflect needs `try` / `catch` and state, or `showBoundary` from `react-error-boundary`.
- In development, caught errors also reach `window.onerror`. In production they do not. An error tracker counts double in development only.
- Placement is a product decision. One root boundary turns any widget failure into a blank app. A boundary per independent panel keeps the rest usable. A missing boundary around a third-party widget or a `use()` call is a real finding.
- React 19 root options (`onCaughtError`, `onUncaughtError`, `onRecoverableError`) are where crash reporting hooks in. See also `crash-and-release-health`.

---

## 11. Server state, client state, and context

### The split

Most "global state" in a client app is a cache of server data. TkDodo and react.dev both argue the remaining client state is small. The failure modes differ by kind.

### Server-state cache (TanStack Query, RTK Query, SWR)

Source: TkDodo, "Practical React Query" series, https://tkdodo.eu/blog/practical-react-query.

- **Do not copy query data into `useState`.** It opts out of background refetches, so the copy goes stale. The exception is seeding form defaults, with `staleTime: Infinity`.
- **The query key is a dependency array.** Every variable that `queryFn` reads must be in `queryKey`. `useQuery({ queryKey: ["todos"], queryFn: () => fetchTodos(filter) })` serves the first filter's data for every filter. `@tanstack/eslint-plugin-query` has an `exhaustive-deps` rule.
- Do not use the query cache as a local state manager. Write to it for optimistic updates and mutation responses only.
- Defaults: `staleTime` 0, `gcTime` 5 minutes. Since v5, refetch-on-focus listens to `visibilitychange`. A complaint of "it refetches all the time" is usually `staleTime: 0`.
- Wrap each query in a custom hook, or share `queryOptions`, so the key and function stay paired.

### Client stores

- **react-redux `useSelector`** compares results with `===` (the old `connect` compared shallowly). A selector that returns a new object or array (`state => state.items.filter(...)`, `state => ({ a: state.a })`) re-renders the component on every dispatch. Since 8.1.0, development checks warn: `stabilityCheck` and `identityFunctionCheck` (`state => state`). Fix with a `createSelector` defined outside the component, several selectors, or `shallowEqual`.
- **Zustand v5**: `create` no longer takes an equality function. A selector that returns a new reference "may cause infinite loops" ("Maximum update depth exceeded"). Fix with `useShallow`, or `createWithEqualityFn` from `zustand/traditional`. Code migrated from v4 is where this appears.
- **Jotai 3** is ESM-only. `atomFamily` moved to `jotai-family` and `loadable` was removed. (Found in a secondary source, not verified.) **VOLATILE.**

### Context

- Every consumer re-renders when the value's identity changes, regardless of which field it reads. Context has no selector API, and `use(Context)` does not add one. The context-selectors RFC never shipped.
- `<Ctx value={{ user, setUser }}>` in a parent that re-renders often re-renders every consumer each time. Fixes: memoize the value (the Compiler does it automatically), split state and dispatch into two contexts, split by update frequency, or move high-frequency state to a store with selectors.
- **Context is a dependency-injection mechanism, not a state manager.** Mouse position, scroll offset, or per-keystroke form state in context renders the whole consumer set per change.

---

## 12. Schools of thought

Each position is stated in its advocates' terms. None is adjudicated.

### A. Memoization: on measurement, by default, or by compiler

- **Measure first.** react.dev: "There is no benefit to wrapping a calculation in useMemo in other cases." Fix structure before memoizing: pass children as JSX, keep state local, keep render pure, use fewer effects, then use the Profiler. Kent C. Dodds ("When to useMemo and useCallback") and Nadia Makarevich ("you can probably remove 90% of all useMemo and useCallbacks", https://developerway.com/posts/how-to-use-memo-use-callback) hold this side. The argument: memoization costs readability and memory, and one unstable value anywhere silently defeats a chain of it.
- **Memoize by default.** react.dev itself concedes the practice: "There is no significant harm to doing that either, so some teams choose to not think about individual cases, and memoize as much as possible." The argument: the cost of a missed memo is a slow interaction found late, and the cost of an extra memo is small. React Compiler is the React team automating this side. (I found no named essay that defends manual memoize-everything. The position is common in practice and anonymous in print.)
- **Compiler skeptics and measurers.** Nadia Makarevich measured the beta compiler on a real app (https://developerway.com/posts/how-react-compiler-performs-on-real-code, 2024-12): initial load did not change, one interaction's blocking time went from 280 ms to 0, and of 9 re-render cases 2 were fully fixed, 2 not at all, and the rest partly. Misses came from query data with new references and a library call that made the compiler bail. Her conclusion: for most apps the compiler is enough, and for maximum performance "welcome back to the manual memoization". DebugBear calls compiled output a debugging black box.

When each is right: a Compiler-enabled codebase makes the first two moot for new code. The third camp's point survives the compiler: unstable data from outside React (a query result, a library return) still defeats it.

### B. Data fetching: effects, a library, or a framework

- **Use a framework or a library.** react.dev: effect-based fetching has races, no cache, and waterfalls. TkDodo, "Why You Want React Query": hand-rolled effect fetching gets races, loading and error state, and caching wrong, repeatedly.
- **You might not need the library.** TkDodo again, "You Might Not Need React Query" (https://tkdodo.eu/blog/you-might-not-need-react-query): when framework loaders or Server Components own fetching, a client cache adds less. 19.3's `use(browser())` and `use(cachedPromise)` push the same way, toward Suspense data rather than effects.
- **Pragmatists**: one request in a small app, with an `AbortController`, does not need a dependency. Common in practice. I found no named primary source.

### C. State architecture: central store, server cache plus local state, or signals

- **Central store** (Mark Erikson, Redux maintainer): one predictable update path, time-travel debugging, and RTK Query for server data. Strongest in large teams where many features touch shared client state.
- **Server cache plus colocated state** (TkDodo, react.dev's "keep state local"): most global state is server cache. Put that in a query library and keep the rest local or in a small store.
- **Signals** (Ryan Carniato, Solid; the Preact team): fine-grained reactivity updates only what read the value, without re-running components or maintaining dependency arrays. Jake Lazaroff, "Were React Hooks a Mistake?" (https://jakelazaroff.com/words/were-react-hooks-a-mistake/): hooks re-create closures every render, which causes stale data and ref workarounds. Signal components look like class components. Devon Govett's counterpoint: signals are mutable state, which gives up React's UI-as-a-pure-function. In React, `@preact/signals-react` needs a Babel transform or `useSignals()`. TC39 Signals is at Stage 1. **VOLATILE.**

This file's thesis (purity, call-schedule independence) is the React camp's model. The signals camp rejects the premise that re-running components is acceptable. A reviewer must not treat the signals critique as a misunderstanding.

### D. Hooks as a leaky abstraction

- **Critics**: Lazaroff and Carniato as above. The Remix team moved Remix 3 off React to a Preact fork with explicit update calls instead of hooks (found via InfoQ, not verified against the primary post). The critics' evidence from inside React: `useEffectEvent` exists because dependency arrays conflate reactive and non-reactive reads, and the Compiler exists because manual memoization was too hard to get right.
- **Defenders**: the React team. Hooks compose, are lintable, and keep the pure-function model. `useEffectEvent` and the Compiler remove most of the manual pain without changing the model.

### E. Forms: controlled or uncontrolled

- **Uncontrolled first**: React Hook Form (Bill Luo) registers inputs by ref and avoids a re-render per keystroke. React 19 `<form action>` with `FormData` and auto-reset, and Remix / React Router form conventions (Ryan Florence, Kent C. Dodds), take the same side: the browser already holds form state.
- **Controlled**: the classic react.dev pattern, TanStack Form. Live validation, dependent fields, and formatting are direct, at the cost of a render per keystroke.
- Friction: React 19 auto-reset surprises uncontrolled forms that return validation errors (section 8). React Hook Form's `watch` is a Compiler incompatible-library case.

### F. Styling under concurrent rendering

- **Runtime CSS-in-JS** (styled-components, Emotion): co-located, dynamic, mature. React 18 added `useInsertionEffect` so these libraries could inject styles at the right time.
- **Zero-runtime** (StyleX from Meta, vanilla-extract, Panda, Tailwind): style injection during render forces style recalculation per render and does not work in Server Components. The React team has discouraged runtime injection for performance (reactwg/react-18 discussion #110, not verified this pass).
- **VOLATILE** status: styled-components maintainer Evan Jacobs put the library in maintenance mode on 2025-03-17 ("For new projects, I would not recommend adopting styled-components"). A 2026-03-27 update describes a path forward, and 6.6.0 shipped on 2026-10-12. Do not state its status from memory.

---

## 13. Custom hooks

- **Name `useX` only if it calls hooks.** Lint and the Compiler treat `use*` as a hook. A plain function named `useFormatDate` gets hook rules, so it cannot be called conditionally, for no reason.
- **Custom hooks share logic, not state.** Each call has its own state. Two components that call `useCart()` do not share a cart unless the hook reads a shared store or context. "Why does the header not see the item I added" is this.
- **A hook that returns a fresh object every render** (`return { data, refetch }`) breaks every consumer that puts the result in a dependency array, unless memoized. The Compiler memoizes hook return values in compiled files.
- **A hook that hides an effect** (`useFetchOnMount`) makes "why did this run again" opaque to the caller. Prefer hooks that wrap a store subscription or a query library, where re-run behavior is documented.
- A hook is the right extraction for a React-coupled concern. Pure logic belongs in a plain function the hook calls, so it can be tested without rendering.

---

## 14. Testing React

Short section. `test-coverage` owns coverage. This covers React-specific test defects.

Source: Kent C. Dodds, "Common mistakes with React Testing Library", https://kentcdodds.com/blog/common-mistakes-with-react-testing-library , and https://testing-library.com/docs/queries/about.

- **Query priority**: `*ByRole` (with `name`), `ByLabelText`, `ByPlaceholderText`, `ByText`, `ByDisplayValue`, then `ByAltText`, `ByTitle`, and `ByTestId` last. A `getByTestId` where a role query works is a test that passes while the UI is inaccessible.
- **`query*` only to assert absence.** Use `get*` for presence, and `find*` for appearance after async work. `waitFor(() => getBy...)` is `findBy` written the long way.
- No side effects inside `waitFor`, and no empty `waitFor` callback.
- **An "act" warning usually means an un-awaited async update.** Fix it with `await user.click(...)` or `findBy`, not with more `act` wrappers. Wrapping to silence the warning hides the real race.
- `userEvent.setup()` then `await user.click()`. Every user-event v14 API is async. `fireEvent` skips the event sequence a user produces (pointerdown, focus, mouseup, click).
- `act` comes from `react` in 19. `React.act` is absent from production builds since 19.1, so a test suite that runs against the production bundle fails.
- `react-test-renderer` is deprecated. Shallow rendering is gone.
- Test reducers and pure helpers as plain functions. Render tests are for behavior a user can observe.

See also `~/.claude/rules/testing-typescript.md` and the project's own harness.

---

## 15. Anti-pattern catalog

Each entry: pattern, trigger, consequence, fix.

1. **Derived state in an Effect.** `useEffect(() => setFullName(first + " " + last), [first, last])`. One render with a stale value, then a second render. Compute in render.
2. **Mirroring a prop in state.** `useState(props.color)`. Later prop changes are ignored. Use the prop, rename to `initialColor`, or reset with `key`.
3. **Reset via Effect.** `useEffect(() => setDraft(""), [userId])`. One frame of the previous user's state. `key={userId}`.
4. **Effect chain.** Effect A sets `x`, effect B on `x` sets `y`. Cascading renders, ordering that breaks on refactor. Compute next state in the event handler.
5. **Notifying the parent from an Effect.** `useEffect(() => onChange(value), [value])`. Extra render pass, parent one frame behind. Call `onChange` in the handler.
6. **Event logic in an Effect.** A POST in an effect that watches `submitted`. Fires again on remount, StrictMode, and Activity show. Move to the handler.
7. **Fetch effect without a race guard.** `fetch(q).then(setResults)`. Out-of-order responses show results for an old query. `AbortController` or `ignore` flag, or a query library.
8. **App init in a top-level Effect.** `useEffect(checkAuth, [])` in `App`. Runs twice in development and on 19.3 hydration. Module-level `didInit` flag.
9. **Index key on a dynamic list.** `key={i}` on reorderable rows. State and input values attach to the wrong row. Stable ID.
10. **Random key in render.** `key={crypto.randomUUID()}`. Remount every render. Stable ID created with the item.
11. **Component defined inside a component.** Input loses focus on each keystroke. Hoist to module scope.
12. **Same type, same position, two entities.** State leaks across a ternary. Different `key` per branch.
13. **Ref read in render for display.** `<span>{countRef.current}</span>`. UI does not update. Use state.
14. **Inline ref callback with setup and no cleanup.** `ref={el => observer.observe(el)}`. Re-runs every render and leaks. Return a cleanup and stabilize the callback.
15. **"Latest ref" written during render in 19.2+.** `latest.current = callback` in the body. Purity violation, Compiler-unsafe. `useEffectEvent`.
16. **Uncached `getSnapshot`.** `() => ({ todos: store.todos })`. "The result of getSnapshot should be cached" and an infinite loop. Return a stable value.
17. **Inline `subscribe`.** Resubscribes every render. Hoist.
18. **Uncached promise to `use()`.** `use(fetch(url))`. Fallback on every render. Cache the promise outside render.
19. **`use()` in try/catch.** Error "Suspense Exception: This is not a real error!". Use an Error Boundary.
20. **`useFormStatus` in the form's own component.** `pending` never true. Move it to a child.
21. **`useActionState` signature mix-up.** `formData.get` is undefined. `(previousState, formData)`.
22. **`useActionState` dispatch outside a transition.** `isPending` wrong, development error. Use `action`, `formAction`, or `startTransition`.
23. **`useOptimistic` setter outside a transition.** Flash and revert. Call inside the action.
24. **`useOptimistic` with a base the parent never updates.** UI reverts after success. Update the source of `value`.
25. **`setState` after `await` in `startTransition`.** The update is urgent and can show a fallback. Nested `startTransition`.
26. **Context value literal in a busy parent.** Every consumer re-renders per parent render. Memoize, split, or use a store.
27. **Context as a high-frequency store.** Per-keystroke consumer renders. Local state or a store with selectors.
28. **`useSelector` returning a new reference.** Re-render on every dispatch. `createSelector` outside the component.
29. **Zustand v5 selector returning a new object.** "Maximum update depth exceeded". `useShallow`.
30. **Server data copied into `useState`.** Background refetches are lost. Use query data directly.
31. **Query key missing a `queryFn` input.** Wrong data served across parameters. Put every input in the key.
32. **`useEffectEvent` used to skip a reactive dependency.** The effect misses changes. Keep the dependency.
33. **`useEffectEvent` passed to a child or called in render.** Lint error or a thrown error. Use a normal callback.
34. **`useMemo` around react-hook-form `watch`.** Frozen UI. `useWatch`.
35. **Bulk removal of manual memoization in a compiled codebase without tests.** Effects keyed on those values can over-fire. Leave it, or test first.
36. **Correctness that depends on memoization.** An effect that relies on an object staying identical in order to run once. Behavior changes when the compiler is added or removed. Depend on primitives, or use `useEffectEvent`.
37. **Media inside a hidden `<Activity>`.** Audio keeps playing. Pause in a `useLayoutEffect` cleanup.
38. **Expecting a hidden `<Activity>` to prefetch effect-fetched data.** Nothing loads until shown. Use Suspense-enabled data.
39. **`<ViewTransition>` on an urgent update.** No animation. Wrap the update in `startTransition`.
40. **Error boundary expected to catch handler or async errors.** No fallback, uncaught error. `try` / `catch` with state, or `showBoundary`.
41. **Mutate then set the same reference.** `items.push(x); setItems(items)`. No re-render. Create a new array.
42. **Expensive initializer without lazy form.** `useState(parse(raw))` parses every render. `useState(() => parse(raw))`.
43. **Mutating updater.** `setItems(prev => { prev.push(x); return prev; })`. Duplicates under StrictMode and no re-render. Return a new array.
44. **Non-hook named `useX`.** Hook rules applied for no reason. Rename.
45. **`mounted` state set in an Effect for browser-only UI.** Extra render and a flash. `use(browser())` (19.3) or `useSyncExternalStore` with `getServerSnapshot`.
46. **Suppressed `exhaustive-deps`.** Hidden stale closure, and the Compiler cannot optimize the component. Fix the dependency or restructure.
47. **A component that is both controlled and uncontrolled.** Value diverges from the prop after the first local edit. Pick one stance.
48. **RTL `query*` to assert presence, or `waitFor` + `get*`.** Weaker failures, flaky tests. `get*` / `find*`.

---

## 16. Authorities

| Source | Good for |
|---|---|
| react.dev Learn and Reference, https://react.dev | Canonical model. "You Might Not Need an Effect", "Choosing the State Structure", "Preserving and Resetting State", "Separating Events from Effects", and each hook's Caveats and Troubleshooting sections. Occasionally lags a release |
| react.dev/blog, GitHub releases, `CHANGELOG.md` | Version truth |
| https://react.dev/reference/eslint-plugin-react-hooks and its CHANGELOG | Lint rule truth, one page per rule |
| https://github.com/reactwg/react-compiler | Compiler discussions and edge cases |
| Mark Erikson, blog.isquaredsoftware.com | Rendering behavior, Redux and React-Redux semantics |
| Dominik Dorfmeister (TkDodo), https://tkdodo.eu/blog | Server state, query keys, React Query versus context |
| Kent C. Dodds, kentcdodds.com | Testing Library philosophy and mistakes |
| Nadia Makarevich, developerway.com | Re-render mechanics, memo, measured Compiler results |
| Dan Abramov, overreacted.io | Mental models ("A Complete Guide to useEffect", "Before You memo()") |
| Ryan Carniato, Jake Lazaroff | The signals critique of React's model |
| React team (Ricky Hanlon, Sebastian Markbåge, Jack Pope, Joe Savona, Lauren Tan, Mofei Zhang) | Concurrent features, Compiler, ViewTransition |
| Brian Vaughn | `react-error-boundary` |
| Daishi Kato | Zustand, Jotai, tearing |

---

## 17. Severity rubric

- **Blocker**: user-visible wrong data or lost data. A fetch race that shows another query's results in a flow where the user acts on them. An index key that attaches typed input to the wrong row in an editable list. An Effect that sends a mutation (payment, delete, message) and can fire twice. A `useSyncExternalStore` or selector loop that hangs the page. An error boundary missing where one widget's throw blanks the app.
- **Major**: a defect that shows under a normal condition. A stale-closure bug behind a suppressed `exhaustive-deps`. A mirrored prop that ignores updates. Server data copied into state. A query key missing an input. `useOptimistic` that reverts on success. Code that depends on memoization for correctness in a compiled codebase. React 19.2.x-specific bugs (#34831, #36947) hit by code in scope.
- **Minor**: an extra render or a one-frame flash with no wrong data. Derived state in an Effect that only displays. A context value literal in a parent that renders rarely. An inline ref callback with cheap setup and a cleanup.
- **Nit**: `forwardRef` or `<Context.Provider>` in new code. A non-hook named `useX`. An index key on a static, stateless list (usually not worth raising).
- **Insight**: a structural option that removes a class of bug: replace a reset Effect with `key`, move server data to a query library, replace a latest-ref with `useEffectEvent`, upgrade the lint plugin to 7.x.

Every finding names the call-schedule assumption and the React feature that breaks it. Not "avoid useEffect here", but "`Profile.tsx:31` resets `draft` in an Effect on `userId`, so the first render for a new user shows the previous user's draft. `key={userId}` on `<Profile>` removes the Effect and the stale frame."

---

Source research: `~/.claude/local/research-notes/react-components-research.md`

## Changelog

- 2026-10-14: Created. Researched against React 19.3.1, Compiler 1.0.0, eslint-plugin-react-hooks 7.1.1.
