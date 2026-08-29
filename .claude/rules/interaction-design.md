---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Interaction Design

A reference for evaluating a user interface from an interaction-design lens. Used by the `interaction-design` subagent. The scope is *what happens when a person touches the thing*: whether an action is discoverable, whether the system says what it is doing, what every state of every view looks like, what happens when the user is wrong, what happens when the system is wrong, and whether the person can get back.

Distinct from `visual-hierarchy.md` (whether the eye goes to the right place), `information-architecture.md` (whether the thing can be found at all), `content-design.md` (whether the words do their job), `accessibility.md` (whether it conforms to Web Content Accessibility Guidelines and works for assistive technology), `expert-user-efficiency.md` (whether the practiced daily user is fast), and `browser-spec.md` (whether a platform primitive was reinvented).

The core thesis: **most interaction defects are missing states, not wrong pixels.** A screen has a designed happy path and four or five undesigned others. The reviewer's first question on any view is not "does this look right" but "what does this look like when the list is empty, when the request is still in flight, when it failed, when the user lacks permission, and when there are ten thousand rows."

The second thesis, from Norman: **the person is not at fault.** When someone makes an error, that is a design finding, not a user finding. Ask what the design invited.

---

## 1. Norman's model, and why it is the foundation

Don Norman's *The Design of Everyday Things* (1988, revised 2013) supplies the vocabulary every other framework here assumes.

### Affordance is not the same as signifier

This distinction is the single most-mangled idea in the field, and Norman added the second term in the 2013 revision *specifically because* practitioners had been misusing the first for twenty-five years.

- An **affordance** is a relationship between an object and an actor: what actions are *possible*. A `div` with a click handler affords clicking whether or not anyone can tell.
- A **signifier** is the perceivable signal that communicates where the action should happen. The underline, the button chrome, the cursor change.

"This button has no affordance" is almost always wrong. The action is possible; it is **unsignified**. Write the finding as a missing signifier, because that names the fix. The commonest real instance: a clickable card, row, or icon with no hover state, no cursor change, and no border, discoverable only by accident.

### The two gulfs

- **Gulf of execution**: the person knows what they want and cannot work out how to do it. Fixed by signifiers, discoverability, and sensible mapping.
- **Gulf of evaluation**: the person did something and cannot work out what happened. Fixed by feedback and visible system state.

Almost every interaction finding lands in one gulf or the other. Naming which one sharpens the finding and points at the class of fix.

### Slips versus mistakes

Norman's error taxonomy, and it determines the remedy:

- A **slip** is right intention, wrong action. The person meant to archive and hit delete because the buttons are adjacent and identical. Remedy: **constraints and layout** -- separate the targets, differentiate them, add undo. Confirmation dialogs work poorly against slips because the slip is an attention failure and the dialog gets the same inattention.
- A **mistake** is wrong intention, formed from a wrong mental model. The person deleted the record believing it was a draft. Remedy: **better system model, better labels, better visible state**. Confirmation with specific consequences ("this deletes 412 tagged tokens permanently") does help here, because it corrects the model at the moment of action.

If a reviewer cannot say whether a hazard produces slips or mistakes, the proposed fix is a guess.

### Mapping, constraints, conceptual model

- **Mapping**: the spatial or logical correspondence between control and effect. Stove-top burner controls are the canonical example. In software: does the panel that edits the selected item sit next to the selected item, or across the screen?
- **Constraints**: physical, logical, semantic, cultural limits that make the wrong action impossible rather than merely discouraged. A date picker that cannot express 31 February beats validation that rejects it.
- **Conceptual model**: the story the interface tells about how it works. Users build one whether or not you supply one, and they build it from what is visible. An interface that hides its model gets a wrong one invented for it.

---

## 2. Nielsen's ten heuristics, with the practical test for each

Jakob Nielsen and Rolf Molich, 1990; refined 1994 by factor analysis over 249 usability problems; wording updated 2020; the ten themselves unchanged since 1994. They are deliberately "broad rules of thumb and not specific usability guidelines," which is both their strength (they still apply to interfaces that did not exist in 1994) and their weakness (they do not settle arguments on their own).

For review, each is only useful with a concrete test attached:

1. **Visibility of system status.** Test: after every user action, name the pixel that changed. If none, the finding writes itself.
2. **Match between system and the real world.** Test: is any label a word from the schema rather than a word from the domain? `corpus_reference_conflict` is a table name; "conflict" is a word a person uses.
3. **User control and freedom.** Test: for each state, name the exit. A view a person can enter and not leave without the browser back button is a finding.
4. **Consistency and standards.** Test: does this control behave like the same-looking control two screens over? Internal inconsistency is worse than deviation from an external standard, because the person has already learned the internal one.
5. **Error prevention.** Test: for each error message in the codebase, ask what constraint would make that message unreachable.
6. **Recognition rather than recall.** Test: does completing this step require remembering something from a previous screen? Identifiers, codes, and settings copied from elsewhere are the usual offenders.
7. **Flexibility and efficiency of use.** Test: is there any accelerator at all? See `expert-user-efficiency.md`; this heuristic is the one place Nielsen's framework acknowledges the expert, and it is thin there on purpose.
8. **Aesthetic and minimalist design.** Test: does every element on screen earn its place *for this task*. Note carefully: Nielsen's own gloss is that this "doesn't require flat design" and is about keeping essentials visible, not about removing density. See section 11 on why this heuristic is the most frequently weaponized.
9. **Help users recognize, diagnose, and recover from errors.** Test: does the message name what happened, why, and the next action, in the user's words, with no error code as the only content?
10. **Help and documentation.** Test: is help available at the point of need, or only in a separate manual nobody opens? Nielsen's own caveat is that needing it at all is a smell.

**Heuristic evaluation as a method** finds different problems than user testing does and neither substitutes for the other. Its known weakness is a high false-positive rate: evaluators report problems that no real user hits. Confidence scoring in `panel-contract.md` is the defense; a heuristic violation with no plausible user consequence is below the reporting bar.

---

## 3. The "laws," which ones carry weight, and which are folklore

The Laws of UX collection is genuinely useful and genuinely over-applied. Encode both.

### Load-bearing, with real evidence

- **Fitts's Law.** Time to acquire a target is a function of distance and target size. Paul Fitts studied physical controls in 1954; it transfers to pointers and to touch. Practical consequences: screen edges and corners are effectively infinite-size targets because the pointer stops there (the macOS menu bar is the canonical exploitation, and a Windows-style menu one pixel below the screen top is the canonical waste); make destructive controls small and distant, primary controls large and near; reduce the *number* of acquisitions, not only the distance of each.
- **Doherty Threshold (1982).** Productivity rises sharply when the system responds in under 400ms, because the person stays in a tight loop with the machine rather than context-switching. This is the number behind "make it feel instant," and it is a much more aggressive bar than "under a second."
- **Jakob's Law.** People spend most of their time on other products, so they expect yours to work like those. This is the strongest single argument against inventing a novel control for a solved problem, and the strongest argument for platform convention. It cuts against novelty for its own sake.
- **Hick's Law.** Decision time rises with the number and complexity of choices. Correctly applied to *decision points* (a menu of unfamiliar options), not to scannable lists of familiar labels, where scanning is parallel and the law's logarithm does not describe the task.
- **Peak-End Rule.** People judge an experience by its most intense moment and its end, not its average. Practical: the error path and the final confirmation carry disproportionate weight relative to their frequency.
- **Serial Position Effect.** First and last items in a series are best remembered. Practical: put the most important navigation item first or last, not third.

### Real but routinely misapplied

- **Miller's Law is the worst offender in the field.** George Miller's 1956 paper measured short-term *recall* of unrelated items, with nothing visible to look at. It says nothing about how many items belong in a visible menu, because a visible menu is *recognition*, not recall, and the options never enter working memory in the first place. Later work (Cowan and others) argues the real capacity is closer to four than seven once chunking and rehearsal are controlled. Miller himself described the number as close to an allegory. **Flagging a nine-item navigation menu by citing Miller is a defect in the review, not in the design.** If the menu is genuinely too long, the argument is Hick's Law (decision cost) or information architecture (wrong grouping), and it needs the actual labels to make.
- **Tesler's Law (conservation of complexity).** Every system has irreducible complexity; the only question is who absorbs it, the user or the developer. Useful as a rebuttal to "just simplify it," which usually means "move the complexity onto the user."
- **Aesthetic-Usability Effect.** Kurosu and Kashimura (1995) tested 26 ATM interface variants with 252 participants and found aesthetic rating correlated more strongly with *perceived* ease of use than with *actual* ease of use; Tractinsky replicated across cultures and summarized it as "what is beautiful is usable." The reviewer-relevant consequence is the dangerous one: **a beautiful interface hides usability problems during evaluation**, both from users in testing and from the reviewer. Later work is mixed on when the effect holds, and a 2023 CHI paper found controlling for processing fluency reduces it. Treat it as a warning about your own judgment, not a licence to prioritize polish.

### Cite a law only when it changes the recommendation

A finding that names a law but would read identically without it is decoration. The law earns its place when it supplies a threshold (400ms), a mechanism (edge targets), or a rebuttal (Tesler against false simplification).

---

## 4. State completeness: the highest-yield lens in this file

Scott Hurff's formulation is the standard five: **blank, loading, partial, error, ideal**. Most interfaces are designed for "ideal" alone. Expand it, because several of these have genuinely different designs:

| State | The design question | Common defect |
|---|---|---|
| **Empty, first use** | Why is this empty and what is the one next action? | A blank panel, or copy that says "No items" and nothing else |
| **Empty, by filter** | Different from first use. The data exists; this query found none | Reusing the first-use empty state, which tells the user to create something they already have |
| **Empty, permanently** | This is correct and final. Say so | Rendering it as a failure or a to-do |
| **Loading, first paint** | Skeleton shaped like the result, so nothing shifts when it arrives | A centered spinner that discards known layout information |
| **Loading, refresh** | Existing content stays, with a subtle in-flight signal | Blanking content the user was reading |
| **Loading, one control** | Spinner on that control, that control disabled, rest of page live | Blocking the whole page for one button |
| **Partial** | Some data arrived, some has not, some failed | Showing partial as though it were complete, which is the most dangerous of these |
| **Error, retriable** | What failed, in plain words, plus a retry that actually retries | Raw exception text, or a toast that vanishes before it is read |
| **Error, terminal** | What failed, why retrying will not help, what to do instead | A retry button that will never succeed |
| **Too much** | 10,000 rows: pagination, virtualization, or a forced filter | A layout tested at 5 rows and shipped |
| **Offline or stale** | Is what I am looking at current? | Silently showing cached data as live |
| **Unauthorized** | You cannot do this, and here is why or who to ask | A disabled control with no explanation, or a 403 page |

**A disabled control with no explanation is a finding every time.** The person cannot tell whether it is broken, whether they lack permission, or whether a precondition is unmet. Either explain on hover and adjacently, or do not render it.

**Toasts for anything the user must retain is a finding.** A timed notification carrying a count, a report, or an error is a design that assumes the person was looking. Use a persistent inline element anchored to the thing that produced it, dismissible by the person rather than by a timer.

---

## 5. Feedback and time

Three sets of numbers, from three sources, that do not conflict:

**Nielsen's response-time limits** (*Usability Engineering*, 1993, unchanged since):
- **0.1s** -- feels instantaneous. No feedback needed beyond the result.
- **1.0s** -- the person notices but their train of thought survives. No special feedback needed, but the delay is perceptible.
- **10s** -- the outer limit of held attention. Past this, give a percent-done indicator and let them do something else.

**Tognazzini's latency rules** (*First Principles of Interaction Design*), which are tighter at the low end:
- Acknowledge every button press within **50ms** with some visual or audible change. This is about the tactile illusion, not about the operation completing.
- Under 1s: show nothing. 1-5s: progress bar. 5-15s: progress bar plus a message. Over 15s: something that can recover attention from across the room.
- Trap repeat clicks so an impatient user does not queue five submissions.

**Doherty's 400ms**, above, is the productivity threshold rather than the perception threshold.

**Skeletons versus spinners.** A skeleton shaped like the incoming content preserves layout and communicates structure and rough duration; a spinner communicates only that something is happening. Prefer skeletons where the shape is known. Do not use a skeleton where the shape is not known, because a skeleton that does not match the arriving content causes a worse layout shift than a spinner would have.

**Optimistic updates** buy perceived speed and owe a rollback design. The reviewer's question is always: what does the screen do when the write fails after the person has moved on? An optimistic update with no visible reconciliation path is a correctness defect wearing a performance costume.

---

## 6. Errors: prevention, then messages

Ordered by strength, strongest first:

1. **Make the error unrepresentable.** A picker cannot express an invalid date. A typed identifier cannot be mistyped if it is selected rather than typed. This is the parse-don't-validate principle at the interface layer, and it is why `~/.claude/rules/coding-style.md` and this file agree.
2. **Constrain the input.** Input masks, min and max, sensible defaults, disabled-until-valid where the reason is visible.
3. **Warn before the commit**, with the specific consequence stated.
4. **Make it reversible.** See section 7.
5. **Only then, message well.**

### Validation timing, with evidence

Baymard Institute's testing found inline validation reduces form errors substantially, **but only when it fires on blur (field exit), not on keystroke.** Validating while the person is still typing tells them their half-entered email is invalid, which is both true and useless, and it reads as nagging. The rules that follow from this:

- Validate on blur for a field the person has left.
- Validate on submit for fields never touched.
- Once a field has errored, re-validate on input so the error clears the moment it is fixed. Making someone blur again to clear an error they have already corrected is its own small cruelty.
- Positive confirmation (a check on a valid field) has measurable value in long forms; it is not just decoration.

### Error message anatomy

Four parts, and a message missing any of them is incomplete:

1. **What happened**, in the person's vocabulary.
2. **Why**, when the reason is actionable. Skip when it is not.
3. **What to do next**, as a specific action, ideally as a control right there.
4. **What was preserved.** Never make someone retype work. Tog's rule is absolute here: restore the whole form on a validation failure.

State the requirement, not only the violation: "Password must be at least 8 characters" beats "Password too short." Never surface an exception, a stack, a status code alone, or a correlation identifier as the entire message. A correlation identifier is a fine *addition* for support.

---

## 7. Undo versus confirm

The governing rule: **friction should be proportional to how much damage the action does and how hard it is to reverse.** Concretely:

| Action | Right treatment |
|---|---|
| Reversible, low cost (archive, hide, reorder) | No friction. Do it, offer undo |
| Reversible, high cost (bulk edit over thousands of rows) | Do it, offer undo, and report exactly what changed |
| Irreversible, low cost | Undo where the system can synthesize one; otherwise a light confirm |
| Irreversible, high cost (permanent delete, close account, send to people) | Confirm with the specific consequence spelled out, and consider requiring the person to type the name of the thing |

**Confirmation fatigue is a real and measurable failure.** A blanket "Are you sure?" on routine actions trains people to dismiss without reading, which spends the one confirmation that mattered. Every confirmation dialog in a codebase makes every other confirmation dialog weaker. That is the argument for auditing them as a set rather than one at a time.

**Undo is strictly better than confirm where it is possible**, because it costs nothing on the happy path and it fixes slips, which confirmation does not. An append-only or event-sourced backend makes undo cheap; where the backend already supports compensating writes, a confirm-only design is leaving the better pattern on the table.

Watch the specific anti-pattern of **a confirm dialog whose default focus is the destructive button**, which converts a slip into a completed destruction with two identical keystrokes.

---

## 8. Motion

Motion is a clarifying layer, not decoration. Every animation should answer one of: where did this come from, where did it go, what is related to what, or is the system working.

**Duration.** The convergent guidance across Material, Fluent, and Apple's HIG:
- ~100ms for micro-interactions (hover, press, toggle).
- 200-300ms for standard transitions; Material's reference is 200ms for standard, 300ms for inter-screen.
- Never above 500ms for functional user interface motion. Past that the animation is latency the person did not ask for.
- Larger objects and longer distances take proportionally longer; small elements moving far look wrong at the same duration as small elements moving a little.

**Easing.**
- `ease-out` for elements entering. They arrive quickly and settle.
- `ease-in` for elements leaving. They start slowly and accelerate away.
- `ease-in-out` for elements moving within the screen.
- Linear only for continuous indeterminate motion (a spinner) and for opacity in some cases. Linear on a moving object reads as mechanical.

**Reduced motion.** `prefers-reduced-motion` is set for vestibular disorders as often as for taste. The rule that is routinely got wrong: **reduced motion means less motion, not no motion.** Replacing a slide with a cross-fade is correct; removing all state-change feedback is a regression, because the person loses the signal that anything happened. See `accessibility.md` for the conformance angle; this file owns whether the fallback still communicates.

**Anti-patterns.** Animation on every state change (the eye has nowhere to rest); entrance animations on content the person scrolled to deliberately, which delays what they asked for; parallax and scroll-jacking, which break the scroll contract the person already knows; and animating a list reorder so slowly that the person loses the item they were tracking.

---

## 9. Deceptive design, and why it is now a compliance surface

Harry Brignull coined "dark patterns" in 2010 and named twelve; the field now prefers "deceptive design." This used to be an ethics section. It is now a legal one.

The named patterns worth recognizing in review: **roach motel** (easy in, hard out), **confirmshaming** ("No thanks, I like paying full price"), **misdirection** (visual weight pushing the choice that benefits the vendor), **sneak into basket**, **hidden costs revealed late**, **forced continuity**, **privacy zuckering**, **disguised ads**, **trick questions**, **bait and switch**, **friend spam**, **nagging**.

Current enforcement, which makes these findings rise in severity:

- The EU **Digital Services Act** (2022) Article 25 prohibits interface design that deceives or manipulates. Formal Article 25 proceedings have been opened, including against X over the deceptive design of the verification badge, and a coordinated complaint against SHEIN (June 2025) over pre-ticked options, false urgency, and hidden subscription consents.
- The US **Federal Trade Commission** pursued cancellation symmetry through the Negative Option Rule. The 2024 click-to-cancel rule was **vacated by the Eighth Circuit in July 2025 on procedural grounds**, but cancellation-symmetry obligations remain enforceable under Section 5 of the FTC Act, ROSCA, and state auto-renewal statutes. In September 2025 the FTC secured a 2.5 billion dollar settlement with Amazon over Prime enrolment and cancellation design.
- The European Commission has signalled a **Digital Fairness Act** proposal for Q4 2026, aimed at dark patterns, addictive design, and unfair personalization.

Practical review rules: cancellation must be no harder than sign-up and reachable by the same channel; a pre-ticked consent is a defect; asymmetric visual weight between accept and reject on a consent surface is a defect; countdown timers that reset or do not reflect real scarcity are a defect. Route the lawfulness question to `app-privacy-compliance`; this lens owns the interface shape.

---

## 10. Forms

The concentrated version, because forms are where interaction defects cluster:

- One column. Multi-column forms produce ambiguous reading order and measurably worse completion.
- Labels above fields, always visible. **A placeholder is not a label**: it disappears on focus, fails contrast almost everywhere, defeats autofill, and leaves the person with a filled field and no idea what it holds.
- Group related fields with visible grouping, and match the group to the person's mental chunking (address as one block, not seven equal siblings).
- Ask for the minimum. Every optional field is a small tax on everyone.
- Mark optional fields rather than required ones when most are required, and the reverse when most are optional. Marking everything is the same as marking nothing.
- Correct `autocomplete` attributes and correct `inputmode` are interaction quality, not just accessibility.
- Never destroy input on error, on navigation, or on session expiry.
- Submit buttons say what they do ("Create book," not "Submit").
- Disabled submit until valid is defensible only when the reason for the disabled state is visible; otherwise it is a dead end.

---

## 11. Schools of thought that genuinely disagree

Preserve these. A review that flattens them produces bland advice.

### Novice-first minimalism versus expert-first density

- **The Nielsen and Krug tradition** (heuristics, *Don't Make Me Think*) is derived largely from consumer web usability with first-time and occasional users. Its default answers: fewer options, more whitespace, self-evident labels, do not make the person think.
- **The Cooper, Raskin, Tufte, and Tognazzini tradition** targets the practiced daily user of a professional tool. Cooper's "perpetual intermediates" argues most users of a working tool are neither novices nor experts and stay that way, so optimizing for the first hour is optimizing for the wrong hour. Tufte argues for maximum information density. Raskin argues for habituation, which requires stability and repetition rather than guidance.
- **They give opposite advice on the same screen.** For a tool someone uses eight hours a day, whitespace that forces scrolling is a cost paid every day; for a signup flow used once, density is a wall.
- The resolution is not a midpoint. It is to **identify which kind of surface this is before critiquing it**, and to say so in the finding. Route the expert-tool case to `expert-user-efficiency`, which argues that side deliberately.

### Nielsen's heuristic 8 versus Tognazzini on hidden controls

Nielsen: remove what is not needed; every extra unit competes. Tognazzini: hiding controls creates an "illusion of simplicity" that damages real productivity, and if the person cannot find a feature it does not exist. Both are right about different failures. Tog's own synthesis is **progressive revelation**: hide the advanced path initially and reveal it as competence develops, rather than hiding it permanently.

### The evidence on progressive disclosure is thinner than its popularity

Carroll and Rosson's "training wheels" work (1987) is the origin and its authors noted limited empirical support. The known failure modes are real: the expand affordance is too discreet to be found; collapsing frequently-used expert controls slows the people who use them most; and "Advanced" becomes a junk drawer for every feature nobody wanted to argue about. Require any disclosure decision to name the evidence (frequency, risk, complexity) and to name its discoverability plan.

### The "five users" question

Nielsen and Landauer's model, popularized as "why you only need to test with five users" (2000), was contested hard: Spool and Schroeder (2001) found the first five users surfaced roughly 35% of the problems a larger set found; Perfetti and Landesman (2002) found participants 6 through 18 each surfaced five or more new problems; Faulkner (2003) sampled random sets from 60 users and found sets of five ranged from 55% to 99% of problems found, with ten users raising the worst case to 80% and twenty to 95%. The honest position: five users per iteration is a good rule for *iterative, formative* testing of a narrow task; it is not a defensible sample for a summative claim, and the variance between any two sets of five is the part usually omitted.

### Testing validates, it does not design

Cooper's persistent argument. Usability testing tells you the current design fails; it does not tell you what to build instead, and a design arrived at purely by fixing test failures converges on a local optimum. Relevant when a review's recommendation is "test it" -- that is sometimes the right answer and sometimes an evasion of the design question.

---

## 12. Anti-pattern catalog

Recognize on sight:

- **The undesigned empty state.** Blank space, or "No data."
- **The page-level spinner** that replaces content the person was already reading.
- **The toast carrying information.** Counts, reports, errors, anything that must be retained.
- **The unexplained disabled control.**
- **The dead end.** A view with no exit, an error with no next action, a wizard with no back.
- **The lying optimistic update.** Success shown, write failed, nobody told.
- **Confirmation on everything**, which is confirmation on nothing.
- **Destructive default focus.**
- **Placeholder as label.**
- **Validation on keystroke.**
- **The reinvented native control** with none of the keyboard behavior. Route the platform-primitive half to `browser-spec`; this lens owns the missing behavior.
- **Hover-only affordances**, which do not exist on touch and do not exist for keyboard users.
- **The mystery-meat icon**, a bare glyph with no label and no tooltip, where the icon is not one of the dozen universally learned ones.
- **Modal stacking**, a dialog opening a dialog, where the escape key's meaning becomes ambiguous.
- **The novel control for a solved problem.** Jakob's Law is the argument.
- **Silent truncation.** Displaying the first N of M without saying M exists.

---

## 13. What to flag, and what not to

**Flag:**
- A missing state from the section 4 table, with the concrete condition that produces it.
- An irreversible action with no undo and no proportional confirmation.
- Feedback absent after an action, or slower than the section 5 thresholds where the code makes the latency knowable.
- Validation timing that fires on keystroke.
- An error message missing what happened, what to do, or what was preserved.
- A deceptive-design pattern, at raised severity given section 9.
- A dead end.
- Work destroyed by an error path.

**Do not flag:**
- Aesthetic preference. That is `visual-hierarchy`, and even there it is a judgment call.
- Contrast ratios, focus order, ARIA. That is `accessibility`. Say `See also: accessibility`.
- Copy quality. That is `content-design`, unless the copy failure *is* the interaction failure (an error message with no next action is this lens; an error message that is merely wordy is not).
- Density, in an expert tool, on minimalist grounds alone. Say what task suffers, or leave it.
- A missing state in a surface that genuinely cannot reach it. Name the condition or drop the finding.
- Anything a linter or type checker catches.

Every finding names the state or the moment: not "the loading experience is poor" but "when `fetchQueue` rejects, this panel renders the empty state, so a failed load is indistinguishable from an empty queue."
