# Papercuts

Recurring repository friction worth fixing. Each entry names what went wrong,
why it costs time, and what would fix it.

## `cargo test` from a shell with `GIT_DIR` exported hijacks the real repo

**Status:** open

**What happens.** `config-manifest`'s tests build throwaway git repositories
as fixtures. With `GIT_DIR=$HOME/.cfg` and `GIT_WORK_TREE=$HOME` exported in
the calling shell, a fixture's `git add .` operates on the **real dotfiles
repo** instead of the fixture. Observed 2026-09-07: a fixture at
`/var/folders/.../T/.tmplD2FsJ` ran `git add .` against `~/.cfg`, took
`~/.cfg/index.lock`, and blocked every subsequent `config` command with
"Another git process seems to be running in this repository" until the
process was killed manually.

**Why it costs time.** The error names a lock, not a cause, so the obvious
next move is to delete the lock, which does not fix it: a live fixture
process immediately retakes it. Diagnosing requires `ps aux | grep git` and
noticing the working directory is a temp path. `~/README.md` and CLAUDE.md
both warn that `config status -uall` and `config stash` can leave the lock
held, which sends the reader down the wrong path entirely.

**Why the repository can fix this.** `tests/pre-push:228-232` already
identifies this exact hazard and guards its Docker call with
`env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE -u GIT_PREFIX`. Nothing
guards a bare `cargo test`, and agents and humans both export those
variables to make `git` usable against the bare repo.

**The fix.** Either make the fixture helper in `config-manifest`'s tests
clear git's environment for the child process it spawns, so the tests are
hermetic no matter who calls them, or document the requirement at the top of
`crates/config-manifest/src/git.rs`'s test module. The first is better: it
makes the property structural rather than a convention every caller must
remember. `tests/rust-checks.sh` (planned in
`docs/superpowers/plans/2026-09-07-rust-gate-and-strict-lints.md`) carries
the `env -u` guard for the gate's own invocation, but that does not protect
a developer running `cargo test` by hand.
