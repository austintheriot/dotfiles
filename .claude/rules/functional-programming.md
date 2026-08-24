---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Functional Programming Principles

Reference rules for functional programming (FP). Intermediate-to-advanced: assume Haskell/OCaml/F#/Clojure/Rust syntax is readable and the reader is choosing what to apply rather than learning what FP is. School-of-thought disagreements are preserved, not collapsed -- pick per codebase.

---

## The core thesis: programs as compositions of pure functions over algebraic data types

**Why it matters.** A pure function's output depends only on its inputs and produces no observable effects beyond returning a value. Combine that with algebraic data types (ADTs) and you get **equational reasoning**: anywhere `f(x)` appears, you can replace it with its value, or with `let y = f(x) in y`, without changing the meaning of the program. This single property is what makes pure FP code refactorable, testable, parallelizable, and amenable to optimization (the compiler can move `f(x)` around freely). Everything else FP gives you is downstream of equational reasoning.

**Practical application.** When reviewing code, flag any function whose return value is not determined by its arguments: silent reads from globals, time, env, random, files, network. Either lift the dependency into a parameter (now it's pure again), or push the effect to the edge of the program. The middle of your codebase should be a forest of pure functions; effects belong at the boundary.

**Where the schools dissent.** Haskell types every effect (`IO`, `ST`, `STM`, custom monad stacks); ML/OCaml/F# lets you mutate freely but expects you to *know* when you're doing it; Clojure uses immutable values pervasively but accepts side effects as a fact of running on the JVM; Rust gives you mutation with borrow-checked aliasing rules. The Hickey position is that purity-as-discipline beats purity-as-type-system for most programmer productivity; the Wadler position is that effects you can't see are effects you can't reason about. Both are right in their domain.

---

## Immutability and persistent data structures

**Why it matters.** "Mutable by default" means that any reference into a data structure is a potential time bomb -- another thread, another stack frame, or a future version of yourself can mutate it. Immutability collapses an entire class of bugs (aliasing, observation order, ABA, torn reads) into impossible states. Persistent data structures -- HAMTs (hash-array mapped tries), finger trees, RRB-trees, Bagwell tries -- give you immutable values with *structural sharing*, so an "update" is `O(log n)` and reuses most of the old structure. You get value semantics at near-reference cost.

**The cost story.** Persistent structures are not free, but they are often competitive. For workloads dominated by reads, a HAMT is within a small constant factor of a hash map and beats it on shared/cloned data. For workloads dominated by tight-loop mutation (numeric kernels, hot inner loops, allocators), mutation wins by 10-100x and you should use it locally. The fix is *interior mutation with exterior immutability*: build the value in a mutable buffer, then freeze it before handing it out. Clojure's transients, Haskell's `ST`, Rust's `Vec` then borrow, OCaml's `Buffer`/`Bytes` all encode this pattern.

**Hickey's "Value of Values" argument.** Data should be a *value*, not an *identity*. A value is a number, a string, a map -- it doesn't change because change isn't a property of values; `5` doesn't become `6`. An identity is a thing that has different values over time (a bank account, a user record). Conflating the two ("the user *is* the row in the database") gives you the OO mess where every object is its own little distributed system with state. Separate them: identities are references that point to successive immutable values, and the only mutation in your system is the atomic swap of "what value does this identity currently denote."

**Review flag.** A function that takes a mutable container and modifies it in place is suspicious unless it's labelled as such by name and type (`sort_in_place`, `&mut Vec<T>`, `StringBuilder`). Default to returning new values.

**Where the schools dissent.** Game-engine and HPC people will note that persistent data structures lose to arenas and SoA layouts for cache-bound work, and they're right. The answer isn't "never use them" -- it's "use them at the level where the program is reasoning about state, and drop to mutation in the inner loop."

---

## Algebraic Data Types: products, sums, recursion

**The algebra.** Types form a semiring: product types (`(a, b)`, records) are conjunction (`a` AND `b`); sum types (variants, tagged unions, discriminated unions) are disjunction (`a` OR `b`); the unit type `1` is the identity for product; the empty type `0` (`Void`, `!`, `Never`) is the identity for sum. Recursive types are fixed points: `List<a> = 1 + a * List<a>` reads "a list is either empty (`1`) or a head paired with a tail." Once you see types this way, ADT-driven design stops feeling like a language feature and starts feeling like arithmetic on the shape of your domain.

**Why ADTs beat class hierarchies for data.** A class hierarchy encodes "behaviour-with-subtyping": `Animal` and `Dog extends Animal` is good when you genuinely want open extension and dynamic dispatch (plugins, UI widgets, strategy patterns). But most of what people use class hierarchies for is *data with cases*: an `Order` is `Cart` or `Submitted` or `Paid` or `Shipped` or `Cancelled`. Modelling that as `abstract class Order` with five subclasses scatters the case logic across files and offers no exhaustiveness check. Modelling it as a closed sum (`type Order = Cart | Submitted | ...`) gives you exhaustiveness, pattern matching, and the cases visible in one place.

**Wlaschin's "make illegal states unrepresentable."** An `Order` with five nullable fields encodes 2^5 = 32 states, of which maybe 4 are valid; the other 28 are bugs waiting to be hit. Refactor to a sum where each variant carries only the fields valid in that state. `Cart { items: [Item] }`, `Submitted { items: [Item], submitted_at: Instant }`, `Paid { items: [Item], submitted_at: Instant, payment: Payment }`. Now the type rejects the illegal states at compile time.

**Practical application.** When you find yourself writing `if order.status == "submitted" && order.payment != null && order.shipped_at == null`, you've already lost. The type system was supposed to tell you which fields are valid together. Rebuild as a sum.

**Where the schools dissent.** OO advocates (Carmack, Armstrong-on-Erlang-not-Haskell, anyone shipping Smalltalk) will point out that genuinely-subtyped behaviour does exist: a renderable scene graph, a UI component tree, a plugin system where extension is open. Sums are the wrong tool there; you want subtype polymorphism. Use both. The decision is "open vs closed extension": closed = sum, open = subtype/trait/interface. The mistake is using subtype polymorphism for *data*.

---

## Pattern matching and exhaustiveness

**Why it matters.** A closed sum plus a `match` expression with compiler-enforced exhaustiveness is one of the highest-leverage practical features in any language. Add a new variant, the compiler tells you every site that needs updating. No grep, no missed case, no production-only bug. This is the single feature that makes ADT-driven design ship.

**Practical application.** Nested patterns and guards let you express domain logic declaratively: `match (user, request) with | (Banned _, _) -> Reject "banned" | (_, Anonymous) when requires_auth -> Reject "auth" | (Active u, Authed r) -> process(u, r)`. The structure of the data drives the structure of the code; you don't need an `if`-pyramid to discriminate.

**Open vs closed sums.** Closed sums (Rust `enum`, ML/OCaml `type`, Haskell `data`, F# `type`, sealed Kotlin/Scala hierarchies, TypeScript discriminated unions) enable exhaustiveness. Open sums (subclasses across modules, OCaml polymorphic variants without a fixed signature, untyped tagged values) don't. Choose closed unless you have a real extensibility requirement; open extension and exhaustive case analysis are in tension.

**The expression problem.** Open extension on data + open extension on operations is hard. Class hierarchies give you the first (subclass to add data) but make the second painful (new method = touch every subclass). ADTs give you the second (new function pattern-matches on cases) but make the first painful (new variant = touch every function). Type classes, traits, multimethods, visitor patterns, and tagless-final encodings are all partial solutions. There is no free lunch; pick the axis you expect to extend along.

**Review flag.** Pattern matches with a wildcard `_` arm that swallows new cases. The wildcard defeats exhaustiveness; the compiler can no longer flag a forgotten case. Use it only when you genuinely want a fallthrough (e.g., `_ -> error "unreachable"` after handling everything), and prefer naming the remaining cases explicitly.

---

## Parametricity and "free theorems"

**Why it matters.** A function with type `forall a. List<a> -> List<a>` cannot inspect the elements -- it has no way to, because it doesn't know what they are. Therefore the only thing it can do is permute, drop, or duplicate elements of its input. From the type alone you know: it cannot invent elements, it cannot compare them, it cannot depend on their values. Wadler's "Theorems for Free" formalises this -- for any parametrically-polymorphic function, the type yields a free theorem about its behaviour, *without looking at the implementation*. `forall a. a -> a` has exactly one implementation (the identity); `forall a, b. (a, b) -> a` has exactly one (`fst`); `forall a. List<a> -> Int` can only compute things derivable from the spine (length, depth).

**Why this matters practically.** Types become enforceable documentation. A reviewer reading `forall a. (a -> Bool) -> List<a> -> List<a>` knows immediately it's a filter -- the only thing it can do is select a sub-list according to the predicate. No need to read the body. This is the deep reason why Haskell signatures feel "tight": the type rules out almost every implementation.

**The escape hatches break the theorems.** Anything that lets you observe a generic value's representation -- Java's `Object.equals`, `instanceof`, reflection, `Any` casts, ad-hoc overloading on type, even `unsafePerformIO` -- breaks parametricity. Now `forall a. List<a> -> List<a>` could be `\xs -> if a is Int then map(+1, xs) else xs`. The free theorem is gone. Languages that *honour* parametricity (Haskell at the source level, with reservations; Idris; PureScript) pay you back with stronger reasoning.

**Practical application.** Prefer the most polymorphic signature your function admits. If your function only uses `length` and indexing, take a `Sequence`, not a `List<T>` constrained to `T = User`. The reader and the type system both win.

**Where the schools dissent.** Type classes / traits / overloading reintroduce a controlled form of inspection-by-type, which trades parametricity for ergonomics. This is fine -- `Ord a => List a -> List a` is more useful than `forall a. List a -> List a` -- but understand the trade you're making. Rust's `T: Trait` bounds, Haskell's `Eq a =>` constraints, OCaml's modular implicits all weaken parametricity in exchange for power.

---

## Higher-order functions and composition

**The vocabulary.** `map`, `filter`, `fold`/`reduce`, `flatMap`/`bind`, `zip`, `unfold`, `scan`. Most data-shaped problems decompose into these. A `fold` is the most general -- everything iterative on a list is a fold -- but using a more specific combinator at the call site communicates intent.

**Function composition.** `(g . f)(x) = g(f(x))`. The pipeline / fluent / `|>` operator (F#, OCaml, Elixir, Elm, Hack, soon JS) is composition spelled the other direction: `x |> f |> g`. Mathematically equivalent; ergonomically very different. The pipeline reads left-to-right, matches the order of intuition, and is what most working FP looks like.

**Currying and partial application.** A curried function `a -> b -> c` is a function that takes an `a` and returns a function `b -> c`. Partial application -- supplying some but not all arguments -- gives you specialized functions for free: `add(5)` is "add 5 to anything." Haskell and OCaml curry by default; F# does too; TypeScript and Rust don't, which is why you see `(x) => add(5, x)` instead.

**Point-free style.** Writing functions as compositions of other functions without naming the argument: `sumOfSquares = sum . map(square)` rather than `sumOfSquares(xs) = sum(map(square, xs))`. Powerful, concise, sometimes opaque. Use it when the composition is the point; avoid it when the argument's identity carries meaning. Tacit Haskell can become unreadable; APL/J style is its logical endpoint.

**Review flag.** A loop that builds a list by `push`/`append` in a mutable accumulator is almost always a `map`, `filter`, `fold`, or `flatMap` in disguise. Prefer the combinator -- it documents the *shape* of the computation. Exception: if the loop has early exit, complex inter-iteration state, or is a hot path where the combinator allocates intermediate structures, the explicit loop may be clearer or faster.

---

## Recursion and recursion schemes

**Why recursion.** In a pure language without mutation, iteration *is* recursion -- there is no other way to compute a fold. Tail recursion with an accumulator is the iterative style: `loop(state) -> next_state -> loop(next_state)`. Languages with guaranteed tail-call elimination (Scheme, ML, Haskell, F# in tail position) turn this into a goto with no stack growth; languages without it (Python, Java pre-21) need explicit loops or trampolining.

**Recursion schemes.** Generalisations of recursion patterns. A **catamorphism** is a fold (consume a structure to a value). An **anamorphism** is an unfold (build a structure from a seed). A **hylomorphism** is unfold-then-fold without materializing the intermediate (the optimization fusion compilers do). The "Bananas, Lenses, Envelopes and Barbed Wire" paper is the canonical reference; the practical takeaway is that *every recursion you write fits into one of a small number of schemes*, and naming the scheme clarifies the structure. In Haskell the `recursion-schemes` library makes them explicit; in any language, mentally tagging "this is a catamorphism over the tree" beats writing it as ad-hoc mutual recursion.

**Practical application.** When you have mutual recursion over a tree-like ADT, ask whether it's a catamorphism (single pass, fold each node's children to a value, then combine). If yes, write `foldTree` once and parameterize by the per-node function. You'll halve the code and the bug surface.

**Where the schools dissent.** Recursion schemes are gorgeous and theoretically tidy; they are also a high-effort abstraction that pays off most in libraries and compilers. For application code, plain recursion with a clear name is often better.

---

## Equational reasoning and refactoring

**The property.** In pure code, `let y = expr in body` is equivalent to substituting `expr` directly for `y` everywhere in `body`, in both directions. You can hoist subexpressions, inline them, common-subexpression-eliminate, reorder independent ones, and the meaning is unchanged. This is the same property compilers exploit for optimization; the gift to humans is that **refactoring is mechanically safe**.

**Practical application.** When you see `f(x) + f(x)`, you can extract `let fx = f(x) in fx + fx` without thinking. When you see `g(f(x))`, you can introduce `let fx = f(x) in g(fx)`. When you see two pure expressions in either order, you can swap them. None of these moves are safe in code with hidden effects -- `print("a"); print("b")` and `print("b"); print("a")` have different observable behaviour, and `f()` called twice may not return the same thing.

**Review flag.** Code that depends on the order of evaluation of "incidental" things (logging, metrics, caching warm-up, lazy initialization side effects). It works today; it will break under refactoring, reordering, or compiler optimization. Either make the dependency explicit (sequence the operations in a monad/effect/explicit ordering) or remove the dependency.

**Bird's "pearls" style.** Derive a program by equational manipulation from its specification, the way you'd derive a math proof. The output is usually short, beautiful, and correct by construction. Rarely the best engineering choice for a 200-engineer codebase, but a powerful drill for the kinds of problems where it applies (algorithm design, parser combinators, compiler IR rewriting).

---

## Totality and partiality

**Total functions.** Defined for every input, always terminate, always return a value of the declared type. Partial functions: may diverge (infinite loop, infinite recursion), throw, or return on only some inputs. Haskell, despite its purity reputation, is *not* total -- it permits `undefined`, `error`, non-terminating recursion, and partial pattern matches. Idris, Agda, Lean, Coq enforce totality (with `partial` or `unsafe` escape hatches).

**Why totality matters.** A total function `A -> B` is a *proof* that you can construct a `B` from any `A`. A partial function is a proof of nothing -- "I might give you a B, or I might loop forever, or throw." If you want types as guarantees rather than types as documentation, you need totality.

**The cost.** Some algorithms are easy partial and hard total. A while-loop terminating on an unknown condition needs a termination proof. General recursion needs a well-founded measure. You end up writing the algorithm and the proof, and the proof can be larger than the code. Production code outside the verification community generally tolerates partiality and pays the cost in tests and runtime checks instead.

**Practical application even outside Idris.** Even in non-total languages, *prefer total functions where you can*. Replace `head` (partial -- crashes on empty list) with `headOption` / `tryFirst` / pattern match (total). Replace `parseInt` (throws) with `Option<int>` / `Result<int, ParseError>` (total). Push partiality to the boundary; the more your inner functions are total, the more your error paths are explicit and the more your tests cover meaningful cases instead of "did it crash."

---

## Denotational design (Conal Elliott's school)

**The conceptual move.** Before writing any code, ask: *what does this thing mean*? Pick a precise mathematical denotation -- a function, a set, a stream, a measure, an algebra -- and then derive the API from the denotation. The "type-class morphism" principle: every operation on your type should be a morphism in the denotation -- that is, `denote(x + y) = denote(x) + denote(y)` for the appropriate `+` on the denotation side. If you can't pin down an operation's denotation, you don't yet understand it.

**Tangible values / FRP.** Elliott's functional reactive programming defines a `Behavior a = Time -> a` (a value that varies over time) and an `Event a = [(Time, a)]` (a stream of timestamped occurrences). Every FRP operation is justified by what it does to those denotations. The library becomes near-impossible to misuse because the denotation rules out incoherent operations.

**Practical application.** When you're designing a non-trivial library or DSL, write down the denotation in math (or pseudocode) first. "A `Stream a` denotes an infinite sequence of `a`s." "A `Parser a` denotes a function `String -> Set<(a, String)>`." Now derive `map`, `combine`, `filter` from the denotation. The code follows; the API is principled; the implementation can be replaced without breaking callers because they were programming against the meaning, not the implementation.

**Where the schools dissent.** Denotational design is slow and front-loaded. For exploratory code, for "let me try something and see if it works," for the inside of a feature you're not sure you'll keep -- it's overkill. Hickey would argue the REPL gets you to "what does it mean" faster than the whiteboard does. Both are right; the choice depends on the half-life of the code.

---

## Hickey's simplicity argument (the Clojure school)

**Simple vs easy.** "Simple" means *un-complected*: one role, one task, one concept, not braided together with others. "Easy" means *near at hand*: familiar, available with low effort. They are not the same; in fact they often trade off. Java is easy (you know it, the tools are there); Java is not simple (objects complect identity, value, behaviour, state, and time). Lisp macros are simple (just code-as-data) but not easy (you have to learn them).

**Complecting is the enemy.** When you intertwine concerns (state and identity; data and behaviour; time and value; what and how), you lose the ability to reason about either separately. The fix is *de-complecting*: separate data from functions on data, separate values from identities holding them over time, separate the *what* (declarative description) from the *how* (execution strategy).

**Values vs places.** A value is something that doesn't change (a number, a date, a snapshot of an account). A place is a slot whose contents change (a variable, a row in a database, an account *as an identity*). In OO, the two are smushed together: the `Account` *object* is both the value (current balance, owner) and the identity (this is the same account I was looking at yesterday). Separate them: the account-as-identity is a reference; the contents are an immutable value; transactions are functions producing new values; updates are atomic swaps. Now the system has *time* as a first-class concept rather than a hidden everywhere-mutation.

**Information vs encoding.** The *data* is the API. A map with named keys is more flexible than a class with named fields because no consumer needs to import the class. Rich's argument is that strong static typing of business data is often a premature encoding decision that makes evolution painful; better to keep data as data and validate at the edges. (The pure-FP school disagrees: the type *is* the documentation, evolution is supported by careful versioning. Both are coherent positions; the trade is "ease of evolution" vs "compile-time guarantee".)

**Practical application.** In any code review, ask: what is complected here? "This function does X and also Y" -- split. "This object is both the cached snapshot and the live reference" -- split. "This type encodes both the data and how it's persisted" -- split. Most "complexity" complaints reduce to "too many things are tangled at this point."

---

## The Curry-Howard correspondence in practice

**The deep claim.** Propositions are types; proofs are programs; normalization (running the program) is proof reduction. A function `A -> B` is a proof of the implication "A implies B": given an `A`, the function constructs a `B`. A product `(A, B)` is a proof of "A and B" -- you have both. A sum `A + B` is a proof of "A or B" -- you have one and you know which. The empty type `Void` / `Never` / `!` is the proposition `False` -- if you can construct one, the world is broken. The unit type is `True` -- trivially constructible.

**Why this matters even if you never touch Lean/Agda/Coq.** Curry-Howard is the *foundational justification* for most of what makes FP type-y. Parametricity falls out of it: `forall a. a -> a` is the proposition "for all propositions A, A implies A", and the only proof is the identity. ADTs *are* propositional logic with constructive proofs. Totality is what lets you read a type as a proposition (a non-total function "proves" nothing -- it might loop). Refinement types (`{x: Int | x > 0}`) encode quantified predicates as types. Dependent types (`Vec n a` -- a vector of *exactly n* `a`s, with `n` a runtime value) encode universally-quantified propositions.

**What it gives you practically.**
- **Parametricity** as discussed above: types as enforced contracts.
- **GADTs** (generalised algebraic data types): an ADT whose constructors carry type evidence. A `Expr Int` constructor carries the proof that its result is `Int`; `eval :: Expr a -> a` is total without runtime type tags.
- **Refinement types** (Liquid Haskell, F*, Idris with views): preconditions and postconditions checked by the type system. `divide :: Int -> {n: Int | n /= 0} -> Int` makes division by zero a compile error.
- **Phantom types** and **type-level state machines**: encode "this connection is open" or "this builder has had `setName` called" as type parameters; the type system tracks the state.
- **"If it compiles, it works"** as an honest aspiration -- not a guarantee, but a strong default that you've ruled out a large class of bugs by construction.

**The fetish to avoid.** Curry-Howard does *not* mean "rewrite your CRUD app in Agda." For most production code, the practical payoff is at the level of ADTs, total functions, parametric polymorphism, and (in the languages that have them) GADTs and refinement types. Dependent types and proof assistants are essential for compilers, security-critical kernels, formal verification, and pedagogy; they are 10-100x development cost and rarely the right choice for a typical product codebase.

**Review flag.** When you find yourself adding a runtime check that "should be impossible," you're paying a runtime cost because the type system doesn't know what you know. Sometimes that's correct (the input crosses a trust boundary). Sometimes it's a signal that a better type (a sum, a refinement, a phantom-type-tagged state) would move the check to compile time.

---

## The schools of FP and where they disagree

Capture the dissent rather than collapse it; pick per codebase.

**Pure FP / Haskell.** Effects in types via monads (`IO`, `ST`, `State`, custom transformer stacks), total purity as the aspiration, lazy evaluation by default, type classes for ad-hoc polymorphism. *Strength*: maximum reasoning power, parametricity honoured, equational reasoning genuinely safe, optimisations the compiler can perform are dramatic. *Weakness*: monad transformer stacks become plumbing-as-code, every effectful operation must be threaded through, error messages on type-class resolution can be cryptic, laziness creates space leaks if you're not careful, the ecosystem is small. *Use when*: correctness matters more than ship speed, the team is committed to the paradigm, the problem decomposes into pure transformations.

**ML / OCaml / F# pragmatic.** ADTs and pattern matching, first-class functions, parametric polymorphism, but unrestricted mutation when it helps, effects implicit in the type system, strict evaluation. *Strength*: very practical, ships features fast, the type system catches the high-value bugs without imposing the monadic discipline, performance is competitive with C. *Weakness*: less reasoning power than Haskell -- you can't tell from the type whether a function is pure, so equational reasoning is local rather than global. *Use when*: you want ADT-driven design and pattern matching but need to ship and to drop to mutation in hot paths.

**Lisp / Clojure dynamic.** ADTs implicit (just maps with namespaced keys), immutability and persistent data structures by default, REPL-driven development, macros for syntactic abstraction, dynamic typing (with optional `spec`/`Schema`/Malli for boundary validation). *Strength*: minimal ceremony, live development, easy to refactor incrementally, runtime introspection, Hickey's de-complecting principles in their natural habitat. *Weakness*: less compile-time guarantee, refactors that a static checker would catch are caught by tests instead (and sometimes by users), type-driven design is harder. *Use when*: the problem is exploratory, the team values REPL-driven iteration, the cost of a bug is small or the test coverage is strong.

**Verification-oriented / dependently-typed (Idris, Agda, Lean, Coq, F\*).** Dependent types, totality, types as propositions taken seriously, proof obligations as part of writing code. *Strength*: highest correctness available, the type is the spec, "if it compiles" approaches "it's correct." *Weakness*: 10-100x development cost, the ecosystem is academic, hiring is hard, even simple algorithms can require non-trivial termination proofs. *Use when*: the cost of failure is catastrophic (cryptography, compilers, aerospace, kernels) or the value is in the proof itself (academic verification, security audits).

**Rust as FP-influenced systems.** ADTs (`enum`), traits as type classes, iterators as fused-by-default streams, pattern matching with exhaustiveness, `Option`/`Result` instead of nulls and exceptions, ownership as a substitute for garbage collection. Mutation pervasive but controlled by the borrow checker. *Strength*: FP-influenced design at zero runtime cost, the type system catches genuine concurrency bugs, no garbage-collector pauses. *Weakness*: no higher-kinded types so true monads are awkward, async-and-traits interactions are still rough, the borrow checker has a learning cliff. *Use when*: you need systems-level performance and want as much FP discipline as fits within that constraint.

**The Carmack / Armstrong dissent.** FP doesn't help everywhere. Some genuinely-subtyped behaviour (a UI component tree, a scene graph, a plugin system) is well-served by classes and inheritance. Some performance-critical code (a physics solver, an allocator, a video codec) is well-served by mutation, locality, and explicit memory layout. Some problems (Erlang's "let it crash" style) are well-served by supervision trees and isolated mutable processes rather than pure data flow. The mature position is "FP is a default, not a religion": apply it where it pays, drop it where it doesn't, and don't let the paradigm prevent you from seeing the problem.

---

## Patterns to flag in review

Quick list of FP-relevant code smells, in order of severity.

- **Nullable field clusters** -- a record with 5+ nullable fields that "go together" by convention. Almost always a sum-type-in-waiting.
- **Stringly-typed states** -- `status: "draft" | "submitted" | "paid"` as a string in a record whose other fields depend on the value. Lift the state into a closed sum.
- **`_ =>` wildcards in match expressions** -- defeats exhaustiveness; future variants silently fall through.
- **Mutation hidden inside an apparently-pure function** -- writes to a cache, a log, a global. Either lift it into the signature or move it to the edge.
- **`as`/`unwrap`/`!!`/`throw` for "impossible" cases** -- you're paying runtime cost because the type doesn't encode what you know. Sometimes correct (trust boundary); often a sign a better type exists.
- **Class hierarchies modelling data cases** -- five sibling subclasses with the same shape modulo one field. Sum type.
- **`for` loops that build a list** -- almost always `map` / `filter` / `flatMap` / `fold` in disguise. Exception: early exit or complex inter-iteration state.
- **Functions that take a "config object" and read 3 fields** -- prefer taking the 3 fields; smaller surface, easier to test, more parametric.
- **Repeated `if x != null` chains** -- monadic bind in disguise. Use `Option`/`Result` combinators (`map`, `andThen`, `?`) or the language's null-coalescing.
- **Returning sentinel values (-1, empty string, null) for "not found"** -- replace with `Option<T>`. The type carries the absence; callers can't forget to check.
- **A "manager"/"service"/"helper" class with no state** -- the methods are functions in a trench coat. Promote to free functions; lose the indirection.
