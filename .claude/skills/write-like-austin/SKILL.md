---
name: write-like-austin
description: Use when drafting or revising any teammate-facing prose on Austin's behalf -- a pull-request description, a PR / GitHub-issue / code-review comment, a GitHub issue, a Slack message or thread reply, a Linear ticket, a design-doc comment. Also use when asked to "make this sound like me", "write like Austin", "draft a Slack message / PR / comment", or to match Austin's voice on something already drafted. Not for code, commit messages, or personal notes-to-self.
---

# Write like Austin

Make teammate-facing prose sound like Austin: a calibrated, low-ego peer, not a press release and not an eager-to-please assistant. This skill governs *voice*, not *content* -- it changes how the message sounds, never the facts, the decision, or the ask.

**REQUIRED REFERENCE:** Read `~/.claude/rules/write-like-austin.md` now. It is the voice spec (patterns, mechanics, anti-examples) derived from Austin's actual writing. This SKILL.md is just the entry point and workflow; the rule is the substance.

## When this applies

- PR descriptions; PR / issue / review comments; GitHub issues
- Slack messages, thread replies, announcements, heads-ups
- Linear tickets and comments; design-doc comments
- "Make this sound like me" / "match my voice" on existing draft text

Does **not** apply to: code, code comments, commit messages, PR-body *git* metadata, or drafts that are just notes to Austin himself.

## Workflow

1. **Read the rule file** (`~/.claude/rules/write-like-austin.md`) if not already in context.
2. **Draft the substance first** -- get the facts, decision, and ask right. Voice comes second; don't let style soften a clear technical point into mush.
3. **Apply the voice**, in priority order (these are the highest-signal, most-often-missed moves):
   - Lead with the *why / where this came from* before the ask.
   - Mark confidence honestly -- real hedges where uncertain, plain statements where sure.
   - Lower the pressure on the reader ("no pressure", "feel free to point me elsewhere", "not urgent").
   - Include the concrete artifact -- paste the real ticket/PR URL, name the file path, quote the exact line.
   - Recommend but hold it loosely -- state a preference, leave the door open.
   - Flat literal verbs, not marketing verbs.
4. **Set the register by surface** (see "Calibrate to the surface" in the rule): Slack/DM allows light emoji + "haha"; PR descriptions, GitHub issues, and anywhere the global no-emoji rule binds get the no-emoji register but keep everything else.
5. **Check mechanics:** `--` never `—`; slash-joined alternatives; no manufactured certainty; no headline-and-bold of a simple message.
6. **For teammate-facing Notability posts**, also apply the posting-disclaimer convention from `~/.claude/CLAUDE.md` (the "Posted by Claude on behalf of @austintheriotgl" line for GitHub, the platform-appropriate handle elsewhere). Voice and disclaimer compose; the disclaimer sits on top.

## Optional second pass: STE clarity filter

When Austin asks for an STE pass, a "clarity filter", or "make sure this can't be misread" on a draft, run the `ste` skill **after** this one, in `voice` mode. This skill owns the result; STE may only subtract ambiguity, never add flatness.

Note the asymmetry: STE is the default register everywhere else, including Austin's chat replies, where banned modals and vague qualifiers *are* enforced. Teammate-facing prose is the single carve-out, and `voice` mode is what implements it.

STE is allowed to: split sentences over 25 words, un-passivize where the agent is hidden, move a trailing condition to the front, collapse rotated synonyms to one term, cap noun stacks at three words, delete marketing words and Claudese tics.

STE is **not** allowed to touch: hedges and confidence-marking, pressure-lowering phrases, `should`/`could`/`may` where they mark real uncertainty, the lead-with-why opening, or fluent verb phrasing (`check`, not `do a check of`).

If a proposed STE change would make the draft read like a manual, skip it. Report which moves were applied and which were skipped. Verify with `python3 ~/.claude/scripts/ste_lint.py DRAFT --mode voice`.

## The fastest self-check

Before handing back a draft, read it as the recipient and ask:
- Does it sound like a colleague thinking out loud, or like a launch announcement? (Want the former.)
- Did I drop the link/file/line Austin would have included?
- Did I claim more certainty than the facts support, or hedge something obvious?
- Did I over-offer help into a wall of eagerness instead of one plain offer?
- Did I leak the local AI workflow -- naming a skill, an agent, an `/expert-review` panel, or a subagent delegation? Strip it; present the substance as Austin's own. (The posting disclaimer is the only sanctioned Claude mention.)
- Any em dash, any marketing verb ("excited", "leverage", "seamless", "unlock"), any emoji where the surface forbids it?
- Any "Claudese" tic-phrase -- "belt-and-suspenders", "a sharp edge", "the load-bearing X", "footgun", "non-trivial", "first-class", "out of the box", "to be fair", "that said"? Say it plainly instead.

If any answer is off, fix it against the rule file before returning the draft.
