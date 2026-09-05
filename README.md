# Dot Files

| | mac | linux |
| --- | --- | --- |
| Test suite | [![mac test suite](https://github.com/austintheriot/dotfiles/actions/workflows/test-suite.yml/badge.svg?branch=mac)](https://github.com/austintheriot/dotfiles/actions/workflows/test-suite.yml?query=branch%3Amac) | [![linux test suite](https://github.com/austintheriot/dotfiles/actions/workflows/test-suite.yml/badge.svg?branch=linux)](https://github.com/austintheriot/dotfiles/actions/workflows/test-suite.yml?query=branch%3Alinux) |
| Branch drift | [![mac branch drift](https://github.com/austintheriot/dotfiles/actions/workflows/branch-drift.yml/badge.svg?branch=mac)](https://github.com/austintheriot/dotfiles/actions/workflows/branch-drift.yml?query=branch%3Amac) | [![linux branch drift](https://github.com/austintheriot/dotfiles/actions/workflows/branch-drift.yml/badge.svg?branch=linux)](https://github.com/austintheriot/dotfiles/actions/workflows/branch-drift.yml?query=branch%3Alinux) |

Welcome! Here is my rather unpolished .dotfile configuration. Below are some setup notes to my future self, for when I inevitably completely forget what I did or how I did it.

## Setup

### Git setup

- Install git

- Install GitHub CLI (for some command-line git utilities): https://cli.github.com/

### Shell setup

- Install Alacritty (cross platform terminal emulator): https://alacritty.org/

- Install zsh (shell): https://ohmyz.sh/

- Install zsh-autosuggestions: https://github.com/zsh-users/zsh-autosuggestions/blob/master/INSTALL.md

- Install `neovim` (editor): https://neovim.io/

- Install `fzf` (command-line fuzzy finder, used by neovim): https://github.com/junegunn/fzf

- Install `ripgrep` (faster grep alternative written in Rust, used by neovim): https://github.com/BurntSushi/ripgrep

- Install `shellcheck` (shell script linter, run by the test suite): https://www.shellcheck.net/

- Install zoxide for `z` in place of `cd`: https://github.com/ajeetdsouza/zoxide

- Install xclip (clipboard integration for tmux copy mode -- WSLg bridges this to the Windows clipboard automatically)

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

- Install via rustup (do NOT use brew for this on mac!): https://www.rust-lang.org/tools/install

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
- `config install` installs any missing tracked dependencies.
- `config install-hooks` links the git hooks and puts `config` on PATH.
- `config reload` reloads the tmux config.
- `config stamp` prints the tree id of the crate in the worktree.
- `config sync` copies the shared paths onto the other branch.
- `config test` runs the test suite.

To add a utility, drop a `config-<name>` script into `.scripts/config/` with a
`# help:` line, and add its name to `EXPECTED_SUBCOMMANDS` in
`tests/config.test.sh`.

`config help` shadows `git help`. Use `config -- <verb>` to send a verb
straight to git without consulting these scripts: `config -- help rebase`
opens the git manual page. The separator applies only as the first argument,
so a later `--` is still a git pathspec.

### Dependency checking

The dependencies listed above, plus a few that exist on one branch only, are
also tracked in a checkable manifest: `.scripts/deps/deps.conf` and
`.scripts/deps/deps-local.conf`.

- `~/.scripts/deps/check-deps.sh` checks them and reports what is missing.
- `depcheck` is a shell alias that checks them and offers to install anything
  missing.
- A shell-startup hook prints one line at most once every 24 hours when
  something has gone missing. It never installs and never blocks startup.

See `.scripts/deps/README.md` for the manifest format, the constraint on
check commands, and how to add a dependency.

### Claude Code notifications

On mac, the Stop hook fires an OS-level notification via aerospace + osascript
(see the mac branch's README). aerospace and osascript are both macOS-only, so
that hook isn't ported here -- Claude Code notifications aren't wired up on
Linux/WSL yet. If picking this back up: WSLg (this box has a live
`DISPLAY`/`WAYLAND_DISPLAY`) can run `notify-send` (apt package
`libnotify-bin`), or `powershell.exe` can be called through WSL interop for a
native Windows toast.
