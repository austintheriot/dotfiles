---
name: one-by-one
description: Walk through a list of items (review findings, design options, PR triage, candidate approaches, etc.) one at a time, accumulating decisions silently, and only synthesize at the end. Use when the user wants to deliberate each item before any cross-item recommendation. Invokable mid-conversation to pivot from "here's a synthesized recommendation" back into deliberation mode. Domain-general; works on any list that has already been produced or that you're about to produce.
---

# One-by-One Discussion

Deliberate each item in a list before synthesizing across them. The user calls when (a) you just produced a list and synthesis, and they want to back up into per-item discussion, or (b) they want to set the mode in advance for the next list-shaped output.

This skill is the conversational counterpart to the user's `feedback_one_by_one_discussion.md` memory: the memory is the default preference, this skill is the explicit invocation.

## When to use

- The user types `/one-by-one`, "let's discuss each one-by-one," "walk me through them," "stop giving me the full list," or similar.
- You're about to produce N items (N >= 3) and want to set expectations.
- The user has rejected an unsolicited synthesis and wants to back up.

## The contract

1. **Identify the list.** It's almost always the most recent multi-item output: review findings, design options, candidate refactors, items in a triage, alternatives in a brainstorm. If ambiguous, ask which list.

2. **Re-present the list briefly.** One line per item, numbered. No prioritization, no skip-list, no recommendation. This re-orients the user before per-item prompts begin.

3. **Walk one item at a time, using `AskUserQuestion` per item.** For each item:
   - Write a short summary of the item (context, tradeoffs, what's at stake, your opinion if relevant) as the question text.
   - Offer decision options: `action-now`, `defer`, `modify`, `drop`, `more-info`, `discuss`. Tailor the option labels and descriptions to the item where it helps -- e.g., for a review finding, `fix`, `defer`, `wontfix`, `discuss` may read more naturally. The user can always type a free-form response instead of picking; flag this implicitly by keeping option count small and option text specific.
   - Record the decision in a running tally (in your head / in a TaskList if it helps).
   - Move to the next item once the current one is resolved, either by their pick or by their free-form answer indicating they're done.

4. **Do not synthesize mid-walk.** Even if patterns emerge across items, don't surface them until the walk is complete. The user is deliberately choosing breadth-first deliberation over depth-first synthesis; respect the order.

5. **At the end, surface the accumulated decisions.** Format:
   ```
   Decisions from the walk:
   - #1: <decision>
   - #2: <decision>
   ...
   What do you want to action first?
   ```
   Then -- and only then -- offer prioritization if the user wants it.

## What this skill is NOT

- **Not a TaskList wrapper.** TaskList tracks work; this skill tracks deliberation. The decisions surfaced at the end may *become* tasks, but the skill itself doesn't write tasks.
- **Not for solo claude work.** If you're working autonomously through a checklist with no user-in-the-loop, just do the items.
- **Not a synthesis-suppressor for everything.** Direct questions ("which of these is most critical?") still get direct answers. The skill applies to *list-of-items* output, not to all output.

## Recovery patterns

The most common entry is mid-conversation: you produced a list with a synthesis at the bottom, the user pushed back. Behavior:

- Acknowledge briefly. ("Got it -- backing up.")
- Restate the list without the synthesis.
- Open the first `AskUserQuestion` with #1's summary and decision options.

Do not re-apologize on every subsequent item. The user wants the pattern, not the meta-conversation about it.

## Interaction with other skills

- **`/expert-review`**: produces a severity-ranked list of findings. This skill can pick up that list and walk it.
- **`/expert-plan`**: produces a list of grilling questions per cycle. This skill can walk those.
- **`/system-design` / `/oo-design` / `/fp-design`**: produce options with tradeoffs. This skill walks them.
- **Ad-hoc lists**: "I have 5 concerns about this PR" -- this skill walks them.

## Anti-patterns

- Listing all items, then jumping into deep discussion of #1 without an explicit "starting with #1, OK?" beat. The user may want #3 first.
- Producing the per-item discussion *and* a running synthesis ("so far it looks like..."). The synthesis comes only at the end.
- Treating the walk as a script. The user can re-order, skip, branch, or ask follow-ups; follow them.
- Forgetting decisions across items. Track them; surface them faithfully at the end.
