---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Simplified Technical English (ASD-STE100)

Compressed from *ASD-STE100 Simplified Technical English, Issue 9 (January 2025)*, the controlled-language standard published by the Aerospace, Security and Defence Industries Association of Europe (ASD). The full standard is 53 numbered writing rules in nine sections, plus 8 general recommendations (GR-1 thru GR-8), plus a controlled dictionary of 875 approved words and 1,274 not-approved words with approved alternatives. Local copy: `~/Downloads/ASD-STE100 Simplified Technical English - Issue 9 (2025).pdf`.

Not auto-loaded by path. Pulled in deliberately when Simplified Technical English (STE) is the target register.

## What STE is, and what it is not

STE is a *controlled natural language*: a closed vocabulary plus a closed grammar, designed so that a non-native English reader, under time pressure, in a safety-critical situation, cannot misread the text. It was built for aircraft maintenance manuals. Its design goal is the elimination of ambiguity, not elegance, brevity, or warmth.

The two levers are independent:

1. **Approved words** (the dictionary): a small vocabulary, each word restricted to one part of speech and usually one meaning.
2. **Approved constructions** (the 53 rules): short sentences, active voice, simple tenses, no phrasal verbs, no `-ing` forms as verbs.

Full STE compliance requires the dictionary, which is not reproduced here. What follows is the rule set (complete) plus the highest-yield vocabulary substitutions. Writing that obeys the rules and prefers the listed substitutions is *STE-shaped* even when it is not certifiably STE-compliant. Say "STE-shaped" or "STE-influenced" rather than claiming compliance.

**Where STE costs something.** It flattens register and social nuance. Prose whose job is partly social (softening an ask, signaling deference, persuading) reads blunt in strict STE. Austin has chosen STE as the default anyway, for everything including chat replies; teammate-facing drafts are the single carve-out, where `write-like-austin.md` wins.

**Uncertainty is not a casualty of this.** STE bans the modal hedges (`should`, `may`, `might`, `could`, `would`) and vague qualifiers (`probably`, `roughly`, `somewhat`). It does not ban *stating* uncertainty, because a plain declarative admission is perfectly good STE. "I am not sure." "I did not verify this." "I did not read that file." "This is a guess." "I was wrong." All pass. So the confidence signal must be converted, never deleted:

| Not this | This |
|---|---|
| "This is probably the cause." | "This is the likely cause. I did not confirm it." |
| "You might want to check X." | "Check X." |
| "I think this could break." | "This breaks if the token expires first. I did not test that path." |
| "This should work." | "I expect this to work. I did not run it." |
| "It's roughly 90 entries." | "It is 87 entries." |

Deleting a hedge to satisfy the register, and thereby implying more certainty than the facts support, is a worse defect than the hedge was. STE exists to remove ambiguity about the *world*, not to hide the writer's ignorance of it.

## Section 1 -- Words (rules 1.1 thru 1.14)

| Rule | Statement |
|---|---|
| 1.1 | Use words that are approved in the dictionary, technical nouns, or technical verbs. |
| 1.2 | Use approved words only as the specified part of speech. |
| 1.3 | Use approved words only with their approved meanings. |
| 1.4 | Use only the approved forms of verbs and adjectives. |
| 1.5 | You can use words that fit a technical noun category. |
| 1.6 | Use a word that is not approved only when it is a technical noun or part of one. |
| 1.7 | Do not use technical nouns as verbs. |
| 1.8 | Use technical nouns that are approved in your company, industry, or subject field. |
| 1.9 | When you must select a technical noun, use one that is short and easy to understand. |
| 1.10 | Do not use regional, slang, or jargon words as technical nouns. |
| 1.11 | Do not use different technical nouns for the same item. |
| 1.12 | You can use verbs that fit a technical verb category. |
| 1.13 | Do not use technical verbs as nouns. |
| 1.14 | Use American English spelling unless other official directives tell you differently. |

**One word, one part of speech, one meaning.** This is the rule that does the most work and is the least intuitive. A word approved as a noun may not be used as a verb, even though English permits it:

- `work` is a noun, not a verb: not "When you work with cleaning agents" but "When you **do work** with cleaning agents."
- `damage` is a noun, not a verb: not "Be careful not to damage the sleeve" but "Be careful not to **cause damage to** the sleeve."
- `help` is a verb, not a noun: not "with the help of a second person" but "with the **aid** of a second person."
- `check` is a noun, not a verb: not "Check the laptop battery" but "**Do a check of** the laptop battery."

**Restricted meanings** bite even on approved words. `wear` means "to become damaged by friction," so "Wear protective clothing" is wrong -- write "**Put on** protective clothing." `see` is only for what the eyes do, never "come to know," so "Move the tube to see if the connection is tight" becomes "...to **make sure that** the connection is tight." `turn` only means rotation about an axis, so "The indicator turns green" becomes "The **color of the indicator changes to** green." `above` and `below` are physical positions only, never limits: "Do not let the pressure go below 20 psi" becomes "Do not let the pressure **become less than** 20 psi."

**Technical nouns** are the escape hatch. Any term that fits one of the 22 categories is usable even when the dictionary rejects the word. The categories, condensed: official parts information; assemblies and systems; tools and support equipment; materials and consumables; facilities and infrastructure; circuits and electrical; mathematical/scientific/engineering terms and formulas; navigation and geographic; numbers, units, and time; quoted text; roles/individuals/organizations/geopolitical entities; parts of the body; personal effects, food, and beverages; medical terms; official documents and standards; environmental and operational conditions; colors; damage terms; computer science and information technology; civil and military operations; law and regulations; animals, plants, and other life forms.

Category 19 (computer science and information and communication technology) is the one that matters most for software work, and it is generous: `add-in`, `AI`, `authentication`, `backup`, `backup file`, `bookmark`, `chatbot`, `content`, `cursor`, `cybersecurity`, `database`, `deep learning`, `digitization`, `e-mail`, `embedding`, `field`, `file`, `firewall`, `hallucination`, `HTML`, `icon`, `interface`, `internet`, `laptop`, `large language model`, `machine learning`, `memory`, `menu`, `metadata`, `network`, `operating system`, `plug-in`, `prompt engineering`, `screen`, `search engine`, `status bar`, `store`, `token`, `toolbar`, `touchscreen`, `tuning`, `update`, `XML`.

The same word can be a technical noun in one context and forbidden in another. `backup` is fine as "Do the backup of the computer" (category 19) but not as "one person available as backup" (no category fits; rewrite the sentence).

**Technical verbs** get four categories: manufacturing processes (remove/add/attach material, change strength or surface or shape); computer processes and applications (input and output -- `click`, `enter`, `press`, `swipe`, `tap`, `type`; user interface -- `close`, `copy`, `delete`, `drag`, `enable`, `encrypt`, `filter`, `open`, `paste`, `save`, `scroll`, `sort`, `validate`, `zoom in`; system operations -- `abort`, `boot`, `debug`, `download`, `format`, `install`, `load`, `process`, `reboot`, `update`, `upgrade`, `upload`); subject-field instructions (engineering, medical, military, navigation, automotive, energy); law and regulations (`acknowledge`, `comply with`, `conform to`, `enforce`, `notify`, `supersede`, `waive`).

Prefer the approved dictionary verb over a technical verb when it says the same thing. "If you **detect** broken wires" is wrong because `find` exists; but "The security scanner **detects** metal objects" is correct, because that is the technical sense.

## Section 2 -- Multi-word nouns (rules 2.1 thru 2.2)

| Rule | Statement |
|---|---|
| 2.1 | Write multi-word nouns of no more than three words. |
| 2.2 | When a technical noun has more than three words, write it in full, then give a shorter form or use hyphens between words used as one unit. |

The head noun is last in English and first in many other languages, so a long modifier stack forces the reader to hold four modifiers before learning what the thing is. `Runway light connection` (3 words) is fine. `Runway light connection resistance calibration` (5 words) does not tell the reader how `runway` relates to `calibration`.

## Section 3 -- Verbs (rules 3.1 thru 3.7)

| Rule | Statement |
|---|---|
| 3.1 | Use only the verb forms given in the dictionary. |
| 3.2 | Use only the infinitive, the imperative, the simple present, the simple past, the simple future, and the past participle (as an adjective). |
| 3.3 | Use the past participle form as an adjective. |
| 3.4 | Do not use auxiliary verbs to make complex verb constructions. |
| 3.5 | Use the `-ing` form of a verb only as a technical noun or as a modifier in a technical noun. |
| 3.6 | Use the active voice. In descriptive writing, use the passive voice only when the agent is unknown. |
| 3.7 | Use an approved verb to describe an action, not a noun or other part of speech. |

**Forbidden tenses:** present perfect (`has adjusted`), past perfect (`had adjusted`), present and past progressive (`is adjusting`, `was adjusting`), and all other complex constructions. "The operator has adjusted the linkage" becomes "The operator **adjusted** the linkage."

**No `-ing` as a verb or participial modifier.** "When you are doing this procedure" becomes "When you **do** this procedure." The `-ing` form survives only as a technical noun (`Cleaning`, `Troubleshooting`, `Packaging`, `Handling`) or as a modifier inside a technical noun (`air-conditioning system`, `grinding wheel`, `switching relay`). Approved `-ing` words are few: nouns `lighting`, `opening`, `routing`, `servicing`; adjectives `mating`, `missing`, `remaining`; the pronoun `something`; the preposition `during`.

**Past participle as adjective is not passive voice.** "Examine all parts of the **disassembled** unit" and "When the unit **is** fully **disassembled**" are both fine -- the participle describes a condition, before a noun or after `be`/`become`/`stay`.

**Four ways out of the passive.** (1) Promote the agent from the `by` phrase to subject: "The circuits are connected by a switching relay" becomes "A switching relay connects the circuits." (2) Replace a hollow infinitive construction with the real verb: "These values are used by the computer to calculate X" becomes "The computer calculates X from these values." (3) In procedures, use the imperative: "The test can be continued by the operator" becomes "Continue the test." (4) When no agent is stated, supply `you` (the reader) or `we` (the organization): "the valve can be opened with the override handle" becomes "**you can open** the valve with the override handle."

The test for passive voice: ask "by whom or by what?" If the text answers, it is passive. The legitimate exception is a genuinely unknown agent in descriptive text: "During transmission, the data was corrupted" is correct, because naming `transmission` as the agent would be technically false.

## Section 4 -- Sentences (rules 4.1 thru 4.5)

| Rule | Statement |
|---|---|
| 4.1 | Write short and clear sentences. |
| 4.2 | Do not omit words or use contractions to make sentences shorter. |
| 4.3 | Use a vertical list for complex text. |
| 4.4 | Use connecting words and phrases to connect sentences that contain related topics. |
| 4.5 | When applicable, use an article (the, a, an) or a demonstrative adjective (this, these) before a noun or multi-word noun. |

**Never omit to shorten.** No contractions (`don't`, `isn't`). No dropped subjects: "If installed, remove the shims" becomes "If **shims are** installed, remove them." No dropped verbs: "Rotary switch to INPUT" becomes "**Set** the rotary switch to INPUT." No dropped articles, which change meaning: "Remove the bolt and stop" (is `stop` a second object, or a command?) becomes "Remove the bolt and **the** stop."

**Vertical lists** carry real mechanics: colon before the list; each item marked; each item starts uppercase; article before the subject noun where applicable; period only if the item is a full sentence, and always on the last item; never a comma or semicolon at line end; every item must read correctly against the stem before the colon; do not nest a second list inside an item; do not mix procedural and descriptive items in one list. In safety lists, repeat the negative on each item -- "DO NOT PUT YOUR FEET ON THE APU LINE" / "DO NOT USE THE APU LINE AS A HANDLE", not a shared "DO NOT:" stem.

**Articles are not automatic.** No article on general statements ("Solvents can cause damage to paint"), abstract qualities ("This software increases performance"), or before a proper noun formed by an alphanumeric identifier ("Tag circuit breaker 36L7", not "the circuit breaker 36L7"). Article placement in a series carries meaning: "Install the new O-rings (15), spacers (14), nut (13), and safety pin (12)" says everything is new; repeating the article ("the new O-rings (15), **the** spacers (14), ...") says only the O-rings are new.

## Section 5 -- Procedural writing (rules 5.1 thru 5.5)

| Rule | Statement |
|---|---|
| 5.1 | Write short sentences. Use a maximum of **20 words** in each sentence. |
| 5.2 | Write only one instruction in each sentence unless two or more actions occur at the same time. |
| 5.3 | Write instructions in the imperative (command) form. |
| 5.4 | When there is a condition the reader must know first, start with a descriptive statement, then divide it from the command with a comma. |
| 5.5 | Write notes only to give information, not instructions. |

Safety instructions obey the 20-word limit too. Notes are the exception: a note is descriptive text, so its limit is 25 words.

**Condition first, always.** "Set the switch to NORMAL when the light comes on" becomes "**When the light comes on,** set the switch to NORMAL." The comma placement is load-bearing: "If the CSD does not operate correctly, disconnect it" and "If the CSD does not operate, correctly disconnect it" are both grammatical and mean different things.

**Do not put `must` before an imperative** unless the instruction is safety-critical. "Before you remove the clamp, you must disconnect the hose" becomes "Before you remove the clamp, disconnect the hose."

**More than one instruction per sentence** is allowed only for simultaneous actions: "Cut and remove the wire." "Hold the panel in its open position and install the fastener."

**The note test.** Read the procedure with all notes deleted. If the reader can still complete it correctly, the notes are used correctly. If not, the content in the note belongs in a work step. Notes must never carry instructions, limits, tolerances, or results -- limits go directly after the action they constrain. Content that prevents damage or injury is not a note; it is a caution or a warning.

## Section 6 -- Descriptive writing (rules 6.1 thru 6.6)

| Rule | Statement |
|---|---|
| 6.1 | Give information gradually. |
| 6.2 | Use key words and key phrases to give your text a logical structure. |
| 6.3 | Write short sentences. Use a maximum of **25 words** in each sentence. |
| 6.4 | Use paragraphs to show related information. |
| 6.5 | Make sure that each paragraph has only one topic. |
| 6.6 | Make sure that no paragraph has more than six sentences. |

No imperative form in descriptive text. One topic per sentence, one topic per paragraph, topic sentence first. Cohesion comes from deliberately repeating key words across sentences rather than varying them -- the opposite of the "avoid repetition" instinct. If the topic sentences of a passage, read alone, form a usable outline, the structure is right.

## Section 7 -- Safety instructions (rules 7.1 thru 7.3)

| Rule | Statement |
|---|---|
| 7.1 | Use an applicable word (for example, "warning" or "caution") to identify the level of risk. |
| 7.2 | Start a safety instruction with a clear and accurate command or condition. |
| 7.3 | Give an explanation to show the risk or possible result. |

A **warning** means risk of injury or death. A **caution** means risk of damage to objects. Both levels present at once means use a warning. Other vocabularies (`danger`, `attention`, `notice`) and symbols are permitted as long as rules 7.1 thru 7.3 hold; refer to ISO 45001, the ANSI Z535 series, and ISO 3864.

The shape is always **command or condition first, then the specific consequence**. Not "EXTREME CLEANLINESS OF OXYGEN TUBES IS IMPERATIVE" (abstract, and wrongly a caution) but "MAKE SURE THAT THE OXYGEN TUBES ARE FULLY CLEAN. OXYGEN AND GREASE MAKE AN EXPLOSIVE MIXTURE. AN EXPLOSION CAN CAUSE INJURY OR DEATH." Naming the actual outcome (`explosion`, `injury`, `death`) is the point. STE itself specifies no formatting; uppercase in the standard's examples is convention, not rule.

## Section 8 -- Punctuation and word count (rules 8.1 thru 8.7)

| Rule | Statement |
|---|---|
| 8.1 | You can use all standard English punctuation marks but not the semicolon (;). |
| 8.2 | Use hyphens (-) to connect words that are directly related. |
| 8.3 | You can use parentheses for the seven listed purposes. |
| 8.4 | In a vertical list, a colon (:) has the same effect on word count as a period. |
| 8.5 | Text in parentheses counts as one word in that sentence. |
| 8.6 | Count numbers, numbers with units, abbreviations, alphanumeric identifiers, quoted text, titles and headings and placard/label text, and proper nouns of individuals/groups/organizations/geopolitical entities as one word each. |
| 8.7 | Hyphenated words count as one word. |

The **semicolon is banned** because it enables long sentences and is easy to misuse. Write two sentences.

**Hyphenate:** multi-word adjectives before a noun (`low-altitude flight`, `high-pressure chamber`, `fire-resistant material`); two-word numbers and fractions (`forty-seven`, `three-sixteenths`); letter-or-number plus noun describing shape (`L-shaped bracket`, `O-ring`, `3-prong connector`); verbs whose first element is another part of speech (`heat-treat`, `short-circuit`, `dry-clean`); prefix ending in a vowel before a root starting with a vowel (`pre-amplifier`, `de-icing`, `anti-icing`).

**Parentheses** are permitted for: references to illustrations or text; item identifiers; work-step identifiers; abbreviations; simultaneous singular and plural (`the test(s)`); explanation of a word or clause; an alternative.

**Word counting** is mechanical and generous. `10 mA` is one word. `NASA` is one word. `No. 1` is one word. A quoted string, a formula, a document title, and a placard text are each one word. A parenthetical is one word in its host sentence but also counts as its own sentence. Numbers that label paragraphs or work steps do not count at all. In a vertical list the colon acts as a period, so the 20/25-word budget resets for each item.

## Section 9 -- Writing practices (rules 9.1 thru 9.4, plus GR-1 thru GR-8)

| Rule | Statement |
|---|---|
| 9.1 | Use a different sentence construction when a word-for-word replacement is not sufficient. |
| 9.2 | Use each approved word correctly (right meaning, right part of speech). |
| 9.3 | When you use two words together, do not make phrasal verbs. |
| 9.4 | When you select terminology or wording, always use a consistent style. |

**No phrasal verbs.** Two approved words juxtaposed can form a third meaning belonging to neither, which is exactly the ambiguity STE exists to remove. "After you **put out** the fire" becomes "After you **extinguish** the fire." "This compound can **give off** poisonous fumes" becomes "can **release** poisonous fumes." Phrasal verbs are generally *not* listed as not-approved in the dictionary, so this rule is enforced by discipline rather than lookup. A few (`put on`, `come on`) are approved with restricted meanings.

**Consistency over variety.** Same action, same wording, every time. Two sentences can both be valid STE and still be a defect if they express the same step differently in the same procedure. Pick one and repeat it.

The eight **general recommendations** are guidance, not rules:

- **GR-1, the conjunction `that`.** Keep it. "Make sure **that** the valve is open." It marks the clause boundary and most languages cannot drop the equivalent word.
- **GR-2, the preposition `with`.** Three approved meanings (association, help/sharing, means/instrument) and therefore frequent ambiguity. "Install the panel with the green fasteners" has three readings. Reread every `with`.
- **GR-3, pronouns.** Only dictionary-approved pronouns; `he` and `she` are not approved. If a pronoun could bind to more than one antecedent, replace it with the noun.
- **GR-4, the pronoun `this`.** Restate the referent when `this` could point at more than one thing. "Make sure that the cover is not locked (this can cause damage)" is unresolvable -- which state causes the damage?
- **GR-5, false friends.** Non-native writers: verify the English meaning, not the look-alike in your own language.
- **GR-6, Latin abbreviations.** Do not use `e.g.`, `i.e.`, `etc.` Write "for example," "that is," "and so on," or delete.
- **GR-7, inclusive language.** STE is gender-neutral by construction. `he`, `she`, `man`, `woman` are not permitted except where genuinely required (medical context).
- **GR-8, possessive form.** Permitted but risky for non-native readers; if unsure the sentence is correct, avoid it.

## High-yield substitutions

Not the dictionary -- the subset that actually fires in software and engineering prose. Left column is not approved; right column is the approved alternative.

| Not approved | Use |
|---|---|
| acceptable | permitted |
| accuracy | precision |
| achieve, acquire, obtain | get |
| additional | more |
| adequate, sufficient | sufficient |
| advise, inform, notify, request | tell |
| allow, enable, permit | let |
| alter, modify | change |
| appropriate, suitable | applicable |
| ascertain, assure, check, ensure, establish, verify | make sure |
| assist, facilitate | help |
| attempt | try |
| avoid, prohibit | prevent |
| aware | know |
| begin, commence, initiate | start |
| cease, discontinue, terminate | stop |
| comply | obey |
| comprise | have |
| conduct, effect, implement, perform, undertake | do |
| considerable | large |
| determine, locate | find |
| difficult | not easy |
| diminish, reduce | decrease |
| eliminate | remove |
| employ, utilize | use |
| entire, complete | full |
| evaluate | examine |
| exceed | more than |
| excessive | too much |
| feasible | possible |
| following | these |
| however | but |
| identical | same |
| incorporate | include |
| indicate, represent, reveal, appear | show |
| insufficient, lack | not sufficient |
| keep, maintain, retain | keep |
| major, principal | primary |
| observe | monitor |
| option | alternative |
| particular | only applicable |
| portion | piece |
| proceed | continue |
| produce | cause |
| provide | give |
| rapid | fast |
| require | necessary |
| respective | related |
| several | some |
| significant, fundamental | important |
| similar | equivalent |
| therefore | thus |
| various | different |
| via | through |
| whether | if |
| within | in |

Constructions rather than word swaps: `depend` becomes an `if` clause; `analyze` becomes `do an analysis of`; `check` becomes `do a check of`; `investigate` becomes `do an investigation of`; `review` becomes `do an inspection of`; `probable` becomes `very possible`; `eventually` becomes `some time`.

## Applying this to non-aerospace prose

The rules divide cleanly by how well they travel.

**Transfers to almost any technical writing** -- documentation, runbooks, error messages, API reference, alert text, migration guides, incident timelines:

- One instruction per sentence; imperative for instructions.
- Condition before command, separated by a comma.
- Active voice; passive only for a genuinely unknown agent.
- Sentence caps (20 procedural, 25 descriptive) as a discipline, not a linter.
- One topic per paragraph; six sentences maximum; topic sentence first.
- Same thing, same name, every time -- no synonym variety for its own sake.
- No dropped articles, subjects, or verbs; no contractions.
- No ambiguous `this` or `it`; restate the referent.
- No `e.g.` / `i.e.` / `etc.`
- Vertical lists with the mechanics from rule 4.3.
- Multi-word noun stacks capped at three words. This one is chronically violated in software: `user session token refresh handler` is five.
- Consequence named concretely in warnings, not gestured at.

**Does not transfer** -- do not force these outside a controlled-language program:

- The closed 875-word dictionary. Software has its own irreducible vocabulary, and `deploy`, `serialize`, `idempotent`, `merge`, `rollback` have no approved equivalents.
- The one-part-of-speech restriction. "Do a check of the config" over "check the config" costs clarity for a fluent audience and gains nothing.
- Banning all `-ing` forms. English gerunds are load-bearing in software prose (`logging`, `caching`, `rate limiting`, `polling`).
Two items that a generic STE treatment would put here, but Austin has explicitly adopted: the **semicolon ban** and the **word caps** apply to his prose too, including argumentative and explanatory writing. Treat the caps as a discipline that forces a split, not as a hard linter gate.

`~/.claude/CLAUDE.md` makes STE the **default register for everything**, chat replies included. Enforce the banned modals and vague qualifiers there, and convert uncertainty rather than deleting it (see "Uncertainty is not a casualty of this" above).

`~/.claude/rules/write-like-austin.md` **outranks** this file for teammate-facing prose only -- PR descriptions, issue and review comments, Slack, Linear, design-doc comments. There, the hedges and pressure-lowering phrases are doing social work that STE cannot see, so keep them. STE contributes only sentence-level discipline (short, active, unambiguous, one idea, one term per concept) and the voice spec governs tone, structure, and register. `ste_lint.py --mode voice` implements that split. The em-dash and emoji prohibitions bind regardless of mode.

## Reference

Aerospace, Security and Defence Industries Association of Europe. (2025). *ASD-STE100 Simplified Technical English: Standard for technical documentation* (Issue 9). https://asd-ste100.org

Free to download from asd-ste100.org. Issue 9 is dated 2025-01-15 and fully replaces all earlier issues. STE is an EU registered trademark (No. 017966390) owned by ASD. The standard is explicitly advisory: it creates no legal obligations.

Supporting references the standard itself points to: Swan, M. (2017). *Practical English Usage* (4th ed.) for articles and grammar. *The Chicago Manual of Style* (18th ed., 2024) for general punctuation. Kirkman, J. (2006). *Punctuation Matters*. ISO 45001:2018, the ANSI Z535 series, and ISO 3864 for safety-instruction vocabulary and symbols.
