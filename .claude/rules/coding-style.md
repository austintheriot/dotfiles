---
paths:
  - "**/*.{ts,tsx,js,jsx,mjs,cjs}"
  - "**/*.{py,rs,go}"
---

# Coding style

These apply when editing or writing code. They are intentionally opinionated; the goal is that named bindings mean one thing for their whole lifetime and that the type system carries proofs rather than leaving them implicit.

## Functions tell a story

- A function's body should read as a declarative outline of what it does. Reach for well-named helpers so the call sites announce intent; the function name plus the names of the calls inside it should be enough to understand the function without reading the bodies of those calls.
- Prefer composability and smaller units that compose together over monolithic functions that do many things inline.
- Avoid internal mutability inside a function: no `let`-then-reassign accumulators, no flags that get flipped, no arrays built up by `push` in a loop when `map`/`filter`/`reduce` (or a comprehension) expresses the same thing as a single expression. The point is not that pipelines are stylish; it's that each named binding should mean one thing for its whole lifetime, so the reader doesn't have to track how a variable mutates over the body.

## Parse, don't validate

When data crosses a boundary (user input, network response, untrusted call site), parse it once into a type that makes the invariant impossible to violate downstream (`NonEmpty<T>`, `ValidatedEmail`, a discriminated union of legal states) rather than passing the loose shape around and re-checking at every layer. Choose data structures so illegal states are unrepresentable. A function whose primary job is to throw on bad input and return `void`/`()` is a smell: have it return the refined type instead, so the proof travels with the value. This is the principled reason callees don't need to re-validate preconditions: the type already carries the proof.

### TypeScript specifics

Structural typing makes brands easy to forge, so:

- Use **branded types** for refined values (`type Email = string & { readonly __brand: unique symbol }`), and let *only* the parser produce the brand. Never `as Email` at a call site; that collapses the whole guarantee.
- Distinguish raw shapes from trusted ones at the type level (`UnvalidatedUser` vs `User`).
- Treat `JSON.parse` results as `unknown` and route them through a parser (zod / valibot / hand-written) that returns a discriminated `{ ok: true, value } | { ok: false, error }` rather than throwing.
- If the same defensive check shows up in three call sites, that's the signal to lift it into a parser at the boundary and delete the downstream checks.
- Do not use `as any` or `as unknown as _`.

## Architecture principles

- **Composition over inheritance.** Use dependency injection. Prefer composing small units over building large ones.
- **Interfaces over singletons.** Enable testing and flexibility.
- **Explicit over implicit.** Clear data flow and dependencies. Surface errors in explicit ways (typed results, thrown errors with context, exhaustive switches) rather than swallowing them or relying on implicit fallbacks.
- **Prefer pure functions.** Use `const` over `let`, return new values instead of mutating arguments, and push side effects to the edges. Especially avoid global mutation.
- **Lean on existing infrastructure.** Before writing new helpers, search for utilities the project (or adjacent subsystems) already provides. Match their patterns rather than parallel-implementing.

## Error handling

- Fail fast with descriptive messages.
- Include context for debugging.
- Handle errors at the appropriate level.
- Never silently swallow exceptions.

## Tests

Testing principles live in their own file: see `~/.claude/rules/testing.md`. It loads automatically when editing test files or any code under matching source paths. Key points in one line each: isolate every test, push setup into a `spawnApp()`-style harness, test through the user-visible seam (not implementation details), prefer functions over nested `describe`/`beforeEach`, mock only at system boundaries, never disable a failing test.
