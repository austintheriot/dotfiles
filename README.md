# Dot Files

| | mac | linux |
| --- | --- | --- |
| Test suite | [![mac test suite](https://github.com/austintheriot/dotfiles/actions/workflows/test-suite.yml/badge.svg?branch=mac)](https://github.com/austintheriot/dotfiles/actions/workflows/test-suite.yml?query=branch%3Amac) | [![linux test suite](https://github.com/austintheriot/dotfiles/actions/workflows/test-suite.yml/badge.svg?branch=linux)](https://github.com/austintheriot/dotfiles/actions/workflows/test-suite.yml?query=branch%3Alinux) |
| Branch drift | [![mac branch drift](https://github.com/austintheriot/dotfiles/actions/workflows/branch-drift.yml/badge.svg?branch=mac)](https://github.com/austintheriot/dotfiles/actions/workflows/branch-drift.yml?query=branch%3Amac) | [![linux branch drift](https://github.com/austintheriot/dotfiles/actions/workflows/branch-drift.yml/badge.svg?branch=linux)](https://github.com/austintheriot/dotfiles/actions/workflows/branch-drift.yml?query=branch%3Alinux) |

Welcome! Here is my rather unpolished .dotfile configuration. Below are some setup notes to my future self, for when I inevitably completely forget what I did or how I did it.

## Platform-specific setup

This file is shared byte-for-byte between the `mac` and `linux` branches.
Anything that is true of only one machine lives beside it:

- [README-MAC.md](./README-MAC.md) -- Homebrew, aerospace, macOS build
  performance, and the Claude Code notification hook.
- [README-LINUX.md](./README-LINUX.md) -- xclip, oh-my-zsh, and the WSL notes.

Both files ship on both branches, so `config check` covers them too.

## Setup

### Bootstrapping a new machine

One command, on a machine that has nothing but git:

```sh
curl -fsSL https://raw.githubusercontent.com/austintheriot/dotfiles/mac/setup.sh | sh -s -- --yes
```

`setup.sh` clones the bare repo to `~/.cfg`, detects the platform and offers
the matching branch, checks the worktree out into `$HOME`, and then hands off
to `config init`. A pre-existing `.zshrc` is moved into a timestamped
`~/.dotfiles-backup-*` directory rather than overwritten, so nothing already
on the machine is lost.

On a machine that is already cloned, run the second half directly:

```sh
config init            # prompts before each install
config init --yes      # unattended
config init --dry-run  # print every step, change nothing
```

`config init` sets `status.showUntrackedFiles`, links the git hooks, puts
`config` on `PATH`, installs the missing tracked dependencies, and builds the
stamped binary, in that order. It is idempotent, so re-running it after a pull
is the intended way to pick up a new dependency.

The split is the chicken-and-egg boundary: `setup.sh` is what you need
*before* the repo exists, and it does nothing else. `setup.sh` refuses to run
against an existing `~/.cfg`, because that directory is the repository and an
unpushed commit lives nowhere else.

`.scripts/deps/test-bootstrap.sh` runs the whole thing in a container that
starts with git, curl and sudo and nothing else, and the `bootstrap` job in
`.github/workflows/deps-check.yml` runs it in CI.

The steps below are the manual equivalents, kept because a bootstrap can only
automate what a package manager will install unattended. Alacritty on macOS
and `nvm` still need a human.

### Git setup

- Install git

- Install GitHub CLI (for some command-line git utilities): https://cli.github.com/
  (`brew install gh` on mac, `sudo apt install gh` on Debian/Ubuntu)

### Shell setup

- Install Alacritty (cross platform terminal emulator): https://alacritty.org/

- Install zsh (shell): https://ohmyz.sh/

- Install zsh-autosuggestions: https://github.com/zsh-users/zsh-autosuggestions/blob/master/INSTALL.md

- Install `neovim` (editor): https://neovim.io/

- (optional) Install NvChad: https://nvchad.com/docs/quickstart/install

To install (modified the install location to not overwrite default Neovim location)

```sh
git clone https://github.com/NvChad/starter ~/.config/nvchad
```

To start (custom alias in ~/.zshrc)

```sh
nvc .
```

- Install `fzf` (command-line fuzzy finder, used by neovim): https://github.com/junegunn/fzf

- Install `ripgrep` (faster grep alternative written in Rust, used by neovim): https://github.com/BurntSushi/ripgrep

- Install `shellcheck` (shell script linter, run by the test suite): https://www.shellcheck.net/

- Install zoxide for `z` in place of `cd`: https://github.com/ajeetdsouza/zoxide

- Install tmux (terminal multiplexer): https://github.com/tmux/tmux

- Clone tpm (tmux plugin manager) (yes, this is to a different directory than the tmux config directory)

```sh
git clone https://github.com/tmux-plugins/tpm ~/.tmux/plugins/tpm
```

- Source the tmux config file within a tmux session

```sh
tmux source ~/.config/tmux/tmux.conf
```

- Install tpm plugins from within tmux session

prefix + I (this is usually Ctrl+b and then capital I)

- Source ~/.zshrc to ensure everything is working (and also maybe `~/.zshrc-mac` or `~/.zshrc-linux` etc. to ensure everything is working correctly)

### Dotfile setup

After git and basic shell stuff is installed, follow instructions here for configuring the `config` git repo at your base path: https://www.atlassian.com/git/tutorials/dotfiles

Or see local copy here, if that link no longer works: [DOTFILES](./DOTFILES.md).

### Env setup

#### JS

- Install nvm (or equivalent on Windows): https://github.com/nvm-sh/nvm

- Use nvm to install latest Node version

#### Rust

- Install via rustup: https://www.rust-lang.org/tools/install
  (on mac, install through rustup rather than Homebrew -- see README-MAC.md)

### Repo utilities

`config` is the front door to this repo. It dispatches to the `config-<sub>`
scripts in `.scripts/config/`, and passes every other verb to git on the bare
repo at `~/.cfg`, so `config status`, `config add <path>` and `config commit`
all work as plain git.

Run `config help` for the current list. The listing is generated from the
`# help:` line at the top of each `config-<sub>`, so it cannot fall behind the
scripts:

- `config build` builds the Rust crate and installs the stamped binary.
- `config check` reports drift between the mac and linux branches.
- `config init` finishes a fresh clone: git config, hooks, dependencies,
  binary. The post-clone half of the bootstrap; `setup.sh` is the other
  half.
- `config install` installs any missing tracked dependencies.
- `config install-hooks` links the git hooks and puts `config` on PATH.
- `config reload` reloads the tmux config.
- `config stamp` prints the tree id of the crate in the worktree.
- `config sync` copies the shared paths onto the other branch.
- `config test` runs the test suite.

Each command documents its own flags. `config <command> --help` (or `-h`)
prints the `# usage:` block from the top of that script, so the help text and
the script header are the same lines:

```
$ config test --help
usage: config test [-q|--quiet] [-d|--docker] [-w|--watch] [suite]
...
```

Flags come in both spellings where a command takes them, so there is nothing
to remember about which form a given flag uses.

To add a utility, drop a `config-<name>` script into `.scripts/config/` with a
`# help:` line and a `# usage:` block, source `.scripts/config/usage.sh` and call
`usage_if_requested "${1:-}"` before parsing anything, and add its name to
`EXPECTED_SUBCOMMANDS` in `tests/config.test.sh`.

`config help` shadows `git help`. Use `config -- <verb>` to send a verb
straight to git without consulting these scripts: `config -- help rebase`
opens the git manual page. The separator applies only as the first argument,
so a later `--` is still a git pathspec.

### Dependency checking

The dependencies listed above, plus a few that belong to one platform only,
are also tracked in a checkable manifest: `.scripts/deps/deps.conf`, plus
`.scripts/deps/deps-mac.conf` and `.scripts/deps/deps-linux.conf`.

- `~/.scripts/deps/check-deps.sh` checks them and reports what is missing.
- `depcheck` is a shell alias that checks them and offers to install anything
  missing.
- A shell-startup hook prints one line at most once every 24 hours when
  something has gone missing. It never installs and never blocks startup.

See `.scripts/deps/README.md` for the manifest format, the constraint on
check commands, and how to add a dependency.
