# shellcheck shell=sh
# Names the platform this machine is, and where a file's platform variant
# lives. Sourced, not executed: callers need $DOTFILES_PLATFORM set in their
# own shell.
#
# Every config file with a platform-specific part (.zshrc, tmux.conf,
# alacritty.toml, deps.conf) is a shared base file plus a `-mac` / `-linux`
# variant beside it. Both variants ship on both branches, so the drift check
# covers them too; the base file loads whichever this returns.
#
# Written in POSIX sh because the callers are zsh (.zshrc), sh (check-deps.sh)
# and tmux's own shell-command hooks.

# The override comes first so one machine can exercise both branches' variant
# files. A value that names no known platform is discarded rather than
# trusted: it would otherwise select a variant file that does not exist, and
# every config would silently fall back to bare shared behavior.
case "${DOTFILES_PLATFORM:-}" in
    mac|linux) ;;
    *)
        case "$(uname -s 2>/dev/null)" in
            Darwin) DOTFILES_PLATFORM=mac ;;
            Linux) DOTFILES_PLATFORM=linux ;;
            # Named rather than guessed. A BSD that fell through to "mac"
            # would fail later inside a Homebrew path instead of here, where
            # the assumption is actually made.
            *) DOTFILES_PLATFORM=unknown ;;
        esac
        ;;
esac
export DOTFILES_PLATFORM

# Prints the platform variant path for a config file: the convention
# (base.ext -> base-<platform>.ext) lives here alone, so no call site is free
# to spell it differently.
platform_variant() {
    platform_variant_path=$1
    platform_variant_dir=${platform_variant_path%/*}
    [ "$platform_variant_dir" = "$platform_variant_path" ] && platform_variant_dir=''
    platform_variant_base=${platform_variant_path##*/}

    # A leading dot is part of the name (".zshrc"), not an extension
    # separator, so it is held aside before looking for a real one.
    platform_variant_lead=''
    case $platform_variant_base in
        .*) platform_variant_lead='.'; platform_variant_base=${platform_variant_base#.} ;;
    esac

    case $platform_variant_base in
        *.*)
            platform_variant_stem=${platform_variant_base%.*}
            platform_variant_ext=".${platform_variant_base##*.}"
            ;;
        *)
            platform_variant_stem=$platform_variant_base
            platform_variant_ext=''
            ;;
    esac

    printf '%s%s%s-%s%s\n' \
        "${platform_variant_dir:+$platform_variant_dir/}" \
        "$platform_variant_lead" \
        "$platform_variant_stem" \
        "$DOTFILES_PLATFORM" \
        "$platform_variant_ext"

    unset platform_variant_path platform_variant_dir platform_variant_base \
        platform_variant_lead platform_variant_stem platform_variant_ext
}

# Sources a config file's platform variant when one exists. The absent case is
# not an error: a platform with nothing extra to say carries no variant file.
platform_source_variant() {
    platform_source_target=$(platform_variant "$1")
    # shellcheck disable=SC1090  # path is computed, by design
    [ -f "$platform_source_target" ] && . "$platform_source_target"
    unset platform_source_target
}
