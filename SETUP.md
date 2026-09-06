# Setup

How a machine goes from nothing to a working configuration. The one-line
install is in [README.md](./README.md); this file is the detail behind it.

## The two halves

The split is the chicken-and-egg boundary. `setup.sh` is what you need
*before* the repository exists, and it does nothing else. `config init` is
everything after, and it lives in the repository because by then there is one.

`setup.sh` clones the bare repo to `~/.cfg`, detects the platform and offers
the matching branch, checks the worktree out into `$HOME`, and hands off to
`config init`. A pre-existing `.zshrc` is moved into a timestamped
`~/.dotfiles-backup-*` directory rather than overwritten, so nothing already on
the machine is lost.

`setup.sh` refuses to run against an existing `~/.cfg`, because that directory
is the repository and an unpushed commit lives nowhere else.

`config init` sets `status.showUntrackedFiles`, links the git hooks, puts
`config` on PATH, installs the missing tracked dependencies, and builds the
stamped binary, in that order. It is idempotent, so re-running it after a pull
is the intended way to pick up a new dependency.

## One URL, both platforms

```sh
curl -fsSL https://raw.githubusercontent.com/austintheriot/dotfiles/mac/setup.sh | sh
```

The same URL on every platform. `mac` in it is only the branch the file is
fetched from, not the branch you get: `setup.sh` reads `uname` and checks out
the matching branch itself. `tests/setup.test.sh` asserts the file is the
same blob on `mac` and `linux`, so one URL is the whole story. Verified on
Linux against the `mac` URL: it selects `linux`.

## Unattended versus interactive

Nothing needs to be installed first, not even git. A piped run has no terminal
to answer a prompt, so the script treats that as unattended: it installs git if
git is missing, then clones, checks out, and installs everything else. Verified
from a bare `debian:bookworm-slim` carrying nothing but curl.

With a terminal it asks before installing git, since a machine that is missing
git is more likely a surprise than an intent. It also offers the detected
branch, and `config init` prompts before each install. `--yes` skips both.

## Options

Passing flags through a pipe needs `sh -s -- <flags>`:

```sh
sh -s -- --dry-run          # print every step and change nothing
sh -s -- --branch work      # check out a branch uname cannot imply
sh -s -- --repo <url|path>  # clone from somewhere other than the default remote
sh -s -- --yes              # force unattended where a terminal IS present
```

Run from a file rather than a pipe (`sh setup.sh --dry-run`) and the flags need
no `-s --`.

On a machine that is already cloned, run the second half directly:

```sh
config init            # prompts before each install
config init --yes      # unattended
config init --dry-run  # print every step, change nothing
```

## What is not automated

A bootstrap can only automate what a package manager will install unattended.
Alacritty on macOS and `nvm` still need a human. The tracked manifest in
`.scripts/deps/` is the list of what is covered; `depcheck` reports what is
missing at any time.

## Tests

`.scripts/deps/test-bootstrap.sh` runs the whole bootstrap in a container that
starts with git, curl and sudo and nothing else. The `bootstrap` job in
`.github/workflows/deps-check.yml` runs it in CI.
