# Exhaustive .sync-manifest design

**Date:** 2026-09-03

**Problem:** A file tracked on one branch but not covered by any
`.sync-manifest` rule is invisible to the branch-drift check. The check reports
"match on all N shared path(s)" while the branches genuinely differ.

**Goal:** Make every tracked file account for itself, so a file added on one
branch cannot silently escape the check.

## Measured scope

Numbers taken from `mac` and `linux` on 2026-09-03, not estimated.

- 251 tracked files on `mac`.
- 201 covered by a manifest rule.
- 2 explicitly excluded (`tests/notify.test.sh`,
  `.my-scripts/deps/deps-local.conf`).
- **48 covered by nothing**, so drift in them is invisible.

Of those 48, two groups behave differently:

**39 are already byte-identical on both branches**, kept in sync by hand with
nothing enforcing it. These are the latent bug:

| Path | Files |
|---|---|
| `.config/nvim/` | 32 |
| `.agents/` | 5 |
| `.claude/data/` | 1 |
| `DOTFILES.md` | 1 |

**9 legitimately differ per platform**: `.claude/hooks/notify.sh`,
`.config/aerospace/aerospace.toml`, `.config/alacritty/alacritty.toml`,
`.config/iterm-profiles/com.googlecode.iterm2.plist`,
`.config/tmux/tmux.conf`, `.zshrc`, `.zshrc-mac`, `README.md`,
`TODO-AGENTS.md`.

39 + 9 = 48, so every uncovered file is accounted for above.

### Reproducing the gap

A new file *inside* a manifest-listed directory is already caught, because the
rule is a path prefix. The gap is only for paths outside every rule:

```sh
# Caught today: tests/ is listed.
printf 'x\n' > tests/only-on-mac.sh && git add tests/only-on-mac.sh
check-branch-drift.sh master linux    # -> diverged on 1 of 2

# Invisible today: no rule covers a new top-level path.
printf 'x\n' > brand-new-script.sh && git add brand-new-script.sh
check-branch-drift.sh master linux    # -> match on all 2
```

## Design decisions

Two decisions were settled before the design, and both shape everything below.

**Unlabeled means error, not warning.** A tracked file matching no rule fails
the check. A warning in a green build is a warning nobody reads, which is
approximately today's behavior. Failing closed is what makes the gap
impossible to reintroduce.

**The 39 hand-synced files get marked shared.** They are identical today, so
the check goes green immediately and stays green, and the first unmirrored
nvim change starts failing instead of drifting quietly.

## Manifest format

Keep the existing line-prefix syntax and add one sigil. Sections
(`[shared]` / `[per-branch]`) were considered and rejected: the parser is
POSIX `sh` doing prefix matching, and sections would mean carrying parser
state across lines for no benefit.

```
# Shared: must be byte-identical on both branches.
tests/
.my-scripts/
.config/nvim/
.agents/
.claude/data/
DOTFILES.md

# Excluded from an otherwise-shared path.
!tests/notify.test.sh
!.my-scripts/deps/deps-local.conf

# Per-branch by design: tracked, never compared. All nine of the
# platform-specific paths, so the example is the real end state.
~.zshrc
~.zshrc-mac
~.zshrc-linux
~.claude/hooks/notify.sh
~.config/aerospace/
~.config/alacritty/
~.config/iterm-profiles/
~.config/tmux/tmux.conf
~README.md
~TODO-AGENTS.md
```

`.zshrc-mac` and `.zshrc-linux` both appear because the manifest is itself a
shared file: each branch carries rules for paths it does not have. A `~` rule
for a path absent on this branch is not an error, it is the normal case.

`!` and `~` both suppress comparison but record different intent, and keeping
them distinct is deliberate:

- `!path` means "inside a shared path, skip this one."
- `~path` means "accounted for, never compared."

Only `~` satisfies exhaustiveness. Collapsing them into one sigil would make
the manifest stop recording *why* a path is not compared.

## The exhaustiveness check

A second phase in `check-branch-drift.sh`, after the existing comparison.

1. Build the tracked file list from **both** refs, unioned.
2. For each file, test whether any rule covers it (shared prefix, `!`, or `~`).
3. Report every file matching nothing, then exit 1.

**The union is load-bearing, not defensive.** The branches differ by six
files today:

| Only on `mac` | Only on `linux` |
|---|---|
| `.claude/hooks/notify.sh` | `.zshrc-linux` |
| `.config/aerospace/aerospace.toml` | |
| `.zshrc-mac` | |
| `tests/notify.test.sh` | |
| `TODO-AGENTS.md` | |

Scanning one ref would let a file added only on the other branch escape,
which is the exact bug this design exists to close.

## CI output

A gate that says "something is unlabeled" without naming it moves the
investigation to the developer. Three changes.

### 1. Capture stderr in the workflow

**This is a live defect, independent of this feature.** The workflow runs:

```sh
output=$(tests/check-branch-drift.sh)
```

Command substitution captures stdout only, and the script writes its summary
to stderr. Verified on a fixture: a real drift failure puts only
`diverged: tests/` in the step summary, and the
`diverged on 1 of 1 shared path(s)` line is dropped.

Fix: `output=$(tests/check-branch-drift.sh 2>&1)`. Without it, every
improvement below is also dropped.

This ships as its own commit ahead of the feature, since it makes today's
failures easier to read regardless.

### 2. Name every unlabeled path

One per line, never truncated, never a bare count, with the fix inline:

```
check-branch-drift: 2 tracked file(s) match no .sync-manifest rule

  .config/foo/bar.toml        (on mac)
  .claude/newthing.md         (on mac, linux)

Every tracked file must match a rule, so a file added on one branch
cannot escape this check. Add one of these to .sync-manifest:

  .config/foo/bar.toml        shared: must be identical on both branches
  ~.config/foo/bar.toml       per-branch: tracked, never compared
  !.config/foo/bar.toml       excluded from an enclosing shared path
```

The `(on mac)` / `(on mac, linux)` annotation carries real information: a
file existing on only one branch usually tells the reader which label it
wants.

### 3. Separate the two failure modes in the step summary

Drift and unlabeled paths are distinct problems and get distinct headings,
rather than both landing in one code fence.

## Testing

The 10 existing assertions in `tests/check-branch-drift.test.sh` stay
unchanged. New fixtures:

| Case | Expected |
|---|---|
| Unlabeled file present | exit 1, file named in output |
| `~`-labeled file | exit 0 |
| Unlabeled file on `linux` only | exit 1 (proves the union) |
| New file under a shared directory prefix | exit 0, no manifest edit needed |
| `(on mac)` annotation | matches the branch the file is really on |

Each assertion is verified to fail when the behavior it guards is removed,
rather than assumed to work because the suite is green.

## Migration

Two commits, ordered so the check is never red on a pushed branch.

1. **Label every currently tracked path.** The four hand-synced groups become
   shared, the platform-specific files become `~`. Verified green before
   commit, since all 39 shared candidates are already identical.
2. **Enable the exhaustiveness phase**, with its tests. Green immediately,
   because step 1 labeled everything.

Splitting it this way means the labels survive if the check itself is ever
reverted.

## Consequences

Stated plainly, including the costs.

- **A genuinely new top-level path requires a manifest edit in the same
  commit.** New files under an already-shared directory (a new nvim plugin,
  a new test) need nothing, so this bites less often than it first appears.
- **`docs/` is only partly covered.** The manifest lists `docs/research/`,
  not `docs/`. This spec lives at `docs/superpowers/specs/`, so it will
  itself be an unlabeled path until labeled: a useful dogfood of the feature.
- **The 39 hand-synced files gain enforcement.** A one-branch nvim change
  starts failing CI. That is the intent, and it will be an adjustment.
- **Two failure modes to read.** A red check now means either drift or a
  missing label, which the separated output is designed to disambiguate.
