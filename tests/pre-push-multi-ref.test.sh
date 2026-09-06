#!/bin/bash
#
# Tests that the pre-push hook gates every ref in a push, not just the first.
#
# mac and linux can be pushed together in one atomic push (`git push --atomic
# origin mac linux`), so two refs can arrive on the hook's stdin where one
# used to. The hook took the first ref and ran the stamp check and the suite
# against that one, which was defended by a comment calling a second ref
# "rare enough" that testing the first was the honest simple behavior. An
# atomic two-branch push makes two refs a real case and retires that premise.
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
#
# Carries a workspace manifest and lockfile alongside the crate, because
# config-stamp now enumerates members from crates/Cargo.toml rather than
# hardcoding a single crate name.
repo=$(make_repo push-multi mac)
git -C "$repo" config user.email t@t
git -C "$repo" config user.name t

mkdir -p "$repo/crates/config-manifest/src"
printf '[workspace]\nmembers = ["config-manifest"]\n' > "$repo/crates/Cargo.toml"
printf 'lock\n' > "$repo/crates/Cargo.lock"
printf 'fn main() {}\n' > "$repo/crates/config-manifest/src/main.rs"
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
# hook compares against config-stamp's read of the pushed ref's actual tree.
# Pointing it at the mac tree models the real situation the hook must catch: a
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
    cat > "$stub_dir/config-manifest" <<STUB
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
            CONFIG_BIN_DIR="$stub_dir" \
            "$HOOK" origin "$repo" >/dev/null 2>&1)
}

# The binary reports the folded stamp for the mac tree, matching what
# config-stamp itself would compute there: <mac-tree>:<lock-blob>:<ws-blob>.
mac_full_stamp() {
    printf '%s:%s:%s' \
        "$mac_tree" \
        "$(git -C "$repo" rev-parse HEAD:crates/Cargo.lock)" \
        "$(git -C "$repo" rev-parse HEAD:crates/Cargo.toml)"
}
linux_full_stamp() {
    printf '%s:%s:%s' \
        "$linux_tree" \
        "$(git -C "$repo" rev-parse linux:crates/Cargo.lock)" \
        "$(git -C "$repo" rev-parse linux:crates/Cargo.toml)"
}

# --- the stamp check must consider every pushed ref --------------------------

# The binary is stamped for mac. A push of mac alone is legitimately fine.
make_stubs "$(mac_full_stamp)"
printf 'refs/heads/mac %s refs/heads/mac %s\n' "$mac_sha" "$mac_sha" \
    | (cd "$repo" && HOME="$FIXTURES/hookhome" PATH="$stub_dir:$PATH" \
        CONFIG_BIN_DIR="$stub_dir" \
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
make_stubs "$(linux_full_stamp)"
run_hook_both_refs
assert_equals 'a two-ref push fails when the binary is stale for the first ref' \
    '1' "$?"

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
