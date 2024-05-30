# Dot Files

Welcome! Here is my rather unpolished .dotfile configuration. Below are some setup notes to my future self, for when I inevitably completely forget what I did or how I did it.

## Setup

### Dotfile setup 

Follow instructions here for configuring git repo at your base path: https://www.atlassian.com/git/tutorials/dotfiles

Or see local copy here, if that link no longer works: [DOTFILES](./DOTFILES.md).

### Shell setup

- Install Alacritty (cross platform terminal emulator): https://alacritty.org/

- Install zsh (shell): https://ohmyz.sh/

- Install zsh-autosuggestions: https://github.com/zsh-users/zsh-autosuggestions/blob/master/INSTALL.md

- Install neovim (editor): https://neovim.io/

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

### Env setup

#### JS

- Install nvm (or equivalent on Windows): https://github.com/nvm-sh/nvm

- Use nvm to install latest Node version

#### Rust

- Install via rustup (do NOT use brew for this on mac!): https://www.rust-lang.org/tools/install


