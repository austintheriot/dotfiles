# CLI dependency checking for the dotfiles repo

Date: 2026-09-03
Status: approved, pending implementation

## Context

`~/README.md` lists the CLI tools this dotfiles setup needs (git, GitHub CLI,
Alacritty, zsh, zsh-autosuggestions, neovim, fzf, ripgrep, zoxide, xclip,
tmux, tpm, nvm, rustup) as manual install steps. There is no automated way to
tell whether a machine actually has all of them, or whether one has quietly
gone missing (removed, broken by an OS upgrade, never installed on a new
machine). This repo already solved an analogous problem for config drift
between the `mac` and `linux` branches (`.sync-manifest` +
`tests/check-branch-drift.sh` + `.github/workflows/branch-drift.yml`, see
`docs/superpowers/specs/2026-09-02-branch-drift-sync-design.md`); this spec
extends the same shared-file-plus-branch-overlay model to dependency
checking.

## Goals

- Catch missing dependencies on a fresh machine (setup) and on a machine
  that's already in use (drift), with one mechanism.
- Best-effort automated install for every dependency that has a real,
  version-stable, non-interactive install path. Dependencies with no such
  path (right now: only `nvm`, whose own docs only publish version-pinned
  install URLs) are check-only, with a docs link.
- Keep the shell fast: a new terminal must never block on a dependency
  check.
- Verify the check/install logic against real, disposable environments
  (Docker locally, GitHub-hosted runners in CI) before trusting it, rather
  than reasoning about `apt`/`brew`/`pacman` behavior from memory alone.
- Follow this repo's existing shared/branch-overlay pattern
  (`.zshrc-mac`/`.zshrc-linux`, `.sync-manifest`) rather than inventing a new
  one.

## Non-goals

- Version-freshness checking. The check verifies a dependency is *present*,
  not that it meets some minimum version. (Several of these tools — neovim
  in particular — ship outdated versions in Ubuntu's default apt repos; that
  is a separate, unsolved problem this feature doesn't take on.)
- Auto-installing on `home`/`work` branches. Those branches aren't covered by
  the existing branch-drift mechanism either; this feature follows that same
  boundary rather than expanding it.
- Migrating dependency declarations out of `~/README.md` prose entirely.
  `deps.conf` becomes the executable source of truth; the README keeps a
  short pointer to it (see Design §5) rather than losing its own narrative
  install instructions, which still matter for the couple of dependencies
  that stay manual.

## Design

### 1. Dependency manifest

`.my-scripts/deps/deps.conf`, tracked identically on `mac` and `linux` (added
to `.sync-manifest`, which already covers `.my-scripts/` as a whole
directory — no manifest edit needed for this file itself). Plain text,
pipe-delimited: `name|check_command|docs_url`. `check_command` is any shell
snippet that exits 0 when the dependency is present (`command -v git`,
`[ -d "$HOME/.oh-my-zsh" ]`, etc.) — most entries are `command -v <bin>`; a
few (`oh-my-zsh`, `zsh-autosuggestions`, `tpm`, whose "installation" is a
cloned directory rather than a binary on `PATH`) use a file/directory test
instead.

Platform-exclusive dependencies (currently: `xclip` on `linux`, `aerospace`
on `mac`) live in a sibling `deps-local.conf` in the same directory, which is
**excluded** from `.sync-manifest` via a `!.my-scripts/deps/deps-local.conf`
negation line (the same mechanism already used for `!tests/notify.test.sh`),
so each branch can carry its own without violating the "identical" contract
on the shared file.

`oh-my-zsh` gets its own manifest row even though `~/README.md`'s "Install
zsh" bullet is the one that actually links to `ohmyz.sh` — `zsh` and
`oh-my-zsh` are genuinely two different dependencies (the latter is what
`zsh-autosuggestions` installs into), and conflating them in the manifest
would make `zsh-autosuggestions`'s check meaningless on a plain-zsh machine.

### 2. Check/install engine

`.my-scripts/deps/check-deps.sh`, tracked identically on `mac` and `linux`.
POSIX `sh` (matches every other script in `.my-scripts/`). Reads `deps.conf`
and, if present, `deps-local.conf` (both overridable via `$DEPS_CONF` /
`$DEPS_LOCAL_CONF`, matching `check-branch-drift.sh`'s `$DOTFILES_ROOT`
override convention — this is what makes the script testable without
touching the real system).

CLI:
- `check-deps.sh` — check only. Prints each missing dependency, exits 1 if
  anything's missing, 0 otherwise. Never installs anything. This is the safe
  default and what the shell-startup hook uses.
- `check-deps.sh --fix` — check, then interactively prompt (`y/N`) before
  installing each missing dependency.
- `check-deps.sh --fix --yes` — check, then install every missing dependency
  without prompting. Used by the Docker harness and CI.
- `check-deps.sh --fix --dry-run` — check, then print what `--fix` would run
  without running it.

Package-manager detection (`pacman` → `apt-get` → `brew`, in that priority
order — a real machine only ever has one of these) picks the install command
per dependency: a `case` statement in the script special-cases the handful of
dependencies whose install genuinely differs from "install a same-named
package" (`gh`'s apt path needs a keyring + repo added first; `alacritty`
needs `--cask` on brew; `oh-my-zsh`/`zsh-autosuggestions`/`tpm` are
`git clone`/curl-script installs with no package-manager involved at all;
`rustup` always uses its own curl script, never `brew install rust`, per
`~/README.md`'s explicit warning; `zoxide`'s official install script is used
on every platform since its apt-repo availability varies by Ubuntu release).
Everything else falls through to a default `<manager> install <name>`.

After running an install command (non-dry-run), the script re-runs that
dependency's `check_command`. If it now passes, the dependency is no longer
counted as missing; if not, it's reported as `install did not satisfy the
check` and still counts — this is what makes the exit code meaningful for
CI's "did the bootstrap actually work" question, not just "did we attempt
something."

### 3. Shell-startup hook

The hook logic (cache check, nag, the `depcheck` alias) lives in its own
file, `.my-scripts/deps/depcheck-hook.sh`, rather than being pasted directly
into `.zshrc` — the same reasoning that moved `tmux.conf`'s shared content
into `tmux-common.conf`: `.zshrc` itself isn't tracked identically between
`mac` and `linux`, so anything pasted straight into it has no drift
protection and depends on remembering to hand-port every future edit to
both branches. `depcheck-hook.sh` sits inside `.my-scripts/`, which
`.sync-manifest` already covers wholesale, so it's automatically checked on
every push with no manifest edit needed. `.zshrc` on each branch gets a
single line: `` [ -f ~/.my-scripts/deps/depcheck-hook.sh ] && source
~/.my-scripts/deps/depcheck-hook.sh ``.

The hook itself checks a cache file (`~/.cache/depcheck-last-run`, a Unix
timestamp) and only runs the real check-only pass when it's more than 24h
stale; on a stale run, if `check-deps.sh` exits non-zero, prints one line
(`` depcheck: missing dependencies detected -- run `depcheck` for details ``)
and nothing else. Never prompts, never blocks. The `depcheck` alias
(`check-deps.sh --fix`) is defined in the same file, for the manual,
interactive path.

### 4. Local Docker test harness

`.my-scripts/deps/docker/Dockerfile.ubuntu` (apt) and
`.my-scripts/deps/docker/Dockerfile.arch` (pacman) — minimal images with
just enough (`sudo`, `curl`, `git`, `wget`) to run `check-deps.sh --fix
--yes` as a real bootstrap. `.my-scripts/deps/test-local.sh` archives the
current branch's tracked tree (`git archive`, not a raw mount — `$HOME`
holds plenty of untracked content that has no business in a Docker build
context) into a temp directory, builds both images from it, and runs
`check-deps.sh --fix --yes` inside each, so the manifest/engine can be
iterated on locally in seconds. macOS has no equivalent (no macOS Docker
base image) — that leg only runs in real CI on `macos-latest`.

### 5. GitHub Action

`.github/workflows/deps-check.yml`, tracked identically on `mac` and
`linux`. Triggers: `schedule` (every two weeks) and `workflow_dispatch`
(manual trigger, for testing the workflow itself without waiting on the
schedule). Three jobs:
- `ubuntu-latest` running `check-deps.sh --fix --yes` directly (apt).
- `macos-latest` running `check-deps.sh --fix --yes` directly (brew).
- `archlinux-container`, running on `ubuntu-latest` inside the same
  `Dockerfile.arch` image the local harness uses (pacman), so local and CI
  stay identical rather than maintaining two separate Arch setups.

Each job checks out the repo fresh (a real bootstrap, not just a lint pass)
and fails the job if `check-deps.sh` exits non-zero.

### 6. Documentation

- `.my-scripts/deps/README.md` — the manifest format, how `install_cmd_for`
  overrides work and when to add one, how to run `check-deps.sh` and
  `test-local.sh` locally, how `deps-local.conf` and the `.sync-manifest`
  exclusion work. For future-you editing the manifest.
- `~/README.md` gets a new "Dependency checking" section (same style as the
  existing Setup subsections), short: what exists, that `depcheck` is the
  manual entry point, and a link to the doc above. The existing per-tool
  install bullets stay as-is — this section supplements them, it doesn't
  replace the narrative walkthrough.

## Rollout order

1. `deps.conf`, `deps-local.conf` (linux), `check-deps.sh`, and its test, TDD
   order, on `linux`.
2. Shell-startup hook + `depcheck` alias in `.zshrc`, on `linux`.
3. Docker harness (`Dockerfile.ubuntu`, `Dockerfile.arch`, `test-local.sh`),
   on `linux`.
4. `deps-check.yml`, plus the `!.my-scripts/deps/deps-local.conf` line in
   `.sync-manifest`, on `linux`.
5. Documentation (`.my-scripts/deps/README.md`, `~/README.md` section), on
   `linux`.
6. Run `tests/run-all.sh` and `test-local.sh`, confirm both green. Push
   `linux`.
7. Mirror every shared file onto `mac` (manifest, engine, docs, docker
   files, workflow, `.zshrc` hook), write `mac`'s own `deps-local.conf`
   (`aerospace`), push. This machine can't execute the mac-side run; note it
   as a follow-up to confirm on that machine, same as the branch-drift
   plan's precedent.

## Open questions

None outstanding — all decisions above were confirmed during brainstorming
(2026-09-03), including the auto-install-by-default choice, the cached
24h-nag shell hook, the full-bootstrap CI scope, the manifest-plus-overlay
split for platform-exclusive dependencies, and adding local Docker
verification before CI.
