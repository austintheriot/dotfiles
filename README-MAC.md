# macOS-specific setup

Setup notes that apply only to the mac machine. Everything shared lives in
[README.md](./README.md). This file ships on the single branch alongside the
Linux one; only a mac reader needs it.

## Rust

Install Rust through rustup, NOT through Homebrew. A brew-installed toolchain
does not carry rustup's target and component management, so `rustup target
add` and `cargo +nightly` stop working, and the two installs shadow each
other on PATH in whichever order the shell resolves them.

## Homebrew setup

- Setup homebrew: https://brew.sh/

## macOS build performance

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

## Window manager setup

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

## Claude Code notifications

`agent-notifyd` (`tools/agent-notify`) shows a card at the top right when a
Claude Code session finishes or needs you, and a click switches tmux to that
session's pane and raises Alacritty. The `agent-notify` plugin's hooks send
the events; agents can also call `agent-notify send` themselves.

Install or update it with `config build --tools`. It runs as the launch
agent `dev.dotfiles.agent-notifyd`. `agent-notify doctor` checks the setup,
`agent-notify dnd on` silences it, and Meta-n jumps to the newest card's
pane. Logs are in `~/Library/Logs/agent-notify/`.
