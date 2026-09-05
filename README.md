# Dot Files

| | mac | linux |
| --- | --- | --- |
| Test suite | [![mac test suite](https://github.com/austintheriot/dotfiles/actions/workflows/test-suite.yml/badge.svg?branch=mac)](https://github.com/austintheriot/dotfiles/actions/workflows/test-suite.yml?query=branch%3Amac) | [![linux test suite](https://github.com/austintheriot/dotfiles/actions/workflows/test-suite.yml/badge.svg?branch=linux)](https://github.com/austintheriot/dotfiles/actions/workflows/test-suite.yml?query=branch%3Alinux) |
| Branch drift | [![mac branch drift](https://github.com/austintheriot/dotfiles/actions/workflows/branch-drift.yml/badge.svg?branch=mac)](https://github.com/austintheriot/dotfiles/actions/workflows/branch-drift.yml?query=branch%3Amac) | [![linux branch drift](https://github.com/austintheriot/dotfiles/actions/workflows/branch-drift.yml/badge.svg?branch=linux)](https://github.com/austintheriot/dotfiles/actions/workflows/branch-drift.yml?query=branch%3Alinux) |

Welcome! Here is my rather unpolished .dotfile configuration. Below are some setup notes to my future self, for when I inevitably completely forget what I did or how I did it.

## Setup

### Homebrew setup

- Setup homebrew: https://brew.sh/

### Git setup

- Install git

- Install GitHub CLI (for some command-line git utilities): https://cli.github.com/

```sh
brew install gh
```

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

- Install via rustup (do NOT use brew for this on mac!): https://www.rust-lang.org/tools/install

### macOS build performance

- Add every terminal you launch builds from (Alacritty, Terminal, iTerm) to
  System Settings -> Privacy & Security -> Developer Tools.

Without it, `syspolicyd` validates each newly built executable the first time
it runs, and that validation can reach out to Apple over the network. Measured
here at ~180ms per fresh binary, and it repeats after every rebuild, so a
compile-and-run loop pays it every iteration. Offline, it can block until the
request times out.

Two things that look like the fix and are not:

- `spctl --global-disable` does not remove it.
- `DevToolsSecurity -status` reports "Developer mode is currently disabled"
  even once this is set correctly. That flag tracks the separate `developer`
  group used for debugger attachment, not this exemption, so do not use it to
  check whether this worked.

To verify, build a small binary, copy it to a new path, and time the first run.
It should be about 0ms rather than ~180ms:

```sh
cp target/release/mybin /tmp/gk-check && /usr/bin/time -p /tmp/gk-check
```

`.zshrc` no longer runs `tmux source` per shell (1.7s per pane, all of it
tpm); run `config reload` after editing the tmux config.

### Window manager setup

- Install aerospace as a MacOS window manager: https://nikitabobko.github.io/AeroSpace/guide.html

```sh
brew install --cask nikitabobko/tap/aerospace
```

Enable `Group windows by application`

```sh
defaults write com.apple.dock expose-group-apps -bool true && killall Dock
```

Disable `Displays have separate spaces`

```sh
defaults write com.apple.spaces spans-displays -bool true && killall SystemUIServer
```

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

Claude Code fires an OS-level notification when it finishes a turn, but only
when the active tmux pane in the frontmost Alacritty window is *not* the one
running Claude.

Wired up via the `Stop` hook in `~/.claude/settings.json`, which calls
`~/.claude/hooks/notify.sh`. The script uses `aerospace list-windows --focused`
plus `tmux display -p '#{pane_active}#{window_active}'` to decide whether to
suppress the notification, then fires it via `osascript -e 'display
notification ...'` (notifications appear under "Script Editor").

`terminal-notifier` was tried first but its notifications were silently
dropped on this machine despite Settings showing them as enabled — a known
issue with its bundle on recent macOS. osascript Just Works and needs no
permission setup.
