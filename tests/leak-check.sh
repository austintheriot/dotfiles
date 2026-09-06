#!/bin/zsh
# Leak guard for the PUBLIC dotfiles repo (github.com/austintheriot/dotfiles).
#
# Called by tests/pre-commit (staged mode) and tests/pre-push (range mode).
#
# Two layers:
#
#   1. Generic credential rules, defined here. They match on shape (key
#      prefixes, secret-shaped assignments, bare UUIDs) and name nothing
#      specific, so they are safe to keep in a public file.
#
#   2. Project term rules, read from ~/.claude/local/leak-patterns.conf. Those
#      are the employer, product, repo, workflow, and ticket-prefix patterns.
#      They live outside this repo on purpose: writing the terms we defend
#      against into a public file would itself be the disclosure this guard
#      exists to prevent. See ~/DOTFILES-GL.md.
#
# The info/exclude list blocks paths known in advance. This guard catches what
# it cannot anticipate: internal content inside a file that legitimately belongs
# in this repo.
#
# Two modes:
#
#   (no argument)        scans staged content; run by tests/pre-commit
#   --range <a>..<b>     scans every commit in the range; run by tests/pre-push
#
# Range mode scans each commit's own diff, not the net diff of the range. A
# secret added in one commit and removed in the next has an empty net diff
# but is still in the pushed history, so it is still published.
#
# Override for a verified false positive:  SKIP_LEAK_CHECK=1 config commit ...
# Never use --no-verify; it skips every hook, the test suite included.

mode=commit
range=''
case "${1:-}" in
  '') ;;
  --range)
    if [ -z "${2:-}" ]; then
      echo "usage: leak-check.sh [--range <a>..<b>]" >&2
      exit 2
    fi
    mode=push
    range=$2
    ;;
  *)
    echo "usage: leak-check.sh [--range <a>..<b>]" >&2
    exit 2
    ;;
esac

if [ "$mode" = push ]; then
  hook=pre-push
  blocked='PUSH BLOCKED: this repo is PUBLIC and the pushed commits look internal.'
else
  hook=pre-commit
  blocked='COMMIT BLOCKED: this repo is PUBLIC and the staged content looks internal.'
fi

# Matched against true values rather than tested for non-emptiness: `[ -n ]`
# is true for the string "0", so SKIP_LEAK_CHECK=0 disabled the guard for
# anyone who meant the opposite. An unrecognized value is announced rather
# than silently declining, because the person who set it believes the guard is
# off and would not understand the block.
case "${SKIP_LEAK_CHECK:-}" in
  '') ;;
  1|true|TRUE|True|yes|YES|Yes)
    echo "$hook: leak check SKIPPED via SKIP_LEAK_CHECK" >&2
    exit 0
    ;;
  *)
    echo "$hook: SKIP_LEAK_CHECK=$SKIP_LEAK_CHECK is not a recognized true value, scanning anyway" >&2
    ;;
esac

if [ "$mode" = push ]; then
  if ! git rev-list --count "$range" >/dev/null 2>&1; then
    echo "leak-check: cannot resolve range $range" >&2
    exit 2
  fi
fi

PATTERN_FILE="${LEAK_PATTERN_FILE:-$HOME/.claude/local/leak-patterns.conf}"
ALLOW_FILE="${LEAK_ALLOW_FILE:-$HOME/.claude/local/leak-allow.conf}"

# Shared range-mode `git log` flags, so changed_paths and added_lines cannot
# drift apart on the filters they scan under.
#
# --diff-merges=first-parent shows each merge commit's own diff against its
# first parent, so content introduced only by the merge (a conflict
# resolution, an "evil merge") is scanned. This is a diff-format flag, not a
# traversal flag: unlike --first-parent, it does not skip side-branch commits,
# which the range still walks and scans on their own.
#
# Renamed content is re-scanned at its new path on purpose (no -M): a project
# term arriving at a new path is a fresh disclosure at that path, so the cost
# of a false block on a pure rename is preferred over a missed leak.
RANGE_LOG_FLAGS=(--diff-filter=ACMR --diff-merges=first-parent)

TMP_OUT=$(mktemp) || exit 2
TMP_ERR=$(mktemp) || exit 2
trap 'rm -f "$TMP_OUT" "$TMP_ERR"' EXIT
trap 'rm -f "$TMP_OUT" "$TMP_ERR"; exit 130' INT
trap 'rm -f "$TMP_OUT" "$TMP_ERR"; exit 143' TERM HUP

# The paths the scan covers, one per line, for the current mode.
changed_paths() {
  if [ "$mode" = push ]; then
    : > "$TMP_OUT"
    if ! git log --format= --name-only "${RANGE_LOG_FLAGS[@]}" "$range" \
        >> "$TMP_OUT" 2>"$TMP_ERR"; then
      echo "leak-check: git log failed scanning changed paths for $range" >&2
      cat "$TMP_ERR" >&2
      return 2
    fi
    grep -v '^$' "$TMP_OUT" | sort -u
  else
    git diff --cached --name-only --diff-filter=ACMR
  fi
}

# The diff text for the given paths (read from stdin, one per line).
#
# Returns the whole diff rather than only the added lines, because the caller
# needs both: the content rules read the + lines, and the hunk-coverage check
# needs the file headers. Returning one value that serves both keeps a single
# git invocation and removes the TMP_OUT aliasing the two callers had.
scan_diff() {
  if [ "$mode" = push ]; then
    : > "$TMP_OUT"
    if ! tr '\n' '\0' | xargs -0 git log --format= --no-color -U0 \
        "${RANGE_LOG_FLAGS[@]}" -p "$range" -- >> "$TMP_OUT" 2>"$TMP_ERR"; then
      echo "leak-check: git log failed scanning added lines for $range" >&2
      cat "$TMP_ERR" >&2
      return 2
    fi
    cat "$TMP_OUT"
  else
    tr '\n' '\0' | xargs -0 git diff --cached --no-color -U0 -- 2>/dev/null
  fi
}

# The scanned paths that produced no file header in the diff.
#
# Pure: both inputs are caller-supplied, so this makes no git call and is
# exercisable without a repository.
#
# Matched against the anchored `+++ b/<path>` header rather than by searching
# the diff for the path anywhere. A free-text search reports a path as scanned
# when a longer path containing it appears (styles.css satisfied by
# vendor/styles.css, verified), which is a hole in the shape this check exists
# to close.
#
# A path with no header was not scanned. Two evasions share that signature: a
# .gitattributes `-diff` marking on ordinary text, and a genuinely binary
# file. Both produce "Binary files ... differ" and no header.
unscannable_paths() {
  local path_list=$1 diff_text=$2
  local headers
  # The header set is extracted once, then compared as sets. Re-piping the
  # whole diff through a fresh grep per path is quadratic: measured at 3.7
  # seconds for 400 paths against a small diff, and pre-push runs this per
  # pushed range with two refs on a `config push-all`.
  #
  # Both header spellings are kept: `+++ b/path` is the default, and
  # `+++ path` is what --no-prefix output produces.
  headers=$(printf '%s\n' "$diff_text" \
    | sed -n -e 's|^+++ b/\(.*\)$|\1|p' -e 's|^+++ \([^b].*\)$|\1|p' \
    | grep -v '^/dev/null$' \
    | sort -u)
  printf '%s\n' "$path_list" | sort -u | comm -23 - <(printf '%s\n' "$headers")
}

raw_paths=$(changed_paths)
scan_status=$?
[ "$scan_status" -eq 0 ] || exit 2

# The self-exclusion is a separate step, not a pipe on the capture above:
# piping would make $? describe grep rather than changed_paths.
#
# This script's own source contains the generic patterns it searches for, so
# scanning it would always self-trip. It is the only path excluded in either
# mode, and a change to this file is therefore unguarded and depends on human
# review, so do not add another file here without the same tradeoff in mind.
scan_paths=$(printf '%s\n' "$raw_paths" | grep -v '^tests/leak-check\.sh$')
[ -z "$scan_paths" ] && exit 0

diff_text=$(printf '%s\n' "$scan_paths" | scan_diff)
scan_status=$?
[ "$scan_status" -eq 0 ] || exit 2

unscannable=$(unscannable_paths "$scan_paths" "$diff_text")
if [ -n "$unscannable" ]; then
  echo "" >&2
  echo "  $hook: BLOCKED, these paths produced no readable diff:" >&2
  printf '    %s\n' "${(@f)unscannable}" >&2
  echo "" >&2
  echo "  A path with no diff header was not scanned. Causes: a" >&2
  echo "  .gitattributes -diff marking, or a binary file." >&2
  echo "  Remove the marking, or move the content out of the repo." >&2
  echo "" >&2
  exit 2
fi

staged=$(printf '%s\n' "$diff_text" | grep '^+' | grep -v '^+++')
[ -z "$staged" ] && exit 0

fail=0
report() {
  # $1 = label, $2 = matching lines
  [ -z "$2" ] && return
  fail=1
  echo "" >&2
  echo "  [$1]" >&2
  echo "$2" | head -5 | sed 's/^/    /' >&2
  local n
  n=$(echo "$2" | wc -l | tr -d ' ')
  [ "$n" -gt 5 ] && echo "    ... and $((n - 5)) more" >&2
}

# --- Layer 1: generic credential rules -------------------------------------

# Known key prefixes and PEM headers.
report "possible credential" \
  "$(echo "$staged" | grep -ainE 'ghp_[A-Za-z0-9]{20}|gho_[A-Za-z0-9]{20}|github_pat_[A-Za-z0-9_]{20}|xox[baprs]-[A-Za-z0-9-]{10}|AKIA[0-9A-Z]{16}|sk-[A-Za-z0-9]{20}|-----BEGIN [A-Z ]*PRIVATE KEY|_authToken[[:space:]]*=[[:space:]]*[A-Za-z0-9-]{16}')"

# Secret-shaped assignments with a literal value. Placeholders and environment
# references are fine; a hardcoded value is not.
report "hardcoded secret assignment" \
  "$(echo "$staged" \
    | grep -ainE '(password|passwd|secret|api_?key|auth_?token|access_?token)[[:space:]]*[:=][[:space:]]*["'"'"']?[A-Za-z0-9!@#$%^&*_+-]{12,}' \
    | grep -viE '\$\{|\$[A-Z_]|process\.env|env\.|os\.environ|getenv|<your|example|placeholder|redact|xxxx|\bTOKEN\b[[:space:]]*[:=][[:space:]]*["'"'"']?$')"

# Bare UUID assigned to something. A common shape for registry and API tokens.
report "bare UUID (possible token)" \
  "$(echo "$staged" | grep -ainE '=[[:space:]]*[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}')"

# --- Layer 2: project term rules, loaded from outside this repo -------------

if [ ! -r "$PATTERN_FILE" ]; then
  if [ "${LEAK_ALLOW_NO_PATTERNS:-}" = 1 ]; then
    echo "" >&2
    echo "  $hook: term rules INACTIVE, permitted by LEAK_ALLOW_NO_PATTERNS" >&2
    echo "  Generic credential rules still ran." >&2
    echo "" >&2
  else
    # Layer 1 matches credential SHAPES. Layer 2 is the only thing defending
    # employer and project terms, which is why this repo needs a guard at all.
    # The pattern file is untracked on purpose, so "absent" is the default
    # state of a fresh machine rather than an exotic failure, and that is
    # precisely when setup work is being committed. Warning and continuing
    # published the content it exists to stop.
    #
    # Status 3, not 2: pre-push reports 2 as "could not scan this range",
    # which is a different problem with a different fix.
    echo "" >&2
    echo "  $hook: BLOCKED, no readable pattern file at $PATTERN_FILE" >&2
    echo "  The project term rules cannot run, so this scan is incomplete." >&2
    echo "  Restore the file (see ~/DOTFILES-GL.md), or set" >&2
    echo "  LEAK_ALLOW_NO_PATTERNS=1 for a machine with no terms to defend." >&2
    echo "" >&2
    exit 3
  fi
else
  # Some tracked files legitimately contain project terms (documented
  # conventions, local directory paths in shell and tmux config). Those were
  # reviewed and are benign, so the term rules skip them to avoid crying wolf
  # on unrelated future edits. The credential rules above still apply to them.
  if [ -r "$ALLOW_FILE" ]; then
    allow_re=$(grep -vE '^[[:space:]]*(#|$)' "$ALLOW_FILE" | paste -sd '|' -)
  else
    allow_re=''
  fi

  if [ -n "$allow_re" ]; then
    term_scope=$(echo "$scan_paths" | grep -vE "$allow_re")
  else
    term_scope="$scan_paths"
  fi

  if [ -n "$term_scope" ]; then
    # Filtered from $diff_text rather than a second git invocation: a header
    # line names its path right after "+++ b/", so restricting to hunks whose
    # header path is in term_scope reuses the diff already captured above.
    term_headers=$(printf '%s\n' "$term_scope" | sed 's|^|+++ b/|')
    # The header list is read as awk's FIRST input file (via process
    # substitution), not passed through -v: -v does not accept a value
    # containing newlines, and term_headers is a multi-line list. Passing it
    # through -v silently produced an empty `want` array (awk printed
    # "newline in string" and moved on), so the term scope matched nothing
    # for any diff with more than one file. NR==FNR is true only while awk is
    # still reading that first file, which is the idiomatic way to build a
    # lookup table before scanning the real input.
    term_staged=$(awk '
      NR == FNR { want[$0] = 1; next }
      /^\+\+\+ / { in_scope = ($0 in want); next }
      in_scope && /^\+/ && !/^\+\+\+/ { print }
    ' <(printf '%s\n' "$term_headers") <(printf '%s\n' "$diff_text"))

    while IFS= read -r pattern; do
      case "$pattern" in
        ''|\#*) continue ;;
      esac
      report "project term" "$(echo "$term_staged" | grep -ainE "$pattern")"
    done < "$PATTERN_FILE"
  fi
fi

if [ "$fail" -ne 0 ]; then
  echo "" >&2
  echo "$blocked" >&2
  echo "" >&2
  echo "  Fix by one of:" >&2
  echo "    - Genericize the wording, then re-stage." >&2
  echo "    - Move the specifics to ~/.claude/local/ and read them at runtime." >&2
  echo "    - Track the file in the other repo instead (see ~/DOTFILES-GL.md)." >&2
  echo "" >&2
  echo "  If this is a verified false positive:" >&2
  if [ "$mode" = push ]; then
    echo "    SKIP_LEAK_CHECK=1 config push ..." >&2
  else
    echo "    SKIP_LEAK_CHECK=1 config commit ..." >&2
  fi
  echo "" >&2
  exit 1
fi

exit 0
