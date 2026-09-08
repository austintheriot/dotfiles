#!/bin/bash
#
# Tests the per-command help surface: `config <sub> --help` and the flag
# spellings each subcommand accepts.
#
# `config help` answers "which commands exist". It does not answer "what does
# `config test -q` do", and that second question is the one a reader has when
# they are already at the right command. Each config-<sub> answers it for
# itself, from a `# usage:` block at the top of the script, for the same
# reason the one-line listing is generated: a hand-maintained second copy of
# the flags is the copy that goes stale.
#
# The block runs from the first `# usage:` line to the first line that is not
# a comment. Everything in it prints verbatim, so the script's own header is
# the help text and there is nothing to keep in step.
#
# Usage: ~/tests/config-usage.test.sh

. "$(dirname "$0")/lib.sh"

CONFIG_DIR="$DOTFILES_ROOT/.scripts/config"
CONFIG="$CONFIG_DIR/config"

# The subcommands that parse flags of their own, and so owe the reader a
# description of each one. The thin wrapper (install) forwards to a program
# that prints its own help, and reload and stamp take no flags.
FLAG_TAKING='test'

# Every subcommand, flags or not, answers --help. A reader who types it should
# never get git usage or a stack trace.
#
# Discovered from the directory rather than listed by hand. A hand-written
# list is a second copy of the same facts, and a subcommand missing from it
# is not reported as a failure -- it is simply never checked, which is the
# quietest way for this suite to stop covering something.
# shellcheck disable=SC2012  # config-<sub> names cannot contain spaces
ALL_SUBCOMMANDS=$(cd "$CONFIG_DIR" && ls config-* | sed 's/^config-//' | tr '\n' ' ')

make_fixture_home() {
    fixture_home="$FIXTURES/home-$1"
    mkdir -p "$fixture_home"
    seed=$(make_repo "seed-$1")
    git clone -q --bare "$seed" "$fixture_home/.cfg"
    git --git-dir="$fixture_home/.cfg" config status.showUntrackedFiles no
    printf '%s\n' "$fixture_home"
}

home=$(make_fixture_home usage)

# Stubs for the programs the thin wrappers exec. A wrapper must answer --help
# itself rather than handing it to a program that may not be installed.
shim_dir="$FIXTURES/shims"
mkdir -p "$shim_dir"
# The binary every thin wrapper execs: `config deps`, `config install`, and
# now `config test` all resolve it by name on PATH rather than under $HOME.
#
# `help` is the exception this stub forwards rather than echoes. Every other
# verb here is asserted on for argv forwarding, which an echo answers, but the
# listing's content is what this suite checks about `config help`, and an echo
# would make those assertions pass against "deps:help" while proving nothing.
# Forwarded to the real binary found on the outer PATH, which is the same
# binary `config help` reaches outside the fixture.
real_config_cli=$(command -v config-cli 2>/dev/null || true)
cat > "$shim_dir/config-cli" <<STUB
#!/bin/sh
if [ "\${1:-}" = help ] && [ -x "$real_config_cli" ]; then
    exec "$real_config_cli" "\$@"
fi
printf "deps:%s\\n" "\$@"
STUB
chmod 755 "$shim_dir/config-cli"
mkdir -p "$home/deps" "$home/tests"

run_config() {
    HOME="$home" PATH="$shim_dir:$PATH" "$CONFIG" "$@"
}

# --- every subcommand answers --help ----------------------------------------

for sub in $ALL_SUBCOMMANDS; do
    output=$(run_config "$sub" --help 2>&1)
    status=$?
    assert_equals "config $sub --help exits 0" '0' "$status"
    assert_contains "config $sub --help names the command" "config $sub" "$output"
done

# -h is the spelling people try when --help is too long, and it must reach the
# same text rather than a different one.
for sub in $ALL_SUBCOMMANDS; do
    long=$(run_config "$sub" --help 2>&1)
    short=$(run_config "$sub" -h 2>&1)
    assert_equals "config $sub -h prints the same text as --help" "$long" "$short"
done

# --help must not run the command. A subcommand that ran its work first and
# printed help after would rebuild, or push, or start a watch loop.
output=$(run_config test --help 2>&1)
assert_equals 'config test --help does not run the suite' '' \
    "$(printf '%s' "$output" | grep -F 'all:' || true)"

output=$(run_config install --help 2>&1)
assert_equals 'config install --help does not exec the deps engine' '' \
    "$(printf '%s' "$output" | grep -F 'deps:' || true)"

output=$(run_config deps --help 2>&1)
assert_equals 'config deps --help does not exec the deps engine' '' \
    "$(printf '%s' "$output" | grep -F 'deps:' || true)"

# install-hooks is the sharpest case: before this suite, `config install-hooks
# --help` linked the hooks and rewrote ~/.local/bin/config, because the script
# ignored its arguments entirely. Asking a command what it does must not be
# the same as doing it.
rm -f "$home/.cfg/hooks/pre-commit" "$home/.cfg/hooks/pre-push" \
    "$home/.local/bin/config"
printf '#!/bin/sh\nexit 0\n' > "$home/tests/pre-commit"
printf '#!/bin/sh\nexit 0\n' > "$home/tests/pre-push"
chmod 755 "$home/tests/pre-commit" "$home/tests/pre-push"
run_config install-hooks --help >/dev/null 2>&1
assert_succeeds 'config install-hooks --help does not link pre-commit' \
    test ! -e "$home/.cfg/hooks/pre-commit"
assert_succeeds 'config install-hooks --help does not link the dispatcher' \
    test ! -e "$home/.local/bin/config"

# --- the usage block is the source ------------------------------------------

# A `# usage:` block in the script is what prints, so the help text and the
# script header cannot disagree.
missing_block=''
for sub in $ALL_SUBCOMMANDS; do
    grep -q '^# usage:' "$CONFIG_DIR/config-$sub" || missing_block="$missing_block $sub"
done
assert_equals 'every config-<sub> carries a "# usage:" block' '' "$missing_block"

for sub in $ALL_SUBCOMMANDS; do
    first_line=$(sed -n 's/^# usage: //p' "$CONFIG_DIR/config-$sub" | head -1)
    output=$(run_config "$sub" --help 2>&1)
    # An empty needle would make assert_contains pass for any output, which
    # is exactly the state this suite starts in. Assert the needle first.
    assert_succeeds "config-$sub has a usage line to print" test -n "$first_line"
    assert_contains "config $sub --help prints its own usage line" \
        "$first_line" "$output"
done

# --- the helper is part of the set ------------------------------------------

# Sourcing usage.sh gives every subcommand a runtime dependency on a file
# beside it. The dispatcher's whole design is "exec whatever sits beside me",
# so a copy that takes the config-<sub> scripts and leaves the helper behind
# is a shape that can really happen. Name the dependency here, so it is a
# stated fact rather than something a reader infers from a not-found error.
assert_succeeds 'the usage helper sits beside the subcommands' \
    test -f "$CONFIG_DIR/usage.sh"

# It must not be named config-*, or the dispatcher would treat it as a
# subcommand called `usage.sh` and `config help` would list it.
assert_succeeds 'the helper is outside the config-<sub> namespace' \
    test ! -e "$CONFIG_DIR/config-usage.sh"

not_sourcing=''
for sub in $ALL_SUBCOMMANDS; do
    grep -q 'usage\.sh' "$CONFIG_DIR/config-$sub" || not_sourcing="$not_sourcing $sub"
done
assert_equals 'every config-<sub> sources the shared helper' '' "$not_sourcing"

# --- the block terminator ---------------------------------------------------

# A `# ---` line ends the block, so a script can follow its help with a note
# for whoever edits it. config-install-hooks does exactly this: its
# trust-boundary rationale answers a maintainer's question, and printing it to
# someone who asked what the command does buries the actual answer.
output=$(run_config install-hooks --help 2>&1)
assert_contains 'install-hooks help keeps the text above the terminator' \
    'Takes no options' "$output"
assert_equals 'install-hooks help stops at the terminator' '' \
    "$(printf '%s' "$output" | grep -F 'trust boundary' || true)"
assert_equals 'the terminator line itself never prints' '' \
    "$(printf '%s' "$output" | grep -x -- '---' || true)"

# The terminator must not be a bare `#`. Every block uses one to separate its
# own paragraphs, so a bare `#` would truncate each block at its first blank
# line. config test has four paragraphs; the last one has to survive.
output=$(run_config test --help 2>&1)
assert_contains 'a multi-paragraph block prints past its first blank line' \
    'Exits 2 on a usage error' "$output"

# --- flags are described ----------------------------------------------------

# Every flag the script parses appears in its help text with a description
# beside it. A flag that is accepted but undocumented is the gap this suite
# exists to close.
for sub in $FLAG_TAKING; do
    output=$(run_config "$sub" --help 2>&1)
    parsed=$(grep -o '^\( *\)\(-[a-z]\||--[a-z-]*\))' "$CONFIG_DIR/config-$sub" \
        | grep -o '\-\{1,2\}[a-z][a-z-]*' | sort -u)
    undocumented=''
    while IFS= read -r flag; do
        [ -n "$flag" ] || continue
        printf '%s\n' "$output" | grep -qF -- "$flag" || undocumented="$undocumented $flag"
    done <<INNER
$parsed
INNER
    assert_equals "every flag config $sub parses is described in its help" \
        '' "$undocumented"
done

# --- short and long spellings -----------------------------------------------

# `config test` is a shim now: it execs `config-cli test "$@"` unparsed, and
# config-cli owns both spellings of every flag. The fixture's config-cli stub
# prints "deps:<arg>" per argument, on its own line, so these assertions
# check what reaches the binary rather than what a shell case statement used
# to do with it.
actual=$(run_config test -q)
assert_equals 'config test -q passes -q through' \
    "$(printf 'deps:test\ndeps:-q')" "$actual"

actual=$(run_config test --quiet)
assert_equals 'config test --quiet is the long spelling of -q' \
    "$(printf 'deps:test\ndeps:--quiet')" "$actual"

actual=$(run_config test --docker)
assert_equals 'config test --docker runs the docker suite' \
    "$(printf 'deps:test\ndeps:--docker')" "$actual"

actual=$(run_config test -d)
assert_equals 'config test -d is the short spelling of --docker' \
    "$(printf 'deps:test\ndeps:-d')" "$actual"

# -w/--watch and an unrecognized flag are both left to config-cli now: the
# shim no longer validates anything, so there is nothing left for this suite
# to probe about flag combinations or unknown flags. config-cli's own
# argv-surface tests (config-cli/tests/test_runner_behavior.rs) cover those.
actual=$(run_config test -w --docker)
assert_equals 'config test -w reaches config-cli alongside --docker' \
    "$(printf 'deps:test\ndeps:-w\ndeps:--docker')" "$actual"

# --- the --describe contract -------------------------------------------------

# config-help builds its listing by running `sed -n 's/^# help: //p'` over each
# subcommand's source text. Pointed at a compiled binary, that sed writes
# "RE error: illegal byte sequence" to stderr and the pipeline still exits 0,
# because `head -1` is the last stage and supplies the status. So the
# description silently becomes empty and a linter-style error leaks into the
# listing. Each subcommand answering for itself removes the assumption that a
# subcommand is readable text.

for sub in $ALL_SUBCOMMANDS; do
    described=$(run_config "$sub" --describe 2>/dev/null)
    status=$?
    assert_equals "config $sub --describe exits 0" '0' "$status"
    # An empty description would make the line-count and match assertions
    # below vacuous, so assert the string exists before asserting its shape.
    assert_succeeds "config $sub --describe prints something" \
        test -n "$described"
done

for sub in $ALL_SUBCOMMANDS; do
    line_count=$(run_config "$sub" --describe 2>/dev/null | grep -c '')
    assert_equals "config $sub --describe prints exactly one line" \
        '1' "$line_count"
done

# config-help formats the value with `printf '  %-14s %s\n'`, so a trailing
# blank line or a second line breaks the column the listing is read in.
for sub in $ALL_SUBCOMMANDS; do
    described=$(run_config "$sub" --describe 2>/dev/null | sed -n '1p')
    assert_equals "config $sub --describe has no leading whitespace" \
        "$described" "$(printf '%s' "$described" | sed 's/^[[:space:]]*//')"
done

# The `# help:` comment stays the single home of the string. A subcommand that
# grew a second, hand-written copy would be free to disagree with the comment
# that config.test.sh and the README both point contributors at.
for sub in $ALL_SUBCOMMANDS; do
    comment=$(sed -n 's/^# help: //p' "$CONFIG_DIR/config-$sub" | head -1)
    described=$(run_config "$sub" --describe 2>/dev/null)
    assert_succeeds "config-$sub has a '# help:' line to describe from" \
        test -n "$comment"
    assert_equals "config $sub --describe matches its own '# help:' line" \
        "$comment" "$described"
done

# stdout carries the description; stderr carries nothing. The leaked sed error
# on the shared terminal is the failure mode this contract exists to remove.
for sub in $ALL_SUBCOMMANDS; do
    noise=$(run_config "$sub" --describe 2>&1 >/dev/null)
    assert_equals "config $sub --describe writes nothing to stderr" \
        '' "$noise"
done

# Asking a command to describe itself must not run it, for the same reason
# --help must not: install-hooks once linked the hooks and rewrote
# ~/.local/bin/config before printing anything.
rm -f "$home/.cfg/hooks/pre-commit" "$home/.cfg/hooks/pre-push" \
    "$home/.local/bin/config"
run_config install-hooks --describe >/dev/null 2>&1
assert_succeeds 'config install-hooks --describe does not link pre-commit' \
    test ! -e "$home/.cfg/hooks/pre-commit"
assert_succeeds 'config install-hooks --describe does not link the dispatcher' \
    test ! -e "$home/.local/bin/config"

# Asserted positively: --describe prints exactly the `# help:` line and
# nothing else. The earlier version grepped each wrapper's output for a
# marker its exec'd program would emit, which failed three ways. The
# `config-manifest` stub's `manifest:` marker was emitted by nothing, so that
# assertion passed even with the --describe handling removed entirely
# (mutation-confirmed). `all:` and `deps:` did fire,
# but only as substrings of `run-all:` and `config deps:`, so renaming either
# program would have silenced them without a failure.
#
# A one-line-exact comparison needs no knowledge of what the exec'd program
# prints: any exec adds output, and any output that is not the description
# fails.
for described in test install doctor; do
    expected_line=$(sed -n 's/^# help: //p' "$CONFIG_DIR/config-$described" | head -1)

    # Positive control: the shim must carry a help line, or the comparison
    # below would be between two empty strings.
    assert_succeeds "config $described carries a help line to describe" \
        test -n "$expected_line"

    output=$(run_config "$described" --describe 2>&1)
    assert_equals "config $described --describe prints only its description" \
        "$expected_line" "$output"
done

# print_describe takes the file to describe, defaulting to $0. Without the
# argument the only way to test it against a named file is to copy a script
# under a new name, and a stray config-* left in the real directory changes
# what `config help` lists for every other suite.
describe_fixture="$FIXTURES/describe-subject"
printf '#!/bin/sh\n# help: A description read from an argument\n' \
    > "$describe_fixture"
described=$(. "$CONFIG_DIR/usage.sh" && print_describe "$describe_fixture")
assert_equals 'print_describe reads the file it is given' \
    'A description read from an argument' "$described"


# --- the listing consumes --describe -----------------------------------------

# The listing asks each subcommand rather than reading it. A subcommand that is
# a compiled binary has no `# help:` line to grep, and grepping one anyway put
# "sed: RE error: illegal byte sequence" on stderr and an empty description in
# the column.
listing=$(run_config help 2>/dev/null)
assert_succeeds 'config help prints a listing' test -n "$listing"

# Every real subcommand's description still reaches the column. This is the
# regression guard on the switch itself: the listing has to say the same thing
# it said when it was grepping source.
undescribed=''
for sub in $ALL_SUBCOMMANDS; do
    described=$(run_config "$sub" --describe 2>/dev/null)
    [ -n "$described" ] || { undescribed="$undescribed $sub"; continue; }
    printf '%s\n' "$listing" | grep -qF "$described" \
        || undescribed="$undescribed $sub"
done
assert_equals 'config help prints every subcommand description' '' \
    "$undescribed"

assert_equals 'the listing has no undocumented entries of its own' '' \
    "$(printf '%s' "$listing" | grep -F '(undocumented)' || true)"

# The odd-shaped siblings run against a copy of the directory rather than the
# real one. config-help resolves its siblings from its own $0, so a copy is a
# faithful harness, and a stray config-* in the real .scripts/config would
# change what every other suite counts there.
listing_dir="$FIXTURES/listing-dir"
mkdir -p "$listing_dir"
cp "$CONFIG_DIR/config-help" "$CONFIG_DIR/usage.sh" "$listing_dir/"

# A binary subcommand, standing in for the first ported one. Not a shell
# script: the point is that the listing works on a file no sed can read. `cp`
# of a real binary rather than a crafted file, so the bytes are whatever a
# compiler actually emits.
if [ -x /bin/echo ]; then
    cp /bin/echo "$listing_dir/config-fixturebin"
    chmod 755 "$listing_dir/config-fixturebin"
    # /bin/echo answers --describe by printing "--describe", which is one line
    # on stdout with exit 0. That satisfies the contract, which is what makes
    # it usable as a stand-in here: the assertion under test is that the
    # listing reads stdout and emits no sed error, not what the text says.
    output=$("$listing_dir/config-help" 2>&1)
    assert_contains 'config help lists a binary subcommand' 'fixturebin' \
        "$output"
    assert_equals 'a binary subcommand produces no sed error in the listing' \
        '' "$(printf '%s' "$output" | grep -F 'illegal byte sequence' || true)"
    assert_equals 'a binary subcommand produces no sed error at all' '' \
        "$(printf '%s' "$output" | grep -F 'sed:' || true)"
    rm -f "$listing_dir/config-fixturebin"
else
    skip '/bin/echo is missing, so there is no binary to stand in for a ported subcommand'
fi

# A config-* sibling that is not executable cannot be reached through the
# dispatcher (.scripts/config/config requires -x before it execs), so the
# listing cannot execute it either. It must degrade to (undocumented) rather
# than emitting a not-found error into the column.
printf '#!/bin/sh\n# help: never runs\n' > "$listing_dir/config-fixtureinert"
chmod 644 "$listing_dir/config-fixtureinert"
output=$("$listing_dir/config-help" 2>&1)
assert_contains 'config help still lists a non-executable sibling' \
    'fixtureinert' "$output"
assert_contains 'a non-executable sibling is marked undocumented' \
    '(undocumented)' "$output"
assert_equals 'a non-executable sibling produces no exec error' '' \
    "$(printf '%s' "$output" | grep -iE 'permission denied|not found' || true)"
# The `# help:` line is present and would be found by a source grep, so the
# assertion above only means anything while the description does not come from
# reading the file.
assert_equals 'the non-executable sibling is described by asking, not reading' \
    '' "$(printf '%s' "$output" | grep -F 'never runs' || true)"
rm -f "$listing_dir/config-fixtureinert"

# --- print_usage does not degrade silently -----------------------------------

# print_usage reads $0, so it cannot delegate to a subprocess the way the
# listing now does: the whole point is that it prints the block out of the
# script the reader asked about. What it can stop doing is printing an empty
# block plus a sed error. A shell subcommand rewritten as a binary without
# porting its help surface is the mistake this catches.
#
# The subject is passed as an argument. $0 for a sourced function is the
# script it was sourced into, so `sh -c '...' _ ...` gives $0 the value `_`,
# and an argument is the only way to name a subject at all.

# Positive control: pointed at a text script, print_usage prints that script's
# block. Without this, the failure assertions below pass when the call is
# simply broken.
text_output=$(sh -c '. "$1/usage.sh"; print_usage "$2"' \
    _ "$CONFIG_DIR" "$CONFIG_DIR/config-help" 2>&1) || true
assert_contains 'print_usage prints the block of the file it is given' \
    'usage: config help' "$text_output"

if [ -x /bin/echo ]; then
    binary_subject="$FIXTURES/binary-usage-subject"
    cp /bin/echo "$binary_subject"
    chmod 755 "$binary_subject"
    binary_output=$(sh -c '. "$1/usage.sh"; print_usage "$2"' \
        _ "$CONFIG_DIR" "$binary_subject" 2>&1) || true
    assert_equals 'print_usage on a binary emits no sed error' '' \
        "$(printf '%s' "$binary_output" | grep -F 'illegal byte sequence' || true)"
    assert_contains 'print_usage on a binary says what went wrong' \
        'not a text file' "$binary_output"
    sh -c '. "$1/usage.sh"; print_usage "$2"' \
        _ "$CONFIG_DIR" "$binary_subject" >/dev/null 2>&1
    status=$?
    assert_equals 'print_usage on a binary returns non-zero' '1' "$status"
else
    skip '/bin/echo is missing, so there is no binary to point print_usage at'
fi

# --- the listing renders identically -----------------------------------------

# The point of the --describe migration is that nothing about `config help`
# changed. A recorded expectation is the only assertion that proves it: the
# column is built with `printf '  %-14s %s\n'`, so a description that gained a
# trailing newline or a second line would shift every row after it and no other
# test in this suite would notice.
#
# The expectation was captured from `config help` before the first --describe
# commit, so a match proves the migration is invisible in the output rather
# than merely self-consistent. Regenerate it only for a deliberate edit to the
# listing's own format, to a `# help:` line, or when a subcommand is added or
# removed.
#
# The listing is read from the real .scripts/config rather than through
# run_config: run_config points HOME at a fixture whose .scripts/config does
# not exist, and the expectation records the real set of siblings.
expectation="$DOTFILES_ROOT/tests/fixtures/config-help-before-describe.txt"
if [ -f "$expectation" ]; then
    expected=$(cat "$expectation")
    # An empty expectation would make the comparison below pass against an
    # equally empty listing, so assert the recorded text exists first.
    assert_succeeds 'the recorded config help listing is not empty' \
        test -n "$expected"
    actual=$("$CONFIG_DIR/config-help" 2>/dev/null)
    # Names the file on failure. A mismatch here is usually a deliberate
    # subcommand addition rather than a defect, and a diff with no path to
    # regenerate sends the reader hunting for it.
    assert_equals "config help renders exactly the recorded listing (regenerate: .scripts/config/config-help > tests/fixtures/config-help-before-describe.txt)" \
        "$expected" "$actual"
else
    assert_equals 'the recorded config help listing exists' \
        'present' 'missing'
fi

# The `config-manifest --describe` cross-check that used to live here is gone
# with the binary it asked. The doctor shim's `# help:` line is now the only
# copy of that description, so there is no second copy to drift from and
# nothing left to compare. `config-cli` carries no `--describe`: it is one
# binary behind several verbs, so a single description could not name them all.
# The listing itself is still asserted above, against the recorded fixture.

# --- config deps resolves through the dispatcher -----------------------------
#
# Written before config-deps existed: at that point `config deps` fell
# through to `git deps`, which failed with git's own "not a git command"
# message and exit 1. That fallthrough is the hazard a missing shim creates,
# and this section is the regression guard against it coming back.

assert_contains 'config help lists deps, or the subcommand is invisible' \
    'deps' "$(HOME="$home" "$CONFIG" help 2>&1)"

bad_flag_output=$(HOME="$home" "$CONFIG" deps --not-a-real-flag 2>&1)
bad_flag_status=$?
assert_equals 'config deps rejects an unrecognized flag with exit 2, the repo-wide convention' \
    '2' "$bad_flag_status"
assert_equals 'the rejection does not fall through to git' '' \
    "$(printf '%s' "$bad_flag_output" | grep -F 'is not a git command' || true)"

finish
