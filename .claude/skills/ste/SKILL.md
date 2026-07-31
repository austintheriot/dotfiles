---
name: ste
description: Use when asked to write, rewrite, or check prose in Simplified Technical English (STE / ASD-STE100) -- "rewrite this in STE", "STE-ify this", "check STE compliance", "simplify this doc", "make this unambiguous". Also the optional second pass for /write-like-austin when Austin asks for a clarity filter on a draft. Applies to documentation, runbooks, error messages, release notes, migration guides, API reference, alert text, and agent-facing prose (tool descriptions, system prompts). Not for marketing copy or anything where persuasion is the point.
---

# Simplified Technical English

Rewrite prose so it cannot be misread. STE is a controlled language: a closed vocabulary plus a closed grammar, built by the aerospace industry so a non-native reader under time pressure cannot misinterpret a maintenance instruction.

**REQUIRED REFERENCE:** Read `~/.claude/rules/simplified-technical-english.md` now. It carries all 53 rules, the restricted-meaning traps, and the "what transfers / what doesn't" split. This file is the workflow only.

## Pick a mode first

| Mode | Use for | Word cap | Modals + hedges |
|---|---|---|---|
| `procedural` | steps, runbooks, instructions, safety text | 20 | enforced |
| `strict` | agent-facing prose, error messages, API reference | 25 | enforced |
| `prose` | docs, PR bodies, commit bodies, chat replies to Austin | 25 | enforced |
| `voice` | teammate-facing drafts (the `/write-like-austin` carve-out) | 25 | **off** |

`voice` is the only mode that permits `should`/`may`/`might`/`could` and vague qualifiers. Use it solely for prose a teammate reads, where the social work of a hedge is the point.

**Everywhere else, including ordinary conversation with Austin, uncertainty is stated as a fact, not smoothed into a modal.** Do not delete the confidence signal; convert it. "This is probably the cause" becomes "This is the likely cause. I did not confirm it." "It's roughly 90" becomes "It is 87" or "I did not count them." Plain declarative admissions ("I am not sure", "I did not verify this", "I was wrong", "this is a guess") pass STE and must survive every rewrite. Never manufacture certainty to satisfy the register.

## Workflow

1. **Classify the text.** Procedural (tells the reader to act) or descriptive (explains). The rules differ and you cannot mix the two in one list.
2. **Fix substance before style.** Never let a rewrite drop a fact, a condition, a scope qualifier, or a safety caveat. If the rewrite would lose one, keep the sentence long and say so.
3. **Apply the rules in this order** -- highest yield first:
   - Split every sentence over the cap.
   - One instruction per sentence, unless the actions are simultaneous.
   - Active voice. Passive only for a genuinely unknown agent.
   - Condition first, then a comma, then the command.
   - Replace banned modals: `should`/`may`/`might`/`could`/`would` become `can`, `will`, or `must` (or restructure).
   - Kill `-ing` verb forms and trailing `, making ...` clauses.
   - One term per concept, repeated. No synonym rotation.
   - Cap noun stacks at three words.
   - Restore dropped articles, subjects, verbs. Expand contractions.
   - Replace not-approved words (see below).
4. **Run the linter.** `python3 ~/.claude/scripts/ste_lint.py FILE --mode MODE`. Fix what it reports. A clean run means the mechanical violations are gone, not that the text is compliant -- the linter cannot see passive voice or restricted meanings.
5. **Report what you could not fix** and why. Do not silently leave a violation.

## Word replacement

Three tiers, in order of how much you should trust them:

- **The linter's built-in list** (~90 entries) covers the words that actually recur in software prose. Trust its suggestions.
- **`python3 ~/.claude/scripts/ste_word.py WORD ...`** queries the full 1,198-entry extraction. Use it for a specific word you suspect. `--scan FILE` reports every hit in a draft.
- **`~/.claude/rules/ste100-word-index.md`** is the human-readable table. Read it only when you need to browse; prefer the CLI.

**The domain carve-out matters.** STE rule 1.5 category 19 and rule 1.12 category 2 approve software vocabulary as technical nouns and technical verbs. The aerospace dictionary restricts `code`, `file`, `test`, `function`, `log`, `loop`, `branch`, `thread`, `build`, `run`, `commit`, `deploy`, `cache`, `queue` to other parts of speech. **Never "fix" these.** Both tools suppress them by default. If a tool surfaces one, it is a false positive.

Never invent a replacement the dictionary does not give. If no approved alternative exists and the term is domain vocabulary, keep it and gloss it once.

## Carve-outs: never rewrite or word-count these

Code blocks, inline code, identifiers, file paths, URLs, quoted error strings, log lines, command invocations, and quoted text you do not own. Rule 8.6 already counts quoted text and alphanumeric identifiers as one word; the rest is a necessary extension for software. The linter masks these automatically.

## Second pass for /write-like-austin

When Austin asks for STE as a filter on a teammate-facing draft, `/write-like-austin` runs **first** and owns the result. STE is a subtractive pass that may only remove ambiguity.

Run in `voice` mode and apply **only** these:

- Split sentences over 25 words.
- Active voice where the passive hides who acts.
- Condition before command.
- One term per concept.
- Noun stacks to three words.
- Delete marketing words and Claudese tics.
- Swap not-approved words **only** where the approved word is at least as natural.

**Do not** apply these to Austin's voice, because they destroy it:

- Do not remove hedges. "I think", "probably", "I might be wrong" are load-bearing confidence-marking.
- Do not remove pressure-lowering ("no rush", "feel free to point me elsewhere").
- Do not ban `should`/`could`/`may`. In Austin's register these mark real uncertainty.
- Do not force `do a check of` over `check`, or otherwise trade fluency for one-part-of-speech purity.
- Do not flatten the lead-with-why opening into a bare imperative.
- Do not mechanically repeat a term where natural English would pronominalize.

The voice spec wins every conflict. If a rule would make the draft sound like a manual, drop the rule. Report which STE moves you applied and which you skipped, so Austin can see the tradeoff.

Verify with `ste_lint.py --mode voice`, which turns off the modal and hedge checks for exactly this case.

## Self-check

- Did I keep every fact, condition, and qualifier?
- Did I classify procedural vs descriptive correctly, and not mix them in one list?
- Did I "fix" a word that software usage protects? (Revert it.)
- Did I invent a replacement the dictionary does not give? (Revert it.)
- Did I run the linter and act on it?
- Did I flatten a hedge that was carrying real uncertainty?
- Am I claiming compliance? (Say "STE-shaped" instead. Certification needs the full dictionary.)
