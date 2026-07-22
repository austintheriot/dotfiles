---
name: collaborative-pr-feedback
description: Address human reviewer feedback across one or more PRs collaboratively -- fix locally with one commit per finding, walk Austin through each commit against its feedback link, push + reply per thread only after his review, then monitor CI. Use when Austin says reviewers have left feedback and asks to start addressing it.
---

# Collaborative PR Feedback

Work reviewer feedback the way Austin and Claude did on the PDF-export stack: Claude
fixes, Austin reviews locally, Claude pushes and answers the reviewers. The human
checkpoints are the point -- never collapse them.

## Phase 1 -- Sweep

For every PR in scope, fetch in one pass:

- Inline review comments and reviews via `gh api --paginate "repos/<repo>/pulls/<n>/comments"` / `.../reviews`, plus issue comments. Exclude bots (`claude[bot]`, CI bots) and Austin himself.
- Thread state via GraphQL `reviewThreads` (`isResolved`, `isOutdated`, full comment chain) -- this reveals which older threads already have replies and which comments are follow-ups in an existing thread.

Record for each item: reviewer, comment id, path:line, and the thread's reply
history. Comment id -> link is `https://github.com/<repo>/pull/<n>#discussion_r<id>`
(inline), `#issuecomment-<id>`, or `#pullrequestreview-<id>`.

## Phase 2 -- Classify, then confirm scope

Three buckets:

1. **Clearly actionable** -- imperative or obviously-right changes. Fix without asking.
2. **Needs Austin's direction** -- product calls, scope tradeoffs, anything where the
   reviewer's suggestion has a real alternative. Surface with a recommendation; do NOT
   action or take a public stance until he decides.
3. **Reply-only** -- questions needing an answer, not a change. Verify the answer in
   the code before drafting it (e.g. trace who actually writes a field rather than
   guessing); hold the reply until the push phase.

Investigate bug reports before classifying them: reproduce or trace the mechanism
(subagent investigation is fine). A confirmed reviewer-reported bug is bucket 1 even
when the fix is substantial.

## Phase 3 -- Fix locally, one commit per finding

- Work on each PR's own branch in its own worktree. **Local only -- no push, no
  GitHub replies, nothing outward-facing yet.**
- **One commit per finding**, in a sensible order (blocking first). Name the reviewer
  in the commit body ("Review feedback (login): ...") and describe behavior, not the
  change history.
- TDD where the finding is behavioral (red test first); say so when skipping (e.g.
  WASM-bound seams no unit harness reaches).
- Per-commit verification, no exceptions: relevant vitest suites, `tsc --noEmit`, and
  **prettier + eslint on EVERY touched file** -- a single unlinted file has turned CI
  red twice (`no-duplicate-imports`, `consistent-type-imports` after a refactor made
  an import type-only).
- Stale local commons after staging merges: `bun run build` in `common-universal`
  then `common-model` (order matters) when `blockWrapSupport`-style tsc errors appear.
- Stacked PRs: fix at the **originating slice**, and defer the cascade until all PRs
  in the round are reviewed and pushed -- don't churn later branches per fix.

## Phase 4 -- Walkthrough for Austin

Per PR, when he asks (he goes one PR at a time): present the commits **in creation
order**, each entry combining:

- **Bare HTTP link(s)** to the feedback thread(s) it addresses -- always bare URLs,
  never markdown-hidden (`[label](url)` renders without the URL in his terminal).
  Include follow-up comment links in the same entry.
- Commit hash, files touched, and what the feedback said.
- What the fix does and **why that shape** -- especially judgment calls he should
  check (tradeoffs taken, behavior changes, anything discovered worse than reported).
- Where to review: the worktree path and
  `git log -p origin/<branch>..HEAD`.

Also list, separately: open items needing his direction, reply-only questions with
the verified answer, and stale-but-already-replied threads that just need his
resolve click (never resolve threads for him).

## Phase 5 -- Push + reply (only after his go-ahead)

On "push and reply" for a PR:

1. Re-run eslint + prettier over every file the branch's local commits touched, then
   push the branch.
2. Reply to **each thread** via
   `gh api -X POST "repos/<repo>/pulls/<n>/comments/<id>/replies"`: lead with the fix
   status and the commit hash, one reply per thread (cover follow-up comments in the
   same reply). Voice per `/write-like-austin`; every reply starts with the
   `Posted by Claude on behalf of @austintheriotgl` line.
3. For discussion-bucket threads where Austin has taken a position, state it as the
   current plan and leave the door open ("happy to talk through it if you think it
   should block"). If he hasn't decided, don't reply at all.
4. Re-request the reviewer (`gh api -X POST .../requested_reviewers`) when they
   haven't approved.
5. Monitor CI on the pushed branch (`/monitor-ci` pattern -- a single delayed
   `ScheduleWakeup` check is enough per push; known-flake list applies). Fix red CI
   caused by the push without re-asking.

## Cross-PR accumulators

Maintain across the whole round and keep current:

- **Known-limitations doc** (product-facing draft, e.g.
  `~/Documents/code/Notability/pdf-export-known-limitations.md`): every degrade path,
  scope gap, or by-design oddity a reviewer or QA surfaced -- user-visible behavior,
  how it degrades, follow-up status. Lives outside the worktrees.
- **Needs-direction list**: re-surface unanswered items at each natural pause; never
  let one silently drop.
- Candidate follow-up tickets (draft on request; Austin files or approves them).

## Hard rules

- Nothing outward-facing (push, reply, ticket, label) before Austin reviews that PR's
  commits -- and posting approval for one PR does not extend to the next.
- Never resolve review threads; that click is his.
- Never reply to a discussion-bucket thread ahead of his decision.
- Bare URLs for every feedback reference, every time.
