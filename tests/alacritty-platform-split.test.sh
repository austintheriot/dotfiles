#!/bin/bash
#
# Tests the alacritty.toml / alacritty-<platform>.toml split.
#
# alacritty.toml used to be per-branch, and it drifted one way in each
# direction: the mac branch never received the Nord colour palette, and the
# linux branch never received option_as_alt or the Cmd+N binding. Neither loss
# was deliberate.
#
# It is now shared and drift-checked. The platform keys live in
# alacritty-mac.toml / alacritty-linux.toml, which both ship on both branches
# so they are drift-checked too.
#
# The awkward part, and the reason this suite exists: Alacritty has no
# conditional import. A shared config that imports both variants applies both
# on every machine. So the shared file imports ONE stable path,
# alacritty-platform.toml, which is a generated pointer to this machine's real
# variant. The pointer is untracked -- it is the only per-machine artifact --
# and `.scripts/alacritty-platform.sh` regenerates it.
#
# The contract:
#   1. the shared config imports the stable pointer, never a named platform
#   2. both real variants ship here
#   3. generating the pointer selects THIS platform's variant
#   4. generating is idempotent and self-healing (a stale or absent pointer
#      is corrected rather than appended to)
#
# Usage: ~/tests/alacritty-platform-split.test.sh

. "$(dirname "$0")/lib.sh"

ALAC_DIR="$DOTFILES_ROOT/.config/alacritty"
SHARED="$ALAC_DIR/alacritty.toml"
GENERATOR="$DOTFILES_ROOT/.scripts/alacritty-platform.sh"

assert_succeeds 'the shared config exists' test -f "$SHARED"
assert_succeeds 'the generator exists' test -f "$GENERATOR"

# --- both real variants ship here -------------------------------------------

assert_succeeds 'the mac variant ships here' test -f "$ALAC_DIR/alacritty-mac.toml"
assert_succeeds 'the linux variant ships here' test -f "$ALAC_DIR/alacritty-linux.toml"

# --- the shared config imports the pointer, not a platform ------------------

assert_succeeds 'the shared config imports the platform pointer' \
    grep -q 'alacritty-platform.toml' "$SHARED"

# Importing a named variant is the bug this design exists to prevent: both
# would load on both machines, because Alacritty has no conditional import.
#
# Comments are stripped before matching. The comment block above the import
# names both variants to explain the design, and grepping the raw file would
# read that prose as configuration.
shared_config_lines=$(grep -vE '^[[:space:]]*#' "$SHARED")
for platform in mac linux; do
    assert_equals "the shared config does not import alacritty-$platform.toml" \
        '' "$(printf '%s' "$shared_config_lines" | grep -n "alacritty-$platform\.toml")"
done

# `general.import` is Alacritty 0.14+. On 0.13 it parses as an unknown key and
# every import is silently dropped -- the config loads, nothing errors, and no
# platform binding is ever applied. Pin the spelling that works on both.
assert_succeeds 'the import is spelled at top level, not under general' \
    grep -qE '^import = \[' "$SHARED"
assert_equals 'the 0.14-only general.import spelling is not used' \
    '' "$(grep -n '^general\.import' "$SHARED")"

# --- generating the pointer selects this platform's variant -----------------

run_generator() {
    local platform=$1 dir=$2
    env DOTFILES_PLATFORM="$platform" ALACRITTY_DIR="$dir" sh "$GENERATOR"
}

for platform in mac linux; do
    dir="$FIXTURES/alac-$platform"
    mkdir -p "$dir"
    printf 'mac-variant\n' > "$dir/alacritty-mac.toml"
    printf 'linux-variant\n' > "$dir/alacritty-linux.toml"

    assert_succeeds "the generator runs for $platform" run_generator "$platform" "$dir"
    assert_succeeds "$platform gets a pointer" test -f "$dir/alacritty-platform.toml"
    assert_contains "the $platform pointer resolves to the $platform variant" \
        "$platform-variant" "$(cat "$dir/alacritty-platform.toml")"

    # The other platform's content must not be reachable through the pointer.
    other=mac; [ "$platform" = mac ] && other=linux
    assert_equals "the $platform pointer does not carry $other content" \
        '' "$(grep -o "$other-variant" "$dir/alacritty-platform.toml" || true)"
done

# --- generating is idempotent and self-healing ------------------------------

# `config reload` and shell startup both run this, so it runs constantly. A
# generator that appended rather than replaced would grow the file without
# bound, and Alacritty would apply every stale copy.
dir="$FIXTURES/alac-idem"
mkdir -p "$dir"
printf 'mac-variant\n' > "$dir/alacritty-mac.toml"
printf 'linux-variant\n' > "$dir/alacritty-linux.toml"
run_generator mac "$dir"
first=$(cat "$dir/alacritty-platform.toml")
run_generator mac "$dir"
second=$(cat "$dir/alacritty-platform.toml")
assert_equals 'running the generator twice changes nothing' "$first" "$second"

# A pointer left behind by the other platform (a synced home directory, a
# branch switch) must be corrected, not trusted.
run_generator linux "$dir"
assert_contains 'a stale pointer is rewritten for the current platform' \
    'linux-variant' "$(cat "$dir/alacritty-platform.toml")"
assert_equals 'the stale platform content is gone' \
    '' "$(grep -o 'mac-variant' "$dir/alacritty-platform.toml" || true)"

# --- the pointer is machine-local, not tracked ------------------------------

# Tracking it would put a per-machine file inside the drift check, which would
# then fail on every push from whichever machine wrote it last.
if git --git-dir="$DOTFILES_ROOT/.cfg" --work-tree="$DOTFILES_ROOT" \
        rev-parse --git-dir >/dev/null 2>&1; then
    tracked=$(git --git-dir="$DOTFILES_ROOT/.cfg" --work-tree="$DOTFILES_ROOT" \
        ls-files ".config/alacritty/alacritty-platform.toml")
    assert_equals 'the generated pointer is not tracked' '' "$tracked"
else
    printf 'alacritty-platform-split: skipped tracking check, no repo\n'
fi

finish
