# Converting the shell test suites to Rust

**A note on vocabulary.** This document says **convert** rather than
**port** for moving a suite to Rust, because `port` is load-bearing
architectural vocabulary in the parent spec: an effect boundary, as in
"`Installer` is the only port". Two of this set's filenames use the migration
sense, which is why the distinction is worth stating once. The tmux spec's
own table column already says "Converts?".

**Date:** 2026-09-07
**Status:** design, not yet planned
**Parent spec:** `docs/superpowers/specs/2026-09-06-pure-core-architecture.md`
(step 6 of section 7.4, and section 7.5)
**Depends on:** nothing for tranche A. Tranches B and C should follow the
subjects they test.

## 1. Why this is last, and why one tranche is not

Parent spec 7.4: "Step 6. The test port. **Last**, because it is the safety
net for everything above it."

That is right for the suites that test the code being ported: converting the
net before the thing it catches is backwards. It is **not** right for the
suites that test tracked *files* rather than ported code. Those depend on
nothing, and one of them has a verified defect behind it today.

So this document splits step 6 by dependency rather than treating it as one
block.

## 2. Current state, measured

41 shell suites (43 in the parent spec, minus two the branch collapse
deleted). 130 Rust tests exist, all covering new `deps-core` and
`dotfiles-path` code. **Zero suites have been converted.**

`tests/lib.sh` and `tests/run-all.sh` are deleted **last**, and only once the
Rust suite has run green alongside them for a while. Converting a reversible
migration into an irreversible one at the moment of the swap buys nothing.

## 3. The three tranches

### Tranche A: real parsers instead of regex (goes first, independently)

These suites parse **structured formats** with `sed`, `grep -oE` and `awk`.
In Rust they get `serde_yaml`, a TOML parser, and a Markdown parser. Measured
by which suites read which format:

| Format | Suites that parse it |
|---|---|
| YAML (`.github/workflows/`) | `bootstrap-harness`, `check-deps`, `container`, `deps-harness`, `readme-badges`, `scripts-dir-name`, `shellcheck` |
| Markdown | `config-docs`, `doc-links`, `deps-docs`, `readme-badges`, `setup`, `container`, `deps-harness`, `leak-check`, `pre-push-multi-ref`, `scripts-dir-name` |
| TOML | `alacritty-platform-split`, `config-manifest-lifecycle`, `container`, `doc-links`, `platform`, `pre-push-multi-ref`, `run-all-filter` |

**The three tranches OVERLAP. They are not a partition, and an earlier
draft implied they were.** The table above names 17 distinct suites, and
17 + 7 + 26 = 50 against a total of 41. The excess is real overlap rather
than an error in the totals: a suite can both parse a structured format
(tranche A) and need `assert_cmd` fixtures (tranche C), and
`container.test.sh` parses all three formats by itself.

So read the tranches as **work streams, not buckets**. The partition that
does sum is the one in section 6: 7 suites keep shell as their subject, 34
become Rust. Tranche A names where a real parser is the payoff. Tranche C
names where the harness is the only change. A suite in both gets its parser
work in A and its fixture work in C.

**The bug class this closes is verified, not theoretical.** Parent spec 7.5's
example, reproduced by execution: `config-docs.test.sh:47-53` extracts
subcommand names with

```sh
sed -n 's/^- `config \([a-z-]*\)`.*/\1/p'
```

Change the bullet marker from `- ` to `* `, which is the edit a Markdown
linter makes, and it yields **zero** extracted names, **zero** loop
iterations, and `assert_equals '' ''` **passes**. The pattern also hardcodes
the backticks and `[a-z-]*`, so a subcommand name containing a digit silently
drops out.

Four more instances of that same shape were found and fixed during the
previous plan, three of them in `deps-docs.test.sh` alone: an exit-127 oracle
read as acceptance, a parser harvest over an absent file, and a `grep` handed
a file's *contents* where a path belongs. A real parser makes all four
unrepresentable rather than merely fixed.

**This tranche pays for itself and blocks on nothing. It should start before
steps 3b, 4 and 5, not after them.**

### Tranche B: shell stays the subject (7 suites)

The six `zshrc-*` suites plus `zsh-git-widgets.test.sh`. Rust *drives* them;
the thing under test stays shell, because `.zshrc` is the shell's own
configuration and a ZLE widget must run in-process.

This is honest rather than a gap, and it means "migrate the majority of shell
tests to Rust" resolves to: the harness becomes Rust, the subject does not.

`zshrc-platform-split.test.sh` needs **re-derivation, not a port**: two of
its contracts assert cross-branch properties ("both variants ship on both
branches so neither can drift unseen") that the branch collapse made
vacuous. One branch cannot drift from itself, so the guarantee holds
trivially and the test asserts a mechanism that no longer exists.

### Tranche C: equivalent, with better fixtures (26 suites)

`assert_cmd` plus `tempfile` replaces the `tests/lib.sh` harness. The payoff
is thin per suite, so this goes last and incrementally.

**One claim withdrawn from the parent spec's first draft, recorded so it is
not used as justification again.** It said `tempfile` "fixes the
fixture-ownership defect where cleanup kills tmux sessions by name pattern on
a shared server." The pattern-kill at `lib.sh:77-79` exists but is
**PID-scoped**: names are `TEST_NAME-$$-suffix` and the grep is
`^${TEST_NAME}-$$-`, with a comment stating the PID is there precisely so
concurrent runs cannot collide. Cross-kill would need the same test file
*and* the same PID on the same server. So that defect is unreachable, and
tranche C rests on consistency alone.

## 4. What the port must preserve

- **The `skip` mechanism.** `tests/lib.sh` distinguishes a skipped assertion
  from a passing one, and `run-all.sh` reports the count (currently 49
  skipped). A port that turns skips into passes hides platform-gated
  coverage. `#[ignore]` is not equivalent: it hides the count.
- **Positive controls.** The previous plan's Global Constraints require every
  empty-expected assertion to assert first that its pipeline produced
  something. That rule survives the port and is easier to hold in Rust,
  where an empty `Vec` and a failed command are different types.
- **The container leg.** `tests/run-in-docker.sh` runs the suite inside
  `debian:bookworm-slim`, which is where three Docker-only defects were
  caught that the host missed, including two shellcheck findings the host's
  newer version does not report. A Rust suite must still run there, which
  means the test image needs the toolchain the deps images deliberately lack.
- **`cargo test` already runs the whole workspace.** Fixed in `fc33e5ac`:
  `run-all.sh` previously pointed `--manifest-path` at one crate, so
  `dotfiles-path`'s tests were outside the suite from the day it landed.
  Measured 4 test binaries before, 6 after.

## 5. Order

1. **Tranche A**, starting with `config-docs.test.sh` because its defect is
   the reproduced one. Then the YAML suites, since `serde_yaml` covers seven
   at once.
2. **Tranche B** after step 5, so the widget port and its test move together.
3. **Tranche C** incrementally, after the subject of each suite has settled.
4. **Delete `lib.sh` and `run-all.sh`** only after both suites have run green
   side by side across several pushes.

## 6. Honest scope statement

Of 41 suites: **7 keep shell as their subject permanently**, and the
remaining 34 become Rust. So the answer to "are we migrating the majority to
Rust" is yes, 34 of 41, but the 7 that stay are staying for a mechanism
reason and not as unfinished work.
