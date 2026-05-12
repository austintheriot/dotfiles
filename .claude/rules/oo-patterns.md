# Object-Oriented Design Patterns

A reference for an FP-leaning engineer reviewing object-oriented (OO) code. The goal is recognition and evaluation: when is a pattern doing real work, when is it ceremony, what is the modern (often functional or hybrid) alternative? No code samples; companion to `coding-style.md` and `functional-patterns.md`.

The "Gang of Four" (GoF) book (1994: Gamma, Helm, Johnson, Vlissides) cataloged 23 patterns drawn from C++ and Smalltalk practice. They were not invented but named, after the same shapes kept recurring. The historical context matters: most GoF patterns exist to compensate for things that 1990s-era OO languages (C++, Java, early Smalltalk) made hard, namely first-class functions, sum types, pattern matching, and parametric polymorphism. "Strategy" in C++ 1994 is a class with a virtual method because you could not pass a function. "Visitor" exists because you had no sum types. Once you see this, half the catalog collapses: in a language with closures and algebraic data types (ADTs), those patterns are noise around a one-liner.

The modern view (Sandi Metz's "All the Little Things", Yegor Bugayenko's "Elegant Objects" critique) is that pattern names are vocabulary for review conversations, not building blocks to assemble. A codebase that visibly "implements the Visitor pattern" with named classes is usually worse than one that pattern-matches on a sum type. The exceptions (Adapter, Facade, Composite for genuine trees, Decorator as middleware) earn their keep. Most of the rest are a smell to investigate, not a goal to reach.

---

## Creational patterns

These patterns control how objects come into existence. The historical pain: constructors in C++/Java are inflexible (cannot return subtypes, cannot fail without exceptions, cannot delay work), so creation logic sprawled across the codebase.

**Singleton: one instance, global access.** Universally considered the most overused pattern. Two standard critiques: (1) it is global mutable state in disguise, which destroys testability (you cannot swap it out per test), and (2) thread-safe lazy initialization is subtle and often wrong. Modern alternative: dependency injection (pass it in), module-level state (Python modules, Rust statics), or a single instance owned by `main` and threaded through. Service Locator is sometimes proposed as a fix but inherits the testability problems. Well-applied only for truly process-global resources where DI buys nothing (logger handle, metrics registry).

**Factory Method: delegate object creation to subclasses.** Solves "the base class needs to create *some* `Foo`, but only the subclass knows *which* `Foo`." Useful when construction needs polymorphic dispatch over a real class hierarchy. Misapplied when the "factory" is just a static method returning `new Foo()` (a constructor with extra steps). Modern alternative: a plain function returning the right variant, or a sum-type constructor. Rust idiom: an associated function like `Foo::from_config`.

**Abstract Factory: families of related objects.** Solves "I need a coherent set of objects (Windows button, scrollbar, menu) without the caller knowing the family." Heavyweight; rarely the right answer in modern code. A struct of functions or a trait with several constructor methods usually suffices. Well-applied for theme/platform abstraction with three-plus parallel hierarchies; overused when the second family never materializes.

**Builder: stepwise construction of complex objects.** Solves "this thing has 14 fields, half optional, and a telescoping constructor is unreadable." Genuinely useful. Modern alternatives: named/keyword arguments (Python, Kotlin, Swift, C#), record literals with defaults, fluent application programming interfaces (APIs), and the typestate builder (see `functional-patterns.md`, `rust.md`) which encodes "you must set X before Y" in the type system. Overused when a record with three optional fields gets a 200-line builder class.

**Prototype: clone an existing instance.** Solves "construction is expensive; copy and tweak." Mostly historical in class-based languages. The prototype-based OO lineage (JavaScript pre-class, Self, Lua, Io) makes this the foundation. In modern class-based code, "copy and modify" is a `with`/`copy` method on an immutable record (Kotlin `data class.copy()`, Rust/F#/Scala struct update syntax, TypeScript spread). Rarely the right shape to reach for explicitly.

---

## Structural patterns

These patterns compose objects into larger structures. They age better than creational patterns because composition is a real, language-agnostic concern.

**Adapter: make one interface look like another.** Solves "third-party library expects `IDuck`, I have a `Turkey`." Genuinely useful at boundaries; "wrap a third-party library to fit your domain interface" is exactly what hexagonal architecture's ports-and-adapters formalizes. Misapplied when used to bridge two of your own interfaces that you could just unify.

**Bridge: separate abstraction from implementation.** Solves "I have a cross-product of two hierarchies (`Shape` x `Renderer`) and inheritance would force four classes." Often confused with Adapter; the distinction is that Bridge is designed up front to vary independently while Adapter retrofits an existing mismatch. Modern code gets this for free via traits/interfaces plus composition: `Shape` holds a `Renderer` by reference. Rarely worth naming explicitly today.

**Composite: tree of objects treated uniformly.** Solves "I want to call `render()` on a leaf and on a container without caring which." Useful for genuine tree structures (file systems, the Document Object Model (DOM), abstract syntax trees (ASTs), UI component trees). In FP, this is a recursive ADT: `Tree = Leaf | Node(Tree[])`. The OO version with `Component`/`Leaf`/`Composite` classes is the same idea expressed without sum types. Overused when forced onto data that is actually a list.

**Decorator: add behavior without modifying the underlying class.** Solves "I want to add logging/caching/retries to this service without subclassing it for every combination." Genuinely useful. Modern alternatives: function composition, middleware (Express, Koa, Axum/Tower, Rack, ASP.NET Core), higher-order functions, Python decorators. A "request handler with five middlewares" in modern web frameworks *is* GoF Decorator, repackaged.

**Facade: simplify a complex subsystem.** A single entry point that hides 40 classes. Universally useful; one of the few patterns nobody argues against. Every well-designed library has a Facade whether or not it admits to it (`requests.get` in Python, the top-level `tokio` re-exports). Misapplied only when the Facade itself grows into a god object.

**Flyweight: share state to save memory.** "A million `Glyph` objects, 98% of state identical." Specialized: game development (instanced rendering), large UI trees (React reconciliation conceptually), text engines. Rare in business code. Modern variants live in data-oriented design and entity-component systems (ECS).

**Proxy: a stand-in object with the same interface.** Solves lazy loading, access control, remote-call marshaling, or transparent instrumentation. Object-relational mappers (ORMs) use lazy-loading proxies heavily (Hibernate, Django, ActiveRecord); remote-procedure-call (RPC) stubs use remote proxies. Modern code often writes the lazy code directly. Smell: if a Proxy hides whether a call is local or remote, you have built a leaky abstraction (Waldo's "Note on Distributed Computing").

---

## Behavioral patterns

These patterns describe how objects communicate. They are the GoF subset most thoroughly obsoleted by first-class functions and sum types.

**Chain of Responsibility: pass a request through handlers until one handles it.** Middleware pipelines are exactly this. Modern alternative: fold over a list of handler functions, or function composition. Well-applied for real pipelines (Express middleware, Servlet filters, Tower layers); overused when the chain has two handlers (an `if`/`else`).

**Command: encapsulate a request as an object.** Solves queueing, logging, undo/replay, or transmitting operations. Useful for undo/redo stacks, job queues, audit logs, Command Query Responsibility Segregation (CQRS) systems. Modern alternative: a discriminated union of command variants plus a handler function. The OO version forces one class per command; the FP version is one ADT case per command, all in one file. Overused when the "command" is invoked once and never serialized or queued.

**Iterator: traverse a collection without exposing internals.** Built into every modern language. Was a pattern because C++ Standard Template Library (STL) iterators were new and Java pre-1.5 made you implement `Enumeration`/`Iterator` by hand. Today: ranges (Rust, C++20), generators (Python, JavaScript), `IEnumerable` (C#), streams (Java 8+). Almost never "implemented" by name anymore.

**Mediator: encapsulate object interaction.** "Five UI widgets need to know about each other; route through one Mediator instead of N-squared references." Useful when many-to-many communication is out of hand. Risk: the Mediator becomes a god object. Modern alternative: event bus, reactive streams (RxJS, signals), or hoisted state (React's "lift state up"). Good Mediator is a stateless dispatcher; bad one is a 3000-line `ApplicationCoordinator`.

**Memento: save/restore state.** Useful for undo, time-travel debugging, transactional rollback. Modern alternative: immutable snapshots are free. In an FP-style codebase where state is an immutable value, "Memento" is `let previous = state; state = step(state); if (cancel) state = previous`. The pattern earns its name only when state is genuinely mutable.

**Observer: notify dependents of state changes.** Genuinely useful but increasingly superseded by reactive frameworks (RxJS, Solid/Svelte signals, Vue reactivity, MobX, Compose State, SwiftUI's `@Published`). Manual `addObserver`/`notifyAll` is a known source of bugs (memory leaks from forgotten unsubscribes, ordering, re-entrancy). Well-applied for simple in-process pub/sub; misapplied for reactive data flow a signals/streams library would express more safely.

**State: behavior depends on object state.** Solves "this object behaves differently in `Draft`/`Submitted`/`Approved`; the alternative is `if (status == ...)` everywhere." The classic OO state machine: one class per state, transitions return new state objects. Modern alternatives are usually a strict improvement: typestate (Rust, encoding states as types so illegal transitions do not compile, see `rust.md`), or state-as-ADT plus pattern match (Elm, F#, Haskell, TypeScript discriminated unions). The OO State pattern is acceptable when the language lacks sum types; with them, it is verbose for no gain.

**Strategy: encapsulate interchangeable algorithms.** "Swap the sort comparator/pricing rule/retry policy at runtime." Modern alternative in almost any modern language: pass a function. `list.sort_by(fn)` is Strategy. The OO version (an interface with one method and three implementations) is a closure in disguise. Well-applied where the strategy is stateful or has multiple methods; otherwise, prefer the function.

**Template Method: define skeleton, let subclasses fill in steps.** Inheritance-heavy; the "yo-yo problem" usually starts here. Modern alternative: a higher-order function taking the variable steps as parameters. Trade `AbstractProcessor` plus two subclasses for one function with two function parameters. Well-applied when many variants share substantial state; otherwise, a strict regression vs. higher-order functions.

**Visitor: separate operations from object structure.** The OO answer to the *expression problem*: you can either easily add new types (OO inheritance) or easily add new operations (Visitor), but not both. Useful for closed-AST traversal, compilers, interpreters. Modern alternative: pattern matching on a closed sum type, or its FP generalization the catamorphism (covered in `functional-patterns.md`). Rust's `match` on an `enum`, OCaml/Haskell ADTs, TypeScript discriminated unions solve the same problem more directly. The `accept`/`visit` double-dispatch is the workaround for languages without sum types; if you have them, do not write a Visitor.

---

## Modern architectural patterns

Beyond GoF, the OO world produced *architectural* patterns concerned with whole-application organization. These appear most often in modern enterprise code review.

**Hexagonal Architecture / Ports-and-Adapters (Cockburn).** The domain core knows nothing of databases, HTTP, message brokers, or UIs. It exposes *ports* (interfaces in the domain's language: `OrderRepository`, `PaymentGateway`) and the infrastructure layer provides *adapters* (Postgres `OrderRepository`, Stripe `PaymentGateway`). Tests substitute in-memory adapters. The "responsible OO" answer to coupling. Well-applied when the domain is rich enough to warrant the isolation; overkill for a create-read-update-delete (CRUD) app where the domain *is* the database schema.

**Clean Architecture (Robert Martin).** Concentric layers (entities, use cases, interface adapters, frameworks/drivers) with the *dependency rule*: source dependencies point inward. Closely related to Hexagonal. Critique: often implemented as ceremony (layer boundaries nothing actually crosses) rather than substance.

**Onion Architecture (Palermo).** A near-twin of Hexagonal/Clean with domain at the center. Differentiator: heavy emphasis on "no infrastructure references in domain code." For an FP-leaning reader, the unifying picture is "pure domain core, effects at the edge." Gary Bernhardt's "functional core, imperative shell" is the same insight without layered diagrams.

**Repository.** Abstract data access behind a domain-flavored interface (`UserRepository.findActive()` rather than `db.query("SELECT ...")`). Useful for testing and decoupling from ORM specifics. Critique: often becomes a leaky abstraction. If `UserRepository` ends up with 47 methods (`findByEmailIgnoringCaseAndIncludingDeleted`), you have reinvented Structured Query Language (SQL) badly. The "just use the ORM" school argues ORMs already implement Repository internally. Middle ground: Repository at module boundaries, raw ORM inside.

**Unit of Work.** Track changes within a transaction and flush together. Often paired with Repository. Most ORMs (Hibernate's session, Entity Framework's `DbContext`, SQLAlchemy's session) implement Unit of Work internally. In review, look for the *absence* of Unit of Work: if a service method does five writes and crashes on the third, do the first two roll back? If not, that is the bug.

**Service Layer.** Stateless services orchestrating domain operations (`OrderService.placeOrder(...)`). Useful for application logic that does not fit on a single entity. Critique: easily becomes the *anemic domain model* anti-pattern where services contain all logic and entities are bags of getters/setters. Reasonable test: if `OrderService` reaches into `order.lineItems` and mutates them, that logic belongs on `Order`.

**Dependency Injection (DI).** Provide collaborators from outside rather than `new`-ing internally. Genuinely useful for testability and per-environment swaps. Constructor injection is the boring, correct default. DI *containers* (Spring, Dagger, Guice, .NET's built-in DI, NestJS) are often overkill for small apps. The functional alternative is the Reader monad / passing an environment record (see `functional-patterns.md`); both solve the same problem of avoiding argument threading.

**CQRS (Command Query Responsibility Segregation).** Separate write model from read model. Covered in `system-design-patterns.md` and `distributed-systems.md`; mentioned here because it pairs with the Command pattern and event sourcing. Well-applied at scale; overkill for typical CRUD.

**Saga.** Long-running multi-step transactions across services, with compensating actions instead of a single atomic commit. Choreography (each service reacts to events) vs. orchestration (a central coordinator). Covered in `system-design-patterns.md` and `distributed-systems.md`. The orchestration flavor is sometimes called *Process Manager*; functionally a state machine driving external effects, which collapses to state-as-ADT plus a step function.

**Specification.** A predicate object encapsulating a business rule (`IsEligibleForDiscount`), composable via `and`/`or`/`not`. Modern alternative: functions and closures compose the same way without ceremony. The pattern earns its name only when specifications are reified data (serialized, persisted, edited in a UI); for in-memory composition, prefer functions.

**Active Record vs. Data Mapper.** Two ORM philosophies. Active Record (Rails, Django ORM, Eloquent) bundles persistence with the entity: `user.save()`. Data Mapper (Hibernate, SQLAlchemy core, Entity Framework, Doctrine) separates them: `mapper.save(user)`. Active Record is faster to write but couples the domain to persistence and tends toward fat models. Data Mapper preserves a pure domain model and pairs naturally with Hexagonal/Clean. Match to project complexity.

**Anemic Domain Model (Fowler's anti-pattern).** Entities are bags of public getters/setters; all behavior lives in services. The DDD school considers this clearly wrong; the procedural-with-objects school considers it pragmatic, especially for CRUD apps where domain logic is thin. Heuristic: if `Order.calculateTotal()` would be a method on a stateful class but instead is `OrderCalculator.calculateTotal(order)`, ask whether the split buys anything.

---

## Anti-patterns to recognize

**God object.** One class doing everything: 4000 lines, 80 methods, references to every module. Cure: decompose along the seams it is hiding (usually 4-5 cohesive responsibilities tangled together).

**Yo-yo problem.** Deep inheritance hierarchy where understanding a method requires scrolling up and down five superclasses. A sign inheritance is being used for code reuse, not substitutability. Composition (or higher-order functions) fixes it.

**Circular dependency.** A depends on B depends on A. The abstraction is in the wrong place. Cure: extract a third module both depend on (dependency inversion), or invert one direction.

**Feature envy.** A method on class A spends all its time poking at class B's data. The behavior belongs on B. Example: `OrderService.calculateTotal(order)` that only reads from `order` -- move it to `Order.total()`.

**Primitive obsession.** Passing `string`, `int`, `string`, `int` where domain types (`UserId`, `Amount`, `Email`, `Age`) would prevent argument-order bugs and document intent. See `coding-style.md` and `functional-patterns.md` (smart constructors, branded types).

**Train wreck (Law of Demeter violation).** `customer.getOrder().getAddress().getCity().getName()`. Caller coupled to four layers of internal structure. Cure depends on intent: if you only need the city name, expose `customer.shippingCityName()`.

**Telescoping constructor.** Five overloaded constructors, each adding one more parameter, all chaining into the longest. Builder, named arguments, or a configuration record are valid fixes. Languages with named/default arguments rarely produce this in the first place.

---

## Cross-language notes

The same patterns wear different clothes across languages. Recognize the underlying shape, not the surface syntax.

**Java.** The canonical GoF target. Most patterns appear with explicit class hierarchies and named interfaces (`Strategy`, `Visitor`, `Observer` are real type names you will see). Java 8 lambdas and sealed classes (Java 17+) collapsed Strategy to `Function` and Visitor to `switch` on a sealed hierarchy. Modern Java looks much more like Kotlin than 2005 Java did.

**Kotlin.** Data classes plus sealed interfaces shrink half the catalog. Strategy is `(Input) -> Output`. State is a sealed class with `when`. Builder is named arguments with defaults. Singleton is `object`. Decorator is function composition or extension functions. Idiomatic Kotlin rarely names GoF patterns.

**Scala.** Case classes, traits with defaults, given/using (DI as language feature), `match`. Visitor is `match`. Strategy is a function. Composite is a recursive sealed trait. The patterns dissolve into the language.

**Swift.** Protocol-oriented programming deliberately reframes patterns around protocols with default implementations. Strategy is a protocol or closure. Observer is `Combine` publishers or `@Observable`. State is an `enum` with associated values. Singletons are `static let shared` (idiomatic but as critique-worthy as anywhere).

**TypeScript.** Discriminated unions plus exhaustive `switch` make State, Command, Visitor, and Strategy all just data plus functions. The Java-style patterns appear heavily in NestJS/Angular (DI, decorators) and look out of place in idiomatic React or Express, where function composition and hooks dominate.

**Rust.** Traits-and-enums is the most pattern-eating substrate of any mainstream language. Visitor and State collapse to `match` on an `enum`. Strategy is a function or trait object. Builder is the standard idiom (often with typestate). Singleton is `OnceCell`/`OnceLock` for genuine cases. Decorator is wrapping a struct implementing the same trait. Adapter is "newtype implementing a trait." See `rust.md`.

**Python.** Duck typing plus decorators (`@property`, `@cache`, `@dataclass`) make many GoF patterns language features. `abc` provides formal interfaces; `dataclasses` plus `match` (3.10+) handle most State/Visitor shapes; `attrs`/`pydantic` provide Builder semantics.

**Go.** No inheritance, no exceptions, no sum types. The language deliberately rejects most of GoF. Structural interfaces make Adapter trivial. Strategy is a function value. The Go pattern vocabulary is much smaller, and "just write the code" is the default; idiomatic reviewers are skeptical of imported GoF vocabulary.

Unifying observation: when a language gives you first-class functions, sum types with exhaustive matching, and parametric polymorphism, the behavioral half of GoF largely disappears. What remains useful (Adapter, Facade, Composite for trees, Decorator as middleware) is useful everywhere. Recognize the names so you can speak the OO dialect during review; reach for them only when the simpler answer (function, ADT, immutable record) does not fit.
