#!/bin/bash
#
# Tests for the config-manifest build lifecycle: the stamp, the build script,
# and the binary being on PATH.
#
# The stamp is the tree id of crates/config-manifest as it is in the WORKTREE,
# computed through a temp index, so the same content stamps identically
# whether or not it is committed. config-build embeds it in the binary via
# CONFIG_MANIFEST_STAMP; pre-push asks the binary for it and compares it to
# the pushed commit's subtree id, refusing a stale binary without compiling.
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
    output=$(CONFIG_BIN_DIR="$bin_dir" "$BUILD" 2>&1)
    status=$?
    assert_equals 'config-build exits 0' '0' "$status"
    assert_succeeds 'config-build installs the binary' test -x "$bin_dir/config-manifest"
    assert_equals 'the installed binary runs' \
        'config-manifest 0.1.0' "$("$bin_dir/config-manifest" --version)"
    assert_contains 'config-build reports where it installed' "$bin_dir/config-manifest" "$output"

    # The stamp lives inside the binary, not in a file beside it, so a stale
    # or foreign config-manifest on PATH cannot report a stamp it was not
    # built with.
    assert_equals 'the installed binary reports the worktree stamp' \
        "$("$STAMP")" "$("$bin_dir/config-manifest" --stamp)"
    assert_contains 'config-build reports the stamp it embedded' "$("$STAMP")" "$output"

    # A build with no CONFIG_MANIFEST_STAMP must say so rather than print an
    # empty line, which pre-push would compare against a real tree id. This
    # is how Docker and CI build the binary.
    unstamped_target="$FIXTURES/unstamped-target"
    CARGO_TARGET_DIR="$unstamped_target" cargo build --release --locked --quiet \
        --manifest-path "$DOTFILES_ROOT/crates/config-manifest/Cargo.toml"
    assert_equals 'a binary built without the variable reports unstamped' \
        'unstamped' "$("$unstamped_target/release/config-manifest" --stamp)"
else
    printf 'SKIP  config-build assertions (cargo not found)\n'
fi

# --- the shell drift check is gone and nothing calls it ---------------------

assert_succeeds 'tests/check-branch-drift.sh is deleted' \
    test ! -e "$DOTFILES_ROOT/tests/check-branch-drift.sh"

callers=$(grep -rln 'check-branch-drift\.sh' \
    "$DOTFILES_ROOT/tests" "$DOTFILES_ROOT/.github" "$DOTFILES_ROOT/.scripts" \
    "$DOTFILES_ROOT/.claude/rules" 2>/dev/null \
    | grep -v -e '/check-branch-drift\.test\.sh$' -e '/config-manifest-lifecycle\.test\.sh$' || true)
assert_equals 'no script, workflow, or rule outside the two drift test files still names check-branch-drift.sh' \
    '' "$callers"

finish
