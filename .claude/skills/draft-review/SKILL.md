---
name: draft-review
description: Use when one pull request needs a code review and Dev QA on its deployed build, combined into one pending GitHub review. Runs /expert-review-pr and /dev-qa in parallel agents, and sends each finding to the other skill for a cross-check. Shows the merged review, asks one question, and on approval creates the pending review through draft-review. The user submits the review. Not for one skill alone. Run /expert-review-pr or /dev-qa for that.
argument-hint: "<PR number | PR URL> [--no-cross-check]"
---

# Draft review

Run a code review and Dev QA on one pull request, cross-check the two results, and create one pending review that holds both parts.

Three things share the work:

| Part | Owns |
|---|---|
| This skill | The order of the steps, the agents, the one question, and the report. |
| `/expert-review-pr` and `/dev-qa` | The analysis, the testing, and the text of each part. Each one writes a contribution file and stops. |
| `draft-review` | The merge, the uploads, and the pending review. Its README is `~/tools/draft-review/README.md`. |

**Read `~/.claude/rules/outside-pr-review.md` first.** It holds the two rules that override everything else, scope resolution, the contribution contract, and the cross-check table. **Read `~/.claude/local/dev-qa.md`** for any agent model rule or environment rule. When that file does not exist, the Dev QA agent writes a `not-run` contribution, and the run continues.

## Rules for this skill

1. **This skill never publishes.** It never submits a review, and it never sets a review event. It never re-requests a review, and it never changes the state of the pull request. The user reads the pending review on GitHub and submits it.
2. **One approval covers one `draft-review create`.** Ask one question. Run `create` only after an answer that approves it. When a later step needs another `create`, show the review again and ask again.
3. **Agents never publish.** An agent writes one contribution file and returns its path. An agent never runs `draft-review add` or `draft-review create`. It never uploads, and it never creates or edits a review or a comment.
4. **The two skills run in parallel agents.** Never run them in the main session one after the other.

## Process

### 1. Scope

Resolve the pull request, its head, and its base, as the posture file says. The argument is a number or a pull-request URL. When the argument is absent, use the pull request of the checked-out branch.

```bash
gh pr view <PR> --json number,url,author,headRefName,headRefOid,baseRefName,title,body
```

Keep the full 40-character head SHA. Every `draft-review` command uses it. When the current folder is not a checkout of the repository, add `--repo <owner/repo>` to each `draft-review` command.

When the pull request author is the signed-in user, print this line and continue:

> This pull request is authored by you. `/expert-review` is the skill built for your own code. Continuing.

Find the pending reviews of the signed-in user on the pull request:

```bash
gh api "repos/<owner>/<repo>/pulls/<n>/reviews" --paginate \
  --jq '.[] | select(.state == "PENDING" and .user.login == "<login>") | .id'
```

Read `reviewId` from `~/.cache/draft-review/<owner>/<repo>/<n>/draft.json`. When a pending review exists, and its id is not that `reviewId`, stop. The file can be absent. Then every pending review counts as one that `draft-review` did not create. Tell the user that a pending review by them is on the pull request, and that `draft-review` did not create it. Name its id. Tell the user to submit it or delete it, and then to run `/draft-review` again. Do not start the agents.

Clear the stored contributions of any earlier run on this head:

```bash
draft-review clear --pr <n> --head <head>
```

The command clears local store files only. It publishes nothing, so it needs no question. It keeps the upload ledger and `draft.json`. The command prints the name of each file that it cleared. When it prints names, tell the user in one line which old files it cleared. Without this step, an old round-two verdict can attach to a new finding that has the same id.

Make a new folder for this run in the session scratchpad: `<scratchpad>/draft-review/<n>-<first 12 characters of the head>/`. The steps below call this folder `<dir>`.

### 2. Round one

Dispatch two agents in one message, so that they run at the same time. Use the `general-purpose` agent type. When the project file names a model for an agent, pass it as `model`. Otherwise omit `model`.

| Agent | Skill call | Writes |
|---|---|---|
| Code review | `/expert-review-pr <n> --contribution <dir>/review.json` | `<dir>/review.json` |
| Dev QA | `/dev-qa <n> --contribution <dir>/dev-qa.json` | `<dir>/dev-qa.json` |

Each prompt says:

- Invoke the Skill tool with the skill name and the arguments in the table. Follow that skill in orchestrated mode.
- The user is not reachable. Do not ask a question. Put each item that needs the user in `handback`.
- Do not run `draft-review`. Do not upload a file. Do not create, edit, or submit a review or a comment on GitHub.
- Reply with only the path of the file you wrote. When you wrote no file, reply with `none:` and the reason.

Wait for both agents. Do not poll them. Do not start step 3 before both agents return.

### 3. Add the round-one files

For each of the two paths, do one of these:

- **The file exists.** Run `draft-review add <path>`.
- **The file does not exist.** Write a `not-run` contribution at that path in one call of the Write tool. Then run `draft-review add <path>`.

A `not-run` code review:

```json
{"version": 1, "kind": "code-review", "pr": <n>, "head": "<head>", "status": "not-run", "reason": "<reason>",
 "body": "Not run.", "comments": [], "findings": [], "uploads": [], "handback": ["<what the agent replied>"]}
```

A `not-run` Dev QA contribution has `"kind": "dev-qa"` and the same other fields.

`draft-review` posts the `reason` in the review body. Write the reason as a plain fact about the pull request or the build. An example: "The deployed build for this head was not found." Never name an agent, a skill, or a tool in the reason. Put the agent's reply in `handback`, because `handback` is never posted.

When `add` exits with a code other than 0 for an agent's file, keep that file. Write the `not-run` contribution to a new path in `<dir>`, and run `add` on it. Put the `add` error and the path of the kept file in `handback`. Report both to the user in step 6.

Each contribution names a `head`. When a `head` is not the head from step 1, the pull request changed during the run. Stop, and tell the user. Do not run `show` or `create`.

### 4. Round two

When the invocation has `--no-cross-check`, skip this step.

Write the routing lists:

```bash
draft-review route --pr <n> --head <head> --out <dir>
```

The command writes `<dir>/to-verify.json` and `<dir>/to-trace.json`. Each file holds `{"findings": [...]}`. Dispatch one agent for each file whose `findings` array is not empty. When both lists are not empty, dispatch both agents in one message.

| List | Skill call | Writes |
|---|---|---|
| `to-verify.json` | `/dev-qa <n> --verify <dir>/to-verify.json --contribution <dir>/dev-qa-verify.json` | `<dir>/dev-qa-verify.json` |
| `to-trace.json` | `/expert-review-pr <n> --trace <dir>/to-trace.json --known <dir>/review.json --contribution <dir>/code-trace.json` | `<dir>/code-trace.json` |

Use the same agent type, model rule, and prompt as in step 2. Wait for every agent that you dispatched. Then check each round-two file that exists:

1. Read its `head`. When the `head` is not the head from step 1, do not add the file.
2. Otherwise, run `draft-review add` on it.

A round-two file can be absent, name another head, or fail `add`. An `add` that exits 65 because the set breaks a merge rule stores nothing. In each case, that cross-check did not run. The contract has no `not-run` status for a round-two kind, so do not write a replacement. Step 1 cleared the store for this head. So a round-two contribution that is not in the store leaves the draft unchanged. Report the cross-check that did not run to the user in step 6.

### 5. Show, ask, create

Run `draft-review show --pr <n> --head <head>`. Print the whole output in chat.

The user's instructions or the repository's `CLAUDE.md` can define an attribution line. When one does, and the `show` output has no attribution line, stop. Tell the user to set `attribution` in the `draft-review` config.

Ask one question: "Create the pending review?" Wait for the answer.

- **The answer approves.** Run `draft-review create --pr <n> --head <head>`.
- **The answer declines, or asks for a change.** Do not run `create`. The store keeps the contributions for this head. A changed contribution needs `add`, then `show`, then the question again.

### 6. Report

On exit code 0, `create` prints the URL of the pending review. Report these items:

- The URL. Tell the user to read the review on GitHub and submit it.
- Each cross-check result that changed the draft:
  - A `reproduced` verdict. The review comment for that finding now says that the deployed build shows the defect.
  - A `root-cause` verdict. A new inline comment sits at the trace anchor.
  - A `root-cause` verdict with `duplicateOf`. The review comment for that finding now holds the Dev QA evidence.
- Each `not-run` part, with its reason.
- Each cross-check that did not run: a round-two file that was absent, named another head, or failed `add`.

Report verdicts only from the round-two files that `draft-review add` accepted for the head from step 1. A file that `add` did not accept is not in the draft, so its verdicts changed nothing. A `not-reproduced`, `not-reachable`, `no-code-cause`, or `inconclusive` verdict does not change the draft. The `show` output already listed those as hand-back lines.

## Exit codes of `create`

When `create` exits with a code other than 0, tell the user what happened and what to do. The README exit code table is the full list. A review URL on stdout with code 3 or 69 means that a pending review exists with no record.

| Code | What happened | What to tell the user |
|---|---|---|
| 3, no URL on stdout | `create` changed nothing on GitHub. Another pending review exists, someone edited the draft, the user submitted the first draft for this head, or a submitted review already holds this content. | Print the message from stderr. Give the path of `synthesized.md`, which holds the merged review. Do not touch the other review. To add new findings to a submitted review, the user copies them from `synthesized.md` by hand. |
| 3, URL on stdout | A new pending review exists, and `draft-review` kept no record of it. Its comments did not match the request. | Give the URL. Tell the user to check the review on GitHub and to delete it before the next `create`. |
| 64 | The command line was wrong. | Print the message. The command in this skill is wrong. Do not run `create` again. |
| 65 | A contribution is invalid, the contributions break a merge rule, an inline comment is outside the pull-request diff, or `draft.json` is invalid. `create` uploaded nothing and deleted nothing. | Print the message. For an invalid `draft.json`, the message names the file and the pending review. Give the user the steps that the message names. For a comment outside the diff, the fix is a changed contribution: `add`, then `show`, then the question again. |
| 66 | A cited file is missing or changed, two contributions recorded different content for one file, or the store holds nothing for the head. | Print the message. When a file changed after `add`, run `add` again on its contribution. Then run `show` again, and ask the question again. |
| 69, no URL on stdout | `gh` is missing or not signed in, or a GitHub call or an upload failed. | Print the message. When the message says that the old draft was deleted, the old draft is gone. Running `create` again needs a new approval. |
| 69, URL on stdout | A new pending review exists, and `draft-review` kept no record of it. The read-back of its comments failed. | Give the URL. Tell the user to check the review on GitHub and to delete it before the next `create`. |
| 73 | Another run holds the lock for this pull request. | Tell the user. Do not run `create` again without a new approval. |
| 74 | A store file cannot be read or written. | Print the message, which names the file. |
| 75 | `create` made the pending review, but a contribution for the head changed while it ran. The draft does not hold that change. | Run `show` again, and ask the question again. On approval, run `create` again. It replaces the stale draft. |
| 78 | The config file is missing or invalid, it has no entry for the repository, or an upload needs an `uploadEndpoint` that the entry does not set. | Tell the user which cause the message names. The config file is `~/.claude/local/draft-review.json`. |

## Red flags

Any of these means stop and re-read the rule it breaks:

- The two skills run in the main session, one after the other. They must run in parallel agents.
- An agent posts, uploads, or runs `draft-review add` or `draft-review create`.
- A second question after the approval, before `create` runs.
- A `create` run before the answer to the question.
- A second `create` on one approval.
- Any submit, review event, review re-request, or change to the pull request state.
- A `not-run` reason that names an agent, a skill, or a tool.
- A pending review by the user that `draft-review` did not create, and the agents start anyway.
- A contribution for a head other than the head from step 1, and the run continues to `show`.
- Round one starts before `draft-review clear` runs for the head.
