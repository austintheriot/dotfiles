#!/bin/bash
#
# Tests that file paths cited in this repo's own documentation resolve.
#
# deps-docs.test.sh makes this assertion for the two dependency-checking
# READMEs. It cannot catch a dead path anywhere else, which is how
# docs/research/dotfiles-management-landscape.md kept a citation to a design
# doc for a full commit after that doc was deleted.
#
# Scope is deliberately narrow: only documentation that describes THIS repo.
# The agent and rules files under .claude/ are excluded on purpose. They cite
# paths as illustrations of what to look for in whatever project the agent is
# reviewing (`CONTRIBUTING.md`, `Cargo.toml`, `docs/api.md`), and those are
# not claims that the file exists here. Asserting on them yields ~200 false
# positives and no real findings.
#
# Only backticked, repo-rooted paths with a file extension are checked. A
# bare word in backticks is prose (a command, a flag, a package name), and a
# URL is somebody else's problem.
#
# Usage: ~/tests/doc-links.test.sh

. "$(dirname "$0")/lib.sh"

cd "$DOTFILES_ROOT" || exit 1

# The docs that describe this repo, enumerated rather than globbed. A glob
# would silently pull in every future .claude/ agent file and reintroduce the
# false positives this scope exists to avoid.
docs=''
for candidate in README.md DOTFILES.md .my-scripts/deps/README.md \
        .claude/rules/dotfiles-tests.md docs/research/*.md; do
    [ -f "$candidate" ] && docs="$docs$candidate
"
done

assert_succeeds 'found at least one Markdown file to check' test -n "$docs"

dead=''
while IFS= read -r doc; do
    [ -n "$doc" ] || continue
    [ -f "$doc" ] || continue
    while IFS= read -r cited; do
        [ -n "$cited" ] || continue
        [ -e "$DOTFILES_ROOT/$cited" ] && continue
        # A path may also be cited relative to the citing file's directory.
        [ -e "$(dirname "$doc")/$cited" ] && continue
        # A bare filename with no directory (`lib.sh`, `tmux.conf`) is a
        # reference to a file the reader is expected to locate, not a path
        # assertion. Accept it if a file by that name exists anywhere in the
        # repo; only flag it when nothing by that name is tracked at all.
        case "$cited" in
            */*) ;;
            *)
                if find "$DOTFILES_ROOT/.my-scripts" "$DOTFILES_ROOT/tests" \
                        "$DOTFILES_ROOT/.config" "$DOTFILES_ROOT/.claude" \
                        -name "$cited" -print -quit 2>/dev/null | grep -q .; then
                    continue
                fi
                ;;
        esac
        dead="$dead $doc:$cited"
    done <<< "$(grep -oE '`[^`]+`' "$doc" 2>/dev/null \
        | tr -d '`' \
        | grep -E '^(\.|~/)?[A-Za-z0-9._/-]+$' \
        | grep -E '\.(sh|conf|yml|yaml|md|toml|py)$' \
        | sed -e 's|^~/||' -e 's|^\./||' \
        | sort -u)"
done <<< "$docs"

assert_equals 'every path cited in tracked Markdown resolves' '' "$dead"

finish
