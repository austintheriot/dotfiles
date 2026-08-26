#!/bin/zsh
# Leak guard for the PUBLIC dotfiles repo (github.com/austintheriot/dotfiles).
#
# Called by tests/pre-commit. Not installed as a hook directly, so that the repo
# keeps a single hook entry point.
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
# Scans staged content only, so it judges what you are about to publish.
#
# Override for a verified false positive:  SKIP_LEAK_CHECK=1 config commit ...
# Never use --no-verify; it skips every hook, the test suite included.

if [ -n "$SKIP_LEAK_CHECK" ]; then
  echo "pre-commit: leak check SKIPPED via SKIP_LEAK_CHECK" >&2
  exit 0
fi

PATTERN_FILE="${LEAK_PATTERN_FILE:-$HOME/.claude/local/leak-patterns.conf}"
ALLOW_FILE="${LEAK_ALLOW_FILE:-$HOME/.claude/local/leak-allow.conf}"

# This script's own source contains the generic patterns it searches for, so
# scanning it would always self-trip. Exclude it; it is reviewed by hand.
scan_paths=$(git diff --cached --name-only --diff-filter=ACMR | grep -v '^tests/leak-check\.sh$')
[ -z "$scan_paths" ] && exit 0

staged=$(echo "$scan_paths" | tr '\n' '\0' \
  | xargs -0 git diff --cached --no-color -U0 -- 2>/dev/null | grep '^+' | grep -v '^+++')
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
  "$(echo "$staged" | grep -inE 'ghp_[A-Za-z0-9]{20}|gho_[A-Za-z0-9]{20}|github_pat_[A-Za-z0-9_]{20}|xox[baprs]-[A-Za-z0-9-]{10}|AKIA[0-9A-Z]{16}|sk-[A-Za-z0-9]{20}|-----BEGIN [A-Z ]*PRIVATE KEY|_authToken[[:space:]]*=[[:space:]]*[A-Za-z0-9-]{16}')"

# Secret-shaped assignments with a literal value. Placeholders and environment
# references are fine; a hardcoded value is not.
report "hardcoded secret assignment" \
  "$(echo "$staged" \
    | grep -inE '(password|passwd|secret|api_?key|auth_?token|access_?token)[[:space:]]*[:=][[:space:]]*["'"'"']?[A-Za-z0-9!@#$%^&*_+-]{12,}' \
    | grep -viE '\$\{|\$[A-Z_]|process\.env|env\.|os\.environ|getenv|<your|example|placeholder|redact|xxxx|\bTOKEN\b[[:space:]]*[:=][[:space:]]*["'"'"']?$')"

# Bare UUID assigned to something. A common shape for registry and API tokens.
report "bare UUID (possible token)" \
  "$(echo "$staged" | grep -inE '=[[:space:]]*[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}')"

# --- Layer 2: project term rules, loaded from outside this repo -------------

if [ ! -r "$PATTERN_FILE" ]; then
  echo "" >&2
  echo "  pre-commit: term rules INACTIVE, no readable pattern file" >&2
  echo "  Generic credential rules still ran. Restore the file to re-enable" >&2
  echo "  the project term rules; see ~/DOTFILES-GL.md." >&2
  echo "" >&2
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
    term_staged=$(echo "$term_scope" | tr '\n' '\0' \
      | xargs -0 git diff --cached --no-color -U0 -- 2>/dev/null | grep '^+' | grep -v '^+++')

    while IFS= read -r pattern; do
      case "$pattern" in
        ''|\#*) continue ;;
      esac
      report "project term" "$(echo "$term_staged" | grep -inE "$pattern")"
    done < "$PATTERN_FILE"
  fi
fi

if [ "$fail" -ne 0 ]; then
  echo "" >&2
  echo "COMMIT BLOCKED: this repo is PUBLIC and the staged content looks internal." >&2
  echo "" >&2
  echo "  Fix by one of:" >&2
  echo "    - Genericize the wording, then re-stage." >&2
  echo "    - Move the specifics to ~/.claude/local/ and read them at runtime." >&2
  echo "    - Track the file in the other repo instead (see ~/DOTFILES-GL.md)." >&2
  echo "" >&2
  echo "  If this is a verified false positive:" >&2
  echo "    SKIP_LEAK_CHECK=1 config commit ..." >&2
  echo "" >&2
  exit 1
fi

exit 0
