---
paths:
  - "src/**/*.{ts,tsx}"
  - "lib/**/*.{ts,tsx}"
  - "app/**/*.{ts,tsx}"
  - "packages/**/*.{ts,tsx}"
  - "services/**/*.{ts,tsx}"
  - "tests/**/*.{ts,tsx}"
  - "test/**/*.{ts,tsx}"
  - "**/*.{test,spec}.{ts,tsx}"
  - "*.{ts,tsx}"
excludePaths:
  - "**/node_modules/**"
  - "**/dist/**"
  - "**/build/**"
  - "**/.next/**"
  - "**/coverage/**"
---

# TypeScript

Cross-cutting principles live in `coding-style.md` (parse-don't-validate, brands, architecture) and `testing.md`. This file is the TypeScript-specific layer: how to design types, manage inference, control the type system itself, and avoid the language's many footguns.

The mental frame underneath everything below: **types are sets of values, and assignability is "is a subset of."** Most surprises (`A & B` having more properties than either, `keyof (A | B)` being `keyof A & keyof B`, distributive conditionals, `never` vanishing inside unions) become obvious once you hold this frame.

## Compiler configuration

- **`strict: true` is the floor, not the ceiling.** Strict misses several flags that catch real bugs. Add at minimum: `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, `noImplicitOverride`, `noFallthroughCasesInSwitch`, `noUnusedLocals`, `noUnusedParameters`. Start from `@tsconfig/bases` (`node-X-strictest-esm` or similar) rather than handcrafting.
- **Configure via `tsconfig.json`, never the CLI.** Examples that pass in one configuration can fail in another. Reproducibility lives in the config file.
- **TypeScript and `@types/*` belong in `devDependencies`.** They don't ship. Don't install TS globally (coworker version drift).
- **Three versions interact in every `@types` install**: the library, its `@types` package, and TypeScript itself. When something breaks confusingly, check `npm ls @types/foo` for duplicates and look for `typesVersions`-gated subpaths.

## The any / unknown decision

- **Default to `unknown`.** `any` is a type-system escape hatch that is both the supertype and the subtype of everything; it silently disables checking and contagiously spreads through inferred return types. `unknown` keeps "anything goes in" but forces narrowing before use.
- **When `any` is unavoidable, give it the narrowest possible scope and the most precise shape.** Prefer `as any` on a single expression over `: any` on a variable or parameter. Prefer `any[]`, `Record<string, any>`, `(...args: any[]) => any` over bare `any`. Never let `any` escape a function's public return type.
- **`as unknown as Foo` beats `as any as Foo`.** Both bypass checking, but `unknown` makes the unsafety honest.
- **Use `// @ts-expect-error` over `// @ts-ignore`.** Expect-error self-removes (errors) once the underlying issue is fixed; ignore is a black hole. Always pair with a one-line justification comment.
- **`unknown` over `{}` over `object` for "some value."** `object` excludes primitives. `{}` accepts everything except null/undefined, which is rarely what you want. `unknown` is honest.
- **Track type coverage** (e.g., `type-coverage` in CI) to catch implicit `any` leaks via `@types/*` packages even when `noImplicitAny` is on.

## Type design (the real expert layer)

This is where the highest-leverage decisions live.

- **Make invalid states unrepresentable.** A state shape that can encode `isLoading: true` AND `error: "..."` AND `data: ...` simultaneously forces every consumer to ask "what does this combination mean?" -- and answer it inconsistently. Verbose tagged unions that are 3-4x the naive shape are usually still the right call. This is the single most important type-design principle.
- **Prefer unions of interfaces to interfaces with unions.** `{ kind: "circle"; radius: number } | { kind: "square"; side: number }` admits three valid states. `{ kind: "circle"|"square"; radius?: number; side?: number }` admits sixteen, most invalid. The discriminant must be a literal type, not `string`.
- **Be liberal in what you accept, strict in what you produce.** Input types union the acceptable shapes (`CameraOptions = Partial<Camera>` with widened field types); output types are one canonical shape. A loose return type forces every caller to narrow, propagating optionality through the codebase. The paired-type pattern (`Camera` / `CameraLike`) is the standard expression of this.
- **Push null to the perimeter.** Don't bake `null`/`undefined` into a named type alias (`type User = ... | null` is hostile to readers). Group correlated nullable fields into a single nullable sub-object that's either fully populated or absent, rather than scattering nullable fields whose nullability is implicitly correlated.
- **Avoid optional properties when you can.** They hide "did you forget to set this?" bugs because forgetting is type-safe. N optional properties = 2^N combinations. Pattern: define a loose `InputConfig` (all optional) and a strict internal `Config` (all required), normalize once at the boundary.
- **Prefer precise alternatives to `string`.** String-literal unions for enumerated values, branded types for domain identifiers, `keyof T` for property names. Two adjacent `string` parameters are a silent-swap hazard.
- **Use distinct types for special values, not in-band sentinels.** `indexOf` returning `-1` for "not found" is the anti-pattern: the type system can't force you to check. Wrap to return `number | null` (or a tagged union for multiple failure modes).
- **Avoid repeated positional parameters of the same type.** `(x: number, y: number, w: number, h: number)` invites silent argument swaps. Wrap into a `Point`/`Rect` object, or use brands. Exception: genuinely commutative functions (`max(a, b)`) and parameters with a universally-agreed natural order (`slice(start, end)`).
- **Unify types instead of modeling differences.** If you maintain `Foo` and `FooDb` / `FooRaw` / `FooApi` pairs with mechanical conversions, the cure (type-level case converters, mapper functions) is worse than the disease (accepting snake_case in TS).
- **Prefer imprecise types to inaccurate ones.** A loose type that accepts everything is honest. A precise-looking type that subtly rejects valid inputs trains users to reach for `as any`, undermining trust in every type in the codebase.
- **Don't bake type information into names or doc comments.** `ageNum`, `nameStr`, `// returns a string` all drift. Encode in the type. Exception: units the type can't capture (`timeoutMs`, `priceUsd`) belong in the name, or use a brand.
- **Name types in the domain's language, not implementation's.** `ConservationStatus`, not `endangered`. `Directory`, not `INodeList`. Avoid the vague-noun cluster: `Info`, `Data`, `Thing`, `Item`, `Entity`. Different words must mean different things; renaming for prose-variety is a code smell.
- **Don't hand-write types by inspecting sample payloads.** Generate from OpenAPI/JSON Schema/GraphQL/Zod. Sample-driven types match the cases you happened to observe, not the cases the spec allows.

## Inference and annotation

- **Annotate function signatures, infer locals.** Public function inputs and (for exported functions) return types are contracts and deserve annotation. Local variables almost never do -- annotations on locals just block refactors.
- **Annotate object literals at construction.** This enables excess property checking, pins errors to the definition site rather than the use site, and prevents widening surprises.
- **Use different variables for different types.** A variable's value can change; its type generally shouldn't. `let` reused across semantically distinct concepts forces awkward unions and confuses readers.
- **Create objects all at once.** `const obj = {}; obj.x = ...; obj.y = ...` builds up an empty type and then fights it. One literal (with conditional spreads for optional fields) lets inference produce a complete type.
- **Reach for `satisfies` over `as` or annotation when you want both contract validation AND preserved literal types.** `: T` widens (you lose the literal); `as T` doesn't validate (you lose the contract); `satisfies T` validates the shape and preserves the narrow inferred type. Use it for config tables, route maps, message catalogs, action types, anywhere `keyof typeof X` or `X[K]` will be needed downstream.
- **Reach for `as const` when you want deep readonly literal inference of an entire structure.** Composes well with `typeof X[keyof typeof X]` to derive union types from value tables.
- **Type-only `const` assertions are safe; value-level `as` assertions are not.** `as const` is verified; `as Foo` is a promise the compiler trusts without checking.
- **Use functional array methods to keep types flowing.** `map`/`filter`/`flatMap`/`reduce` propagate types automatically. Hand-rolled loops with mutable accumulators usually require manual annotations and lose type information at boundaries.
- **Prefer async/await over callbacks.** Better composition, types flow through naturally, and `async` enforces that a function is *always* asynchronous -- "sometimes-sync, sometimes-async" callback APIs are a class of timing bug that `async` makes impossible.

## Generics

The mental model: **a generic is a function from types to types.** You "instantiate" it the way you "call" a function.

- **The Golden Rule of Generics: a type parameter must appear at least twice.** If `T` shows up only in the return position, it's not generic -- it's a hidden assertion (`parseYAML<T>(s): T` is functionally identical to `as`). If `T` shows up only in one input, it's decorative. Twice can mean "in a parameter and in the return," "in a constraint and a parameter," or "in two parameters." Once is a smell.
- **Always constrain unconstrained parameters.** Bare `<T>` has domain `unknown`, which gives the caller and the body too much freedom. Prefer `<T extends Foo>`, or take `Foo` directly if `T` doesn't appear twice.
- **Default unconstrained class generics to `never`, not `unknown`.** A `class Collection<T = never>` makes a forgotten annotation an error; `T = unknown` silently accepts anything.
- **Generic inference is all-or-nothing per call.** If callers need to specify some type parameters and let others infer, split the call across a class constructor + method, or curry the function. Don't write generics that force every type argument to be specified by hand.
- **Name generics descriptively, not single-letter.** `Route`, `Param`, `Entry` beat `T`, `U`, `V`. The Matt Pocock convention of `T`-prefix names (`TRoute`, `TParam`) is also reasonable when collision with built-ins matters (`TElement` not `Element`).
- **A generic that returns its constraint type-instantiated leaks information.** Functions returning `T extends Base` must clone-and-return the input (preserving the caller's extra properties) rather than constructing a fresh object literal -- otherwise the "could be instantiated with a different subtype" error fires.
- **`<const T extends ...>` requests `as const` inference for that parameter.** Use it for routers, schema builders, state machine definitions -- anywhere the literal values of arguments must drive subsequent narrowing.

## Conditional and type-level

- **Prefer conditional types over function overloads for input-dependent return types.** Conditionals distribute over unions correctly (`T extends string ? X : Y` resolves member-by-member); overloads are tried independently and fail on union inputs. The implementation typically needs a single internal overload presenting a broader signature, with `as` assertions in the body.
- **Conditional types distribute when the checked type is a bare type parameter.** `T extends U ? A : B` distributes over `T = X | Y`, yielding `(X extends U ? A : B) | (Y extends U ? A : B)`. This is the engine behind `Extract`, `Exclude`, `NonNullable`, and most filter helpers.
- **Disable distribution by wrapping both sides in one-element tuples: `[T] extends [U]`.** Essential when you want intersection semantics over a union input, or when you don't want `T = never` to vanish into `never` (the empty union). Three landmines that this fixes: (1) accumulator-style recursive helpers stopping distribution unexpectedly; (2) `boolean` being `true | false` internally and distributing both ways; (3) `never` collapsing the whole conditional.
- **Use `infer` to extract embedded types from positions in a pattern.** `T extends Promise<infer R> ? R : never` is the foundational pattern. Works inside arrays, tuples, function signatures, mapped types, and template literals.
- **Key-remap inside mapped types via `as` to filter or rename keys.** `[K in keyof T as Predicate<K, T[K]> extends true ? K : never]: ...` is how you build "drop methods," "keep only string-valued fields," "rename to `onXChanged`" helpers.
- **Tail-recursive types beat non-tail-recursive ones by orders of magnitude.** TypeScript performs Tail Call Optimization on recursive type aliases: tail-recursive forms get a far higher instantiation depth limit. Move work into an accumulator parameter (`Acc extends string = ""`) and return `Acc` at the base case, instead of building the result by wrapping the recursive call.
- **`never` is the empty union; it disappears from unions and forbids properties when assigned.** This is why `Extract`/`Exclude` work (filter to `never`, vanish from union), why `assertNever` works for exhaustiveness, and why `propName?: never` enforces "this property must be absent."
- **Test your types.** Conditional types and mapped types contain logic; logic has bugs. Use `expect-type`, `tsd`, or the Type Challenges-style `Expect<Equal<X, Y>>` helper. Critical: test *equality*, not assignability -- `assertType<{name: string}[]>([{name:"a", extra:1}])` passes (assignable) when the actual type is wider, hiding bugs.

## Template literal types

- **Use template literals to model structured string subsets** (DSLs, key patterns like `` `data-${string}` ``, selectors). Combine with `infer Head` / `infer Rest` to parse string types.
- **Walk strings with an accumulator pattern, not nested template-literal recursion.** Non-tail recursion hits the instantiation limit around 50 characters of input; the accumulator form goes far further.
- **Use `Capitalize`/`Uncapitalize`/`Uppercase`/`Lowercase` for case manipulation at the type level.** Note these only exist as type operators; runtime needs a separate function.
- **Stop before crossing into "inaccurate" territory.** A template-literal parser that almost-but-not-quite matches the real grammar (CSS selectors with descendant combinators, SQL with `GROUP BY`) produces silently-wrong types that are worse than honest imprecision. When the type can't model the grammar fully, return `unknown` (or a broader honest type) and validate at runtime.
- **For nontrivial DSL typing, consider codegen.** Type-level SQL/GraphQL parsers exist, but a `pgtyped`-style generator is more debuggable, has better display, and won't tank the compiler.

## Classes

- **Prefer ES `#private` over TS `private`.** TS `private` is compile-time only -- erased at runtime, enumerable, bypassable via `as any`. ES `#private` is a runtime feature with real encapsulation.
- **Enable `noImplicitOverride` and use `override`.** Catches subclass methods that have been silently orphaned when the base class is renamed or its signature changes.
- **For async or multi-step init, use a `static async create()` factory.** Constructors can't be async, so anything that tries forces null-checks across every method.
- **Generic class parameters that touch only one method belong on the method, not the class.** Reduces the surface where callers have to spell out type arguments.
- **`this` types make methods aware of the concrete subclass.** Use them for builder patterns (return `this`, not the class name, to preserve subclass chainability) and for "equals" methods that must reject cross-subclass comparison.
- **Static-only classes and namespaces are anti-patterns.** Use ES modules. Namespaces remain useful only for declaration merging into globals (`declare namespace JSX`).

## Avoid these features

- **Enums** -- prefer string-literal unions, or `const Foo = {...} as const; type Foo = typeof Foo[keyof typeof Foo]`. Numeric enums accept *any* number (because of bitflag use cases). String enums are nominally typed in an otherwise structural type system, forcing consumers to import and reference the enum even when they have the string. Non-`const` enums emit a runtime object that bloats output.
- **Parameter properties** (`constructor(public name: string)`) -- they hide class shape from readers and only TypeScript understands them.
- **Triple-slash directives and `namespace` for module organization** -- use ES modules.
- **`experimentalDecorators`** -- ES decorators (stage 3, TS 5.0+) are the path forward.
- **`Function` type and bare `Object` / `String` / `Number` / `Boolean`** -- use `(...args: any[]) => any` and lowercase primitives.

## Brands and nominal typing

- **TypeScript is structural; use brands when you need nominal semantics.** `type AbsolutePath = string & { readonly __brand: unique symbol }`. Pair with a constructor/validator (`makeAbsolutePath: (s: string) => AbsolutePath | null`) and never use `as AbsolutePath` outside it.
- **Brands work on primitives** where you can't actually attach a property at runtime -- the brand exists purely in the type system. Reach for them for domain identifiers (`UserId`, `OrderId`), units (`Meters`, `Seconds`, `Cents`), filesystem distinctions (`AbsolutePath` vs `RelativePath`), and any value where confusing two of them would be a category error.
- **`unique symbol` brand keys prevent casual forgery.** Importable string keys can be reproduced; a `unique symbol` exported only from your module forces consumers through your constructor.
- **Arithmetic on branded numbers strips the brand.** Accept this (and re-brand at the boundary) or wrap operators (`addCents(a: Cents, b: Cents): Cents`).

## Iteration and indexing

- **`Object.keys(obj)` returns `string[]`, not `keyof T`.** This is sound, not a bug -- the runtime object may carry extra keys. For iteration, either accept `string[]` and check with `in`, write a type guard, or use a generic `<T extends Foo>(t: T)` so `keyof T` is exact.
- **`for...in` keys are typed `string`, not `keyof T`.** Same reason. Cast only when you genuinely own the object.
- **`Object.entries` returns `[string, T[keyof T]][]`.** Honest about the openness; preferred when you actually need values.
- **`array.filter(Boolean)` does not narrow the type.** Type-patch the `filter` overload or write a typed predicate (`(value): value is NonNullable<T> => value != null`).
- **`array.includes(x)` over an `as const` tuple fails when `x` is wider than the tuple's element type.** Either widen the predicate locally or write a generic helper.
- **`noUncheckedIndexedAccess` adds `| undefined` to array/object index access.** It's a small inconvenience that prevents a real category of bug. Turn it on for new projects.

## Type assertions

- **`as` is a claim, not a conversion.** `value as number` does nothing at runtime; use `Number(value)`. `as` only allows up/down the structural lattice -- you can't assert between unrelated types without going through `unknown`.
- **Prefer annotation to assertion.** `const x: Foo = ...` validates; `... as Foo` doesn't.
- **Non-null assertion (`!`) is an assertion.** Use only when you can defend why the value can't be null; otherwise narrow.
- **Hide unsafe assertions inside well-typed helpers.** If `as` is unavoidable (parsing, runtime introspection), give the helper an honest signature (`parseJson(s: string): unknown` not `<T>(s: string): T`) and bury the assertion inside, paired with a comment justifying it and a unit test exercising it.

## Boundaries and validation

- **Type assertions at I/O boundaries should be loud.** Patch `Body.json()` and similar `Promise<any>`-returning APIs to `Promise<unknown>` via declaration merging, so every consumer has to make an explicit `as` or use a runtime validator.
- **For real runtime validation, use Zod (or Valibot/ArkType) and let `z.infer` produce the static type.** Hand-written `typeof` checks duplicate the type definition with no enforcement they stay in sync.
- **Generate types from external contracts** (OpenAPI, GraphQL, JSON Schema). Hand-rolled API types match samples, not specs.
- **`catch` parameters can only be `any` or `unknown`** (JS lets you throw anything). Always type as `unknown` and narrow with `instanceof` / `typeof` / library guards.

## Declaration files and external types

- **Export every type that appears in a public signature.** Hidden internal types force consumers into `Parameters<typeof fn>` / `ReturnType<typeof fn>` gymnastics.
- **Use TSDoc (`/** ... */`) for exported APIs.** Editors surface JSDoc on hover and completion. `@deprecated` is rendered with strikethrough. Don't put types in JSDoc; the TS type already says it.
- **Mirror upstream types to sever dependencies.** Don't drag `@types/node` into your public types because you reference `Buffer` -- define a minimal `StringEncodable` interface that `Buffer` happens to satisfy. Write a test asserting assignability so the mirror can't silently drift.
- **Module augmentation tightens loose built-ins.** `declare module 'foo' { interface X { ... } }` for adding missing fields, narrowing `JSON.parse` to `unknown`, banning `new Set(string)`. Consider `ts-reset` for the common bundle of tightenings.
- **Augment globals with `declare global { var foo: ... | undefined }`.** The `| undefined` forces feature-detection.

## Common review-flag patterns

When reviewing TypeScript, these are the recurring smells:

- `: any` or `as any` without a justifying comment, especially escaping a public signature.
- `// @ts-ignore` (use `// @ts-expect-error` with a reason).
- `as Foo` on an object literal where annotation would work.
- A function returning `T | Promise<T>` (use `async` to force one shape).
- Multiple correlated nullable fields where the correlation isn't expressed in the type.
- A boolean flag plus optional payload fields (probably wants to be a tagged union).
- Optional properties added "to avoid breaking changes" -- usually creates a `??` default scattered across call sites.
- `string` parameters that should be string-literal unions or branded.
- Adjacent same-typed positional parameters of arity ≥ 2 (silent-swap hazard).
- `<T>(...): T` generic signatures where `T` appears only in the return type.
- `as keyof T` after `Object.keys` on a value whose source isn't fully controlled.
- Snapshot tests on type-level helpers (use `Expect<Equal<...>>` instead).
- `parseJSON`-style functions returning `any`.
- Hand-rolled types mirroring an API/wire format that has a published schema.
- Type complexity exceeding the implementation complexity it describes -- consider codegen or a simpler annotation.
- `enum` declarations (prefer string-literal union or `as const` object).
- TS `private` rather than ES `#private` in new code.
- `let` reused for semantically different concepts.
- `for...in` with a `keyof T` cast on an externally-sourced object.
- Recursive types built with non-tail recursion (visible when the input length matters).
- A complex conditional return type that's actually two or three concrete call shapes (use overloads).
- Two overloads that differ only by union expansion (use a conditional return type).

## Decision frameworks worth memorizing

**Overloads vs conditional types** (Cookbook 12.7, the cleanest framework):
- Use **conditional types** when the variants form a regular pattern (variadic `concat`, mapping types through a transformation) or when many input types map to many output types in a structured way.
- Use **overloads** when variants have *different argument shapes* (different arity, optional callback that changes return type, exact-argument coupling between two parameters), or when readability matters more than cleverness.
- **Combine**: overloads on the public surface, conditional return type used internally.
- The deep reason: conditional types over function parameter unions hit the "lowest common denominator intersection" problem (TS uses the intersection of property types as the assignable type), forcing `as any` in the body.

**`satisfies` vs `: T` vs `as const` vs no annotation**:
- `: T` -- you want the value to *be* T downstream (polymorphism). Widens literals.
- `satisfies T` -- you want T's contract validated AND the narrow inferred type preserved (config tables, action maps, route definitions).
- `as const` -- you want deep readonly literal inference of an entire structure with no contract check.
- No annotation -- type is purely local and inference is fine.

**`interface` vs `type` for object shapes**:
- `interface` -- the shape is meant to be extended by consumers (public surface, declaration merging desired).
- `type` -- everything else, especially anything `interface` can't express (unions, tuples, conditional/mapped/template-literal computations) and anything inside your module boundary where you want duplicate-name errors instead of silent merging.
- Be especially careful naming `interface`s that match built-ins (`FormData`, `Window`, `Event`); silent merging with `lib.d.ts` is a real and confusing footgun.

## Touchstones

When in doubt, the two canonical references are:

- *Effective TypeScript*, 2nd ed. (Vanderkam, 2024) -- 83 numbered items, especially chapters 4 (Type Design), 5 (Unsoundness and `any`), and 6 (Generics and Type-Level Programming). The Item titles themselves are a usable mental index.
- *TypeScript Cookbook* (Baumgartner, 2023) -- problem/solution/discussion shaped. Especially chapter 5 (Conditional Types), chapter 6 (Template Literals), chapter 8 (Helper Types), and chapter 12 (Type Development Strategies -- contains the overloads-vs-conditionals framework and the `satisfies` decision).
