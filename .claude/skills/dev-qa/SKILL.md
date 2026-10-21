---
name: dev-qa
description: Use when asked to dev QA, smoke test, or verify a pull request on its deployed build (a per-commit preview URL, a review environment, or a staging deploy). The check runs the pull request's Test Steps in the test environments the project file names. It drafts the result as a pending review on the pull request. Also runs under an orchestrator that merges its contribution with other work. Not for QA of an undeployed local branch. The repository's own local QA skills cover that.
argument-hint: "[<PR number | PR URL>] [--contribution <path>] [--verify <findings.json>] [--no-confirm]"
---

# Dev QA

Test a pull request on the build it actually deployed, with a video of every step and a screenshot of every checked state, and report one verdict per Test Steps step.

Three files share the work:

| File | Owns |
|---|---|
| This skill | The process, the verdict rules, the evidence rules, the section shape. Project-neutral and environment-neutral. |
| `~/.claude/local/dev-qa.md` | The project and environment bindings: deployed URLs, deployed-commit proof, the test environments and which one to prefer, accounts, forbidden writes, driver quirks, the video recorder, uploads, and the mapping onto the repository's QA framework. |
| `~/.claude/rules/outside-pr-review.md` | Approval, the never-submit rule, scope resolution, voice, the internals rule, the contribution contract, the local summary, uploads, and the standalone ending through `draft-review`. |

**Read `~/.claude/local/dev-qa.md` and `~/.claude/rules/outside-pr-review.md` before you test.** If the project file does not exist, ask the user for the deployed URL and a test account before you test.

In orchestrated mode, never ask the user. When a blocker stops every step from starting, write the contribution with `status` `not-run` and the blocker as `reason`. A blocker is a missing project file, a failed target proof, or a failed session proof. When some steps ran before the blocker, use `status` `incomplete` instead.

## Test environments

A test environment is where a step runs and what drives it: a browser on this machine, a virtual machine, a simulator, a device, or a remote browser, plus its driver and its recorder. This skill does not choose one. The project file names the environments for each platform, which one to prefer, when to fall back, and how to set each one up and tear it down. Use the preferred environment unless it cannot run the step. When you fall back, record the reason in the step's log entry. When the project file names no environment for a platform, use the drivers this session has, and say which one you used in the local summary.

## Modes

- **Standalone** (`/dev-qa [PR]`): this skill writes its contribution and ends through `draft-review`, as `~/.claude/rules/outside-pr-review.md` says. It runs `draft-review add`, prints `draft-review show`, asks one question, and on approval runs `draft-review create`. The result is one pending review for the head, with the Dev QA section in it. The user submits that review.
- **Orchestrated** (`/dev-qa [PR] --contribution <path>`): this skill never runs `draft-review`, never uploads, and never asks for approval. It writes its result as one JSON file at `<path>`, under the Orchestrated mode rules in `~/.claude/rules/outside-pr-review.md`. The orchestrator owns the summary, the approval, and the two `draft-review` commands. The user is not reachable in this mode. Put a needed hand-off into `handback` instead of asking for it. Wait for every confirmer verdict before you write the file.
- **Verify** (`/dev-qa [PR] --verify <findings.json> --contribution <path>`): an orchestrated mode that tries to reproduce findings from another source on the deployed build. It runs no Test Steps. All Orchestrated rules apply, including the rule that the user is never asked. The `status`, `reason`, and `body` rules of orchestrated mode do not apply, because a verify contribution has none of those fields. See Verify mode.

`--no-confirm` skips the independent confirm stage. Every verdict is then reported as unconfirmed. Verify mode ignores `--no-confirm` and always confirms, because the orchestrator counts `reproduced` as confirmed.

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

When the project file maps no framework, use the fallbacks: a `results.md` plus screenshots per section, and a fresh `general-purpose` subagent as the confirmer with the confirmer rules below. A session can deny a mapped command or agent, for example in an allowlisted unattended run. In that case, use the fallback for that role only. Name each role that fell back in `handback` (orchestrated) or the local summary (standalone).

**The repository's run orchestrator is a different tool.** A local QA runner builds the branch and drives its own targets. It tests the branch, not the deployed build. Do not hand a deployed-build check to it. Some Test Steps need a platform that no available test environment can drive. Report each such step as `SKIP` with the reason. Name the repository command that runs it in `handback` (orchestrated) or the local summary (standalone).

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
| Every step not `PASS` is test-covered (below), and the project file allows test-covered steps for the platform | `pass` |
| Otherwise (some `INCONCLUSIVE`, `SKIP`, or `MISSING_PREREQ`, no `FAIL`) | `incomplete` |

**A test-covered step** is a step that was not run because running it is impractical, and an automated test already checks its expected result. Impractical means no reasonable rig fix removes the block: a rig-scope exclusion, real hardware, a third-party flow. A stale deploy, a signed-out session, or a fixable account state is not impractical. Fix it and run the step. Name the covering test by file and test name, and read it first: it must assert the step's expected result, not only run the same code. A step whose test you cannot name stays `incomplete`. The step keeps its own verdict (`INCONCLUSIVE` or `SKIP`). Only the rollup changes.

**An impractical step with no test is a review comment.** When a step is impractical, no test covers it, and a feasible test could check its expected result, record an untested-step item. It names the step, why it is impractical, the changed line in the diff that implements the step's behavior, and the test to add: the test file (prefer extending an existing test), the setup it drives, and the assertion that fails when the behavior breaks. First search the tests for one that already covers the step, because a test you missed makes the step test-covered instead. When no realistic test can reach the behavior (it needs real hardware, a real third-party service, or a mock so deep the test only re-asserts the code), record nothing. In every mode, write the item as an entry in `comments` (see Contribution). The comment is on that changed line and opens with "Possible coverage gap:". Point the step's result line to the comment in a few words.

**Every rollup carries its counts**, out of the listed steps: `pass (6/8 pass, 2/8 incomplete)`, `fail (1/8 fail, 6/8 pass, 1/8 incomplete)`, `incomplete (5/8 pass, 3/8 incomplete)`. Count `INCONCLUSIVE`, `SKIP`, and `MISSING_PREREQ` steps as `incomplete`. List only the nonzero buckets, in the order fail, pass, incomplete. An all-pass run still shows its count: `pass (8/8 pass)`.

A step that depends on an earlier step that did not run is `INCONCLUSIVE` with the earlier step's cause, unless another path reaches it.

## Process

### 1. Scope

Resolve the pull request, its head commit, and its base, per the posture file. In orchestrated mode, resolve them the same way, and write the full head SHA to `head`.

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

When the pull request has no Test Steps, say so and stop in standalone mode. In orchestrated mode, write a contribution with `status` `not-run`, the reason "no Test Steps", and a one-line `body` under `## Dev QA` that says so.

### 4. Session proof

Sign in the way the project file says, with an account fit for each section (for example a negative account for a gated page). Before you rely on a negative result, prove the session is signed in. Record the proof (a cookie, a state read, the account menu) as evidence.

### 5. Drive and capture

Run each section's steps in order, in the preferred test environment for the step's platform, through the driving recipes.

**Record a video and take screenshots, for every step.** The two serve different readers.

- **Video is the posted evidence.** It shows the path to the state, not only the state, and a reader can replay a reproduction long after the build is gone. Make each video the way the project file's video skill says: its recorder, trimming, input marks, caption, and style. Start one recording per step before the step's first action, and stop it when the outcome is on screen. Name it `step-<N>.<ext>`.
- **Screenshots are the confirmer's evidence.** The confirmer judges from images and state reads, and it cannot watch a video. While the recording runs, capture one screenshot for every state a step checks, every failure, and both sides of every state change (before and after a save, a discard, a search, a delete). The last frame of each step is the outcome frame: the asserted end state. Capture it just before you stop the recording. A step with no outcome frame is not `PASS`.
- **Screenshots alone** are allowed when the step checks something truly static, with no interaction to show (copy on a page, a layout, a static state), when the project file names no recorder for the driver, when the step has no screen (a test run, an API read), or when the recorder fails. Record the reason in the step's log entry.

- Name frames `step-<N>-<SEQ>-<what-it-shows>.png`, with `SEQ` zero-padded in capture order. Name each frame for what it shows, not for the step's goal.
- Save them in the section's unit directory (the project file says where). Never cite a file you did not write.
- Look at each frame you cite. Zoom or crop to check a small region, rather than trusting the file name.
- Clear popups before every recording and every screenshot, the way the video skill's "Clear the screen before you record" says. A screenshot in which a popup covers the checked state is not evidence of that state. Dismiss the popup and take the screenshot again.
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

### 6. Confirm

Unless `--no-confirm`, dispatch the independent confirmer once per section, all in parallel, with the scenario record, the trace, and an output path for the verdict. The trace's evidence for each step is its screenshots and state reads. Do not hand the confirmer a video as a step's only evidence. The confirmer judges from the receipts, not from your conclusion.

- **Agree**: the final verdict is yours.
- **Dispute with a retest directive**: run the one action the directive names, capture its receipt, rebuild the trace, and confirm once more. A second dispute becomes `INCONCLUSIVE` with cause `needs-human`.
- **Unverifiable**: capture the missing receipt it names, the same way.

The confirmer's final verdict is the reported verdict. When confirm was skipped, the section states that the results are unconfirmed.

**Fallback confirmer rules.** Use these when the project maps no confirmer. Hand it the screenshots and state reads, not the videos. Read the evidence before the verdict. Check the outcome frame of every non-`SKIP` step against the step's expected result. Dispute a `PASS` over a wrong screen, an error dialog, or a missing outcome frame. Dispute any environmental cause with no logged remediation try.

### 7. Writes

QA writes only data it creates, and removes it before it finishes. When the product allows a name, name created data so it is clearly QA's. Delete the data QA created without asking. Cancel every confirmation dialog that changes shared state QA did not create. The project file lists forbidden writes. Those are absolute, under every authorization.

### 8. Report

Compose the section (next heading). Copy the evidence set (videos and screenshots) and the unit directories to the session scratchpad.

- **Standalone**: write the contribution file to a path in the session scratchpad (see Contribution). Write every video and screenshot to disk first. If a cited file does not exist, `draft-review add` exits with code 66. Then follow the sequence in the posture file: `draft-review add`, `draft-review show` printed in chat, one question, and on approval `draft-review create`. The result is one pending review for the head. Running `/expert-review-pr` later on the same head updates the same draft, so one review holds both parts. The user submits the review.
- **Orchestrated**: write the contribution file at `--contribution <path>` (see Contribution).

## The Dev QA section

Exactly these parts, in this order:

1. `## Dev QA`
2. One plain paragraph: the result, the deployed URL, and the deployed commit. If the results are unconfirmed, say so.
3. One `<details>` block per Test Steps section. The `<summary>` is the section name and its rollup with counts (`Test Steps: pass (6/8 pass, 2/8 incomplete)`). Inside, one line per step: the step number, its verdict in plain words (`Pass`, `Fail`, `Inconclusive (<reason>)`, `Skipped (<reason>)`), `covered by <test file>: <test name>` when the rollup counted it as test-covered, `adapted:` and the change when adapted, then its video, placed the way the project file says. Add a screenshot next to the video when a still makes a point clearer than the video can: an error, exact copy, or a specific state or value the reader should not have to pause the video to find. A truly static step posts its screenshots alone.
4. An optional `<details>` block titled `Other observations`: free-play findings worth sharing (problem, repro steps, expected and actual, repro video, screenshot), other problems outside the listed steps, and documentation disagreements. Leave the block out when there is nothing to report. Never written as `Fail` lines. When an orchestrator posts a live-reproduced finding that came from outside this run, the section gets one pointer line for it here, not a restatement. A finding that the orchestrator drops does not appear.
5. A closing status line for the whole run, from the rollup table with counts: `Dev QA Overall Result: Pass (6/8 pass, 2/8 incomplete)`, `Dev QA Overall Result: Fail (1/8 fail, 7/8 pass)`, or `Dev QA Overall Result: Incomplete (5/8 pass, 3/8 incomplete)`.

**Leave the test environment out of the post.** The reader needs the result, not the rig. Do not name the machine, virtual machine, or simulator, the audio device, the browser profile, the driver, the recorder, or any workaround that made the rig run. Name an environment detail only when a reader needs it to reproduce or read a result: a repro that needs a specific setup (an account tier, a narrow window, a meeting with no calendar end), or a result that holds only under that setup. An adaptation always stays, because it changes what was tested ("a Google Meet call in place of a Zoom call"), but its rig reason does not ("the test machine has no Zoom"). The build under test (the deployed URL and commit, or the commit a local build came from) always stays. Environment details belong in the local summary (standalone) or `handback` (orchestrated).

Never put a step result outside a `<details>` block. Be terse and matter-of-fact. When a step's failure is also a posted code finding, point to it in one line ("see the comment on `default.conf:192`") rather than restating it. Write no attribution line and no zone headers inside the section. `draft-review` adds them.

## Contribution

Write the contribution file as `kind: "dev-qa"`, at `--contribution <path>` in orchestrated mode and at a scratchpad path in standalone mode. The posture file `~/.claude/rules/outside-pr-review.md` defines the types and rules for every field. This section maps each result of the run to a field.

- `body`: the Dev QA section, with local video and screenshot paths in place of URLs.
- `uploads`: the video and screenshot paths that `body` and the comments cite (the posted ones only), in citation order. Every path in a finding's `evidence` also appears here.
- `findings`: one entry for every failed step and every bug the run observed, including each Other observations finding. Each entry has `kind: "behavior"`, a non-empty `behavior` text, and `evidence` that holds the screenshot or video paths. Number the ids `Q1`, `Q2`, and so on. Each untested-step item (see Verdicts) also gets an entry with `kind: "code"`, an `anchor` on its changed line, and no `behavior`.
- `comments`: one inline comment for each untested-step item, on the changed line with `side` `RIGHT`, with the test to add. Each comment names its finding id in `finding`. A finding for a failed step or a bug has an inline comment only on the changed line it maps to.
- `handback`: one line for each of these items.
  - Each step this skill cannot drive, with the repository command that runs it.
  - Each framework role that fell back, and why.
  - Each verdict that is unconfirmed.
  - Each hand-off that needs the user.
- `status`: the whole-run rollup. `pass` gives `passed`. `fail` gives `failed`. `incomplete` gives `incomplete`. A run that did not start gives `not-run`. Every status except `passed` also needs `reason`. A `not-run` contribution has empty `findings` and empty `comments`. A blocked run writes no untested-step findings.
- `platform`: the label of the platform the run tested, when the project file names one. Omit the field otherwise.

Write the file in one call, after every confirmer verdict is in. In orchestrated mode, this is the last action of the run. In standalone mode, the sequence in the Report step follows it.

## Verify mode

Verify mode answers one question for each finding it receives: does the symptom appear on the deployed build? The findings come from any source, usually a code review. The posture file `~/.claude/rules/outside-pr-review.md` defines the `dev-qa-verify` kind, its `verdicts` table, and every field type. Follow it.

**Input.** `--verify <findings.json> --contribution <path>`. The findings file is a JSON object `{"findings": [...]}`. Each entry has the finding shape from the posture file. The user is not reachable. Put every blocker into `handback`.

**Run these steps, in this order:**

1. Scope. Resolve the pull request and its head, and write the full head SHA to `head`.
2. Target proof, as in Process step 2.
3. Session proof, as in Process step 4.
4. For each finding in input order, reach the state the finding names and look for the symptom in its `behavior`. Drive and capture as in Process step 5. Use the same recorder, screenshot, popup, and no-injection rules as for a Test Steps step.
5. Confirm, as in Process step 6, for every `reproduced` and `not-reproduced` verdict. The confirmer judges the symptom from the screenshots and state reads. When the confirmer disputes a verdict twice, write `not-reachable`. The `note` says what the driver saw and that the confirmer disputed it twice. Add one `handback` line for the verdict with the finding `id` and the observed symptom.
6. Writes, as in Process step 7.

Do not run the steps extractor, do not write scenario records, and do not play freely. Verify mode runs no Test Steps, and it adds no section to the Dev QA post.

**Results.**

| Result | Means | `evidence` |
|---|---|---|
| `reproduced` | The confirm stage saw the symptom. | Required, non-empty. The screenshots and video that show the symptom. |
| `not-reproduced` | The run reached the state the finding names, and the symptom did not appear. | Required. The screenshot that shows the reached state with no symptom. |
| `not-reachable` | The run could not reach the state the finding names. The `note` gives the reason. | Can be empty. |

`not-reproduced` is not proof that the code is correct. It says only that this run, on this build, did not see the symptom.

**Never report `not-reproduced` for a state the run did not reach.** When the run cannot reach the state, the result is `not-reachable`. Valid reasons:

- The state needs an account that the project file forbids creating.
- No available test environment can drive the platform.
- The deployed commit does not carry a file the finding exercises (cause `needs-retest`).
- The rig rules list the path as out of rig scope.

**Coverage.** Write exactly one verdict for each input finding, in input order, with the finding's `id` in `id`. When time runs out before the run reaches a finding, write `not-reachable` for it with the note "not reached before the run ended". When a blocker stops every finding (a missing project file, a failed target proof, or a failed session proof), write `not-reachable` for every finding with the blocker as the note, and put the blocker in `handback`.

**Unreadable input.** When the findings file cannot be read, is invalid JSON, or lacks a `findings` array, that is a blocker. Write `verdicts` as an empty array and add one `handback` line that names the file and the parse error. An empty `findings` array is not a blocker. It gives empty `verdicts`, and that is legal.

**Contribution.** Write one file at `--contribution <path>` as `kind: "dev-qa-verify"`, with `version`, `kind`, `pr`, `head`, `verdicts`, `uploads`, `handback`, and an optional `platform` only. When the project file names the platform that the run tested, write that label in `platform`. It has no `body`, `comments`, `findings`, `status`, or `reason`. A verdict has `id`, `result`, `evidence`, and `note`. Write the `note` as one or two sentences on what the run did and saw. Every path in a verdict's `evidence` also appears in `uploads`, in citation order. Put these items in `handback`:

- Each framework role that fell back.
- Each verdict that is unconfirmed.
- Each verdict that the confirmer disputed twice, with the finding `id` and the observed symptom.
- Each hand-off that needs the user.

Write the file in one call, as the last action of the run, after every confirmer verdict is in.

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
- A posted video or screenshot in which a popup covers the window under test, when the step does not test that popup.
- A video posted after a check of a few spot frames, with no contact sheets of the whole video.
- A step with a video but no outcome screenshot.
- A step with screenshots but no video, with no stated reason, when the project names a recorder.
- A video handed to the confirmer as a step's only evidence.
- A free-play finding with no repro video when a recorder was available.
- The raw free-play recording posted, or a free-play section that reports nothing.
- A hand-rolled Test Steps parser in a repository whose project file maps an extractor.
- A deployed-build check handed to the local run orchestrator.
- An unconfirmed verdict presented as confirmed.
- Any posted sentence naming a confirmer, an agent, a scenario record, or a run directory.
- A posted sentence naming the test machine, the rig, the audio setup, the driver, or a rig workaround, when no result or repro depends on it.
