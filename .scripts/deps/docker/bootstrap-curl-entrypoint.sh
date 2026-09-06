#!/bin/sh
# Verifies the documented one-liner end to end, genuinely fetched and piped:
#
#   curl -fsSL <base>/setup.sh | sh -s -- --repo <base>/repo.git
#
# bootstrap-bare-entrypoint.sh beside this one covers the same bare image by
# copying setup.sh out of the seed and running it as a local file. That
# leaves the fetch-and-pipe path itself untested, and three of its properties
# exist only when it is real: stdin is the pipe rather than a terminal, flags
# arrive through `sh -s --`, and the clone URL is a URL. See
# Dockerfile.bootstrap-curl for the full argument.
#
# The seed is served from inside this container, so no host networking is
# needed and the test behaves the same on a laptop and on a CI runner.
set -eu

failed=0
check() {
    check_description=$1
    shift
    if "$@"; then
        printf 'ok: %s\n' "$check_description"
    else
        printf 'FAIL: %s\n' "$check_description"
        failed=$((failed + 1))
    fi
    unset check_description
}

# The bind mount is owned by the host user while this container runs as root,
# and `git clone` over HTTP does not care about ownership -- but
# `update-server-info` writes into the repo, so the seed is copied to a
# root-owned path first. Same reasoning as bootstrap-bare-entrypoint.sh,
# different trigger.
SEED_SRC=/seed
SERVE=/tmp/serve
mkdir -p "$SERVE"
cp "$SEED_SRC/setup.sh" "$SERVE/setup.sh"
cp -R "$SEED_SRC/repo.git" "$SERVE/repo.git"

# Dumb HTTP transport needs this: without it `git clone` over plain HTTP
# fails with "repository not found", because there is no smart-http CGI here
# and the client falls back to reading info/refs directly.
#
# Run through `env -i` with an explicit PATH because git is not installed yet
# at this point in the run -- except it is needed HERE, before setup.sh, to
# prepare the served repo. Resolved by doing it with python instead: the
# refs file is a two-column text file and generating it needs no git.
python3 - "$SERVE/repo.git" <<'PYEOF'
import os, sys
repo = sys.argv[1]
lines = []
for root, _dirs, files in os.walk(os.path.join(repo, "refs")):
    for name in files:
        full = os.path.join(root, name)
        ref = os.path.relpath(full, repo)
        with open(full) as handle:
            sha = handle.read().strip()
        lines.append(f"{sha}\t{ref}\n")
packed = os.path.join(repo, "packed-refs")
if os.path.exists(packed):
    with open(packed) as handle:
        for line in handle:
            line = line.strip()
            if not line or line.startswith(("#", "^")):
                continue
            sha, _, ref = line.partition(" ")
            if ref.startswith("refs/"):
                lines.append(f"{sha}\t{ref}\n")
os.makedirs(os.path.join(repo, "info"), exist_ok=True)
with open(os.path.join(repo, "info", "refs"), "w") as handle:
    handle.writelines(sorted(set(lines)))
# objects/info/packs, for the same dumb-transport reason.
pack_dir = os.path.join(repo, "objects", "pack")
os.makedirs(os.path.join(repo, "objects", "info"), exist_ok=True)
with open(os.path.join(repo, "objects", "info", "packs"), "w") as handle:
    if os.path.isdir(pack_dir):
        for name in sorted(os.listdir(pack_dir)):
            if name.endswith(".pack"):
                handle.write(f"P {name}\n")
    handle.write("\n")
PYEOF

PORT=8771
BASE="http://127.0.0.1:$PORT"

# Serve in the background and wait for it, rather than sleeping a fixed
# interval: a fixed sleep is either flaky or slow, and this loop is neither.
( cd "$SERVE" && python3 -m http.server "$PORT" --bind 127.0.0.1 >/tmp/httpd.log 2>&1 ) &
server_pid=$!
trap 'kill "$server_pid" 2>/dev/null || true' EXIT INT TERM HUP

waited=0
until curl -fsS "$BASE/setup.sh" >/dev/null 2>&1; do
    waited=$((waited + 1))
    if [ "$waited" -gt 50 ]; then
        printf 'FAIL: the seed server never came up\n' >&2
        cat /tmp/httpd.log >&2
        exit 1
    fi
    sleep 0.1
done

printf '=== the documented one-liner, fetched and piped ===\n'
printf 'curl -fsSL %s/setup.sh | sh -s -- --repo %s/repo.git\n\n' "$BASE" "$BASE"

# The real invocation. No `cp`, no local file, and deliberately no `--yes`:
# stdin is the pipe, so setup.sh has to infer unattended by itself. That
# inference is the thing under test, and passing --yes would hide a
# regression in it.
#
# The fetch is checked separately BEFORE the pipe, because a pipeline reports
# only its last command's status: `curl` failing writes nothing to stdout,
# `sh` reads an empty script, and the pipeline exits 0. Measured -- a 404
# piped into `sh` gives exit 0 -- so a single `curl ... | sh` status check
# passes when setup.sh was never fetched at all. The other assertions would
# still fail, but the one that names the cause would have lied.
set +e
curl -fsSL "$BASE/setup.sh" >/tmp/fetched-setup.sh 2>/tmp/fetch-err
fetch_status=$?
set -e

printf '=== assertions ===\n'
check 'the fetch itself succeeds' test "$fetch_status" -eq 0
check 'the fetched script is not empty' test -s /tmp/fetched-setup.sh

# Piped from curl for real, rather than from the file just saved: the saved
# copy proves the fetch worked, and this proves the documented shape works.
set +e
run_output=$(curl -fsSL "$BASE/setup.sh" | sh -s -- --repo "$BASE/repo.git" 2>&1)
run_status=$?
set -e

printf '%s\n' "$run_output"
printf '\n'

check 'the piped run exits 0' test "$run_status" -eq 0

# git is absent on this image, so the run must install it and carry on. Same
# assertions as the bare harness, because the same regression is reachable
# through this path and a fix to one entry point must not miss the other.
check 'the run reports git is missing' \
    grep -qi 'git is not installed' <<EOFI
$run_output
EOFI
check 'it installs git rather than only naming it' \
    grep -qi 'installing git' <<EOFI
$run_output
EOFI
check 'it never tells the reader to run it again' \
    test -z "$(printf '%s\n' "$run_output" | grep -i 'then run this again' || true)"
check 'git is on PATH afterwards' command -v git

# The clone came from a URL over HTTP, which is the path a real machine takes
# and which bootstrap-bare-entrypoint.sh does not exercise.
check 'the bare repo exists' test -d "$HOME/.cfg"
check 'status.showUntrackedFiles is no' \
    test "$(git --git-dir="$HOME/.cfg" config --get status.showUntrackedFiles)" = 'no'

# Branch selection. This container is Linux, so a mac-only seed would be a
# harness bug rather than a product bug -- the CI job seeds both branches.
checked_out=$(git --git-dir="$HOME/.cfg" --work-tree="$HOME" rev-parse --abbrev-ref HEAD 2>/dev/null || printf '<none>')
printf 'checked-out branch: %s\n' "$checked_out"
check 'it checked out a branch this platform can use' \
    test "$checked_out" != '<none>'

check '.zshrc is checked out' test -f "$HOME/.zshrc"
check '.scripts is checked out' test -d "$HOME/.scripts"

backup=$(find "$HOME" -maxdepth 1 -type d -name '.dotfiles-backup-*' | head -1)
check 'the pre-existing .zshrc was moved aside' test -n "$backup"

check 'config is on PATH' test -x "$HOME/.local/bin/config"
check 'the pre-commit hook is linked' test -L "$HOME/.cfg/hooks/pre-commit"
check 'the pre-push hook is linked' test -L "$HOME/.cfg/hooks/pre-push"

# A privileged install resolved to a runnable command. Root with no sudo, so
# a `sudo`-prefixed command would have failed.
check 'a privileged install actually succeeded (tmux)' command -v tmux
check 'no install reported a missing sudo' \
    test -z "$(printf '%s\n' "$run_output" | grep 'sudo: not found' || true)"

# The end state has to be a working shell, not just a populated directory.
# `zsh -i` sources the whole .zshrc including the depcheck hook, so a syntax
# error or a missing sourced file surfaces here and nowhere else in this
# harness.
if command -v zsh >/dev/null 2>&1; then
    set +e
    zsh_output=$(zsh -i -c 'printf ZSH_OK' 2>&1)
    set -e
    check 'an interactive zsh starts and reaches the end of .zshrc' \
        grep -q 'ZSH_OK' <<EOFI
$zsh_output
EOFI
    check 'zsh startup reports no error' \
        test -z "$(printf '%s\n' "$zsh_output" | grep -iE 'command not found|parse error|no such file' || true)"
else
    printf 'FAIL: zsh was never installed\n'
    failed=$((failed + 1))
fi

printf '\n=== curl-pipe bootstrap: %d check(s) failed ===\n' "$failed"
[ "$failed" -eq 0 ]
