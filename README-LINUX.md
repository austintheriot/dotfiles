# Linux / WSL-specific setup

Setup notes that apply only to the Linux machine. Everything shared lives in
[README.md](./README.md). This file ships on both branches so the drift check
covers it; only a Linux reader needs it.

## Shell setup

- Install oh-my-zsh: https://ohmyz.sh/

  zsh-autosuggestions is installed as an oh-my-zsh custom plugin here, rather
  than from Homebrew the way the mac machine does it. `.zshrc-linux` sources
  it from `~/.oh-my-zsh/custom/plugins`, which is why `oh-my-zsh` is listed in
  `.scripts/deps/deps-linux.conf`: drop it and the shell sources a plugin
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

Not wired up here. The mac hook drives aerospace and osascript, both of which
are macOS-only, so `~/.claude/hooks/notify.sh` is a mac-branch file.

If picking this back up: WSLg gives this box a live `DISPLAY` and
`WAYLAND_DISPLAY`, so `notify-send` (apt package `libnotify-bin`) can fire a
native notification, or `powershell.exe` can be called through WSL interop for
a Windows toast.
