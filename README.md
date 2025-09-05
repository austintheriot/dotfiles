# Dot Files

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

- Install neovim (editor): https://neovim.io/

- (optional) Install NvChad: https://nvchad.com/docs/quickstart/install

To install (modified the install location to not overwrite default Neovim location)

```sh
git clone https://github.com/NvChad/starter ~/.config/nvchad
```

To start (custom alias in ~/.zshrc)

```sh
nvc .
```

- Install fzf (command-line fuzzy finder, used by neovim): https://github.com/junegunn/fzf

- Install ripgrep (faster grep alternative written in Rust, used by neovim): https://github.com/BurntSushi/ripgrep

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
