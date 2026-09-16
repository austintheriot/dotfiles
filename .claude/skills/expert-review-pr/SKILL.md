---
name: expert-review-pr
description: Use when reviewing someone else's pull request and posting the result to GitHub as an outside reviewer. Runs the expert panel over the PR diff plus a second independent pass over the tests (untested paths, uncovered edge cases, surviving mutants), holds both to the same bar of a named defect with a concrete trigger and a real consequence, summarizes locally, and on approval posts a COMMENT-only review with inline comments. Not for reviewing your own work -- use /expert-review for that.
---

# Expert Review PR

Review another person's pull request and post the result to GitHub.

Analysis comes from the `/expert-review` panel. Everything after analysis is different: a far harsher filter, a posted artifact, and a voice that is openly Claude's rather than the user's.

## Two rules that override everything else in this file

1. **The review is posted only after the user says to post it, in a message that answers the question you asked.** Draft, summarize, ask, wait. Silence is not approval. A prior approval in this session does not carry to a second run.
2. **The submitted review event is always `COMMENT`.** Never `APPROVE`. Never `REQUEST_CHANGES`. A human decides whether a pull request is approved or blocked.

## Scope resolution

No argument: resolve the open pull request for the checked-out branch.

```bash
gh pr view --json number,url,author,headRefName,baseRefName,title,body
```

An argument that is a number or a pull-request URL overrides the branch. If the branch has no open pull request, stop and say so; do not guess at a nearby one.

Resolve the posting identity once, for the self-authorship check and for pending-review discovery:

```bash
gh api graphql -f query='query { viewer { login } }' --jq '.data.viewer.login'
```

If the pull request author equals that login, print one line and continue:

> This pull request is authored by you. `/expert-review` is the skill built for your own work. Continuing.

## Analysis

Follow `/expert-review`'s process for Stage 1 through Stage 6: fetch the change set with the `pr-diff` agent, discover project conventions, classify regions, dispatch specialists in parallel, synthesize with the skepticism discipline.

Read `~/.claude/skills/expert-review/SKILL.md` for those stages rather than reimplementing them. Two amendments to the dispatch prompt:

- Mode is always `diff`.
- Add to each dispatch prompt: *"Findings will be posted publicly to the author of this change. Only findings that name a concrete trigger and a concrete consequence will survive synthesis. Report your full range as the panel contract defines it; the synthesis stage filters."*

Do not tell the subagents to self-filter for severity. They obey that literally and drop real bugs. The filter below is applied once, at synthesis, where the whole set is visible.

## The test pass

Every run makes a second, independent pass over the tests. It is not optional, and it does not depend on whether `test-coverage` was matched to a region in Stage 3.

Run it in parallel with the Stage 5 dispatch, as its own `test-coverage` agent call. Independent means the test pass reasons about the change from scratch rather than reading the other lenses' output: it asks what this change can get wrong, then asks whether any test fails when it does.

Give it this scope, in addition to the standard dispatch template:

> Review this change for what the tests do not catch. For each in-scope behavior: the branches and error paths no test exercises, the boundary and edge-case inputs no test supplies, the states and orderings no test reaches, and the assertions that pass whether or not the behavior is correct. Also audit the tests the change adds or edits: a test that cannot fail, asserts on a mock instead of the behavior, or re-asserts the implementation is a gap wearing a test's name.
>
> Ground findings in execution rather than reading where you can. Run the suite. Run coverage tooling if the repository configures it. Mutate the source -- invert a condition, drop a guard, change a boundary, delete a line -- re-run the affected tests, and report the mutants that survive. A surviving mutant is evidence a real defect could ship; name the mutation and the tests that stayed green.
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

Percentages and line-coverage numbers are not findings. Neither is a missing test for code that cannot fail, nor a request for tests in general.

Severity floor: `blocker` and `major` only. `minor`, `nit`, and `insight` are dropped as a class, whatever their confidence. A 95-confidence nit is still a nit. High confidence that something is cosmetic is not a reason to post it.

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

One finding is posted: the surviving `<=` mutant, because it names the behavior that ships. The percentage is dropped. The untested error path is dropped as written, and would post only if it named what goes wrong when that path runs. The assert-on-the-mock test is dropped unless the behavior it fails to check is itself wrong.

## Local summary

Before posting anything, print the complete intended review in chat:

- The headline verdict line.
- The body paragraph.
- Every inline comment, each with its file, line, and full text.
- Every `<details>` block.
- A one-line count: how many findings the panel produced, and how many cleared the filter.

The test findings are summarized here with the rest, in one list, ordered with them by severity. Do not print them as a separate section, do not hold them back for a later message, and do not ask about them separately. One summary, one question, one post.

Then ask whether to post, and wait for the answer.

## Voice

The posted review is Claude's own writing, and reads that way.

- Do not invoke `/write-like-austin`. Do not imitate the user's voice.
- No praise, no flattery, no compliments on the change. Open on the finding, not on what the pull request does well.
- No softeners: "ignore me", "worth a ticket if you agree", "just a thought", "feel free to disregard", "nice work but".
- Plain literal words. A reader who has not seen the diff should follow it without decoding a figure of speech.
- State uncertainty as a fact when it is load-bearing: "I did not verify X." Do not hedge a finding you are posting.
- Follow the repository's own attribution convention if it defines one. Read the repository `CLAUDE.md` for a rule about identifying AI-authored comments and follow what it says. Add no attribution line if the repository defines none.

## Internals stay out

The posted review says nothing about how it was produced. No mention of a panel, experts, lenses, agents, subagents, skills, severities, confidence scores, dispatch, or synthesis. No mention that the review is one of several passes, or that a local tool ran.

The review contains findings about the code and nothing about itself.

The test pass is an internal too. A surviving test finding is posted as a finding about the code, interleaved with the others by severity and anchored to its own line. Nothing in the posted review says a test pass ran, groups the test findings together, or labels them as being about coverage rather than about the defect.

**This rule is violated most often in a comment's opening clause**, where a scene-setting phrase feels like orientation rather than internals. Every one of these is a violation: "Test coverage pass.", "A second review pass focused on X.", "Second pass here.", "From the coverage review,". Delete the clause and open on the finding. A comment that begins by saying what kind of review it is has already broken the rule.

The same applies across comments. Do not number or rank findings against each other in the posted text ("the first group", "the cheapest of the three", "my other four comments"), and do not tell the author that a second round happened. Each comment stands on its own defect. The one exception is the summary comment described below, which may say it summarizes the others, because a reader needs that to not count it as another defect.

**Technique is not process.** Naming how you obtained evidence about the code is allowed, and often makes a finding credible: "deleting this line leaves the suite green at 1213 passed", "I could not get a mutant past this latch", "changing `??=` to `=` and re-running the suite". Those are facts about the code and its tests. Naming how the review was organized is not: passes, rounds, panels, lenses, agents, severities, confidence scores. Evidence in, org chart out.

When naming what was examined (clean runs only, below), name the concern in plain language: "concurrency and race conditions", "authentication and access control", "input validation", "query patterns and load behavior". Never the internal name of a lens or agent.

Before posting, re-read the first sentence of every comment on its own. If it describes the review rather than the code, rewrite it.

## Body shape

The review body has exactly these parts, in this order:

1. **A headline verdict line.** One short sentence naming what the review found. Declarative, and phrased as an observation rather than a decision about the pull request's fate: "Two correctness problems, both in the token rotation path." Never "Requesting changes", "Approving", "LGTM", "Two things to fix before merge", or any other phrase that reads as a merge decision. The review is a comment; the body must not imply otherwise.
2. **One paragraph in plain language.** What breaks and what it costs, for a reader who has not read the diff. Walk the mechanism in order: when X happens, Y does Z, so W results. Keep it to one paragraph.
3. **A `<details>` block per unanchored finding.** Only findings that cleared the filter and have no changed line to attach to. Each block: `<summary>` with a short title, then the finding in the same what-breaks, what-triggers-it, what-it-costs order, then the fix in one sentence.

Nothing else goes in the body on a run that has findings. No coverage inventory, no list of what was examined, no summary of what looked fine, no closing offer, no restatement of the inline comments.

## Inline comments

Every filtered-in finding that has a changed line attaches to that line.

Verify the anchor before posting. Fetch the patch for the file and confirm the line is in the changed range:

```bash
gh api "repos/$REPO/pulls/$PR/files" --jq '.[] | select(.filename=="<path>") | .patch'
```

The patch hunk headers (`@@ -old,count +new,count @@`) give the valid line numbers on the `RIGHT` side. A finding whose line does not verify moves to a `<details>` block in the body, described by file path alone. Never post an anchor you just disproved.

Attach to the closest changed line that relates to the issue. A finding about code a few lines from the edit still anchors to the edit, as long as the connection is clear from the comment text.

Comment text is the finding itself: what breaks, what triggers it, what it costs, and the fix if it is short. No severity labels, no confidence numbers, no lens names.

## Posting

Check for an existing pending review by the posting identity:

```bash
gh api "repos/$REPO/pulls/$PR/reviews" --paginate \
  --jq '[.[] | select(.state=="PENDING" and .user.login=="<viewer>")] | {count: length, ids: [.[].id]}'
```

This check goes stale. A human can start a review between the check and the post, so treat a `422` saying `User can only have one pending review per pull request` on the create call as a pending review that appeared mid-run, not as an error to retry. Switch to the append path below.

**If a pending review exists**, it may contain comments the user wrote by hand. Fetch its comments, show the user what is already staged, and ask whether to append to it or create a separate review. Do not merge into a hand-written draft without asking.

**If none exists**, create the review with all comments in one call. A single POST with a `comments` array and `event: "COMMENT"` both creates and submits:

```bash
gh api "repos/$REPO/pulls/$PR/reviews" \
  --method POST \
  --input review.json
```

Where `review.json` is:

```json
{
  "body": "<the body>",
  "event": "COMMENT",
  "comments": [
    { "path": "src/session/refresh.ts", "line": 88, "side": "RIGHT", "body": "<finding>" }
  ]
}
```

Build the JSON with a heredoc or `jq` rather than inline shell escaping; review bodies contain backticks, quotes, and newlines that break naive quoting.

After posting, print the review URL.

### Appending to a pending review

REST cannot add a comment to an existing pending review. Do not spend calls rediscovering this:

- `POST repos/{owner}/{repo}/pulls/{pr}/reviews/{review_id}/comments` returns `404`. It does not add comments to a pending review.
- `POST repos/{owner}/{repo}/pulls/{pr}/comments` returns `422` with `user_id can only have one pending review per pull request`. It tries to create its own review instead of joining the pending one.

Use GraphQL. First get the pending review's node ID:

```bash
gh api graphql -f query='query {
  repository(owner:"OWNER", name:"REPO") {
    pullRequest(number:NNN) {
      reviews(last:5, states:PENDING) {
        nodes { id state author { login } }
      }
    }
  }
}'
```

Then append each finding as a thread on that review with `addPullRequestReviewThread`, passing `pullRequestReviewId`, `path`, `line`, `side: RIGHT`, and `body`. One mutation per comment.

**Read the human's existing comments first.** Fetch them before writing anything, so you do not restate a point they already made and do not touch what they wrote:

```bash
gh api graphql -f query='query {
  repository(owner:"OWNER", name:"REPO") {
    pullRequest(number:NNN) {
      reviews(last:5, states:PENDING) {
        nodes {
          comments(first:20) { nodes { id databaseId path body } }
        }
      }
    }
  }
}'
```

Pending-review comments are invisible to REST: `GET repos/{owner}/{repo}/pulls/comments/{comment_id}` returns `404` for one that belongs to an unsubmitted review, so the read-modify-write `PATCH` cycle does not work. Edit with the `updatePullRequestReviewComment` mutation, passing `pullRequestReviewCommentId` and the new `body`. That ID is the GraphQL node `id` (a `PRRC_...` string), not the `databaseId`. Passing `databaseId` fails.

**Never write the review body.** On someone else's pending review the body is theirs, and text you put there publishes under their name. Leave it untouched and leave it empty if it is empty.

**Attribution goes on each comment you author.** When the repository defines an AI-attribution convention, the body is not available to carry it, so every inline comment you write carries the prefix on its own line, then a blank line, then the finding. Never add the prefix to a comment the human wrote.

### The summary as an extra inline comment

When the body is unavailable, the paragraph that would have gone in it becomes one more inline comment. Anchor it to the changed line the findings are most about (the shared type, the shared call site, the shared branch). Open it with a sentence saying it summarizes the other comments, so a reader does not count it as one more defect.

## Clean runs

When no finding clears the filter, say so in chat, and offer to post a short review whose body is:

1. A headline verdict line stating that the review found no blocking problems.
2. One short paragraph.
3. A list of the concern areas examined, in plain language. Name the test examination among them, in plain words ("what the tests would catch if this broke"), when the test pass produced nothing that cleared the filter.

That list of examined areas appears **only** on clean runs. On a run with findings it does not appear, because the findings are the substance and the inventory competes with them.

Ask before posting the clean review. The user may prefer silence on the pull request.

## Red flags

Any of these means stop and re-apply the filter or the voice rules:

- An inline comment about whitespace, naming, formatting, or import order.
- A finding you cut from inline that reappears in the body as a question or a note.
- The words "consider", "might want to", "could be cleaner", "nit:", "worth a ticket" in the posted text.
- A body that opens by praising the change.
- A body sentence containing "approve", "requesting changes", "LGTM", "before merge", or "ship it".
- A list of what was examined on a run that has findings.
- Any sentence describing how the review was produced.
- **A comment whose first clause names a kind of review rather than a defect**: "Test coverage pass.", "Second pass.", "A review focused on X." Read every comment's opening sentence alone; this is where internals leak.
- A comment that positions itself against the others: "the first group", "the cheapest of the three", "my other four comments", "a second pass found".
- A posted finding that stops at "this is untested" without naming the behavior that ships wrong.
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

- Panel process and dispatch: `~/.claude/skills/expert-review/SKILL.md`
- Panel output contract: `~/.claude/rules/panel-contract.md`
- Agent mode contract: `~/.claude/skills/agent-modes/SKILL.md`
