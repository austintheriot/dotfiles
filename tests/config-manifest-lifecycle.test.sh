#!/bin/bash
#
# Tests for the crate build lifecycle: the stamp, the build script, and the
# binaries being on PATH.
#
# A stamp is the tree id of a crate as it is in the WORKTREE, computed through
# a temp index, so the same content stamps identically whether or not it is
# committed. config-build embeds it in each binary via CONFIG_MANIFEST_STAMP;
# pre-push asks each binary for it and compares it to the pushed commit's
# subtree id, refusing a stale binary without compiling.
#
# The crate named config-manifest appears throughout as a fixture crate name,
# built inside throwaway repositories under $FIXTURES. The real crate of that
# name is library-only and installs no binary; config-cli is the installed
# binary these assertions reach for.
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
printf '[workspace]\nmembers = ["config-manifest"]\n' > "$repo/crates/Cargo.toml"
printf 'lock\n' > "$repo/crates/Cargo.lock"
printf '[package]\nname = "config-manifest"\nversion = "0.1.0"\nedition = "2024"\n' \
    > "$repo/crates/config-manifest/Cargo.toml"
printf 'fn main() {}\n' > "$repo/crates/config-manifest/src/main.rs"
git -C "$repo" add crates
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m 'add crate'

committed=$(git -C "$repo" rev-parse HEAD:crates/config-manifest)
stamped=$(DOTFILES_ROOT="$repo" "$STAMP" config-manifest)

assert_succeeds 'the stamp is a folded triple of 40-hex object ids' \
    sh -c "printf '%s' '$stamped' | grep -qE '^[0-9a-f]{40}:[0-9a-f]{40}:[0-9a-f]{40}$'"
assert_equals 'a clean worktree stamps to the committed subtree id' \
    "$committed" "$(printf '%s' "$stamped" | cut -d: -f1)"

printf 'fn main() { println!("edited"); }\n' > "$repo/crates/config-manifest/src/main.rs"
edited=$(DOTFILES_ROOT="$repo" "$STAMP" config-manifest)
assert_succeeds 'an uncommitted edit changes the stamp' \
    test "$edited" != "$committed"

git -C "$repo" add crates
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m 'edit'
assert_equals 'committing the same content stamps to the new subtree id' \
    "$(git -C "$repo" rev-parse HEAD:crates/config-manifest)" "$(printf '%s' "$edited" | cut -d: -f1)"

# Files that are not tracked and not addable (ignored) do not move the stamp.
mkdir -p "$repo/crates/config-manifest/target"
printf 'junk\n' > "$repo/crates/config-manifest/target/junk"
printf 'target/\n' > "$repo/crates/config-manifest/.gitignore"
git -C "$repo" add crates/config-manifest/.gitignore
git -C "$repo" -c user.email=t@t -c user.name=t commit -q -m 'ignore target'
assert_equals 'ignored files do not move the stamp' \
    "$(git -C "$repo" rev-parse HEAD:crates/config-manifest)" \
    "$(DOTFILES_ROOT="$repo" "$STAMP" config-manifest | cut -d: -f1)"

# --- the binary is on PATH wherever the suite runs ---------------------------

# config-cli, not config-manifest: config-manifest is library-only now, so it
# installs no binary and has no --version to answer. config-cli is the binary
# that carries the stamp pre-push compares and the doctor and verify-stamps
# subcommands the gate runs.
assert_succeeds 'config-cli is on PATH' command -v config-cli
assert_succeeds 'config-manifest installs no binary' \
    sh -c '! command -v config-manifest'

# --- config-build installs the binary and writes the stamp -------------------

if command -v cargo >/dev/null 2>&1; then
    bin_dir="$FIXTURES/bin"
    output=$(CONFIG_BIN_DIR="$bin_dir" "$BUILD" 2>&1)
    status=$?
    assert_equals 'config-build exits 0' '0' "$status"
    assert_succeeds 'config-build installs the binary' test -x "$bin_dir/config-cli"
    assert_contains 'config-build reports where it installed' "$bin_dir/config-cli" "$output"

    # A library member is built but not installed, so a crate that stops being
    # a binary stops producing one. Asserted here because config-manifest is
    # exactly that crate, and a silent install of a stale copy is how the
    # two-binary window would reopen.
    assert_contains 'config-build says a library installs nothing' \
        'config-manifest is a library, nothing to install' "$output"
    assert_succeeds 'config-build installs no binary for a library member' \
        test ! -e "$bin_dir/config-manifest"

    # The stamp lives inside the binary, not in a file beside it, so a stale
    # or foreign config-cli on PATH cannot report a stamp it was not built
    # with.
    assert_equals 'the installed binary reports the worktree stamp' \
        "$("$STAMP" config-cli)" "$("$bin_dir/config-cli" --stamp)"
    assert_contains 'config-build reports the stamp it embedded' \
        "$("$STAMP" config-cli)" "$output"

    # A build with no CONFIG_MANIFEST_STAMP must say so rather than print an
    # empty line, which pre-push would compare against a real tree id. This
    # is how Docker and CI build the binary.
    unstamped_target="$FIXTURES/unstamped-target"
    CARGO_TARGET_DIR="$unstamped_target" cargo build --release --locked --quiet \
        --manifest-path "$DOTFILES_ROOT/crates/config-cli/Cargo.toml"
    assert_equals 'a binary built without the variable reports unstamped' \
        'unstamped' "$("$unstamped_target/release/config-cli" --stamp)"
else
    printf 'SKIP  config-build assertions (cargo not found)\n'
fi

# --- the shell drift check is gone and nothing calls it ---------------------

assert_succeeds 'tests/check-branch-drift.sh is deleted' \
    test ! -e "$DOTFILES_ROOT/tests/check-branch-drift.sh"

find_callers() {
    grep -rln "$1" \
        "$DOTFILES_ROOT/tests" "$DOTFILES_ROOT/.github" "$DOTFILES_ROOT/.scripts" \
        "$DOTFILES_ROOT/.claude/rules" 2>/dev/null \
        | grep -v -e '/config-manifest-lifecycle\.test\.sh$' || true
}

# Positive control. The assertion below expects an empty result, so without a
# pattern that must match, a broken grep invocation would report a pass.
assert_contains 'the caller search finds a name that is genuinely referenced' \
    'config/config-build' "$(find_callers 'config-build')"

assert_equals 'no script, workflow, or rule still names check-branch-drift.sh' \
    '' "$(find_callers 'check-branch-drift\.sh')"

# --- toolchain pin, workspace, and untracked seed ----------------------------
#
# The runtime container image is deliberately Rust-free (the binary is copied
# in from a builder stage) and does not COPY crates/, so these checks skip
# there the same way the config-build assertions above skip on no cargo.

TOOLCHAIN_FILE="$DOTFILES_ROOT/crates/rust-toolchain.toml"
if [ -f "$TOOLCHAIN_FILE" ] && command -v rustc >/dev/null 2>&1; then
    # The stamp covers source, not compiler. edition = "2024" is not a pin:
    # every toolchain from 1.85 onward compiles it, so mac and linux could
    # produce matching stamps from different compilers with the gate seeing
    # no difference.
    assert_succeeds 'the pin names an exact version, not a channel' \
        grep -qE '^channel = "1\.[0-9]+\.[0-9]+"' "$TOOLCHAIN_FILE"

    pinned=$(sed -n 's/^channel = "\(.*\)"/\1/p' "$TOOLCHAIN_FILE")
    # Read from inside crates/, because rustup only honours
    # crates/rust-toolchain.toml when the working directory is under it.
    # Reading from wherever the suite happens to run reports the machine's
    # DEFAULT toolchain, so this assertion passed on a developer box whose
    # default already matched the pin and failed on a runner whose default
    # was 1.98.0 -- reporting a pin mismatch that was really a measurement
    # taken in the wrong place.
    installed=$(cd "$(dirname "$TOOLCHAIN_FILE")" && rustc --version | awk '{print $2}')
    assert_equals 'the pinned toolchain is the one installed' "$pinned" "$installed"
else
    skip 'the toolchain is pinned (crates/ or rustc not present here)'
    skip 'the pin names an exact version, not a channel (crates/ or rustc not present here)'
    skip 'the pinned toolchain is the one installed (crates/ or rustc not present here)'
fi

# One workspace, one lockfile, one resolution. The members list is also what
# the stamp enumerates, so it is the single place that says which crates exist.
WORKSPACE_MANIFEST="$DOTFILES_ROOT/crates/Cargo.toml"
if [ -f "$WORKSPACE_MANIFEST" ]; then
    assert_succeeds 'the workspace declares members' \
        grep -q '^members = \[' "$WORKSPACE_MANIFEST"
    assert_succeeds 'the shared lockfile is at the workspace root' \
        test -f "$DOTFILES_ROOT/crates/Cargo.lock"
else
    skip 'a workspace root exists (crates/ not present here)'
    skip 'the workspace declares members (crates/ not present here)'
    skip 'the shared lockfile is at the workspace root (crates/ not present here)'
fi

if [ -d "$DOTFILES_ROOT/.cfg" ]; then
    cfg_git() {
        git --git-dir="$DOTFILES_ROOT/.cfg" --work-tree="$DOTFILES_ROOT" "$@"
    }

    # Positive control first. An empty-expected assertion passes when its
    # pipeline breaks for an unrelated reason, so prove the pipeline reaches
    # the repo before asserting the narrow property.
    assert_succeeds 'ls-files reaches the crates tree' \
        test -n "$(cfg_git ls-files 'crates/*')"
    assert_equals 'no per-crate lockfile remains' '' \
        "$(cfg_git ls-files 'crates/*/Cargo.lock')"
    assert_equals 'no proptest regressions file is tracked' '' \
        "$(cfg_git ls-files 'crates/*/proptest-regressions/*')"
else
    skip 'ls-files reaches the crates tree (no repository here)'
    skip 'no per-crate lockfile remains (no repository here)'
    skip 'no proptest regressions file is tracked (no repository here)'
fi

# --- config stamp is per-crate and ref-scoped --------------------------------
#
# Driven against the $repo fixture above, not the ambient $DOTFILES_ROOT: the
# Docker runner's image carries no .git or .cfg at all (see the "no repository
# here" skips above), so a call defaulting to $DOTFILES_ROOT/$HOME would fail
# there for a reason unrelated to config-stamp itself.

# config stamp enumerates crates. The per-crate form is what pre-push
# iterates; the single-crate form is for scripting.
output=$(DOTFILES_ROOT="$repo" "$STAMP")
assert_succeeds 'config stamp names config-manifest with a folded stamp' \
    grep -qE '^config-manifest [0-9a-f]{40}:[0-9a-f]{40}:[0-9a-f]{40}$' <<<"$output"

one=$(DOTFILES_ROOT="$repo" "$STAMP" config-manifest)
assert_succeeds 'config stamp <crate> prints a bare folded stamp' \
    grep -qE '^[0-9a-f]{40}:[0-9a-f]{40}:[0-9a-f]{40}$' <<<"$one"

# The error path names the crate, so a status 2 from an unrelated cause
# (a missing usage.sh, a mktemp failure) does not read as this error.
err=$(DOTFILES_ROOT="$repo" "$STAMP" no-such-crate 2>&1 || true)
assert_contains 'an unknown crate is named in the error' 'no-such-crate' "$err"
status=0
DOTFILES_ROOT="$repo" "$STAMP" no-such-crate >/dev/null 2>&1 || status=$?
assert_equals 'an unknown crate exits 2' '2' "$status"

# A ref-scoped stamp is what the push gate needs: it must compare the binary
# against what is being published, not against the worktree.
head_stamp=$(DOTFILES_ROOT="$repo" "$STAMP" --ref HEAD config-manifest)
assert_succeeds 'a ref-scoped stamp is well formed' \
    grep -qE '^[0-9a-f]{40}:[0-9a-f]{40}:[0-9a-f]{40}$' <<<"$head_stamp"

finish
