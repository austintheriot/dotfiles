#!/bin/zsh
# Installs Softnet 0.19.0 setuid-root in /usr/local/bin so Tart can isolate desktop-QA VMs.
# Run it yourself: it asks for your sudo password once.
set -euo pipefail

readonly VERSION=0.19.0
readonly SHA256=1612e1296834aae0b6389650c7c5190add1ee8d71474e328691e67679ecda53c
readonly URL="https://github.com/cirruslabs/softnet/releases/download/$VERSION/softnet.tar.gz"
readonly TARGET=/usr/local/bin/softnet

work=$(mktemp -d /tmp/softnet-install-XXXXXX)
trap 'rm -rf "$work"' EXIT

curl -fsSL -o "$work/softnet.tar.gz" "$URL"
echo "$SHA256  $work/softnet.tar.gz" | shasum -a 256 -c
tar -xzf "$work/softnet.tar.gz" -C "$work" softnet

sudo install -o root -g wheel -m 4755 "$work/softnet" "$TARGET"

ls -l "$TARGET"
[[ "$(stat -f '%Su %Mp%Lp' "$TARGET")" == "root 4755" ]] || { echo "unexpected owner or mode on $TARGET" >&2; exit 1; }
[[ "$(command -v softnet)" == "$TARGET" ]] || echo "warning: 'softnet' on PATH resolves to $(command -v softnet), not $TARGET" >&2
echo "softnet $VERSION installed"
