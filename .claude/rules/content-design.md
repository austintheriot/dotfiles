---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Content Design

A reference for evaluating the words inside a product. Used by the `content-design` subagent. The scope is *interface language as a design material*: button labels, field labels, error messages, empty states, confirmations, notifications, tooltips, onboarding text, and the terminology that runs through all of them.

Distinct from `documentation.md` (which owns the doc surface: API reference, READMEs, changelogs, runbooks, doc comments), `information-architecture.md` (which owns *navigation labels and category names* specifically, because those are structural claims), `interaction-design.md` (which owns whether a message exists and what job it must do, where this lens owns whether the words do that job), `i18n.md` (locale, translation mechanics, pluralization, formatting), and `accessibility.md` (text alternatives, link text as a conformance matter).

The core thesis, from Sarah Winters (formerly Richards), who named the discipline: **content design is using data and evidence to give the audience what they need, at the time they need it, in the way they expect.** The emphasis on evidence is the part that distinguishes it from copywriting, and the part usually dropped. Content designers "generally don't move without research."

The second thesis: **the words are not a layer applied to a finished design.** A screen designed with placeholder text and populated afterwards will have the wrong layout, because the real content is longer, more variable, and less tidy than the placeholder. Lorem ipsum is a design decision to defer the hardest constraint.

---

## 1. The disciplinary boundary, because it changes the finding

- **Copywriting** persuades. Its job is conversion, and its success metric is action.
- **UX writing** is the craft of interface text: labels, buttons, errors, tooltips. Microcopy in transactions.
- **Content design** is broader and is positioned as a *design* discipline rather than an editorial one. It covers whether content should exist, what form it should take (a paragraph, a table, a diagram, a form field, nothing), where it goes, and only then the words.

The practical consequence for review: **the strongest content-design finding is often "delete this" or "this should be a table," not "reword this."** A tooltip explaining a badly named button is a content-design failure that adds words; renaming the button is the fix that removes them.

In this user's environment, `~/.claude/rules/simplified-technical-english.md` is the house register and it governs. Where this file and that one conflict on style, that one wins. This file adds the interface-specific surfaces it does not cover.

---

## 2. Voice and tone

**Voice is constant; tone varies by context.** A product has one voice and many tones, and confusing the two produces either an inconsistent product or a tone-deaf one.

Nielsen Norman Group's four tone dimensions are the useful axes, because each is a slider rather than a binary:

1. Formal to casual
2. Serious to funny
3. Respectful to irreverent
4. Matter-of-fact to enthusiastic

The rule that matters most: **tone must move with stakes.** The voice that is charming on an onboarding screen is offensive in an error that just cost someone an hour of work. A product with one tone everywhere has not thought about the error path. Concretely: dial down humour, enthusiasm, and irreverence as consequence rises, to zero at data loss.

Where a project has a stated voice, that stated voice wins over any generic guidance here.

---

## 3. Buttons and action labels

- **Verb plus object.** "Create book," "Delete 412 tokens," "Send invite." A label that names the action can be understood without reading the surrounding sentence, which is how people actually read interfaces.
- **"Submit" is never the right label.** It describes the mechanism, not the outcome.
- **"OK" and "Cancel" are ambiguous on anything but a plain acknowledgement.** On a question, especially a negative one ("Discard unsaved changes?"), "OK" requires the person to reconstruct what they are agreeing to. Label the buttons with the actions: "Discard" and "Keep editing."
- **"Yes" and "No" are worse**, for the same reason plus polarity confusion.
- **The button should complete the heading's sentence.** If the dialog says "Delete this book?" the button says "Delete book," not "Confirm."
- **The destructive button names the destruction.** "Delete permanently" beats "Delete" when the distinction exists in the system.
- **The safe option is the one that reads as the default**, and the button label should make the safe path obvious without relying on colour or position alone.
- **Consistent capitalization.** Sentence case or Title Case, one of them, everywhere. Mixed casing on adjacent controls is the clearest possible signal that nobody owns the copy.

---

## 4. Error messages

`interaction-design.md` section 6 owns whether the message exists and whether it names a next action. This lens owns the words.

**The four parts**, restated because the wording of each is a separate craft problem:

1. **What happened**, in the person's vocabulary, in the first sentence.
2. **Why**, when knowing why changes what they do.
3. **What to do**, as a specific action.
4. **What was preserved**, when there was any risk to their work.

**Rules:**

- **State the requirement, not just the violation.** "Password must be at least 8 characters" beats "Password too short." The requirement is actionable; the violation is a complaint.
- **Never blame the person.** "You entered an invalid date" makes them the defect. "That date is outside the allowed range (1500 to 1800)" states the constraint. Norman's whole argument is that the design invited the error.
- **No cuteness in failure.** "Oops!", "Uh oh!", "Something went wonky" spend the person's patience at the exact moment they have least of it, and they carry no information. This is the single most common tone failure in shipped software.
- **"Something went wrong" is the canonical empty message.** It says only that a message was required. If the system genuinely does not know what failed, say *that* precisely: "The save did not complete and we do not know why. Your work is still here. Try again, and if it fails again, send this identifier to support: `a37e61b`."
- **No error codes as the whole message.** A code is a fine addition for support and never a substitute.
- **No raw exception text, stack traces, or database constraint names.** `duplicate key value violates unique constraint "book_slug_key"` is not a message.
- **Do not apologize repeatedly.** One acknowledgement at most, and only where the system is genuinely at fault.
- **Match the message to the actual scope.** A field-level problem gets a field-level message; a page-level banner for a single invalid field makes the person hunt.

---

## 5. Empty states

Three genuinely different states, and reusing one for another is a defect (see `interaction-design.md` section 4 for the full state table):

- **First use**: nothing exists yet. Say what this space is for and give the one action that fills it. This is the only empty state that should have a call to action.
- **Filtered to nothing**: the data exists, this query found none. Say what was searched or filtered, and offer to widen or clear. **Never tell someone to create the thing they already have.**
- **Permanently and correctly empty**: this is the desired state. Say so plainly and offer nothing. "No open conflicts" is finished; it does not need a button.

The failure common to all three is a bare "No results" with nothing else, which answers neither "why" nor "what now."

---

## 6. Confirmations and consequential copy

- **Name the object and the scale.** "Delete 3,840 tokens?" not "Are you sure?"
- **State what is irreversible, explicitly**, and only where it is true. Over-claiming irreversibility trains people to disbelieve the claim.
- **State what is preserved**, when something is.
- **For the highest-stakes actions, require typing the object's name.** It converts a slip into a deliberate act, and it is the one form of friction habituation does not defeat.
- **Report results with numbers, including what did not happen.** "Swept 3,840, skipped 12 already verified, skipped 2 illegible" is a good report; "Done" is not. Silence about skipped items is where trust in bulk operations dies.

---

## 7. Labels, fields, and helper text

- **Labels are nouns; buttons are verbs.** Mixing them produces "Submission" as a button and "Enter your name" as a label.
- **A placeholder is not a label.** It vanishes on focus, usually fails contrast, and defeats autofill. `interaction-design.md` owns the interaction consequence; the content consequence is that the person ends up with a filled field and no idea what it holds.
- **Helper text goes below the field and stays visible.** Help that appears only on focus is help that arrives after the decision.
- **Say what you need and why, when the why is not obvious.** A field asking for a slug should say what a slug does here.
- **Mark the smaller set.** If most fields are required, mark the optional ones, and vice versa. Marking everything marks nothing.
- **Tooltips are a last resort**, and a tooltip that is the only source of essential information fails on touch, on keyboard, and for anyone who did not hover.

---

## 8. Terminology governance

This is where content design and information architecture meet, and it is the highest-leverage content work in a product with a domain model.

- **One term per concept, everywhere.** Interface, documentation, error messages, support replies, and the codebase's user-facing strings. Synonym rotation is a defect, not variety. This is the same rule as `simplified-technical-english.md`, and the same rule `information-architecture.md` states for labels.
- **Keep a glossary, and make it the source of truth.** A domain product with no written glossary will invent terms per screen.
- **Expand a domain acronym on first use** in any given surface, then use the short form. Universally known acronyms are exempt.
- **Do not invent a term where the domain has one.** In specialist software the domain's own vocabulary is the users' vocabulary, and simplifying it is condescension that costs precision. This is the specific point where plain-language guidance and specialist software conflict, and the domain usually wins.
- **Do not leak the schema.** `bulk_verified`, `corpus_reference_conflict`, `machine_selected` are internal names. `information-architecture.md` owns this for navigation labels; this lens owns it for every other string.
- **Distinguish the near-synonyms the system actually distinguishes.** If "remove" and "delete" do different things, they must be used consistently and the difference must be legible. If they do the same thing, use one word.

---

## 9. Plain language, and its limit

The standard guidance holds: short sentences, active voice, common words, front-load the important thing, one idea per sentence, second person for instructions.

**The limit, stated because it is routinely got wrong in specialist products:** plain language means *not needlessly complex*, not *dumbed down*. A tool for palaeographers should say "collation" if collation is the concept, and should not say "comparing the different versions" to score a readability point. Substituting a vague common word for a precise technical one loses information and signals to the expert that the product does not know the domain.

The reviewable version: **is the complexity in this sentence load-bearing?** If removing a word loses meaning, keep it. If it only loses formality, cut it.

---

## 10. Consequences for translation and layout

Even in a monolingual product, these are content-design constraints:

- **Text expands.** German and Finnish commonly run substantially longer than English; short strings expand proportionally more than long ones. A button sized to its English label will break. Design to the longest plausible string, not the current one.
- **Never build a sentence by concatenation.** `"Deleted " + count + " items"` cannot be translated correctly, cannot be pluralized correctly, and cannot be reordered. Use a complete parameterized message.
- **Do not embed markup or line breaks in translatable strings** as a layout mechanism.
- Pluralization, dates, numbers, currency, and locale rules are `i18n`'s. Say `See also: i18n` and do not duplicate.

---

## 11. Schools of thought that genuinely disagree

### Delight versus invisibility

- **The delight school**: microcopy is a brand surface and a chance to be human; a well-judged joke in an empty state builds affection.
- **The invisibility school**: interface text is functional, the person is trying to do something else, and personality is friction charged to them. The strongest version holds that noticing the copy at all is a failure.
- The reconciling variable is **frequency and stakes**. A joke read once is charming; the same joke read four hundred times is an obstacle. Both schools agree it drops to zero at failure and at data loss.

### Conversational versus terse

Conversational interfaces read as friendly and cost words; terse ones read as respectful of time and can read as brusque. The choice is legitimately a product decision. What is not legitimate is inconsistency: a product that is chatty in onboarding and telegraphic in errors has no voice, it has two authors.

### Brand voice versus clarity

Where a strong brand voice conflicts with the clearest possible statement, clarity wins in transactional and failure surfaces, and the brand can have the marketing ones. This is not universally agreed; some organizations defend voice everywhere. The reviewable position: name the conflict, and note that whoever is choosing voice over clarity in an error message is spending the user's time on the company's personality.

### Content-first versus design-first

Content-first (write the real content, then design around it) is well supported by practice and produces layouts that survive real data. Design-first with lorem ipsum is faster and reliably produces components that break on contact with reality. The middle position that most teams actually run: real content for the hard cases (longest label, worst error, emptiest state), placeholders for the rest.

---

## 12. Anti-pattern catalog

- **"Something went wrong."**
- **"Oops!" / "Uh oh!" / "Yikes!"** in a failure.
- **Raw exception, stack trace, or database constraint name** shown to a person.
- **Error code as the whole message.**
- **Blame framing**: "You entered an invalid...".
- **"Submit."**
- **"OK" / "Cancel" on a consequential question.**
- **"Yes" / "No" on a negatively phrased question.**
- **"Are you sure?"** with no object and no scale.
- **Placeholder as label.**
- **Tooltip carrying essential information.**
- **Schema vocabulary in a user-facing string.**
- **Synonym rotation** for one concept.
- **Mixed sentence case and Title Case** on adjacent controls.
- **"Click here"** as link text, which is also an accessibility finding.
- **A bare "No results"** with no explanation and no next step.
- **First-use empty-state copy on a filtered-empty state.**
- **Concatenated sentence** assembled from fragments.
- **Silent success.** An operation that reports nothing, so the person cannot tell it happened.
- **Over-apologizing**, which reads as evasion.

---

## 13. What to flag, and what not to

**Flag:**
- An error message missing what happened, what to do, or what was preserved, quoting the current text.
- Blame framing, cute framing in failure, or a raw technical string surfaced to a person.
- A button label that names a mechanism rather than an outcome, or that cannot be understood without its heading.
- Ambiguous confirmation labels on a consequential action.
- The wrong empty state for the condition.
- A schema term in a user-facing string, with the domain word proposed.
- Terminology inconsistency, listing every variant found.
- A concatenated user-facing sentence.
- A string whose length will break its container at plausible content or plausible translation.
- Copy that exists to explain a badly named control, where renaming removes the copy.

**Do not flag:**
- Navigation and category labels. `information-architecture`.
- README, changelog, API reference, doc comments. `documentation`.
- Whether the message should exist at all, or whether the state is missing. `interaction-design`.
- Pluralization, date and number formatting, locale mechanics. `i18n`.
- Text alternatives and link-text conformance. `accessibility`.
- Marketing copy on a marketing surface unless it makes a factual claim the product does not support.
- Style preferences already settled by the project's own rules. Project conventions win; read them first.
- Rewriting for its own sake. If the current text does its job, leave it.

Every finding quotes the current string and proposes a replacement: not "improve the error message" but "`ImportPanel.tsx:88` shows `Error: ECONNREFUSED`; propose `The import service is not responding. Your file was not uploaded. Try again in a moment.`"
