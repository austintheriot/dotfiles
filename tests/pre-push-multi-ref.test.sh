#!/bin/bash
#
# Tests that the pre-push hook gates every ref in a push, not just the first.
#
# `config push-all` sends mac and linux in one atomic push, so two refs
# arrive on the hook's stdin where one used to. The hook took the first ref
# and ran the stamp check and the suite against that one, which was defended
# by a comment calling a second ref "rare enough" that testing the first was
# the honest simple behavior. push-all makes two refs the normal case and
# retires that premise.
#
# The gap that matters is the stamp check. It verifies the built
# config-manifest against the crate tree in the pushed ref, and mac and
# linux can hold different trees there. Checking only the first ref lets a
# binary that is stale for the second ref through the gate that exists to
# catch exactly that.
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

# A repository with mac and linux holding DIFFERENT crate trees. That
# difference is the whole point: if both branches carried the same tree, a
# hook checking either ref would pass and the bug would be invisible.
repo=$(make_repo push-multi mac)
git -C "$repo" config user.email t@t
git -C "$repo" config user.name t

mkdir -p "$repo/crates/config-manifest"
printf 'mac-crate\n' > "$repo/crates/config-manifest/lib.rs"
git -C "$repo" add -A
git -C "$repo" commit -q -m 'mac crate'
mac_sha=$(git -C "$repo" rev-parse HEAD)
mac_tree=$(git -C "$repo" rev-parse "HEAD:crates/config-manifest")

git -C "$repo" checkout -q -b linux
printf 'linux-crate\n' > "$repo/crates/config-manifest/lib.rs"
git -C "$repo" add -A
git -C "$repo" commit -q -m 'linux crate'
linux_sha=$(git -C "$repo" rev-parse HEAD)
linux_tree=$(git -C "$repo" rev-parse "HEAD:crates/config-manifest")
git -C "$repo" checkout -q mac

assert_succeeds 'the fixture branches hold different crate trees' \
    test "$mac_tree" != "$linux_tree"

# The stubs. config-manifest reports the tree it was "built" from, which the
# hook compares against the pushed ref's tree. Pointing it at the mac tree
# models the real situation the hook must catch: a binary built for one
# branch while a push carries both.
stub_dir="$FIXTURES/hook-stubs"
mkdir -p "$stub_dir" "$FIXTURES/hookhome/tests"

make_stubs() {
    local stamp=$1
    cat > "$stub_dir/config-manifest" <<STUB
#!/bin/sh
case \$1 in
    --stamp) printf '%s\n' "$stamp" ;;
    check) exit 0 ;;
    *) exit 0 ;;
esac
STUB
    chmod 755 "$stub_dir/config-manifest"

    # The leak scan and the Docker suite are the two slow halves. Both pass
    # unconditionally here so a failure in this file can only come from the
    # ref-selection logic under test.
    printf '#!/bin/sh\nexit 0\n' > "$FIXTURES/hookhome/tests/leak-check.sh"
    printf '#!/bin/sh\nexit 0\n' > "$FIXTURES/hookhome/tests/run-in-docker.sh"
    chmod 755 "$FIXTURES/hookhome/tests/leak-check.sh" \
        "$FIXTURES/hookhome/tests/run-in-docker.sh"
}

# Feeds the hook a push of both refs and returns its exit status.
run_hook_both_refs() {
    printf 'refs/heads/mac %s refs/heads/mac %s\nrefs/heads/linux %s refs/heads/linux %s\n' \
        "$mac_sha" "$mac_sha" "$linux_sha" "$linux_sha" \
        | (cd "$repo" && HOME="$FIXTURES/hookhome" PATH="$stub_dir:$PATH" \
            "$HOOK" origin "$repo" >/dev/null 2>&1)
}

# --- the stamp check must consider every pushed ref --------------------------

# The binary is stamped for mac. A push of mac alone is legitimately fine.
make_stubs "$mac_tree"
printf 'refs/heads/mac %s refs/heads/mac %s\n' "$mac_sha" "$mac_sha" \
    | (cd "$repo" && HOME="$FIXTURES/hookhome" PATH="$stub_dir:$PATH" \
        "$HOOK" origin "$repo" >/dev/null 2>&1)
assert_equals 'a mac-only push passes when the binary matches mac' '0' "$?"

# The same binary, now pushing BOTH branches. linux carries a different
# crate tree, so the binary is stale for linux and the push must be blocked.
# Before the fix the hook read only the first ref, saw mac, and passed.
run_hook_both_refs
assert_equals 'a two-ref push fails when the binary is stale for the second ref' \
    '1' "$?"

# The mirror image: stamped for linux, pushing both. Whichever ref git lists
# first, the hook must not pass a binary that is stale for the other one.
make_stubs "$linux_tree"
run_hook_both_refs
assert_equals 'a two-ref push fails when the binary is stale for the first ref' \
    '1' "$?"

finish
