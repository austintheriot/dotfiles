# Dependency checking

Checks the command-line dependencies this dotfiles setup needs, and installs
the missing ones on a best-effort basis. The dependency list in `~/README.md`
is the prose version. The manifest in this directory is the executable one.

## Files

- `deps.conf` -- dependencies shared by every machine, regardless of
  platform. One line per dependency: `name|check_command|docs_url`.
- `deps-ci.conf` -- dependencies of the test suite itself, not of the
  working environment: `python3`, `pyyaml`, `dash`. Never selected by
  platform detection, so a `depcheck` on a developer machine never asks for
  them. `.github/workflows/test-suite.yml` reads it by setting `DEPS_CONF`.
- `deps-mac.conf` / `deps-linux.conf` -- dependencies that belong to one
  platform only. Same format. Both files ship together; what differs per
  machine is only which one gets read. `deps-mac.conf` holds `aerospace`;
  `deps-linux.conf` holds `oh-my-zsh` and `xclip`.
- `config deps` -- the engine, a Rust binary built from `crates/`. Reads
  `deps.conf`, then whichever of `deps-mac.conf` and `deps-linux.conf`
  matches this machine, if that file exists. The platform is detected the
  same way `~/.scripts/platform.sh` detects it, and the `DEPS_LOCAL_CONF`
  environment variable overrides the choice.
- `depcheck-hook.sh` -- sourced from `.zshrc`. Defines the `depcheck` alias
  and a startup check that runs at most once every 24 hours.
- `docker/Dockerfile.ubuntu`, `docker/Dockerfile.arch` -- minimal images for
  exercising a bootstrap from scratch. Used by `test-local.sh` and by
  `.github/workflows/deps-check.yml`. Neither carries a Rust toolchain, so
  the engine arrives through a `/seed` mount that
  `docker/deps-image-entrypoint.sh` reads. Installing a toolchain into
  these images would pre-satisfy `rustup`, which is itself a manifest
  entry, so the run would stop exercising the dependency it exists to test.
- `test-local.sh` -- builds and runs both images against the current branch,
  for iterating without waiting on continuous integration.

## The manifest format

One dependency per line: `name|check_command|docs_url`.

`check_command` is any shell snippet that exits 0 when the dependency is
present. Most are `command -v <binary>`. A few dependencies are not binaries
on `PATH`, so they check for a directory or a file instead. `tpm` checks for
a cloned directory. `nvm` checks for a sourceable script.

`node` checks both `PATH` and nvm's version directory. A non-interactive
shell does not run `.zshrc`, so an nvm-managed node is not on its `PATH`, and
a `command -v node` alone would report a machine that has node as missing it.
The entry is separate from `nvm` on purpose: nvm's check passes as soon as
its own sourceable script exists, which says nothing about whether a node
version was ever installed through it. Mason installs several language servers from npm, so
nvm-without-node fails all of them on the first `nvim` launch.

### A check_command must never contain a pipe

The manifest parser splits each line on `|` into exactly three fields. A
literal `|` anywhere in the check field therefore ends the check early and
pushes the remainder into `docs_url`. The truncated check still runs, and it
still returns an answer. The answer is wrong, and nothing reports an error.
This is the easiest way to break the manifest, and the failure is silent.

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

Add a line to `deps.conf`, or to `deps-mac.conf` / `deps-linux.conf` when
the dependency belongs to one platform only.

The default install command is `<package manager> install <name>`, which is
correct while the package name matches the `name` field. When the names
differ, or when the dependency does not come from a package manager at all,
add an entry to the install catalog in
`crates/config-cli/src/deps/catalog.rs`.

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
`node` installs through nvm rather than the package manager, so the version
this shell selects is the one nvm manages. Its command sources nvm's own
script first, because `nvm` is a shell function rather than a binary and is
not on `PATH` for a non-interactive shell. The case is empty when nvm is absent:
installing node has no meaning before its version manager exists, and the
`nvm` entry already reports that gap on its own line.

install. `nvm` is the one such case today, because its own documentation
publishes only version-pinned install URLs. An empty case still gets checked
and reported. It never gets installed, and it never fails the exit code of
`config deps install`.

## PATH

The engine prepends `~/.local/bin` and `~/.cargo/bin` to `PATH`.
`rustup` installs into the second directory, and `zoxide` into the first when
it falls back to its own installer, so without this a `command -v` check
fails on the line right after its own install succeeded. An interactive shell
usually exports both already, which is what hides the problem on a machine
already in use.

`zoxide` reaches that fallback only where the package manager has no package
for it. `apt`, `brew` and `pacman` all have one, and it is installed from
there, because
zoxide's upstream install script resolves the latest release through the unauthenticated
GitHub API. That quota is 60 requests an hour per IP, shared by every Actions
runner on that IP, and it failed a CI run that had nothing to do with zoxide.
The installer accepts no token, so caching would not have helped: a cache
miss still calls the API.

## Running it

```sh
config deps check                     # check only
config deps install                   # check, then prompt per install
config deps install --yes             # check, then install without prompting
config deps install --dry-run         # print what install would run
config deps check --only tmux,fzf     # restrict the run to a subset
depcheck                              # alias for `config deps install`
~/.scripts/deps/test-local.sh         # bootstrap fresh containers
~/.scripts/deps/test-bootstrap.sh     # full bootstrap in a container
```

### `--only <names>`

Restricts the run to a comma-separated subset of the manifest.

This exists for `.github/workflows/test-suite.yml`. That workflow used to
carry a hand-written apt list and a hand-written brew list naming `tmux`,
`zsh`, `git`, `fzf`, `ripgrep` and `shellcheck` -- every one of them already a
`deps.conf` entry. Two copies of the same package names meant the copy in YAML
was the one that drifted, and nothing checked it. The workflow now names the
set it wants and the engine resolves each name to the right package for
whichever manager the runner has.

The suite's set is a subset on purpose: a test run has no use for `neovim`,
`alacritty`, `aerospace`, `tpm` or `nvm`, and installing them on every push
would add minutes and more upstream services that can fail a run about a shell
script.

A name that matches no entry exits 2 rather than selecting nothing. A typo in
a workflow file that quietly installed none of what it promised would still
report success, which is worse than a hard failure.

`depcheck` is defined in `depcheck-hook.sh` as:

```sh
alias depcheck='~/.local/bin/config deps install'
```

An unknown argument exits 2.

### Exit codes

- `config deps check`: non-zero when anything is missing. This is
  informational. The startup hook and a plain manual run both use it.
- `config deps install`: non-zero only when a dependency that had an automated
  install command still fails its check after the install ran. A dependency
  with no automated install path is reported and does not affect the exit
  code, since the install had nothing to do differently. A declined prompt is
  treated the same way.
- Either verb with `--dry-run`: always 0.
- Either verb given an unknown flag, or an `--only` name that matches no
  entry: 2.

## The startup hook

`depcheck-hook.sh` runs the check-only path at most once every 24 hours and
prints one line when anything is missing. It never installs, never prompts,
and never blocks startup. The throttle timestamp lives in
`~/.cache/depcheck-last-run`. An unwritable `~/.cache` costs the throttle and
nothing else, so the next shell checks again.

## Continuous integration

`.github/workflows/deps-check.yml` runs `config deps install --yes` as a real
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
