---
name: expert-review-pr
description: Use when reviewing someone else's pull request and posting the result to GitHub as an outside reviewer. Runs the expert panel over the PR diff plus a second independent pass over the tests (untested paths, uncovered edge cases, surviving mutants) and, when the change adds files, modules, or public names, a pass checking that they match the project's existing structure and naming, holds both to the same bar of a named defect with a concrete trigger and a real consequence, summarizes locally, and on approval posts a COMMENT-only review with inline comments. Not for reviewing your own work -- use /expert-review for that. When asked, also dev-QAs the PR on its deployed build and folds a Dev QA section, with screenshots of every checked state, into the same top-level review body.
---

# Expert Review PR

Review another person's pull request and post the result to GitHub.

Analysis comes from the `/expert-review` panel. Everything after analysis is different: a far harsher filter, a posted artifact, and a voice that is openly Claude's rather than the user's.

**Read `~/.claude/rules/outside-pr-review.md` first.** It holds the posture this skill shares with `/dev-qa`: the two overriding rules (post only on an answered approval, `COMMENT` only), scope resolution, the posting identity, voice, the internals rule, the local summary, uploads, and the GitHub posting mechanics. This file adds what is specific to a code review: the analysis, the filter, the body shape, and the inline comment shapes.

For the self-authorship line, name `/expert-review`:

> This pull request is authored by you. `/expert-review` is the skill built for your own work. Continuing.

## Analysis

Follow `/expert-review`'s process for Stage 1 through Stage 6: fetch the change set with the `pr-diff` agent, discover project conventions, classify regions, dispatch specialists in parallel, synthesize with the skepticism discipline.

Read `~/.claude/skills/expert-review/SKILL.md` for those stages rather than reimplementing them. Two amendments to the dispatch prompt:

- Mode is always `diff`.
- Add to each dispatch prompt: *"Findings will be posted publicly to the author of this change. Only findings that name a concrete trigger and a concrete consequence will survive synthesis. Report your full range as the panel contract defines it; the synthesis stage filters."*

Do not tell the subagents to self-filter for severity. They obey that literally and drop real bugs. The filter below is applied once, at synthesis, where the whole set is visible.

## Model selection

Every agent call uses the cheapest model that can do that call's job, chosen per call. The choice is between Haiku and Sonnet. Do not go above Sonnet unless the user names a model in the invocation or in the conversation. Pass `model` explicitly on every Agent call. Do not rely on the agent definition's frontmatter or on inheritance from the main session, because both resolve to the session model.

**Start from Haiku. Move a call to Sonnet when any of these is true:**

- **The lens finds defects by reasoning about behavior.** `bug-hunter`, `security`, `concurrency`, `distsys-data`, `distsys-runtime`, `data-flow`, `rust-unsafe`, `rust-async`, `typescript-types`, `fp-types`, `sync-and-offline`, `platform-payments`, and any lens whose region touches money, auth, persistence, or concurrency. A missed defect here costs the most, and the main session cannot recover a finding that the agent never reported.
- **The call runs experiments and explains them.** The test pass runs the suite and mutation testing, then must say what each surviving mutant lets through.
- **The region is large or spread out.** More than roughly 300 changed lines for that agent, or changes across more than three top-level directories or modules.
- **The call must weigh conventions against each other.** For the structure and naming pass: the change adds a new top-level directory, module, package, crate, or namespace, renames or moves across directories, or changes a name that crosses a serialization, storage, or protocol boundary.

**Keep these on Haiku:**

- `pr-diff`, and any other call that fetches, lists, or counts.
- Checklist-shaped lenses on a small region: `documentation`, `readability`, `accessibility`, `i18n`, `ci-pipeline`, `licensing-and-oss`, and similar, when the region is under the size threshold above.
- The structure and naming pass when the change only adds files beside existing siblings of the same kind, or adds local names inside existing modules. The job then is counting and comparing.

When unsure between the two, pick Sonnet for a lens in the first list and Haiku otherwise.

**Rerun once on Sonnet** when a Haiku call returns output without file and line references, contradicts the code on a fresh read, or obviously did not read the region (for example, it describes functions the diff does not contain). Do not rerun to get more findings from a call that returned a clean result with evidence.

Synthesis, the filter, verification of survivors against the code, and posting stay in the main session. Those steps decide what goes public, so they do not move to a smaller model.

A user instruction outranks this rule. "Use Opus for the panel" or `--model opus` applies to every specialist call for that run. A model named for one agent applies to that agent only.

In the local summary, add one line listing each agent with the model it ran on, and any rerun. That line is how the user tunes this rule. It is never posted.

## The structure and naming pass

Dispatch `project-structure` and `naming-conventions` only when the change gives them something to judge. Decide each one separately from the change set, before the Stage 5 dispatch. When one fires, run it in parallel with the Stage 5 dispatch, as its own agent call separate from the panel. This decision replaces Stage 3 classification for these two lenses.

**`project-structure` fires when the change does any of these:**

- Adds, moves, renames, or deletes a file or directory. `git diff --name-status <base>...<head>` shows `A`, `R`, or `D`.
- Adds a module, package, crate, namespace, workspace member, Gradle subproject, Swift target, or feature folder.
- Adds an import that crosses a top-level directory, feature, or package boundary that did not cross before.
- Adds or edits a barrel or `index` re-export, a path alias, project references, or boundary lint config.
- Places a test, fixture, mock, story, or generated file.

**`naming-conventions` fires when the change does any of these:**

- Adds an exported or public name: a function, type, class, module, namespace, constant, or component.
- Adds or renames a field in a serialized type, schema, migration, `.proto`, GraphQL schema, or OpenAPI document.
- Adds or renames an environment variable, CLI flag, config key, HTTP header, route, metric, or event name.
- Renames anything, including a case-only rename.
- Adds a file whose name a tool reads (test files, stories, Go platform suffixes, framework-routed files).

**Neither fires** when the change only edits bodies of existing functions and adds names visible only inside one function, or is documentation-only, lockfile-only, or generated output. Say in the local summary which of the two ran, and why, in one line.

The pass asks one question: **does this change fit the project as it already is?** The reference is the repository's existing structure and naming, not the agents' canon. A project that organizes by type, uses `userID`, and puts tests in a mirrored `test/` tree sets the standard for this pull request. A change that matches the project and departs from an ecosystem guide is correct for this review.

Give each agent this scope, in addition to the standard dispatch template:

> Before you judge anything in the diff, establish the project's existing conventions from the code on the base branch. Read any documented convention (`CLAUDE.md`, `CONTRIBUTING.md`, `ARCHITECTURE.md`, style guides, lint config). Then measure the undocumented ones by counting. For each convention the diff touches, report the count: "31 of 33 feature folders hold a single `*Slice.ts`", "`git grep -w userID` returns 212 hits and `userId` returns 4". The existing project is the standard. An ecosystem canon applies only where the project has no measurable convention.
>
> Then check every new or moved file, directory, module, import edge, and name in the diff against those conventions. For each departure, report the convention with its count, the departure with its location, and what the departure costs: a search that misses half the uses, a tool that does not discover the file, a second word for one concept, a boundary the project enforces elsewhere, a future change that must now touch two places.
>
> Report three classes separately. First, departures from an established project convention. Second, defects at a boundary (a name that changes spelling across serialization, storage, or a tool, or a file a tool reads under the wrong name), whether or not the project is consistent. Third, places where the project's existing convention is itself harmful. Report the third class only, never as a reason to depart from the convention in this change.
>
> Findings will be posted publicly to the author of this change. Report your full range; the synthesis stage filters.

The pass and the panel can flag the same line. `readability` judges whether one name is clear. This pass judges whether the name matches the project. Keep both if both clear the filter, and merge them into one comment when they anchor to the same line.

## The test pass

Every run makes a second, independent pass over the tests. It is not optional, and it does not depend on whether `test-coverage` was matched to a region in Stage 3.

Run it in parallel with the Stage 5 dispatch, as its own `test-coverage` agent call. Independent means the test pass reasons about the change from scratch rather than reading the other lenses' output: it asks what this change can get wrong, then asks whether any test fails when it does.

Give it this scope, in addition to the standard dispatch template:

> Review this change for what the tests do not catch. For each in-scope behavior: the branches and error paths no test exercises, the boundary and edge-case inputs no test supplies, the states and orderings no test reaches, and the assertions that pass whether or not the behavior is correct. Also audit the tests the change adds or edits: a test that cannot fail, asserts on a mock instead of the behavior, or re-asserts the implementation is a gap wearing a test's name.
>
> Ground findings in execution rather than reading where you can. Run the suite. Run coverage tooling if the repository configures it. Mutate the source -- invert a condition, drop a guard, change a boundary, delete a line -- re-run the affected tests, and report the mutants that survive. A surviving mutant is evidence a real defect could ship; name the mutation and the tests that stayed green. For each survivor, also name the user-facing guarantee it leaves unprotected, and one plausible edit (a refactor, a feature reusing the path, a cleanup of a line that looks redundant) that would make the same change for an ordinary reason. Run mutants against every test file that imports the mutated module, not only the files the change touches.
>
> Findings will be posted publicly to the author of this change. Report your full range; the synthesis stage filters.

Mutation is a local experiment, not an edit to the change. Every mutation is reverted before the pass reports, and the working tree is left as it was found. Confirm that with `git status --porcelain` before the local summary; if the tree is dirty from the pass, restore it before going further.

The test pass and the code passes can flag the same region. Keep both: one says the code has a defect, the other says nothing would catch it.

## The filter

Apply this after synthesis, to the merged finding list.

**A finding is posted when it has all three of these:**

1. **A defect.** Something behaves other than intended, not something written other than you would write it.
2. **A trigger.** The input, state, sequence, or timing that produces the defect, specific enough that the author could reproduce it.
3. **A consequence.** What a user, an operator, or a downstream system experiences when the trigger fires.

Write all three out for each candidate before deciding. A finding you cannot state in that form does not go in the review.

**Everything else is dropped.** Dropped means absent from the posted artifact: not inline, not in the body, not a question, not a forward-looking note, not an aside, not a parenthetical. A finding removed from the inline set and then mentioned in the body has not been dropped. This is the most common way the filter fails.

**A coverage finding meets those three through the bug it would let through.** A missing test is not itself a defect, so a finding that stops at "this path is untested" is dropped, whatever its confidence. To survive, it must name the specific wrong behavior that reaches production because no test fails: the defect is that behavior, the trigger is the input or state that produces it, the consequence is what the user or operator gets. "The retry path has no test" is dropped. "Nothing fails when the retry path double-charges, because the only test asserts the mock was called" is posted. A surviving mutant, named with the mutation and the tests that stayed green, is the strongest form of this evidence.

**A coverage finding must also name a likely edit that would cause the regression, or it is dropped.** A surviving mutant proves that a test is missing. It does not prove that anyone will ever make that change. The author's question is "why should I care that you could break it this way?" Answer it with an edit someone could plausibly make for an ordinary reason:
- A refactor that moves or merges the code, for example folding this helper into its caller.
- A new feature that reuses the path, for example adding a tab, a route, or a file type.
- A cleanup that deletes a line that looks redundant, for example a reset that seems to run twice, or a guard whose reason is not visible at the call site.
- A dependency or design-system change that alters a default the code relies on.

Coverage findings are not the maintenance hazards the verify step drops: the behavior is correct today and unpinned, and the fix is a test, not a code change.

**A coverage gap with no likely edit is still posted, as a short inline comment.** General coverage has value even when no specific edit threatens it. A surviving mutant with no likely edit is posted as a coverage gap when all of these are true:
- **The behavior is relevant to the pull request.** New or changed code qualifies. Existing behavior also qualifies when the change depends on it, moves it, or reuses its path. A gap in unrelated code next to the change is not posted.
- **A user or a contract depends on the behavior.** Ask whether anyone outside the code would notice if this behavior broke. That includes what a user sees or loses, an analytics property, a stored or sent value, and a function other modules call. The test is where the effect lands, not whether the function is public. A private helper qualifies when its output reaches the user, such as the function that decides which cards land in the deck. A branch no user or caller can reach does not qualify, such as a guard that only narrows a string to a union type.
- **A feasible test is named,** under the same rule as other coverage findings.
- **The panel rated the gap `minor` or higher.** A `nit` is still dropped.

A coverage gap does not need the `major` severity floor or a likely edit. When a gap also has a likely edit, it is a full coverage finding and gets the full comment shape described under Inline comments.

Keep coverage gaps from burying the real findings. When the gaps outnumber the other posted comments, keep the ones a user meets most directly and drop the rest.

Percentages and line-coverage numbers are not findings. Neither is a missing test for code that cannot fail, nor a request for tests in general.

**A coverage finding must also name the test that would catch it, or it is dropped.** Before posting, write out the specific test to add: which test file it goes in (or which existing test to extend), the setup or sequence it drives, and the assertion that fails when the behavior breaks. Prefer extending an existing test the file already has over a new one when the existing test already walks the path. When no test could realistically catch the behavior -- it needs a real OS, real hardware, real network timing, a microtask interleaving no harness can force deterministically, or a mock so deep the test would only re-assert the implementation -- drop the finding, whatever its severity or confidence. A gap that no feasible test closes is not actionable for the author.

**An alignment finding meets those three through the project's own convention.** The defect is the departure from a convention the project demonstrably follows. The trigger is the new file, directory, import, or name, at its location. The consequence is the concrete cost the agent named. An alignment finding is posted when it has all of these:

- **The convention is established.** A documented project rule, an enforced lint or boundary rule, or a measured count where the project follows the convention in nearly every case and the counts are stated. A count of 3 of 5 is not a convention. A count of 31 of 33 is.
- **The change departs from it** at a line in the diff.
- **The cost is concrete.** "Inconsistent" is not a cost. "A `git grep -w userID` now misses this field" is.

Alignment findings that meet those three take the lower floor described under Severity floor. They still need the confidence floor and the verify step. Post the count in the comment. The count is evidence about the code, and it makes the finding something other than a preference.

A departure toward a better convention is still a departure. Post it as an alignment finding. Do not post it as an endorsement of the new pattern. A pull request that wants to change the project's convention has to change it everywhere, or say in its description that a migration is starting.

Findings in the third class (the existing convention is itself harmful) are never posted on someone else's pull request. They are not about this change. Show them in the local summary under a separate line so the user can raise them elsewhere.

Boundary defects from the pass need no convention count. They take the same lower floor as alignment findings.

Severity floor: `blocker` and `major` only. `minor`, `nit`, and `insight` are dropped as a class, whatever their confidence.

**Exception: findings from the structure and naming pass have a floor of `minor`** (the panel scale's medium level). `nit` and `insight` from the pass are still dropped. The lower floor applies only to findings those two agents report. A naming or placement finding from any other lens takes the normal floor. A 95-confidence nit is still a nit. High confidence that something is cosmetic is not a reason to post it.

Confidence floor for posting: 70. A finding whose body says the reporter could not determine reachability is dropped rather than posted as a question, at any severity. Unresolved speculation posted to another person's pull request costs them time and costs the rest of the review its credibility.

### Verify every survivor against the code

Before a finding clears the filter, re-read the lines it cites and confirm the trigger it describes is the trigger the code has. A confidence number is a claim, not evidence. A 90 does not survive a fresh read that contradicts it.

Check these specifically, because they are where confident findings fail:

- **Stated durations and windows.** A claimed lost-update window of "hundreds of milliseconds" that the code shows is one lock release is a different finding, and usually not one.
- **Stated races between timers.** Two periodic jobs only race when their intervals and phases allow it. Read the actual values before accepting the race.
- **Maintenance hazards dressed as defects.** "A future field would be dropped here" fails the defect test when every current call site is correct. Drop it.

A finding whose trigger does not survive the re-read is dropped, whatever its severity or confidence.

### Worked example

Given a panel that returned: a non-transactional token rotation (blocker, 91), a quota check inside a per-file loop with load-test numbers attached (major, 84), a destructuring style preference (minor, 77), a naming inconsistency (nit, 88), trailing whitespace (nit, 95), a module-wide refactor suggestion (insight, 70), and a possible partial-write path the reporter could not determine was reachable (major, 52):

Two findings are posted. The token rotation and the quota loop. The other five appear nowhere in the review, including the refactor suggestion and the unreachable-path question.

Given a test pass that returned: `refresh.ts` at 41% branch coverage (major, 88), no test for the rotation error path (major, 80), a suite that stays green when the expiry comparison flips from `<` to `<=` so an expired token is accepted for one more request (major, 86), and a new test that asserts `mockStore.set` was called rather than what was stored (major, 75):

One finding is posted: the surviving `<=` mutant, because it names the behavior that ships, and because someone tidying the comparison to match its neighbors is a likely way to make that edit. The posted comment opens on the user-facing guarantee ("an expired token is refused"), not on the mutation. The percentage is dropped. The untested error path is dropped as written, and would post only if it named what goes wrong when that path runs. The assert-on-the-mock test is dropped unless the behavior it fails to check is itself wrong.

## Local summary

Before posting anything, print the complete intended review in chat:

- The headline verdict line.
- The body paragraph.
- Every inline comment, each with its file, line, and full text.
- Every `<details>` block.
- A one-line count: how many findings the panel produced, and how many cleared the filter.
- One line listing any convention the structure and naming pass found harmful in the existing project. These are not posted. Omit the line when there are none.

The test findings are summarized here with the rest, in one list, ordered with them by severity. Do not print them as a separate section, do not hold them back for a later message, and do not ask about them separately. One summary, one question, one post.

Then ask whether to post, and wait for the answer.

## Dev QA against the deployed build

Run this only when the invocation asks for dev QA. Invoke the `dev-qa` skill (Skill tool) in embedded mode, in the main session, right after the Stage 5 dispatch, so the listed steps run while the panel agents work. Pass the pull request number, the head, and the base. After synthesis, hand it the candidate findings that a browser can reproduce, for its live check. `/dev-qa` owns the target proof, the session proof, the steps, the evidence, the confirm stage, the writes, and the section shape. Do not restate or override its rules here.

Fold its hand-back into this review:

- The Dev QA section is the last part of the review body (see Body shape). The review and the Dev QA result are one top-level comment, not two.
- A candidate `/dev-qa` reproduced live is stronger evidence for that finding. It still goes through the filter. When it posts, the comment states the reproduction, and the section's Other observations block points to the comment in one line. A candidate it did not reproduce goes back through the verify step before it is posted.
- A step failure that also cleared the filter is a review finding. The section points to its comment in one line.
- The local summary shows the review and the section together, with one question. Upload the section's screenshots only after approval.

## Voice

Follow the voice rules in `~/.claude/rules/outside-pr-review.md`. One rule is specific to findings:

- Hedge each posted finding lightly, so it reads as a likely bug, not a verdict. Open with "Possible bug here:", write the claim with "may" ("a close that lands during the upgrade may be dropped for good"), and say in one plain clause what the claim rests on ("based on reading through the code and tweaking the tests a bit", or for a finding reproduced on the deployed build, "reproduced on the preview: <what happened>"). The headline line of the body takes the same hedge. A coverage finding is not a bug in today's code, so it does not open with "Possible bug here:". It opens on the guarantee, with the same light hedge: "Nothing currently pins that typed cards survive a click outside the modal." A short coverage-gap comment opens with "Possible coverage gap:" instead. The hedge sets the tone only. Keep the trigger, the consequence, and any reproduction exact. The filter still decides what gets posted, and the hedge never lets through a finding that failed the filter.
## Internals stay out

Follow the internals rule in `~/.claude/rules/outside-pr-review.md`. The review contains findings about the code and nothing about itself. These parts are specific to a code review.

The test pass is an internal too. A surviving test finding is posted as a finding about the code, interleaved with the others by severity and anchored to its own line. Nothing in the posted review says a test pass ran, groups the test findings together, or labels them as being about coverage rather than about the defect.

The same applies across comments. Do not number or rank findings against each other in the posted text ("the first group", "the cheapest of the three", "my other four comments"), and do not tell the author that a second round happened. Each comment stands on its own defect. The one exception is the summary comment described below, which may say it summarizes the others, because a reader needs that to not count it as another defect.

In a coverage comment, evidence about the tests ("deleting this line leaves the suite green") supports the guarantee and the likely edit. It never leads the comment and never replaces them (see Inline comments).

When naming what was examined (clean runs only, below), name the concern in plain language: "concurrency and race conditions", "authentication and access control", "input validation", "query patterns and load behavior". Never the internal name of a lens or agent.

## Body shape

The review body starts with the zone headers from `~/.claude/rules/outside-pr-review.md`, then the attribution line. After those, it has exactly these parts, in this order:

1. **A headline verdict line.** One short sentence naming what the review found. Declarative, and phrased as an observation rather than a decision about the pull request's fate: "Two correctness problems, both in the token rotation path." Never "Requesting changes", "Approving", "LGTM", "Two things to fix before merge", or any other phrase that reads as a merge decision. The review is a comment; the body must not imply otherwise.
2. **One paragraph in plain language.** What breaks and what it costs, for a reader who has not read the diff. Walk the mechanism in order: when X happens, Y does Z, so W results. Keep it to one paragraph.
3. **A `<details>` block per unanchored finding.** Only findings that cleared the filter and have no changed line to attach to. Each block: `<summary>` with a short title, then the finding in the same what-breaks, what-triggers-it, what-it-costs order, then the fix in one sentence.
4. **The Dev QA section**, only when the invocation asked for dev QA. `/dev-qa` defines its shape. Never post it as a separate comment beside the review, except in the pending-review case the posture file's delivery table covers.

Nothing else goes in the body on a run that has findings. No coverage inventory, no list of what was examined, no summary of what looked fine, no closing offer, no restatement of the inline comments.

## Inline comments

Every filtered-in finding that has a changed line attaches to that line.

Verify each anchor against the patch before posting, as the posture file describes. A finding whose line does not verify moves to a `<details>` block in the body, described by file path alone.

Attach to the closest changed line that relates to the issue. A finding about code a few lines from the edit still anchors to the edit, as long as the connection is clear from the comment text.

Comment text is the finding itself: what breaks, what triggers it, what it costs, and the fix if it is short. No severity labels, no confidence numbers, no lens names. For a coverage finding, the fix is the test: name the file, the scenario it drives, and the assertion that fails when the behavior breaks.

**A coverage comment is ordered by what the author cares about, not by how the gap was found:**

1. **The guarantee.** One sentence on what a user or downstream system relies on this line for, in their terms: "Typed cards survive a stray click outside the modal." Not "removing `onInteractOutside` leaves the suite green."
2. **The likely edit.** The plausible refactor, feature, or cleanup that would break the guarantee, and why it would look safe to the person making it: "this prop looks like a duplicate of `onEscapeKeyDown`, and nothing tells the next person why both are there."
3. **The evidence.** At most one clause, after the first two: "No current test fails with it removed." Leave out test counts, suite sizes, and a list of every mutation you tried.
4. **The test.** The file, the scenario, and the failing assertion.

Use one guarantee per comment. If several mutants protect one guarantee, merge them into one comment that names the guarantee once. Do not list the mutants.

**A coverage gap with no likely edit gets a short comment:** two or three sentences anchored to the line it covers. It opens with "Possible coverage gap:", names the untested case in user terms, and names the test that would cover it. It has no likely-edit story, no evidence clause, and no bug framing.

> Possible coverage gap: no test pastes text into the dialog and then presses Escape, so nothing checks that the pasted text survives it. A case next to the existing "stays open on Escape" test would cover it.

When several gaps anchor to the same line, merge them into one short comment with one sentence per case.

## Posting

Post through the mechanics in `~/.claude/rules/outside-pr-review.md`: the pending-review check, the single create call, the GraphQL append path, attribution per comment when the body is unavailable, and where the summary paragraph and the Dev QA section go when it is. The summary paragraph that moves into an inline comment anchors to the changed line the findings are most about (the shared type, the shared call site, the shared branch).

## Clean runs

When no finding clears the filter, say so in chat, and offer to post a short review whose body is:

1. A headline verdict line stating that the review found no blocking problems.
2. One short paragraph.
3. A list of the concern areas examined, in plain language. Name the test examination among them, in plain words ("what the tests would catch if this broke"), when the test pass produced nothing that cleared the filter. Name the structure and naming examination the same way ("fit with the project's existing layout and naming") when it ran and produced nothing that cleared the filter. Do not name it when it did not run.
4. The Dev QA section, when dev QA ran.

The structure and naming pass is an internal, like the test pass. A posted alignment finding is a finding about the code, interleaved with the others by severity. Nothing in the posted review says that a structure or naming pass ran.

That list of examined areas appears **only** on clean runs. On a run with findings it does not appear, because the findings are the substance and the inventory competes with them.

Ask before posting the clean review. The user may prefer silence on the pull request.

## Red flags

Any of these means stop and re-apply the filter or the voice rules:

- An inline comment about whitespace, formatting, or import order.
- A Dev QA result posted as its own comment while the review body was available to carry it.
- An inline comment about naming or file placement that does not state the project's convention with its count, or does not name a concrete cost.
- An alignment comment that argues for an ecosystem guide over the project's own convention.
- A finding you cut from inline that reappears in the body as a question or a note.
- The words "consider", "might want to", "could be cleaner", "nit:", "worth a ticket" in the posted text.
- A body that opens by praising the change.
- A body sentence containing "approve", "requesting changes", "LGTM", "before merge", or "ship it".
- A list of what was examined on a run that has findings.
- Any sentence describing how the review was produced.
- **A comment whose first clause names a kind of review rather than a defect**: "Test coverage pass.", "Second pass.", "A review focused on X." Read every comment's opening sentence alone; this is where internals leak.
- A comment that positions itself against the others: "the first group", "the cheapest of the three", "my other four comments", "a second pass found".
- A posted finding that stops at "this is untested" without naming the behavior that ships wrong.
- A posted coverage finding that does not name the test to add (file, scenario, failing assertion), or that names a test no harness could realistically run.
- A coverage comment that opens on the mutation ("If this line is deleted, all N tests still pass"). The author reads it as "why should I care that you broke it this way?"
- A full-shape coverage comment with no likely edit a real person would make. It belongs in the short "Possible coverage gap:" form.
- A "Possible coverage gap:" comment that invents a future edit, frames the gap as a bug, covers code the pull request does not touch or depend on, or runs past three sentences.
- Test counts or suite sizes ("all 1384 tests in 43 files") in a posted comment or body.
- A coverage percentage, a line-coverage number, or a coverage-tool name in the posted text.
- Test findings grouped together, posted after the others, or surfaced in a second message.
- The word "I" attached to a preference rather than an observation.

## What NOT to do

- Do not submit `APPROVE` or `REQUEST_CHANGES` under any circumstance.
- Do not post without an approval that answers the question you asked.
- Do not post a finding whose anchor you could not verify against the patch.
- Do not append to someone's hand-written pending review without asking.
- Do not write into another person's review body, and do not edit or re-prefix the comments they wrote.
- Do not apply fixes or push commits. This skill reads and comments; the test pass's mutations are reverted, never committed and never suggested as the change.
- Do not report a finding count in the posted body. The comments speak for themselves.
- Do not re-post a finding the pull request already carries as a comment. Fetch existing comments and match on the defect, not the location.

## Decision references

- Shared outside-reviewer posture and posting: `~/.claude/rules/outside-pr-review.md`
- Dev QA process and section shape: `~/.claude/skills/dev-qa/SKILL.md`, project bindings in `~/.claude/local/dev-qa.md`
- Panel process and dispatch: `~/.claude/skills/expert-review/SKILL.md`
- Structure and naming lenses: `~/.claude/rules/project-structure.md`, `~/.claude/rules/naming-conventions.md`
- Panel output contract: `~/.claude/rules/panel-contract.md`
- Agent mode contract: `~/.claude/skills/agent-modes/SKILL.md`
