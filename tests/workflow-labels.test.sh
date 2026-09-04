#!/bin/bash
#
# Tests that every GitHub Actions workflow and job carries a readable label.
#
# GitHub falls back to the filename when a workflow has no `name:`, and to the
# job key when a job has no `name:`. The Actions sidebar then reads
# "branch-drift.yml" with a single job called "check", which says nothing
# about what ran or why it matters.
#
# Job keys are deliberately not asserted on: tests/deps-harness.test.sh reads
# jobs by key (`jobs['macos']`), so the keys are an interface. This file
# checks the display names layered on top of them.
#
# Usage: ~/tests/workflow-labels.test.sh

. "$(dirname "$0")/lib.sh"

WORKFLOW_DIR="$DOTFILES_ROOT/.github/workflows"

assert_succeeds 'the workflow directory exists' test -d "$WORKFLOW_DIR"

python_bin=$(command -v python3 || command -v python) || {
    printf '      skipped: no python interpreter\n'
    finish
    exit 0
}

facts="$FIXTURES/labels.txt"
"$python_bin" - "$WORKFLOW_DIR" "$facts" <<'PYEOF'
import sys, os, re

directory, out = sys.argv[1], sys.argv[2]
unnamed_workflows = []
unnamed_jobs = []
terse_workflows = []
workflows = []

for filename in sorted(os.listdir(directory)):
    if not filename.endswith(('.yml', '.yaml')):
        continue
    path = os.path.join(directory, filename)
    text = open(path).read()
    workflows.append(filename)

    match = re.search(r'(?m)^name:[ \t]*(.+?)[ \t]*$', text)
    if not match:
        unnamed_workflows.append(filename)
    else:
        title = match.group(1).strip().strip('"\'')
        # A name that is just the filename stem, or has no spaces, is the
        # opaque case: it gives the reader nothing the filename did not.
        stem = filename.rsplit('.', 1)[0]
        if title == stem or ' ' not in title:
            terse_workflows.append('%s:%s' % (filename, title))

    # Job blocks are the two-space keys under a top-level `jobs:`.
    jobs_block = re.search(r'(?ms)^jobs:[ \t]*$\n(.*)', text)
    if jobs_block:
        body = jobs_block.group(1)
        for job_match in re.finditer(r'(?m)^  ([A-Za-z0-9_-]+):[ \t]*$', body):
            key = job_match.group(1)
            start = job_match.end()
            nxt = re.search(r'(?m)^  [A-Za-z0-9_-]+:[ \t]*$', body[start:])
            job_body = body[start:start + nxt.start()] if nxt else body[start:]
            if not re.search(r'(?m)^    name:[ \t]*\S', job_body):
                unnamed_jobs.append('%s/%s' % (filename, key))

with open(out, 'w') as handle:
    handle.write('workflows=%s\n' % ' '.join(workflows))
    handle.write('unnamed_workflows=%s\n' % ' '.join(unnamed_workflows))
    handle.write('terse_workflows=%s\n' % ' '.join(terse_workflows))
    handle.write('unnamed_jobs=%s\n' % ' '.join(unnamed_jobs))
PYEOF

value_of() { sed -n "s/^$1=//p" "$facts"; }

assert_succeeds 'found at least one workflow' test -n "$(value_of workflows)"

assert_equals 'every workflow has a name' '' "$(value_of unnamed_workflows)"

# A name equal to the filename stem, or a single word, is what produced the
# opaque "branch-drift.yml" heading in the Actions sidebar.
assert_equals 'no workflow name merely restates its filename' \
    '' "$(value_of terse_workflows)"

assert_equals 'every job has a name' '' "$(value_of unnamed_jobs)"

# --- the test suite runs in CI, on both platforms ------------------------
#
# Before this existed, no workflow ran tests/run-all.sh at all: the suite only
# ever ran from the local pre-push hook, so a green push meant "Docker was
# running on the author's laptop", not "CI verified this".
#
# Both platforms are required. Linux alone would skip notify.test.sh (it
# drives .claude/hooks/notify.sh) and the Darwin-gated Alacritty app-bundle
# check in check-deps.test.sh. macOS alone would skip the container suites,
# since GitHub's macOS runners ship no Docker daemon.

SUITE_WORKFLOW="$WORKFLOW_DIR/test-suite.yml"
assert_succeeds 'a workflow runs the test suite' test -f "$SUITE_WORKFLOW"
suite_text=$(cat "$SUITE_WORKFLOW" 2>/dev/null)

assert_contains 'the suite workflow runs on push' 'push:' "$suite_text"
assert_contains 'the suite workflow runs run-all.sh' 'run-all.sh' "$suite_text"

# The runners are read out of the parsed matrix, not grepped from the file.
# Grepping matches the human-readable `label:` strings too, so deleting the
# macOS entry from the matrix left a whole-file grep green.
runners=$("$python_bin" - "$SUITE_WORKFLOW" <<'RUNNEREOF'
import sys, re
text = open(sys.argv[1]).read()
# Deliberately regex rather than yaml: pyyaml is not installed everywhere the
# suite runs, and this file must not skip on the machine it is guarding.
found = sorted(set(re.findall(r'(?m)^\s*runner:\s*(\S+)\s*$', text)))
print(' '.join(found))
RUNNEREOF
)

assert_contains 'the matrix includes a linux runner' 'ubuntu-latest' "$runners"
assert_contains 'the matrix includes a macos runner' 'macos-latest' "$runners"

# A matrix job that stops at the first failing platform hides whether the
# other one is also broken, which is the whole point of running both.
assert_contains 'the matrix does not fail fast' 'fail-fast: false' "$suite_text"

# --- branch-drift.yml labels the combined failure case -------------------
#
# config-manifest check has two failure modes that can both fire in one run:
# diverged shared paths (stdout, "diverged: <path>") and unlabeled files
# (stderr, "... match no .sync-manifest rule"). Before this section existed,
# the workflow's step summary checked for the unlabeled-files marker first
# and picked that heading whenever it matched, even when drift was also
# present in the same output. A push with both problems got told only about
# the label it happened to grep first.
#
# Asserted structurally, not by invoking the workflow (GitHub Actions logic
# lives in shell embedded in YAML, so there is no runner to invoke here):
# the combined case needs its own branch that requires both markers, and it
# needs to come before the single-mode branches so a combined failure cannot
# fall through to one of them.

DRIFT_WORKFLOW="$WORKFLOW_DIR/branch-drift.yml"
assert_succeeds 'the branch-drift workflow exists' test -f "$DRIFT_WORKFLOW"
drift_text=$(cat "$DRIFT_WORKFLOW" 2>/dev/null)

assert_contains 'the drift workflow checks for diverged paths' \
    'diverged:' "$drift_text"
assert_contains 'the drift workflow checks for unlabeled files' \
    'match no .sync-manifest rule' "$drift_text"

combined_branch=$("$python_bin" - "$DRIFT_WORKFLOW" <<'COMBINEDEOF'
import sys, re
text = open(sys.argv[1]).read()

# The step summary is a shell if/elif/else chain inside the YAML block
# scalar, testing shell variables set earlier from grepping the output for
# each marker (e.g. has_drift, has_unlabeled). Find each `grep -q PATTERN &&
# var=1` line to learn which marker each variable stands for, then find an
# if/elif condition line that tests two variables standing for different
# markers -- that is the combined case, however its heading is worded.
grep_sets = re.findall(r"grep -q '([^']*)'\s*&&\s*(\w+)=1", text)
marker_of = {}
for pattern, var in grep_sets:
    if 'diverged' in pattern:
        marker_of[var] = 'diverged'
    elif 'sync-manifest' in pattern or 'unlabeled' in pattern.lower():
        marker_of[var] = 'unlabeled'

lines = text.splitlines()
for index, line in enumerate(lines):
    stripped = line.strip()
    if not (stripped.startswith('if ') or stripped.startswith('elif ')):
        continue
    vars_in_cond = [v for v in marker_of if re.search(r'\b' + re.escape(v) + r'\b', line)]
    markers_in_cond = {marker_of[v] for v in vars_in_cond}
    if {'diverged', 'unlabeled'} <= markers_in_cond:
        # Grab this condition line plus its body: every following line
        # indented deeper than the condition itself, up to the next line at
        # the same or shallower indent.
        indent = len(line) - len(line.lstrip())
        block_lines = [line]
        for later in lines[index + 1:]:
            if later.strip() and (len(later) - len(later.lstrip())) <= indent:
                break
            block_lines.append(later)
        print('\n'.join(block_lines))
        break
COMBINEDEOF
)

assert_succeeds 'a step-summary branch tests for both failure markers together' \
    test -n "$combined_branch"
assert_contains 'the combined branch has its own heading' \
    '###' "$combined_branch"

# The combined heading must differ from both single-mode headings, or the
# reader can't tell "both problems" apart from "just one of them" by looking
# at the summary. Compared as an exact heading string, not a substring: a
# substring check would let a longer combined heading like "### Branch
# drift and unlabeled files" pass a "starts with the same words" comparison
# against the plain "### Branch drift" heading.
combined_heading=$(printf '%s\n' "$combined_branch" \
    | sed -n "s/^[[:space:]]*echo '\(###[^']*\)'.*/\1/p" | head -1)

assert_succeeds 'the combined branch has a parseable heading' \
    test -n "$combined_heading"
assert_succeeds 'the combined heading differs from "Unlabeled files"' \
    sh -c '[ "$1" != "### Unlabeled files" ]' _ "$combined_heading"
assert_succeeds 'the combined heading differs from "Branch drift"' \
    sh -c '[ "$1" != "### Branch drift" ]' _ "$combined_heading"

finish
