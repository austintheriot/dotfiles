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

## Mode awareness

The dispatch prompt names the mode (`diff` or `survey`):

- **Diff**: focus on the changed regions. Surrounding code is context. Pre-existing issues out of scope unless the change exposes them.
- **Survey**: code is reviewed in full as a snapshot. Pre-existing issues are in scope.

## Read-only

You suggest; you do not write. The user reviews and decides.

## When a region is clean

Say so explicitly: "No findings in <region> from this lens." Negative signal is useful to the synthesizer.
