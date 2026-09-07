# Branch-Collapse Residue and the `deps-core` Port Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the branch-collapse residue that leaves a fresh bootstrap
checking out a frozen branch, then port `check-deps.sh` to the pure-core
`deps-core` crate behind the architecture the spec describes.

**Architecture:** Task 1 fixes a live bootstrap defect and the three gates
that failed to catch it. Tasks 2 to 4 add `--describe` to the dispatcher
(spec 7.4 step 1), which every later port depends on for its help text.
Tasks 5 to 12 build `deps-core` as a pure core with one `Installer` port,
convert the manifest to data, implement the fixpoint loop, and land the
18-consumer rename atomically.

**Tech Stack:** POSIX sh (`setup.sh`, `.scripts/config/*`, `.scripts/deps/*`),
bash (`tests/*.test.sh`), Rust 2024 edition, Cargo workspace, Docker.

**Spec:** `docs/superpowers/specs/2026-09-06-pure-core-architecture.md`
(sections 3.3, 3.5, 4, 5.1 to 5.5, 6.1 to 6.3, 7.1, 7.3, 7.4 steps 1 and 3,
7.5a, 7.6, plus the 7.6a this plan's Task 1 adds)

**Predecessors:** `docs/superpowers/plans/2026-09-06-blockers-and-workspace-foundation.md`
is complete: the four blockers are closed, the Cargo workspace exists,
per-crate stamps are in place, and `dotfiles-path` ships with `CheckRelPath`.
This plan starts from `main` at that plan's final commit.

**Scope:** spec 7.4 step 1 and step 3, plus the 7.6 residue. Steps 4, 5 and 6
(remaining `config-*` subcommands, the tmux scripts, the 43-suite test port)
are deliberately excluded and are planned separately; see "What this plan
does not cover" below.

---

## Global Constraints

Copied verbatim from the predecessor plan, because every one still applies
and a paraphrase is how a constraint gets lost.

- No em dashes anywhere. Use a comma, a colon, parentheses, or two hyphens.
- No emoji anywhere.
- No single-letter variable names, except numeric loop indices (`i`, `j`, `k`)
  and math or geometry values. Lambda and closure parameters get no
  exception.
- Comment why, not what. No comment that restates the code.
- Never `--no-verify`. Never disable a test instead of fixing it.
- Pure core, IO at the edges, dependency-injectable. `deps-core` must contain
  **zero** references to `std::fs`, `std::process`, `std::env`, `std::io`, or
  `Command::new`. Verify with a grep in the task that creates each module,
  the way `config-manifest`'s pure modules already hold that line.
- `deps-core` depends on `dotfiles-path` only. **No edge to
  `config-manifest`**: that would make the dependency-manifest domain depend
  on the git-sync domain and transitively carry a 595-line `git` module it
  never calls.
- A pure Rust module earns its place only if the Rust binary is the sole
  producer of that value.
- In shell: a function that performs IO must not also decide. It returns its
  result to the caller and the caller decides.
- Tests inject through existing env seams (`LEAK_PATTERN_FILE`,
  `LEAK_ALLOW_FILE`, `DEPS_CONF`, `DEPS_LOCAL_CONF`, `DOTFILES_ROOT`,
  `CONFIG_BIN_DIR`, `CARGO_TARGET_DIR`, `DEPS_FORCE_ROOT`). Never read
  `~/.claude/local/` from a test.
- **Every empty-expected assertion needs a positive control.** An
  `assert_equals 'no X' '' "$(cmd)"` passes when `cmd` breaks for an
  unrelated reason, so assert first that the pipeline produced something,
  then assert the narrow property.
- **A fixture built to make the subject pass is not a gate.** Three instances
  of this shape are already fixed on this branch (Task 1's four satisfied
  fixtures, Task 1a's dead stamp gate, Task 12's exit-127 oracle). When a
  task deletes a code path, it deletes that path's fixture in the same
  commit.
- `tests/leak-check.sh` is `#!/bin/zsh`: arrays and `${(f)}` are available.
  `status` is a **read-only** variable, and `${pipestatus[1]}` does not
  survive command substitution.
- Rust: no `unwrap()` or `expect()` in non-test code without a proven
  invariant.

---

## Task 1: The pre-push stamp gate is dead on every push

**This is the most serious finding in the plan and it is not a documentation
problem.** Found by audit while the plan was being written, and verified by
execution rather than by reading.

`tests/pre-push:56-58`:

```sh
case $local_ref in
    refs/heads/mac|refs/heads/linux) pushing_synced_branch=1 ;;
esac
```

The entire stamp verification block at `tests/pre-push:156-186` is guarded on
`if [ "$pushing_synced_branch" -eq 1 ]`. Pushing `main` matches neither arm,
so the flag stays 0 and `config-manifest verify-stamps` never runs.

**Verified two ways.** First, the hook's own logic against a `main` ref line:

```
pushing main -> pushing_synced_branch=0
STAMP GATE SKIPPED
```

Second, and more convincingly, a real `config push --dry-run origin main`
(git runs `pre-push` on a dry run; it skips only the network send). Its
output:

```
pre-push: leak scan passed for 1 range(s)
pre-push: pushed changes touch tested code, running the suite in Docker
```

The leak scan reports and the Docker suite starts. **`pre-push: stamp gate
passed` never appears**, and `tests/pre-push:184` prints that line
unconditionally on the gated path. The absence is the defect, visible in an
ordinary push.

So since the collapse, every push has skipped the gate whose whole purpose is
refusing a push whose installed binary does not match its source. No test
failed, because the only suite that drives this path
(`tests/pre-push-multi-ref.test.sh`) feeds the hook `refs/heads/mac` and
`refs/heads/linux` literals -- it is the sole reason the dead branch still
looks alive.

**No damage occurred.** Checked at the time of writing: the installed
`config-manifest` stamp matches `config-stamp`'s expected value exactly, and
`config doctor` exits 0 silently. That is a fact about luck, not about the
gate. It is also the concrete argument for keeping `config doctor`: it is the
only way to answer "did anything slip through" once the push-time gate is
known to have been off.

**Files:**
- Modify: `tests/pre-push` (~56-58)
- Modify: `tests/pre-push-multi-ref.test.sh` (premise at ~5-16, fixtures at
  ~39-59, ref lines at ~120-124 and ~146-164; the per-crate stamp-scope block
  at ~166-213 is branch-independent and stays)

**Interfaces:**
- Consumes: nothing. Runs first, since the tasks after it push and should push through a live gate and should
  push through a live gate.
- Produces: `pushing_synced_branch` is replaced. Any later task that adds a
  ref-name case to `pre-push` must not reintroduce a frozen branch name.

- [ ] **Step 1: Write the failing test**

The variable name encodes the retired model, so it goes too. What the gate
actually wants is "this push updates a branch whose binaries ship", which is
every branch now. Add to `tests/pre-push-multi-ref.test.sh`:

The hook already prints an observable trace, so this needs no new seam.
`tests/pre-push:184` emits `pre-push: stamp gate passed` on the success path,
and the file's own comment at ~187-189 explains why the skip path reports too:
"A hook that says nothing when it skips is indistinguishable from a hook that
is not installed at all." That is exactly the property that failed here, so
assert on the existing message:

```sh
# The gate must run for main. This is the assertion whose absence let the
# collapse silently disable stamp verification: the old case arm matched
# refs/heads/mac and refs/heads/linux, so pushing main skipped the block
# entirely and no suite noticed, because this file only ever fed the hook
# the two frozen branch names.
output=$(printf 'refs/heads/main %s refs/heads/main %s\n' "$head_sha" "$head_sha" \
    | "$HOOK" origin "$repo" 2>&1 || true)

assert_succeeds 'the hook produced output for a main push' test -n "$output"
assert_succeeds 'pushing main reaches the stamp gate' \
    sh -c 'printf "%s\n" "$1" | grep -q "stamp gate passed"' _ "$output"
```

The positive control is load-bearing: without it, a hook that dies before
reaching either branch produces empty output, and `grep -q` on empty input
would be the only thing failing, which reads as the gate being absent rather
than the hook being broken.

- [ ] **Step 2: Run the test to verify it fails**

Run: `bash tests/pre-push-multi-ref.test.sh`
Expected: FAIL. `pushing main reaches the stamp gate` fails because the case
arm does not match `refs/heads/main`.

- [ ] **Step 3: Fix the gate**

Replace the case block. The gate should key on "a branch ref is being
pushed", not on an enumerated branch name, so that a new branch cannot
silently opt out of stamp verification the way `main` did:

```sh
    # Any branch push gates on stamps. This used to enumerate
    # refs/heads/mac and refs/heads/linux, so the 2026-09-06 collapse to
    # main silently disabled the whole block at line 156 and no test
    # failed: the only suite driving this path fed the hook those two
    # literals. Keying on the ref shape rather than a name means a new
    # branch cannot opt out by not being on a list.
    case $local_ref in
        refs/heads/*) pushing_branch=1 ;;
    esac
```

Rename `pushing_synced_branch` to `pushing_branch` at every site, including
the guard at line 156. "Synced" described the two-branch model.

- [ ] **Step 4: Rewrite the multi-ref suite's premise**

Its stated premise (~5-16) is the atomic two-branch push. Rewrite the header
comment to say what the file now covers: multiple refs in one push, and the
per-crate stamp scope. Change the `mac`/`linux` fixtures to `main` plus a
feature branch, which is the real multi-ref case that remains (pushing a
branch and a tag, or two branches, in one invocation).

Keep the per-crate stamp-scope block at ~166-213 unchanged: it asserts that a
crate with no `src/main.rs` is dropped from both sides, which has nothing to
do with branch names.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `bash tests/pre-push-multi-ref.test.sh`
Expected: PASS.

Run: `tests/run-all.sh`
Expected: PASS.

- [ ] **Step 6: Verify the gate actually refuses a stale binary**

The gate has provably not run in weeks, so "the test passes" is not enough.
Prove it refuses:

`config push --dry-run` runs the hook (verified: git skips only the network
send), but it also triggers the Docker suite, which takes minutes. Drive the
hook directly instead, which is what the multi-ref suite already does:

```sh
cd ~ && cp "$HOME/.local/bin/config-manifest" /tmp/config-manifest.good

# A binary whose stamp cannot match its source.
printf '#!/bin/sh\necho stale-stamp\n' > "$HOME/.local/bin/config-manifest"
chmod 755 "$HOME/.local/bin/config-manifest"

head_sha=$(config rev-parse HEAD)
printf 'refs/heads/main %s refs/heads/main %s\n' "$head_sha" "$head_sha" \
    | tests/pre-push origin "$HOME/.cfg" 2>&1
echo "sabotaged exit=$?"    # must be non-zero

cp /tmp/config-manifest.good "$HOME/.local/bin/config-manifest"
rm /tmp/config-manifest.good
printf 'refs/heads/main %s refs/heads/main %s\n' "$head_sha" "$head_sha" \
    | tests/pre-push origin "$HOME/.cfg" 2>&1
echo "restored exit=$?"     # must be 0, and must print "stamp gate passed"
```

**There is no suite-skip seam**, verified: `tests/pre-push` has only the
`TRIGGER_PATHS` test at ~193 and `DOTFILES_TEST_REF` at ~218, neither of
which disables the run. So do not write `SKIP_DOTFILES_SUITE` above. Two
workable options, in preference order:

1. Point the hook at a scratch repository whose pushed paths match no
   `TRIGGER_PATHS` entry, so it takes the "no pushed path touches tested
   code, skipping the suite" branch at ~193 and still reaches the stamp
   block. `tests/pre-push-multi-ref.test.sh` already builds such a fixture
   with `make_repo`; reuse it.
2. Accept the Docker run once, since this is a one-time verification.

Do not add an env seam to the production hook for a check that runs once.

Record both exit codes and whether the restored run printed
`pre-push: stamp gate passed`. If the sabotaged run succeeds, the gate is
still not running and Step 3 is wrong. **This verification is not optional
here:** the gate provably has not run since the collapse, so "the test
passes" only shows the test agrees with the code, not that the gate refuses
anything.

- [ ] **Step 7: Commit**

```bash
config add tests/pre-push tests/pre-push-multi-ref.test.sh
config commit -m "Run the stamp gate for every branch, not two frozen names

tests/pre-push matched refs/heads/mac and refs/heads/linux to set
pushing_synced_branch, and the whole stamp verification block was guarded on
that flag. The 2026-09-06 collapse to main therefore disabled stamp
verification on every push, and no test failed: pre-push-multi-ref.test.sh
is the only suite that drives this path and it feeds the hook those two
literals, which is what kept the dead branch looking alive.

Verified by running the hook's own logic against a main ref line:
pushing_synced_branch=0, gate skipped. No stale binary actually shipped --
the installed stamp matches config-stamp and config doctor is silent -- but
that is luck, not the gate working.

Keys on the ref shape now rather than an enumerated name, so a new branch
cannot opt out of stamp verification by not being on a list.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: The bootstrap still checks out a frozen branch

Spec 7.6 collapsed `mac` and `linux` into `main` and enumerated what that
deletes (`config check`, `run_sync`, `.sync-manifest`) and what must be
re-derived (`check-branch-drift.test.sh`, two `zshrc-platform-split.test.sh`
contracts). **It says nothing about `setup.sh`, and `setup.sh` is what a new
machine runs.** This task closes that gap and adds the missing spec section.

### The defect

`setup.sh:316-367` derives a platform from `uname -s`, then:

```sh
branch=$platform
```

So a fresh Darwin machine checks out `mac` and a fresh Linux machine checks
out `linux`. Both refs are frozen history that receive no commits. The
documented one-liner compounds it -- `README.md:13` fetches
`.../dotfiles/mac/setup.sh`, so even the script doing the checkout is the
pre-collapse copy.

Measured on the live repo at the time of writing: `refs/heads/mac` is **31
commits ahead of `origin/mac`**, and `origin/main` carries 40+ commits
neither has. A bootstrap today lands on a tree missing the entire collapse.

### Why four gates missed it

Each one is satisfied by a fixture that encodes the two-branch model:

| Gate | Line | Why it passes anyway |
|---|---|---|
| `tests/setup.test.sh` | 409-411 | Asserts `refs/heads/$readme_branch` exists. A stale **local** `mac` branch satisfies it, and the assertion never compares that ref to the remote or to `main`. |
| `tests/setup.test.sh` | 415-421 | Compares `mac:setup.sh` to `linux:setup.sh`. Both frozen, so the blobs agree forever. Skips when either is absent. |
| `.scripts/deps/test-bootstrap.sh` | 94-98 | Seed repo **creates** `mac` and `linux` (`for platform_branch in mac linux`), manufacturing the branches the stale detection needs. |
| `.github/workflows/deps-check.yml` | 125, 137, 188, 197 | `ref="${GITHUB_REF_NAME:-mac}"` defaults to `mac`, then creates both platform branches in the seed for the reason the comment states: "setup.sh detects `linux`". |

This is the failure shape the workspace-foundation plan hit twice: a gate that
passes because its environment was built not to fail it. The fix has to
change the fixtures, not only the script -- otherwise the corrected
`setup.sh` keeps passing against seeds that still carry `mac` and `linux`.

**Files:**
- Modify: `setup.sh` (branch selection, ~316-367; usage comment line 14)
- Modify: `README.md:13` (bootstrap URL)
- Modify: `tests/setup.test.sh` (~390-435)
- Modify: `.scripts/deps/test-bootstrap.sh` (~88-98)
- Modify: `.github/workflows/deps-check.yml` (~125, 137, 188, 197)
- Modify: `docs/superpowers/specs/2026-09-06-pure-core-architecture.md` (add 7.6a)
- Modify: `TODO-AGENTS.md` (~287, 395, 428: drop the `.sync-manifest` invariant
  and the `branch-drift.yml` message-text entries)

**Interfaces:**
- Consumes: nothing from earlier tasks; this is first.
- Produces: `setup.sh` no longer has a platform-to-branch mapping. The
  `--branch` flag survives as the only branch selector. Later tasks that
  touch `setup.sh` must not reintroduce `uname`-derived branch names.

- [ ] **Step 1: Write the failing tests**

Replace the branch-existence block in `tests/setup.test.sh` (lines ~407-427,
from `if [ -d "$DOTFILES_ROOT/.cfg" ]; then` through its `fi`) with
assertions that pin the ref to the branch that actually receives commits.
The positive control matters here: `readme_branch` must be non-empty before
the narrow assertion runs, or a README edit that breaks the `sed` makes every
assertion below it pass vacuously.

```sh
assert_equals 'the README bootstrap URL names main' 'main' "$readme_branch"

if [ -d "$DOTFILES_ROOT/.cfg" ]; then
    assert_succeeds 'main is a real branch here' \
        git --git-dir="$DOTFILES_ROOT/.cfg" rev-parse --verify --quiet refs/heads/main
    assert_succeeds 'main carries setup.sh' \
        git --git-dir="$DOTFILES_ROOT/.cfg" cat-file -e main:setup.sh

    # The frozen branches must not be reachable as a bootstrap target. This
    # is the assertion whose absence let the collapse ship half-done: it
    # fails while setup.sh still maps a platform to a branch name, and it
    # cannot be satisfied by a stale local ref.
    assert_equals 'setup.sh maps no platform to a branch name' '' \
        "$(grep -n 'branch=\$platform' "$DOTFILES_ROOT/setup.sh" || true)"
else
    skip 'main is a real branch here' 'no repository in this environment'
    skip 'main carries setup.sh' 'no repository in this environment'
fi
```

Delete the `mac_blob`/`linux_blob` comparison and its `skip` arm entirely: it
compares two frozen refs, so it can never fail again and it documents a
guarantee that no longer exists.

Then add a test that the detected platform still reaches `config init`,
because removing the branch mapping must not remove platform detection --
`.zshrc-mac` versus `.zshrc-linux` selection still needs it:

```sh
# Platform detection survives; only the branch mapping goes. A machine that
# detects mac must still get the mac variant, which is a runtime file
# selection rather than a branch checkout.
HOME="$home" DOTFILES_PLATFORM=mac "$SETUP" --yes --repo "$seed" >/dev/null 2>&1
assert_equals 'a mac machine checks out the single branch' \
    'main branch marker' "$(cat "$home/.marker" 2>/dev/null)"
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `bash tests/setup.test.sh`
Expected: FAIL. `the README bootstrap URL names main` fails on `mac`;
`setup.sh maps no platform to a branch name` fails with the matched line
number; `a mac machine checks out the single branch` fails because the seed
still serves `mac`.

Record which assertions fail and why. An assertion that fails for a fixture
reason rather than the real defect is not a red test.

- [ ] **Step 3: Fix `setup.sh`**

Replace the whole `if [ -z "$branch" ]; then ... fi` block (~321-367) with a
default plus the surviving override. Platform detection is **not** deleted;
it moves to where it is actually used, which is `config init`'s variant
selection, and stops deciding a branch name.

```sh
# One branch. Platform differences are per-platform FILES selected at
# runtime (.zshrc-mac, tmux-mac.conf, deps-mac.conf), so nothing about this
# machine's OS implies a ref. --branch remains the only branch selector: it
# is how a reader reaches `work` or `home`, which no amount of uname implies.
#
# This used to read `branch=$platform`, mapping Darwin to `mac` and Linux to
# `linux`. Those branches are frozen history as of the 2026-09-06 collapse,
# so that mapping bootstrapped every new machine onto a stale tree.
[ -n "$branch" ] || branch=main
```

Update the usage comment at line 14 to the `main` URL. Confirm no other
`$platform` reader in `setup.sh` depended on the variable the deleted block
set -- if one does, it reads `DOTFILES_PLATFORM` or calls `platform.sh`
directly instead.

- [ ] **Step 4: Fix `README.md:13`**

```
curl -fsSL https://raw.githubusercontent.com/austintheriot/dotfiles/main/setup.sh | sh
```

The `url_count` assertion at `tests/setup.test.sh:432` must still see exactly
one such URL; do not add a second.

- [ ] **Step 5: Fix the two seed builders**

Both manufacture the frozen branches. In `.scripts/deps/test-bootstrap.sh`,
delete the `for platform_branch in mac linux` loop (~94-98) and rewrite the
comment above it, which currently explains why both branches are needed:

```sh
# One branch. This used to create both `mac` and `linux` because setup.sh
# detected a platform and checked out the matching branch, so a seed built
# only on the host's branch made a Linux container fail with "Not a valid
# object name linux". setup.sh now defaults to main, so the seed needs one
# branch and creating the frozen pair would hide a regression of that.
```

In `.github/workflows/deps-check.yml`, apply the same deletion at both sites
(~137 and ~197) and change both `ref="${GITHUB_REF_NAME:-mac}"` defaults to
`main`. Keep `GITHUB_REF_NAME` as the primary: a PR from a feature branch
should seed that branch, not `main`.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `bash tests/setup.test.sh`
Expected: PASS, with no `skip` for the branch assertions when run from a
machine that has `.cfg`.

Then the two suites that drive the seed builders:

Run: `bash tests/bootstrap-harness.test.sh`
Expected: PASS.

Run: `tests/run-all.sh`
Expected: PASS. `setup.test.sh`, `bootstrap-harness.test.sh`,
`deps-harness.test.sh` and `readme-badges.test.sh` all read files this task
edits.

- [ ] **Step 7: Verify the gate now fails on the original defect**

The point of Step 1's third assertion is that it could not have passed
before. Prove it rather than assuming it:

```sh
cd ~ && cp setup.sh /tmp/setup.sh.bak
# Reintroduce the defect exactly as it was.
sed -i '' 's/^\[ -n "\$branch" \] || branch=main$/branch=$platform/' setup.sh
bash tests/setup.test.sh; echo "exit=$?"   # must be non-zero
cp /tmp/setup.sh.bak setup.sh && rm /tmp/setup.sh.bak
bash tests/setup.test.sh; echo "exit=$?"   # must be 0
```

Record both exit codes in the report. If the sabotaged run passes, the
assertion is not testing what it claims and Step 1 needs rework.

- [ ] **Step 8: Add spec section 7.6a**

7.6 enumerated the collapse's consequences and missed this one. Insert 7.6a
directly after 7.6 (before `## 8. Consequences for existing decisions`), so
the spec records the gap rather than leaving the plan as the only account:

```markdown
### 7.6a What the collapse missed: the bootstrap path

7.6 enumerated the collapse's deletions and re-derivations from the
*maintainer's* side and missed the *new machine's* side entirely.
`setup.sh` derived a branch name from `uname -s` (`branch=$platform`), so
every fresh bootstrap checked out `mac` or `linux` after both became frozen
history, and `README.md`'s documented one-liner fetched `setup.sh` itself
from `mac`.

Four gates covered this path and none failed, each because its fixture
encodes the two-branch model: `setup.test.sh` asserted the README's branch
exists (a stale **local** `mac` ref satisfies that, and the live repo's was
31 commits ahead of `origin/mac`); `setup.test.sh` also compared
`mac:setup.sh` to `linux:setup.sh`, two frozen refs that agree forever;
`test-bootstrap.sh` and `deps-check.yml` both **create** `mac` and `linux`
in their seed repositories, manufacturing exactly the branches the stale
detection required.

**The correction.** `setup.sh` defaults to `main` and keeps `--branch` as
the only selector, because `work` and `home` are real refs that no amount
of `uname` implies. Platform detection stays -- it selects per-platform
files at runtime -- but no longer names a ref. Both seed builders create one
branch, so a regression of the mapping fails instead of being accommodated.

**The general lesson, which is this repo's established bug class.** A gate
whose fixture is built to make the subject pass is not a gate. 7.5 cites the
same shape in `deps-docs.test.sh` (exit 127 read as "flag accepted") and the
workspace-foundation plan hit it twice more. When a fixture exists to satisfy
a code path, deleting the code path must delete the fixture in the same
commit.
```

- [ ] **Step 9: Clean the remaining dead references**

`TODO-AGENTS.md` lines ~287, 395, 428 describe `branch-drift.yml` grepping
message text and the `.sync-manifest` every-file-matches-a-rule invariant.
Both subsystems are deleted. Remove those entries.

Leave alone, deliberately: `DOTFILES.md`'s `config checkout` (git's real
verb through the dispatcher, not the deleted subcommand), everything under
`docs/research/`, and `docs/superpowers/plans/2026-09-04-*` (historical
records of decisions correct when made; rewriting them falsifies the
archive). `crates/config-manifest/src/stamp.rs:14` already says the
branch-drift renderer is gone and reads correctly as history.

- [ ] **Step 10: Commit**

```bash
config add setup.sh README.md tests/setup.test.sh \
    .scripts/deps/test-bootstrap.sh .github/workflows/deps-check.yml \
    docs/superpowers/specs/2026-09-06-pure-core-architecture.md TODO-AGENTS.md
config commit -m "Bootstrap onto main instead of the frozen platform branches

setup.sh derived a branch from uname (branch=\$platform), so every fresh
machine checked out mac or linux after the 2026-09-06 collapse froze both.
README's one-liner fetched setup.sh from mac as well, so even the script
doing the checkout was the pre-collapse copy. Measured: refs/heads/mac was
31 commits ahead of origin/mac, and origin/main carried 40+ neither had.

Four gates covered this path and none failed. setup.test.sh asserted the
README's branch exists, which a stale local ref satisfies; it also compared
mac:setup.sh to linux:setup.sh, two frozen refs that agree forever; and
test-bootstrap.sh plus deps-check.yml both created mac and linux in their
seeds, manufacturing the branches the stale detection needed. Fixing the
script alone would have left it passing against those fixtures, so the
fixtures change too.

Platform detection stays: it selects .zshrc-mac over .zshrc-linux at
runtime. It no longer names a ref. --branch remains the only selector,
since work and home are real branches no uname implies.

Spec 7.6a records the gap and the general shape, which is this repo's
established bug class: a fixture built to make the subject pass is not a
gate.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

## Task 3: The rest of the collapse residue

Audit findings that are real but lower severity than 1a and 1. Grouped into
one task because they are the same mechanical edit repeated, and one reviewer
gate over the batch is the right granularity.

**Files:**
- Modify: `tests/setup.test.sh` (`make_seed` at ~25-51, and the two
  branch-selection assertions at ~384-392)
- Modify: `SETUP.md` (~12-13, the "One URL, both platforms" section at ~27-36,
  ~47, ~55)
- Modify: `README.md` (~17, ~21-23, which contradict ~50-53 in the same file)
- Modify: `setup.sh` usage block (~24-26)
- Modify: `.scripts/deps/test-bootstrap.sh:34`, `.scripts/deps/test-local.sh:33`,
  `tests/run-in-docker.sh:42-45` and `:77` (operator messages naming frozen
  branches)
- Modify: `.github/workflows/deps-check.yml` (~257, ~325: two MORE
  `${GITHUB_REF_NAME:-mac}` sites and branch-manufacturing loops beyond the
  two Task 1 covers -- there are four jobs, not two)
- Modify: `tests/config-manifest-lifecycle.test.sh` (~103-111: the `grep -v`
  exempts `check-branch-drift.test.sh`, a file the collapse deleted)
- Modify: `.github/workflows/test-suite.yml:3` (comment says "both platforms
  this repo is branched for")
- Modify: `.github/workflows/deps-check.yml:16-19` (add `branches: [main]`
  for symmetry with `test-suite.yml`; it currently fires on any branch)
- Delete: `docs/porting-a-fix-across-branches.md` (a 3.7 KB live runbook for
  the retired cross-branch workflow. Verified unreferenced: nothing in the
  repo greps for `porting-a-fix`, and it is not in `doc-links.test.sh`'s doc
  list, so no test would notice it rotting.)
- Delete: `crates/config-manifest/proptest-regressions/plan.txt`, and remove
  the `proptest` dev-dependency from `crates/config-manifest/Cargo.toml:12`
  and `crates/Cargo.toml:20`. **Verified: `proptest` appears in zero source
  or test files under `crates/config-manifest/`.** The saved failure seeds
  belong to the deleted check/sync tree planner, and the dependency is
  declared but unused.

**Interfaces:**
- Consumes: Task 2's `setup.sh` default of `main`. `make_seed` must build the
  branch Task 1's `setup.sh` checks out, so these two are coupled: if Task 1
  runs first, its suite is red until this task fixes the fixtures. Run this task's
  `make_seed` edit **inside** Task 2 if the reviewer prefers one green step.
- Produces: no frozen branch name remains in any live file.

- [ ] **Step 1: Fix `make_seed` and delete the assertions that defend the defect**

`tests/setup.test.sh:25-51` builds every fixture with `mac` and `linux` and
no `main`, so all ~13 fixtures encode the two-branch world. Add `main` as the
initial branch. Then delete the two assertions at ~384-392:

```sh
assert_equals 'a linux machine checks out linux regardless of the fetch branch' ...
assert_equals 'a mac machine checks out mac regardless of the fetch branch' ...
```

These assert that the defect is correct behavior. Task 2 replaces them with
the platform-detection-survives assertion.

- [ ] **Step 2: Run the suite**

Run: `bash tests/setup.test.sh`
Expected: PASS, together with Task 2's `setup.sh` change.

- [ ] **Step 3: Fix the docs that describe the two-branch model as current**

`SETUP.md:27-36` is a second, undocumented copy of the `README.md:13` defect:
it heads a section "One URL, both platforms", serves the `mac` URL, and cites
the now-vacuous `setup.sh is the same blob on mac and linux` test as its
evidence. Rewrite the section for one branch, and point the URL at `main`.

`README.md:21-23` directly contradicts `README.md:50-53` in the same file:
the first says `setup.sh` "reads `uname` and checks out the matching branch",
the second correctly says "Everything lives on `main`." Delete the first.

`setup.sh:24-26`'s usage text describes `--branch` as an override for "the
detected branch". There is no detected branch; it overrides `main`.

- [ ] **Step 4: Fix the operator messages and the remaining CI sites**

Three scripts tell an operator to check out a frozen branch:
`test-bootstrap.sh:34`, `test-local.sh:33`, `run-in-docker.sh:77`. Each
should name `main` or `$DOTFILES_TEST_REF`. `run-in-docker.sh:42-45`'s
comment describes pushing `linux` from a worktree as "how this repo normally
ships a linux change", which is the retired workflow.

`deps-check.yml` has **four** jobs with `${GITHUB_REF_NAME:-mac}` and a
branch-manufacturing loop, not the two Task 2 addresses. Fix ~257 and ~325
the same way.

- [ ] **Step 5: Delete the orphaned artifacts**

```bash
config rm docs/porting-a-fix-across-branches.md
config rm crates/config-manifest/proptest-regressions/plan.txt
```

Then remove `proptest = { workspace = true }` from
`crates/config-manifest/Cargo.toml` and `proptest = "1.11.0"` from
`crates/Cargo.toml`'s `[workspace.dependencies]`.

Run: `cd ~/crates && cargo build --release --locked && cargo test --locked`
Expected: PASS. `Cargo.lock` changes; commit it.

- [ ] **Step 6: Sweep the rationale comments**

Roughly 25 files carry a comment saying some variant of "ships on both
branches so the drift check covers it". The **assertions** are still correct
(both variant files must ship); only the stated reason is gone. Files:
`.config/alacritty/alacritty.toml:5`, `alacritty-mac.toml:2`,
`alacritty-linux.toml:2`, `.config/tmux/tmux.conf:6`, `tmux-mac.conf:2`,
`tmux-linux.conf:2`, `.zshrc-mac:2`, `.zshrc-linux:2`,
`.scripts/deps/deps-linux.conf:3`, `.scripts/platform.sh:8,14`,
`README-MAC.md:4`, `README-LINUX.md:4` and `:34`, `tests/docker/Dockerfile:110`,
`tests/alacritty-platform-split.test.sh:11`, `tests/check-deps.test.sh:325,394`,
`tests/tmux-conf-split.test.sh:7,9,65-66`,
`tests/zshrc-platform-split.test.sh:14,33,36`, `tests/platform.test.sh:13,56`,
`tests/leak-check.test.sh:6` (cites `config sync` as a live caller of
`git commit-tree`).

Replace the reason with the runtime-variant mechanism, matching the wording
already committed to `.scripts/alacritty-platform.sh` and `.zshrc`. Do not
change any assertion in this step; if an assertion needs changing, it belongs
in its own task with its own red test.

Leave alone: `DOTFILES.md` (verbatim third-party bare-repo tutorial; its
"different branches for different computers" line is upstream prose, not a
claim about this repo), everything under `docs/research/`, and
`docs/superpowers/plans/2026-09-04-*`.

- [ ] **Step 7: Verify nothing live references a frozen branch**

```sh
cd ~ && config grep -rn -I -e 'refs/heads/mac' -e 'refs/heads/linux' \
    -e 'origin/mac' -e 'origin/linux' -- . \
    | grep -v -e '^docs/research/' -e '^docs/superpowers/plans/2026-09-0[46]' \
              -e '^\.claude/CLAUDE\.md'
```

Expected: no output. `.claude/CLAUDE.md:118` legitimately records that the
frozen branches remain on the remote as history.

Positive control first, so an empty result is not a broken pipeline:

```sh
config grep -rn -I 'refs/heads' -- . | head -3   # must print something
```

- [ ] **Step 8: Commit**

```bash
config add -A
config commit -m "Remove the branch-collapse residue the first pass missed

An audit after the setup.sh fix found the collapse left more behind than
spec 7.6 enumerated:

  - tests/setup.test.sh built every fixture with mac and linux and no main,
    and two assertions actively asserted the stale selection was correct.
  - SETUP.md carried a second undocumented copy of the frozen-branch
    bootstrap URL, citing the now-vacuous same-blob test as its evidence.
  - README.md:21-23 contradicted README.md:50-53 in the same file.
  - deps-check.yml had four jobs defaulting to mac, not the two already
    fixed.
  - Three scripts told an operator to check out a frozen branch.
  - docs/porting-a-fix-across-branches.md was a live runbook for the
    retired workflow, referenced by nothing and covered by no test.
  - proptest was declared in two manifests and used in zero source files;
    its saved regression seeds belong to the deleted tree planner.

The rationale comments across roughly 25 files said the drift check was why
both platform variants ship. The assertions were right and the reason was
gone, so only the reason changed.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: `--describe` in `usage.sh`, sourced from the `# help:` line

`config-help:38` reads each subcommand's one-line description with
`sed -n 's/^# help: //p' "$script" | head -1`, where `$script` is the
subcommand file itself. That works only while every subcommand is a text
file. I confirmed the spec's claim about the failure mode by running the same
`sed` against a real Mach-O binary at `~/.local/bin/config-manifest`: it
prints `sed: RE error: illegal byte sequence` to stderr and exits 0 through
the pipeline, because `head -1` is the last stage and supplies the pipeline
status. So the listing loses the description AND leaks a linter-style error
into `config help` output, non-fatally. Silent degradation, exactly as spec
7.3 states.

The fix is a `--describe` contract every subcommand answers for itself. All
nine shell subcommands already source the shared helper (verified: `grep -n
'usage\.sh' config-*` matches `config-build:32`, `config-doctor:23`,
`config-help:26`, `config-init:52`, `config-install:13`,
`config-install-hooks:24`, `config-reload:15`, `config-stamp:43`,
`config-test:23`), and eight of the nine call `usage_if_requested "${1:-}"`
on the next line. Adding `--describe` handling inside `usage_if_requested`
therefore gives eight subcommands the new verb with zero edits to any of
them. `config-help` is the ninth: it deliberately does not exit on `--help`
(`config-help:22-29` explains why), so it needs the one call site added by
hand.

The description string must stay in the `# help:` comment. `tests/config.test.sh:292-297`
asserts every subcommand carries a `# help:` line, and `README.md`'s "Repo
utilities" section tells a new contributor to add one. Reading the comment out
of `$0` at runtime keeps one home for the string while changing only how it is
retrieved. This task deliberately does NOT change `config-help`; the listing
still source-greps after this task. Task 3 flips the consumer.

**Files:**
- Modify: `.scripts/config/usage.sh` (add `print_describe`; extend
  `usage_if_requested` at lines 24-28)
- Modify: `.scripts/config/config-help` (add the `--describe` arm beside the
  existing `--help|-h` arm at lines 27-29)
- Modify: `tests/config-usage.test.sh` (new section; existing sections at
  lines 67-107 and 109-127 stay untouched)

**Interfaces:**
- Consumes: nothing.
- Produces: `config <sub> --describe` prints exactly one line to stdout and
  exits 0, for every one of the nine shell subcommands. The line is the text
  after `# help: ` on the subcommand's `# help:` line, with no leading spaces
  and exactly one trailing newline. Nothing is written to stderr. The command
  does no work beyond reading its own source. `--describe` takes precedence
  over nothing: it is checked in the same `case` as `--help`, so `--describe`
  and `--help` are independent spellings, not composable.

- [ ] **Step 1: Write the failing test**

Appended to `tests/config-usage.test.sh`, immediately before the final
`finish` (currently line 230). It reuses the `run_config` helper defined at
lines 63-65 and the `ALL_SUBCOMMANDS` list built at line 37.

```bash
# --- the --describe contract -------------------------------------------------

# config-help's listing used to read each subcommand's one-line description
# with `sed -n 's/^# help: //p'` over the subcommand's source text. Pointed at
# a compiled binary that sed prints "RE error: illegal byte sequence" to
# stderr and the pipeline still exits 0, because `head -1` supplies the
# status, so the description silently became empty and an error leaked into
# the listing. Every subcommand answering for itself removes the assumption
# that a subcommand is readable text.

for sub in $ALL_SUBCOMMANDS; do
    described=$(run_config "$sub" --describe 2>/dev/null)
    status=$?
    assert_equals "config $sub --describe exits 0" '0' "$status"
    # An empty description would make the line-count and prefix assertions
    # below vacuous, so assert the string exists before asserting anything
    # about its shape.
    assert_succeeds "config $sub --describe prints something" \
        test -n "$described"
done

for sub in $ALL_SUBCOMMANDS; do
    line_count=$(run_config "$sub" --describe 2>/dev/null | grep -c '')
    assert_equals "config $sub --describe prints exactly one line" \
        '1' "$line_count"
done

# The `# help:` comment stays the single home of the string. A subcommand
# that grew a second, hand-written copy would be free to disagree with the
# comment that config.test.sh and the README both point contributors at.
for sub in $ALL_SUBCOMMANDS; do
    comment=$(sed -n 's/^# help: //p' "$CONFIG_DIR/config-$sub" | head -1)
    described=$(run_config "$sub" --describe 2>/dev/null)
    assert_succeeds "config-$sub has a '# help:' line to describe from" \
        test -n "$comment"
    assert_equals "config $sub --describe matches its own '# help:' line" \
        "$comment" "$described"
done

# stdout carries the description; stderr carries nothing. config-help formats
# the value with a %-14s column, so a stray warning on the shared terminal is
# the failure mode this contract exists to remove.
for sub in $ALL_SUBCOMMANDS; do
    noise=$(run_config "$sub" --describe 2>&1 >/dev/null)
    assert_equals "config $sub --describe writes nothing to stderr" \
        '' "$noise"
done

# Asking a command to describe itself must not run it, for the same reason
# --help must not: install-hooks once linked the hooks and rewrote
# ~/.local/bin/config before printing anything.
rm -f "$home/.cfg/hooks/pre-commit" "$home/.cfg/hooks/pre-push" \
    "$home/.local/bin/config"
run_config install-hooks --describe >/dev/null 2>&1
assert_succeeds 'config install-hooks --describe does not link pre-commit' \
    test ! -e "$home/.cfg/hooks/pre-commit"
assert_succeeds 'config install-hooks --describe does not link the dispatcher' \
    test ! -e "$home/.local/bin/config"

output=$(run_config test --describe 2>&1)
assert_equals 'config test --describe does not run the suite' '' \
    "$(printf '%s' "$output" | grep -F 'all:' || true)"

output=$(run_config install --describe 2>&1)
assert_equals 'config install --describe does not exec check-deps' '' \
    "$(printf '%s' "$output" | grep -F 'deps:' || true)"

output=$(run_config doctor --describe 2>&1)
assert_equals 'config doctor --describe does not exec config-manifest' '' \
    "$(printf '%s' "$output" | grep -F 'manifest:' || true)"
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `~/tests/run-all.sh config-usage`

Expected: FAIL. `--describe` is an unrecognized argument today, so each
subcommand falls through `usage_if_requested` into its own body. Concretely:
`config doctor --describe` reaches `config-doctor:26`
(`exec config-manifest doctor "$@"`) and prints `manifest:doctor --describe`,
so `config doctor --describe prints exactly one line` passes for the wrong
reason while `config doctor --describe matches its own '# help:' line` and
`config doctor --describe does not exec config-manifest` both FAIL.
`config test --describe` reaches the test runner and fails the `all:` control.
`config install-hooks --describe` relinks the hooks, failing both
`does not link` assertions. `config build --describe` and
`config stamp --describe` run real work. No assertion in the new section can
pass on the description-matching check, because nothing prints the `# help:`
text.

- [ ] **Step 3: Implement**

`.scripts/config/usage.sh`, replacing the current `usage_if_requested` (lines
21-28) and adding `print_describe` beside `print_usage`:

```sh
# Prints the block and exits 0 when the first argument is --help or -h, or the
# one-line description and exits 0 when it is --describe.
# Call it before parsing anything else, so asking a command what it does never
# runs the command.
usage_if_requested() {
    case ${1:-} in
        --help|-h) print_usage; exit 0 ;;
        --describe) print_describe; exit 0 ;;
    esac
}

# Prints the `# help:` line out of the calling script, without its marker.
#
# config-help formats this with `printf '  %-14s %s\n'`, so the contract is
# exactly one line: a second line, or a line with leading whitespace, breaks
# the column the listing is read in. `head -1` enforces the first half and the
# substitution's own anchor enforces the second.
#
# Read out of the script rather than assigned in each one, so the `# help:`
# comment stays the single home of the string. config.test.sh asserts every
# subcommand carries that comment and the README tells contributors to add
# one, so a second copy here would be the copy that drifts.
print_describe() {
    script=$(readlink -f "$0")
    sed -n 's/^# help: //p' "$script" | head -1
}
```

`.scripts/config/config-help`, replacing the `case` at lines 27-29. `help` is
the one subcommand that does not route through `usage_if_requested`, so it
gets the arm by hand:

```sh
. "$here/usage.sh"
case ${1:-} in
    --help|-h) print_usage; printf '\n' ;;
    --describe) print_describe; exit 0 ;;
esac
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `~/tests/run-all.sh config-usage && ~/tests/run-all.sh config && ~/tests/run-all.sh shellcheck`

Expected: PASS on all three. `config-usage` covers the new contract;
`config` (which asserts the `config help` listing at
`tests/config.test.sh:277-311`) proves the listing is unchanged, since this
task leaves `config-help`'s generator loop alone; `shellcheck` lints the two
edited shell files, and `usage.sh` carries `# shellcheck shell=sh` at line 1
so the new function is checked as POSIX sh.

- [ ] **Step 5: Commit**

```sh
config add .scripts/config/usage.sh .scripts/config/config-help \
    tests/config-usage.test.sh
config commit -m "$(cat <<'MSG'
Add a --describe verb to every shell subcommand

config help builds its listing by running
`sed -n 's/^# help: //p'` over each subcommand's source text. That reads a
subcommand as a text file, which every subcommand is today and which the
first compiled subcommand will not be.

The failure is not a blank description. Pointed at a Mach-O binary, that sed
writes "RE error: illegal byte sequence" to stderr and the pipeline still
exits 0, because `head -1` is the last stage and supplies the status. So the
listing loses the description and gains a linter-style error line, without
failing.

A --describe verb moves the retrieval into the subcommand, where the
subcommand knows its own shape. All nine shell subcommands source usage.sh,
and eight call usage_if_requested before parsing anything, so handling it
there covers eight with no edit to any of them. config-help gets the arm by
hand because it deliberately does not exit on --help.

print_describe still reads the `# help:` comment, so the string keeps one
home. config.test.sh asserts every subcommand carries that comment and the
README tells contributors to add one; a hand-written second copy in each
script would be the copy that drifts.

config-help still source-greps after this commit. Switching the consumer is
the next change, so the contract exists and is tested before anything
depends on it.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
MSG
)"
```

---

## Task 5: `config-help` calls `--describe`; `print_usage` survives a binary subject

With Task 2 landed, `config-help:38` is the only remaining source-grep for
the description, and it is the one that runs on every `config help`. Switching
it to `config-<sub> --describe` is what actually removes the text-file
assumption from the listing.

Two details in the existing loop matter. First, `config-help:35-41` iterates
`"$here"/config-*` and guards with `[ -f "$script" ]`, not `[ -x "$script" ]`.
The dispatcher at `.scripts/config/config:30` only execs `config-$1` when it
is executable, so a non-executable `config-*` file is listed by help but not
reachable through the dispatcher. Calling `--describe` on it would fail to
execute. Second, `config-help` lists itself: `config-help` matches
`config-*`, and `tests/config.test.sh:14` includes `help` in
`EXPECTED_SUBCOMMANDS`, so `config help` must keep describing `help`. Task 2's
`config-help --describe` arm covers that, and it exits before the listing runs,
so there is no recursion.

`usage.sh:30-36`'s `print_usage` has the same text-file assumption and the
same `sed` in it. It cannot be replaced by a subprocess call, because
`print_usage` runs INSIDE the subcommand whose block it is printing: it reads
`$0`, which is exactly the point (`usage.sh:3-5` states this). So the fix
there is different in kind: make it fail loudly instead of silently when `$0`
is not text. A compiled subcommand will never source `usage.sh`, so this is a
guard against a shell subcommand becoming a binary without its help surface
being ported, which is the mistake spec 7.3's "breaks at the same moment"
warns about. I want to flag a wording mismatch rather than plan against it:
spec 7.3 says `usage.sh` "needs the same treatment," but the same treatment is
not available. `print_usage` has no subprocess to delegate to. What it can do
is stop degrading silently.

**Files:**
- Modify: `.scripts/config/config-help` (the generator loop, lines 35-41, plus
  the header prose at lines 13-16 that documents the mechanism)
- Modify: `.scripts/config/usage.sh` (`print_usage`, lines 30-36)
- Modify: `tests/config-usage.test.sh` (new section)

**Interfaces:**
- Consumes: Task 2's contract. `config <sub> --describe` prints one line to
  stdout, exits 0, writes nothing to stderr, and runs no work.
- Produces: `config help`'s listing is generated by executing each executable
  `config-*` sibling with `--describe`. A sibling that is not executable, or
  that exits non-zero, or that prints nothing, is listed as `(undocumented)`
  rather than omitted or crashing the listing. `print_usage` exits 1 with a
  message on stderr when `$0` is not a text file, instead of printing a `sed`
  error and empty help.

- [ ] **Step 1: Write the failing test**

Appended to `tests/config-usage.test.sh` after Task 2's section.

```bash
# --- the listing consumes --describe ----------------------------------------

# The listing is generated by asking each subcommand, not by reading it. A
# subcommand that is a compiled binary has no `# help:` line to grep, and
# grepping one anyway put "sed: RE error: illegal byte sequence" on stderr and
# an empty description in the column.
listing=$(run_config help 2>/dev/null)
assert_succeeds 'config help prints a listing' test -n "$listing"

# A binary subcommand, standing in for the first ported one. Not a shell
# script: the point is that the listing works on a file no sed can read. `cp`
# of a real binary rather than a crafted file, so the bytes are whatever a
# compiler actually emits.
binary_sub="$CONFIG_DIR/config-fixturebin"
cleanup_binary_sub() { rm -f "$binary_sub"; }
trap cleanup_binary_sub EXIT

if [ -x /bin/echo ]; then
    cp /bin/echo "$binary_sub"
    chmod 755 "$binary_sub"
    # /bin/echo answers --describe by printing "--describe", which is one line
    # on stdout with exit 0. That satisfies the contract, which is what makes
    # it usable as a stand-in here: the assertion under test is that the
    # listing reads stdout and emits no sed error, not what the text says.
    output=$(run_config help 2>&1)
    assert_contains 'config help lists a binary subcommand' 'fixturebin' \
        "$output"
    assert_equals 'a binary subcommand produces no sed error in the listing' \
        '' "$(printf '%s' "$output" | grep -F 'illegal byte sequence' || true)"
    assert_equals 'a binary subcommand produces no sed error at all' '' \
        "$(printf '%s' "$output" | grep -F 'sed:' || true)"
    rm -f "$binary_sub"
else
    skip '/bin/echo is missing, so there is no binary to stand in for a ported subcommand'
fi
trap - EXIT

# A config-* sibling that is not executable cannot be reached through the
# dispatcher (.scripts/config/config:30 requires -x), so the listing cannot
# execute it either. It must degrade to (undocumented) rather than emitting a
# not-found error into the column.
inert_sub="$CONFIG_DIR/config-fixtureinert"
cleanup_inert_sub() { rm -f "$inert_sub"; }
trap cleanup_inert_sub EXIT
printf '#!/bin/sh\n# help: never runs\n' > "$inert_sub"
chmod 644 "$inert_sub"
output=$(run_config help 2>&1)
assert_contains 'config help still lists a non-executable sibling' \
    'fixtureinert' "$output"
assert_contains 'a non-executable sibling is marked undocumented' \
    '(undocumented)' "$output"
assert_equals 'a non-executable sibling produces no exec error' '' \
    "$(printf '%s' "$output" | grep -iE 'permission denied|not found' || true)"
rm -f "$inert_sub"
trap - EXIT

# Every real subcommand's description still reaches the column. This is the
# regression guard on the switch itself: the listing has to say the same thing
# it said when it was grepping source.
undescribed=''
for sub in $ALL_SUBCOMMANDS; do
    described=$(run_config "$sub" --describe 2>/dev/null)
    [ -n "$described" ] || { undescribed="$undescribed $sub"; continue; }
    printf '%s\n' "$listing" | grep -qF "$described" \
        || undescribed="$undescribed $sub"
done
assert_equals 'config help prints every subcommand description' '' \
    "$undescribed"

assert_equals 'the listing has no undocumented entries of its own' '' \
    "$(printf '%s' "$listing" | grep -F '(undocumented)' || true)"

# --- print_usage does not degrade silently ----------------------------------

# print_usage reads $0, so it cannot delegate to a subprocess the way the
# listing now does: the whole point is that it prints the block out of the
# script the reader asked about. What it can stop doing is printing an empty
# block plus a sed error. A shell subcommand rewritten as a binary without
# porting its help surface is the mistake this catches.
probe="$FIXTURES/print-usage-probe"
cat > "$probe" <<'PROBE'
#!/bin/sh
set -eu
. "$1/usage.sh"
shift
print_usage
PROBE
chmod 755 "$probe"

# Positive control: pointed at a text script, the probe prints that script's
# block. Without this, the failure assertion below passes when the probe is
# simply broken.
text_output=$(cd "$CONFIG_DIR" && "$probe" "$CONFIG_DIR" 2>&1) || true
assert_succeeds 'the print_usage probe runs at all' test -n "$text_output"

if [ -x /bin/echo ]; then
    cp /bin/echo "$FIXTURES/binary-probe-subject"
    chmod 755 "$FIXTURES/binary-probe-subject"
    binary_output=$(cd "$FIXTURES" && ln -sf "$probe" ./binary-shaped-probe \
        && HOME="$home" sh -c '
            . "$1/usage.sh"
            set -- "$2"
            print_usage
        ' _ "$CONFIG_DIR" "$FIXTURES/binary-probe-subject" 2>&1) || true
    assert_equals 'print_usage on a binary emits no sed error' '' \
        "$(printf '%s' "$binary_output" | grep -F 'illegal byte sequence' || true)"
    assert_contains 'print_usage on a binary says what went wrong' \
        'not a text file' "$binary_output"
else
    skip '/bin/echo is missing, so there is no binary to point print_usage at'
fi
```

The second probe deliberately sets `$0` by invoking `sh -c` with a positional
argument rather than by copying the helper: `print_usage` reads `$0`, and the
only way to control `$0` for a sourced function is to control the script it is
sourced into. `sh -c '...' _ "$CONFIG_DIR" "$subject"` gives `$0` the value
`_`, so the implementation below must accept the subject as an argument with
`$0` as its default rather than reading `$0` unconditionally.

- [ ] **Step 2: Run the test to verify it fails**

Run: `~/tests/run-all.sh config-usage`

Expected: FAIL on four counts.
`a binary subcommand produces no sed error at all` fails, because
`config-help:38` still greps `config-fixturebin` with `sed` and macOS `sed`
prints `RE error: illegal byte sequence` for the copied `/bin/echo`.
`a non-executable sibling is marked undocumented` fails, because
`config-fixtureinert` carries a real `# help:` line and today's grep finds it,
so the column shows `never runs` instead.
`print_usage on a binary emits no sed error` and
`print_usage on a binary says what went wrong` both fail, because
`usage.sh:32` runs the same `sed` and the function has no argument to accept a
subject on.

- [ ] **Step 3: Implement**

`.scripts/config/config-help`, replacing the header paragraph at lines 13-16
and the loop at lines 35-41:

```sh
# The listing is generated by asking each config-<sub> beside this script to
# describe itself, so a new utility documents itself the moment it lands here.
# A hand-written list would be a second copy of the same facts, and the one
# that nobody edits is the one that goes stale.
#
# Asked rather than read: the description used to come from a
# `sed -n 's/^# help: //p'` over the subcommand's source text, which assumed
# every subcommand is a readable text file. Against a compiled binary that sed
# prints "RE error: illegal byte sequence" to stderr and still exits 0, so the
# listing lost the description and gained an error line without failing.
```

```sh
for script in "$here"/config-*; do
    [ -f "$script" ] || continue
    name=${script##*/config-}
    # Only an executable sibling is reachable through the dispatcher, which
    # requires -x before it execs. A file that help lists but the dispatcher
    # will not run has no description to ask for, and asking anyway would put
    # a permission error in the column.
    description=''
    if [ -x "$script" ]; then
        description=$("$script" --describe 2>/dev/null | head -1) || description=''
    fi
    [ -n "$description" ] || description='(undocumented)'
    printf '  %-14s %s\n' "$name" "$description"
done
```

`.scripts/config/usage.sh`, replacing `print_usage` (lines 30-36):

```sh
# Prints the `# usage:` block out of a script, defaulting to the caller's own.
#
# The subject is an argument with a default rather than a bare read of $0, so
# the function is callable against a named file and therefore testable without
# a second copy of the script under a new name.
#
# Unlike the one-line description, this cannot delegate to
# `<script> --describe`: print_usage runs inside the very script whose block
# it prints, and reading that script is the point. So the text-file assumption
# stays, and the guard below is what keeps it from failing silently. A shell
# subcommand rewritten as a binary that still sources this helper is the
# mistake worth naming: without the guard, sed writes "RE error: illegal byte
# sequence" to stderr and the reader gets empty help.
print_usage() {
    usage_script=$(readlink -f "${1:-$0}")
    if ! LC_ALL=C grep -qI '' "$usage_script" 2>/dev/null; then
        printf '%s: not a text file, so it has no "# usage:" block to print\n' \
            "$usage_script" >&2
        return 1
    fi
    sed -n '/^# usage:/,/^[^#]/p' "$usage_script" \
        | sed -e '/^[^#]/d' \
        | awk '/^# ---/ { exit } { print }' \
        | sed -e 's/^# \{0,1\}//'
}
```

`grep -qI ''` is the text-file probe: `-I` makes `grep` treat a binary file as
a non-match, and the empty pattern matches every line of any file that has
lines, so the exit status is "this file is text and non-empty". `LC_ALL=C`
pins the binary determination so it does not vary with the caller's locale.

`print_describe` from Task 2 needs the same subject-argument shape, for the
same testability reason:

```sh
print_describe() {
    describe_script=$(readlink -f "${1:-$0}")
    sed -n 's/^# help: //p' "$describe_script" | head -1
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `~/tests/run-all.sh config-usage && ~/tests/run-all.sh config && ~/tests/run-all.sh config-docs && ~/tests/run-all.sh shellcheck`

Expected: PASS on all four. `config-docs` is in the list because it harvests
subcommand names from the README with
`sed -n 's/^- \`config \([a-z-]*\)\`.*/\1/p'` (`tests/config-docs.test.sh:51`)
and cross-checks them against `ls config-*` (line 29). I checked: neither
direction is affected by this change. The README bullets and the `config-*`
filenames are untouched, and `config-docs.test.sh` never reads a `# help:`
line or runs `config help` at all -- its header at lines 11-14 says the
descriptions are deliberately not duplicated there. The one live risk is the
test fixtures: `config-fixturebin` and `config-fixtureinert` are created
inside `$CONFIG_DIR`, which is the real repo directory, so `config-docs` would
report them as undocumented if it ran while they existed. Both are removed
before the assertions that follow and each has an `EXIT` trap as a backstop,
and `run-all.sh` runs suites serially (`tests/run-all.sh:26-28`), so no
overlap is possible within a run.

- [ ] **Step 5: Commit**

```sh
config add .scripts/config/config-help .scripts/config/usage.sh \
    tests/config-usage.test.sh
config commit -m "$(cat <<'MSG'
Generate the config help listing by asking, not by grepping

config-help built its listing by running
`sed -n 's/^# help: //p'` over each subcommand's source text. Now that every
shell subcommand answers --describe, the listing asks instead, which removes
the assumption that a subcommand is a readable text file.

The loop also gained an -x guard. .scripts/config/config execs config-<sub>
only when it is executable, so a non-executable sibling is listed by help but
unreachable through the dispatcher. Grepping such a file worked; executing it
does not, and the error would land in the description column. It reads
(undocumented) instead, which is what the column already said for a file with
no description.

print_usage could not get the same treatment, and the spec's "same treatment"
wording overstates what is available there. print_usage runs inside the script
whose block it prints and reads $0 to find it, so there is no subprocess to
delegate to. What it can stop doing is degrading silently: it now checks that
its subject is a text file and returns 1 with a message, rather than printing
a sed error to stderr and empty help to stdout. A shell subcommand rewritten
as a binary that still sources this helper is the case that reaches it.

Both functions take the subject as an argument defaulting to $0, so each is
testable against a named file without a second copy of a script under a new
name.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
MSG
)"
```

---

## Task 6: `--describe` on `config-manifest`, and a rendering check on `config help`

The shell side now satisfies a contract that the Rust side does not. `config
doctor` is already a shim over a binary (`.scripts/config/config-doctor:26`
is `exec config-manifest doctor "$@"`), and spec 7.4 step 3 ports
`check-deps.sh` into a crate next. The first subcommand that is a binary rather
than a shim needs `--describe` from the binary itself, so this task adds it
where the pattern is established: `config-manifest`.

`crates/config-manifest/src/main.rs:10-29` declares `Cli` with two global
flags (`--stamp` at line 20, `--root` at line 24) and an optional subcommand.
`--stamp` is the exact precedent: a bare boolean flag, handled at lines 65-75
before the subcommand match, printing one line and returning
`ExitCode::SUCCESS`. `--describe` is the same shape. Two constraints from the
existing code carry over. It must not be `exclusive`, for the reason the
comment at lines 16-19 gives: `--root` is env-backed, so a shell exporting
`DOTFILES_ROOT` makes clap treat `--root` as supplied and an exclusive flag
collides with it and exits 2. And `help_cli.rs:107-116` asserts `--stamp` and
`--version` stay global flags rather than becoming subcommands, so
`--describe` belongs in the same place rather than as a `Command` variant.

The second half of this task is a rendering check. Spec 7.4 step 1 is worth
nothing if `config help` looks different afterward, and `config-help:40`
formats the value with `printf '  %-14s %s\n'`, so a trailing newline or a
second line breaks the column. Capturing the listing before Task 2 and
diffing it after Task 3 is the only assertion that actually proves the
migration was invisible.

**Files:**
- Modify: `crates/config-manifest/src/main.rs` (`Cli` struct, lines 14-29;
  `main`, lines 57-100)
- Modify: `crates/config-manifest/tests/help_cli.rs` (new tests)
- Modify: `tests/config-usage.test.sh` (the rendering check)

**Interfaces:**
- Consumes: Task 2's contract (one line to stdout, exit 0, nothing on stderr,
  no work performed) and Task 3's consumer (`config-help` executes each
  executable sibling with `--describe`).
- Produces: `config-manifest --describe` prints one line to stdout and exits 0,
  with `DOTFILES_ROOT` set or unset, and touches no filesystem path. Any future
  binary subcommand reached through a `config-<sub>` shim inherits the pattern:
  a global `--describe` flag handled before the subcommand match.

- [ ] **Step 1: Write the failing test**

Appended to `crates/config-manifest/tests/help_cli.rs`, using the `run` and
`stdout_of` helpers already defined at lines 5-14:

```rust
#[test]
fn describe_prints_one_line_and_succeeds() {
    // Every config subcommand answers --describe with one line on stdout and
    // exit 0, because config-help formats the value with a `%-14s` column. A
    // trailing blank line or a second line breaks that column, so the line
    // count is the assertion, not just the presence of text.
    let assert = run(&["--describe"]).success();
    let description = stdout_of(&assert);
    assert!(
        !description.trim().is_empty(),
        "--describe printed nothing: {description:?}"
    );
    assert_eq!(
        description.lines().count(),
        1,
        "--describe printed more than one line: {description:?}"
    );
    assert!(
        description.ends_with('\n'),
        "--describe did not end with exactly one newline: {description:?}"
    );
    assert!(
        !description.starts_with(char::is_whitespace),
        "--describe indented its line, which the %-14s column already does: {description:?}"
    );
}

#[test]
fn describe_writes_nothing_to_stderr() {
    // A warning on stderr lands in the same terminal as the listing, which is
    // the failure mode the shell side replaced: `sed` on a binary printed
    // "RE error: illegal byte sequence" while the pipeline still exited 0.
    let assert = run(&["--describe"]).success();
    let noise = stderr_of(&assert);
    assert!(noise.is_empty(), "--describe wrote to stderr: {noise:?}");
}

#[test]
fn describe_stays_a_global_flag_not_a_subcommand() {
    // Same constraint --stamp and --version carry: config-help invokes
    // `config-<sub> --describe`, with no subcommand word available to put in
    // front of it.
    let assert = run(&["--help"]).success();
    let help = stdout_of(&assert);
    assert!(
        help.contains("--describe"),
        "--help omits --describe: {help}"
    );
}

#[test]
fn describe_works_while_dotfiles_root_is_set() {
    // --root is env-backed, so a shell that exports DOTFILES_ROOT makes clap
    // treat --root as supplied. An `exclusive` --describe would collide with
    // it and exit 2, which is the bug the --stamp comment already records.
    Command::cargo_bin("config-manifest")
        .expect("binary built")
        .env("DOTFILES_ROOT", "/tmp")
        .arg("--describe")
        .assert()
        .success();
}

#[test]
fn describe_does_no_work() {
    // Asking a command what it does must not be the same as doing it. Pointed
    // at a directory that is not a repo, `doctor` fails; `--describe` must
    // succeed there, which proves it returns before the gather.
    let empty = tempfile::tempdir().expect("tempdir");
    Command::cargo_bin("config-manifest")
        .expect("binary built")
        .args(["--root".as_ref(), empty.path().as_os_str()])
        .arg("--describe")
        .assert()
        .success();
    Command::cargo_bin("config-manifest")
        .expect("binary built")
        .args(["--root".as_ref(), empty.path().as_os_str()])
        .arg("doctor")
        .assert()
        .failure();
}
```

And the rendering check, appended to `tests/config-usage.test.sh`:

```bash
# --- the listing renders identically ----------------------------------------

# The point of the --describe migration is that nothing about `config help`
# changed. A recorded expectation is the only assertion that proves it: the
# column is built with `printf '  %-14s %s\n'`, so a description that gained a
# trailing newline or a second line would shift every row after it and no
# other test in this suite would notice.
#
# The expectation is the file below, committed alongside this suite. It was
# captured from `config help` before the migration and is regenerated only by
# a deliberate edit to the listing's own format or to a `# help:` line.
expectation="$DOTFILES_ROOT/tests/fixtures/config-help-listing.txt"
if [ -f "$expectation" ]; then
    actual=$(run_config help 2>/dev/null)
    expected=$(cat "$expectation")
    assert_succeeds 'the recorded config help listing is not empty' \
        test -n "$expected"
    assert_equals 'config help renders exactly the recorded listing' \
        "$expected" "$actual"
else
    assert_equals 'the recorded config help listing exists' \
        'present' 'missing'
fi
```

The `else` arm fails rather than skipping on purpose: a missing expectation
file is a broken gate, and `tests/shellcheck.test.sh:43-49` sets the precedent
that a missing gate FAILS rather than skipping quietly.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd ~/crates && cargo test -p config-manifest --test help_cli`

Expected: FAIL. `--describe` is not a declared argument, so clap rejects it
with `error: unexpected argument '--describe' found` and exits 2. Every one of
the five new tests fails on `.success()` (or on `.contains("--describe")` for
the `--help` test).

Run: `~/tests/run-all.sh config-usage`

Expected: FAIL with `the recorded config help listing exists`, because
`tests/fixtures/config-help-listing.txt` does not exist yet.

- [ ] **Step 3: Implement**

`crates/config-manifest/src/main.rs`, adding the flag to `Cli` beside
`--stamp` (after line 21):

```rust
    /// Print the one-line description config help lists this command under.
    ///
    /// Not `exclusive`, for the same reason --stamp is not: --root is
    /// env-backed, so a shell that exports DOTFILES_ROOT makes clap treat
    /// --root as supplied, and an exclusive --describe would collide with it
    /// and exit 2. config-build and the test suite both run with
    /// DOTFILES_ROOT set.
    #[arg(long)]
    describe: bool,
```

and handling it in `main`, immediately before the `cli.stamp` block at line 65
so a bare `--describe` never reaches the subcommand match:

```rust
    // config-help builds its listing by running `config-<sub> --describe` and
    // formatting the result with `printf '  %-14s %s\n'`, so the contract is
    // exactly one line on stdout and exit 0. `println!` supplies the single
    // trailing newline. Handled before every other branch, because asking a
    // command what it does must not run it.
    if cli.describe {
        println!("Report installed binaries that do not match their source");
        return ExitCode::SUCCESS;
    }
```

The string is the same text `.scripts/config/config-doctor:2` carries on its
`# help:` line. Today `config doctor` is a shim and `config-help` reads the
shim, so the shim's `# help:` line is what the listing shows and the binary's
string is unreached. It is written here because spec 7.4 step 3 turns
`config deps` into a binary subcommand, and the binary is where the
description will have to live once there is no shell script to hold it.

Then capture the expectation file, from a tree with Tasks 2 and 3 applied but
before this listing is asserted:

```sh
mkdir -p ~/tests/fixtures
config -- stash --quiet
config -- stash pop --quiet
~/.scripts/config/config help > ~/tests/fixtures/config-help-listing.txt
```

The capture must be taken from the pre-migration listing to be worth
anything. Concretely: check out the commit before Task 2, run
`~/.scripts/config/config help > /tmp/config-help-before.txt`, return to the
working branch, and move that file into `tests/fixtures/`. A capture taken
after the migration asserts only that the migration agrees with itself.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd ~/crates && cargo test -p config-manifest && ~/tests/run-all.sh config-usage && ~/tests/run-all.sh config && ~/tests/run-all.sh config-docs`

Expected: PASS on all four. The rendering check is the load-bearing one: it
compares `config help` against the listing captured before Task 2 and proves
the three-task migration is invisible in the output. `cargo test -p
config-manifest` rather than just `--test help_cli`, so the existing
`stamp_and_version_stay_global_flags_not_subcommands` test at
`help_cli.rs:107-116` confirms the new flag did not disturb the two flags
`config-build` and the pre-push stamp check call directly.

- [ ] **Step 5: Commit**

```sh
config add crates/config-manifest/src/main.rs \
    crates/config-manifest/tests/help_cli.rs \
    tests/config-usage.test.sh \
    tests/fixtures/config-help-listing.txt
config commit -m "$(cat <<'MSG'
Answer --describe from config-manifest, and pin the help listing

The shell subcommands now answer --describe and config help consumes it. The
Rust side did not, which leaves the contract half-implemented at exactly the
boundary it exists to cross: config doctor is already a shim over this binary,
and the next port turns check-deps.sh into a crate, at which point there is no
shell script left to hold the description.

--describe is a global flag rather than a subcommand, because config-help
invokes `config-<sub> --describe` with no subcommand word to put in front of
it, and because help_cli.rs already asserts --stamp and --version stay global
for the same reason. It is not `exclusive`: --root is env-backed, so a shell
that exports DOTFILES_ROOT makes clap treat --root as supplied, and an
exclusive flag collides with it and exits 2. That bug is already recorded in
the --stamp comment.

The listing expectation is the assertion that makes the whole migration
checkable. config-help formats each description with `printf '  %-14s %s\n'`,
so a description that gained a trailing newline or a second line would shift
every row after it and no other test would notice. The file was captured from
`config help` before the first --describe commit, so a match proves the three
changes are invisible in the output rather than merely self-consistent.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
MSG
)"
```

---

## Tasks 7 to 11: the `deps-core` crate

*Drafted separately and appended below.*

---

## Task 12: The rename, atomically

Spec 7.4 step 3 puts the rename blast radius at **18 consumers** and
enumerates five in a table plus roughly thirteen in prose.

**Measured, and the spec undercounts by nearly 2x.** The real figure at the
time of writing:

```
config grep -rl -I 'check-deps.sh' -- . | grep -v '^docs/'   ->  34 files
config grep -rn -I 'check-deps.sh' -- . | grep -v '^docs/'   ->  97 references
```

That is not a reason to re-plan the step, but it is a reason not to trust a
count from memory halfway through. The full file list is below, and the task
is done when the grep returns only intended hits.

The 34 files, grouped by what breaks if one is missed:

**Pins the literal program name (a miss here fails a test, loudly):**
`tests/deps-harness.test.sh`, `tests/depcheck-hook.test.sh`,
`tests/shellcheck.test.sh`, `tests/scripts-dir-name.test.sh`,
`tests/check-deps.test.sh`, `tests/deps-docs.test.sh`,
`tests/config-usage.test.sh`, `tests/config.test.sh`,
`tests/config-init.test.sh`, `tests/platform.test.sh`,
`tests/bootstrap-harness.test.sh`, `tests/run-in-docker.sh`

**Executes it (a miss here fails a bootstrap, on someone's new machine):**
`setup.sh`, `.scripts/config/config-init`, `.scripts/config/config-install`,
`.scripts/deps/depcheck-hook.sh`, `.scripts/deps/test-local.sh`,
`.scripts/deps/test-bootstrap.sh`, `.scripts/platform.sh`,
`.scripts/deps/check-deps.sh` itself

**Container entrypoints and build stages (a miss here fails CI only):**
`.scripts/deps/docker/Dockerfile.ubuntu`, `Dockerfile.arch`,
`Dockerfile.bootstrap`, `Dockerfile.bootstrap-curl-arch`,
`docker/bootstrap-entrypoint.sh`, `docker/bootstrap-curl-entrypoint.sh`,
`tests/docker/Dockerfile`

**CI:** `.github/workflows/deps-check.yml`, `.github/workflows/test-suite.yml`

**Docs, which are also test inputs:** `README.md`, `.scripts/deps/README.md`

**Manifests, which mention it in comments:** `.scripts/deps/deps.conf`,
`deps-mac.conf`, `deps-linux.conf`

### The two blockers inside this step

Both are spec 7.4 step 3's, restated because they are the parts a mechanical
rename does not cover.

**The Docker images cannot run the replacement.** They are pre-toolchain
consumers per spec 3.7: `ENTRYPOINT` is the script, the build context is only
`.scripts/deps`, and the image installs just
`sudo curl git wget ca-certificates`. Spec decision: **add a build stage**,
because the images exist to exercise a real bootstrap on a clean machine and
a bootstrap that cannot build the tool is not the bootstrap being shipped.
This is its own task, not a line in the rename.

**`deps-docs.test.sh` will pass vacuously.** It harvests every `--flag` from
README lines mentioning `check-deps.sh` or `depcheck`, probes each against
`$CHECK_SCRIPT`, and uses exit 2 as its oracle:
`[ "$?" -ne 2 ] || rejected_flags="..."`. **Verified by execution in the
spec:** a missing program exits **127**, `[ 127 -ne 2 ]` is true, so nothing
is added to `rejected_flags` and the assertion passes while probing a program
that does not exist.

This test must be updated or deleted **in the same commit as the rename**,
not deferred. It is the same failure shape as Task 1a's dead stamp gate and
Task 1's four satisfied fixtures, which is now three instances of this bug
class on this branch alone.

- [ ] **Step 1: Prove the vacuous pass before changing anything**

`CHECK_SCRIPT` is assigned at `tests/deps-docs.test.sh:23` from `$DEPS_DIR`
and is **not** an injectable seam, so it cannot be overridden from the
environment. Isolate the oracle instead, which has been verified to
reproduce:

```sh
cat > /tmp/oracle-probe.sh <<'PROBE'
#!/bin/bash
# tests/deps-docs.test.sh:106-116 against a program that does not exist.
CHECK_SCRIPT=/nonexistent/check-deps.sh
documented_flags=$'--fix\n--yes\n--dry-run'
rejected_flags=''
while IFS= read -r flag; do
    [ -n "$flag" ] || continue
    "$CHECK_SCRIPT" "$flag" --dry-run >/dev/null 2>&1
    [ "$?" -ne 2 ] || rejected_flags="$rejected_flags $flag"
done <<< "$documented_flags"
echo "rejected_flags='$rejected_flags'"
"$CHECK_SCRIPT" --fix >/dev/null 2>&1; echo "missing-program status: $?"
PROBE
bash /tmp/oracle-probe.sh
```

Verified output:

```
rejected_flags=''
missing-program status: 127
```

So the assertion `'check-deps.sh accepts every documented flag' '' ""`
passes about a program that does not exist. Note also that running the whole
suite against a root with no `check-deps.sh` yields **13 passing
assertions**, which is the blast radius of the missing seam.

Adding `CHECK_SCRIPT=${CHECK_SCRIPT:-$DEPS_DIR/check-deps.sh}` at line 23 is
part of this task: without an injectable seam the corrected oracle cannot be
tested against an absent program, and an untested oracle is what got us
here.

- [ ] **Step 2: Fix the oracle first, with the old name still in place**

Change the probe to distinguish "flag rejected" (exit 2) from "program
absent" (127) from "flag accepted" (0). A missing program must fail the test,
not satisfy it:

```sh
"$CHECK_SCRIPT" "$flag" --dry-run >/dev/null 2>&1
probe_status=$?
if [ "$probe_status" -eq 127 ]; then
    # 127 is "no such program", which used to read as "flag accepted"
    # because the oracle only asked whether the status differed from 2.
    missing_program=1
elif [ "$probe_status" -eq 2 ]; then
    rejected_flags="$rejected_flags $flag"
fi
```

Then assert `missing_program` is 0, with the positive control that the
harvest found flags at all:

```sh
assert_succeeds 'the README harvest found at least one flag' test -n "$harvested_flags"
assert_equals 'the probed program exists' '0' "$missing_program"
assert_equals 'the README documents no rejected flag' '' "$rejected_flags"
```

- [ ] **Step 3: Verify the fixed oracle now fails**

```sh
cd ~ && CHECK_SCRIPT=/nonexistent/check-deps.sh bash tests/deps-docs.test.sh
echo "exit=$?"
```

Expected: **FAIL, non-zero.** Same command as Step 1, opposite result. If it
still passes, the oracle is still wrong.

Then confirm the real path is green:

Run: `bash tests/deps-docs.test.sh`
Expected: PASS.

- [ ] **Step 4: Commit the oracle fix separately**

The oracle fix is independently correct and independently reviewable. It
lands before the rename so that the rename has a gate that can actually fail.

```bash
config add tests/deps-docs.test.sh
config commit -m "Fail deps-docs when the probed program is absent

The flag probe used exit 2 as its only oracle: [ \"\$?\" -ne 2 ] || record.
A missing program exits 127, so the guard was true, nothing was recorded,
and the assertion passed while probing a program that does not exist.
Verified: CHECK_SCRIPT=/nonexistent passed with exit 0 before this change
and fails after it.

This matters now because the check-deps.sh rename is about to move the
program this test probes, and a test that passes when the program is gone
cannot gate that rename.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

- [ ] **Step 5: Do the rename in one commit**

Atomicity is a correctness requirement, not tidiness. Spec 7.4 step 4 states
the reason for the `config-*` case and it applies here: the dispatcher falls
through to `git` for any unmatched verb, so a window where the old name is
gone before the new one is installed silently reinterprets a command.

Work from the file list above. After editing, the grep must return only
intended hits:

```sh
cd ~ && config grep -rn -I 'check-deps\.sh' -- . | grep -v '^docs/'
```

Expected: only historical mentions inside commit-message-shaped comments that
deliberately record the old name. Every live invocation, entrypoint,
workflow step and test literal names the new one.

- [ ] **Step 6: Run everything**

Run: `tests/run-all.sh`
Expected: PASS.

Run: `cd ~/crates && cargo test --locked`
Expected: PASS.

Run: `bash tests/run-in-docker.sh`
Expected: PASS. This is the leg that exercises the container entrypoints, and
the entrypoints are where a missed rename hides from the host suite.

- [ ] **Step 7: Commit**

```bash
config add -A
config commit -m "Rename check-deps.sh to its ported entry point

Measured 34 files and 97 references, not the 18 consumers the spec
estimated. One commit, because a window where the old name is gone before
the new one is installed changes what an unmatched verb means.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## What this plan does not cover

Named explicitly so a later reader does not mistake absence for oversight.
Each of these is a real remaining step with a real reason it is not here.

**Spec 7.4 step 4: the remaining `config-*` subcommands.** Nine subcommands
(`build`, `doctor`, `help`, `init`, `install`, `install-hooks`, `reload`,
`stamp`, `test`). Ports must be atomic per the dispatcher-fallthrough
argument above, and each one needs its own `--describe` and `--help`
contract, which is why Tasks 2 to 4 come first. One plan per two or three
subcommands is the right granularity.

**Spec 7.4 step 5: the tmux scripts.** Blocked on two structural sourcing
dependencies the spec verified and the first draft missed:
`tmux-start.sh:34` sources `tmux-split.sh` with **no arguments**, relying on
positional-parameter inheritance so `LAYOUT_TYPE=${1:-}` reads the caller's
`$1` (verified: sourced sees the outer `$1`, exec'd sees empty). And
`show_usage` ends `return 1`, a value, with an adjacent comment recording
that `exit` was deliberately rejected. `tmux-split.sh` therefore does not
convert cleanly and needs a decided argument-passing contract before any
plan is written.

**Spec 7.4 step 6 and 7.5: the 43-suite test port.** Deliberately last,
because it is the safety net for everything above it. Spec 7.5's corrected
three-way split:

| Tranche | Suites | Why this order |
|---|---|---|
| Gets better | 10 | Currently `grep`/`sed` over tracked files; gain real parsers (`serde_yaml`, a Markdown parser). Has a verified bug class behind it, tests only tracked files, needs no architecture. Can go early and independently. |
| Shell stays the subject | 7 | Six `zshrc-*` plus `zsh-git-widgets.test.sh`. Rust drives them; the subject stays shell. Honest, not a gap. `zshrc-platform-split.test.sh` needs re-derivation, not a port (spec 7.6). |
| Equivalent, better fixtures | 26 | `assert_cmd` plus `tempfile` replaces the harness. Thin payoff per suite, so last, and incremental. |

`tests/lib.sh` and `tests/run-all.sh` are deleted **last**, and only after the
Rust suite has run green alongside them for a while. Converting a reversible
migration into an irreversible one at the moment of the swap buys nothing.

The "gets better" tranche is the one worth doing soon and on its own: it is
where `config-docs.test.sh:47-53` extracts subcommand names with
`sed -n 's/^- `config \([a-z-]*\)`.*/\1/p'`, the bug class spec 7.5 cites.

**Not in the spec at all: the leak-guard scan range.** `tests/leak-check.sh`
scans from the empty tree when git passes `remote_sha` as zeros, which is
every first push of a branch. Measured during the previous plan: 607 commits
scanned where 40 were new. One genuinely unscannable path exists in history
(a deleted binary `.config/nvim/spell/en.utf-8.add.spl`), so the guard is
correct to block, and it will block the next first-push-of-a-branch. The fix
is to exclude commits reachable from any remote ref. Deferred by the owner
during the previous plan; recorded here so it is not rediscovered as a
surprise.

**Not in the spec at all: `origin/HEAD`.** Still points at `origin/mac`.
Changing a repository's default branch is a GitHub setting, not a commit, so
it needs the owner or an authenticated `gh` call. Until it changes, a fresh
`git clone` of the repository checks out `mac`.
