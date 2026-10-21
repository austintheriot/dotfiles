#!/bin/zsh
# Installs the vm CLI and what it needs on this Mac, then runs vm doctor.
# Safe to rerun: each step skips what is already in place.
set -euo pipefail

readonly ROOT="${0:A:h}"
readonly LINK_DIR="$HOME/.local/bin"

step() { print -P "%B==> $1%b"; }
fail() { print -u2 -r -- "install.sh: $1"; exit 1; }

[[ "$(uname -s)" == Darwin && "$(uname -m)" == arm64 ]] || fail "vm needs a Mac with Apple silicon: Tart runs macOS guests only there."

bun=$(command -v bun || true)
[[ -z "$bun" && -x "$HOME/.bun/bin/bun" ]] && bun="$HOME/.bun/bin/bun"
[[ -n "$bun" ]] || fail "Bun is not installed. Install it with: curl -fsSL https://bun.sh/install | bash, then rerun ./install.sh"

command -v brew >/dev/null || fail "Homebrew is not installed. Install it from https://brew.sh, then rerun ./install.sh"

step "Homebrew packages"
command -v tart >/dev/null || brew install --quiet cirruslabs/cli/tart
command -v ffmpeg >/dev/null || brew install --quiet ffmpeg

step "Softnet (VM network isolation)"
if [[ "$(stat -f '%Su %Mp%Lp' /usr/local/bin/softnet 2>/dev/null)" == "root 4755" ]]; then
  echo "already installed"
else
  "$ROOT/setup/install-softnet.sh"
fi

step "vm dependencies"
(cd "$ROOT" && "$bun" install --frozen-lockfile)

step "vm command"
mkdir -p "$LINK_DIR"
ln -sfn "$ROOT/bin/vm" "$LINK_DIR/vm"
echo "linked $LINK_DIR/vm"
if [[ ":$PATH:" != *":$LINK_DIR:"* ]]; then
  print -r -- "$LINK_DIR is not on PATH. Add this line to ~/.zshrc, then open a new terminal:"
  print -r -- '  export PATH="$HOME/.local/bin:$PATH"'
fi

step "vm doctor"
PATH="$LINK_DIR:$PATH" "$LINK_DIR/vm" doctor || {
  echo
  echo "Fix the missing items above. To build the base image, follow $ROOT/setup/README.md."
  exit 1
}
