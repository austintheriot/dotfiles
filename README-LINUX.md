# Linux / WSL-specific setup

Setup notes that apply only to the Linux machine. Everything shared lives in
[README.md](./README.md). This file ships on the single branch alongside the
mac one; only a Linux reader needs it.

## Shell setup

- Install oh-my-zsh: https://ohmyz.sh/

  zsh-autosuggestions is installed as an oh-my-zsh custom plugin here, rather
  than from Homebrew the way the mac machine does it. `.zshrc-linux` sources
  it from `~/.oh-my-zsh/custom/plugins`, which is why `oh-my-zsh` is listed in
  `deps/deps-linux.toml`: drop it and the shell sources a plugin
  nothing installs, losing autosuggestions silently.

- Install xclip: `sudo apt install xclip`

  tmux copy mode pipes through xclip on this machine (`pbcopy` on mac). See
  `.config/tmux/tmux-linux.conf`. Under WSLg, xclip's X selection is bridged
  to the Windows clipboard automatically, so no WSL-specific handling is
  needed.

## fzf

The Ubuntu apt package predates `fzf --zsh` (added in fzf 0.48). The shared
`.zshrc` falls back to the shell-integration scripts the package ships under
`/usr/share/doc/fzf/examples`, so both key bindings and completion still work
on the older build.

## Claude Code notifications

Not wired up yet. tools/agent-notify runs on macOS only until its Linux plan
lands. The CLI's fallback (notify-send) works on Pop!_OS COSMIC today.
