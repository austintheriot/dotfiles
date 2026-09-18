---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-18
---

# Separation logic and ownership reasoning

**Thesis.** Separation logic is the formal account of the thing every engineer already wants to be true and usually cannot prove: *I can read this function in isolation.* Its central contribution, the frame rule, is the license to do that, and its precondition is the license's price -- a precondition in separation logic is not a constraint on inputs, it is a **claim of ownership over a footprint**. Everything else in the field follows from taking that reading seriously: fractional permissions are ownership divided, Iris's resource algebras are ownership generalized past the heap, and Rust's borrow checker is ownership enforced by a compiler instead of a prover.

**The agent's operational question:** *who owns this memory, for how long, and what does the code assume about aliasing that nothing enforces?*

**Empirical priority order.** What actually bites:

1. **Unstated aliasing assumptions in `unsafe` code.** An informal ownership argument in a comment, or none at all.
2. **Delivery, not logic.** Whether the analysis runs where a developer will act on it. The largest measured effect in the entire field is about timing, not expressiveness (see §6).
3. **Tool-guarantee confusion.** Treating Miri-clean as sound, an SMT result as a foundational proof, or a model's verification as the implementation's.
4. **Ownership transfer that is never completed.** Affine types permit leaks; typestate enforces protocol *safety*, never protocol *completion*.
5. **The wrong primitive.** Reaching for a prover where a model checker, Miri, or Loom answers the question in an afternoon.

---

## Volatile surface

`last-verified` in the frontmatter. The logic is durable; the tooling and the Rust aliasing model are not.

| Claim class | Rots | Re-verify at |
|---|---|---|
| Miri's default aliasing model | Medium | `rust-lang/miri` `src/eval.rs`; the README |
| Rust memory model / opsem status | Medium | The opsem team repo and Unsafe Code Guidelines |
| Tool maintenance status | Fast | `gh api` the repo; check last push and open-issue count |
| Iris version and cadence | Annual | `gitlab.mpi-sws.org/iris/iris`; iris-project.org |
| Infer / Pulse deployment status | Medium | `facebook/infer` `infer/src` contents |
| Verify-Rust-Std approved tool list | Medium (governed, revocable) | `model-checking.github.io/verify-rust-std/tools.html` |
| The logic itself, frame rule, classic papers | Durable | Does not rot |

---

## 1. The core logic

### Why Hoare logic fails on the heap

Hoare's **rule of constancy** says an assertion about untouched state survives a command. On a heap with aliasing it is **false**. Reynolds' counterexample, verbatim:

```
{x ↦ − ∧ y ↦ 3}  [x] := 4  {x ↦ 4 ∧ y ↦ 3}
```

This fails when `x = y`. The conjunction `∧` says both assertions hold *of the same heap*, and nothing in it says the two cells are distinct. Without a way to say "distinct," every function's specification must be re-derived at every call site -- which is exactly what makes a codebase with unrestricted aliasing un-reviewable.

### The separating conjunction

`p ∗ q` holds of a heap that **splits** into two disjoint parts, one satisfying `p` and the other `q`. Disjointness is built into the connective rather than asserted alongside it.

The points-to predicate `e ↦ e'` means the heap has **exactly** the one cell:

```
dom h = {⟦e⟧s}   and   h(⟦e⟧s) = ⟦e'⟧s
```

**That is exact equality of the domain, not containment.** This is the most common beginner error in the whole subject. `x ↦ 3` does not mean "the heap contains a cell at `x` holding 3"; it means "the heap **is** that one cell." Ownership of a bigger heap is written by starring things together.

Consequently `x ↦ 3 ∗ y ↦ 4` *entails* `x ≠ y`, for free, with no side condition.

### The frame rule

The central contribution:

```
        {p} c {q}
─────────────────────────      where no variable free in r is modified by c
   {p ∗ r} c {q ∗ r}
```

The one side condition is about **the store (program variables), not the heap**. This is routinely garbled. The heap side needs no condition precisely because `∗` already guarantees disjointness; only the mutable program variables need protecting, since they are not tracked by the heap assertions. *(Iris, which has no mutable program variables in that sense, has a frame rule with **no side condition at all**: `{P} e {Q} ⊢ {P ∗ R} e {Q ∗ R}`.)*

**Why it is sound, and this is the part worth internalizing.** Separation-logic triples are **fault-avoiding**: `{p} c {q}` asserts not merely partial correctness but that `c` *does not go wrong* when started in any heap satisfying `p`. Reynolds quotes O'Hearn paraphrasing Milner: *"Well-specified programs don't go wrong."* Combined with safety monotonicity (a command that runs safely on a small heap runs safely and identically on a larger one, touching only the small part), the precondition becomes a genuine **upper bound on the footprint**.

**Fault-avoidance is what turns a precondition into an ownership claim.** Without it the frame rule is false -- which is exactly why the rule of constancy fails, since Hoare triples are not fault-avoiding.

Reynolds on what this buys, verbatim:

> "To understand how a program works, it should be possible for reasoning and specification to be confined to the cells that the program actually accesses."

O'Hearn's term for that set of cells is the **footprint**. A "small axiom" specification written against the smallest heap a function needs is automatically valid in every larger context.

### The magic wand

`p −∗ q` holds of a heap `h` when, for any disjoint `h'` satisfying `p`, the combined heap satisfies `q`. Read it as "a `q` with a `p`-shaped hole."

Three real uses: expressing the residual you hold after handing ownership away; the weakest precondition of the frame rule (the strongest thing you could frame off); and ownership-transfer protocols, where a lock's resource invariant is handed to a thread and the wand encodes "give this back and you may proceed." The continuation/hole idiom dominates in practice.

**Anti-pattern:** writing `−∗` where `⇒` suffices. When `p` is pure (heap-independent), `p −∗ q` is a more expensive `p ⇒ q` dragged through a frame quantifier for nothing. The wand earns its keep only when the antecedent owns heap.

### Not linear logic

Reynolds **explicitly denies** that separation logic is linear logic. The real parent is **BI** (the logic of Bunched Implications, O'Hearn and Pym), which has both an additive and a multiplicative conjunction side by side. The resemblance to linear logic is real but the lineage claim is wrong, and repeating it obscures why BI's two conjunctions are the point.

### History

Burstall's 1972 prehistory; Reynolds' and O'Hearn's papers 2001-2002; **Gödel Prize 2016** to O'Hearn and Brookes, awarded specifically for **concurrent** separation logic.

---

## 2. Concurrent separation logic

O'Hearn's disjoint-concurrency rule: if two threads operate on disjoint heaps, their specifications compose.

```
  {p₁} c₁ {q₁}     {p₂} c₂ {q₂}
────────────────────────────────────
   {p₁ ∗ p₂}  c₁ ∥ c₂  {q₁ ∗ q₂}
```

This made concurrent-program verification tractable, and it is why the Gödel Prize cites CSL rather than the sequential logic.

**Resource invariants** handle the shared case: a lock owns an invariant, acquiring transfers ownership in, releasing transfers it back out.

**Fractional permissions** (Boyland) split ownership into shares `q ≤ 1`. A full permission `1` allows writes; any fraction allows reads; fractions recombine to a full permission.

**This is literally Rust's aliasing-XOR-mutability rule expressed as arithmetic.** `&mut` is permission `1`; `&` is a fraction. RustBelt's lifetime tokens `[κ]_q` are themselves fractional. If you understand `&`/`&mut`, you already understand fractional permissions; the logic just makes the arithmetic explicit and lets you write down protocols the borrow checker cannot express.

**Rely-guarantee** is the other tradition (interference specified as a relation rather than ownership as a resource). RGSep and LRG are the unification attempts. Worth knowing the split exists; ownership won in practice.

---

## 3. Iris

The dominant modern framework: a higher-order concurrent separation logic, implemented in Rocq.

**VOLATILE** (2026-09-18): Iris **4.5.0**, released 2026-03-05, repository active 2026-09-17, roughly annual cadence. Sixth Iris Workshop June 2026. Maintained by the Aarhus Logic and Semantics group with MPI-SWS. Documentation has moved to **Rocq** terminology.

**The key generalization.** Reynolds flagged it himself in 2002, verbatim:

> "A more precise name might be storage separation logics, since it is becoming apparent that the underlying idea can be generalized to describe the separation of other kinds of resources."

That sentence is the seed of Iris. Replace "heap" with **any partial commutative monoid of resources** and `∗` keeps working. Iris's **resource algebras** (cameras) are that generalization made user-definable: you define your own notion of resource, Iris gives you separation logic over it.

**Ghost state** is resource that exists only in the proof. It is how you track protocol state, monotone counters, authoritative-fragment ownership patterns, and anything else the program does not physically store.

**Invariants and the later modality.** Higher-order references (a heap cell holding a predicate about the heap) create a circularity that naive semantics cannot support. Iris uses **step-indexing**, exposed in the logic as the later modality `▷`. The practical cost: you often have a `▷ P` when you need `P`, and you discharge it by taking a program step. Most beginner friction in Iris is `▷` friction.

**Weakest preconditions** rather than triples as the primitive. The **Iris Proof Mode (IPM)** gives you a context of separation-logic hypotheses with tactics that manipulate them like ordinary Rocq hypotheses -- this is what made the framework usable by people who are not its authors, and it is a genuine HCI contribution, not only a technical one.

**Built with Iris:** RustBelt, RefinedC, Perennial (crash safety), Simuliris (refinement), RustHornBelt.

**People:** Ralf Jung, Derek Dreyer, Lars Birkedal, Robbert Krebbers, Jacques-Henri Jourdan, Aleš Bizjak.

---

## 4. RustBelt and the formal account of Rust

**What RustBelt proved.** Rust's `unsafe` blocks mean the type system's guarantees cannot be checked syntactically. RustBelt's approach is **semantic typing**: give each type a semantic interpretation (a separation-logic predicate over what owning a value of that type means), then prove each `unsafe` abstraction actually inhabits its type's interpretation. If it does, it can be linked with any well-typed safe code without breaking anything.

**Verified libraries** (verbatim from the paper): `Arc`, `Rc`, `Cell`, `RefCell`, `Mutex`, `RwLock`, `mem::swap`, `thread::spawn`, `rayon::join`, `take_mut`.

**Honest scope caveats.** RustBelt verifies **λRust**, a formal model, not `rustc`. It verifies **ports** of these libraries, not the shipped `std` source. The result is "the design of these abstractions is sound," not "the shipped bytes are correct."

**The engineer's takeaway.** `Send` and `Sync` are **not annotations**. They are claims about a sharing predicate. Writing `unsafe impl Send for T` is **asserting a theorem** -- that it is safe to move `T` across threads -- and RustBelt is what makes that sentence literal rather than metaphorical. The same holds for every `unsafe` block: you are discharging a proof obligation, and the only question is whether you wrote the proof down.

**Lifetime logic** is RustBelt's account of borrowing: lifetime tokens (fractional, §2), borrows as a resource, and the reborrowing discipline.

### Stacked Borrows, Tree Borrows, and what Miri actually checks

**VOLATILE and frequently misreported** (2026-09-18): **Miri still defaults to Stacked Borrows.** Verified three ways, including `rust-lang/miri` `src/eval.rs:180` reading `borrow_tracker: Some(BorrowTrackerMethod::StackedBorrows)`. Tree Borrows is available as `-Zmiri-tree-borrows` and is described in the README as an "optional alternative" that "replaces" the default.

This has **not** moved despite Tree Borrows winning a **PLDI'25 Distinguished Paper** (Villani, Hostert, Dreyer, Jung), rejecting 54% fewer programs across 30,000 crates and enabling read-read reordering.

**Miri's own stated reason is decisive, and quote it to anyone tempted to flip the flag:** the eventual official model *"will be stricter than Tree Borrows,"* so code that is Tree-Borrows-clean *"might be declared UB in the future."* Passing under the looser model is not evidence of soundness under the model Rust will actually adopt.

**Rust has no finalized memory model.** The opsem team and the Unsafe Code Guidelines working group are still deciding. Practical consequence: `unsafe` code that is Miri-clean today is not guaranteed sound against a model that does not yet exist.

**What Miri is and is not.** Miri is an interpreter. It checks **the paths your tests actually execute**. Miri-clean means "no UB was detected on the executed paths under the current default model." It is not a proof, it does not explore all inputs, and it says nothing about paths your tests miss. It remains the highest value-per-effort tool in Rust `unsafe` work by a wide margin, and both statements are true at once.

---

## 5. The verification tool landscape

**VOLATILE** (2026-09-18): every tool below was pushed within two weeks of this date. **Nothing in this list is dead** -- which is itself worth knowing, because the field's reputation for abandonware is out of date.

| Tool | Verifies | Language | Note |
|---|---|---|---|
| **Iris** | Anything you can model | Rocq | Maximum expressiveness, maximum cost |
| **RefinedC** | C, foundationally | C | Iris-backed, automated |
| **RefinedRust** | Rust, foundationally | Rust | Iris-backed. **Repo trap: the canonical home is MPI-SWS GitLab (`lgaeher/refinedrust-dev`), not GitHub** |
| **VeriFast** | Separation logic, annotation-driven | C, Java, Rust | On the Verify-Rust-Std approved list |
| **Viper** | Intermediate verification language | via frontends | The substrate for the three below |
| **Prusti** | Rust via Viper | Rust | The laggard: 301 open issues |
| **Gobra** | Go via Viper | Go | |
| **Nagini** | Python via Viper | Python | |
| **Creusot** | Rust, **not** via separation logic | Rust | See below |
| **Aeneas** | Rust by translation to a pure model | Rust → Lean/F\*/Rocq | See `[[lean-proof-engineering]]` |
| **Verus** | Rust with linear ghost state | Rust | **Rolling releases cut daily** |
| **Infer / Pulse** | Bug-finding at scale | C, C++, Java, Hack, Erlang, Python, Rust, Swift | See §6 |
| **CN** | C, refinement types | C | Cerberus-based |
| **Gillian** | Parametric symbolic execution | JS, C, Rust | |

**Creusot deliberately does not use separation logic**, and its rationale is worth understanding because it inverts the usual argument: **the borrow checker already excludes mutable aliasing**, so a well-typed Rust program can be translated to *pure first-order logic* and handed to an SMT solver. You do not need a logic for aliasing when the compiler has already ruled it out. Mutable borrows are handled with **prophecy variables** (`^b` denotes the borrow's *final* value, known in advance). The irony worth keeping: prophecies were proved sound **using Iris** (RustHornBelt).

**Verus** re-hosts linear/affine ghost state in Rust's own type system rather than building a separation logic. (The `tracked` / `PointsTo` / tokenized-state-machine mechanism is **FOUND-UNVERIFIED** here; check primary sources before describing it in detail.)

**A live datapoint for the automation-versus-expressiveness split:** the Verify Rust Standard Library effort maintains a **governed, revocable** approved-tools list -- ESBMC, Flux, Kani, KMIR, RAPx, and **VeriFast for Rust**. A separation-logic tool is deployed against the real standard library. Iris and RefinedRust are **not** on that list.

---

## 6. The finding that matters most, and it is not about logic

**Bi-abduction** (Calcagno, Distefano, O'Hearn, Yang) is the breakthrough that made separation logic scale: infer both the missing precondition and the leftover frame at each call site, which makes the analysis **compositional** -- analyze each procedure once, against no whole-program context, and compose.

That compositionality is what let Infer run on Facebook-scale code with no annotations.

**And then the deployment result overshadowed the logic.** From "Scaling Static Analyses at Facebook" (CACM 2019): the **same** analysis, at the **same** sub-20% false-positive rate, had a fix rate of

- **near 0%** deployed as **nightly batch reports**, and
- **over 70%** deployed **at diff time**, in code review.

Same findings. Same precision. Two orders of magnitude difference in whether anyone acted. The authors attribute it to context-switch cost and relevance: a report about code you wrote twenty minutes ago is actionable; a report about code you wrote last month is an interruption.

**This connects back to the theory by exactly one link:** diff-time review requires sub-20-minute incremental analysis, which requires compositionality, which is what bi-abduction bought. The logic was in service of the deployment model.

**The generalizable lesson, and it is the most useful thing in this file:** for any verification or analysis tool, *when and where the result is delivered dominates how strong the guarantee is.* A weaker analysis in the pull request beats a stronger one in a nightly report, by a factor of seventy.

Other verified Infer results: 100,000+ issues fixed since 2014; more than 50% of fixes in key categories involved interprocedural traces (so the compositional part is load-bearing, not incidental); RacerD drove 2,500+ concurrency fixes in a year, with an Android engineer on record that *"without Infer, multithreading in News Feed would not have been tenable."*

**VOLATILE** (2026-09-18): **Infer's bi-abduction engine is gone.** `infer/src` contains `pulse/` and **no `biabduction/`** directory; the docs state verbatim that *"Pulse replaces the original biabduction analysis of Infer."* Infer itself is very much alive (v1.3.0 released 2026-05-12, pushed 2026-09-18, 15.7k stars), with Pulse backends for Clang, Erlang, Hack, Java, CIL, Python, Rust and Swift. Describe bi-abduction as the historically decisive idea, not as the shipping engine.

### Incorrectness logic, and the correction almost everyone gets wrong

O'Hearn's later work inverts the goal: instead of proving the absence of bugs, prove their **presence** with no false positives.

```
Hoare:         {p} r {q}   requires   post(r)p ⊆ q      (over-approximate)
Incorrectness: [p] r [q]   requires   post(r)p ⊇ q      (under-approximate)
```

Every reachable state in `q` is genuinely reachable. A reported bug is a real bug.

**The correction:** the POPL 2020 "Incorrectness Logic" paper has **no frame rule and no separation logic at all**. Its §8 states verbatim that *"the standard heap model of separation logic does not mesh well with under-approximation (**separation logic's frame rule can become unsound**)."* The phrase "Incorrectness Separation Logic" **does not appear in the paper**. ISL is a separate CAV 2020 follow-on that **required a change of model**.

So the natural assumption -- "incorrectness logic just flips the triples and keeps `∗`" -- **is wrong**, and it is wrong in the specific way that matters: the frame rule does not survive the flip for free.

What else changes: the rule of consequence **flips direction**; post-weakening **dies**; Hoare's backward assignment axiom **dies** while Floyd's forward one **survives**; the conjunction rule **dies**; loop invariants become **trivial**; Constancy **survives**.

Provenance worth keeping: **Derek Dreyer coined the term "incorrectness logic"** at POPL'19. The opposing camp named it.

---

## 7. The mainstream descendants

What separation-logic thinking gives an engineer who will never open a prover:

- **Ownership as a design discipline.** Every piece of mutable state has exactly one owner at a time. Transfer is explicit. This is a code-review question you can ask without any tooling.
- **Aliasing XOR mutability.** The single rule behind Rust's borrow checker, Swift's Law of Exclusivity (SE-0176: *"two accesses to the same variable are not allowed to overlap unless both accesses are reads"*), and C++'s lifetime profiles.
- **The frame rule as the justification for modular review.** When you can state a function's footprint, you can review it alone. When you cannot, you cannot -- and that is a design defect, not a review-effort problem.
- **`&mut` uniqueness enables both reasoning and optimization.** `noalias` is the compiler exploiting the same fact the reviewer exploits.
- **Capability-based design.** Hold a token to perform an action; the token is the resource; passing it is transfer.
- **Region and arena allocation.** Lifetime made structural rather than per-object.
- **Pure core, effects at the edges.** Reframed in these terms: a pure core has an empty heap footprint, so it needs no frame reasoning at all. This is why the sans-IO pattern makes code reviewable. See `[[feedback-sans-io-di-core]]`.

**Affine, not linear, and why the difference ships bugs.** Rust's ownership is **affine** (a value may be dropped) rather than **linear** (it must be consumed). `Copy` is the opt-in reinstatement of *contraction*; implicit `drop` is the ambient *weakening*. The **leakpocalypse** (`JoinGuard` and `mem::forget`, pre-1.0) is the canonical case: Rust cannot guarantee a destructor runs, so any design whose soundness depends on cleanup happening is broken. `mem::forget` is safe.

**Typestate enforces protocol safety, never protocol completion.** You can prevent "send after close." You cannot force "eventually close." **That gap is exactly the affine/linear gap**, and it is the right way to explain to a Rust engineer what linear types would add.

---

## 8. When to reach for this, and the honest answer that you usually should not

An eight-step ladder, cheapest first. **Most teams should stop at step 4 and feel no guilt.**

1. **Ownership discipline in review.** Name the owner. Free.
2. **The type system.** `&`/`&mut`, `Send`/`Sync`, newtypes, typestate. Free.
3. **Sanitizers.** ASan, TSan, UBSan. Minutes.
4. **Miri** for `unsafe` Rust, **Loom** or **Shuttle** for concurrency, **property-based testing** for everything. Hours to days. *Most projects should stop here.*
5. **Bounded model checking.** **Kani** for Rust, CBMC for C. Days. Real guarantees over bounded inputs, no proof engineering.
6. **SMT-backed verification.** Verus, Creusot, Prusti, VeriFast. Weeks. Annotation cost is real; the guarantee is modulo the tool's own soundness.
7. **Foundational verification.** Iris, RefinedRust, RefinedC. Months to years. Machine-checked to a small kernel.
8. **Full functional correctness of a real system.** seL4, CompCert scale. Person-decades.

**Genuine candidates for steps 6-8:** a lock-free data structure, an `unsafe` abstraction whose soundness argument nobody can state, a memory allocator, a concurrency primitive, a cryptographic implementation, a kernel.

**VOLATILE** (2026-09-18) tool health at step 4-5: **Loom** is the quiet one (last push 2026-02-20, v0.7.2 Aug 2025 -- alive but slowing). **Kani** (0.68.0, 2026-09-16) and **Shuttle** (pushed 2026-09-18) are thriving.

---

## 9. Schools of thought

Live, unreconciled. Stated at full strength.

### Proving correctness versus finding bugs

**Correctness camp:** absence of a bug class is the only thing worth the effort. Bug-finding is a treadmill.
**Bug-finding camp:** O'Hearn -- who won a Gödel Prize for the correctness side -- **pivoted to under-approximation himself**, which is the strongest available evidence. 100,000+ real fixes at Meta versus a handful of verified artifacts industry-wide.
**The sharpest version:** the bug-finding camp argues the correctness camp optimized the wrong variable. Precision and soundness were never the binding constraint; **delivery timing was** (§6).

### Automation-first versus expressiveness-first

**Automation-first** (Infer, Verus, Creusot, Kani): a tool that needs a PhD to operate will not be used, so accept a larger TCB and SMT incompleteness for something a team can run in CI.
**Expressiveness-first** (Iris, RefinedC, RefinedRust): foundational proofs are machine-checked to a small kernel. SMT-backed tools are trusting a large, complex, occasionally-buggy solver, and "verified" then means something weaker than people hear.
**Live evidence for the split:** the Verify-Rust-Std approved list includes VeriFast, Kani, Flux, ESBMC, and **not** Iris or RefinedRust.

### Stacked Borrows versus Tree Borrows

**Tree Borrows:** less restrictive, rejects 54% fewer real programs, enables read-read reordering, PLDI'25 Distinguished Paper.
**Stacked Borrows / status quo:** simpler, stricter, and Miri's maintainers argue the eventual official model **will be stricter than Tree Borrows** -- so adopting the looser model now teaches people that code is fine when it may later be UB.
**Unresolved, and consequential:** people write `unsafe` code today against a model that does not exist yet.

### Is Rust's unfinished memory model a crisis?

**Crisis:** every `unsafe` block in the ecosystem is written against an unspecified semantics. Soundness claims are provisional.
**Non-issue:** the practical rules (no aliasing `&mut`, no UB on uninitialized reads, respect provenance) have been stable for years; Miri catches the real mistakes; formalization is finishing a job the community already follows informally.

### Separation logic versus ownership types versus linear types

**Separation logic:** most expressive; handles arbitrary sharing protocols; needs a prover.
**Ownership types / borrow checking:** a decidable fragment enforced by a compiler with zero annotation cost -- which is why it shipped to millions of engineers and separation logic did not.
**Linear types:** guarantee consumption, which affine ownership cannot. Would fix the leak gap; cost is ergonomics that no mainstream language has accepted.

---

## 10. Anti-pattern catalog

| Pattern | Trigger | Consequence | Fix |
|---|---|---|---|
| **Informal aliasing argument in `unsafe`** | A comment saying "safe because nothing else holds this" | The blast radius is the whole module's invariants, not the block | Write the ownership claim as a `# Safety` contract; run Miri; consider Kani |
| **Miri-clean read as sound** | Green Miri run | Miri checks executed paths under the *current default* model only | Say "no UB on tested paths"; expand test coverage; note the model is not final |
| **Flipping to `-Zmiri-tree-borrows` to silence a failure** | Stacked Borrows rejects the code | The official model will be *stricter* than Tree Borrows; the failure may be real | Fix the aliasing; use the flag for investigation, not absolution |
| **SMT success reported as a foundational proof** | Verus / Creusot / Prusti passes | The TCB includes a large solver; "verified" means something weaker than heard | State the trust base explicitly |
| **Verifying a model, shipping the code** | A hand-written abstraction proved, the implementation not | The proof is about a document | Differential testing against the model, or verify the real code |
| **Fractional permissions where a protocol is needed** | Reaching for read-sharing to model a state machine | Fractions express sharing, not ordering | Ghost state or a typestate encoding |
| **`−∗` where `⇒` suffices** | Pure antecedent | Needless frame quantifier, harder proofs | Use `⇒` unless the antecedent owns heap |
| **`dom h ⊇ {e}` read into `e ↦ e'`** | Reading `↦` as "contains" | Every ownership argument is subtly wrong | `↦` is **exact**: the heap *is* that cell |
| **"Incorrectness logic keeps the frame rule"** | Assuming the flip is mechanical | Wrong: the frame rule can become unsound under-approximately | ISL is a separate result requiring a model change |
| **"Separation logic is linear logic"** | Surface resemblance | Obscures BI's two conjunctions | The parent is BI (O'Hearn and Pym) |
| **Nightly batch static analysis** | Standing up a new analysis | ~0% fix rate regardless of precision | Deliver at diff time; 70%+ |
| **Assuming a destructor runs** | Soundness resting on cleanup | `mem::forget` is safe; Rust is affine, not linear | Do not make soundness depend on `Drop` |
| **`unsafe impl Send` without an argument** | Making a type cross threads | You asserted a theorem with no proof | State the sharing invariant; check for interior mutability |
| **Proving what Miri or Loom would find** | Reaching for step 6-8 first | Weeks spent on an afternoon's question | Walk the ladder from step 1 |
| **Citing RefinedRust on GitHub** | Searching the obvious place | 404; the GitHub org does not host it | MPI-SWS GitLab, `lgaeher/refinedrust-dev` |

---

## 11. Authorities

**People.** John C. Reynolds (the 2002 paper; the canonical exposition). Peter O'Hearn (CSL, bi-abduction, incorrectness logic; Gödel Prize 2016 with Stephen Brookes). Hongseok Yang, Cristiano Calcagno, Dino Distefano (bi-abduction, Infer). Ralf Jung (Iris, RustBelt, Stacked Borrows). Derek Dreyer (Iris, RustBelt; coined "incorrectness logic"). Robbert Krebbers (Iris Proof Mode). Lars Birkedal (step-indexing, Iris). Jacques-Henri Jourdan. Bart Jacobs (VeriFast). Peter Müller (Viper). John Boyland (fractional permissions).

**Papers, by what they are good for.**
- Reynolds, *"Separation Logic: A Logic for Shared Mutable Data Structures"* (LICS 2002) -- **read this one**. Genuinely readable, and the source of nearly every precise statement in §1.
- O'Hearn, *"Resources, Concurrency and Local Reasoning"* -- CSL, the Gödel-cited work.
- Jung et al., *"Iris from the ground up"* -- the framework's own exposition. Heavy.
- Jung et al., *"RustBelt: Securing the Foundations of the Rust Programming Language"* (POPL 2018) -- the Rust connection.
- Calcagno, Distefano, O'Hearn, Yang, *"Compositional Shape Analysis by Means of Bi-Abduction"* -- why it scaled.
- O'Hearn, *"Incorrectness Logic"* (POPL 2020) -- the pivot. Note §8 on the frame rule.
- *"Scaling Static Analyses at Facebook"* (CACM 2019) -- **the most practically useful paper in the field**, and it is about deployment.
- Villani, Hostert, Dreyer, Jung, *"Tree Borrows"* (PLDI 2025).

**Route in for an engineer:** Reynolds 2002 §1-3 for the logic, then the Infer CACM paper for why deployment beat expressiveness, then RustBelt if you write `unsafe` Rust. Iris only if you are going to use it.

---

## 12. Severity rubric (this domain)

- **blocker** -- An `unsafe` block whose soundness rests on an aliasing assumption that is false, unstated, or unenforced, with a reachable trigger. `unsafe impl Send`/`Sync` on a type with interior mutability or non-thread-safe internals. Soundness depending on a destructor running. A verification claim materially stronger than what the tool establishes (model proved, implementation shipped; SMT result called foundational).
- **major** -- `unsafe` with no `# Safety` contract stating the caller's obligations. A data structure with a sharing protocol nobody has written down. Miri never run on `unsafe` code. A static analysis deployed where nobody will act on it (§6). Ownership that crosses an API boundary with no documented transfer.
- **minor** -- Ownership expressible in the type system but left to convention. `Rc<RefCell<T>>` where `&mut` threading would work. Missing Loom or Shuttle coverage on a concurrency primitive.
- **nit** -- Naming that obscures ownership (`get_mut` that transfers). Comment phrasing about lifetimes.
- **insight** -- "This is the affine/linear gap: typestate gives you protocol safety here but cannot force completion." "The footprint of this function cannot be stated, which is why it cannot be reviewed in isolation." "This analysis would find ~70% more fixes at diff time than as a nightly report."

---

## Source research

`~/.claude/local/research-notes/separation-logic-research.md` (2,379 lines; 280+ VERIFIED / FOUND-UNVERIFIED / INFERRED tags, with a verification ledger for fast-rotting facts). Primary PDFs were fetched and extracted; `gh api` and the GitLab API established maintenance status; a headless browser was required past Cloudflare for the CACM paper and the ACM DL PDFs.

Recorded as FOUND-UNVERIFIED and not filled: the Verus `tracked` / `PointsTo` / tokenized-state-machine mechanism.

## Changelog

- **2026-09-18** -- File created. Iris 4.5.0; Miri still defaults to Stacked Borrows; Infer's bi-abduction engine replaced by Pulse.
