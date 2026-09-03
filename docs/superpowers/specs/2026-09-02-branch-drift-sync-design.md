# Keeping the mac/linux dotfiles branches in sync

Date: 2026-09-02
Status: approved, pending implementation

## Context

The dotfiles repo (`~/.cfg`, bare git repo tracking `$HOME`) uses one branch per
machine (`mac`, `linux`, `home`, `work`). Most content is genuinely portable
(`.claude/agents`, `.claude/rules`, `.claude/skills`, `.claude/scripts`,
`tests/`, `.my-scripts/`), some is genuinely platform-exclusive (Aerospace,
iTerm profiles, `notify.sh`), and a small number of files are 90% shared with
a few lines that must differ by platform (`.config/tmux/tmux.conf`, `.zshrc`,
`.config/alacritty/alacritty.toml`).

Syncing mac's changes into linux has been a fully manual process: notice (or
be told) that mac has moved on, diff the two branches, classify every changed
path as portable / platform-exclusive / needs-manual-merge, and hand-port the
portable pieces. This is expensive to redo from scratch every time and easy to
get wrong under time pressure (see: the `.claude/.credentials.json` near-miss
during the last sync, caused by a bare `git add .claude` sweeping in untracked
runtime files).

A research pass (2026-09-02) compared this bare-repo model against chezmoi,
yadm, GNU Stow/dotbot/rcm, and Nix home-manager as alternatives that solve
platform variance via templating rather than branch divergence. Findings are
recorded in `docs/research/dotfiles-management-landscape.md`. Conclusion:
chezmoi is the legitimate answer if a full migration is ever justified, but
the actual platform-varying surface here is small (three files), so the
lightweight plan below gets most of the practical benefit for near-zero
migration cost. Chezmoi stays shelved as a documented option, not pursued now.

## Goals

- Catch drift on portable paths automatically, at push time, regardless of
  which machine pushed.
- Remove `tmux.conf` from the manually-diffed set by extracting its actually-
  shared content into a file that's simply tracked identically on both
  branches (folding it into the same automated check).
- Do this without adopting new infrastructure (no bots, no new services) and
  without changing the branch-per-machine model.

## Non-goals

- Eliminating manual merging entirely. `.zshrc` and `alacritty.toml` still
  have real platform-conditional content that isn't worth extracting yet (the
  shared/varying line ratio is much lower than tmux.conf's was, and neither
  has caused repeated pain the way tmux.conf just did).
- Migrating to chezmoi, yadm, or Nix. Documented as a later option, not built.
- Auto-merging drift. The check's job is to make drift loud, not to resolve it.

## Design

### 1. Research note

`docs/research/dotfiles-management-landscape.md`, committed to both `mac` and
`linux` (portable reference content, same bucket as `.claude`). Contents: the
comparison table and recommendation from the 2026-09-02 research pass.

### 2. Portability manifest

`.sync-manifest` at the repo root, tracked identically on both branches.
Plain text, one path prefix per line, `#`-comments allowed. Initial contents:

```
# Paths that must be byte-identical between the mac and linux branches.
# Checked by tests/check-branch-drift.sh and .github/workflows/branch-drift.yml.

.sync-manifest
.github/workflows/
.claude/agents/
.claude/rules/
.claude/skills/
.claude/scripts/
.claude/CLAUDE.md
tests/
.my-scripts/
docs/research/
.config/tmux/tmux-common.conf
```

The manifest lists itself and the workflow directory: both must be identical
on both branches for the mechanism to bootstrap correctly (a branch running a
stale drift-checker or an out-of-sync manifest would silently under-check
itself).

`.claude/hooks/` is deliberately excluded — `notify.sh` is mac-only by design.

### 3. Drift-check script

`tests/check-branch-drift.sh <ref-a> <ref-b>` (defaults to `origin/mac` and
`origin/linux` when called with no args). For each path in `.sync-manifest`,
compares the tree object at that path between the two refs (`git rev-parse
<ref>:<path>` or `git diff --quiet <ref-a> <ref-b> -- <path>`). Prints every
diverged path and exits non-zero if any are found; prints a one-line summary
and exits 0 otherwise.

TDD: `tests/check-branch-drift.test.sh` is written first, using the existing
`lib.sh` fixture helpers to build a throwaway git repo with two branches — one
scenario where manifest paths match, one where a manifest path has been
edited on only one branch. Red (script doesn't exist / doesn't detect it),
then green. Auto-discovered by `tests/run-all.sh`'s existing `*.test.sh` glob.

### 4. GitHub Action

`.github/workflows/branch-drift.yml`, tracked identically on both branches
(itself covered by the manifest once added — see rollout order below).

- Trigger: `on: push: branches: [mac, linux]`.
- Steps: checkout with `fetch-depth: 0` (need both branches' full history to
  diff trees), fetch both `origin/mac` and `origin/linux` explicitly (the
  checkout action only fetches the pushed ref by default), run
  `tests/check-branch-drift.sh`.
- On failure: the diverged paths from the script's stdout become the job's
  failure output; a `GITHUB_STEP_SUMMARY` write gives a readable list in the
  Actions UI. No issue-bot, no external notification — a failed check on your
  own push is sufficient signal (confirmed during brainstorming: fail-check
  only, no tracking issue).

### 5. tmux.conf common/platform split

New file `.config/tmux/tmux-common.conf`, tracked identically on both
branches, added to `.sync-manifest`. Contains everything currently duplicated
verbatim between the branches: the prefix-remap comments, TPM plugin list,
custom keybindings (split/resize/window-select), window numbering, mouse
mode, wheel-scroll forwarding, passthrough, the vi copy-mode structure (`v`,
`Escape`, `q` bindings — everything except the yank command itself), the
window-naming hooks, and the TPM init line.

Each branch's `.config/tmux/tmux.conf` shrinks to just its platform-specific
lines plus a source of the common file:

- `linux`: the `default-terminal "tmux-256color"` / `terminal-overrides
  ",*:RGB"` lines, the two `xclip`-based yank bindings, then `source-file
  ~/.config/tmux/tmux-common.conf`.
- `mac`: the two `pbcopy`-based yank bindings, then the same `source-file`
  line.

Testing: no existing test covers tmux.conf's actual keybindings (the test
suite covers the scripts, not the conf file). This is a pure refactor
(extraction, not new behavior), so classic red/green TDD doesn't map cleanly
— there's nothing failing yet to make pass. Practical substitute: capture
`tmux list-keys` output from a throwaway session on the *current* (unsplit)
config as a snapshot, do the split, start a new throwaway session on the
*split* config, and assert the two `list-keys` outputs are identical. Written
as `tests/tmux-conf-split.test.sh` before the split lands, so it's genuinely
red (no `tmux-common.conf` exists yet) before it's green.

## Rollout order

1. Write and commit the research note on `linux`.
2. Write `.sync-manifest` (without the tmux-common.conf line yet) and
   `tests/check-branch-drift.sh` + its test, TDD order, on `linux`.
3. Write `.github/workflows/branch-drift.yml` on `linux`.
4. Split `tmux.conf`, add `tests/tmux-conf-split.test.sh`, add the
   `tmux-common.conf` line to `.sync-manifest`, on `linux`.
5. Run the full suite (`tests/run-all.sh`), confirm green.
6. Push `linux`.
7. Mirror every file touched above onto `mac` (research note, manifest,
   drift-check script + test, workflow, tmux split + its test), adapting only
   the platform-specific `tmux.conf` remainder (pbcopy instead of xclip/RGB
   lines). This machine can't run the mac-side test suite, but the
   script/test logic being mirrored is platform-neutral and already verified
   green on `linux`.
8. Push `mac`.
9. Confirm the Action runs and passes on both branches' latest pushes.

## Open questions

None outstanding — all decisions above were confirmed during brainstorming.
