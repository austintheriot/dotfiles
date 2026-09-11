---
name: expert-review-pr
description: Use when reviewing someone else's pull request and posting the result to GitHub as an outside reviewer. Runs the expert panel over the PR diff, keeps only findings that name a concrete trigger and a real consequence, summarizes locally, and on approval posts a COMMENT-only review with inline comments. Not for reviewing your own work -- use /expert-review for that.
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

## The filter

Apply this after synthesis, to the merged finding list.

**A finding is posted when it has all three of these:**

1. **A defect.** Something behaves other than intended, not something written other than you would write it.
2. **A trigger.** The input, state, sequence, or timing that produces the defect, specific enough that the author could reproduce it.
3. **A consequence.** What a user, an operator, or a downstream system experiences when the trigger fires.

Write all three out for each candidate before deciding. A finding you cannot state in that form does not go in the review.

**Everything else is dropped.** Dropped means absent from the posted artifact: not inline, not in the body, not a question, not a forward-looking note, not an aside, not a parenthetical. A finding removed from the inline set and then mentioned in the body has not been dropped. This is the most common way the filter fails.

Severity floor: `blocker` and `major` only. `minor`, `nit`, and `insight` are dropped as a class, whatever their confidence. A 95-confidence nit is still a nit. High confidence that something is cosmetic is not a reason to post it.

Confidence floor for posting: 70. A finding whose body says the reporter could not determine reachability is dropped rather than posted as a question, at any severity. Unresolved speculation posted to another person's pull request costs them time and costs the rest of the review its credibility.

### Worked example

Given a panel that returned: a non-transactional token rotation (blocker, 91), a quota check inside a per-file loop with load-test numbers attached (major, 84), a destructuring style preference (minor, 77), a naming inconsistency (nit, 88), trailing whitespace (nit, 95), a module-wide refactor suggestion (insight, 70), and a possible partial-write path the reporter could not determine was reachable (major, 52):

Two findings are posted. The token rotation and the quota loop. The other five appear nowhere in the review, including the refactor suggestion and the unreachable-path question.

## Local summary

Before posting anything, print the complete intended review in chat:

- The headline verdict line.
- The body paragraph.
- Every inline comment, each with its file, line, and full text.
- Every `<details>` block.
- A one-line count: how many findings the panel produced, and how many cleared the filter.

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

When naming what was examined (clean runs only, below), name the concern in plain language: "concurrency and race conditions", "authentication and access control", "input validation", "query patterns and load behavior". Never the internal name of a lens or agent.

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

## Clean runs

When no finding clears the filter, say so in chat, and offer to post a short review whose body is:

1. A headline verdict line stating that the review found no blocking problems.
2. One short paragraph.
3. A list of the concern areas examined, in plain language.

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
- The word "I" attached to a preference rather than an observation.

## What NOT to do

- Do not submit `APPROVE` or `REQUEST_CHANGES` under any circumstance.
- Do not post without an approval that answers the question you asked.
- Do not post a finding whose anchor you could not verify against the patch.
- Do not append to someone's hand-written pending review without asking.
- Do not apply fixes or push commits. This skill reads and comments.
- Do not report a finding count in the posted body. The comments speak for themselves.
- Do not re-post a finding the pull request already carries as a comment. Fetch existing comments and match on the defect, not the location.

## Decision references

- Panel process and dispatch: `~/.claude/skills/expert-review/SKILL.md`
- Panel output contract: `~/.claude/rules/panel-contract.md`
- Agent mode contract: `~/.claude/skills/agent-modes/SKILL.md`
