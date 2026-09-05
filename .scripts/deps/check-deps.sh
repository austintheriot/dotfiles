#!/bin/sh
#
# Checks whether the CLI dependencies listed in deps.conf (and, if present,
# this platform's deps-<platform>.conf) are installed, and optionally
# installs the missing ones.
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
# script) and, if it exists, $DEPS_LOCAL_CONF -- the platform-exclusive
# dependencies that don't belong in the shared, cross-branch-identical
# deps.conf. That defaults to deps-mac.conf or deps-linux.conf beside this
# script, chosen by ~/.scripts/platform.sh. Both variants ship on both
# branches, so the drift check covers them; only the selection differs per
# machine. See README.md in this directory for the manifest format and how to
# add a new dependency.
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

# platform.sh lives one directory up and defines platform_variant, which
# spells the deps.conf -> deps-<platform>.conf convention in one place.
#
# Sourced only when the caller has not already chosen a variant, and only
# when it is actually there. This script is run from stripped environments --
# the Docker images set DEPS_LOCAL_CONF explicitly, and a test harness may
# invoke it with a PATH that has no `dirname`, which leaves SCRIPT_DIR empty.
# Neither case needs platform detection, and neither should fail because of
# it.
if [ -z "${DEPS_LOCAL_CONF:-}" ]; then
    if [ -f "$SCRIPT_DIR/../platform.sh" ]; then
        # shellcheck source=../platform.sh
        . "$SCRIPT_DIR/../platform.sh"
        DEPS_LOCAL_CONF=$(platform_variant "$SCRIPT_DIR/deps.conf")
    else
        # No platform helper reachable: the shared manifest is the whole
        # check. read_entries() skips a file that is not there.
        DEPS_LOCAL_CONF=''
    fi
fi

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
only=''
while [ "$#" -gt 0 ]; do
    case "$1" in
        --fix) fix=1 ;;
        --yes) yes=1 ;;
        --dry-run) dry_run=1 ;;
        --only)
            # Takes a value, so this loop shifts rather than iterating "$@".
            # Without the arity check, `--only` with nothing after it would
            # select the empty set and report a vacuous success -- a workflow
            # step that installed none of what it promised and passed.
            if [ "$#" -lt 2 ]; then
                printf 'check-deps: --only needs a comma-separated list of names\n' >&2
                exit 2
            fi
            only=$2
            shift
            ;;
        *)
            printf 'check-deps: unknown argument: %s\n' "$1" >&2
            exit 2
            ;;
    esac
    shift
done

# Restricts the run to a named subset of the manifest.
#
# This exists so .github/workflows/test-suite.yml can name the dependencies
# the suite needs instead of hand-maintaining an apt list and a brew list
# that restate deps.conf. Two copies of the package names meant the copy in
# YAML was the one that drifted, and nothing checked it.
#
# A name matching no entry is a typo in a caller, and it exits 2 rather than
# selecting nothing: an install step that quietly installs none of what it
# promised still reports success, which is worse than a hard failure.
in_only_set() {
    in_only_name=$1
    in_only_rest=$only
    while [ -n "$in_only_rest" ]; do
        in_only_head=${in_only_rest%%,*}
        if [ "$in_only_head" = "$in_only_name" ]; then
            unset in_only_name in_only_rest in_only_head
            return 0
        fi
        [ "$in_only_rest" = "$in_only_head" ] && break
        in_only_rest=${in_only_rest#*,}
    done
    unset in_only_name in_only_rest in_only_head
    return 1
}

# Privilege escalation, decided once rather than assumed by every command.
#
# A bare root container reported 11 failures in one run, every one of them
# "sh: 1: sudo: not found", because each install command carried a hardcoded
# `sudo`. Minimal images do not ship sudo, and a process already running as
# root has nothing to escalate to. The same commands run directly succeed.
#
# Four environments, three answers:
#   root, no sudo      -> run directly (the container that reported this)
#   root, sudo present -> run directly anyway; escalating from root is
#                         pointless, and on an image with a misconfigured
#                         sudo it is one more way to fail
#   non-root, sudo     -> escalate (the normal laptop and CI runner)
#   non-root, no sudo  -> no way to install, so a command needing privilege is
#                         reported manual-only rather than emitted as a
#                         command that cannot work
#
# $SUDO carries its own trailing space and the commands spell `${SUDO}apt-get`,
# so the expansion happens in the `sh -c` that runs the command and both
# states produce valid text. The space lives in the variable because
# `${SUDO} apt-get` would leave a leading space when empty, and `${SUDO}apt-get`
# without it produced `sudoapt-get` -- measured, not guessed.
#
# DEPS_FORCE_ROOT overrides the detection so both sides are testable without
# running the suite as root.
case "${DEPS_FORCE_ROOT:-}" in
    1) is_root=1 ;;
    0) is_root=0 ;;
    *) if [ "$(id -u 2>/dev/null || printf 1)" -eq 0 ]; then is_root=1; else is_root=0; fi ;;
esac

if [ "$is_root" -eq 1 ]; then
    SUDO=''
    can_escalate=1
elif command -v sudo >/dev/null 2>&1; then
    SUDO='sudo '
    can_escalate=1
else
    SUDO=''
    can_escalate=0
fi
export SUDO

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
            # The apt path writes the keyring through `sudo tee` rather than
            # `wget -O <path>`, and that is load-bearing rather than stylistic.
            #
            # `sudo mkdir -p -m 755 /etc/apt/keyrings` creates a root-owned
            # directory, and the sudo on that command does not extend to the
            # next command in the chain. A bare `wget -O
            # /etc/apt/keyrings/...` therefore fails with "Permission denied"
            # for any non-root user, and because the failure is a write error
            # rather than a non-zero install, the check for gh failed
            # immediately after its own install reported success.
            #
            # This survived because every environment that ran it before was
            # root or already had gh: the Docker deps images run as root, and
            # the GitHub runners ship gh preinstalled. The bootstrap container
            # (docker/Dockerfile.bootstrap) is the first one that is neither,
            # which is what surfaced it.
            #
            # `wget -O-` to stdout piped into `sudo tee` is the shape
            # upstream's own instructions use, and it keeps the privilege with
            # the write.
            case "$manager" in
                apt)
                    printf '%s' '${SUDO}apt-get update -qq && ${SUDO}apt-get install -y wget && ${SUDO}mkdir -p -m 755 /etc/apt/keyrings && wget -nv -O- https://cli.github.com/packages/githubcli-archive-keyring.gpg | ${SUDO}tee /etc/apt/keyrings/githubcli-archive-keyring.gpg > /dev/null && ${SUDO}chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg && echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | ${SUDO}tee /etc/apt/sources.list.d/github-cli.list > /dev/null && ${SUDO}apt-get update -qq && ${SUDO}apt-get install -y gh'
                    ;;
                brew) printf 'brew install gh' ;;
                pacman) printf '${SUDO}pacman -Sy --noconfirm github-cli' ;;
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
                apt) printf '${SUDO}apt-get update -qq && ${SUDO}apt-get install -y alacritty' ;;
                pacman) printf '${SUDO}pacman -Sy --noconfirm alacritty' ;;
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
            # Package manager first, installer only as a fallback.
            #
            # zoxide's install.sh resolves the latest release through
            # api.github.com/repos/.../releases/latest with no credentials.
            # That quota is 60 requests an hour per IP and every GitHub
            # Actions runner on a given IP shares it, so the deps-check
            # workflow failed with "you have exceeded GitHub's API rate
            # limit" on a run that had nothing to do with zoxide.
            #
            # Caching the binary would not have helped: a cache miss still
            # calls the API, and the installer takes no token. Both apt and
            # brew package zoxide, so on the platforms CI runs, the API is
            # simply not needed. The installer stays for a manager that has
            # no package for it.
            case "$manager" in
                apt) printf '${SUDO}apt-get update -qq && ${SUDO}apt-get install -y zoxide' ;;
                brew) printf 'brew install zoxide' ;;
                # Arch ships zoxide in `extra`, so pacman had no business
                # falling through to the installer. It did, and the arch leg
                # of deps-check.yml failed with "you have exceeded GitHub's
                # API rate limit" on a run that had nothing to do with
                # zoxide -- the same failure the apt and brew cases above
                # were added to prevent.
                pacman) printf '${SUDO}pacman -Sy --noconfirm zoxide' ;;
                *) printf 'curl -sS https://raw.githubusercontent.com/ajeetdsouza/zoxide/main/install.sh | sh' ;;
            esac
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
        pyyaml)
            # A pip package, not a system package, so the default
            # `<manager> install pyyaml` case produces a command that fails
            # everywhere: apt calls it python3-yaml, brew has no formula, and
            # pacman calls it python-yaml. Named here so the one manifest
            # entry works on every platform the suite runs on.
            #
            # apt and pacman prefer the distribution package. On a
            # Debian-derived system pip refuses to install into the
            # system interpreter at all (PEP 668, "externally-managed-
            # environment") without a flag that overrides the protection,
            # and the distro package is what the runner's python3 imports.
            #
            # --break-system-packages is the brew and fallback path. The name
            # is alarming and the flag is correct here: a CI runner and a
            # throwaway container are exactly where overriding PEP 668 costs
            # nothing, and it is what test-suite.yml already did by hand.
            case "$manager" in
                apt) printf '${SUDO}apt-get update -qq && ${SUDO}apt-get install -y python3-yaml' ;;
                pacman) printf '${SUDO}pacman -Sy --noconfirm python-yaml' ;;
                *) printf 'python3 -m pip install --break-system-packages pyyaml' ;;
            esac
            ;;
        dash)
            # Not packaged by Homebrew under this name, and macOS ships no
            # dash at all. The suite's one dash assertion skips when it is
            # absent, so reporting it manual-only on brew is the honest
            # answer rather than installing something else and calling it
            # dash.
            case "$manager" in
                apt) printf '${SUDO}apt-get update -qq && ${SUDO}apt-get install -y dash' ;;
                pacman) printf '${SUDO}pacman -Sy --noconfirm dash' ;;
            esac
            ;;
        python3)
            # apt and pacman spell the package differently from the binary,
            # and macOS runners ship python3 already.
            case "$manager" in
                apt) printf '${SUDO}apt-get update -qq && ${SUDO}apt-get install -y python3' ;;
                pacman) printf '${SUDO}pacman -Sy --noconfirm python' ;;
                brew) printf 'brew install python' ;;
            esac
            ;;
        *)
            case "$manager" in
                apt) printf '${SUDO}apt-get update -qq && ${SUDO}apt-get install -y %s' "$name" ;;
                brew) printf 'brew install %s' "$name" ;;
                pacman) printf '${SUDO}pacman -Sy --noconfirm %s' "$name" ;;
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

if [ -n "$only" ]; then
    selected=''
    old_ifs=$IFS
    IFS='
'
    for entry in $entries; do
        IFS=$old_ifs
        if in_only_set "${entry%%|*}"; then
            selected="${selected:+$selected
}$entry"
        fi
        IFS='
'
    done
    IFS=$old_ifs

    # Every selector must have matched. Checked per name rather than by
    # counting, so the error names the one that is wrong.
    unmatched=''
    rest=$only
    while [ -n "$rest" ]; do
        head=${rest%%,*}
        if [ -n "$head" ]; then
            found=0
            old_ifs=$IFS
            IFS='
'
            for entry in $selected; do
                IFS=$old_ifs
                [ "${entry%%|*}" = "$head" ] && found=1
                IFS='
'
            done
            IFS=$old_ifs
            [ "$found" -eq 1 ] || unmatched="$unmatched $head"
        fi
        [ "$rest" = "$head" ] && break
        rest=${rest#*,}
    done

    if [ -n "$unmatched" ]; then
        printf 'check-deps: --only named no such dependency:%s\n' "$unmatched" >&2
        printf 'check-deps: known names come from %s and %s\n' \
            "$DEPS_CONF" "${DEPS_LOCAL_CONF:-(no platform variant)}" >&2
        exit 2
    fi

    entries=$selected
fi

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

        # A command that needs root cannot run on a machine with no way to
        # become root. Emitting it anyway is what produced "sh: 1: sudo: not
        # found" eleven times in a single run.
        #
        # Keyed on the command containing ${SUDO} rather than on a list of
        # dependency names, because a list drifts: brew never needs root, a
        # git clone into $HOME never needs root, and a new entry would have to
        # remember to join the list. The command itself already says whether
        # it needs privilege.
        if [ "$can_escalate" -eq 0 ] && [ "${cmd#*\$\{SUDO\}}" != "$cmd" ]; then
            printf '  %s needs root, and this machine has neither root nor sudo -- see %s\n' \
                "$name" "$docs"
            IFS='
'
            continue
        fi

        # ${SUDO} is substituted here rather than eval'd. An eval over
        # command text is both fragile and an injection shape, and nothing
        # here needs it: the only placeholder is ${SUDO}, so a plain
        # substitution is exact.
        #
        # Done for display as well as for running, so --dry-run prints a
        # command the reader can copy instead of a variable they have to
        # resolve themselves.
        while :; do
            case "$cmd" in
                *'${SUDO}'*) cmd="${cmd%%'${SUDO}'*}$SUDO${cmd#*'${SUDO}'}" ;;
                *) break ;;
            esac
        done

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
