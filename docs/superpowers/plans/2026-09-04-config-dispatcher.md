# `config` Dispatcher and Utilities Implementation Plan (Plan C)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the per-branch `config` git alias with a tracked POSIX `config` dispatcher that runs `config-<sub>` siblings when present and passes everything else through to the bare-repo git, and ship the utilities behind it: `install-hooks`, `check`, `sync`, `test`, `install`, `reload`.

**Architecture:** `.scripts/config/config` resolves its own directory, execs `<dir>/config-<sub>` if that is executable, else execs `git --git-dir="$HOME/.cfg" --work-tree="$HOME" "$@"`. Every `config-<sub>` is a small POSIX sh script that derives every path from `$HOME`, so one tracked copy serves both branches and every test can isolate itself by pointing `HOME` at a fixture. `config-check` and `config-sync` are one-line execs into the `config-manifest` binary, so the dispatcher is the single front door and the Rust binary stays a build artifact. `install-hooks` is the only writer and checks the permission trust boundary before it writes.

**Tech Stack:** POSIX sh (`#!/bin/sh`), the existing bash test harness (`tests/lib.sh`: `assert_equals`, `assert_contains`, `assert_succeeds`, `finish`, `FIXTURES`, `DOTFILES_ROOT`), `shellcheck` if present. `config-manifest` from Plan B (already on PATH on the host, in Docker, and in CI).

**Spec:** `docs/superpowers/specs/2026-09-04-config-command-and-manifest-crate-design.md`, sections 5, 7 (exit codes), 7.1 through 7.7, 9.5, 10 step 4. Plan B2 (`docs/superpowers/plans/2026-09-04-config-manifest-sync.md`) delivered `config-manifest sync`.

## Global Constraints

- Every script is `#!/bin/sh` POSIX: no arrays, no `[[`, no `local` beyond what dash accepts (dash accepts `local`), no `-O` test operator, no `readlink -f` except in the dispatcher (present on Darwin 25 and Linux), no `stat` (its flags differ between BSD and GNU; use `find -perm` and `find -user`).
- The dispatcher is under 40 lines. Name rules: a `config-<sub>` must not shadow a git command; the set of `config-<sub>` names equals an allowlist in the test.
- All paths derive from `$HOME`. No script hardcodes `/Users/austin`. Tests isolate by setting `HOME` to a fixture directory under `$FIXTURES`; no test writes to the real `$HOME`, real `.cfg/hooks`, or real `~/.local/bin`, except Task 2's one deliberate real installation step run by the controller-directed implementer and stated in the report.
- `install-hooks` refuses (exit 1, message on stderr) when `$HOME/.local/bin` or the dispatcher's directory is group- or world-writable or not owned by the invoking user. It is idempotent (`ln -sfn`).
- Exit codes: 0 success, 1 refusal or failing wrapped command (pass the wrapped command's status through), 2 usage error. Diagnostics to stderr, results to stdout.
- Never push, never checkout, never `--no-verify`. The pre-commit leak check must pass on every commit.
- New executables must be added to `EXECUTED_SCRIPTS` and `expected_exec` in `tests/scripts-dir-name.test.sh`, with the committed count updated, and must be committed with mode `100755`.
- Shell suites: host `tests/run-all.sh` must pass; Docker `tests/run-in-docker.sh` must pass (the container has no `.cfg`, so every new test runs on fixtures).
- No em dashes, no emoji, no single-letter identifiers, comments only where the why is non-obvious.
- Shared paths changed here (`.scripts/`, `tests/`, `.zshrc`, `README.md`, `DOTFILES.md`, `.claude/rules/dotfiles-tests.md`) are synced to `linux` with `config sync` in Task 5. Per-branch files (`.zshrc-mac`, `.zshrc-linux`, `TODO-AGENTS.md`, `docs/superpowers/`) are edited on their own branch.

---

## File Structure

| File | Responsibility |
|---|---|
| `.scripts/config/config` (create, 100755) | dispatcher: sibling `config-<sub>` or git passthrough |
| `.scripts/config/config-install-hooks` (create, 100755) | permission checks, three symlinks, idempotent |
| `.scripts/config/config-check` (create, 100755) | `exec config-manifest check "$@"` |
| `.scripts/config/config-sync` (create, 100755) | `exec config-manifest sync "$@"` |
| `.scripts/config/config-test` (create, 100755) | wraps `run-all.sh` / `run-in-docker.sh`, `--watch` poll |
| `.scripts/config/config-install` (create, 100755) | wraps `check-deps.sh --fix` |
| `.scripts/config/config-reload` (create, 100755) | `tmux source` when in tmux, prints the zsh line |
| `tests/config.test.sh` (create) | dispatcher and utilities suite on fixture HOMEs |
| `tests/scripts-dir-name.test.sh` (modify) | allowlists and committed count |
| `.zshrc` (modify) | remove the per-shell `tmux source` at line 174 |
| `.zshrc-mac` (modify, per-branch) | remove the `config` alias at line 2 |
| `.zshrc-linux` (modify on linux, per-branch) | remove the `config` alias at line 2 |
| `DOTFILES.md`, `README.md`, `.claude/rules/dotfiles-tests.md` (modify) | bootstrap and hook docs point at `config install-hooks` |
| `TODO-AGENTS.md` (modify, per-branch) | close the `config:*` and `tmux source` items |

---

### Task 1: The dispatcher and its test suite

**Files:**
- Create: `.scripts/config/config`
- Create: `tests/config.test.sh`
- Modify: `tests/scripts-dir-name.test.sh` (allowlists, count)

**Interfaces:**
- Produces: `config <sub> [args...]`. `<dir>/config-<sub>` executable → `exec` it with the remaining args. Otherwise `exec git --git-dir="$HOME/.cfg" --work-tree="$HOME" "$@"`. With no arguments, passes through to git (which prints git's usage, exit 1).
- Produces for later tasks: the allowlist constant in `tests/config.test.sh` named `EXPECTED_SUBCOMMANDS`, which Tasks 2 through 4 extend.

- [ ] **Step 1: Write the failing test suite**

Create `tests/config.test.sh`:

```bash
#!/bin/bash
#
# Tests the `config` dispatcher and the config-<sub> utilities beside it.
# Every test runs against a fixture HOME under $FIXTURES: a bare repo at
# .cfg with a worktree, so nothing here touches the real dotfiles repo.
#
# Usage: ~/tests/config.test.sh

. "$(dirname "$0")/lib.sh"

CONFIG_DIR="$DOTFILES_ROOT/.scripts/config"
CONFIG="$CONFIG_DIR/config"

EXPECTED_SUBCOMMANDS='build stamp'

make_fixture_home() {
    fixture_home="$FIXTURES/home-$1"
    mkdir -p "$fixture_home"
    seed=$(make_repo "seed-$1")
    git clone -q --bare "$seed" "$fixture_home/.cfg"
    git --git-dir="$fixture_home/.cfg" config status.showUntrackedFiles no
    printf '%s\n' "$fixture_home"
}

# --- passthrough ------------------------------------------------------------

home=$(make_fixture_home passthrough)
expected=$(git --git-dir="$home/.cfg" --work-tree="$home" rev-parse HEAD)
actual=$(HOME="$home" "$CONFIG" rev-parse HEAD)
assert_equals 'config rev-parse HEAD passes through to the bare repo' \
    "$expected" "$actual"

actual=$(HOME="$home" "$CONFIG" status --porcelain --untracked-files=no 2>&1)
assert_equals 'config status passes through with the fixture worktree' '' "$actual"

output=$(HOME="$home" "$CONFIG" definitely-not-a-command 2>&1)
status=$?
assert_equals 'an unknown subcommand falls through to git and fails' '1' "$status"
assert_contains 'the failure is git own message' \
    "git: 'definitely-not-a-command' is not a git command" "$output"

output=$(HOME="$home" "$CONFIG" 2>&1)
status=$?
assert_equals 'no arguments passes through to git usage' '1' "$status"
assert_contains 'git usage is what prints' 'usage: git' "$output"

# --- name rules -------------------------------------------------------------

actual=$(cd "$CONFIG_DIR" && ls config-* 2>/dev/null | sed 's/^config-//' | sort | tr '\n' ' ' | sed 's/ $//')
expected=$(printf '%s\n' $EXPECTED_SUBCOMMANDS | sort | tr '\n' ' ' | sed 's/ $//')
assert_equals 'the config-<sub> set equals the allowlist' "$expected" "$actual"

not_executable=''
for sub in $EXPECTED_SUBCOMMANDS; do
    [ -x "$CONFIG_DIR/config-$sub" ] || not_executable="$not_executable $sub"
done
assert_equals 'every listed subcommand is executable' '' "$not_executable"

git_commands=$(git --list-cmds=main,others)
shadowing=''
for sub in $EXPECTED_SUBCOMMANDS; do
    printf '%s\n' "$git_commands" | grep -qx "$sub" && shadowing="$shadowing $sub"
done
assert_equals 'no config-<sub> shadows a git command' '' "$shadowing"

line_count=$(grep -c '' "$CONFIG")
[ "$line_count" -lt 40 ] && under_40=yes || under_40="no ($line_count lines)"
assert_equals 'the dispatcher stays under 40 lines' 'yes' "$under_40"

# --- sibling dispatch -------------------------------------------------------

home=$(make_fixture_home sibling)
sibling_dir="$FIXTURES/sibling-bin"
mkdir -p "$sibling_dir"
cp "$CONFIG" "$sibling_dir/config"
printf '#!/bin/sh\nprintf "probe:%%s\\n" "$@"\n' > "$sibling_dir/config-probe"
chmod 755 "$sibling_dir/config-probe"
actual=$(HOME="$home" "$sibling_dir/config" probe one "two words")
assert_equals 'a sibling config-<sub> receives the remaining args intact' \
    'probe:one
probe:two words' "$actual"

link_dir="$FIXTURES/linked-bin"
mkdir -p "$link_dir"
ln -s "$sibling_dir/config" "$link_dir/config"
actual=$(HOME="$home" "$link_dir/config" probe via-symlink)
assert_equals 'the dispatcher resolves siblings through its own symlink' \
    'probe:via-symlink' "$actual"

finish
```

Make it executable: `chmod 755 tests/config.test.sh`. Run: `bash tests/config.test.sh` Expected: fails at the first assertion because `$CONFIG` does not exist (bash prints "No such file or directory" and the passthrough assertion fails). RED.

Note for the implementer: `make_repo NAME [BRANCH]` in `tests/lib.sh` creates `$FIXTURES/NAME` with one empty commit and prints its path; the fixture clones it bare into `<home>/.cfg`, the same shape as the real dotfiles repo (a bare repo driven with `--git-dir` and `--work-tree`, `status.showUntrackedFiles=no` because the worktree is a whole home directory). The passthrough assertion compares against a `git --git-dir --work-tree` call on the same fixture.

- [ ] **Step 2: Write the dispatcher**

Create `.scripts/config/config`:

```sh
#!/bin/sh
# Front door for the dotfiles repo. `config <sub>` runs the sibling
# config-<sub> when one exists, so new tooling lands beside this file, and
# passes everything else to git on the bare repo, so every git verb keeps
# working exactly as the old alias did.
set -eu

self=$(readlink -f "$0")
here=$(dirname "$self")

if [ "$#" -gt 0 ] && [ -x "$here/config-$1" ]; then
    sub=$1
    shift
    exec "$here/config-$sub" "$@"
fi

exec git --git-dir="$HOME/.cfg" --work-tree="$HOME" "$@"
```

`chmod 755 .scripts/config/config`. Run: `bash tests/config.test.sh` Expected: all assertions pass (13 passed).

- [ ] **Step 3: Update the scripts allowlists**

In `tests/scripts-dir-name.test.sh`: add `config/config` to `EXECUTED_SCRIPTS`; add `"$NEW_NAME/config/config"` to `expected_exec`; bump the committed count assertion from `'17'` to `'18'`. Run: `bash tests/scripts-dir-name.test.sh` Expected: passes. Run `shellcheck .scripts/config/config tests/config.test.sh` if `shellcheck` is installed (report whether it was).

- [ ] **Step 4: Commit**

```bash
cd /Users/austin
g() { /usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin "$@"; }
g add .scripts/config/config tests/config.test.sh tests/scripts-dir-name.test.sh
g commit -F - <<'EOF'
Add the config dispatcher

`config <sub>` runs the sibling config-<sub> when one exists and passes
everything else to git on the bare repo, which is what the old per-branch
alias did. All paths derive from $HOME, so one tracked copy serves both
branches and the tests isolate themselves by pointing HOME at a fixture
bare repo.

The test suite pins the name rules from the spec: the set of config-<sub>
files equals an allowlist, none shadows a git command, and the dispatcher
stays under 40 lines. The alias itself is removed in the next change, once
install-hooks has put the dispatcher on PATH.
EOF
```

---

### Task 2: `install-hooks`, the real installation, and the alias removal

**Files:**
- Create: `.scripts/config/config-install-hooks`
- Modify: `tests/config.test.sh` (allowlist + tests), `tests/scripts-dir-name.test.sh`
- Modify: `.zshrc-mac` (delete line 2, the alias), `.claude/rules/dotfiles-tests.md` (lines 121-122 area), `DOTFILES.md` (bootstrap)

**Interfaces:**
- Produces: `config install-hooks`. Checks `$HOME/.local/bin` (created if missing) and the dispatcher's directory: refuse with exit 1 if either is group- or world-writable or not owned by the invoking user. Then `ln -sfn "$HOME/tests/pre-commit" "$HOME/.cfg/hooks/pre-commit"`, same for `pre-push`, and `ln -sfn <dispatcher dir>/config "$HOME/.local/bin/config"`. Prints one line per link on stdout. Idempotent.

- [ ] **Step 1: Write the failing tests**

Append to `tests/config.test.sh` before `finish`, and change `EXPECTED_SUBCOMMANDS` to `'build stamp install-hooks'`:

```bash
# --- install-hooks ----------------------------------------------------------

home=$(make_fixture_home hooks)
mkdir -p "$home/tests" "$home/.cfg/hooks"
printf '#!/bin/sh\nexit 0\n' > "$home/tests/pre-commit"
printf '#!/bin/sh\nexit 0\n' > "$home/tests/pre-push"
chmod 755 "$home/tests/pre-commit" "$home/tests/pre-push"
install_dir="$FIXTURES/install-bin"
mkdir -p "$install_dir"
cp "$CONFIG" "$install_dir/config"
cp "$CONFIG_DIR/config-install-hooks" "$install_dir/config-install-hooks"
chmod 755 "$install_dir/config" "$install_dir/config-install-hooks"

output=$(HOME="$home" "$install_dir/config" install-hooks 2>&1)
status=$?
assert_equals 'install-hooks succeeds on a clean fixture' '0' "$status"
assert_equals 'pre-commit is linked' "$home/tests/pre-commit" "$(readlink "$home/.cfg/hooks/pre-commit")"
assert_equals 'pre-push is linked' "$home/tests/pre-push" "$(readlink "$home/.cfg/hooks/pre-push")"
assert_equals 'the dispatcher is linked into ~/.local/bin' "$install_dir/config" "$(readlink "$home/.local/bin/config")"

output_again=$(HOME="$home" "$install_dir/config" install-hooks 2>&1)
status=$?
assert_equals 'a second install-hooks succeeds' '0' "$status"
assert_equals 'a second install-hooks prints the same report' "$output" "$output_again"

chmod o+w "$home/.local/bin"
output=$(HOME="$home" "$install_dir/config" install-hooks 2>&1)
status=$?
assert_equals 'install-hooks refuses a world-writable ~/.local/bin' '1' "$status"
assert_contains 'the refusal names the directory' "$home/.local/bin" "$output"
chmod o-w "$home/.local/bin"

chmod g+w "$install_dir"
output=$(HOME="$home" "$install_dir/config" install-hooks 2>&1)
status=$?
assert_equals 'install-hooks refuses a group-writable dispatcher directory' '1' "$status"
chmod g-w "$install_dir"
```

Run: `bash tests/config.test.sh` Expected: the allowlist assertion fails (`install-hooks` listed but no file) and the install-hooks block fails (`cp` of a missing file, then exit code assertions). RED.

- [ ] **Step 2: Write `config-install-hooks`**

```sh
#!/bin/sh
# The dispatcher execs whatever sits beside it and ~/.local/bin is on PATH,
# so those two directories are the trust boundary. Anyone who can write to
# them can run code as this user; refuse to install into them unless they
# are owned by this user and writable by nobody else.
set -eu

self=$(readlink -f "$0")
here=$(dirname "$self")
bin_dir="$HOME/.local/bin"
hooks_dir="$HOME/.cfg/hooks"

mkdir -p "$bin_dir"

check_dir() {
    dir=$1
    if [ -n "$(find "$dir" -maxdepth 0 -perm -g+w -o -maxdepth 0 -perm -o+w)" ]; then
        printf 'config install-hooks: refusing, %s is group- or world-writable\n' "$dir" >&2
        exit 1
    fi
    if [ -z "$(find "$dir" -maxdepth 0 -user "$(id -un)")" ]; then
        printf 'config install-hooks: refusing, %s is not owned by %s\n' "$dir" "$(id -un)" >&2
        exit 1
    fi
}

check_dir "$bin_dir"
check_dir "$here"

if [ ! -d "$hooks_dir" ]; then
    printf 'config install-hooks: no hooks directory at %s (is %s/.cfg the bare repo?)\n' "$hooks_dir" "$HOME" >&2
    exit 1
fi

link() {
    ln -sfn "$1" "$2"
    printf '%s -> %s\n' "$2" "$1"
}

link "$HOME/tests/pre-commit" "$hooks_dir/pre-commit"
link "$HOME/tests/pre-push" "$hooks_dir/pre-push"
link "$here/config" "$bin_dir/config"
```

`chmod 755 .scripts/config/config-install-hooks`. Note on the `find` expression: `-perm -g+w -o -perm -o+w` matches when either bit is set; `-maxdepth 0` limits it to the directory itself. Both BSD and GNU find accept symbolic `-perm` modes and `-maxdepth`. If `find` on Darwin rejects the second `-maxdepth`, use `find "$dir" -maxdepth 0 \( -perm -g+w -o -perm -o+w \)` instead; the test suite decides.

Run: `bash tests/config.test.sh` Expected: all pass (13 + 10 = 23). Update `tests/scripts-dir-name.test.sh`: add `config/config-install-hooks` to both lists, count `'18'` → `'19'`. Run it.

- [ ] **Step 3: Install for real, then remove the alias**

This is the one step that writes outside fixtures, deliberately:

```bash
~/.scripts/config/config install-hooks
ls -l ~/.local/bin/config ~/.cfg/hooks/pre-commit ~/.cfg/hooks/pre-push
command -v config       # must print /Users/austin/.local/bin/config in a fresh sh
sh -c 'config rev-parse --short HEAD'
```

Then delete line 2 of `.zshrc-mac` (the `alias config=...` line; confirm with `sed -n '2p' .zshrc-mac` first, then `sed -i '' '2d' .zshrc-mac` on Darwin). Run `bash tests/githooks-installed.test.sh` (still passes: the symlinks are what it checks).

- [ ] **Step 4: Docs**

In `.claude/rules/dotfiles-tests.md`, where the two `ln -sf` lines appear (around lines 121-122), replace the pair with a one-liner: `~/.scripts/config/config install-hooks` and one sentence that it also links the dispatcher into `~/.local/bin`. In `DOTFILES.md`, after the bootstrap step that defines the alias, add: "On a machine that has this repo's tracked files checked out, run `~/.scripts/config/config install-hooks` instead of defining the alias; it links `config` into `~/.local/bin` and installs the git hooks." Do not remove the historical alias lines from `DOTFILES.md`: they document the bootstrap that runs before the tracked files exist. Run `bash tests/doc-links.test.sh`.

- [ ] **Step 5: Commit**

```bash
cd /Users/austin
g() { /usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin "$@"; }
g add .scripts/config/config-install-hooks tests/config.test.sh tests/scripts-dir-name.test.sh .zshrc-mac .claude/rules/dotfiles-tests.md DOTFILES.md
g commit -F - <<'EOF'
Add config install-hooks and retire the config alias on mac

install-hooks links the two git hooks into .cfg/hooks and the dispatcher
into ~/.local/bin, idempotently. Before writing it refuses if ~/.local/bin
or the dispatcher's own directory is group- or world-writable or owned by
someone else: the dispatcher execs whatever sits beside it, so those two
directories are the trust boundary.

With the dispatcher on PATH the per-branch alias in .zshrc-mac is gone; the
linux alias goes when this change is synced. The bootstrap doc keeps the
alias for the step that runs before the tracked files exist.
EOF
```

---

### Task 3: `check`, `sync`, `install`, `test`

**Files:**
- Create: `.scripts/config/config-check`, `config-sync`, `config-install`, `config-test`
- Modify: `tests/config.test.sh` (allowlist + tests), `tests/scripts-dir-name.test.sh`

**Interfaces:**
- `config check [args]` → `exec config-manifest check "$@"`.
- `config sync [args]` → `exec config-manifest sync "$@"`.
- `config install [--yes] [--dry-run]` → `exec "$HOME/.scripts/deps/check-deps.sh" --fix "$@"`.
- `config test [-q] [--docker] [--watch] [suite]`: without `--docker`, `exec "$HOME/tests/run-all.sh" ${quiet:+-q}`; with `--docker`, `exec "$HOME/tests/run-in-docker.sh" ${suite:+"$suite"}`; `--watch` loops: run once, then every second recompute a fingerprint over tracked files and rerun when it changes. Unknown flag → usage on stderr, exit 2.

- [ ] **Step 1: Write the failing tests**

Change `EXPECTED_SUBCOMMANDS` to `'build stamp install-hooks check sync install test'`. Append before `finish`:

```bash
# --- thin wrappers ----------------------------------------------------------

shim_dir="$FIXTURES/shims"
mkdir -p "$shim_dir"
printf '#!/bin/sh\nprintf "manifest:%%s\\n" "$@"\n' > "$shim_dir/config-manifest"
chmod 755 "$shim_dir/config-manifest"

actual=$(PATH="$shim_dir:$PATH" "$CONFIG" check origin/mac origin/linux)
assert_equals 'config check execs config-manifest check with its args' \
    'manifest:check
manifest:origin/mac
manifest:origin/linux' "$actual"

actual=$(PATH="$shim_dir:$PATH" "$CONFIG" sync --dry-run --to linux)
assert_equals 'config sync execs config-manifest sync with its args' \
    'manifest:sync
manifest:--dry-run
manifest:--to
manifest:linux' "$actual"

home=$(make_fixture_home wrappers)
mkdir -p "$home/.scripts/deps" "$home/tests"
printf '#!/bin/sh\nprintf "deps:%%s\\n" "$@"\n' > "$home/.scripts/deps/check-deps.sh"
printf '#!/bin/sh\nprintf "all:%%s\\n" "$@"; [ "$#" -eq 0 ] && printf "all:(none)\\n"\n' > "$home/tests/run-all.sh"
printf '#!/bin/sh\nprintf "docker:%%s\\n" "$@"; [ "$#" -eq 0 ] && printf "docker:(none)\\n"\n' > "$home/tests/run-in-docker.sh"
chmod 755 "$home/.scripts/deps/check-deps.sh" "$home/tests/run-all.sh" "$home/tests/run-in-docker.sh"

actual=$(HOME="$home" "$CONFIG" install --yes --dry-run)
assert_equals 'config install wraps check-deps --fix and passes flags through' \
    'deps:--fix
deps:--yes
deps:--dry-run' "$actual"

actual=$(HOME="$home" "$CONFIG" test)
assert_equals 'config test runs the host suite' 'all:(none)' "$actual"

actual=$(HOME="$home" "$CONFIG" test -q)
assert_equals 'config test -q passes -q through' 'all:-q' "$actual"

actual=$(HOME="$home" "$CONFIG" test --docker)
assert_equals 'config test --docker runs the whole suite in docker' 'docker:(none)' "$actual"

actual=$(HOME="$home" "$CONFIG" test --docker check-deps)
assert_equals 'config test --docker <suite> passes the suite name' 'docker:check-deps' "$actual"

output=$(HOME="$home" "$CONFIG" test --bogus 2>&1)
status=$?
assert_equals 'config test rejects an unknown flag with exit 2' '2' "$status"
assert_contains 'the rejection prints usage' 'usage: config test' "$output"

watch_home=$(make_fixture_home watch)
mkdir -p "$watch_home/tests"
printf '#!/bin/sh\nprintf "run\\n" >> "%s/runs"\n' "$watch_home" > "$watch_home/tests/run-all.sh"
chmod 755 "$watch_home/tests/run-all.sh"
( HOME="$watch_home" "$CONFIG" test --watch >/dev/null 2>&1 & echo $! > "$watch_home/watch.pid" )
sleep 2
runs_before=$(grep -c '' "$watch_home/runs" 2>/dev/null || echo 0)
printf 'changed\n' >> "$watch_home/tracked.txt"
git --git-dir="$watch_home/.cfg" --work-tree="$watch_home" add tracked.txt
sleep 3
runs_after=$(grep -c '' "$watch_home/runs" 2>/dev/null || echo 0)
kill "$(cat "$watch_home/watch.pid")" 2>/dev/null || true
assert_equals 'watch runs the suite once at start' '1' "$runs_before"
[ "$runs_after" -gt "$runs_before" ] && reran=yes || reran="no ($runs_before -> $runs_after)"
assert_equals 'watch reruns the suite when a tracked file changes' 'yes' "$reran"
```

Run: `bash tests/config.test.sh` Expected: the allowlist assertion and every wrapper assertion fail (missing files, `config check` falls through to git and errors). RED.

- [ ] **Step 2: Write the four wrappers**

`.scripts/config/config-check`:

```sh
#!/bin/sh
exec config-manifest check "$@"
```

`.scripts/config/config-sync`:

```sh
#!/bin/sh
exec config-manifest sync "$@"
```

`.scripts/config/config-install`:

```sh
#!/bin/sh
exec "$HOME/.scripts/deps/check-deps.sh" --fix "$@"
```

`.scripts/config/config-test`:

```sh
#!/bin/sh
set -eu

usage='usage: config test [-q] [--docker] [--watch] [suite]'
quiet=''
docker=''
watch=''
suite=''

for arg in "$@"; do
    case $arg in
        -q) quiet=-q ;;
        --docker) docker=yes ;;
        --watch) watch=yes ;;
        -*) printf '%s\n' "$usage" >&2; exit 2 ;;
        *) suite=$arg ;;
    esac
done

run_suite() {
    if [ -n "$docker" ]; then
        "$HOME/tests/run-in-docker.sh" ${suite:+"$suite"}
    elif [ -n "$suite" ]; then
        "$HOME/tests/$suite.test.sh"
    else
        "$HOME/tests/run-all.sh" ${quiet:+"$quiet"}
    fi
}

# A cksum over every tracked file's content, tracked only: the worktree is
# the whole home directory, so `ls-files --others` walks all of it and takes
# minutes. Polling is dependency-free on both machines; a real watcher would
# be a platform split.
fingerprint() {
    git --git-dir="$HOME/.cfg" --work-tree="$HOME" ls-files -z --cached \
        | (cd "$HOME" && xargs -0 cksum 2>/dev/null) | cksum
}

if [ -z "$watch" ]; then
    exec_status=0
    run_suite || exec_status=$?
    exit "$exec_status"
fi

last=$(fingerprint)
run_suite || true
while :; do
    sleep 1
    current=$(fingerprint)
    if [ "$current" != "$last" ]; then
        last=$current
        run_suite || true
    fi
done
```

`chmod 755` all four. Note on `--watch`: the fingerprint covers tracked files only (`ls-files --cached`), because the worktree is the whole home directory and `ls-files --others` walks all of it (measured: over two minutes on the mac). A new file is picked up once it is `git add`ed, which is what the test does. `run_suite` failures do not stop the watch loop.

Run: `bash tests/config.test.sh` Expected: all pass (23 + 13 = 36). Update `tests/scripts-dir-name.test.sh` allowlists with the four new scripts, count `'19'` → `'23'`. Run it. Run `shellcheck` on the four scripts if available.

- [ ] **Step 3: Run the host suite, commit**

```bash
bash /Users/austin/tests/run-all.sh 2>&1 | tail -2      # all 24 suite(s) passed (config.test.sh is new)
cd /Users/austin
g() { /usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin "$@"; }
g add .scripts/config/config-check .scripts/config/config-sync .scripts/config/config-install .scripts/config/config-test tests/config.test.sh tests/scripts-dir-name.test.sh
g commit -F - <<'EOF'
Add config check, sync, install, and test

check and sync are one-line execs into config-manifest, so the dispatcher
is the single front door and the binary stays a build artifact. install
wraps check-deps.sh --fix and passes --yes and --dry-run through. test
wraps run-all.sh (-q passes through) or run-in-docker.sh (--docker, with an
optional suite name), and --watch polls a cksum fingerprint over the tracked
files once a second, rerunning on change. Tracked only, because the worktree
is the whole home directory and listing untracked files walks all of it.
Dependency-free on both machines, where a real watcher would be a platform
split.

Every wrapper is tested against shims on PATH or under a fixture HOME, so
the suite never runs itself recursively and never touches the real repo.
EOF
```

---

### Task 4: `reload`, the per-shell `tmux source` removal, and the TODO cleanup

**Files:**
- Create: `.scripts/config/config-reload`
- Modify: `.zshrc` (remove lines 173-174: the comment and the `tmux source` line), `tests/config.test.sh`, `tests/scripts-dir-name.test.sh`, `README.md` (macOS build performance section: one line), `TODO-AGENTS.md` (per-branch)

**Interfaces:**
- `config reload`: when `$TMUX` is set, runs `tmux source "$HOME/.config/tmux/tmux.conf"` and prints `reloaded tmux config`; always prints the line `source ~/.zshrc` to stdout with a stderr note that a child process cannot re-source the caller's shell. Exit 0, or tmux's status if `tmux source` fails.

- [ ] **Step 1: Write the failing tests**

Change `EXPECTED_SUBCOMMANDS` to `'build stamp install-hooks check sync install test reload'`. Append before `finish`:

```bash
# --- reload -----------------------------------------------------------------

printf '#!/bin/sh\nprintf "tmux:%%s\\n" "$@"\n' > "$shim_dir/tmux"
chmod 755 "$shim_dir/tmux"

home=$(make_fixture_home reload)
actual=$(env -u TMUX HOME="$home" PATH="$shim_dir:$PATH" "$CONFIG" reload 2>/dev/null)
assert_equals 'reload outside tmux only prints the zsh line' 'source ~/.zshrc' "$actual"

actual=$(TMUX=fake HOME="$home" PATH="$shim_dir:$PATH" "$CONFIG" reload 2>/dev/null)
assert_equals 'reload inside tmux sources the tmux config first' \
    "tmux:source
tmux:$home/.config/tmux/tmux.conf
reloaded tmux config
source ~/.zshrc" "$actual"

assert_equals 'the per-shell tmux source is gone from .zshrc' '' \
    "$(grep -n 'tmux source' "$DOTFILES_ROOT/.zshrc" || true)"
```

Note: `env -u` is GNU; on Darwin `env -u` exists as of macOS 10.x (`/usr/bin/env -u` is supported). If the Docker image's `env` lacks `-u`, use `(unset TMUX; HOME=... "$CONFIG" reload)` instead; both work, pick the subshell form if in doubt.

Run: `bash tests/config.test.sh` Expected: allowlist and reload assertions fail; the `.zshrc` assertion fails because line 174 still exists. RED.

- [ ] **Step 2: Write `config-reload` and remove the per-shell source**

```sh
#!/bin/sh
set -eu

if [ -n "${TMUX:-}" ]; then
    tmux source "$HOME/.config/tmux/tmux.conf"
    printf 'reloaded tmux config\n'
fi

printf 'a child process cannot re-source the calling shell; run the line below\n' >&2
printf 'source ~/.zshrc\n'
```

`chmod 755 .scripts/config/config-reload`. In `.zshrc`, delete the two lines `# make sure tmux has correct config` and `[[ -n "$TMUX" ]] && tmux source ~/.config/tmux/tmux.conf` (confirm with `sed -n '173,174p' .zshrc` first; then delete by line number). Do not leave a double blank line.

Run: `bash tests/config.test.sh` Expected: all pass (36 + 3 = 39). Update `tests/scripts-dir-name.test.sh` (add `config/config-reload`, count `'23'` → `'24'`). Run `bash tests/zshrc-node-startup.test.sh` and `bash tests/scripts-dir-name.test.sh`.

- [ ] **Step 3: README and TODO**

In `README.md`, in the "macOS build performance" section (or the shell startup notes near it), add one sentence: "`.zshrc` no longer runs `tmux source` per shell (1.7s per pane, all of it tpm); run `config reload` after editing the tmux config." In `TODO-AGENTS.md`: delete the `config:*` utilities item (all six exist: test, test:watch as `--watch`, sync, install, install-hooks, reload) and the "remove per-shell `tmux source` coupled with `config:reload`" item. Leave every other item. Run `bash tests/doc-links.test.sh`.

- [ ] **Step 4: Full suites, commit**

```bash
bash /Users/austin/tests/run-all.sh 2>&1 | tail -2        # all 24 suite(s) passed
~/tests/run-in-docker.sh 2>&1 | tail -2                   # all 22 suite(s) passed
cd /Users/austin
g() { /usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin "$@"; }
g add .scripts/config/config-reload .zshrc tests/config.test.sh tests/scripts-dir-name.test.sh README.md TODO-AGENTS.md
g commit -F - <<'EOF'
Add config reload and stop re-sourcing tmux from every new shell

Every new zsh ran `tmux source` when inside tmux, which costs 1.7s per pane
(all of it tpm re-initialising) and is why opening many panes at once took
minutes. `config reload` does that work on demand: it sources the tmux
config when run inside tmux and prints the `source ~/.zshrc` line for the
caller, since a child process cannot re-source its parent shell.
EOF
```

---

### Task 5: Sync to linux, retire the linux alias, push

**Files:**
- On `linux`: `.zshrc-linux` (delete the alias line 2)

- [ ] **Step 1: Sync with the tool**

```bash
cd /Users/austin
~/.scripts/config/config-build
config sync --dry-run          # lists set/remove edits for .scripts/config/*, tests/*, .zshrc, README.md, DOTFILES.md, .claude/rules/dotfiles-tests.md
```

If the dry run lists only the shared paths this plan touched, apply: `config sync`. Then `config check mac linux` must report a match on all shared paths. If the dry run lists anything unexpected, stop and report the plan output; fall back to the by-hand procedure in `.claude/rules/dotfiles-tests.md`.

- [ ] **Step 2: The linux alias**

`.zshrc-linux` is per-branch, so it is edited on linux directly. Do this with plumbing rather than a checkout, the same way sync does, so the mac worktree is untouched:

```bash
g() { /usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin "$@"; }
g show linux:.zshrc-linux | sed -n '1,3p'                       # confirm line 2 is the alias
g show linux:.zshrc-linux | sed '2d' > "$TMPDIR/zshrc-linux"
blob=$(g hash-object -w "$TMPDIR/zshrc-linux")
export GIT_INDEX_FILE="$TMPDIR/linux-index"
g read-tree linux
g update-index --add --cacheinfo "100644,$blob,.zshrc-linux"
tree=$(g write-tree)
old=$(g rev-parse linux)
commit=$(g commit-tree "$tree" -p "$old" -m "Retire the config alias on linux; the dispatcher is on PATH")
g update-ref refs/heads/linux "$commit" "$old"
unset GIT_INDEX_FILE; rm -f "$TMPDIR/linux-index" "$TMPDIR/zshrc-linux"
g show linux:.zshrc-linux | sed -n '1,3p'                       # alias gone
config check mac linux                                          # still a match
```

- [ ] **Step 3: Push both, watch CI**

```bash
g push origin linux
g push origin mac
gh run list --limit 6
```

Both pushes must pass the hooks (leak scan, stamp match, drift check, Docker suite). CI green on both branches, allowing for the known branch-drift push race (rerun the first-pushed branch's drift job if it raced). On the linux machine, the next `git pull` plus `~/.scripts/config/config install-hooks` finishes the switch there; note that in the final report.

---

## Self-review

**Spec coverage.** Section 5 dispatcher (resolve dir, sibling exec, git passthrough, under 40 lines, alias deletion, symlink install, name rules with two tests, trust assumptions at install): Tasks 1 and 2. 7.1 `check` and 7.2 `sync` front doors: Task 3. 7.3 `test` with `-q`, `--docker`, `--watch`, suite: Task 3. 7.4 `install`: Task 3. 7.5 `install-hooks`: Task 2. 7.6 `reload` and the `.zshrc:174` removal: Task 4. 7.7 `build`: exists from Plan B1. 9.5 suite additions: `tests/config.test.sh` covers every listed assertion (passthrough, unknown falls through, allowlist, no shadowing, install-hooks idempotent and both symlinks and refusal on a world-writable dir); the stamp-check assertions already exist in `tests/config-manifest-lifecycle.test.sh`. 10 step 4: this plan.

**Placeholder scan.** None. Every script and test is written out.

**Name consistency.** `EXPECTED_SUBCOMMANDS` grows `build stamp` → `+install-hooks` → `+check sync install test` → `+reload`; `tests/scripts-dir-name.test.sh` counts 17 → 18 → 19 → 23 → 24; suite counts host 23 → 24 (from Task 3 on), Docker 21 → 22. `make_fixture_home`, `shim_dir`, `CONFIG`, `CONFIG_DIR` are defined in Task 1 and reused. If an executor's count differs by one, the assertion list is authoritative.

**Things the executor must know.**
- `.zshrc-mac` loses its alias in Task 2 only after `install-hooks` has linked the dispatcher into `~/.local/bin`, so `config` never stops working in a new shell on this machine.
- The `--watch` test uses `sleep`; the poll interval is 1s, so the 2s and 3s waits leave a margin without making the suite slow. If the Docker suite proves flaky on this test, widen the waits to 3s and 4s rather than loosening the assertions.
- Nothing in this plan runs `config sync` except Task 5 Step 1, and nothing runs `config test` for real inside the suite (the wrappers are tested against shims).
