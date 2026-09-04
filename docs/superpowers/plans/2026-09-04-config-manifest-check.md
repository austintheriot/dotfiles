# config-manifest `check` Implementation Plan (Plan B1)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create the `crates/config-manifest` Rust crate with a `check` subcommand that replaces tests/check-branch-drift.sh byte-for-byte in output, prove the build lifecycle (host stamp, Docker builder stage, CI) before any logic exists, and switch every consumer to the binary.

**Architecture:** Sans-IO core: `path` (proof-carrying newtypes), `manifest` (rules ADT, parse/print/classify/partition), `tree` (`TreeListing` from `git ls-tree -r -z`), `check` (pure comparison producing a `CheckReport`, plus a `render` that reproduces the shell script's exact stdout, stderr, and exit code). The only module that spawns git is `git.rs`, which knows both repo shapes (bare at `<root>/.cfg` with `<root>` as worktree, or a normal repo at `<root>`). `main.rs` gathers inputs at the edge and calls the core. The existing 25-assertion shell suite is the equivalence harness during the port.

**Tech Stack:** Rust 1.94 (edition 2024), `anyhow` at the edge only; dev: `proptest`, `assert_cmd`, `tempfile`. POSIX sh for `config-build` and `config-stamp`. Docker multi-stage build. GitHub Actions with preinstalled Rust.

**Spec:** `docs/superpowers/specs/2026-09-04-config-command-and-manifest-crate-design.md`, sections 1, 3, 4, 6.1 to 6.4, 6.6, 6.7, 7.1, 7.7, 8, 9, 10 steps 1 and 2. Plan B2 (`sync`) follows.

## Global Constraints

- Sans-IO: `path`, `manifest`, `tree`, `check` perform no IO and spawn nothing. Only `git.rs` and `main.rs` may use `std::process::Command`. Tests of the pure modules need no git and no filesystem.
- Domain errors are enums (`PathError`, `IdError`, `ManifestError`, `TreeError`). `anyhow` appears only in `git.rs` and `main.rs`.
- All types derive `Debug, Clone, PartialEq, Eq`; ordered ones also `PartialOrd, Ord`. `RelPath`, `BlobId`, `CommitId` have private fields and a single fallible `parse` constructor each.
- No `unwrap()` outside `#[cfg(test)]` and `tests/`. No single-letter identifiers except loop indices. No comments that restate code.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` pass at every commit.
- The output contract is exact. `check` prints the same bytes to the same streams as tests/check-branch-drift.sh, including the `check-branch-drift:` prefix on summary lines, because `tests/pre-push`, `.github/workflows/branch-drift.yml`, and `tests/deps-harness.test.sh` grep that text. Exit codes: 0 clean, 1 any finding or missing manifest, 2 usage.
- Precedence when a path matches several rules, identical to the shell: `!` excluded wins; then a shared rule; then `~` per-branch. A `~` path under a shared directory is still compared, as the shell compares it today.
- The stamp is the tree id of `crates/config-manifest` **as in the worktree** (temp index, `write-tree`, subtree), so the same content stamps identically whether or not it is committed yet. The stamp is a staleness check, not a trust boundary.
- No compile step in any hook. `tests/pre-push` reads the stamp and refuses with `run ~/.scripts/config/config-build` when missing or stale.
- `CARGO_TARGET_DIR` defaults to `$HOME/.cache/config-manifest/target` everywhere (`config-build`, `run-all.sh`); no `target/` ever appears under the tracked tree.
- `Cargo.lock` is committed; every `cargo` invocation in scripts, hooks, Docker, and CI passes `--locked`.
- Never `--no-verify`. Commit messages have no em dashes and no emoji. The pre-commit leak check scans every commit; Rust identifiers must not form `password = "…"`-shaped literals.
- Shared paths (`crates/`, `tests/`, `.scripts/`, `.github/workflows/`, `.claude/rules/`, `docs/research/`, `.sync-manifest`) must be synced to `linux` before pushing `mac`, or the drift gate blocks the push. `docs/superpowers/` is per-branch.
- Fixture repos in tests set `user.email`/`user.name` per command (`-c user.email=t@t -c user.name=t`) and never touch the real repo.

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/config-manifest/Cargo.toml`, `Cargo.lock` | crate manifest, full dependency set from Task 1 |
| `crates/config-manifest/src/main.rs` | CLI: `--version`, `check [ref-a] [ref-b]`; resolve root, discover repo, gather inputs, call core, print, exit |
| `crates/config-manifest/src/path.rs` | `RelPath`, `BlobId`, `CommitId` smart constructors |
| `crates/config-manifest/src/manifest.rs` | `Rule`, `PathPattern`, `Manifest`, `parse`, `print`, `classify`, `partition`, `SharedPaths` |
| `crates/config-manifest/src/tree.rs` | `FileMode`, `TreeListing`, `parse_ls_tree` |
| `crates/config-manifest/src/check.rs` | `check` (pure) and `render` (exact text) |
| `crates/config-manifest/src/git.rs` | `Git::discover`, `show_manifest`, `ls_tree` |
| `crates/config-manifest/tests/check_cli.rs` | `assert_cmd` integration tests on `tempfile` fixture repos, both repo shapes |
| `.scripts/config/config-stamp` | prints the worktree tree id of the crate directory |
| `.scripts/config/config-build` | builds, installs to `~/.local/bin`, writes the stamp, runs `--version` once |
| `tests/config-manifest-lifecycle.test.sh` | stamp semantics, build install, binary on PATH |
| `tests/check-branch-drift.test.sh` (modify) | `CHECK_CMD` parameter; the equivalence harness, then the binary's suite |
| `tests/deps-harness.test.sh` (modify) | its two direct script calls switch to the binary |
| `tests/run-all.sh` (modify) | `cargo test` suite when cargo is present, `SKIP` otherwise |
| `tests/pre-push` (modify) | `^crates/` trigger; stamp check; binary call |
| `tests/docker/Dockerfile` (modify) | builder stage, `COPY --from=builder` |
| `tests/run-in-docker.sh` (modify) | `crates` in the overlay list |
| `tests/scripts-dir-name.test.sh` (modify) | the two new scripts join `EXECUTED_SCRIPTS` |
| `.sync-manifest` (modify) | `crates/` shared rule |
| `.github/workflows/test-suite.yml`, `branch-drift.yml` (modify) | build step, binary on PATH, binary call |
| `.claude/rules/dotfiles-tests.md`, `docs/research/shell-to-rust-prior-art.md` (modify) | citations of the deleted script |

---

### Task 1: Scaffold the crate and prove the build lifecycle

Nothing in this task has logic. It proves that the binary gets built, installed, stamped, copied into the Docker image, built in CI, and that a stale stamp blocks a push, using a binary that only prints its version. The full dependency set goes in now so the lifecycle is proven against the real dependency tree.

**Files:**
- Create: `crates/config-manifest/Cargo.toml`, `crates/config-manifest/Cargo.lock`, `crates/config-manifest/src/main.rs`, `.scripts/config/config-stamp`, `.scripts/config/config-build`, `tests/config-manifest-lifecycle.test.sh`
- Modify: `.sync-manifest` (shared block), `tests/run-in-docker.sh:96`, `tests/pre-push:27` and the drift block, `tests/docker/Dockerfile` (before line 14 and after line 39), `tests/run-all.sh` (after the python block), `tests/scripts-dir-name.test.sh:56-57`, `.github/workflows/test-suite.yml` (after the brew step), `.github/workflows/branch-drift.yml` (after the fetch step)

**Interfaces:**
- Produces: binary `config-manifest` on PATH in host, Docker, and CI; `config-manifest --version` prints `config-manifest 0.1.0`; `.scripts/config/config-stamp` prints a 40-hex tree id; `.scripts/config/config-build` installs and stamps; env knobs `CARGO_TARGET_DIR`, `CONFIG_BIN_DIR` (default `$HOME/.local/bin`), `CONFIG_STAMP_DIR` (default `$HOME/.cache/config-manifest`), `DOTFILES_ROOT`.

- [ ] **Step 1: Write the failing lifecycle test**

Create `tests/config-manifest-lifecycle.test.sh` (then `chmod +x`):

```bash
#!/bin/bash
#
# Tests for the config-manifest build lifecycle: the stamp, the build script,
# and the binary being on PATH.
#
# The stamp is the tree id of crates/config-manifest as it is in the WORKTREE,
# computed through a temp index, so the same content stamps identically
# whether or not it is committed. pre-push compares it to the pushed commit's
# subtree id and refuses a stale binary without ever compiling.
#
# The build half needs cargo. Inside the Docker suite there is no cargo (the
# runtime image is Rust-free; the binary is copied in from a builder stage), so
# that half prints SKIP and the binary assertions still run.
#
# Usage: ~/tests/config-manifest-lifecycle.test.sh

. "$(dirname "$0")/lib.sh"

STAMP="$DOTFILES_ROOT/.scripts/config/config-stamp"
BUILD="$DOTFILES_ROOT/.scripts/config/config-build"

# --- the stamp follows worktree content, not commits ------------------------

repo=$(make_repo stamp main)
mkdir -p "$repo/crates/config-manifest/src"
printf '[package]\nname = "config-manifest"\nversion = "0.1.0"\nedition = "2024"\n' \
    > "$repo/crates/config-manifest/Cargo.toml"
printf 'fn main() {}\n' > "$repo/crates/config-manifest/src/main.rs"
git -C "$repo" add crates
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m 'add crate'

committed=$(git -C "$repo" rev-parse HEAD:crates/config-manifest)
stamped=$(DOTFILES_ROOT="$repo" "$STAMP")

assert_succeeds 'the stamp is a 40-hex object id' \
    sh -c "printf '%s' '$stamped' | grep -qE '^[0-9a-f]{40}$'"
assert_equals 'a clean worktree stamps to the committed subtree id' \
    "$committed" "$stamped"

printf 'fn main() { println!("edited"); }\n' > "$repo/crates/config-manifest/src/main.rs"
edited=$(DOTFILES_ROOT="$repo" "$STAMP")
assert_succeeds 'an uncommitted edit changes the stamp' \
    test "$edited" != "$committed"

git -C "$repo" add crates
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m 'edit'
assert_equals 'committing the same content stamps to the new subtree id' \
    "$(git -C "$repo" rev-parse HEAD:crates/config-manifest)" "$edited"

# Files that are not tracked and not addable (ignored) do not move the stamp.
mkdir -p "$repo/crates/config-manifest/target"
printf 'junk\n' > "$repo/crates/config-manifest/target/junk"
printf 'target/\n' > "$repo/crates/config-manifest/.gitignore"
git -C "$repo" add crates/config-manifest/.gitignore
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m 'ignore target'
assert_equals 'ignored files do not move the stamp' \
    "$(git -C "$repo" rev-parse HEAD:crates/config-manifest)" "$(DOTFILES_ROOT="$repo" "$STAMP")"

# --- the binary is on PATH wherever the suite runs ---------------------------

assert_succeeds 'config-manifest is on PATH' command -v config-manifest
assert_equals 'config-manifest reports its version' \
    'config-manifest 0.1.0' "$(config-manifest --version 2>/dev/null)"

# --- config-build installs the binary and writes the stamp -------------------

if command -v cargo >/dev/null 2>&1; then
    bin_dir="$FIXTURES/bin"
    stamp_dir="$FIXTURES/stamp"
    output=$(CONFIG_BIN_DIR="$bin_dir" CONFIG_STAMP_DIR="$stamp_dir" "$BUILD" 2>&1)
    status=$?
    assert_equals 'config-build exits 0' '0' "$status"
    assert_succeeds 'config-build installs the binary' test -x "$bin_dir/config-manifest"
    assert_equals 'the installed binary runs' \
        'config-manifest 0.1.0' "$("$bin_dir/config-manifest" --version)"
    assert_equals 'config-build writes the worktree stamp' \
        "$("$STAMP")" "$(cat "$stamp_dir/stamp")"
    assert_contains 'config-build reports where it installed' "$bin_dir/config-manifest" "$output"
else
    printf 'SKIP  config-build assertions (cargo not found)\n'
fi

finish
```

- [ ] **Step 2: Run it to verify it fails for the right reason**

Run: `bash tests/config-manifest-lifecycle.test.sh 2>&1 | tail -4`
Expected: the first assertion fails because `.scripts/config/config-stamp` does not exist (`No such file or directory`), and `config-manifest is on PATH` fails.

- [ ] **Step 3: Create the crate**

```bash
cd /Users/austin && mkdir -p crates && cd crates
cargo new --bin config-manifest --edition 2024 --vcs none -q
cd config-manifest
cargo add anyhow -q
cargo add --dev proptest assert_cmd tempfile -q
```

Replace `crates/config-manifest/src/main.rs` with:

```rust
use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") => {
            println!("config-manifest {VERSION}");
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: config-manifest --version");
            ExitCode::from(2)
        }
    }
}
```

Then: `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test --locked -q` (expect 0 tests, no warnings). Confirm `Cargo.lock` exists.

- [ ] **Step 4: Write `config-stamp`**

Create `.scripts/config/config-stamp` (then `chmod +x`):

```sh
#!/bin/sh
#
# Prints the tree id of crates/config-manifest as it is in the worktree.
#
# A HEAD-based stamp false-refuses the `commit -a` workflow: build while
# dirty, commit that same content, push, refused although the binary matches.
# Reading the worktree through a temp index makes the same content stamp
# identically whether or not it is committed yet, and `git add` honours
# .gitignore, so an ignored target/ never moves the stamp.
#
# Usage: config-stamp        (respects DOTFILES_ROOT, default $HOME)

set -eu

ROOT=${DOTFILES_ROOT:-$HOME}
CRATE=crates/config-manifest

if [ -d "$ROOT/.cfg" ]; then
    git_cmd() { git --git-dir="$ROOT/.cfg" --work-tree="$ROOT" "$@"; }
else
    git_cmd() { git -C "$ROOT" "$@"; }
fi

index=$(mktemp "${TMPDIR:-/tmp}/config-stamp-XXXXXX")
rm -f "$index"
trap 'rm -f "$index"' EXIT INT TERM HUP
export GIT_INDEX_FILE="$index"

git_cmd read-tree --empty
(cd "$ROOT" && git_cmd add -- "$CRATE")
root_tree=$(git_cmd write-tree)
git_cmd rev-parse "$root_tree:$CRATE"
```

- [ ] **Step 5: Write `config-build`**

Create `.scripts/config/config-build` (then `chmod +x`):

```sh
#!/bin/sh
#
# Builds crates/config-manifest, installs the binary, and writes the stamp
# pre-push compares against. Run this after editing the crate; pre-push
# never compiles.
#
# Usage: config-build
#   DOTFILES_ROOT      repo root (default $HOME)
#   CARGO_TARGET_DIR   default $HOME/.cache/config-manifest/target, so no
#                      target/ lands inside the tracked tree
#   CONFIG_BIN_DIR     default $HOME/.local/bin
#   CONFIG_STAMP_DIR   default $HOME/.cache/config-manifest

set -eu

ROOT=${DOTFILES_ROOT:-$HOME}
CRATE_DIR="$ROOT/crates/config-manifest"
BIN_DIR=${CONFIG_BIN_DIR:-$HOME/.local/bin}
STAMP_DIR=${CONFIG_STAMP_DIR:-$HOME/.cache/config-manifest}
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/config-manifest/target}"

cargo build --release --locked --quiet --manifest-path "$CRATE_DIR/Cargo.toml"

mkdir -p "$BIN_DIR" "$STAMP_DIR"
cp "$CARGO_TARGET_DIR/release/config-manifest" "$BIN_DIR/config-manifest"
"$ROOT/.scripts/config/config-stamp" > "$STAMP_DIR/stamp"

# Paying any first-run cost here rather than in a hook.
"$BIN_DIR/config-manifest" --version >/dev/null

printf 'config-build: installed %s (stamp %s)\n' \
    "$BIN_DIR/config-manifest" "$(cut -c1-12 "$STAMP_DIR/stamp")"
```

Run it once for real: `~/.scripts/config/config-build` and then `config-manifest --version`. Expected: `config-manifest 0.1.0`.

- [ ] **Step 6: Wire the path filters, the overlay, and the executed-scripts list**

In `.sync-manifest`, add `crates/` on its own line directly after `.scripts/` in the shared block.

In `tests/run-in-docker.sh:96`, change `for tree in tests .scripts .claude .github .config/tmux; do` to `for tree in tests .scripts .claude .github .config/tmux crates; do`.

In `tests/pre-push:27`, change the end of `TRIGGER_PATHS` from `|^\.github/workflows/'` to `|^\.github/workflows/|^crates/'`.

In `tests/scripts-dir-name.test.sh:56-57`, change `EXECUTED_SCRIPTS` to:

```bash
EXECUTED_SCRIPTS='deps/check-deps.sh deps/test-local.sh
tmux-update-window-names.sh tmux-worktree-config.sh
config/config-stamp config/config-build'
```

- [ ] **Step 7: Add the stamp check to pre-push**

In `tests/pre-push`, inside the `if [ "$pushing_synced_branch" -eq 1 ]; then` block, insert this before the existing `if [ ! -x "$HOME/tests/check-branch-drift.sh" ]` check:

```sh
    # The drift check runs the config-manifest binary. It is built by
    # ~/.scripts/config/config-build, never here: a hook that compiles gets
    # bypassed. The stamp is the worktree tree id of the crate at build time;
    # the pushed commit's subtree id must match it.
    stamp_file="${CONFIG_STAMP_DIR:-$HOME/.cache/config-manifest}/stamp"
    pushed_tree=$(git rev-parse --verify --quiet "$push_ref:crates/config-manifest" 2>/dev/null || true)
    if [ -n "$pushed_tree" ]; then
        if [ ! -f "$stamp_file" ]; then
            printf 'pre-push: config-manifest has not been built; run ~/.scripts/config/config-build\n' >&2
            exit 1
        fi
        if [ "$(cat "$stamp_file")" != "$pushed_tree" ]; then
            printf 'pre-push: config-manifest is stale for %s; run ~/.scripts/config/config-build\n' "$push_ref" >&2
            exit 1
        fi
        printf 'pre-push: config-manifest stamp matches the pushed crate\n'
    fi
```

- [ ] **Step 8: Add the builder stage to the Dockerfile**

Insert before line 14 (`FROM debian:bookworm-slim@…`):

```dockerfile
# Builder stage: compiles crates/config-manifest so the runtime image below
# carries only the binary and stays Rust-free. Digest-pinned like the runtime
# base. The dependency layer is built first from a stub main.rs so a source
# edit does not recompile dependencies; a single COPY of the whole crate would
# silently defeat that cache and the measured 0.8s/2.6s rebuilds rest on it.
FROM rust:1.94-slim-bookworm@sha256:cf9dd0ec73e75f827fe59123fff9dc65af1a1c8363c3c31ee8d7f8ad0b6a5fb2 AS builder
WORKDIR /build
COPY crates/config-manifest/Cargo.toml crates/config-manifest/Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs \
    && cargo build --release --locked --quiet \
    && rm -rf src target/release/config-manifest target/release/deps/config_manifest*
COPY crates/config-manifest/src ./src
RUN cargo build --release --locked --quiet

```

Insert after line 39 (the end of the `apt-get` RUN, after `&& rm -rf /var/lib/apt/lists/*`):

```dockerfile

# The suite's shell tests call config-manifest by name, as pre-push and CI do.
COPY --from=builder /build/target/release/config-manifest /usr/local/bin/config-manifest
```

- [ ] **Step 9: Add the cargo suite to run-all.sh**

In `tests/run-all.sh`, after the `python_count` block and before `total_suites=…`, insert:

```bash
cargo_manifest="$DOTFILES_ROOT/crates/config-manifest/Cargo.toml"
cargo_count=0
if command -v cargo >/dev/null 2>&1 && [ -f "$cargo_manifest" ]; then
    cargo_count=1
fi
```

Change `total_suites=$((integration_count + python_count))` to `total_suites=$((integration_count + python_count + cargo_count))`.

After the python `else … fi` block (the one printing `SKIP  python unit tests`), insert:

```bash
# --- Rust unit and integration tests --------------------------------------
#
# Skipped, and said so, where cargo is absent: the Docker runtime image is
# Rust-free by design and gets the binary from a builder stage instead.

if [ "$cargo_count" -eq 1 ]; then
    export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/config-manifest/target}"
    run_suite "cargo test in crates/config-manifest" \
        cargo test --locked --quiet --manifest-path "$cargo_manifest"
else
    printf 'SKIP  cargo test (cargo not found)\n'
fi
```

- [ ] **Step 10: Add the CI build steps**

In `.github/workflows/test-suite.yml`, after the brew step (after line 79) insert:

```yaml
      # Rust is preinstalled on both runners. The suite's shell tests call
      # config-manifest by name, so the release binary goes on PATH. cargo test
      # itself runs inside run-all.sh.
      - name: Build config-manifest
        run: |
          cargo build --release --locked --manifest-path crates/config-manifest/Cargo.toml
          echo "$GITHUB_WORKSPACE/crates/config-manifest/target/release" >> "$GITHUB_PATH"
```

In `.github/workflows/branch-drift.yml`, after the `Fetch both branches` step insert:

```yaml
      - name: Build config-manifest
        run: |
          cargo build --release --locked --manifest-path crates/config-manifest/Cargo.toml
          echo "$GITHUB_WORKSPACE/crates/config-manifest/target/release" >> "$GITHUB_PATH"
```

Note: the CI build writes `target/` inside the checkout on the runner. That is a throwaway VM, not the tracked tree on a developer machine, so the `CARGO_TARGET_DIR` constraint does not apply there.

- [ ] **Step 11: Run the lifecycle test, then the whole suite, then the Docker suite**

Run: `bash tests/config-manifest-lifecycle.test.sh 2>&1 | tail -3`
Expected: `config-manifest-lifecycle: 11 passed, 0 failed` (cargo present on the host).

Run: `bash tests/run-all.sh 2>&1 | tail -3`
Expected: `all 23 suite(s) passed` (21 before, plus this suite and the cargo suite).

Run: `~/tests/run-in-docker.sh 2>&1 | tail -4`
Expected: the builder stage compiles (about 100s the first time on this machine, seconds after), the suite output includes `SKIP  cargo test (cargo not found)` and `config-manifest-lifecycle.test.sh ... PASS`, and the final line is `all 22 suite(s) passed` (no `notify.test.sh` and no cargo suite in the container).

- [ ] **Step 12: Commit**

```bash
cd /Users/austin
g() { /usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin "$@"; }
g add crates .scripts/config/config-stamp .scripts/config/config-build tests/config-manifest-lifecycle.test.sh \
      .sync-manifest tests/run-in-docker.sh tests/pre-push tests/docker/Dockerfile tests/run-all.sh \
      tests/scripts-dir-name.test.sh .github/workflows/test-suite.yml .github/workflows/branch-drift.yml
g commit -F - <<'EOF'
Scaffold config-manifest and prove its build lifecycle before any logic

crates/config-manifest is a Rust binary that only prints its version. That
is the point: this commit proves how the binary gets built, installed,
stamped, copied into the Docker image, built in CI, and how a stale binary
blocks a push, with no logic to muddy the result. The full dependency set
is already in Cargo.toml and Cargo.lock so the lifecycle is proven against
the real dependency tree.

config-stamp prints the worktree tree id of the crate through a temp index,
so the same content stamps identically whether or not it is committed; a
HEAD-based stamp would false-refuse the `commit -a` workflow. config-build
builds with --locked into ~/.cache/config-manifest/target, installs to
~/.local/bin, writes the stamp, and runs the binary once. pre-push compares
the stamp to the pushed commit's subtree id and refuses with "run
config-build" when missing or stale; it never compiles.

The Docker image gains a digest-pinned builder stage with a dependency-first
layer and copies only the binary into the Rust-free runtime. run-all.sh runs
cargo test where cargo exists and says SKIP where it does not. Both CI
workflows build the binary onto PATH with preinstalled Rust.
EOF
```

The pre-commit leak check runs on the commit; the stamp check does not (it is pre-push). Do not push in this task; Task 6 syncs and pushes.

---

### Task 2: `path` and `manifest`, the pure parsing core

**Files:**
- Create: `crates/config-manifest/src/path.rs`, `crates/config-manifest/src/manifest.rs`
- Modify: `crates/config-manifest/src/main.rs` (add `mod path; mod manifest;`)

**Interfaces:**
- Produces: `RelPath::parse(&str) -> Result<RelPath, PathError>`, `RelPath::as_str`, `BlobId::parse`, `CommitId::parse`, `IdError`; `PathPattern { path, is_dir }` with `PathPattern::parse(&str)`, `matches(&RelPath) -> bool`, `Display` (path plus trailing `/` when `is_dir`); `Rule::{Shared, Excluded, PerBranch}(PathPattern)`; `Manifest::rules()`, `shared_rules()`, `classify(&RelPath) -> Classification`, `partition(impl Iterator<Item=&RelPath>) -> Partition { shared: SharedPaths, unmatched: Vec<RelPath> }`; `manifest::parse(&str) -> Result<Manifest, ManifestError>`; `manifest::print(&Manifest) -> String`. Tasks 3 and 4 consume these names exactly.

- [ ] **Step 1: Write the failing tests for `path`**

Create `crates/config-manifest/src/path.rs` with only the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rel_path_accepts_nested_paths_with_spaces_and_unicode() {
        let path = RelPath::parse("dir/sub dir/fïle.txt").expect("valid");
        assert_eq!(path.as_str(), "dir/sub dir/fïle.txt");
    }

    #[test]
    fn rel_path_rejects_empty_absolute_and_parent_traversal() {
        assert_eq!(RelPath::parse(""), Err(PathError::Empty));
        assert_eq!(
            RelPath::parse("/etc/passwd"),
            Err(PathError::Absolute("/etc/passwd".to_string()))
        );
        assert_eq!(
            RelPath::parse("a/../b"),
            Err(PathError::ParentTraversal("a/../b".to_string()))
        );
        assert_eq!(RelPath::parse(".."), Err(PathError::ParentTraversal("..".to_string())));
    }

    #[test]
    fn rel_path_allows_a_single_dot_segment_and_dotfiles() {
        assert!(RelPath::parse(".zshrc").is_ok());
        assert!(RelPath::parse("a/./b").is_ok());
    }

    #[test]
    fn ids_accept_40_and_64_lowercase_hex_only() {
        let sha1 = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
        let sha256 = "a".repeat(64);
        assert!(BlobId::parse(sha1).is_ok());
        assert!(BlobId::parse(&sha256).is_ok());
        assert!(CommitId::parse(sha1).is_ok());
        assert_eq!(BlobId::parse("abc"), Err(IdError::Length(3)));
        assert_eq!(
            BlobId::parse(&"G".repeat(40)),
            Err(IdError::NonHex("G".repeat(40)))
        );
        assert_eq!(
            CommitId::parse(&"A".repeat(40)),
            Err(IdError::NonHex("A".repeat(40)))
        );
    }

    #[test]
    fn rel_paths_order_by_string() {
        let first = RelPath::parse("a").expect("valid");
        let second = RelPath::parse("b").expect("valid");
        assert!(first < second);
    }
}
```

Add `mod path;` to `main.rs`. Run: `cargo test --locked -q path::` in `crates/config-manifest`.
Expected: compile errors, `RelPath` not found. That is the red state.

- [ ] **Step 2: Implement `path`**

Above the test module in `path.rs`:

```rust
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelPath(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    Empty,
    Absolute(String),
    ParentTraversal(String),
}

impl fmt::Display for PathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathError::Empty => write!(formatter, "empty path"),
            PathError::Absolute(raw) => write!(formatter, "absolute path not allowed: {raw}"),
            PathError::ParentTraversal(raw) => write!(formatter, "'..' segment not allowed: {raw}"),
        }
    }
}

impl RelPath {
    pub fn parse(raw: &str) -> Result<Self, PathError> {
        if raw.is_empty() {
            return Err(PathError::Empty);
        }
        if raw.starts_with('/') {
            return Err(PathError::Absolute(raw.to_string()));
        }
        if raw.split('/').any(|segment| segment == "..") {
            return Err(PathError::ParentTraversal(raw.to_string()));
        }
        Ok(RelPath(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdError {
    Length(usize),
    NonHex(String),
}

impl fmt::Display for IdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdError::Length(len) => write!(formatter, "object id has {len} characters, expected 40 or 64"),
            IdError::NonHex(raw) => write!(formatter, "object id is not lowercase hex: {raw}"),
        }
    }
}

fn parse_object_id(raw: &str) -> Result<String, IdError> {
    let len = raw.len();
    if len != 40 && len != 64 {
        return Err(IdError::Length(len));
    }
    let is_lower_hex = raw.chars().all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch));
    if !is_lower_hex {
        return Err(IdError::NonHex(raw.to_string()));
    }
    Ok(raw.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlobId(String);

impl BlobId {
    pub fn parse(raw: &str) -> Result<Self, IdError> {
        parse_object_id(raw).map(BlobId)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommitId(String);

impl CommitId {
    pub fn parse(raw: &str) -> Result<Self, IdError> {
        parse_object_id(raw).map(CommitId)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
```

Run: `cargo test --locked -q path::` Expected: 5 passed.

- [ ] **Step 3: Write the failing tests for `manifest`**

Create `crates/config-manifest/src/manifest.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn rel(raw: &str) -> RelPath {
        RelPath::parse(raw).expect("valid test path")
    }

    const SAMPLE: &str = "\
# shared
.sync-manifest
tests/
DOTFILES.md

# excluded
!tests/notify.test.sh

# per-branch
~.zshrc
~docs/superpowers/
";

    #[test]
    fn parse_reads_three_rule_kinds_and_skips_comments_and_blanks() {
        let manifest = parse(SAMPLE).expect("valid manifest");
        assert_eq!(manifest.rules().len(), 6);
        assert_eq!(manifest.shared_rules().count(), 3);
        assert!(matches!(manifest.rules()[3], Rule::Excluded(_)));
        assert!(matches!(manifest.rules()[4], Rule::PerBranch(_)));
    }

    #[test]
    fn a_trailing_slash_marks_a_directory_rule() {
        let manifest = parse(".sync-manifest\ntests/\n").expect("valid");
        let Rule::Shared(pattern) = &manifest.rules()[1] else {
            panic!("expected a shared rule")
        };
        assert!(pattern.is_dir);
        assert_eq!(pattern.path.as_str(), "tests");
        assert_eq!(pattern.to_string(), "tests/");
    }

    #[test]
    fn parse_reports_the_line_number_of_a_bad_path() {
        let err = parse(".sync-manifest\n\n/absolute\n").expect_err("must fail");
        assert_eq!(
            err,
            ManifestError::Path {
                line: 3,
                source: PathError::Absolute("/absolute".to_string())
            }
        );
    }

    #[test]
    fn directory_rules_match_beneath_and_exact_rules_match_exactly() {
        let manifest = parse("tests/\nDOTFILES.md\n").expect("valid");
        assert_eq!(manifest.classify(&rel("tests/lib.sh")), Classification::Shared);
        assert_eq!(manifest.classify(&rel("tests")), Classification::Shared);
        assert_eq!(manifest.classify(&rel("DOTFILES.md")), Classification::Shared);
        assert_eq!(manifest.classify(&rel("DOTFILES.md.bak")), Classification::Unmatched);
        assert_eq!(manifest.classify(&rel("testsuite/x")), Classification::Unmatched);
    }

    #[test]
    fn excluded_wins_over_shared_and_shared_wins_over_per_branch() {
        let manifest = parse(SAMPLE).expect("valid");
        assert_eq!(manifest.classify(&rel("tests/notify.test.sh")), Classification::Excluded);
        assert_eq!(manifest.classify(&rel("tests/lib.sh")), Classification::Shared);
        assert_eq!(manifest.classify(&rel(".zshrc")), Classification::PerBranch);
        assert_eq!(manifest.classify(&rel("docs/superpowers/specs/a.md")), Classification::PerBranch);
        let overlap = parse("dir/\n~dir/local.txt\n").expect("valid");
        assert_eq!(overlap.classify(&rel("dir/local.txt")), Classification::Shared);
    }

    #[test]
    fn partition_splits_shared_from_unmatched_and_drops_the_rest() {
        let manifest = parse(SAMPLE).expect("valid");
        let paths = [
            rel("tests/lib.sh"),
            rel("tests/notify.test.sh"),
            rel(".zshrc"),
            rel("stray.txt"),
            rel(".sync-manifest"),
        ];
        let partition = manifest.partition(paths.iter());
        let shared: Vec<&str> = partition.shared.iter().map(RelPath::as_str).collect();
        assert_eq!(shared, vec![".sync-manifest", "tests/lib.sh"]);
        assert_eq!(partition.unmatched, vec![rel("stray.txt")]);
    }

    #[test]
    fn print_is_canonical_and_round_trips() {
        let manifest = parse(SAMPLE).expect("valid");
        let printed = print(&manifest);
        assert_eq!(
            printed,
            ".sync-manifest\ntests/\nDOTFILES.md\n!tests/notify.test.sh\n~.zshrc\n~docs/superpowers/\n"
        );
        assert_eq!(parse(&printed).expect("valid"), manifest);
    }

    fn arbitrary_segment() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9_.-]{0,6}".prop_filter("no dot-dot", |segment| segment != ".." && segment != ".")
    }

    fn arbitrary_rule_line() -> impl Strategy<Value = String> {
        (
            prop_oneof![Just(""), Just("!"), Just("~")],
            prop::collection::vec(arbitrary_segment(), 1..4),
            any::<bool>(),
        )
            .prop_map(|(prefix, segments, is_dir)| {
                let slash = if is_dir { "/" } else { "" };
                format!("{prefix}{}{slash}", segments.join("/"))
            })
    }

    proptest! {
        #[test]
        fn parse_print_round_trips(lines in prop::collection::vec(arbitrary_rule_line(), 0..8)) {
            let text = lines.join("\n") + "\n";
            let manifest = parse(&text).expect("generated lines are valid");
            prop_assert_eq!(parse(&print(&manifest)).expect("printed form is valid"), manifest);
        }

        #[test]
        fn shared_partition_never_contains_an_excluded_or_per_branch_path(
            lines in prop::collection::vec(arbitrary_rule_line(), 1..8),
            candidates in prop::collection::vec(prop::collection::vec(arbitrary_segment(), 1..4), 0..12),
        ) {
            let manifest = parse(&(lines.join("\n") + "\n")).expect("valid");
            let paths: Vec<RelPath> = candidates
                .iter()
                .map(|segments| RelPath::parse(&segments.join("/")).expect("valid"))
                .collect();
            let partition = manifest.partition(paths.iter());
            for path in partition.shared.iter() {
                prop_assert_eq!(manifest.classify(path), Classification::Shared);
            }
            for path in &partition.unmatched {
                prop_assert_eq!(manifest.classify(path), Classification::Unmatched);
            }
        }
    }
}
```

Add `mod manifest;` to `main.rs`. Run: `cargo test --locked -q manifest::` Expected: compile errors (`parse`, `Rule` not found). Red.

- [ ] **Step 4: Implement `manifest`**

Above the test module in `manifest.rs`:

```rust
use std::collections::BTreeSet;
use std::fmt;

use crate::path::{PathError, RelPath};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathPattern {
    pub path: RelPath,
    pub is_dir: bool,
}

impl PathPattern {
    pub fn parse(raw: &str) -> Result<Self, PathError> {
        let is_dir = raw.ends_with('/');
        let path = RelPath::parse(raw.trim_end_matches('/'))?;
        Ok(PathPattern { path, is_dir })
    }

    pub fn matches(&self, candidate: &RelPath) -> bool {
        if candidate == &self.path {
            return true;
        }
        self.is_dir
            && candidate.as_str().starts_with(self.path.as_str())
            && candidate.as_str()[self.path.as_str().len()..].starts_with('/')
    }
}

impl fmt::Display for PathPattern {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.path.as_str())?;
        if self.is_dir {
            formatter.write_str("/")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rule {
    Shared(PathPattern),
    Excluded(PathPattern),
    PerBranch(PathPattern),
}

impl Rule {
    fn pattern(&self) -> &PathPattern {
        match self {
            Rule::Shared(pattern) | Rule::Excluded(pattern) | Rule::PerBranch(pattern) => pattern,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    Path { line: usize, source: PathError },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::Path { line, source } => write!(formatter, ".sync-manifest line {line}: {source}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    Shared,
    Excluded,
    PerBranch,
    Unmatched,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedPaths(BTreeSet<RelPath>);

impl SharedPaths {
    pub fn iter(&self) -> impl Iterator<Item = &RelPath> {
        self.0.iter()
    }

    pub fn contains(&self, path: &RelPath) -> bool {
        self.0.contains(path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partition {
    pub shared: SharedPaths,
    pub unmatched: Vec<RelPath>,
}

pub fn parse(text: &str) -> Result<Manifest, ManifestError> {
    let mut rules = Vec::new();
    for (index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line_number = index + 1;
        let (constructor, body): (fn(PathPattern) -> Rule, &str) = match line.as_bytes()[0] {
            b'!' => (Rule::Excluded, &line[1..]),
            b'~' => (Rule::PerBranch, &line[1..]),
            _ => (Rule::Shared, line),
        };
        let pattern = PathPattern::parse(body)
            .map_err(|source| ManifestError::Path { line: line_number, source })?;
        rules.push(constructor(pattern));
    }
    Ok(Manifest { rules })
}

pub fn print(manifest: &Manifest) -> String {
    let mut out = String::new();
    for rule in &manifest.rules {
        let prefix = match rule {
            Rule::Shared(_) => "",
            Rule::Excluded(_) => "!",
            Rule::PerBranch(_) => "~",
        };
        out.push_str(prefix);
        out.push_str(&rule.pattern().to_string());
        out.push('\n');
    }
    out
}

impl Manifest {
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    pub fn shared_rules(&self) -> impl Iterator<Item = &PathPattern> {
        self.rules.iter().filter_map(|rule| match rule {
            Rule::Shared(pattern) => Some(pattern),
            Rule::Excluded(_) | Rule::PerBranch(_) => None,
        })
    }

    pub fn classify(&self, path: &RelPath) -> Classification {
        let matching = |wanted: fn(&Rule) -> Option<&PathPattern>| {
            self.rules.iter().filter_map(wanted).any(|pattern| pattern.matches(path))
        };
        if matching(|rule| match rule {
            Rule::Excluded(pattern) => Some(pattern),
            _ => None,
        }) {
            return Classification::Excluded;
        }
        if matching(|rule| match rule {
            Rule::Shared(pattern) => Some(pattern),
            _ => None,
        }) {
            return Classification::Shared;
        }
        if matching(|rule| match rule {
            Rule::PerBranch(pattern) => Some(pattern),
            _ => None,
        }) {
            return Classification::PerBranch;
        }
        Classification::Unmatched
    }

    pub fn partition<'a>(&self, paths: impl Iterator<Item = &'a RelPath>) -> Partition {
        let mut shared = BTreeSet::new();
        let mut unmatched = Vec::new();
        for path in paths {
            match self.classify(path) {
                Classification::Shared => {
                    shared.insert(path.clone());
                }
                Classification::Unmatched => unmatched.push(path.clone()),
                Classification::Excluded | Classification::PerBranch => {}
            }
        }
        unmatched.sort();
        unmatched.dedup();
        Partition { shared: SharedPaths(shared), unmatched }
    }
}
```

Run: `cargo test --locked -q manifest::` Expected: 9 passed (7 unit, 2 property). Then `cargo fmt && cargo clippy --all-targets -- -D warnings`.

- [ ] **Step 5: Commit**

```bash
cd /Users/austin
g() { /usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin "$@"; }
g add crates/config-manifest/src
g commit -F - <<'EOF'
Add the manifest parser and the proof-carrying path and id types

RelPath, BlobId and CommitId have private fields and one fallible parse
constructor each, so every downstream function receives the proof (no
leading slash, no `..`, 40 or 64 lowercase hex) and never re-validates. A
hand-edited .sync-manifest line with a leading slash is now a parse error
with a line number.

Manifest is the ADT the shell script encoded in 24 IFS gymnastics: three
rule kinds, trailing slash means directory, exact rules match exactly so
DOTFILES.md does not absorb DOTFILES.md.bak. Precedence is the shell's:
excluded wins, then shared, then per-branch, so a ~ path under a shared
directory is still compared. partition() classifies the union of both
trees' paths once and hands both check and (later) sync the same
SharedPaths, so they cannot disagree and the planner can never be handed an
excluded path. Property tests pin parse/print round-trip and that the
shared partition never contains an excluded or per-branch path.
EOF
```

---

### Task 3: `tree` and `git`, the listing and the IO edge

**Files:**
- Create: `crates/config-manifest/src/tree.rs`, `crates/config-manifest/src/git.rs`
- Modify: `crates/config-manifest/src/main.rs` (add `mod tree; mod git;`)

**Interfaces:**
- Consumes: `RelPath`, `BlobId`, `PathError`, `IdError` from Task 2.
- Produces: `FileMode::{Regular, Executable, Symlink}`; `TreeListing` with `get(&RelPath) -> Option<&(FileMode, BlobId)>`, `paths() -> impl Iterator<Item=&RelPath>`, `is_empty()`; `tree::parse_ls_tree(&[u8]) -> Result<TreeListing, TreeError>`; `Git::discover(root: &Path) -> Git`, `Git::show_manifest(&self, rev: &str) -> anyhow::Result<Option<String>>`, `Git::ls_tree(&self, rev: &str) -> anyhow::Result<TreeListing>`.

- [ ] **Step 1: Write the failing tests for `tree`**

Create `crates/config-manifest/src/tree.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn rel(raw: &str) -> RelPath {
        RelPath::parse(raw).expect("valid test path")
    }

    // Recorded `git ls-tree -r -z` output: mode, type, id, tab, path, NUL.
    const RECORDED: &[u8] = b"100644 blob 4b825dc642cb6eb9a060e54bf8d69288fbee4904\t.sync-manifest\0\
100755 blob e69de29bb2d1d6434b8b29ae775ad8c2e48c5391\ttests/lib.sh\0\
120000 blob 8b137891791fe96927ad78e64b0aad7bded08bdc\t.cfg/hooks/pre-commit\0\
100644 blob 3b18e512dba79e4c8300dd08aeb37f8e728b8dad\tdir/with space.txt\0";

    #[test]
    fn parses_modes_ids_and_paths_with_spaces() {
        let listing = parse_ls_tree(RECORDED).expect("valid listing");
        assert_eq!(listing.paths().count(), 4);
        let (mode, blob) = listing.get(&rel("tests/lib.sh")).expect("present");
        assert_eq!(*mode, FileMode::Executable);
        assert_eq!(blob.as_str(), "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391");
        assert_eq!(listing.get(&rel(".cfg/hooks/pre-commit")).expect("present").0, FileMode::Symlink);
        assert!(listing.get(&rel("dir/with space.txt")).is_some());
    }

    #[test]
    fn an_empty_tree_is_an_empty_listing() {
        let listing = parse_ls_tree(b"").expect("valid");
        assert!(listing.is_empty());
    }

    #[test]
    fn rejects_unknown_modes_non_blob_entries_and_malformed_lines() {
        assert_eq!(
            parse_ls_tree(b"160000 commit 4b825dc642cb6eb9a060e54bf8d69288fbee4904\tsub\0"),
            Err(TreeError::UnsupportedType("commit".to_string()))
        );
        assert_eq!(
            parse_ls_tree(b"040000 tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\tdir\0"),
            Err(TreeError::UnsupportedType("tree".to_string()))
        );
        assert_eq!(
            parse_ls_tree(b"100600 blob 4b825dc642cb6eb9a060e54bf8d69288fbee4904\tf\0"),
            Err(TreeError::UnsupportedMode("100600".to_string()))
        );
        assert_eq!(
            parse_ls_tree(b"garbage\0"),
            Err(TreeError::Malformed("garbage".to_string()))
        );
    }

    #[test]
    fn paths_are_returned_in_sorted_order() {
        let listing = parse_ls_tree(RECORDED).expect("valid");
        let paths: Vec<&str> = listing.paths().map(RelPath::as_str).collect();
        assert_eq!(paths, vec![".cfg/hooks/pre-commit", ".sync-manifest", "dir/with space.txt", "tests/lib.sh"]);
    }
}
```

Add `mod tree;` to `main.rs`. Run: `cargo test --locked -q tree::` Expected: compile errors. Red.

- [ ] **Step 2: Implement `tree`**

Above the test module in `tree.rs`:

```rust
use std::collections::BTreeMap;
use std::fmt;

use crate::path::{BlobId, IdError, PathError, RelPath};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileMode {
    Regular,
    Executable,
    Symlink,
}

impl FileMode {
    pub fn as_git_mode(self) -> &'static str {
        match self {
            FileMode::Regular => "100644",
            FileMode::Executable => "100755",
            FileMode::Symlink => "120000",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TreeListing(BTreeMap<RelPath, (FileMode, BlobId)>);

impl TreeListing {
    pub fn get(&self, path: &RelPath) -> Option<&(FileMode, BlobId)> {
        self.0.get(path)
    }

    pub fn paths(&self) -> impl Iterator<Item = &RelPath> {
        self.0.keys()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeError {
    Malformed(String),
    UnsupportedMode(String),
    UnsupportedType(String),
    NotUtf8(Vec<u8>),
    Path(PathError),
    Id(IdError),
}

impl fmt::Display for TreeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TreeError::Malformed(entry) => write!(formatter, "malformed ls-tree entry: {entry}"),
            TreeError::UnsupportedMode(mode) => write!(formatter, "unsupported file mode: {mode}"),
            TreeError::UnsupportedType(kind) => write!(formatter, "unsupported tree entry type: {kind}"),
            TreeError::NotUtf8(bytes) => write!(formatter, "ls-tree entry is not UTF-8: {bytes:?}"),
            TreeError::Path(source) => write!(formatter, "ls-tree path: {source}"),
            TreeError::Id(source) => write!(formatter, "ls-tree object id: {source}"),
        }
    }
}

pub fn parse_ls_tree(bytes: &[u8]) -> Result<TreeListing, TreeError> {
    let mut entries = BTreeMap::new();
    for raw_entry in bytes.split(|byte| *byte == 0).filter(|entry| !entry.is_empty()) {
        let entry = std::str::from_utf8(raw_entry).map_err(|_| TreeError::NotUtf8(raw_entry.to_vec()))?;
        let (meta, path_text) = entry
            .split_once('\t')
            .ok_or_else(|| TreeError::Malformed(entry.to_string()))?;
        let mut fields = meta.split(' ');
        let (Some(mode_text), Some(kind), Some(id_text), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return Err(TreeError::Malformed(entry.to_string()));
        };
        if kind != "blob" {
            return Err(TreeError::UnsupportedType(kind.to_string()));
        }
        let mode = match mode_text {
            "100644" => FileMode::Regular,
            "100755" => FileMode::Executable,
            "120000" => FileMode::Symlink,
            other => return Err(TreeError::UnsupportedMode(other.to_string())),
        };
        let blob = BlobId::parse(id_text).map_err(TreeError::Id)?;
        let path = RelPath::parse(path_text).map_err(TreeError::Path)?;
        entries.insert(path, (mode, blob));
    }
    Ok(TreeListing(entries))
}
```

Run: `cargo test --locked -q tree::` Expected: 4 passed.

- [ ] **Step 3: Write `git.rs` and its fixture-backed test**

`git.rs` is the IO edge, so its tests need real repos. Create `crates/config-manifest/src/git.rs`:

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};

use crate::tree::{TreeListing, parse_ls_tree};

/// Both shapes this repo comes in: the real dotfiles repo is bare at
/// `<root>/.cfg` with `<root>` as the worktree, and every test fixture is a
/// normal repository with `.git` inside `<root>`.
#[derive(Debug, Clone)]
pub struct Git {
    prefix: Vec<String>,
}

impl Git {
    pub fn discover(root: &Path) -> Git {
        let cfg: PathBuf = root.join(".cfg");
        let prefix = if cfg.is_dir() {
            vec![
                format!("--git-dir={}", cfg.display()),
                format!("--work-tree={}", root.display()),
            ]
        } else {
            vec!["-C".to_string(), root.display().to_string()]
        };
        Git { prefix }
    }

    fn output(&self, args: &[&str]) -> anyhow::Result<std::process::Output> {
        Command::new("git")
            .args(&self.prefix)
            .args(args)
            .output()
            .with_context(|| format!("failed to spawn git {}", args.join(" ")))
    }

    pub fn show_manifest(&self, rev: &str) -> anyhow::Result<Option<String>> {
        let spec = format!("{rev}:.sync-manifest");
        let output = self.output(&["show", &spec])?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(Some(String::from_utf8(output.stdout).context(".sync-manifest is not UTF-8")?))
    }

    pub fn ls_tree(&self, rev: &str) -> anyhow::Result<TreeListing> {
        let output = self.output(&["ls-tree", "-r", "-z", rev])?;
        if !output.status.success() {
            bail!(
                "git ls-tree {rev} failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        parse_ls_tree(&output.stdout).with_context(|| format!("parsing ls-tree output for {rev}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::RelPath;
    use crate::tree::FileMode;

    fn run(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.email=t@t", "-c", "user.name=t"])
            .args(args)
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?} failed");
    }

    #[test]
    fn lists_a_normal_repo_and_reads_its_manifest() {
        let dir = tempfile::tempdir().expect("tempdir");
        run(dir.path(), &["init", "-q", "-b", "main"]);
        std::fs::write(dir.path().join(".sync-manifest"), ".sync-manifest\nrun.sh\n").expect("write");
        std::fs::write(dir.path().join("run.sh"), "#!/bin/sh\n").expect("write");
        run(dir.path(), &["add", "."]);
        run(dir.path(), &["update-index", "--chmod=+x", "run.sh"]);
        run(dir.path(), &["commit", "-q", "-m", "init"]);

        let git = Git::discover(dir.path());
        let manifest = git.show_manifest("main").expect("git works").expect("manifest present");
        assert_eq!(manifest, ".sync-manifest\nrun.sh\n");
        let listing = git.ls_tree("main").expect("git works");
        let exec = RelPath::parse("run.sh").expect("valid");
        assert_eq!(listing.get(&exec).expect("present").0, FileMode::Executable);
        assert_eq!(git.show_manifest("no-such-ref").expect("git ran"), None);
    }

    #[test]
    fn discovers_a_bare_repo_at_root_dot_cfg() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = dir.path().join(".cfg");
        let status = Command::new("git")
            .args(["init", "-q", "--bare", "-b", "main"])
            .arg(&cfg)
            .status()
            .expect("git runs");
        assert!(status.success());
        let bare = |args: &[&str]| {
            let status = Command::new("git")
                .arg(format!("--git-dir={}", cfg.display()))
                .arg(format!("--work-tree={}", dir.path().display()))
                .args(["-c", "user.email=t@t", "-c", "user.name=t"])
                .args(args)
                .current_dir(dir.path())
                .status()
                .expect("git runs");
            assert!(status.success(), "git {args:?} failed");
        };
        std::fs::write(dir.path().join(".sync-manifest"), ".sync-manifest\n").expect("write");
        bare(&["add", ".sync-manifest"]);
        bare(&["commit", "-q", "-m", "init"]);

        let git = Git::discover(dir.path());
        let listing = git.ls_tree("main").expect("git works");
        assert_eq!(listing.paths().count(), 1);
    }
}
```

Add `mod git;` to `main.rs`. Run: `cargo test --locked -q git::` Expected: 2 passed. Then `cargo fmt && cargo clippy --all-targets -- -D warnings`.

- [ ] **Step 4: Commit**

```bash
cd /Users/austin
g() { /usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin "$@"; }
g add crates/config-manifest/src
g commit -F - <<'EOF'
Add the tree listing parser and the git edge

TreeListing is the snapshot the core reasons over: path to (mode, blob id),
parsed from `git ls-tree -r -z`. FileMode is part of it because exec bits
are load-bearing in this repo (scripts-dir-name.test.sh asserts them) and
symlinks are tracked (.cfg/hooks). Anything but a blob, or a mode other
than 100644/100755/120000, is an error rather than a silent skip.

git.rs is the only module that spawns git. Git::discover knows both repo
shapes: bare at <root>/.cfg with <root> as worktree (the real dotfiles
repo) and a normal repo at <root> (every test fixture). Its tests are the
only unit tests in the crate that touch a filesystem, and they say so.
EOF
```

---

### Task 4: `check` and the equivalence harness

**Files:**
- Create: `crates/config-manifest/src/check.rs`, `crates/config-manifest/tests/check_cli.rs`
- Modify: `crates/config-manifest/src/main.rs` (the `check` subcommand), `tests/check-branch-drift.test.sh:14,29-32` (the `CHECK_CMD` parameter)

**Interfaces:**
- Consumes: everything from Tasks 2 and 3.
- Produces: `check::check(&Manifest, &TreeListing, &TreeListing) -> CheckReport`; `CheckReport { findings: Vec<Finding>, shared_rules_checked: usize }`; `Finding::{Diverged { rule: PathPattern }, Unmatched { path: RelPath, present_on: PresentOn }}`; `PresentOn::{A, B, Both}`; `check::render(&CheckReport, ref_a: &str, ref_b: &str) -> Rendered { stdout: String, stderr: String, exit_code: u8 }`; CLI `config-manifest check [ref-a] [ref-b]` honouring `DOTFILES_ROOT`.

- [ ] **Step 1: Write the failing tests for `check`**

Create `crates/config-manifest/src/check.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest;
    use crate::tree::parse_ls_tree;

    const ID_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const ID_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const ID_M: &str = "cccccccccccccccccccccccccccccccccccccccc";

    fn listing(entries: &[(&str, &str, &str)]) -> TreeListing {
        let mut bytes = Vec::new();
        for (mode, id, path) in entries {
            bytes.extend_from_slice(format!("{mode} blob {id}\t{path}\0").as_bytes());
        }
        parse_ls_tree(&bytes).expect("valid listing")
    }

    fn manifest_of(text: &str) -> Manifest {
        manifest::parse(text).expect("valid manifest")
    }

    #[test]
    fn identical_shared_paths_are_clean_and_counted() {
        let manifest = manifest_of(".sync-manifest\nshared.txt\n");
        let listing_a = listing(&[("100644", ID_M, ".sync-manifest"), ("100644", ID_A, "shared.txt")]);
        let report = check(&manifest, &listing_a, &listing_a);
        assert!(report.findings.is_empty());
        assert_eq!(report.shared_rules_checked, 2);
        let rendered = render(&report, "mac", "linux");
        assert_eq!(rendered.stdout, "check-branch-drift: mac and linux match on all 2 shared path(s)\n");
        assert_eq!(rendered.stderr, "");
        assert_eq!(rendered.exit_code, 0);
    }

    #[test]
    fn a_differing_blob_under_a_shared_rule_is_diverged() {
        let manifest = manifest_of(".sync-manifest\nshared.txt\n");
        let listing_a = listing(&[("100644", ID_M, ".sync-manifest"), ("100644", ID_A, "shared.txt")]);
        let listing_b = listing(&[("100644", ID_M, ".sync-manifest"), ("100644", ID_B, "shared.txt")]);
        let report = check(&manifest, &listing_a, &listing_b);
        assert_eq!(report.findings.len(), 1);
        let rendered = render(&report, "mac", "linux");
        assert_eq!(rendered.stdout, "diverged: shared.txt\n");
        assert_eq!(
            rendered.stderr,
            "\ncheck-branch-drift: mac and linux diverged on 1 of 2 shared path(s)\n"
        );
        assert_eq!(rendered.exit_code, 1);
    }

    #[test]
    fn a_mode_change_alone_is_diverged() {
        let manifest = manifest_of("run.sh\n");
        let listing_a = listing(&[("100644", ID_A, "run.sh")]);
        let listing_b = listing(&[("100755", ID_A, "run.sh")]);
        assert_eq!(check(&manifest, &listing_a, &listing_b).findings.len(), 1);
    }

    #[test]
    fn a_directory_rule_diverges_on_added_removed_or_changed_files_beneath() {
        let manifest = manifest_of("dir/\n");
        let listing_a = listing(&[("100644", ID_A, "dir/x.txt")]);
        let listing_b = listing(&[("100644", ID_A, "dir/x.txt"), ("100644", ID_B, "dir/y.txt")]);
        let report = check(&manifest, &listing_a, &listing_b);
        assert_eq!(render(&report, "a", "b").stdout, "diverged: dir/\n");
    }

    #[test]
    fn an_excluded_path_inside_a_shared_directory_may_differ() {
        let manifest = manifest_of("dir/\n!dir/platform.txt\n");
        let listing_a = listing(&[("100644", ID_A, "dir/common.txt"), ("100644", ID_A, "dir/platform.txt")]);
        let listing_b = listing(&[("100644", ID_A, "dir/common.txt"), ("100644", ID_B, "dir/platform.txt")]);
        let report = check(&manifest, &listing_a, &listing_b);
        assert!(report.findings.is_empty());
        assert_eq!(report.shared_rules_checked, 1);
    }

    #[test]
    fn a_per_branch_path_is_neither_compared_nor_unmatched() {
        let manifest = manifest_of(".sync-manifest\n~per-branch.txt\n");
        let listing_a = listing(&[("100644", ID_M, ".sync-manifest"), ("100644", ID_A, "per-branch.txt")]);
        let listing_b = listing(&[("100644", ID_M, ".sync-manifest"), ("100644", ID_B, "per-branch.txt")]);
        let report = check(&manifest, &listing_a, &listing_b);
        assert!(report.findings.is_empty());
        assert_eq!(report.shared_rules_checked, 1);
    }

    #[test]
    fn an_unmatched_file_on_either_side_is_reported_with_its_refs() {
        let manifest = manifest_of(".sync-manifest\n");
        let listing_a = listing(&[("100644", ID_M, ".sync-manifest"), ("100644", ID_A, "only-a.txt")]);
        let listing_b = listing(&[("100644", ID_M, ".sync-manifest"), ("100644", ID_B, "only-b.txt"), ("100644", ID_A, "only-a.txt")]);
        let report = check(&manifest, &listing_a, &listing_b);
        assert_eq!(
            report.findings,
            vec![
                Finding::Unmatched { path: RelPath::parse("only-a.txt").expect("valid"), present_on: PresentOn::Both },
                Finding::Unmatched { path: RelPath::parse("only-b.txt").expect("valid"), present_on: PresentOn::B },
            ]
        );
        let rendered = render(&report, "origin/mac", "origin/linux");
        assert_eq!(rendered.stdout, "");
        assert_eq!(
            rendered.stderr,
            "\ncheck-branch-drift: 2 tracked file(s) match no .sync-manifest rule\n\n\
  only-a.txt                                   (on origin/mac, origin/linux)\n\
  only-b.txt                                   (on origin/linux)\n\
\nEvery tracked file must match a rule, so a file added on one branch\n\
cannot escape this check. Add one of these to .sync-manifest:\n\n\
  path/to/file                                 shared: must be identical on both branches\n\
  ~path/to/file                                per-branch: tracked, never compared\n\
  !path/to/file                                excluded from an enclosing shared path\n\n"
        );
        assert_eq!(rendered.exit_code, 1);
    }

    #[test]
    fn diverged_lines_print_before_the_unmatched_block_and_unmatched_decides_the_summary() {
        let manifest = manifest_of(".sync-manifest\nshared.txt\n");
        let listing_a = listing(&[("100644", ID_M, ".sync-manifest"), ("100644", ID_A, "shared.txt"), ("100644", ID_A, "stray.txt")]);
        let listing_b = listing(&[("100644", ID_M, ".sync-manifest"), ("100644", ID_B, "shared.txt")]);
        let rendered = render(&check(&manifest, &listing_a, &listing_b), "mac", "linux");
        assert_eq!(rendered.stdout, "diverged: shared.txt\n");
        assert!(rendered.stderr.contains("1 tracked file(s) match no .sync-manifest rule"));
        assert!(!rendered.stderr.contains("diverged on"));
        assert_eq!(rendered.exit_code, 1);
    }

    #[test]
    fn a_lookalike_path_is_not_absorbed_by_an_exact_rule() {
        let manifest = manifest_of(".sync-manifest\nfile.txt\n");
        let listing_a = listing(&[("100644", ID_M, ".sync-manifest"), ("100644", ID_A, "file.txt"), ("100644", ID_A, "file.txt.bak")]);
        let report = check(&manifest, &listing_a, &listing_a);
        assert_eq!(report.findings.len(), 1);
        assert!(render(&report, "a", "b").stderr.contains("file.txt.bak"));
    }
}
```

Add `mod check;` to `main.rs`. Run: `cargo test --locked -q check::` Expected: compile errors. Red.

- [ ] **Step 2: Implement `check`**

Above the test module in `check.rs`:

```rust
use std::collections::BTreeSet;

use crate::manifest::{Classification, Manifest, PathPattern};
use crate::path::RelPath;
use crate::tree::TreeListing;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentOn {
    A,
    B,
    Both,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    Diverged { rule: PathPattern },
    Unmatched { path: RelPath, present_on: PresentOn },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckReport {
    pub findings: Vec<Finding>,
    pub shared_rules_checked: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: u8,
}

pub fn check(manifest: &Manifest, listing_a: &TreeListing, listing_b: &TreeListing) -> CheckReport {
    let union: BTreeSet<&RelPath> = listing_a.paths().chain(listing_b.paths()).collect();
    let mut findings = Vec::new();
    let mut shared_rules_checked = 0;

    for rule in manifest.shared_rules() {
        shared_rules_checked += 1;
        let diverged = union.iter().any(|path| {
            rule.matches(path)
                && manifest.classify(path) != Classification::Excluded
                && listing_a.get(path) != listing_b.get(path)
        });
        if diverged {
            findings.push(Finding::Diverged { rule: rule.clone() });
        }
    }

    let partition = manifest.partition(union.iter().copied());
    for path in partition.unmatched {
        let present_on = match (listing_a.get(&path).is_some(), listing_b.get(&path).is_some()) {
            (true, true) => PresentOn::Both,
            (true, false) => PresentOn::A,
            (false, true) => PresentOn::B,
            (false, false) => continue,
        };
        findings.push(Finding::Unmatched { path, present_on });
    }

    CheckReport { findings, shared_rules_checked }
}

/// Reproduces tests/check-branch-drift.sh byte for byte, including which
/// stream each line goes to: pre-push, branch-drift.yml and deps-harness
/// grep this text.
pub fn render(report: &CheckReport, ref_a: &str, ref_b: &str) -> Rendered {
    let mut stdout = String::new();
    let mut stderr = String::new();

    let diverged: Vec<&PathPattern> = report
        .findings
        .iter()
        .filter_map(|finding| match finding {
            Finding::Diverged { rule } => Some(rule),
            Finding::Unmatched { .. } => None,
        })
        .collect();
    let unmatched: Vec<(&RelPath, PresentOn)> = report
        .findings
        .iter()
        .filter_map(|finding| match finding {
            Finding::Unmatched { path, present_on } => Some((path, *present_on)),
            Finding::Diverged { .. } => None,
        })
        .collect();

    for rule in &diverged {
        stdout.push_str(&format!("diverged: {rule}\n"));
    }

    if !unmatched.is_empty() {
        stderr.push_str(&format!(
            "\ncheck-branch-drift: {} tracked file(s) match no .sync-manifest rule\n\n",
            unmatched.len()
        ));
        for (path, present_on) in &unmatched {
            let refs = match present_on {
                PresentOn::A => ref_a.to_string(),
                PresentOn::B => ref_b.to_string(),
                PresentOn::Both => format!("{ref_a}, {ref_b}"),
            };
            stderr.push_str(&format!("  {:<44} (on {refs})\n", path.as_str()));
        }
        stderr.push_str("\nEvery tracked file must match a rule, so a file added on one branch\n");
        stderr.push_str("cannot escape this check. Add one of these to .sync-manifest:\n\n");
        stderr.push_str(&format!("  {:<44} shared: must be identical on both branches\n", "path/to/file"));
        stderr.push_str(&format!("  {:<44} per-branch: tracked, never compared\n", "~path/to/file"));
        stderr.push_str(&format!("  {:<44} excluded from an enclosing shared path\n\n", "!path/to/file"));
        return Rendered { stdout, stderr, exit_code: 1 };
    }

    if !diverged.is_empty() {
        stderr.push_str(&format!(
            "\ncheck-branch-drift: {ref_a} and {ref_b} diverged on {} of {} shared path(s)\n",
            diverged.len(),
            report.shared_rules_checked
        ));
        return Rendered { stdout, stderr, exit_code: 1 };
    }

    stdout.push_str(&format!(
        "check-branch-drift: {ref_a} and {ref_b} match on all {} shared path(s)\n",
        report.shared_rules_checked
    ));
    Rendered { stdout, stderr, exit_code: 0 }
}
```

Run: `cargo test --locked -q check::` Expected: 9 passed.

- [ ] **Step 3: Wire the `check` subcommand in `main.rs`**

Replace `crates/config-manifest/src/main.rs` with:

```rust
mod check;
mod git;
mod manifest;
mod path;
mod tree;

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const USAGE: &str = "usage: config-manifest --version | check [ref-a] [ref-b]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") => {
            println!("config-manifest {VERSION}");
            ExitCode::SUCCESS
        }
        Some("check") => match run_check(&args[1..]) {
            Ok(code) => ExitCode::from(code),
            Err(error) => {
                eprintln!("check-branch-drift: {error:#}");
                ExitCode::from(1)
            }
        },
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn dotfiles_root() -> anyhow::Result<PathBuf> {
    if let Some(root) = std::env::var_os("DOTFILES_ROOT") {
        return Ok(PathBuf::from(root));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("neither DOTFILES_ROOT nor HOME is set")
}

fn run_check(args: &[String]) -> anyhow::Result<u8> {
    if args.len() > 2 {
        eprintln!("{USAGE}");
        return Ok(2);
    }
    let ref_a = args.first().map(String::as_str).unwrap_or("origin/mac");
    let ref_b = args.get(1).map(String::as_str).unwrap_or("origin/linux");

    let repo = git::Git::discover(&dotfiles_root()?);
    let Some(manifest_text) = repo.show_manifest(ref_a)? else {
        eprintln!("check-branch-drift: could not read .sync-manifest from {ref_a}");
        return Ok(1);
    };
    let manifest = manifest::parse(&manifest_text)?;
    let listing_a = repo.ls_tree(ref_a)?;
    let listing_b = repo.ls_tree(ref_b)?;

    let rendered = check::render(&check::check(&manifest, &listing_a, &listing_b), ref_a, ref_b);
    std::io::stdout().write_all(rendered.stdout.as_bytes())?;
    std::io::stderr().write_all(rendered.stderr.as_bytes())?;
    Ok(rendered.exit_code)
}
```

`ManifestError` needs to be an `Error` for `?` into `anyhow`: add `impl std::error::Error for ManifestError {}` in `manifest.rs`, and likewise `impl std::error::Error for TreeError {}` in `tree.rs`, `impl std::error::Error for PathError {}` and `impl std::error::Error for IdError {}` in `path.rs`.

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo build --release --locked -q` then `~/.scripts/config/config-build` so the installed binary is current.

- [ ] **Step 4: Write the CLI integration tests**

Create `crates/config-manifest/tests/check_cli.rs`:

```rust
use std::path::Path;
use std::process::Command;

use assert_cmd::prelude::*;

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.email=t@t", "-c", "user.name=t"])
        .args(args)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?} failed");
}

/// A repo with a linux branch and a mac branch, both from the same manifest
/// and the same shared content; the same shape check-branch-drift.test.sh uses.
fn manifest_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "-q", "-b", "linux"]);
    std::fs::write(dir.path().join(".sync-manifest"), "# comment\n\n.sync-manifest\nshared.txt\n").expect("write");
    std::fs::write(dir.path().join("shared.txt"), "same content\n").expect("write");
    git(dir.path(), &["add", ".sync-manifest", "shared.txt"]);
    git(dir.path(), &["commit", "-q", "-m", "add manifest"]);
    git(dir.path(), &["branch", "mac"]);
    dir
}

fn check(dir: &Path, refs: &[&str]) -> assert_cmd::assert::Assert {
    Command::cargo_bin("config-manifest")
        .expect("binary built")
        .env("DOTFILES_ROOT", dir)
        .arg("check")
        .args(refs)
        .assert()
}

#[test]
fn matching_paths_exit_zero_and_report_the_count() {
    let dir = manifest_repo();
    check(dir.path(), &["mac", "linux"])
        .success()
        .stdout("check-branch-drift: mac and linux match on all 2 shared path(s)\n")
        .stderr("");
}

#[test]
fn a_diverged_path_exits_one_and_names_it() {
    let dir = manifest_repo();
    git(dir.path(), &["checkout", "-q", "mac"]);
    std::fs::write(dir.path().join("shared.txt"), "different content\n").expect("write");
    git(dir.path(), &["add", "shared.txt"]);
    git(dir.path(), &["commit", "-q", "-m", "diverge on mac"]);
    check(dir.path(), &["mac", "linux"])
        .code(1)
        .stdout("diverged: shared.txt\n")
        .stderr("\ncheck-branch-drift: mac and linux diverged on 1 of 2 shared path(s)\n");
}

#[test]
fn a_missing_manifest_exits_one_and_names_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "-q", "-b", "linux"]);
    git(dir.path(), &["commit", "-q", "--allow-empty", "-m", "init"]);
    git(dir.path(), &["branch", "mac"]);
    check(dir.path(), &["mac", "linux"])
        .code(1)
        .stderr("check-branch-drift: could not read .sync-manifest from mac\n");
}

#[test]
fn an_unmatched_file_reports_the_branch_it_is_on() {
    let dir = manifest_repo();
    git(dir.path(), &["checkout", "-q", "mac"]);
    std::fs::write(dir.path().join("brand-new.txt"), "brand new\n").expect("write");
    git(dir.path(), &["add", "brand-new.txt"]);
    git(dir.path(), &["commit", "-q", "-m", "add unlabeled file"]);
    let assert = check(dir.path(), &["mac", "linux"]).code(1);
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8");
    assert!(stderr.contains("brand-new.txt"));
    assert!(stderr.contains("(on mac)"));
    assert!(stderr.contains("match no .sync-manifest rule"));
}

#[test]
fn too_many_arguments_is_a_usage_error() {
    let dir = manifest_repo();
    check(dir.path(), &["a", "b", "c"]).code(2);
}

#[test]
fn works_against_a_bare_repo_at_root_dot_cfg() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = dir.path().join(".cfg");
    assert!(Command::new("git").args(["init", "-q", "--bare", "-b", "linux"]).arg(&cfg).status().expect("git").success());
    let bare = |args: &[&str]| {
        let status = Command::new("git")
            .arg(format!("--git-dir={}", cfg.display()))
            .arg(format!("--work-tree={}", dir.path().display()))
            .args(["-c", "user.email=t@t", "-c", "user.name=t"])
            .args(args)
            .current_dir(dir.path())
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?} failed");
    };
    std::fs::write(dir.path().join(".sync-manifest"), ".sync-manifest\n").expect("write");
    bare(&["add", ".sync-manifest"]);
    bare(&["commit", "-q", "-m", "init"]);
    bare(&["branch", "mac"]);
    check(dir.path(), &["mac", "linux"])
        .success()
        .stdout("check-branch-drift: mac and linux match on all 1 shared path(s)\n");
}
```

Run: `cargo test --locked -q` in the crate. Expected: all unit tests plus 6 integration tests pass (5 path, 9 manifest, 4 tree, 2 git, 9 check, 6 CLI = 35).

- [ ] **Step 5: Parameterise the shell harness and run it against both implementations**

In `tests/check-branch-drift.test.sh`, replace line 14 and lines 29-32 with:

```bash
# CHECK_CMD selects the implementation under test. The default is the shell
# script; `CHECK_CMD='config-manifest check'` runs the same 25 assertions
# against the Rust binary. Word-splitting the unquoted expansion is the
# intent: the value is a command plus its subcommand.
CHECK_CMD=${CHECK_CMD:-$DOTFILES_ROOT/tests/check-branch-drift.sh}

run_check() {
    local repo=$1; shift
    DOTFILES_ROOT="$repo" $CHECK_CMD "$@"
}
```

Also update line 3 of that file to `# Integration tests for the branch-drift check (shell script and config-manifest binary)`.

Run both:

```bash
bash tests/check-branch-drift.test.sh 2>&1 | tail -1
CHECK_CMD='config-manifest check' bash tests/check-branch-drift.test.sh 2>&1 | tail -1
```

Expected: both print `check-branch-drift: 25 passed, 0 failed`. If the binary run differs on any assertion, the binary is wrong (the shell script is the reference); fix the binary, never the assertion. Then run the full suite: `bash tests/run-all.sh 2>&1 | tail -2`, expected `all 23 suite(s) passed`.

- [ ] **Step 6: Commit**

```bash
cd /Users/austin
g() { /usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin "$@"; }
~/.scripts/config/config-build
g add crates/config-manifest tests/check-branch-drift.test.sh
g commit -F - <<'EOF'
Add `config-manifest check`, equivalent to check-branch-drift.sh

check() is pure: two TreeListings and a Manifest in, a CheckReport out.
Byte-identical means equal blob id and equal mode; the core never reads
file bytes. Unmatched files come from the partition over the union of both
listings, so a file added on one branch only cannot escape.

render() reproduces the shell script's output exactly, including which
stream each line goes to and the check-branch-drift: prefix, because
pre-push, branch-drift.yml and deps-harness.test.sh grep that text. The
ordering quirk is preserved on purpose: diverged lines print to stdout as
they are found, then an unlabeled-files block on stderr ends the run before
the diverged summary would have printed.

The 25-assertion shell suite is now parameterised by CHECK_CMD and passes
against both implementations. That is the equivalence proof for the
switch-over in the next commit.
EOF
```

---

### Task 5: Switch every consumer to the binary and delete the shell script

**Files:**
- Modify: `tests/pre-push` (the drift block), `.github/workflows/branch-drift.yml:30`, `tests/deps-harness.test.sh:378,388`, `tests/check-branch-drift.test.sh` (default `CHECK_CMD`), `.claude/rules/dotfiles-tests.md:41,103`, `docs/research/shell-to-rust-prior-art.md`, `docs/superpowers/specs/2026-09-04-config-command-and-manifest-crate-design.md`, `docs/superpowers/plans/2026-09-04-leak-check-range-mode.md`
- Delete: tests/check-branch-drift.sh

**Interfaces:**
- Consumes: `config-manifest check [ref-a] [ref-b]` from Task 4, on PATH via Task 1.
- Produces: no caller of tests/check-branch-drift.sh remains; `doc-links.test.sh` stays green.

- [ ] **Step 1: Write the failing assertion that no caller remains**

Append to `tests/config-manifest-lifecycle.test.sh`, before `finish`:

```bash
# --- the shell drift check is gone and nothing calls it ---------------------

assert_succeeds 'tests/check-branch-drift.sh is deleted' \
    test ! -e "$DOTFILES_ROOT/tests/check-branch-drift.sh"

callers=$(grep -rln 'check-branch-drift\.sh' \
    "$DOTFILES_ROOT/tests" "$DOTFILES_ROOT/.github" "$DOTFILES_ROOT/.scripts" \
    "$DOTFILES_ROOT/.claude/rules" 2>/dev/null \
    | grep -v '/check-branch-drift\.test\.sh$' || true)
assert_equals 'no script, workflow, or rule still names check-branch-drift.sh' '' "$callers"
```

Run: `bash tests/config-manifest-lifecycle.test.sh 2>&1 | grep -E 'FAIL|passed'` Expected: the two new assertions fail (the script exists; pre-push, branch-drift.yml, deps-harness.test.sh, dotfiles-tests.md name it).

- [ ] **Step 2: Switch the callers**

In `tests/pre-push`, inside the `pushing_synced_branch` block, replace the existence check and the call:

```sh
    if ! command -v config-manifest >/dev/null 2>&1; then
        printf 'pre-push: config-manifest is not on PATH; run ~/.scripts/config/config-build\n' >&2
        exit 1
    fi

    if ! config-manifest check mac linux; then
```

(keep the three `printf` lines and `exit 1` that follow, and the `printf 'pre-push: branch-drift check passed\n'`.)

In `.github/workflows/branch-drift.yml:30`, change `output=$(tests/check-branch-drift.sh 2>&1)` to `output=$(config-manifest check 2>&1)`.

In `tests/deps-harness.test.sh:378` and `:388`, change `"$DOTFILES_ROOT/tests/check-branch-drift.sh" mac linux` to `config-manifest check mac linux` (both lines).

In `tests/check-branch-drift.test.sh`, change the default: `CHECK_CMD=${CHECK_CMD:-config-manifest check}` and update the comment above it to say the shell script has been removed and the variable remains so a future implementation can be checked against this suite.

Delete the script: `g rm -q tests/check-branch-drift.sh` (using the `g()` long form).

- [ ] **Step 3: Update the citations doc-links checks**

`doc-links.test.sh` resolves every backticked path ending in `.sh/.conf/.yml/.yaml/.md/.toml/.py` in tracked Markdown. Update each:

- `.claude/rules/dotfiles-tests.md:41`: change `` `~/tests/check-branch-drift.sh mac linux` `` to `` `config-manifest check mac linux` ``. Line 103: change `` `check-branch-drift.sh` defaults to comparing`` to `` `config-manifest check` defaults to comparing``.
- `docs/research/shell-to-rust-prior-art.md`: every `` `tests/check-branch-drift.sh` `` becomes `` `tests/check-branch-drift.test.sh` `` where the sentence is about the test suite, or plain text `check-branch-drift.sh` (no backticks) where it is historical narrative about the script that was ported. Run `grep -n 'check-branch-drift' docs/research/shell-to-rust-prior-art.md` and decide per line; historical mentions stay as prose.
- `docs/superpowers/specs/2026-09-04-config-command-and-manifest-crate-design.md`: same rule; references in section 12 become plain text "the former `check-branch-drift` shell script, replaced by `config-manifest check`".
- `docs/superpowers/plans/2026-09-04-leak-check-range-mode.md`: it cites the script only in prose about the hook; change any backticked `.sh` citation to plain text.

Then: `bash tests/doc-links.test.sh 2>&1 | tail -1` Expected: `doc-links: 2 passed, 0 failed`.

- [ ] **Step 4: Run the lifecycle test and the full suite**

Run: `bash tests/config-manifest-lifecycle.test.sh 2>&1 | tail -1` Expected: `config-manifest-lifecycle: 13 passed, 0 failed`.
Run: `bash tests/run-all.sh 2>&1 | tail -2` Expected: `all 23 suite(s) passed`. `workflow-labels.test.sh` still passes: its assertions grep the drift workflow for the `diverged` and `unlabeled` markers, which are unchanged.
Run: `~/tests/run-in-docker.sh 2>&1 | tail -2` Expected: `all 22 suite(s) passed`.

- [ ] **Step 5: Commit**

```bash
cd /Users/austin
g() { /usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin "$@"; }
g add -A tests .github .claude/rules docs
g commit -F - <<'EOF'
Switch the drift check to config-manifest and delete the shell script

pre-push, branch-drift.yml and deps-harness.test.sh now call
`config-manifest check`. The 25-assertion suite that proved the two
implementations equivalent keeps running against the binary, with
CHECK_CMD left in place so any future implementation can be held to it.

The shell script is gone rather than kept as a fallback: two parsers for
one format is the defect the second consumer (sync, next) exposed, and a
fallback that is never exercised rots silently. Citations in the rules
file, the research doc, the spec and the earlier plan are updated so
doc-links stays green.
EOF
```

---

### Task 6: Sync to linux and push both branches

**Files:**
- Sync to `linux`: `crates/`, `tests/`, `.scripts/config/`, `.github/workflows/`, `.claude/rules/dotfiles-tests.md`, `docs/research/shell-to-rust-prior-art.md`, `.sync-manifest`, `tests/docker/Dockerfile`, `tests/run-in-docker.sh`
- Stays on `mac`: `docs/superpowers/`

- [ ] **Step 1: Build, so the stamp matches what is about to be pushed**

```bash
cd /Users/austin
~/.scripts/config/config-build
g() { /usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin "$@"; }
g status -s -uno            # must be empty
g rev-parse --short HEAD
```

- [ ] **Step 2: Sync every shared path that changed and push linux**

```bash
cd /Users/austin
g() { /usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin "$@"; }
g checkout -q linux
g branch --show-current     # linux
g checkout mac -- crates tests .scripts/config .github/workflows .claude/rules/dotfiles-tests.md docs/research/shell-to-rust-prior-art.md .sync-manifest
g rm -q --cached tests/check-branch-drift.sh 2>/dev/null || true
g status -s -uno
g commit -q -F - <<'EOF'
Sync the config-manifest crate and its consumers from mac

Every path here is shared in .sync-manifest. check-branch-drift.sh is
deleted on this branch too, since nothing calls it.
EOF
config-manifest check mac linux
g push origin linux
g checkout -q mac
```

Expected: `g status -s -uno` lists the crate files, the two scripts, the modified tests and workflows, the rules file, the research doc, the manifest, and the deletion of tests/check-branch-drift.sh; `config-manifest check mac linux` prints `match on all 16 shared path(s)` (15 before plus `crates/`); the linux push runs the leak scan, the stamp check (the crate subtree is identical on both branches, so the stamp matches), the binary drift check, and the Docker suite.

Note: after `g checkout -q linux`, the working tree is the linux branch; `~/.scripts/config/config-stamp` and the installed binary are unaffected because the crate content is identical on both branches after the sync.

- [ ] **Step 3: Push mac and confirm**

```bash
cd /Users/austin
g() { /usr/bin/git --git-dir=/Users/austin/.cfg/ --work-tree=/Users/austin "$@"; }
g branch --show-current     # mac
g push origin mac
g branch -vv | grep -E '^\*|linux'
```

Expected: both branches show `[origin/<branch>]` with no `ahead`; the mac push prints `pre-push: config-manifest stamp matches the pushed crate` and `pre-push: branch-drift check passed`.

---

## Self-review

**Spec coverage.** Section 4 layout and path filters: Task 1 (manifest rule, overlay, `TRIGGER_PATHS`, CI no filter change). Section 6.1 to 6.4 types: Tasks 2 to 4 with the exact names. Section 6.6 `Git::discover`, both shapes: Task 3. Section 6.7 main gathers at the edge: Task 4 Step 3. Section 7.1 `check` defaults and exit codes: Task 4 (`origin/mac`, `origin/linux`, 1 on findings, 2 on usage). Section 7.7 `config build` and the content stamp: Task 1 (as `config-build` and `config-stamp` invoked by path; the dispatcher that makes it `config build` is Plan C). Section 8 lifecycle table: Task 1 (host, pre-push stamp check with the two messages, Docker builder with the stated layer order, CI with preinstalled Rust and `--locked`). Section 9.1 unit tests, 9.2 two of the property tests (round trip; partition never contains excluded or per-branch), 9.3 integration on both shapes, 9.4 equivalence harness, 9.5 stamp check messages: Tasks 1 to 5. Section 10 steps 1 and 2: Tasks 1 to 5. The remaining property tests (idempotence, check-after-sync, `Set` blob presence) belong to `plan` and are Plan B2. Section 5 dispatcher and section 7.5 install-hooks are Plan C.

**Placeholder scan.** None; every code step carries its code. The one description-only step is Task 5 Step 3's per-line citation rewrite, which lists the files and the exact rule to apply.

**Type and name consistency.** `RelPath::parse`, `as_str`; `PathPattern { path, is_dir }`, `matches`, `Display`; `Rule::{Shared, Excluded, PerBranch}`; `Manifest::rules/shared_rules/classify/partition`; `Partition { shared: SharedPaths, unmatched }`; `SharedPaths::iter/contains`; `manifest::parse/print`; `FileMode::{Regular, Executable, Symlink}`; `TreeListing::get/paths/is_empty`; `tree::parse_ls_tree`; `TreeError::{Malformed, UnsupportedMode, UnsupportedType, NotUtf8, Path, Id}`; `Git::discover/show_manifest/ls_tree`; `check::check/render`, `CheckReport`, `Finding`, `PresentOn`, `Rendered { stdout, stderr, exit_code }`. Task 4's tests and `main.rs` use exactly these. `CommitId` and `FileMode::as_git_mode` are defined here and consumed by Plan B2; clippy may warn `dead_code` on them: allow it with `#[allow(dead_code)]` on those two items with a one-line comment naming Plan B2, and remove the allow there.

**Counts stated.** Lifecycle suite: 11 assertions in Task 1 (5 stamp, 2 binary, 5 build when cargo is present; 6 when SKIP), 13 after Task 5. Rust: 35 tests after Task 4. Shell harness: 25, unchanged. Suite totals: 23 on host and CI, 22 in Docker. If an executor's count differs by one, the assertion list is authoritative and the number is bookkeeping.

**Things the executor must know.**
- The pre-commit leak check scans every commit. The Rust test ids (`"a".repeat(40)`, `ID_A` constants of repeated letters) match no rule; never introduce a `password = "..."`-shaped literal.
- `cargo new --edition 2024` requires Rust 1.85 or newer; both machines have 1.94.
- After Task 4, run `~/.scripts/config/config-build` before any push, or the stamp check in Task 1's pre-push refuses.
- The Docker builder pull is about 100s the first time on a machine. It is already cached on the mac from the measurement run.
