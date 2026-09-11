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

Run `cd ~/crates && cargo test --locked` and confirm it passes before pushing
a change to any of the paths above. `config test` is the same run with a
friendlier front end, and `config test <filter>` narrows it: the argument is
cargo's own test-name filter, not a suite file name.

There is **one harness**. The shell harness -- a `lib.sh` library, a
`run-all.sh` runner, and one `*.test.sh` file per suite, all under `tests/` --
was deleted on 2026-09-11, after every suite that tested something other than
the harness itself was ported to Rust. Anything that still tells you to run
`run-all.sh` is stale.

Budget about **six minutes** on the host. Measured 2026-09-10 on an M1 Max
against the shell harness: 374 seconds, of which the `config` suite was 105
seconds because it calls `config build` and compiles all six crates. The port
did not change that cost. An earlier version of this file said "about 25
seconds"; the claim was true when written and nothing updated it as suites
accumulated. Re-measure before quoting a number, and if you need a fast signal
for a narrow change, filter to the tests you touched.

A pre-push hook at `~/tests/pre-push` (symlinked from `~/.cfg/hooks/pre-push`)
runs two gates and blocks the push on either.

- `~/tests/rust-checks.sh` runs on the **host**, against a `git archive`
  snapshot of the ref being pushed, not the working tree. It runs
  `cargo test --locked --quiet` and `cargo clippy` there.
- `~/tests/run-in-docker.sh` runs the same cargo suite **in Docker**, gated to
  pushes whose commits touch tested code, so it needs a running Docker daemon.
  There is no host fallback on purpose: on the host the tmux suites spawn
  sessions on the real server, which made the window-naming suite flaky enough
  to block a push whose code was fine.

The hook tests the ref being pushed, not the working tree. It sets
`$DOTFILES_TEST_REF` so the container archives that ref. When that variable
names a ref other than the checked-out branch, the working-tree overlay is
skipped and the container tests the ref exactly as it will land on the remote.

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

### `config build` can leave the installed binary SIGKILLed, and it hides it

**This cost three debugging cycles in one day (2026-09-10).** Read this before
diagnosing any "the binary prints nothing" symptom.

`config build` installs over the live `~/.local/bin/config-cli`. On macOS an
in-place write invalidates the running image's code signature, so the kernel
SIGKILLs the process. The binary then exits **137** printing nothing, and it
keeps doing so on every later run until it is re-signed.

Three things make the defect expensive to recognize:

- `codesign -v` still reports the file as **valid**, so the symptom does not
  read as a signing problem at all.
- Any invocation through a pipe (`config-cli --version | head`) reports the
  **pipeline's** exit status, not the 137, so the binary looks like it merely
  prints nothing.
- The failure travels. `config build` before a push left the binary killed,
  and `tests/rust-checks.sh` then reported `config_dispatcher` failing 7 of 34
  inside the archived snapshot with exit code `-1` and empty output, while the
  same suite passed 34 of 34 on the host. The push was refused.

The diagnosis, and it is the only reliable one:

    ~/.local/bin/config-cli help > /tmp/h.txt 2>&1; echo $?

**Redirect to a file.** A pipe hides the signal. An exit of 137 means the
binary was killed, not that the command failed.

The fix is `codesign -f -s - ~/.local/bin/config-cli`. After that the suite
passed 34 of 34 in the snapshot too.

So: **if `config-cli help` prints nothing, or a test fails with an empty
`left: ""`, check for 137 before concluding anything about the code.** The
proper repair is a rename-into-place in `config build` (write to a temp path
in the same directory, then `rename(2)`), which is the standard way to replace
a running executable. That is not done yet.

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

## Revert every sabotage, and verify the revert

Sabotage is how this repo proves an assertion is load-bearing, so it runs
constantly. Twice on 2026-09-10 a sabotage was left in the tree:

- Two `// staleness probe` comments appended to
  `crates/config-cli/src/doctor.rs`, production code, still modified when the
  agent moved on. Harmless in content, and it would have shipped.
- A `find -L` flag dropped from `config-install-hooks`, which is a security
  control. That one was heading for a commit.

The discipline:

- **Back up before, restore after, and `touch` the file.** A restore-by-move
  carries the backup's older mtime, so cargo runs the stale binary and the
  next result is a lie in either direction.
- **Verify the revert**, with `config diff --stat -- <path>` or by reading the
  lines back. "I restored it" is not evidence.
- **Prefer sabotaging a copy.** Point the test at a fixture rather than the
  real file where the test's design allows it.
- **Never leave a sabotage staged.** Check `config diff --cached --name-only`
  before committing; an unrelated production file in that list means stop.

## A converted test must not change the thing it tests

Caught 2026-09-10, mid-conversion. An agent porting `config.test.sh` began
editing `.scripts/config/config-install-hooks`, dropping the `-L` from

    find -L "$dir" -maxdepth 0 \( -perm -g+w -o -perm -o+w \)

in `check_dir`. That is the whole enforcement of the install-hooks trust
boundary. `-L` was added deliberately in `654a9cac`, "Follow symlinks in the
install-hooks trust check", and the comment directly above it explains why:
without it, macOS "lets a world-writable directory through whenever it is
reached by a link", and `~/.local/bin` and `~/tests` both plausibly arrive as
symlinks on a synced or restored home.

So the edit would have silently reverted a security fix to make a test pass.
It never committed, because the tree was being watched, but nothing
structural stopped it.

Two rules follow:

- **A conversion changes the test, never the subject.** If a ported test
  fails, the finding is either a defect in the port or a real defect in the
  subject. Both get reported. Neither gets fixed by editing the subject to
  agree with the test.
- **When scoping an agent to files, say which files are forbidden and why**,
  not only which are allowed. This agent's brief listed allowed paths; it
  still reached for a production script when a test would not pass.

The general shape: a test that can be made to pass by weakening what it
guards is worse than no test, because it launders the weakening as a
green run.

## Stage path-scoped whenever anything else might be running

`config add <file>` stages that file. `config commit` then commits
**everything staged**, including work another process put there.

Both halves bit this repo on 2026-09-10. An agent's unscoped commit swept a
sibling's staged files; it recovered with `config reset --soft`. Later a
commit whose message describes only a new Rust test also carried a 264-line
suite deletion that a concurrent agent had staged, and that commit's message
does not mention it. The tree was correct both times; the history is not.

So, whenever another agent, a hook, or a background task might touch the
index:

- Stage your own paths, then **verify** with `config diff --cached --name-only`
  before committing. Read it; do not assume.
- Prefer `config commit -o <paths>` so the commit cannot take more than you
  named.
- Never run `config status -uall` in this situation: it walks all of `$HOME`,
  held the index for over two minutes here, and caused an `index.lock`
  collision.

## Prove every gate can fail, with a non-zero exit

**The single most repeated defect in this repo is a gate that reports PASS
while measuring nothing.** Four instances were found on 2026-09-10 alone, in
one day, which makes it a pattern rather than an accident.

Three of the four came from one mechanism in the deleted shell harness, and
the mechanism is worth stating even though it is gone, because the general
rule is what survives it. `lib.sh`'s `finish` was what returned a suite's exit
status. A suite that printed `FAIL:` and then fell off the end of the file
exited **0**, and the runner recorded a pass.

- `nvim-lua-format.test.sh` had no `finish` call at all: its last statement
  was an assertion inside an `if`. An unformatted Lua file made it print
  `FAIL:` and exit 0, so the stylua gate had been open since the suite was
  written. The Rust port exits 101 on the same sabotage.
- `deps-manifest.test.sh` was worse, because it looked correct. It **did**
  call `finish`, and then carried four more assertions below it: the whole
  piped-output block. Measured before deletion, sabotaging the first of them
  printed `FAIL: the piped run produced output (exited 1)` and the suite
  **exited 0**. A reviewer scanning for "does this file call `finish`" would
  have passed it. The check was never whether `finish` is called but whether
  it is **last**.
- `skip-reporting.test.sh` called `finish` at line 229 with five assertions
  below it. The suite written to guarantee a silent skip cannot ship had five
  silent assertions of its own.

Cargo's harness cannot reproduce that exact shape: a `#[test]` that panics
fails, and there is no trailing statement to strand. The general rule it
produced is not about `finish` and still binds:

**Before trusting any gate, break its subject once and confirm a non-zero
exit code.** Not a `FAIL:` line on stdout, not red text, not an error message
in a log. The exit status, read directly. Every gate in this repo is consumed
by something that branches on that status and by nothing else.

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

There is one harness. Integration tests live in `crates/<crate>/tests/` as
Rust integration targets, one file per former shell suite, and each file opens
with a `//!` module doc naming what it was converted from and how many
assertions it carried. Shared helpers live in the `dotfiles-test-support`
crate.

Unit tests live next to the code they cover. Rust unit tests go in a
`#[cfg(test)] mod tests` in the same file. Python unit tests are named
`test_*.py` and run under stdlib `unittest`; nothing in this repo installs
pytest, and `test-suite.yml` discovers them rather than listing them.

## Writing a new test

- **Locate tracked files with `dotfiles_test_support::repo::root()`**, never
  `CARGO_MANIFEST_DIR` alone. Cargo runs an integration test from the crate
  directory, so a path relative to the cwd resolves against
  `crates/<crate>`, not the repo root.
- **`git ls-tree` and `git ls-files` from a test need a rooted pathspec
  (`:/deps`) and `--full-name`.** A bare relative pathspec resolves against
  the crate directory and matches nothing. That made one whole discovery
  branch dead: it returned zero paths and ten tracked scripts were never
  linted.
- **Positive controls are required.** Any assertion expecting an empty result
  must first prove its pipeline produced something. An assertion over an empty
  set passes for the wrong reason.
- **Strip comments and join backslash continuations before matching file
  contents.** Both shapes made assertions vacuous during the port: a grep
  satisfied by a comment, and a `^RUN (apt-get|pacman)` that could not see a
  package list on a continuation line.
- **Never use `#[ignore]` for a runtime skip.** Use
  `dotfiles_test_support::skip(reason)` and return.
- **Never call `skip()` to prove skipping works.** It writes to the ambient
  `DOTFILES_SKIP_LOG` the gate sets, so such a test records a phantom skip.
  That bit the skip mechanism's own first test.

### Skipping an assertion

Call `dotfiles_test_support::skip("<reason>")` and return when a check cannot
run in the current environment. Do not print the skip yourself and do not
silently return.

A skip is green on purpose: it says a check could not execute here, not that
it would have failed. The container harness builds its tree with `git archive`
and so carries no repository, which is a correct reason for a
commit-inspection assertion to stand down.

What a skip must not be is **free**. The incident that produced this rule: a
commit-inspection block used to print one line into a suite's captured output
and nothing else, so the tally counted only passes and failures, and the
pre-push Docker gate printed `PASS` over an assertion that never ran. A stale
committed-script count shipped, and both CI platforms caught it instead,
because `actions/checkout` gives a runner the repository the container lacks.

The rule the pre-push hook already states one level up: **a gate that says
nothing when it skips is indistinguishable from a gate that is not installed.**

`skip` appends a line to `$DOTFILES_SKIP_LOG`, and `tests/rust-checks.sh`
reads that log and prints the count **with each reason**. The count is not
swallowed by `--quiet`, which is what the pre-push hook shows.
`crates/dotfiles-test-support/tests/skip_log.rs` holds both halves in place:
it runs the gate's own reporting block, so a change to its wording or its
`sed` turns that test red. Proven load-bearing by sabotage: dropping the
reasons turns that test alone red.

A run with no skips says nothing about them, deliberately. A trailing "0
skipped" on every run is noise, and noise is what a reader learns to scan past.
`skip_log.rs` asserts that silence too.

One count caveat, and it is honest rather than a defect: cargo builds an
integration target per lib and bin target of the crate under test, so one
skipping test can appear twice. Read the reasons, not the number.

A test that starts its own tmux server with `-L <name>` must set
`TMUX_TMPDIR` to a directory it owns and removes, because of three tmux
behaviours that each cost a debugging cycle on 2026-09-09:

- tmux 3.4 does not unlink its socket file on `kill-server`, even on a clean
  exit. Without a private directory, every run left one dead socket per
  server in the shared `/tmp/tmux-<uid>/`; 1435 were found there.
- A Unix socket path is capped at 104 bytes on macOS. The default temporary
  directory on macOS lives under the long `/var/folders/.../T/` prefix, which
  is where `tempfile` puts a scratch directory by default, so a socket there
  came to 111 bytes and tmux failed with "File name too long". Put the
  directory directly under `/tmp`:
  `tempfile::Builder::new().tempdir_in("/tmp")`.
- When `TMUX_TMPDIR` names a directory whose parent does not exist, tmux
  falls back to the shared directory and exits 0 with no message. Create the
  directory before the first tmux call, and assert at the end of the test
  that the shared directory does not hold your socket.
  `crates/tmux-tools/tests/conf_split.rs` and
  `crates/tmux-tools/tests/plugin_path.rs` are the shape to copy.

Route the teardown's `kill-server` through the same wrapper that sets
`TMUX_TMPDIR`, or it aims at a path that no longer exists, the real server
survives, and the scratch directory's removal takes its socket out from under
a running process.

**And assert it, because the obvious guard does not catch it.** Measured
2026-09-10: a teardown missing `TMUX_TMPDIR` fails with "error connecting",
leaves the real server running (confirmed by pid), and puts **nothing** in
the shared directory. So the shared-directory assertion above, which catches
the other two traps, is blind to this one. The check that works is
`has-session` through the wrapper *after* the kill: the server must be gone.
`crates/tmux-tools/tests/support/mod.rs` does this.

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

- A test that creates tmux sessions must clean them up on an abort, not only
  on a normal return. The deleted shell harness trapped `INT`, `TERM` and
  `HUP` alongside `EXIT` for this reason. In Rust the equivalent is a guard
  whose `Drop` kills the server, because `Drop` runs while a panic unwinds and
  an early `?` return. Enough orphaned sessions will bog the tmux server down.
- A caller that exports a git environment, which a pre-commit or pre-push hook
  does, would otherwise redirect every fixture `git init` and `git commit` at
  the dotfiles repo. Both callers clear it: `tests/pre-push` runs
  `env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE -u GIT_PREFIX` around its
  container call, and `tests/rust-checks.sh` does the same around cargo. A
  fixture that took `~/.cfg/index.lock` once blocked every `config` command
  until the process was killed.

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
