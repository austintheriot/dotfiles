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
# Overridable so the oracles below can be exercised against a program that
# is absent. Without that seam a probe cannot prove it distinguishes "the
# parser rejected this" from "there is no parser", which is the bug this
# suite shipped with.
#
# The engine is a Rust binary now, reached as `config-cli deps <verb>`. The
# verb is part of the probe rather than part of this variable, because the
# two verbs take the same flags and both must accept every documented one.
CHECK_SCRIPT=${CHECK_SCRIPT:-config-cli}
HOOK="$DEPS_DIR/depcheck-hook.sh"

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
# The engine exits 2 on an unknown argument, so the parser itself is the
# oracle. Every flag the docs name must be accepted, and every flag the
# parser accepts must be documented.
#
# Only flags on a line that also names `config deps` or depcheck count.
# Both READMEs document other tools whose flags this suite must not try to
# feed to the argument parser.

documented_flags=$(printf '%s\n' "$docs_text" \
    | grep -E 'config deps|depcheck' \
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

# Three outcomes, not two. The oracle used to ask only whether the status
# differed from 2, so 127 ("no such program") read as acceptance and every
# flag passed against a program that was not there.
rejected_flags=''
probes_run=0
missing_program=0
while IFS= read -r flag; do
    [ -n "$flag" ] || continue
    value=$(flag_probe_value "$flag")
    # --dry-run is appended so a probe never mutates this machine, except
    # when the flag under probe IS --dry-run. The engine rejects a repeated
    # flag, so passing it twice reports the parser refusing its own flag.
    # The shell parser tolerated the repeat, which is why this only appeared
    # when the engine became a binary.
    guard=--dry-run
    [ "$flag" = --dry-run ] && guard=''
    if [ -n "$value" ]; then
        # shellcheck disable=SC2086
        "$CHECK_SCRIPT" deps check "$flag" "$value" $guard >/dev/null 2>&1
    else
        # shellcheck disable=SC2086
        "$CHECK_SCRIPT" deps check "$flag" $guard >/dev/null 2>&1
    fi
    probe_status=$?
    probes_run=$((probes_run + 1))
    if [ "$probe_status" -eq 127 ]; then
        missing_program=1
    elif [ "$probe_status" -eq 2 ]; then
        rejected_flags="$rejected_flags $flag"
    fi
done <<< "$documented_flags"

# The positive controls come first: a run that probed nothing, or probed a
# program that does not exist, must not reach the narrow assertion and report
# an empty list as success.
assert_succeeds 'the flag probe ran at least once' test "$probes_run" -gt 0
assert_equals 'the probed program exists' '0' "$missing_program"
assert_equals 'the engine accepts every documented flag' '' "$rejected_flags"

# The arity guard itself, which the probe above deliberately steps around.
# Without it `--only` with nothing after it selects the empty set and reports
# a vacuous success.
"$CHECK_SCRIPT" deps check --only >/dev/null 2>&1
assert_equals 'a value-taking flag rejects a missing value' '2' "$?"

# Harvested from the engine's own --help rather than from its source. The
# shell version grepped a `case` statement, which a compiled binary has no
# equivalent of, and --help is the better oracle regardless: it is what the
# parser publishes, so a flag the parser accepts but never lists is a
# documentation defect on its own.
#
# Both verbs are harvested. They take the same flags today, and a flag added
# to one alone would be a surface the docs cannot describe consistently.
parser_flags=$(
    { "$CHECK_SCRIPT" deps check --help 2>/dev/null
      "$CHECK_SCRIPT" deps install --help 2>/dev/null
    } | grep -oE '^ +--[a-z][a-z-]*' | tr -d ' ' | sort -u
)

# Same blind spot as the probe above, in the other direction: an absent or
# unreadable program harvests nothing, the loop below never runs, and the
# empty-expected assertion passes having compared nothing.
assert_succeeds 'the parser harvest found at least one flag' test -n "$parser_flags"

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

# --- the platform variants both ship ------------------------------------
#
# deps-local.conf used to hold one branch's own list. The deps-mac.conf /
# deps-linux.conf pair replaced it precisely so both variants ship together
# and the platform check at runtime decides which one gets read.
for platform in mac linux; do
    assert_succeeds "deps-$platform.conf ships here" \
        test -f "$DEPS_DIR/deps-$platform.conf"
done

# Greps the FILE, not the file's contents. This passed
# "$deps_readme" (the text) where grep expects a path, so grep warned about a
# filename too long, printed nothing to stdout, and the empty-expected
# assertion passed no matter what the README said. Verified before the fix by
# appending a deps-local.conf mention and watching the assertion still report
# ok.
#
# The positive control matters for the same reason the assertion did not: with
# an unreadable path, "no match" and "the grep never ran" look identical.
assert_succeeds 'the deps README is readable for the retirement check' \
    test -r "$DEPS_README"
assert_succeeds 'the retirement check greps a file with content in it' \
    sh -c 'test -s "$1"' _ "$DEPS_README"

assert_equals 'the retired deps-local.conf is gone' \
    '' "$(grep -n 'deps-local\.conf' "$DEPS_README" || true)"

# --- the documented pipe constraint matches the parser ------------------
#
# The docs warn that a check_command must not contain a pipe. That warning is
# only true while the manifest parser still splits fields on one.
#
# Asserted against the parser's behaviour rather than against its source. The
# shell version grepped for a literal `IFS='|' read` line, which a compiled
# binary has no equivalent of, and a grep for source text could never have
# proved the behaviour anyway.
#
# A four-field line is the probe: if `|` is the delimiter, the fourth field
# makes the line malformed and the engine must reject the manifest. A parser
# that split on something else would accept it.
pipe_probe_conf="$FIXTURES/pipe-probe.conf"
printf 'probe|command -v probe|https://example.invalid|fourth\n' > "$pipe_probe_conf"
DEPS_CONF="$pipe_probe_conf" DEPS_LOCAL_CONF="$FIXTURES/no-such-local.conf" \
    "$CHECK_SCRIPT" deps check --dry-run >/dev/null 2>&1
assert_equals 'a fourth field is rejected, so | is still the delimiter' \
    '2' "$?"

# The positive control. Without it the assertion above passes against an
# engine that rejects every manifest, including a well-formed one.
#
# The check names `sh`, which is present wherever this suite can run, so a
# well-formed manifest exits 0. A dependency that is merely absent exits 1,
# which is not a parse failure and would not distinguish the two outcomes
# this pair exists to compare.
pipe_control_conf="$FIXTURES/pipe-control.conf"
printf 'probe|command -v sh|https://example.invalid\n' > "$pipe_control_conf"
DEPS_CONF="$pipe_control_conf" DEPS_LOCAL_CONF="$FIXTURES/no-such-local.conf" \
    "$CHECK_SCRIPT" deps check --dry-run >/dev/null 2>&1
assert_equals 'a three-field line is accepted' '0' "$?"

# The phrase the probe above proves true. Asserting the warning still states
# the three-field rule is what keeps the prose and the parser in step.
assert_contains 'the deps README warns about the pipe constraint' \
    'exactly three fields' "$deps_readme"

finish
