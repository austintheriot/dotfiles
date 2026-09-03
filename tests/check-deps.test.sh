#!/bin/bash
#
# Tests for check-deps.sh: dependency presence checking and the --fix
# install flow. Package managers and installable tools are stubbed with fake
# executables on a fixture PATH rather than touching the real system --
# this script's whole job is deciding *which* command to run and whether the
# result satisfies the check, and running a real apt-get/brew/pacman here
# would either fail (no root, no such tool) or mutate the machine running
# the tests.
#
# Usage: ~/tests/check-deps.test.sh

. "$(dirname "$0")/lib.sh"

SCRIPT="$DOTFILES_ROOT/.my-scripts/deps/check-deps.sh"
BIN="$FIXTURES/bin"
mkdir -p "$BIN"

cat > "$BIN/sudo" <<'EOF'
#!/bin/sh
exec "$@"
EOF
chmod +x "$BIN/sudo"

cat > "$BIN/apt-get" <<EOF
#!/bin/sh
printf 'apt-get %s\n' "\$*" >> "$FIXTURES/apt.log"
exit 0
EOF
chmod +x "$BIN/apt-get"

cat > "$BIN/some-tool" <<'EOF'
#!/bin/sh
exit 0
EOF
chmod +x "$BIN/some-tool"

run_check() {
    local conf=$1; shift
    PATH="$BIN:$PATH" DEPS_CONF="$conf" DEPS_LOCAL_CONF="$FIXTURES/no-such-local.conf" \
        "$SCRIPT" "$@"
}

# --- everything present ---------------------------------------------------

conf="$FIXTURES/deps-present.conf"
printf 'some-tool|command -v some-tool|https://example.invalid\n' > "$conf"

output=$(run_check "$conf")
status=$?
assert_equals 'all present exits 0' '0' "$status"
assert_contains 'reports the count' 'all 1 dependencies present' "$output"

# --- a missing dependency, no --fix ---------------------------------------

conf="$FIXTURES/deps-missing.conf"
printf 'nonexistent-tool|command -v nonexistent-tool|https://example.invalid\n' > "$conf"

output=$(run_check "$conf")
status=$?
assert_equals 'a missing dep exits 1 without --fix' '1' "$status"
assert_contains 'names the missing dep' 'missing: nonexistent-tool' "$output"

# --- --fix --yes runs the default package-manager install, verifies it ---

: > "$FIXTURES/apt.log"
marker="$FIXTURES/installed-marker"
rm -f "$marker"
cat > "$BIN/apt-get" <<EOF
#!/bin/sh
printf 'apt-get %s\n' "\$*" >> "$FIXTURES/apt.log"
touch "$marker"
exit 0
EOF

conf="$FIXTURES/deps-fix-success.conf"
printf 'marker-tool|[ -f "%s" ]|https://example.invalid\n' "$marker" > "$conf"

output=$(run_check "$conf" --fix --yes)
status=$?
apt_log=$(cat "$FIXTURES/apt.log")
assert_contains 'runs apt-get install for the default case' 'install -y marker-tool' "$apt_log"
assert_contains 'confirms the install' 'installed marker-tool' "$output"
assert_equals 'a successful --fix exits 0' '0' "$status"

# --- --fix --yes reports (but does not fail on) an install that does not
#     satisfy its own check -----------------------------------------------

: > "$FIXTURES/apt.log"
cat > "$BIN/apt-get" <<EOF
#!/bin/sh
printf 'apt-get %s\n' "\$*" >> "$FIXTURES/apt.log"
exit 0
EOF

conf="$FIXTURES/deps-fix-fail.conf"
printf 'some-tool|command -v some-tool-not-really-installed|https://example.invalid\n' > "$conf"

output=$(run_check "$conf" --fix --yes)
status=$?
assert_contains 'reports the unsatisfied check' 'install did not satisfy the check' "$output"
assert_equals 'an unsatisfied install exits 1' '1' "$status"

# --- --fix --dry-run never executes anything, always exits 0 -------------

: > "$FIXTURES/apt.log"
conf="$FIXTURES/deps-dryrun.conf"
printf 'some-tool|command -v some-tool-not-really-installed|https://example.invalid\n' > "$conf"

output=$(run_check "$conf" --fix --dry-run)
status=$?
apt_log=$(cat "$FIXTURES/apt.log")
assert_equals 'dry-run never invokes apt-get' '' "$apt_log"
assert_contains 'prints what it would run' \
    'would run: sudo apt-get update -qq && sudo apt-get install -y some-tool' "$output"
assert_equals 'dry-run always exits 0' '0' "$status"

# --- a manual-only dependency is reported but never fails --fix ----------

conf="$FIXTURES/deps-manual.conf"
printf 'nvm|[ -f "%s/nonexistent-nvm.sh" ]|https://github.com/nvm-sh/nvm\n' "$FIXTURES" > "$conf"

output=$(run_check "$conf" --fix --yes)
status=$?
assert_contains 'names it as manual-only' 'no automated install' "$output"
assert_contains 'links the docs' 'https://github.com/nvm-sh/nvm' "$output"
assert_equals 'a manual-only dependency does not fail --fix' '0' "$status"

finish
