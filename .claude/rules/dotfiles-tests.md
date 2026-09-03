---
paths:
  - ".my-scripts/**/*.sh"
  - ".claude/scripts/**/*.py"
  - ".claude/hooks/**/*.sh"
  - "tests/**"
---

# Dotfiles tests

These apply when editing tooling tracked in the dotfiles repo: shell scripts in
`.my-scripts/`, Python in `.claude/scripts/`, hooks in `.claude/hooks/`, and the
tests themselves.

## Run the suite before you push

Run `~/tests/run-all.sh` and confirm it passes before pushing a change to any
of the paths above. It takes about 25 seconds, and announces each suite as
`[n/total]` while it runs.

A pre-push hook at `~/tests/pre-push` (symlinked from `~/.cfg/hooks/pre-push`)
runs the suite too, gated to pushes whose commits touch tested code, and
blocks the push on failure. It runs the suite **in Docker**
(`~/tests/run-in-docker.sh`), not on this machine, so it needs a running
Docker daemon. There is no host fallback on purpose: the host suite spawns
tmux sessions on the real server and writes fixture repos under `$HOME`,
which made `tmux-update-window-names.test.sh` flaky enough to block a push
whose code was fine.

The hook tests the ref being pushed, not the working tree. It sets
`$DOTFILES_TEST_REF` so the container archives that ref, which matters
because the pushed ref is not always the checked-out branch: pushing `linux`
from a worktree while `$HOME` sits on `mac` is the normal way this repo ships
a linux change. When that variable names a ref other than the checked-out
branch, the working-tree overlay is skipped and the container tests the ref
exactly as it will land on the remote.

Run `~/tests/run-in-docker.sh` yourself first anyway. Discovering a failure
from a blocked push costs a round trip.

The same hook runs `~/tests/check-branch-drift.sh mac linux` when the push
includes the `mac` or `linux` branch, and blocks the push if a path listed in
`.sync-manifest` differs between the two. This is a local pre-flight: the
`branch-drift` GitHub Action re-checks `origin/mac` against `origin/linux`
after the push and is the authoritative gate.

A separate pre-commit hook at `~/tests/pre-commit` (symlinked from
`~/.cfg/hooks/pre-commit`) runs a leak guard (`~/tests/leak-check.sh`) on every
commit, regardless of which paths are staged. This repo is public, so the
guard refuses staged content that looks like a credential or like internal
project detail. Its project-term patterns load from an untracked local file,
so the guard names nothing specific in this repo. The guard stays at
pre-commit, not pre-push, so a leak never even lands in a local commit.

If the guard blocks a commit, genericize the wording or move the specifics to a
machine-local file the tracked file reads at runtime. For a verified false
positive, use `SKIP_LEAK_CHECK=1 config commit ...`.

Never pass `--no-verify` to get around any of these gates. It skips every hook
at that stage, so bypassing one gate would silently take the others with it.

## Where tests live

Integration tests live in `~/tests/` as `<script-name>.test.sh`. They source
`~/tests/lib.sh` for assertions, fixtures, and tmux session management.

Unit tests live next to the code they cover, named `test_*.py`, and run under
stdlib `unittest`. Nothing in this repo installs pytest.

## Writing a new test

Use the helpers in `lib.sh` rather than calling tmux directly:

- `new_test_session` creates a detached session named after the test file and
  the pid, registers it for teardown, and neutralises the globally installed
  window-naming hooks so they cannot race the assertions.
- `target_window` makes a window current. tmux resolves an unqualified target
  to the current window of the current session and ignores `$TMUX_PANE`, so a
  script that acts on "the current pane" needs this first.
- `in_pane` runs a command with `$TMUX` cleared. This is a safety boundary, not
  a convenience: without it a script that splits or kills "the current pane"
  operates on the live pane you are sitting in.
- `in_session` is for scripts that refuse to run when `$TMUX` is empty. It
  points `$TMUX` at a test session instead of clearing it.

Two failure modes are worth knowing about, because both have bitten this repo:

- A test that creates tmux sessions must clean them up on signals, not only on
  exit. `lib.sh` traps `INT`, `TERM`, and `HUP` alongside `EXIT` for this
  reason. Enough orphaned sessions will bog the tmux server down.
- A caller that exports a git environment, which a pre-commit or pre-push hook
  does, would otherwise redirect every fixture `git init` and `git commit` at
  the dotfiles repo. `lib.sh` unsets those variables, and the pre-push hook
  clears them too before it runs `run-all.sh`.

## The fetch refspec on another machine

A bare repo cloned with `--bare` has no `remote.origin.fetch`, so
`config fetch origin` updates `FETCH_HEAD` and leaves `refs/remotes/origin/*`
frozen at whatever they were when the remote was added. Nothing warns about
this.

`check-branch-drift.sh` defaults to comparing `origin/mac` against
`origin/linux`, so on a machine missing the refspec it reads stale refs and
reports drift that does not exist (or, worse, misses drift that does). Set it
once per machine:

    config config --add remote.origin.fetch '+refs/heads/*:refs/remotes/origin/*'
    config fetch origin

Confirm with `config rev-list --left-right --count origin/mac...mac`. A local
branch that is level with the remote reports `0 0`. A nonzero left count on a
branch you just pushed means the tracking refs are stale, not that the push
failed.

## Installing the hooks on another machine

Both hooks live in the work tree so they travel with the repo. The symlinks do
not, so create them once per machine:

    ln -sf "$HOME/tests/pre-commit" "$HOME/.cfg/hooks/pre-commit"
    ln -sf "$HOME/tests/pre-push" "$HOME/.cfg/hooks/pre-push"

`tests/githooks-installed.test.sh` asserts both symlinks exist, are
executable, and point at the tracked scripts, so a machine that skipped this
step fails the suite instead of pushing with no gates. It also asserts
`core.hooksPath` is unset, because setting it replaces `.cfg/hooks` wholesale
and would stop git running these hooks at all. The suite skips the whole file
where there is no `.cfg` repository, which is the case inside the test image.
