# Branch Drift-Check + tmux.conf Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Automatically catch when the `mac` and `linux` dotfiles branches drift on paths that are supposed to be identical, and remove `tmux.conf` (the highest-recurring-pain file) from the manually-diffed set entirely.

**Architecture:** A tracked manifest (`.sync-manifest`) names the paths that must be byte-identical between branches. A POSIX-sh script (`tests/check-branch-drift.sh`) diffs those paths between two refs and reports divergence. A GitHub Action runs that script on every push to either branch. `tmux.conf`'s shared content moves into a new `tmux-common.conf`, tracked identically on both branches and covered by the manifest, so it stops needing manual diffing.

**Tech Stack:** POSIX `sh` (repo convention for `.my-scripts/`), `bash` for test files (repo convention for `tests/*.test.sh`), GitHub Actions (`ubuntu-latest`), the existing `tests/lib.sh` harness.

**Spec:** `docs/superpowers/specs/2026-09-02-branch-drift-sync-design.md`

## Global Constraints

- This repo is public. The pre-commit leak guard (`tests/leak-check.sh`) must pass on every commit; never bypass it with `--no-verify` or by disabling it.
- Run `tests/run-all.sh` and confirm it passes before every commit that touches `.my-scripts/`, `.claude/scripts/`, `.claude/hooks/`, or `tests/` (per `.claude/rules/dotfiles-tests.md`).
- Shell scripts under `.my-scripts/` and `tests/*.sh` (non-test-file scripts) use `#!/bin/sh` (POSIX), matching every existing script in those directories. Test files (`tests/*.test.sh`) use `#!/bin/bash`, matching every existing test file.
- Commit messages follow this repo's established style: lowercase, imperative, no period, a body paragraph explaining *why* when the summary line isn't self-evident (see recent `git log` on `linux`).
- `.sync-manifest`, `.github/workflows/branch-drift.yml`, and every path they cover must end up byte-identical on both `mac` and `linux` — that identity is the entire mechanism.
- Never use `git add <directory>` on `.claude` or the repo root — always name exact files/subpaths. (See the credentials near-miss in this session's history: a bare `git add .claude` swept up `.claude/.credentials.json` and embedded plugin-cache repos.)

---

### Task 1: Research note

**Files:**
- Create: `docs/research/dotfiles-management-landscape.md`

**Interfaces:**
- Consumes: nothing (standalone reference document).
- Produces: nothing consumed by later tasks; referenced by the spec.

This is documentation, not code — no test cycle applies. Skipping TDD here because there's no behavior to make pass; the deliverable is the content itself being present and accurate.

- [ ] **Step 1: Create the research note**

```markdown
# Dotfiles management landscape (2026-09-02)

Research pass done while designing the mac/linux branch drift-check
(see `docs/superpowers/specs/2026-09-02-branch-drift-sync-design.md`).
Question: is there a better model than branch-per-machine for keeping
platform-varying dotfiles in sync? Revisit this if drift keeps recurring
even after the manifest + GitHub Action + tmux.conf split are in place.

## Comparison

| Tool | Model | Solves "one file, few OS lines"? | Secrets | Momentum (2026) | Migration cost from a bare `~/.cfg` repo |
|---|---|---|---|---|---|
| **chezmoi** | Source dir renders to `$HOME` (not symlinks) | **Yes, natively.** Go templates: `dot_config/tmux/tmux.conf.tmpl` with `{{ if eq .chezmoi.os "darwin" }}...{{ else if eq .chezmoi.os "linux" }}...{{ end }}` inline in one file | Built-in: age/GPG/1Password/Bitwarden, safe to make the repo public | 21.4k GitHub stars, shipped a release Aug 30 2026, very active | Moderate — documented migrations run ~2-4 hrs: rename to `dot_*`, port install scripts to `run_once_` |
| **yadm** | Bare-repo wrapper (same core model as this repo) + alt-files (`##os.Darwin`) | Weaker — alt-files mean a whole separate file per OS, not one templated file. Historically leaned on `envtpl`/`j2cli`, both now unmaintained | GPG-encrypted files, built-in | 6.4k stars, maintained but slower-moving | **Lowest** — literally this repo's model plus alt-files/encryption bolted on |
| **GNU Stow / dotbot / rcm** | Symlink farms | **Doesn't solve it** — platform variance means separate directories/whole-file forks, same problem this repo already has | None built-in | Mature but stagnant (Stow); smaller (dotbot) | N/A — not a step up from where this repo is |
| **Nix home-manager (+ nix-darwin, flakes)** | Fully declarative, reproducible config-as-code | Solves it at a deeper level (conditional modules, cross-platform `nixpkgs`), but the config becomes Nix, not `tmux.conf` | Via agenix/sops-nix, more setup | Real 2025-2026 traction among power users, but recent write-ups pair it *with* chezmoi rather than replace it | **Highest** — new mental model, new apply/build loop, every tool needs to be Nix-packaged or shimmed |

## The concrete "few lines differ" case

chezmoi's answer for `tmux.conf`, as an example — the file becomes
`dot_config/tmux/tmux.conf.tmpl`:

```
{{- if eq .chezmoi.os "darwin" }}
bind-key -T copy-mode-vi 'y' send -X copy-pipe "pbcopy"
{{- else if eq .chezmoi.os "linux" }}
bind-key -T copy-mode-vi 'y' send -X copy-pipe "xclip -selection clipboard"
{{- end }}
```

Everything else in the file is written once and never diverges, because
there is only one file. This is a strictly better version of the
common-file-plus-`source-file` split this repo uses instead — no second
file to remember to source, at the cost of adopting chezmoi's templating
and apply/diff workflow.

## Community sentiment

From an HN "Better Dotfiles" thread and related discussion, as of 2026:

- chezmoi is the clear favorite among people who outgrew symlink farms —
  praised for templating, secrets handling, and an actively-engaged
  maintainer.
- Bare git repo (this repo's exact model) still has a real, vocal
  long-term user base — "used this for nearly a decade without issues"
  is a common sentiment. Not considered obsolete, just less capable on
  the templating/secrets axis.
- Consensus is explicitly "no single best tool, depends on your
  tolerance for setup cost," not "bare-repo users are wrong."
- Nix/home-manager users call it "end-game" but often keep chezmoi in
  the loop anyway, specifically to avoid Nix's immutability fighting
  with everyday app configs.

## Recommendation

chezmoi is the legitimate answer if this repo ever migrates to
anything — mature, actively maintained, and it directly beats the
common/platform file split used here. But the actual platform-varying
surface in this repo is small: three files (`tmux.conf`, `.zshrc`,
`alacritty.toml`), with everything else either fully portable already
(`.claude`, `tests`, `.my-scripts`) or fully platform-exclusive
(Aerospace, iTerm, `notify.sh`). The manifest + GitHub Action + tmux.conf
split gets most of chezmoi's practical benefit for the two or three
files that actually need it, with zero migration cost and zero new tool
to learn. Nix is not worth pursuing here at all — its value proposition
(reproducible package management) doesn't match a problem that's
specifically about syncing dotfiles content.

**Decision: stay on the branch-per-machine model. Revisit chezmoi only
if drift keeps recurring even with the manifest + Action + tmux split in
place.**
```

- [ ] **Step 2: Commit**

```bash
config() { /usr/bin/git --git-dir="$HOME/.cfg/" --work-tree="$HOME" "$@"; }
config add docs/research/dotfiles-management-landscape.md
config commit -m "add dotfiles-management research note (chezmoi/yadm/Nix comparison)"
```

---

### Task 2: Portability manifest + drift-check script

**Files:**
- Create: `.sync-manifest`
- Create: `tests/check-branch-drift.sh`
- Test: `tests/check-branch-drift.test.sh`

**Interfaces:**
- Consumes: `$DOTFILES_ROOT` env var (defaults to `$HOME`, same convention as `tests/run-all.sh`); reads `$DOTFILES_ROOT/.sync-manifest` from the ref passed as the first argument.
- Produces: `tests/check-branch-drift.sh [ref-a] [ref-b]` (defaults `origin/mac` `origin/linux`) — exit 0 and a one-line summary to stdout if every manifest path matches; exit 1, one `diverged: <path>` line per divergence on stdout, and a summary on stderr otherwise. Exit 1 with a `.sync-manifest could not be read` message on stderr if the manifest is missing from ref-a. This is the contract Task 3's GitHub Action step depends on.

- [ ] **Step 1: Write `.sync-manifest`**

```
# Paths that must be byte-identical between the mac and linux branches.
# Checked by tests/check-branch-drift.sh and .github/workflows/branch-drift.yml.
#
# The manifest lists itself and the workflow directory: both must be
# identical on both branches for the mechanism to bootstrap correctly.

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
```

(`.config/tmux/tmux-common.conf` is added in Task 4, once it exists.)

- [ ] **Step 2: Write the failing test — `tests/check-branch-drift.test.sh`**

```bash
#!/bin/bash
#
# Integration tests for tests/check-branch-drift.sh
#
# The script compares two git refs against every path listed in
# .sync-manifest and reports any that differ. Fixtures are real git
# repositories, not fakes: this is orchestration of `git diff`, and
# stubbing git would only test the stub.
#
# Usage: ~/tests/check-branch-drift.test.sh

. "$(dirname "$0")/lib.sh"

SCRIPT="$DOTFILES_ROOT/tests/check-branch-drift.sh"

# Builds a repo with a "linux" branch (default) and a "mac" branch, both
# starting from the same manifest and the same shared file content.
make_manifest_repo() {
    local repo
    repo=$(make_repo "$1" linux)
    printf '# comment, ignored\n\nshared.txt\n' > "$repo/.sync-manifest"
    printf 'same content\n' > "$repo/shared.txt"
    git -C "$repo" add .sync-manifest shared.txt
    git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "add manifest"
    git -C "$repo" branch mac
    printf '%s' "$repo"
}

run_check() {
    local repo=$1; shift
    DOTFILES_ROOT="$repo" "$SCRIPT" "$@"
}

# --- matching manifest paths pass ---------------------------------------

repo=$(make_manifest_repo repo-match)
output=$(run_check "$repo" mac linux)
status=$?
assert_equals 'matching paths exit 0' '0' "$status"
assert_contains 'reports the match' 'match on all 1 shared path' "$output"

# --- a diverged manifest path fails --------------------------------------

repo=$(make_manifest_repo repo-diverge)
git -C "$repo" checkout -q mac
printf 'different content\n' > "$repo/shared.txt"
git -C "$repo" add shared.txt
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "diverge on mac"
git -C "$repo" checkout -q linux

output=$(run_check "$repo" mac linux)
status=$?
assert_equals 'a diverged path exits non-zero' '1' "$status"
assert_contains 'names the diverged path' 'diverged: shared.txt' "$output"

# --- comments and blank lines in the manifest are ignored ----------------

repo=$(make_repo repo-comments linux)
printf '# just a comment\n\n\n' > "$repo/.sync-manifest"
git -C "$repo" add .sync-manifest
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m "empty manifest"
git -C "$repo" branch mac

output=$(run_check "$repo" mac linux)
status=$?
assert_equals 'a manifest with only comments passes' '0' "$status"
assert_contains 'reports zero paths checked' 'match on all 0 shared path' "$output"

# --- a missing manifest fails loudly, not silently ------------------------

repo=$(make_repo repo-no-manifest linux)
git -C "$repo" branch mac

output=$(run_check "$repo" mac linux 2>&1)
status=$?
assert_equals 'a missing manifest exits non-zero' '1' "$status"
assert_contains 'names the missing manifest' '.sync-manifest' "$output"

finish
```

- [ ] **Step 3: Run the test to verify it fails**

```bash
chmod +x ~/tests/check-branch-drift.test.sh
bash ~/tests/check-branch-drift.test.sh
```

Expected: fails immediately with `No such file or directory` (`check-branch-drift.sh` doesn't exist yet).

- [ ] **Step 4: Write the minimal implementation — `tests/check-branch-drift.sh`**

```sh
#!/bin/sh
#
# Checks that every path listed in .sync-manifest is identical between two
# git refs. Prints each diverged path and exits non-zero if any are found.
#
# Usage:
#   check-branch-drift.sh [ref-a] [ref-b]
#
# Defaults to comparing origin/mac against origin/linux inside
# $DOTFILES_ROOT. Reads the manifest from ref-a, since both refs are
# expected to carry an identical copy of it.

set -u

DOTFILES_ROOT=${DOTFILES_ROOT:-$HOME}
REF_A=${1:-origin/mac}
REF_B=${2:-origin/linux}

manifest=$(git -C "$DOTFILES_ROOT" show "$REF_A:.sync-manifest" 2>/dev/null) || {
    printf 'check-branch-drift: could not read .sync-manifest from %s\n' "$REF_A" >&2
    exit 1
}

diverged_count=0
checked_count=0

# Splitting on newline in a for-loop, not the usual pipe-into-while: a piped
# while runs in a subshell in POSIX sh, and these counters need to survive
# the loop.
old_ifs=$IFS
IFS='
'
for path in $manifest; do
    IFS=$old_ifs
    case $path in
        ''|'#'*) continue ;;
    esac
    checked_count=$((checked_count + 1))

    if ! git -C "$DOTFILES_ROOT" diff --quiet "$REF_A" "$REF_B" -- "$path"; then
        printf 'diverged: %s\n' "$path"
        diverged_count=$((diverged_count + 1))
    fi
    IFS='
'
done
IFS=$old_ifs

if [ "$diverged_count" -gt 0 ]; then
    printf '\ncheck-branch-drift: %s and %s diverged on %d of %d shared path(s)\n' \
        "$REF_A" "$REF_B" "$diverged_count" "$checked_count" >&2
    exit 1
fi

printf 'check-branch-drift: %s and %s match on all %d shared path(s)\n' \
    "$REF_A" "$REF_B" "$checked_count"
exit 0
```

- [ ] **Step 5: Make it executable and run the test to verify it passes**

```bash
chmod +x ~/tests/check-branch-drift.sh
bash ~/tests/check-branch-drift.test.sh
```

Expected: `check-branch-drift: 4 passed, 0 failed`.

- [ ] **Step 6: Run the full suite**

```bash
bash ~/tests/run-all.sh
```

Expected: all suites pass, including the new one.

- [ ] **Step 7: Commit**

```bash
config() { /usr/bin/git --git-dir="$HOME/.cfg/" --work-tree="$HOME" "$@"; }
config add .sync-manifest tests/check-branch-drift.sh tests/check-branch-drift.test.sh
config commit -m "add .sync-manifest and a drift-check script for the mac/linux branches"
```

---

### Task 3: GitHub Action

**Files:**
- Create: `.github/workflows/branch-drift.yml`

**Interfaces:**
- Consumes: `tests/check-branch-drift.sh` (Task 2), invoked with no arguments (its defaults `origin/mac`/`origin/linux` are exactly right in a CI checkout).
- Produces: a GitHub Actions check named `branch-drift` on every push to `mac` or `linux`; nothing else depends on this.

There's no way to red/green-test a workflow file without pushing it — that verification happens in Task 5/6. What can be verified locally: the YAML parses, and the exact command the workflow runs behaves as expected against this machine's real fetched branches right now.

- [ ] **Step 1: Write the workflow**

```yaml
name: branch-drift

on:
  push:
    branches: [mac, linux]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Fetch both branches
        run: |
          git fetch origin mac:refs/remotes/origin/mac
          git fetch origin linux:refs/remotes/origin/linux

      - name: Check for drift
        env:
          DOTFILES_ROOT: ${{ github.workspace }}
        run: |
          # No `set -e`: the assignment below must survive a non-zero exit so
          # the output and step summary are still written before exiting.
          # (`set -e` combined with `output=$(cmd)` exits immediately on a
          # failing cmd, before the summary-writing code ever runs -- caught
          # by actually reasoning through the failure path before shipping it.)
          output=$(tests/check-branch-drift.sh)
          status=$?
          echo "$output"
          if [ -n "$GITHUB_STEP_SUMMARY" ]; then
            {
              echo '### Branch drift check'
              echo '```'
              echo "$output"
              echo '```'
            } >> "$GITHUB_STEP_SUMMARY"
          fi
          exit "$status"
```

- [ ] **Step 2: Validate the YAML parses**

```bash
python3 -c "import yaml, sys; yaml.safe_load(open('$HOME/.github/workflows/branch-drift.yml'))" && echo "YAML OK"
```

Expected: `YAML OK`. (`PyYAML` ships with most systems' python3; if this errors with `ModuleNotFoundError`, `pip install --user pyyaml` first — it's a local dev-time check only, not a repo dependency.)

- [ ] **Step 3: Dry-run the check command against the real branches**

```bash
cd ~ && tests/check-branch-drift.sh
```

Expected at this point in the plan: exits 1, since `linux` doesn't have `.sync-manifest`/`tests/check-branch-drift.sh`/etc. on `mac` yet (Task 6 hasn't run). Confirms the script correctly detects real drift, not just fixture drift.

- [ ] **Step 4: Commit**

```bash
config() { /usr/bin/git --git-dir="$HOME/.cfg/" --work-tree="$HOME" "$@"; }
config add .github/workflows/branch-drift.yml
config commit -m "add a GitHub Action that fails the push if mac/linux drift on shared paths"
```

---

### Task 4: tmux.conf common/platform split

**Files:**
- Create: `.config/tmux/tmux-common.conf`
- Modify: `.config/tmux/tmux.conf`
- Modify: `.sync-manifest`
- Test: `tests/tmux-conf-split.test.sh`

**Interfaces:**
- Consumes: nothing new.
- Produces: `.config/tmux/tmux-common.conf`, sourced by `.config/tmux/tmux.conf` via `source-file`. Task 6 mirrors this file byte-for-byte onto `mac`, and writes a `mac`-specific (pbcopy) remainder for that branch's `tmux.conf`.

This is a pure refactor (extraction, no new behavior) with no prior test coverage of tmux.conf's actual keybindings. Genuine red/green is still possible: write the test against the *target* end-state first, do the extraction *without* wiring the `source-file` line (red — the extracted content is now missing from the loaded config), then wire it up (green).

- [ ] **Step 1: Write the test — `tests/tmux-conf-split.test.sh`**

```bash
#!/bin/bash
#
# Confirms the tmux.conf / tmux-common.conf split preserves the config's
# actual behavior: the file parses cleanly, and specific keybindings/hooks
# that move into tmux-common.conf still resolve when tmux loads the real,
# split ~/.config/tmux/tmux.conf.
#
# Runs against the real installed config file, on a throwaway tmux SERVER
# (its own -L socket), not the developer's live server. `-f` only takes
# effect when tmux starts a fresh server -- passing it to a client of an
# already-running server (the one this test itself runs inside) is silently
# ignored, so a shared-socket test would assert on stale global state
# instead of the file actually under test.
#
# Usage: ~/tests/tmux-conf-split.test.sh

. "$(dirname "$0")/lib.sh"

CONFIG="$DOTFILES_ROOT/.config/tmux/tmux.conf"
SOCKET="tmux-conf-split-test-$$"
SESSION=$(session_name conf-split)

tmux_t() { tmux -L "$SOCKET" "$@"; }

extra_cleanup() {
    tmux_t kill-server 2>/dev/null
    cleanup
}
trap extra_cleanup EXIT
trap 'extra_cleanup; exit 130' INT
trap 'extra_cleanup; exit 143' TERM HUP

parse_errors=$(tmux_t -f "$CONFIG" new-session -d -s "$SESSION" -c "$FIXTURES" 2>&1 >/dev/null)
assert_equals 'the split config parses with no errors' '' "$parse_errors"

assert_contains 'wheel-scroll forwarding survives the split' 'Up Up Up' \
    "$(tmux_t list-keys -T root)"

assert_contains 'the window-naming after-new-window hook survives the split' \
    'tmux-update-window-names.sh' "$(tmux_t show-hooks -g)"

assert_equals 'mouse mode is on' 'on' "$(tmux_t show-options -g -v mouse)"

yank_binding=$(tmux_t list-keys -T copy-mode-vi | grep "copy-pipe")
assert_contains 'the platform yank command is present' 'xclip' "$yank_binding"

finish
```

**Two bugs found and fixed while actually running this test (both worth knowing before mirroring it to `mac` in Task 6):**

1. Without a dedicated `-L` socket, `tmux -f "$CONFIG" new-session` against an
   already-running server silently ignores `-f` and just creates a new
   session on the *existing* server, inheriting its already-loaded global
   state. The test passed 5/5 on the very first run, before
   `tmux-common.conf` even existed — a false green, not a real one.
2. `mouse_any_flag` is not a good substring for "our custom WheelUpPane
   binding exists" — it's part of tmux's own *built-in default* WheelUpPane
   binding too, so that assertion passed even with the custom binding
   absent. `Up Up Up` (from the `send -t= Up Up Up` arrow-key translation) is
   unique to our binding and was verified to actually go red before the
   split's `source-file` line was added, and green after.

- [ ] **Step 2: Extract the shared content into `tmux-common.conf`, without wiring it up yet**

Create `.config/tmux/tmux-common.conf`:

```
# remap prefix from 'C-b' to 'C-a'
# unbind C-b
# set-option -g prefix C-Space
# bind-key C-Space send-prefix

# TPM (Tmux Plugin Manager) Plugins #########################################################
# Note: all plugins listed below require `tpm` to be installed:
# see https://github.com/tmux-plugins/tpm
set -g @plugin 'tmux-plugins/tpm'
set -g @plugin 'tmux-plugins/tmux-sensible' # adds sensible defaults to tmux
set -g @plugin "nordtheme/tmux"

# Custom Keybindings #########################################################################
# split panes using prefix+| or -
bind | split-window -h -c "#{pane_current_path}"
bind \\ split-window -h -c "#{pane_current_path}"
bind - split-window -v -c "#{pane_current_path}"
unbind '"'
unbind %

# unbind Ctrl+t to avoid conflict with fzf
unbind -n C-t

# enable switching panes (switching panes defaults to Alt+ combinations)
# (must use alt for these to not conflict with custom neovim keybindings)
bind -n M-h select-pane -L
bind -n M-l select-pane -R
bind -n M-k select-pane -U
bind -n M-j select-pane -D

# windows (moving between windows defaults to Alt+ combinations)
bind -n C-M-h swap-window -t -1\; select-window -t -1
bind -n C-M-l swap-window -t +1\; select-window -t +1
bind -n M-1 select-window -t 1
bind -n M-2 select-window -t 2
bind -n M-3 select-window -t 3
bind -n M-4 select-window -t 4
bind -n M-5 select-window -t 5
bind -n M-6 select-window -t 6
bind -n M-7 select-window -t 7
bind -n M-8 select-window -t 8
bind -n M-9 select-window -t 9
bind -n M-0 command-prompt "select-window -t '%%'"
bind -n M-w choose-tree -Zw
bind -n M-v copy-mode

# named windows
bind -n M-c select-window -t Config
bind -n M-s select-window -t Staging
bind -n M-r select-window -t Reviews
bind -n M-o select-window -t Other
bind -n M-p select-window -t Plugins
bind -n M-t select-window -t DevTool

# window numbering
set -g base-index 1       # Start numbering windows at 1, not 0.
set -g pane-base-index 1  # Start numbering panes at 1, not 0.

# resize panes with prefix and Ctrl+hjkl
bind C-l resize-pane -R 10
bind C-k resize-pane -U 10
bind C-j resize-pane -D 10
bind C-h resize-pane -L 10

# Enable mouse mode (tmux 2.1 and above)
set -g mouse on

# Wheel scrolling inside full-screen apps (Claude Code, less, man, git log).
# tmux's default WheelUpPane binding only forwards the wheel when the app has
# requested mouse tracking; Claude Code does not, so the default drops into
# tmux copy-mode and scrolls the terminal instead of the app. When the pane is
# on the alternate screen and has no mouse tracking, translate the wheel into
# arrow keys so the application scrolls its own view.
bind -n WheelUpPane   if -F -t= '#{||:#{pane_in_mode},#{mouse_any_flag}}' 'send -M' \
  { if -F -t= '#{alternate_on}' 'send -t= Up Up Up' 'copy-mode -e' }
bind -n WheelDownPane if -F -t= '#{||:#{pane_in_mode},#{mouse_any_flag}}' 'send -M' \
  { if -F -t= '#{alternate_on}' 'send -t= Down Down Down' 'send -M' }

# Let escape sequences (Claude Code progress bar, desktop notifications, etc.)
# pass through tmux to the outer terminal.
set -g allow-passthrough on

# Use vi keybindings in copy mode
setw -g mode-keys vi

# Vi-style copy mode bindings (platform clipboard command lives in the
# per-branch tmux.conf; everything else here is shared)
bind-key -T copy-mode-vi 'v' send -X begin-selection                            # Start visual selection
bind-key -T copy-mode-vi Escape send -X clear-selection                          # Clear selection (visual -> normal mode)
bind-key -T copy-mode-vi 'q' send -X cancel                                      # Exit copy mode

# Automatic window names ####################################################################
# Name each window after its active pane: "repo/branch" if that directory is a
# repository, otherwise the directory basename. Repositories matching
# @wname_bare_repos are named by branch alone. Rename a window yourself with
# `prefix ,` to pin a name; rename it to an empty name to hand it back.
# See ~/.my-scripts/tmux-update-window-names.sh for the full precedence.
set-hook -g after-new-window      'run-shell -b "~/.my-scripts/tmux-update-window-names.sh -w #{window_id}"'
set-hook -g after-split-window    'run-shell -b "~/.my-scripts/tmux-update-window-names.sh -w #{window_id}"'
set-hook -g after-select-window   'run-shell -b "~/.my-scripts/tmux-update-window-names.sh -w #{window_id}"'
set-hook -g after-select-pane     'run-shell -b "~/.my-scripts/tmux-update-window-names.sh -w #{window_id}"'
set-hook -g after-kill-pane       'run-shell -b "~/.my-scripts/tmux-update-window-names.sh -w #{window_id}"'
set-hook -g client-session-changed 'run-shell -b "~/.my-scripts/tmux-update-window-names.sh -w #{window_id}"'

# Initialize TMUX plugin manager (keep this line at the very bottom of the
# fully-assembled config -- tmux.conf sources this file last for that reason)
run '~/.tmux/plugins/tpm/tpm'
```

Replace `.config/tmux/tmux.conf` with just the platform-specific remainder (no `source-file` line yet):

```
# Use the tmux-specific terminfo entry (ships with tmux, better escape-sequence
# and attribute support than the implicit screen-256color fallback) and pass
# true color through to the outer terminal.
set -g default-terminal "tmux-256color"
set -ag terminal-overrides ",*:RGB"

# Vi-style copy mode bindings: platform clipboard command differs (xclip on
# Linux/WSL, pbcopy on mac). See tmux-common.conf for everything else.
bind-key -T copy-mode-vi 'y' send -X copy-pipe "xclip -selection clipboard"     # Yank to clipboard, stay in copy mode
bind-key -T copy-mode-vi MouseDragEnd1Pane send -X copy-pipe "xclip -selection clipboard" # Mouse selection copies, stays in copy mode
```

- [ ] **Step 3: Run the test to verify it fails**

```bash
chmod +x ~/tests/tmux-conf-split.test.sh
bash ~/tests/tmux-conf-split.test.sh
```

Expected: FAIL on the `mouse mode is on` and `wheel-scroll forwarding` and `window-naming hook` assertions (that content isn't loaded — `tmux-common.conf` exists on disk but nothing sources it). The yank-binding assertion still passes (unaffected by the split).

- [ ] **Step 4: Wire up the source**

Append to `.config/tmux/tmux.conf`:

```
source-file ~/.config/tmux/tmux-common.conf
```

- [ ] **Step 5: Run the test to verify it passes**

```bash
bash ~/tests/tmux-conf-split.test.sh
```

Expected: `tmux-conf-split: 5 passed, 0 failed`.

- [ ] **Step 6: Add `tmux-common.conf` to the manifest**

Edit `.sync-manifest`, adding this line at the end:

```
.config/tmux/tmux-common.conf
```

- [ ] **Step 7: Source the live tmux server's config and confirm no regression in a real session**

```bash
tmux source-file ~/.config/tmux/tmux.conf
tmux list-keys -T root | grep WheelUpPane
tmux list-keys -T copy-mode-vi | grep copy-pipe
```

Expected: both greps return matching lines (this reloads *this* actual tmux server, not a throwaway fixture — confirms the split works in the session you're sitting in, not just in a test harness).

- [ ] **Step 8: Run the full suite**

```bash
bash ~/tests/run-all.sh
```

Expected: all suites pass, including both new ones.

- [ ] **Step 9: Commit**

```bash
config() { /usr/bin/git --git-dir="$HOME/.cfg/" --work-tree="$HOME" "$@"; }
config add .config/tmux/tmux-common.conf .config/tmux/tmux.conf .sync-manifest tests/tmux-conf-split.test.sh
config commit -m "split tmux.conf into common/platform files, add to the sync manifest"
```

---

### Task 5: Push `linux`

**Files:** none (verification task).

**Interfaces:**
- Consumes: all commits from Tasks 1-4.
- Produces: an updated `origin/linux`, which Task 6 diffs against when building `mac`'s mirror.

- [ ] **Step 1: Confirm the remote hasn't moved and the push is a clean fast-forward**

```bash
config() { /usr/bin/git --git-dir="$HOME/.cfg/" --work-tree="$HOME" "$@"; }
config fetch origin linux
config merge-base --is-ancestor origin/linux linux && echo "safe fast-forward"
```

Expected: `safe fast-forward`.

- [ ] **Step 2: Push**

```bash
config push origin linux
```

- [ ] **Step 3: Confirm**

```bash
config rev-parse linux origin/linux
```

Expected: both hashes identical.

---

### Task 6: Mirror to `mac`, push, confirm the Action runs green on both

**Files (on the `mac` branch):**
- Create: `docs/research/dotfiles-management-landscape.md` (identical to `linux`)
- Create: `.sync-manifest` (identical to `linux`)
- Create: `tests/check-branch-drift.sh`, `tests/check-branch-drift.test.sh` (identical to `linux`)
- Create: `.github/workflows/branch-drift.yml` (identical to `linux`)
- Create: `.config/tmux/tmux-common.conf` (identical to `linux`)
- Create: `tests/tmux-conf-split.test.sh` (identical to `linux`, but the yank-binding assertion checks for `pbcopy` instead of `xclip`)
- Modify: `.config/tmux/tmux.conf` on `mac` (platform remainder + `source-file`, adapted to whatever mac's current platform-specific lines are)

**Interfaces:**
- Consumes: every file produced by Tasks 1-4, verbatim except the two files noted above that are intentionally platform-specific.
- Produces: an `origin/mac` that the drift-check considers identical to `origin/linux` on every manifest path.

Everything here is a direct `git checkout <linux-commit> -- <path>` port — the same mechanism used throughout this session's earlier mac→linux sync, just in reverse. The one file that needs actual authorship (not a straight copy) is `mac`'s `tmux.conf` remainder, since it must keep `mac`'s own platform lines (pbcopy, no terminal-overrides block per this session's earlier diff) rather than linux's.

- [ ] **Step 1: Switch to `mac` and pull its latest**

```bash
config() { /usr/bin/git --git-dir="$HOME/.cfg/" --work-tree="$HOME" "$@"; }
config checkout mac
config pull --ff-only origin mac
```

Expected: fast-forwards cleanly (no local `mac` commits diverge from `origin/mac`).

- [ ] **Step 2: Port every platform-neutral file from `linux`**

```bash
config checkout linux -- \
  docs/research/dotfiles-management-landscape.md \
  .sync-manifest \
  tests/check-branch-drift.sh \
  tests/check-branch-drift.test.sh \
  .github/workflows/branch-drift.yml \
  .config/tmux/tmux-common.conf
```

- [ ] **Step 3: Read mac's current `tmux.conf` to see what platform lines it actually has right now**

```bash
config show mac:.config/tmux/tmux.conf
```

Note the current pbcopy-based yank bindings and any other mac-only lines (this session's earlier research found mac's copy did **not** have a `default-terminal`/`terminal-overrides` block — confirm that's still true before assuming it).

- [ ] **Step 4: Remove from `tmux.conf` everything now in `tmux-common.conf`, keep only the platform remainder, add the `source-file` line**

Edit `.config/tmux/tmux.conf` (on `mac`) so it contains exactly: whatever platform-only lines Step 3 revealed (expected: the two pbcopy `copy-pipe` bindings, and any other line absent from `tmux-common.conf`), followed by:

```
source-file ~/.config/tmux/tmux-common.conf
```

- [ ] **Step 5: Write `tests/tmux-conf-split.test.sh` for mac (pbcopy variant)**

Same file as `linux`'s, with one line changed:

```bash
yank_binding=$(tmux list-keys -T copy-mode-vi | grep "copy-pipe")
assert_contains 'the platform yank command is present' 'pbcopy' "$yank_binding"
```

(This machine can't execute this test — it's `sh`/tmux-portable code with no macOS-specific calls, matching every other script mirrored in this session, but running it here would test the *linux* config, not mac's. It must be run on the mac machine to actually verify; note this as a follow-up for Austin to confirm next time he's on that machine.)

- [ ] **Step 6: Verify the manifest paths are now identical between the two branches, locally**

```bash
config fetch origin linux mac
DOTFILES_ROOT="$HOME" bash -c '
  cd "$HOME"
  export GIT_DIR="$HOME/.cfg"
  export GIT_WORK_TREE="$HOME"
  tests/check-branch-drift.sh mac linux
'
```

Expected: exits 0, `check-branch-drift: mac and linux match on all N shared path(s)` (comparing the *local, not-yet-pushed* `mac` branch against `linux`, using the working-tree copies of the script/manifest that are about to be committed — this is the acceptance check for the whole plan).

If this fails, the diverged-path list tells you exactly what's still out of sync; fix it before committing.

- [ ] **Step 7: Commit on `mac`**

```bash
config add docs/research/dotfiles-management-landscape.md .sync-manifest \
  tests/check-branch-drift.sh tests/check-branch-drift.test.sh \
  .github/workflows/branch-drift.yml .config/tmux/tmux-common.conf \
  .config/tmux/tmux.conf tests/tmux-conf-split.test.sh
config commit -m "merge in linux changes - drift-check manifest/script/Action, tmux.conf common/platform split"
```

- [ ] **Step 8: Push `mac`**

```bash
config fetch origin mac
config merge-base --is-ancestor origin/mac mac && echo "safe fast-forward"
config push origin mac
```

- [ ] **Step 9: Confirm the Action ran and passed on both branches**

```bash
gh run list --branch linux --workflow branch-drift.yml --limit 1
gh run list --branch mac --workflow branch-drift.yml --limit 1
```

Expected: both show a recent run with conclusion `success`. If `gh` isn't authenticated in this environment, report the two branches' latest commit SHAs to Austin and ask him to confirm the Actions tab shows green, since this is the one step that can't be fully verified headlessly.

---

## Self-review notes

- Spec coverage: all five spec sections (research note, manifest, drift script, GitHub Action, tmux split) map to Tasks 1-4; the spec's rollout order (steps 1-9) maps to Tasks 1-6.
- No placeholders: every script, test, and config file above is complete, real content — nothing marked TBD.
- Type/name consistency checked: `check-branch-drift.sh`'s ref-arg order (`ref-a` then `ref-b`) matches its test's calls and the Action's no-arg default; `.sync-manifest`'s path list in Task 2 gains exactly one line in Task 4 Step 6, matching the file's final state referenced in Task 6.
- Task 6 Step 5 and Step 9 are the two places this plan cannot fully self-verify from this machine (mac-side test execution, GitHub Actions UI) — flagged explicitly rather than asserted as done.
