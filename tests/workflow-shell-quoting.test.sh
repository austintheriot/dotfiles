#!/bin/bash
#
# Guards the single-quoted `sh -c '...'` bodies in .github/workflows.
#
# THE BUG THIS EXISTS FOR, 2026-09-08. A job passes a whole script to a
# container as one single-quoted argument:
#
#     depcheck-pop -c '
#       set -eu
#       ...
#     '
#
# Any single quote inside that body closes the argument early. A runtime
# assertion added to the Pop!_OS leg wrapped its Lua in single quotes and
# included the word "repo's" in a comment, and the step died with
#
#     ##[error]Process completed with exit code 127.
#
# with no output whatsoever -- not a failed assertion, an unparseable
# command. 127 is "command not found", so the job reported a missing binary
# for what was really a quoting error, and the assertion the step existed to
# run never executed.
#
# WHY A GUARD RATHER THAN CARE. The failure is invisible to every cheap
# check: the YAML parses, `actionlint` does not read into the string, and the
# body only breaks when the shell splits it. It cost a full red CI round to
# find, and the natural fix (an apostrophe in an English comment) reintroduces
# it silently.
#
# WHAT IS ASSERTED. Each single-quoted `-c '...'` body must contain no single
# quote at all, and must parse as a shell script under `sh -n`. The second
# half is what catches an unbalanced construct that happens to avoid
# apostrophes.
#
# Usage: ~/tests/workflow-shell-quoting.test.sh

. "$(dirname "$0")/lib.sh"

WORKFLOW_DIR="$DOTFILES_ROOT/.github/workflows"

assert_succeeds 'the workflow directory exists' test -d "$WORKFLOW_DIR"

# Extracted with python rather than sed: the bodies are YAML block scalars,
# so the indentation that terminates one is structural and a line-oriented
# parser would have to reimplement it.
extract_bodies() {
    "$PYTHON_BIN" - "$1" <<'PYEOF'
import sys, yaml, pathlib

# Every `-c '<body>'` argument in a step's `run:` text, keyed by job so a
# failure can name where to look.
#
# THE BODY ENDS AT A LINE WHOSE ONLY CONTENT IS A QUOTE, not at the next
# quote character. A first version split on the first `'` it found, which
# made this guard blind to the very bug it exists for: an apostrophe in a
# mid-body comment terminated the extracted body there, so the quote was
# never inside the text being checked and the sabotage passed. The shell
# does not stop at the first quote of a line, so neither may the extractor.
path = pathlib.Path(sys.argv[1])
document = yaml.safe_load(path.read_text())
for job_name, job in (document.get("jobs") or {}).items():
    for index, step in enumerate(job.get("steps") or []):
        run = step.get("run")
        if not run or "-c '" not in run:
            continue
        collecting = False
        body = []
        for line in run.splitlines():
            if not collecting:
                if line.rstrip().endswith("-c '"):
                    collecting = True
                    body = []
                continue
            if line.strip() == "'":
                if any(entry.strip() for entry in body):
                    print("=== {}:{}:step{}".format(path.name, job_name, index))
                    print("\n".join(body))
                collecting = False
                continue
            body.append(line)
        # An unterminated body is itself a defect: the quote-only line the
        # shell needs to close the argument is missing.
        if collecting and any(entry.strip() for entry in body):
            print("=== {}:{}:step{}:UNTERMINATED".format(path.name, job_name, index))
            print("\n".join(body))
PYEOF
}

bodies_found=0
violations=''

for workflow in "$WORKFLOW_DIR"/*.yml; do
    [ -f "$workflow" ] || continue
    extracted=$(extract_bodies "$workflow") || {
        violations="$violations
$(basename "$workflow"): could not be parsed"
        continue
    }
    [ -n "$extracted" ] || continue

    # Split on the header lines the extractor emits, so each body is checked
    # on its own and a failure names the job.
    current=''
    header=''
    while IFS= read -r line; do
        case $line in
            '=== '*)
                if [ -n "$header" ]; then
                    bodies_found=$((bodies_found + 1))
                    printf '%s\n' "$current" | grep -q "'" \
                        && violations="$violations
$header: contains a single quote, which closes the sh -c argument early"
                    printf '%s\n' "$current" | sh -n 2>/dev/null \
                        || violations="$violations
$header: does not parse as a shell script"
                fi
                header=${line#=== }
                current=''
                ;;
            *) current="$current$line
" ;;
        esac
    done <<EOF
$extracted
EOF
    if [ -n "$header" ]; then
        bodies_found=$((bodies_found + 1))
        printf '%s\n' "$current" | grep -q "'" \
            && violations="$violations
$header: contains a single quote, which closes the sh -c argument early"
        printf '%s\n' "$current" | sh -n 2>/dev/null \
            || violations="$violations
$header: does not parse as a shell script"
    fi
done

# The count is folded into the compared value rather than asserted
# separately. An extractor that silently found nothing would otherwise report
# zero violations and pass while checking nothing, which is the exact failure
# shape this repo keeps hitting: a guard whose derivation broke compares
# empty to empty and reports success.
assert_equals 'every single-quoted sh -c body is quote-free and parses' \
    'bodies>0 violations:' \
    "$([ "$bodies_found" -gt 0 ] && printf 'bodies>0' || printf 'bodies=0') violations:$violations"
