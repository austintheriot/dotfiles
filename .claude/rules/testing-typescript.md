---
paths:
  - "**/*.{test,spec}.{ts,tsx,js,jsx,mjs,cjs}"
  - "**/__tests__/**/*.{ts,tsx,js,jsx}"
  - "**/tests/**/*.{ts,tsx,js,jsx}"
  - "**/vitest.config.{ts,js,mjs}"
  - "**/jest.config.{ts,js,mjs,cjs}"
---

# TypeScript / JavaScript Testing

Companion to `testing.md` (cross-language testing principles). This file covers Vitest / Jest specifics and the TS/JS testing ecosystem. The cross-language rules in `testing.md` still apply: isolation, harness-driven setup, testing through user-visible seams, mocking at boundaries.

## Concurrency: prefer it, but don't force it

The default goal is fast, isolated tests. Vitest already parallelizes at the file level (one worker per file), which captures most of the available speedup with zero configuration. Concurrent execution *within* a file is an additional lever, not the default one.

- **Prefer `it.concurrent` / `describe.concurrent` (Vitest) for `async` tests within a file when there are multiple independently-awaitable tests and isolation already holds.** Applies only to `async` test bodies; sync tests gain nothing from `.concurrent` (the test body runs to completion before yielding), so the marker is noise on a sync test and shouldn't be the default there. The harness-per-test pattern from `testing.md` (each test calls `spawnApp()` / `setup()` and gets its own world) is the precondition that makes concurrent-in-file safe. Isolation and concurrency are the same property viewed from two angles: tests that share no mutable state run correctly in any order, including simultaneously.
- **Use the scoped `expect` from the test context, not the top-level import**, when running concurrently. Vitest tracks assertion attribution per-test via the local context; the top-level `expect` can mis-attribute a failure to the wrong test under concurrency.

  ```ts
  it.concurrent('subscribes a new user', async ({ expect }) => {
    const world = await spawnApp();
    const response = await world.subscribe({ email: 'a@example.com' });
    expect(response.status).toBe(200);
  });
  ```

  The destructured `expect` is the same API as the imported one; the only difference is per-test attribution. Test bodies and assertion helpers that take `expect` as a parameter compose cleanly with both modes.
- **Don't force it where it doesn't pay.** A file with one or two fast unit tests gains nothing from `it.concurrent` and adds API noise. The wins are concentrated in files with multiple slow async tests (integration tests hitting a real database, tests with built-in waits, tests with substantial setup that can overlap).
- **Jest's `test.concurrent` is experimental and has known issues with mocks.** Treat it as a per-case decision in Jest projects, not a default. The Vitest story is cleaner.
- **Concurrency does not relax the isolation rules in `testing.md`.** File-scoped `let` reassigned in hooks, module-level singletons, shared fakes, and process-global mutation are isolation failures whether tests run sequentially or concurrently; concurrency just surfaces them faster. If a test cannot run concurrently with its siblings, the underlying cause is almost always shared mutable state that should be moved into per-test setup.
