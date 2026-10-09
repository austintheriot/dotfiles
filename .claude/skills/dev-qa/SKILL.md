---
name: dev-qa
description: Use when asked to dev QA, smoke test, or verify a pull request on its deployed build (a per-commit preview URL, a review environment, or a staging deploy). The check runs the pull request's Test Steps in a browser and reports the result on the pull request. Also used by /expert-review-pr when a code review must include dev QA. Not for QA of an undeployed local branch. The repository's own local QA skills cover that.
argument-hint: "[<PR number | PR URL>] [--steps-only] [--no-confirm]"
---

# Dev QA

Test a pull request on the build it actually deployed, with a video of every step and a screenshot of every checked state, and report one verdict per Test Steps step.

Three files share the work:

| File | Owns |
|---|---|
| This skill | The process, the verdict rules, the evidence rules, the section shape. Project-neutral. |
| `~/.claude/local/dev-qa.md` | The project bindings: deployed URLs, deployed-commit proof, accounts, forbidden writes, browser quirks, the video recorder, uploads, and the mapping onto the repository's QA framework. |
| `~/.claude/rules/outside-pr-review.md` | Approval, the `COMMENT`-only event, scope resolution, voice, the internals rule, the local summary, uploads, and GitHub posting. |

**Read `~/.claude/local/dev-qa.md` and `~/.claude/rules/outside-pr-review.md` before you test.** If the project file does not exist, ask the user for the deployed URL and a test account before you test.

## Modes

- **Standalone** (`/dev-qa [PR]`): this skill owns the post. It ends with the local summary, one question, and on approval a `COMMENT` review whose body is the zone headers from `~/.claude/rules/outside-pr-review.md`, then the Dev QA section.
- **Embedded** (invoked by `/expert-review-pr`): this skill never posts and never asks to post. It returns the section and the hand-back items under Embedded hand-back. The caller owns the summary, the question, and the post.

`--steps-only` skips the live check of panel candidates in embedded mode. `--no-confirm` skips the independent confirm stage. Every verdict is then reported as unconfirmed.

## The repository's QA framework

Work inside the repository's QA framework wherever it reaches. The project file maps each role below to a concrete script, schema, or agent. Use the mapped item. Do not re-implement a role the repository already implements, because the repository's version is the one its maintainers keep correct.

| Role | Used for |
|---|---|
| Steps extractor | Read the pull request's Test Steps. It owns the heading grammar. Do not parse the body by hand when it exists. |
| Scenario record | One record per Test Steps section, in the repository's scenario schema. |
| Evidence log | An append-only log of actions, step checkpoints, and the preliminary verdict, written through the repository's logging script. |
| Trace builder and validator | Build the trace from the log, and validate every record at the seam. |
| Independent confirmer | An agent that judges the verdict from the evidence alone and never drives the app. |
| Rig rules | The platform's known blocks, their remediations, and what is out of rig scope. |
| Oracle source | The documentation that says how the feature is meant to behave. |
| Driving recipes | The platform skill: selectors, state reads, sign-in helpers, known input quirks. |

When the project file maps no framework, use the fallbacks: a `results.md` plus screenshots per section, and a fresh `general-purpose` subagent as the confirmer with the confirmer rules below. A session can deny a mapped command or agent, for example in an allowlisted unattended run. In that case, use the fallback for that role only. Name each role that fell back in the hand-back or the local summary.

**The repository's run orchestrator is a different tool.** A local QA runner builds the branch and drives a simulator or a local dev server. It tests the branch, not the deployed build. Do not hand a deployed-build check to it. Some Test Steps need a platform this skill cannot drive, such as a native app or a desktop shell. Report each such step as `SKIP` with the reason. Name the repository command that runs it in the hand-back.

## Verdicts

Use the repository's verdict vocabulary. The default, and the vocabulary the confirmer expects:

| Verdict | Means |
|---|---|
| `PASS` | The step ran and its expected result matched. |
| `FAIL` | The step ran, the result did not match, **and** the cause traces to the pull request's change. |
| `INCONCLUSIVE` | The step could not be reliably run. Always carries a cause tag from the repository's list (for example `sign-in-failed`, `staging-network`, `account-state`, `steps-inaccessible`). |
| `SKIP` | Not run, on purpose, with the reason. |
| `MISSING_PREREQ` | A stated prerequisite was not met, so no step in the section ran. |

**Never FAIL an environmental block.** A 404 while signed out, a stale deploy, a network error, or a missing account state is `INCONCLUSIVE` with its cause. Before you record an environmental cause, try the remediation the rig rules give for it, and log the attempt. An environmental cause with no logged attempt is invalid.

**An adapted step keeps its verdict and says it was adapted.** Sometimes the deployed data makes a step impossible as written. Then run the closest path that reaches the same code. Record the adaptation in the step's log entry and in the posted result line. Never adapt a step the rig rules list as out of rig scope. A state dispatch or a script that skips the user's path does not reach the same code.

**The rollup covers the listed Test Steps only.** Problems found outside the listed steps never appear as a `FAIL` line (see Other observations). Apply the first row that matches, for each section and for the whole run:

| Steps | Rollup |
|---|---|
| Any `FAIL` | `fail` |
| Every step `PASS`, adapted or not | `pass` |
| Otherwise (some `INCONCLUSIVE`, `SKIP`, or `MISSING_PREREQ`, no `FAIL`) | `incomplete` |

A step that depends on an earlier step that did not run is `INCONCLUSIVE` with the earlier step's cause, unless another path reaches it.

## Process

### 1. Scope

Resolve the pull request, its head commit, and its base, per the posture file. In embedded mode, take them from the caller.

### 2. Target proof

Before you test, prove the deployed build carries this pull request's code. Find the deployed URL and the deployed commit the way the project file says. Never build a URL by hand when the project file says to read it.

Fetch the pull request head and the deployed commit first. When the deployed commit is behind the head, diff the two over the pull request's files:

```bash
git diff <deployed> <head> -- $(git diff --name-only <base>...<head>)
```

A diff command that fails proves nothing. Treat it as unproven, not as "no difference". Look for a newer deploy before you settle for a stale one.

Name the files that differ. Do not drive steps that exercise those files. Record each one as `INCONCLUSIVE` with cause `needs-retest`, and say why in the section. No confirmer verdict overrides this, because a check of the wrong code is not evidence.

Before the report, resolve the head once more. If the head moved during the run, repeat this check for the new commits.

### 3. Scenarios

Run the steps extractor on the pull request. Write one scenario record per Test Steps section: the steps verbatim, the expected results as the oracle, setup prose as precondition notes, and the source marked as extracted from the pull request. Validate each record.

Read the rig rules before driving. A step the rig rules list as out of rig scope is recorded `INCONCLUSIVE` with its cause before you start, not after an hour of trying.

Read the oracle source for the feature. When a step's expected result and the documentation disagree, test against the pull request's step and record the disagreement as an observation.

When the pull request has no Test Steps, say so and stop in standalone mode. In embedded mode, return "no Test Steps" and let the caller decide.

### 4. Session proof

Sign in the way the project file says, with an account fit for each section (for example a negative account for a gated page). Before you rely on a negative result, prove the session is signed in. Record the proof (a cookie, a state read, the account menu) as evidence.

### 5. Drive and capture

Run each section's steps in order, through the driving recipes.

**Record a video and take screenshots, for every step.** The two serve different readers.

- **Video is the posted evidence.** It shows the path to the state, not only the state, and a reader can replay a reproduction long after the build is gone. Make each video the way the project file's video skill says: its recorder, trimming, input marks, caption, and style. Start one recording per step before the step's first action, and stop it when the outcome is on screen. Name it `step-<N>.<ext>`.
- **Screenshots are the confirmer's evidence.** The confirmer judges from images and state reads, and it cannot watch a video. While the recording runs, capture one screenshot for every state a step checks, every failure, and both sides of every state change (before and after a save, a discard, a search, a delete). The last frame of each step is the outcome frame: the asserted end state. Capture it just before you stop the recording. A step with no outcome frame is not `PASS`.
- **Screenshots alone** are allowed when the step checks something truly static, with no interaction to show (copy on a page, a layout, a static state), when the project file names no recorder for the driver, when the step has no screen (a test run, an API read), or when the recorder fails. Record the reason in the step's log entry.

- Name frames `step-<N>-<SEQ>-<what-it-shows>.png`, with `SEQ` zero-padded in capture order. Name each frame for what it shows, not for the step's goal.
- Save them in the section's unit directory (the project file says where). Never cite a file you did not write.
- Look at each frame you cite. Zoom or crop to check a small region, rather than trusting the file name.
- Prefer a structured state read over a visual check when the driving recipes offer one. Save the read as `state-<N>.json`.
- Never inject anything into the page under test to make a screenshot show state: no overlay, banner, or added element, and no restyled app element. A screenshot shows only what the app rendered. When a step asserts state with no visible UI (storage, analytics, network), the state read (`state-<N>.json`) is the outcome evidence, and the outcome frame is the unmodified page at the moment of the read. Quote the read's relevant values in the step's result text.
- When a step turns on a quantity ("narrow width", "about a second"), test values across the range, and record each value.

Append to the evidence log as you go: one action entry per action with its observation and evidence references, then one step checkpoint per step with its verdict. Write the observation as what the screen showed, with no verdict words. End each section with the preliminary verdict event, then build and validate the trace.

**Free play.** After a section's listed steps, play with the feature the way a curious user would. Test Steps cover the path the author thought of. Free play finds what the author did not think of. Start from what the diff touches, then go one step sideways:

- Vary the inputs the steps used: empty, very long, non-Latin text, pasted, repeated quickly.
- Interrupt the flow: back, refresh, a second tab, cancel halfway, resize to a narrow width, sign out and in.
- Try the neighbours: other entry points to the same feature, other personas or account states, features that share the changed code or state.
- Chase anything that looks off until it reproduces or clearly does not.

Spend about as long on free play as on the listed steps, and stop sooner when nothing new turns up. Record free play on video (`free-play.<ext>`) as a working record. It is never posted. Log each free-play action in the evidence log like any other action. When something looks off, screenshot the odd state, then reproduce it once more from a known start state as its own trimmed video (`repro-<SEQ>.<ext>`) that holds only the reproduction. Use screenshots alone only when no recorder is available. Free-play results never change a verdict or the rollup. Post only findings worth a reader's time: a defect, a confusing state, or a risk the author likely missed. Each posted finding goes in Other observations with a one-line statement of the problem, numbered repro steps from the known start state, what you expected and what happened, its repro video, and a screenshot of the problem. When free play finds nothing worth sharing, post nothing about it. The Writes rules apply in free play too.

### 6. Live check of panel candidates (embedded mode only)

The caller hands in candidates after its synthesis, so this step runs after the listed steps and after that hand-in. For each candidate that a browser can reproduce, try it live in its own unit directory. Name each one's result: reproduced, did not reproduce, or not reachable from the deployed build. A reproduced candidate is stronger evidence than a re-read. A candidate that does not reproduce goes back to the caller for re-examination before it is posted.

### 7. Confirm

Unless `--no-confirm`, dispatch the independent confirmer once per section, all in parallel, with the scenario record, the trace, and an output path for the verdict. The trace's evidence for each step is its screenshots and state reads. Do not hand the confirmer a video as a step's only evidence. The confirmer judges from the receipts, not from your conclusion.

- **Agree**: the final verdict is yours.
- **Dispute with a retest directive**: run the one action the directive names, capture its receipt, rebuild the trace, and confirm once more. A second dispute becomes `INCONCLUSIVE` with cause `needs-human`.
- **Unverifiable**: capture the missing receipt it names, the same way.

The confirmer's final verdict is the reported verdict. When confirm was skipped, the section states that the results are unconfirmed.

**Fallback confirmer rules.** Use these when the project maps no confirmer. Hand it the screenshots and state reads, not the videos. Read the evidence before the verdict. Check the outcome frame of every non-`SKIP` step against the step's expected result. Dispute a `PASS` over a wrong screen, an error dialog, or a missing outcome frame. Dispute any environmental cause with no logged remediation try.

### 8. Writes

QA writes only data it creates, and removes it before it finishes. When the product allows a name, name created data so it is clearly QA's. Ask the user before you confirm any delete, even of data QA created. Cancel every confirmation dialog that changes shared state QA did not create. The project file lists forbidden writes. Those are absolute, under every authorization.

### 9. Report

Compose the section (next heading). Copy the evidence set (videos and screenshots) and the unit directories to the session scratchpad.

- **Standalone**: follow the posture file. Print the local summary with local paths in place of video and image URLs, ask one question, and wait. On approval, upload, embed, and post a `COMMENT` review. The body is the attribution line the posture file requires (when one applies), a blank line, then the section.
- **Embedded**: return the hand-back.

## The Dev QA section

Exactly these parts, in this order:

1. `## Dev QA`
2. One plain paragraph: the result, the deployed URL, and the deployed commit. If the results are unconfirmed, say so.
3. One `<details>` block per Test Steps section. The `<summary>` is the section name and its rollup. Inside, one line per step: the step number, its verdict in plain words (`Pass`, `Fail`, `Inconclusive (<reason>)`, `Skipped (<reason>)`), `adapted:` and the change when adapted, then its video, embedded the way the project file says. Add a screenshot next to the video when a still makes a point clearer than the video can: an error, exact copy, or a specific state or value the reader should not have to pause the video to find. A truly static step posts its screenshots alone.
4. An optional `<details>` block titled `Other observations`: free-play findings worth sharing (problem, repro steps, expected and actual, repro video, screenshot), other problems outside the listed steps, and documentation disagreements. Leave the block out when there is nothing to report. Never written as `Fail` lines. In embedded mode, a live-reproduced candidate that the caller posts as a finding gets one pointer line here, not a restatement. A candidate the caller drops does not appear.
5. A closing status line from the rollup table: `Dev QA: pass`, `Dev QA: fail (<n> steps)`, or `Dev QA: incomplete (<n> steps not run)`.

Never put a step result outside a `<details>` block. Be terse and matter-of-fact. When a step's failure is also a posted code finding, point to it in one line ("see the comment on `default.conf:192`") rather than restating it. Write no attribution line inside the section. The body's attribution line covers it.

## Embedded hand-back

Return these to the caller, and nothing else:

- The section, with local video and screenshot paths in place of URLs.
- The video and screenshot paths to upload, in the order the section cites them (the posted ones only).
- Per panel candidate: reproduced, not reproduced, or not reachable, with the evidence path.
- Steps this skill cannot drive, with the repository command that runs them.
- Each framework role that fell back, and why.
- If any verdict is unconfirmed, which ones.

## Red flags

Stop and re-read the rule it breaks:

- A `PASS` line with no outcome frame.
- A `FAIL` whose cause is the environment, the account, or a stale deploy.
- A step driven on a deploy that does not carry the files it exercises.
- An out-of-rig-scope step recorded as an adapted `PASS`.
- An environmental `INCONCLUSIVE` with no logged remediation try.
- A negative result recorded before the session was proven signed in.
- Results reported for files the deployed commit does not carry.
- A problem outside the listed steps written as a `Fail` line.
- A screenshot cited that you did not open.
- A step with a video but no outcome screenshot.
- A step with screenshots but no video, with no stated reason, when the project names a recorder.
- A video handed to the confirmer as a step's only evidence.
- A free-play finding with no repro video when a recorder was available.
- The raw free-play recording posted, or a free-play section that reports nothing.
- A hand-rolled Test Steps parser in a repository whose project file maps an extractor.
- A deployed-build check handed to the local run orchestrator.
- An unconfirmed verdict presented as confirmed.
- A delete confirmed without asking the user.
- Any posted sentence naming a confirmer, an agent, a scenario record, or a run directory.
