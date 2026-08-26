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

## Run the suite before you commit

Run `~/tests/run-all.sh` and confirm it passes before committing a change to any
of the paths above. It takes about 22 seconds.

A pre-commit hook at `~/tests/pre-commit` (symlinked from `~/.cfg/hooks/pre-commit`)
runs the same suite and blocks the commit on failure. Run the suite yourself
first anyway. Discovering a failure from a blocked commit costs a round trip,
and the hook's output is quieter than the suite's.

Never pass `--no-verify` to get around the hook.

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
- A caller that exports a git environment, which a pre-commit hook does, would
  otherwise redirect every fixture `git init` and `git commit` at the dotfiles
  repo. `lib.sh` unsets those variables, and the hook clears them too.

## Installing the hook on another machine

The hook lives in the work tree so it travels with the repo. The symlink does
not, so create it once per machine:

    ln -sf "$HOME/tests/pre-commit" "$HOME/.cfg/hooks/pre-commit"
