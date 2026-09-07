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

## Tasks 7 to 11: `deps-core` as a pure core

Spec: `docs/superpowers/specs/2026-09-06-pure-core-architecture.md`.
Subject: `.scripts/deps/deps.conf` (16 entries), `deps-mac.conf` (1),
`deps-linux.conf` (2), `deps-ci.conf` (3), total 22, and
`.scripts/deps/check-deps.sh`.

Tasks 7 to 11 build the pure core only. The CLI adapter, the 18-consumer
rename, and the Docker build stage are later tasks.

### Verification notes: where the spec and the real files disagree

Read before starting. Each was checked by reading the file.

1. **The manifest is not TOML.** Spec 5.5 shows a `requires = ["nvm"]` field
    inside a `[[dependency]]` table. No conf file has TOML syntax, a
    `requires` field, or a `version` field. `deps.conf:2` states the real
    format: `name|check_command|docs_url`, pipe-separated, one entry per
    line, and `deps.conf:9-13` warns that a literal `|` in the check field
    truncates it. Task 7 parses the real pipe format. Task 9 therefore has
    no `requires` data to read, so it takes the requirement graph as a
    separate argument rather than a manifest field, and `deps.conf:18-20`
    is the evidence that no ordering exists in the file today ("no ordering
    between it and zsh-autosuggestions is guaranteed here").
    `PlanError::ManifestVersion { found, supported }` from spec 5.4 is
    retained as a variant, because a future format change needs it, but
    nothing constructs it in these five tasks and the plan says so rather
    than inventing a version line.

2. **`dotfiles-path` exports two items, not seven.** Spec 7.1 lists
    `CheckRelPath, CommandName, ModuleName, GlobPattern, PackageId, DocsUrl,
    BoundedText` in that crate. `crates/dotfiles-path/src/lib.rs:11` exports
    only `CheckRelPath` and `PathError`. The other five do not exist. Task 7
    adds `CommandName`, `ModuleName`, `GlobPattern`, `PackageId` and
    `DocsUrl` to `dotfiles-path` because Task 8's `Check` enum cannot be
    written without them. `BoundedText` is added in Task 10, which is the
    first task whose types need it (`ExecFailure::NonZeroExit`).

3. **Three cited line numbers are off by one or two.** Spec 3.5 cites
    `check-deps.sh:173-188` for the elevation computation; the real block is
    168 to 188 (`DEPS_FORCE_ROOT` case at 168, `can_escalate=0` at 187).
    Spec 5.1 cites `:568` as the site that cannot distinguish empty from
    missing; the real test is `:569` (`if [ -z "$cmd" ]`). Spec 5.1 cites
    `:374,389`; the real sites are 374 (the empty `nvm` case) and 390
    (`fi` closing node's guard). The substance of all three claims is
    correct. The exact figures cited below are the re-read ones.

4. **These spec citations verified byte-exact.** `check-deps.sh:236` (the
    `gh` apt keyring pipeline, one line, adds a third-party APT trust root),
    `:309` (`curl -sS .../zoxide/main/install.sh | sh` in the `*)` fallback),
    `:411` (`python3 -m pip install --break-system-packages pyyaml`),
    `:524` (`if sh -c "$check"`), `:545` (the `${SUDO}` string sniff),
    `:600-602` (dry-run exits 0 unconditionally).

5. **The fixpoint evidence verified in the real file.**
    `check-deps.sh:339-341` emits the `zsh-autosuggestions` clone only when
    `[ -d "${ZSH_CUSTOM:-$HOME/.oh-my-zsh/custom}" ]`, and the comment at
    `:328-335` says why. `deps-linux.conf:12` is where `oh-my-zsh` lives.
    So installing `oh-my-zsh` in wave 1 is what makes
    `zsh-autosuggestions` installable in wave 2, and one pass does not
    converge. This is the reason Task 11 exists.

---

## Task 7: `deps-core` crate skeleton and the manifest as pure data

Spec 3.6 is the reason this task comes first: `deps.conf` field 2 is an
arbitrary shell string that `check-deps.sh:524` evaluates with `sh -c`, on a
path (`depcheck-hook.sh`) that reaches it with no `--dry-run` gate. Spec 3.6
also records the amplifier: `config sync` writes across branches with
`commit-tree`, which runs no hooks, so an edited conf file arrives without
passing pre-commit. Turning field 2 into a closed enum with no shell escape
hatch is what deletes that path, and it cannot be done before a typed
manifest exists to hold the enum.

The parse function takes `&str`, not a path. That is the whole no-IO claim
for this task, and it is the shape `crates/config-manifest/src/doctor.rs`
already proves works in this repo: 262 lines, zero references to `std::fs`,
`std::process`, `std::env` or `std::io`, verified by grep, because
`diagnose` compares two caller-supplied maps and `git.rs` does the gathering
(`doctor.rs:1-6` states exactly this).

Spec 5.1's `NotAutomatable` lost its `docs` field to the manifest entry,
where the URL already lives. `DocsUrl` is scheme-agnostic rather than
`HttpsUrl` because `deps-ci.conf:23` is
`http://gondor.apana.org.au/~herbert/dash/`, verified: 21 of 22 entries are
`https://` and that one is not. The URLs are printed for a human and never
fetched.

**Files:**
- Create: `crates/deps-core/Cargo.toml`
- Create: `crates/deps-core/src/lib.rs`
- Create: `crates/deps-core/src/manifest.rs`
- Create: `crates/dotfiles-path/src/name.rs`
- Modify: `crates/dotfiles-path/src/lib.rs`
- Modify: `crates/Cargo.toml`

**Interfaces:**
- Consumes: `dotfiles_path::{CheckRelPath, PathError}` (existing,
  `crates/dotfiles-path/src/rel.rs:67`, `:16`).
- Produces, in `dotfiles-path`:
  - `pub struct CommandName(String)` with
    `pub fn parse(raw: &str) -> Result<Self, NameError>` and
    `pub fn as_str(&self) -> &str`
  - `pub struct ModuleName(String)`, same two methods
  - `pub struct GlobPattern(String)`, same two methods
  - `pub struct PackageId(String)`, same two methods
  - `pub struct DocsUrl(String)`, same two methods
  - `pub enum NameError { Empty, TooLong { len: usize, max: usize },
    ControlByte, PathSeparator, LeadingDash, NotAnIdentifier,
    NotPrintable, NoScheme }`
- Produces, in `deps-core`:
  - `pub struct DependencyName(String)` with
    `pub fn parse(raw: &str) -> Result<Self, NameError>`, `as_str`
  - `pub struct ManifestEntry { pub name: DependencyName, pub check: Check,
    pub docs: DocsUrl }`
  - `pub struct Manifest { entries: Vec<ManifestEntry> }` with
    `pub fn entries(&self) -> &[ManifestEntry]` and
    `pub fn get(&self, name: &DependencyName) -> Option<&ManifestEntry>`
  - `pub enum ParseError { WrongFieldCount { line: usize, found: usize },
    BadName { line: usize, cause: NameError },
    BadCheck { line: usize, cause: CheckParseError },
    BadDocs { line: usize, cause: NameError },
    DuplicateName { line: usize, name: DependencyName },
    InterpreterCheckInPlatformConf { line: usize } }`
  - `pub enum ConfKind { PlatformSelected, ExplicitOnly }`
  - `pub fn parse_manifest(text: &str, kind: ConfKind) -> Result<Manifest, ParseError>`

`Check` and `CheckParseError` are Task 8's. Task 7 lands
`parse_manifest` against a `Check` that has only the two variants Task 7's
tests need (`Command`, `DirExists`); Task 8 extends the enum and the check
parser without touching `parse_manifest`'s signature.

- [ ] **Step 1: Write the failing test for the name primitives**

In `crates/dotfiles-path/src/name.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // A name containing `/` would bypass PATH lookup entirely if the value
    // ever reached Command::new, which is why the rule is at parse time
    // rather than at use (spec 5.2, "Names are validated at parse time").
    #[test]
    fn command_name_rejects_a_path_separator() {
        assert!(matches!(
            CommandName::parse("../../bin/sh"),
            Err(NameError::PathSeparator)
        ));
    }

    #[test]
    fn command_name_rejects_a_leading_dash() {
        assert!(matches!(CommandName::parse("-rf"), Err(NameError::LeadingDash)));
    }

    #[test]
    fn command_name_rejects_control_bytes() {
        assert!(matches!(
            CommandName::parse("git\u{1b}[2J"),
            Err(NameError::ControlByte)
        ));
    }

    #[test]
    fn command_name_rejects_over_64_bytes() {
        let long_name = "a".repeat(65);
        assert!(matches!(
            CommandName::parse(&long_name),
            Err(NameError::TooLong { len: 65, max: 64 })
        ));
    }

    // Every command name in the four conf files, verified by reading them:
    // git gh alacritty zsh nvim fzf rg zoxide tmux shellcheck cc rustup
    // node aerospace xclip python3.
    #[test]
    fn command_name_accepts_every_real_manifest_command() {
        let real_commands = [
            "git", "gh", "alacritty", "zsh", "nvim", "fzf", "rg", "zoxide",
            "tmux", "shellcheck", "cc", "rustup", "node", "aerospace",
            "xclip", "python3",
        ];
        for command in real_commands {
            assert!(
                CommandName::parse(command).is_ok(),
                "a real manifest command was rejected: {command}"
            );
        }
    }

    // PythonImport's surface is `python3 -c "import <this>"`, so the
    // argument must be an identifier and nothing else (spec 5.2).
    #[test]
    fn module_name_rejects_anything_but_an_identifier() {
        assert!(matches!(
            ModuleName::parse("yaml; import os"),
            Err(NameError::NotAnIdentifier)
        ));
        assert!(matches!(ModuleName::parse("os.path"), Err(NameError::NotAnIdentifier)));
        assert_eq!(
            ModuleName::parse("yaml").expect("a bare module name parses").as_str(),
            "yaml"
        );
    }

    // deps-ci.conf:23 is http://, not https://, so a scheme-agnostic type
    // is required. An HttpsUrl cannot parse the manifest this repo ships.
    #[test]
    fn docs_url_accepts_both_schemes_and_rejects_neither() {
        assert!(DocsUrl::parse("https://git-scm.com/downloads").is_ok());
        assert!(
            DocsUrl::parse("http://gondor.apana.org.au/~herbert/dash/").is_ok(),
            "deps-ci.conf:23 must parse"
        );
        assert!(matches!(
            DocsUrl::parse("git-scm.com/downloads"),
            Err(NameError::NoScheme)
        ));
    }

    // An error string reaches a terminal and the input is untrusted, which
    // is the rule rel.rs:29-32 already states for PathError.
    #[test]
    fn name_error_does_not_echo_the_input() {
        let rendered = CommandName::parse("git\u{1b}[2J")
            .expect_err("a control byte is rejected")
            .to_string();
        assert!(
            !rendered.contains('\u{1b}'),
            "the error rendered the escape byte: {rendered:?}"
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd ~/crates && cargo test --locked -p dotfiles-path name::`
Expected: FAIL. `error[E0433]: failed to resolve: use of undeclared type
`CommandName`` and the same for `ModuleName`, `DocsUrl`, `NameError`,
because `name.rs` holds only the test module and `lib.rs` does not declare
`mod name`. This is the missing behavior, not a typo: the types do not
exist.

- [ ] **Step 3: Implement the name primitives**

Prepend to `crates/dotfiles-path/src/name.rs`:

```rust
use std::fmt;

/// The maximum byte length of a parsed name.
///
/// 64 bytes covers every name in the four conf files with room to spare, and
/// a bound is what keeps a name out of an unbounded allocation when it
/// arrives from a file that reaches this crate without passing pre-commit
/// (spec 3.6).
const MAX_NAME_LEN: usize = 64;

/// The maximum byte length of a parsed documentation URL.
const MAX_URL_LEN: usize = 512;

/// Why a candidate name was refused.
///
/// One variant per rule, matching `PathError`'s shape, so a caller reports
/// the cause rather than "invalid name" and a new rule cannot hide inside an
/// existing variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameError {
    Empty,
    TooLong { len: usize, max: usize },
    ControlByte,
    PathSeparator,
    LeadingDash,
    NotAnIdentifier,
    NotPrintable,
    NoScheme,
}

impl fmt::Display for NameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // No variant renders the offending input, for the reason
        // PathError's Display states: these errors reach a terminal and a
        // rejected control byte written into the message would run as an
        // escape sequence.
        match self {
            NameError::Empty => write!(formatter, "the name is empty"),
            NameError::TooLong { len, max } => {
                write!(formatter, "the name is {len} bytes, over the {max}-byte limit")
            }
            NameError::ControlByte => write!(formatter, "the name contains a control byte"),
            NameError::PathSeparator => {
                write!(formatter, "the name contains a path separator, which would bypass PATH lookup")
            }
            NameError::LeadingDash => {
                write!(formatter, "the name starts with `-`, which reads as an option")
            }
            NameError::NotAnIdentifier => {
                write!(formatter, "the name is not a bare identifier")
            }
            NameError::NotPrintable => write!(formatter, "the name contains a non-printable byte"),
            NameError::NoScheme => write!(formatter, "the URL has no `http://` or `https://` scheme"),
        }
    }
}

impl std::error::Error for NameError {}

fn reject_common(raw: &str, max: usize) -> Result<(), NameError> {
    if raw.is_empty() {
        return Err(NameError::Empty);
    }
    if raw.len() > max {
        return Err(NameError::TooLong { len: raw.len(), max });
    }
    // Control bytes first: a message about a shape is less useful than one
    // about a byte that would corrupt the message itself.
    if raw.chars().any(|character| character.is_control()) {
        return Err(NameError::ControlByte);
    }
    Ok(())
}

/// The name of a program looked up on PATH.
///
/// # Errors
///
/// Returns `NameError::PathSeparator` for a name containing `/`, because
/// such a name bypasses PATH lookup entirely once it reaches a process
/// spawn, and `NameError::LeadingDash` for a name an argument parser in the
/// spawned program would read as an option.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CommandName(String);

impl CommandName {
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        reject_common(raw, MAX_NAME_LEN)?;
        if raw.contains('/') || raw.contains('\\') {
            return Err(NameError::PathSeparator);
        }
        if raw.starts_with('-') {
            return Err(NameError::LeadingDash);
        }
        if raw.chars().any(|character| !character.is_ascii_graphic()) {
            return Err(NameError::NotPrintable);
        }
        Ok(CommandName(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CommandName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// A Python module name, safe to place after `import`.
///
/// # Errors
///
/// Returns `NameError::NotAnIdentifier` for anything but a bare identifier.
/// A dotted or punctuated value would let manifest text reach an
/// interpreter as code, which is the escape spec 5.2 closes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ModuleName(String);

impl ModuleName {
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        reject_common(raw, MAX_NAME_LEN)?;
        let mut characters = raw.chars();
        let starts_well = characters
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == '_');
        let rest_is_well_formed = characters
            .all(|character| character.is_ascii_alphanumeric() || character == '_');
        if !starts_well || !rest_is_well_formed {
            return Err(NameError::NotAnIdentifier);
        }
        Ok(ModuleName(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModuleName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// A filename glob pattern, matched inside one already-validated directory.
///
/// # Errors
///
/// Returns `NameError::PathSeparator` for a pattern containing `/`, so the
/// pattern cannot widen the directory the caller chose.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GlobPattern(String);

impl GlobPattern {
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        reject_common(raw, MAX_NAME_LEN)?;
        if raw.contains('/') || raw.contains('\\') {
            return Err(NameError::PathSeparator);
        }
        if raw.chars().any(|character| !character.is_ascii_graphic()) {
            return Err(NameError::NotPrintable);
        }
        Ok(GlobPattern(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GlobPattern {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// A package name passed to a package manager.
///
/// # Errors
///
/// Returns `NameError::LeadingDash` so a package name cannot arrive at a
/// manager as a flag, and `NameError::NotPrintable` for shell-active bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageId(String);

impl PackageId {
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        reject_common(raw, MAX_NAME_LEN)?;
        if raw.starts_with('-') {
            return Err(NameError::LeadingDash);
        }
        let allowed = |character: char| {
            character.is_ascii_alphanumeric()
                || matches!(character, '-' | '_' | '.' | '+' | ':' | '@')
        };
        if !raw.chars().all(allowed) {
            return Err(NameError::NotPrintable);
        }
        Ok(PackageId(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PackageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// A documentation URL, printed for a human and never fetched.
///
/// Scheme-agnostic across `http` and `https` because 21 of the 22 manifest
/// entries are `https://` and `deps-ci.conf:23` is
/// `http://gondor.apana.org.au/~herbert/dash/`. An https-only type cannot
/// parse the manifest this repo ships.
///
/// # Errors
///
/// Returns `NameError::NoScheme` for a value with neither scheme, so a
/// bare word cannot be rendered to a reader as a link.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DocsUrl(String);

impl DocsUrl {
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        reject_common(raw, MAX_URL_LEN)?;
        if !raw.starts_with("https://") && !raw.starts_with("http://") {
            return Err(NameError::NoScheme);
        }
        if raw.chars().any(|character| !character.is_ascii_graphic()) {
            return Err(NameError::NotPrintable);
        }
        Ok(DocsUrl(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DocsUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}
```

Then in `crates/dotfiles-path/src/lib.rs`, replace the module and re-export
block:

```rust
mod name;
mod rel;

pub use name::{CommandName, DocsUrl, GlobPattern, ModuleName, NameError, PackageId};
pub use rel::{CheckRelPath, PathError};
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd ~/crates && cargo test --locked -p dotfiles-path`
Expected: PASS, including the 12 pre-existing `rel::tests` cases.

- [ ] **Step 5: Write the failing test for `parse_manifest`**

In `crates/deps-core/src/manifest.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // The real shared manifest, verbatim from deps.conf. Comment lines and
    // blank lines are skipped; the format is name|check|docs.
    const REAL_SHARED_HEAD: &str = "\
# CLI dependencies shared by every machine, regardless of platform.
# Format: name|check_command|docs_url

git|command -v git|https://git-scm.com/downloads
gh|command -v gh|https://cli.github.com/
tpm|[ -d \"$HOME/.tmux/plugins/tpm\" ]|https://github.com/tmux-plugins/tpm
";

    #[test]
    fn parses_the_real_shared_manifest_head() {
        let manifest = parse_manifest(REAL_SHARED_HEAD, ConfKind::PlatformSelected)
            .expect("the real deps.conf head parses");
        assert_eq!(manifest.entries().len(), 3);
        assert_eq!(manifest.entries()[0].name.as_str(), "git");
        assert_eq!(
            manifest.entries()[0].docs.as_str(),
            "https://git-scm.com/downloads"
        );
        assert_eq!(
            manifest.entries()[0].check,
            Check::Command(CommandName::parse("git").expect("git is a name"))
        );
    }

    // deps.conf:9-13 warns that a literal `|` in the check field truncates
    // the check and leaks the remainder into docs_url. Under `IFS='|' read`
    // that is silent. Here it is an error, which is the point of the port.
    #[test]
    fn rejects_a_line_with_an_extra_pipe() {
        let text = "gh|command -v gh || true|https://cli.github.com/\n";
        assert!(matches!(
            parse_manifest(text, ConfKind::PlatformSelected),
            Err(ParseError::WrongFieldCount { line: 1, found: 4 })
        ));
    }

    #[test]
    fn rejects_a_line_with_a_missing_field() {
        let text = "gh|command -v gh\n";
        assert!(matches!(
            parse_manifest(text, ConfKind::PlatformSelected),
            Err(ParseError::WrongFieldCount { line: 1, found: 2 })
        ));
    }

    // A duplicate name means two entries claim one dependency, and the
    // later one silently wins under the shell loop.
    #[test]
    fn rejects_a_duplicate_name() {
        let text = "\
git|command -v git|https://git-scm.com/downloads
git|command -v git|https://git-scm.com/downloads
";
        assert!(matches!(
            parse_manifest(text, ConfKind::PlatformSelected),
            Err(ParseError::DuplicateName { line: 2, .. })
        ));
    }

    // deps-ci.conf:23 uses http://, so a manifest holding it must parse.
    #[test]
    fn accepts_the_one_plain_http_docs_url() {
        let text = "dash|command -v dash|http://gondor.apana.org.au/~herbert/dash/\n";
        let manifest = parse_manifest(text, ConfKind::ExplicitOnly)
            .expect("deps-ci.conf:23 must parse");
        assert_eq!(
            manifest.entries()[0].docs.as_str(),
            "http://gondor.apana.org.au/~herbert/dash/"
        );
    }

    #[test]
    fn get_finds_an_entry_by_name() {
        let manifest = parse_manifest(REAL_SHARED_HEAD, ConfKind::PlatformSelected)
            .expect("the real deps.conf head parses");
        let wanted = DependencyName::parse("tpm").expect("tpm is a name");
        assert!(manifest.get(&wanted).is_some());
        let absent = DependencyName::parse("nvm").expect("nvm is a name");
        assert!(manifest.get(&absent).is_none());
    }

    // parse takes &str, never a path. This test is the no-IO claim for
    // this module: a caller reads the file, and doctor.rs:1-6 is the
    // in-repo precedent for the shape.
    #[test]
    fn parse_is_a_function_of_text_alone() {
        let first = parse_manifest(REAL_SHARED_HEAD, ConfKind::PlatformSelected)
            .expect("the real deps.conf head parses");
        let second = parse_manifest(REAL_SHARED_HEAD, ConfKind::PlatformSelected)
            .expect("the real deps.conf head parses");
        assert_eq!(first.entries(), second.entries());
    }
}
```

- [ ] **Step 6: Run the test to verify it fails**

Run: `cd ~/crates && cargo test --locked -p deps-core manifest::`
Expected: FAIL with `error: could not find `deps-core` in the workspace`
until `crates/Cargo.toml` lists it, then FAIL with
`error[E0425]: cannot find function `parse_manifest` in this scope` plus
unresolved `Manifest`, `ConfKind`, `ParseError`, `DependencyName`. The
missing behavior is the parser, not a typo.

- [ ] **Step 7: Implement the crate and the parser**

`crates/deps-core/Cargo.toml`:

```toml
[package]
name = "deps-core"
edition = "2024"
version = "0.1.0"
publish = false

# dotfiles-path only, deliberately. An edge to config-manifest would make
# the dependency domain depend on the git-sync domain and carry its 595-line
# git module into a crate that never calls it (spec 7.1).
[dependencies]
dotfiles-path = { path = "../dotfiles-path" }
```

In `crates/Cargo.toml`, extend the members list, keeping it on one line
because `.scripts/config/config-stamp` reads it with sed:

```toml
members = ["config-manifest", "deps-core", "dotfiles-path"]
```

`crates/deps-core/src/lib.rs`:

```rust
//! The dependency-check core. No IO of any kind.
//!
//! Every function here is a function of its arguments. Parsing takes text,
//! planning takes an observation map, and the loop driver lives in the CLI
//! crate. The crate boundary is what makes that compiler-enforced rather
//! than a discipline (spec 7.1), and `config_manifest::doctor` is the
//! in-repo precedent for the module shape.

mod manifest;

pub use manifest::{
    ConfKind, DependencyName, Manifest, ManifestEntry, ParseError, parse_manifest,
};
```

`crates/deps-core/src/manifest.rs`, above the test module:

```rust
use std::collections::BTreeSet;

use dotfiles_path::{CommandName, DocsUrl, NameError};

/// The name of a manifest entry.
///
/// A separate type from `CommandName` because they are different
/// propositions: `neovim` is a dependency whose command is `nvim`, and
/// `pyyaml` is a dependency with no command at all.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DependencyName(String);

impl DependencyName {
    /// # Errors
    ///
    /// Returns `NameError::NotPrintable` for a name outside
    /// `[A-Za-z0-9._-]`, which covers every one of the 22 real entries and
    /// excludes the `|` that would corrupt a re-serialized manifest.
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        if raw.is_empty() {
            return Err(NameError::Empty);
        }
        if raw.len() > 64 {
            return Err(NameError::TooLong { len: raw.len(), max: 64 });
        }
        if raw.starts_with('-') {
            return Err(NameError::LeadingDash);
        }
        let allowed = |character: char| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        };
        if !raw.chars().all(allowed) {
            return Err(NameError::NotPrintable);
        }
        Ok(DependencyName(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DependencyName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// A presence check.
///
/// Task 8 extends this to the full seven-variant enum of spec 5.2. The two
/// variants here are what Task 7's manifest tests exercise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {
    Command(CommandName),
    DirExists(dotfiles_path::CheckRelPath),
}

/// Why a check expression was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckParseError {
    Unrecognized,
    BadCommandName(NameError),
    BadPath(dotfiles_path::PathError),
}

/// Whether the conf file was chosen by platform detection or named
/// explicitly through `DEPS_CONF`.
///
/// Load-bearing rather than informational: spec 5.2 makes
/// `PythonImport` a parse error in a platform-selected file, so the sole
/// interpreter-spawning check can never reach the shell-startup path.
/// `deps-ci.conf:3-5` states that the file is selected only by an explicit
/// `DEPS_CONF`; nothing enforced it before.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfKind {
    PlatformSelected,
    ExplicitOnly,
}

/// One manifest entry.
///
/// `docs` lives here rather than on `NoInstallReason`, per spec 5.1: the URL
/// already lives in the manifest and a second home is a drift shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    pub name: DependencyName,
    pub check: Check,
    pub docs: DocsUrl,
}

/// The parsed manifest.
///
/// Order is the file's order, preserved because a report reads better in the
/// order the maintainer wrote. Uniqueness of names is a parse invariant, so
/// `get` cannot see two entries for one dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    entries: Vec<ManifestEntry>,
}

impl Manifest {
    pub fn entries(&self) -> &[ManifestEntry] {
        &self.entries
    }

    pub fn get(&self, name: &DependencyName) -> Option<&ManifestEntry> {
        self.entries.iter().find(|entry| &entry.name == name)
    }
}

/// Why a manifest was refused.
///
/// Every variant carries the 1-based line number, because the caller reports
/// a file it read and a message with no line is unactionable against a
/// 45-line conf file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    WrongFieldCount { line: usize, found: usize },
    BadName { line: usize, cause: NameError },
    BadCheck { line: usize, cause: CheckParseError },
    BadDocs { line: usize, cause: NameError },
    DuplicateName { line: usize, name: DependencyName },
    InterpreterCheckInPlatformConf { line: usize },
}

/// Parse manifest text into typed entries.
///
/// Takes `&str`, never a path: the caller reads the file, so this function
/// makes no syscall and the module holds no capability. `parse_manifest` is
/// the boundary spec 3.6 requires, replacing the `sh -c "$check"` at
/// `check-deps.sh:524` with a closed enum that has no shell escape hatch.
///
/// # Errors
///
/// Returns `ParseError` naming the 1-based line and the rule it broke. A
/// line with more than three pipe-separated fields is
/// `WrongFieldCount` rather than a silent truncation, which is the failure
/// `deps.conf:9-13` documents and cannot detect.
pub fn parse_manifest(text: &str, kind: ConfKind) -> Result<Manifest, ParseError> {
    let mut entries = Vec::new();
    let mut seen: BTreeSet<DependencyName> = BTreeSet::new();

    for (index, raw_line) in text.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let fields: Vec<&str> = trimmed.split('|').collect();
        let [raw_name, raw_check, raw_docs] = fields.as_slice() else {
            return Err(ParseError::WrongFieldCount { line, found: fields.len() });
        };

        let name = DependencyName::parse(raw_name)
            .map_err(|cause| ParseError::BadName { line, cause })?;
        let check = parse_check(raw_check, kind, line)?;
        let docs = DocsUrl::parse(raw_docs)
            .map_err(|cause| ParseError::BadDocs { line, cause })?;

        if !seen.insert(name.clone()) {
            return Err(ParseError::DuplicateName { line, name });
        }
        entries.push(ManifestEntry { name, check, docs });
    }

    Ok(Manifest { entries })
}

/// Recognize a check expression.
///
/// Task 8 replaces this body with the full grammar of spec 5.2. It is
/// written here only far enough to parse the two shapes Task 7's tests use,
/// and `kind` is threaded through now so Task 8's `PythonImport` rule has
/// the argument it needs without a signature change.
fn parse_check(raw: &str, kind: ConfKind, line: usize) -> Result<Check, ParseError> {
    let _ = kind;
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix("command -v ") {
        let name = CommandName::parse(rest.trim()).map_err(|cause| ParseError::BadCheck {
            line,
            cause: CheckParseError::BadCommandName(cause),
        })?;
        return Ok(Check::Command(name));
    }
    if let Some(rest) = home_dir_test(trimmed) {
        let path = dotfiles_path::CheckRelPath::parse(rest).map_err(|cause| {
            ParseError::BadCheck { line, cause: CheckParseError::BadPath(cause) }
        })?;
        return Ok(Check::DirExists(path));
    }
    Err(ParseError::BadCheck { line, cause: CheckParseError::Unrecognized })
}

/// Extract the `$HOME`-relative path from `[ -d "$HOME/<rest>" ]`.
fn home_dir_test(raw: &str) -> Option<&str> {
    raw.strip_prefix("[ -d \"$HOME/")?.strip_suffix("\" ]")
}
```

- [ ] **Step 8: Run the test to verify it passes**

Run: `cd ~/crates && cargo test --locked -p deps-core && cargo clippy --locked --all-targets -- -D warnings`
Expected: PASS. Also run `cargo test --locked --workspace` once, to confirm
adding a member did not disturb `config-manifest`.

- [ ] **Step 9: Assert the no-IO property mechanically**

The claim "this crate performs no IO" needs a check that survives a future
edit, not a comment. Add to `crates/deps-core/src/lib.rs`:

```rust
#[cfg(test)]
mod purity {
    /// The crate source must name no IO capability.
    ///
    /// `include_str!` reads at compile time, so this test spawns nothing and
    /// opens nothing at runtime. It is the mechanism spec 7.1 says the crate
    /// boundary buys, made checkable inside the crate as well: a `deps-core`
    /// that grows a `std::fs` call fails here before it fails a review.
    #[test]
    fn no_module_names_an_io_capability() {
        let sources = [
            ("lib.rs", include_str!("lib.rs")),
            ("manifest.rs", include_str!("manifest.rs")),
        ];
        for (file_name, source) in sources {
            for forbidden in ["std::fs", "std::process", "std::env", "std::io"] {
                assert!(
                    !source.contains(forbidden),
                    "{file_name} names {forbidden}; deps-core performs no IO"
                );
            }
        }
    }
}
```

Run: `cd ~/crates && cargo test --locked -p deps-core purity::`
Expected: PASS. Tasks 6 through 9 each add their new module to the
`sources` array in the same commit that adds the module.

- [ ] **Step 10: Commit**

```
config add crates/Cargo.toml crates/deps-core crates/dotfiles-path/src/lib.rs crates/dotfiles-path/src/name.rs
config commit -m "Add deps-core with the manifest as pure data

deps.conf field 2 is an arbitrary shell string that check-deps.sh:524
evaluates with sh -c, reached through depcheck-hook.sh with no --dry-run
gate. config sync writes across branches with commit-tree, which runs no
hooks, so an edited conf file arrives without passing pre-commit. A typed
manifest is what closes that path, and it has to exist before the closed
check enum can be held anywhere.

parse_manifest takes &str, not a path, so the module holds no capability.
config-manifest/src/doctor.rs is the in-repo precedent: 262 lines, zero
references to std::fs, std::process, std::env or std::io, because the
gathering lives in git.rs.

deps.conf:9-13 documents that an extra pipe in the check field truncates the
check and leaks the remainder into docs_url, and that IFS='|' read cannot
detect it. WrongFieldCount detects it.

DocsUrl is scheme-agnostic because deps-ci.conf:23 is
http://gondor.apana.org.au/~herbert/dash/. An https-only type cannot parse
the manifest this repo ships.

CommandName, ModuleName, GlobPattern, PackageId and DocsUrl land in
dotfiles-path, which spec 7.1 lists as their home and which exported only
CheckRelPath before this commit.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Presence checks, three-state observation, and pure evaluation

Spec 5.2 counts all 22 checks and I re-counted them by reading the four conf
files: 16 in `deps.conf`, 1 in `deps-mac.conf`, 2 in `deps-linux.conf`, 3 in
`deps-ci.conf`. The top-level distribution matches the spec's table:
15 `Command`, 2 `DirExists` (`tpm` at `deps.conf:32`, `oh-my-zsh` at
`deps-linux.conf:12`), 1 `FileNonEmpty` (`nvm` at `deps.conf:36`, which uses
`-s` not `-f`), 1 `PythonImport` (`pyyaml` at `deps-ci.conf:22`), and 3
`AnyOf` (`alacritty` at `deps.conf:24`, `zsh-autosuggestions` at `:26`,
`node` at `:45`). All three `AnyOf` entries have exactly two branches.

The live bug spec 5.2 fixes is real and I read it: `deps.conf:26` is one
`test` with two `-f` operands joined by `-o`, and the second operand is
`"$(brew --prefix 2>/dev/null)/share/zsh-autosuggestions/zsh-autosuggestions.zsh"`.
On a machine with no brew the substitution is empty and the test becomes
`test -f /share/zsh-autosuggestions/zsh-autosuggestions.zsh` against the
filesystem root. `PathRoot` as a closed sum deletes the expansion, and
`Observation::Unresolvable { root }` is what distinguishes "brew is absent so
this check is unanswerable" from "the file is missing," because those have
different remedies. `PathRoot::MacApplications` replaces the first draft's
open `Absolute` variant, and I confirmed the only absolute path in the whole
corpus is `/Applications/Alacritty.app` at `deps.conf:24`.

`AnyOf { first: Box<Check>, rest: Vec<Check> }` rather than
`AnyOf(Vec<Check>)` because `AnyOf(vec![])` is a well-typed value that
evaluates false under any reasonable rule, so a manifest entry parsing to it
reports a dependency permanently missing with no diagnostic and nags forever
with no way to satisfy it. Non-emptiness is structural here.

**Files:**
- Create: `crates/deps-core/src/check.rs`
- Modify: `crates/deps-core/src/manifest.rs`
- Modify: `crates/deps-core/src/lib.rs`

**Interfaces:**
- Consumes: `deps_core::{ConfKind, ParseError}` and the `parse_check(raw,
  kind, line) -> Result<Check, ParseError>` seam from Task 7;
  `dotfiles_path::{CheckRelPath, CommandName, GlobPattern, ModuleName}`.
- Produces:
  - `pub enum PathRoot { Home, MacApplications, BrewPrefix, OhMyZshCustom }`
  - `pub struct CheckPath { pub root: PathRoot, pub rest: CheckRelPath }`
    with `pub fn new(root: PathRoot, rest: CheckRelPath) -> Self`
  - `pub enum Check { Command(CommandName), DirExists(CheckPath),
    FileExists(CheckPath), FileNonEmpty(CheckPath),
    GlobExists { dir: CheckPath, pattern: GlobPattern },
    PythonImport(ModuleName),
    AnyOf { first: Box<Check>, rest: Vec<Check> } }`
  - `pub enum Observation { Present, Absent, Unresolvable { root: PathRoot } }`
  - `pub trait Observations { fn observe(&self, check: &Check) -> Observation; }`
    plus `pub struct ObservationMap(BTreeMap<Check, Observation>)` with
    `pub fn from_pairs(pairs: Vec<(Check, Observation)>) -> Self` and an
    `Observations` impl
  - `pub fn evaluate(check: &Check, observed: &impl Observations) -> Observation`
  - `pub enum CheckParseError { Unrecognized, BadCommandName(NameError),
    BadPath(PathError), BadModuleName(NameError), BadGlob(NameError),
    EmptyAlternation, InterpreterCheck }`

`evaluate` is the only place `AnyOf` semantics live, and it is total over the
three-state observation. `Observations` is a trait rather than a bare map so
Task 11's scripted sequence of worlds can be a plain struct in a test without
building a map keyed on every leaf.

- [ ] **Step 1: Write the failing test for `AnyOf` three-state evaluation**

In `crates/deps-core/src/check.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use dotfiles_path::{CheckRelPath, CommandName};

    fn command(name: &str) -> Check {
        Check::Command(CommandName::parse(name).expect("a test command name parses"))
    }

    fn home_file(rest: &str) -> CheckPath {
        CheckPath::new(
            PathRoot::Home,
            CheckRelPath::parse(rest).expect("a test path parses"),
        )
    }

    fn brew_file(rest: &str) -> CheckPath {
        CheckPath::new(
            PathRoot::BrewPrefix,
            CheckRelPath::parse(rest).expect("a test path parses"),
        )
    }

    // deps.conf:26, the real zsh-autosuggestions check, as the two-branch
    // AnyOf it decomposes to.
    fn real_zsh_autosuggestions_check() -> Check {
        Check::AnyOf {
            first: Box::new(Check::FileExists(home_file(
                ".oh-my-zsh/custom/plugins/zsh-autosuggestions/zsh-autosuggestions.zsh",
            ))),
            rest: vec![Check::FileExists(brew_file(
                "share/zsh-autosuggestions/zsh-autosuggestions.zsh",
            ))],
        }
    }

    #[test]
    fn any_of_is_present_when_the_first_branch_is_present() {
        let check = real_zsh_autosuggestions_check();
        let Check::AnyOf { first, .. } = &check else {
            panic!("the fixture is an AnyOf");
        };
        let observed = ObservationMap::from_pairs(vec![(
            (**first).clone(),
            Observation::Present,
        )]);
        assert_eq!(evaluate(&check, &observed), Observation::Present);
    }

    #[test]
    fn any_of_is_present_when_a_later_branch_is_present() {
        let check = real_zsh_autosuggestions_check();
        let Check::AnyOf { first, rest } = &check else {
            panic!("the fixture is an AnyOf");
        };
        let observed = ObservationMap::from_pairs(vec![
            ((**first).clone(), Observation::Absent),
            (rest[0].clone(), Observation::Present),
        ]);
        assert_eq!(evaluate(&check, &observed), Observation::Present);
    }

    // The bug spec 5.2 fixes. On a machine with no brew the shell
    // substitution at deps.conf:26 is empty, so the second operand tests
    // /share/... at the filesystem root. Present is the wrong answer and
    // Absent is also wrong: the branch is unanswerable, and the remedy for
    // "brew is missing" differs from the remedy for "the file is missing".
    #[test]
    fn any_of_reports_unresolvable_when_no_branch_is_present_and_one_root_is_unresolvable() {
        let check = real_zsh_autosuggestions_check();
        let Check::AnyOf { first, rest } = &check else {
            panic!("the fixture is an AnyOf");
        };
        let observed = ObservationMap::from_pairs(vec![
            ((**first).clone(), Observation::Absent),
            (
                rest[0].clone(),
                Observation::Unresolvable { root: PathRoot::BrewPrefix },
            ),
        ]);
        assert_eq!(
            evaluate(&check, &observed),
            Observation::Unresolvable { root: PathRoot::BrewPrefix },
            "an unanswerable branch must not collapse to Absent"
        );
    }

    #[test]
    fn any_of_is_absent_only_when_every_branch_is_absent() {
        let check = real_zsh_autosuggestions_check();
        let Check::AnyOf { first, rest } = &check else {
            panic!("the fixture is an AnyOf");
        };
        let observed = ObservationMap::from_pairs(vec![
            ((**first).clone(), Observation::Absent),
            (rest[0].clone(), Observation::Absent),
        ]);
        assert_eq!(evaluate(&check, &observed), Observation::Absent);
    }

    // A leaf nobody observed is Absent, not a panic. gather runs at the
    // edge and a leaf it skipped is a leaf whose subject is not there.
    #[test]
    fn an_unobserved_leaf_is_absent() {
        let observed = ObservationMap::from_pairs(vec![]);
        assert_eq!(evaluate(&command("git"), &observed), Observation::Absent);
    }

    // deps.conf:36 uses -s, not -f. A truncated nvm.sh passes -f and
    // sources to nothing, so collapsing the two variants would introduce a
    // bug during the port.
    #[test]
    fn file_non_empty_is_a_distinct_check_from_file_exists() {
        let path = home_file(".nvm/nvm.sh");
        let non_empty = Check::FileNonEmpty(path.clone());
        let exists = Check::FileExists(path);
        assert_ne!(non_empty, exists);
        let observed = ObservationMap::from_pairs(vec![(exists, Observation::Present)]);
        assert_eq!(
            evaluate(&non_empty, &observed),
            Observation::Absent,
            "a satisfied -f must not satisfy a -s"
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd ~/crates && cargo test --locked -p deps-core check::`
Expected: FAIL with `error[E0425]: cannot find function `evaluate`` plus
unresolved `PathRoot`, `CheckPath`, `ObservationMap`, `Observation`. The
missing behavior is the evaluator and the three-state observation, not an
import.

- [ ] **Step 3: Implement the check types and the evaluator**

Prepend to `crates/deps-core/src/check.rs`:

```rust
use std::collections::BTreeMap;

use dotfiles_path::{CheckRelPath, CommandName, GlobPattern, ModuleName};

/// A root a check path is joined onto.
///
/// A closed sum, which is what deletes all shell expansion from the check
/// field. `deps.conf:26` embeds `$(brew --prefix 2>/dev/null)`, and on a
/// machine with no brew that substitution is empty, so the real test runs
/// against the filesystem root. A root that fails to resolve is
/// `Observation::Unresolvable`, not false.
///
/// `MacApplications` rather than an open `Absolute` variant: the only
/// absolute path in the whole corpus is `/Applications/Alacritty.app`
/// (`deps.conf:24`), and a named variant per need makes each addition a
/// reviewable decision that states its own blast radius.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PathRoot {
    Home,
    MacApplications,
    BrewPrefix,
    OhMyZshCustom,
}

/// A path to check: a closed root plus a validated relative remainder.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CheckPath {
    pub root: PathRoot,
    pub rest: CheckRelPath,
}

impl CheckPath {
    pub fn new(root: PathRoot, rest: CheckRelPath) -> Self {
        CheckPath { root, rest }
    }
}

/// A presence check.
///
/// There is no `Shell` variant. All 22 real checks fit these seven, verified
/// by reading the four conf files, and a named variant per need is strictly
/// better than an escape hatch that grants all future blast radius at once.
///
/// `AnyOf` carries `first` and `rest` rather than one `Vec`, because
/// `AnyOf(vec![])` is a well-typed value that evaluates false under any
/// rule, and a manifest entry parsing to it would report a dependency
/// permanently missing with no diagnostic. All three real `AnyOf` entries
/// have exactly two branches.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Check {
    Command(CommandName),
    DirExists(CheckPath),
    FileExists(CheckPath),
    /// `deps.conf:36` uses `-s`, not `-f`: a truncated `nvm.sh` passes `-f`
    /// and sources to nothing.
    FileNonEmpty(CheckPath),
    GlobExists { dir: CheckPath, pattern: GlobPattern },
    PythonImport(ModuleName),
    AnyOf { first: Box<Check>, rest: Vec<Check> },
}

/// What was observed about one check.
///
/// Three-state rather than boolean. `reconcile` must distinguish "brew is
/// absent so this check is unanswerable" from "the file is missing", because
/// they have different remedies, and a boolean collapses the first into the
/// second silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observation {
    Present,
    Absent,
    Unresolvable { root: PathRoot },
}

/// What the edge observed, injected into the core.
///
/// A trait rather than a concrete map so a test can script a world as a
/// small struct without enumerating every leaf, and so the core never holds
/// the capability that produced the answers. `gather` implements this at the
/// edge; nothing in this crate does.
pub trait Observations {
    fn observe(&self, check: &Check) -> Observation;
}

/// An observation map built from explicit pairs.
///
/// Absence means `Absent`: `gather` runs at the edge over the selected
/// manifest, and a leaf it did not record is a leaf whose subject is not
/// there.
#[derive(Debug, Clone, Default)]
pub struct ObservationMap(BTreeMap<Check, Observation>);

impl ObservationMap {
    pub fn from_pairs(pairs: Vec<(Check, Observation)>) -> Self {
        ObservationMap(pairs.into_iter().collect())
    }
}

impl Observations for ObservationMap {
    fn observe(&self, check: &Check) -> Observation {
        self.0.get(check).copied().unwrap_or(Observation::Absent)
    }
}

/// Evaluate a check against an injected observation.
///
/// Pure: the observation comes in as an argument, so this function opens no
/// file and spawns no process. `AnyOf` short-circuits on the first
/// `Present`, and otherwise prefers a reported `Unresolvable` over `Absent`,
/// because an unanswerable branch collapsed to `Absent` is exactly the
/// silent-false bug `deps.conf:26` has today.
pub fn evaluate(check: &Check, observed: &impl Observations) -> Observation {
    let Check::AnyOf { first, rest } = check else {
        return observed.observe(check);
    };

    let mut unresolved_root = None;
    for branch in std::iter::once(first.as_ref()).chain(rest.iter()) {
        match evaluate(branch, observed) {
            Observation::Present => return Observation::Present,
            Observation::Unresolvable { root } => unresolved_root = unresolved_root.or(Some(root)),
            Observation::Absent => {}
        }
    }
    match unresolved_root {
        Some(root) => Observation::Unresolvable { root },
        None => Observation::Absent,
    }
}
```

Extend `crates/deps-core/src/lib.rs`:

```rust
mod check;
mod manifest;

pub use check::{
    Check, CheckPath, Observation, ObservationMap, Observations, PathRoot, evaluate,
};
pub use manifest::{
    ConfKind, DependencyName, Manifest, ManifestEntry, ParseError, parse_manifest,
};
```

Delete the placeholder `Check` and `CheckParseError` from `manifest.rs` and
`use crate::check::Check;` there instead. Add `("check.rs",
include_str!("check.rs"))` to the `purity` test's `sources` array.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd ~/crates && cargo test --locked -p deps-core check::`
Expected: PASS, six tests.

- [ ] **Step 5: Write the failing test for the check grammar**

Append to `crates/deps-core/src/check.rs`'s test module:

```rust
    // Every check expression in the four conf files, verbatim. Read from
    // deps.conf, deps-mac.conf, deps-linux.conf and deps-ci.conf, and each
    // is asserted to the variant spec 5.2's table names for it.
    #[test]
    fn parses_every_real_check_expression() {
        let cases: [(&str, fn(&Check) -> bool); 8] = [
            ("command -v git", |check| matches!(check, Check::Command(_))),
            (
                "[ -d \"$HOME/.tmux/plugins/tpm\" ]",
                |check| matches!(check, Check::DirExists(_)),
            ),
            (
                "[ -d \"$HOME/.oh-my-zsh\" ]",
                |check| matches!(check, Check::DirExists(_)),
            ),
            (
                "[ -s \"$HOME/.nvm/nvm.sh\" ]",
                |check| matches!(check, Check::FileNonEmpty(_)),
            ),
            (
                "python3 -c \"import yaml\"",
                |check| matches!(check, Check::PythonImport(_)),
            ),
            (
                "if test -d /Applications/Alacritty.app; then true; else command -v alacritty; fi",
                |check| matches!(check, Check::AnyOf { .. }),
            ),
            (
                "test -f \"$HOME/.oh-my-zsh/custom/plugins/zsh-autosuggestions/zsh-autosuggestions.zsh\" -o -f \"$(brew --prefix 2>/dev/null)/share/zsh-autosuggestions/zsh-autosuggestions.zsh\"",
                |check| matches!(check, Check::AnyOf { .. }),
            ),
            (
                "if command -v node; then true; else ls -d \"$HOME/.nvm/versions/node\"/v* >/dev/null 2>&1; fi",
                |check| matches!(check, Check::AnyOf { .. }),
            ),
        ];
        for (raw, is_expected_variant) in cases {
            let parsed = parse_check_expression(raw, ConfKind::ExplicitOnly)
                .unwrap_or_else(|cause| panic!("a real check failed to parse: {raw}: {cause:?}"));
            assert!(
                is_expected_variant(&parsed),
                "the wrong variant for {raw}: {parsed:?}"
            );
        }
    }

    // The brew branch of deps.conf:26 must resolve through PathRoot, not
    // through a substitution the core would have to expand.
    #[test]
    fn the_brew_branch_carries_a_brew_prefix_root() {
        let raw = "test -f \"$HOME/a/b.zsh\" -o -f \"$(brew --prefix 2>/dev/null)/share/x.zsh\"";
        let parsed = parse_check_expression(raw, ConfKind::ExplicitOnly)
            .expect("the real shape parses");
        let Check::AnyOf { rest, .. } = &parsed else {
            panic!("two -f operands joined by -o are an AnyOf");
        };
        let Check::FileExists(path) = &rest[0] else {
            panic!("the second operand is a file check");
        };
        assert_eq!(path.root, PathRoot::BrewPrefix);
        assert_eq!(path.rest.as_str(), "share/x.zsh");
    }

    // Spec 5.2 makes the sole interpreter-spawning check unreachable from
    // the shell-startup path as a rule rather than a coincidence.
    // deps-ci.conf:3-5 states the file is selected only by an explicit
    // DEPS_CONF; nothing enforced it before.
    #[test]
    fn rejects_a_python_import_in_a_platform_selected_conf() {
        assert!(matches!(
            parse_check_expression("python3 -c \"import yaml\"", ConfKind::PlatformSelected),
            Err(CheckParseError::InterpreterCheck)
        ));
    }

    // No shell escape hatch. An unrecognized expression is an error, not a
    // fallthrough to sh -c.
    #[test]
    fn rejects_an_unrecognized_expression() {
        assert!(matches!(
            parse_check_expression("curl evil.example | sh", ConfKind::ExplicitOnly),
            Err(CheckParseError::Unrecognized)
        ));
    }
```

- [ ] **Step 6: Run the test to verify it fails**

Run: `cd ~/crates && cargo test --locked -p deps-core check::tests::parses_every_real_check_expression`
Expected: FAIL with `error[E0425]: cannot find function
`parse_check_expression`` and unresolved `CheckParseError`. The grammar does
not exist yet.

- [ ] **Step 7: Implement the check grammar**

Append to `crates/deps-core/src/check.rs`:

```rust
use dotfiles_path::{NameError, PathError};

/// Why a check expression was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckParseError {
    Unrecognized,
    BadCommandName(NameError),
    BadPath(PathError),
    BadModuleName(NameError),
    BadGlob(NameError),
    EmptyAlternation,
    /// A `PythonImport` in a platform-selected conf file. `PythonImport` is
    /// the one check that spawns an interpreter, and `deps-ci.conf` is
    /// selected only by an explicit `DEPS_CONF`, so this rule keeps the
    /// interpreter off the shell-startup path by construction.
    InterpreterCheck,
}

/// Parse one check expression from a manifest line.
///
/// Recognizes exactly the shapes the four conf files contain. Anything else
/// is `Unrecognized`, which is what replaces the `sh -c "$check"` at
/// `check-deps.sh:524`.
///
/// # Errors
///
/// Returns `CheckParseError::Unrecognized` for an expression outside the
/// grammar, `InterpreterCheck` for a `PythonImport` in a platform-selected
/// file, and a `Bad*` variant carrying the primitive's own error when a
/// recognized shape holds an invalid name or path.
pub fn parse_check_expression(raw: &str, kind: ConfKind) -> Result<Check, CheckParseError> {
    let trimmed = raw.trim();

    if let Some(alternation) = parse_if_then_else(trimmed, kind)? {
        return Ok(alternation);
    }
    if let Some(alternation) = parse_test_or(trimmed, kind)? {
        return Ok(alternation);
    }
    parse_leaf(trimmed, kind)
}

/// `if <a>; then true; else <b>; fi`, the shape at `deps.conf:24` and `:45`.
fn parse_if_then_else(raw: &str, kind: ConfKind) -> Result<Option<Check>, CheckParseError> {
    let Some(body) = raw.strip_prefix("if ") else {
        return Ok(None);
    };
    let Some(body) = body.strip_suffix("; fi") else {
        return Err(CheckParseError::Unrecognized);
    };
    let Some((consequent_source, alternative)) = body.split_once("; then true; else ") else {
        return Err(CheckParseError::Unrecognized);
    };
    let first = parse_leaf(consequent_source.trim(), kind)?;
    let second = parse_leaf(alternative.trim(), kind)?;
    Ok(Some(Check::AnyOf { first: Box::new(first), rest: vec![second] }))
}

/// `test -f "A" -o -f "B"`, the shape at `deps.conf:26`.
fn parse_test_or(raw: &str, kind: ConfKind) -> Result<Option<Check>, CheckParseError> {
    let Some(body) = raw.strip_prefix("test ") else {
        return Ok(None);
    };
    let mut operands = body.split(" -o ");
    let Some(head) = operands.next() else {
        return Err(CheckParseError::EmptyAlternation);
    };
    let first = parse_test_operand(head.trim())?;
    let mut rest = Vec::new();
    for tail in operands {
        rest.push(parse_test_operand(tail.trim())?);
    }
    if rest.is_empty() {
        // A one-operand `test` is a leaf, not an alternation, and building an
        // AnyOf with an empty `rest` would misreport the structure.
        let _ = kind;
        return Ok(Some(first));
    }
    Ok(Some(Check::AnyOf { first: Box::new(first), rest }))
}

fn parse_test_operand(raw: &str) -> Result<Check, CheckParseError> {
    if let Some(quoted) = raw.strip_prefix("-f ") {
        return Ok(Check::FileExists(parse_quoted_path(quoted.trim())?));
    }
    if let Some(quoted) = raw.strip_prefix("-d ") {
        return Ok(Check::DirExists(parse_quoted_path(quoted.trim())?));
    }
    if let Some(quoted) = raw.strip_prefix("-s ") {
        return Ok(Check::FileNonEmpty(parse_quoted_path(quoted.trim())?));
    }
    Err(CheckParseError::Unrecognized)
}

fn parse_leaf(raw: &str, kind: ConfKind) -> Result<Check, CheckParseError> {
    if let Some(name) = raw.strip_prefix("command -v ") {
        let parsed = CommandName::parse(name.trim()).map_err(CheckParseError::BadCommandName)?;
        return Ok(Check::Command(parsed));
    }
    if let Some(module) = raw
        .strip_prefix("python3 -c \"import ")
        .and_then(|rest| rest.strip_suffix('"'))
    {
        if kind == ConfKind::PlatformSelected {
            return Err(CheckParseError::InterpreterCheck);
        }
        let parsed = ModuleName::parse(module.trim()).map_err(CheckParseError::BadModuleName)?;
        return Ok(Check::PythonImport(parsed));
    }
    if let Some(glob) = parse_glob_listing(raw)? {
        return Ok(glob);
    }
    if let Some(bracket) = raw.strip_prefix("[ ").and_then(|rest| rest.strip_suffix(" ]")) {
        return parse_test_operand(bracket.trim());
    }
    if raw.starts_with("test ") {
        // `test -d /Applications/Alacritty.app`, the one unquoted absolute
        // operand in the corpus.
        return parse_test_operand(raw.trim_start_matches("test ").trim());
    }
    Err(CheckParseError::Unrecognized)
}

/// `ls -d "$HOME/.nvm/versions/node"/v* >/dev/null 2>&1`, `deps.conf:45`.
fn parse_glob_listing(raw: &str) -> Result<Option<Check>, CheckParseError> {
    let Some(body) = raw.strip_prefix("ls -d ") else {
        return Ok(None);
    };
    let body = body
        .strip_suffix(" >/dev/null 2>&1")
        .unwrap_or(body)
        .trim();
    let Some((quoted, pattern)) = split_after_closing_quote(body) else {
        return Err(CheckParseError::Unrecognized);
    };
    let dir = parse_quoted_path(quoted)?;
    let pattern = pattern
        .strip_prefix('/')
        .ok_or(CheckParseError::Unrecognized)?;
    let parsed = GlobPattern::parse(pattern).map_err(CheckParseError::BadGlob)?;
    Ok(Some(Check::GlobExists { dir, pattern: parsed }))
}

fn split_after_closing_quote(raw: &str) -> Option<(&str, &str)> {
    let rest = raw.strip_prefix('"')?;
    let close = rest.find('"')?;
    Some((&raw[..close + 2], &rest[close + 1..]))
}

/// Resolve a quoted operand to a closed root plus a relative remainder.
///
/// This is where the shell expansion is deleted. `$(brew --prefix
/// 2>/dev/null)` becomes `PathRoot::BrewPrefix` rather than text the core
/// would have to expand, and an unresolvable brew is then an
/// `Observation::Unresolvable` at gather time instead of a test against `/`.
fn parse_quoted_path(raw: &str) -> Result<CheckPath, CheckParseError> {
    let inner = raw.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')).unwrap_or(raw);

    let roots: [(&str, PathRoot); 4] = [
        ("$(brew --prefix 2>/dev/null)/", PathRoot::BrewPrefix),
        ("${ZSH_CUSTOM:-$HOME/.oh-my-zsh/custom}/", PathRoot::OhMyZshCustom),
        ("$HOME/", PathRoot::Home),
        ("/Applications/", PathRoot::MacApplications),
    ];
    for (prefix, root) in roots {
        if let Some(rest) = inner.strip_prefix(prefix) {
            let parsed = CheckRelPath::parse(rest).map_err(CheckParseError::BadPath)?;
            return Ok(CheckPath::new(root, parsed));
        }
    }
    Err(CheckParseError::Unrecognized)
}
```

In `manifest.rs`, replace Task 7's placeholder `parse_check` body with a
delegation, so `ParseError::BadCheck` wraps the real error:

```rust
fn parse_check(raw: &str, kind: ConfKind, line: usize) -> Result<Check, ParseError> {
    crate::check::parse_check_expression(raw, kind)
        .map_err(|cause| ParseError::BadCheck { line, cause })
}
```

Re-export `parse_check_expression` and `CheckParseError` from `lib.rs`.

- [ ] **Step 8: Run the test to verify it passes**

Run: `cd ~/crates && cargo test --locked -p deps-core && cargo clippy --locked --all-targets -- -D warnings`
Expected: PASS. The manifest tests from Task 7 still pass, because
`parse_manifest`'s signature did not change.

- [ ] **Step 9: Commit**

```
config add crates/deps-core/src/check.rs crates/deps-core/src/manifest.rs crates/deps-core/src/lib.rs
config commit -m "Port the 22 presence checks to a closed enum with no shell escape

deps.conf:26 is one test with two -f operands joined by -o, and the second
operand is \"\$(brew --prefix 2>/dev/null)/share/...\". On a machine with no
brew that substitution is empty, so the real command becomes
test -f /share/zsh-autosuggestions/zsh-autosuggestions.zsh against the
filesystem root. PathRoot as a closed sum deletes the expansion, and
Observation::Unresolvable distinguishes \"brew is absent so this is
unanswerable\" from \"the file is missing\": those have different remedies,
and a boolean collapses the first into the second silently.

AnyOf carries first and rest rather than one Vec. AnyOf(vec![]) is a
well-typed value that evaluates false under any rule, so a manifest entry
parsing to it would report a dependency permanently missing with no
diagnostic and nag forever with no way to satisfy it. All three real AnyOf
entries have exactly two branches, so nothing is lost.

FileNonEmpty stays distinct from FileExists because deps.conf:36 uses -s,
not -f: a truncated nvm.sh passes -f and sources to nothing.

MacApplications replaces an open Absolute variant. The only absolute path in
the whole corpus is /Applications/Alacritty.app at deps.conf:24.

A PythonImport in a platform-selected conf file is a parse error, so the one
interpreter-spawning check can never reach the shell-startup path.
deps-ci.conf:3-5 already states the file is selected only by an explicit
DEPS_CONF; nothing enforced it.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: The plan, with privilege as data and the requirement graph

Spec 3.5 puts the privilege requirement on the plan step, derived purely from
the `(action, manager)` pair by `plan`. The first draft rejected a manifest
field (right: a manifest could claim otherwise and be wrong), rejected
adapter-only (right: `--dry-run` must disclose privileged steps before the
first password prompt), and rejected an `Executor::needs_elevation` query
(right: it opens a check-then-act gap), then left the driver with no way to
route a step and claimed the absence of a privileged installer made
privileged actions structurally unreachable. It did not.

Today the "no root available" case is enforced by sniffing the command text
for a literal `${SUDO}`, at `check-deps.sh:545`:
`if [ "$can_escalate" -eq 0 ] && [ "${cmd#*\$\{SUDO\}}" != "$cmd" ]`. That is
a runtime string test standing in for a type, and it is one refactor from
silently passing. The elevation three-state itself already exists at
`check-deps.sh:168-188`, including the `DEPS_FORCE_ROOT` override that exists
only so both branches are testable, so naming it a type deletes that
environment seam. Resolving it once at the edge is what keeps `--dry-run`
truthful: it makes the plan a deterministic function of one observation.

`plan` may order steps and may not condition an action on another step's
outcome (spec 6.3). That is why `plan` takes an observation map and returns
`Blocked { on }` for a step whose prerequisite is not yet satisfied, rather
than taking a callback. The requirement graph arrives as a separate argument
because, as the verification note above records, **no conf file has a
`requires` field**: spec 5.5's TOML example does not exist in this repo.
`deps.conf:18-20` is the evidence, stating in the file itself that "no
ordering between it and zsh-autosuggestions is guaranteed here." Adding the
field to the pipe format is a later decision; `plan` is written against an
explicit graph so the port does not have to invent a manifest column.

`plan` also returns events rather than logging (spec 4.1). Passing `Services`
into `plan` would falsify the no-IO claim and would contradict 3.3, which
rejected `Probe` for being exactly a trait the core invokes.

**Files:**
- Create: `crates/deps-core/src/action.rs`
- Create: `crates/deps-core/src/plan.rs`
- Modify: `crates/deps-core/src/lib.rs`

**Interfaces:**
- Consumes: `deps_core::{Check, DependencyName, Manifest, Observation,
  Observations, evaluate}` from Tasks 5 and 6.
- Produces, in `action.rs`:
  - `pub enum PackageManager { Apt, Brew, Pacman, Unknown }`
  - `pub enum BrewKind { Formula, Cask }`
  - `pub enum ScriptInstaller { Rustup, OhMyZsh, Zoxide }`
  - `pub enum CloneSource { Tpm, ZshAutosuggestions }`
  - `pub enum NoInstallReason { UpstreamPublishesNoStableUrl,
    RequiresInteractiveApproval, NotPackagedForThisManager,
    ManagerNotNamedInManifest { manager: PackageManager },
    PrivilegeUnavailable,
    PrerequisiteNotYetInstalled { dependency: DependencyName } }`
  - `pub enum InstallAction { Package { id: PackageId },
    Brew { kind: BrewKind, id: PackageId, tap: Option<TapName> },
    AptSource { keyring: KeyringSource, list: SourceListEntry },
    Pip { id: PackageId, break_system_packages: bool },
    Script { installer: ScriptInstaller },
    GitClone { source: CloneSource, into: CheckPath },
    NvmInstall, NotAutomatable { reason: NoInstallReason } }`
  - `pub enum KeyringSource { GithubCli }`,
    `pub enum SourceListEntry { GithubCli }`,
    `pub struct TapName(String)` with `parse`/`as_str`
  - `pub enum PackageAvailability { Named(PackageId),
    ViaScript(ScriptInstaller), Unavailable(NoInstallReason) }`
  - `pub struct PackageMap { per_manager: BTreeMap<PackageManager,
    PackageAvailability>, fallback: PackageAvailability }` with
    `pub fn new(per_manager: BTreeMap<PackageManager, PackageAvailability>,
    fallback: PackageAvailability) -> Self` and
    `pub fn resolve(&self, manager: PackageManager) -> &PackageAvailability`
- Produces, in `plan.rs`:
  - `pub enum Elevation { AlreadyRoot, ViaSudo, Unavailable }`
  - `pub enum PrivilegeRequirement { None, Root }`
  - `pub struct Step { pub dependency: DependencyName,
    pub action: InstallAction, pub privilege: PrivilegeRequirement }`
  - `pub struct Plan { pub steps: Vec<Step> }`
  - `pub struct Selection(BTreeSet<DependencyName>)` with
    `pub fn all(manifest: &Manifest) -> Self` and
    `pub fn named(names: Vec<DependencyName>) -> Self` and
    `pub fn contains(&self, name: &DependencyName) -> bool`
  - `pub struct Requirements(BTreeMap<DependencyName, Vec<DependencyName>>)`
    with `pub fn none() -> Self`,
    `pub fn from_pairs(pairs: Vec<(DependencyName, Vec<DependencyName>)>) -> Self`,
    `pub fn prerequisites(&self, of: &DependencyName) -> &[DependencyName]`
  - `pub enum Event { CheckSatisfied { dependency: DependencyName },
    CheckUnanswerable { dependency: DependencyName, root: PathRoot },
    StepPlanned { dependency: DependencyName,
    privilege: PrivilegeRequirement },
    StepBlocked { dependency: DependencyName, on: DependencyName },
    PrerequisiteNotSelected { dependency: DependencyName,
    on: DependencyName } }`
  - `pub enum PlanError { UnknownDependency { name: DependencyName,
    did_you_mean: Option<DependencyName> },
    MalformedSelector { raw: RawSelector },
    ManifestParse { path: CheckRelPath, detail: ParseError },
    ManifestVersion { found: u32, supported: u32 },
    RequirementCycle { chain: Vec<DependencyName> } }`
  - `pub struct RawSelector(String)` with `parse`/`as_str`
  - `pub fn plan(manifest: &Manifest, manager: PackageManager,
    selection: &Selection, requirements: &Requirements,
    observations: &impl Observations, elevation: Elevation,
    packages: &BTreeMap<DependencyName, PackageMap>)
    -> Result<(Plan, Vec<Event>), PlanError>`

`Blocked` is not an `InstallAction`; it is a `StepOutcome` (Task 10), so a
blocked dependency is a step the driver skips rather than an action it
performs. `plan` records blocking through `Event::StepBlocked` plus an
`InstallAction::NotAutomatable { reason: PrerequisiteNotYetInstalled }` only
for the dry-run reading, matching spec 5.5's distinction.

- [ ] **Step 1: Write the failing test for privilege as data**

In `crates/deps-core/src/plan.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Check, ObservationMap};
    use dotfiles_path::{CommandName, PackageId};

    fn dependency(name: &str) -> DependencyName {
        DependencyName::parse(name).expect("a test dependency name parses")
    }

    fn package(name: &str) -> PackageId {
        PackageId::parse(name).expect("a test package id parses")
    }

    fn manifest_of(names: &[&str]) -> Manifest {
        let text: String = names
            .iter()
            .map(|name| format!("{name}|command -v {name}|https://example.invalid/{name}\n"))
            .collect();
        crate::parse_manifest(&text, crate::ConfKind::PlatformSelected)
            .expect("a synthesized manifest parses")
    }

    fn packages_named(names: &[&str]) -> BTreeMap<DependencyName, PackageMap> {
        names
            .iter()
            .map(|name| {
                let mut per_manager = BTreeMap::new();
                per_manager.insert(
                    PackageManager::Apt,
                    PackageAvailability::Named(package(name)),
                );
                per_manager.insert(
                    PackageManager::Brew,
                    PackageAvailability::Named(package(name)),
                );
                (
                    dependency(name),
                    PackageMap::new(
                        per_manager,
                        PackageAvailability::Unavailable(
                            NoInstallReason::ManagerNotNamedInManifest {
                                manager: PackageManager::Unknown,
                            },
                        ),
                    ),
                )
            })
            .collect()
    }

    // Privilege is a property of the manager, not of the dependency: apt
    // needs root and brew never does (spec 3.5). It is derived here rather
    // than sniffed out of command text, which is what check-deps.sh:545
    // does today.
    #[test]
    fn an_apt_package_step_is_marked_root_and_a_brew_step_is_not() {
        let manifest = manifest_of(&["ripgrep"]);
        let selection = Selection::all(&manifest);
        let packages = packages_named(&["ripgrep"]);
        let observations = ObservationMap::from_pairs(vec![]);

        let (apt_plan, _) = plan(
            &manifest,
            PackageManager::Apt,
            &selection,
            &Requirements::none(),
            &observations,
            Elevation::ViaSudo,
            &packages,
        )
        .expect("a one-entry plan succeeds");
        assert_eq!(apt_plan.steps.len(), 1);
        assert_eq!(apt_plan.steps[0].privilege, PrivilegeRequirement::Root);

        let (brew_plan, _) = plan(
            &manifest,
            PackageManager::Brew,
            &selection,
            &Requirements::none(),
            &observations,
            Elevation::ViaSudo,
            &packages,
        )
        .expect("a one-entry plan succeeds");
        assert_eq!(brew_plan.steps[0].privilege, PrivilegeRequirement::None);
    }

    // Elevation::Unavailable never emits a Root step at all. The condition
    // flows through the report and the exit code instead of being
    // discovered at perform time, and check-deps.sh:546-548 already prints
    // this message from its string sniff.
    #[test]
    fn elevation_unavailable_emits_privilege_unavailable_instead_of_a_root_step() {
        let manifest = manifest_of(&["ripgrep"]);
        let selection = Selection::all(&manifest);
        let packages = packages_named(&["ripgrep"]);
        let observations = ObservationMap::from_pairs(vec![]);

        let (built, _) = plan(
            &manifest,
            PackageManager::Apt,
            &selection,
            &Requirements::none(),
            &observations,
            Elevation::Unavailable,
            &packages,
        )
        .expect("a one-entry plan succeeds");
        assert_eq!(built.steps[0].privilege, PrivilegeRequirement::None);
        assert!(matches!(
            built.steps[0].action,
            InstallAction::NotAutomatable {
                reason: NoInstallReason::PrivilegeUnavailable
            }
        ));
        assert!(
            built
                .steps
                .iter()
                .all(|step| step.privilege == PrivilegeRequirement::None),
            "no Root step may be planned when elevation is unavailable"
        );
    }

    // AlreadyRoot still needs the requirement on the step, because the
    // driver's dispatch is an exhaustive match on it rather than a
    // predicate. Root here means "this must go to the privileged slot",
    // and when the process is already root that slot is the ordinary one.
    #[test]
    fn already_root_still_marks_an_apt_step_root() {
        let manifest = manifest_of(&["ripgrep"]);
        let packages = packages_named(&["ripgrep"]);
        let (built, _) = plan(
            &manifest,
            PackageManager::Apt,
            &Selection::all(&manifest),
            &Requirements::none(),
            &ObservationMap::from_pairs(vec![]),
            Elevation::AlreadyRoot,
            &packages,
        )
        .expect("a one-entry plan succeeds");
        assert_eq!(built.steps[0].privilege, PrivilegeRequirement::Root);
    }

    // A satisfied check is not a step. plan is a function of the injected
    // observation, so this is the whole no-IO surface of the planner.
    #[test]
    fn a_present_dependency_gets_no_step_and_one_event() {
        let manifest = manifest_of(&["ripgrep"]);
        let check = Check::Command(CommandName::parse("ripgrep").expect("a name parses"));
        let observations = ObservationMap::from_pairs(vec![(check, Observation::Present)]);
        let (built, events) = plan(
            &manifest,
            PackageManager::Brew,
            &Selection::all(&manifest),
            &Requirements::none(),
            &observations,
            Elevation::ViaSudo,
            &packages_named(&["ripgrep"]),
        )
        .expect("a one-entry plan succeeds");
        assert!(built.steps.is_empty());
        assert_eq!(
            events,
            vec![Event::CheckSatisfied { dependency: dependency("ripgrep") }]
        );
    }

    // plan returns events; it holds no logger. Passing Services in would
    // falsify the no-IO claim and contradict spec 3.3, which rejected Probe
    // for being a trait the core invokes.
    #[test]
    fn plan_is_deterministic_over_the_same_inputs() {
        let manifest = manifest_of(&["ripgrep", "fzf"]);
        let packages = packages_named(&["ripgrep", "fzf"]);
        let selection = Selection::all(&manifest);
        let observations = ObservationMap::from_pairs(vec![]);
        let run = || {
            plan(
                &manifest,
                PackageManager::Apt,
                &selection,
                &Requirements::none(),
                &observations,
                Elevation::ViaSudo,
                &packages,
            )
            .expect("a two-entry plan succeeds")
        };
        assert_eq!(run(), run());
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd ~/crates && cargo test --locked -p deps-core plan::`
Expected: FAIL with `error[E0425]: cannot find function `plan`` plus
unresolved `Elevation`, `PrivilegeRequirement`, `Selection`, `Requirements`,
`Event`, `PackageManager`, `PackageMap`. The planner does not exist.

- [ ] **Step 3: Implement the action types**

`crates/deps-core/src/action.rs`:

```rust
use std::collections::BTreeMap;
use std::fmt;

use dotfiles_path::{NameError, PackageId};

use crate::check::CheckPath;
use crate::manifest::DependencyName;

/// The detected package manager.
///
/// `Unknown` is a variant, not an error. `check-deps.sh:191-199` returns the
/// literal `unknown` and the script proceeds: every dependency reports
/// manual-only with its docs URL. On a fresh macOS box with no brew, which
/// is the machine `setup.sh` exists for, that list is the useful output and
/// aborting would replace it with one line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageManager {
    Apt,
    Brew,
    Pacman,
    Unknown,
}

impl PackageManager {
    /// Whether an install through this manager needs root.
    ///
    /// A property of the manager, never of the dependency: a manifest could
    /// claim otherwise and be wrong (spec 3.5). This function is what
    /// replaces the `${SUDO}` string sniff at `check-deps.sh:545`.
    pub fn needs_root(self) -> bool {
        match self {
            PackageManager::Apt | PackageManager::Pacman => true,
            PackageManager::Brew | PackageManager::Unknown => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BrewKind {
    Formula,
    Cask,
}

/// A closed set of script-installer identities.
///
/// Identities rather than URLs. The adapter maps each to a hardcoded URL, so
/// adding one is a code change that appears in a diff and passes pre-commit.
/// A manifest-supplied URL would make one edited line an arbitrary-code
/// vector, amplified by the `commit-tree` bypass in spec 3.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScriptInstaller {
    Rustup,
    OhMyZsh,
    Zoxide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CloneSource {
    Tpm,
    ZshAutosuggestions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum KeyringSource {
    GithubCli,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceListEntry {
    GithubCli,
}

/// A Homebrew tap name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TapName(String);

impl TapName {
    /// # Errors
    ///
    /// Returns `NameError::NotPrintable` outside `[A-Za-z0-9._/-]`. A tap
    /// name legitimately contains one `/`, unlike every other name type
    /// here, which is why it is not a `PackageId`.
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        if raw.is_empty() {
            return Err(NameError::Empty);
        }
        if raw.len() > 128 {
            return Err(NameError::TooLong { len: raw.len(), max: 128 });
        }
        let allowed = |character: char| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '/')
        };
        if !raw.chars().all(allowed) {
            return Err(NameError::NotPrintable);
        }
        Ok(TapName(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TapName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// Why a dependency has no automated install here.
///
/// A sum, not a comment. `NotAutomatable` with no reason cannot distinguish
/// a permanent upstream fact from a regression nobody noticed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoInstallReason {
    /// nvm: `check-deps.sh:370-374` records that nvm's own docs publish only
    /// version-pinned install URLs, so a hardcoded one would go stale.
    UpstreamPublishesNoStableUrl,
    /// cc on macOS: the install needs an interactive Xcode prompt.
    RequiresInteractiveApproval,
    /// dash on brew: not packaged under this name and macOS ships none.
    NotPackagedForThisManager,
    /// The manifest author named no package for this manager. Distinct from
    /// `NotPackagedForThisManager`, which is a claim about upstream, and
    /// fabricating that claim from an omission would be a false positive.
    ManagerNotNamedInManifest { manager: PackageManager },
    /// The step needs root and this machine has neither root nor sudo.
    PrivilegeUnavailable,
    /// Not in this wave. Named "NotYetInstalled" rather than "Missing"
    /// because under the fixpoint a later wave can change the answer, which
    /// is a different claim from "this can never be automated".
    PrerequisiteNotYetInstalled { dependency: DependencyName },
}

/// What it means to install a dependency.
///
/// Intent, never a command string: the core knows what it means to do, not
/// how the command is written.
///
/// `plan` returns an `InstallAction` for every missing dependency, never
/// `Option<InstallAction>`. That totality is what fixes today's bug, where
/// "no install" is signalled by an empty string that `check-deps.sh:569`
/// cannot distinguish from a missing prerequisite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallAction {
    Package { id: PackageId },
    Brew { kind: BrewKind, id: PackageId, tap: Option<TapName> },
    /// `check-deps.sh:236` is not a package install. One line installs
    /// wget, creates `/etc/apt/keyrings` mode 755, fetches
    /// `githubcli-archive-keyring.gpg` from `cli.github.com`, tees it under
    /// sudo, appends a deb line to
    /// `/etc/apt/sources.list.d/github-cli.list`, runs `apt-get update`,
    /// and only then installs. Collapsing that to `Package { id: "gh" }`
    /// would let `describe` print "install package gh" for an action that
    /// permanently adds a third-party APT trust root.
    AptSource { keyring: KeyringSource, list: SourceListEntry },
    /// `check-deps.sh:411`. `break_system_packages` is a named field rather
    /// than a hidden default because it overrides PEP 668, and blast radius
    /// belongs in the type.
    Pip { id: PackageId, break_system_packages: bool },
    Script { installer: ScriptInstaller },
    GitClone { source: CloneSource, into: CheckPath },
    /// No payload: there is one node entry, no conf file pins a version, and
    /// the only value is `--lts` (`check-deps.sh:388`). A String payload
    /// would reopen spec 3.6 by admitting shell-adjacent text as data.
    NvmInstall,
    NotAutomatable { reason: NoInstallReason },
}

/// Whether a dependency is installable through a given manager.
///
/// A map with absent keys cannot distinguish "same name here" from "not
/// installable here", and the conf files contain both: git on brew is git,
/// while cc on brew is genuinely unavailable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageAvailability {
    Named(PackageId),
    /// zoxide needs this: apt, brew and pacman all have packages, and any
    /// other manager gets the installer script (`check-deps.sh:309`).
    ViaScript(ScriptInstaller),
    Unavailable(NoInstallReason),
}

/// Per-manager availability with a mandatory fallback.
///
/// The fallback is mandatory and states its own reason, which is what makes
/// `resolve` genuinely total. An `Option<PackageId>` default made it total
/// only in the trivial sense: with `None` and no key for the queried
/// manager there is no correct answer, and fabricating
/// `NotPackagedForThisManager` would be a false claim about upstream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageMap {
    per_manager: BTreeMap<PackageManager, PackageAvailability>,
    fallback: PackageAvailability,
}

impl PackageMap {
    pub fn new(
        per_manager: BTreeMap<PackageManager, PackageAvailability>,
        fallback: PackageAvailability,
    ) -> Self {
        PackageMap { per_manager, fallback }
    }

    /// Total: every manager resolves to an availability that states itself.
    pub fn resolve(&self, manager: PackageManager) -> &PackageAvailability {
        self.per_manager.get(&manager).unwrap_or(&self.fallback)
    }
}
```

- [ ] **Step 4: Implement the planner**

Prepend to `crates/deps-core/src/plan.rs`:

```rust
use std::collections::{BTreeMap, BTreeSet};

use dotfiles_path::{CheckRelPath, NameError};

use crate::action::{
    BrewKind, InstallAction, NoInstallReason, PackageAvailability, PackageManager, PackageMap,
    ScriptInstaller,
};
use crate::check::{Observation, Observations, PathRoot, evaluate};
use crate::manifest::{DependencyName, Manifest, ParseError};

/// The elevation state, resolved once at the edge before `gather`.
///
/// `check-deps.sh:168-188` already computes exactly this three-state value,
/// including a `DEPS_FORCE_ROOT` override that exists only so both branches
/// are testable, so naming it a type deletes that environment seam.
///
/// `ViaSudo` is a prediction, not a guarantee: `command -v sudo` proves a
/// binary is on PATH, not that the user is in sudoers, that the credential
/// cache is valid, or that NOPASSWD applies. Resolving once is still correct
/// for a different reason: it makes the plan a deterministic function of one
/// observation, which is what keeps `--dry-run` truthful.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elevation {
    AlreadyRoot,
    ViaSudo,
    Unavailable,
}

/// Whether a step must run through the privileged installer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivilegeRequirement {
    None,
    Root,
}

/// One planned install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub dependency: DependencyName,
    pub action: InstallAction,
    pub privilege: PrivilegeRequirement,
}

/// An ordered, heterogeneous collection of steps.
///
/// One `Vec<Step>` rather than phantom-typed `Step<Ready>` and
/// `Step<Blocked>`. With phantom types the options are `Vec<Box<dyn
/// StepLike>>` (which erases the parameter exactly where the driver consumes
/// it), two vectors (which destroys the topological order that is the whole
/// point), or `Vec<Either<..>>`, which is this closed sum written verbosely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub steps: Vec<Step>,
}

/// Which dependencies this run considers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection(BTreeSet<DependencyName>);

impl Selection {
    pub fn all(manifest: &Manifest) -> Self {
        Selection(manifest.entries().iter().map(|entry| entry.name.clone()).collect())
    }

    pub fn named(names: Vec<DependencyName>) -> Self {
        Selection(names.into_iter().collect())
    }

    pub fn contains(&self, name: &DependencyName) -> bool {
        self.0.contains(name)
    }
}

/// The requirement graph.
///
/// A separate argument rather than a manifest field, because no conf file has
/// a `requires` column: the real format is `name|check_command|docs_url`
/// (`deps.conf:2`), and `deps.conf:18-20` states in the file itself that "no
/// ordering between it and zsh-autosuggestions is guaranteed here". Adding
/// the column is a later decision; the planner does not need to invent it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Requirements(BTreeMap<DependencyName, Vec<DependencyName>>);

impl Requirements {
    pub fn none() -> Self {
        Requirements(BTreeMap::new())
    }

    pub fn from_pairs(pairs: Vec<(DependencyName, Vec<DependencyName>)>) -> Self {
        Requirements(pairs.into_iter().collect())
    }

    pub fn prerequisites(&self, of: &DependencyName) -> &[DependencyName] {
        self.0.get(of).map_or(&[], Vec::as_slice)
    }
}

/// A `--only` value as the caller typed it.
///
/// Its only guarantee is "bounded and safe to render". It exists so
/// `MalformedSelector` can quote the input without an unbounded or
/// terminal-active string entering an error type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSelector(String);

impl RawSelector {
    /// # Errors
    ///
    /// Returns `NameError::ControlByte` for terminal-active input and
    /// `NameError::TooLong` past 256 bytes.
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        if raw.len() > 256 {
            return Err(NameError::TooLong { len: raw.len(), max: 256 });
        }
        if raw.chars().any(|character| character.is_control()) {
            return Err(NameError::ControlByte);
        }
        Ok(RawSelector(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The run never started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    UnknownDependency { name: DependencyName, did_you_mean: Option<DependencyName> },
    MalformedSelector { raw: RawSelector },
    ManifestParse { path: CheckRelPath, detail: ParseError },
    /// Reserved. Nothing constructs it yet: the pipe format carries no
    /// version line, and inventing one would be a manifest change disguised
    /// as a port. The variant exists so a future format change has a place
    /// to report rather than folding into `ManifestParse`.
    ManifestVersion { found: u32, supported: u32 },
    /// A distinct error from `ManifestParse`, because every line parses fine.
    RequirementCycle { chain: Vec<DependencyName> },
}

/// What the core observed while planning.
///
/// Returned rather than logged. The driver drains these into a `Services`
/// handle that owns the `Log`; `Services` is a parameter to the driver, not
/// to the core. Passing it here would falsify the no-IO claim and contradict
/// spec 3.3, which rejected `Probe` for being exactly a trait the core
/// invokes. Under the fixpoint the vectors concatenate across iterations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    CheckSatisfied { dependency: DependencyName },
    CheckUnanswerable { dependency: DependencyName, root: PathRoot },
    StepPlanned { dependency: DependencyName, privilege: PrivilegeRequirement },
    StepBlocked { dependency: DependencyName, on: DependencyName },
    /// The prerequisite is not in the selected manifest, which differs from
    /// "requires a dependency that does not exist". `oh-my-zsh` is in
    /// `deps-linux.conf:12` and legitimately absent on macOS, where
    /// `zsh-autosuggestions` installs fine through brew, so
    /// `UnknownDependency` would be the wrong error.
    PrerequisiteNotSelected { dependency: DependencyName, on: DependencyName },
}

/// Build the plan for one wave.
///
/// Pure: every input is an argument and the observation arrives as an
/// injected `Observations`, so this function opens no file and spawns no
/// process. It may order steps. It may not condition an action on another
/// step's outcome, which is why blocking is decided from `observations`
/// rather than from a callback (spec 6.3).
///
/// # Errors
///
/// Returns `PlanError::UnknownDependency` when the selection names an entry
/// the manifest does not hold, and `PlanError::RequirementCycle` naming the
/// chain when the requirement graph does not sort.
pub fn plan(
    manifest: &Manifest,
    manager: PackageManager,
    selection: &Selection,
    requirements: &Requirements,
    observations: &impl Observations,
    elevation: Elevation,
    packages: &BTreeMap<DependencyName, PackageMap>,
) -> Result<(Plan, Vec<Event>), PlanError> {
    let ordered = topological_order(manifest, selection, requirements)?;
    let mut satisfied: BTreeSet<DependencyName> = BTreeSet::new();
    let mut steps = Vec::new();
    let mut events = Vec::new();

    for name in &ordered {
        let entry = manifest.get(name).ok_or_else(|| PlanError::UnknownDependency {
            name: name.clone(),
            did_you_mean: nearest_name(manifest, name),
        })?;

        match evaluate(&entry.check, observations) {
            Observation::Present => {
                satisfied.insert(name.clone());
                events.push(Event::CheckSatisfied { dependency: name.clone() });
                continue;
            }
            Observation::Unresolvable { root } => {
                events.push(Event::CheckUnanswerable { dependency: name.clone(), root });
            }
            Observation::Absent => {}
        }

        if let Some(blocker) = first_unsatisfied_prerequisite(
            name,
            requirements,
            selection,
            manifest,
            &satisfied,
            &mut events,
        ) {
            events.push(Event::StepBlocked { dependency: name.clone(), on: blocker.clone() });
            steps.push(Step {
                dependency: name.clone(),
                action: InstallAction::NotAutomatable {
                    reason: NoInstallReason::PrerequisiteNotYetInstalled { dependency: blocker },
                },
                privilege: PrivilegeRequirement::None,
            });
            continue;
        }

        let availability = packages.get(name).map_or(
            &PackageAvailability::Unavailable(NoInstallReason::ManagerNotNamedInManifest {
                manager,
            }),
            |map| map.resolve(manager),
        );
        let (action, privilege) = action_for(availability, manager, elevation);
        events.push(Event::StepPlanned { dependency: name.clone(), privilege });
        steps.push(Step { dependency: name.clone(), action, privilege });
    }

    Ok((Plan { steps }, events))
}

/// Derive the action and its privilege from availability and elevation.
///
/// The privilege is derived from the `(action, manager)` pair here, once,
/// which is what makes the driver's dispatch an exhaustive match rather than
/// a predicate it could get wrong. `Elevation::Unavailable` never yields a
/// `Root` step: it yields `PrivilegeUnavailable`, so the condition flows
/// through the report and the exit code instead of surfacing at perform time
/// (`check-deps.sh:546-548` already prints this message).
fn action_for(
    availability: &PackageAvailability,
    manager: PackageManager,
    elevation: Elevation,
) -> (InstallAction, PrivilegeRequirement) {
    let (action, wants_root) = match availability {
        PackageAvailability::Named(id) => match manager {
            PackageManager::Brew => (
                InstallAction::Brew { kind: BrewKind::Formula, id: id.clone(), tap: None },
                false,
            ),
            other => (InstallAction::Package { id: id.clone() }, other.needs_root()),
        },
        PackageAvailability::ViaScript(installer) => {
            (InstallAction::Script { installer: *installer }, false)
        }
        PackageAvailability::Unavailable(reason) => (
            InstallAction::NotAutomatable { reason: reason.clone() },
            false,
        ),
    };

    if wants_root && elevation == Elevation::Unavailable {
        return (
            InstallAction::NotAutomatable { reason: NoInstallReason::PrivilegeUnavailable },
            PrivilegeRequirement::None,
        );
    }
    let privilege = if wants_root { PrivilegeRequirement::Root } else { PrivilegeRequirement::None };
    (action, privilege)
}

fn first_unsatisfied_prerequisite(
    name: &DependencyName,
    requirements: &Requirements,
    selection: &Selection,
    manifest: &Manifest,
    satisfied: &BTreeSet<DependencyName>,
    events: &mut Vec<Event>,
) -> Option<DependencyName> {
    for prerequisite in requirements.prerequisites(name) {
        if manifest.get(prerequisite).is_none() || !selection.contains(prerequisite) {
            // Not an error: oh-my-zsh is in deps-linux.conf:12 and
            // legitimately absent on macOS, where zsh-autosuggestions
            // installs through brew.
            events.push(Event::PrerequisiteNotSelected {
                dependency: name.clone(),
                on: prerequisite.clone(),
            });
            continue;
        }
        if !satisfied.contains(prerequisite) {
            return Some(prerequisite.clone());
        }
    }
    None
}

/// Order the selection so every prerequisite precedes its dependent.
///
/// # Errors
///
/// Returns `PlanError::RequirementCycle` carrying the chain, because every
/// line parses fine and `ManifestParse` would misreport the cause.
fn topological_order(
    manifest: &Manifest,
    selection: &Selection,
    requirements: &Requirements,
) -> Result<Vec<DependencyName>, PlanError> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mark {
        InProgress,
        Done,
    }

    let mut marks: BTreeMap<DependencyName, Mark> = BTreeMap::new();
    let mut ordered = Vec::new();

    for entry in manifest.entries() {
        if !selection.contains(&entry.name) {
            continue;
        }
        visit(
            &entry.name,
            manifest,
            selection,
            requirements,
            &mut marks,
            &mut Vec::new(),
            &mut ordered,
        )?;
    }
    Ok(ordered)
}

fn visit(
    name: &DependencyName,
    manifest: &Manifest,
    selection: &Selection,
    requirements: &Requirements,
    marks: &mut BTreeMap<DependencyName, Mark>,
    chain: &mut Vec<DependencyName>,
    ordered: &mut Vec<DependencyName>,
) -> Result<(), PlanError> {
    match marks.get(name) {
        Some(Mark::Done) => return Ok(()),
        Some(Mark::InProgress) => {
            let mut reported = chain.clone();
            reported.push(name.clone());
            return Err(PlanError::RequirementCycle { chain: reported });
        }
        None => {}
    }

    marks.insert(name.clone(), Mark::InProgress);
    chain.push(name.clone());
    for prerequisite in requirements.prerequisites(name) {
        if manifest.get(prerequisite).is_some() && selection.contains(prerequisite) {
            visit(prerequisite, manifest, selection, requirements, marks, chain, ordered)?;
        }
    }
    chain.pop();
    marks.insert(name.clone(), Mark::Done);
    ordered.push(name.clone());
    Ok(())
}

/// The closest manifest name by common prefix length.
///
/// Prefix length rather than an edit distance, because a dependency inside a
/// no-dependency crate cannot pull in a Levenshtein implementation and a
/// hand-rolled one is more code than the suggestion is worth.
fn nearest_name(manifest: &Manifest, wanted: &DependencyName) -> Option<DependencyName> {
    manifest
        .entries()
        .iter()
        .map(|entry| {
            let shared = entry
                .name
                .as_str()
                .chars()
                .zip(wanted.as_str().chars())
                .take_while(|(left, right)| left == right)
                .count();
            (shared, entry.name.clone())
        })
        .filter(|(shared, _)| *shared >= 2)
        .max_by_key(|(shared, _)| *shared)
        .map(|(_, name)| name)
}
```

Extend `lib.rs` with `mod action; mod plan;` and the re-exports, and add both
files to the `purity` test's `sources` array.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cd ~/crates && cargo test --locked -p deps-core plan::`
Expected: PASS, five tests.

- [ ] **Step 6: Write the failing test for the requirement graph**

Append to `plan.rs`'s test module:

```rust
    // deps.conf:36 and :45. node requires nvm, and nvm's own install is
    // manual-only (check-deps.sh:370-374), so on a machine with neither,
    // node is blocked rather than attempted.
    #[test]
    fn a_dependent_is_blocked_when_its_prerequisite_is_absent() {
        let manifest = manifest_of(&["nvm", "node"]);
        let requirements =
            Requirements::from_pairs(vec![(dependency("node"), vec![dependency("nvm")])]);
        let (built, events) = plan(
            &manifest,
            PackageManager::Apt,
            &Selection::all(&manifest),
            &requirements,
            &ObservationMap::from_pairs(vec![]),
            Elevation::ViaSudo,
            &packages_named(&["nvm", "node"]),
        )
        .expect("a two-entry plan succeeds");

        assert_eq!(built.steps[0].dependency, dependency("nvm"), "nvm sorts first");
        let node_step = built
            .steps
            .iter()
            .find(|step| step.dependency == dependency("node"))
            .expect("node has a step");
        assert!(matches!(
            node_step.action,
            InstallAction::NotAutomatable {
                reason: NoInstallReason::PrerequisiteNotYetInstalled { .. }
            }
        ));
        assert!(events.contains(&Event::StepBlocked {
            dependency: dependency("node"),
            on: dependency("nvm"),
        }));
    }

    // A prerequisite absent from the selected manifest is not an error.
    // oh-my-zsh is in deps-linux.conf:12 and legitimately not in the macOS
    // manifest, where zsh-autosuggestions installs through brew, so
    // PlanError::UnknownDependency would be the wrong answer.
    #[test]
    fn a_prerequisite_outside_the_selected_manifest_is_an_event_not_an_error() {
        let manifest = manifest_of(&["zsh-autosuggestions"]);
        let requirements = Requirements::from_pairs(vec![(
            dependency("zsh-autosuggestions"),
            vec![dependency("oh-my-zsh")],
        )]);
        let (built, events) = plan(
            &manifest,
            PackageManager::Brew,
            &Selection::all(&manifest),
            &requirements,
            &ObservationMap::from_pairs(vec![]),
            Elevation::ViaSudo,
            &packages_named(&["zsh-autosuggestions"]),
        )
        .expect("an absent prerequisite is not a plan error");
        assert!(events.contains(&Event::PrerequisiteNotSelected {
            dependency: dependency("zsh-autosuggestions"),
            on: dependency("oh-my-zsh"),
        }));
        assert!(matches!(built.steps[0].action, InstallAction::Brew { .. }));
    }

    // A cycle is RequirementCycle, not ManifestParse: every line parses.
    #[test]
    fn a_requirement_cycle_names_its_chain() {
        let manifest = manifest_of(&["fzf", "ripgrep"]);
        let requirements = Requirements::from_pairs(vec![
            (dependency("fzf"), vec![dependency("ripgrep")]),
            (dependency("ripgrep"), vec![dependency("fzf")]),
        ]);
        let failure = plan(
            &manifest,
            PackageManager::Apt,
            &Selection::all(&manifest),
            &requirements,
            &ObservationMap::from_pairs(vec![]),
            Elevation::ViaSudo,
            &packages_named(&["fzf", "ripgrep"]),
        )
        .expect_err("a cycle does not plan");
        let PlanError::RequirementCycle { chain } = failure else {
            panic!("a cycle must be RequirementCycle, not {failure:?}");
        };
        assert!(chain.len() >= 2, "the chain names the cycle: {chain:?}");
    }

    // A manager with no entry resolves through the mandatory fallback, which
    // states its own reason. This is what makes resolve total without
    // fabricating a claim about upstream.
    #[test]
    fn an_unnamed_manager_resolves_through_the_fallback() {
        let manifest = manifest_of(&["ripgrep"]);
        let (built, _) = plan(
            &manifest,
            PackageManager::Unknown,
            &Selection::all(&manifest),
            &Requirements::none(),
            &ObservationMap::from_pairs(vec![]),
            Elevation::ViaSudo,
            &packages_named(&["ripgrep"]),
        )
        .expect("Unknown is a variant, not an error");
        assert!(matches!(
            built.steps[0].action,
            InstallAction::NotAutomatable {
                reason: NoInstallReason::ManagerNotNamedInManifest {
                    manager: PackageManager::Unknown
                }
            }
        ));
    }
```

- [ ] **Step 7: Run the test to verify it fails, then passes**

Run: `cd ~/crates && cargo test --locked -p deps-core plan::tests`
Expected on the first run: FAIL. `a_dependent_is_blocked_when_its_prerequisite_is_absent`
fails on the ordering assertion or on the missing `StepBlocked` event if
`topological_order` and `first_unsatisfied_prerequisite` were not written in
Step 4; if they were, this cycle is a confirmation run and the four new tests
pass immediately. Where a test passes on the first run, delete it and
re-derive it against a deliberately broken planner (return the manifest order
unchanged) to confirm it fails for the right reason before restoring.

Then: `cd ~/crates && cargo test --locked -p deps-core && cargo clippy --locked --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 8: Commit**

```
config add crates/deps-core/src/action.rs crates/deps-core/src/plan.rs crates/deps-core/src/lib.rs
config commit -m "Plan installs purely, with privilege as data on the step

Whether an install needs root is a property of the package manager, not of
the dependency: apt needs it and brew never does. So it is derived by plan
from the (action, manager) pair rather than read from a manifest that could
claim otherwise and be wrong.

Today the no-root case is enforced by sniffing the command text for a
literal \${SUDO} at check-deps.sh:545. That is a runtime string test standing
in for a type, and it is one refactor from silently passing. The elevation
three-state already exists at check-deps.sh:168-188, including the
DEPS_FORCE_ROOT override that exists only so both branches are testable, so
naming it a type deletes that environment seam.

Elevation::Unavailable never emits a Root step. It emits
NoInstallReason::PrivilegeUnavailable, so the condition flows through the
report and the exit code instead of being discovered at perform time.

plan returns events and holds no logger. Services is a parameter to the
driver only; passing it here would falsify the no-IO claim and contradict
the reason Probe was rejected.

The requirement graph is a separate argument, not a manifest field. No conf
file has a requires column: the real format is name|check_command|docs_url
per deps.conf:2, and deps.conf:18-20 says in the file itself that no
ordering is guaranteed. Adding the column is a later decision.

PackageMap's fallback is mandatory and states its own reason, so resolve is
total without fabricating NotPackagedForThisManager for what is really an
omission by the manifest author.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## Task 10: Outcomes, the split summaries, and one exit code

Spec 5.4 splits the first draft's single `summarize(outcomes, verb)` into
`summarize_check` and `summarize_install` with disjoint status types. The
reason is that the first draft's table had two "(unused)" cells, and those
cells were a convention the function upheld by hand rather than an
impossibility the types enforced: nothing stopped
`summarize(_, Verb::Check)` from returning `AttemptFailed`. Splitting by verb
makes "unused" an absent variant instead of a table cell.

`ExitStatus` is a newtype with a private field, not a `pub enum`. The first
draft declared `pub enum ExitStatus` and claimed its constructors were
private to the render module. That is not implementable: a `pub enum` has
public constructors, and `#[non_exhaustive]` restrains only other crates
while `config-cli` is in the same workspace. The nine-site regression this
prevents is real, so the mechanism has to work.

Exit 2 keeps its repo-wide meaning of "the caller made a usage error". I
verified all three misuse sites: `check-deps.sh:109` (`--only` with no
value, `exit 2`), `:116` (unknown argument, `exit 2`), and the `--only`
naming a nonexistent dependency. The pinning is load-bearing and the first
draft did not know about it: `deps-docs.test.sh` uses
`[ "$?" -ne 2 ]` as its oracle for "the parser rejected this flag", so
redefining 2 breaks that oracle's semantics. `--dry-run` exits 0
unconditionally today (`check-deps.sh:600-602`, verified), which is a latent
hole: a CI gate on `--dry-run` passes on a machine with everything missing.
That behavior change is deliberate, and it makes two currently-green
assertions red on purpose.

**Files:**
- Create: `crates/deps-core/src/outcome.rs`
- Create: `crates/dotfiles-path/src/bounded.rs`
- Modify: `crates/dotfiles-path/src/lib.rs`
- Modify: `crates/deps-core/src/lib.rs`

**Interfaces:**
- Consumes: `deps_core::{Check, InstallAction, NoInstallReason, PlanError,
  DependencyName}` from Tasks 6 and 7.
- Produces, in `dotfiles-path`:
  - `pub struct BoundedText(String)` with
    `pub fn truncating(raw: &str) -> Self` and `pub fn as_str(&self) -> &str`
- Produces, in `deps-core`:
  - `pub enum SpawnError { NotFound, PermissionDenied, Other }`
  - `pub enum ExecFailure { NonZeroExit { code: i32, stderr: BoundedText },
    AuthenticationRefused, Spawn(SpawnError) }`
  - `pub enum StepOutcome { AlreadyPresent, Installed,
    InstalledButCheckStillFails { check: Check },
    InstallFailed { action: InstallAction, cause: ExecFailure },
    NotAutomatable { reason: NoInstallReason },
    Blocked { on: DependencyName } }`
  - `pub enum CheckStatus { Ready, NotReady }`
  - `pub enum InstallStatus { AllSucceeded, AttemptFailed }`
  - `pub fn summarize_check(outcomes: &[StepOutcome]) -> CheckStatus`
  - `pub fn summarize_install(outcomes: &[StepOutcome]) -> InstallStatus`
  - `pub enum Verdict { Check(CheckStatus), Install(InstallStatus),
    DryRun(CheckStatus) }`
  - `pub struct ExitStatus(u8)` with `pub fn code(&self) -> u8` and NO
    public constructor
  - `pub fn exit_status(result: Result<Verdict, PlanError>) -> ExitStatus`

`ExitStatus` has one constructor and it is `exit_status`, in this module.
Nothing outside `outcome.rs` can build one, which is the mechanism.

- [ ] **Step 1: Write the failing test for the split summaries**

In `crates/deps-core/src/outcome.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Check, DependencyName, InstallAction, NoInstallReason};
    use dotfiles_path::{BoundedText, CommandName, PackageId};

    fn command_check(name: &str) -> Check {
        Check::Command(CommandName::parse(name).expect("a test command name parses"))
    }

    fn a_package_action() -> InstallAction {
        InstallAction::Package {
            id: PackageId::parse("ripgrep").expect("a test package id parses"),
        }
    }

    #[test]
    fn a_run_where_everything_was_already_present_is_ready() {
        let outcomes = [StepOutcome::AlreadyPresent, StepOutcome::AlreadyPresent];
        assert_eq!(summarize_check(&outcomes), CheckStatus::Ready);
    }

    // The defect spec 5.4 names: today a manual-only dependency is
    // invisible to the exit code, so config-init can report success on a
    // machine that is not ready. NotAutomatable being a variant is what
    // forces this function to decide about it.
    #[test]
    fn a_manual_only_dependency_makes_the_check_not_ready() {
        let outcomes = [
            StepOutcome::AlreadyPresent,
            StepOutcome::NotAutomatable {
                reason: NoInstallReason::UpstreamPublishesNoStableUrl,
            },
        ];
        assert_eq!(
            summarize_check(&outcomes),
            CheckStatus::NotReady,
            "a manual-only dependency must count toward not-ready"
        );
    }

    #[test]
    fn a_blocked_dependency_makes_the_check_not_ready() {
        let outcomes = [StepOutcome::Blocked {
            on: DependencyName::parse("nvm").expect("nvm is a name"),
        }];
        assert_eq!(summarize_check(&outcomes), CheckStatus::NotReady);
    }

    // An install that ran and whose check still fails is an install
    // failure, not a check failure: the tool did something and the
    // something did not work.
    #[test]
    fn an_install_whose_check_still_fails_is_an_attempt_failure() {
        let outcomes = [StepOutcome::InstalledButCheckStillFails {
            check: command_check("rg"),
        }];
        assert_eq!(summarize_install(&outcomes), InstallStatus::AttemptFailed);
    }

    #[test]
    fn an_install_failure_is_an_attempt_failure() {
        let outcomes = [StepOutcome::InstallFailed {
            action: a_package_action(),
            cause: ExecFailure::NonZeroExit {
                code: 100,
                stderr: BoundedText::truncating("E: Unable to locate package"),
            },
        }];
        assert_eq!(summarize_install(&outcomes), InstallStatus::AttemptFailed);
    }

    // summarize_install must NOT report AttemptFailed for a manual-only
    // dependency: nothing was attempted, so no attempt failed. The
    // not-ready signal for that case belongs to summarize_check, which the
    // driver also calls. This is the distinction the single summarize
    // function upheld by hand.
    #[test]
    fn a_manual_only_dependency_is_not_an_attempt_failure() {
        let outcomes = [StepOutcome::NotAutomatable {
            reason: NoInstallReason::RequiresInteractiveApproval,
        }];
        assert_eq!(summarize_install(&outcomes), InstallStatus::AllSucceeded);
        assert_eq!(summarize_check(&outcomes), CheckStatus::NotReady);
    }

    // InstalledButCheckStillFails carries the Check so the report can name
    // which predicate failed rather than saying "the install did not
    // satisfy the check" as check-deps.sh:589 does today.
    #[test]
    fn installed_but_check_still_fails_names_the_predicate() {
        let outcome = StepOutcome::InstalledButCheckStillFails {
            check: command_check("rg"),
        };
        let StepOutcome::InstalledButCheckStillFails { check } = outcome else {
            panic!("the fixture is that variant");
        };
        assert_eq!(check, command_check("rg"));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd ~/crates && cargo test --locked -p deps-core outcome::`
Expected: FAIL with `error[E0425]: cannot find function `summarize_check``
plus unresolved `StepOutcome`, `CheckStatus`, `InstallStatus`,
`ExecFailure`, and `BoundedText` not found in `dotfiles_path`. The missing
behavior is the two summary functions and the outcome sum.

- [ ] **Step 3: Implement `BoundedText`, the outcomes, and the summaries**

`crates/dotfiles-path/src/bounded.rs`:

```rust
use std::fmt;

/// The byte cap on captured subprocess output.
const MAX_TEXT_LEN: usize = 4096;

/// Subprocess output, bounded and safe to render.
///
/// An unbounded subprocess string inside an error type is how a terminal
/// gets a control sequence written to it, so control bytes are replaced
/// rather than carried and the length is capped. `truncating` cannot fail:
/// the caller holds bytes a process already produced, and refusing them
/// would lose the only diagnostic the failure has.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BoundedText(String);

impl BoundedText {
    pub fn truncating(raw: &str) -> Self {
        let mut kept = String::with_capacity(raw.len().min(MAX_TEXT_LEN));
        for character in raw.chars() {
            if kept.len() >= MAX_TEXT_LEN {
                break;
            }
            // Newline and tab survive: subprocess stderr is line-oriented
            // and stripping them would run the diagnostic together.
            if character.is_control() && character != '\n' && character != '\t' {
                kept.push('\u{fffd}');
            } else {
                kept.push(character);
            }
        }
        BoundedText(kept)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BoundedText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_a_control_byte_rather_than_carrying_it() {
        let bounded = BoundedText::truncating("E: failed\u{1b}[2J");
        assert!(!bounded.as_str().contains('\u{1b}'));
        assert!(bounded.as_str().starts_with("E: failed"));
    }

    #[test]
    fn keeps_newlines_and_tabs() {
        let bounded = BoundedText::truncating("line one\nline\ttwo");
        assert_eq!(bounded.as_str(), "line one\nline\ttwo");
    }

    #[test]
    fn caps_the_length() {
        let bounded = BoundedText::truncating(&"a".repeat(8192));
        assert!(bounded.as_str().len() <= 4096);
    }
}
```

Add `mod bounded;` and `pub use bounded::BoundedText;` to
`crates/dotfiles-path/src/lib.rs`.

Prepend to `crates/deps-core/src/outcome.rs`:

```rust
use dotfiles_path::BoundedText;

use crate::action::{InstallAction, NoInstallReason};
use crate::check::Check;
use crate::manifest::DependencyName;
use crate::plan::PlanError;

/// Why a process could not be started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnError {
    NotFound,
    PermissionDenied,
    Other,
}

/// Why an install command failed.
///
/// Specified rather than left undefined. `stderr` is `BoundedText` because
/// an unbounded subprocess string in an error type is how a terminal gets a
/// control sequence written to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecFailure {
    NonZeroExit { code: i32, stderr: BoundedText },
    /// sudo said no, and every remaining privileged step will too. Knowable
    /// at step 1 of 8 rather than at step 8, which is why
    /// `Elevation::ViaSudo` is a prediction rather than a guarantee: a sudo
    /// binary on PATH does not prove the user is in sudoers.
    AuthenticationRefused,
    Spawn(SpawnError),
}

/// What happened to one planned step.
///
/// `NotAutomatable` appears here and in `InstallAction`, and that is not
/// duplication: they are different propositions. As an action it is the
/// terminal element of the algebra, which is what makes `plan` total. As an
/// outcome it records that the driver performed the step and correctly did
/// nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    AlreadyPresent,
    Installed,
    /// Carries the `Check` so the report names which predicate failed.
    /// `check-deps.sh:589` prints "install did not satisfy the check for
    /// %s" and cannot say more, because the predicate was a shell string it
    /// re-ran rather than a value it holds.
    InstalledButCheckStillFails { check: Check },
    InstallFailed { action: InstallAction, cause: ExecFailure },
    NotAutomatable { reason: NoInstallReason },
    /// Not in this wave. A later wave can unblock it, which is what
    /// separates this from `NoInstallReason::PrerequisiteNotYetInstalled`:
    /// the latter is what a dry run reports, because a dry run cannot know
    /// what a later wave would do.
    Blocked { on: DependencyName },
}

/// The result of `deps check`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Ready,
    NotReady,
}

/// The result of `deps install`.
///
/// A separate type from `CheckStatus`, not a shared status enum. The first
/// draft's single `summarize(outcomes, verb)` had a table with two
/// "(unused)" cells, and nothing in the types stopped
/// `summarize(_, Verb::Check)` from returning `AttemptFailed`. Splitting by
/// verb makes "unused" an absent variant rather than a convention upheld by
/// hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStatus {
    AllSucceeded,
    AttemptFailed,
}

/// Whether the environment is ready.
///
/// Pure. `NotAutomatable` counts toward not-ready, which is the direct fix
/// for today's behavior where a manual-only dependency is invisible to the
/// exit code and `config-init` can report success on a machine that is not
/// ready.
pub fn summarize_check(outcomes: &[StepOutcome]) -> CheckStatus {
    let ready = outcomes.iter().all(|outcome| {
        matches!(outcome, StepOutcome::AlreadyPresent | StepOutcome::Installed)
    });
    if ready { CheckStatus::Ready } else { CheckStatus::NotReady }
}

/// Whether every actionable install succeeded.
///
/// Pure. `NotAutomatable` is not an attempt failure: nothing was attempted,
/// so no attempt failed. The not-ready signal for that case belongs to
/// `summarize_check`, which the driver also calls.
pub fn summarize_install(outcomes: &[StepOutcome]) -> InstallStatus {
    let attempt_failed = outcomes.iter().any(|outcome| {
        matches!(
            outcome,
            StepOutcome::InstallFailed { .. } | StepOutcome::InstalledButCheckStillFails { .. }
        )
    });
    if attempt_failed { InstallStatus::AttemptFailed } else { InstallStatus::AllSucceeded }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd ~/crates && cargo test --locked -p deps-core outcome:: && cargo test --locked -p dotfiles-path bounded::`
Expected: PASS, seven plus three tests.

- [ ] **Step 5: Write the failing test for the single exit code**

Append to `outcome.rs`'s test module:

```rust
    // The table in spec 5.4. Every cell, including the reserved ones,
    // asserted as one test so a renumbering cannot slip through per-case.
    #[test]
    fn the_exit_code_table_holds() {
        let cases = [
            (Ok(Verdict::Check(CheckStatus::Ready)), 0),
            (Ok(Verdict::Check(CheckStatus::NotReady)), 1),
            (Ok(Verdict::Install(InstallStatus::AllSucceeded)), 0),
            (Ok(Verdict::Install(InstallStatus::AttemptFailed)), 3),
            (Ok(Verdict::DryRun(CheckStatus::Ready)), 0),
            (Ok(Verdict::DryRun(CheckStatus::NotReady)), 1),
        ];
        for (verdict, expected) in cases {
            assert_eq!(
                exit_status(verdict.clone()).code(),
                expected,
                "the wrong code for {verdict:?}"
            );
        }
    }

    // Exit 2 keeps its repo-wide meaning: the caller made a usage error.
    // Three misuse conditions exit 2 today (check-deps.sh:109 for --only
    // with no value, :116 for an unknown argument, and --only naming a
    // nonexistent dependency), clap exits 2 for its own usage errors
    // deliberately, and deps-docs.test.sh uses exit 2 as its oracle for
    // "the parser rejected this flag". Narrowing 2 breaks that oracle.
    #[test]
    fn every_plan_error_exits_two() {
        let errors = [
            PlanError::UnknownDependency {
                name: DependencyName::parse("ripgpre").expect("a name parses"),
                did_you_mean: DependencyName::parse("ripgrep").ok(),
            },
            PlanError::MalformedSelector {
                raw: crate::RawSelector::parse("a,,b").expect("a bounded selector parses"),
            },
            PlanError::RequirementCycle {
                chain: vec![
                    DependencyName::parse("fzf").expect("a name parses"),
                    DependencyName::parse("ripgrep").expect("a name parses"),
                ],
            },
            PlanError::ManifestVersion { found: 2, supported: 1 },
        ];
        for error in errors {
            assert_eq!(
                exit_status(Err(error.clone())).code(),
                2,
                "every caller error is 2, including {error:?}"
            );
        }
    }

    // The behavior change spec 5.4 makes deliberately.
    // check-deps.sh:600-602 exits 0 unconditionally on --dry-run, pinned by
    // check-deps.test.sh:130 ('dry-run always exits 0'). That is a latent
    // hole: a CI gate on --dry-run passes on a machine with everything
    // missing. "Would install three things" means "three things are
    // missing".
    #[test]
    fn a_dry_run_with_something_missing_does_not_exit_zero() {
        assert_eq!(exit_status(Ok(Verdict::DryRun(CheckStatus::NotReady))).code(), 1);
    }

    // Codes are per-verb disjoint, so a consumer learning "nonzero and not
    // 2 means the environment is not ready" is correct for both verbs
    // permanently.
    #[test]
    fn nonzero_and_not_two_always_means_not_ready() {
        let not_ready = [
            Ok(Verdict::Check(CheckStatus::NotReady)),
            Ok(Verdict::Install(InstallStatus::AttemptFailed)),
            Ok(Verdict::DryRun(CheckStatus::NotReady)),
        ];
        for verdict in not_ready {
            let code = exit_status(verdict).code();
            assert!(code != 0 && code != 2, "the rule requires nonzero and not 2, got {code}");
        }
    }
```

- [ ] **Step 6: Run the test to verify it fails**

Run: `cd ~/crates && cargo test --locked -p deps-core outcome::tests::the_exit_code_table_holds`
Expected: FAIL with `error[E0425]: cannot find function `exit_status`` and
unresolved `Verdict`. The mapping does not exist.

- [ ] **Step 7: Implement the single exit-code funnel**

Append to `crates/deps-core/src/outcome.rs`:

```rust
/// What one run concluded.
///
/// `DryRun` carries a `CheckStatus` rather than its own type, because a dry
/// run answers the same question `deps check` answers: is the environment
/// ready. It is a separate variant only so the table can give it its own
/// column if the codes ever diverge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Check(CheckStatus),
    Install(InstallStatus),
    DryRun(CheckStatus),
}

/// A process exit code.
///
/// A newtype with a private field, not a `pub enum`. A `pub enum` has public
/// constructors and `#[non_exhaustive]` restrains only other crates, while
/// `config-cli` is in this same workspace, so a `pub enum` cannot make
/// `exit_status` the only constructor. The nine-site provenance regression
/// this prevents is real, so the mechanism has to work rather than be
/// documented.
///
/// The field is private to this module and no `From`, `new`, or `Default`
/// impl exists. `exit_status` is the sole way to obtain one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitStatus(u8);

impl ExitStatus {
    pub fn code(&self) -> u8 {
        self.0
    }
}

/// Map one run's verdict to a process exit code.
///
/// The single place this mapping exists. Codes are per-verb disjoint, so a
/// consumer learning "nonzero and not 2 means the environment is not ready"
/// is correct for both verbs permanently, and the unused cells allow growth
/// without renumbering.
///
/// Exit 2 means every caller error, matching the repo-wide convention that
/// `check-deps.sh:109` and `:116` already use and that
/// `deps-docs.test.sh` relies on as its oracle for "the parser rejected
/// this flag". Narrowing 2 to one condition would break that oracle's
/// semantics.
pub fn exit_status(result: Result<Verdict, PlanError>) -> ExitStatus {
    let Ok(verdict) = result else {
        return ExitStatus(2);
    };
    match verdict {
        Verdict::Check(CheckStatus::Ready) => ExitStatus(0),
        Verdict::Check(CheckStatus::NotReady) => ExitStatus(1),
        Verdict::DryRun(CheckStatus::Ready) => ExitStatus(0),
        Verdict::DryRun(CheckStatus::NotReady) => ExitStatus(1),
        Verdict::Install(InstallStatus::AllSucceeded) => ExitStatus(0),
        Verdict::Install(InstallStatus::AttemptFailed) => ExitStatus(3),
    }
}
```

Add `mod outcome;` plus the re-exports to `lib.rs`, and add `outcome.rs` to
the `purity` test's `sources` array.

- [ ] **Step 8: Verify the private constructor mechanically**

The claim "`exit_status` is the only constructor" needs a check, not a
comment. Add to `crates/deps-core/src/outcome.rs`:

```rust
/// A compile-fail witness for `ExitStatus`'s private field.
///
/// The doctest is `compile_fail`, so `cargo test` fails if the field ever
/// becomes public or a public constructor appears. That is the mechanism
/// spec 5.4 requires, and a comment claiming privacy is not.
///
/// ```compile_fail
/// let forged = deps_core::ExitStatus(1);
/// ```
///
/// ```compile_fail
/// let forged: deps_core::ExitStatus = Default::default();
/// ```
#[allow(dead_code)]
fn exit_status_has_no_public_constructor() {}
```

Run: `cd ~/crates && cargo test --locked -p deps-core --doc`
Expected: PASS. Both doctests must fail to compile, which is the assertion.
If either compiles, the private-field mechanism is not in place and the task
is not done.

- [ ] **Step 9: Run the full suite**

Run: `cd ~/crates && cargo test --locked --workspace && cargo clippy --locked --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 10: Commit**

```
config add crates/deps-core/src/outcome.rs crates/deps-core/src/lib.rs crates/dotfiles-path/src/bounded.rs crates/dotfiles-path/src/lib.rs
config commit -m "Split the outcome summaries by verb and funnel one exit code

The first draft had one summarize(outcomes, verb) whose table carried two
\"(unused)\" cells. Those cells were a convention the function upheld by
hand, not an impossibility the types enforced: nothing stopped
summarize(_, Verb::Check) from returning AttemptFailed. CheckStatus and
InstallStatus are disjoint types, so \"unused\" is an absent variant.

ExitStatus is a newtype with a private field, not a pub enum. A pub enum has
public constructors and #[non_exhaustive] restrains only other crates, while
config-cli is in this workspace, so a pub enum cannot make exit_status the
only constructor. A compile_fail doctest asserts the privacy rather than a
comment claiming it.

Exit 2 keeps its repo-wide meaning of caller error. check-deps.sh:109 and
:116 already exit 2 for two misuse conditions, clap exits 2 for its own
usage errors deliberately, and deps-docs.test.sh uses exit 2 as its oracle
for \"the parser rejected this flag\". Narrowing 2 would break that oracle.

NotAutomatable counts toward not-ready and is not an attempt failure. That
is the direct fix for today's behavior, where a manual-only dependency is
invisible to the exit code and config-init can report success on a machine
that is not ready.

--dry-run now exits as deps check does. check-deps.sh:600-602 exits 0
unconditionally, which means a CI gate on --dry-run passes on a machine with
everything missing. This makes check-deps.test.sh:130 ('dry-run always exits
0') and :141 ('a manual-only dependency does not fail --fix') red on
purpose; whoever ports those suites updates both with the new expectation
stated.

ExecFailure::NonZeroExit carries BoundedText because an unbounded subprocess
string inside an error type is how a terminal gets a control sequence
written to it.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## Task 11: The fixpoint loop, `attempted` with a private constructor, and `reconcile`

The pipeline is a fixpoint, not a single pass, and I verified the mechanism
in the real file rather than taking the spec's word. `check-deps.sh:339`
emits the `zsh-autosuggestions` clone only inside
`if [ -d "${ZSH_CUSTOM:-$HOME/.oh-my-zsh/custom}" ]`, and the comment at
`:328-335` explains why: cloning into a nonexistent `~/.oh-my-zsh` would
land the plugin where nothing sources it, and the check would then report
success for an install that never loads. `oh-my-zsh` lives in
`deps-linux.conf:12`. So on a Linux machine with neither, wave 1 installs
`oh-my-zsh` and only wave 2 can install `zsh-autosuggestions`. One `--fix`
pass does not converge, which matches the container evidence the spec cites
("no automated install for zsh-autosuggestions" at line 312 and "installed
oh-my-zsh" at line 1237 of the same run).

The re-gather is full, not scoped to `plan.attempted`. A scoped re-gather
would re-observe only `oh-my-zsh` and would still call
`zsh-autosuggestions` missing, which is the exact failure the full re-gather
exists to prevent. Spec 3.3 establishes each check costs microseconds, so
scoping was a false economy, and a full re-gather is also required if apt
installs are ever batched: one `apt-get install a b c` yields one exit status
for three dependencies, so per-step outcomes stop being derivable from
per-step exit codes.

`attempted` comes back from the loop, not from `plan`. In the first draft
`after = gather(&plan.attempted)` took an argument available the instant
`plan` returned, so writing the re-gather before the perform loop compiled,
type-checked, and reconciled every outcome against a pre-install world.
Making `Attempted`'s constructor private to the driver module turns that
reorder into a compile error, which is the technique Task 10 already applies
to `ExitStatus`.

**Files:**
- Create: `crates/deps-core/src/reconcile.rs`
- Create: `crates/deps-core/src/driver.rs`
- Modify: `crates/deps-core/src/lib.rs`

`driver.rs` lives in `deps-core` and holds no IO. It owns the loop's shape
and the `Attempted` type; the effectful `Installer` implementations and
`gather` live in `config-cli`, which is a later task. The loop takes its
effects as function arguments, so the module names no capability and the
`purity` test from Task 7 covers it.

**Interfaces:**
- Consumes: `deps_core::{Plan, Step, Event, PlanError, StepOutcome,
  CheckStatus, InstallStatus, Manifest, Observations, PrivilegeRequirement,
  DependencyName, plan, summarize_check, summarize_install}` from Tasks 5
  through 8.
- Produces, in `reconcile.rs`:
  - `pub struct Report { pub rows: Vec<ReportRow>,
    pub check: CheckStatus, pub install: InstallStatus }`
  - `pub struct ReportRow { pub dependency: DependencyName,
    pub outcome: StepOutcome, pub after: Observation }`
  - `pub fn reconcile(manifest: &Manifest,
    outcomes: &[(DependencyName, StepOutcome)],
    observations: &impl Observations) -> (Report, Vec<Event>)`
- Produces, in `driver.rs`:
  - `pub struct Attempted(BTreeSet<DependencyName>)` with
    `pub fn contains(&self, name: &DependencyName) -> bool`,
    `pub fn len(&self) -> usize`, `pub fn is_empty(&self) -> bool`, and NO
    public constructor
  - `pub trait Installer { fn describe(&self, action: &InstallAction)
    -> ActionDescription; fn perform(&self, action: &InstallAction)
    -> StepOutcome; }`
  - `pub struct ActionDescription { pub summary: String,
    pub privilege: PrivilegeRequirement,
    pub command_preview: Option<String>, pub changes_trust_root: bool }`
  - `pub struct Installers { pub ordinary: Box<dyn Installer>,
    pub privileged: Option<Box<dyn Installer>> }`
  - `pub fn perform_all(installers: &Installers, ready: &[Step])
    -> (Vec<(DependencyName, StepOutcome)>, Attempted)`
  - `pub fn describe(installer: &dyn Installer, plan: &Plan)
    -> Vec<ActionDescription>`
  - `pub fn run_to_fixpoint<GatherFn>(manifest: &Manifest, ...,
    gather: GatherFn) -> Result<(Report, Vec<Event>), PlanError>` where
    `GatherFn: FnMut() -> Gathered` and `Gathered: Observations`

`--dry-run` is `describe` over the plan, not an `Installer` impl. Every
`StepOutcome` variant is a false statement about a run that did nothing:
`Installed` and `AlreadyPresent` make the summary report success,
`InstallFailed` exits 3 for a successful dry run, `NotAutomatable` destroys
the distinction `NoInstallReason` exists to preserve, and `Blocked` means
something else. A port whose return type cannot express one of its own
implementations' outcomes is leaking.

- [ ] **Step 1: Write the failing test for the private `Attempted` constructor**

In `crates/deps-core/src/driver.rs`:

```rust
/// A compile-fail witness for `Attempted`'s private constructor.
///
/// This is the whole mechanism. In the first draft the re-gather read
/// `plan.attempted`, an argument available the instant `plan` returned, so
/// writing `let after = gather(&plan.attempted);` BEFORE the perform loop
/// compiled and then reconciled every outcome against a pre-install world.
/// `Attempted` coming back from `perform_all` with no public constructor
/// makes that reorder a compile error rather than a review finding.
///
/// ```compile_fail
/// use std::collections::BTreeSet;
/// let forged = deps_core::Attempted(BTreeSet::new());
/// ```
///
/// ```compile_fail
/// let forged: deps_core::Attempted = Default::default();
/// ```
///
/// ```compile_fail
/// let forged = deps_core::Attempted::new();
/// ```
#[allow(dead_code)]
fn attempted_has_no_public_constructor() {}
```

- [ ] **Step 2: Run the doctest to verify it fails**

Run: `cd ~/crates && cargo test --locked -p deps-core --doc attempted_has_no_public_constructor`
Expected: FAIL, but read the failure carefully. A `compile_fail` doctest
fails when the code **compiles**, so the first run must instead fail with
`error[E0433]: failed to resolve: could not find `Attempted` in
`deps_core``, meaning the doctest passes vacuously against a type that does
not exist. That is not the assertion. So the red state for this step is:
the doctest passes for the wrong reason, and Step 3 must make `Attempted`
exist so the doctest asserts something. Confirm the vacuity by temporarily
adding a fourth doctest without `compile_fail` that names `Attempted` and
watching it fail to resolve.

- [ ] **Step 3: Write the failing test for the loop ordering**

Append to `crates/deps-core/src/driver.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Check, CheckPath, ConfKind, Elevation, NoInstallReason, ObservationMap, Observation,
        PackageAvailability, PackageManager, PackageMap, PathRoot, Requirements, Selection,
        parse_manifest,
    };
    use dotfiles_path::{CheckRelPath, PackageId};
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    fn dependency(name: &str) -> DependencyName {
        DependencyName::parse(name).expect("a test dependency name parses")
    }

    // The real Linux pair, from deps-linux.conf:12 and deps.conf:26. The
    // second entry's check points into the directory the first entry's
    // install creates, which is what makes one pass insufficient.
    fn oh_my_zsh_manifest() -> Manifest {
        let text = "\
oh-my-zsh|[ -d \"$HOME/.oh-my-zsh\" ]|https://ohmyz.sh/
zsh-autosuggestions|[ -f \"$HOME/.oh-my-zsh/custom/plugins/zsh-autosuggestions/zsh-autosuggestions.zsh\" ]|https://github.com/zsh-users/zsh-autosuggestions
";
        parse_manifest(text, ConfKind::PlatformSelected).expect("the real pair parses")
    }

    fn home_path(rest: &str) -> CheckPath {
        CheckPath::new(
            PathRoot::Home,
            CheckRelPath::parse(rest).expect("a test path parses"),
        )
    }

    fn oh_my_zsh_check() -> Check {
        Check::DirExists(home_path(".oh-my-zsh"))
    }

    fn autosuggestions_check() -> Check {
        Check::FileExists(home_path(
            ".oh-my-zsh/custom/plugins/zsh-autosuggestions/zsh-autosuggestions.zsh",
        ))
    }

    fn packages() -> BTreeMap<DependencyName, PackageMap> {
        ["oh-my-zsh", "zsh-autosuggestions"]
            .into_iter()
            .map(|name| {
                let mut per_manager = BTreeMap::new();
                per_manager.insert(
                    PackageManager::Apt,
                    PackageAvailability::Named(
                        PackageId::parse(name).expect("a test package id parses"),
                    ),
                );
                (
                    dependency(name),
                    PackageMap::new(
                        per_manager,
                        PackageAvailability::Unavailable(
                            NoInstallReason::NotPackagedForThisManager,
                        ),
                    ),
                )
            })
            .collect()
    }

    /// An installer that succeeds and records what it was asked to do.
    struct RecordingInstaller {
        performed: RefCell<Vec<InstallAction>>,
    }

    impl Installer for RecordingInstaller {
        fn describe(&self, action: &InstallAction) -> ActionDescription {
            ActionDescription {
                summary: format!("{action:?}"),
                privilege: PrivilegeRequirement::None,
                command_preview: None,
                changes_trust_root: matches!(action, InstallAction::AptSource { .. }),
            }
        }

        fn perform(&self, action: &InstallAction) -> StepOutcome {
            self.performed.borrow_mut().push(action.clone());
            match action {
                InstallAction::NotAutomatable { reason } => {
                    StepOutcome::NotAutomatable { reason: reason.clone() }
                }
                _ => StepOutcome::Installed,
            }
        }
    }

    /// A scripted sequence of worlds, one per gather.
    ///
    /// This is the whole test harness: no mock, no call-order semantics.
    /// The loop body is a pure step function, so a Vec of observation maps
    /// is enough to drive it.
    struct ScriptedWorlds {
        worlds: RefCell<Vec<ObservationMap>>,
        gathers: RefCell<usize>,
    }

    impl ScriptedWorlds {
        fn next(&self) -> ObservationMap {
            *self.gathers.borrow_mut() += 1;
            let mut worlds = self.worlds.borrow_mut();
            if worlds.len() > 1 {
                worlds.remove(0)
            } else {
                worlds[0].clone()
            }
        }
    }

    // The fixpoint. Wave 1 sees nothing installed, so only oh-my-zsh can
    // be planned, because zsh-autosuggestions' own check points inside a
    // directory that does not exist yet. Wave 2 sees oh-my-zsh present and
    // plans zsh-autosuggestions. A single pass leaves the second missing,
    // which is what check-deps.sh:328-341 does today.
    #[test]
    fn the_loop_converges_only_after_a_second_wave() {
        let manifest = oh_my_zsh_manifest();
        let worlds = ScriptedWorlds {
            worlds: RefCell::new(vec![
                ObservationMap::from_pairs(vec![]),
                ObservationMap::from_pairs(vec![(oh_my_zsh_check(), Observation::Present)]),
                ObservationMap::from_pairs(vec![
                    (oh_my_zsh_check(), Observation::Present),
                    (autosuggestions_check(), Observation::Present),
                ]),
            ]),
            gathers: RefCell::new(0),
        };
        let installers = Installers {
            ordinary: Box::new(RecordingInstaller { performed: RefCell::new(Vec::new()) }),
            privileged: None,
        };

        let (report, _events) = run_to_fixpoint(
            &manifest,
            PackageManager::Apt,
            &Selection::all(&manifest),
            &Requirements::none(),
            Elevation::AlreadyRoot,
            &packages(),
            &installers,
            || worlds.next(),
        )
        .expect("the pair converges");

        assert!(
            *worlds.gathers.borrow() >= 3,
            "one initial gather plus one per wave: got {}",
            worlds.gathers.borrow()
        );
        assert_eq!(report.check, CheckStatus::Ready, "the fixpoint converges");
        assert_eq!(report.rows.len(), 2);
    }

    // Termination. Every iteration either performs a step or breaks, and a
    // dependency is removed from consideration once attempted, so a world
    // that never changes cannot loop forever.
    #[test]
    fn a_world_that_never_changes_still_terminates() {
        let manifest = oh_my_zsh_manifest();
        let worlds = ScriptedWorlds {
            worlds: RefCell::new(vec![ObservationMap::from_pairs(vec![])]),
            gathers: RefCell::new(0),
        };
        let installers = Installers {
            ordinary: Box::new(RecordingInstaller { performed: RefCell::new(Vec::new()) }),
            privileged: None,
        };
        let (report, _events) = run_to_fixpoint(
            &manifest,
            PackageManager::Apt,
            &Selection::all(&manifest),
            &Requirements::none(),
            Elevation::AlreadyRoot,
            &packages(),
            &installers,
            || worlds.next(),
        )
        .expect("a static world terminates");
        assert_eq!(
            report.check,
            CheckStatus::NotReady,
            "nothing became present, so the environment is not ready"
        );
        assert!(
            *worlds.gathers.borrow() <= 1 + manifest.entries().len(),
            "at most one gather per dependency plus the initial one"
        );
    }

    // perform_all returns attempted; nothing else constructs it. A test
    // cannot forge one, and the driver cannot read one before the loop.
    #[test]
    fn perform_all_reports_what_it_attempted() {
        let manifest = oh_my_zsh_manifest();
        let (built, _) = crate::plan(
            &manifest,
            PackageManager::Apt,
            &Selection::all(&manifest),
            &Requirements::none(),
            &ObservationMap::from_pairs(vec![]),
            Elevation::AlreadyRoot,
            &packages(),
        )
        .expect("a two-entry plan succeeds");
        let installers = Installers {
            ordinary: Box::new(RecordingInstaller { performed: RefCell::new(Vec::new()) }),
            privileged: None,
        };
        let (outcomes, attempted) = perform_all(&installers, &built.steps);
        assert_eq!(outcomes.len(), built.steps.len());
        assert_eq!(attempted.len(), built.steps.len());
        assert!(attempted.contains(&dependency("oh-my-zsh")));
    }

    // A Root step with no privileged installer is not performed. plan
    // already refuses to emit one when elevation is unavailable, so this is
    // the second guard, and it is an exhaustive match rather than a
    // predicate over command text.
    #[test]
    fn a_root_step_without_a_privileged_installer_is_not_performed() {
        let recorder = RecordingInstaller { performed: RefCell::new(Vec::new()) };
        let installers = Installers { ordinary: Box::new(recorder), privileged: None };
        let steps = vec![Step {
            dependency: dependency("ripgrep"),
            action: InstallAction::Package {
                id: PackageId::parse("ripgrep").expect("a test package id parses"),
            },
            privilege: PrivilegeRequirement::Root,
        }];
        let (outcomes, attempted) = perform_all(&installers, &steps);
        assert_eq!(
            outcomes[0].1,
            StepOutcome::NotAutomatable {
                reason: NoInstallReason::PrivilegeUnavailable
            }
        );
        assert!(
            attempted.contains(&dependency("ripgrep")),
            "the step was considered and resolved, so it is not retried"
        );
    }

    // reconcile classifies against the world AFTER the loop. An install
    // that ran and whose check still fails is caught here, and it carries
    // the Check so the report names the predicate.
    #[test]
    fn reconcile_reports_an_install_whose_check_still_fails() {
        let manifest = oh_my_zsh_manifest();
        let outcomes = vec![
            (dependency("oh-my-zsh"), StepOutcome::Installed),
            (dependency("zsh-autosuggestions"), StepOutcome::Installed),
        ];
        let after = ObservationMap::from_pairs(vec![(oh_my_zsh_check(), Observation::Present)]);
        let (report, _events) = crate::reconcile(&manifest, &outcomes, &after);
        let row = report
            .rows
            .iter()
            .find(|row| row.dependency == dependency("zsh-autosuggestions"))
            .expect("the second entry has a row");
        assert!(matches!(
            row.outcome,
            StepOutcome::InstalledButCheckStillFails { .. }
        ));
        assert_eq!(report.install, InstallStatus::AttemptFailed);
        assert_eq!(report.check, CheckStatus::NotReady);
    }
}
```

- [ ] **Step 4: Run the test to verify it fails**

Run: `cd ~/crates && cargo test --locked -p deps-core driver::`
Expected: FAIL with `error[E0425]: cannot find function `run_to_fixpoint``
plus unresolved `perform_all`, `Installers`, `Installer`,
`ActionDescription`, `Attempted`, and `crate::reconcile`. The loop does not
exist.

- [ ] **Step 5: Implement `reconcile`**

`crates/deps-core/src/reconcile.rs`:

```rust
use crate::check::{Observation, Observations, evaluate};
use crate::manifest::{DependencyName, Manifest};
use crate::outcome::{
    CheckStatus, InstallStatus, StepOutcome, summarize_check, summarize_install,
};
use crate::plan::Event;

/// One dependency's final state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportRow {
    pub dependency: DependencyName,
    pub outcome: StepOutcome,
    pub after: Observation,
}

/// What the run concluded, per dependency and in aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub rows: Vec<ReportRow>,
    pub check: CheckStatus,
    pub install: InstallStatus,
}

/// Classify each outcome against the world observed after the loop.
///
/// Pure: `observations` is the post-loop world, injected. This is where an
/// `Installed` whose check still fails becomes
/// `InstalledButCheckStillFails`, carrying the `Check` so the report can
/// name the predicate.
///
/// A dependency the loop never attempted still gets a row, classified from
/// its observation, so the report covers the selection rather than only the
/// steps.
pub fn reconcile(
    manifest: &Manifest,
    outcomes: &[(DependencyName, StepOutcome)],
    observations: &impl Observations,
) -> (Report, Vec<Event>) {
    let mut rows = Vec::new();
    let mut events = Vec::new();

    for entry in manifest.entries() {
        let after = evaluate(&entry.check, observations);
        if let Observation::Unresolvable { root } = after {
            events.push(Event::CheckUnanswerable { dependency: entry.name.clone(), root });
        }

        let recorded = outcomes
            .iter()
            .find(|(name, _)| name == &entry.name)
            .map(|(_, outcome)| outcome.clone());

        let outcome = match (recorded, after) {
            (Some(StepOutcome::Installed), Observation::Present) => StepOutcome::Installed,
            // The install ran and the predicate is still not satisfied.
            // Carrying the Check is what lets the report say which one.
            (Some(StepOutcome::Installed), _) => StepOutcome::InstalledButCheckStillFails {
                check: entry.check.clone(),
            },
            (Some(other), _) => other,
            (None, Observation::Present) => StepOutcome::AlreadyPresent,
            (None, _) => StepOutcome::Blocked { on: entry.name.clone() },
        };

        rows.push(ReportRow { dependency: entry.name.clone(), outcome, after });
    }

    let classified: Vec<StepOutcome> = rows.iter().map(|row| row.outcome.clone()).collect();
    let report = Report {
        check: summarize_check(&classified),
        install: summarize_install(&classified),
        rows,
    };
    (report, events)
}
```

- [ ] **Step 6: Implement the port, `perform_all`, and the loop**

Prepend to `crates/deps-core/src/driver.rs`:

```rust
use std::collections::BTreeSet;

use crate::action::{InstallAction, NoInstallReason, PackageManager, PackageMap};
use crate::check::Observations;
use crate::manifest::{DependencyName, Manifest};
use crate::outcome::StepOutcome;
use crate::plan::{
    Elevation, Event, Plan, PlanError, PrivilegeRequirement, Requirements, Selection, Step, plan,
};
use crate::reconcile::{Report, reconcile};

/// What the driver considered and resolved in one wave.
///
/// The field is private to this module and there is no `new`, no `From`, no
/// `Default`. The only way to obtain an `Attempted` is to call
/// `perform_all`, which means a re-gather cannot be written before the
/// perform loop: the value it needs does not exist yet.
///
/// This is the fix for a real first-draft defect. `plan.attempted` was
/// available the instant `plan` returned, so `let after =
/// gather(&plan.attempted);` placed above the loop compiled, type-checked,
/// and reconciled every outcome against a pre-install world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempted(BTreeSet<DependencyName>);

impl Attempted {
    pub fn contains(&self, name: &DependencyName) -> bool {
        self.0.contains(name)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A structured description of one action.
///
/// Structured, not a string. A bare `String` cannot support the requirement
/// that `--dry-run` disclose privileged steps before the first password
/// prompt, because the driver must aggregate that across steps beforehand,
/// and aggregating over strings means grepping for `sudo`, which
/// resurrects the string sniff at `check-deps.sh:545` inside the new
/// design.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionDescription {
    pub summary: String,
    pub privilege: PrivilegeRequirement,
    pub command_preview: Option<String>,
    /// `AptSource` and any future equivalent. It exists so the `gh` apt
    /// pipeline at `check-deps.sh:236`, which permanently adds a
    /// third-party APT trust root, cannot be disclosed as an ordinary
    /// package install.
    pub changes_trust_root: bool,
}

/// The one effect port.
///
/// One trait, not two. `--dry-run` is not an implementation of it: every
/// `StepOutcome` variant is a false statement about a run that did nothing,
/// so a dry run is `describe` over the plan with the effectful segment not
/// executed.
///
/// `describe` must be a pure function of exactly the inputs `perform`
/// consumes. Today's script gets this right by a stronger mechanism than
/// two methods: it substitutes `${SUDO}` once into a single string used for
/// both display and execution (`check-deps.sh:557-567`, with a comment
/// saying so). Two methods can diverge, so an implementation builds the
/// command once and has both methods read it.
pub trait Installer {
    fn describe(&self, action: &InstallAction) -> ActionDescription;
    fn perform(&self, action: &InstallAction) -> StepOutcome;
}

/// One trait, two slots.
///
/// The driver's dispatch is an exhaustive match on
/// `Step::privilege` rather than a predicate over command text, and
/// `privileged: None` makes "cannot install with root" a property of the
/// wiring. `depcheck-hook.sh` runs on shell startup, so wiring it with no
/// privileged installer is what makes that a structural fact rather than a
/// missing flag.
pub struct Installers {
    pub ordinary: Box<dyn Installer>,
    pub privileged: Option<Box<dyn Installer>>,
}

/// Perform one wave's ready steps, sequentially.
///
/// Returns the per-step outcomes and what was attempted. `attempted`
/// includes every step this call resolved, including one refused for want
/// of a privileged installer, because a refused step must not be retried in
/// the next wave: the refusal will not change.
///
/// This function is the effectful segment. It calls into `Installer`, which
/// is why it takes `installers` rather than performing anything itself.
pub fn perform_all(
    installers: &Installers,
    ready: &[Step],
) -> (Vec<(DependencyName, StepOutcome)>, Attempted) {
    let mut outcomes = Vec::with_capacity(ready.len());
    let mut attempted = BTreeSet::new();

    for step in ready {
        let outcome = match step.privilege {
            PrivilegeRequirement::None => installers.ordinary.perform(&step.action),
            PrivilegeRequirement::Root => match &installers.privileged {
                Some(privileged) => privileged.perform(&step.action),
                None => StepOutcome::NotAutomatable {
                    reason: NoInstallReason::PrivilegeUnavailable,
                },
            },
        };
        attempted.insert(step.dependency.clone());
        outcomes.push((step.dependency.clone(), outcome));
    }

    (outcomes, Attempted(attempted))
}

/// Describe every step in a plan.
///
/// This is `--dry-run`. `plan` is pure and complete before any effect, so a
/// dry run needs no `Installer` implementation of its own.
///
/// One honest limitation, visible rather than hidden: under the fixpoint a
/// dry run cannot simulate later waves, because it cannot know what
/// installing `nvm` does to the observations. It reports the first wave.
pub fn describe(installer: &dyn Installer, built: &Plan) -> Vec<ActionDescription> {
    built
        .steps
        .iter()
        .map(|step| installer.describe(&step.action))
        .collect()
}

/// Run the pipeline to a fixpoint.
///
/// The loop body is a pure step function: `plan` over the current
/// observations, then `perform_all`, then a **full** re-gather. Full, not
/// scoped to `attempted`: installing `oh-my-zsh` makes
/// `zsh-autosuggestions` installable, and a scoped re-gather would still
/// call it missing (`check-deps.sh:339` emits that clone only when the
/// oh-my-zsh custom directory exists). A full re-gather is also required if
/// apt installs are ever batched, because one `apt-get install a b c`
/// yields one exit status for three dependencies.
///
/// `gather` is a caller-supplied closure, so this module names no IO
/// capability. `Services` and `Log` belong to the CLI driver that calls
/// this, never to the core.
///
/// Termination: a dependency is removed from consideration once attempted,
/// so the loop runs at most once per dependency in the selection.
///
/// # Errors
///
/// Propagates `PlanError` from `plan`, which aborts before any effect.
pub fn run_to_fixpoint<GatherFn, Gathered>(
    manifest: &Manifest,
    manager: PackageManager,
    selection: &Selection,
    requirements: &Requirements,
    elevation: Elevation,
    packages: &std::collections::BTreeMap<DependencyName, PackageMap>,
    installers: &Installers,
    mut gather: GatherFn,
) -> Result<(Report, Vec<Event>), PlanError>
where
    GatherFn: FnMut() -> Gathered,
    Gathered: Observations,
{
    let mut observations = gather();
    let mut outcomes: Vec<(DependencyName, StepOutcome)> = Vec::new();
    let mut resolved: BTreeSet<DependencyName> = BTreeSet::new();
    let mut events = Vec::new();

    loop {
        let (built, wave_events) = plan(
            manifest,
            manager,
            selection,
            requirements,
            &observations,
            elevation,
            packages,
        )?;
        events.extend(wave_events);

        let ready: Vec<Step> = built
            .steps
            .into_iter()
            .filter(|step| !resolved.contains(&step.dependency))
            .collect();
        if ready.is_empty() {
            break;
        }

        let (wave_outcomes, attempted) = perform_all(installers, &ready);
        outcomes.extend(wave_outcomes);
        // `attempted` is only obtainable here, which is what forbids
        // writing the re-gather above the perform call.
        for step in &ready {
            if attempted.contains(&step.dependency) {
                resolved.insert(step.dependency.clone());
            }
        }

        observations = gather();
    }

    let (report, reconcile_events) = reconcile(manifest, &outcomes, &observations);
    events.extend(reconcile_events);
    Ok((report, events))
}
```

Add `mod driver; mod reconcile;` plus the re-exports to `lib.rs`, and add
both files to the `purity` test's `sources` array.

- [ ] **Step 7: Run the test to verify it passes**

Run: `cd ~/crates && cargo test --locked -p deps-core driver:: && cargo test --locked -p deps-core --doc`
Expected: PASS. The three `compile_fail` doctests in Step 1 now assert
something real, because `Attempted` exists and its field is private. Confirm
they are not vacuous: temporarily change one to a plain doctest, watch
`cargo test --doc` fail with `error[E0603]: tuple struct constructor
`Attempted` is private`, then restore `compile_fail`.

- [ ] **Step 8: Assert the ordering constraint directly**

The compile-fail doctests prove `Attempted` cannot be forged. They do not
prove the loop calls `gather` after `perform_all`. Add to `driver.rs`'s test
module:

```rust
    // The ordering the private constructor enforces, asserted from the
    // observable side as well. A re-gather written before perform_all
    // cannot compile, and this test says what the correct order produces:
    // the wave-2 plan sees the wave-1 install.
    #[test]
    fn the_regather_happens_after_the_perform_loop() {
        let manifest = oh_my_zsh_manifest();
        let order = RefCell::new(Vec::new());
        let worlds = ScriptedWorlds {
            worlds: RefCell::new(vec![
                ObservationMap::from_pairs(vec![]),
                ObservationMap::from_pairs(vec![(oh_my_zsh_check(), Observation::Present)]),
                ObservationMap::from_pairs(vec![
                    (oh_my_zsh_check(), Observation::Present),
                    (autosuggestions_check(), Observation::Present),
                ]),
            ]),
            gathers: RefCell::new(0),
        };

        struct OrderingInstaller<'a> {
            order: &'a RefCell<Vec<&'static str>>,
        }
        impl Installer for OrderingInstaller<'_> {
            fn describe(&self, _action: &InstallAction) -> ActionDescription {
                ActionDescription {
                    summary: String::new(),
                    privilege: PrivilegeRequirement::None,
                    command_preview: None,
                    changes_trust_root: false,
                }
            }
            fn perform(&self, _action: &InstallAction) -> StepOutcome {
                self.order.borrow_mut().push("perform");
                StepOutcome::Installed
            }
        }

        let installers = Installers {
            ordinary: Box::new(OrderingInstaller { order: &order }),
            privileged: None,
        };
        let (_report, _events) = run_to_fixpoint(
            &manifest,
            PackageManager::Apt,
            &Selection::all(&manifest),
            &Requirements::none(),
            Elevation::AlreadyRoot,
            &packages(),
            &installers,
            || {
                order.borrow_mut().push("gather");
                worlds.next()
            },
        )
        .expect("the pair converges");

        let recorded = order.borrow();
        assert_eq!(recorded[0], "gather", "one gather precedes the first plan");
        assert_eq!(recorded[1], "perform", "the first wave performs before re-gathering");
        assert_eq!(
            recorded[2], "gather",
            "the re-gather follows the perform loop, not the plan"
        );
    }
```

Run: `cd ~/crates && cargo test --locked -p deps-core driver::tests::the_regather_happens_after_the_perform_loop`
Expected: FAIL first if the loop was written with the re-gather above
`perform_all` (the sequence reads gather, gather, perform), which is the
reorder this task exists to forbid. With the Step 6 body it passes.

- [ ] **Step 9: Run the full suite**

Run: `cd ~/crates && cargo test --locked --workspace && cargo clippy --locked --all-targets -- -D warnings && cargo fmt --check`
Expected: PASS. Also re-run the `purity` test with all six modules listed:
`cargo test --locked -p deps-core purity::`.

- [ ] **Step 10: Commit**

```
config add crates/deps-core/src/driver.rs crates/deps-core/src/reconcile.rs crates/deps-core/src/lib.rs
config commit -m "Drive the pipeline to a fixpoint, with attempted coming back from the loop

The pipeline is a fixpoint, not a single pass. check-deps.sh:339 emits the
zsh-autosuggestions clone only inside
if [ -d \"\${ZSH_CUSTOM:-\$HOME/.oh-my-zsh/custom}\" ], and the comment at
:328-335 says why: cloning into a nonexistent ~/.oh-my-zsh lands the plugin
where nothing sources it, and the check would then report success for an
install that never loads. oh-my-zsh lives in deps-linux.conf:12. So wave 1
installs oh-my-zsh and only wave 2 can install zsh-autosuggestions. One
--fix pass does not converge, which one container run showed as \"no
automated install for zsh-autosuggestions\" and \"installed oh-my-zsh\" in
the same output.

The re-gather is full, not scoped to what was attempted. A scoped re-gather
would re-observe only oh-my-zsh and would still call zsh-autosuggestions
missing. Each check costs microseconds, so scoping was a false economy, and
a full re-gather is required anyway if apt installs are ever batched: one
apt-get install a b c yields one exit status for three dependencies, so
per-step outcomes stop being derivable from per-step exit codes.

attempted comes back from perform_all, not from plan, and Attempted's
constructor is private to this module. In the first draft plan.attempted was
available the instant plan returned, so writing the re-gather above the
perform loop compiled, type-checked, and reconciled every outcome against a
pre-install world. Three compile_fail doctests assert the privacy, and one
ordering test asserts what the correct order produces.

There is one Installer trait, not two. --dry-run is describe over the plan:
every StepOutcome variant is a false statement about a run that did nothing,
so a port whose return type cannot express one of its own implementations'
outcomes is leaking.

run_to_fixpoint takes gather as a closure, so this module names no IO
capability and the purity test covers it. Services and Log belong to the CLI
driver that calls this.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### What Tasks 7 to 11 deliberately leave to later tasks

- The `Installer` implementations, `gather`, `resolve_elevation`, the
  `Approval` prompt, `render`, and `Services`/`Log`. All live in
  `config-cli` and all hold capabilities, which is why none of them appear
  above.
- The `PackageMap` data for the real 22 entries. Task 9 defines the type and
  tests it with synthesized maps; populating it from `install_cmd_for`
  (`check-deps.sh:210-430`) is the adapter task, because the mapping from
  dependency to `InstallAction` is where the six hardcoded URLs live and
  those belong beside the adapter that spells the command.
- The 18-consumer rename, listed in spec 7.4 with the five literal-string
  pinners called out (`deps-harness.test.sh:136` and `:319-323`,
  `shellcheck.test.sh:85`, `scripts-dir-name.test.sh:56,220`,
  `README.md:111`).
- The Docker build stage. `docker/Dockerfile.ubuntu` and `Dockerfile.arch`
  copy only `.scripts/deps` and install only
  `sudo curl git wget ca-certificates`, so they cannot build a Rust binary
  today.
- `deps-docs.test.sh`, which spec 7.4 requires be updated or deleted in the
  same commit as the rename, because it uses exit 2 as its oracle and a
  missing program exits 127.
- `check-deps.test.sh:130` and `:141`, the two assertions Task 10's exit-code
  change makes red on purpose.

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
