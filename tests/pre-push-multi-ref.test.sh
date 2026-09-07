#!/bin/bash
#
# Tests two properties of the pre-push hook: it gates every ref in a push
# rather than just the first, and its stamps are per-crate rather than one
# workspace-wide id.
#
# A single push can carry several refs (`git push --atomic origin main
# feature`, or a branch and a tag together), so several lines can arrive on
# the hook's stdin where one used to. The hook took the first ref and ran the
# stamp check and the suite against that one, which was defended by a comment
# calling a second ref "rare enough" that testing the first was the honest
# simple behavior. A multi-ref push makes that a real case and retires the
# premise.
#
# The gap that matters is the stamp check. It verifies the built
# config-cli against the crate tree in the pushed ref, and two pushed
# refs can hold different trees there. Checking only the first ref lets a
# binary that is stale for the second ref through the gate that exists to
# catch exactly that.
#
# This file also holds the gate's entry condition, which is the reason the
# 2026-09-06 branch collapse went unnoticed. The hook used to set its gate
# flag by matching refs/heads/mac and refs/heads/linux, and this file fed it
# exactly those two literals, so pushing main skipped the whole stamp block
# and every suite stayed green.
#
# The hook is driven through its documented stdin protocol, one line per ref
# as "<local ref> <local sha> <remote ref> <remote sha>". The costly halves
# (the leak scan and the Docker suite) are replaced with stubs, because what
# is under test is which refs the hook considers, not what those two
# programs do with them.
#
# Usage: ~/tests/pre-push-multi-ref.test.sh

. "$(dirname "$0")/lib.sh"

HOOK="$DOTFILES_ROOT/tests/pre-push"

assert_succeeds 'the pre-push hook is executable' test -x "$HOOK"

# A repository with main and a feature branch holding DIFFERENT crate trees.
# That difference is the whole point: if both branches carried the same tree, a
# hook checking either ref would pass and the bug would be invisible.
#
# The two branches used to be mac and linux, which is what let the collapse to
# a single branch disable the stamp gate without failing a test. A feature
# branch alongside main is the multi-ref push that remains.
#
# Carries a workspace manifest and lockfile alongside the crate, because
# config-stamp now enumerates members from crates/Cargo.toml rather than
# hardcoding a single crate name.
repo=$(make_repo push-multi main)
git -C "$repo" config user.email t@t
git -C "$repo" config user.name t

mkdir -p "$repo/crates/config-manifest/src"
printf '[workspace]\nmembers = ["config-manifest"]\n' > "$repo/crates/Cargo.toml"
printf 'lock\n' > "$repo/crates/Cargo.lock"
printf 'fn main() {}\n' > "$repo/crates/config-manifest/src/main.rs"
printf 'main-crate\n' > "$repo/crates/config-manifest/lib.rs"
git -C "$repo" add -A
git -C "$repo" commit -q -m 'main crate'
main_sha=$(git -C "$repo" rev-parse HEAD)
main_tree=$(git -C "$repo" rev-parse "HEAD:crates/config-manifest")

git -C "$repo" checkout -q -b feature
printf 'feature-crate\n' > "$repo/crates/config-manifest/lib.rs"
git -C "$repo" add -A
git -C "$repo" commit -q -m 'feature crate'
feature_sha=$(git -C "$repo" rev-parse HEAD)
feature_tree=$(git -C "$repo" rev-parse "HEAD:crates/config-manifest")
git -C "$repo" checkout -q main

assert_succeeds 'the fixture branches hold different crate trees' \
    test "$main_tree" != "$feature_tree"

# The stubs. config-cli reports the tree it was "built" from, which the
# hook compares against config-stamp's read of the pushed ref's actual tree.
# Pointing it at the main tree models the real situation the hook must catch: a
# binary built for one branch while a push carries both.
#
# config-stamp itself is NOT stubbed: pre-push resolves it relative to its own
# script location (so the tool that gates a push is the one that ships with
# the hook, not something read out of a possibly-fake $HOME), and the fixture
# repo above carries a real workspace manifest, lockfile, and main.rs, so the
# genuine config-stamp reads a genuine per-ref tree id from it. The fixed
# lock/workspace blobs come along for free since both refs share one
# crates/Cargo.lock and crates/Cargo.toml.
#
# verify-stamps IS stubbed, rather than left to a wildcard `exit 0`. The real
# subcommand reads stdin and refuses on a mismatch; a stub that exits 0
# unconditionally would pass this file regardless of what pre-push piped in,
# which would hide the exact regression this suite exists to catch.
stub_dir="$FIXTURES/hook-stubs"
mkdir -p "$stub_dir" "$FIXTURES/hookhome/tests"

make_stubs() {
    local stamp=$1
    cat > "$stub_dir/config-cli" <<STUB
#!/bin/sh
case \$1 in
    --stamp) printf '%s\n' "$stamp" ;;
    verify-stamps)
        status=0
        while IFS= read -r line; do
            [ -n "\$line" ] || continue
            crate=\${line%% *}
            expected=\${line#* }
            if [ "\$expected" != "$stamp" ]; then
                printf 'stub: %s is stale (built %s, pushed %s)\n' \
                    "\$crate" "$stamp" "\$expected" >&2
                status=1
            fi
        done
        exit "\$status"
        ;;
    *) exit 0 ;;
esac
STUB
    chmod 755 "$stub_dir/config-cli"

    # The leak scan, the Rust checks and the Docker suite are the slow halves.
    # All pass unconditionally here so a failure in this file can only come
    # from the ref-selection logic under test.
    #
    # EVERY script the hook requires needs a stub here, because the hook fails
    # closed on a missing dependency and this fixture points HOME at a
    # directory holding only what this block creates. Adding a hook dependency
    # without adding its stub makes every "the push passes" assertion in this
    # file fail with exit 1, which reads as a ref-selection bug and is not
    # one. That happened when the Rust checks were added.
    printf '#!/bin/sh\nexit 0\n' > "$FIXTURES/hookhome/tests/leak-check.sh"
    printf '#!/bin/sh\nexit 0\n' > "$FIXTURES/hookhome/tests/run-in-docker.sh"
    printf '#!/bin/sh\nexit 0\n' > "$FIXTURES/hookhome/tests/rust-checks.sh"
    chmod 755 "$FIXTURES/hookhome/tests/leak-check.sh" \
        "$FIXTURES/hookhome/tests/run-in-docker.sh" \
        "$FIXTURES/hookhome/tests/rust-checks.sh"
}

# Feeds the hook a push of both refs and returns its exit status.
run_hook_both_refs() {
    printf 'refs/heads/main %s refs/heads/main %s\nrefs/heads/feature %s refs/heads/feature %s\n' \
        "$main_sha" "$main_sha" "$feature_sha" "$feature_sha" \
        | (cd "$repo" && HOME="$FIXTURES/hookhome" PATH="$stub_dir:$PATH" \
            CONFIG_BIN_DIR="$stub_dir" \
            "$HOOK" origin "$repo" >/dev/null 2>&1)
}

# Feeds the hook a push of main alone and prints everything it wrote, so an
# assertion can read the hook's own trace rather than only its exit status.
run_hook_main_only() {
    printf 'refs/heads/main %s refs/heads/main %s\n' "$main_sha" "$main_sha" \
        | (cd "$repo" && HOME="$FIXTURES/hookhome" PATH="$stub_dir:$PATH" \
            CONFIG_BIN_DIR="$stub_dir" \
            "$HOOK" origin "$repo" 2>&1)
}

# The binary reports the folded stamp for the main tree, matching what
# config-stamp itself would compute there: <main-tree>:<lock-blob>:<ws-blob>.
main_full_stamp() {
    printf '%s:%s:%s' \
        "$main_tree" \
        "$(git -C "$repo" rev-parse HEAD:crates/Cargo.lock)" \
        "$(git -C "$repo" rev-parse HEAD:crates/Cargo.toml)"
}
feature_full_stamp() {
    printf '%s:%s:%s' \
        "$feature_tree" \
        "$(git -C "$repo" rev-parse feature:crates/Cargo.lock)" \
        "$(git -C "$repo" rev-parse feature:crates/Cargo.toml)"
}

# --- the stamp check must consider every pushed ref --------------------------

# The binary is stamped for main. A push of main alone is legitimately fine.
make_stubs "$(main_full_stamp)"
printf 'refs/heads/main %s refs/heads/main %s\n' "$main_sha" "$main_sha" \
    | (cd "$repo" && HOME="$FIXTURES/hookhome" PATH="$stub_dir:$PATH" \
        CONFIG_BIN_DIR="$stub_dir" \
        "$HOOK" origin "$repo" >/dev/null 2>&1)
assert_equals 'a main-only push passes when the binary matches main' '0' "$?"

# The gate must actually run for main. This is the assertion whose absence let
# the collapse silently disable stamp verification: the old case arm matched
# refs/heads/mac and refs/heads/linux only, so pushing main skipped the whole
# block and no suite noticed, because this file fed the hook nothing but those
# two literals. Exit status alone cannot catch that -- a skipped gate and a
# passed gate both exit 0 -- so this reads the hook's own trace line.
main_output=$(run_hook_main_only)

# Positive control. Without it, a hook that dies before reaching either branch
# produces no output, and the grep below would be the only failing assertion,
# which reads as a missing gate rather than a broken hook.
assert_succeeds 'the hook produced output for a main push' test -n "$main_output"
assert_contains 'pushing main reaches the stamp gate' \
    'stamp gate passed' "$main_output"

# The same binary, now pushing BOTH branches. feature carries a different
# crate tree, so the binary is stale for feature and the push must be blocked.
# Before the fix the hook read only the first ref, saw main, and passed.
run_hook_both_refs
assert_equals 'a two-ref push fails when the binary is stale for the second ref' \
    '1' "$?"

# The mirror image: stamped for feature, pushing both. Whichever ref git lists
# first, the hook must not pass a binary that is stale for the other one.
make_stubs "$(feature_full_stamp)"
run_hook_both_refs
assert_equals 'a two-ref push fails when the binary is stale for the first ref' \
    '1' "$?"

# --- a ref with no crates is skipped, a broken workspace is not --------------

# Now that the gate runs for every branch rather than two named ones, it meets
# refs that carry no crate workspace at all. config-stamp exits 2 for those
# AND for a manifest that is malformed or names zero members, so the hook must
# not read the two as one condition: the first has nothing to gate, and the
# second is the "a gate that checks nothing" failure this whole change closes.

crateless=$(make_repo push-crateless main)
printf 'notes\n' > "$crateless/README.md"
git -C "$crateless" -c user.email=t@t -c user.name=t add -A
git -C "$crateless" -c user.email=t@t -c user.name=t commit -q -m 'no crates'
crateless_sha=$(git -C "$crateless" rev-parse HEAD)

# CONFIG_BIN_DIR points at an empty directory on purpose. A ref with no crates
# needs no built binary, so the hook must not demand one before it discovers
# there is nothing to verify.
empty_bin="$FIXTURES/no-binaries"
mkdir -p "$empty_bin"

crateless_output=$(printf 'refs/heads/main %s refs/heads/main %s\n' \
    "$crateless_sha" "$crateless_sha" \
    | (cd "$crateless" && HOME="$FIXTURES/hookhome" \
        CONFIG_BIN_DIR="$empty_bin" "$HOOK" origin "$crateless" 2>&1))
crateless_status=$?

# Positive control before the narrow property: an empty output would satisfy
# a "does not mention config-cli" check for the wrong reason.
assert_succeeds 'the hook produced output for a crateless push' \
    test -n "$crateless_output"
assert_equals 'a ref carrying no crates passes the stamp gate' \
    '0' "$crateless_status"
assert_contains 'a crateless push still reports the gate ran' \
    'stamp gate passed' "$crateless_output"

# The other side of the same exit code. An empty member list must still block
# the push, so the skip above cannot be widened to "config-stamp failed".
broken=$(make_repo push-broken main)
mkdir -p "$broken/crates/config-manifest/src"
printf '[workspace]\nmembers = []\n' > "$broken/crates/Cargo.toml"
printf 'lock\n' > "$broken/crates/Cargo.lock"
printf 'fn main() {}\n' > "$broken/crates/config-manifest/src/main.rs"
git -C "$broken" -c user.email=t@t -c user.name=t add -A
git -C "$broken" -c user.email=t@t -c user.name=t commit -q -m 'empty members'
broken_sha=$(git -C "$broken" rev-parse HEAD)

broken_output=$(printf 'refs/heads/main %s refs/heads/main %s\n' \
    "$broken_sha" "$broken_sha" \
    | (cd "$broken" && HOME="$FIXTURES/hookhome" PATH="$stub_dir:$PATH" \
        CONFIG_BIN_DIR="$stub_dir" "$HOOK" origin "$broken" 2>&1))
broken_status=$?

assert_succeeds 'the hook produced output for a broken-workspace push' \
    test -n "$broken_output"
assert_equals 'a workspace naming zero members blocks the push' \
    '1' "$broken_status"
assert_contains 'the block names the unreadable workspace' \
    'cannot read workspace stamps' "$broken_output"

# --- stamps are per-crate, not one workspace-wide id -------------------------

# The reason the stamp is per-crate. With one workspace-wide stamp, editing
# any crate marks every binary stale and the gate refuses a push over a binary
# byte-identical to what its own sources produce. A false refusal is how a gate
# gets bypassed.
ws="$FIXTURES/stamp-scope"
mkdir -p "$ws/crates/crate-one" "$ws/crates/crate-two"
printf '[workspace]\nmembers = ["crate-one", "crate-two"]\n' > "$ws/crates/Cargo.toml"
printf 'lock\n' > "$ws/crates/Cargo.lock"
printf 'one\n' > "$ws/crates/crate-one/src.rs"
printf 'two\n' > "$ws/crates/crate-two/src.rs"

git -C "$ws" init -q -b main
git -C "$ws" -c user.email=t@t -c user.name=t add -A
git -C "$ws" -c user.email=t@t -c user.name=t commit -q -m init

real_stamp_cmd="$DOTFILES_ROOT/.scripts/config/config-stamp"
stamp_in_ws() { DOTFILES_ROOT="$ws" "$real_stamp_cmd" "$@"; }

before_two=$(stamp_in_ws crate-two)
# Positive control. Both assertions below compare two stamp outputs, so if
# config-stamp failed and printed nothing they would compare empty to empty
# and pass vacuously.
assert_succeeds 'the fixture stamp is well formed' \
    grep -qE '^[0-9a-f]{40}:' <<<"$before_two"

printf 'one changed\n' > "$ws/crates/crate-one/src.rs"
git -C "$ws" -c user.email=t@t -c user.name=t add -A
git -C "$ws" -c user.email=t@t -c user.name=t commit -q -m 'edit crate-one'

after_two=$(stamp_in_ws crate-two)
after_one=$(stamp_in_ws crate-one)

assert_equals 'editing one crate leaves the other stamp unchanged' \
    "$before_two" "$after_two"
assert_succeeds 'editing one crate changes its own stamp' \
    test "$after_one" != "$before_two"

# An empty member list must abort rather than iterate zero crates. A gate that
# checks nothing and exits 0 is the failure this whole plan closes.
printf '[workspace]\nmembers = []\n' > "$ws/crates/Cargo.toml"
git -C "$ws" -c user.email=t@t -c user.name=t add -A
git -C "$ws" -c user.email=t@t -c user.name=t commit -q -m 'empty members'

status=0
stamp_in_ws >/dev/null 2>&1 || status=$?
assert_equals 'an empty member list is an error, not an empty run' '2' "$status"

finish
