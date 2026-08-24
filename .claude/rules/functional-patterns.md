---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Functional Programming Patterns

A patterns-and-decisions companion to the FP principles file. Each entry names a move you can invoke ("the Reader pattern", "the smart-constructor pattern") and tells you when to reach for it and when it would be overkill. Cross-language analogues are called out because the patterns are not confined to Haskell or Scala -- once you see them, you start spotting them in Rust, TypeScript, Kotlin, Python, and C#.

The unifying claim: most FP "patterns" are the same idea applied to different containers. Recognizing the shape is the win; implementing the heavy machinery is optional.

---

## Functor: containers that respect `map`

A type `F<A>` with `map : (A -> B) -> F<A> -> F<B>` preserving identity and composition. Lift a function into a context without disturbing the context.

**Reach for it** when you have a value-in-context (`Option`, `Result`, `List`, `Promise`, `Future`, `IO`, parser, stream) and want to transform the inner value. The vocabulary -- `map`, `fmap`, `.map`, `Select`, `Array.prototype.map` -- works across all of them. If you find yourself writing `if x is not None: return f(x) else: return None`, you want `map`.

**Skip it** when there is no container. `map` over a bare value is just function application.

**Cross-language**: Rust `Option::map`/`Iterator::map`, fp-ts `O.map`/`E.map`, Java `Optional.map`, C# LINQ `Select`, Python `returns.Maybe.map`. The per-container plumbing falls away once you see the shape.

---

## Applicative: independent combination inside a container

A functor plus `pure : A -> F<A>` and `map2 : (A, B) -> C -> F<A> -> F<B> -> F<C>`. Combine multiple independent containers without one depending on the other.

**Reach for it** when you want to *accumulate* errors rather than short-circuit -- `(validateName, validateEmail, validateAge).mapN(User.apply)` collects every failure where the monadic version stops at the first. Also for parallel `Future`/`IO`: applicative says "no dependency between the parts," which enables parallelism.

**Skip it** when the next step depends on the previous value. That's a monad; faking it with applicative gets ugly.

**Cross-language**: Cats `Validated`, fp-ts `Validation`, Haskell `liftA2`, Rust `try_join!` for async, Effect-TS `Effect.all`. The "form validation with all errors" pattern lives here under the name "applicative validation."

---

## Monad: sequencing where the next step depends on the previous

A functor plus `pure` and `flatMap : (A -> F<B>) -> F<A> -> F<B>` (a.k.a. `bind`, `>>=`, `chain`, `andThen`). Each step picks its successor from the current value; the container handles failure short-circuit, state threading, async, nondeterminism.

**Reach for it** for pipelines where each step can fail and later steps depend on earlier ones: `parseInt(s).flatMap(validate).flatMap(lookup).flatMap(authorize)`. `Option` short-circuits on `None`, `Result` on `Err`, `IO` threads the effect.

**Skip it** when steps are independent (applicative gives parallelism / error accumulation) or there's only one step.

**The "do notation" correspondence**: Haskell `do`, Scala `for { ... } yield`, F# computation expressions, Rust `?`, TypeScript `async/await`, OCaml `let*` are all monadic syntax sugar. Understand one and you understand all of them. `async/await` is the monad-comprehension that escaped the FP ghetto.

**The hierarchy**: every monad is an applicative is a functor. Reach for the *weakest* abstraction that does the job: `map` over `flatMap` over manual `match`.

---

## Reader: dependency injection without argument threading

`Reader<R, A> = R -> A`. A computation that "reads" context `R` and produces `A`. Compose with `flatMap`; `run(env)` once at the edge.

**Reach for it** when a deep call tree needs the same config / database handle / logger and you don't want every signature to grow an `env` parameter. `ReaderT AppEnv IO` is the most common Haskell production stack precisely because it solves this.

**Skip it** when one or two functions need context; just pass it. The implicit threading must save more than the type bookkeeping costs.

**Cross-language**: Scala 3's `given`/`using` is `Reader` made implicit. Rust uses `&self` on a service struct. React Context, Spring's DI, anywhere you'd say "dependency injection" -- it's the same pattern.

---

## Writer: pure logging / accumulation

`Writer<W, A> = (W, A)` where `W` is a monoid. Each step produces a value and an accumulated log; `flatMap` combines logs via the monoid.

**Reach for it** for pure computation that emits a structured trace: compiler passes accumulating warnings, a state machine emitting events, an audit log built during a calculation. The monoid choice matters: list (slow), `DList`/`Chain` (fast append), `Set` (deduplicating), `Sum` (numeric).

**Skip it** when you want real logging (use a logger) or to report errors (use `Result`/`Validated`). Also skip strict `Writer` for long-running computation -- it leaks space.

**Cross-language**: rare as a named pattern outside Haskell/Scala. The mainstream equivalent is "return a value and a list of events," which is exactly `Writer`, unnamed.

---

## State: pure stateful computation

`State<S, A> = S -> (S, A)`. Thread mutable-looking state through a pure pipeline. `get`, `put`, `modify` are primitives; `flatMap` threads the state.

**Reach for it** for simulations, interpreters, random-number-generator chains, parsers tracking position. Pure state is a value: you can fork it, print it, diff it, replay it.

**Skip it** when one mutable cell would do (a `Ref`), or when the state is genuinely external (a database) -- that's `IO`, not `State`. Nested state across structures is usually lens + `State`, not `State` alone.

**Cross-language**: React `useState`/`useReducer` is `State`-flavored. Redux is `State` made global. Rust's `iter::scan` is a constrained version.

---

## Monad transformers (mtl-style stacks)

Stack monads to combine capabilities: `ReaderT Config (StateT AppState (ExceptT AppError IO))`. The `mtl` typeclasses (`MonadReader`, `MonadState`, etc.) let you write code generic over any stack including that capability.

**Reach for it** when you need exactly the capabilities (read config, mutate state, fail with typed errors, do I/O) and the stack is shallow. This is the workhorse for medium Haskell production.

**Skip it** past three layers: incomprehensible type errors, `O(n^2)` instance problem, every new effect requires another `MonadX` instance. Switch to tagless final or algebraic effects.

**The pragmatic rule**: one or two transformers -- transformers. Three or more -- tagless final or `polysemy`/`effectful`/`eff`. Not really a thing outside Haskell; Scala mostly uses ZIO or Cats Effect directly, TypeScript uses Effect-TS.

---

## Free monad: computation as data, interpreted later

Define an algebra (`data InteractionF a = GetInput (String -> a) | PutOutput String a`), wrap in `Free`, and programs become *data*. Write multiple interpreters: prod, test, debug.

**Reach for it** for DSLs needing multiple interpretations: a test interpreter that records calls, a prod interpreter that hits the network, a debug interpreter that pretty-prints. Useful for property-testing interpreters against each other.

**Skip it** for a single interpreter or three sequential I/O calls. Boxing every step into a tree to fold is not free.

**Freer monad variant**: Kiselyov's `Freer`/`Eff` drops the `Functor` constraint by storing a continuation. Slightly faster, more flexible, the basis of `freer-simple`, `polysemy`, `eff`.

**Cross-language**: the "Command" and "Interpreter" patterns from OO design are degenerate free-monad-ish moves. Redux actions and Elm `Cmd` are free-monad-shaped.

---

## Tagless final: abstract over the type constructor, not the AST

Instead of an ADT for your DSL, write a typeclass: `class Monad m => Console m where { getLine :: m String; putLine :: String -> m () }`. Programs are polymorphic in `m`: instantiate with `IO` for prod, `State` for tests.

**Reach for it** for the multi-interpreter benefit of free monads without the runtime overhead. Composing algebras is just adding constraints (`(Console m, Database m) => m a`), no coproduct gymnastics. Used heavily in production Haskell at scale because the performance is "just" running the underlying monad.

**Skip it** for single-implementation code -- the abstraction tax is still real. Also tricky for higher-order operations like `local` or `bracket`; you need `MonadUnliftIO` or similar.

**Cross-language**: Scala traits with `F[_]` parameters (`def program[F[_]: Console: Database]: F[Unit]`). Kotlin/Arrow analog. Rust can't do it -- no higher-kinded types. TypeScript via fp-ts' HKT encoding.

---

## Algebraic effects: the modern alternative to monad stacks

Effects (`Reader<Config>`, `Throws<DbError>`, `State<Counter>`) tracked in the type but handled by explicit handlers, not a fixed transformer stack. Resumable continuations let handlers do things transformers can't.

**Reach for it** in Effect-TS (TypeScript), ZIO (Scala), OCaml 5's effect system, or Koka. Effect-TS in particular has become the default "FP in TypeScript" choice for new code -- it absorbs the use cases of `Reader`, `State`, `Either`, `Future`, retries, scheduling, and structured concurrency into one library.

**Skip it** if you're in Haskell with a small mtl stack that works, or in a language without good effect-system tooling. Rolling your own is research-grade.

---

## Lenses and the optics hierarchy

A `Lens<S, A>` is a `(get : S -> A, set : (A, S) -> S)` pair satisfying laws (get-set, set-get, set-set). Compose lenses to dive into nested structures. `Prism` (sum-variant focus), `Traversal` (zero-or-more), `Iso`, `Getter`, `Setter`, `Fold` form a hierarchy where stronger optics compose with weaker ones.

**Reach for it** for deeply nested immutable updates. `user.address.city.zipcode = "94110"` becomes `set (address . city . zipcode) "94110" user`; without lenses, four levels of record-copy. Also when the same nested field is updated from many call sites.

**Skip it** for one-level updates (`{ ...user, name: newName }` is fine) and performance-critical paths. Violating put-put means it's not a lens; it's a setter, and it won't compose.

**Cross-language**: Haskell `lens` (Kmett), Scala `monocle`, TypeScript `monocle-ts`/`optics-ts`, Ramda for JS. Immer is "lenses for the imperative crowd." In Rust, usually you write the update directly.

---

## Smart constructors

Make the constructor private; expose `mkEmail : String -> Either Error Email` that validates. The type then *means* "validated email," not just "a string." Downstream code stops re-validating.

**Reach for it** for any value with invariants: email, non-empty string, sorted list, percentage in `[0, 100]`, hex color. "Parse, don't validate" lives here.

**Skip it** when the invariant is cheap enough to recheck or when a refinement type does it stronger.

**Cross-language**: Rust newtype with private inner field and `try_new`. TypeScript branded types: `type Email = string & { __brand: 'Email' }` with `makeEmail(s): Email | null`. Java/Kotlin sealed classes with factory methods. The pattern is universal once any nominal typing exists.

---

## Phantom types

A type parameter that doesn't appear in any constructor: `data Connection state = Connection Handle` where `state` is `Open` or `Closed`. Functions like `open : Connection Closed -> IO (Connection Open)` encode state-machine transitions at the type level.

**Reach for it** to make the compiler reject "use of a closed connection," "double-spend," "unauthenticated request," "uncalibrated sensor." Anywhere a state machine has a small number of states and wrong transitions should be unrepresentable.

**Skip it** for two states with one transition -- the encoding costs more than the bug it prevents. Also skip when states form a graph; type-level state machines get unwieldy past a handful of transitions.

**Cross-language**: Rust's typestate pattern is the gold standard (`Builder<NeedsName>`, `Builder<Complete>`). TypeScript with generic parameters and brands. Scala 3's match types. Java/Kotlin via sealed interfaces, weaker.

---

## Refinement types

A type refined by a predicate: `type Nat = { x : Int | x >= 0 }`. The compiler / SMT-solver checks at compile time that every `Nat`-producing expression satisfies the predicate.

**Reach for it** in LiquidHaskell, F\*, Dafny, or Scala 3 with creative use of match types. Stronger than smart constructors (predicate is part of the type, not a runtime check), weaker than full dependent types (predicate must be SMT-decidable).

**Skip it** in languages without native support. Bolting refinement types onto mainstream Haskell/Scala/TypeScript is research-grade. Smart constructors are the pragmatic alternative.

---

## Generalized Algebraic Data Types (GADTs)

Constructors that refine the type parameter: `data Expr a where { IntLit :: Int -> Expr Int; BoolLit :: Bool -> Expr Bool; Add :: Expr Int -> Expr Int -> Expr Int }`. The interpreter `eval : Expr a -> a` is type-safe because matching `IntLit` refines `a` to `Int`.

**Reach for it** for typed embedded DSLs, well-typed ASTs, interpreters, state machines too rich for phantom types. Anywhere the constructor you matched tells the compiler something about the type parameter.

**Skip it** when a regular ADT works. GADTs hurt inference and the type errors are worse.

**Cross-language**: Haskell (with `GADTs`), OCaml (native), Scala 3 (match types + singletons), TypeScript (discriminated unions + conditional types -- approximates them), Rust (limited, via enum + `PhantomData` + traits).

---

## Recursion schemes

Separate the *shape* of structural recursion from the *operation*. A catamorphism (`cata`) is "fold," anamorphism (`ana`) is "unfold," hylomorphism is "unfold then fold without the intermediate tree." Factor out the recursion plumbing; write only the per-node logic.

**Reach for it** when you have a complex recursive structure (typed AST, JSON tree, deeply nested config) and keep writing the same recursive plumbing for different operations. `para` (cata with original subtree), `zygo` (two folds fused), `histo` (course-of-values) round out the toolkit.

**Skip it** for a 10-line tree walk; just write the recursion. Recursion schemes are gorgeous when needed and pure overhead when not.

**Worth knowing**: "Bananas, Lenses, Envelopes and Barbed Wire" is the foundational paper; the Haskell `recursion-schemes` library is the reference. Once you've seen `cata`, every fold in every language looks like one.

**Cross-language**: rare as an explicit pattern. Scala's `droste`. In most other languages, people write the recursion or use a visitor.

---

## Parser combinators

Small parsers (`char`, `digit`, `string`) composed via `<|>` (alternative), `*>`/`<*` (sequencing), `many`, `sepBy`. The parser is code, not a grammar specification.

**Reach for it** for any custom syntax -- config formats, query DSLs, log parsing, expression languages. As fast as a hand-rolled recursive-descent parser with much less code, fully refactorable and testable.

**Skip it** when a regex suffices or you need a bulletproof LALR/PEG generator (ANTLR, tree-sitter) for a major language.

**Applicative vs monadic split**: applicative parsers (`Foo <$> a <*> b <*> c`) cannot dispatch on values during parsing -- the grammar is static, which enables Earley/GLR and better error messages. Monadic gives flexibility. Reach for applicative when the grammar is static.

**Cross-language**: Haskell (megaparsec, attoparsec), F# (fparsec), Rust (nom, chumsky, winnow), TypeScript (parsimmon, ts-parsec), Scala (cats-parse), OCaml (angstrom).

---

## Continuation-Passing Style and defunctionalization

Instead of `f : A -> B`, write `f_cps : A -> (B -> R) -> R`. Every function takes "what to do next." Defunctionalization replaces continuation closures with a first-order data type plus an `apply` function.

**Reach for it** for compilers (CPS is a canonical intermediate representation), async runtimes (every `.then` is a continuation), stackless interpreters, escaping recursion-depth limits via trampolining.

**Skip it** in application code -- CPS for a regular pipeline is unreadable. Use the higher-level abstraction (monad, async/await, generators).

**The async/await connection**: `async/await` is automatic CPS transformation by the compiler. Every `await` splits the function and registers the continuation. Filinski's "Representing Monads" shows every monad can be encoded via continuations -- one of the deeper results in PL.

---

## Property-based testing

Instead of `assertEqual(f(2), 4)`, describe properties (`forall xs: reverse(reverse(xs)) == xs`) and let the framework generate random inputs (including edge cases) and *shrink* failures to minimal counterexamples.

**Reach for it** for pure functions with an obvious property: round-trip (`parse . print == id`), idempotence (`sort . sort == sort`), invariant preservation, algebraic laws (associativity, commutativity, identity). Falsification beats verification: one counter-example trumps 1000 passing examples.

**Skip it** for effectful code where generation is hard, or when you can't articulate a property.

**Cross-language**: Haskell QuickCheck, Python Hypothesis, Rust proptest, TypeScript fast-check, Scala ScalaCheck, F# FsCheck, Java jqwik. Hypothesis has industrial-strength shrinking and is approachable from non-FP codebases.

---

## Make illegal states unrepresentable

Yaron Minsky's slogan. Use sum types (not subclasses), smart constructors, phantom types, and refinement types to push invariants into the type system so the compiler rejects wrong states.

**Reach for it** during domain modeling. If two booleans give you four states but only three are valid, replace with a three-variant sum. If a `User` is either authenticated-with-token or anonymous, model them as variants, not as `User { token: Option<Token>, anonymous: bool }` where `(Some, true)` is nonsense.

**Skip it** never -- this is the through-line. The cost is occasional ceremony; the benefit is whole categories of bugs that cannot occur.

**Cross-language**: OCaml variants, Rust enums, TypeScript discriminated unions, Scala 3 enums, Kotlin sealed classes, Java sealed interfaces (17+), Python `match` + dataclasses. Same pattern: model the *valid* states, disallow encoding invalid combinations.

---

## Railway-Oriented Programming

Wlaschin's metaphor. A two-track railway: success on top, failure on bottom. Operations are switches: `validate : Input -> Result<Output, Error>` routes to one track or the other. Compose with `bind`/`flatMap`; failure short-circuits past every downstream success operation.

**Reach for it** for pipeline-shaped business logic with multiple validation/operation steps. The "errors bypass the success rail" mental model is more accessible than "monad" for non-FP audiences. F#'s `Result` + `|>` makes it concrete.

**Skip it** when errors must be accumulated (use applicative validation) or when each step should recover locally (handle there, don't push to the bottom track).

**Cross-language**: Rust's `?` is railway syntax. F# computation expressions. The `returns` library in Python. fp-ts `Either` pipelines. Even Go's `if err != nil` is railway-oriented programming, just verbose -- the two-track shape is identical.

---

## Pipelines over method chains

`data |> step1 |> step2 |> step3` instead of `obj.step1().step2().step3()`. Functions in the open, not bound to a class.

**Reach for it** for data transformation. The pipeline keeps data prominent and steps trivial. Method chaining requires every step to be a method on the data, coupling shape to operations.

**Skip it** when the host language lacks the operator and `pipe(data, step1, step2)` is awkward (JavaScript and Python users live this).

**Cross-language**: F# `|>`, Elixir `|>`, OCaml `|>`, Elm `|>`, R `|>`. Rust iterator chains. fp-ts' `pipe`. JavaScript TC39 pipeline proposal. The pattern is universal; the syntax varies.

---

## Builder via partial application

Replace a `Builder` class with a chain of partially-applied functions. Each step is `User -> User`; the pipeline composes them. Combined with phantom types you get compile-time-checked required fields (Rust typestate builders are the gold standard: `Builder<Missing>` won't compile to `build()`).

**Reach for it** for many-optional-fields constructors. Shorter than a builder class, type-safe when combined with typestate.

**Skip it** for two or three fields; a regular constructor or factory is fine.

**Cross-language**: Rust typestate, Kotlin named arguments + defaults often replace builders entirely, TypeScript with discriminated unions and brands.

---

## Type-driven development

Sketch the types before the code. `parseRequest : RawInput -> Result<ParseError, Request>`, `authorize : User -> Request -> Result<AuthError, AuthorizedRequest>`. Let types dictate shape, then fill in bodies. "If it compiles, it probably works" becomes plausible once the types are tight enough.

**Reach for it** for new business logic with several variants. Choosing the types often reveals modeling errors before any code is written -- sum types catch "I forgot the cancelled-but-already-paid case" at design time.

**Skip it** for glue code, scripts, exploratory work. Type-first there is premature.

**Cross-language**: strongest in Haskell/OCaml/F#/Scala/Rust. Weaker but real in TypeScript, Kotlin, Swift, C#. Effectively absent in dynamic languages -- though Python type hints + mypy get you partway.

---

## Cross-language note: FP features have been absorbed everywhere

For two decades the mainstream has been absorbing FP. Today:

- **Rust**: ADTs (enums), pattern matching, traits-as-typeclasses, `Option`/`Result`, iterators, `?` (railway). Missing: HKT, effect tracking.
- **TypeScript**: discriminated unions, structural types, brands, ts-pattern, fp-ts, Effect-TS. Missing: native HKT, ecosystem encodes it.
- **Kotlin**: sealed classes, data classes, `when`, Arrow for full FP.
- **Java**: records (16+), sealed interfaces (17+), pattern matching (21+), `Optional`/`Stream`. Catching up fast.
- **C#**: records, pattern matching, LINQ, expression trees, discriminated unions proposed.
- **Python**: `dataclasses`, `Optional`, `match` (3.10+), `returns`, mypy.

Use the FP move in whatever language you're in, even if transcribed. Recognizing the shape is the win.

---

## When to skip the heavy machinery

The temptation is real once you know the patterns:

- **Free monads for a 3-step computation** -- just write the three steps.
- **Lenses for a one-level update** -- just construct the new record.
- **Tagless final for a single-implementation codebase** -- just call the function.
- **Recursion schemes for a 10-line tree walk** -- just write the recursion.
- **Monad transformers for one effect** -- just use that effect.
- **CPS for a regular pipeline** -- just write the pipeline.
- **Property tests for a constant** -- just assert the constant.

The principle: reach for the heavy machinery when the equivalent direct code would be repetitive, error-prone, or hard to test. The patterns earn their cost at scale and lose at small scale. Let them lose there.
