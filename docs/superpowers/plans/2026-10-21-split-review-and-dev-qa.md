# Split Code Review and Dev QA Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `/expert-review-pr` and `/dev-qa` two separate tasks. Neither skill invokes the other. Each one alone drafts a pending review. When both run on one PR head, at the same time or one after the other, their results land in one synthesized pending review. The scheduled review runs both, plus one cross-check round, with no person.

**Architecture:** `~/.claude/rules/outside-pr-review.md` already holds the shared posture. It gains one shared contract: the **contribution file**. When a skill gets `--contribution <path>`, it runs in orchestrated mode: it never uploads, never posts, and never asks to post. It writes one JSON contribution to the path and stops. Without the flag, a skill runs standalone: it writes its contribution to the `draft-review` store, shows the summary, asks once, and on approval runs `draft-review create`. `draft-review` (a new Bun tool in `~/tools/draft-review`) is the only code that synthesizes, uploads, and creates or replaces the pending review. It synthesizes every contribution in the store for that PR and head, so a second skill that finishes later updates the same draft. The embedded mode and every cross-reference between the two skills are deleted. Two narrow modes, `/dev-qa --verify` and `/expert-review-pr --trace`, let an orchestrator send one skill's findings to the other as data.

**Tech Stack:** Markdown skill files, one rules file, and `~/tools/draft-review` (Bun, TypeScript 5.9.3, zod 3.25.76, `bun test`, same layout as `~/tools/vm`). The scheduled-review orchestrator is private (see the private plan below) and calls `draft-review`.

**Spec:** No separate spec. The owner decided on 2026-10-21: "The two should be 2 distinct tasks that share some common skills/postures/workflows", and "the orchestrator owns the post". The private orchestrator plan is `~/.claude/local/scheduled-pr-review/docs/plans/2026-10-21-review-orchestrator.md`.

## Global Constraints

- These three files are public (tracked by `config`). They stay project-neutral: no project names, hosts, or paths. Project bindings stay in `~/.claude/local/dev-qa.md`.
- Commit with `config commit -m "<msg>" -- <home-relative paths>`.
- STE-shaped prose, no em dashes, no emojis. Check each changed file with `python3 ~/.claude/scripts/ste_lint.py <file>`: no new findings in changed lines.
- Every run ends in a pending review that the user submits, never in a submitted review.
- Interactive runs (standalone skills and `/draft-review`) ask one question before `draft-review create`, because that step uploads. `draft-review create` itself never prompts: the question belongs to the interactive skill. An unattended orchestrator calls `create` directly and uploads without asking.
- The submitted event, when the user submits, is still `COMMENT` only. `draft-review` never submits.

## Review Focus

1. **A stale contribution.** The review ran on one head and Dev QA on a newer head after a push. The contribution carries `head`, so an orchestrator can refuse a mixed merge. The schema in Task 1 must require `head`.
2. **A skill that still uploads in orchestrated mode.** An upload is permanent. Task 2 and Task 3 must say plainly that orchestrated mode uploads nothing and lists local paths only.
3. **The user edited the draft before the second skill finished.** `draft-review` replaces only a pending review whose body hash matches what it last wrote. When the user typed above the zone headers, it refuses, writes the new synthesized body to a file, and prints the path. Test in Task 8.
4. **The live check of review candidates.** `/dev-qa` step 6 reproduced panel candidates live, inside the review's run. It moves out of the coupled flow and becomes `/dev-qa --verify <findings.json>` (Task 4), which an orchestrator calls with findings as data. Neither skill names the other. A finding the verify run cannot reach must report `not-reachable`, never `not-reproduced`, because the two mean different things to the synthesis.
5. **A reader who follows an old link.** `/expert-review-pr` "Dev QA against the deployed build" and `/dev-qa` "Embedded hand-back" disappear. Search every file under `~/.claude` for the old headings and the word `embedded` and fix each hit (Task 6).

---

### Task 1: The contribution contract in the posture file

**Files:**
- Modify: `~/.claude/rules/outside-pr-review.md`

**Interfaces:**
- Produces the contribution schema, written into the file as a JSON example plus a field table:

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

A `dev-qa` contribution has the same fields plus `"platform": "<free label, optional>"`, `"status": "passed" | "failed" | "incomplete" | "not-run"`, and `"reason"` (required when the status is not `passed`). Its `body` is the Dev QA section only.

Both kinds also carry `findings`, so an orchestrator can route single findings to the other skill:

```json
"findings": [{"id": "R3", "kind": "code" | "behavior", "title": "<one line>", "body": "<markdown>",
              "anchor": {"path": "src/a.ts", "line": 42, "side": "RIGHT"},
              "behavior": "<the user-visible symptom, when there is one>", "evidence": ["/abs/local/path.mp4"]}]
```

`id` is unique within the contribution (`R<n>` for review, `Q<n>` for Dev QA). Every inline comment in `comments` names the finding id it came from in a `finding` field. A review finding with no user-visible effect has no `behavior`, and an orchestrator does not send it to Dev QA.

Two round-two contributions answer a list of findings from the other skill:

- `kind: "dev-qa-verify"`: `verdicts: [{id, result: "reproduced" | "not-reproduced" | "not-reachable", evidence: [...], note}]`.
- `kind: "code-trace"`: `verdicts: [{id, result: "root-cause" | "no-code-cause" | "inconclusive", anchor?, body, duplicateOf?}]`. `duplicateOf` names a review finding id that already covers the same defect.

- [ ] **Step 1: Add** a section `## Orchestrated mode` with these rules: `--contribution <path>` selects the mode. The skill writes the file once, with the Write tool, as its last action. It never uploads, never creates or edits a review or comment, and never asks to post. `body` and `comments` use local paths, and `uploads` lists every local path they cite, in citation order. No attribution line and no zone headers, because the orchestrator adds them. The orchestrator owns the summary, the approval, the uploads, the merge, and the post.
- [ ] **Step 2: Replace** the "When two skills contribute to one post" paragraph (line 74 today) with the `draft-review` rule: every run, standalone or orchestrated, ends in one pending review per PR head, built by `draft-review` from every contribution for that head. A standalone run asks one question before `draft-review create`, because that step uploads.
- [ ] **Step 3: Remove** the "A long section (a Dev QA section)" delivery row only if nothing standalone still needs it. Read the delivery table first: if a standalone `/dev-qa` post can still hit the pending-review case, keep the row and drop the words that assume a code review beside it.
- [ ] **Step 4: Run** the STE lint on the file. Expected: no new findings in the changed lines.
- [ ] **Step 5: Commit** `config commit -m "outside-pr-review: add the contribution contract for orchestrated runs" -- .claude/rules/outside-pr-review.md`

### Task 2: `/expert-review-pr` without Dev QA

**Files:**
- Modify: `~/.claude/skills/expert-review-pr/SKILL.md`

- [ ] **Step 1: Delete** the last sentence of the `description` ("When asked, also dev-QAs the PR..."), the section "Dev QA against the deployed build" (lines 223 to 245 today), body-shape item 4, posting-order item 4, the red flag "A Dev QA result posted as its own comment...", and the Dev QA line in the references.
- [ ] **Step 2: Fix** line 54: the "dev QA confirmer" example goes. Keep the rule with a neutral example.
- [ ] **Step 3: Add** a short "Orchestrated mode" paragraph: with `--contribution <path>`, follow the posture file's orchestrated mode and write a `code-review` contribution instead of the summary, question, and post.
- [ ] **Step 4: Run** `grep -n -i 'dev qa\|dev-qa\|embedded' ~/.claude/skills/expert-review-pr/SKILL.md`. Expected: no hits.
- [ ] **Step 5: Run** the STE lint. Expected: no new findings in the changed lines.
- [ ] **Step 6: Commit** `config commit -m "expert-review-pr: stop running Dev QA, add orchestrated mode" -- .claude/skills/expert-review-pr/SKILL.md`

### Task 3: `/dev-qa` without the review

**Files:**
- Modify: `~/.claude/skills/dev-qa/SKILL.md`

- [ ] **Step 1: Rewrite** `## Modes`: standalone (unchanged) and orchestrated (`--contribution <path>`, per the posture file). Delete the embedded mode and every "caller" that means `/expert-review-pr`. In orchestrated mode the user is not reachable: a needed hand-off goes into `handback`, and the run waits for every confirmer verdict before it writes the file.
- [ ] **Step 2: Delete** step 6 "Live check of panel candidates" from the main process, the `--steps-only` flag, and the panel-candidate line of the hand-back. Task 4 brings the live check back as its own mode. Keep the "live-reproduced finding gets one pointer line" rule in the section shape, worded for any orchestrator.
- [ ] **Step 3: Replace** `## Embedded hand-back` with `## Contribution`: map each old hand-back item to a contribution field. The section goes to `body`. Videos and screenshots go to `uploads`. Untested-step items go to `comments`, as the inline comments standalone mode posts. Undrivable steps, fallbacks, and unconfirmed verdicts go to `handback`. The rollup sets `status`.
- [ ] **Step 4: Fix** the description: "Also used by /expert-review-pr when a code review must include dev QA" becomes "Also runs under an orchestrator that merges its contribution with other work."
- [ ] **Step 5: Run** `grep -n -i 'embedded\|expert-review-pr\|panel candidate\|steps-only' ~/.claude/skills/dev-qa/SKILL.md`. Expected: no hits.
- [ ] **Step 6: Run** the STE lint. Expected: no new findings in the changed lines.
- [ ] **Step 7: Commit** `config commit -m "dev-qa: stop depending on the code review, add orchestrated mode" -- .claude/skills/dev-qa/SKILL.md`

### Task 4: `/dev-qa --verify`

**Files:**
- Modify: `~/.claude/skills/dev-qa/SKILL.md`

- [ ] **Step 1: Add** a section `## Verify mode`. Input: `--verify <findings.json> --contribution <path>`, where the file holds `findings` from any source. The run does the scope, target proof, and session proof steps, then tries to reproduce each finding's `behavior` on the deployed build with the same drive, capture, and confirm rules as a Test Steps step. It runs no Test Steps. It writes a `dev-qa-verify` contribution.
- [ ] **Step 2: Define** the three results: `reproduced` (the confirm stage saw the symptom, with evidence), `not-reproduced` (the run reached the state the finding names and the symptom did not appear), and `not-reachable` (the run could not reach that state, with the reason).
- [ ] **Step 3: Run** the STE lint. Expected: no new findings in the changed lines.
- [ ] **Step 4: Commit** `config commit -m "dev-qa: add verify mode for findings from another source" -- .claude/skills/dev-qa/SKILL.md`

### Task 5: `/expert-review-pr --trace`

**Files:**
- Modify: `~/.claude/skills/expert-review-pr/SKILL.md`

- [ ] **Step 1: Add** a section `## Trace mode`. Input: `--trace <findings.json> --contribution <path>`, with behavior findings from any source (a failed step, an observed bug). For each one, the run finds the code in the diff (or the code the diff calls) that causes the behavior, and holds the result to the skill's usual bar: a named defect, a concrete trigger, and a real consequence. It dispatches at most the panel agents whose lens fits the behavior, not the full panel. It writes a `code-trace` contribution.
- [ ] **Step 2: Define** the results: `root-cause` (with an anchor on a changed line, or on the nearest changed line that calls the cause, and a comment body in the skill's inline shape), `no-code-cause` (the behavior comes from config, data, the environment, or is expected), and `inconclusive`. Set `duplicateOf` when a finding id from the round-one review list (passed with `--known <findings.json>`) covers the same defect.
- [ ] **Step 3: Run** the STE lint. Expected: no new findings in the changed lines.
- [ ] **Step 4: Commit** `config commit -m "expert-review-pr: add trace mode for observed behavior" -- .claude/skills/expert-review-pr/SKILL.md`

### Task 6: Remove stale references

**Files:**
- Modify: every public file the search finds (`~/.claude/skills/dev-qa-video/SKILL.md` is one candidate). Private hits (`~/.claude/local/...`) go to the private orchestrator plan, not here.

- [ ] **Step 1: Run** `grep -rn -i 'embedded mode\|Embedded hand-back\|Dev QA against the deployed build\|steps-only\|panel candidate' ~/.claude/skills ~/.claude/rules ~/.claude/agents`. Expected after the fix: no hits.
- [ ] **Step 2: Fix** each public hit. List each private hit in the summary for the private plan.
- [ ] **Step 3: Commit** `config commit -m "skills: drop references to the embedded Dev QA mode" -- <each changed path>`

### Task 7: `draft-review` core

**Files:**
- Create: `~/tools/draft-review/package.json` (scripts `check` and `test`, as in `~/tools/vm`), `tsconfig.json`, `src/contribution.ts`, `src/synthesize.ts`, and a `.test.ts` beside each

**Interfaces:**
- Produces:
  - `src/contribution.ts`: zod schemas for the four kinds in Task 1 (`code-review`, `dev-qa`, `dev-qa-verify`, `code-trace`), `parseContribution(text: string): Parsed<Contribution>`, and `notRun(input: {kind: 'code-review' | 'dev-qa'; pr: number; head: string; platform?: string; reason: string}): Contribution`.
  - `src/synthesize.ts`: `routeFindings(contributions: Contribution[]): {toVerify: Finding[]; toTrace: Finding[]}` and `synthesize(contributions: Contribution[], attribution: string | undefined): {ok: true; payload: ReviewPayload} | {ok: false; reason: string}`, with `ReviewPayload = {body: string; comments: InlineComment[]; uploads: string[]; handback: string[]}`.
- `synthesize` body order: the attribution line, the zone headers from the posture file, the code-review body, then the Dev QA part. Several Dev QA contributions each get a `### <platform>` heading. A `not-run` contribution becomes one line, `Dev QA (<platform>): not run. <reason>`, or `Code review: not run. <reason>`. It applies the cross-check table:

| Round one | Round two | Result |
|---|---|---|
| Review finding | `reproduced` | Keep the comment. Add "Reproduced on the deployed build (<platforms>)" with the evidence. The Dev QA section gets one pointer line |
| Review finding | `not-reproduced` or `not-reachable` | Keep the comment unchanged. Hand-back only |
| Dev QA finding | `root-cause` | New inline comment at the trace anchor. The Dev QA line points to it |
| Dev QA finding | `root-cause` with `duplicateOf` | No new comment. The review comment gets the Dev QA evidence line |
| Dev QA finding | `no-code-cause` or `inconclusive` | Unchanged |
| Any | no round-two contribution | Unchanged, no note (a standalone or two-skill run has no round two by design) |

- [ ] **Step 1: Write failing tests:** parse each kind, and reject a missing `head`, an unknown `kind`, and a `dev-qa` with no `status`. Synthesize: review only, Dev QA only, review plus Dev QA, two Dev QA platforms, a `not-run` part, one test per table row, a verdict for an unknown id (refused, naming the id), mixed heads or PRs (refused, naming both), a file cited twice (one upload), and ids from two platforms kept apart (`Q1@macOS`, `Q1@Windows`). `routeFindings` skips review findings with no `behavior`.
- [ ] **Step 2: Run** `bun run test`. Expected: FAIL, modules not found.
- [ ] **Step 3: Implement.** Pure code: no `node:fs`, no `Bun.spawn`, no clock.
- [ ] **Step 4: Run** `bun run check && bun run test`. Expected: PASS.
- [ ] **Step 5: Commit** `config commit -m "draft-review: contribution schema and synthesis" -- tools/draft-review`

### Task 8: `draft-review` shell

**Files:**
- Create: `~/tools/draft-review/bin/draft-review`, `src/main.ts`, `src/store.ts`, `src/github.ts`, `src/store.test.ts`, `README.md`

**Interfaces:**
- Consumes: Task 7.
- Produces the commands:
  - `draft-review add <contribution.json>`: copies it into the store at `~/.cache/draft-review/<owner>/<repo>/<pr>/<head>/<kind>[-<platform>].json` (temp file plus rename). A second file of the same kind and platform replaces the first.
  - `draft-review route --pr <n> --head <sha> --out <dir>`: writes `to-verify.json` and `to-trace.json` from the store.
  - `draft-review show --pr <n> --head <sha>`: prints the synthesized body and every inline comment, with local paths, for the one approval question.
  - `draft-review create --pr <n> --head <sha>`: under a per-PR lock, synthesizes the store, uploads each cited file not yet in `uploads.json` (the ledger, updated after each upload), puts the URLs in, and creates the pending review. When a pending review that `draft-review` created exists and its body hash matches `draft.json`, it deletes that review and creates the new one. Any other pending review by the user: exit 3, write the body to `<store>/synthesized.md`, and print the path. Prints the pending review URL. Never submits.
- Project bindings come from `~/.claude/local/draft-review.json` (upload endpoint, repository id, attribution per repository), parsed with zod. With no file, `create` exits 78 and names the file.
- `<owner>/<repo>` comes from `--repo` or `gh repo view` in the current directory.

- [ ] **Step 1: Write failing tests** in `store.test.ts` with a temp store root: `add` twice for one kind keeps one file. Two `add` calls from two processes at once both land. The hash rule: matching hash gives `replace`, a changed body gives `refuse`, no prior draft gives `create` (test the pure decision `draftAction(existing, lastWritten)`). The ledger keeps uploaded URLs across a failed run (simulate with a fake uploader function passed in: this is the one seam, because the core must ask for each URL mid-decision).
- [ ] **Step 2: Run** `bun run test`. Expected: FAIL.
- [ ] **Step 3: Implement.** The lock is `flock` through `bun:ffi`, as in `~/tools/vm` admission.
- [ ] **Step 4: Run** once against a real open PR with a one-line `code-review` contribution and no files. Expected: a pending review URL. Run `create` again: the old pending review is replaced. Edit its body on GitHub, run `create` again: exit 3 and a file path. Delete the pending review with `gh api -X DELETE repos/<owner>/<repo>/pulls/<n>/reviews/<id>` and check that it is gone.
- [ ] **Step 5: Run** `bun run check && bun run test`. Expected: PASS.
- [ ] **Step 6: Commit** `config commit -m "draft-review: store, uploads, and the one pending review" -- tools/draft-review`

### Task 9: Standalone runs end through `draft-review`

**Files:**
- Modify: `~/.claude/rules/outside-pr-review.md`, `~/.claude/skills/dev-qa/SKILL.md`, `~/.claude/skills/expert-review-pr/SKILL.md`

- [ ] **Step 1: Update** the posture file's Posting section: a standalone run writes its contribution, runs `draft-review add`, prints `draft-review show`, asks one question, and on approval runs `draft-review create`. The GitHub create, append, and upload mechanics move out of the posture file into the `draft-review` README, and the posture file points there.
- [ ] **Step 2: Update** both skills' report steps to that sequence. State the result for the user: running the other skill on the same head later updates the same draft.
- [ ] **Step 3: Run** `/dev-qa` and `/expert-review-pr` on one open PR at the same time, in two sessions. Expected: one pending review with both parts. Delete it after the check.
- [ ] **Step 4: Run** the STE lint on the three files. Expected: no new findings in the changed lines.
- [ ] **Step 5: Commit** `config commit -m "outside-pr-review: every run drafts through draft-review" -- .claude/rules/outside-pr-review.md .claude/skills/dev-qa/SKILL.md .claude/skills/expert-review-pr/SKILL.md`

### Task 10: The `/draft-review` skill

**Files:**
- Create: `~/.claude/skills/draft-review/SKILL.md`
- Modify: `~/.claude/rules/outside-pr-review.md` (one line naming the skill as the interactive orchestrator)

**Interfaces:**
- Consumes: `/expert-review-pr --contribution`, `/dev-qa --contribution`, `--verify`, `--trace` (Tasks 1 to 5), and the `draft-review` commands (Task 8).
- Produces: `/draft-review <PR> [--no-cross-check]`. The description says when to use it: a code review and Dev QA on one PR, combined into one pending review.

- [ ] **Step 1: Write** the skill with this process, and nothing project-specific:
  1. Resolve the PR, head, and base per the posture file. Stop when the user already has a pending review on the PR that `draft-review` did not create, and say so.
  2. Round one: dispatch two background agents in one message. One invokes `/expert-review-pr <PR> --contribution <dir>/review.json`. The other invokes `/dev-qa <PR> --contribution <dir>/dev-qa.json`. Each returns only the path it wrote. Read the project file (`~/.claude/local/dev-qa.md` for Dev QA) for any agent model or environment rule.
  3. Run `draft-review add` on each file. A missing file becomes a `not-run` contribution with the reason.
  4. Round two, unless `--no-cross-check`: `draft-review route`. For a non-empty list, dispatch `/dev-qa --verify` and `/expert-review-pr --trace --known <dir>/review.json` as two background agents in one message, then `draft-review add` their files.
  5. Print `draft-review show`, ask one question, and wait. On approval, run `draft-review create`.
  6. Report the pending review URL, and each cross-check result that changed the draft.
- [ ] **Step 2: Add** red flags: running the two skills in the main session one after the other (they must run in parallel agents), posting or uploading from an agent, and a second question after approval.
- [ ] **Step 3: Run** `/draft-review` on one open PR. Expected: four contribution files at most, one question, and one pending review with both parts and any cross-check lines. Delete the pending review after the check.
- [ ] **Step 4: Run** the STE lint on both files. Expected: no new findings in the changed lines.
- [ ] **Step 5: Commit** `config commit -m "draft-review: interactive skill that runs review and Dev QA together" -- .claude/skills/draft-review/SKILL.md .claude/rules/outside-pr-review.md`
