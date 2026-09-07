#!/bin/bash
#
# Tests that every GitHub Actions workflow and job carries a readable label.
#
# GitHub falls back to the filename when a workflow has no `name:`, and to the
# job key when a job has no `name:`. A workflow with neither reads as its
# bare filename with a single job named after its key in the Actions
# sidebar, which says nothing about what ran or why it matters.
#
# Job keys are deliberately not asserted on: tests/deps-harness.test.sh reads
# jobs by key (`jobs['macos']`), so the keys are an interface. This file
# checks the display names layered on top of them.
#
# Usage: ~/tests/workflow-labels.test.sh

. "$(dirname "$0")/lib.sh"

WORKFLOW_DIR="$DOTFILES_ROOT/.github/workflows"

assert_succeeds 'the workflow directory exists' test -d "$WORKFLOW_DIR"

# $PYTHON_BIN comes from lib.sh, which resolves the real interpreter once,
# past the pyenv shim: 750ms per start against 40ms. Three starts happen below.
if [ -z "${PYTHON_BIN:-}" ]; then
    skip 'no python interpreter: found at least one workflow'
    skip 'no python interpreter: every workflow has a name'
    skip 'no python interpreter: no workflow name merely restates its filename'
    skip 'no python interpreter: every job has a name'
    skip 'no python interpreter: a workflow runs the test suite'
    skip 'no python interpreter: the suite workflow runs on push'
    skip 'no python interpreter: the suite workflow runs run-all.sh'
    skip 'no python interpreter: the matrix includes a linux runner'
    skip 'no python interpreter: the matrix includes a macos runner'
    skip 'no python interpreter: the matrix does not fail fast'
    finish
    exit 0
fi

facts="$FIXTURES/labels.txt"
"$PYTHON_BIN" - "$WORKFLOW_DIR" "$facts" <<'PYEOF'
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
# check in deps-manifest.test.sh. macOS alone would skip the container suites,
# since GitHub's macOS runners ship no Docker daemon.

SUITE_WORKFLOW="$WORKFLOW_DIR/test-suite.yml"
assert_succeeds 'a workflow runs the test suite' test -f "$SUITE_WORKFLOW"
suite_text=$(cat "$SUITE_WORKFLOW" 2>/dev/null)

assert_contains 'the suite workflow runs on push' 'push:' "$suite_text"
assert_contains 'the suite workflow runs run-all.sh' 'run-all.sh' "$suite_text"

# The runners are read out of the parsed matrix, not grepped from the file.
# Grepping matches the human-readable `label:` strings too, so deleting the
# macOS entry from the matrix left a whole-file grep green.
runners=$("$PYTHON_BIN" - "$SUITE_WORKFLOW" <<'RUNNEREOF'
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

finish
