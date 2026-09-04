# Dependency checking

Checks the command-line dependencies this dotfiles setup needs, and installs
the missing ones on a best-effort basis. The dependency list in `~/README.md`
is the prose version. The manifest in this directory is the executable one.

## Files

- `deps.conf` -- shared dependencies, tracked identically on the `mac` and
  `linux` branches. `.sync-manifest` covers `.scripts/` as a whole
  directory, so this file is checked for drift between the two branches.
  One line per dependency: `name|check_command|docs_url`.
- `deps-local.conf` -- dependencies that exist on one branch only. Same
  format. `.sync-manifest` excludes this file with the line
  `!.scripts/deps/deps-local.conf`, so each branch carries its own copy
  and the drift check does not flag the difference. On the `mac` branch this
  file holds `aerospace`. It also holds `oh-my-zsh` on a branch whose machine
  uses oh-my-zsh. This machine does not, so `oh-my-zsh` is absent here.
- `check-deps.sh` -- the engine. Reads `deps.conf`, then `deps-local.conf` if
  that file exists.
- `depcheck-hook.sh` -- sourced from `.zshrc`. Defines the `depcheck` alias
  and a startup check that runs at most once every 24 hours. The logic lives
  here rather than inside `.zshrc` so that `.sync-manifest` covers it. The
  platform-conditional content of `.zshrc` itself is not covered, and can
  drift between branches without anything noticing.
- `docker/Dockerfile.ubuntu`, `docker/Dockerfile.arch` -- minimal images for
  exercising a bootstrap from scratch. Used by `test-local.sh` and by
  `.github/workflows/deps-check.yml`.
- `test-local.sh` -- builds and runs both images against the current branch,
  for iterating without waiting on continuous integration.

## The manifest format

One dependency per line: `name|check_command|docs_url`.

`check_command` is any shell snippet that exits 0 when the dependency is
present. Most are `command -v <binary>`. A few dependencies are not binaries
on `PATH`, so they check for a directory or a file instead. `tpm` checks for
a cloned directory. `nvm` checks for a sourceable script.

### A check_command must never contain a pipe

`read_entries` in `check-deps.sh` splits each line with
`IFS='|' read -r name check docs`. A literal `|` anywhere in the check field
therefore ends the check early and pushes the remainder into `docs_url`. The
truncated check still runs, and it still returns an answer. The answer is
wrong, and nothing reports an error. This is the easiest way to break the
manifest, and the failure is silent.

For a check that needs an either-or, use one of these shapes instead:

- `test A -o B`
- `if <first check>; then true; else <second check>; fi`

A shell pipe, a `||`, and a `|&` are all forbidden for the same reason.

### Checks accept either platform's install shape

`deps.conf` is shared across branches, so a check in it must pass on every
platform that runs it:

- `alacritty` passes on a macOS `.app` bundle under `/Applications`, and on
  a binary found on `PATH`.
- `zsh-autosuggestions` passes on the Homebrew `share` path, and on the
  oh-my-zsh custom-plugin path.

`oh-my-zsh` itself is not shared, so no ordering between `oh-my-zsh` and
`zsh-autosuggestions` is guaranteed inside `deps.conf`.

## Adding a dependency

Add a line to `deps.conf`, or to `deps-local.conf` when the dependency
belongs to one branch only.

The default install command is `<package manager> install <name>`, which is
correct while the package name matches the `name` field. When the names
differ, or when the dependency does not come from a package manager at all,
add a case to `install_cmd_for()` in `check-deps.sh`.

Two install commands are aware of the package manager rather than the
dependency alone:

- `alacritty` installs as a normal package on `apt` and `pacman`, and is
  manual-only on Homebrew. Homebrew disabled its cask on 2026-09-01 because
  the app does not pass the macOS Gatekeeper check, and the release `.dmg` is
  no help: the app is adhoc-signed with no Team ID, `spctl -a` rejects it,
  and approving it is interactive by design. The Alacritty install guide
  documents only a source build, so macOS has no automated path.
- `aerospace` is macOS-only. It installs as a cask from a third-party tap,
  which the install command taps and trusts first, and is manual-only on
  every other package manager because no Linux build exists.
- `zsh-autosuggestions` installs from a Homebrew formula on Homebrew. On any
  other package manager it clones into the oh-my-zsh custom-plugin
  directory, and only when that directory already exists. Without oh-my-zsh
  the clone would land where nothing sources it, and the check would then
  report success for an install that never loads. The engine reports the
  dependency as manual-only instead.

Leave a case empty to declare that a dependency has no safe automated
install. `nvm` is the one such case today, because its own documentation
publishes only version-pinned install URLs. An empty case still gets checked
and reported. It never gets installed, and it never fails the exit code of
`--fix`.

## PATH

`check-deps.sh` prepends `~/.local/bin` and `~/.cargo/bin` to `PATH`.
`zoxide` installs into the first directory and `rustup` into the second, so
without this a `command -v` check fails on the line right after its own
install succeeded. An interactive shell usually exports both already, which
is what hides the problem on a machine already in use.

## Running it

```sh
~/.scripts/deps/check-deps.sh                  # check only
~/.scripts/deps/check-deps.sh --fix            # check, then prompt per install
~/.scripts/deps/check-deps.sh --fix --yes      # check, then install without prompting
~/.scripts/deps/check-deps.sh --fix --dry-run  # print what --fix would run
depcheck                                          # alias for --fix
~/.scripts/deps/test-local.sh                  # bootstrap fresh containers
```

`depcheck` is defined in `depcheck-hook.sh` as:

```sh
alias depcheck='~/.scripts/deps/check-deps.sh --fix'
```

An unknown argument exits 2.

### Exit codes

- Without `--fix`: non-zero when anything is missing. This is informational.
  The startup hook and a plain manual run both use it.
- With `--fix`: non-zero only when a dependency that had an automated install
  command still fails its check after the install ran. A dependency with no
  automated install path is reported and does not affect the exit code, since
  `--fix` had nothing to do differently. A declined prompt is treated the
  same way.
- With `--dry-run`: always 0.

## The startup hook

`depcheck-hook.sh` runs the check-only path at most once every 24 hours and
prints one line when anything is missing. It never installs, never prompts,
and never blocks startup. The throttle timestamp lives in
`~/.cache/depcheck-last-run`. An unwritable `~/.cache` costs the throttle and
nothing else, so the next shell checks again.

## Continuous integration

`.github/workflows/deps-check.yml` runs `check-deps.sh --fix --yes` as a real
bootstrap on the 1st and the 15th of each month, and on demand through
`workflow_dispatch`. It does not run on push, because every job installs
packages over the network, and an upstream outage would then fail unrelated
commits.

Three jobs cover the three package managers: `ubuntu-latest` for apt,
`macos-latest` for brew, and an Arch container built from
`docker/Dockerfile.arch` for pacman.

`docker/Dockerfile.ubuntu` pins `ubuntu:24.04` by digest.
`docker/Dockerfile.arch` is deliberately unpinned, because `archlinux:base`
is a rolling-release image and a pinned digest goes stale within weeks.
