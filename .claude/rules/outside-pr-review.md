---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Outside pull-request review: shared posture

The shared contract for any skill that posts Claude's own work onto another person's pull request. `/expert-review-pr` (code findings) and `/dev-qa` (results of testing the deployed build) both read this file. Each skill keeps its own analysis, its own filter, and its own body shape. This file holds what they share: approval, the review event, scope resolution, voice, the internals rule, the local summary, attachments, and the GitHub posting mechanics.

When a skill and this file disagree, the skill wins for its own artifact.

## Two rules that override everything else

1. **The post happens only after the user says to post it, in a message that answers the question you asked.** Draft, summarize, ask, wait. Silence is not approval. A prior approval in this session does not carry to a second run.
2. **The submitted review event is always `COMMENT`.** Never `APPROVE`. Never `REQUEST_CHANGES`. A human decides whether a pull request is approved or blocked.

A standing authorization written for an unattended run (for example a scheduled-run prompt that says "create a pending review, never submit") replaces rule 1 for that run only, and only in the way it states.

## Scope resolution

No argument: resolve the open pull request for the checked-out branch.

```bash
gh pr view --json number,url,author,headRefName,headRefOid,baseRefName,title,body
```

An argument that is a number or a pull-request URL overrides the branch. If the branch has no open pull request, stop and say so. Do not guess at a nearby one.

Resolve the posting identity once, for the self-authorship check and for pending-review discovery:

```bash
gh api graphql -f query='query { viewer { login } }' --jq '.data.viewer.login'
```

If the pull request author equals that login, print one line naming the self-review skill for that artifact, and continue.

## Voice

The posted text is Claude's own writing, and reads that way.

- Do not invoke `/write-like-austin`. Do not imitate the user's voice.
- No praise, no flattery, no compliments on the change. Open on the result, not on what the pull request does well.
- No softeners: "ignore me", "worth a ticket if you agree", "just a thought", "feel free to disregard", "nice work but".
- Plain literal words. A reader who has not seen the diff follows it without decoding a figure of speech.
- State uncertainty as a fact when it is load-bearing: "I did not verify X."
- Attribution comes from the first source that defines it: the user's global instructions for that project (for example a "Posted by Claude on behalf of" rule in `~/.claude/CLAUDE.md`), then the repository's own rule in its `CLAUDE.md`. Add no attribution line when neither defines one. One attribution line at the top of the body covers the whole body, including every section another skill contributes to it. **Every inline comment Claude writes also starts with the attribution line**, then a blank line, then the comment. An inline comment shows on its own in the Files tab and in notifications, so the body's line does not cover it.
- **Zone headers.** The top-level body (the review body, or a separate top-level comment that carries body content) starts with these two headings, so Austin can write his own text above Claude's before he submits:

  ```markdown
  # Pre-Claude zone

  # Claude zone

  ```

  The `Pre-Claude zone` heading stays empty. Everything Claude writes, starting with the attribution line, goes under `Claude zone`. Inline comments get no zone headers. Never add the headers to a body someone else wrote.

## Internals stay out

The posted text says nothing about how it was produced. No mention of a panel, experts, lenses, agents, subagents, skills, severities, confidence scores, dispatch, synthesis, confirm stages, or plans. No mention that the post is one of several passes, or that a local tool ran.

**This rule is violated most often in an opening clause**, where a scene-setting phrase feels like orientation: "Test coverage pass.", "A second review pass focused on X.", "Automated QA run.", "From the QA agent,". Delete the clause and open on the result.

**Technique is not process.** Naming how you obtained evidence is allowed, and often makes a result credible: "deleting this line leaves the suite green", "signed in as a non-employee account, the page returns 404", "the deployed commit is `abc1234`". Naming how the work was organized is not: passes, rounds, panels, lenses, agents, severities, confidence scores. Evidence in, org chart out.

Before posting, re-read the first sentence of every comment and section on its own. If it describes the review rather than the code or the build, rewrite it.

## Orchestrated mode

The argument `--contribution <path>` selects orchestrated mode. The skill writes its result as one JSON file at that path. An orchestrator then merges the contributions and posts.

Rules for the skill in this mode:

1. Write the file in one call of the Write tool, as the last action of the run.
2. Never upload anything. Never create or edit a review or a comment. Never ask to post.
3. Use local file paths in `body` and in every comment. List every local path that the text cites in `uploads`, in citation order.
4. Add no attribution line and no zone headers. The orchestrator adds both.

The orchestrator owns the summary, the approval, the uploads, the merge, and the post.

### Contribution contract

Every contribution is one JSON object. This example is a `code-review` contribution:

```json
{
  "version": 1,
  "kind": "code-review",
  "pr": 1234,
  "head": "<full head SHA>",
  "body": "<top-level body markdown, local file paths in place of URLs>",
  "comments": [{"path": "src/a.ts", "line": 42, "side": "RIGHT", "startLine": 40, "body": "<markdown>"}],
  "uploads": ["/abs/local/path.png"],
  "handback": ["<one line per item the orchestrator must see>"]
}
```

A `dev-qa` contribution has the same fields plus `"platform": "<free label, optional>"`, `"status": "passed" | "failed" | "incomplete" | "not-run"`, and `"reason"` (required if the status is not `passed`). Its `body` is the Dev QA section only.

Both kinds also carry `findings`, so an orchestrator can route single findings to the other skill:

```json
"findings": [{"id": "R3", "kind": "code" | "behavior", "title": "<one line>", "body": "<markdown>",
              "anchor": {"path": "src/a.ts", "line": 42, "side": "RIGHT"},
              "behavior": "<the user-visible symptom, when there is one>", "evidence": ["/abs/local/path.mp4"]}]
```

`id` is unique in the contribution (`R<n>` for review, `Q<n>` for Dev QA). Every inline comment in `comments` names the finding id it came from in a `finding` field. A review finding with no user-visible effect has no `behavior`, and an orchestrator does not send it to Dev QA.

Two round-two contributions answer a list of findings from the other skill:

- `kind: "dev-qa-verify"`: `verdicts: [{id, result: "reproduced" | "not-reproduced" | "not-reachable", evidence: [...], note}]`.
- `kind: "code-trace"`: `verdicts: [{id, result: "root-cause" | "no-code-cause" | "inconclusive", anchor?, body, duplicateOf?}]`. `duplicateOf` names a review finding id that already covers the same defect.

A round-two contribution carries `version`, `kind`, `pr`, `head`, `verdicts`, `uploads`, and `handback`. It has no `body`, `comments`, or `findings`.

#### Top-level fields

| Field | Type | Required | Meaning |
|---|---|---|---|
| `version` | number, always `1` | all kinds | Schema version. |
| `kind` | `"code-review"`, `"dev-qa"`, `"dev-qa-verify"`, or `"code-trace"` | all kinds | Selects the shape of the rest of the object. |
| `pr` | number | all kinds | Pull request number. |
| `head` | string | all kinds | Full head SHA that the skill examined. An orchestrator refuses to merge contributions with different heads. |
| `body` | string | `code-review`, `dev-qa` | Top-level body markdown with local paths in place of URLs. For `dev-qa`, the body is the Dev QA section only. |
| `comments` | array of comment | `code-review`, `dev-qa` | Inline comments. The array can be empty. |
| `uploads` | array of string | all kinds | Absolute local paths that the text cites, in citation order. The array can be empty. |
| `handback` | array of string | all kinds | One line per item the orchestrator must see. The array can be empty. |
| `findings` | array of finding | `code-review`, `dev-qa` | Findings that an orchestrator can route to the other skill. The array can be empty. |
| `platform` | string | `dev-qa`, optional | Free label for the test environment. |
| `status` | `"passed"`, `"failed"`, `"incomplete"`, or `"not-run"` | `dev-qa` | Overall result of the Dev QA run. |
| `reason` | string | `dev-qa`, required when `status` is not `passed` | Why the run did not pass. |
| `verdicts` | array of verdict | `dev-qa-verify`, `code-trace` | One verdict per finding that the skill received. The verdict shape depends on `kind`. |

#### Comment fields

| Field | Type | Required | Meaning |
|---|---|---|---|
| `path` | string | yes | File path in the pull request. |
| `line` | number | yes | Line number on the chosen side. |
| `side` | `"LEFT"` or `"RIGHT"` | yes | Diff side. `RIGHT` is the new file. |
| `startLine` | number | optional | First line of a multi-line comment. `line` is the last line. |
| `body` | string | yes | Comment markdown. |
| `finding` | string | yes | `id` of the finding that this comment came from. |

#### Finding fields

| Field | Type | Required | Meaning |
|---|---|---|---|
| `id` | string | yes | Unique within the contribution. `R<n>` for review, `Q<n>` for Dev QA. |
| `kind` | `"code"` or `"behavior"` | yes | `code` is a defect in the diff. `behavior` is a symptom seen in the running build. |
| `title` | string | yes | One line. |
| `body` | string | yes | Markdown. |
| `anchor` | object | optional | Location in the diff: `path` (string), `line` (number), `side` (`"LEFT"` or `"RIGHT"`). |
| `behavior` | string | optional | The user-visible symptom. Omit it when the finding has no user-visible effect. |
| `evidence` | array of string | optional | Absolute local paths of screenshots or videos. |

#### Verdict fields

| Field | Type | Required | Meaning |
|---|---|---|---|
| `id` | string | yes, both kinds | `id` of the finding that the verdict answers. |
| `result` | string | yes, both kinds | `dev-qa-verify`: `"reproduced"`, `"not-reproduced"`, or `"not-reachable"`. `code-trace`: `"root-cause"`, `"no-code-cause"`, or `"inconclusive"`. |
| `evidence` | array of string | `dev-qa-verify`, yes | Absolute local paths that show the result. |
| `note` | string | `dev-qa-verify`, yes | One or two sentences on what the run did and saw. |
| `anchor` | object | `code-trace`, optional | Location of the cause. Same shape as a finding `anchor`. |
| `body` | string | `code-trace`, yes | Markdown explanation of the trace. |
| `duplicateOf` | string | `code-trace`, optional | Review finding `id` that already covers the same defect. |

## Local summary

Run the pending-review check (under Posting) before you print the summary, so the summary shows the real delivery shape.

Before posting anything, print the complete intended post in chat: the whole body, every inline comment with its file, line, and full text, every `<details>` block, and every attachment by local path. Then ask one question, whether to post, and wait for the answer.

Every run, standalone or orchestrated, ends in one pending review per PR head. The tool `draft-review` builds that review from every contribution for that head. A standalone run asks one question before `draft-review create`, because that step uploads.

## Attachments

Uploads are permanent. Upload only after the user approves the post, and before the review is created, because the returned URLs go into the body. Before approval, the local summary shows local file paths in place of URLs. The project file the calling skill names (for example `~/.claude/local/dev-qa.md`) says which upload endpoint to use and how to embed the result. After posting, check that every embedded image resolves.

## Posting

Check for an existing pending review by the posting identity:

```bash
gh api "repos/$REPO/pulls/$PR/reviews" --paginate \
  --jq '[.[] | select(.state=="PENDING" and .user.login=="<viewer>")] | {count: length, ids: [.[].id]}'
```

This check goes stale, because a human can start a review between the check and the post. A `422` on the create call saying `User can only have one pending review per pull request` means a pending review appeared mid-run. It is not an error to retry. Switch to the append path below.

**If a pending review exists**, it can contain comments the user wrote by hand. GitHub allows one pending review per user, so a second review cannot be created while it exists. Fetch its comments, show the user what is already staged, and offer two choices: append to it (the user then submits it), or hold the post until the user submits or discards it. Do not merge into a hand-written draft without asking.

**If none exists**, create the review with all comments in one call. A single POST with a `comments` array and `event: "COMMENT"` both creates and submits:

```bash
gh api "repos/$REPO/pulls/$PR/reviews" --method POST --input review.json
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

A post with no inline comments omits the `comments` key. Build the JSON with the Write tool or `jq`, not inline shell escaping. Bodies contain backticks, quotes, and newlines that break naive quoting.

After posting, print the review URL.

### Verify inline anchors

Before posting an inline comment, fetch the patch for the file and confirm the line is in the changed range:

```bash
gh api "repos/$REPO/pulls/$PR/files" --jq '.[] | select(.filename=="<path>") | .patch'
```

The hunk headers (`@@ -old,count +new,count @@`) give the valid `RIGHT`-side lines. A comment whose line does not verify moves into the body, described by file path alone. Never post an anchor you just disproved.

### Appending to a pending review

REST cannot add a comment to an existing pending review. Do not spend calls rediscovering this:

- `POST repos/{owner}/{repo}/pulls/{pr}/reviews/{review_id}/comments` returns `404`.
- `POST repos/{owner}/{repo}/pulls/{pr}/comments` returns `422` with `user_id can only have one pending review per pull request`.

Use GraphQL. Get the pending review's node ID and the human's existing comments in one query:

```bash
gh api graphql -f query='query {
  repository(owner:"OWNER", name:"REPO") {
    pullRequest(number:NNN) {
      reviews(last:5, states:PENDING) {
        nodes {
          id state author { login }
          comments(first:20) { nodes { id databaseId path body } }
        }
      }
    }
  }
}'
```

Read the human's comments before writing anything, so you do not restate a point they already made. Append each comment as a thread with `addPullRequestReviewThread`, passing `pullRequestReviewId`, `path`, `line`, `side: RIGHT`, and `body`. One mutation per comment.

Pending-review comments are invisible to REST: `GET repos/{owner}/{repo}/pulls/comments/{comment_id}` returns `404` for one that belongs to an unsubmitted review. Edit with the `updatePullRequestReviewComment` mutation, passing `pullRequestReviewCommentId` and the new `body`. That ID is the GraphQL node `id` (a `PRRC_...` string), not the `databaseId`.

**Never write the review body of someone else's pending review.** The body is theirs, and text you put there publishes under their name. Leave it untouched.

**Attribution goes on each comment you author**, the same as on any inline comment: the prefix on its own line, a blank line, then the comment. Never add the prefix to a comment the human wrote.

**When the body is unavailable**, its content moves as follows. This table outranks any skill rule that says a part never posts separately.

| Body part | Goes to |
|---|---|
| The headline line and the summary paragraph | One more inline comment, anchored to the changed line the other comments are most about. It opens by saying it summarizes the other comments. |
| Unanchored `<details>` blocks | The same summary comment, after the paragraph. |
| A long section, such as the Dev QA section of a standalone Dev QA post | A separate top-level pull-request comment, with the attribution line at its top. Its pointers name comments that stay invisible until the user submits the pending review. So write it to a file, and post it only when the user says the review is submitted and asks for this post. |

Say in the local summary where each part goes, and that the appended comments publish only when the user submits the pending review.

## Red flags

Any of these means stop and re-read this file:

- A body sentence containing "approve", "requesting changes", "LGTM", "before merge", or "ship it".
- A body that opens by praising the change.
- Any sentence describing how the post was produced.
- A comment or section whose first clause names a kind of review rather than a result.
- The words "consider", "might want to", "could be cleaner", "nit:", "worth a ticket" in the posted text.
- A post made without an approval that answers the question you asked.
- An inline comment Claude wrote that does not start with the attribution line, when the project defines one.
- An image uploaded before approval.

## What NOT to do

- Do not submit `APPROVE` or `REQUEST_CHANGES` under any circumstance.
- Do not post without an approval that answers the question you asked.
- Do not append to someone's hand-written pending review without asking.
- Do not write into another person's review body, and do not edit or re-prefix the comments they wrote.
- Do not apply fixes or push commits. Outside reviews read, test, and comment.
- Do not re-post a result the pull request already carries as a comment. Fetch existing comments and match on substance, not location.
