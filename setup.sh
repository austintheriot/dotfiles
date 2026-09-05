#!/bin/sh
# Bootstraps this dotfiles setup onto a machine that has nothing but git.
#
# usage: setup.sh [-y|--yes] [-n|--dry-run] [-b|--branch <name>]
#                 [-r|--repo <url>]
#
# Clones the bare repo to ~/.cfg, picks the branch for this machine, checks
# the worktree out into $HOME, then hands off to `config init`, which does
# every post-clone step. This half is the part that cannot live inside the
# repo it is cloning, and nothing more.
#
# The remote-server one-liner:
#
#   curl -fsSL https://raw.githubusercontent.com/austintheriot/dotfiles/mac/setup.sh | sh -s -- --yes
#
# On a machine that is already cloned, run `config init` instead. This script
# refuses to touch an existing ~/.cfg.
#
# Options:
#   -y, --yes          Do not prompt: accept the detected branch and install
#                      dependencies unattended. For CI, Docker, and a remote
#                      box with no one at the terminal.
#   -n, --dry-run      Print every step and change nothing.
#   -b, --branch NAME  Check out NAME instead of the detected branch. The
#                      only way to reach a branch uname cannot imply, such as
#                      `work` or `home`.
#   -r, --repo URL     Clone from URL instead of the default remote. Also
#                      accepts a local path, which is what the tests use.
#
# ---
# Written in POSIX sh, and it must stay that way: it is fetched by curl and
# piped to `sh`, so on Debian and Ubuntu it runs under dash. A bashism here
# fails on exactly the remote-server case the script exists for, and
# setup.test.sh runs `dash -n` to catch one.
#
# Deliberately not a wrapper that grows: every step after the checkout lives
# in `config init`, beside the other config-<sub> scripts, where it is
# reachable on an already-cloned machine and covered by that suite. The split
# is the chicken-and-egg boundary -- this file is what you need *before* the
# repo exists, and nothing else belongs here.
set -eu

DEFAULT_REPO='https://github.com/austintheriot/dotfiles.git'

print_usage() {
    sed -n '/^# usage:/,/^[^#]/p' "$0" \
        | sed -e '/^[^#]/d' \
        | awk '/^# ---/ { exit } { print }' \
        | sed -e 's/^# \{0,1\}//'
}

case ${1:-} in
    --help|-h) print_usage; exit 0 ;;
esac

yes=0
dry_run=0
branch=''
repo=$DEFAULT_REPO

while [ "$#" -gt 0 ]; do
    case $1 in
        -y|--yes) yes=1 ;;
        -n|--dry-run) dry_run=1 ;;
        -b|--branch)
            if [ "$#" -lt 2 ]; then
                printf 'setup.sh: --branch needs a name\n' >&2
                exit 2
            fi
            branch=$2
            shift
            ;;
        -r|--repo)
            if [ "$#" -lt 2 ]; then
                printf 'setup.sh: --repo needs a URL\n' >&2
                exit 2
            fi
            repo=$2
            shift
            ;;
        *)
            printf 'setup.sh: unknown argument: %s\n' "$1" >&2
            printf 'setup.sh: run `setup.sh --help` for usage\n' >&2
            exit 2
            ;;
    esac
    shift
done

git_dir="$HOME/.cfg"

# `git` is the one thing this script cannot install, because installing
# anything is what the repo it has not cloned yet knows how to do.
if ! command -v git >/dev/null 2>&1; then
    printf 'setup.sh: git is not installed, and it is the one prerequisite\n' >&2
    printf 'setup.sh: install git, then run this again\n' >&2
    exit 1
fi

# Refuse rather than re-clone. ~/.cfg *is* the repository: an unpushed commit
# lives nowhere else, so a script that clobbers it to look idempotent can
# destroy work that has no other copy. `config init` is the idempotent half,
# and it is what the reader wants here.
if [ -d "$git_dir" ]; then
    printf 'setup.sh: %s already exists, so this machine is already cloned\n' "$git_dir" >&2
    printf 'setup.sh: run `config init` to finish or re-run the setup steps\n' >&2
    exit 1
fi

# Branch selection. Detection names the platform; the branch is the reader's
# to confirm, because `work` and `home` are real branches that no amount of
# uname will imply. So the detected value is offered as a default rather than
# taken silently, and Enter accepts it. Checking out the wrong branch on a
# fresh machine is tedious to undo, and one keypress is cheaper than that.
if [ -z "$branch" ]; then
    platform='unknown'
    if [ -f "$HOME/.scripts/platform.sh" ]; then
        # An already-cloned machine has the real helper. Prefer it, so the
        # branch-naming convention has one definition.
        # shellcheck source=.scripts/platform.sh
        . "$HOME/.scripts/platform.sh"
        platform=$DOTFILES_PLATFORM
    else
        case "${DOTFILES_PLATFORM:-}" in
            mac|linux) platform=$DOTFILES_PLATFORM ;;
            # An explicitly exported value is a decision, including the
            # decision that detection failed. Re-deriving from uname here
            # would override the caller and, in the container suites, check
            # out a branch for the host rather than the container.
            unknown) platform=unknown ;;
            *)
                # The same mapping platform.sh makes, spelled here because on
                # a fresh machine that file does not exist yet. A platform
                # uname cannot name is reported, never guessed: defaulting an
                # unrecognized system to `mac` would check out Homebrew paths
                # onto a machine that has no Homebrew.
                case "$(uname -s 2>/dev/null)" in
                    Darwin) platform=mac ;;
                    Linux) platform=linux ;;
                    *) platform=unknown ;;
                esac
                ;;
        esac
    fi

    if [ "$platform" = 'unknown' ]; then
        printf 'setup.sh: cannot tell which branch this machine wants (uname -s: %s)\n' \
            "$(uname -s 2>/dev/null || printf 'unavailable')" >&2
        printf 'setup.sh: pass --branch <name> to choose one\n' >&2
        exit 1
    fi

    branch=$platform

    # Offer it rather than take it. A prompt is skipped under --yes, and
    # skipped when there is no terminal to answer it -- a curl-piped run has
    # stdin bound to the pipe, so reading from it would consume the script.
    if [ "$yes" -eq 0 ] && [ "$dry_run" -eq 0 ] && [ -t 0 ]; then
        printf 'setup.sh: detected %s. Branch to check out [%s]: ' "$platform" "$branch"
        read -r reply
        [ -n "$reply" ] && branch=$reply
    fi
fi

run() {
    if [ "$dry_run" -eq 1 ]; then
        printf 'setup.sh: would run: %s\n' "$*"
    else
        "$@"
    fi
}

printf 'setup.sh: branch %s, from %s\n' "$branch" "$repo"

run git clone --bare "$repo" "$git_dir"

if [ "$dry_run" -eq 1 ]; then
    printf 'setup.sh: would check out %s into %s\n' "$branch" "$HOME"
    printf 'setup.sh: would run: config init%s\n' "$([ "$yes" -eq 1 ] && printf ' --yes')"
    printf '\nsetup.sh: dry run, nothing was changed\n'
    exit 0
fi

# The worktree is $HOME, so every git call needs both directories named.
# There is no `config` on PATH yet: putting it there is a step in
# `config init`, which cannot run until this checkout has delivered it.
cfg() {
    git --git-dir="$git_dir" --work-tree="$HOME" "$@"
}

# A fresh machine almost always already has a .zshrc, and `git checkout`
# refuses to overwrite an untracked file rather than destroying it. That
# refusal is correct and it is also the single most likely way this script
# fails, so handle it: move each colliding file aside, keep it, and retry.
#
# The list comes from git's own porcelain rather than a guess about which
# files tend to exist, so it covers whatever this branch actually tracks.
if ! cfg checkout "$branch" 2>/dev/null; then
    backup_dir="$HOME/.dotfiles-backup-$(date +%Y%m%d%H%M%S)"
    moved=0

    # `checkout` names each blocking path on stderr, but parsing English is
    # fragile. Ask git for the tracked paths instead and move the ones that
    # exist and are untracked.
    for path in $(cfg ls-tree -r --name-only "$branch"); do
        [ -e "$HOME/$path" ] || continue
        mkdir -p "$backup_dir/$(dirname "$path")"
        mv "$HOME/$path" "$backup_dir/$path"
        printf 'setup.sh: moved aside %s\n' "$path"
        moved=$((moved + 1))
    done

    if [ "$moved" -gt 0 ]; then
        printf 'setup.sh: %d pre-existing file(s) kept in %s\n' "$moved" "$backup_dir"
    fi

    # Second attempt, and this one is allowed to fail loudly: if it still
    # cannot check out, the cause is not a collision and guessing further
    # would only bury the real message.
    cfg checkout "$branch"
fi

# Hand off. Everything from here lives in the repo that now exists, which is
# the whole point of the split.
init="$HOME/.scripts/config/config-init"
if [ ! -x "$init" ]; then
    printf 'setup.sh: checked out, but %s is missing\n' "$init" >&2
    printf 'setup.sh: the clone succeeded -- finish by hand or check the branch\n' >&2
    exit 1
fi

if [ "$yes" -eq 1 ]; then
    exec "$init" --yes
fi
exec "$init"
