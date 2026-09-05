#!/bin/bash
#
# Lints every tracked shell script that shellcheck can parse.
#
# The repo already carried SC2086 suppression comments while shellcheck was
# neither installed nor a tracked dependency, so those comments were
# decorative and nothing was linted. This suite makes them load-bearing.
#
# Do not write a literal suppression comment in this file's prose. shellcheck
# 0.9.0 (the version in the Debian bookworm test container) reads one inside
# a comment as a real directive and fails to parse the file, while 0.11.0
# ignores it. The container caught exactly that.
#
# Dialect matters. shellcheck supports sh, bash, dash and ksh, and rejects
# zsh outright. The zsh scripts are excluded explicitly, by name, rather than
# skipped by a silent parse failure: a wrong exclusion should be visible in
# this file, not inferred from a tool's error message. Every excluded name is
# asserted to still exist and to still be zsh, so the list cannot rot into
# excusing a bash script.
#
# Files sourced at shell-init time carry no shebang, so shellcheck cannot
# infer their dialect. Each gets an explicit dialect here for the same reason.
#
# Version skew is real and deliberately not pinned. The Debian bookworm test
# container ships 0.9.0; Homebrew on macOS (local and CI) ships 0.11.0. A
# newer release adds checks, so the strictest environment decides whether a
# push passes, and that is the macOS CI job. Findings are fixed at their site
# rather than suppressed globally, so a version bump surfaces new findings
# instead of silently widening a disable.
#
# Usage: ~/tests/shellcheck.test.sh

. "$(dirname "$0")/lib.sh"

cd "$DOTFILES_ROOT" || exit 1

# The linter is a tracked dependency (.scripts/deps/deps.conf), is installed
# in the test container (tests/docker/Dockerfile) and on both CI runners
# (.github/workflows/test-suite.yml). Those are the three places the lint has
# to gate, so a missing binary there is a broken gate, not a soft skip: the
# suite FAILS rather than passing quietly. Only an ad-hoc host run is allowed
# to skip, and it says so loudly.
if ! command -v shellcheck >/dev/null 2>&1; then
    if [ -n "${CI:-}" ] || [ -f /.dockerenv ]; then
        assert_equals 'shellcheck is installed where the lint has to gate' \
            'installed' 'missing'
        finish
        exit 1
    fi
    skip 'shellcheck is not installed; `config install` adds it'
    finish
    exit 0
fi

# --- the lint has to actually gate -----------------------------------------
#
# A lint suite that exists but never runs where a push is blocked is
# decorative, which is the exact failure this whole item was filed against.
# Three places have to carry shellcheck, and each is asserted here rather
# than assumed:
#   - .scripts/deps/deps.conf     so `config install` provides it locally
#   - tests/docker/Dockerfile     so the pre-push gate runs it (container.test.sh
#                                 also asserts this, from the image's side)
#   - .github/workflows/test-suite.yml  so both CI runners run it

DEPS_CONF="$DOTFILES_ROOT/.scripts/deps/deps.conf"
DOCKERFILE="$DOTFILES_ROOT/tests/docker/Dockerfile"
CI_WORKFLOW="$DOTFILES_ROOT/.github/workflows/test-suite.yml"

assert_succeeds 'shellcheck is a tracked dependency' \
    grep -q '^shellcheck|' "$DEPS_CONF"

assert_succeeds 'the test container installs shellcheck' \
    grep -q 'shellcheck' "$DOCKERFILE"

# Both runners, not just one. A lint that gates on Linux and not macOS lets a
# macOS-only script regress.
#
# The workflow no longer carries a per-platform package list to grep: it calls
# check-deps.sh with an --only set, and the engine resolves each name to the
# right package for whichever manager the runner has. So the fact to assert is
# that shellcheck is in that set, and that the step naming it is not gated to
# one platform -- an `if: runner.os == ...` on that step would restore exactly
# the one-platform gap this pair of assertions exists to prevent.
only_step=$(grep -n 'check-deps.sh --fix --yes' -A 2 "$CI_WORKFLOW" || true)
assert_contains 'CI installs shellcheck through the deps engine' 'shellcheck' "$only_step"

install_step_block=$(awk '/Install the suite.s dependencies \(from deps.conf\)/{found=1} found && /^      - name:/ && ++seen>1{exit} found' "$CI_WORKFLOW")
assert_equals 'the dependency step runs on both runners' '' \
    "$(printf '%s\n' "$install_step_block" | grep 'runner.os' || true)"

# --- the two exclusion lists ------------------------------------------------
#
# ZSH_SCRIPTS: shellcheck refuses zsh, so these are never linted.
# SOURCED_BASH: no shebang because they are sourced, linted as bash.

ZSH_SCRIPTS='.scripts/tmux-close.sh
.scripts/tmux-setup.sh
.scripts/tmux-split.sh
.scripts/tmux-start.sh
.scripts/zsh-git-widgets.sh
tests/leak-check.sh'

# depcheck-hook.sh is sourced from .zshrc but is portable POSIX shell, not
# zsh-specific, so it is linted rather than excluded.
SOURCED_BASH='tests/lib.sh
.scripts/deps/depcheck-hook.sh'

# Every excluded name must still exist. A rename would otherwise leave a dead
# entry here and silently drop a real script from the lint.
dead=''
while IFS= read -r script; do
    [ -n "$script" ] || continue
    [ -f "$script" ] || dead="$dead $script"
done <<EOF
$ZSH_SCRIPTS
$SOURCED_BASH
EOF
assert_equals 'every excluded script still exists' '' "$dead"

# Every zsh exclusion must still be zsh. If one is rewritten in bash, it
# belongs in the lint, and this is what says so.
not_zsh=''
while IFS= read -r script; do
    [ -n "$script" ] || continue
    [ -f "$script" ] || continue
    head -1 "$script" | grep -q '^#!.*zsh' && continue
    # A sourced zsh file has no shebang; zsh-only syntax is what marks it.
    grep -qE 'zle |autoload -Uz|zmodload|^emulate ' "$script" && continue
    not_zsh="$not_zsh $script"
done <<EOF
$ZSH_SCRIPTS
EOF
assert_equals 'every zsh exclusion is still a zsh script' '' "$not_zsh"

# A shebang-less file in SOURCED_BASH must still be shebang-less. Once it
# grows one, shellcheck infers the dialect and the override is stale.
has_shebang=''
while IFS= read -r script; do
    [ -n "$script" ] || continue
    [ -f "$script" ] || continue
    head -1 "$script" | grep -q '^#!' && has_shebang="$has_shebang $script"
done <<EOF
$SOURCED_BASH
EOF
assert_equals 'every sourced-bash override is still shebang-less' '' \
    "$has_shebang"

# --- the lint ---------------------------------------------------------------

# Codes excluded everywhere, as flags rather than a .shellcheckrc: the rc file
# needs shellcheck 0.10+ and would have to be copied into the test image,
# while -e works on every version this repo runs against.
#
#   SC1091  sourced path shellcheck cannot resolve. -x follows tests/lib.sh
#           where the working directory allows it; the container runs from a
#           different directory and cannot, and that is not a defect.
#   SC2016  this repo prints shell commands as text (install instructions the
#           reader runs) and passes shell snippets to another shell or to
#           grep. Both are correctly single-quoted; expanding them is the bug.
EXCLUDES='SC1091,SC2016'

is_excluded() {
    printf '%s\n' "$ZSH_SCRIPTS" | grep -qxF "$1"
}

# Tracked files only. The worktree is the whole home directory, so a plain
# find would walk every vendored plugin and cache in it.
if git --git-dir="$DOTFILES_ROOT/.cfg" --work-tree="$DOTFILES_ROOT" \
        rev-parse --verify HEAD >/dev/null 2>&1; then
    scripts=$(git --git-dir="$DOTFILES_ROOT/.cfg" --work-tree="$DOTFILES_ROOT" \
        ls-files '*.sh')
else
    # The container has no repository, so fall back to the directories this
    # repo owns rather than walking $HOME.
    scripts=$(find .scripts tests .claude/hooks -name '*.sh' -type f 2>/dev/null \
        | sed 's|^\./||' | sort)
fi

assert_succeeds 'found shell scripts to lint' test -n "$scripts"

linted=0
findings=''
while IFS= read -r script; do
    [ -n "$script" ] || continue
    [ -f "$script" ] || continue
    is_excluded "$script" && continue

    if printf '%s\n' "$SOURCED_BASH" | grep -qxF "$script"; then
        output=$(shellcheck -x -e "$EXCLUDES" -s bash -f gcc "$script" 2>&1) || true
    else
        output=$(shellcheck -x -e "$EXCLUDES" -f gcc "$script" 2>&1) || true
    fi
    linted=$((linted + 1))
    [ -n "$output" ] && findings="$findings
$output"
done <<EOF
$scripts
EOF

# A lint that checks nothing passes trivially. This is the assertion that
# stops a broken discovery step from reading as a clean run.
[ "$linted" -ge 25 ] && enough=yes || enough="no (only $linted linted)"
assert_equals 'the lint covers at least 25 scripts' 'yes' "$enough"

# assert_equals prints the actual value on failure, which is the finding
# list itself, so there is no separate report to write here.
findings=$(printf '%s' "$findings" | grep -v '^$' || true)
assert_equals 'shellcheck reports no findings' '' "$findings"

finish
