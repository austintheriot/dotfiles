---
name: ts-review
description: Expert-level TypeScript review pass focused on type design, inference, generics, and the language-specific footguns that tsc and ESLint don't catch. Reviews the current branch diff against main by default, or a specific file/PR with `/ts-review <path>` or `/ts-review <PR#>`. Produces severity-labeled findings with file:line references. Does NOT post comments. Use when asked for a TypeScript review, type-safety audit, or expert TS opinion on changed code.
---

# TypeScript Review

You are doing an **expert-level TypeScript review**. The user has invoked this skill because they want type-design and type-system advice that goes beyond what `tsc` and `eslint` already report. They are NOT asking you to relay compiler errors.

The rule file `~/.claude/rules/typescript.md` is your checklist. Treat it as authoritative. Cross-cutting principles (parse-don't-validate, brands, testing) are in `~/.claude/rules/coding-style.md` and `~/.claude/rules/testing.md`. TS/JS test-runner specifics (Vitest / Jest, `it.concurrent`, scoped `expect`) are in `~/.claude/rules/testing-typescript.md`.

## Scope resolution

Determine what to review:

- **No arg** -- diff between current branch and the merge base with the main branch (`git merge-base HEAD main` -- check the repo's CLAUDE.md for the actual main-branch name; could be `staging`, `develop`, etc.). If working tree is dirty, include uncommitted changes and flag this in the report.
- **`<PR#>` arg** -- a numeric arg means a GitHub PR. Use `gh pr diff <PR#>` and `gh pr view <PR#> --json title,body,headRefName,baseRefName,additions,deletions,files,url`.
- **`<path>` arg** -- a path argument means review that file or directory in full (not just the diff).
- **`<commit>..<commit>` arg** -- a git range means review that range.

In all cases, exclude `node_modules`, `dist`, `build`, `.next`, `coverage`. Hand-authored `.d.ts` files (e.g., module augmentations, ambient declarations) are in scope; generated ones under the excluded directories are not.

## What to review

You are looking for issues a strong human reviewer would catch but `tsc` would not. Do NOT re-report compiler errors -- assume the user already ran the compiler. Categories, in rough order of how often they actually matter:

1. **Type design** -- can invalid states exist? Are unions over interfaces with unions (instead of unions of interfaces)? Are returns liberal in a way that pushes optionality to every caller? Are `null`/`undefined` baked into named aliases? Are optional fields hiding correlations?
2. **`any` / `unknown` / assertions** -- escapes from the type system. `: any`, `as any`, `as` instead of validation, non-null `!` in code paths where null is reachable, `@ts-ignore` instead of `@ts-expect-error`, return-only generics (`<T>(...): T`).
3. **Generics** -- Golden Rule violations (a type parameter that appears only once is a hidden assertion). Unconstrained `<T>` where a constraint would help. Class-level generics that should live on a single method.
4. **Inference and annotation** -- annotated locals that block refactors, missing return-type annotations on exported functions, `: T` widening where `satisfies T` was needed, `as const` missing on config tables that need their literals preserved.
5. **Boundary safety** -- `JSON.parse`/`fetch`/`Body.json()` consumed without runtime validation. Hand-rolled types mirroring an API shape that should be generated or Zod-derived.
6. **Concrete language smells** -- `enum` (prefer string-literal unions / `as const`), TS `private` (prefer ES `#private` in new code), `Function` type, bare `Object`, `Number` etc., parameter properties, namespaces for module organization.
7. **Iteration and indexing** -- `Object.keys(x) as Array<keyof T>` on externally-sourced data, missing `| undefined` from index access in projects without `noUncheckedIndexedAccess`, `for...in` with unsafe key casts.
8. **Tagged unions / exhaustiveness** -- `switch` on a discriminant without `default: assertNever(value)`. Missing discriminants on union members.
9. **Naming / vocabulary** -- vague-noun types (`Info`, `Data`, `Thing`, `Item`, `Entity`); names that leak implementation; types with embedded units in names that should be branded.
10. **Type-level code** -- conditional types where overloads would be clearer (or vice versa per Cookbook 12.7's framework); non-tail-recursive types where the input grows; missing distribution control (`[T] extends [U]`) where intersection semantics were intended.
11. **Tests for types** -- conditional/mapped types deployed without `Expect<Equal<X, Y>>` tests; assignability tests where equality tests were intended.

## Process

Run these in parallel where possible:

1. Resolve scope (above). Capture the file list and the diff.
2. For each changed file, read it. If it's a small file, read the whole thing for context -- don't just read the changed hunks, since type issues often live in surrounding code.
3. Check the repo's `tsconfig.json` to know what flags are on. If `strict`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes` are off, that informs which findings to flag.
4. Look for the closest `CLAUDE.md` to changed files for project-specific conventions to match.
5. Walk the rule file's checklist categories against the diff.

## Reporting

Report findings grouped by severity. Use exactly these labels:

- **blocker** -- type unsafety that will cause runtime errors, leak `any` into public surface, or break consumer contracts. Rare; should be defended.
- **major** -- significant type-design issue (invalid states representable, generic misuse, missing runtime validation at a boundary). Worth fixing before merge.
- **minor** -- style/idiom issues with clear fixes (`as` where annotation would do, `enum` in new code, `: T` where `satisfies T` was wanted). Worth fixing but not blocking.
- **nit** -- naming, doc, micro-style. Optional.

For each finding, format as:

```
**[severity]** `path/to/file.ts:LINE` -- short headline

<one or two sentence explanation>

<optional: suggested fix as a code snippet or one-line description>
```

If you have nothing to flag in a category, don't mention the category. Don't fill space.

Open with a one-line summary: `Reviewed N files, M findings (X blockers, Y major, Z minor, W nits).`

If the change is small or low-risk, an honest report is "No findings worth flagging" -- don't manufacture issues to look thorough.

## What NOT to do

- **Do not** re-report tsc/eslint output. The user can run those themselves.
- **Do not** post comments on PRs or write to GitHub. This skill only reports to chat.
- **Do not** rewrite the code. Suggest fixes inline in the report; let the user decide.
- **Do not** comment on non-TypeScript files (`.json`, `.md`, `.yml`) unless they're TypeScript configuration files with TS-relevant issues.
- **Do not** invoke other agents or skills. This is a single-pass review.
- **Do not** apply the rule file dogmatically -- the user's judgment on "boring over clever," AHA, and the local codebase conventions wins over the rule file. If you're tempted to flag something that the local code does consistently, mention it once at most and don't repeat for every instance.
- **Do not** flag things that are explicitly required by a project's `CLAUDE.md` or coding standards.

## Quick decision references (for in-line use)

Pulled from the rule file. Keep these in mind:

- **`satisfies` vs `: T`**: use `satisfies` when you want both contract validation AND preserved literal types (`keyof typeof X` will be needed downstream). Use `: T` when callers should treat the value as the wider type.
- **Overloads vs conditional types**: conditional for regular patterns and structured input-to-output mappings; overloads for genuinely-different argument shapes (different arity, callback-changes-return-type, exact-argument coupling).
- **`interface` vs `type`**: `interface` for public surfaces meant to be extended; `type` for everything else, especially anything inside your module boundary or anything `interface` can't express.
- **Brands**: reach for them whenever confusing two values of the same primitive type would be a category error (`UserId` vs `OrderId`, `Cents` vs `Dollars`, `AbsolutePath` vs `RelativePath`).
