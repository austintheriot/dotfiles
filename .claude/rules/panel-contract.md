---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Panel review contract

The shared output and scope contract for any subagent dispatched by `/expert-review`. Agents read this once per call so the dispatch prompt doesn't have to restate it.

## Output format

For every finding:

- **severity**: `blocker` | `major` | `minor` | `nit` | `insight`
- **confidence**: 0-100. **Only report findings >= 50.**
- **file:line** anchoring the finding.
- **Headline** -- one sentence.
- **Body** -- 1-3 sentences. The issue, and the fix if obvious. Acknowledge cost / tradeoff if non-trivial.

### Severity rubric

- **blocker** -- will break in production, violates an explicit project rule, security or correctness defect with a reachable trigger.
- **major** -- significant gap or design problem worth addressing before merge.
- **minor** -- noticeable but not blocking.
- **nit** -- cosmetic / style.
- **insight** -- structural / outside-the-box reframing (often FP- or architecture-flavored). Additive, not competing with bug-find severities.

### Confidence rubric

- **90-100** -- verified real. Specific trigger, specific consequence.
- **70-89** -- very likely real. Strong evidence, not fully verified.
- **50-69** -- probably real but may be a nitpick / edge case / matter of taste.
- **Below 50** -- do not report.

## Scope

- Review only within your lens.
- Findings outside your lens: mention briefly as a `See also: <other-lens>` line; do not duplicate other experts' work.
- The synthesis pass merges overlapping findings across experts. Don't try to pre-coordinate.

## Do NOT flag

- Issues a linter / typechecker / compiler would catch (imports, formatting, basic types). Assume CI runs these.
- Pre-existing issues outside your lens **in diff mode**. In survey mode, pre-existing issues ARE the point.
- Code with explicit lint-ignore or suppression comments.
- Intentional functionality changes consistent with the code's purpose.
- Project-conventional style choices the team has deliberately adopted (read `CLAUDE.md` / `.claude/rules/*.md` / `CONTRIBUTING.md` first; project rules win).
- Pedantic nitpicks a senior engineer would not bother calling out. When in doubt, omit.

## Evidence discipline

These rules come from a benchmark of review agents against 1,800 confirmed bugs (Yang et al., arXiv 2610.13668, 2026). Agents opened the buggy file in 95.6% of runs but named the bug in only 56.2%. Most misses are diagnosis failures, not search failures.

- **Author claims are claims, not evidence.** The PR description, commit messages, linked issue, code comments, and test names state what the author intended. Test the code against the claim. Do not use the claim to decide that the code is correct. When the code does something other than the stated intent, that mismatch is a finding. In the benchmark, every model approved a buggy condition because a comment and the PR description called it intended. In a separate study, framing a PR as bug-free cut vulnerability detection sharply (Alexopoulos et al., arXiv 2603.18740).
- **Look past the diff.** A bug visible in the diff had 2.6 times the odds of being caught as a bug that needed outside context. For each changed function, type, schema, or contract, read its callers and its dependents before you report the region clean. State in the finding which outside code the bug depends on.
- **On a bug-fix change, ask what the fix breaks.** Bug-fix PRs had half the odds of a caught bug after adjustment. The likely cause: reviewers spend their attention checking that the fix works. Check the other callers of the changed code, and check the inputs that worked before the fix.
- **Run a probe when reading is not enough.** Only 8% of agent turns ran code. When a claim turns on behavior you can execute (a parser on an edge input, a boundary comparison, a query plan), run it in a scratch location and report the result. Do not edit the files under review.
- **Agreement is not verification.** Ten agents can endorse the same wrong finding. Cite the specific evidence for your finding, not a pattern label, so the synthesis can tell independent agreement from a shared blind spot.

## Vocabulary: robustness is not resilience

Call retries, timeouts, circuit breakers, fallbacks, failover, and planned degraded modes **robustness** or **fault tolerance**. Keep **resilience** for the capacity to adapt to surprises the design did not plan for (David Woods's "graceful extensibility"; `distributed-systems.md` § 45 and § 48). A finding that says "add a retry for resilience" names the wrong property and hides the real question: what happens past the edge of the plan, and who can act there?

## Mode awareness

The dispatch prompt names the mode (`diff` or `survey`):

- **Diff**: focus on the changed regions. Surrounding code is context. Pre-existing issues out of scope unless the change exposes them.
- **Survey**: code is reviewed in full as a snapshot. Pre-existing issues are in scope.

## Read-only

You suggest; you do not write. A probe in a scratch directory, or a test you write and delete again, is not a write. Leave the working tree as you found it. The user reviews and decides.

## When a region is clean

Say so explicitly: "No findings in <region> from this lens." Negative signal is useful to the synthesizer.
