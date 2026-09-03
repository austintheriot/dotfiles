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

finish
