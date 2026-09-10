---
paths:
  - ".scripts/**/*.sh"
  - ".claude/scripts/**/*.py"
  - ".claude/hooks/**/*.sh"
  - "tests/**"
---

# Dotfiles tests

These apply when editing tooling tracked in the dotfiles repo: shell scripts in
`.scripts/`, Python in `.claude/scripts/`, hooks in `.claude/hooks/`, and the
tests themselves.

## Run the suite before you push

Run `~/tests/run-all.sh` and confirm it passes before pushing a change to any
of the paths above. It announces each suite as `[n/total]` while it runs.

Budget about **six minutes** on the host. Measured 2026-09-10 on an M1 Max:
374 seconds for `-q`, of which `config.test.sh` is 105 seconds because it
calls `config-build` and compiles all six crates. This file said "about 25
seconds" until that measurement; the claim was true when written and nothing
updated it as suites accumulated. If you need a fast signal for a narrow
change, run the specific suite rather than the whole set.

A pre-push hook at `~/tests/pre-push` (symlinked from `~/.cfg/hooks/pre-push`)
runs the suite too, gated to pushes whose commits touch tested code, and
blocks the push on failure. It runs the suite **in Docker**
(`~/tests/run-in-docker.sh`), not on this machine, so it needs a running
Docker daemon. There is no host fallback on purpose: the host suite spawns
tmux sessions on the real server and writes fixture repos under `$HOME`,
which made `tmux-update-window-names.test.sh` flaky enough to block a push
whose code was fine.

The hook tests the ref being pushed, not the working tree. It sets
`$DOTFILES_TEST_REF` so the container archives that ref. When that variable
names a ref other than the checked-out branch, the working-tree overlay is
skipped and the container tests the ref exactly as it will land on the
remote.

Run `~/tests/run-in-docker.sh` yourself first anyway. Discovering a failure
from a blocked push costs a round trip.

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

## Rebuild after editing a crate

Editing a crate under `crates/` does not change what the installed binary
does. Run `config build` after any crate edit; it builds every workspace
member and re-stamps each one.

`config doctor` reports which installed binaries no longer match their source
and names the fix. It is silent when everything is current, so it is safe to
run habitually.

There is deliberately no runtime freshness check. Two were measured and
rejected: a binary that verifies its own stamp on startup costs about 124ms
per invocation, on a path that runs before every shell prompt, and a rebuild
triggered from a prompt hook serializes every pane behind cargo's build lock
(a no-op release build measures 0.7 to 1.5 seconds). The guarantee is at
pre-push, which refuses a push when any binary is stale for the ref being
pushed, and `config doctor` is how you ask before then.

## Plan checkboxes are never ticked here, so an open box means nothing

`docs/superpowers/plans/` holds implementation plans whose steps are
`- [ ]` checkboxes. **Across all 13 plans, 464 boxes are open and 0 are
ticked**, including in plans whose work demonstrably shipped months ago.
Nobody in this repo has ever ticked one.

So an open box is not evidence of pending work, and a plan full of them is
not a backlog. Reading them that way has already caused one wrong
recommendation: on 2026-09-10 the `rust-gate-and-strict-lints` plan's 36
open boxes were read as a ready-to-execute task, when every one of them had
shipped (`tests/rust-checks.sh` runs `cargo test` and
`cargo clippy -D warnings`, `tests/pre-push` runs them on the host,
`test-suite.yml` runs clippy, and `crates/Cargo.toml` carries the workspace
lint policy).

**Before executing any plan in that directory, verify its deliverables
against the tree.** Read the files it says to create and the functions it
says to add, and check whether they are already there. Each plan now carries
a dated status banner recording that verification; trust the banner over the
boxes, and re-verify if the banner is old.

The durable record of intent is the **spec** in `docs/superpowers/specs/`,
not the plan. A plan is scaffolding for one pass of work.

## A suite with no `finish` call exits 0 no matter what

`finish` is what returns the exit status. A suite that prints `FAIL:` and
then falls off the end of the file exits **0**, and `run-all.sh` records a
pass.

Found 2026-09-10 converting `nvim-lua-format.test.sh`, which had no `finish`
call at all: its last statement was an assertion inside an `if`. An
unformatted Lua file made it print `FAIL:` and exit 0, so the stylua gate had
been open since the suite was written. The Rust port exits 101 on the same
sabotage.

Two related shapes found the same day, both in suites that looked fine:

- **`nvim-version-floor.test.sh`** asserted its live simulation's output
  contained `0.10`. With the version guard disabled so startup fell through,
  it still passed, because the run reached lazy.nvim and printed
  `markdown-preview.nvim v0.0.10`. The guard's own message appeared zero
  times. Assert the guard's message, and that what the guard prevents did not
  happen.
- **`profile-path.test.sh`** checked for bashisms with `sh -n` and `dash -n`.
  Neither rejects `[[ ]]`: dash parses `[[` as a command word and the syntax
  check passes. A parse check is not an execution check.

The rule: **every suite ends with `finish`, and every gate must be shown to
fail.** Before trusting a new suite, break its subject once and confirm a
non-zero exit, not just a `FAIL:` line on stdout.

## Sabotage the measurement, not only the subject

The standard sabotage check is "break the thing this test asserts about and
confirm it goes red". That is necessary and it is not sufficient, because it
cannot detect a test whose *measurement* is swamped.

Found 2026-09-10 converting `zshrc-startup-budget`. The test times an
interactive zsh against a threshold. Run with a tty inherited, bare
`zsh -f -i` spends about **405ms** on terminal setup, against a real signal of
about 154ms and a shell-suite baseline of 7ms. So the measurement was almost
entirely noise, and a threshold deliberately shrunk to 1ms **still passed**.
Breaking the subject would never have revealed that: the subject was fine.

The fix is `Stdio::null()` on all three streams, stdin especially.

The rule: for any test that compares a measurement to a threshold, sabotage
the **threshold** as well as the subject. Set it absurdly tight and confirm
the test fails. If it passes, the measurement is not measuring what the test
claims, and every future run of that assertion is theatre.

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

### Skipping an assertion

Call `skip '<reason>'` when a check cannot run in the current environment.
Do not `printf` the skip yourself and do not silently `return`.

A skip is green on purpose: it says a check could not execute here, not that
it would have failed. The container harness builds its tree with
`git archive` and so carries no repository, which is a correct reason for the
commit-inspection block in `scripts-dir-name.test.sh` to stand down.

What a skip must not be is free. That block used to print one line into a
suite's captured output and nothing else, so `finish` counted only passes and
failures and the pre-push Docker gate printed `PASS` over an assertion that
never ran. A stale committed-script count shipped, and both CI platforms
caught it instead, because `actions/checkout` gives a runner the repository
the container lacks.

`skip` counts the skip so `finish` reports it, and `run-all.sh` carries the
count up to the per-suite verdict line and the run summary. Both survive
`-q`, which is what the pre-push hook shows. A run with no skips says nothing
about them: a trailing "0 skipped" everywhere is noise, and noise is what a
reader learns to scan past.

The rule is the one the pre-push hook already states one level up: a gate that
says nothing when it skips is indistinguishable from a gate that is not
installed. `tests/skip-reporting.test.sh` holds this behavior in place.

A test that starts its own tmux server with `-L <name>` must set
`TMUX_TMPDIR` to a directory it owns and removes, because of three tmux
behaviours that each cost a debugging cycle on 2026-09-09:

- tmux 3.4 does not unlink its socket file on `kill-server`, even on a clean
  exit. Without a private directory, every run left one dead socket per
  server in the shared `/tmp/tmux-<uid>/`; 1435 were found there.
- A Unix socket path is capped at 104 bytes on macOS. `$FIXTURES` lives
  under the long `/var/folders/.../T/` prefix, so a socket there came to
  111 bytes and tmux failed with "File name too long". Make the directory
  directly under `/tmp` (`mktemp -d /tmp/tmux-test-XXXXXX`, or
  `tempfile::Builder::new().tempdir_in("/tmp")` in Rust).
- When `TMUX_TMPDIR` names a directory whose parent does not exist, tmux
  falls back to the shared directory and exits 0 with no message. Create the
  directory before the first tmux call, and assert at the end of the suite
  that the shared directory does not hold your socket. `tmux-conf-split`
  and `tmux-plugin-path` are the shape to copy.

Route the EXIT trap's `kill-server` through the same wrapper that sets
`TMUX_TMPDIR`, or it aims at a path that no longer exists, the real server
survives, and `rm -rf` removes its socket from under a running process.

### A headless nvim test cannot assert that mason installed anything

`mason-lspconfig` gates `ensure_installed` on an attached user interface.
Verified verbatim at `mason-lspconfig/lua/mason-lspconfig/init.lua:31`, at
the commit this repo pins (`63a3c6a8`):

    if not platform.is_headless and #settings.current.ensure_installed > 0 then

and `is_headless` is `#vim.api.nvim_list_uis() == 0` in
`mason-core/platform.lua:38`. So a test that runs `nvim --headless` and then
asserts the servers are installed is asserting THE GATE, not the install.
Nothing was attempted. That assertion passes on a machine where mason is
entirely broken, which makes it worse than no test: it reports a guarantee
it never checked.

A headless test has to drive the install directly instead. Two ways that
work, both in use here:

- `MasonToolsInstallSync`, which `crates/config-cli/tests/nvim_config_load.rs`
  runs in a scratch XDG home. It is synchronous, so the test can assert
  afterwards without polling.
- `:MasonInstall <pkg>` or the `pkg:install` API for one package.

`crates/config-cli/tests/nvim_mason_runtimes.rs` asserts the gate still exists at the
pinned commit, and skips where the plugin tree is absent, which is the case
in the container. The assertion is deliberately about the gate rather than
about mason's behaviour: when a bump removes the gate, a headless install
test becomes possible and this section's advice becomes wrong, so it should
fail loudly rather than rot.

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
this, and a stale tracking ref makes `config rev-list --left-right --count
origin/main...main` report a wrong count. Set the refspec once per machine:

    config config --add remote.origin.fetch '+refs/heads/*:refs/remotes/origin/*'
    config fetch origin

Confirm with `config rev-list --left-right --count origin/main...main`. A
local branch that is level with the remote reports `0 0`. A nonzero left
count on a branch you just pushed means the tracking ref is stale, not that
the push failed.

## Installing the hooks on another machine

Both hooks live in the work tree so they travel with the repo. The symlinks do
not, so create them once per machine:

    ~/.scripts/config/config install-hooks

This also links the dispatcher itself into `~/.local/bin`, so `config` works
as a plain command once the tracked files are checked out.

`crates/config-cli/tests/githooks_installed.rs` asserts both symlinks exist, are
executable, and point at the tracked scripts, so a machine that skipped this
step fails the suite instead of pushing with no gates. It also asserts
`core.hooksPath` is unset, because setting it replaces `.cfg/hooks` wholesale
and would stop git running these hooks at all. The suite skips the whole file
where there is no `.cfg` repository, which is the case inside the test image.
