# CLI Dependency Checking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the dotfiles repo an automated, best-effort way to check whether the CLI dependencies listed in `~/README.md` are installed, offer to install the missing ones, and verify the check/install logic against real disposable environments (Docker locally, GitHub Actions in CI) instead of trusting it on read-through alone.

**Architecture:** A pipe-delimited manifest (`deps.conf`, shared across `mac`/`linux`; `deps-local.conf`, per-branch and excluded from the drift-check) lists each dependency's presence-check command and docs URL. A POSIX-sh engine (`check-deps.sh`) reads both files, detects the machine's package manager, and either reports what's missing (default) or installs it (`--fix`, with `--yes` for non-interactive use and `--dry-run` for a preview). A cached, 24h-throttled hook in `.zshrc` runs the check-only path on shell startup and prints a one-line nag if anything's missing, never blocking. Two Dockerfiles (ubuntu/apt, archlinux/pacman) back both a local test harness and the `arch` leg of a scheduled GitHub Action; `macos-latest` covers brew directly since there's no macOS Docker base image.

**Tech Stack:** POSIX `sh` for `.my-scripts/deps/*.sh` (repo convention), `bash` for `tests/*.test.sh` (repo convention), Docker for local + CI Arch/Ubuntu verification, GitHub Actions (`ubuntu-latest`, `macos-latest`) for the scheduled full-bootstrap check, the existing `tests/lib.sh` harness.

**Spec:** `docs/superpowers/specs/2026-09-03-dependency-checking-design.md`

## Global Constraints

- This repo is public. The pre-commit leak guard (`tests/leak-check.sh`) must pass on every commit; never bypass it with `--no-verify` or by disabling it.
- Run `tests/run-all.sh` and confirm it passes before pushing any change to `.my-scripts/`, `tests/`, or `.claude/scripts/`, per `.claude/rules/dotfiles-tests.md`.
- Shell scripts under `.my-scripts/` use `#!/bin/sh` (POSIX), matching every existing script in that directory. Test files (`tests/*.test.sh`) use `#!/bin/bash`, matching every existing test file.
- Commit messages: lowercase, imperative, no period, a body paragraph when the summary line isn't self-evident (matches this repo's `git log`).
- `deps.conf`, `check-deps.sh`, `.my-scripts/deps/README.md`, the Docker files, and `.github/workflows/deps-check.yml` must end up byte-identical on both `mac` and `linux` (this is what `.my-scripts/` and `.github/workflows/` being in `.sync-manifest` already enforces). `deps-local.conf` is deliberately excluded and diverges per branch by design.
- Never use `git add <directory>` on `.claude` or the repo root — always name exact files/subpaths (see the credentials near-miss noted in the branch-drift-sync plan).
- `rustup` must never be installed via `brew install rust` on the `mac` branch — `~/README.md` calls this out explicitly.

---

### Task 1: Manifest + check/install engine

**Files:**
- Create: `.my-scripts/deps/deps.conf`
- Create: `.my-scripts/deps/deps-local.conf`
- Create: `.my-scripts/deps/check-deps.sh`
- Test: `tests/check-deps.test.sh`

**Interfaces:**
- Consumes: `$DEPS_CONF` / `$DEPS_LOCAL_CONF` env vars (default: `deps.conf` / `deps-local.conf` next to the script), matching `check-branch-drift.sh`'s `$DOTFILES_ROOT` override convention. `$PATH` for package-manager and dependency-binary detection.
- Produces: `check-deps.sh [--fix] [--yes] [--dry-run]`. Without `--fix`: prints `missing: <name>` per missing dependency, exits 1 if any, 0 otherwise. With `--fix` (not `--dry-run`): attempts an install per missing dependency, re-checks it, prints `installed: <name>` or `install did not satisfy the check for <name>`; exits 1 only if a real install attempt still failed its check afterward (a `--dry-run` run always exits 0; a dependency with no automated install path, or one the user declines, never makes `--fix` exit non-zero). This exit-code contract is what Task 3's Docker harness and Task 4's GitHub Action rely on to mean "the bootstrap actually worked."

- [ ] **Step 1: Write `deps.conf`**

```
# Shared CLI dependencies, tracked identically on every branch (see
# .sync-manifest). Format: name|check_command|docs_url
#
# check-deps.sh (.my-scripts/deps/check-deps.sh) reads this file plus, if
# present, deps-local.conf in the same directory, for platform-exclusive
# dependencies. See README.md in this directory for the install-command
# logic and how to add a new entry.
#
# oh-my-zsh must come before zsh-autosuggestions: the latter installs as an
# oh-my-zsh custom plugin, and its install command assumes oh-my-zsh already
# exists.

git|command -v git|https://git-scm.com/downloads
gh|command -v gh|https://cli.github.com/
alacritty|command -v alacritty|https://alacritty.org/
zsh|command -v zsh|https://www.zsh.org/
oh-my-zsh|[ -d "$HOME/.oh-my-zsh" ]|https://ohmyz.sh/
zsh-autosuggestions|[ -f "$HOME/.oh-my-zsh/custom/plugins/zsh-autosuggestions/zsh-autosuggestions.zsh" ]|https://github.com/zsh-users/zsh-autosuggestions
neovim|command -v nvim|https://neovim.io/
fzf|command -v fzf|https://github.com/junegunn/fzf
ripgrep|command -v rg|https://github.com/BurntSushi/ripgrep
zoxide|command -v zoxide|https://github.com/ajeetdsouza/zoxide
tmux|command -v tmux|https://github.com/tmux/tmux
tpm|[ -d "$HOME/.tmux/plugins/tpm" ]|https://github.com/tmux-plugins/tpm
rustup|command -v rustup|https://www.rust-lang.org/tools/install
nvm|[ -s "$HOME/.nvm/nvm.sh" ]|https://github.com/nvm-sh/nvm
```

- [ ] **Step 2: Write `deps-local.conf` (linux)**

```
# Linux/WSL-exclusive CLI dependencies. Excluded from .sync-manifest (see
# the `!.my-scripts/deps/deps-local.conf` line added in Task 4) so mac can
# carry its own unrelated version of this file. Format matches deps.conf.

xclip|command -v xclip|https://github.com/astrand/xclip
```

- [ ] **Step 3: Write the failing test — `tests/check-deps.test.sh`**

```bash
#!/bin/bash
#
# Tests for check-deps.sh: dependency presence checking and the --fix
# install flow. Package managers and installable tools are stubbed with fake
# executables on a fixture PATH rather than touching the real system --
# this script's whole job is deciding *which* command to run and whether the
# result satisfies the check, and running a real apt-get/brew/pacman here
# would either fail (no root, no such tool) or mutate the machine running
# the tests.
#
# Usage: ~/tests/check-deps.test.sh

. "$(dirname "$0")/lib.sh"

SCRIPT="$DOTFILES_ROOT/.my-scripts/deps/check-deps.sh"
BIN="$FIXTURES/bin"
mkdir -p "$BIN"

cat > "$BIN/sudo" <<'EOF'
#!/bin/sh
exec "$@"
EOF
chmod +x "$BIN/sudo"

cat > "$BIN/apt-get" <<EOF
#!/bin/sh
printf 'apt-get %s\n' "\$*" >> "$FIXTURES/apt.log"
exit 0
EOF
chmod +x "$BIN/apt-get"

cat > "$BIN/some-tool" <<'EOF'
#!/bin/sh
exit 0
EOF
chmod +x "$BIN/some-tool"

run_check() {
    local conf=$1; shift
    PATH="$BIN:$PATH" DEPS_CONF="$conf" DEPS_LOCAL_CONF="$FIXTURES/no-such-local.conf" \
        "$SCRIPT" "$@"
}

# --- everything present ---------------------------------------------------

conf="$FIXTURES/deps-present.conf"
printf 'some-tool|command -v some-tool|https://example.invalid\n' > "$conf"

output=$(run_check "$conf")
status=$?
assert_equals 'all present exits 0' '0' "$status"
assert_contains 'reports the count' 'all 1 dependencies present' "$output"

# --- a missing dependency, no --fix ---------------------------------------

conf="$FIXTURES/deps-missing.conf"
printf 'nonexistent-tool|command -v nonexistent-tool|https://example.invalid\n' > "$conf"

output=$(run_check "$conf")
status=$?
assert_equals 'a missing dep exits 1 without --fix' '1' "$status"
assert_contains 'names the missing dep' 'missing: nonexistent-tool' "$output"

# --- --fix --yes runs the default package-manager install, verifies it ---

: > "$FIXTURES/apt.log"
marker="$FIXTURES/installed-marker"
rm -f "$marker"
cat > "$BIN/apt-get" <<EOF
#!/bin/sh
printf 'apt-get %s\n' "\$*" >> "$FIXTURES/apt.log"
touch "$marker"
exit 0
EOF

conf="$FIXTURES/deps-fix-success.conf"
printf 'marker-tool|[ -f "%s" ]|https://example.invalid\n' "$marker" > "$conf"

output=$(run_check "$conf" --fix --yes)
status=$?
apt_log=$(cat "$FIXTURES/apt.log")
assert_contains 'runs apt-get install for the default case' 'install -y marker-tool' "$apt_log"
assert_contains 'confirms the install' 'installed marker-tool' "$output"
assert_equals 'a successful --fix exits 0' '0' "$status"

# --- --fix --yes reports (but does not fail on) an install that does not
#     satisfy its own check -----------------------------------------------

: > "$FIXTURES/apt.log"
cat > "$BIN/apt-get" <<EOF
#!/bin/sh
printf 'apt-get %s\n' "\$*" >> "$FIXTURES/apt.log"
exit 0
EOF

conf="$FIXTURES/deps-fix-fail.conf"
printf 'some-tool|command -v some-tool-not-really-installed|https://example.invalid\n' > "$conf"

output=$(run_check "$conf" --fix --yes)
status=$?
assert_contains 'reports the unsatisfied check' 'install did not satisfy the check' "$output"
assert_equals 'an unsatisfied install exits 1' '1' "$status"

# --- --fix --dry-run never executes anything, always exits 0 -------------

: > "$FIXTURES/apt.log"
conf="$FIXTURES/deps-dryrun.conf"
printf 'some-tool|command -v some-tool-not-really-installed|https://example.invalid\n' > "$conf"

output=$(run_check "$conf" --fix --dry-run)
status=$?
apt_log=$(cat "$FIXTURES/apt.log")
assert_equals 'dry-run never invokes apt-get' '' "$apt_log"
assert_contains 'prints what it would run' \
    'would run: sudo apt-get update -qq && sudo apt-get install -y some-tool' "$output"
assert_equals 'dry-run always exits 0' '0' "$status"

# --- a manual-only dependency is reported but never fails --fix ----------

conf="$FIXTURES/deps-manual.conf"
printf 'nvm|[ -f "%s/nonexistent-nvm.sh" ]|https://github.com/nvm-sh/nvm\n' "$FIXTURES" > "$conf"

output=$(run_check "$conf" --fix --yes)
status=$?
assert_contains 'names it as manual-only' 'no automated install' "$output"
assert_contains 'links the docs' 'https://github.com/nvm-sh/nvm' "$output"
assert_equals 'a manual-only dependency does not fail --fix' '0' "$status"

finish
```

- [ ] **Step 4: Run the test to verify it fails**

```bash
chmod +x ~/tests/check-deps.test.sh
bash ~/tests/check-deps.test.sh
```

Expected: fails immediately (`No such file or directory` — `check-deps.sh` doesn't exist yet).

- [ ] **Step 5: Write the minimal implementation — `.my-scripts/deps/check-deps.sh`**

```sh
#!/bin/sh
#
# Checks whether the CLI dependencies listed in deps.conf (and, if present,
# deps-local.conf) are installed, and optionally installs the missing ones.
#
# Usage:
#   check-deps.sh                 check only, exit 1 if anything is missing
#   check-deps.sh --fix           check, then interactively offer to install
#                                  each missing dependency
#   check-deps.sh --fix --yes     check, then install every missing
#                                  dependency without prompting (CI/Docker)
#   check-deps.sh --fix --dry-run print what --fix would run, without
#                                  running it (always exits 0)
#
# Reads dependencies from $DEPS_CONF (default: deps.conf next to this
# script) and, if it exists, $DEPS_LOCAL_CONF (default: deps-local.conf next
# to this script) -- the platform-exclusive dependencies that don't belong in
# the shared, cross-branch-identical deps.conf. See README.md in this
# directory for the manifest format and how to add a new dependency.
#
# Exit code:
#   without --fix: non-zero if anything is missing (informational -- used by
#     the shell-startup hook and a plain manual run).
#   with --fix (and not --dry-run): non-zero only if a dependency that had an
#     automated install command attempted still fails its check afterward.
#   Dependencies with no automated install path ("manual" in the output) are
#   reported but never make --fix's exit code non-zero -- there's nothing
#   --fix could have done differently. A user who declines an interactive
#   prompt is treated the same way: an informed choice, not a failure.

set -u

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
DEPS_CONF=${DEPS_CONF:-"$SCRIPT_DIR/deps.conf"}
DEPS_LOCAL_CONF=${DEPS_LOCAL_CONF:-"$SCRIPT_DIR/deps-local.conf"}

fix=0
yes=0
dry_run=0
for arg in "$@"; do
    case "$arg" in
        --fix) fix=1 ;;
        --yes) yes=1 ;;
        --dry-run) dry_run=1 ;;
        *)
            printf 'check-deps: unknown argument: %s\n' "$arg" >&2
            exit 2
            ;;
    esac
done

detect_pm() {
    if command -v pacman >/dev/null 2>&1; then
        printf 'pacman'
    elif command -v apt-get >/dev/null 2>&1; then
        printf 'apt'
    elif command -v brew >/dev/null 2>&1; then
        printf 'brew'
    else
        printf 'unknown'
    fi
}

# Returns (via stdout) the install command for dependency $1 under package
# manager $2, or nothing if there's no automated install for that
# combination. Anything not listed here falls through to the default case,
# which assumes the package name matches the dependency name -- true for
# most of deps.conf (git, zsh, tmux, fzf, ripgrep, xclip).
install_cmd_for() {
    name=$1
    manager=$2
    case "$name" in
        gh)
            case "$manager" in
                apt)
                    printf '%s' 'sudo apt-get update -qq && sudo apt-get install -y wget && sudo mkdir -p -m 755 /etc/apt/keyrings && wget -nv -O /etc/apt/keyrings/githubcli-archive-keyring.gpg https://cli.github.com/packages/githubcli-archive-keyring.gpg && sudo chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg && echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | sudo tee /etc/apt/sources.list.d/github-cli.list > /dev/null && sudo apt-get update -qq && sudo apt-get install -y gh'
                    ;;
                brew) printf 'brew install gh' ;;
                pacman) printf 'sudo pacman -Sy --noconfirm github-cli' ;;
            esac
            ;;
        alacritty)
            case "$manager" in
                apt) printf 'sudo apt-get update -qq && sudo apt-get install -y alacritty' ;;
                brew) printf 'brew install --cask alacritty' ;;
                pacman) printf 'sudo pacman -Sy --noconfirm alacritty' ;;
            esac
            ;;
        zoxide)
            printf 'curl -sS https://raw.githubusercontent.com/ajeetdsouza/zoxide/main/install.sh | sh'
            ;;
        oh-my-zsh)
            printf 'sh -c "$(curl -fsSL https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh)" "" --unattended'
            ;;
        zsh-autosuggestions)
            printf 'git clone https://github.com/zsh-users/zsh-autosuggestions "${ZSH_CUSTOM:-$HOME/.oh-my-zsh/custom}/plugins/zsh-autosuggestions"'
            ;;
        tpm)
            printf 'git clone https://github.com/tmux-plugins/tpm "$HOME/.tmux/plugins/tpm"'
            ;;
        rustup)
            printf "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
            ;;
        nvm)
            # Deliberately no automated install: nvm's own docs only publish
            # version-pinned install URLs, so a hardcoded one here would go
            # stale. Manual only -- see the docs_url column.
            ;;
        *)
            case "$manager" in
                apt) printf 'sudo apt-get update -qq && sudo apt-get install -y %s' "$name" ;;
                brew) printf 'brew install %s' "$name" ;;
                pacman) printf 'sudo pacman -Sy --noconfirm %s' "$name" ;;
            esac
            ;;
    esac
}

read_entries() {
    file=$1
    [ -f "$file" ] || return 0
    while IFS='|' read -r name check docs || [ -n "$name" ]; do
        case "$name" in
            ''|'#'*) continue ;;
        esac
        printf '%s|%s|%s\n' "$name" "$check" "$docs"
    done < "$file"
}

pm=$(detect_pm)
missing_count=0
checked_count=0
failed_fix_count=0

entries=$( { read_entries "$DEPS_CONF"; read_entries "$DEPS_LOCAL_CONF"; } )

old_ifs=$IFS
IFS='
'
for entry in $entries; do
    IFS=$old_ifs
    name=${entry%%|*}
    rest=${entry#*|}
    check=${rest%%|*}
    docs=${rest#*|}

    checked_count=$((checked_count + 1))

    if sh -c "$check" >/dev/null 2>&1; then
        IFS='
'
        continue
    fi

    missing_count=$((missing_count + 1))
    printf 'missing: %s\n' "$name"

    if [ "$fix" -eq 1 ]; then
        cmd=$(install_cmd_for "$name" "$pm")
        if [ -z "$cmd" ]; then
            printf '  no automated install for %s on this system -- see %s\n' "$name" "$docs"
        elif [ "$dry_run" -eq 1 ]; then
            printf '  would run: %s\n' "$cmd"
        else
            proceed=0
            if [ "$yes" -eq 1 ]; then
                proceed=1
            else
                printf '  install %s? [y/N] ' "$name"
                read -r reply
                case "$reply" in
                    y|Y) proceed=1 ;;
                esac
            fi

            if [ "$proceed" -eq 1 ]; then
                sh -c "$cmd"
                if sh -c "$check" >/dev/null 2>&1; then
                    printf '  installed %s\n' "$name"
                else
                    printf '  install did not satisfy the check for %s\n' "$name"
                    failed_fix_count=$((failed_fix_count + 1))
                fi
            fi
        fi
    fi
    IFS='
'
done
IFS=$old_ifs

if [ "$dry_run" -eq 1 ]; then
    printf '\ncheck-deps: dry run, %d of %d dependencies missing\n' "$missing_count" "$checked_count"
    exit 0
fi

if [ "$fix" -eq 1 ]; then
    if [ "$failed_fix_count" -gt 0 ]; then
        printf '\ncheck-deps: %d automated install(s) did not satisfy their check\n' "$failed_fix_count" >&2
        exit 1
    fi
    printf '\ncheck-deps: no unresolved failures (%d of %d were already missing)\n' "$missing_count" "$checked_count"
    exit 0
fi

if [ "$missing_count" -gt 0 ]; then
    printf '\ncheck-deps: %d of %d dependencies missing\n' "$missing_count" "$checked_count" >&2
    exit 1
fi

printf 'check-deps: all %d dependencies present\n' "$checked_count"
exit 0
```

- [ ] **Step 6: Make it executable and run the test to verify it passes**

```bash
chmod +x ~/.my-scripts/deps/check-deps.sh
bash ~/tests/check-deps.test.sh
```

Expected: `check-deps: 15 passed, 0 failed`.

- [ ] **Step 7: Run the full suite**

```bash
bash ~/tests/run-all.sh
```

Expected: all suites pass, including the new one.

- [ ] **Step 8: Sanity-check the real script against this actual machine**

```bash
~/.my-scripts/deps/check-deps.sh
```

Expected: `check-deps: all 15 dependencies present` (14 from `deps.conf` + `xclip` from `deps-local.conf`) — this machine already has every README dependency installed (confirmed at the start of this session).

- [ ] **Step 9: Commit**

```bash
config() { /usr/bin/git --git-dir="$HOME/.cfg/" --work-tree="$HOME" "$@"; }
config add .my-scripts/deps/deps.conf .my-scripts/deps/deps-local.conf \
  .my-scripts/deps/check-deps.sh tests/check-deps.test.sh
config commit -m "add a dependency manifest and check/install engine"
```

---

### Task 2: Shell-startup hook

**Files:**
- Create: `.my-scripts/deps/depcheck-hook.sh`
- Modify: `.zshrc` (append a two-line source at the end, after the existing NVM block)

**Interfaces:**
- Consumes: `.my-scripts/deps/check-deps.sh` (Task 1), called with no arguments (check-only) from the hook's nag check and `--fix` from the `depcheck` alias.
- Produces: nothing consumed by later tasks in this plan; this is a leaf.

The actual hook logic goes in its own file rather than being pasted into
`.zshrc` directly, for the same reason `tmux.conf`'s shared content moved
into `tmux-common.conf` in the branch-drift-sync work: `.zshrc` itself isn't
tracked identically between `mac` and `linux` (it has real
platform-conditional content), so anything pasted straight into it has no
drift protection and depends on remembering to hand-port every future edit
to both branches. `.my-scripts/deps/depcheck-hook.sh` sits inside a
directory `.sync-manifest` already covers wholesale, so it's automatically
byte-identical-checked on every push with no manifest edit needed. `.zshrc`
only gets a one-line `source`, on both branches — small and unlikely to
drift silently, matching the same accepted-risk boilerplate as the existing
`.zshrc-mac`/`.zshrc-linux` source lines.

Not testable through `tests/lib.sh` (sourcing the real `.zshrc` in a test
would run this repo's `git config --global` setup block against the real
machine's `.gitconfig`, which isn't safe to do from an automated test).
Verified manually instead, against a real new zsh process, in Step 4 below.

- [ ] **Step 1: Write `.my-scripts/deps/depcheck-hook.sh`**

```sh
# Sourced from .zshrc. Nags at most once every 24h if a CLI dependency from
# .my-scripts/deps/deps.conf is missing. Never blocks startup, never
# prompts -- see .my-scripts/deps/README.md for the manual `depcheck`
# command this also defines.

alias depcheck='~/.my-scripts/deps/check-deps.sh --fix'

_depcheck_cache="$HOME/.cache/depcheck-last-run"
_depcheck_last=0
if [ -f "$_depcheck_cache" ]; then
    _depcheck_last=$(cat "$_depcheck_cache" 2>/dev/null)
    case "$_depcheck_last" in
        ''|*[!0-9]*) _depcheck_last=0 ;;
    esac
fi

if [ $(($(date +%s) - _depcheck_last)) -ge 86400 ]; then
    mkdir -p "$HOME/.cache"
    date +%s > "$_depcheck_cache"
    if ! ~/.my-scripts/deps/check-deps.sh >/dev/null 2>&1; then
        echo "depcheck: missing dependencies detected -- run \`depcheck\` for details"
    fi
fi
unset _depcheck_cache _depcheck_last
```

- [ ] **Step 2: Source it from `.zshrc`**

Append at the end of `.zshrc`:

```sh

# DEPENDENCY CHECK ##########################################################
[ -f ~/.my-scripts/deps/depcheck-hook.sh ] && source ~/.my-scripts/deps/depcheck-hook.sh
```

- [ ] **Step 3: Confirm it doesn't break shell startup**

```bash
zsh -i -c 'echo shell started ok'
```

Expected: prints `shell started ok` with no errors above it (a syntax error in either new file would abort sourcing `.zshrc` before this line runs).

- [ ] **Step 4: Manually verify the cache/nag behavior against a real new shell**

```bash
rm -f ~/.cache/depcheck-last-run
zsh -i -c 'true'   # first run: cache is missing, so this is "stale" -- runs the real check
cat ~/.cache/depcheck-last-run   # expect: a unix timestamp was written
zsh -i -c 'true'   # second run: cache is fresh -- should NOT re-run the check
```

Since every dependency is currently installed on this machine (Task 1 Step 8 confirmed this), neither run should print the `depcheck: missing dependencies detected` line. To confirm the nag itself actually fires, temporarily point `DEPS_CONF` at a fixture with one always-missing entry:

```bash
printf 'always-missing-dep|command -v always-missing-dep|https://example.invalid\n' > /tmp/depcheck-verify.conf
rm -f ~/.cache/depcheck-last-run
DEPS_CONF=/tmp/depcheck-verify.conf zsh -i -c 'true'
```

This won't pick up `DEPS_CONF` inside the hook itself (the hook calls `check-deps.sh` with no override), so instead verify the nag line directly against the script:

```bash
DEPS_CONF=/tmp/depcheck-verify.conf ~/.my-scripts/deps/check-deps.sh >/dev/null 2>&1; echo "exit: $?"
```

Expected: `exit: 1` — confirms a real missing dependency produces the non-zero exit the hook's `if ! ...; then` branches on. Clean up:

```bash
rm -f /tmp/depcheck-verify.conf ~/.cache/depcheck-last-run
```

- [ ] **Step 5: Run the full suite (confirms this change didn't break anything test-covered)**

```bash
bash ~/tests/run-all.sh
```

- [ ] **Step 6: Commit**

```bash
config() { /usr/bin/git --git-dir="$HOME/.cfg/" --work-tree="$HOME" "$@"; }
config add .my-scripts/deps/depcheck-hook.sh .zshrc
config commit -m "add a cached dependency-check nag to shell startup"
```

---

### Task 3: Local Docker test harness

**Files:**
- Create: `.my-scripts/deps/docker/Dockerfile.ubuntu`
- Create: `.my-scripts/deps/docker/Dockerfile.arch`
- Create: `.my-scripts/deps/test-local.sh`

**Interfaces:**
- Consumes: `.my-scripts/deps/check-deps.sh`, `deps.conf`, `deps-local.conf` (Task 1), run as each image's entrypoint.
- Produces: `depcheck-ubuntu` / `depcheck-arch` Docker images, and `test-local.sh` as the command that builds and runs both. Task 4's `arch` CI job reuses `Dockerfile.arch` directly.

- [ ] **Step 1: Write `Dockerfile.ubuntu`**

```dockerfile
FROM ubuntu:22.04

RUN apt-get update && apt-get install -y --no-install-recommends \
        sudo curl git wget ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY . /dotfiles
WORKDIR /dotfiles

ENV HOME=/root
ENTRYPOINT ["/dotfiles/.my-scripts/deps/check-deps.sh"]
CMD ["--fix", "--yes"]
```

- [ ] **Step 2: Write `Dockerfile.arch`**

```dockerfile
FROM archlinux:base

RUN pacman -Sy --noconfirm sudo curl git wget

COPY . /dotfiles
WORKDIR /dotfiles

ENV HOME=/root
ENTRYPOINT ["/dotfiles/.my-scripts/deps/check-deps.sh"]
CMD ["--fix", "--yes"]
```

- [ ] **Step 3: Write `test-local.sh`**

```sh
#!/bin/sh
#
# Builds and runs check-deps.sh against fresh ubuntu and archlinux
# containers, so the manifest/engine can be iterated on without waiting on
# GitHub Actions. Mirrors .github/workflows/deps-check.yml's ubuntu/arch
# legs -- same Dockerfiles, same command.
#
# Builds from a clean `git archive` of the current branch, not $HOME
# directly -- $HOME holds plenty of untracked content that has no business
# in a Docker build context.
#
# Usage: ~/.my-scripts/deps/test-local.sh

set -eu

GD="$HOME/.cfg"
WT="$HOME"
BRANCH=$(git --git-dir="$GD" --work-tree="$WT" branch --show-current)

workdir=$(mktemp -d "${TMPDIR:-/tmp}/depcheck-docker-XXXXXX")
trap 'rm -rf "$workdir"' EXIT

git --git-dir="$GD" --work-tree="$WT" archive "$BRANCH" | tar -x -C "$workdir"

for image in ubuntu arch; do
    printf '\n=== %s ===\n' "$image"
    docker build -q \
        -f "$workdir/.my-scripts/deps/docker/Dockerfile.$image" \
        -t "depcheck-$image" \
        "$workdir"
    docker run --rm "depcheck-$image"
done
```

- [ ] **Step 4: Make it executable**

```bash
chmod +x ~/.my-scripts/deps/test-local.sh
```

- [ ] **Step 5: Run it and confirm both containers bootstrap cleanly**

```bash
~/.my-scripts/deps/test-local.sh
```

Expected: two sections (`=== ubuntu ===`, `=== arch ===`), each ending in `check-deps: no unresolved failures ...` and the container exiting 0. If `docker` isn't installed or the daemon isn't running, this step can't run on this machine — note that explicitly rather than skipping silently, and confirm it in Task 4's CI run instead.

- [ ] **Step 6: Commit**

```bash
config() { /usr/bin/git --git-dir="$HOME/.cfg/" --work-tree="$HOME" "$@"; }
config add .my-scripts/deps/docker/Dockerfile.ubuntu .my-scripts/deps/docker/Dockerfile.arch \
  .my-scripts/deps/test-local.sh
config commit -m "add a local Docker harness for check-deps.sh (ubuntu + arch)"
```

---

### Task 4: GitHub Action

**Files:**
- Create: `.github/workflows/deps-check.yml`
- Modify: `.sync-manifest`

**Interfaces:**
- Consumes: `.my-scripts/deps/check-deps.sh` (Task 1), `.my-scripts/deps/docker/Dockerfile.arch` (Task 3).
- Produces: a scheduled + manually-triggerable `deps-check` GitHub Actions workflow with three jobs (`ubuntu`, `macos`, `arch`); nothing else in this plan depends on it.

- [ ] **Step 1: Add the `deps-local.conf` exclusion to `.sync-manifest`**

Append this line (with the rest of the platform-exclusive exclusions):

```
!.my-scripts/deps/deps-local.conf
```

- [ ] **Step 2: Write the workflow**

```yaml
name: deps-check

on:
  schedule:
    - cron: '0 13 1,15 * *'
  workflow_dispatch:

jobs:
  ubuntu:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Bootstrap and check dependencies (apt)
        run: .my-scripts/deps/check-deps.sh --fix --yes

  macos:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - name: Bootstrap and check dependencies (brew)
        run: .my-scripts/deps/check-deps.sh --fix --yes

  arch:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build the Arch test image
        run: docker build -f .my-scripts/deps/docker/Dockerfile.arch -t depcheck-arch .
      - name: Bootstrap and check dependencies (pacman)
        run: docker run --rm depcheck-arch
```

- [ ] **Step 3: Validate the YAML parses**

```bash
python3 -c "import yaml, sys; yaml.safe_load(open('$HOME/.github/workflows/deps-check.yml'))" && echo "YAML OK"
```

Expected: `YAML OK`.

- [ ] **Step 4: Run the full suite**

```bash
bash ~/tests/run-all.sh
```

- [ ] **Step 5: Commit**

```bash
config() { /usr/bin/git --git-dir="$HOME/.cfg/" --work-tree="$HOME" "$@"; }
config add .github/workflows/deps-check.yml .sync-manifest
config commit -m "add a scheduled GitHub Action that bootstraps and checks dependencies"
```

---

### Task 5: Documentation

**Files:**
- Create: `.my-scripts/deps/README.md`
- Modify: `~/README.md`

**Interfaces:**
- Consumes: nothing new — describes Tasks 1-4.
- Produces: nothing consumed by later tasks.

Documentation, no test cycle. Content accuracy is the deliverable.

- [ ] **Step 1: Write `.my-scripts/deps/README.md`**

```markdown
# Dependency checking

Checks and (best-effort) installs the CLI dependencies listed in
`~/README.md`. See `docs/superpowers/specs/2026-09-03-dependency-checking-design.md`
for the full design rationale.

## Files

- `deps.conf` -- shared dependencies, tracked identically on `mac` and
  `linux` (part of `.sync-manifest`). One line per dependency:
  `name|check_command|docs_url`.
- `deps-local.conf` -- platform-exclusive dependencies (e.g. `xclip` on
  `linux`, `aerospace` on `mac`). Same format. Deliberately **excluded**
  from `.sync-manifest` (`!.my-scripts/deps/deps-local.conf`), so each
  branch carries its own without the drift-check flagging it.
- `check-deps.sh` -- the engine. Reads both files above.
- `depcheck-hook.sh` -- sourced from `.zshrc`; the cached, 24h-throttled
  startup nag plus the `depcheck` alias. Lives here (rather than being
  pasted into `.zshrc` directly) so it's covered by `.sync-manifest`'s
  existing `.my-scripts/` entry and can't silently drift between branches
  the way `.zshrc`'s own platform-conditional content does.
- `docker/Dockerfile.ubuntu`, `docker/Dockerfile.arch` -- minimal images
  used by both `test-local.sh` and the `arch` leg of
  `.github/workflows/deps-check.yml`.
- `test-local.sh` -- builds and runs both Docker images against the current
  branch, for iterating without waiting on CI.

## Adding a dependency

Add a line to `deps.conf` (or `deps-local.conf` for something
platform-exclusive): `name|check_command|docs_url`.

- `check_command` is any shell snippet that exits 0 when the dependency is
  present. Most are `command -v <binary>`; a few (`oh-my-zsh`,
  `zsh-autosuggestions`, `tpm`) check for a cloned directory/file instead,
  since they aren't binaries on `PATH`.
- The default install command is `<package manager> install <name>` --
  correct as long as the package name matches `name`. If it doesn't (or the
  dependency isn't installed via a package manager at all -- a `git clone`,
  a curl-to-shell script), add a case to `install_cmd_for()` in
  `check-deps.sh`. Leave the case empty (see `nvm`'s) for a dependency with
  no safe automated install -- it'll still be checked and reported, just
  never auto-installed.

## Running it

```bash
~/.my-scripts/deps/check-deps.sh              # check only
~/.my-scripts/deps/check-deps.sh --fix        # check + interactively install
depcheck                                      # alias for the line above
~/.my-scripts/deps/check-deps.sh --fix --dry-run  # preview what --fix would run
~/.my-scripts/deps/test-local.sh              # bootstrap fresh ubuntu + arch containers
```

A shell-startup hook in `.zshrc` runs the check-only path at most once every
24h and prints a one-line nag if anything's missing. It never installs
anything and never blocks startup.

## CI

`.github/workflows/deps-check.yml` runs `check-deps.sh --fix --yes` as a
real bootstrap every two weeks (plus on-demand via `workflow_dispatch`) on
`ubuntu-latest` (apt), `macos-latest` (brew), and an Arch container built
from `docker/Dockerfile.arch` (pacman) running on `ubuntu-latest`.
```

- [ ] **Step 2: Add a "Dependency checking" section to `~/README.md`**

Insert after the existing "Env setup" section (before "Claude Code
notifications"):

```markdown
### Dependency checking

The dependencies above (plus a few platform-exclusive ones) are also
tracked in a checkable manifest: `.my-scripts/deps/deps.conf` /
`deps-local.conf`. `~/.my-scripts/deps/check-deps.sh` checks them, and
`depcheck` (a shell alias) checks and offers to install anything missing. A
shell-startup hook nags once a day if something's gone missing. See
`.my-scripts/deps/README.md` for the details, and
`docs/superpowers/specs/2026-09-03-dependency-checking-design.md` for the
design.
```

- [ ] **Step 3: Run the full suite**

```bash
bash ~/tests/run-all.sh
```

- [ ] **Step 4: Commit**

```bash
config() { /usr/bin/git --git-dir="$HOME/.cfg/" --work-tree="$HOME" "$@"; }
config add .my-scripts/deps/README.md README.md
config commit -m "document the dependency-checking setup"
```

---

### Task 6: Push `linux`

**Files:** none (verification task).

**Interfaces:**
- Consumes: all commits from Tasks 1-5.
- Produces: an updated `origin/linux`, which Task 7 diffs against when building `mac`'s mirror.

- [ ] **Step 1: Confirm the remote hasn't moved and the push is a clean fast-forward**

```bash
config() { /usr/bin/git --git-dir="$HOME/.cfg/" --work-tree="$HOME" "$@"; }
config fetch origin linux
config merge-base --is-ancestor origin/linux linux && echo "safe fast-forward"
```

Expected: `safe fast-forward`.

- [ ] **Step 2: Push**

```bash
config push origin linux
```

- [ ] **Step 3: Confirm the branch-drift Action still passes (this branch's manifest changed)**

```bash
gh run list --branch linux --workflow branch-drift.yml --limit 1
```

Expected: shows a run, but it will only be meaningful once `mac` has the same manifest change (Task 7) — a mismatch here before Task 7 lands is expected, not a regression.

---

### Task 7: Mirror to `mac`, push, confirm

**Files (on the `mac` branch):**
- Create: `.my-scripts/deps/deps.conf`, `check-deps.sh`, `depcheck-hook.sh`, `README.md`, `docker/Dockerfile.ubuntu`, `docker/Dockerfile.arch`, `test-local.sh` (identical to `linux`)
- Create: `.my-scripts/deps/deps-local.conf` (mac-specific — **not** a copy of linux's)
- Create: `tests/check-deps.test.sh` (identical to `linux`)
- Create: `.github/workflows/deps-check.yml` (identical to `linux`)
- Modify: `.sync-manifest` (identical to `linux`)
- Modify: `~/README.md` (the new section, identical to `linux`; the rest of the file keeps `mac`'s existing content)
- Modify: `.zshrc` (the same two-line source addition as Task 2 Step 2 — `.zshrc` isn't in `.sync-manifest`, so this one line is a manual port, not a `git checkout linux --`; the actual logic being sourced, `depcheck-hook.sh`, *is* ported verbatim via `git checkout linux --` like everything else)

**Interfaces:**
- Consumes: every file produced by Tasks 1-5, verbatim except `deps-local.conf`.
- Produces: an `origin/mac` that the branch-drift check considers identical to `origin/linux` on every manifest path.

- [ ] **Step 1: Switch to `mac` and pull its latest**

```bash
config() { /usr/bin/git --git-dir="$HOME/.cfg/" --work-tree="$HOME" "$@"; }
config checkout mac
config pull --ff-only origin mac
```

- [ ] **Step 2: Port every shared file from `linux`**

```bash
config checkout linux -- \
  .my-scripts/deps/deps.conf \
  .my-scripts/deps/check-deps.sh \
  .my-scripts/deps/depcheck-hook.sh \
  .my-scripts/deps/README.md \
  .my-scripts/deps/docker/Dockerfile.ubuntu \
  .my-scripts/deps/docker/Dockerfile.arch \
  .my-scripts/deps/test-local.sh \
  tests/check-deps.test.sh \
  .github/workflows/deps-check.yml \
  .sync-manifest
```

- [ ] **Step 3: Write `mac`'s own `deps-local.conf`**

```
# mac-exclusive CLI dependencies. Excluded from .sync-manifest so linux can
# carry its own unrelated version of this file. Format matches deps.conf.
#
# aerospace powers this branch's Claude Code Stop-hook notification
# (~/.claude/hooks/notify.sh) -- see ~/README.md's "Claude Code
# notifications" section.

aerospace|command -v aerospace|https://github.com/nikitabobko/AeroSpace
```

- [ ] **Step 4: Add the README section**

Read `mac`'s current `~/README.md`:

```bash
config show mac:README.md
```

Insert the same "Dependency checking" section written in Task 5 Step 2,
in the equivalent location (after "Env setup", before "Claude Code
notifications" — confirm that section ordering still matches; `mac`'s copy
may have diverged since the two READMEs aren't in `.sync-manifest`).

- [ ] **Step 5: Source the hook from `mac`'s `.zshrc`**

Append the same two lines as Task 2 Step 2:

```sh

# DEPENDENCY CHECK ##########################################################
[ -f ~/.my-scripts/deps/depcheck-hook.sh ] && source ~/.my-scripts/deps/depcheck-hook.sh
```

This is the one hand-ported line in this task — everything it sources
(`depcheck-hook.sh`) came across verbatim in Step 2, already verified on
`linux`.

- [ ] **Step 6: This machine can't run mac's test suite or Docker harness**

`tests/check-deps.test.sh` and `.my-scripts/deps/test-local.sh` are
platform-neutral shell/Docker logic already verified green on `linux` in
Tasks 1 and 3 — mirrored here unchanged. Note as a follow-up for Austin to
run `bash ~/tests/run-all.sh` and `~/.my-scripts/deps/check-deps.sh` on the
mac machine next time he's on it, to confirm `deps-local.conf`'s `aerospace`
check and the brew install paths (`install_cmd_for`'s `alacritty`/`gh`/
default-case `brew` branches) actually work there — those are the one part
of this plan no Linux machine can verify directly.

- [ ] **Step 7: Verify the manifest paths are identical between the two branches, locally**

```bash
config fetch origin linux mac
DOTFILES_ROOT="$HOME" bash -c '
  cd "$HOME"
  export GIT_DIR="$HOME/.cfg"
  export GIT_WORK_TREE="$HOME"
  tests/check-branch-drift.sh mac linux
'
```

Expected: exits 0, `check-branch-drift: mac and linux match on all N shared
path(s)` (comparing the local, not-yet-pushed `mac` branch against `linux`).
If this fails, the diverged-path list names exactly what's still out of
sync — fix it before committing.

- [ ] **Step 8: Commit on `mac`**

```bash
config add .my-scripts/deps/deps.conf .my-scripts/deps/deps-local.conf \
  .my-scripts/deps/check-deps.sh .my-scripts/deps/depcheck-hook.sh .my-scripts/deps/README.md \
  .my-scripts/deps/docker/Dockerfile.ubuntu .my-scripts/deps/docker/Dockerfile.arch \
  .my-scripts/deps/test-local.sh tests/check-deps.test.sh \
  .github/workflows/deps-check.yml .sync-manifest README.md .zshrc
config commit -m "merge in linux changes - dependency manifest, check/install engine, Docker harness, GitHub Action"
```

- [ ] **Step 9: Push `mac`**

```bash
config fetch origin mac
config merge-base --is-ancestor origin/mac mac && echo "safe fast-forward"
config push origin mac
```

- [ ] **Step 10: Confirm both branch-drift and deps-check ran and passed**

```bash
gh run list --branch linux --workflow branch-drift.yml --limit 1
gh run list --branch mac --workflow branch-drift.yml --limit 1
gh run list --branch linux --workflow deps-check.yml --limit 1
gh run list --branch mac --workflow deps-check.yml --limit 1
```

`deps-check.yml` only runs on a schedule or `workflow_dispatch`, not on
push, so the last two may show nothing yet — trigger it manually to confirm
sooner:

```bash
gh workflow run deps-check.yml --ref linux
gh workflow run deps-check.yml --ref mac
```

Expected (after triggering): all four show a recent run with conclusion
`success`. If `gh` isn't authenticated in this environment, report the
branches' latest commit SHAs to Austin and ask him to confirm the Actions
tab shows green — same caveat as the branch-drift plan's precedent.

---

## Self-review notes

- Spec coverage: all six spec design sections (manifest, engine,
  shell-startup hook, Docker harness, GitHub Action, documentation) map to
  Tasks 1-5; the spec's rollout order maps to Tasks 1-7.
- No placeholders: every script, test, Dockerfile, and doc above is
  complete, real content.
- Type/name consistency checked: `check-deps.sh`'s `--fix`/`--yes`/
  `--dry-run` flags and exit-code contract (stated in Task 1's Interfaces
  block) match every later task that invokes it (the `.zshrc` hook calls it
  with no flags; `depcheck` and the Dockerfiles' `CMD` call it with
  `--fix`/`--fix --yes`); `DEPS_CONF`/`DEPS_LOCAL_CONF` env-var names match
  between the script and the test file.
- One correction made while writing this plan, not caught during
  brainstorming: the original `--fix` design (spec §2) implied any
  still-missing dependency after `--fix` would fail the exit code. Applied
  literally, `nvm` — deliberately manual-only by design — would make every
  `--fix --yes` CI run fail forever with no way to succeed. Task 1's engine
  instead only counts a dependency against the exit code if an automated
  install was actually attempted and still didn't satisfy its check;
  manual-only dependencies and user-declined prompts are reported but never
  fail `--fix`. Documented in `check-deps.sh`'s own header comment so this
  doesn't need re-deriving from the spec.
- Task 3 Step 5 and Task 7 Step 6/10 are the three places this plan cannot
  fully self-verify from this machine (no Docker daemon confirmed available
  here, no mac hardware, no confirmed `gh` auth) — flagged explicitly rather
  than asserted as done, matching the branch-drift-sync plan's precedent.
- Drift-prevention check requested after the initial draft: the
  shell-startup hook was originally going to be pasted directly into
  `.zshrc`, which isn't in `.sync-manifest` and would have needed manual,
  unchecked re-porting on every future edit — the same problem the
  branch-drift-sync plan explicitly solved for `tmux.conf`. Fixed by moving
  the hook logic into `.my-scripts/deps/depcheck-hook.sh` (Task 2), which
  `.sync-manifest`'s existing `.my-scripts/` entry already covers with no
  manifest edit needed. `.zshrc` and `~/README.md` still carry small
  hand-ported pieces (a one-line `source`, a documentation section) since
  neither file is tracked identically between branches at all — extracting
  those two files wholesale is out of scope here (see the spec's
  Non-goals), so this plan accepts the same low-risk, boilerplate-level
  duplication those files already carry for their `mac`/`linux` overlay
  sourcing lines.
