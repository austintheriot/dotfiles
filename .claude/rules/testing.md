---
paths:
  - "**/*.{test,spec}.{ts,tsx,js,jsx,mjs,cjs}"
  - "**/*_test.{py,go}"
  - "**/test_*.py"
  - "**/*Tests.{swift,kt}"
  - "**/tests/**"
  - "**/__tests__/**"
  - "**/*.{ts,tsx,js,jsx,py,rs,go,swift,kt}"
---

# Testing

The guiding principle: a test should read as a short story about one behavior, and it should not be able to influence, or be influenced by, any other test. Push setup into helpers that hand the test a fully-configured, isolated world. Then make the assertions look as much as possible like how the software is actually used.

Two slogans worth internalizing, both from Kent C. Dodds:

- **"The more your tests resemble the way your software is used, the more confidence they can give you."** This is the answer to most "should I test X this way?" questions.
- **"Write tests. Not too many. Mostly integration."** Integration tests catch the bugs that matter (the seams between units) at a fraction of the cost of end-to-end tests and with far more confidence than isolated unit tests. Pure-logic helpers still deserve unit tests; thin glue code rarely does.

## Isolation is non-negotiable

- **Every test gets its own state.** No shared mutable fixtures across tests. No "first test seeds the store, second test reads it." If two tests in the same file are coupled by ordering, they are one test pretending to be two, or they have a bug waiting to happen when the runner parallelizes or shuffles them.
- **Isolate at the right boundary, then commit to it.** Whatever resources the system under test owns (data stores, listening sockets, queues, caches, in-memory singletons), give each test its own freshly-constructed instance. Concrete shapes that work across languages: a per-test database with a randomly-generated name, a server bound to an ephemeral port assigned by the kernel, a per-test fake of every outbound dependency. Pick one isolation strategy per suite and apply it uniformly. Mixing "mostly isolated, except this one shares the global instance" is where flakes are born.
- **Fake outbound dependencies at the wire**, not by patching internal modules. If your code makes Hypertext Transfer Protocol (HTTP) requests to a payment provider, stand up a local mock HTTP server and point the production client at it; don't patch the client object itself. The wire-level fake exercises your real serialization, retry, and parsing code; the module patch lets all of that drift undetected. The same principle applies to message brokers, blob storage, and other network services: prefer the in-process server or local emulator over reaching into your own code.
- **No reliance on wall-clock time, real network, real filesystem, or process-global singletons** unless that is what's under test. Inject a clock, inject the HTTP client, inject the storage; production wiring constructs the real ones, test wiring constructs fakes. Dependency injection is what makes this cheap.
- **Determinism beats convenience.** Drive async work explicitly (a method on the harness that runs the background worker until its queue is empty) rather than `sleep(500)`. Seed any randomness. If something can only be observed by polling, the production code is missing a hook.

## Test harnesses do the work

The bias is strongly toward **more harness, less duplication**. Repeated setup in test bodies rots fast: the harness is where API changes get absorbed once instead of fifty times.

- **One entry point per suite that returns a handle to everything.** The canonical shape: a single `spawnApp()` (or `makeWorld()`, `bootTestSystem()`, `setupApp()`) function that boots an isolated instance of the system under test and returns a single value -- struct, object, record, whatever the language calls it -- with fields for every resource the test will touch (`address`, `db`, `mockEmailServer`, `apiClient`, `testUser`, `clock`, ...). Tests open with one line: `const world = await spawnApp();` and read top-to-bottom from there.
- **Wrap interactions in methods on the harness from the start.** `world.subscribe(body)`, `world.dashboard()`, `world.user.login()`. Test bodies should contain *what is being tested*, not the wire format, headers, auth dance, or paging logic. When the underlying call changes, you update the harness, not every test that touched it. Lean toward harness methods even when only one test currently uses them: the second caller will come, and the per-test maintenance cost of "I'll duplicate now, refactor later" is consistently higher than the cost of writing the wrapper. The exception is genuinely one-off, idiosyncratic setup that won't generalize -- inline that.
- **Factories with sensible defaults and overrides** for domain objects: `makeUser({ admin: true })`, `makeSubscription({ tier: 'pro', daysRemaining: 0 })`. The factory fills in every required field; the test overrides only the fields it cares about. A reader can see at a glance which fields are load-bearing for the assertion.
- **Helpers should produce *refined* types, not raw shapes** (see "Parse, don't validate" in `coding-style.md`). `makeUser` returns a fully-typed `User`/`TestUser` whose invariants are guaranteed, not a loose record that callers have to re-validate.
- **Custom assertions for recurring shapes**: `assertRedirectsTo(response, '/login')` beats two lines of status-and-header checking repeated forty times, and the failure message is better.

## Test like a user, not like an implementor

This is Kent's "testing implementation details" point, generalized to any kind of code:

- **Identify your users, then test from their perspective.** An HTTP service's users are clients sending requests over the wire: test by issuing real requests through the harness, not by calling controller methods directly. A library's users are developers calling its public API: test through the public API, not the internal modules. A user interface's users are humans driving the interface plus developers passing inputs: test through what the interface renders and the events it dispatches, not its internal state.
- **An implementation detail is anything users of your code don't see, use, or know about.** Internal state, private methods, helper modules, the names of internal variables. Asserting on these creates a third "test user" that the code has to satisfy in addition to its real users, which is what makes refactor-induced test churn so painful.
- **Two failure modes you're trying to avoid simultaneously**: false negatives (tests fail when behavior is unchanged but internals were refactored) and false positives (tests pass when the behavior is broken because they never exercised the user-visible path). Testing through user-visible seams kills both.
- **Avoid replacing real collaborators with stand-ins to make tests "simpler".** Heavy mocking of components, modules, or layers you own severs the integration you're trying to verify. If the real version is too slow or too noisy, that's a hint about a real bug or a real design problem, not a testing problem to mock around.
- **Query the way a real user would.** When testing user interfaces, prefer queries grounded in what the user perceives (the visible label of a control, the role of an element, the text on screen) over queries grounded in implementation choices (a class name, a test-only identifier, a Cascading Style Sheets (CSS) selector). Test-only identifiers are a last resort. Apply the same idea everywhere: when you have a choice between asserting on something a user observes and something only a maintainer would notice, pick the user-observable thing.

## Writing the test itself

- **Arrange / Act / Assert, visibly.** Three blocks, often three comments. If "arrange" is more than a few lines, that setup belongs in the harness or a factory.
- **Test behavior, not implementation.** Assert on observable outputs (responses, persisted state, emitted events, rendered output). Don't assert that a particular internal method was called unless that call *is* the contract being tested (e.g. when the unit under test is a wrapper whose job is to delegate).
- **One behavior per test.** Multiple assertions are fine when they all describe the same behavior (status + body + side-effect). Don't pack two scenarios into one test to save a harness setup call; that's what parallel test runners are for.
- **Names describe the scenario and expected outcome**, not the function under test. `subscribing_with_an_invalid_email_returns_400` beats `test_subscribe_2`. Table-driven negative cases should fold the case description into the failure message so a failing row is self-explanatory.
- **Never disable a failing test.** Fix it, or delete it with a note in the commit about why the behavior is no longer required. A skipped test rots and lies.

## Structure: prefer functions over nested grouping + lifecycle hooks

Kent's "avoid nesting" argument, distilled:

- **Nested grouping plus per-group setup/teardown hides data flow.** Reading a test means scanning upward through several lifecycle hooks to figure out what `user` or `subject` actually is, including which level last reassigned it. This is the single biggest source of test-maintenance pain in any framework that supports it.
- **Reach for plain functions instead of lifecycle hooks.** A `setup()` helper that returns the configured world -- `let world = setup(admin: true)` -- makes the data flow visible in the test body and lets each test specialize without mutating shared variables.
- **Bias toward the harness; abstract early when the duplication is real.** Two identical setup lines that will obviously be repeated are worth wrapping right away. Genuinely one-off setup can stay inline. The aim is that tests read top-to-bottom: the harness call, the action, the assertion.
- **Group by file, not by deeply nested grouping.** One level of grouping for the unit under test is plenty; split into more files before you nest more.
- **Scope lifecycle hooks tightly when you use them at all.** Scope to the smallest group that needs them. File-level hooks leak across unrelated groups -- a recurring source of cross-test interference in real codebases.
- **Never reassign a shared variable inside `beforeEach` / `afterEach` / `setUp` / `tearDown` (or equivalent).** A `let user; beforeEach(() => { user = makeUser(); })` pattern looks isolated but breaks isolation the moment any test mutates `user` after a hook in a different lifecycle phase has reassigned it -- or the moment the runner parallelizes, shuffles, or runs a single test from the middle. Each test must own its own bindings: call `const user = makeUser()` (or `const world = setup()`) inside the test body, or have the setup helper return a fresh object the test names locally. Lifecycle hooks should do framework wiring (start a server, open a connection) and teardown only; they should not assign to file-scoped or describe-scoped variables that tests then read.
- **Avoid mutating process globals, environment variables, module-level singletons, or platform objects from a test or a lifecycle hook.** Setting up and clearing framework-managed mocks and stubs in `beforeEach` / `afterEach` (`vi.spyOn`, `jest.spyOn`, `vi.restoreAllMocks`, etc.) is fine -- the framework owns the save/restore. Direct assignment to globals is the problem: patterns like `let originalNavigator; beforeEach(() => { originalNavigator = globalThis.navigator; Object.defineProperty(globalThis, 'navigator', {...}) }); afterEach(() => { Object.defineProperty(globalThis, 'navigator', {value: originalNavigator, ...}) })` -- or the equivalents with `process.env.FOO`, `window.location`, a singleton client, a module-level cache -- are an isolation failure dressed up in a save/restore ritual. Any test running in parallel sees the mutated global; any test that crashes between mutate and restore leaks the mutation to every later test in the same worker; the save/restore code itself is untested. Prefer injecting the dependency: take `navigator` (or the clock, the env, the client) as a parameter to the code under test, and have the harness construct a test-controlled instance per test. When a third-party library reads a global you cannot reasonably route around, the save/restore pattern is an escape hatch -- use it sparingly, and document why dependency injection wasn't viable.

## Mocking: at the boundary, sparingly

- **Mock at system boundaries**: anything that crosses a process or network edge -- HTTP services, third-party Software Development Kits (SDKs), payment processors, email/Short Message Service (SMS) providers, message brokers, the wall clock, animation/timing libraries. Mock the *protocol* (a fake server speaking the same wire format) more often than the *client object* (patching out the SDK); the protocol-level fake exercises your real client code.
- **Don't mock what you own.** If you're tempted to mock your own service layer to test your controller, the seam is in the wrong place: either test the controller against a real (in-memory) service, or test the service directly. Mocking your own code freezes the current shape of internal collaborators into the test suite, and the assertions become "the controller called the method I expected" rather than "the system did the thing I expected."
- **Never mock for speed alone.** "It's faster" is the most common bad reason to mock and the one that costs the most confidence. If a real test is too slow, the underlying code probably has a perf issue worth finding.
- **Every mock is a bet that the real thing behaves the way you stubbed it.** Keep that surface small and explicit; assert that the mock was called (count, args, sequence) so you notice when the contract drifts under you.

## Common pitfalls

- **File-level lifecycle hooks leak across unrelated test groups.** Scope hooks to the smallest group that needs them, or put setup in the harness call inside each test.
- **Snapshot tests as a substitute for assertions.** A snapshot tells you something changed; it doesn't tell you whether the new behavior is correct. Reserve snapshots for output where the *exact* shape is the contract (rendered output for a stable API, a generated artifact), and write real assertions for everything else.
- **Querying by test-only identifiers when a user-meaningful one exists.** Test IDs are an escape hatch; reach for them only when no query grounded in user-visible state works.
- **Chasing 100% coverage.** Beyond roughly 70%, the marginal test is usually exercising trivial code or asserting on implementation details. Spend the effort writing the integration test you skipped instead.
- **Sharing a "test user" or other entity across tests** because creation is "slow." Either creation is fast enough (usually true), or the harness should create one per test from a template/clone, not hand back the same record.
- **Waiting wrong.** Don't busy-wait with `sleep`; don't put side effects inside a polling/awaiting block; don't double-wrap helpers that already wait; one assertion per wait.

## Touchstones

- **Rust, integration gold standard**: `LukeMathWalker/zero-to-production` final-chapter `tests/api/helpers.rs` and `tests/api/*.rs`. A single `spawn_app() -> TestApp` with per-test Postgres database (UUID name), per-test `wiremock::MockServer` for the email provider, wrapped HTTP methods on `TestApp`, tests that read as arrange/act/assert in 10-20 lines. This is the clearest worked example of the principles in this file.

## Further reading

The principles above are condensed from Kent C. Dodds's testing essays. When a judgment call comes up that this file doesn't cover, his pieces are the place to look:

- "Write tests. Not too many. Mostly integration." -- the testing trophy and why integration beats unit for return on investment.
- "Testing Implementation Details" -- the canonical definition of implementation detail and the two-failure-modes framing.
- "Avoid Nesting when you're Testing" -- functions over hooks.
- "The Merits of Mocking" -- which boundaries to mock and which to leave alone.
- "Common mistakes with React Testing Library" -- query priority, debugging, async waits. (UI-specific but the underlying principles generalize.)
