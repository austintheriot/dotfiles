#!/bin/bash
#
# Tests for the dependency-checking documentation: .scripts/deps/README.md
# and the "Dependency checking" section of ~/README.md.
#
# Scope is deliberately narrow. These assert only the facts that rot
# silently: a cited file path that gets renamed, a documented flag the
# argument parser stops accepting, an alias definition that drifts from the
# hook, a dependency list that falls behind deps.conf. Prose, wording, and
# section order are not asserted, because freezing those would make every
# edit to the writing a test failure.
#
# Every assertion reads through $DOTFILES_ROOT, so pointing that at a copy
# of the tree with one thing mutated is what proves an assertion can fail.
#
# Usage: ~/tests/deps-docs.test.sh

. "$(dirname "$0")/lib.sh"

DEPS_DIR="$DOTFILES_ROOT/.scripts/deps"
DEPS_README="$DEPS_DIR/README.md"
HOME_README="$DOTFILES_ROOT/README.md"
CHECK_SCRIPT="$DEPS_DIR/check-deps.sh"
HOOK="$DEPS_DIR/depcheck-hook.sh"
MANIFEST="$DOTFILES_ROOT/.sync-manifest"

# --- both documents exist ----------------------------------------------

assert_succeeds 'the deps README exists' test -f "$DEPS_README"
assert_succeeds 'the home README exists' test -f "$HOME_README"

deps_readme=$(cat "$DEPS_README" 2>/dev/null)
home_readme=$(cat "$HOME_README" 2>/dev/null)
docs_text="$deps_readme
$home_readme"

# --- every repo-relative path the docs cite resolves --------------------
#
# Paths are extracted from the prose rather than listed here. A hardcoded
# list would pass while the docs cited something else entirely.

# A citation is either rooted (a leading `.` or `~/`, resolved against
# $DOTFILES_ROOT) or bare (`test-local.sh`, resolved against this directory,
# which is how the deps README refers to its own neighbours).
cited_paths=$(printf '%s\n' "$docs_text" \
    | grep -oE '`[^`]+`' \
    | tr -d '`' \
    | grep -E '^(\.|~/)?[A-Za-z0-9._/-]+$' \
    | grep -E '\.(sh|conf|yml|md)$|Dockerfile\.[a-z]+$' \
    | sed -e 's|^~/||' -e 's|^\./||' \
    | sort -u)

assert_succeeds 'the docs cite at least one file path' test -n "$cited_paths"

missing_paths=''
while IFS= read -r cited; do
    [ -n "$cited" ] || continue
    [ -e "$DOTFILES_ROOT/$cited" ] && continue
    [ -e "$DEPS_DIR/$cited" ] && continue
    missing_paths="$missing_paths $cited"
done <<< "$cited_paths"
assert_equals 'every cited file path exists' '' "$missing_paths"

# --- the documented depcheck alias matches the hook ---------------------

hook_alias=$(grep -E "^alias depcheck=" "$HOOK" 2>/dev/null \
    | sed -e "s/^alias depcheck='//" -e "s/'$//")
assert_succeeds 'the hook defines a depcheck alias' test -n "$hook_alias"

# The README states the expansion once, as a literal `alias depcheck=...`
# line copied from the hook. Matching that exact line is what makes the
# assertion fail when the hook's definition changes. Searching the whole
# file instead would match the same command spelled out in the flag
# reference below it, which lets the alias drift unnoticed.
assert_contains 'the deps README documents the depcheck alias expansion' \
    "alias depcheck='$hook_alias'" "$deps_readme"

# --- the documented flags are the flags the parser accepts --------------
#
# check-deps.sh exits 2 on an unknown argument, so the parser itself is the
# oracle. Every flag the docs name must be accepted, and every flag the
# parser accepts must be documented.
#
# Only flags on a line that also names check-deps.sh or depcheck count.
# Both READMEs document other tools whose flags this script must not try to
# feed to the argument parser.

documented_flags=$(printf '%s\n' "$docs_text" \
    | grep -E 'check-deps\.sh|depcheck' \
    | grep -oE '\-\-[a-z][a-z-]*' | sort -u)
assert_succeeds 'the docs name at least one flag' test -n "$documented_flags"

# A flag that takes a value cannot be probed bare: it exits 2 on purpose,
# because selecting the empty set and reporting success would be worse. Give
# each one a valid value so the probe tests acceptance rather than arity.
#
# The value must name a real dependency, since --only rejects a name that
# matches no entry -- also on purpose, so a typo in a workflow cannot install
# nothing and pass.
flag_probe_value() {
    case $1 in
        --only) printf 'git' ;;
    esac
}

rejected_flags=''
while IFS= read -r flag; do
    [ -n "$flag" ] || continue
    value=$(flag_probe_value "$flag")
    if [ -n "$value" ]; then
        "$CHECK_SCRIPT" "$flag" "$value" --dry-run >/dev/null 2>&1
    else
        "$CHECK_SCRIPT" "$flag" --dry-run >/dev/null 2>&1
    fi
    [ "$?" -ne 2 ] || rejected_flags="$rejected_flags $flag"
done <<< "$documented_flags"
assert_equals 'check-deps.sh accepts every documented flag' '' "$rejected_flags"

# The arity guard itself, which the probe above deliberately steps around.
# Without it `--only` with nothing after it selects the empty set and reports
# a vacuous success.
"$CHECK_SCRIPT" --only >/dev/null 2>&1
assert_equals 'a value-taking flag rejects a missing value' '2' "$?"

parser_flags=$(grep -oE '^ +--[a-z-]+\)' "$CHECK_SCRIPT" 2>/dev/null \
    | tr -d ' )' | sort -u)
undocumented_flags=''
while IFS= read -r flag; do
    [ -n "$flag" ] || continue
    case "$deps_readme" in
        *"$flag"*) ;;
        *) undocumented_flags="$undocumented_flags $flag" ;;
    esac
done <<< "$parser_flags"
assert_equals 'the docs name every flag the parser accepts' '' "$undocumented_flags"

# --- every dependency the docs list is really in deps.conf --------------
#
# The docs name specific dependencies when explaining the non-binary and
# platform-tolerant checks. Those names are the ones that go stale when an
# entry is renamed or moved to deps-local.conf.

conf_names=$(sed -e 's/#.*//' "$DEPS_DIR/deps.conf" 2>/dev/null \
    | cut -d'|' -f1 | grep -E '^[a-z]' | sort -u)

documented_deps=$(printf '%s\n' "$docs_text" \
    | grep -oE '`[a-z][a-z0-9-]+`' \
    | tr -d '`' \
    | sort -u)

# Only names that look like a manifest entry are checked, so ordinary prose
# in backticks (a command name, a package manager) is not mistaken for a
# dependency claim.
GUARDED_DEPS='zsh-autosuggestions tpm nvm rustup zoxide alacritty neovim ripgrep fzf'

absent_deps=''
while IFS= read -r candidate; do
    [ -n "$candidate" ] || continue
    case " $GUARDED_DEPS " in
        *" $candidate "*)
            printf '%s\n' "$conf_names" | grep -qx "$candidate" \
                || absent_deps="$absent_deps $candidate"
            ;;
    esac
done <<< "$documented_deps"
assert_equals 'every dependency the docs name is in deps.conf' '' "$absent_deps"

# A guarded name the docs never mention in backticks is never reached by the
# loop above, so it contributes nothing while making the guard list look
# broader than it is. `ripgrep` sat here in bare prose: renaming its
# deps.conf entry left this suite green. Requiring every guarded name to be
# extractable is what keeps the list honest as the prose is edited.
unreached_guards=''
for guarded in $GUARDED_DEPS; do
    printf '%s\n' "$documented_deps" | grep -qx "$guarded" \
        || unreached_guards="$unreached_guards $guarded"
done
assert_equals 'every guarded dependency name appears in the docs in backticks' \
    '' "$unreached_guards"

# --- oh-my-zsh is documented as not shared, and really is not -----------

assert_contains 'the deps README says oh-my-zsh is not in the shared file' \
    'oh-my-zsh' "$deps_readme"
oh_my_zsh_shared=$(printf '%s\n' "$conf_names" | grep -cx 'oh-my-zsh')
assert_equals 'oh-my-zsh is absent from deps.conf, as documented' \
    '0' "$oh_my_zsh_shared"

# --- the platform variants are shared, not excluded ---------------------
#
# deps-local.conf used to be excluded from .sync-manifest so each branch
# could carry its own. The variants replaced it precisely to end that: both
# ship on both branches, so both are inside the drift check. An exclusion
# reappearing would silently take them back out of it.
for platform in mac linux; do
    assert_succeeds "deps-$platform.conf ships here" \
        test -f "$DEPS_DIR/deps-$platform.conf"
    assert_equals "the manifest does not exclude deps-$platform.conf" \
        '' "$(grep -nxF "!.scripts/deps/deps-$platform.conf" "$MANIFEST")"
done

assert_equals 'the retired deps-local.conf is gone' \
    '' "$(grep -n 'deps-local\.conf' "$deps_readme")"

# --- the documented pipe constraint matches read_entries ----------------
#
# The docs warn that a check_command must not contain a pipe. That warning
# is only true while read_entries still splits on one.

assert_succeeds 'read_entries still splits fields on a pipe' \
    grep -qF "IFS='|' read -r name check docs" "$CHECK_SCRIPT"
assert_contains 'the deps README warns about the pipe constraint' \
    'read_entries' "$deps_readme"

finish
