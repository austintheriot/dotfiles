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

## `config build` leaves a stale binary when a crate becomes a library

**What happens.** A workspace member that had a `src/main.rs` and loses it
becomes library-only. `config build` handles the build correctly: it compiles
the member and prints `config-build: <name> is a library, nothing to install`.
It does not remove the binary a previous run installed, so
`~/.local/bin/<name>` survives with the last compiled copy and stays on PATH.

**Why it costs time.** The stale binary answers `--stamp` with whatever it was
built from, so `config doctor` and the pre-push stamp gate both keep it out of
their comparison (the crate has no `src/main.rs`, so both sides drop it) while
`command -v <name>` still finds it. Nothing reports the leftover. A reader
checking whether a migration finished sees the binary and concludes it did
not, or worse, a script that resolves the old name by mistake keeps working
locally and fails on any machine that never had it installed.

**Why the repository can fix this.** `config-build:64` already knows the
member is a library, at the exact moment it decides not to install. It has
`$BIN_DIR` and `$member` in hand and could remove a leftover in the same
branch.

**The fix.** In the library branch of `.scripts/config/config-build`, remove
`$BIN_DIR/$member` when it exists, and say so on stdout the same way the
install path reports where it installed. Silence there is the same defect as
silence anywhere else in that script.
