---
paths:
  - "__agent_only_never_match_at_startup__/**"
last-verified: 2026-09-18
---

# Lean 4 proof engineering

**Thesis.** Lean's kernel checks proofs, not statements. Every serious failure in Lean work -- human or model-generated -- lives in that gap. A proof that closes tells you the term inhabits the type you wrote; it says nothing about whether the type you wrote means what you intended. The kernel is ~8K lines of C++ and is the most trustworthy component in your stack; the specification above it is ordinary fallible prose-turned-code, and it is unverified by construction.

**The agent's operational question:** *what exactly did this proof establish, and is that the thing the caller needs?*

**Empirical priority order.** What actually bites, most often first:

1. **Statement fidelity.** The theorem proves something weaker, different, or vacuous. Includes vacuous hypotheses, wrong quantifier order, `Nat` truncated subtraction, division by zero conventions, and models that have drifted from the shipped code.
2. **Axiom and `sorry` hygiene.** What the artifact actually depends on, and whether your audit script still detects it (see the `native_decide` breaker below -- this silently broke).
3. **Automation misdiagnosis.** Treating a `simp` loop as slowness, raising the wrong heartbeat budget, reaching for `decide` on something the kernel cannot reduce.
4. **Definitional-reduction surprises.** Well-founded recursion is irreducible by design; `rfl` stops working and the error does not say why.
5. **Proof fragility.** `simp`-heavy proofs against a library with 4,404 live deprecation shims.
6. **Cost misjudgment.** Modeling something that needed a weekend, or committing to something that needed a quarter.

---

## Volatile surface

`last-verified` in the frontmatter. These rot; the logic and the type theory do not.

| Claim class | Rots | Re-verify at |
|---|---|---|
| Lean version, release cadence | Monthly | `gh api repos/leanprover/lean4/releases/latest` |
| Mathlib size, tactic-usage counts | Continuous | Clone and measure; do not quote stale digits |
| `grind` capability | Fast (under active development) | Release notes; the v4.22 paper is already behind |
| AI proving benchmarks, costs, SOTA claims | Very fast (weeks) | Treat every number here as a date-stamped snapshot |
| Tool liveness (Loogle, LeanSearch, MCP integrations) | Medium | `curl` the endpoint; `gh api` the repo |
| FRO funding, roadmap, stability pledge | Annual | Lean Together deck each January |
| Axiom names emitted by `native_decide` | Changed in 2026 | Grep the Lean source, not your memory |

**VOLATILE** (2026-09-18): Lean stable is **v4.34.0** (2026-09-14), monthly cadence. The FRO's Jan-2026 deck states verbatim: *"2026 will be the last year with significant changes to the language and standard library... After 2026, we shift to a stability-first approach."* Practical read: a 2026 adopter is pinning to a toolchain still in its last declared breaking year. Budget for churn through end of 2026.

---

## 1. The statement-fidelity problem (read this before anything else)

The kernel validates that a term has a type. Nobody validates that the type says what you meant. This is not a theoretical caveat; it is the dominant failure mode in practice, and it gets *worse* as proof automation gets better.

**The decisive finding.** "Beyond Compilation" (arXiv:2606.31002) measures the gap between *compiles* and *faithful to the informal statement*. The gap **grows with model capability** -- 29 percentage points for GPT-5.2. Compile rate is a *worsening* proxy for correctness. A stronger prover is better at finding a proof of whatever you actually wrote, including the vacuous reading.

Supporting evidence, all **VOLATILE** (2026-09-18):

- More than 50% of miniF2F has formal/informal discrepancies.
- Biderman et al. found 4,833 issues across 10,318 benchmark problems. The top hazards are *ordinary programming hazards*, not exotic logic: **1,518 division-by-zero** and **796 `Nat` subtraction** (truncation at zero).
- Hallucinated Mathlib lemma names account for 7.7% of failures, and **retrieval augmentation makes this worse**, not better.
- **LongCat-Flash-Prover** is a documented production reward-hacking case: roughly **70 percentage points** of apparent success survived *both* conventional checks, because the harness permitted the model to edit the theorem context.

That last one falsifies the comfortable claim that reinforcement learning with verifiable rewards cannot be hacked because the kernel is sound. **The attack surface is the harness, not the kernel.**

**The institutional response.** The Lean FRO built **Comparator** ("a trustworthy judge for Lean proofs... verify that a solution proves exactly what was claimed, using only permitted axioms"). Its pipeline builds challenge and solution in sandboxes, exports both to kernel-checkable form, **verifies the declarations match exactly**, and checks the axiom set. Step three exists precisely because the checker validates proofs and not statements. The FRO building tooling for the statement-substitution attack is strong evidence the attack is real and common.

FRO doctrine, verbatim: *"Don't trust us. Verify."*

### Lean's own hazards that produce vacuous or wrong theorems

These are the specification bugs that a Lean beginner writes and a Lean expert greps for:

- **`Nat` subtraction truncates.** `a - b = 0` when `b ≥ a`. A theorem about `n - 1` quietly covers `n = 0` with a meaning you did not intend.
- **Division by zero is total.** `x / 0 = 0` in Lean's `Nat`, `Int`, and `Real`. So is `x⁻¹` at zero. A theorem "proving" a division identity may be proving it in the degenerate case for free.
- **Vacuous hypotheses.** A hypothesis no inhabitant satisfies makes the theorem true and useless. Check inhabitation of your antecedents.
- **Quantifier order.** `∀ ε, ∃ δ` and `∃ δ, ∀ ε` are different theorems and both typecheck.

**Review move:** for any theorem statement, ask what makes it *false*. If you cannot construct a would-be counterexample shape, the statement may be vacuous.

---

## 2. Axiom and `sorry` hygiene

### The audit-script breaker (2026)

**`Lean.ofReduceBool` no longer exists.** Grep the Lean source at HEAD: it is absent. Since roughly v4.29-v4.31 (RFC #12216, PR #12217), each `native_decide` use mints a **fresh per-use axiom** named `<decl>._native.<tactic>.<suffix>`.

**Consequence: any CI check that denylists `ofReduceBool` now silently passes forever.** It will never fire again, on any code, regardless of how much `native_decide` you add.

**The fix is to allowlist, never denylist.** The complete set of axioms a normal Lean development should depend on:

```
propext            -- propositional extensionality
Classical.choice   -- the axiom of choice
Quot.sound         -- quotient soundness
```

Anything else in `#print axioms myTheorem` output is a finding. `sorryAx` means an incomplete proof. A `_native` suffix means the kernel did not check that computation.

*(Version note: sources split on whether the per-use axiom first became user-visible in 4.29 or 4.31. Safe phrasing is "4.29-4.31 and later." Verify against the source rather than quoting a version.)*

### `native_decide` enlarges the trusted computing base

`native_decide` compiles a decision procedure to machine code and trusts the result. It puts the Lean compiler, the C toolchain, and your CPU inside the TCB. Mathlib uses it **6 times** in 2.3M lines. Treat that ratio as the norm. It is legitimate for a large finite check that the kernel cannot feasibly reduce; it is not legitimate in a security-relevant proof, and a reviewer should treat it as an explicit trust decision, not an implementation detail.

`bv_decide` is the underappreciated alternative for fixed-width integer reasoning (overflow, masking, shifts): verified bit-blasting with a Lean-implemented and Lean-verified AIG and LRAT checker, solving ~96% of SMT-LIB bitvector problems, producing **kernel-checkable** proofs with no native trust.

---

## 3. Tactics: what each discharges and where it falls off a cliff

### `decide` -- two documented failure modes

Lean's own compiler source (`src/Lean/Elab/Tactic/Decide.lean`) names both, verbatim:

- **Stuck on `Eq.rec`**: *"one of the `Decidable` instances is defined using tactics such as `rw` or `simp`"*. A tactic-built instance is opaque to kernel reduction.
- **Stuck on `Classical.choice`**: *"can occur due to the `open scoped Classical` command"*. A noncomputable instance cannot be evaluated.

And the tell for elaborator/kernel divergence: *"The elaborator is able to reduce the `Decidable` instance, but the kernel fails with..."* -- meaning it worked interactively and failed at check time.

**Consequence for modeling:** if you want `by decide` to close your finite state-machine goals, every `Decidable` instance on the path must be built by structural definition, not by tactic. This constraint shapes how you write the model, and it is the single most common reason a "finite, should be decidable" model does not close.

### The automation ladder

| Tactic | Discharges | Cliff |
|---|---|---|
| `rfl` | Definitional equality | Fails on well-founded recursion (§4); fails when the recursion argument differs (§5) |
| `decide` | Closed decidable props | Needs kernel-reducible instances; exponential blowup on large finite spaces |
| `omega` | Linear integer/`Nat` arithmetic, including truncated subtraction | Nonlinear terms; leaves the fragment silently |
| `simp` | Rewriting to normal form | Loops; see below |
| `norm_num` | Numeric evaluation | Symbolic goals |
| `ring` | Commutative ring identities | Non-ring structure, side conditions |
| `linarith` | Linear arithmetic over ordered fields | Nonlinear; needs the hypotheses supplied |
| `bv_decide` | Fixed-width bitvector goals | Bit-width blowup |
| `grind` | SMT-inspired: congruence closure, E-matching, case analysis, linear integer arithmetic, Gröbner basis, fields | Newer, moving fast; heuristic instantiation is an acknowledged weakness |
| `aesop` | Best-first proof search over a rule set | Needs a good rule set; opaque failures |

**`grind` is the one to watch for software verification.** Introduced v4.22 (Aug 2025), it is native to dependent type theory with no translation to first-order logic, has no Mathlib dependency, and the FRO's own deck says *"Great for software verification applications."* Measured in Mathlib at 6,975 occurrences, from zero in Aug 2025 -- the fastest riser in the library.

**`polyrith` is effectively dead** (2 occurrences in Mathlib; required a Sage server). Do not recommend it.

### `simp` failure modes

- `simp` failing with **"maximum number of steps exceeded"** means a **loop**, not slowness. Raising `maxSteps` (default 100000) turns a fast failure into a slow one. Find the rewrite pair whose left and right sides reduce to each other.
- `simp?` prints the `simp only [...]` call it found. Use it, then commit the explicit form.
- Mathlib itself runs bare `simp` 63,133 times against `simp only` 27,910 -- so "always use `simp only`" is not the library's own practice. The real rule is narrower: **`simp only` in anything whose proof must survive library churn**; bare `simp` is fine in a leaf proof you are willing to repair.

### Resource limits people routinely misdiagnose

Source-verified defaults:

- `maxHeartbeats` = **200000**, and it is **per command**.
- `synthInstance.maxHeartbeats` = **20000** -- one tenth, and it is **per instance problem**.
- `synthInstance.maxSize` = **128**.
- `simp` `maxSteps` = **100000**.

**Raising `maxHeartbeats` never fixes a `synthInstance` timeout.** They are separate budgets. A typeclass resolution failure that looks like "Lean is slow" is usually an instance loop or a diamond, and the fix is to the instance graph, not the budget.

---

## 4. Well-founded recursion: irreducible by design

`src/Lean/Elab/PreDefinition/Mutual.lean:70` calls `setIrreducibleAttribute` on every non-theorem predefinition. This is deliberate, not an accident.

**Consequence:** a function defined by well-founded recursion does **not** reduce definitionally. `rfl` stops working. `simp [myFunction]` does not unfold it. The error message does not explain why, and this is the single most confusing transition for someone arriving from structural recursion.

What to do:
- Prefer **structural recursion** whenever the argument genuinely shrinks structurally. It reduces definitionally and everything downstream is easier.
- When you need well-founded recursion, use the generated **equation lemmas** (`myFunction.eq_def`, the `simp` set Lean generates) rather than expecting unfolding.
- `termination_by` names the measure; `decreasing_by` discharges the decrease obligation.
- **GuessLex** (Breitner) searches a *bounded combinatorial space* of lexicographic measures. Its design doc states that anything it infers must also be writable by hand -- so if it fails, writing `termination_by` explicitly is always available and is the fix, not a workaround.
- `partial` opts out of the termination obligation and gives you a function you cannot reason about. `unsafe` is stronger and worse. Both are legitimate for `IO` plumbing and illegitimate in the part of the model you intend to prove things about.

---

## 5. The two definitional-equality traps

### Polarity reverses between Lean and Rocq

This is the trap that catches people arriving from Coq/Rocq literature, and it catches them silently because both systems look identical at the surface.

- **Rocq's `add` recurses on the first argument.** So `0 + n = n` holds by `reflexivity`, and `n + 0 = n` needs induction.
- **Lean's `Nat.add` recurses on the second argument.** So `n + 0 = n := rfl`, and `0 + n` needs induction.

**Any tutorial stating one version is wrong for the other system.** The general lesson is worth more than the instance: *which equations hold judgmentally is an artifact of the recursion argument, not a fact about arithmetic.* When `rfl` unexpectedly fails, the first question is which argument the function recurses on.

### `Prop` vs `Type`, proof irrelevance, and universes

- `Prop` is proof-irrelevant: any two proofs of the same proposition are definitionally equal. `Type` is not.
- `Prop` is impredicative; `Type u` is predicative. `Type : Type` is inconsistent (Girard's paradox), which forces the universe hierarchy -- this is settled design, not a live debate.
- **Asserting univalence in Lean 4 is inconsistent**, not merely unprovable. And the usual escape does not work: a fresh identity type in `Type` does not rescue it, because you can prove it equivalent to proof-irrelevant `eq`, making it a subsingleton too. (Sourced to Floris van Doorn, who wrote the Lean 2 HoTT library.) This is Lean's *particular* design choice, not a property of proof irrelevance as such.
- "Motive is not type correct" almost always means a `rw` is trying to rewrite a term that later terms depend on. Reach for `conv`, `subst`, or a `generalize` before fighting the motive directly.

---

## 6. Mathlib: measured, and why you should not calibrate on it

Measured at HEAD, 2026-09-18 (**VOLATILE** -- clone and re-measure rather than quoting these):

| Metric | Value |
|---|---|
| Files | 8,543 |
| Lines | 2,331,909 |
| Definitions | 136,930 |
| Theorems | 288,652 |
| Contributors | 772 |
| Live `@[deprecated]` shims | **4,404** |

That last row is the churn metric that matters to you. It is the standing count of renames that have not yet been cleaned up.

**The critical inversion.** Mathlib has 9,842 `noncomputable` declarations, and `omega` and `decide` each appear only ~970 times. That is because **Mathlib is abstract mathematics**: real numbers, topology, category theory -- domains where nothing is decidable and nothing computes.

**A software verification project should have the opposite mix.** Finite state spaces, decidable predicates, concrete arithmetic, computable definitions. **Do not calibrate your proof style on Mathlib.** The idioms that make sense for formalizing measure theory are close to the opposite of what makes a protocol model tractable.

### Lemma search: what is alive

- **Loogle** -- live, and **has a JSON API**: `curl -sG https://loogle.lean-lang.org/json --data-urlencode 'q=...'`. This is the practical way to wire Mathlib lemma search into an agent or editor.
- **LeanSearch** -- live, natural-language search.
- **`exact?` / `apply?`** -- interactive search tactics. Note they appear **4** and **0** times respectively in Mathlib itself: they are tools you run and then *delete*, replacing with the lemma they found. A committed `exact?` is a code smell.
- **Moogle** -- **dead** (404 on all three domains). Stop recommending it.
- **`lean-lsp-mcp`** (`oOo0oOo/lean-lsp-mcp`) -- the current agent-integration path.
- **Dead or dormant:** `llmstep` (pinned to Lean v4.1.0, untouched since Nov 2023), LeanDojo v1 (officially deprecated), ReProver (unmaintained; scores 0/672 on PutnamBench).

---

## 7. Practical modeling of ordinary software problems

This is the section that governs whether to reach for Lean at all.

### Two different decisions, and only one of them is about the repo

Keep these apart. Conflating them suppresses the cheap, common, useful case.

**Decision 1: model this problem in Lean, as a reasoning tool.** A scratch file, outside the repo or in a `scratch/` directory, written to settle a specific question. It may be deleted the same day. **Lean does not need to be in the project, and usually will not end up there.** What you get is a forced enumeration of the state space, a statement of the invariant precise enough to be wrong, and a machine telling you whether it holds. Most of the value arrives during *modeling*, before any proof closes -- writing the inductive type is what surfaces the state nobody considered.

This is the case that should fire often. It competes with a whiteboard and a long argument in review, not with a verification program.

**Decision 2: introduce Lean into the repository.** A toolchain pin, a `lakefile`, CI minutes, onboarding cost, and someone who maintains it when it breaks. This is a real engineering commitment and a much higher bar.

**Decision 1 does not require Decision 2, and does not imply it.** Model first. If the model earns its keep and the invariant is durable enough to be worth re-checking on every change, *then* raise Decision 2 as its own conversation, with the costs named: the build, the pin, who else can edit it, and what happens when the toolchain moves during Lean's declared breaking year.

### The feasibility gate (governs Decision 1)

Offer a Lean model when **all four** hold:

1. **Bounded or cleanly inductive state space.** A finite enum of states, or a structurally recursive datatype.
2. **A stated invariant or safety property.** Something you can name before you start. "Model it and see" is not a property.
3. **Decidable, or provable without deep mathematics.** If closing the goal requires real analysis, you are formalizing mathematics, not verifying software.
4. **Modeling cost under roughly a day.** See the cliff below.

**Note what is deliberately absent: whether Lean is already in the project.** That is Decision 2 and is not a precondition. A throwaway model of a protocol in a TypeScript codebase is a legitimate and common use.

If any one of the four fails, say so and name the better tool. Staying quiet is the correct output most of the time.

### The escalation from model to repo

When a model has proved its worth and someone asks whether it should live in the codebase, the honest answers, cheapest first:

1. **Keep it as a scratch artifact.** Paste the model and its theorem into the design doc or the pull-request description as evidence. Zero ongoing cost, and it still communicates the invariant. **This is the right answer most of the time.**
2. **Commit the `.lean` file without wiring it to CI.** It documents the intended invariant and can be re-run by hand. Costs a stale-file risk and nothing else.
3. **Add Lean to CI.** Now the invariant is enforced on every change, and now you own a toolchain pin, build minutes, and a broken-build owner. Justified when the invariant is load-bearing and the model tracks code that actually changes.
4. **Generate or check the implementation against the model.** Differential testing between the model and the code (the Cedar approach, §7 below). The most valuable and the most work.

Rung 3 is where the cost jumps, and it is worth saying out loud that a `.lean` file in CI whose model has drifted from the implementation is worse than no file, because it asserts a guarantee nobody is checking.

### The cost cliff (the number that matters)

For a competent-but-not-expert user, one safety property, a ~6-state protocol:

- **1-3 days** if it stays **finite and decidable** (`Fintype` + `by decide`, or `grind`).
- **1-3 weeks** once it is **parameterized** (arbitrary `n` clients, unbounded queues) and needs induction and invariant strengthening.

**That is a cliff, not a slope.** The transition from "enumerate the finite space" to "prove it for all `n`" is where the cost discontinuity lives, and it is the single most important thing to tell someone before they start. Parameterizing a model that was tractable yesterday can make it a week of work today.

### What the model looks like

The shape for a protocol or state machine:

```lean
inductive State where
  | idle | connecting | connected | closing | closed | failed
  deriving DecidableEq, Fintype, Repr

inductive Step : State → State → Prop where
  | connect    : Step .idle .connecting
  | established : Step .connecting .connected
  | fail       : Step .connecting .failed
  -- ...

def Reachable : State → Prop := Relation.ReflTransGen Step .idle

theorem no_connected_without_connecting :
    ∀ s, Reachable s → s = .connected → ... := by
  decide  -- if everything on this path is kernel-reducible
```

The three moves that make this work: `deriving DecidableEq, Fintype` so the space is enumerable, transitions as an **inductive relation** rather than a function (partiality is then explicit), and the property stated over **reachable** states rather than all states.

`#guard` gives you decidable unit tests that run at elaboration time -- the cheapest possible entry point, and often enough on its own.

### When Lean is the wrong tool, and it usually is

- **TLA+** -- concurrent and distributed algorithms, temporal properties (liveness, fairness), and a model checker (TLC) that produces counterexample traces. For "does this distributed protocol have a race," TLA+ beats Lean decisively. Amazon's published use is the industrial evidence.
- **Alloy** -- structural and relational models, bounded scope, fast counterexamples. Best for "can this data model reach a bad shape."
- **Quint** -- TLA+ semantics with a syntax engineers will actually read.
- **P** -- event-driven distributed systems with state machines as the primitive.
- **Kani / Verus / Creusot** -- for Rust specifically, verify the *real code* rather than a model of it.
- **Property-based testing** (proptest, Hypothesis, fast-check) -- an afternoon, no new language, finds most of what a model would.

**The decisive question:** do you want a **counterexample** or a **proof**? Model checkers hand you a trace showing what breaks; that is usually what you want and it is enormously cheaper. Lean hands you a proof for all inputs, which matters when the space is infinite or the stakes justify it.

The honest default: **most teams should reach for property-based testing and a model checker, and feel no guilt.**

### The model-drift problem

A Lean model is a *separate artifact* from the code you ship. It can be correct while your code is wrong. Two mitigations, both from real deployments:

- **Cedar** (AWS authorization) proves a **1,673-line model** (5,714 lines of proof, ~0.23:1 against 24,915 lines of Rust) and bridges the gap with **differential random testing** between the model and the implementation. The bridge is the essential part; without it the proof is about a document.
- **SymCrypt** proves the **real Rust** and pays roughly **12:1** and person-years for it.

Choose one deliberately. A model with no bridge to the implementation is a design document with a theorem attached.

---

## 8. Build and tooling

- **elan** manages toolchains and reads each project's `lean-toolchain` file, exactly as rustup reads `rust-toolchain.toml`. Version is pinned **per project, by the project**. `.olean` artifacts are not compatible across toolchain versions, so a global pin is wrong for any machine with more than one project.
- **lake** is the build tool. `lakefile.toml` is the current recommendation for straightforward projects; `lakefile.lean` when you need programmatic configuration.
- **`#print axioms <decl>`** is the audit tool. See §2.
- **`#eval`**, **`#check`**, **`#guard`** for interactive work.
- The VS Code extension and its **InfoView** are the primary interface; the goal display is the thing you actually work against.
- **Mathlib is a heavy dependency.** ~485 MB checkout, long build times, continuous churn, 4,404 deprecation shims. Take it when you need real mathematics. For a protocol model over finite state, the standard library plus `grind` may be enough -- and `grind` explicitly has no Mathlib dependency.

### Editor integration (machine-specific, and a known trap)

On this machine, Lean is the one LSP the dotfiles config cannot pin, and the trap is real: **mason's `lean-language-server` package is Lean 3** (it declares `languages: [Lean 3]` and installs `lean-language-server@3.4.0`). Wiring it up installs cleanly and is wrong. The real server is an elan shim -- `lake serve` for a project with a lakefile, `lean --server` standalone. `lean.nvim` ships its own `lsp/leanls.lua` and must never enter the mason-lspconfig handler loop, which silently overwrites its handlers and leaves the infoview empty. See `[[project-nvim-lean-tooling]]`.

---

## 9. Lean as a general-purpose language

Lean 4 is a real programming language, not only a prover: `IO`, monads, an FFI, and compilation to C. This matters for two reasons. It means a model can be *executed* and differentially tested against the implementation (see Cedar above), and it means the proof and the program can be the same artifact.

Software-verification tooling built on Lean is **2025-vintage and still forming**: **Velvet** (imperative program verifier, Oct 2025), **mvcgen** (the FRO's monadic verification framework; weakest-precondition VC generation over `Std.Do`), **Aeneas** (Rust via translation to Lean), **Strata**, and **CSLib** (launched late 2025, backed by AWS, Google DeepMind, SDU, Centaur). Do not assume Dafny-level or Why3-level maturity; several of these are months old.

Industry named as using Lean (FRO deck, Jan 2026): AWS, Google DeepMind, Meta FAIR, Microsoft, OpenAI, plus Harmonic, Axiom, Math Inc., Mistral, Logical Intelligence, Axiomatic AI.

---

## 10. Proof assistants and AI code assistance

**The state of the art has moved past proof search.** Both benchmarks people cite are **saturated** (**VOLATILE**, 2026-09-18): miniF2F at 100% (Leanstral 1.5, Apache-2.0 open weights), PutnamBench at 672/672 by four separate systems, with mean cost collapsing roughly 440x in three weeks to $0.17 per problem.

**So proof search is no longer the bottleneck. Statement fidelity and cost are.** See §1 -- this is why that section leads the file.

### What this means for working with a model on Lean

- **The kernel is a real reward signal, and that is genuinely special.** A hallucinated proof does not typecheck. This is why formal proof is unusually well-suited to model generation, and the claim is true as far as it goes.
- **It does not extend to statements.** The model writes the theorem too, and nothing checks that. Review the statement by hand. Always. This is the single highest-value human contribution in a model-assisted Lean workflow.
- **The harness is the attack surface.** LongCat-Flash-Prover's ~70-point reward hack worked by editing the theorem context. If a model can edit anything upstream of the goal -- definitions, hypotheses, `axiom` declarations, imports -- the kernel's guarantee is scoped to a statement the model chose.
- **Hallucinated lemma names are 7.7% of failures and retrieval makes it worse.** Wire in Loogle's JSON API for ground truth rather than trusting recall.
- **Audit with an allowlist** (§2). This is the one mechanical check that catches `sorry`, `native_decide`, and stray `axiom` declarations in one pass.

### The `sorry`-driven workflow

`sorry` is the right tool and should be used deliberately: state the full theorem, `sorry` it, build the downstream structure against the statement, and discharge the `sorry`s last. This front-loads exactly the part that needs human judgment (is the statement right?) and defers the part a model is good at (find the proof term).

The failure mode is losing track. `#print axioms` at the end of every session, and a CI check with an allowlist, not a denylist.

---

## 11. Schools of thought

Live disagreements. Each stated at full strength; do not reconcile them.

### Tactic-heavy versus term-mode proofs

**Tactic camp:** tactics are how non-trivial proofs get written and maintained; term mode for anything substantial is showmanship that nobody can modify later.
**Term camp:** a term-mode proof is a value whose type you can read, it survives tactic-framework churn, and it does not silently change meaning when a `simp` set changes upstream.
**Where each is right:** term mode for small, stable, library-facing lemmas; tactics for anything with real case analysis.

### Does `simp`-heavy proving produce maintainable proofs?

**Pro:** it is how Mathlib is actually written (63,133 bare `simp` calls), and demanding explicit rewrites everywhere would have made the library impossible to build at 772 contributors.
**Con:** a bare `simp` proof depends on the entire global `simp` set, so an upstream lemma attribute change breaks your proof for reasons with no local explanation. With 4,404 live deprecations, this is a standing tax.
**Note:** Mathlib's practice is evidence about *Mathlib's* constraints. A 2,000-line software model is not a 2.3M-line community library, and copying its norms is the calibration error in §6.

### Lean versus Rocq/Coq for software verification

**Rocq camp:** the software-verification ecosystem is *there* -- CompCert, Iris, RustBelt, Perennial, VST. Iris in particular is a decade of concurrent separation-logic infrastructure with no Lean equivalent. Choosing Lean for a concurrency proof means rebuilding foundations.
**Lean camp:** better ergonomics, a single integrated language for programs and proofs, the fastest-growing community, and real momentum (the FRO deck notes Lean's RedMonk placement against Coq's -- though that is a partisan source on a competitive comparison, and should be read as the FRO's framing).
**Honest state:** for concurrent separation logic today, Rocq has Iris and Lean does not. For a new greenfield functional-correctness project with a team that has to learn the tool anyway, Lean's ergonomics argument is strong. See `[[separation-logic]]`.

### Is AI proof automation a step change or benchmark saturation?

**Step-change camp:** benchmarks saturated, cost fell 440x in weeks, and Ilya Sergey is quoted in the FRO deck: *"The research community's perception of program verification is about to change irreversibly."*
**Skeptic camp:** miniF2F and PutnamBench are *competition mathematics*, not software verification. More than half of miniF2F has statement discrepancies, so saturating it partly measures fitting a flawed benchmark. The compile-versus-faithful gap grows with capability, and there is a documented production reward-hacking case. Saturation on a broken benchmark is not capability.
**Both are looking at different things:** proof *search* genuinely improved a lot; proof *specification* did not improve at all, and it was always the harder half.

### `native_decide` and TCB purity

**Purist:** it puts the compiler, the C toolchain, and the CPU in the TCB, which discards the reason to use a proof assistant. Mathlib's 6 uses in 2.3M lines is the correct rate.
**Pragmatist:** some finite checks are infeasible for the kernel, and refusing the tool means not doing the verification at all. An explicit, audited, documented trust assumption beats no proof.
**Both agree on one thing:** it must be *visible*. Which is exactly what the 2026 axiom change broke for anyone using a denylist.

---

## 12. Anti-pattern catalog

| Pattern | Trigger | Consequence | Fix |
|---|---|---|---|
| **Denylisting `ofReduceBool` in CI** | An audit script written before 2026 | Silently passes forever; `native_decide` is now undetectable to it | Allowlist `propext`, `Classical.choice`, `Quot.sound` |
| **Proving a theorem that is vacuous** | A hypothesis nothing satisfies; `Nat` subtraction; division by zero | A green proof that guarantees nothing | Ask what would make the statement false; check antecedent inhabitation |
| **Trusting a model's theorem statement** | Model-assisted formalization | The kernel validates a statement the model chose | Human-review every statement; `#print axioms`; Comparator-style declaration matching |
| **Letting the harness expose the context** | Agentic proof loops with file write access | Reward hacking; ~70pp of fake success documented in production | Constrain edits to the proof body; diff the statement before accepting |
| **Raising `maxHeartbeats` for a typeclass timeout** | `synthInstance` failure misread as slowness | Slower failure, same outcome | Fix the instance graph; the budget is separate and 10x smaller |
| **Treating "maximum steps exceeded" as slow** | `simp` loop | Endless budget-raising | It is a loop; find the rewrite pair that cycles |
| **Bare `simp` in a proof that must survive churn** | Library-facing or long-lived lemma | Breaks on upstream `simp`-set changes, with no local cause | `simp?` then commit `simp only [...]` |
| **Well-founded recursion where structural would do** | Using `termination_by` reflexively | Definition becomes irreducible; `rfl` and unfolding stop working | Restructure for structural recursion; else use equation lemmas |
| **Calibrating proof style on Mathlib** | Reading Mathlib for idiom | Abstract, noncomputable style imported into a domain that should be finite and decidable | Invert the mix: `Fintype`, `DecidableEq`, `decide`, `grind` |
| **`Decidable` instance built by tactic** | `decide` fails on an obviously finite goal | Kernel cannot reduce the instance | Define instances structurally; `deriving DecidableEq` |
| **Committed `exact?` / `apply?`** | Leftover from interactive search | Slow, fragile, re-searches every build | Delete and inline the lemma it found |
| **A model with no bridge to the code** | Proving properties of a hand-written abstraction | The proof is about a document, not the shipped system | Differential random testing (Cedar) or verify the real code (SymCrypt) |
| **Reaching for Lean where TLA+ fits** | Concurrency, liveness, temporal properties | Weeks of induction for what a model checker answers in an afternoon with a trace | Counterexample vs proof: pick the tool for the one you need |
| **Parameterizing a finite model casually** | "Now let's do it for arbitrary `n`" | 1-3 days becomes 1-3 weeks | Name the cliff before crossing it |
| **Recommending Moogle, `llmstep`, LeanDojo v1, ReProver** | Stale tooling knowledge | Dead links and abandoned repos | Loogle (JSON API), LeanSearch, `lean-lsp-mcp` |
| **`partial` in the part you want to prove about** | Termination obligation is annoying | Nothing downstream can be reasoned about | `partial` at IO edges only |

---

## 13. Authorities

**People.** Leonardo de Moura (creator; Chief Architect, Lean FRO; also AWS) -- language design, `grind`. Sebastian Ullrich -- compiler and elaborator internals. Jeremy Avigad -- *Theorem Proving in Lean 4*, *Mathematics in Lean*, pedagogy. Kevin Buzzard -- Mathlib and the mathematical case; note he is a **Lean partisan**, not a type-theory critic, and his polemics target constructivism and ZFC's lack of universes. Mario Carneiro -- metatheory, `lean4lean`. Joachim Breitner -- GuessLex, well-founded recursion, tooling. Kim Morrison -- `grind`, Mathlib maintenance. Patrick Massot -- pedagogy, `Mathematics in Lean`. Floris van Doorn -- HoTT in Lean, the univalence-inconsistency result.

**Books and docs.** *Theorem Proving in Lean 4* (the canonical entry). *Functional Programming in Lean* (Christiansen; for the programming half, and the right starting point for an engineer). *Mathematics in Lean* (Avigad and Massot). *The Hitchhiker's Guide to Logical Verification*. The Lean Language Reference, including its "Validating Lean Proofs" section for high-trust applications.

**The real knowledge base is the Lean Zulip** (`leanprover.zulipchat.com`). Searchable, and the maintainers answer. Most non-trivial questions have already been answered there, and it is more current than any book.

**Tools.** Loogle (JSON API), LeanSearch, Comparator, Lean Kernel Arena (`arena.lean-lang.org`), `lean4checker`, `nanoda` (Rust kernel), `lean4lean`, `lean-lsp-mcp`.

**Route in for an engineer:** *Functional Programming in Lean* first (it meets you as a programmer), then `#guard`-based decidable modeling of something small and real, then *Theorem Proving in Lean 4* when you hit the first proof you cannot brute-force. Skip Mathlib entirely until you need mathematics.

---

## 14. Severity rubric (this domain)

- **blocker** -- A theorem that does not say what the caller needs: vacuous hypothesis, wrong quantifier order, `Nat`-subtraction or division-by-zero degeneracy, or a statement the model wrote and nobody reviewed. An axiom audit that cannot detect `sorry` or `native_decide` (the denylist bug). `native_decide` load-bearing in a security-relevant proof. A `sorry` reachable in shipped or published artifacts.
- **major** -- A model with no bridge to the implementation, presented as verifying the implementation. `partial` or `unsafe` in the reasoned-about core. A proof whose fragility is structural (bare `simp` in a library-facing lemma). Reaching for Lean where a model checker answers the question in an afternoon. Missing `#print axioms` in a project that claims verification.
- **minor** -- Committed `exact?` / `apply?`. `polyrith` or other dead tactics. Well-founded recursion where structural works. Mathlib dependency for a model that needs no mathematics.
- **nit** -- Proof style, naming against Mathlib conventions, `simp only` versus `simp` in a leaf proof.
- **insight** -- "This passes the feasibility gate and a one-day decidable model would settle the invariant you are arguing about." "This is the parameterization cliff; the cost is about to go up 5-10x." "TLA+ gives you a counterexample trace here, which is what you actually want."

---

## Source research

`~/.claude/local/research-notes/lean-proof-engineering-research.md` (1,030 lines; source-verified by cloning `leanprover/lean4` and `mathlib4` at HEAD and measuring directly, plus `gh api`, `curl` liveness probes, and `pdftotext` extraction of primary PDFs). Records what was verified against a primary source, what was not, and which routes were blocked.

Known gaps recorded there and not filled: the exact Lean version where the per-use `_native` axiom became user-visible (sources split 4.29 vs 4.31); whether the IMO 2026 Huawei/Xiaohongshu results were natural-language or formal; whether AxiomProver's six IMO 2026 *statements* are faithful (five of six divergence reports were agent-written -- nobody should cite "42/42 formally verified" without a human diff); Amazon's FRO donation amount; s2n effort figures; and the Veil "Lessons from Building an Auto-Active Verifier in Lean" paper (Dafny 2026), which failed text extraction and is the highest-value unread source for the honest-comparison question.

## Changelog

- **2026-09-18** -- File created. Lean v4.34.0. Research pass source-verified against lean4 and mathlib4 at HEAD.
