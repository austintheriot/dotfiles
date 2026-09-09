#!/bin/bash
#
# Tests the alacritty.toml / alacritty-<platform>.toml split.
#
# alacritty.toml used to be per-branch, and it drifted one way in each
# direction: the mac branch never received the Nord colour palette, and the
# linux branch never received option_as_alt or the Cmd+N binding. Neither loss
# was deliberate.
#
# It is now shared, and the platform keys live in alacritty-mac.toml /
# alacritty-linux.toml, which both ship on the single branch. The assertions
# below are what now catches a variant going missing.
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

# --- Linux launches zsh, because its login shell is not zsh -----------------

# A fresh Pop!_OS machine has bash as the login shell, so Alacritty opened
# there lands in bash. Nothing in `.zshrc` is loaded, which means the `s`
# alias (`.zshrc:66`) does not exist and `s code` fails until zsh is started
# by hand. macOS needs no equivalent: its login shell has been zsh since
# Catalina.
#
# Declared here rather than by `chsh` deliberately. `chsh` writes
# /etc/passwd, which is system state outside $HOME that nothing else in this
# repo touches, needs a password or root, has no idempotent declaration, and
# cannot be exercised by this suite. A config key is tracked, platform-
# selected by machinery that already exists, and reversible by editing a file.
LINUX_VARIANT="$ALAC_DIR/alacritty-linux.toml"

# Comments stripped before matching, for the same reason the shared config's
# import assertions do it: the comment block explaining the 0.13-vs-0.14 key
# names mentions `terminal.shell` by name, and grepping the raw file reads
# that prose as configuration.
linux_variant_lines=$(grep -vE '^[[:space:]]*#' "$LINUX_VARIANT")

assert_succeeds 'the linux variant launches zsh' \
    grep -qE '^program = "/bin/zsh"' "$LINUX_VARIANT"
assert_succeeds 'the shell program is declared under the 0.13 [shell] table' \
    grep -qE '^\[shell\]' "$LINUX_VARIANT"

# `terminal.shell` is the 0.14+ spelling. On 0.13 it parses as an unknown key
# and the shell setting is silently dropped -- Alacritty starts, nothing
# errors, and the login shell runs anyway. Verified against the installed
# binary: `strings` on alacritty 0.13.2 contains `shell` and
# `working_directory` and no `terminal` config section at all. This is the
# same silent-ignore trap the `general.import` assertion above pins.
assert_equals 'the 0.14-only terminal.shell spelling is not used' \
    '' "$(printf '%s' "$linux_variant_lines" | grep -n 'terminal\.shell\|^\[terminal')"

# The mac variant must NOT carry it. Hardcoding /bin/zsh on a mac would
# override a Homebrew zsh the user chose, and the key is unnecessary there.
#
# Matched on the [shell] TABLE HEADER, not on a bare `program =` line. The
# mac variant already has `program = "open"` under
# [keyboard.bindings.command], and the looser pattern flagged that unrelated
# key -- the assertion failed for the wrong reason before this was narrowed.
assert_equals 'the mac variant declares no shell' \
    '' "$(grep -n '^\[shell\]\|^shell\.' "$ALAC_DIR/alacritty-mac.toml")"

# The shared file must not carry it either: values in the shared config WIN
# over an imported variant (alacritty.toml:13), so a shell key there would
# apply on macOS too and defeat the split.
assert_equals 'the shared config declares no shell' \
    '' "$(printf '%s' "$shared_config_lines" | grep -n '^\[shell\]\|^shell\.')"

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

# --- the pointer is replaced atomically, never truncated in place -----------

# `.zshrc:188` backgrounds the generator from every shell, so on the first
# startup after a variant edit all ~107 panes run it at once and the
# content-equality guard lets every one of them through. Alacritty watches the
# pointer, so a read landing inside a truncate-then-fill window gets a partial
# config and Alacritty applies whatever parsed.
#
# Asserted behaviourally rather than by grepping for `mv`: a sampling reader
# watches the file while a writer loops, and no sample may be shorter than the
# finished file. Measured on the pre-fix code, 14% of reads saw a partial or
# zero-length file; the atomic form produced 0 out of 3.49M.
#
# A large payload is what makes the window observable at all. The real variants
# are small enough that a single write(2) usually completes between two
# samples, which is why this pads to ~200KB: the bug is present either way, and
# this size is what makes a regression fail here rather than on somebody's
# terminal.
if command -v python3 >/dev/null 2>&1; then
    dir="$FIXTURES/alac-atomic"
    mkdir -p "$dir"
    printf 'mac-variant\n' > "$dir/alacritty-mac.toml"
    {
        printf 'linux-variant\n'
        i=0
        while [ "$i" -lt 4000 ]; do
            printf 'key%04d = "padding that widens the write window"\n' "$i"
            i=$((i + 1))
        done
    } > "$dir/alacritty-linux.toml"

    # One clean run establishes the finished size the samples are judged against.
    run_generator linux "$dir"
    pointer="$dir/alacritty-platform.toml"
    full_size=$(wc -c < "$pointer" | tr -d ' ')

    sampler="$FIXTURES/alac-atomic-sampler.py"
    cat > "$sampler" <<'SAMPLER'
import os, sys, time

# Counts reads that found the pointer PRESENT but shorter than finished.
#
# A missing file is deliberately not counted. The writer loop below deletes
# the pointer to force a write past the content-equality guard, so an absence
# is the harness's own doing and says nothing about how the write happens --
# counting it would fail the atomic form too, which is exactly the false
# result this comment exists to prevent. What only a non-atomic write can
# produce is a file that exists and is incomplete, so that is the signal.
path, seconds, full_size = sys.argv[1], float(sys.argv[2]), int(sys.argv[3])
short = 0
deadline = time.time() + seconds
while time.time() < deadline:
    try:
        size = os.path.getsize(path)
    except OSError:
        continue
    if size < full_size:
        short += 1
print(short)
SAMPLER

    python3 "$sampler" "$pointer" 3 "$full_size" > "$dir/short-count" &
    sampler_pid=$!
    sleep 0.3
    # Rewrite in a loop. The content guard would exit early on an unchanged
    # file, so each iteration alternates the variant content to force a write.
    #
    # Deleting the pointer is what forces the write: the guard reads the
    # pointer, so an absent one can never match and the generator always
    # reaches the write. Alternating the VARIANT instead does not work -- the
    # guard would still be satisfied on the iterations where the content
    # happened to match, and a loop that mostly exits early samples nothing.
    loop_end=$(( $(date +%s) + 2 ))
    while [ "$(date +%s)" -lt "$loop_end" ]; do
        rm -f "$pointer"
        run_generator linux "$dir"
    done
    wait "$sampler_pid"

    short_reads=$(cat "$dir/short-count")
    assert_equals 'no reader ever sees a partial pointer' '0' "$short_reads"
else
    skip 'no python3, so the atomic-write sampler cannot run'
fi

# --- the pointer is machine-local, not tracked ------------------------------

# Tracking it would put a per-machine file in the repo, so each machine would
# fight the last one over its contents on every push.
if git --git-dir="$DOTFILES_ROOT/.cfg" --work-tree="$DOTFILES_ROOT" \
        rev-parse --git-dir >/dev/null 2>&1; then
    tracked=$(git --git-dir="$DOTFILES_ROOT/.cfg" --work-tree="$DOTFILES_ROOT" \
        ls-files ".config/alacritty/alacritty-platform.toml")
    assert_equals 'the generated pointer is not tracked' '' "$tracked"
else
    skip 'no repository here, so the generated pointer cannot be checked against it'
fi

# --- the terminal ground is pure black -------------------------------------
#
# Requested 2026-09-09: keep the Nord status bar, make the terminal's own
# background full black. The two are separable because nvim runs with
# `transparent = true` and Nord's tmux panes use `bg=default`, so both show
# the terminal through, and only Alacritty paints the ground.
#
# Pinned because of how the old value got there. `0x2E3440` is Nord's polar
# night, pasted in as part of the whole Nord palette, and a future palette
# refresh would paste it back. The palette entries are deliberately NOT
# pinned: `black = 0x3B4252` is what keeps the bar looking like Nord, and
# that is wanted.
ground=$(sed -n '/^\[colors.primary\]/,/^\[/p' "$SHARED" | sed -n 's/^background *= *"\(0x[0-9A-Fa-f]*\)".*/\1/p')
assert_equals 'the terminal background is pure black' '0x000000' "$ground"

finish
