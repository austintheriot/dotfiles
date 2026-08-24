---
name: monitor-ci
description: Watch CircleCI and the `Claude Opt In` AI review for the current branch's PR (Ginger-Labs/Notability) using the `nd ci` CLI from notability-dev-tool. Diagnoses failures, plans fixes, offers to rerun flaky workflows. Uses `nd ci` for all CircleCI interaction (status, jobs, logs, retry, artifacts) and `gh` for PR/review data. Triggers -- "monitor CI", "watch CI", "is the build green", "check my PR build", "wait for CI", "babysit the PR", "is CI passing".
argument-hint: "[PR number or URL -- defaults to PR for current branch]"
---

# Monitor CI + AI Review (via `nd ci`)

Watch CircleCI status (via `nd ci`) and the Claude review bot's comments on a PR (via `gh`) until the build completes and any review feedback is in. Runs two wake mechanisms in tandem: a background `nd ci wait` (precise — the harness notifies the moment CI finishes) plus a ~60s `ScheduleWakeup` safety-net poll, so a dropped notification can never leave the skill hanging.

## Prerequisites

- `nd` (notability-dev-tool) installed and on `PATH`. Verify with `nd ci --help`.
- `nd ci install` has been run once to save a CircleCI personal API token. If `nd ci pipelines` errors with "No CircleCI token set", surface the install instructions and stop.
- `gh` authenticated against `Ginger-Labs/Notability`.

If `nd` is missing, print:
> `nd` (notability-dev-tool) is not on PATH. Install from `~/Documents/code/notability-dev-tool` (run its `install.sh`), then `nd ci install` to save your CircleCI token, then re-invoke this skill.

## Step 1: Resolve PR

If `$ARGUMENTS` is provided, use it as the PR number or URL. Otherwise:

```bash
gh pr view --json number,headRefName,url,labels,isDraft,createdAt
```

Fail cleanly with a clear message if no PR exists for the current branch.

Record:
- **PR number, URL, branch name, draft state, labels**.
- **Monitor start time** (current ISO-8601 timestamp). Used later to filter AI review comments to "posted after we started watching" -- the `Claude Opt In` label gets auto-removed by the workflow after the review runs, so label presence is not a reliable signal on its own.

## Step 2: Single status check

Issue these in parallel — independent commands, single tool-call batch.

### 2a: CircleCI status

```bash
nd ci workflows --latest --branch <headRefName>
```

Parse status strings: terminal = `success` / `failed` / `error` / `canceled`; running = `running`; not-yet = `created` / `pending`; gated = `on_hold`.

`on_hold` is **not** in-flight. A gated workflow is waiting on a human to approve a job and will never advance on its own, so it is terminal-pending for monitoring purposes: report it, never wait it out. Read the **workflow-level** status column, not the adjacent job-activity column -- a gated workflow can print `on_hold` in the status column while the next column still says `running`, so a grep for "running" misclassifies it. The `⏸` glyph marks the gated rows.

For any `failed` / `error` workflow, fetch its failed jobs:

```bash
nd ci jobs --workflow-id <workflow-uuid>
```

Record `{workflow_name, workflow_id, job_name, job_number}` for each failed job.

### 2b: AI review comments

```bash
gh api --paginate "repos/Ginger-Labs/Notability/pulls/<PR>/comments"
gh api --paginate "repos/Ginger-Labs/Notability/pulls/<PR>/reviews"
```

Filter to entries where:
- `user.login == "claude[bot]"` AND `user.type == "Bot"`, AND
- `created_at` (comments) or `submitted_at` (reviews) is after the monitor start time.

### 2c: Decide next state

Evaluate cumulatively — one cycle can hit both a CI failure and new review comments, and both must surface.

1. If **new AI review comments** arrived this cycle → run **Step 4 (review comments)**.
2. If **any workflow is `failed` / `error`** → run **Step 3 (CI failure)**.
3. If **any workflow is `on_hold`** → run **Step 3b (gated workflow)**.
4. If any of the above ran, hand off: the developer needs to act. Step 3, Step 3b, and Step 4 outputs all appear in the same turn if more than one triggered.
5. Otherwise, if **any workflow is `running` / `created` / `pending`** → run **Step 2d (await completion)**.
6. Otherwise (all workflows `success` or `canceled`, no gated workflows, no new review comments, review has run or the PR carries `Claude Opt Out`) → run **Step 5 (final report)**.

A gated workflow does **not** keep the poll loop alive. If `on_hold` workflows are the only non-terminal thing left across everything being monitored, that is terminal for monitoring: report the hold (Step 3b), print the final report (Step 5), and stop scheduling wakeups. Only genuinely in-flight workflows (`running` / `created` / `pending`) route to Step 2d.

### 2d: Await completion (background wait + safety-net poll, in tandem)

Run **two** wake mechanisms at once. They are belt-and-suspenders, not alternatives -- a single dropped harness notification has caused this skill to hang indefinitely, so the poll exists to guarantee that can't happen.

**1. Background `nd ci wait` (primary, precise).** Launch it and let the harness notify you on exit:

```bash
nd ci wait --backend --show-failures --interval 30 2>&1 | tee /tmp/ci-wait-<PR>.log
```

Use the Bash tool's `run_in_background: true`. Choose the filter flag (table below) for what you're waiting on.

**2. Safety-net poll (`ScheduleWakeup`, always alongside).** Immediately after launching the background wait, schedule a self-check ~60s out so you re-run Step 2 on a fixed cadence even if the background notification is late or never arrives:

```
ScheduleWakeup(delaySeconds: 60, reason: "monitor-ci safety-net poll, PR <PR>", prompt: "<the original /monitor-ci invocation, verbatim>")
```

Deliberately sub-300s: the prompt cache stays warm and the whole point is a frequent liveness check. Pass the same `/monitor-ci` input each time so the next firing re-enters this skill and continues monitoring.

**Re-run eagerly once monitoring is active.** The first invocation is standing authorization to keep checking -- do NOT re-ask the user before each subsequent poll. Every time you re-enter Step 2 (woken by the background notification *or* by the safety-net poll), re-read `/tmp/ci-wait-<PR>.log`, re-classify, and surface anything new. If CI is still in flight: confirm the background `nd ci wait` process is still alive and re-launch it if it died (background processes don't always survive across wake paths), then schedule the next ~60s `ScheduleWakeup`. Keep this loop going until you hit a terminal state (Step 5) or the user says stop. **Deduplicate**: if both mechanisms fire close together, one Step-2 pass covers both -- don't double-report the same status or re-show review comments already surfaced this session.

Filter flags for `nd ci wait`:

| Filter | When to use |
|--------|-------------|
| `--backend` | Backend-only PRs; equivalent to filtering on `build_test_conditionally_deploy_notability_backend` |
| `--app` | Web / mobile PR workflows |
| `--workflow <name>` (repeatable) | Specific workflow(s) — use when neither shorthand fits |
| (no filter) | Watch every workflow in the pipeline |

Why background `nd ci wait` rather than a Bash foreground timeout: backend workflows routinely run > 10 minutes (the foreground Bash limit). Backgrounding sidesteps that and lets the user keep talking to you while CI runs, and the harness pings you the moment the process exits — a precise wake-up the foreground path can't give. The safety-net poll then sits on top of that precision purely as the dropped-notification backstop.

Once both are launched, hand back to the user with a short status line ("backend workflow running, waiting in background + polling every ~60s — you can keep working").

**If backgrounding is unavailable** (the user declines, or you're in a context where it's inappropriate), drop the background wait and rely on `ScheduleWakeup` alone as the *only* wake mechanism. In that single-mechanism case, the ~60s safety-net cadence is too aggressive without a precise notification backstop, so use a longer interval matched to what you're waiting on:

| Situation | Delay |
|-----------|-------|
| First check immediately after PR creation | 120s (catch lint / TypeScript failures fast) |
| Mid-build, no result imminent | 600s |
| Long workflow known to take >10min | 1200s |
| Right after a flake rerun | 270s (cache stays warm) |

Never use exactly 300s — the prompt cache has expired but you haven't committed to a long wait.

## Step 3: CI Failure

For each failing workflow:

1. Print workflow name, status, and a one-line job list (`✗ <job_name>  #<job_number>`).
2. For each failed job, fetch the step output **first** (most lint, type-check, and compile failures never write artifacts):

   ```bash
   nd ci logs --job-number <job_number> --failed-only --tail 100
   ```

   Or fan out across every failed job in the workflow:

   ```bash
   nd ci logs --workflow-id <workflow-uuid> --failed-only
   ```

   This pulls the actual stderr/stdout from CircleCI's step-output endpoint — no download, no artifact dance, no web UI hop.

3. If the failure clearly points at a specific test or build artifact, *then* check for artifacts:

   ```bash
   nd ci artifacts --job-number <job_number>
   ```

   Download only if a junit / coverage / trace file would help.

4. Classify each failure:
   - **Real failure** (test assertion, compile error, lint error, type error) → present a fix plan with file paths and the smallest diff that satisfies the rule. Cite the exact error text from `nd ci logs`. Hand off to the developer; do NOT edit files unless explicitly asked (see "Observe vs implement" below).
   - **Infrastructure / flake** (network errors, runner provisioning, timeouts unrelated to the change, known-flaky tests) → offer to rerun:

     ```bash
     nd ci retry --workflow-id <workflow-uuid>
     ```

     `--from-failed` is the default. Ask once before rerunning; do not rerun silently. On approval, run the retry, then resume at Step 2 (record a new monitor start time so we don't re-show old review comments).

## Step 3b: Gated workflow (`on_hold`)

A workflow sitting at `on_hold` is blocked on a manual-approval job. Nothing in CI will move it; the user's attention is the blocker, so say so.

For each `on_hold` workflow:

1. Identify the gate:

   ```bash
   nd ci jobs --workflow-id <workflow-uuid>
   ```

   The gating job is the one whose own status is `on_hold` (`⏸`). Jobs downstream of it read `blocked` (`?`) and are waiting on the approval, not failing. The canonical gate on this repo's deploy-adjacent workflows is `approve_berry_rollback`, with `trigger_berry_rollback_workflow` blocked behind it.

2. Check every other job in the workflow. If they all passed, the report is "only the gate remains"; if any `failed` / `error` jobs are also present, say that too and route them through Step 3 -- do not let the hold mask a real failure.

3. Report the hold explicitly and stop counting that workflow as pending. Name the gating job, the blocked job(s) behind it, and what approving it would trigger as far as the job name reveals (e.g. `approve_berry_rollback` gates `trigger_berry_rollback_workflow`, a rollback deploy). Example:

   > `build_test_conditionally_deploy_notability_backend` is `on_hold`: all jobs passed except the manual gate `approve_berry_rollback` (`⏸`), with `trigger_berry_rollback_workflow` `blocked` behind it. Approving it would kick off the Berry rollback workflow. Your call.

4. **Never approve the gate.** Approval is a human decision, and on a deploy-adjacent job it is outward-facing. This skill has no approve command and must not invent one: if the user wants it approved, they do it in the CircleCI UI, or they explicitly ask you to go find a command for it.

5. If the gated workflow is the only thing left un-terminal across everything being monitored, go to **Step 5 (final report)** and stop the poll loop. Do not schedule another wakeup on a gate.

## Step 4: AI Review Comments

Summarize the new comments: total count and an actionable-vs-question breakdown (heuristic: imperative requests for change → actionable; "why" / "should we" / "what about" / "could we" → question). Present the list with file paths and line numbers from the comment payload.

Offer next steps (e.g. "address these now," "defer," "discuss with reviewer"). Do NOT auto-edit files.

## Step 5: Final Report

Print:

- PR URL, title, draft/ready state
- Workflow summary: green / red / mixed, failed workflow names (if any), reruns used this session
- Gated workflows (`on_hold`), listed separately from green and red: workflow name plus the gating job. "All green except one human gate" is a real and common outcome for a stack, and the report should say exactly that, naming the job (e.g. "green except `approve_berry_rollback`, awaiting your approval in the CircleCI UI").
- AI review status: ran / skipped / N comments addressed
- Suggested next action, e.g.:
  - "Approve or skip the manual gate `approve_berry_rollback` in the CircleCI UI (this skill will not approve it for you)"
  - "Mark ready for review: `gh pr ready <PR>`"
  - "Investigate failed `lint_backend` job: `nd ci logs --job-number <n> --failed-only --tail 100`"
  - "Rerun the flaky workflow: `nd ci retry --workflow-id <uuid>`"

## Important

- **Background wait and safety-net poll run together; neither is a pure fallback.** Background `nd ci wait` gives the precise wake-up; the ~60s `ScheduleWakeup` poll is the liveness backstop that catches a dropped or late notification. Run both whenever CI is in flight. Only when backgrounding is unavailable does `ScheduleWakeup` become the sole mechanism (with the longer interval table, not the 60s cadence). Once the user has invoked the skill, keep re-scheduling the poll each cycle without re-asking — stop only at a terminal state or on the user's say-so.
- **`on_hold` is never waited out and never auto-approved.** A gated workflow needs a human, so it is terminal-pending: report the gate (Step 3b), name it in the final report, and stop polling once it's the only thing left. Approving a manual gate is outside this skill's default the same way editing files is (see "Observe vs implement") -- and unlike a fix, an explicit "approve it" ask has no `nd ci` command behind it, so point the user at the CircleCI UI rather than improvising one.
- **Background processes don't survive session close.** The `nd ci wait` process is tied to the Claude Code session. If the developer exits Claude Code, they must re-invoke `/monitor-ci` to resume — same constraint as the polling path.
- **Observe vs implement.** Default to observe-and-plan: surface the failure with a fix plan, let the developer apply it. The developer can opt into implementation by saying so ("fix it", "apply the fix", "and then continue monitoring") — in auto mode, treat the explicit ask as authorization to edit, build/lint locally, commit, push, and resume the wait.
- **`nd ci` is the only CircleCI surface.** Do not scrape the CircleCI web UI, do not call the CircleCI REST API directly, and do not require the CircleCI MCP. If `nd ci` lacks a capability, surface that to the user rather than working around it.
- **Project constants.** Project slug (inside `nd ci`): `gh/Ginger-Labs/Notability`. AI review bot login: `claude[bot]`. AI review workflow file: `.github/workflows/claude-review.yml`.
- **Useful `nd ci` reference**:
  - `nd ci pipelines [--branch <b>] [--limit <n>]` — recent pipelines for a branch.
  - `nd ci workflows --latest [--branch <b>]` or `--pipeline-id <id>` — workflows in a pipeline.
  - `nd ci wait` — blocking poll with options (`--backend`, `--app`, `--workflow <name>`, `--complete-on-job <name>`, `--interval`, `--timeout`, `--show-failures`). **Run in the background**; the harness notifies on exit.
  - `nd ci jobs --workflow-id <id>` — jobs in a workflow.
  - `nd ci logs --job-number <n> | --workflow-id <id>` with `--step`, `--failed-only`, `--tail` — step output (stderr/stdout) without downloading artifacts. **Prefer this over `nd ci artifacts` for failure diagnosis.**
  - `nd ci retry --workflow-id <id> [--from-failed]` — rerun (defaults to failed only).
  - `nd ci artifacts --job-number <n> | --workflow-id <id> [--download]` — list / download artifact files (junit, coverage, traces). Reach for after `nd ci logs` if a specific file is needed.
