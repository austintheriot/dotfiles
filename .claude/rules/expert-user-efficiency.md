---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Expert User Efficiency

A reference for evaluating an interface from the point of view of the person who uses it every day for hours. Used by the `expert-user-efficiency` subagent. The scope is *the hundredth use, not the first*: how many actions a routine task costs, whether the hands leave the keyboard, whether the layout can be learned as a place rather than read as a page, whether repetition builds a habit or fights one, and whether the design's helpfulness has become a tax.

**This lens is deliberately adversarial to the novice-first defaults in `interaction-design.md`.** That is the point of carving it out. The two agents will sometimes reach opposite conclusions about the same screen, and the disagreement is the useful product: it forces the question "which kind of surface is this?" to be answered explicitly rather than assumed. Where they conflict, neither is automatically right, and a review that reports both positions with their reasoning is better than one that splits the difference.

The core thesis: **for a tool used daily, every cost is paid every day and every learning cost is paid once.** A confirmation dialog on a routine action costs one second times ten thousand repetitions. Whitespace that pushes the fifteenth row below the fold costs a scroll on every session forever. The arithmetic that justifies hand-holding on a signup form inverts completely on a working tool, and applying consumer-web guidance to professional software is the most common way good interfaces get made worse.

The second thesis, from Raskin: **the practiced user is not paying attention to your interface.** They are paying attention to their work. Every element that demands conscious thought steals from the task. The design goal is to become invisible through habit, and habit requires that the same action always produces the same result.

---

## 1. Who this lens serves

### Cooper's perpetual intermediates

Alan Cooper's observation in *About Face*: users of a working tool are not distributed as novices and experts. Almost nobody stays a novice for long, and almost nobody becomes a true expert. The overwhelming majority land quickly in a middle band and **stay there indefinitely** -- competent at the paths they use, ignorant of the rest, occasionally needing reference material for something they use twice a year.

The design consequence: optimizing for the first hour optimizes for a state the person leaves in a week. Optimizing for the theoretical expert optimizes for a state most never reach. Design for the perpetual intermediate: fast paths for what they do constantly, discoverable reference for what they do rarely, and no tax on either from features aimed at the first session.

### Tognazzini's frequency test

Tog's `Learnability` principle gives the decision rule directly: **identify frequency of use.** A product used once (a kiosk, a tax form, an onboarding flow) should prioritize learnability. A product used habitually should prioritize usability at the practiced state. His methodological point is sharper and routinely ignored: testing only the initial 20-to-60-minute learning curve measures the wrong thing for habitual software. He recommends having people spend weeks with an interface and measuring the *end-state* productivity.

He also rejects the framing that these trade off: "both learnability and usability can improve simultaneously; it's not an inherent trade-off." That is worth holding, because "we made it simpler" is often a claim that learnability was bought with usability and no one measured the second.

---

## 2. Raskin's Humane Interface

Jef Raskin's *The Humane Interface* (2000) is the deepest single source for this lens, and it is idiosyncratic enough that its positions should be stated as his rather than as consensus.

### Locus of attention

The person has exactly one locus of attention. Anything the interface does outside it is not perceived; anything that forcibly moves it is an interruption with a real cost. This is the foundation for everything else here: a warning that appears outside the locus is not read, and one that appears inside it steals the task.

### Modelessness

**A mode exists when the same gesture produces different results depending on hidden state.** Modes cause errors because the person acts from habit and habit does not consult state. Raskin's prescription is elimination: every action should always mean the same thing.

The test for whether something is a harmful mode: could the person perform the gesture without first checking a state they cannot see? If yes, it is a mode and it will produce errors.

### Quasimodes

Raskin's constructive alternative, and the most useful single idea in the book. A **quasimode** is a state maintained only by continuous physical action. The Shift key is the canonical example: you cannot forget you are in shift-mode, because your finger is holding it. Quasimodes give the power of modes without the error class, because the body remembers what the mind does not.

Practical consequences: prefer hold-to-activate over toggle-to-activate for anything transient (a temporary tool, a preview, a reveal). Where a toggle is unavoidable, make the state loud, persistent, and inside the locus of attention.

### Monotony

**There should be exactly one way to accomplish a given atomic task.** Raskin's argument is that multiple paths (button, menu item, shortcut, context menu, gesture) prevent automation of the action: the person has to choose, and choosing is conscious thought, and conscious thought is what habit is supposed to eliminate.

**This is his most contested position and it should be presented as contested.** Nearly all shipped software does the opposite, deliberately: a visible control for discovery plus a shortcut for speed is the standard accommodation of novices and experts on one surface, and Nielsen's heuristic 7 endorses exactly that. The strongest version of Raskin's argument survives anyway: *redundant paths have a cost that is never counted*, they multiply the surface that must stay consistent, and a fifth way to do something is almost certainly not paying for itself. Use it as a challenge to redundancy, not as a rule.

### Habituation, and the two-edged consequence

Habituation is the goal: the practiced user should execute without deliberation. But the same mechanism means:

- **Confirmation dialogs on routine actions stop working**, because the dismissal habituates along with everything else. `interaction-design.md` reaches the same conclusion from the confirmation-fatigue direction. Both lenses agree here.
- **Redesign has a real cost that is invisible to the designers.** Moving a control that thousands of people reach for without looking imposes a re-learning cost on every one of them, and it produces errors, not just complaints. A redesign should have to justify itself against that cost, and "it looks more modern" does not.
- **Raskin's position against blocking warnings** follows: if warnings are routine they are habituated, so a warning that matters must be rare. This aligns exactly with the confirmation-fatigue argument and is a good example of two traditions converging.

---

## 3. Quantitative modelling: GOMS and the Keystroke-Level Model

The reason this lens can produce arguments rather than opinions.

**GOMS** (Goals, Operators, Methods, Selection rules; Card, Moran and Newell) models a task as a sequence of operators with known durations. The **Keystroke-Level Model** is its simplified form (Card and Moran, 1980) and is directly usable in review.

### Operator times

| Operator | Time (seconds) | Meaning |
|---|---|---|
| **K** | varies, below | One keystroke |
| **P** | 1.10 | Point to a target with the mouse |
| **H** | 0.40 | Home the hands between keyboard and mouse |
| **M** | 1.35 | Mental preparation for the next step |
| **D** | 0.9n + 0.16l | Draw n straight segments of total length l |
| **R** | system-dependent | Waiting for the system |

K by typing skill: 0.08 (best typist, ~135 wpm), 0.12 (good, ~90 wpm), 0.20 (average skilled, ~55 wpm), 0.28 (average non-secretary, ~40 wpm), 0.50 (random letters), 0.75 (complex codes), 1.20 (worst case or unfamiliar keyboard).

### A worked example, because the numbers only matter applied

Take "accept the suggested answer and move to the next item," in a queue the person will do 900 times.

**Mouse path**: home to mouse (H 0.40) + mentally locate and prepare (M 1.35) + point to Accept (P 1.10) + click (K 0.20) + point to the next item (P 1.10) + click (K 0.20) = **4.35 seconds**.

**Keyboard path**: mentally prepare (M 1.35) + press the accept key (K 0.20) = **1.55 seconds**, with the advance happening automatically.

Difference: 2.80 seconds per item. Across 900 items that is **42 minutes**, on one pass, for one reviewer. That is the argument, and it is checkable. It is also why "add a keyboard shortcut" is rarely a nit in a high-volume surface.

### The model's real limits, which must be stated with it

- It predicts **expert, error-free, routine** execution only. It says nothing about learning, discovery, or recovery.
- It requires a pre-specified method; it cannot tell you which method a person will pick.
- Published error is roughly **21% root-mean-square**. Use it for comparing two designs, not for promising an absolute number.
- The single **M** operator (1.35s) aggregates all cognition into one constant, which is a fiction. Tasks with genuine decision content are badly served by it, and a queue where each item requires real judgment is exactly such a task. In that case the M term dominates and shaving keystrokes matters less than the design of the decision.

That last caveat is important and self-limiting: **KLM arguments are strongest for mechanical repetition and weakest where the person is actually thinking.** A reviewer who applies it to a judgment-heavy task and predicts a large saving is overstating.

### Fitts's Law, applied by an expert-efficiency reading

- Screen **edges and corners are effectively infinite targets** because the pointer stops there. Corners are the fastest acquirable points on a screen and are usually wasted.
- Reduce the **number** of acquisitions, not just the distance of each. Two nearby clicks can cost more than one distant one.
- Make destructive controls **small and far**; make high-frequency controls **large and near**. This is the same law that `interaction-design` cites, read for throughput rather than for error prevention.
- Pie and radial menus beat linear menus for a fixed small set, because every item is equidistant and direction is learnable as a gesture. Rarely worth the novelty cost, but worth knowing as a real option.

---

## 4. Keyboard-first design

- **A shortcut for every high-frequency action.** The threshold is frequency, not importance.
- **Single-key shortcuts where a text field is not focused** are the strongest form and are used by every serious high-volume tool. They require a considered focus model so that typing in a field never triggers them.
- **Follow the platform's established chords absolutely.** Overriding Cmd-W, Cmd-Q, Cmd-Z, Cmd-F, or Cmd-A is an error every time; the habit is stronger than any product's ability to retrain it, and this is Jakob's Law at its most literal.
- **Mnemonic beats positional** for learnability, positional beats mnemonic for speed. Where both are wanted, offer a primary mnemonic and let the person remap.
- **Discoverability of shortcuts**: display the shortcut beside its control in menus and tooltips, and provide one overview (the conventional `?`). Raskin would object to the redundancy; the accommodation is that the redundant path is the *learning* path, and the person abandons it.
- **Type-ahead in every list.** Typing in a focused list should jump to the match. This is a decades-old platform convention and it is missing from most custom lists.
- **A command palette** (typically `Cmd-K`) is the standard answer to the shortcut-memorization problem: it turns recall into recognition-plus-search, so the person needs to know only that the action exists. It is the right shape when the action count is large. It does not replace shortcuts for the ten things done constantly, because opening a palette to run the action you do every eight seconds is slower than a key.
- **Focus must be reachable and visible without a mouse for every interactive element**, which is where this lens and `accessibility` want the same thing for different reasons. Note the overlap and let `accessibility` own the conformance finding.

---

## 5. Density, spatial memory, and layout stability

The efficiency argument for density, which `visual-hierarchy.md` section 6 also carries:

- Practiced users locate information **by position**, not by reading. After a week, the eye goes to a coordinate. That is spatial memory, it is fast, and it is destroyed by layouts that reflow, reorder, or push content below the fold.
- **Consequences for layout stability**: do not reorder lists by relevance if position is a handle; do not collapse or expand sections automatically; do not change control positions between states; keep the same information in the same place across related screens.
- **Scrolling is not free.** An item below the fold is not merely slower to reach, it is invisible to peripheral awareness, and it drops out of the person's model of the current state.
- Compact and comfortable **density settings** are a legitimate resolution to the argument with the minimalist school and are cheap to build. Offering one, defaulting to the denser option for a professional tool, ends most density debates.

---

## 6. Selection, bulk operations, and repetition

High-volume work is dominated by acting on many things at once. The affordances that matter:

- **Range selection** (shift-click) and **additive selection** (modifier-click) in every list. Their absence turns a fifty-item operation into fifty operations.
- **Select-all-visible versus select-all-matching are different operations** and must be distinguishable. The classic defect: "select all" checks the page, the person applies an action, and only 50 of 3,840 items change, silently.
- **Report what a bulk operation did**, including what it skipped and why. Silence about skips is where trust in bulk operations dies.
- **Bulk operations need reversal**, and their scale is exactly why. See `interaction-design.md` section 7.
- **Repeat-last-action** is a cheap and underused accelerator.
- **Same-thing-again shortcuts**: after acting on item N, the person is almost always going to item N+1. Auto-advance, with an explicit way to turn it off, converts two actions into one.

---

## 7. Interruption and flow

- **Modal dialogs steal the locus of attention** and should be rare, reserved for things that genuinely cannot proceed. A modal that appears during routine work is a design that has decided its message matters more than the person's task.
- **Nothing should steal focus while the person is typing.** A dialog, toast, or auto-focus that arrives mid-keystroke will eat keystrokes and can trigger the dialog's default action.
- **Autosave, always**, with visible state. Tog's rule is absolute: never lose the person's work for any reason including their own error. Manual save in a working tool is an unnecessary mode.
- **Preserve position across navigation.** Returning from a detail view to a list should return to the same scroll position and the same selection. Losing it in a 900-item queue is a serious cost and a common defect.
- **Long operations should not block.** The person should be able to keep working while something runs, with its progress visible somewhere that is not in the way.

---

## 8. Where this lens is wrong, and should say so

Stating these is what keeps the agent honest rather than doctrinaire.

- **First-use and occasional-use surfaces.** Onboarding, invitations, account recovery, annual configuration. Frequency is the deciding variable and here it points the other way. Defer to `interaction-design`.
- **Irreversible, high-consequence actions.** Habituation is a hazard, not an asset, when the action deletes a corpus. Friction is correct there precisely because it is rare, and this lens's usual argument against friction does not apply.
- **Safety-critical and regulated surfaces**, where a deliberate step exists because someone must be accountable for it.
- **Anything an assistive technology user experiences.** Efficiency arguments never license removing a label, a focus indicator, or a text alternative. `accessibility` outranks this lens on every conflict, without exception.
- **Genuinely novel interactions** where no habit exists yet, and where the discoverability argument temporarily wins.
- **When the M term dominates.** If the task is real judgment rather than mechanical repetition, keystroke savings are marginal and the design of the decision matters more. Say so rather than producing a KLM estimate that overstates.

---

## 9. Schools of thought that genuinely disagree

### This lens versus "Don't Make Me Think"

Steve Krug's book is the clearest statement of the opposing position: self-evident, scannable, no thought required, and design for the person who arrives with no context. It is right about the web and about first encounters, and it has been applied to professional software with real damage. The two positions are not reconcilable in general; they are reconcilable per surface, by frequency of use. A review that does not name the surface type is going to produce advice from whichever tradition the reviewer happens to hold.

### Raskin versus Tognazzini on visibility

Raskin wants monotony: one path, habituated, invisible. Tog wants discoverability: controls visible at all times, and he explicitly attacks the "illusion of simplicity" achieved by hiding. They agree that hiding things for aesthetics is bad. They disagree about whether redundant visible paths are worth their cost. Tog's **progressive revelation** (hide the advanced path initially, reveal as competence develops) is the closest thing to a synthesis, and it is a different claim from progressive disclosure as usually practised, because the reveal is tied to demonstrated competence rather than to a chevron.

### Modal editing

The Vim and Emacs lineages sit on opposite sides of Raskin's own principle. Vim is aggressively modal and its users report large efficiency gains after a large learning cost; Raskin would predict exactly the error class modal editing is known for (typing text into normal mode). Emacs is modeless and chord-heavy, which trades the mode-error class for a memorization cost and for the RSI complaint. The honest summary is that **modes are demonstrably survivable when the mode indicator is strong and the user's investment is high**, which bounds Raskin's claim without refuting it. Neither is a template for general software.

### Customization

One camp holds that letting people remap and rearrange is the ultimate efficiency affordance. The other holds it fragments the product, breaks every piece of documentation and support, and mostly serves a small minority while the default stays badly designed because it can always be fixed by the user. Both are observably true. The reviewable position: **customization is not a substitute for a good default**, and a product whose answer to a layout complaint is "it is configurable" has usually not made the decision it should have.

---

## 10. Anti-pattern catalog

- **No keyboard path** for a high-frequency action.
- **Hijacked platform chord.**
- **Shortcuts that fire while typing** in a field.
- **No range or additive selection** in a list that supports bulk actions.
- **Select-all that means select-visible**, silently.
- **Confirmation on a routine reversible action.**
- **Focus theft** during typing.
- **Lost scroll position** on back-navigation.
- **Auto-reordering lists** where position is a handle.
- **Auto-collapsing sections** that the person re-opens every session.
- **Manual save** in a tool used continuously.
- **Modal blocking** for something that could be non-blocking.
- **Whitespace that costs a scroll** on the primary working surface.
- **No type-ahead** in a long list.
- **A wizard for a task an experienced person does weekly**, with no way to skip to the end.
- **Redesign that moves high-frequency controls** with no migration affordance and no stated benefit that outweighs the re-learning.
- **Hover-dependent controls** in a high-volume workflow, which force the mouse into the loop.

---

## 11. What to flag, and what not to

**Flag:**
- A high-frequency action with no keyboard path, ideally with a KLM estimate and the repetition count.
- Friction on a routine reversible action, named with the per-day cost.
- A missing bulk or selection affordance that turns one operation into N.
- Layout instability that destroys spatial memory, named against the specific working surface.
- Lost state (scroll, selection, filter, draft) across a transition.
- A mode with a weak indicator, and whether a quasimode would serve.
- Redundant paths that are not paying for themselves (Raskin's monotony, offered as a challenge and marked as contested).
- Platform-convention violations in shortcuts.
- A design that optimizes for the first session at the expense of the thousandth, with the frequency argument stated.

**Do not flag:**
- Anything on a genuinely first-use or occasional-use surface. Say `See also: interaction-design` and stop.
- Friction on irreversible high-consequence actions. That friction is correct.
- Accessibility affordances as overhead. Never.
- Density arguments in a consumer surface where the practiced-user premise does not hold.
- Aesthetics. `visual-hierarchy`.
- A KLM saving on a judgment-heavy task without acknowledging that M dominates.

Every finding names the frequency and the cost: not "add a keyboard shortcut" but "Accept is mouse-only; at roughly 4.4s per item by keystroke-level model against 1.6s for a single-key path, a 900-item queue costs about 42 minutes of pointing per pass."
