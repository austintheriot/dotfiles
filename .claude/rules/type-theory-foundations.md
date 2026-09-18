---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-18
---

# Type theory and category theory foundations

**Thesis.** Logic, computation, and category theory are three views of one structure. A proposition is a type is an object; a proof is a program is a morphism; normalization is evaluation. This is the Curry-Howard-Lambek correspondence, and its practical value is not that it lets you write proofs -- almost nobody should -- but that it tells you **which guarantees a type can carry, and what each one costs.**

**The agent's operational question:** *what invariant is being asserted here, what is the cheapest construct that enforces it, and what does the enforcement actually cost?*

**This agent explains why things work.** It does not make practical language-level design choices -- `[[fp-types]]` and `[[fp-effects]]` own those. The division: they answer "what should I do in my language," this file answers "why does that work, what is the general structure, and where on the ladder should this sit."

**Empirical priority order:**

1. **Escalation-ladder misjudgment.** Reaching past the rung that solves the problem, in either direction. Most common single error, and §7 is the honest cost account.
2. **Guarantees asserted that the language does not deliver.** Parametricity assumed where `seq`, specialization, or reflection breaks it (§3). Phantom types a cast can forge.
3. **Cargo-culted structure.** Categorical vocabulary imported without the laws that make it mean anything (§6, §9).
4. **Missed cheap wins.** A sum type that would make the illegal state unrepresentable; applicative where the code awaits sequentially for no reason.

---

## Volatile surface

`last-verified` in the frontmatter. **Most of this file is durable theory that does not rot.** §1, §2, and the theory in §3 and §5 are stable on a decades scale. Confine re-verification to:

| Claim class | Rots | Re-verify at |
|---|---|---|
| HoTT / cubical tooling status | Fast | Agda, 1Lab, Cubical Agda repos |
| Proof-assistant type-theory specifics | Medium | The system's own docs |
| Language feature status (Rust linear types, TS) | Medium | Official release notes only -- see the §5 disinformation warning |
| Empirical cost figures | Slow but contested | The primary papers; see §7 |
| Book editions and recommended routes in | Slow | Publisher pages |

---

## 1. The three-way correspondence

### The dictionary

| Logic | Computation | Category theory |
|---|---|---|
| proposition | type | object |
| proof | program (term) | morphism |
| implication `A → B` | function type | exponential `B^A` |
| conjunction `A ∧ B` | product / pair | categorical product |
| disjunction `A ∨ B` | sum / tagged union | coproduct |
| true | unit type | terminal object |
| false | `Void` / `Never` | initial object |
| negation `¬A` | `A → Void` | `A → 0` |
| `∀x:A. B(x)` | dependent function `Π` | right adjoint to weakening |
| `∃x:A. B(x)` | dependent pair `Σ` | left adjoint to weakening |
| proof normalization | evaluation | (categorical composition/equality) |

The quantifier rows are **Lawvere's** account: `Σ ⊣ substitution ⊣ Π`. Quantifiers are adjoints to context weakening. This is the deepest row in the table and the one that generalizes best -- it says quantification is not a primitive notion but a universal construction.

### Lambek's theorem, stated carefully

The folklore version -- "simply-typed lambda calculus and cartesian closed categories are equivalent" -- is **not** something this research pass could confirm at a primary source in that crisp form.

**What is verified:** there is an adjunction `Syn ⊣ Lang` between typed lambda theories and cartesian closed categories, which is an equivalence **in the well-behaved cases**, with real 2-categorical subtleties in the general statement.

Use that wording. A garbled Lambek statement is worse than an omitted one, and the crisp version circulates widely without a citation that supports it.

### What the correspondence buys without a proof assistant

- **Sums and products are logic.** Modeling with ADTs is doing propositional reasoning, and exhaustive pattern matching is case analysis. This is why "make illegal states unrepresentable" works: you are asserting a proposition the compiler checks.
- **`Void`/`Never` is falsity.** A function returning `Never` cannot return. A branch producing `Never` is unreachable. Exhaustiveness checks against an empty type are proofs of impossibility.
- **Totality is load-bearing.** A partial function proves nothing -- it might diverge. Every guarantee below degrades in a language that permits `undefined`, unchecked exceptions, or nontermination.

---

## 2. Category theory at the depth that pays

### Universal properties

The organizing idea, and the one most worth internalizing. A product is not "a pair"; it is *the* object with projections through which any other candidate factors uniquely. Definition by universal property means **unique up to unique isomorphism** -- there is one right answer and all constructions of it agree.

**The engineering payoff:** when you find yourself writing a "canonical" or "best" construction and wondering whether it is unique, look for the universal property. If there is one, the answer is yes and the property proves it. If there is not, you are making an arbitrary choice and should expose it as a parameter.

### Adjunctions

The most important concept in the subject.

`F ⊣ G` means `Hom(F a, b) ≅ Hom(a, G b)`, naturally. Read: **left adjoint is the most efficient way to ADD structure; right adjoint is the most information-preserving way to FORGET it.**

Concrete instances a programmer already knows:
- **Currying** is the adjunction `(− × A) ⊣ (A → −)`. The isomorphism `(a, b) → c ≅ a → (b → c)` is the adjunction's hom-set bijection.
- **Free/forgetful**: `List` is free monoid; `Set`→`Monoid` forgets. Free constructions are always left adjoints.
- **Galois connections** in static analysis: abstraction and concretization are adjoint. This is the formal content of "sound approximation."
- **Quantifiers** as adjoints to weakening (§1).

**RAPL:** right adjoints preserve limits, left adjoints preserve colimits. Practically: a right adjoint commutes with products and pullbacks; a left adjoint commutes with sums and pushouts.

### Monads, and the split people miss

A monad is a monoid in the category of endofunctors. More useful: a monad is an algebraic theory, and its algebras are the models.

**Kleisli is the INITIAL resolution; Eilenberg-Moore is the TERMINAL one.** The Kleisli category is the full subcategory of free algebras. Getting this direction backwards is common; the practical shadow is that Kleisli composition is "programs with effects" while Eilenberg-Moore is "structures that can absorb the effect."

**The single highest-value practical consequence in this whole file:** *applicative versus monad is the parallel versus sequential distinction, in the types.* `Applicative` combines independent effects; `Monad` sequences dependent ones. If your code `await`s three independent requests one at a time, you used a monadic interface where an applicative one was available, and that is a **measurable performance bug** the vocabulary lets you see before the profiler does.

### Initial algebras and final coalgebras

The real explanation for ADTs and streams, and the one most people never get.

- An **inductive type is an initial algebra** of a functor. `List a` is initial for `X ↦ 1 + a × X`. Initiality gives you the **fold** (catamorphism) -- the unique map out. That uniqueness is why fold-based APIs are canonical rather than one option among many.
- A **coinductive type is a final coalgebra** of the same shape. Streams are final for `X ↦ a × X`. Finality gives you the **unfold** (anamorphism) -- the unique map *in*.
- **Data versus codata:** data is defined by how it is constructed and consumed by folding; codata is defined by how it is observed and produced by unfolding. Finite lists are data; infinite streams are codata. This distinction explains why total languages accept infinite structures (productive corecursion) but reject unbounded recursion.

Recursion schemes (cata, ana, hylo, para) are the systematic exploitation of this. In a mainstream language the payoff is modest -- usually just "write the fold, get the structure right" -- but it explains why.

### Yoneda

The Yoneda lemma: `Nat(Hom(a, −), F) ≅ F a`. An object is determined by its maps out.

**"Yoneda is just CPS" is true at the identity functor and understates it in both directions.** CPS *is* the Yoneda embedding specialized to `F = Id`: `∀r. (a → r) → r ≅ a`. Difference lists are the same move for monoids. Profunctor optics are a coend calculus over it.

**Critically: that isomorphism holds only because parametricity supplies naturality.** In a language without parametricity, `∀r. (a → r) → r ≅ a` is **false** -- the callback can inspect `r` and misbehave. So the CPS/Yoneda correspondence is parametricity-dependent, and §3 is where that gets fragile.

### Optics

Profunctor optics are a **coend**; the van Laarhoven and profunctor encodings are equivalent, with Theorem 4.14 proved by Double Yoneda. The optic-to-monoidal-action table is the systematic account (lens ↔ product, prism ↔ coproduct, traversal ↔ applicative).

**Practical caution:** optics earn their keep in a language with poor update ergonomics. In TypeScript with Immer, or Rust with `&mut`, the problem optics solve has already been solved more cheaply. See §6's cargo-cult list.

### Kan extensions

Mac Lane: "all concepts are Kan extensions" (CWM ch. X §7 -- verified). Practically they appear as the general form behind `Codensity`, free-monad performance improvements, and right/left Kan extensions used to make certain constructions computationally efficient. Worth knowing the name; rarely worth reaching for directly.

---

## 3. Type theory proper

### System F and parametricity

Reynolds' abstraction theorem, popularized by Wadler's *Theorems for Free!*: a parametrically polymorphic type constrains its inhabitants so severely that you can derive theorems from the signature alone. `∀a. a → a` has exactly one inhabitant (identity). `∀a. [a] → [a]` must produce a permutation of a sub-selection of its input; it cannot invent elements.

**Parametricity is a special case of naturality.** For types of the shape `∀a. F a → G a`, the free theorem *is* the naturality square.

**The correction that matters:** plain **dinaturality is not equivalent to parametricity and does not compose** -- Church numerals are the standard counterexample. **Strong dinaturality** is the correct notion for the general case. If you see "parametricity is just naturality" stated without qualification, that is the gap.

### Where parametricity breaks in real languages

This section matters more than the theory, because your code lives here.

**Haskell's `seq`** is the canonical case, and it is documented by Wadler himself as a co-author of *A History of Haskell*:

> "seq weakens the parametricity property that polymorphic functions enjoy, because seq does not satisfy the parametricity property for its type `∀a,b. a → b → b`"

The design fight is recorded: Launchbury argued parametricity was too important to give up; Hughes argued `seq` had to work at any type, including type variables; the paper says "these two goals are virtually incompatible." Haskell 1.3 had an `Eval` class to *track* `seq` in types, and it was killed by real-world pain -- Hughes's students' TCP/IP stack, where "each insertion of a `seq` became a nightmare" as signatures cascaded.

The authors' own verdict:

> "We have sacrificed parametricity in the interests of programming agility and (sometimes dramatic) optimisations. GHC still uses short-cut deforestation, but it is unsound... Haskell's designers love semantics, but even semantics has its price."

It is in the Haskell 2010 Report (§6.2). Nobody contests it.

**The precise mechanism, and why the obvious fix fails.** Johann and Voigtländer (POPL 2004), verbatim: *"Contrary to the folklore... not even quantifying only over strict and bottom-reflecting relations... is sufficient."* The failure is that **eta-reduction dies**, so `(∀x. a x = b (g x)) ⟺ a = b ∘ g` survives in only one direction.

**Andrej Bauer's "Hask is not a category"** is the same hole viewed from the category-law side: `f . id = f` is false in Haskell, so the *first* category axiom fails. This is the rigorous version of a complaint usually made loosely.

**Rust.** The drop checker relied on a parametricity assumption, and specialization invalidated it. RFC 1238 (Felix Klock, 2015) exists to remove that reliance, verbatim: *"The parametricity-based reasoning in the [Drop Check analysis] was clever, but fragile and unproven... parametricity is a necessary but not sufficient condition to justify the inferences that dropck makes."*

Sharp detail: the specialization RFC (1210) contains **zero** occurrences of "parametric" -- the framing arrived afterward, via RFC 1238 and community argument.

Two camps, **less opposed than they look**:
- Diggsey (2016): *"rust has never had runtime parametricity, and downcasting does not break compile-time parametricity (which rust has also explicitly opted out of with specialization)."*
- Fylwind (2017): *"types in Rust are not parametric with the introduction of specializations"*

The first means *runtime* parametricity (`Any`, downcasting); the second means *compile-time*. And Diggsey's own phrasing concedes Rust "explicitly opted out" -- gave it up deliberately, rather than never having had it.

pcwalton on lifetime-dependent specialization: *"I would be pretty terrified if the particular lifetime the borrow check assigned could cause arbitrary different code to be executed at runtime."*

**A camp that does not exist:** one would expect "Java/Scala reflection breaks parametricity" to be a live debate. It is not. Erasure is documented and uncontroversial (Wadler's own GJ page: *"GJ is translated by erasure: no information about type parameters is maintained at run-time"*), but nobody argues about it in free-theorems terms. Do not manufacture this disagreement.

**Two scope limits to respect:**
- The imprecise-exceptions work targets **referential transparency and transformation validity**, not Reynolds parametricity. Cite it for equational reasoning; the parametricity link is inferred.
- *"Fast and Loose Reasoning is Morally Correct"* (Danielsson, Hughes, Jansson, Gibbons) covers **totality and `⊥` only**. It does **not** extend to `seq`, exceptions, or effects, and cannot be used to wave away the hole above.

### Dependent type theory

- **MLTT**: dependent functions `Π`, dependent pairs `Σ`, identity types `Id`, universes.
- **CIC** (Rocq/Coq's basis) versus **Lean's theory**: both have impredicative `Prop`, but they differ in proof irrelevance, quotient handling (Lean's `Quot.sound` is an axiom), and definitional-equality details. They are not interchangeable, and tutorials transfer badly between them -- see `[[lean-proof-engineering]]` §5 for the `n + 0` polarity reversal that catches everyone.
- **Judgmental versus propositional equality.** Judgmental equality is decided by the checker (`rfl` works); propositional equality is a type you must inhabit. Which equations are judgmental is an artifact of how definitions were written, not a mathematical fact. This distinction causes more practical pain than any other in dependently-typed programming.
- **Universes.** `Type : Type` is inconsistent (Girard's paradox), forcing a hierarchy. **Predicativity is settled design, not a live debate** -- the paradox closes the space.
- **Impredicative `Prop`** lets you quantify over all propositions to form a proposition. Useful; constrains what else the system can have (large elimination is restricted).

### Intuitionistic versus classical

Constructively, `A ∨ ¬A` and `¬¬A → A` are not available. The cost: no proof by contradiction for existence claims, so an existence proof carries a witness. The benefit is exactly the same thing -- proofs compute.

The Gödel-Gentzen translation embeds classical logic into intuitionistic, so classical reasoning is available at the cost of double-negation bookkeeping. In practice, most proof assistants let you assert excluded middle (`Classical.choice` in Lean), which makes some proofs vastly easier and makes the resulting terms non-computational. That trade is explicit and auditable.

---

## 4. Homotopy type theory, honestly

Identity types as paths; univalence (`(A ≃ B) ≃ (A = B)`); higher inductive types; cubical type theory as the computational interpretation.

**VOLATILE** (2026-09-18). **What it delivered:** univalence as a theorem rather than an axiom in cubical systems, computing higher inductive types and quotients, the first ∞-categorical Yoneda lemma formalized. Real results.

**What it did not deliver:** a replacement foundation. Thirteen years, one book edition, no conference since 2023, and no scheme or number field formalized in any HoTT system.

**Current friction, and it is severe:** a canonicity-failure bug open roughly four years (fixed 2026-09-06); error messages that required ~20GB of RAM to display; Kan solving undecidable in general; and two specialist groups building competing implementations over performance. **The 1Lab has left Agda for its own fork (Mikan)** -- corroborated from both sides, with an Agda commit dated 2026-09-18 reading "remove ad for 1lab."

**Why an engineer might still care:** transport along equivalences (change your representation, carry your proofs), and quotient types that actually compute. Both are things you want and mostly cannot have.

**Note for `[[lean-proof-engineering]]` users:** asserting univalence in Lean 4 is **inconsistent**, not merely unprovable, and a fresh identity type in `Type` does not rescue it.

---

## 5. Linear and substructural types

Drop a structural rule, get a discipline:

| Rule dropped | Discipline | Meaning |
|---|---|---|
| none | ordinary | use freely |
| contraction | **affine** | use at most once |
| weakening | **relevant** | use at least once |
| both | **linear** | use exactly once |
| exchange | ordered | use in order |

**The framing that lands for a Rust engineer:** `Copy` is the **opt-in reinstatement of contraction**; implicit `drop` is the **ambient weakening**. Rust is therefore affine, not linear.

**Why the difference ships bugs.** Affine permits discarding, so Rust cannot guarantee a destructor runs. `mem::forget` is safe. The **leakpocalypse** (`JoinGuard`, pre-1.0) is the canonical case: an API whose soundness depended on `Drop` running was unsound and had to be redesigned.

**The sharpest statement of the gap:** *typestate enforces protocol safety, never protocol completion.* You can prevent "send after close." You cannot force "eventually close." **That gap is exactly the affine/linear gap** -- and it is the best one-sentence explanation of what linear types would add to Rust.

**VOLATILE and a live disinformation warning** (2026-09-18): **Rust has no linear types.** A search-visible blog post claims Rust 1.95 stabilized them via a `MustMove` trait. It is **fabricated** -- wrong date, and the official 1.95.0 announcement (2026-04-16) contains no such feature. Verify language-feature claims against release notes only.

**Quantitative type theory** (Idris 2) generalizes: each binding carries a multiplicity (0 = erased, 1 = linear, ω = unrestricted). Multiplicity 0 is the elegant solution to "types that exist only at compile time."

**Session types** encode communication protocols; their natural home is linear logic, because a channel endpoint must be used exactly once per protocol step. This is the principled account of what a protocol state machine is.

**Graded modal types** generalize multiplicities to arbitrary semirings (security levels, differential privacy budgets, sensitivity).

---

## 6. What this actually buys in mainstream code

| Idea | Cash value in TS / Rust / Swift / Kotlin |
|---|---|
| Sum types | Make illegal states unrepresentable. The single highest-yield move. |
| Exhaustiveness | Case analysis checked by the compiler; adding a variant surfaces every site |
| `Void` / `Never` | Prove unreachability; exhaustiveness assertions |
| Parametricity | API boundary as a **security property**: what a function cannot see, it cannot depend on |
| Phantom / branded types | Compile-time distinctions with zero runtime cost (`UserId` vs `OrderId`) |
| Typestate | Protocol safety in the type (`Connection<Open>` vs `Connection<Closed>`) |
| GADTs / discriminated unions | Tag-refined types; the branch knows the payload shape |
| Smart constructors | The cheap rung of refinement; validate once at the boundary |
| Applicative vs monad | **Parallel vs sequential, visible in the type.** Catches real performance bugs |
| Initial algebras | Why fold-based APIs are canonical rather than arbitrary |
| Universal properties | "What is the unique map out of this?" as a design question |
| Adjunctions | Spotting that a "best" construction exists and is unique |
| Totality discipline | No `head` on empty; every guarantee above depends on this |

### The honest inverse: eight named cargo cults

Where importing this vocabulary makes code **worse**:

1. **HKT-emulating typeclass hierarchies in TypeScript.** The type system cannot express higher-kinded types; the encodings are elaborate, inference-hostile, and unreadable to your team.
2. **Monad transformers** in a language without do-notation and inference to support them. The stack becomes the architecture, and error messages become unreadable.
3. **Free monads as application architecture.** An interpreter layer you will maintain forever to gain testability that dependency injection already gave you.
4. **Optics where `Immer` or `&mut` already won.** Solving an update-ergonomics problem the language solved.
5. **Type-level arithmetic in TypeScript.** Compile times collapse; the guarantee is usually not worth it.
6. **Categorical names as documentation.** Calling something a `Profunctor` explains nothing to a reader who does not already know, and the name is not the abstraction.
7. **Point-free style** past the point of legibility. Composition is a tool, not a virtue.
8. **A userland `Task`/`IO` type in a language that already has `async`.** You are rebuilding the runtime's abstraction with worse tooling and worse stack traces.

**The general rule:** the value is in the *structure* the idea identifies, not the *vocabulary*. If the structure is already enforced by the language, importing the vocabulary is cost with no benefit.

---

## 7. The escalation ladder, with honest costs

The ladder, cheapest first. **Stop at the lowest rung that solves the problem.**

1. **ADTs / sum types** -- free, in any language with them.
2. **Smart constructors** -- validate at the boundary, keep the invariant by construction.
3. **Phantom / branded types** -- compile-time tags, no runtime cost. *Caveat: forgeable by a cast unless the constructor is private.*
4. **Typestate** -- protocol state in the type. Safety, not completion (§5).
5. **GADTs / refined discriminated unions** -- the branch knows the payload.
6. **Refinement types** (LiquidHaskell, F\*, Flux) -- SMT-decidable predicates. Real automation; the solver is in your TCB.
7. **Dependent types** -- arbitrary value-indexed propositions. You are now writing proofs.
8. **Full functional verification** -- see `[[lean-proof-engineering]]` and `[[separation-logic]]`.

### The cost evidence, and why the number you have heard is wrong

**"Formal verification costs 10-100x" is folklore.** No primary source states a measured effort multiplier. This was checked exhaustively against primary PDFs, and the finding is unambiguous.

**Two different quantities get conflated, and that conflation is the origin of the myth:**

- **Proof-to-code LINE ratio** -- well documented, genuinely large: HACL\* 2:1, EverCrypt 3.1:1, Cedar 3.4:1, IronFleet 3.6:1, Ironclad 4.8:1, CompCert 6:1, VeriBetrKV 7:1, seL4 23:1 (2009) rising to 55:1 (2014). **Cite this confidently.**
- **Effort multiplier versus the same software unverified** -- barely measured anywhere. Every verifiable figure is **≤3.3x**, and several are *negative*.

**seL4's own authors compute 3.3x** (18 person-years of proof against L4Ka::Pistachio's 6 person-years of comparable development), and argue verification was **cheaper** than the EAL6/EAL7 certification alternative: *"formally verified software is actually less expensive than traditionally engineered 'high-assurance' software."* Their redo estimate is 8 py total, *"only twice the SLOCCount estimate"* for an unassured system.

**The famous "$10k/LOC for EAL6" figure is an acknowledged typo.** TOCS 2014, footnote 3: the 2009 paper *"contained an embarrassing typo, claiming $10k/LOC."* The corrected figure is $1k/LOC. A 10x error sat for five years in the most-cited cost paper in the field, and the wrong version is the one people quote. Neither figure is cited to any source.

**The cleanest specimen of the conflation in the wild:** ShardStore (SOSP 2021) says its artifacts are *"an overhead that compares favorably to formal verification approaches that report 3-10× overhead."* That "3-10×" is a **line count**, in a peer-reviewed venue, phrased so it reads exactly like a cost figure.

**Counter-evidence runs the other way, repeatedly:**
- **SHOLIS** (King, Hammond, Chapman, Pryor, *IEEE TSE* 26(8), 2000) -- the best controlled comparison available. Proof found **50** specification faults against test-case generation's **4**, same period, comparable effort, third-party team on the testing side. Authors' caveat: *"not a scientific experiment."*
- **Tokeneer** (NSA, commissioned explicitly as an affordability demo): 10K lines, 260 person-days, **zero** defects found in independent assessment, 38 SLOC/day overall.
- **MULTOS CA**: 0.04 defects/KLOC at ITSEC E6 (≈EAL7), 28 SLOC/day, and the authors' claim that *"a process that achieves normal commercial productivity can deliver a highly reliable system."*
- **AWS TLA+**: engineers *"from entry level to Principal"* productive in **2-3 weeks**, and the investment was *"less time consuming"* than the informal proofs it replaced.
- **Rockwell Collins**: 10x running *backwards* -- 300+ hours/instruction on first PVS use, dropping *"by almost an order of magnitude"* on the second.
- **Woodcock et al.** (62 industrial projects): *"five times as many projects reported reduced costs as those that reported increased costs."* Quality improved in 92%, worsened in 0%.

**The honest concession, from inside the pro-verification data:** SHOLIS was the only Praxis project with full functional code proof, and it has the **worst** productivity in their table (7.0 SLOC/day, a quarter of MULTOS). MULTOS reached 0.04 defects/KLOC **without** functional code proof (*"We did not perform proof of partial correctness of the code"*). **The productivity story is carried by the lighter formality -- specification plus static analysis -- not by proof.**

**Three caveats that keep this honest in the other direction:**

1. **Selection bias.** Favorable numbers come from expert teams on small high-value cores. Woodcock's sample is admittedly biased toward successes; Praxis published five hand-picked projects with no failures shown.
2. **The absolute scaling problem is real, and it is the honest version of what the folklore garbles.** Ironclad: *"assuming ~2000 verified LOC per person-year, a fully verified million-LOC project would still require ~100s of person-years."* That is an absolute-throughput objection, not a multiplier.
3. **"Cheaper than what."** For DO-178B and Common Criteria cases, the baseline is *other certified development*, not ordinary commercial development. **"Cheaper than certifying by test" is far narrower than "cheaper than building software normally,"** and the literature slides between them. The certification side runs on folklore too: Amey's *"factor of five"* for DO-178B Level A is another uncited factor-of-N.

**Why no constant multiplier can exist:** Matichuk et al. (ICSE 2015), across 15,018 lemmas and ~215,000 proof lines, find a **quadratic** relationship between formal statement size and final proof size, with effort linear in proof size. The cost is scale-dependent by construction, and varies 2:1 to 6:1 *within* HACL\* alone.

**What to actually say:** cost tracks **how much formality you apply and how automated it is**, not formality as such. Quote throughput (~2,000 verified LOC/person-year at the top of the ladder), not a multiplier.

---

## 8. Schools of thought

### Does category theory earn its keep for working programmers?

**Pro** (Milewski, Wadler, Conal Elliott): it identifies structure that is already there, names it so you can reuse theorems, and prevents reinventing lawless versions of lawful things. Wadler's *Propositions as Types* is the case that this is not decoration but the actual foundation.

**Con, and the strongest version comes from the friendliest witness:** **Wadler himself concedes that "no knowledge of category theory is required" to use the methods it discovered.** The vocabulary was the *discovery* tool, not the *usage* tool. A working programmer can use `map`, `fold`, and `Result` correctly forever without knowing what a functor is.

**Bauer's contribution** sharpens the con: "Hask" is not even a category, so the claim that Haskell *is* categorical is false in the literal sense. The structures are approximations.

**Note on adjacent positions this research could NOT source:** several expected critics (Diehl, McKenna, Armstrong on this specific point, Riehl's position) could not be verified. Do not manufacture their arguments.

### Constructive versus classical in verification practice

**Constructive:** proofs compute, existence proofs carry witnesses, and the result can be extracted as a program.
**Classical:** vastly easier for many statements, and most real developments assert excluded middle anyway. The non-computational consequence is explicit and auditable.
**Settled adjacent question:** predicativity. Girard's paradox forces the design; this is not a live debate.

### Did HoTT deliver?

**Yes:** univalence as a computing theorem, HITs and quotients that compute, ∞-categorical results formalized.
**No:** thirteen years, no replacement foundation, no scheme or number field formalized, severe tooling friction, specialists forking implementations over performance.
**Both are true.** It is a successful research program and an unsuccessful foundation-replacement program.

### Does parametricity survive contact with real languages?

**No, demonstrably** -- `seq`, specialization, reflection, exceptions (§3).
**But it survives usefully** -- the free theorems are true in the fragment you normally write, and "fast and loose reasoning is morally correct" for the totality-and-`⊥` part specifically (not for `seq`).
**Not a symmetric disagreement:** nobody claims parametricity is unbroken. The disagreement is over whether the breakage matters in practice, and it matters exactly where you rely on it for *security*, not merely for reasoning.

### Correcting two positions people expect to find

**Harper is not anti-category-theory** -- he authored computational trinitarianism, whose thesis is that logic, computation, and category theory are three views of one thing ("There is no preferred route to enlightenment"). **Buzzard is not a type-theory critic** -- he is a Lean partisan whose criticism targets ZFC's lack of universes and whose polemic targets constructivism. Attributing the opposite view to either is a common error.

---

## 9. Anti-pattern catalog

| Pattern | Trigger | Consequence | Fix |
|---|---|---|---|
| **Importing a typeclass hierarchy the type system cannot express** | Haskell habits in TypeScript | Inference-hostile, unreadable, team cannot maintain | Plain generics and concrete types |
| **"Monad" as a vibe** | Naming anything with a `flatMap` a monad | Laws unchecked; refactors break silently | Verify the laws or drop the name |
| **Lawless instances** | Implementing an interface to satisfy a signature | Every generic function over it is now wrong | Test the laws with property-based tests |
| **Phantom type a cast can forge** | Branded type with a public constructor | The invariant is documentation, not enforcement | Private constructor; smart constructor at the boundary |
| **Assuming parametricity where the language breaks it** | Security argument resting on `∀a` | `seq`, specialization, reflection, or downcasting defeats it | Know your language's escape hatches (§3) |
| **Dependent types where a runtime check is honest** | Reaching for rung 7 | Enormous cost, invariant was cheap to check | Walk up from rung 1 |
| **A categorical name as an explanation** | `Profunctor` in a doc comment | Explains nothing to anyone who does not already know | Describe what it does |
| **Quoting "10-100x" for verification cost** | Any cost discussion | Repeats a conflation of line ratio with effort | Line ratio 2:1-55:1; effort ≤3.3x; throughput ~2,000 LOC/py |
| **`n + 0` reasoning transferred between Rocq and Lean** | A tutorial from the other system | The polarity reverses; the proof does not typecheck | See `[[lean-proof-engineering]]` §5 |
| **Sequential `await` on independent effects** | Three unrelated fetches in a row | Measurable latency bug | Applicative combination (`Promise.all`, `join!`) |
| **Total-function claims in a partial language** | Reasoning as if `∀a. a → a` is identity | `undefined`, exceptions, and nontermination inhabit everything | Totality is a discipline you enforce, not a given |
| **Citing a fabricated language feature** | A blog claiming Rust has linear types | Confidently wrong advice | Official release notes only (§5) |

---

## 10. Authorities, with readability tiers

**Readable by an engineer, start here:**
- **Wadler, *Propositions as Types*** (paper and talk) -- the single best entry point to the whole subject.
- **Wadler, *Theorems for Free!*** -- parametricity, short, concrete.
- **Milewski, *Category Theory for Programmers*** -- the standard programmer's route in. Free online.
- **Harper's computational trinitarianism** essays -- the three-way correspondence stated directly.
- **Pierce, *Software Foundations*** -- interactive, Rocq-based, genuinely pedagogical.

**Requires real mathematical maturity:**
- **Lambek and Scott, *Introduction to Higher Order Categorical Logic*** -- the source for the correspondence's categorical half.
- **Mac Lane, *Categories for the Working Mathematician*** -- the reference. Ch. X §7 for Kan extensions.
- **Riehl, *Category Theory in Context*** -- modern, rigorous, free. (Her position on programming applications could not be sourced here.)
- **Awodey, *Category Theory*** -- the standard bridge text.
- **Martin-Löf's original papers** -- MLTT from the source.
- **The HoTT Book** -- collaborative, excellent, and a research program rather than a tutorial.

**Reference:**
- **Harper, *Practical Foundations for Programming Languages*** -- the comprehensive modern reference on type theory as such.
- **Pierce, *TAPL*** -- still the canonical type-systems textbook.

**People:** Wadler (parametricity, propositions-as-types), Reynolds (abstraction theorem, System F), Martin-Löf (MLTT), Girard (System F, linear logic), Lawvere (quantifiers as adjoints, functorial semantics), Lambek (the categorical correspondence), Harper (trinitarianism, PFPL), Milewski (programmer-facing exposition), Conal Elliott (denotational design), Bauer (the "Hask" critique), Johann and Voigtländer (the `seq` mechanism).

**Route in:** *Propositions as Types* → *Theorems for Free!* → Milewski's first six chapters → then stop unless you have a specific need. Most of the practical value is in the first two.

---

## 11. Severity rubric (this domain)

- **blocker** -- A guarantee claimed that the language does not deliver, with a reachable exploit: a security boundary resting on parametricity in a language with reflection or specialization; a branded type a public cast forges; an invariant asserted in a phantom parameter and violated at the construction site.
- **major** -- Escalation-ladder error with real cost: a dependent-type or verification approach where rung 1-3 solves it; or an invariant left to convention where a sum type would enforce it. Lawless instances used by generic code. Illegal states representable in a domain model where the enumeration is small and known.
- **minor** -- Sequential composition where applicative is available (unless the latency is irrelevant). Recursion schemes in a codebase with no other categorical vocabulary. Phantom types where a newtype suffices.
- **nit** -- Naming that invokes categorical vocabulary without payoff. Point-free style past legibility.
- **insight** -- "This is an initial algebra, which is why the fold is canonical and your three ad-hoc traversals should be one." "The invariant you are documenting is a sum type." "This is the affine/linear gap: you can prevent misuse but not enforce completion." "Cost here tracks statement size quadratically, so splitting the property is the lever."

---

## Source research

`~/.claude/local/research-notes/type-theory-foundations-research.md` (~2,160 lines; 194 VERIFIED / 25 FOUND-UNVERIFIED / 38 INFERRED, with a headline-corrections list at the top) and `~/.claude/local/research-notes/formal-verification-cost-dossier.md` (the §7 evidence, verbatim from primary PDFs run through `pdftotext`).

Gaps recorded and deliberately not filled: the precise 2-categorical statement of Lambek's theorem; Bishop's polemical quotes; Riehl's position on programming applications (403); Armstrong and Diehl unsourced; the Mikan fork's stated rationale; Colbert & Boehm 2008 (host does not resolve -- and note it is a **COCOMO model output, not measured data**, which weakens one side of seL4's own comparison); Behm et al. FM'99 at full primary strength (the Paris Métro figures are citable only at one remove via Woodcock citing Abrial 2007).

## Changelog

- **2026-09-18** -- File created. Category theory included as a first-class leg per user request. §7 cost account replaces the "10-100x" folklore that the retired `fp-verification` agent asserted.
