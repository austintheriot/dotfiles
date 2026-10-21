---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Outside pull-request review: shared posture

The shared contract for any skill that posts Claude's own work onto another person's pull request. `/expert-review-pr` (code findings) and `/dev-qa` (results of testing the deployed build) both read this file. Each skill keeps its own analysis, its own filter, and its own body shape. This file holds what they share: approval, the never-submit rule, scope resolution, voice, the internals rule, the contribution contract, the local summary, and the standalone ending through `draft-review`. The GitHub create, upload, and pending-review mechanics live in the `draft-review` README at `~/tools/draft-review/README.md`.

When a skill and this file disagree, the skill wins for its own artifact.

## Two rules that override everything else

1. **`draft-review create` runs only after the user says to create the review, in a message that answers the question you asked.** Write, add, show, ask, wait. Silence is not approval. A prior approval in this session does not carry to a second run.
2. **A pending review is a draft, and no skill submits it.** Never submit a review. Never set a review event. The user reads the pending review on GitHub and submits it. Only a human decides the approval or the block of a pull request.

A standing authorization written for an unattended run (for example a scheduled-run prompt that says "create a pending review, never submit") replaces rule 1 for that run only, and only in the way it states.

## Scope resolution

No argument: resolve the open pull request for the checked-out branch.

```bash
gh pr view --json number,url,author,headRefName,headRefOid,baseRefName,title,body
```

An argument that is a number or a pull-request URL overrides the branch. If the branch has no open pull request, stop and say so. Do not guess at a nearby one.

For the self-authorship check, resolve the signed-in user one time:

```bash
gh api graphql -f query='query { viewer { login } }' --jq '.data.viewer.login'
```

If the pull request author equals that login, print one line naming the self-review skill for that artifact, and continue.

## Voice

The posted text is Claude's own writing, and reads that way.

- Do not invoke `/write-like-austin`. Do not imitate the user's voice.
- No praise, no flattery, no compliments on the pull request. Open on the result, not on what the pull request does well.
- No softeners: "ignore me", "worth a ticket if you agree", "just a thought", "feel free to disregard", "nice work but".
- Plain literal words. A reader who has not seen the diff follows it without decoding a figure of speech.
- State uncertainty as a fact when it is load-bearing: "I did not verify X."
- Write no attribution line and no zone headers in a contribution. `draft-review` adds both. It puts the zone headers at the top of the body. It puts the `attribution` line from its config at the top of the body and of each inline comment. The `Pre-Claude zone` heading stays empty so the user can write above Claude's text before submitting. Never add the headers to a body someone else wrote.
- When the user's instructions or the repository's `CLAUDE.md` define an attribution line, and the `show` output has none, stop. Tell the user to set `attribution` in the `draft-review` config.

## Internals stay out

The posted text says nothing about how it was produced. No mention of a panel, experts, lenses, agents, subagents, skills, severities, confidence scores, dispatch, synthesis, confirm stages, or plans. No mention that the post is one of several passes, or that a local tool ran.

**This rule is violated most often in an opening clause**, where a scene-setting phrase feels like orientation: "Test coverage pass.", "A second review pass focused on X.", "Automated QA run.", "From the QA agent,". Delete the clause and open on the result.

**Technique is not process.** Naming how you obtained evidence is allowed, and often makes a result credible: "deleting this line leaves the suite green", "signed in as a non-employee account, the page returns 404", "the deployed commit is `abc1234`". Naming how the work was organized is not: passes, rounds, panels, lenses, agents, severities, confidence scores. Evidence in, org chart out.

Before you write the contribution, re-read the first sentence of every comment and section on its own. If it describes the review rather than the code or the build, rewrite it.

## Orchestrated mode

The argument `--contribution <path>` selects orchestrated mode. The skill writes its result as one JSON file at that path. An orchestrator then runs `draft-review add` on each contribution and `draft-review create` for the head.

The interactive orchestrator is the `/draft-review` skill. It runs both skills on one pull request and asks one question before `draft-review create`.

Rules for the skill in this mode:

1. Write the file in one call of the Write tool, as the last action of the run.
2. Never run `draft-review add` or `draft-review create`. Never upload anything. Never create or edit a review or a comment on GitHub. Never ask the user for approval.
3. Use local file paths in `body` and in every comment. List every local path that the text cites in `uploads`, in citation order. Write each of those files before the run ends.
4. Add no attribution line and no zone headers. `draft-review` adds both.
5. When the run used a panel of agents, put one line in `handback`. The line starts with `Models:` and lists each agent with its model, and any rerun. A hand-back line is never posted.

A clean `code-review` or `dev-qa` run writes empty `comments` and empty `findings`, with the clean body in `body`.

The orchestrator owns the summary, the approval, and the two `draft-review` commands. `draft-review create` owns the uploads, the merge, and the pending review.

### Contribution contract

Every contribution is one JSON object. This example is a `code-review` contribution:

```json
{
  "version": 1,
  "kind": "code-review",
  "pr": 1234,
  "head": "<full head SHA>",
  "body": "<top-level body markdown, local file paths in place of URLs>",
  "comments": [{"path": "src/a.ts", "line": 42, "side": "RIGHT", "startLine": 40, "body": "<markdown>", "finding": "R1"}],
  "findings": [{"id": "R1", "kind": "code", "title": "<one line>", "body": "<markdown>", "anchor": {"path": "src/a.ts", "line": 42, "side": "RIGHT"}}],
  "uploads": ["/abs/local/path.png"],
  "handback": ["<one line per item the orchestrator must see>"]
}
```

A `dev-qa` contribution has the same fields plus `"platform": "<free label, optional>"`, `"status": "passed" | "failed" | "incomplete" | "not-run"`, and `"reason"` (required for every status except `passed`). Its `body` is the Dev QA section only. A `not-run` Dev QA contribution has empty `findings` and empty `comments`.

A `code-review` contribution can also carry `"status": "completed" | "not-run"` and `"reason"`. An absent `status` means `completed`. A `not-run` code review has a `reason`, empty `comments`, empty `findings`, and a `body` that says the review did not run.

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

A round-two contribution carries `version`, `kind`, `pr`, `head`, `verdicts`, `uploads`, and `handback`. It has no `body`, `comments`, or `findings`. A `dev-qa-verify` contribution can also carry `platform`, with the same meaning as on `dev-qa`. An orchestrator can receive one `dev-qa-verify` contribution per platform. When any platform reproduced a review finding, the finding counts as reproduced.

#### Rules for every kind

- Omit an optional field to leave it absent. Never write `null`.
- A required array is always present and can be empty.
- A reader rejects unknown keys.
- Every path in any `evidence` array, in a finding or in a verdict, also appears in `uploads`.
- A comment `finding` equals an `id` in `findings` of the same contribution. A finding can have zero comments.

#### Top-level fields

| Field | Type | Required | Meaning |
|---|---|---|---|
| `version` | integer, always `1` | required for all kinds | Schema version. |
| `kind` | `"code-review"`, `"dev-qa"`, `"dev-qa-verify"`, or `"code-trace"` | required for all kinds | Selects the shape of the rest of the object. |
| `pr` | integer | required for all kinds | Pull request number. |
| `head` | string, 40-character hex commit SHA | required for all kinds | Full head SHA that the skill examined. An orchestrator refuses to merge contributions with different heads. |
| `body` | non-empty string | required for `code-review` and `dev-qa` | Top-level body markdown with local paths in place of URLs. For `dev-qa`, the body is the Dev QA section only. |
| `comments` | array of comment | required for `code-review` and `dev-qa` | Inline comments. The array can be empty. |
| `uploads` | array of string | required for all kinds | Absolute local paths that the text cites, in citation order. The array can be empty. |
| `handback` | array of string | required for all kinds | One line per item the orchestrator must see. The array can be empty. |
| `findings` | array of finding | required for `code-review` and `dev-qa` | Findings that an orchestrator can route to the other skill. The array can be empty. |
| `platform` | string | optional for `dev-qa` and `dev-qa-verify` | Free label for the test environment. |
| `status` | `dev-qa`: `"passed"`, `"failed"`, `"incomplete"`, or `"not-run"`. `code-review`: `"completed"` or `"not-run"`. | required for `dev-qa`, optional for `code-review` | Overall result of the run. An absent `code-review` status means `completed`. |
| `reason` | non-empty string | required for `dev-qa` when `status` is not `passed`, required for `code-review` when `status` is `not-run`, otherwise optional | Why the run did not pass or did not run. |
| `verdicts` | array of verdict | required for `dev-qa-verify` and `code-trace` | One verdict per finding that the skill received. The verdict shape depends on `kind`. |

#### Comment fields

| Field | Type | Required | Meaning |
|---|---|---|---|
| `path` | string | required for every comment | File path in the pull request. |
| `line` | integer | required for every comment | Line number on the chosen side. |
| `side` | `"LEFT"` or `"RIGHT"` | required for every comment | Diff side. `RIGHT` is the new file. |
| `startLine` | integer, less than or equal to `line` | optional for every comment | First line of a multi-line comment. `line` is the last line. |
| `body` | non-empty string | required for every comment | Comment markdown. |
| `finding` | string | required for every comment | `id` of the finding that this comment came from. |

#### Finding fields

| Field | Type | Required | Meaning |
|---|---|---|---|
| `id` | string | required for every finding | Unique in the contribution. `R<n>` for review, `Q<n>` for Dev QA. |
| `kind` | `"code"` or `"behavior"` | required for every finding | `code` is a defect in the diff. `behavior` is a symptom seen in the running build. |
| `title` | non-empty string | required for every finding | One line. |
| `body` | non-empty string | required for every finding | Markdown. |
| `anchor` | object | optional for every finding | Location in the diff: `path` (string), `line` (integer), `side` (`"LEFT"` or `"RIGHT"`). |
| `behavior` | non-empty string | required for `behavior`, optional for `code` | The user-visible symptom. Omit it when a `code` finding has no user-visible effect. |
| `evidence` | array of string | required and non-empty for `behavior`, optional for `code` | Absolute local paths of screenshots or videos. |

#### Verdict fields

| Field | Type | Required | Meaning |
|---|---|---|---|
| `id` | string | required for `dev-qa-verify` and `code-trace` | `id` of the finding that the verdict answers. |
| `result` | string | required for `dev-qa-verify` and `code-trace` | `dev-qa-verify`: `"reproduced"`, `"not-reproduced"`, or `"not-reachable"`. `code-trace`: `"root-cause"`, `"no-code-cause"`, or `"inconclusive"`. |
| `evidence` | array of string, can be empty | required for `dev-qa-verify` | Absolute local paths that show the result. A `not-reachable` verdict has nothing to show and uses an empty array. |
| `note` | non-empty string | required for `dev-qa-verify` | One or two sentences on what the run did and saw. |
| `anchor` | object | optional for `code-trace` | Location of the cause. Same shape as a finding `anchor`. |
| `body` | non-empty string | required for `code-trace` | Markdown explanation of the trace. |
| `duplicateOf` | string | optional for `code-trace`, allowed with `root-cause` and `no-code-cause` | Review finding `id` that already covers the same defect. It means this finding has the same cause as that finding. |

#### Ids

- A `code-review` finding `id` is `R` followed by digits, for example `R3`.
- A `dev-qa` finding `id` is `Q` followed by digits, for example `Q1`.
- A `platform` label never contains `@`.
- When a `dev-qa` contribution has a `platform`, the orchestrator qualifies each of its finding ids as `<id>@<platform>`, for example `Q1@macOS`.
- The orchestrator never qualifies a review id.
- A `dev-qa-verify` verdict `id` and a `duplicateOf` are review ids.
- A `code-trace` verdict `id` is a Dev QA id. If its `dev-qa` contribution has a `platform`, the verdict uses the qualified id.

#### Merge rules

The orchestrator refuses to merge a set of contributions that breaks one of these rules:

- Every contribution names the same `pr` and the same `head`.
- The set has at least one `code-review` or `dev-qa` contribution.
- The set has at most one `code-review` contribution and at most one `code-trace` contribution.
- When the set has two or more `dev-qa` contributions, each one has a `platform`, and no two share a platform.
- No two `dev-qa-verify` contributions share a platform. Two that have no platform also collide.
- A `dev-qa-verify` verdict answers only a review finding that has `behavior`.
- A `code-trace` verdict answers only a Dev QA finding of kind `behavior`.
- A `duplicateOf` names a review finding in the set.

#### Cross-check

The orchestrator applies round-two verdicts to round-one findings with this table:

| Round one | Round two | Result |
|---|---|---|
| Review finding | `reproduced` | Keep the comment. Add "Reproduced on the deployed build (<platforms>)" with the evidence. The Dev QA section gets one pointer line. |
| Review finding | `not-reproduced` or `not-reachable` | Keep the comment unchanged. Hand-back only. |
| Dev QA finding | `root-cause` | New inline comment at the trace anchor. The Dev QA line points to it. |
| Dev QA finding | `root-cause` with `duplicateOf` | No new comment. The review comment gets the Dev QA evidence line. |
| Dev QA finding | `no-code-cause` or `inconclusive` | Unchanged. |
| Any | no round-two contribution | Unchanged, no note. A standalone or two-skill run has no round two by design. |

The pointer line for a reproduced finding goes in the Dev QA part that has the same `platform` as the `dev-qa-verify` contribution. When no Dev QA part matches, the line goes before the first platform heading.

#### Hand-back order

The orchestrator lists hand-back lines in this order:

1. The `code-review` lines.
2. The lines of each `dev-qa` contribution, in input order.
3. The lines of each `dev-qa-verify` contribution, in input order.
4. The `code-trace` lines.
5. One line for each `not-reproduced` or `not-reachable` verdict.

## Local summary

Every run, standalone or orchestrated, ends in at most one pending review per PR head. The tool `draft-review` builds that review from every contribution for that head. A standalone run asks one question before `draft-review create`, because that step uploads. An unattended orchestrator runs `draft-review create` directly and asks nothing.

Before the question, print the complete intended review in chat: the output of `draft-review show`. That output holds the whole body and every `<details>` block. It also holds every inline comment with its file, line, and full text, every upload by local path, and the hand-back lines. Local-only items go in `handback`, because `show` prints them and `create` never posts them.

## Attachments

Uploads are permanent. `draft-review create` uploads each file in `uploads`, and only after the user approves. Before approval, the local summary shows local file paths in place of URLs. The `uploadEndpoint` for the repository comes from the `draft-review` config (see Config in its README).

Write every evidence file (screenshot, video) to disk before you run `draft-review add`. If a cited file does not exist, `add` stores nothing and exits with code 66. After the review exists, check that every embedded image resolves.

## Posting

A standalone run ends with this sequence:

1. Write the contribution file to a path in the session scratchpad, in one call of the Write tool.
2. Run `draft-review add <contribution.json>`.
3. Run `draft-review show --pr <n> --head <sha>`, and print the output in chat. The head is the full 40-character SHA.
4. Ask one question: "Create the pending review?" Wait for the answer.
5. On approval, run `draft-review create --pr <n> --head <sha>`. Print the URL that the command prints.

When the current folder is not a checkout of the repository, add `--repo <owner/repo>` to each command. The README describes every flag.

The result for the user is one pending review for that head. The user reads it on GitHub and submits it. While the first draft is still pending, the other skill can run later on the same head. Its `add` and `create` replace that draft with the merged review, so one review holds both parts. When someone edited the draft, `create` refuses to replace it.

When `add` stores a `code-review` or `dev-qa` contribution, it removes the round-two verdicts on the earlier one, so a rerun never carries a stale cross-check.

When `create` exits with a code other than 0, act as follows. The README exit code table has the full list.

- Code 3 with no review URL on stdout: another pending review exists, or someone edited the draft. `create` wrote nothing to GitHub. Tell the user the path of `synthesized.md`, which holds the merged review. Do not touch the other review. When the other review is the user's own hand-written draft, the user can copy text from `synthesized.md` into it. After the user submits or discards it, run `create` again.
- Code 3 with a review URL on stdout: the new pending review exists, and it has no record. The read-back found different comments. Tell the user to check it on GitHub and delete it before you run `create` again.
- Code 65: a contribution is invalid, breaks a merge rule, or has an inline comment outside the diff. `create` uploaded nothing and deleted nothing. Fix the contribution and run `add` again. Then run `show` again, and ask the question again before `create`.
- Code 66: a cited file does not exist or was edited after `add`. Write the file again and run `add` again. Then run `show` again, and ask the question again before `create`.
- Code 75: a contribution changed while `create` ran. Run `create` again.
- Code 69 with a review URL on stdout: the new pending review exists, and it has no record. Tell the user to check it on GitHub and delete it before you run `create` again.
- Code 69 with no review URL: a GitHub call failed. Tell the user the message. If the message says that the review was deleted, run `create` again.
- Code 78: the `draft-review` config does not exist, is not valid, has no entry for this repository, or has no `uploadEndpoint` for an upload. Tell the user which cause the error message names.

### Verify inline anchors

Before you write an inline comment into a contribution, fetch the patch for the file and confirm the line is in the changed range:

```bash
gh api "repos/$REPO/pulls/$PR/files" --jq '.[] | select(.filename=="<path>") | .patch'
```

The hunk headers (`@@ -old,count +new,count @@`) give the valid `RIGHT`-side lines. A comment whose line does not verify moves into the body, described by file path alone. Never write an anchor you just disproved. Before any upload or delete, `draft-review create` checks each anchor against the diff hunks. When an anchor is outside the diff, `create` exits with code 65 and names the comment.

## Red flags

Any of these means stop and re-read this file:

- A body sentence containing "approve", "requesting changes", "LGTM", "before merge", or "ship it".
- A body that opens by praising the change.
- Any sentence describing how the post was produced.
- A comment or section whose first clause names a kind of review rather than a result.
- The words "consider", "might want to", "could be cleaner", "nit:", "worth a ticket" in the posted text.
- A contribution body or comment that carries an attribution line or zone headers.
- A `draft-review create` run before approval, because that run uploads the images.

## What NOT to do

- Do not submit a review. Do not set a review event. Never write `APPROVE` or `REQUEST_CHANGES`.
- Do not run `draft-review create` without an approval that answers the question you asked.
- Do not edit another person's pending review. When one exists, `create` exits with code 3 and writes nothing.
- Do not write into another person's review body, and do not edit the comments they wrote.
- Do not apply fixes or push commits. Outside reviews read, test, and comment.
- Do not re-post a result the pull request already carries as a comment. Fetch existing comments and match on substance, not location.
