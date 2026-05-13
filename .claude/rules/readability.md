# Code Readability

A reference for evaluating code from a human-reader lens during review. Used by the `readability` subagent. Distinct from `simplification.md` (which asks "is there surplus complexity?") -- this file asks "even at appropriate complexity, can a human follow this?"

The grounding text is the user's `~/.claude/rules/coding-style.md` (especially "Functions tell a story" and the architectural principles). This file extends that with the established readability canon: Kernighan & Plauger's *Elements of Programming Style*, Kent Beck's *Smalltalk Best Practice Patterns* and *Tidy First?*, John Ousterhout's *A Philosophy of Software Design*, Robert Martin's *Clean Code* (cited selectively -- the field broadly agrees on its naming and small-function guidance and broadly disagrees with its three-line-function dogma), Dan North's CUPID properties (Composable, Unix-philosophy, Predictable, Idiomatic, Domain-based), and Sandi Metz's "All the Little Things."

The core thesis: code is read far more than written. The reader is the production user of source code. Readability is the cost of every future change -- bugs found, features added, onboardings completed. Optimizing for it is rarely wasted, but it has a sharp limit: *over-naming, over-paragraphing, and ceremony-as-clarity* are themselves readability failures.

Project-specific style guides override the principles here. The agent must read the repo's `CLAUDE.md` and any `.claude/rules/*.md` first; project conventions win.

---

## Naming

Naming is the most-leveraged readability lever. A good name is precise, contextually-fit, and pronounceable in a code review without giving up.

### What good names do
- **Convey intent without consulting the body.** `daysUntilExpiry` over `dx`; `findFirstUnpaidInvoice` over `process`. The reader's mental model of a function is built first from its name.
- **Match the abstraction level of the surrounding code.** A high-level orchestration function calls `chargeCardAndEmailReceipt`, not `httpPostStripeChargeAndQueueMailgunRequest`. Each layer names at its altitude.
- **Use the project's ubiquitous language.** When the team says "tenant," the code says `tenant`, not `customer` or `account` or `org`. Naming drift across a codebase is friction at every cognitive translation.
- **Distinguish similar things audibly.** `XYZControllerForEfficientHandlingOfStrings` vs `XYZControllerForEfficientStorageOfStrings` -- the eye slides over the difference. Renamings that survive code review tend to make distinctions audible at glance.

### What bad names do
- **Encode the implementation in the name.** `userMap`, `accountList`, `infoArray` -- the type leaks into the name, ages poorly when the type changes, adds no information the reader doesn't already have from the declaration.
- **Use abbreviations the team didn't agree to.** `usr`, `pmt`, `txn`, `acct` -- save four characters per use, cost a constant translation step for every reader who isn't already initiated.
- **Lie.** `userList` that's really a Set. `getCustomer` that creates one. `validateInput` that mutates the input as a side effect. Names that lie are worse than no names.
- **Be too short or too long for their scope.** A loop index in a 3-line block can be `i`. A field on a class accessed from 40 sites cannot. Scope-fit is the rule.

### Specific name-shape findings

- **Single-letter variables outside the narrow exceptions** (numeric loop indices, math/physics domain values where the letter maps to the domain). User's global rule. Flag `data.map(x => x.id)` -- write `data.map(item => item.id)`.
- **Hungarian notation** (`strUserName`, `bIsActive`, `arrItems`). Encodes the type in the name; obsolete in any statically-typed language; rarely useful in dynamic ones.
- **`get` / `compute` / `fetch` / `load` confused.** `getUser` should be cheap and side-effect-free; `fetchUser` implies I/O; `loadUser` implies it might be slow / cache-warming. Mixed conventions across a module signal nothing-is-cheap-and-everything-is-suspect.
- **Boolean names without polarity.** `flag`, `status`, `state` for booleans. Replace with `isActive`, `hasPermission`, `shouldRetry` -- the answer to "true means what?" should be in the name.
- **Negation in name + negation in test** (`if (!isNotEmpty(...))`). Double negative; rewrite the predicate.

---

## Function shape

The user's `coding-style.md` is the primary reference: function body reads as a declarative outline; named bindings mean one thing for their lifetime; no internal mutability accumulators. This file adds the reader-facing dimensions on top.

### What the reader needs
- **A function name that announces what it does, not how.** `chargeCustomerAndIssueReceipt` over `runChargeFlow`. The "what" survives refactors; the "how" rots.
- **A function body that reads top-to-bottom in the order the logic happens.** When the order is "compute everything, then decide," put the computations first and the decision last. When the order is "decide, then maybe compute," guard-clause early and compute after the guards. Match the prose order to the logic order.
- **A consistent altitude.** A function that mixes one line of high-level orchestration (`processPayment(order)`) with twenty lines of low-level byte manipulation forces the reader to context-switch on each line. Extract the low-level work into a named helper at the lower altitude.
- **Stable inputs and outputs at the call site.** The reader should be able to predict what `result = foo(input)` produces by reading the call line. A function whose return type depends on the input value (`foo(x)` returns `User` or `Error` or `null` or `undefined` based on shape) makes the call site a question instead of a statement.

### Where readability fights size

The Martin "functions should be small" guidance is widely cited and widely overapplied. The honest formulation: **functions should be exactly as long as the single thing they do.** Some single things are 3 lines; some are 30. Extracting a one-call helper because "the function got long" is a readability *regression* -- the reader now has to jump to the helper to verify it does what its name implies, and the original function's logic is split across two locations.

Ousterhout's counter-framing: prefer **deep modules** (small interface, substantial implementation hidden behind it) over **shallow modules** (small interface, small implementation, mostly forwards to another module). A 30-line function with a precise name and clear local logic is a deep function. A 3-line function that calls two helpers and returns is a shallow function -- the abstraction tax exceeds the savings.

### Specific function-shape findings

- **Functions that take a flag parameter and switch on it** (`render(item, isPreview)`). Often two functions wearing a trench coat. Split when the two paths share less than half their logic.
- **Functions where the return value is the secondary outcome** (`function getUser(id, options): User; // also mutates options.touched`). Hidden side effects on parameters are readability traps. Either name them in the function signature or eliminate them.
- **Functions with three or more boolean parameters.** The call site is unreadable: `createNotification(true, false, true, false)`. Replace with a config object, an enum, or named/keyword arguments.
- **Functions where the parameter order is non-obvious.** `copy(src, dst)` vs `copy(dst, src)` -- both exist in the wild. Either match the language's idiom (Unix-style cmd: src first; assignment-style: dst first) or use named arguments.
- **Functions whose body is a single return of a complex expression**, especially with nested ternaries or chained `&&`/`||`. Named intermediates pay even when "it fits on one line."

---

## Flow, layout, and paragraphing

Beck's *Smalltalk Best Practice Patterns* introduced the idea that a method should read as paragraphs: a small group of statements that do one thing, separated by blank lines, with the first line of each paragraph naming what it does. This generalizes to most languages.

### Paragraph code
- **Group related statements; separate groups with a blank line.** The reader processes one paragraph at a time. A 30-line function as one continuous block forces the reader to load the whole thing; as five paragraphs with topic sentences (often a comment or a named intermediate), it loads in chunks.
- **Newline before a return.** If a function does work then returns, a blank line before the return separates the "doing" from the "yielding." Cheap, universally helpful.
- **Don't paragraph at every line.** Five paragraphs of one statement each is just noise. Group what's genuinely cohesive.

### Ordering
- **Guard clauses first.** The reader's most useful first question is "under what conditions does this function bail?" Frontload them. Indented happy-path code at the bottom is harder to read than guard-then-fall-through. (Linus Torvalds famously argued for guard clauses on this exact basis.)
- **Public before private** in many language idioms (Java, Kotlin, C#, Swift). Reader looking at the file wants the interface first.
- **Definition before use** when the language allows it. Hoisting works in JavaScript but human readers don't have hoisting; declaration-then-use is friendlier.
- **Locality of behavior.** Code that's read together should live together. A constant used only in one function lives in that function (or just above it), not in a `constants.ts` file 400 lines away.

### Indentation depth
- **Three levels is the soft ceiling.** Past three, the eye loses track of which `if` we're inside. Refactor with guard clauses, early returns, extracted helpers, or pattern matching.
- **Nested ternaries are readability bankruptcy.** `a ? b : c ? d : e ? f : g` -- the reader cannot scan it. Replace with `switch`/`match` or named intermediates.

---

## Comments

Comments are not a substitute for unreadable code. They are documentation of *why*, *when not to follow the obvious path*, and *what the type system doesn't carry*.

### Comments that earn their keep
- **Why the code is the way it is, when "the way it is" is non-obvious.** "We retry on 503 but not 502 because upstream returns 502 for non-retryable misconfiguration." This is the kind of comment that survives refactors because the *why* survives.
- **The constraint the code is encoding that the type system can't.** "This function assumes `items` is sorted by `created_at` descending; resorting it elsewhere is wrong." Where a refinement type would say it in the signature, the comment is the runtime fallback.
- **The reason a not-obviously-better alternative was rejected.** "We chose mutation here instead of a fold because the profiler showed the fold was the bottleneck at p99." Future-you will be tempted to "fix" the mutation; the comment is the brake.
- **The pointer to the external context.** "Mirrors the algorithm in RFC 7231 §6.1.1." Links the code to authoritative sources.

### Comments that should be deleted
- **Restating the code in English.** `// increment counter` above `counter++`. Adds noise; rots when the code changes.
- **Block comment "headers" on every function.** `///@param x: number; @returns number;` for a function whose signature already says `(x: number): number` -- duplication that the next refactor will desync.
- **Out-of-date commentary.** Comments rot faster than code because the linter can't check them. Stale comments lie. If a comment doesn't match the code, the comment is *worse than nothing* because the reader will trust it.
- **Commented-out code.** Version control is where dead code lives. `// old impl: foo.bar()` is signal-to-noise loss.

### Doc comments and docstrings
- **At public API boundaries: required.** Function signature alone rarely tells the caller what counts as valid input, what errors can be thrown, what the side effects are.
- **For internal helpers: write them when they aid the reader of *this* file.** Boilerplate `///` on every private function is ceremony.

---

## API and interface readability

Ousterhout's "deep modules" framing applies here -- the readability of an API is the readability of *the smallest information the caller needs to use it correctly.* Smaller surfaces are easier; ambiguous surfaces are harder regardless of size.

### What good APIs do
- **Make common cases easy, rare cases possible.** The 90% caller writes 1-2 lines; the 1% caller threads through more configuration.
- **Hide what doesn't need to be public.** Every exported symbol is a contract; readers must understand it. Modules that export everything are unread by definition.
- **Use types that carry the invariant.** `Result<T, E>` over magic sentinel values; `NonEmpty<T>` over `assert items.length > 0`. Per the user's `coding-style.md` parse-don't-validate.
- **Compose.** A library where building blocks combine is a library where the reader can predict what `foo |> bar |> baz` does without reading `bar` and `baz`.

### What bad APIs do
- **Leak the implementation.** Naming a method `getCachedAndValidatedUserOrThrow` exposes internal mechanism. Prefer `getUser` and document the validation/caching in the doc comment or move them inside.
- **Use sentinel returns.** `findIndex` returning `-1` for not-found requires every caller to remember the convention. `findIndex(): Option<number>` doesn't.
- **Mix concerns.** `saveAndEmail(user)` -- two operations bound at the wrong layer. Reader can't tell which one matters when it goes wrong.

---

## What is NOT a readability finding

The dual lens. Reviewers who flag every "this could read better" become noise.

- **Local style choices the team has made.** If the repo uses `let` for reassignable bindings and `const` only for true constants, the readability agent does not flag `let`-everywhere as a style problem -- it's the project's convention. Read `CLAUDE.md` first.
- **Idioms in the language's ecosystem.** Rust's `?` chains, Haskell point-free style, Pandas method chaining, Kotlin scope functions -- if the language's community reads them fluently, they're not unreadable, they're idiomatic.
- **Density that aids review.** A 5-line `match` on a closed sum is more readable than a 50-line `if`/`else if`/`else` chain even if it's denser per line. Density is not the inverse of readability.
- **Verbosity at trust boundaries.** Explicit error types, exhaustive case handling at edges, named intermediates that match the validation steps -- these cost lines but pay in audit trail.
- **Code the author has documented as deliberately unusual.** A comment that says "this looks weird because we measured the obvious form and it was 10x slower" is the documentation that the reviewer's instinct was anticipated and overruled.

---

## Severity and confidence

The `readability` subagent produces findings at these severity levels (matching `/expert-review`'s scale):

- **major**: the code is genuinely hard to follow at first read. A reader new to the file would take substantial time to build a mental model. Examples: deeply nested control flow, names that lie, mixed altitudes within a function, public APIs without doc comments.
- **minor**: noticeable friction but not blocking. Awkward naming, paragraph-less long functions, comments that restate the code, return values that need explanation at every call site.
- **nit**: cosmetic preferences. Names that could be slightly clearer, helper that could be inlined, parameter order matching/not matching the rest of the file.
- **insight**: a deeper readability observation -- "this entire module would be easier to read as a state-machine ADT instead of a flag-on-flag struct," "this file's import order obscures the dependency direction." Discussion-worthy.

Confidence: high when the readability problem is concrete and verifiable ("this 90-line function has 7 nested levels of `if`/`else`; the deepest block contains the actual logic"); medium when reasoned ("the naming feels inconsistent across these three functions, though I cannot fully verify the team's convention without more context").

---

## Process for the readability agent

1. **Read the project's style guide.** `CLAUDE.md`, `.claude/rules/*.md`, any `STYLE.md` or `CONTRIBUTING.md`. Project conventions override generic principles.
2. **Read the code given.** For survey mode, the unit is the file or module; for diff mode, the changed region plus enough surrounding context to judge altitude and naming consistency.
3. **Walk the categories** -- naming, function shape, flow/layout, comments, API/interface. Flag readability problems anchored to specific lines.
4. **Apply the "what is NOT a readability finding" filter.** Don't flag idioms, team conventions, or documented-deliberate choices.
5. **State the better form concretely.** "Hard to read" is not a finding; "this 14-line guard-pyramid would read as 4 guard clauses followed by 6 lines of happy-path logic" is.
6. **Acknowledge tradeoffs.** Renaming has cost (callers, mental models, git blame); the agent doesn't pretend it's free.
7. **Stay read-only.** Suggest; do not apply.
