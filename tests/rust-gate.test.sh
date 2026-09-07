#!/usr/bin/env bash
#
# Asserts the Rust gate is wired and that its skip path is loud.
#
# The gate exists because a push once printed "SKIP cargo test (cargo not
# found)" and passed: the container leg is Rust-free by design, so the
# container can never run these checks. A test that only asserted the
# checks pass would not catch the gate being removed from the hook, which
# is the failure that actually happened.

set -uo pipefail
# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"

assert_equals "tests/rust-checks.sh must exist and be executable" "0" \
    "$([ -x "$DOTFILES_ROOT/tests/rust-checks.sh" ] && echo 0 || echo 1)"

# The hook is the only thing that makes these checks a gate rather than a
# script nobody runs. Asserted against the hook text because driving a real
# push from a test would push.
assert_contains "tests/pre-push must call rust-checks.sh, or the gate is not a gate" \
    "rust-checks.sh" "$(cat "$DOTFILES_ROOT/tests/pre-push")"

# Ordering matters: the Rust checks are seconds and the Docker suite is
# minutes, so a Rust failure must not wait behind a container build.
#
# Matched against the invocation shape, "$HOME/tests/<script>.sh", not the
# bare filename. A bare-filename match also hits prose comments that
# mention either script, so a comment could flip this assertion without
# touching how the hook actually runs. That happened once: a header comment
# that happened to mention "rust-checks.sh" before "run-in-docker.sh" made
# this assertion pass regardless of the real call order, and rewording the
# comment would have made it fail regardless of the real call order too.
# The quoted "$HOME/tests/..." form appears only at the actual call sites.
hook_text=$(cat "$DOTFILES_ROOT/tests/pre-push")
rust_line=$(printf '%s\n' "$hook_text" \
    | grep -n '\$HOME/tests/rust-checks\.sh' | head -1 | cut -d: -f1)
docker_line=$(printf '%s\n' "$hook_text" \
    | grep -n '\$HOME/tests/run-in-docker\.sh' | head -1 | cut -d: -f1)
assert_equals "rust-checks.sh must be invoked before run-in-docker.sh" "0" \
    "$([ -n "$rust_line" ] && [ -n "$docker_line" ] && [ "$rust_line" -lt "$docker_line" ] && echo 0 || echo 1)"

# PATH emptied so `command -v cargo` fails. The script must say so and
# still exit 0: a machine without Rust can still push shell changes.
skip_output=$(env PATH=/nonexistent "$DOTFILES_ROOT/tests/rust-checks.sh" HEAD 2>&1)
skip_status=$?
assert_equals "an absent cargo must not block the push" "0" "$skip_status"
assert_contains "an absent cargo must print SKIP, not pass silently" "SKIP" "$skip_output"

# Counted, not a bare command: an uncounted check does not appear in the
# suite total, so its absence is invisible in the summary line.
runner_text=$(cat "$DOTFILES_ROOT/tests/run-all.sh")
assert_contains "run-all.sh must run clippy, or the invariant has no gate in CI" \
    "cargo clippy" "$runner_text"
assert_contains "the clippy leg must go through run_suite so it is counted and timed" \
    'run_suite "cargo clippy in crates/"' "$runner_text"

# Parsed rather than grepped, matching deps-harness.test.sh: a reformat of
# the workflow must not produce a false pass or a false failure. $PYTHON_BIN
# rather than a bare python3, for the reason deps-harness.test.sh gives:
# lib.sh resolves the real interpreter once, and the shim on PATH costs
# 750ms per start.
ci_has_clippy=$("$PYTHON_BIN" - "$DOTFILES_ROOT/.github/workflows/test-suite.yml" <<'PY'
import sys, yaml
with open(sys.argv[1]) as handle:
    workflow = yaml.safe_load(handle)
steps = [
    step
    for job in workflow["jobs"].values()
    for step in job.get("steps", [])
]
found = any("cargo clippy" in str(step.get("run", "")) for step in steps)
print("0" if found else "1")
PY
)
assert_equals "test-suite.yml must run cargo clippy, so the invariant gates before merge" \
    "0" "$ci_has_clippy"

finish
