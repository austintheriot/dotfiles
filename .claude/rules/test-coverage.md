---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Test Coverage Principles

A reference for identifying under-tested code during review. Used by the `test-coverage` subagent. Companion to `~/.claude/rules/testing.md` (which covers *how* to write tests well) -- this file covers *what* to test and *how to find* what's missing.

The core thesis: code coverage is necessary but not sufficient. 100% line coverage with assertion-free tests proves nothing. The right question is not "is every line executed?" but "is every load-bearing behavior, branch, and edge case covered by a test that would fail if it broke?"

The subagent's job: find the gaps between what's tested and what *should* be tested, prioritized by failure-cost.

---

## Discover the project's testing landscape

Before reasoning about gaps, learn what the project already has. Each repo has its own conventions, tooling, and existing coverage. Spend the first part of the review discovering, not opining.

### What to look for

- **Test frameworks in use.** Jest / Vitest / Mocha (JS); pytest / unittest (Python); cargo test + nextest + proptest (Rust); JUnit / Kotest (JVM); XCTest (Swift); Go's built-in. The framework dictates idiomatic test shapes.
- **Test directories.** Conventions vary: colocated `*.test.ts` next to source, separate `tests/` directory, `__tests__/`, language-specific (`src/.../tests.rs`, `Tests/` for Swift). Find them all.
- **Test types in the repo.** Unit, integration, end-to-end (E2E), contract, property-based, snapshot, mutation. Different test types catch different bug categories; the gap analysis differs depending on what's already present.
- **Existing coverage configuration.** `coverage.json`, `.nycrc`, `jest.config.js` with `collectCoverageFrom`, `pytest --cov` config in `pyproject.toml`, `cargo-llvm-cov` setup, `tarpaulin.toml`, gradle JaCoCo. The presence (or absence) of coverage tooling tells you what the team values.
- **CI test commands.** Read `.github/workflows/`, `.circleci/config.yml`, `Makefile`, `package.json` scripts, `justfile`. Find the commands that run tests and produce coverage.

### Tools to run when present

If the repo has tooling configured, **run it** and read the output. Don't just guess at coverage. The subagent has Bash access; use it.

- **Coverage reports**: `npm test -- --coverage`, `pytest --cov --cov-report=term-missing`, `cargo llvm-cov --summary-only`, `go test -cover`, `mvn test jacoco:report`. Most produce a per-file or per-function percentage and uncovered line ranges.
- **Mutation testing** (where configured): Stryker (JS), mutmut / cosmic-ray (Python), cargo-mutants (Rust), PIT (Java). Mutation testing measures whether tests actually catch behavioral changes -- a far stronger signal than line coverage.
- **Profiling and flamegraphs** (for performance-sensitive code): `cargo flamegraph`, `perf record + perf script`, py-spy, Node `--inspect` + Chrome devtools, async-profiler. Use these to find **hot paths** -- a hot uncovered function is higher risk than a cold uncovered one.
- **Property-based test generators** already in use: hypothesis, fast-check, proptest, ScalaCheck. Presence indicates the team is comfortable with property-shaped tests -- you can suggest more without selling the concept.
- **Snapshot diffs**: if the repo uses snapshot tests, identify whether they're meaningfully asserted or just rubber-stamped.

If a tool is present but not in CI, that's itself a finding -- the data exists but isn't load-bearing.

### When tooling is absent

If no coverage tooling is configured, do **not** stop. Reason about coverage from the code itself: which branches have corresponding test cases? Which functions are referenced in tests at all? Which error paths are exercised? `grep` for function names in test files is a crude but effective coverage proxy.

Also flag the absence: "no coverage tooling configured; running `<framework's coverage command>` would surface uncovered areas mechanically."

---

## Where to focus the gap analysis

Coverage gaps are not all equal. Rank by failure-cost.

### Highest priority: critical paths

The code paths whose failure is most painful. Money flows, authentication, authorization, data persistence, security boundaries, user-facing core features. These deserve high coverage **and** high-confidence tests (real assertions, not just "doesn't throw").

**Specific gaps to flag**: a payment / charge / refund function with no test; an auth check whose negative case ("user is not authorized") is untested; a database migration with no rollback test; an encryption / signing function with only happy-path tests.

### High priority: branch coverage on conditionals

Line coverage misses branch coverage. A function with `if (cond) doA() else doB()` reaches 50% line coverage from testing one branch and 100% from testing both. Modern tools report branch coverage separately; surface it.

**Specific gaps to flag**: every `if`/`else`/`switch`/`match` whose minority branch is untested. The minority branch is usually the error path, the empty case, the degenerate case -- exactly where bugs lurk.

### High priority: error paths and edge cases

Happy-path tests are usually present; error-path tests are often absent. Bugs concentrate in error paths because they're rarely exercised.

**Specific gaps to flag**: error returns / exceptions / Result::Err variants with no test; resource cleanup on error (file close, connection release) untested; partial-failure paths in multi-step operations; retry exhaustion behavior.

### High priority: boundary conditions

Empty / single / max-size / zero / negative / NaN / infinity / first / last / off-by-one. See `~/.claude/rules/bug-patterns.md` § Boundary Conditions for the full catalog. Property-based tests are particularly good here; suggest them when boundaries are numerous.

**Specific gaps to flag**: collection-handling code with no `[]` / `[single]` / `[many, many]` cases; numeric code with no `0` / `MIN` / `MAX` / `-1` cases; pagination with no first-page / last-page / empty-page cases.

### Medium priority: concurrency / async

Tests for concurrent code are notoriously thin because they're hard to write. The frameworks exist (loom for Rust, JMC for Java, JavaScript's `Promise.all` patterns, pytest-asyncio) but uptake is uneven. Even basic "two tasks racing on this state" tests catch most bugs.

**Specific gaps to flag**: shared mutable state with no concurrent test; async functions with no cancellation test; mutex-protected critical sections with no contention test.

### Medium priority: integration / contract tests for I/O boundaries

Unit tests with mocks pass while the integration silently breaks. If a service has a database, queue, HTTP client, file system, the boundary deserves an integration test that uses the real thing (or a high-fidelity fake -- testcontainers, in-memory variants, recorded HTTP fixtures).

**Specific gaps to flag**: a function tested only with mocks where the mock contract is not separately verified against the real dependency; an external API client with no test against a real or recorded response; a database query layer tested only with an in-memory stand-in (sqlite mocking postgres is the classic).

### Lower priority: pure helpers

Pure utility functions are usually well-tested or trivially correct (single expression, easy to read). Coverage gaps here matter less than gaps in stateful, effectful, or boundary code.

### Lowest priority: framework glue

Boilerplate, framework setup, DI wiring, config parsing of declarative formats. Often "tested" by the fact that the app boots. Flag only when there's a genuine logic concern.

---

## Property-based testing opportunities

Property-based tests describe invariants (`forall input: f(g(input)) == input`) and let the framework generate inputs. They catch entire classes of bugs example-based tests miss. Spot opportunities and surface them.

**Properties worth flagging**:
- Round-trip: parse(print(x)) == x, encode(decode(x)) == x, serialize then deserialize, save then load.
- Idempotence: f(f(x)) == f(x). Normalization, deduplication, "set" semantics.
- Algebraic laws: associativity, commutativity, identity. Merge / combine functions, math operations, set unions.
- Monotonicity / preservation: sort preserves multiset, filter preserves order, map preserves length.
- Invariant preservation: every state transition preserves a data invariant.
- Equivalence: two implementations (fast and reference) should agree on all inputs.

**When to suggest**: code with clear invariants (parsers, serializers, math, data transformations, state machines) and a project that already uses property-based testing or has a reasonable barrier to adopting it.

**When not to**: untestable side effects, complex setup, no clear invariant -- example-based tests are right there.

---

## Mutation testing opportunities

Mutation testing changes a small thing (flip a boolean, swap `<` for `<=`, remove a line, change a return value) and runs the test suite -- if no test fails, the mutation "survived" and the code is under-tested *behaviorally* even if it's covered by lines. Mutation score is the percentage of mutants killed.

**When to suggest**: high-stakes code (money, security, persistence) where line coverage looks good but you suspect tests are checking the wrong things; code that handles a small number of cases but with subtle interactions; libraries with a public API that needs strong behavioral guarantees.

**When not to**: mutation testing is slow (5-50x the test runtime); not a default reach for routine review. Suggest as a targeted tool, not a continuous practice.

---

## Anti-patterns to flag in existing tests

Coverage that does not catch bugs. The subagent should also flag *bad* tests, not just missing ones.

- **Assertion-free tests**: test that calls a function and doesn't assert anything. Coverage goes up; the test catches nothing except a thrown exception.
- **Mock-heavy tests where the mocks are the contract**: the test confirms "the code calls `repo.save()` with arg X." If `repo.save()` semantics change, the code breaks; the test still passes. Mocks should be at trust boundaries, not at internal call seams.
- **Snapshot tests with no review**: every change updates the snapshot mechanically. No human ever reads the diff. The snapshot is documentation of nothing.
- **Tests that mirror the implementation**: re-implementing the function inside the test. Refactoring breaks the test even when behavior is unchanged.
- **Brittle UI / E2E tests with no isolation**: shared database state, unsealed network, time-dependent assertions. Flake-prone tests get retried, retries get auto-merged, regressions ship.
- **Tests in the wrong layer**: integration tests where a unit test would suffice (slow, brittle), or unit tests where integration is needed (mocks lying).

See `~/.claude/rules/testing.md` for the full set of testing principles. The coverage subagent should defer to that file for the "how to write good tests" answers; this file's lens is "what is missing."

---

## Output and severity

The `test-coverage` subagent surfaces findings at these severity levels (matching `/expert-review`'s scale):

- **blocker**: a critical-path function or behavior has zero test coverage, OR existing tests cannot detect a known-required behavior. "The charge function has no test that fails when it charges the wrong amount" is a blocker.
- **major**: a high-priority area (auth, persistence, security) has only happy-path coverage; error paths, edge cases, or concurrency cases untested. Or a function the diff modifies has no test that exercises the modified branch.
- **minor**: medium-priority gaps, missing property tests where they'd be a clean fit, mock-heavy tests where integration would catch more.
- **nit**: pure-helper coverage gaps, boilerplate untested, snapshot tests with no apparent review.
- **insight**: structural suggestions ("this module's tests would benefit from a shared fixture / harness," "mutation testing on the payment module would surface latent gaps").

Confidence: high when the gap is concrete and verified ("function `chargeCard` is referenced zero times in any test file"); medium when reasoned ("error path on line 42 has no obvious test, but I cannot exhaustively verify the test suite did not cover it indirectly").

When coverage tooling is available and the agent ran it, cite the report's specifics: "coverage report shows `services/billing/charge.ts` at 47% line, 22% branch; lines 88-104 are the error-handling block."

---

## Process for the coverage agent

1. **Discover**: find test directories, frameworks, coverage tooling, CI commands. Read enough to understand the project's conventions.
2. **Run tooling when present**: coverage reports, mutation tests if feasible (cheap subset), flamegraphs for hot-path identification.
3. **Map code regions to existing tests**: for each region in scope, grep for its function/class name in test files. Note what is and is not referenced.
4. **Prioritize by failure-cost**: critical paths first, then branches, errors, boundaries, concurrency, integration.
5. **Surface gaps as findings**: with file:line, severity, confidence, and a concrete test suggestion ("add a test for `chargeCard` that asserts the amount, currency, and idempotency-key are correct when the upstream returns a partial response").
6. **Flag bad tests too**: assertion-free, brittle, mock-heavy where it shouldn't be.
7. **Suggest tooling when absent**: "this repo has no coverage tooling; adding `vitest --coverage` would surface gaps mechanically."
8. **Stay read-only**: do not write tests. The user decides.
