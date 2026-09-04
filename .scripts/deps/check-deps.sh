#!/bin/sh
#
# Checks whether the CLI dependencies listed in deps.conf (and, if present,
# deps-local.conf) are installed, and optionally installs the missing ones.
#
# Usage:
#   check-deps.sh                 check only, exit 1 if anything is missing
#   check-deps.sh --fix           check, then interactively offer to install
#                                  each missing dependency
#   check-deps.sh --fix --yes     check, then install every missing
#                                  dependency without prompting (CI/Docker)
#   check-deps.sh --fix --dry-run print what --fix would run, without
#                                  running it (always exits 0)
#
# Reads dependencies from $DEPS_CONF (default: deps.conf next to this
# script) and, if it exists, $DEPS_LOCAL_CONF (default: deps-local.conf next
# to this script) -- the platform-exclusive dependencies that don't belong in
# the shared, cross-branch-identical deps.conf. See README.md in this
# directory for the manifest format and how to add a new dependency.
#
# Exit code:
#   without --fix: non-zero if anything is missing (informational -- used by
#     the shell-startup hook and a plain manual run).
#   with --fix (and not --dry-run): non-zero only if a dependency that had an
#     automated install command attempted still fails its check afterward.
#   Dependencies with no automated install path ("manual" in the output) are
#   reported but never make --fix's exit code non-zero -- there's nothing
#   --fix could have done differently. A user who declines an interactive
#   prompt is treated the same way: an informed choice, not a failure.

set -u

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
DEPS_CONF=${DEPS_CONF:-"$SCRIPT_DIR/deps.conf"}
DEPS_LOCAL_CONF=${DEPS_LOCAL_CONF:-"$SCRIPT_DIR/deps-local.conf"}

# The curl-to-shell installers below write outside a default non-login PATH:
# zoxide to ~/.local/bin, rustup to ~/.cargo/bin. Without these, a `command
# -v` check fails on the line after its own install succeeded, so --fix can
# never converge and the CI exit code stops meaning anything. An interactive
# shell usually exports these already, which is what hides the bug on a
# developer machine.
PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
export PATH

fix=0
yes=0
dry_run=0
for arg in "$@"; do
    case "$arg" in
        --fix) fix=1 ;;
        --yes) yes=1 ;;
        --dry-run) dry_run=1 ;;
        *)
            printf 'check-deps: unknown argument: %s\n' "$arg" >&2
            exit 2
            ;;
    esac
done

detect_pm() {
    if command -v pacman >/dev/null 2>&1; then
        printf 'pacman'
    elif command -v apt-get >/dev/null 2>&1; then
        printf 'apt'
    elif command -v brew >/dev/null 2>&1; then
        printf 'brew'
    else
        printf 'unknown'
    fi
}

# Returns (via stdout) the install command for dependency $1 under package
# manager $2, or nothing if there's no automated install for that
# combination. Anything not listed here falls through to the default case,
# which assumes the package name matches the dependency name -- true for
# most of deps.conf (git, zsh, tmux, fzf, ripgrep, xclip).
install_cmd_for() {
    name=$1
    manager=$2
    case "$name" in
        gh)
            case "$manager" in
                apt)
                    printf '%s' 'sudo apt-get update -qq && sudo apt-get install -y wget && sudo mkdir -p -m 755 /etc/apt/keyrings && wget -nv -O /etc/apt/keyrings/githubcli-archive-keyring.gpg https://cli.github.com/packages/githubcli-archive-keyring.gpg && sudo chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg && echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | sudo tee /etc/apt/sources.list.d/github-cli.list > /dev/null && sudo apt-get update -qq && sudo apt-get install -y gh'
                    ;;
                brew) printf 'brew install gh' ;;
                pacman) printf 'sudo pacman -Sy --noconfirm github-cli' ;;
            esac
            ;;
        alacritty)
            # No brew case on purpose, and no .dmg fallback either.
            #
            # Homebrew disabled the alacritty cask on 2026-09-01 -- "does not
            # pass the macOS Gatekeeper check" -- so `brew install --cask
            # alacritty` now fails on every Mac.
            #
            # Downloading the release .dmg instead does not rescue it. The
            # app Alacritty ships is adhoc-signed with no Team ID, and
            # `spctl -a` rejects it, which is the very reason the cask was
            # disabled. Installing it would leave an app macOS refuses to
            # launch until someone approves it by hand, and that approval is
            # interactive by design. INSTALL.md documents only a source
            # build.
            #
            # So macOS has no automated path at all. Reporting it as
            # manual-only with the docs URL keeps one upstream disablement
            # from failing the whole bootstrap. Revisit if the cask is
            # re-enabled or upstream starts notarizing.
            case "$manager" in
                apt) printf 'sudo apt-get update -qq && sudo apt-get install -y alacritty' ;;
                pacman) printf 'sudo pacman -Sy --noconfirm alacritty' ;;
            esac
            ;;
        aerospace)
            # A cask, not a formula, and one that lives in a third-party tap.
            # The default `brew install <name>` fails twice over: "No
            # available formula with the name aerospace" because it is a cask,
            # and an untapped third-party cask is not findable even with
            # --cask. `brew tap` is idempotent, so re-running costs a no-op.
            #
            # macOS-only. Every non-brew manager reports it as manual-only
            # rather than guessing, because there is no Linux build.
            #
            # Tapping alone is not enough: current Homebrew refuses to load a
            # cask from an untrusted tap. `brew trust` is a recent
            # subcommand, so its failure is tolerated for an older Homebrew
            # that does not have it and does not need it.
            case "$manager" in
                brew) printf 'brew tap nikitabobko/tap && { brew trust nikitabobko/tap || true; } && brew install --cask aerospace' ;;
            esac
            ;;
        zoxide)
            printf 'curl -sS https://raw.githubusercontent.com/ajeetdsouza/zoxide/main/install.sh | sh'
            ;;
        oh-my-zsh)
            printf 'sh -c "$(curl -fsSL https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh)" "" --unattended'
            ;;
        zsh-autosuggestions)
            # The oh-my-zsh custom-plugin clone only satisfies the check on a
            # machine that actually has oh-my-zsh. On brew the plugin comes
            # from its own formula, and cloning into a nonexistent
            # ~/.oh-my-zsh would leave a directory nothing ever sources.
            # `git clone` creates its parent directories, so cloning on a
            # machine with no oh-my-zsh lands the plugin somewhere nothing
            # sources -- and the check then reports success for an install
            # that will never load. Report it as manual-only instead.
            case "$manager" in
                brew) printf 'brew install zsh-autosuggestions' ;;
                *)
                    if [ -d "${ZSH_CUSTOM:-$HOME/.oh-my-zsh/custom}" ]; then
                        printf 'git clone https://github.com/zsh-users/zsh-autosuggestions "${ZSH_CUSTOM:-$HOME/.oh-my-zsh/custom}/plugins/zsh-autosuggestions"'
                    fi
                    ;;
            esac
            ;;
        tpm)
            printf 'git clone https://github.com/tmux-plugins/tpm "$HOME/.tmux/plugins/tpm"'
            ;;
        rustup)
            printf "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
            ;;
        nvm)
            # Deliberately no automated install: nvm's own docs only publish
            # version-pinned install URLs, so a hardcoded one here would go
            # stale. Manual only -- see the docs_url column.
            ;;
        *)
            case "$manager" in
                apt) printf 'sudo apt-get update -qq && sudo apt-get install -y %s' "$name" ;;
                brew) printf 'brew install %s' "$name" ;;
                pacman) printf 'sudo pacman -Sy --noconfirm %s' "$name" ;;
            esac
            ;;
    esac
}

read_entries() {
    file=$1
    [ -f "$file" ] || return 0
    while IFS='|' read -r name check docs || [ -n "$name" ]; do
        case "$name" in
            ''|'#'*) continue ;;
        esac
        printf '%s|%s|%s\n' "$name" "$check" "$docs"
    done < "$file"
}

pm=$(detect_pm)
missing_count=0
checked_count=0
failed_fix_count=0

entries=$( { read_entries "$DEPS_CONF"; read_entries "$DEPS_LOCAL_CONF"; } )

old_ifs=$IFS
IFS='
'
for entry in $entries; do
    IFS=$old_ifs
    name=${entry%%|*}
    rest=${entry#*|}
    check=${rest%%|*}
    docs=${rest#*|}

    checked_count=$((checked_count + 1))

    if sh -c "$check" >/dev/null 2>&1; then
        IFS='
'
        continue
    fi

    missing_count=$((missing_count + 1))
    printf 'missing: %s\n' "$name"

    if [ "$fix" -eq 1 ]; then
        cmd=$(install_cmd_for "$name" "$pm")
        if [ -z "$cmd" ]; then
            printf '  no automated install for %s on this system -- see %s\n' "$name" "$docs"
        elif [ "$dry_run" -eq 1 ]; then
            printf '  would run: %s\n' "$cmd"
        else
            proceed=0
            if [ "$yes" -eq 1 ]; then
                proceed=1
            else
                printf '  install %s? [y/N] ' "$name"
                read -r reply
                case "$reply" in
                    y|Y) proceed=1 ;;
                esac
            fi

            if [ "$proceed" -eq 1 ]; then
                sh -c "$cmd"
                if sh -c "$check" >/dev/null 2>&1; then
                    printf '  installed %s\n' "$name"
                else
                    printf '  install did not satisfy the check for %s\n' "$name"
                    failed_fix_count=$((failed_fix_count + 1))
                fi
            fi
        fi
    fi
    IFS='
'
done
IFS=$old_ifs

if [ "$dry_run" -eq 1 ]; then
    printf '\ncheck-deps: dry run, %d of %d dependencies missing\n' "$missing_count" "$checked_count"
    exit 0
fi

if [ "$fix" -eq 1 ]; then
    if [ "$failed_fix_count" -gt 0 ]; then
        printf '\ncheck-deps: %d automated install(s) did not satisfy their check\n' "$failed_fix_count" >&2
        exit 1
    fi
    printf '\ncheck-deps: no unresolved failures (%d of %d were already missing)\n' "$missing_count" "$checked_count"
    exit 0
fi

if [ "$missing_count" -gt 0 ]; then
    printf '\ncheck-deps: %d of %d dependencies missing\n' "$missing_count" "$checked_count" >&2
    exit 1
fi

printf 'check-deps: all %d dependencies present\n' "$checked_count"
exit 0
