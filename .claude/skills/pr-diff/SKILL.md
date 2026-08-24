---
name: pr-diff
description: Produce a clean "only what this PR / branch adds" diff, excluding noise from merges with the base branch. Resolves base branch, computes merge-base, runs three-dot diff, fetches PR metadata when applicable, returns a structured summary plus the full diff when it fits. Use when about to review a PR or branch and you want the change set without 24 merge commits' worth of staging in the diff. Invoked directly (cheap, dirties main context) or via the `pr-diff` agent (isolated context, recommended for review workflows).
---

# PR Diff

Produce the cleanest possible "only what changed on this branch" view, suitable for handing to a reviewer (human or agent).

The problem this solves: a branch that has merged from `main` / `staging` repeatedly accumulates merge commits that pollute `git log` and `git diff` output. The naive `git diff main..HEAD` includes everything the branch ever touched plus everything that came in via merges. The right answer is **three-dot diff** (`git diff main...HEAD`), which is equivalent to `git diff $(git merge-base main HEAD)..HEAD` -- only the changes unique to this branch since it diverged.

## Inputs

One of:
- **PR number** (`53065`): look up via `gh pr view <N>`, extract base branch and head SHA.
- **Branch name** (`gm/shared-model-tests`): try `gh pr list --head <branch>` to find an associated PR; if exactly one, use it. If zero or multiple, fall back to diffing against the repo's default branch (`gh repo view --json defaultBranchRef -q .defaultBranchRef.name`).
- **Range** (`main..HEAD`, `staging...feature`): use directly; skip PR metadata.
- **No input**: assume current branch; resolve as above.

Optional flags (caller-supplied):
- `--no-pr`: skip PR metadata fetch even if a PR is associated.
- `--paths <globs>`: restrict diff to matching paths (e.g., `'*.ts' '*.tsx' 'Backend/**'`).
- `--exclude <globs>`: exclude paths (lockfiles, generated code). Sensible defaults applied unless `--no-default-excludes`.
- `--max-diff-lines <N>`: override the inline-diff size threshold (default 2000).

## Default exclusions

Applied unless `--no-default-excludes`. These are pure noise in review:
- Lockfiles: `package-lock.json`, `yarn.lock`, `pnpm-lock.yaml`, `bun.lock`, `Cargo.lock`, `Gemfile.lock`, `poetry.lock`, `go.sum`, `composer.lock`
- Generated: `*.pb.go`, `*.pb.ts`, `*_generated.*`, `*.gen.*`
- Build output: `dist/`, `build/`, `out/`, `.next/`, `target/`, `node_modules/`
- Snapshots: `__snapshots__/`, `*.snap`

If the caller cares about lockfile changes (security review, dependency audit), they pass `--no-default-excludes`.

## Procedure

1. **Resolve the input** to a concrete `(base_branch_or_sha, head_sha)` pair.
   - PR number: `gh pr view <N> --json baseRefName,headRefOid,number,title,body,author,url`.
   - Branch name: `gh pr list --head <branch> --state open --json number,baseRefName,headRefOid`. If one result, use it. If zero/multiple, fall back to default branch + skip PR metadata.
   - Range: parse directly. Two dots = `A..B`, three dots = `A...B`. Honor as given.

2. **Compute the merge-base** for non-range inputs: `git merge-base <base> <head>`. Capture the SHA.

3. **Run the three-dot diff**:
   ```
   git diff <merge_base>..<head> [--] [paths] [':!exclude' ...]
   ```
   Use `--stat` first to get the file list and sizes, then full diff if under threshold.

4. **Fetch PR metadata** if a PR is associated and `--no-pr` not set:
   - Title, body, base branch, head SHA, author, URL (already fetched in step 1).
   - Linked issues: parse body for `(?i)(fixes|closes|resolves)\s+#(\d+)`, cap at 3, fetch each with `gh issue view <N> --json number,title,body,state`.

5. **Decide diff inclusion**:
   - If total diff lines <= `max_diff_lines` (default 2000), include full diff.
   - Else, include `--stat` summary + first ~200 lines as `excerpt`, set `truncated: true`.

6. **Return the structured result** (see below).

## Output contract

Markdown sections, in this order:

```
## PR / Branch

- Number: <N> (or "none" if no PR)
- Title: <title>
- Author: <author>
- URL: <url>
- Base branch: <base>
- Base SHA (merge-base): <sha>
- Head SHA: <sha>
- Body:
  <body, indented>

## Linked issues

- #<N>: <title> [<state>]
  <one-line body excerpt>

## Stats

- Files changed: N (M after exclusions)
- Additions: +X
- Deletions: -Y
- Truncated: true | false

## Files

- M  path/to/file.ts  (+12 -3)
- A  path/to/new.ts   (+45 -0)
- D  path/to/old.ts   (+0 -78)
- R  old/path -> new/path  (+0 -0)

## Diff

<full diff if under threshold, else excerpt + "use git diff <merge_base>..<head> -- <path> to fetch more">
```

When called from an agent, the caller is the consumer -- format for readability over machine-parseability. The structure is regular enough that a downstream agent can pattern-match against it.

## When to use directly vs via the agent

- **Directly** (in the main thread): you're already in a review and have context loaded; one quick `git diff` won't drown you. Cheap.
- **Via the `pr-diff` agent**: you're *about to* start a review and the diff is the first thing you need. Isolated context, returns clean summary, no `gh pr view` JSON dump in main.

The agent's prompt is thin: "Run this skill with these args, return the structured result."

## Common gotchas

- **`git diff A..B` vs `A...B`**: two dots = "changes from A to B" (includes everything in B not in A, *plus* anything merged into A that wasn't in B yet -- not what you want). Three dots = "changes on B since it diverged from A." Always three dots for PR review.
- **Stale local base branch**: long-lived worktrees frequently have a local `main` / `staging` months behind the remote. Computing `merge-base` against the stale local ref produces a massively inflated diff (everything that landed on the remote in the interim). **Always prefer `origin/<base>` over local `<base>`** when the remote ref exists; `git fetch origin <base>` first if accuracy matters. The agent should detect this: if `git rev-list --count <local-base>..origin/<base>` returns > 0, use `origin/<base>` as the merge-base input and note the local staleness in the output.
- **Repo not a git repo / `gh` not authed**: fall back gracefully; report the failure mode in the output rather than crashing.
- **Forked PRs**: `gh pr view` returns the right SHA on the fork; `git merge-base` may need `git fetch origin pull/<N>/head` first.
- **Very large diffs** (10k+ lines): the threshold protects the caller. The caller can `git diff <merge_base>..<head> -- <specific path>` per file.
