---
name: monitor-ci
description: Watch CI and any AI review bot on the current branch's PR until the build completes and review feedback is in. Diagnoses failures, plans fixes, offers to rerun flaky workflows, and never waits out or approves a manual gate. Provider-agnostic -- reads the concrete CI CLI, workflow names, and known gates from a machine-local config file (`~/.claude/local/ci-config.md`), so it works across projects and CI providers. Triggers -- "monitor CI", "watch CI", "is the build green", "check my PR build", "wait for CI", "babysit the PR", "is CI passing".
argument-hint: "[PR number or URL -- defaults to PR for current branch]"
---

# Monitor CI + AI Review

Watch CI status and any AI review bot's comments on a PR until the build completes and review feedback is in. Runs two wake mechanisms in tandem: an event wait (a background command or a `Monitor`, which wakes you the moment something changes) plus a long `ScheduleWakeup` fallback heartbeat, so a dropped notification can never leave the skill hanging.

The monitoring shape below is universal. The project-specific parts — which CLI talks to CI, what the workflows are called, which jobs are manual gates — live in a local config file, not in this skill.

## Step 0: Load local CI configuration

Read `~/.claude/local/ci-config.md` and select the project section whose repo matches the current remote (`gh repo view --json nameWithOwner`).

That file supplies:

- **CI CLI and command map** — the concrete command for each generic step in the table below.
- **Status vocabulary** — which status strings mean terminal / running / not-yet / gated.
- **Wait filters** — shorthand flags for narrowing what to wait on.
- **Known manual gates** — gating job names and what approving each one would trigger.
- **AI review bot** — login, opt-in / opt-out labels, workflow file, and the events that trigger a review.
- **Prerequisites** — how the CLI is installed and how it stores its API token.

**If the file is missing, or has no section for this repo**, do not guess. Say so, name the two or three things you would need (how to query CI status from the terminal, what the workflows are called, which jobs are manual gates), and offer to write a new section. A wrong guess about a deploy-adjacent job is the failure mode worth avoiding here.

**If the config names a CLI that is not installed**, print its documented install steps from the config and stop.

The generic steps this skill needs a command for:

| Generic step | Used in |
|---|---|
| List workflows for a branch | Step 2a |
| List jobs in a workflow | Steps 2a, 3b |
| Fetch failure logs for a job or workflow | Step 3 |
| List / download artifacts | Step 3 |
| Retry failed jobs in a workflow | Step 3 |
| Blocking wait until CI finishes | Step 2d |

## Step 1: Resolve PR

If `$ARGUMENTS` is provided, use it as the PR number or URL. Otherwise:

```bash
gh pr view --json number,headRefName,url,labels,isDraft,createdAt
```

Fail cleanly with a clear message if no PR exists for the current branch.

Record:
- **PR number, URL, branch name, draft state, labels**.
- **Monitor start time** (current ISO-8601 timestamp). Used later to filter AI review comments to "posted after we started watching" -- review opt-in labels are often auto-removed by the workflow after the review runs, so label presence is not a reliable signal on its own.

## Step 2: Single status check

Issue these in parallel — independent commands, single tool-call batch.

### 2a: CI status

Run the config's **list workflows for a branch** command for `<headRefName>`.

Classify each workflow's status using the config's status vocabulary: terminal, running, not-yet-started, or gated.

A gated workflow is **not** in-flight. It is waiting on a human to approve a job and will never advance on its own, so it is terminal-pending for monitoring purposes: report it, never wait it out. Read the **workflow-level** status column, not an adjacent job-activity column -- a gated workflow can print its gated status while the next column still says running, so a naive grep for "running" misclassifies it.

For any failed workflow, run the config's **list jobs** command and record `{workflow_name, workflow_id, job_name, job_number}` for each failed job.

### 2b: AI review comments

```bash
gh api --paginate "repos/<owner>/<repo>/pulls/<PR>/comments"
gh api --paginate "repos/<owner>/<repo>/pulls/<PR>/reviews"
```

Filter to entries where:
- `user.login` matches the config's review-bot login AND `user.type == "Bot"`, AND
- `created_at` (comments) or `submitted_at` (reviews) is after the monitor start time.

Read the config's review trigger events. A review covers the commit that was the head when it was triggered. If the trigger does not fire on a push, commits pushed after that point stay unreviewed. Report which commit each review covered (`commit_id` on the review). When the head has moved past it, say so, and name the config's re-trigger step. Ask before you run that step, because it posts to the PR.

### 2c: Decide next state

Evaluate cumulatively — one cycle can hit both a CI failure and new review comments, and both must surface.

1. If **new AI review comments** arrived this cycle → run **Step 4 (review comments)**.
2. If **any workflow failed** → run **Step 3 (CI failure)**.
3. If **any workflow is gated** → run **Step 3b (gated workflow)**.
4. If any of the above ran, hand off: the developer needs to act. Step 3, Step 3b, and Step 4 outputs all appear in the same turn if more than one triggered.
5. Otherwise, if **any workflow is running / not-yet-started** → run **Step 2d (await completion)**.
6. Otherwise (all workflows terminal, no gated workflows, no new review comments, review has run or the PR carries the opt-out label) → run **Step 5 (final report)**.

A gated workflow does **not** keep the poll loop alive. If gated workflows are the only non-terminal thing left across everything being monitored, that is terminal for monitoring: report the hold (Step 3b), print the final report (Step 5), and stop scheduling wakeups. Only genuinely in-flight workflows route to Step 2d.

### 2d: Await completion (background wait + safety-net poll, in tandem)

Run **two** wake mechanisms at once. They are belt-and-suspenders, not alternatives -- a single dropped harness notification has caused this skill to hang indefinitely, so the poll exists to guarantee that can't happen.

**1. Event wait (primary, precise).** Pick the tool by how many signals you wait on:

- **One signal** (a CI workflow finishing): launch the config's **blocking wait** command with the Bash tool's `run_in_background: true`, applying whichever wait filter matches, and tee it to a log:

  ```bash
  <wait command> 2>&1 | tee /tmp/ci-wait-<PR>.log
  ```

- **Several signals** (CI, the AI review workflow run, new bot reviews or comments): arm one `Monitor` whose script loops, prints one line per change, and exits at the terminal state. Each printed line wakes you. Load `Monitor` with `ToolSearch` if it is deferred.

**Run the loop body once in the foreground before you arm it.** A script error inside a background wait or a Monitor surfaces only after the wait is armed, so it costs a full re-arm cycle. One foreground iteration catches it first.

**The scripts run under the user's login shell, which can be zsh.** In zsh, `status`, `pipestatus`, `argv`, and `path` are special variables. An assignment such as `status=$(...)` kills the script at once with `read-only variable: status`. Use names like `run_state` and `seen_reviews`.

**2. Fallback heartbeat (`ScheduleWakeup`, always alongside).** Immediately after arming the wait, schedule one long fallback so a dropped notification cannot strand the skill:

```
ScheduleWakeup(delaySeconds: 1200, reason: "monitor-ci fallback heartbeat, PR <PR>", prompt: "<the original /monitor-ci invocation, verbatim>")
```

Use 1200 to 1800 seconds. The harness wakes you when a background command exits or a Monitor prints, so a short poll adds turns and catches nothing. Pass the same `/monitor-ci` input each time so the next firing re-enters this skill.

**Do not poll by hand while a wait is armed.** No `sleep N` followed by a read of the output file, and no "quick check" of the Monitor output. The notification is the wake signal. After arming, end the turn with a status line and let the user work. Read the output only when a notification or the heartbeat wakes you.

**Re-run eagerly once monitoring is active.** The first invocation is standing authorization to keep checking -- do NOT re-ask the user before each subsequent cycle. Every time a notification or the heartbeat wakes you, re-enter Step 2: read the log or Monitor output, re-classify, and surface anything new. If CI is still in flight: confirm the wait is still alive, re-arm it if it died or a Monitor expired, then schedule the next heartbeat. Keep this loop going until you hit a terminal state (Step 5) or the user says stop. **Deduplicate**: if both mechanisms fire close together, one Step-2 pass covers both -- don't double-report the same status or re-show review comments already surfaced this session.

Why a background wait rather than a Bash foreground timeout: real workflows routinely run longer than the foreground Bash limit (10 minutes). Backgrounding sidesteps that and lets the user keep talking to you while CI runs, and the harness pings you the moment the process exits -- a precise wake-up the foreground path can't give.

Once both are armed, hand back to the user with a short status line ("backend workflow running, waiting in background with a 20-minute fallback -- you can keep working").

**If backgrounding is unavailable** (the user declines, or you're in a context where it's inappropriate), `ScheduleWakeup` becomes the only wake mechanism. Match the delay to what you're waiting on:

| Situation | Delay |
|-----------|-------|
| First check immediately after PR creation | 120s (catch lint / type-check failures fast) |
| Mid-build, no result imminent | 600s |
| Long workflow known to take >10min | 1200s |
| Right after a flake rerun | 270s |

## Step 3: CI Failure

For each failing workflow:

1. Print workflow name, status, and a one-line job list (`✗ <job_name>  #<job_number>`).
2. For each failed job, fetch the step output **first** (most lint, type-check, and compile failures never write artifacts). Use the config's **failure logs** command, per-job or fanned out across the whole workflow.

   This pulls the actual stderr/stdout from the CI provider — no download, no artifact dance, no web UI hop.

3. If the failure clearly points at a specific test or build artifact, *then* run the config's **artifacts** command. Download only if a junit / coverage / trace file would help.

4. Classify each failure:
   - **Real failure** (test assertion, compile error, lint error, type error) → present a fix plan with file paths and the smallest diff that satisfies the rule. Cite the exact error text from the logs. Hand off to the developer; do NOT edit files unless explicitly asked (see "Observe vs implement" below).
   - **Infrastructure / flake** (network errors, runner provisioning, timeouts unrelated to the change, known-flaky tests) → offer to rerun using the config's **retry** command. Ask once before rerunning; do not rerun silently. On approval, run the retry, then resume at Step 2 (record a new monitor start time so we don't re-show old review comments).

## Step 3b: Gated workflow

A workflow sitting at the gated status is blocked on a manual-approval job. Nothing in CI will move it; the user's attention is the blocker, so say so.

For each gated workflow:

1. Identify the gate by running the config's **list jobs** command. The gating job is the one whose own status is gated; jobs downstream of it read as blocked and are waiting on the approval, not failing. Cross-reference the config's **known manual gates** table for what approving it would trigger.

2. Check every other job in the workflow. If they all passed, the report is "only the gate remains"; if any failed jobs are also present, say that too and route them through Step 3 -- do not let the hold mask a real failure.

3. Report the hold explicitly and stop counting that workflow as pending. Name the gating job, the blocked job(s) behind it, and what approving it would trigger per the config. Example shape:

   > `<workflow>` is gated: all jobs passed except the manual gate `<gate_job>`, with `<blocked_job>` blocked behind it. Approving it would `<effect from config>`. Your call.

4. **Never approve the gate.** Approval is a human decision, and on a deploy-adjacent job it is outward-facing. This skill has no approve command and must not invent one: if the user wants it approved, they do it in the CI provider's UI, or they explicitly ask you to go find a command for it.

5. If the gated workflow is the only thing left un-terminal across everything being monitored, go to **Step 5 (final report)** and stop the poll loop. Do not schedule another wakeup on a gate.

## Step 4: AI Review Comments

Summarize the new comments: total count and an actionable-vs-question breakdown (heuristic: imperative requests for change → actionable; "why" / "should we" / "what about" / "could we" → question). Present the list with file paths and line numbers from the comment payload.

Offer next steps (e.g. "address these now," "defer," "discuss with reviewer"). Do NOT auto-edit files.

## Step 5: Final Report

Print:

- PR URL, title, draft/ready state
- Workflow summary: green / red / mixed, failed workflow names (if any), reruns used this session
- Gated workflows, listed separately from green and red: workflow name plus the gating job. "All green except one human gate" is a real and common outcome for a stack, and the report should say exactly that, naming the job.
- AI review status: ran / skipped / N comments addressed
- Suggested next action, e.g.:
  - "Approve or skip the manual gate `<gate_job>` in the CI UI (this skill will not approve it for you)"
  - "Mark ready for review: `gh pr ready <PR>`"
  - "Investigate failed `<job>` job: `<failure-logs command>`"
  - "Rerun the flaky workflow: `<retry command>`"

## Important

- **Project specifics live in `~/.claude/local/ci-config.md`, never in this file.** This skill is checked into a public dotfiles repo; the config file is machine-local and untracked. Repo names, internal workflow and job names, deploy gates, and tool install paths belong in the config. If you learn a new gate or workflow name while monitoring, offer to add it to the config — do not write it here.
- **The event wait and the fallback heartbeat run together.** The event wait gives the precise wake-up. The 1200-1800s `ScheduleWakeup` heartbeat only catches a dropped notification. Run both whenever CI is in flight, and never poll by hand between them. Only when backgrounding is unavailable does `ScheduleWakeup` become the sole mechanism (with the interval table in Step 2d). Once the user has invoked the skill, keep re-scheduling the heartbeat each cycle without re-asking -- stop only at a terminal state or on the user's say-so.
- **A gated workflow is never waited out and never auto-approved.** It needs a human, so it is terminal-pending: report the gate (Step 3b), name it in the final report, and stop polling once it's the only thing left. Approving a manual gate is outside this skill's default the same way editing files is (see "Observe vs implement") -- and unlike a fix, an explicit "approve it" ask has no command behind it in most configs, so point the user at the CI UI rather than improvising one.
- **Background processes don't survive session close.** The wait process is tied to the Claude Code session. If the developer exits Claude Code, they must re-invoke `/monitor-ci` to resume — same constraint as the polling path.
- **Observe vs implement.** Default to observe-and-plan: surface the failure with a fix plan, let the developer apply it. The developer can opt into implementation by saying so ("fix it", "apply the fix", "and then continue monitoring") — in auto mode, treat the explicit ask as authorization to edit, build/lint locally, commit, push, and resume the wait.
- **The configured CLI is the only CI surface.** Do not scrape the CI provider's web UI, do not call its REST API directly, and do not require a CI MCP server. If the configured CLI lacks a capability, surface that to the user rather than working around it.
