#!/bin/bash
#
# Tests for depcheck-hook.sh: the 24h-throttled shell-startup nag.
#
# The hook resolves check-deps.sh through ~, so every test runs it under an
# isolated $HOME holding a stub check-deps.sh that logs each invocation. That
# is what makes "the throttle skipped the check" observable at all -- wall
# clock cannot distinguish it, because this machine's real shell startup is
# dominated by nvm.
#
# The hook is sourced by an interactive zsh in real use, so it is sourced by
# zsh here too: two of these cases exist because zsh's arithmetic reacts to a
# malformed cache differently than sh's would.
#
# Usage: ~/tests/depcheck-hook.test.sh

. "$(dirname "$0")/lib.sh"

HOOK="$DOTFILES_ROOT/.my-scripts/deps/depcheck-hook.sh"

# A fresh isolated HOME with a stub check-deps.sh whose exit status is fixed
# by the caller. Prints the HOME path.
make_home() {
    local exit_status=$1 home
    home=$(mktemp -d "$FIXTURES/home-XXXXXX")
    mkdir -p "$home/.my-scripts/deps" "$home/.cache"
    cat > "$home/.my-scripts/deps/check-deps.sh" <<EOF
#!/bin/sh
printf 'invoked\n' >> "$home/invocations.log"
exit $exit_status
EOF
    chmod +x "$home/.my-scripts/deps/check-deps.sh"
    cp "$HOOK" "$home/.my-scripts/deps/depcheck-hook.sh"
    : > "$home/invocations.log"
    printf '%s' "$home"
}

# Sources the hook in a non-interactive zsh under the given HOME. Stdout only.
run_hook() {
    local home=$1
    env HOME="$home" zsh -c ". '$home/.my-scripts/deps/depcheck-hook.sh'" 2>/dev/null
}

# Sources the hook and returns only what it wrote to stderr, which must always
# be empty: this runs during shell startup.
#
# Truncated, because a failure here reports the offending cache value back and
# one of these fixtures is deliberately 200KB long.
run_hook_stderr() {
    local home=$1
    env HOME="$home" zsh -c ". '$home/.my-scripts/deps/depcheck-hook.sh'" 2>&1 >/dev/null \
        | cut -c1-120
}

invocations() {
    local home=$1
    grep -c invoked "$home/invocations.log" 2>/dev/null || printf '0'
}

# --- a cold cache runs the check and nags ---------------------------------

home=$(make_home 1)
output=$(run_hook "$home")
assert_contains 'a cold cache nags' \
    'depcheck: missing dependencies detected' "$output"
assert_equals 'a cold cache runs the check' '1' "$(invocations "$home")"
assert_succeeds 'a cold cache writes the timestamp' \
    test -s "$home/.cache/depcheck-last-run"

# --- a fresh cache skips the check entirely -------------------------------

output=$(run_hook "$home")
assert_equals 'a fresh cache does not nag' '' "$output"
assert_equals 'a fresh cache does not re-run the check' '1' "$(invocations "$home")"

# --- a cache older than 24h runs the check again --------------------------

printf '%s\n' "$(( $(date +%s) - 90000 ))" > "$home/.cache/depcheck-last-run"
output=$(run_hook "$home")
assert_contains 'a stale cache nags again' \
    'depcheck: missing dependencies detected' "$output"
assert_equals 'a stale cache re-runs the check' '2' "$(invocations "$home")"

# --- nothing is missing, so there is no nag -------------------------------

home=$(make_home 0)
output=$(run_hook "$home")
assert_equals 'a passing check does not nag' '' "$output"
assert_equals 'a passing check still ran' '1' "$(invocations "$home")"

# --- the check is never asked to install ----------------------------------
#
# The nag path must stay non-interactive. check-deps.sh only prompts behind
# --fix, so the hook passing any argument at all would be the defect.

home=$(make_home 1)
cat > "$home/.my-scripts/deps/check-deps.sh" <<EOF
#!/bin/sh
printf '%s\n' "\$*" >> "$home/args.log"
exit 1
EOF
chmod +x "$home/.my-scripts/deps/check-deps.sh"
run_hook "$home" >/dev/null
assert_equals 'the startup check passes no flags' '' "$(cat "$home/args.log")"

# --- a malformed cache never leaks an error into startup ------------------
#
# An all-digit value longer than zsh's arithmetic width passes a digits-only
# guard and then makes $((...)) print "number truncated after 19 digits".
# Every one of these must be treated as stale and produce clean stderr.

for bad in '' 'notanumber' '   ' '-500' '1e10' '99999999999999999999999999' '12 34'; do
    home=$(make_home 0)
    printf '%s' "$bad" > "$home/.cache/depcheck-last-run"
    assert_equals "a cache of [$bad] keeps startup silent" '' "$(run_hook_stderr "$home")"
done

home=$(make_home 0)
head -c 200000 /dev/zero | tr '\0' '9' > "$home/.cache/depcheck-last-run"
assert_equals 'an oversized cache keeps startup silent' '' "$(run_hook_stderr "$home")"

# --- an unwritable cache directory never leaks an error -------------------

home=$(make_home 0)
rm -rf "$home/.cache"
: > "$home/.cache"
assert_equals 'an unwritable cache path keeps startup silent' \
    '' "$(run_hook_stderr "$home")"
rm -f "$home/.cache"

# --- the manual alias is defined ------------------------------------------

home=$(make_home 0)
output=$(env HOME="$home" zsh -ic \
    ". '$home/.my-scripts/deps/depcheck-hook.sh'; alias depcheck" 2>/dev/null)
assert_contains 'defines the depcheck alias' 'check-deps.sh --fix' "$output"

# --- the hook is portable to a POSIX shell --------------------------------
#
# .my-scripts/ is shared between the mac and linux branches, so the hook must
# not depend on a zsh-only construct.

assert_succeeds 'parses as POSIX sh' sh -n "$HOOK"
assert_succeeds 'parses as zsh' zsh -n "$HOOK"

finish
