---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-10-17
---

# Frontend Architecture: The Elm Architecture (TEA) in Any Framework

A reference for how a client application organizes state, effects, IO, and views. The principles come from The Elm Architecture (TEA): a model, messages, a pure update, effects returned as data, and subscriptions. The principles hold in any view layer (React, Lit, Svelte, Vue, web components). One appendix maps them onto Redux Toolkit (RTK), which is the stack these rules were proven on.

Provenance: one production single-page app followed these rules for two months. A research survey of Elm, Redux, MobX, signals, XState, Effector, Cycle.js, and others came first. Every "failure mode" below happened in that app. Every "enforcement" below is a lint rule or a test that the app runs.

Out of scope, with the owner:

- Render purity, hooks, effects inside React components, keys, and external-store tearing: `react-components.md` (agent `react-components`).
- Who owns an object and how long it lives: `data-flow-and-ownership.md` (agent `data-flow`).
- Effect systems in the abstract (IO, Reader, free monads, tagless final): `functional-patterns.md` (agent `fp-effects`).
- Folder layout and dependency direction between folders: `project-structure.md` (agent `project-structure`).
- Type design for unions, brands, and exhaustiveness: `typescript.md` (agent `typescript-types`).
- Platform built-ins (`AbortController`, `URL`, `<dialog>`): `browser-spec.md` (agent `browser-spec`).

## Thesis

**A frontend is a pure function from events to state, plus a thin shell that performs IO.** The view renders state and emits events. The update decides the next state and which effects to request. The effects are values. One interpreter performs them through injected services. Nothing else in the app touches the outside world.

This is the functional core / imperative shell rule from `CLAUDE.md`, applied to a client. The browser makes the shell unusually large and unusually ambient: `fetch`, timers, storage, `location`, `history`, `crypto`, `Date`, and visibility are all globals that any line can call. The architecture exists to make every one of those calls go through one door.

### The five properties

Use these five properties to judge any frontend architecture, library, or change. They recur wherever an architecture is praised for testability and predictability, and they are absent wherever one is criticized.

1. **Pure, inspectable transitions.** The next state is a function of the current state and one event. A test can fold events through the update with no mocks.
2. **Effects described, not performed.** Code that decides an effect returns or dispatches a value. A different piece of code performs it.
3. **A mandatory parse boundary.** Data from the server, the URL, storage, and the user becomes typed domain data at one place, and nowhere else.
4. **A closed vocabulary of events.** Every message is a variant of a closed union. An unhandled variant is a compile error.
5. **Liveness.** State changes reach the view without manual refresh calls, and every subscription has an owner that tears it down.

When you evaluate a library, score it on all five. Example: a library with no single closed union of events fails property 4 structurally, however healthy its maintenance is.

## 1. State

- **One store is the single source of truth for shared state.** The URL, a cache, and component state are projections of the store or are local. They are never a second source of truth.
- **Model actions as events, not setters.** `queueItemResolved`, not `setQueueItems`. An event names what happened. A setter names an implementation and lets several callers fight over one field.
- **Derive, do not store.** A value that a selector can compute from other state is not state. Memoize the selector when the computation is expensive or returns a new reference.
- **Local UI state is a named exception, not a leak.** An open dropdown, a hover, and an unsent text field can live in the component. Write the exception into the project rules so a reviewer can tell "allowed local state" from "shared state that escaped the store". The test: if another component, the URL, a test, or a log ever needs the value, it belongs in the store.
- **Keep the store serializable.** Class instances, `File` objects, promises, timer handles, and unsubscribe functions do not go in the store. See section 3 for where they go.
- **Model optional values as a plain data `Maybe` or a discriminated union**, not as a class. A class instance fails serializability checks and fails structural equality in tests.

## 2. Effects as data

### The shape

An effect is a value in a closed tagged union. The update layer (thunks, reducers that return commands, or event handlers) creates effect values. One interpreter switches over the union and performs each effect through the services.

```ts
type Effect =
  | { kind: "readQueue"; bookId: BookId; digest: Maybe<Digest> }
  | { kind: "startPoll"; token: PollToken; intervalMs: number }
  | { kind: "navigate"; route: Route };

async function runEffect(effect: Effect, services: Services, dispatch: Dispatch): Promise<void> {
  switch (effect.kind) {
    case "readQueue": { /* call services.server, dispatch the result as an event */ return; }
    case "startPoll": { /* services.timer */ return; }
    case "navigate": { /* services.navigator */ return; }
    default: return assertNever(effect);
  }
}
```

Why: a thunk that calls `services.server.readQueue(...)` inline performs an effect. A thunk that dispatches `{ kind: "readQueue", ... }` describes one. The described form lets a test assert on the decision without performing it, and the decision survives a change to how the effect is performed.

### Rules

- **The interpreter is the only code that calls a service.** It is the shell. It awaits, dispatches result events, and can request follow-up effects. It does not need to be pure.
- **Results come back as events.** The interpreter never writes state directly. It dispatches `queueLoaded(result)` and the update decides what changes.
- **Run the reducer before the effect.** In middleware, call `next(action)` first. The state then records the request before the effect can resolve. Otherwise a synchronous result in a test dispatches into a state that did not yet record the request.
- **Catch every fire-and-forget effect at the runner and log it.** Effects run without an awaiting caller, so a throw becomes a silent unhandled rejection. Log the throw as a defect. Do not invent a generic "effect failed" event: expected failures are typed `Result` values that the effect dispatches itself.
- **Exhaustiveness guards one direction only.** `assertNever` catches a variant with no handler. It does not catch a variant that nothing produces, or an event that nothing dispatches. Add a structural test: every effect variant and every exported action creator has at least one production dispatcher. In the source app, both defects that the effect union hid in its first milestone were in the unguarded direction (a poll that was built, tested, and never started).
- **Split the union by feature before it becomes a hub.** One root union with one interpreter imports every feature's slice. The interpreter file then grows without bound. The source app reached 118 variants and one 4,300-line interpreter module, and the escalation trigger the design named ("the switch grows unwieldy") fired with no re-evaluation. Instead, give each feature its own effect union and its own exhaustive handler. The root union is the union of the feature unions, and the root interpreter routes by feature. Exhaustiveness stays, and a feature's effects live next to its state.
- **Name the escalation trigger concretely.** "Adopt a saga or an actor library when it strains" never fires. "Adopt it when two effects must cancel each other across actions" does. `AbortController`, an injected timer, and `Promise.race` cover cancellation, debounce, and racing without a library.

## 3. Subscriptions and resources

A subscription is an effect that lasts: popstate, visibility change, a poll, a websocket, a keyboard listener at document level.

- **Create subscriptions in a factory that the store factory calls.** Each store gets its own listener instances and its own handles. A module-level `let unsubscribe` is process-scoped while the store is not: two stores in one test process share it, and a test leaks into the next.
- **The store factory installs every subscription. The app entry point installs none.** In a test, a subscription that nothing installs looks identical to a subscription that correctly declines to act. A store-level test must assert that each one is installed.
- **Keep non-serializable resources outside the store, keyed by a token in the store.** The store holds `pollToken: "poll-7"`. A registry owned by the store factory holds the timer handle under that key. "The token in the store is the identity. The handle in the registry is the resource."
- **Every subscription returns its own teardown**, and an owner calls it. One `AbortController` per subscription is the usual tool.

## 4. Services

### What becomes a service

Anything that reads or writes outside the process, or is non-deterministic:

- network (`fetch`, websockets, beacons)
- time: the clock (`Date.now`, `new Date()`, `performance.now`) and timers (`setTimeout`, `setInterval`, `requestAnimationFrame` when it schedules logic)
- randomness and ids (`Math.random`, `crypto.randomUUID`)
- storage (`localStorage`, `sessionStorage`, IndexedDB, cookies)
- navigation (`location`, `history`, popstate, leaving to an external URL)
- page lifecycle (`document.visibilityState`, `pagehide`, back-forward cache restore)
- logging (`console`)
- platform detection (`navigator.platform`, `navigator.userAgent`)
- file save and file pick
- the server API client

The clock is the one most often forgotten. The source app banned timers but not `Date`, had no clock service, and shipped thunks that stamped `new Date().toISOString()` into events. Ban `Date` construction without arguments and `Date.now` with the other globals.

### Interface shape

- **One interface per capability.** Do not fold page lifecycle into the navigator because both touch `window`: one name per capability.
- **Domain types only.** No DOM types in a signature. `ITimer.setTimeout` returns `{ id: TimerId }`, not a `number` that only means something to `window`.
- **Fallible calls return a `Result`, never throw.** The network interface returns `Result<Response, NetworkFailure>`.
- **Subscriptions return an unsubscribe function.**
- **Minimal value spaces.** A platform check that the app only branches on as Mac or not-Mac returns `"mac" | "other"`.

### Construction and injection

- **One composition root constructs every real implementation.** Write it as a function (`createRealServices()`). Everything else takes a `Services` bag as input.
- **The bag reaches code through exactly one path: the effect layer.** With RTK that is the thunk `extraArgument` and the middleware factories. Never through a component, a context provider, or a module import. A second path is the crack this convention exists to close, and it widens the lint exemption into component code.
- **Pay the cost of the single funnel on purpose.** A component cannot debounce, because it has no timer. It dispatches on every keystroke and the effect layer debounces. That cost is correct: the debounce is now testable with a fake timer.

### Fakes, and the global "no interface only to mock" rule

`CLAUDE.md` says: do not add an interface for an IO boundary only to mock it. In a browser, this rule and the Services layer do not conflict. Browser IO is ambient: every line can call `fetch` or `Date.now`. The `Services` interface is not a mock seam added for tests. It is the shell boundary itself, and lint enforces it (section 7). Without it, the functional core has no edge.

- **Write fakes with working behavior, not empty spies.** A fake timer keeps a pending queue that the test flushes. A fake id generator returns unique, deterministic ids from a counter. An empty `vi.fn()` id generator returns `undefined` for every id, and in the source app it hid a real collision bug.
- **Provide `makeFakeServices(overrides: Partial<Services>)`** that builds the whole bag from fakes. Tests override only what they examine.
- **Each real implementation is a thin wrapper over one global**, with its own browser-mode test against the real global. That test is the shell's integration test.

## 5. View

- **Two kinds of component.** A container reads the store through selectors and dispatches. A presentational component takes data as properties and emits events upward. Most components are presentational.
- **Components dispatch and render. They never call a service.** No `fetch`, no timer, no storage, no `location`.
- **Trigger loads from intent, not from render.** A data load starts because an event happened (a route changed, a button was pressed), so a thunk on that event requests the effect. A load started from a render or update hook (`useEffect`, Lit `willUpdate`) needs a "did I already request this" guard in the component. That guard is local state that duplicates store state. The source app did this, and every page carried its own dedup field.
- **Get the store through context, not a module singleton.** A singleton makes every component test share one store and one set of services.
- **The store is the test seam for a container.** A container with no callback properties has nothing to spy on. Mount it with a real store over fake services and assert on what renders.
- **A message in the view is data.** A toast or a menu item that carries "what to do when pressed" carries an action value, not a callback. The reducer stays pure and the action is logged like any other.
- **Write down the policy for DOM listeners in components.** Document-level key handlers, window resize, and layout reads (`innerHeight`) are not IO in the effect sense. They are global subscriptions all the same. Route them through a subscription service, or permit them in components with a required teardown. The source app allowed them by omission, which is the worst of the three choices.

## 6. The URL is a projection

- **The route is a slice of the store.** To navigate, dispatch.
- **One read path and one write path.** Read: on load, parse the URL into the initial state before the store exists. On popstate, parse the URL and dispatch `hydrated(route)`. Write: one middleware compares the route before and after each action and calls `navigator.pushState` or `replaceState`. It skips `hydrated`, so a back press does not push.
- **Push or replace follows the meaning of the change.** A change of destination pushes, because Back must undo it. A refinement of the current view (a filter, a sort, a cursor) replaces. "Path is what you are looking at. Query is how you are looking at it."
- **The route is a closed union. `matchRoute` and `routeToUrl` are pure.** Test them by round trip: `matchRoute(routeToUrl(route))` equals `route` for every variant. The round trip catches a field that the type carries and the URL drops. That defect shape caused every routing bug the source app recorded.
- **When the router libraries are unstable, hand-roll the router.** A closed union, two pure functions, and one middleware is less code than most router integrations. The app owns all of it.

## 7. Parse boundary and errors as values

- **Parse every inbound value at the boundary** with a schema, ideally generated from the server's types. Call the non-throwing form (`safeParse`).
- **Map transport status before parsing the body.** Otherwise an expired session (401) reads as a decode failure.
- **Errors are values.** Every server call returns a `Result` with a closed error union. Shared transport errors (network, unauthorized, forbidden, decode) plus one domain error union per operation.
- **Enumerate errors where they are produced, not where they are consumed.** The service enumerates every failure. The UI handles the cases it has real user-facing behavior for and sends the rest to one default.
- **Optional: ban `throw` and `try` outside the shell** with lint, and permit them in services, `assertNever`, and tests. This is strong and it works, but it needs a `Result` library the whole team uses. Do not add a second `Maybe` or `Result` type from a different library: two competing types for one concept is worse than either.

## 8. Enforcement

A rule that lint, the type checker, or a test does not check is a wish. In the source app, every unenforced rule was violated: feature isolation (one feature imported eleven others' internals), `shared` depending upward on `app`, the clock, and DOM listeners.

- **Ban effectful globals outside `services/`** with `no-restricted-globals` and `no-restricted-syntax` (for member access such as `window.location` and `globalThis.fetch`). Every message names the replacement: `"Use services.timer (ITimer) through an effect."`
- **Put `services/` and the test harness outside the feature layers**, at the top level. An exemption that names a top-level directory is easy to read and hard to widen.
- **Test the lint configuration itself.** Lint an in-memory probe file at a fake path inside `services/` and at one outside it. Assert that each ban fires outside and stays silent inside. A later config edit cannot silently drop a ban.
- **Enforce import direction with a rule, not a README.** `import-x/no-restricted-paths` or `eslint-plugin-boundaries`: `shared` imports nothing from `features` or `app`, a feature imports another feature only through its public entry, and `app` composes. Add `import-x/no-cycle`.
- **Write structural scan tests for what lint cannot see.** Every action creator has a producer. Every server operation has a client method. No raw `<button>` outside the design system. Share one comment-stripping scanner between these tests.
- **Strict compiler flags:** `strict`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, `noImplicitOverride`, `noFallthroughCasesInSwitch`, `verbatimModuleSyntax`. Lint `switch-exhaustiveness-check`. If the view layer has a template checker, type-check templates.
- **One gate.** One script runs type check, lint with `--max-warnings 0`, format check, and tests. CI runs the same script, so there is one definition of "passing".

## 9. Testing by layer

| Layer | Runner | What it asserts |
|---|---|---|
| Reducers, selectors, pure model and view-model functions, route matching | Unit (Node) | Fold events through the reducer. No mocks. Round-trip properties. |
| Decision logic in a thunk (which effect, with which arguments) | Unit (Node) | The effect value, with a recording dispatch. Use for branching logic only. |
| Thunk plus effect plus reducer, end to end inside the app | Unit (Node), real store, fake services | Configure the fake server answer, dispatch the intent, wait for a selector. This is the default thunk test. |
| The interpreter, one variant at a time | Unit (Node) | Service calls and the events it dispatches, with fakes. |
| Containers and presentational components | Real browser (Vitest browser mode, Playwright component tests) | Mount with a real store over fake services. Drive real presses, not dispatched `click()` events. |
| Real service implementations | Real browser | Each wrapper against its real global. |
| The whole stack | Playwright against the real server | User journeys. |

Two lessons from the source app:

- **The effect-value test is blind to defects inside an action creator.** The expected value and the code under test call the same action creator. A defect inside a `prepare` callback then appears on both sides and cancels out. Write the expected value as a literal, or test through the store.
- **A synthetic `click()` is not a press.** A real mouse did not open one menu, and that menu passed every test that dispatched clicks. For behavior the platform owns (focus, pointer capture, outside-click dismissal), at least one test drives a real press.

## 10. Feature organization and naming

Feature folders with colocation are the settled default. Feature-Sliced Design, Bulletproof React, the Redux style guide ("feature folders with single-file logic", priority B), and screaming architecture all reach this shape from different starting points. No current source argues for a parallel test tree or for folder-by-type. See `project-structure.md` for the full trade-off.

### Top-level layers

Three layers, after Bulletproof React. Feature-Sliced Design's six layers are overkill below a large team, by that methodology's own documented criticism.

```
src/
  main.ts       entry point: create services, read the URL, create the store, mount the app
  app/          store factory, root reducer, middleware, root interpreter, app shell, page outlet
  features/     one folder per domain area
  shared/       no domain knowledge: design-system components, pure helpers, view base classes
  services/     service interfaces, real implementations, composition root
  test/         fakes, makeFakeServices, render-with-providers, structural scan tests
  generated/    output of code generators (wire types, schemas), never edited by hand
```

- **Dependency direction: `shared` <- `features` <- `app`.** `app` composes features. Features do not compose each other. `shared` imports nothing from `features` or `app`, not even a type.
- **`services/` and `test/` sit outside the three layers.** Code imports service interfaces as types anywhere. Only the composition root imports a real implementation.
- **Keep pure helpers out of `services/`.** A pure URL builder in `services/` becomes a value import from `services/` into feature code, and it blurs the lint exemption.
- **State that two features both need belongs in `app/` or in its own entity feature**, not inside one of the two.
- **A selector that combines two slices** belongs in the feature that consumes the combination, or in `app/`. It never belongs in `shared/`.
- **Name `shared/` subfolders by what they hold**: `components/`, `format/`, `keyboard/`, `lib/`, `styles/`. Do not create `utils/`, `common/`, `misc/`, or `helpers/`. A file with no better home than `utils/` usually belongs to the one feature that uses it.

### Inside a feature

Name the feature folder after the domain, as a user or product owner says it: `review-queue/`, `token-table/`, `publish/`. Do not name it after a technical role (`api/`, `forms/`, `modals/`). To delete a feature, delete one folder.

```
features/review-queue/
  state/
    reviewQueueSlice.ts             slice: reducer, events, selectors in one file
    reviewQueueSlice.test.ts
    reviewQueueThunks.ts            intent: read state, guard, dispatch "started", request an effect
    reviewQueueThunks.test.ts
    reviewQueueEffects.ts           the feature's effect union and its handler (section 2)
    reviewQueueListener.ts          subscription factories, when the feature has any
  model/
    queueOrdering.ts                pure domain logic and view-models, no framework imports
    queueOrdering.test.ts
  components/
    ReviewQueuePage.ts              container: reads selectors, dispatches
    ReviewQueuePage.browser.test.ts
    QueueGroupTable.ts              presentational: properties in, events out
    QueueGroupTable.stories.ts
  reviewQueueFixtures.ts            test-data builders for this feature's tests and stories
```

- **Use one fixed set of subfolders.** `state/` holds the slice, thunks, effects, and listeners. `model/` holds pure logic and view-models. `components/` holds views. A small feature can stay flat until it has two files of one kind. When it grows, it moves to the full shape, not to a private variant.
- **`model/` is the functional core of the feature.** No store, no framework, no services. These files get plain unit tests and are the cheapest code in the app to test. Move logic out of components and thunks into `model/` until both are thin.
- **Colocate tests, stories, and styles with their source file.** `Foo.ts`, `Foo.test.ts`, `Foo.browser.test.ts`, and `Foo.stories.ts` sit in one directory.
- **One slice per feature by default.** A second slice is fine when the feature holds two state lifetimes that reset independently. Name it `<feature><Concept>` (`witnessesOcrRuns`, not `ocrRuns`), so an action type in a log still names the feature.
- **A feature that reads and writes another feature's slice** is a sign of one of two things. Either the two are one feature, or the slice belongs in `app/`.
- **No feature `index.ts`** unless an import-boundary rule makes it the only legal entry (see "Imports").

### File and symbol naming

Pick one convention per role and hold it everywhere. The table is the convention that worked. Consistency matters more than the specific choice.

| Role | File | Exported names |
|---|---|---|
| Slice | `<feature>Slice.ts` | `<feature>Slice`, events as `<feature>Slice.actions.<eventPastTense>`, selectors as `select<Thing>` |
| Thunks | `<feature>Thunks.ts` | Verb-first intent: `loadQueue`, `resolveFlag`, `beginMove` |
| Effects | `<feature>Effects.ts` | `<Feature>Effect` union, `run<Feature>Effect` handler |
| Subscriptions | `<feature>Listener.ts` | `create<Feature>ListenerMiddleware(services)` |
| Pure logic | `model/<concept>.ts`, camelCase | Plain functions named for the domain concept |
| Component | `PascalCase.ts`, plus a framework suffix when the project mixes view layers | `PascalCase` class or function. Custom element tag `<app-prefix>-<kebab-case-of-class>` |
| Test data | `<feature>Fixtures.ts` | `make<Thing>(overrides)` builders |
| Service | `services/I<Capability>.ts`, `services/<Platform><Capability>.ts` | `I<Capability>`, `Browser<Capability>`, test fake `Fake<Capability>` |
| URL schema | `<feature>UrlSchema.ts` | The schema for the feature's query parameters |

- **One stem for the folder, the slice, the action-type prefix, and the file names.** Folder `token-table/`, slice `name: "tokenTable"`, thunk types `"tokenTable/beginMove"`, files `tokenTable*.ts`. A slice named `ocrRuns` in folder `witnesses/`, or `identity` in folder `token-identity/`, breaks the trail. A reader cannot then find an action's owner from its type string in a log.
- **Events are past tense. Thunks are imperative.** `queueLoaded`, `cursorMoved`, and `loadStarted` are events. `loadQueue` and `resolveFlag` are intents. The name then tells a reader whether it changes state directly or requests work.
- **Selectors start with `select`.** A derived boolean reads as a predicate: `selectQueueIsArmed`.
- **One prefix rule for fixtures.** `reviewQueueFixtures.ts`, not `queueFixtures.ts` in one feature and `tokenTableFixtures.ts` in the next.
- **A test file's base name matches its source file exactly, including case.** On a case-insensitive file system, `toggle.browser.test.ts` beside `Toggle.ts` passes locally. On Linux CI it fails, or it tests the wrong file. A scan test catches this.
- **Test suffixes select the runner.** `*.test.ts` runs in Node. `*.browser.test.ts` runs in a real browser. `*.capture.ts` takes screenshots. A browser test misnamed `*.test.ts` passes for the wrong reason: the DOM it needs is a stub.
- **Every custom element tag gets one app prefix**, derived mechanically from the class name. Two apps or two libraries on one page then cannot collide.

### Imports

- **Use the alias for anything outside the current directory** (`@/features/session/state/sessionSlice`). Ban `../` with `no-restricted-imports`. A file can then move without breaking its relative paths, and a reader sees the layer in the import.
- **No barrel files** (an `index.ts` that re-exports a folder) by default. Barrels defeat tree-shaking, create import cycles, and hide which file holds a symbol. The one exception is a feature's public entry, when an import-boundary rule requires one.
- **`import-x/no-cycle` and `import-x/order`** (grouped, alphabetized) on every file.

### Enforce the shape, or it drifts

In the source app, a README stated the shape and nothing checked it. Within two months:

- Four features kept their slices at the feature root, and the rest used `state/`.
- One feature kept its view-models at the feature root, and the rest used `model/`.
- Fixture files took three different prefix rules.
- Five features held two or three slices each, and eight slice names did not start with their folder stem.
- One feature imported the slices, thunks, and fixtures of eleven other features.
- `shared/` imported from `app/`.
- The spec adopted a no-barrel lint rule that never reached the lint config.

Each one is cheap to prevent and expensive to find later:

- **Import boundaries:** `import-x/no-restricted-paths` or `eslint-plugin-boundaries`, with zones for `shared`, each feature, `app`, and `services`.
- **Folder shape and file names:** a scan test that lists every feature folder and asserts the permitted subfolders and file-name patterns. Keep a documented exception list, so an exception is a reviewed diff and not an accident.
- **Name agreement:** a scan test that each slice's `name` and each thunk's type prefix match the folder stem.
- **Test-file casing:** a scan test, as above.
- **Every spec decision that names a lint rule:** a probe test (section 8) that proves the rule is in the config and fires.

## 11. Failure modes to flag in review

- A component, a selector, or a reducer that calls `fetch`, a timer, storage, `Date.now`, `new Date()`, `Math.random`, or `location`.
- A thunk that calls a service inline instead of requesting an effect. A documented exception is permitted, with its reason beside the code. Example: a `File` that cannot pass a serializability check.
- A `Services` bag passed into a component, a context, or a module-level import.
- A module-level `let` holding a handle, an unsubscribe function, or a debounce slot.
- An effect variant or action creator with no production dispatcher.
- One effect union and one interpreter file shared by every feature, past a few dozen variants.
- A data load started from a render or update hook with a local "already requested" guard.
- A route field that the type carries and the URL does not.
- A fire-and-forget effect with no rejection handler.
- A rule stated in a README with no lint rule or test behind it.
- A feature that imports another feature's slice, thunks, or fixtures directly, or `shared/` importing from `app/` or `features/`.
- A slice name or action-type prefix that differs from its folder stem.
- A feature folder whose layout differs from the project's fixed shape, with no entry in the exception list.
- A new `utils/`, `common/`, or `helpers/` folder, or a barrel `index.ts`.
- An empty `vi.fn()` standing in for a service whose return value the code uses.

## 12. When to skip the machinery

A form with three fields and one submit does not need a store, an effect union, or a services bag. Use local state and one injected client. Adopt the architecture when two or more of these are true:

- Distant components share state.
- The URL must restore a view.
- Effects need cancellation or polling.
- A reviewer must read every transition in a log.

If you adopt it, adopt all of it. A half-applied effect boundary has the cost of both styles.

## Appendix: the Redux Toolkit mapping

| TEA concept | Redux Toolkit |
|---|---|
| Model | The store, one slice per feature (`createSlice`) |
| Message | An action. Name it as an event: `feature/somethingHappened` |
| Update (state part) | The slice reducer |
| Update (command part) | A thunk (`createAsyncThunk.withTypes<{ state; dispatch; extra: Services }>()`) that reads state, guards, dispatches a "started" event, and dispatches `perform(effect)` |
| `Cmd` | `perform = createAction<Effect>("effects/perform")` plus one effect middleware that calls `runEffect` after `next(action)` |
| `Sub` | `createListenerMiddleware()` built in a factory that takes `Services`, one instance per store |
| Services injection | `configureStore({ middleware: (getDefault) => getDefault({ thunk: { extraArgument: services } }).concat(...) })` |
| Selectors | `createSelector`, in the slice file |
| View connection | A context-provided store. A base container class (Lit) or `useSelector` / `useDispatch` (React) |

Middleware order is a decision, not an accident. Use logger, then effects, then URL sync. Every action is logged before anything acts on it, effects see the reduced state, and the URL reflects the final route.

Keep `serializableCheck` on. When it refuses a value, the value belongs in a resource registry (section 3), not in an exemption list.

## Authorities

- The Elm Guide, "The Elm Architecture" and "Commands and Subscriptions": https://guide.elm-lang.org/architecture/
- Redux Style Guide (priority levels A, B, C): https://redux.js.org/style-guide/
- Redux Toolkit, listener middleware: https://redux-toolkit.js.org/api/createListenerMiddleware
- Bulletproof React, project structure: https://github.com/alan2207/bulletproof-react/blob/master/docs/project-structure.md
- Firezone, "sans-IO": https://www.firezone.dev/blog/sans-io
- Alexis King, "Parse, don't validate": https://lexi-lambda.github.io/blog/2019/11/05/parse-don-t-validate/
